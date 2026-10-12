//! Melee combat: to-hit, hit location, criticals, damage and weapon wear
//! (segment 2AA1, plus the percentile roll at 1194:556E).
//!
//! Combat records share the character record layout (see
//! `character::field`); the player is at DGROUP 01A4, other combatants
//! live in the segment held at DGROUP 1042. Flag word +40D bit 0x1000
//! marks a creature (as opposed to the player or a human NPC).
//!
//! These routines assume DS is DGROUP where the original does not set it
//! itself, which holds for both callers (2AA1:0000 and 2AA1:00D8).
//!
//! Several routines call inventory code in segment 1194 that is not
//! ported yet; its results are passed in through `Gear`.

use crate::character::field;
use crate::machine::Machine;
use crate::mem::DS;
use crate::scale::to256;

/// A combat record: segment and offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ptr {
    pub seg: u16,
    pub off: u16,
}

impl Ptr {
    pub fn new(seg: u16, off: u16) -> Self {
        Self { seg, off }
    }
}

/// DGROUP variables used by combat.
pub mod var {
    /// Hit location of the current attack.
    pub const LOCATION: u16 = 0x129A;
    /// Hit chance of the current attack (256 scale).
    pub const CHANCE: u16 = 0x129C;
    /// Added to the hit chance against non-creatures.
    pub const CHANCE_BONUS: u16 = 0x05C6;
    /// Bit 0x20 disables weapon wear and the Ranger bonus.
    pub const FLAGS_5CA: u16 = 0x05CA;
    /// Offset of the attacker / defender record.
    pub const ATTACKER: u16 = 0x1400;
    pub const DEFENDER: u16 = 0x1402;
    /// Segment holding non-player combatants; the current creature
    /// target is at offset 0.
    pub const CREATURE_SEG: u16 = 0x1042;
    /// Something the creature-type-7 damage halving depends on.
    pub const D49: u16 = 0x0D49;
    pub const PLAYER_LEVEL: u16 = 0x01AA;
    pub const PLAYER_HP: u16 = 0x01FD;
}

/// DGROUP tables.
pub mod table {
    /// Hand-to-hand damage (min, max) by gauntlet material; index 1 when bare.
    pub const FIST_DAMAGE: u16 = 0x8270;
    /// Weapon wear per hit, percent, by material.
    pub const WEAR: u16 = 0x8276;
    /// Hit location thresholds for a 0..19 roll.
    pub const LOCATIONS: u16 = 0x8291;
    /// Creature types weapons of material 2 do double damage to.
    pub const SLAYABLE: u16 = 0x8298;
    /// Minimum material to hurt creature types 0x14 and up.
    pub const MATERIAL_NEEDED: u16 = 0x829F;
}

/// Results of inventory routines (segment 1194) that are not ported yet.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gear {
    /// 1194:7B1C: armor value of the defender at the hit location.
    pub armor: i16,
    /// 1194:7510: offset of the attacker's weapon item, in the
    /// attacker's segment.
    pub weapon: Option<u16>,
    /// 1194:7449 (item 1, bare-handed): offset of worn gauntlets.
    pub gauntlets: Option<u16>,
    /// 1194:39ED: damage bonus of the weapon.
    pub weapon_bonus: u16,
    /// 1194:78DE: a missile weapon is in hand.
    pub missile: bool,
}

/// Item record fields (the shared layout).
pub use crate::item;

impl Machine {
    fn rec_b(&self, p: Ptr, f: u16) -> u8 {
        self.mem.r8(p.seg, p.off.wrapping_add(f))
    }

    fn rec_w(&self, p: Ptr, f: u16) -> u16 {
        self.mem.r16(p.seg, p.off.wrapping_add(f))
    }

    fn is_creature(&self, p: Ptr) -> bool {
        self.rec_w(p, field::FLAGS) & field::FLAG_CREATURE != 0
    }

    /// The current creature target: offset 0 of the creature segment.
    fn creature_target(&self) -> Ptr {
        Ptr::new(self.mem.r16(DS, var::CREATURE_SEG), 0)
    }

    /// 1194:556E. 0..99.
    pub fn percent(&mut self) -> u16 {
        self.rng.next() % 100
    }

    /// 2AA1:0638. Level difference ×5% plus 50%, plus the luck difference;
    /// Redguards, Dark Elves and Wood Elves (non-creatures) add their level.
    pub fn hit_base(&self, att: Ptr, def: Ptr) -> u16 {
        let diff = self.rec_b(att, 6).wrapping_sub(self.rec_b(def, 6)) as i8 as i16;
        let mut ax = (to256(diff * 5) as u16).wrapping_add(0x80);
        ax = ax.wrapping_add(self.rec_w(att, field::LCK_MOD).wrapping_sub(self.rec_w(def, field::LCK_MOD)));
        if !self.is_creature(att) && matches!(self.rec_b(att, 4), 1 | 3 | 5) {
            ax = ax.wrapping_add(to256(self.rec_b(att, 6) as i16) as u16);
        }
        ax
    }

    /// 2AA1:060F. `hit_base` minus armor plus attack (+49) minus defence (+47).
    pub fn hit_chance(&self, att: Ptr, def: Ptr, gear: &Gear) -> u16 {
        let ax = self.hit_base(att, def).wrapping_sub(to256(gear.armor) as u16);
        ax.wrapping_add(self.rec_w(att, field::ATTACK_MOD).wrapping_sub(self.rec_w(def, field::DEFENCE_MOD)))
    }

    /// 2AA1:058B. Rolls 0..255 against the hit chance; returns (hit, roll).
    ///
    /// Against non-creatures the chance gets DGROUP 05C6 added, and Monk
    /// and Acrobat defenders scale it by (level+1)/32 using 8-bit maths.
    /// The floor is 20% plus the *player's* level in percent, whoever
    /// is fighting. The flag checks read the defender at DGROUP 1402 in
    /// the defender's segment.
    pub fn roll_to_hit(&mut self, att: Ptr, def: Ptr, location: u16, gear: &Gear) -> Option<(bool, u16)> {
        self.mem.w16(DS, var::LOCATION, location);
        let chance = self.hit_chance(att, def, gear);
        self.mem.w16(DS, var::CHANCE, chance);
        let roll = self.rng.range(0, 0x100)?;

        let d = Ptr::new(def.seg, self.mem.r16(DS, var::DEFENDER));
        if !self.is_creature(d) {
            let c = self.mem.r16(DS, var::CHANCE).wrapping_add(self.mem.r16(DS, var::CHANCE_BONUS));
            self.mem.w16(DS, var::CHANCE, c);
            if matches!(self.rec_b(d, 5), 0x4C | 0xC9) {
                let scaled = (c.wrapping_mul(8) >> 8) as u8 as u16;
                let level = self.rec_b(d, 6).wrapping_add(1) as u16;
                self.mem.w16(DS, var::CHANCE, scaled * level);
            }
        }
        let floor = ((self.mem.r8(DS, var::PLAYER_LEVEL) as u16) << 8) / 100 + 0x33;
        if (self.mem.r16(DS, var::CHANCE) as i16) < floor as i16 {
            self.mem.w16(DS, var::CHANCE, floor);
        }
        Some(((roll as i16) < self.mem.r16(DS, var::CHANCE) as i16, roll))
    }

    /// 2AA1:0687. Hit location 0..7 from a 0..19 roll.
    pub fn hit_location(&mut self) -> Option<u16> {
        let r = self.rng.range(0, 20)? as u8;
        let mut loc = 0;
        while loc < 7 && r > self.mem.r8(DS, table::LOCATIONS + loc) {
            loc += 1;
        }
        Some(loc)
    }

    /// 2AA1:0852. Damage multiplier: 3 on a critical hit, else 1.
    ///
    /// Only classes with bit 0x40 can crit, at (multiplier × level)%:
    /// Thief 2, Assassin 3, Monk 3 at half level, Archer 3 but only with
    /// a missile weapon; others 1. Except for Archers, a missile weapon
    /// rules out a critical.
    pub fn critical_multiplier(&mut self, att: Ptr, gear: &Gear) -> u16 {
        let mut level = self.rec_b(att, 6) as u16;
        let class = self.rec_b(att, 5);
        if class & 0x40 == 0 {
            return 1;
        }
        let mut mult = 1u16;
        let idx = class & 0x1f;
        let archer = idx > 10 && class == 0x4D;
        if idx == 10 {
            mult = 2;
        } else if archer {
            if !gear.missile {
                return 1;
            }
            mult = 3;
        } else if idx > 10 && class == 0xCB {
            mult = 3;
        }
        if !archer {
            if gear.missile {
                return 1;
            }
            if class == 0x4C {
                mult = 3;
                level >>= 1;
            }
        }
        let chance = mult.wrapping_mul(level);
        if self.percent() >= chance { 1 } else { 3 }
    }

    /// 2AA1:0298. Creature types 0x11..0x13 need a weapon with a material
    /// of at least type-0x11+3. Returns (can hurt, type index or FFFF).
    fn special_low(&self, weapon: Ptr) -> (bool, u16) {
        let mon = self.creature_target();
        let ty = self.rec_b(mon, 4);
        if !self.is_creature(mon) || !(0x11..=0x13).contains(&ty) {
            return (false, 0xFFFF);
        }
        let t = ty as u16 - 0x11;
        let ok = self.rec_b(weapon, item::FLAGS) & 1 != 0 && self.rec_b(weapon, item::MATERIAL) as i8 >= (t as u8 + 3) as i8;
        (ok, t)
    }

    /// 2AA1:02DB. Creature types 0x14 and up need a minimum material from
    /// a table (read through the weapon's segment). Returns (can hurt, AX).
    fn special_high(&self, weapon: Ptr) -> (bool, u16) {
        let mon = self.creature_target();
        let ty = self.rec_b(mon, 4);
        if !self.is_creature(mon) || ty < 0x14 {
            return (false, 0xFFFF);
        }
        let t = ty as u16 - 0x14;
        if self.rec_b(weapon, item::FLAGS) & 1 == 0 {
            return (false, t);
        }
        let mat = self.rec_b(weapon, item::MATERIAL);
        if t == 2 && mat == 2 {
            return (true, t);
        }
        let need = self.mem.r8(weapon.seg, table::MATERIAL_NEEDED.wrapping_add(t));
        (mat as i8 >= need as i8, need as u16)
    }

    /// 2AA1:0544. Whether `v` is in the 7-byte table at seg:8298.
    fn in_slay_table(&self, seg: u16, v: u8) -> bool {
        (0..7).any(|i| self.mem.r8(seg, table::SLAYABLE + i) == v)
    }

    /// 2AA1:01FD. The creature target's type is in the slay table.
    fn slays(&self, seg: u16) -> bool {
        let mon = self.creature_target();
        self.is_creature(mon) && self.in_slay_table(seg, self.rec_b(mon, 4))
    }

    /// 2AA1:017C. Weapon wear; returns the damage, doubled for material-2
    /// weapons against creatures in the slay table.
    ///
    /// Wear is damage × material rate / 100, halved if the item's first
    /// byte (read through `es`) has bit 0x80. When the weapon can't hurt
    /// a special creature, the type index left in AX is used as the wear
    /// instead. A condition that drops to 0, or below 0 by less than
    /// 15536, breaks the weapon (the message is not ported).
    pub fn weapon_wear(&mut self, weapon: Ptr, es: u16, damage: u16) -> u16 {
        let mut saved = damage;
        let (cf, t) = self.special_low(weapon);
        let mut ax = t;
        let mut wear_now = false;
        if !cf {
            let al = t as u8;
            if al != 0xFF && al != 0 {
                wear_now = true;
            } else {
                let (cf2, t2) = self.special_high(weapon);
                ax = t2;
                if cf2 && t2 as u8 == 0 {
                    wear_now = true;
                }
            }
        }
        if !wear_now {
            ax = damage;
            if self.slays(weapon.seg) && self.rec_b(weapon, item::MATERIAL) == 2 {
                ax <<= 1;
            }
            saved = ax;
            // Unlike the other checks of this flag, this one reads 05CA
            // through DS, i.e. the weapon's segment.
            if self.mem.r16(weapon.seg, var::FLAGS_5CA) & 0x20 == 0 {
                let mut m = 1;
                if self.rec_b(weapon, item::FLAGS) & 1 != 0 && self.rec_b(weapon, item::MATERIAL) != 0xFF {
                    m = self.rec_b(weapon, item::MATERIAL) as u16;
                }
                let rate = self.mem.r8(DS, table::WEAR.wrapping_add(m)) as u16;
                ax = ax.wrapping_mul(rate).wrapping_add(0x32) / 100;
            }
        }
        if self.mem.r8(es, weapon.off) & 0x80 != 0 {
            ax >>= 1;
        }
        let cond = self.rec_w(weapon, item::HEALTH).wrapping_sub(ax);
        self.mem.w16(weapon.seg, weapon.off.wrapping_add(item::HEALTH), cond);
        if (cond as i16) < 0 && cond > 50000 {
            self.mem.w16(weapon.seg, weapon.off.wrapping_add(item::HEALTH), 0);
        }
        saved
    }

    /// 2AA1:0525. Rangers add level+1 to the low byte of the damage
    /// (8-bit, so it can wrap), unless the defender is a creature and the
    /// *damage* byte matches the slay table: the original compares the
    /// damage where it means the creature type. Both the defender offset
    /// (1402) and its flags are read through the attacker's segment.
    fn ranger_bonus(&self, att: Ptr, damage: u16) -> u16 {
        if self.rec_b(att, 5) != 0x0E {
            return damage;
        }
        let d = Ptr::new(att.seg, self.mem.r16(att.seg, var::DEFENDER));
        if self.is_creature(d) && self.in_slay_table(att.seg, damage as u8) {
            return damage;
        }
        let lo = (damage as u8).wrapping_add(self.rec_b(att, 6)).wrapping_add(1);
        damage & 0xFF00 | lo as u16
    }

    /// 2AA1:04A5. Melee damage by the attacker at `seg`:[1400].
    ///
    /// Rolls the weapon's (or fists') damage range, adds the weapon bonus
    /// and the damage modifier (+39), multiplies by `mult` (criticals),
    /// then applies the Ranger bonus and weapon wear. Bare-handed with
    /// flag 5CA bit 0x20 set, the player loses 1 HP 80% of the time.
    /// None where the original divides by zero (min == max).
    pub fn weapon_damage(&mut self, seg: u16, es: u16, mult: u16, gear: &Gear) -> Option<u16> {
        let (lo, hi) = match gear.weapon {
            Some(w) => {
                let w = Ptr::new(seg, w);
                (self.rec_b(w, item::DAMAGE_MIN), self.rec_b(w, item::DAMAGE_MAX))
            }
            None => {
                let i = match gear.gauntlets {
                    Some(g) => ((self.mem.r8(seg, g.wrapping_add(item::FLAGS)) & 0x30) >> 4) as u16 * 2,
                    None => 2,
                };
                (self.mem.r8(DS, table::FIST_DAMAGE + i), self.mem.r8(DS, table::FIST_DAMAGE + i + 1))
            }
        };
        let mut r = self.rng.range(lo as u16, hi as u16)?;
        let att = Ptr::new(seg, self.mem.r16(DS, var::ATTACKER));
        if gear.weapon.is_some() {
            r = r.wrapping_add(gear.weapon_bonus);
        }
        let mut dmg = r.wrapping_add(self.rec_w(att, field::DAMAGE_MOD)).wrapping_mul(mult);
        if self.mem.r16(DS, var::FLAGS_5CA) & 0x20 == 0 {
            dmg = self.ranger_bonus(att, dmg);
        }
        match gear.weapon {
            Some(w) => dmg = self.weapon_wear(Ptr::new(seg, w), es, dmg),
            None => {
                if self.mem.r16(DS, var::FLAGS_5CA) & 0x20 != 0 && self.percent() >= 20 {
                    let hp = self.mem.r16(DS, var::PLAYER_HP).wrapping_sub(1);
                    self.mem.w16(DS, var::PLAYER_HP, hp);
                }
            }
        }
        Some(dmg)
    }

    /// 2AA1:0325. Creature type 7 takes half damage while DGROUP 0D49 is
    /// 1, 5 or 6.
    pub fn creature_damage_taken(&self, damage: u16) -> u16 {
        let mon = self.creature_target();
        if self.is_creature(mon) && self.rec_b(mon, 4) == 7 && matches!(self.mem.r8(DS, var::D49), 1 | 5 | 6) {
            damage >> 1
        } else {
            damage
        }
    }

    /// 2AA1:0112..0122. A creature's attack damage: its +09..+0A range.
    pub fn creature_damage(&mut self, att: Ptr) -> Option<u16> {
        self.rng.range(self.rec_b(att, field::CREATURE_MIN_DAMAGE) as u16, self.rec_b(att, field::CREATURE_MAX_DAMAGE) as u16)
    }
}
