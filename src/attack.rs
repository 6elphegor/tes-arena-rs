//! A blow at the player (2AA1:00D8): the to-hit roll, the damage from a
//! creature's range or a person's weapon, wear on the armour that takes
//! it, the magic shield, and death.
//!
//! The pieces are in `combat`, `inventory` and `effects`; this ties
//! them together in the original's order and fills in what `combat`
//! takes as `Gear` with the inventory routines the original calls. The
//! sound (23BA:002A) and the message box of a broken special item
//! (1194:A36C) are not ported.

use crate::character::{field, PLAYER};
use crate::combat::{var, Gear, Ptr};
use crate::inventory::{item, LAST_COUNT, PLAYER_SLOTS, SLOT_SIZE};
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod avar {
    /// The attack's argument (AX on entry); unused by this blow.
    pub const ARG: u16 = 0x1298;
    /// 0000:07F7 points 194B at one of two hurt palettes, unless 1949 is set.
    pub const PALETTE_LOCK: u16 = 0x1949;
    pub const PALETTE: u16 = 0x194B;
    /// The player's HP is low (bit 0x80; also "effects changed").
    pub const CHANGED: u16 = 0x0D39;
    /// The player's weapon class (1194:7316), which some creatures resist.
    pub const WEAPON_CLASS: u16 = 0x0D49;
    /// The message shown over the view: far pointer and time.
    pub const MESSAGE: u16 = 0x061A;
    pub const MESSAGE_TIME: u16 = 0x061E;
    pub const MESSAGE_BUF: u16 = 0x7979;
    /// "Your %s is broken."
    pub const BROKEN_FORMAT: u16 = 0x827E;
    pub const FLAGS_5E0: u16 = 0x05E0;
    /// The message box a special item's break shows (1194:A36C): text
    /// number and the item.
    pub const BOX_TEXT: u16 = 0x137A;
    pub const BOX_ITEM: u16 = 0x1AA4;
}

impl Machine {
    /// 2AA1:00D8. The record at DGROUP 1400 in the creature segment hits
    /// at the player (1402 = 01A4), with `arg` in AX (0 from the actors'
    /// frame). None where the original faults (an empty damage range).
    pub fn attack_player(&mut self, arg: u16) -> Option<()> {
        self.mem.w16(DS, avar::ARG, arg);
        let location = self.hit_location()?;
        let att = Ptr::new(self.mem.r16(DS, var::CREATURE_SEG), self.mem.r16(DS, var::ATTACKER));
        let def = Ptr::new(DS, self.mem.r16(DS, var::DEFENDER));
        let armor = self.armor_at(def.seg, def.off, location) as i16;
        let gear = Gear { armor, ..Gear::default() };
        let (hit, _) = self.roll_to_hit(att, def, location, &gear)?;
        if hit {
            // 2AA1:00F7: a blow landing sounds 0Ah.
            self.sound_here(0xA)?;
            self.blow(att, location)?;
        }
        // 2AA1:0166: unsigned, so a negative HP never counts as low.
        let hp = self.mem.r16(DS, PLAYER + field::HP);
        if hp <= self.mem.r16(DS, PLAYER + field::MAX_HP) / 10 {
            let c = self.mem.r8(DS, avar::CHANGED) | 0x80;
            self.mem.w8(DS, avar::CHANGED, c);
        }
        Some(())
    }

    /// 2AA1:00FE..0161: the blow lands.
    fn blow(&mut self, att: Ptr, location: u16) -> Option<()> {
        let damage = if self.mem.r16(att.seg, att.off.wrapping_add(field::FLAGS)) & field::FLAG_CREATURE != 0 {
            // 2AA1:0459 looks for armour of category 4, which no item has.
            let armoured = self.find_player_equipped(DS, att.off, location, 4).0;
            self.creature_special_attack(att.seg, att.off, armoured)?;
            self.creature_damage(att)?
        } else {
            let mult = self.person_critical(att);
            let gear = self.attacker_gear(att);
            // A person's weapon is in the creature segment: it can't break.
            self.weapon_damage(att.seg, DS, mult, &gear)?.0
        };
        let damage = self.armor_wear(DS, PLAYER, location, damage)?;
        // 0000:07F7
        if self.mem.r16(DS, avar::PALETTE_LOCK) == 0 {
            self.mem.w16(DS, avar::PALETTE, if damage >= 15 { 0x3C42 } else { 0x3C62 });
        }
        let through = self.absorb_damage(DS, PLAYER, damage);
        let hp = self.mem.r16(DS, PLAYER + field::HP).wrapping_sub(through);
        self.mem.w16(DS, PLAYER + field::HP, hp);
        if hp as i16 <= 0 {
            self.player_dies();
        }
        Some(())
    }

    /// 2AA1:0852 for a person, with 1194:78DE for its missile check.
    fn person_critical(&mut self, att: Ptr) -> u16 {
        let m = self.missile_weapon(att.seg, att.off);
        self.critical_multiplier(att, &Gear { missile: m.is_some(), missile_cx: m.unwrap_or(0), ..Gear::default() })
    }

    /// What 2AA1:04A5 asks the inventory for: the attacker's weapon
    /// (1194:7510), else gauntlets (1194:7449, searched from SI = FFFF,
    /// i.e. through the player's slot offsets in the attacker's segment),
    /// else the weapon's racial bonus (1194:39ED).
    fn attacker_gear(&mut self, att: Ptr) -> Gear {
        let w = self.equipped_weapon(att.seg, att.off);
        if w == 0xFFFF {
            let (found, g) = self.find_player_equipped(att.seg, 0xFFFF, 1, 0);
            return Gear { gauntlets: found.then_some(g), ..Gear::default() };
        }
        Gear { weapon: Some(w), weapon_bonus: self.weapon_bonus(att.seg, att.off), ..Gear::default() }
    }

    /// 2AA1:046E. The armour of record `rec` at the hit location takes
    /// wear: a shield covering it, else the piece worn there (looked up
    /// through the player's slot offsets, in `seg`). When a shield
    /// doesn't cover the location the search starts from the shield's
    /// slot. Returns the damage (doubled by `weapon_wear` in its odd
    /// case). ES is DGROUP throughout.
    pub(crate) fn armor_wear(&mut self, seg: u16, rec: u16, location: u16, damage: u16) -> Option<u16> {
        let slots = rec.wrapping_add(crate::inventory::SLOTS);
        let (covered, si) = self.shield_at(seg, slots, location);
        let piece = if covered {
            si
        } else {
            match self.find_player_equipped(seg, si, location, 0) {
                (true, p) => p,
                _ => return Some(damage),
            }
        };
        self.wear_item(Ptr::new(seg, piece), DS, damage)
    }

    /// `weapon_wear`, then 2AA1:01EF: an item of the player's (DS =
    /// DGROUP) worn to 0 breaks.
    pub fn wear_item(&mut self, it: Ptr, es: u16, damage: u16) -> Option<u16> {
        let d = self.weapon_wear(it, es, damage);
        if it.seg == DS && self.mem.r16(DS, it.off.wrapping_add(item::HEALTH)) == 0 {
            self.item_broken(it.off)?;
        }
        Some(d)
    }

    /// 2AA1:021B. The player's item at DGROUP `si` broke. A special item
    /// (type bit 0x80) shows a message box and leaves the inventory;
    /// another says "Your %s is broken." over the view and is unequipped.
    /// None where naming the item faults.
    pub fn item_broken(&mut self, si: u16) -> Option<()> {
        let ty = self.mem.r8(DS, si);
        if ty & 0x80 != 0 {
            self.mem.w16(DS, avar::BOX_TEXT, 0x1F8);
            self.mem.w8(DS, avar::BOX_ITEM, ty & 0x7F);
            let index = si.wrapping_sub(PLAYER_SLOTS) / SLOT_SIZE;
            self.remove_item(DS, si, index);
            self.player_weapon_class();
            return Some(());
        }
        let flags = self.mem.r8(DS, si.wrapping_add(item::FLAGS));
        self.mem.w8(DS, si.wrapping_add(item::FLAGS), flags | item::UNIDENTIFIED);
        let name = self.item_name(DS, si)?;
        self.sprintf_s((DS, avar::MESSAGE_BUF), (DS, avar::BROKEN_FORMAT), &[(DS, name)]);
        self.mem.w16(DS, avar::MESSAGE, avar::MESSAGE_BUF);
        self.mem.w16(DS, avar::MESSAGE + 2, DS);
        self.mem.w16(DS, avar::MESSAGE_TIME, 0x1E);
        self.mem.w8(DS, si.wrapping_add(item::FLAGS), flags & 0x7F);
        let f = self.mem.r16(DS, avar::FLAGS_5E0) & !0x10;
        self.mem.w16(DS, avar::FLAGS_5E0, f);
        self.player_weapon_class();
        Some(())
    }

    /// 1194:7316 for the player.
    pub fn player_weapon_class(&mut self) {
        self.weapon_class(DS, PLAYER);
    }

    /// 1194:7316. The weapon class of the record `rec` in `seg` into
    /// DGROUP 0D49, and returned: by base type, 0 for type 0, 1 below
    /// 0Bh, 2..5 for 0Bh..0Eh, 5 for 0Fh and 6 above; 1 for a weapon
    /// whose condition is FFFF. Bare-handed, 7 plus the armour kind of
    /// equipped gauntlets (2 without), searched from SI = FFFF over the
    /// occupied-slot count, both read in `seg` (the count meant is
    /// DGROUP's, the same for the player).
    pub fn weapon_class(&mut self, seg: u16, rec: u16) -> u16 {
        let w = self.equipped_weapon(seg, rec);
        let class = if w == 0xFFFF {
            let n = self.mem.r8(seg, LAST_COUNT) as u16;
            let kind = match self.find_equipped(seg, 0xFFFF, n, 3, 1, false) {
                (true, s) => self.armor_kind(seg, s),
                _ => 2,
            };
            kind.wrapping_add(7)
        } else if self.mem.r16(seg, w.wrapping_add(item::HEALTH)) == 0xFFFF {
            1 // AX as 1194:7510 leaves it on a find
        } else {
            match self.base_type(self.mem.r8(seg, w)).0 {
                0 => 0,
                b if b < 0xB => 1,
                b if b > 0xF => 6,
                0xF => 5,
                b => b - 0xB + 2,
            }
        };
        self.mem.w8(DS, avar::WEAPON_CLASS, class as u8);
        class
    }

    /// 2686:02B4. The player dies: flags 05B1 bit 0x100 and C398 bit
    /// 0x80, effects on the player end, and 29CF:0CEF clears the
    /// spell-casting state.
    pub fn player_dies(&mut self) {
        let f = self.mem.r16(DS, 0x05B1) | 0x100;
        self.mem.w16(DS, 0x05B1, f);
        let f = self.mem.r16(DS, 0xC398) | 0x80;
        self.mem.w16(DS, 0xC398, f);
        self.clear_effects_on(DS, PLAYER);
        self.spells_clear();
    }
}
