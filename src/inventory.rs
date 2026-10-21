//! Inventory and equipment (segment 1194, around 7449..7C6D).
//!
//! A character record holds 40 item slots of 19 bytes from +6E (the
//! player's start at DGROUP 0212). Item fields are those of
//! `combat::item`/the loot item record; the ones used here:
//!
//!   +00 type: weapon 0..17 or armour 0..10; bit 0x80 marks a special
//!       (magic) item whose base type comes from DGROUP 36DD
//!   +04 armour rating / min damage (signed byte here)
//!   +06 health, +08 max health (words)
//!   +0E flags: 0x01 enchanted, 0x0C category, 0x30 armour kind,
//!       0x40 slot occupied, 0x80 equipped
//!   +0F/+11/+12 enchantment, its value and the attribute it affects
//!
//! Item category (flags & 0x0C) >> 2: 0 armour, 1 weapon, 2 accessory,
//! 3 stack (a counted item: +01 is the count).
//!
//! Routines take the record's segment because the original works through
//! whatever DS the caller set. Several of them count occupied slots and
//! then walk that many *consecutive* slots, so items after a gap are
//! missed; that is kept.

use crate::character::field;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// Record offset of the first item slot.
pub const SLOTS: u16 = 0x6E;
pub const SLOT_COUNT: u16 = 40;
pub const SLOT_SIZE: u16 = item::SIZE;
/// DGROUP offset of the player's first item slot.
pub const PLAYER_SLOTS: u16 = 0x0212;

/// Item record fields (the shared layout).
pub use crate::item;

/// Item categories, `(flags & 0x0C) >> 2`.
pub mod category {
    pub const ARMOR: u16 = 0;
    pub const WEAPON: u16 = 1;
    pub const ACCESSORY: u16 = 2;
    pub const STACK: u16 = 3;
}

/// DGROUP tables and variables.
const MAGIC_BASE_TYPE: u16 = 0x36DD;
const SHIELD_COVERAGE: u16 = 0x5DC5;
const CONDITION_STEPS: u16 = 0x5DEA;
const CONDITION_NAMES: u16 = 0x5DF1;
/// Byte: occupied-slot count left by the last equipment lookup.
pub const LAST_COUNT: u16 = 0x129D;
/// Word, in the caller's DS: 1 if the wielder has no `MISSILE_TYPES`
/// weapon, set by `weapon_bonus`.
const NOT_MISSILE: u16 = 0x12B8;
/// Player armour class per location (7 bytes), from `player_armor_class`.
pub const ARMOR_CLASS: u16 = 0x942E;

/// Item name lists (DGROUP offsets; the strings lie in segment 3276's
/// tables past DGROUP's start) and the formatted-name buffer.
const WEAPON_NAMES: u16 = 0x2204;
const ARMOR_LISTS: u16 = 0x5D7C;
const ARMOR_PARTS: u16 = 0x2424;
const ARMOR_SUFFIXES: u16 = 0x254D;
const WEAPON_SUFFIXES: u16 = 0x231F;
const MATERIALS: u16 = 0x263F;
const POTION: u16 = 0x5E30;
const POTION_NAMES: u16 = 0x28CD;
const RING_NAMES: u16 = 0x1DD1;
const RING_SUFFIXES_0: u16 = 0x1E03;
const RING_SUFFIXES_1: u16 = 0x1F0A;
const RING_SUFFIXES_2: u16 = 0x1F9F;
const BRACELET_NAMES: u16 = 0x2028;
const ATTRIBUTE_SUFFIXES: u16 = 0x2052;
pub const NAME_BUF: u16 = 0x1395;

/// Tables in segment 3276.
const MISSILE_TYPES: u16 = 0x6CC0;
const CLASS_WEAPONS: u16 = 0x3C25;
const CLASS_SHIELDS: u16 = 0x3C71;
const CLASS_MAX_ARMOR: u16 = 0x3CA4;

impl Machine {
    fn inv_b(&self, seg: u16, off: u16, f: u16) -> u8 {
        self.mem.r8(seg, off.wrapping_add(f))
    }

    pub(crate) fn inv_flags(&self, seg: u16, it: u16) -> u8 {
        self.inv_b(seg, it, item::FLAGS)
    }

    fn inv_rating(&self, seg: u16, it: u16) -> u16 {
        self.inv_b(seg, it, item::RATING) as i8 as i16 as u16
    }

    /// 1194:7472. Occupied slots among the 40 from `si`. Also returns the
    /// carry of the loop's last `add si, 13h`, which `find_equipped`
    /// passes on when the count is 0.
    pub fn count_items(&self, seg: u16, si: u16) -> (u16, bool) {
        let n = (0..SLOT_COUNT)
            .filter(|&i| self.inv_flags(seg, si.wrapping_add(i * SLOT_SIZE)) & item::OCCUPIED != 0)
            .count() as u16;
        let last = si.wrapping_add((SLOT_COUNT - 1) * SLOT_SIZE);
        (n, last as u32 + SLOT_SIZE as u32 > 0xFFFF)
    }

    /// 1194:748E.
    pub fn item_category(&self, seg: u16, it: u16) -> u16 {
        ((self.inv_flags(seg, it) & 0x0C) >> 2) as u16
    }

    /// 1194:7499.
    pub fn armor_kind(&self, seg: u16, it: u16) -> u16 {
        ((self.inv_flags(seg, it) & 0x30) >> 4) as u16
    }

    /// 1194:7596. Base type of an item type byte; special items (bit 0x80)
    /// map through DGROUP 36DD, sign-extended. The flag (CF) is set when
    /// that mapping is negative.
    pub fn base_type(&self, ty: u8) -> (u16, bool) {
        if ty & 0x80 == 0 {
            return (ty as u16, false);
        }
        let v = self.mem.r8(DS, MAGIC_BASE_TYPE + (ty & 0x7F) as u16) as i8;
        (v as i16 as u16, v < 0)
    }

    /// 1194:7502. Shields are armour base types 7..=10.
    pub fn is_shield(base: u16) -> bool {
        (7..=10).contains(&base)
    }

    /// 1194:75D3. Walks up to `cx` slots from `si` for an occupied item of
    /// base type `ty` and category `cat`. Returns (found, si, cx left).
    pub fn find_item(&self, seg: u16, mut si: u16, mut cx: u16, ty: u16, cat: u16) -> (bool, u16, u16) {
        while cx != 0 {
            if self.inv_flags(seg, si) & item::OCCUPIED != 0
                && self.base_type(self.inv_b(seg, si, item::TYPE)).0 == ty
                && self.item_category(seg, si) == cat
            {
                return (true, si, cx - 1);
            }
            si = si.wrapping_add(SLOT_SIZE);
            cx -= 1;
        }
        (false, si, 0)
    }

    /// 1194:745A. Like `find_item`, but only an equipped match counts. With
    /// `cx` 0 it returns `carry` unchanged (the caller's CF).
    pub fn find_equipped(&self, seg: u16, si: u16, cx: u16, ty: u16, cat: u16, carry: bool) -> (bool, u16) {
        let (f, si, _) = self.inv_find_equipped_cx(seg, si, cx, ty, cat, carry);
        (f, si)
    }

    /// `find_equipped`, also returning the CX it leaves (slots left to
    /// walk), which 1194:7804 continues from.
    fn inv_find_equipped_cx(&self, seg: u16, mut si: u16, mut cx: u16, ty: u16, cat: u16, carry: bool) -> (bool, u16, u16) {
        if cx == 0 {
            return (carry, si, cx);
        }
        loop {
            let (found, s, c) = self.find_item(seg, si, cx, ty, cat);
            if !found {
                return (false, s, c);
            }
            if self.inv_flags(seg, s) & item::EQUIPPED != 0 {
                return (true, s, c);
            }
            si = s.wrapping_add(SLOT_SIZE);
            cx = c;
        }
    }

    /// 1194:7449. Finds an equipped item of base type `ty` and category
    /// `cat` among the *player's* slots, walking as many slots as are
    /// occupied from `si` in `seg` (callers pass a record, not its slots).
    /// Combat uses it with (1, 0) for gauntlets.
    pub fn find_player_equipped(&self, seg: u16, si: u16, ty: u16, cat: u16) -> (bool, u16) {
        let (n, carry) = self.count_items(seg, si);
        self.find_equipped(seg, PLAYER_SLOTS, n, ty, cat, carry)
    }

    /// 1194:74A4. The equipped shield among the slots at `si` (base types
    /// 7..=10 in turn). Stores the occupied count at DGROUP 129D.
    pub fn equipped_shield(&mut self, seg: u16, si: u16) -> Option<u16> {
        let (n, _) = self.count_items(seg, si);
        self.mem.w8(DS, LAST_COUNT, n as u8);
        if n as u8 == 0 {
            return None;
        }
        for ty in 7..=10 {
            let cx = self.mem.r8(DS, LAST_COUNT) as u16;
            if let (true, s) = self.find_equipped(seg, si, cx, ty, category::ARMOR, false) {
                return Some(s);
            }
        }
        None
    }

    /// 1194:74DA. Does the equipped shield cover `location`? Its
    /// coverage table (pointer per shield type at 5DC5, read through
    /// `seg`) marks covered locations with bit 0x80 (the original returns
    /// this in SF). Also returns SI: the shield if there is one, else `si`.
    pub fn shield_at(&mut self, seg: u16, si: u16, location: u16) -> (bool, u16) {
        let Some(shield) = self.equipped_shield(seg, si) else { return (false, si) };
        let (base, special) = self.base_type(self.inv_b(seg, shield, item::TYPE));
        if special {
            return (false, shield);
        }
        let table = self.mem.r16(seg, SHIELD_COVERAGE.wrapping_add(base.wrapping_sub(7).wrapping_mul(2)));
        (self.mem.r8(seg, table.wrapping_add(location)) & 0x80 != 0, shield)
    }

    /// 1194:7B1C. Armour rating of record `rec` at hit `location`: the
    /// equipped armour piece whose base type is the location, plus a
    /// shield covering it, plus accessories (`accessory_armor`).
    pub fn armor_at(&mut self, seg: u16, rec: u16, location: u16) -> u16 {
        let slots = rec.wrapping_add(SLOTS);
        let piece = match self.find_equipped(seg, slots, SLOT_COUNT, location, category::ARMOR, false) {
            (true, s) => self.inv_rating(seg, s),
            _ => 0,
        };
        let shield = match self.shield_at(seg, slots, location) {
            (true, s) => self.inv_rating(seg, s),
            _ => 0,
        };
        piece.wrapping_add(shield).wrapping_add(self.accessory_armor(seg, slots))
    }

    /// The armour combat's to-hit roll uses (2AA1:060F): `armor_at` for
    /// the defender at the hit location in DGROUP 129A. This is
    /// `combat::Gear::armor`.
    pub fn defender_armor(&mut self, def: crate::combat::Ptr) -> i16 {
        let location = self.mem.r16(DS, crate::combat::var::LOCATION);
        self.armor_at(def.seg, def.off, location) as i16
    }

    /// 1194:7B58. Armour from accessories: equipped kind-2 accessories of
    /// base types 0..=3 (unless special), plus `special_armor`.
    fn accessory_armor(&self, seg: u16, slots: u16) -> u16 {
        let mut sum = 0u16;
        for ty in 0..4 {
            let (found, s) = self.inv_find_kind2_accessory(seg, slots, ty);
            if found && self.inv_b(seg, s, item::TYPE) & 0x80 == 0 {
                sum = sum.wrapping_add(self.inv_rating(seg, s));
            }
        }
        sum.wrapping_add(self.special_armor(seg, slots))
    }

    /// 1194:7B89. An equipped accessory of base type `ty` and armour kind 2.
    fn inv_find_kind2_accessory(&self, seg: u16, mut si: u16, ty: u16) -> (bool, u16) {
        let mut cx = SLOT_COUNT;
        loop {
            let (found, s, c) = self.find_item(seg, si, cx, ty, category::ACCESSORY);
            if !found {
                return (false, s);
            }
            cx = c;
            if self.armor_kind(seg, s) == 2 {
                if self.inv_flags(seg, s) & item::EQUIPPED != 0 {
                    return (true, s);
                }
                if cx == 0 {
                    return (false, s);
                }
            }
            si = s.wrapping_add(SLOT_SIZE);
        }
    }

    /// 1194:7BB1. 15 if special item 85h is equipped, plus the ratings of
    /// equipped special items 8Ah..8Ch.
    fn special_armor(&self, seg: u16, slots: u16) -> u16 {
        let base = if self.find_equipped_type(seg, slots, 0x85).0 { 15 } else { 0 };
        let mut sum = 0u16;
        for ty in 0x8A..=0x8C {
            if let (true, s) = self.find_equipped_type(seg, slots, ty) {
                sum = sum.wrapping_add(self.inv_rating(seg, s));
            }
        }
        base + sum
    }

    /// 1194:7C6D. An equipped item with exactly this type byte.
    pub fn find_equipped_type(&self, seg: u16, mut si: u16, ty: u8) -> (bool, u16) {
        for _ in 0..SLOT_COUNT {
            let f = self.inv_flags(seg, si);
            if f & item::EQUIPPED != 0 && self.inv_b(seg, si, item::TYPE) == ty {
                return (true, si);
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        (false, si)
    }

    /// 1194:7510. The equipped weapon of record `rec`, or 0xFFFF. Stores
    /// the occupied count at DGROUP 129D.
    pub fn equipped_weapon(&mut self, seg: u16, rec: u16) -> u16 {
        let mut si = rec.wrapping_add(SLOTS);
        let (n, _) = self.count_items(seg, si);
        self.mem.w8(DS, LAST_COUNT, n as u8);
        for _ in 0..n as u8 {
            if self.inv_flags(seg, si) & item::EQUIPPED != 0 && self.item_category(seg, si) == category::WEAPON {
                return si;
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        0xFFFF
    }

    /// 1194:78DE. Is the equipped weapon one of the three types at
    /// 3276:6CC0 (missile weapons, per combat)?
    pub fn has_missile_weapon(&mut self, seg: u16, rec: u16) -> bool {
        self.missile_weapon(seg, rec).is_some()
    }

    /// `has_missile_weapon`, returning the CX left on a find: 3 minus the
    /// type's place in the table.
    pub fn missile_weapon(&mut self, seg: u16, rec: u16) -> Option<u16> {
        let w = self.equipped_weapon(seg, rec);
        if w == 0xFFFF {
            return None;
        }
        let ty = self.mem.r8(seg, w);
        (0..3).find(|&i| self.mem.r8(DATA, MISSILE_TYPES + i) == ty).map(|i| 3 - i)
    }

    /// 1194:39ED. Racial weapon damage bonus for record `rec` in `seg`:
    /// Dark Elves level/4; Redguards level/3 with a `MISSILE_TYPES`
    /// weapon, Wood Elves level/3 without one (the reverse of what the
    /// races are known for). Creatures and other races get 0. Writes the
    /// flag word at `seg`:12B8 for Redguards and Wood Elves.
    pub fn weapon_bonus(&mut self, seg: u16, rec: u16) -> u16 {
        if self.mem.r16(seg, rec.wrapping_add(field::FLAGS)) & field::FLAG_CREATURE != 0 {
            return 0;
        }
        let race = self.mem.r8(seg, rec.wrapping_add(field::RACE));
        let level = self.mem.r8(seg, rec.wrapping_add(field::LEVEL)) as u16;
        if race == 3 {
            return level / 4;
        }
        let not_missile = !self.has_missile_weapon(seg, rec);
        self.mem.w16(seg, NOT_MISSILE, not_missile as u16);
        match (race, not_missile) {
            (1, false) | (5, true) => level / 3,
            _ => 0,
        }
    }

    /// 1194:7757. Classes whose armour limit (3276:3CA4) is 3 wear none.
    pub fn class_wears_no_armor(&self, class: u16) -> bool {
        self.mem.r8(DATA, CLASS_MAX_ARMOR + (class & 0x1F)) == 3
    }

    /// 1194:77F2. Searches the 0xFF-terminated byte list at 3276:`list`.
    /// None where the original would loop forever (neither byte anywhere
    /// in the segment).
    fn inv_in_list(&self, list: u16, v: u8) -> Option<bool> {
        let mut p = list;
        for _ in 0..0x10000u32 {
            match self.mem.r8(DATA, p) {
                0xFF => return Some(false),
                b if b == v => return Some(true),
                _ => p = p.wrapping_add(1),
            }
        }
        None
    }

    /// 1194:7772 (far entry 776E). Can class index `class` use the item?
    /// - type 8Dh and up, a health of 0, or category 3: no;
    /// - weapons: per-class list of base types (3276:3C25; none = all);
    /// - shields: per-class list (3276:3C71);
    /// - other armour: armour kind at least the class limit (3276:3CA4),
    ///   and Knights (17) cannot use kind 2;
    /// - accessories: yes.
    ///
    /// The Knight check also runs after a shield matched, but there the
    /// class register has been doubled, so it never applies.
    pub fn class_can_use(&self, seg: u16, it: u16, class: u16) -> Option<bool> {
        let ty = self.inv_b(seg, it, item::TYPE);
        if ty >= 0x8D || self.mem.r16(seg, it.wrapping_add(item::HEALTH)) == 0 {
            return Some(false);
        }
        let (base, _) = self.base_type(ty);
        let (bx, al) = match self.item_category(seg, it) {
            category::STACK => return Some(false),
            category::WEAPON => {
                let list = self.mem.r16(DATA, CLASS_WEAPONS.wrapping_add(class.wrapping_mul(2)));
                return if list == 0 { Some(true) } else { self.inv_in_list(list, base as u8) };
            }
            category::ARMOR if Self::is_shield(base) => {
                let bx = class.wrapping_mul(2);
                let list = self.mem.r16(DATA, CLASS_SHIELDS.wrapping_add(bx));
                if !self.inv_in_list(list, base as u8)? {
                    return Some(false);
                }
                (bx, base as u8)
            }
            category::ARMOR => {
                let kind = self.armor_kind(seg, it) as u8;
                if kind < self.mem.r8(DATA, CLASS_MAX_ARMOR.wrapping_add(class)) {
                    return Some(false);
                }
                (class, kind)
            }
            _ => return Some(true),
        };
        Some(!(bx == 0x11 && al == 2))
    }

    /// 1194:5EB5 (far entry 5EB1). First free slot of record `rec`:
    /// Ok((slot, index)), or Err(slot) one past the last when all 40 are
    /// taken.
    pub fn free_slot(&self, seg: u16, rec: u16) -> Result<(u16, u16), u16> {
        let mut si = rec.wrapping_add(SLOTS);
        for i in 0..SLOT_COUNT {
            if self.inv_flags(seg, si) & item::OCCUPIED == 0 {
                return Ok((si, i));
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        Err(si)
    }

    /// 0000:7732. memmove of `n` bytes from src_seg:si to dst_seg:di,
    /// choosing the direction by linear address. Returns the advanced
    /// (si, di); they do not move when source and destination coincide.
    pub fn memmove(&mut self, src_seg: u16, si: u16, dst_seg: u16, di: u16, n: u16) -> (u16, u16) {
        let src = src_seg as u32 * 16 + si as u32;
        let dst = dst_seg as u32 * 16 + di as u32;
        if src == dst {
            return (si, di);
        }
        let bytes: Vec<u8> = (0..n).map(|i| self.mem.r8(src_seg, si.wrapping_add(i))).collect();
        for (i, b) in bytes.into_iter().enumerate() {
            self.mem.w8(dst_seg, di.wrapping_add(i as u16), b);
        }
        (si.wrapping_add(n), di.wrapping_add(n))
    }

    /// 24FA:02C4. Copies the 19-byte item at src_seg:src into the first
    /// free slot of record `rec`, or just past the last slot when the
    /// inventory is full (the original does not check). Returns the slot.
    pub fn add_item(&mut self, src_seg: u16, src: u16, seg: u16, rec: u16) -> u16 {
        let slot = self.free_slot(seg, rec).map_or_else(|end| end, |(s, _)| s);
        self.memmove(src_seg, src, seg, slot, SLOT_SIZE);
        slot
    }

    /// 1194:788D (far entry 7889). Removes the item at slot pointer `si`,
    /// slot number `index`, from an inventory in `seg`. A stack only loses
    /// one (and is marked occupied) unless its count goes negative.
    /// Otherwise the slots above `index` move down one and the last is
    /// cleared; `index` above 39 makes the shift run on through memory.
    ///
    /// For the player (seg = DGROUP) the item's enchantment comes off
    /// first (`unequip_enchantment`).
    pub fn remove_item(&mut self, seg: u16, si: u16, index: u16) {
        if seg == DS {
            self.unequip_enchantment(si);
        }
        if self.item_category(seg, si) == category::STACK {
            let f = self.inv_flags(seg, si) | item::OCCUPIED;
            self.mem.w8(seg, si.wrapping_add(item::FLAGS), f);
            let n = self.mem.r16(seg, si.wrapping_add(item::COUNT)).wrapping_sub(1);
            self.mem.w16(seg, si.wrapping_add(item::COUNT), n);
            if (n as i16) >= 0 {
                return;
            }
        }
        let mut di = si;
        let cx = 0x27u16.wrapping_sub(index);
        if cx != 0 {
            let mut src = si.wrapping_add(SLOT_SIZE);
            for _ in 0..cx {
                (src, di) = self.memmove(seg, src, seg, di, SLOT_SIZE);
            }
        }
        for i in 0..SLOT_SIZE {
            self.mem.w8(seg, di.wrapping_add(i), 0);
        }
    }

    /// 1194:511A. Before an equipped item at DGROUP `si` leaves the
    /// player: an enchantment that raised attribute +12 by +11 percent
    /// gives it back (8-bit: a value over the attribute wraps, then
    /// clamps at 255) and rebases drain snapshots; enchantment value C1h
    /// clears flag 5CA bit 0x1000. The modifiers are always recomputed.
    /// Fixed (#13), the subtraction stops at 0: the bonus given back
    /// takes what the attribute has, as the clamp at 255 was meant to keep
    /// it in range, not to make a drained attribute its maximum.
    pub fn unequip_enchantment(&mut self, si: u16) {
        use crate::scale::{to100, to256};
        let p = crate::character::PLAYER;
        let b = |m: &Self, f: u16| m.mem.r8(DS, si.wrapping_add(f));
        if self.inv_flags(DS, si) & item::EQUIPPED != 0 {
            if self.inv_flags(DS, si) & item::ENCHANTED != 0 && b(self, item::ENCHANT) != 0xFF && b(self, item::ENCHANT_ATTR) != 0xFF {
                let attr = b(self, item::ENCHANT_ATTR) as u16;
                let at = p + field::ATTRS + attr;
                let r = to100(self.mem.r8(DS, at) as i16) as u16;
                let ax = if self.fixed(crate::fixes::UNEQUIP_WRAP) {
                    r & 0xFF00 | (r as u8).saturating_sub(b(self, item::ENCHANT_VALUE)) as u16
                } else {
                    r & 0xFF00 | (r as u8).wrapping_sub(b(self, item::ENCHANT_VALUE)) as u16
                };
                let ax = (to256(ax as i16) as u16).min(0xFF);
                self.mem.w8(DS, at, ax as u8);
                let v = (to256(b(self, item::ENCHANT_VALUE) as i16) as u16).min(0xFF);
                self.rebase_drain_snapshots(v, attr);
            }
            if b(self, item::ENCHANT_VALUE) == 0xC1 {
                let f = self.mem.r16(DS, 0x05CA) & !0x1000;
                self.mem.w16(DS, 0x05CA, f);
            }
        }
        self.attribute_modifiers(p);
    }

    /// 1194:7719. Player armour class at the 7 locations, into DGROUP
    /// 942E: 10 - (armour + AGI modifier in percent) / 5.
    pub fn player_armor_class(&mut self) {
        use crate::character::PLAYER;
        for i in 0..7 {
            self.mem.w8(DS, ARMOR_CLASS + i, 0);
        }
        for loc in 0..7u16 {
            let armor = self.armor_at(DS, PLAYER, loc);
            let agi = crate::scale::to100(self.mem.r16(DS, PLAYER + field::AGI_MOD_4B) as i16) as u16;
            let ac = (10i16).wrapping_sub(agi.wrapping_add(armor) as i16 / 5);
            self.mem.w8(DS, ARMOR_CLASS + loc, ac as u8);
        }
    }

    /// 1194:7BE7. Name of an item's condition: health as a percentage of
    /// max health (low byte only) against the steps at 5DEA picks one of
    /// the strings at 5DF1 (both read through `seg`). A max health of 0
    /// picks the first.
    pub fn condition_name(&self, seg: u16, it: u16) -> u16 {
        let health = self.mem.r16(seg, it.wrapping_add(item::HEALTH)) as u32;
        let max = self.mem.r16(seg, it.wrapping_add(item::MAX_HEALTH)) as u32;
        let mut idx = 0u16;
        if max != 0 {
            let pct = (health * 100 / max) as u8;
            while idx < 7 && pct > self.mem.r8(seg, CONDITION_STEPS + idx) {
                idx += 1;
            }
        }
        self.skip_strings(seg, CONDITION_NAMES, idx)
    }

    /// 1194:7A8A. Does the item have an effect the owner can use (it is
    /// listed by `nth_usable_item`)? Stacks always; special items other
    /// than 88h when equipped or of type 8Dh and up; other items only when
    /// equipped and enchanted: kind-0 accessories, or an enchantment with
    /// no attribute (+12 = FFh) that is not C1h and not 0.
    pub fn item_has_effect(&self, seg: u16, it: u16) -> bool {
        let f = self.inv_flags(seg, it);
        if f & item::OCCUPIED == 0 {
            return false;
        }
        let cat = self.item_category(seg, it);
        if cat == category::STACK {
            return true;
        }
        let ty = self.inv_b(seg, it, item::TYPE);
        if ty & 0x80 != 0 {
            return ty != 0x88 && (ty >= 0x8D || f & item::EQUIPPED != 0);
        }
        if f & item::EQUIPPED == 0 || f & item::ENCHANTED == 0 {
            return false;
        }
        if cat == category::ACCESSORY {
            return self.armor_kind(seg, it) == 0;
        }
        self.inv_b(seg, it, item::ENCHANT_ATTR) == 0xFF
            && self.inv_b(seg, it, item::ENCHANT_VALUE) != 0xC1
            && self.inv_b(seg, it, item::ENCHANT) != 0
    }

    /// 1194:7AE7. Slot pointer of the `n`th (0-based) item with an effect
    /// in record `rec`, or where the walk stopped. It walks as many slots
    /// as are occupied (65536 when none are, as `loop` does with CX = 0),
    /// and n = FFFFh matches the first slot.
    pub fn nth_usable_item(&self, seg: u16, rec: u16, n: u16) -> u16 {
        let mut si = rec.wrapping_add(SLOTS);
        let count = self.count_items(seg, si).0;
        let steps = if count == 0 { 0x10000 } else { count as u32 };
        let mut seen = 0xFFFFu16;
        for _ in 0..steps {
            if self.item_has_effect(seg, si) {
                seen = seen.wrapping_add(1);
            }
            if seen == n {
                return si;
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        si
    }

    /// 1194:7804. Does the player's item at `it` (DGROUP) clash with an
    /// accessory already worn? Only enchanted, unequipped, non-special
    /// accessories are checked. An equipped accessory of the same base
    /// type clashes if its armour kind matches, or if neither kind is 0;
    /// but for type 3 with a kind other than 0, special items 8Ah/8Bh
    /// (which can map to base type 3) do not. The original returns CF
    /// clear on a clash and walks as many slots as are occupied. Leaves
    /// the kind and type at DGROUP 12A0/12A2.
    pub fn accessory_clash(&mut self, it: u16) -> bool {
        let f = self.inv_flags(DS, it);
        let ty = self.inv_b(DS, it, item::TYPE);
        if ty & 0x80 != 0
            || f & item::EQUIPPED != 0
            || f & item::ENCHANTED == 0
            || self.item_category(DS, it) != category::ACCESSORY
        {
            return false;
        }
        let kind = self.armor_kind(DS, it);
        self.mem.w16(DS, 0x12A0, kind);
        self.mem.w16(DS, 0x12A2, ty as u16);
        let (n, carry) = self.count_items(DS, PLAYER_SLOTS);
        let (mut si, mut cx, mut carry) = (PLAYER_SLOTS, n, carry);
        loop {
            let ty = self.mem.r16(DS, 0x12A2);
            let (found, s, c) = self.inv_find_equipped_cx(DS, si, cx, ty, category::ACCESSORY, carry);
            if !found {
                return false;
            }
            let k = self.armor_kind(DS, s);
            let want = self.mem.r16(DS, 0x12A0);
            if k == want || (k != 0 && want != 0) {
                // 1194:786D
                let exempt = want != 0
                    && self.mem.r16(DS, 0x12A2) == 3
                    && matches!(self.inv_b(DS, s, item::TYPE), 0x8A | 0x8B);
                return !exempt;
            }
            si = s.wrapping_add(SLOT_SIZE);
            if c == 0 {
                return false;
            }
            (cx, carry) = (c, false);
        }
    }

    /// 1194:7A78. Swaps `n` bytes between DS:si and DS:di.
    fn inv_swap(&mut self, si: u16, di: u16, n: u16) {
        if si == di {
            return;
        }
        for i in 0..n {
            let (a, b) = (self.mem.r8(DS, si.wrapping_add(i)), self.mem.r8(DS, di.wrapping_add(i)));
            self.mem.w8(DS, si.wrapping_add(i), b);
            self.mem.w8(DS, di.wrapping_add(i), a);
        }
    }

    /// 1194:7A0D. Appends to the order list at `di` the slot numbers of
    /// all items of base type `ty` and category `cat`, walking as many
    /// slots as are occupied. Returns the new end of the list.
    fn inv_list_items(&mut self, mut di: u16, ty: u16, cat: u16) -> u16 {
        let mut cx = self.mem.r8(DS, LAST_COUNT) as u16;
        let mut si = PLAYER_SLOTS;
        loop {
            let (found, s, c) = self.find_item(DS, si, cx, ty, cat);
            if !found {
                return di;
            }
            self.mem.w8(DS, di, (s.wrapping_sub(PLAYER_SLOTS) / SLOT_SIZE) as u8);
            di = di.wrapping_add(1);
            si = s.wrapping_add(SLOT_SIZE);
            cx = c;
        }
    }

    /// 1194:7A35. `inv_list_items` for accessories, keeping those of
    /// armour kind 0 (`nonzero_kind` false) or of any other kind (true).
    fn inv_list_accessories(&mut self, mut di: u16, ty: u16, nonzero_kind: bool) -> u16 {
        let mut cx = self.mem.r8(DS, LAST_COUNT) as u16;
        self.mem.w16(DS, 0x12A0, nonzero_kind as u16);
        let mut si = PLAYER_SLOTS;
        loop {
            let (found, s, c) = self.find_item(DS, si, cx, ty, category::ACCESSORY);
            if !found {
                return di;
            }
            if (self.armor_kind(DS, s) != 0) == nonzero_kind {
                self.mem.w8(DS, di, (s.wrapping_sub(PLAYER_SLOTS) / SLOT_SIZE) as u8);
                di = di.wrapping_add(1);
            }
            si = s.wrapping_add(SLOT_SIZE);
            cx = c;
        }
    }

    /// 1194:7911 (far entry 790D). Sorts the player's items: body armour
    /// in location order 2, 6, 0, 3, 4, 1, 5, shields, weapons 0..17,
    /// accessories 0..3 of kind 0 then of other kinds, stacks 0..14. The
    /// order list of slot numbers is built at DGROUP 110C, its length at
    /// 110A, then slots are swapped into place. Only as many slots as are
    /// occupied are walked, so items after a gap are left out of the list.
    pub fn sort_inventory(&mut self) {
        const LIST: u16 = 0x110C;
        let (n, _) = self.count_items(DS, PLAYER_SLOTS);
        if n == 0 {
            return;
        }
        self.mem.w8(DS, LAST_COUNT, n as u8);
        let mut di = LIST;
        for ty in [2, 6, 0, 3, 4, 1, 5] {
            di = self.inv_list_items(di, ty, category::ARMOR);
        }
        for ty in 7..=10 {
            di = self.inv_list_items(di, ty, category::ARMOR);
        }
        for ty in 0..18 {
            di = self.inv_list_items(di, ty, category::WEAPON);
        }
        for nonzero in [false, true] {
            for ty in 0..4 {
                di = self.inv_list_accessories(di, ty, nonzero);
            }
        }
        for ty in 0..15 {
            di = self.inv_list_items(di, ty, category::STACK);
        }
        let len = di.wrapping_sub(LIST);
        self.mem.w16(DS, 0x110A, len);
        let mut si = PLAYER_SLOTS;
        for bx in 0..len {
            let want = self.mem.r8(DS, LIST.wrapping_add(bx));
            // repne scasb for bx's low byte; when it is absent DI ends one
            // past the list, so the last entry is overwritten.
            let pos = (0..len).find(|&i| self.mem.r8(DS, LIST.wrapping_add(i)) == bx as u8).unwrap_or(len - 1);
            self.mem.w8(DS, LIST.wrapping_add(pos), want);
            self.inv_swap(si, PLAYER_SLOTS.wrapping_add((want as u16).wrapping_mul(SLOT_SIZE)), SLOT_SIZE);
            self.mem.w8(DS, LIST.wrapping_add(bx), bx as u8);
            si = si.wrapping_add(SLOT_SIZE);
        }
    }

    /// 1194:7390. Name of the item at seg:`it`, as an offset in DGROUP
    /// (the caller's DS). Plain items, and enchanted armour and weapons
    /// not yet identified (flags bit 1), take a string from a list:
    /// weapons by type, armour by type from its material's list, potions
    /// by type. Others are built by 27CE:000C (`item_name_format`) at
    /// offset 1395 of the item's own segment, while 1395 is returned:
    /// for an item outside DGROUP the name read is whatever DGROUP holds
    /// there. The type byte is sign-extended, so types from 0x80 skip
    /// nearly 65536 strings. None where the original calls through a
    /// jump table entry that isn't code (accessory kind 3).
    pub fn item_name(&mut self, seg: u16, it: u16) -> Option<u16> {
        let ty = self.mem.r8(seg, it.wrapping_add(item::TYPE)) as i8 as u16;
        let flags = self.inv_flags(seg, it);
        let cat = ((flags & 0x0C) >> 2) as u16;
        let plain = |m: &Self, cat: u16| {
            let list = if cat != 0 {
                WEAPON_NAMES
            } else {
                m.mem.r16(DS, ARMOR_LISTS.wrapping_add((((flags & 0x30) >> 4) as u16) * 2))
            };
            m.skip_strings(DS, list, ty)
        };
        if flags & item::ENCHANTED == 0 {
            return Some(plain(self, cat));
        }
        match cat {
            3 => Some(if flags & 2 != 0 { POTION } else { self.skip_strings(DS, POTION_NAMES, ty) }),
            2 => {
                let kind = (flags & 0x30) >> 4;
                let b10 = self.mem.r8(seg, it.wrapping_add(0x10));
                let mut suffix = self.mem.r8(seg, it.wrapping_add(item::ENCHANT));
                if kind == 2 {
                    suffix = 0;
                }
                if flags & 2 != 0 {
                    suffix = 0xFF;
                }
                let ah = if kind == 0 { b10 } else { 0 };
                self.item_name_format(seg, 2, kind, ah, b10 as u16, ty as u8, suffix)?;
                Some(NAME_BUF)
            }
            _ if flags & 2 != 0 => Some(plain(self, cat)),
            _ => {
                let material = self.mem.r8(seg, it.wrapping_add(0x10));
                let suffix = self.mem.r8(seg, it.wrapping_add(item::ENCHANT));
                self.item_name_format(seg, cat as u8, material, 0, ty, 0, suffix)?;
                Some(NAME_BUF)
            }
        }
    }

    /// 27CE:000C. Builds a name at seg:1395. For armour (`dl` 0) and
    /// weapons (1): a material (`al`, 0xFF none) and a space, the type's
    /// name (`bx`) and a suffix ("of ...", `suffix`, 0xFF none). For
    /// accessories (2), `al` picks the handler: 0 a ring-like name with a
    /// spell suffix chosen by `ah` (0, 1 or other), 1 a bracelet-like
    /// name with an attribute suffix, 2 a material (`bx` + 3) and the
    /// bracelet-like name; `cl` is the type and `suffix` the suffix.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn item_name_format(&mut self, seg: u16, dl: u8, al: u8, ah: u8, bx: u16, cl: u8, suffix: u8) -> Option<()> {
        let mut di = NAME_BUF;
        let copy = |m: &mut Self, list: u16, n: u16, di: &mut u16| {
            let mut si = m.skip_strings(DS, list, n);
            loop {
                let c = m.mem.r8(DS, si);
                m.mem.w8(seg, *di, c);
                si = si.wrapping_add(1);
                *di = di.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        };
        let space = |m: &mut Self, di: u16| m.mem.w8(seg, di.wrapping_sub(1), b' ');
        if dl == 2 {
            // AH is kept in DGROUP while the handler is looked up.
            self.mem.w8(DS, 0x1258, ah);
            match al {
                0 => {
                    copy(self, RING_NAMES, cl as u16, &mut di);
                    let list = match ah {
                        0 => RING_SUFFIXES_0,
                        1 => RING_SUFFIXES_1,
                        _ => RING_SUFFIXES_2,
                    };
                    if suffix != 0xFF {
                        space(self, di);
                        copy(self, list, suffix as u16, &mut di);
                    }
                }
                1 => {
                    copy(self, BRACELET_NAMES, cl as u16, &mut di);
                    if suffix != 0xFF {
                        space(self, di);
                        copy(self, ATTRIBUTE_SUFFIXES, suffix as u16, &mut di);
                    }
                }
                2 => {
                    if suffix == 0 {
                        copy(self, MATERIALS, (bx as u8 as u16).wrapping_add(3), &mut di);
                        space(self, di);
                    }
                    copy(self, BRACELET_NAMES, cl as u16, &mut di);
                }
                _ => return None,
            }
            return Some(());
        }
        if al != 0xFF {
            copy(self, MATERIALS, al as u16, &mut di);
            space(self, di);
        }
        let (names, suffixes) = if dl == 0 { (ARMOR_PARTS, ARMOR_SUFFIXES) } else { (WEAPON_NAMES, WEAPON_SUFFIXES) };
        copy(self, names, bx, &mut di);
        space(self, di);
        if suffix != 0xFF {
            copy(self, suffixes, suffix as u16, &mut di);
        }
        self.mem.w8(seg, di.wrapping_sub(1), 0);
        Some(())
    }

}
