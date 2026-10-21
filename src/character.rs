//! Character statistics: creation rolls, attribute modifiers, hit and
//! spell points (segment 1194).
//!
//! Routines work on a character record in DGROUP at `rec` (the player is
//! at 01A4). Attributes are percent in the record and are converted to the
//! 256 scale around the modifier calculation.

use crate::dice::dice;
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::scale::{attrs_to100, attrs_to256, to100, to100_u, to256};

pub const PLAYER: u16 = 0x01A4;

/// Record field offsets. Names marked `?` are guesses from usage.
pub mod field {
    /// Race for characters; creature type for creatures.
    pub const RACE: u16 = 0x04;
    /// Low 5 bits: class index (see `CLASS_IDS`); 0x20: spellcaster.
    pub const CLASS: u16 = 0x05;
    /// Level, 0-based (8-bit).
    pub const LEVEL: u16 = 0x06;
    pub const SEX: u16 = 0x07;
    /// Portrait: race × 32 + 0..31 (0..30 for race 7).
    pub const PORTRAIT: u16 = 0x08;
    /// Name, written by 1194:080B (`npc_name`).
    pub const NAME: u16 = 0x09;
    /// Creature damage range, 8-bit.
    pub const CREATURE_MIN_DAMAGE: u16 = 0x09;
    pub const CREATURE_MAX_DAMAGE: u16 = 0x0A;
    /// STR INT WIL AGI SPD END PER LCK.
    pub const ATTRS: u16 = 0x29;
    /// Base attributes, a copy of ATTRS made at creation; status effects
    /// snapshot and restore against them.
    pub const BASE_ATTRS: u16 = 0x31;
    pub const DAMAGE_MOD: u16 = 0x39;
    pub const MAX_WEIGHT: u16 = 0x3B; // ?
    pub const MAX_SPELL_POINTS_2: u16 = 0x3D;
    pub const INT_MOD: u16 = 0x3F;
    pub const MAGIC_DEF: u16 = 0x43; // ?
    pub const WIL_MOD_HALF: u16 = 0x45;
    /// AGI - 128, subtracted from an attacker's chance to hit.
    pub const DEFENCE_MOD: u16 = 0x47;
    /// AGI - 128, added to the chance to hit.
    pub const ATTACK_MOD: u16 = 0x49;
    pub const AGI_MOD_4B: u16 = 0x4B;
    pub const SPD_MOD: u16 = 0x4D;
    pub const HP_MOD: u16 = 0x4F;
    pub const END_X2: u16 = 0x51;
    pub const HEAL_MOD: u16 = 0x53; // ?
    pub const PER_MOD: u16 = 0x55;
    /// Always 50 (see `attribute_modifiers`); enters to-hit as a
    /// difference, so it cancels out.
    pub const LCK_MOD: u16 = 0x57;
    pub const HP: u16 = 0x59;
    pub const MAX_HP: u16 = 0x5B;
    /// Current fatigue (effects ticks step it; scaled by 64).
    pub const FATIGUE: u16 = 0x5D;
    pub const B_5F: u16 = 0x5F;
    /// +60..+64 are set per level tier by `level_tier_stats` (meaning
    /// unknown; they fall as level rises, like target numbers). +62 and
    /// +64 are first written by `attribute_modifiers`.
    pub const TIER_A1: u16 = 0x60;
    pub const TIER_A2: u16 = 0x61;
    pub const TIER_B: u16 = 0x62;
    pub const TIER_A3: u16 = 0x63;
    pub const TIER_C: u16 = 0x64;
    pub const SPELL_POINTS: u16 = 0x66;
    pub const MAX_SPELL_POINTS: u16 = 0x68;
    pub const B_6A: u16 = 0x6A;
    /// Face roll 0..9, unique among DGROUP 0FD4.
    pub const FACE: u16 = 0x6B;
    /// First inventory slot.
    pub const ITEMS: u16 = 0x6E;
    /// Creature special-attack kind and its data.
    pub const SPECIAL: u16 = 0x366;
    pub const SPECIAL_DATA: u16 = 0x367;
    /// Gold a creature carries (word).
    pub const CREATURE_GOLD: u16 = 0x407;
    /// Experience points (dword).
    pub const XP: u16 = 0x409;
    /// Status flags: `FLAG_CREATURE`, 0x100 dead/defeated (inferred).
    pub const FLAGS: u16 = 0x40D;
    pub const FLAG_CREATURE: u16 = 0x1000;
    /// Per-kind counters of active status effects (9 bytes).
    pub const EFFECT_COUNTS: u16 = 0x40F;
    /// Timed spell flags.
    pub const SPELL_FLAGS: u16 = 0x41A;
    /// Magic shield points, absorbed before HP.
    pub const MAGIC_SHIELD: u16 = 0x41C;
}
use field::*;

/// DGROUP 2A50: class byte for each class, in name order (Mage .. Knight).
pub const CLASS_IDS: u16 = 0x2A50;
/// DGROUP 2BD5: hit die per class index.
pub const HIT_DIE: u16 = 0x2BD5;
/// 3276:3A63: base attributes (256 scale) per race: 8 male then 8 female.
pub const RACE_BASE: u16 = 0x3A63;
/// 3276:6150: spell point multiplier kind per class index.
pub const SPELL_KIND: u16 = 0x6150;
/// DGROUP 10A8: scratch copy of the player's attributes.
const ATTR_SCRATCH: u16 = 0x10A8;

impl Machine {
    fn hit_die(&self, rec: u16) -> u16 {
        self.hit_die_at(DS, rec)
    }

    /// Hit die of the class of a record in any segment (table DGROUP 2BD5).
    pub fn hit_die_at(&self, seg: u16, rec: u16) -> u16 {
        let class = self.mem.r8(seg, rec.wrapping_add(CLASS)) & 0x1f;
        self.mem.r8(DS, HIT_DIE.wrapping_add(class as u16)) as u16
    }

    /// 1194:370C. Rolls the 8 attributes for the record's race and sex.
    pub fn roll_attributes(&mut self, rec: u16) -> Option<()> {
        self.roll_attributes_at(DS, rec)
    }

    /// `roll_attributes` for a record in any segment (NPCs and creatures).
    pub fn roll_attributes_at(&mut self, seg: u16, rec: u16) -> Option<()> {
        // The race byte is not masked.
        let mut base = RACE_BASE.wrapping_add((self.mem.r8(seg, rec.wrapping_add(RACE)) as u16) << 4);
        if self.mem.r8(seg, rec.wrapping_add(SEX)) != 0 {
            base = base.wrapping_add(8);
        }
        for i in 0..8 {
            let r = dice(&mut self.rng, 20)?.wrapping_add(1);
            let v = (to256(r as i16) as u8).wrapping_add(self.mem.r8(DATA, base.wrapping_add(i)));
            self.mem.w8(seg, rec.wrapping_add(ATTRS + i), v);
        }
        Some(())
    }

    /// 1194:37D3. Spell points for a class from INT in percent.
    /// Returns (max, current); Sorcerers get triple max and start at 0.
    pub fn spell_points(&self, rec: u16, int: u16) -> (u16, u16) {
        self.spell_points_at(DS, rec, int)
    }

    /// `spell_points` for a record in any segment (creatures live outside
    /// DGROUP).
    pub fn spell_points_at(&self, seg: u16, rec: u16, int: u16) -> (u16, u16) {
        let class = self.mem.r8(seg, rec.wrapping_add(CLASS));
        let sp = if class & 0x20 == 0 || class == 0x23 {
            0
        } else if class == 0xE6 {
            int
        } else {
            match self.mem.r8(DATA, SPELL_KIND.wrapping_add((class & 0x1f) as u16)) {
                2 => int.wrapping_add(int),
                0 => int.wrapping_add(int >> 1),
                _ => int.wrapping_add(int >> 2).wrapping_add(int >> 1),
            }
        };
        let max = if class == 0x23 { int.wrapping_mul(3) } else { sp };
        (max, sp)
    }

    /// 1194:3621. Attribute modifiers; attributes must be on the 256 scale.
    pub fn attribute_modifiers(&mut self, rec: u16) {
        self.attribute_modifiers_at(DS, rec)
    }

    /// `attribute_modifiers` for a record in any segment (1194:361D is the
    /// far entry the effects engine uses for creatures).
    pub fn attribute_modifiers_at(&mut self, seg: u16, rec: u16) {
        let b: [u8; 8] = std::array::from_fn(|i| self.mem.r8(seg, rec.wrapping_add(ATTRS + i as u16)));
        let [str, int, wil, agi, spd, end, per, lck] = b.map(|v| v as i16);
        let w = |m: &mut Self, f: u16, v: i16| m.mem.w16(seg, rec.wrapping_add(f), v as u16);

        w(self, DAMAGE_MOD, (str - 128 + 6) / 13);
        w(self, MAX_WEIGHT, str * 2);
        let (max_sp, _) = self.spell_points_at(seg, rec, to100(int) as u16);
        w(self, MAX_SPELL_POINTS, max_sp as i16);
        w(self, MAX_SPELL_POINTS_2, max_sp as i16);
        w(self, INT_MOD, int - 128);
        w(self, MAGIC_DEF, -(wil - 128));
        // Logical shift: negative modifiers become large positive values.
        // Fixed (#24), an arithmetic shift: half the signed modifier.
        if self.fixed(crate::fixes::ATTRIBUTE_MODIFIERS) {
            w(self, WIL_MOD_HALF, (wil - 128) >> 1);
        } else {
            w(self, WIL_MOD_HALF, ((wil - 128) as u16 >> 1) as i16);
        }
        for f in [ATTACK_MOD, DEFENCE_MOD, AGI_MOD_4B] {
            w(self, f, agi - 128);
        }
        w(self, SPD_MOD, (256 - spd + 12) / 25);
        let hp_mod = (end - 128 + 12) / 25;
        w(self, HP_MOD, hp_mod);
        w(self, HEAL_MOD, hp_mod);
        w(self, END_X2, end * 2);
        w(self, PER_MOD, per - 128);
        // The luck modifier is computed and discarded; the original then
        // doubles the divisor, so this is always 50.
        // Fixed (#24), the quotient is doubled instead (`add bx, bx` was
        // meant for AX, which the store was to take): 2 x (LCK - 116) / 25,
        // so the luck difference in the hit chance counts.
        if self.fixed(crate::fixes::ATTRIBUTE_MODIFIERS) {
            w(self, LCK_MOD, (lck - 128 + 12) / 25 * 2);
        } else {
            let _ = lck;
            w(self, LCK_MOD, 50);
        }
        // 8-bit sums that wrap, divided so they are always 0. Fixed mode
        // leaves them: summed in 16 bits the divisions still give 0 for
        // any attributes (at most 1FEh + 200h < 400h, 1FEh + 400h < 800h).
        let (str_b, int_b, wil_b, end_b) = (b[0], b[1], b[2], b[5]);
        self.mem.w8(seg, rec.wrapping_add(B_5F), ((wil_b.wrapping_add(end_b) as u16 + 0x200) / 0x400) as u8);
        self.mem.w8(seg, rec.wrapping_add(B_6A), ((str_b.wrapping_add(int_b) as u16 + 0x400) / 0x800) as u8);
        self.mem.w8(seg, rec.wrapping_add(TIER_B), (end >> 2) as u8);
        self.mem.w8(seg, rec.wrapping_add(TIER_C), self.mem.r16(seg, rec.wrapping_add(MAGIC_DEF)) as u8);
    }

    /// 1194:1408. Derived stats for the player. While the flag at DGROUP
    /// 1296 is 1 (character creation), it first sets spell points from INT
    /// in percent.
    pub fn player_derived_stats(&mut self) {
        let rec = PLAYER;
        let attrs = rec + ATTRS;
        if self.mem.r8(DS, 0x1296) == 1 {
            let (max, cur) = self.spell_points(rec, self.mem.r8(DS, rec + ATTRS + 1) as u16);
            self.mem.w16(DS, rec + MAX_SPELL_POINTS, max);
            if self.mem.r16(DS, 0x5C8) & 0x10 != 0 {
                self.mem.w16(DS, rec + SPELL_POINTS, cur);
            }
        }
        for i in 0..8 {
            self.mem.w8(DS, ATTR_SCRATCH + i, self.mem.r8(DS, attrs + i));
        }
        attrs_to256(&mut self.mem, attrs, attrs, 8);
        self.attribute_modifiers(rec);

        let b = |m: &Self, i: u16| m.mem.r8(DS, attrs + i);
        let v = to100(self.mem.r16(DS, rec + MAGIC_DEF) as i16);
        self.mem.w8(DS, rec + TIER_C, v as u8);
        self.mem.w8(DS, rec + B_5F, (b(self, 2).wrapping_add(b(self, 5)) as u16 / 200) as u8);
        self.mem.w8(DS, rec + B_6A, (b(self, 0).wrapping_add(b(self, 1)) as u16 / 400) as u8);
        let fatigue = (b(self, 0) as u16 + b(self, 5) as u16) << 6;
        self.mem.w16(DS, rec + FATIGUE, fatigue);
        self.mem.w8(DS, rec + TIER_B, to100_u(b(self, 5) as u16 >> 2) as u8);

        for i in 0..8 {
            self.mem.w8(DS, attrs + i, self.mem.r8(DS, ATTR_SCRATCH + i));
        }
    }

    /// The player's modifiers from its percent attributes, as
    /// `player_derived_stats` works them out (on the 256 scale, the
    /// player-only bytes in percent) but leaving spell points and fatigue
    /// as they are: what fixed mode's effect pulses use for the player
    /// (#23) in place of 1194:361D on percent attributes.
    pub fn player_modifiers(&mut self) {
        let rec = PLAYER;
        let attrs = rec + ATTRS;
        let saved: [u8; 8] = std::array::from_fn(|i| self.mem.r8(DS, attrs + i as u16));
        attrs_to256(&mut self.mem, attrs, attrs, 8);
        self.attribute_modifiers(rec);
        let b = |m: &Self, i: u16| m.mem.r8(DS, attrs + i);
        let v = to100(self.mem.r16(DS, rec + MAGIC_DEF) as i16);
        self.mem.w8(DS, rec + TIER_C, v as u8);
        self.mem.w8(DS, rec + B_5F, (b(self, 2).wrapping_add(b(self, 5)) as u16 / 200) as u8);
        self.mem.w8(DS, rec + B_6A, (b(self, 0).wrapping_add(b(self, 1)) as u16 / 400) as u8);
        self.mem.w8(DS, rec + TIER_B, to100_u(b(self, 5) as u16 >> 2) as u8);
        for (i, v) in saved.into_iter().enumerate() {
            self.mem.w8(DS, attrs + i as u16, v);
        }
    }

    /// 1194:374E. Rolls starting hit points: hit die plus the HP modifier,
    /// at least 1 (compared unsigned, so a negative total stays huge).
    pub fn roll_max_hp(&mut self, rec: u16) -> Option<()> {
        let die = self.hit_die(rec);
        let mut hp = dice(&mut self.rng, die)?
            .wrapping_add(self.mem.r16(DS, rec.wrapping_add(HP_MOD)));
        if hp < 1 {
            hp = 1;
        }
        self.mem.w16(DS, rec.wrapping_add(MAX_HP), hp);
        self.mem.w16(DS, rec.wrapping_add(HP), hp);
        Some(())
    }

    /// 1194:377B. One hit die, at least 1.
    pub fn roll_hit_die(&mut self, rec: u16) -> Option<u16> {
        let die = self.hit_die(rec);
        Some(dice(&mut self.rng, die)?.max(1))
    }

    /// 1194:2B87. Starting gold, 50..199, into DGROUP 05C2.
    pub fn roll_starting_gold(&mut self) -> Option<()> {
        let g = self.rng.range(50, 200)?;
        self.mem.w32(DS, 0x5C2, g as u32);
        Some(())
    }

    /// 1194:2AFA..2B32: the stat part of character creation, after race,
    /// class and sex are chosen.
    pub fn create_player_stats(&mut self) -> Option<()> {
        self.roll_attributes(PLAYER)?;
        attrs_to100(&mut self.mem, PLAYER + ATTRS, PLAYER + ATTRS, 8);
        for i in 0..8 {
            self.mem.w8(DS, PLAYER + ATTRS + 8 + i, self.mem.r8(DS, PLAYER + ATTRS + i));
        }
        self.mem.w8(DS, 0x1296, 1);
        self.player_derived_stats();
        self.mem.w8(DS, 0x1296, 0);
        self.roll_max_hp(PLAYER)?;
        let max = self.mem.r16(DS, PLAYER + MAX_HP).wrapping_add(25);
        let hp = self.mem.r16(DS, PLAYER + HP).wrapping_add(25);
        self.mem.w16(DS, PLAYER + MAX_HP, max);
        self.mem.w16(DS, PLAYER + HP, hp);
        Some(())
    }
}
