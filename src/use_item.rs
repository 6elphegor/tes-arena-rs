//! Using items in play: the "use" button's list of items with an effect
//! and what using one does (24FA:07CD, 29CF:07C3), the two once-a-day
//! items (2294:1051, 0FFA), and the inventory screen's rules for putting
//! on, taking off (1194:1AFB, 7605) and dropping (1194:18F6) an item,
//! a dropped item landing in the level's item list (2433:09E1).
//!
//! Using an item (29CF:07C3) casts its spell (+11h) on the player:
//! - a stack (a potion) is drunk: spell FFh heals 25 points (to the
//!   maximum), another is cast at level byte 0Fh; one is taken from the
//!   stack (or the slot emptied) and the inventory sorted;
//! - armour and weapons pay the spell's cost (+32h of its record) out of
//!   their condition: a special item (type bit 80h) half the cost, cast
//!   at level byte 13h (and F84 set to 8000h), and it breaks once its
//!   condition goes negative, after the cast; another a fifteenth, cast
//!   at level byte 0Eh, breaking instead of casting when that is more
//!   than it has;
//! - accessories by the table at 7F81 (per type): of armour kind 0 a
//!   charge (+3) each use, breaking with none left; of other kinds as
//!   armour, the table's value the divisor; special ones as armour.
//!
//! The level byte (player +6, 1AA) is set around the cast to the cast's
//! level less one, as the caster's level is read from it.
//!
//! The inventory screen's drawing, its lists and prompts are the front
//! end's (`UseHooks`); the routines here start from the slot the screen
//! has selected (index at 1296 for dropping) or that a click picked.
//!
//! The segment registers the original leaves to its callers (ES in
//! 1194:5074, 2433:0201 and the item naming, FS in 1194:7527) are DGROUP
//! there; the port takes them so.

use crate::bsa::Files;
use crate::character::PLAYER;
use crate::format::Arg;
use crate::inventory::{category, item, LAST_COUNT, PLAYER_SLOTS, SLOT_SIZE};
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The list of usable items: its count, the next name's place, and
    /// the names (NUL-terminated, one after another).
    pub const USABLE_COUNT: u16 = 0x127C;
    pub const USABLE_NEXT: u16 = 0x127E;
    pub const USABLE_NAMES: u16 = 0x9686;
    /// The level byte of the player's record (player +6).
    pub const LEVEL: u16 = 0x01AA;
    /// Health and its maximum (player +66h, +68h).
    pub const HEALTH: u16 = 0x020A;
    pub const MAX_HEALTH: u16 = 0x020C;
    /// Per accessory type: the level byte its cast is made at, plus one,
    /// and (for kinds other than 0) the divisor of its cost.
    pub const ACCESSORY_LEVEL: u16 = 0x7F81;
    /// A waiting Open spell's power (7D0h: sure).
    pub const OPEN_SPELL: u16 = 0x0DC4;
    /// The game's minutes (dword) and the player's gold (dword).
    pub const MINUTES: u16 = 0x05CC;
    pub const GOLD: u16 = 0x05C2;
    /// The inventory screen: the selected slot (FFh none), the first
    /// line shown, whether the player's own items are shown (else the
    /// other list's count at 50A), the other list's flag (1299), a
    /// shop's or a looting's flag (1270) and the line count (126C).
    pub const SELECTED: u16 = 0x1296;
    pub const FIRST_LINE: u16 = 0x1297;
    pub const OWN_ITEMS: u16 = 0x129A;
    pub const OTHER_LIST: u16 = 0x1299;
    pub const NOT_OWN: u16 = 0x1270;
    pub const LINES: u16 = 0x126C;
    pub const OTHER_COUNT: u16 = 0x050A;
    /// The item being dropped: its slot number and a copy.
    pub const DROPPED_INDEX: u16 = 0x13C8;
    pub const DROPPED: u16 = 0x13DA;
    /// The record a dropped item is placed with (1Ch bytes), the level's
    /// item count (byte) and its list (in the EMS frame, from A000h).
    pub const FLOOR_ITEM: u16 = 0x4BF6;
    pub const LEVEL_ITEMS: u16 = 0x0FF2;
    pub const LEVEL_ITEM_LIST: u16 = 0xA000;
    /// The last special item put on (1194:5074).
    pub const SPECIAL_WORN: u16 = 0x1AA4;
}
use var::*;

/// The EMS bank the level's item list lives in.
const ITEMS_BANK: u16 = 6;

/// What the front end does for the inventory screen and the use button.
pub trait UseHooks {
    /// 29CF:00FA with SI the player and FS DGROUP: the player casts spell
    /// `spell` (the level byte at 1AA as the caller set it).
    fn cast(&mut self, m: &mut Machine, files: &mut Files, spell: u8) -> Option<()> {
        m.player_cast(files, spell)
    }
    /// 2AA1:021B: the player's item at DGROUP `si` broke.
    fn broken(&mut self, m: &mut Machine, si: u16) -> Option<()> {
        m.item_broken(si)
    }
    /// 24FA:0B78: the list of usable items shown (`count` names from
    /// USABLE_NAMES). The pick, or FFFFh (any negative) for none.
    fn pick(&mut self, _m: &mut Machine, _count: u16) -> u16 {
        0xFFFF
    }
    /// 1194:59C8: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:1BE0: the item at DGROUP `si` (category `cat`) can't be used
    /// by the player's class, or (NOT_OWN set) items aren't put on here.
    fn cant_use(&mut self, _m: &mut Machine, _si: u16, _cat: u16) {}
    /// 1194:1B78: the accessory at DGROUP `si` clashes with one worn (its
    /// base type `base`; the kind looked for is at 12A0).
    fn clash(&mut self, _m: &mut Machine, _si: u16, _base: u16) {}
}

/// `UseHooks` with the defaults: casts and breakages ported, no picks.
pub struct NoUseHooks;

impl UseHooks for NoUseHooks {}

/// What putting an item on or taking it off did (1194:1AFB).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Equip {
    /// Put on or taken off (`equip_toggle`).
    Toggled,
    /// Refused: the class can't use it (`UseHooks::cant_use`).
    CantUse,
    /// Refused: an accessory of its kind is worn (`UseHooks::clash`).
    Clash,
}

impl Machine {
    /// 24FA:07CD. The use button: the player's items with an effect are
    /// listed (`usable_list`) and one picked; type 8Fh is the Open charm
    /// (`open_charm`), 8Dh the gold charm (`gold_charm`), any other is
    /// used (`item_cast`). None where naming an item, a cast or a
    /// breakage fails.
    pub fn use_item(&mut self, files: &mut Files, hooks: &mut dyn UseHooks) -> Option<()> {
        let n = self.usable_list(PLAYER)?;
        let ax = if n == 0 { 0xFFFF } else { hooks.pick(self, n) };
        if (ax as i16) < 0 {
            return Some(());
        }
        let si = self.nth_usable_item(DS, PLAYER, ax);
        match self.mem.r8(DS, si) {
            0x8F => {
                self.open_charm(hooks, si);
                Some(())
            }
            0x8D => self.gold_charm(hooks, si),
            _ => self.item_cast(files, hooks, si),
        }
    }

    /// 24FA:0AFE. The usable items of record `rec` (all 40 slots, those
    /// with an effect, 1194:7A86) named one after another at
    /// USABLE_NAMES (each at most 19 characters, cut short without its
    /// NUL then given one; a second NUL is written after each, which the
    /// next name covers). Returns their count, kept at USABLE_COUNT.
    pub fn usable_list(&mut self, rec: u16) -> Option<u16> {
        self.mem.w16(DS, USABLE_COUNT, 0);
        self.mem.w16(DS, USABLE_NEXT, USABLE_NAMES);
        let mut si = rec.wrapping_add(crate::inventory::SLOTS);
        for _ in 0..0x28 {
            if self.item_has_effect(DS, si) {
                let mut name = self.display_name(si)?;
                let mut di = self.mem.r16(DS, USABLE_NEXT);
                let mut ended = false;
                for _ in 0..0x13 {
                    let b = self.mem.r8(DS, name);
                    name = name.wrapping_add(1);
                    self.mem.w8(DS, di, b);
                    di = di.wrapping_add(1);
                    if b == 0 {
                        ended = true;
                        break;
                    }
                }
                if !ended {
                    self.mem.w8(DS, di, 0);
                    di = di.wrapping_add(1);
                }
                self.mem.w16(DS, USABLE_NEXT, di);
                self.mem.w8(DS, di, 0);
                let n = self.mem.r16(DS, USABLE_COUNT).wrapping_add(1);
                self.mem.w16(DS, USABLE_COUNT, n);
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        Some(self.mem.r16(DS, USABLE_COUNT))
    }

    /// 1194:7388. An item's name as lists show it: a special item's (type
    /// bit 80h) from the list at 35F0 by its type, any other's
    /// `item_name`.
    pub fn display_name(&mut self, it: u16) -> Option<u16> {
        let ty = self.mem.r8(DS, it);
        if ty & 0x80 != 0 {
            return Some(self.skip_strings(DS, 0x35F0, (ty & 0x7F) as u16));
        }
        self.item_name(DS, it)
    }

    /// 29CF:07C3. The item at DGROUP `si` is used (see the module notes).
    /// The waiting spells' state (DC0, DC2, DC4, DC8) is cleared first.
    /// None where an accessory's divisor is 0 (the original's divide
    /// fault), or a cast or breakage fails.
    pub fn item_cast(&mut self, files: &mut Files, hooks: &mut dyn UseHooks, si: u16) -> Option<()> {
        for o in [0x0DC0, 0x0DC2, OPEN_SPELL, 0x0DC8] {
            self.mem.w16(DS, o, 0);
        }
        let mut cx = 0x0Fu16;
        let ty = self.mem.r8(DS, si);
        match self.item_category(DS, si) {
            category::STACK => return self.drink(files, hooks, si),
            category::ACCESSORY if ty & 0x80 == 0 => {
                cx = self.mem.r8(DS, ACCESSORY_LEVEL.wrapping_add(ty as u16)) as u16;
                if self.armor_kind(DS, si) == 0 {
                    let charges = self.mem.r8(DS, si.wrapping_add(item::HANDS));
                    if charges == 0 {
                        return hooks.broken(self, si);
                    }
                    self.mem.w8(DS, si.wrapping_add(item::HANDS), charges - 1);
                    let spell = self.mem.r8(DS, si.wrapping_add(item::ENCHANT_VALUE));
                    return self.item_spell(files, hooks, spell, cx as u8);
                }
            }
            _ => {}
        }
        // 29CF:081F: the spell's cost out of the item's condition.
        let spell = self.mem.r8(DS, si.wrapping_add(item::ENCHANT_VALUE));
        let (gs, rec) = self.spell_record(spell);
        let cost = self.mem.r16(gs, rec.wrapping_add(0x32));
        let health = self.mem.r16(DS, si.wrapping_add(item::HEALTH));
        if ty & 0x80 != 0 {
            let left = health.wrapping_sub(cost >> 1);
            self.mem.w16(DS, si.wrapping_add(item::HEALTH), left);
            self.mem.w16(DS, 0x0F84, 0x8000);
            self.item_spell(files, hooks, spell, 0x14)?;
            if (left as i16) >= 0 {
                return Some(());
            }
        } else {
            if cx == 0 {
                return crate::giveup::original("29CF:081F: a divide by zero");
            }
            let bx = cost / cx;
            self.mem.w16(DS, si.wrapping_add(item::HEALTH), health.wrapping_sub(bx));
            if health >= bx {
                return self.item_spell(files, hooks, spell, cx as u8);
            }
        }
        // 29CF:085E
        self.mem.w16(DS, si.wrapping_add(item::HEALTH), 0);
        hooks.broken(self, si)
    }

    /// 29CF:08BE. Spell `spell` cast by the player with the level byte
    /// at `cl` less one meanwhile.
    fn item_spell(&mut self, files: &mut Files, hooks: &mut dyn UseHooks, spell: u8, cl: u8) -> Option<()> {
        let level = self.mem.r8(DS, LEVEL);
        self.mem.w8(DS, LEVEL, cl.wrapping_sub(1));
        let r = hooks.cast(self, files, spell);
        self.mem.w8(DS, LEVEL, level);
        r
    }

    /// 29CF:0869. A potion at `si` is drunk: spell FFh heals 25 points,
    /// another is cast at level byte 0Fh; then one is taken from the
    /// stack (1194:7889) and the inventory sorted (1194:790D).
    fn drink(&mut self, files: &mut Files, hooks: &mut dyn UseHooks, si: u16) -> Option<()> {
        let spell = self.mem.r8(DS, si.wrapping_add(item::ENCHANT_VALUE));
        if spell == 0xFF {
            let hp = self.mem.r16(DS, HEALTH).wrapping_add(0x19).min(self.mem.r16(DS, MAX_HEALTH));
            self.mem.w16(DS, HEALTH, hp);
        } else {
            let level = self.mem.r8(DS, LEVEL);
            self.mem.w8(DS, LEVEL, 0x0F);
            let r = hooks.cast(self, files, spell);
            self.mem.w8(DS, LEVEL, level);
            r?;
        }
        let index = si.wrapping_sub(PLAYER_SLOTS) / SLOT_SIZE;
        self.remove_item(DS, si, index);
        self.sort_inventory();
        Some(())
    }

    /// 2294:1085. The day of the year (0..359) of the game's minutes.
    fn charm_day(&self) -> u16 {
        let minutes = self.mem.r32(DS, MINUTES);
        ((minutes % 0x7E900) / 0x5A0) as u16
    }

    /// 2294:1067. "Already used today" (41E8), in a message box.
    fn charm_used(&mut self, hooks: &mut dyn UseHooks) {
        let v = self.mem.r16(DS, 0x1036);
        self.mem.w16(DS, 0xB038, v);
        self.mem.w8(DS, 0x9288, 0xD4);
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        hooks.message_box(self, 0x41E8);
    }

    /// 2294:1051. Type 8Fh, once a day (the day kept at +1): a sure Open
    /// spell waits (OPEN_SPELL 7D0h).
    pub fn open_charm(&mut self, hooks: &mut dyn UseHooks, si: u16) {
        let day = self.charm_day();
        if day == self.mem.r16(DS, si.wrapping_add(1)) {
            self.charm_used(hooks);
            return;
        }
        self.mem.w16(DS, si.wrapping_add(1), day);
        self.mem.w16(DS, OPEN_SPELL, 0x7D0);
    }

    /// 2294:0FFA. Type 8Dh, once a day (the day kept at +1): 10..100 gold
    /// out of its condition, "You find %d gold." for 15 frames; it
    /// breaks when its condition goes negative, giving none. None where
    /// the breakage fails.
    pub fn gold_charm(&mut self, hooks: &mut dyn UseHooks, si: u16) -> Option<()> {
        let day = self.charm_day();
        if day == self.mem.r16(DS, si.wrapping_add(1)) {
            self.charm_used(hooks);
            return Some(());
        }
        self.mem.w16(DS, si.wrapping_add(1), day);
        let gold = self.rng.range(0x0A, 0x65)?;
        let left = self.mem.r16(DS, si.wrapping_add(item::HEALTH)).wrapping_sub(gold);
        self.mem.w16(DS, si.wrapping_add(item::HEALTH), left);
        if (left as i16) < 0 {
            self.mem.w16(DS, si.wrapping_add(item::HEALTH), 0);
            return hooks.broken(self, si);
        }
        let g = self.mem.r32(DS, GOLD).wrapping_add(gold as u32);
        self.mem.w32(DS, GOLD, g);
        self.sprintf((DS, 0x7979), (DS, 0x41C7), &[Arg::Int(gold)]);
        self.mem.w16(DS, 0x061E, 0x0F);
        self.mem.w16(DS, 0x061A, 0x7979);
        self.mem.w16(DS, 0x061C, DS);
        Some(())
    }

    /// 1194:1AFB..1B37. A click puts on or takes off the player's item in
    /// slot `index` (the line under the cursor): not while another list
    /// is shown (NOT_OWN), nor what the class can't use (1194:7772), nor
    /// an accessory clashing with one worn (1194:7804); otherwise it is
    /// toggled (`equip_toggle`), the weapon class (1194:731A) and the
    /// armour class (1194:7719, in the redraw 23B1) made again. None
    /// where the class's list has no end. With NOT_OWN set the original
    /// names the category of the item its caller's SI points at; the
    /// port passes the clicked one.
    pub fn equip_click(&mut self, hooks: &mut dyn UseHooks, index: u16) -> Option<Equip> {
        let si = PLAYER_SLOTS.wrapping_add(index.wrapping_mul(SLOT_SIZE));
        if self.mem.r16(DS, NOT_OWN) != 0 {
            let cat = self.item_category(DS, si);
            hooks.cant_use(self, si, cat);
            return Some(Equip::CantUse);
        }
        let class = (self.mem.r8(DS, PLAYER + crate::character::field::CLASS) & 0x1F) as u16;
        if !self.class_can_use(DS, si, class)? {
            let cat = self.item_category(DS, si);
            hooks.cant_use(self, si, cat);
            return Some(Equip::CantUse);
        }
        if self.accessory_clash(si) {
            let (base, _) = self.base_type(self.mem.r8(DS, si));
            hooks.clash(self, si, base);
            return Some(Equip::Clash);
        }
        self.equip_toggle(si);
        self.player_weapon_class();
        self.player_armor_class();
        Some(Equip::Toggled)
    }

    /// 1194:7605. The player's item at DGROUP `si` is put on or taken
    /// off (flag 80h flipped). Special items 80h..88h lose their armour
    /// kind first. Taking off gives back its enchantment (1194:511A);
    /// putting on takes off what it can't be worn with (`unequip_clashes`),
    /// applies its enchantment (`equip_enchantment`) and remakes the
    /// player's attribute modifiers (1194:3621).
    pub fn equip_toggle(&mut self, si: u16) {
        let ty = self.mem.r8(DS, si);
        if ty & 0x80 != 0 && ty <= 0x88 {
            let f = self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & 0xCF;
            self.mem.w8(DS, si.wrapping_add(item::FLAGS), f);
        }
        if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::EQUIPPED != 0 {
            self.unequip_enchantment(si);
        } else {
            self.unequip_clashes(si);
            self.equip_enchantment(si);
            self.attribute_modifiers(PLAYER);
        }
        self.toggle_equipped(si);
    }

    fn toggle_equipped(&mut self, si: u16) {
        let f = self.mem.r8(DS, si.wrapping_add(item::FLAGS)) ^ item::EQUIPPED;
        self.mem.w8(DS, si.wrapping_add(item::FLAGS), f);
    }

    /// 1194:511A then the flag flipped: a worn item taken off.
    fn take_off(&mut self, si: u16) {
        self.unequip_enchantment(si);
        self.toggle_equipped(si);
    }

    /// 1194:7644. Before the item at `si` is put on: armour of the same
    /// location (base type) worn among the LAST_COUNT slots comes off, a
    /// shield takes off a shield or a two-handed weapon
    /// (`shield_or_two_hander`); a weapon takes off the worn weapon
    /// (`worn_weapon`) and, two-handed (+3 not 1), a shield too.
    fn unequip_clashes(&mut self, si: u16) {
        match self.item_category(DS, si) {
            category::ARMOR => {
                let (base, _) = self.base_type(self.mem.r8(DS, si));
                if Self::is_shield(base) {
                    self.take_off_shield();
                    return;
                }
                let cx = self.mem.r8(DS, LAST_COUNT) as u16;
                if let (true, found) = self.find_equipped(DS, PLAYER_SLOTS, cx, base, category::ARMOR, false) {
                    self.take_off(found);
                }
            }
            category::WEAPON => {
                let hands = (self.mem.r8(DS, si.wrapping_add(item::HANDS)) as u16).wrapping_sub(1);
                if let Some(w) = self.worn_weapon() {
                    self.take_off(w);
                }
                if hands != 0 {
                    self.take_off_shield();
                }
            }
            _ => {}
        }
    }

    /// 1194:769F
    fn take_off_shield(&mut self) {
        if let Some(s) = self.shield_or_two_hander() {
            self.take_off(s);
        }
    }

    /// 1194:7527. The first worn weapon among the player's LAST_COUNT
    /// slots.
    fn worn_weapon(&self) -> Option<u16> {
        let mut si = PLAYER_SLOTS;
        for _ in 0..self.mem.r8(DS, LAST_COUNT) {
            let f = self.mem.r8(DS, si.wrapping_add(item::FLAGS));
            if f & item::EQUIPPED != 0 && self.item_category(DS, si) == category::WEAPON {
                return Some(si);
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        None
    }

    /// 1194:7548. The first worn shield or two-handed weapon (the table
    /// at 229E per base type holds 2) among the player's LAST_COUNT
    /// slots; special items mapping to a negative base type and
    /// accessories are passed over.
    fn shield_or_two_hander(&self) -> Option<u16> {
        let mut si = PLAYER_SLOTS;
        for _ in 0..self.mem.r8(DS, LAST_COUNT) {
            if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::EQUIPPED != 0 {
                let cat = self.item_category(DS, si);
                let (base, negative) = self.base_type(self.mem.r8(DS, si));
                if !negative {
                    let hit = match cat {
                        category::ARMOR => Self::is_shield(base),
                        category::WEAPON => self.mem.r8(DS, 0x229E_u16.wrapping_add(base)) == 2,
                        _ => false,
                    };
                    if hit {
                        return Some(si);
                    }
                }
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        None
    }

    /// 1194:5074. An item put on applies its enchantment: a special item
    /// is recorded (SPECIAL_WORN); an enchanted one with an enchantment
    /// (+F not FFh) raises the attribute +12h by +11h's place: an
    /// accessory of kind 1 by its type's amount (table 4AFE), others by
    /// 10, in percent, the amount kept at +11h; reaching 100 the
    /// attribute is FFh and +11h what was given. With no attribute
    /// (+12h FFh), value C1h sets 5CA bit 1000h.
    fn equip_enchantment(&mut self, si: u16) {
        let ty = self.mem.r8(DS, si);
        if ty & 0x80 != 0 {
            self.mem.w8(DS, SPECIAL_WORN, ty);
            return;
        }
        let b = |m: &Self, f: u16| m.mem.r8(DS, si.wrapping_add(f));
        if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::ENCHANTED == 0 || b(self, item::ENCHANT) == 0xFF {
            return;
        }
        if b(self, item::ENCHANT_ATTR) == 0xFF {
            if b(self, item::ENCHANT_VALUE) == 0xC1 {
                let f = self.mem.r16(DS, 0x05CA) | 0x1000;
                self.mem.w16(DS, 0x05CA, f);
            }
            return;
        }
        let at = PLAYER.wrapping_add(crate::character::field::ATTRS).wrapping_add(b(self, item::ENCHANT_ATTR) as u16);
        let pct = crate::scale::to100(self.mem.r8(DS, at) as i16) as u16;
        let ring = self.item_category(DS, si) == category::ACCESSORY && self.armor_kind(DS, si) == 1;
        let (sum, gain) = if ring {
            let gain = self.mem.r8(DS, 0x4AFE_u16.wrapping_add(ty as u16));
            (pct & 0xFF00 | (pct as u8).wrapping_add(gain) as u16, gain)
        } else {
            (pct.wrapping_add(0x0A), 0x0A)
        };
        if sum >= 0x64 {
            self.mem.w8(DS, at, 0xFF);
            self.mem.w8(DS, si.wrapping_add(item::ENCHANT_VALUE), 0x64u16.wrapping_sub(pct) as u8);
            return;
        }
        self.mem.w8(DS, si.wrapping_add(item::ENCHANT_VALUE), gain);
        let v = crate::scale::to256(pct.wrapping_add(gain as u16) as i16);
        self.mem.w8(DS, at, v as u8);
    }

    /// 1194:18F6..195C. The selected item (SELECTED, slot number; FFh is
    /// none, which shows a message the port leaves to the front end) is
    /// dropped: copied to DROPPED (its slot number at DROPPED_INDEX),
    /// taken out of the inventory (1194:788D) and the weapon class made
    /// again; nothing is selected; the line count is taken again
    /// (1194:1CF0: the player's occupied slots, or the other list's
    /// count) unless the other list is up (OTHER_LIST), with the armour
    /// class (the redraw's 7719), and the first line shown kept in
    /// range; the copy is no longer worn and lands on the floor
    /// (`drop_to_floor`). Returns whether an item was dropped.
    pub fn drop_item(&mut self) -> bool {
        let index = self.mem.r8(DS, SELECTED);
        if index == 0xFF {
            return false;
        }
        let si = PLAYER_SLOTS.wrapping_add((index as u16).wrapping_mul(SLOT_SIZE));
        self.mem.w8(DS, DROPPED_INDEX, index);
        self.memmove(DS, si, DS, DROPPED, SLOT_SIZE);
        self.remove_item(DS, si, index as u16);
        self.player_weapon_class();
        self.mem.w8(DS, SELECTED, 0xFF);
        if self.mem.r8(DS, OTHER_LIST) == 0 {
            let mut al = self.mem.r8(DS, OTHER_COUNT);
            if self.mem.r8(DS, OWN_ITEMS) != 0 {
                al = self.count_items(DS, PLAYER_SLOTS).0 as u8;
            }
            self.mem.w8(DS, LAST_COUNT, al);
            self.player_armor_class();
        }
        let ax = (self.mem.r8(DS, LAST_COUNT) as u16).wrapping_add(self.mem.r16(DS, LINES)).wrapping_sub(7);
        if self.mem.r8(DS, FIRST_LINE) >= ax as u8 {
            let al = ax.wrapping_add(6) as u8;
            if self.mem.r8(DS, FIRST_LINE) > al {
                self.mem.w8(DS, FIRST_LINE, al);
            }
        }
        let f = self.mem.r8(DS, DROPPED.wrapping_add(item::FLAGS)) & 0x7F;
        self.mem.w8(DS, DROPPED.wrapping_add(item::FLAGS), f);
        self.drop_to_floor(DROPPED);
        true
    }

    /// 2433:09E1. The item at DGROUP `si` placed on the player's square:
    /// a record at FLOOR_ITEM (1, the kind of place `floor_kind`, the
    /// square (row, column doubled), the weather byte BB0A, FFFFh, FFh,
    /// 1, the item) appended to the level's item list in EMS bank 6
    /// (mapped meanwhile, when EMS is on with that bank). A record with
    /// FFFFh, FFh and a price of 0 gets 0Fh at +5 (2433:027D, after an
    /// INT 3 the game's DOS handler ignores).
    pub fn drop_to_floor(&mut self, si: u16) {
        use crate::level::var::{EMS_MAPPED, EMS_ON, EMS_PAGES};
        self.memmove(DS, si, DS, FLOOR_ITEM + 9, SLOT_SIZE);
        let mapped = self.mem.r8(DS, EMS_ON) != 0 && ITEMS_BANK < self.mem.r16(DS, EMS_PAGES);
        let old = self.mem.r16(DS, EMS_MAPPED) >> 2;
        self.ems_map(ITEMS_BANK);
        self.mem.w16(DS, FLOOR_ITEM + 5, 0xFFFF);
        self.mem.w8(DS, FLOOR_ITEM + 7, 0xFF);
        self.mem.w8(DS, FLOOR_ITEM, 1);
        let row = (self.mem.r16(DS, crate::movement::var::Y) >> 7) as u8;
        let col = ((self.mem.r16(DS, crate::movement::var::X) >> 7) as u8).wrapping_add((self.mem.r16(DS, crate::movement::var::X) >> 7) as u8);
        self.mem.w16(DS, FLOOR_ITEM + 2, (row as u16) << 8 | col as u16);
        self.mem.w8(DS, FLOOR_ITEM + 8, 1);
        let kind = self.floor_kind();
        self.mem.w8(DS, FLOOR_ITEM + 1, kind);
        let w = self.mem.r8(DS, 0xBB0A);
        self.mem.w8(DS, FLOOR_ITEM + 4, w);
        // 2433:0258
        let frame = self.mem.r16(DS, crate::movement::var::FRAME_SEG);
        let di = (self.mem.r8(DS, LEVEL_ITEMS) as u16).wrapping_mul(0x1C).wrapping_add(LEVEL_ITEM_LIST);
        if self.mem.r16(DS, FLOOR_ITEM + 5) == 0xFFFF
            && self.mem.r8(DS, FLOOR_ITEM + 7) == 0xFF
            && self.mem.r32(DS, FLOOR_ITEM + 0x13) == 0
        {
            self.mem.w16(DS, FLOOR_ITEM + 5, 0x0F);
        }
        self.memmove(DS, FLOOR_ITEM, frame, di, 0x1C);
        let n = self.mem.r8(DS, LEVEL_ITEMS).wrapping_add(1);
        self.mem.w8(DS, LEVEL_ITEMS, n);
        if mapped {
            self.ems_map(old);
        }
    }

    /// 2433:0201. The kind of place an item lies in: 5 in the wild, 3
    /// outdoors or anywhere but a building (5E6 not 4), else by the
    /// building (D48): 1 for 1, 2 for 3, 4 for 8 and 0Bh, 0 otherwise.
    pub fn floor_kind(&self) -> u8 {
        if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x200 != 0 {
            return 5;
        }
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 4 != 0 || self.mem.r8(DS, 0x05E6) != 4 {
            return 3;
        }
        match self.mem.r8(DS, 0x0D48) {
            1 => 1,
            3 => 2,
            8 | 0x0B => 4,
            _ => 0,
        }
    }
}
