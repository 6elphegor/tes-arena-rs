//! What a person in a fight is drawn as (2686:0423): the list of the
//! graphics files the fight loads for people (0000:1AF9), built at DGROUP
//! 1044 from the first person's record.
//!
//! The look (A836) is the armour most worn: plate, chain or leather (0..2)
//! when three or more pieces of that kind are worn with its cuirass among
//! them, else by class: a mage (class bit 20h) 4, class 4Ch 5 (a monk),
//! 0Fh 6 (a barbarian), anyone else 3 (unarmoured). The list holds, each
//! name with `@` for the view (1..5, front to back):
//! - the walk, `0@pltwlk.cfa` with the look's three letters;
//! - the attack: `att`, or `bow` for a bow (weapon class 6); a monk's
//!   kick (`0@mnkkic.cfa`), a mage's sword or staff (`magswd`, `magstf`,
//!   by weapon class 1 or 0);
//! - the weapon over the attack, by the weapon's kind (sword, axe, mace)
//!   and the look (plain, `p` for plate, `b` for a barbarian), unless
//!   the look has none;
//! - a shield (plain, or `pshld` in plate), if one is worn.
//!
//! A woman's files (record +7 nonzero) start with `1` but in plate. The
//! record's flags at +40Dh get bit 400h for a weapon drawn; the shield's
//! bit 800h goes astray (see `outfit_shield`). The drawing (0000:1B7F)
//! puts the attack over the walk's place and the weapon over the attack;
//! the shield it skips.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The list's place, and its end as it is built.
    pub const LIST: u16 = 0x1044;
    pub const LIST_END: u16 = 0xA840;
    /// The look (armour or class, 0..6), the weapon (an index into the
    /// weapons' names, 0Ch none), the shield (0 none, 1 plain, 2 plate),
    /// the class (record +5) and +7 (a woman).
    pub const LOOK: u16 = 0xA836;
    pub const WEAPON: u16 = 0xA838;
    pub const SHIELD: u16 = 0xA83C;
    pub const CLASS: u16 = 0xA83E;
    pub const WOMAN: u16 = 0xA842;
    /// Pieces of armour worn by kind (bytes, counted as words), and
    /// cuirasses by kind (12ABh).
    pub const WORN: u16 = 0x12A8;
    pub const CUIRASSES: u16 = 0x12AB;
}
use var::*;

/// Names: the looks' letters, then the walk, attack and bow files; the
/// mage's and monk's attacks; the weapons; the shields.
const LOOKS: u16 = 0x699A;
const WALK: u16 = 0x69B6;
const ATTACK: u16 = 0x69C3;
const BOW: u16 = 0x69D0;
const MAGE_ATTACKS: u16 = 0x69DD;
const KICK: u16 = 0x6A04;
const WEAPONS: u16 = 0x6A11;
const SHIELDS: u16 = 0x6A7A;
/// Weapon classes with a drawn weapon: blade, axe, mace.
const DRAWN: u16 = 0x6997;
/// The weapon class (1194:7316).
const WEAPON_CLASS: u16 = 0x0D49;

impl Machine {
    /// 2686:0423. The graphics list for the person at `rec` in `seg`
    /// (nothing unless +9 is set), at DGROUP 1044, which it returns (the
    /// original's SI). The weapon class at 0D49 is kept.
    pub fn person_outfit(&mut self, seg: u16, rec: u16) -> u16 {
        let kept = self.mem.r8(DS, WEAPON_CLASS);
        self.mem.w16(DS, LIST, 0);
        self.mem.w16(DS, LIST_END, LIST);
        if self.mem.r8(seg, rec.wrapping_add(9)) != 0 {
            let class = self.mem.r8(seg, rec.wrapping_add(5)) as u16;
            self.mem.w16(DS, CLASS, class);
            let woman = self.mem.r8(seg, rec.wrapping_add(7)) as u16;
            self.mem.w16(DS, WOMAN, woman);
            let look = self.outfit_look(seg, rec);
            self.mem.w16(DS, LOOK, look);
            let shield = self.outfit_shield(seg, rec);
            self.mem.w16(DS, SHIELD, shield);
            let weapon = self.outfit_weapon(seg, rec);
            self.mem.w16(DS, WEAPON, weapon);
            self.outfit_body(WALK);
            self.outfit_attack();
            self.outfit_end();
            let weapon = self.mem.r16(DS, WEAPON);
            if weapon != 0xC {
                let si = self.skip_strings(DS, WEAPONS, weapon);
                self.outfit_add(si);
            }
            self.outfit_end();
            let shield = self.mem.r16(DS, SHIELD);
            if shield != 0 {
                let si = self.skip_strings(DS, SHIELDS, shield.wrapping_sub(1));
                self.outfit_add(si);
            }
        }
        self.outfit_end();
        self.mem.w8(DS, WEAPON_CLASS, kept);
        LIST
    }

    /// An empty name at the list's end (not past it).
    fn outfit_end(&mut self) {
        let di = self.mem.r16(DS, LIST_END);
        self.mem.w8(DS, di, 0);
    }

    /// 2686:04B9. The look (0..6); the weapon becomes 0Ch, 0Dh for a mage.
    fn outfit_look(&mut self, seg: u16, rec: u16) -> u16 {
        self.mem.w16(DS, WEAPON, 0xC);
        self.outfit_count_armor(seg, rec.wrapping_add(crate::inventory::SLOTS));
        let worn = (0..3u16).find(|&k| {
            self.mem.r8(DS, WORN + k) >= 3 && self.mem.r16(DS, CUIRASSES + k) != 0
        });
        let class = self.mem.r16(DS, CLASS);
        if worn.is_none() {
            if class & 0x20 != 0 {
                self.mem.w16(DS, WEAPON, 0xD);
                return 4;
            }
            if class == 0x4C {
                return 5;
            }
        }
        // A barbarian whatever is worn.
        if class == 0xF {
            6
        } else {
            worn.unwrap_or(3)
        }
    }

    /// 2686:0676. Counts the armour among as many slots from `si` as are
    /// occupied (taken in a row, occupied or not): pieces but shields by
    /// kind (word counters a byte apart), and cuirasses (base type 0).
    fn outfit_count_armor(&mut self, seg: u16, mut si: u16) {
        for i in 0..5 {
            self.mem.w8(DS, WORN + i, 0);
        }
        let (n, _) = self.count_items(seg, si);
        for _ in 0..n {
            if self.item_category(seg, si) == 0 {
                let (base, _) = self.base_type(self.mem.r8(seg, si));
                let kind = self.armor_kind(seg, si);
                if base == 0 {
                    let v = self.mem.r16(DS, CUIRASSES + kind).wrapping_add(1);
                    self.mem.w16(DS, CUIRASSES + kind, v);
                }
                if !(7..=0xA).contains(&base) {
                    let v = self.mem.r16(DS, WORN + kind).wrapping_add(1);
                    self.mem.w16(DS, WORN + kind, v);
                }
            }
            si = si.wrapping_add(crate::inventory::SLOT_SIZE);
        }
    }

    /// 2686:0513. The shield drawn: 0 none, 1 plain, 2 in plate; none for
    /// looks 4 and up. Bit 800h is cleared at +40Dh of the slots' place
    /// (+47Bh of the record) and set at +40Dh of the shield's slot.
    fn outfit_shield(&mut self, seg: u16, rec: u16) -> u16 {
        let slots = rec.wrapping_add(crate::inventory::SLOTS);
        let at = slots.wrapping_add(0x40D);
        let f = self.mem.r16(seg, at) & !0x800;
        self.mem.w16(seg, at, f);
        let Some(shield) = self.equipped_shield(seg, slots) else {
            return 0;
        };
        let look = self.mem.r16(DS, LOOK);
        if look >= 4 {
            return 0;
        }
        let at = shield.wrapping_add(0x40D);
        let f = self.mem.r16(seg, at) | 0x800;
        self.mem.w16(seg, at, f);
        if look == 0 {
            2
        } else {
            1
        }
    }

    /// 2686:0541. The weapon drawn: an index into the weapons' names by
    /// the weapon class (blade, axe, mace: 0..2; +3 in plate, +6 for a
    /// barbarian), with bit 400h at the record's +40Dh; otherwise, for
    /// looks 4 and 5 or another class, the bit is cleared and the index
    /// is what the class left (bare hands: the class, 7..9).
    fn outfit_weapon(&mut self, seg: u16, rec: u16) -> u16 {
        let at = rec.wrapping_add(0x40D);
        let class = self.weapon_class(seg, rec);
        let clear = |m: &mut Machine| {
            let f = m.mem.r16(seg, at) & !0x400;
            m.mem.w16(seg, at, f);
        };
        if class & 0xFF >= 7 {
            clear(self);
            return class;
        }
        let f = self.mem.r16(seg, at) | 0x400;
        self.mem.w16(seg, at, f);
        // 2686:0590: an axe's class 3 is drawn as 2, 4 as 5.
        let al = match class as u8 {
            3 => 2,
            4 => 5,
            c => c,
        };
        let class = (class & 0xFF00) | al as u16;
        let Some(k) = (0..3u16).find(|&k| self.mem.r8(DS, DRAWN + k) == al) else {
            clear(self);
            return class;
        };
        match self.mem.r16(DS, LOOK) {
            0 => k + 3,
            6 => k + 6,
            l if l > 3 => {
                clear(self);
                k
            }
            _ => k,
        }
    }

    /// 2686:059D. The attack's file.
    fn outfit_attack(&mut self) {
        let look = self.mem.r16(DS, LOOK);
        let class = self.mem.r8(DS, WEAPON_CLASS);
        let si = if look == 5 {
            KICK
        } else if look == 4 && class == 1 {
            MAGE_ATTACKS
        } else if look == 4 && class == 0 {
            self.skip_strings(DS, MAGE_ATTACKS, 1)
        } else if class == 6 {
            BOW
        } else {
            ATTACK
        };
        self.outfit_body(si);
    }

    /// 2686:05E3. Adds the name at `si`, with the look's three letters
    /// from its third character.
    fn outfit_body(&mut self, si: u16) {
        let di = self.mem.r16(DS, LIST_END);
        self.outfit_copy(si);
        let look = self.mem.r16(DS, LOOK);
        let letters = self.skip_strings(DS, LOOKS, look);
        for i in 0..3 {
            let b = self.mem.r8(DS, letters.wrapping_add(i));
            self.mem.w8(DS, di.wrapping_add(2 + i), b);
        }
        self.outfit_woman(di);
    }

    /// 2686:061E (after its skip). Adds the name at `si`.
    fn outfit_add(&mut self, si: u16) {
        let di = self.mem.r16(DS, LIST_END);
        self.outfit_copy(si);
        self.outfit_woman(di);
    }

    /// 0000:9CA0: the name copied to the list's end, NUL included.
    fn outfit_copy(&mut self, mut si: u16) {
        let mut di = self.mem.r16(DS, LIST_END);
        loop {
            let b = self.mem.r8(DS, si);
            self.mem.w8(DS, di, b);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if b == 0 {
                break;
            }
        }
        self.mem.w16(DS, LIST_END, di);
    }

    /// A woman's file (not in plate) starts with 1.
    fn outfit_woman(&mut self, di: u16) {
        if self.mem.r16(DS, LOOK) != 0 && self.mem.r16(DS, WOMAN) != 0 {
            self.mem.w8(DS, di, b'1');
        }
    }
}
