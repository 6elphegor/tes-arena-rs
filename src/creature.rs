//! Creature and NPC generation (segment 1194).
//!
//! Human NPCs start as a 16-byte seed record (`npc_seed`, 1194:38B9):
//! level, class, sex, race, an INT roll and the RNG seed they will be
//! expanded from. `expand_npc` (1194:351D) turns that into a full 0x41E
//! character record, reseeding the RNG from the seed record so the same
//! NPC always comes out the same, and restoring the caller's seed after.
//! Monsters are built from per-type tables by `create_monster`
//! (1194:8CEE) into the creature segment (DGROUP 1042).
//!
//! Records may live outside DGROUP (creatures do); routines take the
//! record's segment. Where the original runs the RNG with DS set to the
//! record's segment, it uses whatever lies at that segment's 91F0..91F9
//! as RNG state; `cr_range_in` reproduces that.
//!
//! `expand_npc` leaves the name out (1194:080B saves and restores the
//! seed, so nothing else changes); `expand_npc_named` writes it.


use crate::bsa::Files;
use crate::character::field::*;
use crate::dice::{dice, roll};
use crate::effects::Applied;
use crate::machine::Machine;
use crate::random::CycleCheck;
use crate::mem::{DATA, DS};
use crate::scale::{to100, to256};

/// Size of a character or creature record.
pub const RECORD_SIZE: u16 = 0x41E;
/// DGROUP word: segment of the creature records.
pub const CREATURE_SEG: u16 = 0x1042;

/// Seed record fields (16 bytes).
pub mod seed {
    /// RNG seed the NPC is expanded from (dword).
    pub const SEED: u16 = 0x00;
    /// Race; bit 0x80 is stripped on expansion.
    pub const RACE: u16 = 0x04;
    pub const CLASS: u16 = 0x05;
    pub const LEVEL: u16 = 0x06;
    pub const SEX: u16 = 0x07;
    /// INT (256 scale), copied over the rolled INT on expansion.
    pub const INT: u16 = 0x08;
    /// A random byte other than 0xFF (meaning unknown).
    pub const B_0B: u16 = 0x0B;
    /// Cleared dword.
    pub const D_0C: u16 = 0x0C;
}

/// Record fields used here that `character::field` does not name.
pub mod rec {
    pub use crate::character::field::{BASE_ATTRS, FACE, NAME, PORTRAIT, SPECIAL, SPECIAL_DATA};
    pub const B_28: u16 = 0x28;
    /// The XP the creature is worth, for creatures (dword at XP).
    pub const XP_VALUE: u16 = super::XP;
}

/// DGROUP tables.
mod table {
    /// Cumulative NPC level chances (3 bytes).
    pub const NPC_LEVEL: u16 = 0x419A;
    /// NPC XP multiplier by class-byte group (3 bytes).
    pub const NPC_XP: u16 = 0x419D;
    /// Faces in use (9 bytes, 0xFF = free).
    pub const FACES: u16 = 0x0FD4;
    /// Per monster type (index type - 1).
    pub const M_LEVEL: u16 = 0x2C6A;
    pub const M_XP: u16 = 0x2D12;
    pub const M_DBE: u16 = 0x2C3A;
    pub const M_100D: u16 = 0x2BF2;
    pub const M_FF3: u16 = 0x2C0A;
    pub const M_F83: u16 = 0x2C22;
    pub const M_DAMAGE: u16 = 0x2D8A;
    pub const M_HP: u16 = 0x2CB2;
    pub const M_XP_PER_HP: u16 = 0x2D72;
    pub const M_SPELL_FLAGS: u16 = 0x2C82;
    pub const M_SPECIAL: u16 = 0x2DBA;
    pub const M_SPECIAL_DATA: u16 = 0x2DC4;
    pub const M_CLASS: u16 = 0x2DD3;
    pub const M_PORTRAIT: u16 = 0x2DDD;
    pub const TYPE_2A_DATA: u16 = 0x2DCD;
}

/// 3276 tables.
mod data {
    /// Cumulative NPC class chances (18 bytes).
    pub const NPC_CLASS: u16 = 0x6157;
    /// Monster attributes, 8 per type.
    pub const M_ATTRS: u16 = 0x3B2B;
    /// Race base attributes (as character::RACE_BASE).
    pub const RACE_BASE: u16 = 0x3A63;
}

/// DGROUP variables.
mod var {
    pub const FACE_BASE: u16 = 0x128A;
    pub const FACE_ROLL: u16 = 0x128C;
    pub const A898: u16 = 0xA898;
    pub const A886: u16 = 0xA886;
    pub const F74: u16 = 0x0F74;
    pub const F75: u16 = 0x0F75;
    pub const DBE: u16 = 0x0DBE;
    pub const B_100D: u16 = 0x100D;
    pub const FF3: u16 = 0x0FF3;
    pub const F83: u16 = 0x0F83;
    /// Far pointer (offset, segment) used by type 2A.
    pub const P_1020: u16 = 0x1020;
}

const RNG_TABLE_OFF: u16 = 0x91EE;
const RNG_TABLE_SEG: u16 = 0x91F0;
const RNG_LEFT: u16 = 0x91F2;
const RNG_REFILL: u16 = 0x91F4;
const RNG_SEED: u16 = 0x91F6;

impl Machine {
    /// 0000:6E10 with DS = `seg`: steps the LCG stored at seg:91F6.
    fn cr_step_in(&mut self, seg: u16) -> u16 {
        let s = self.mem.r32(seg, RNG_SEED).wrapping_mul(0x006D_002D);
        self.mem.w32(seg, RNG_SEED, s);
        (s >> 16) as u16
    }

    /// 0000:6EA8 with DS = `seg`. DGROUP uses the machine's RNG.
    fn cr_next_in(&mut self, seg: u16) -> u16 {
        if seg == DS {
            return self.rng.next();
        }
        let tseg = self.mem.r16(seg, RNG_TABLE_SEG);
        if tseg == 0 {
            return self.cr_step_in(seg);
        }
        let off = self.mem.r16(seg, RNG_TABLE_OFF);
        let mut v = self.mem.r16(tseg, off);
        self.mem.w16(seg, RNG_TABLE_OFF, off.wrapping_add(2));
        let left = self.mem.r16(seg, RNG_LEFT).wrapping_sub(1);
        self.mem.w16(seg, RNG_LEFT, left);
        if left == 0 {
            self.mem.w16(seg, RNG_TABLE_OFF, 0);
            let n = self.mem.r16(seg, RNG_REFILL);
            self.mem.w16(seg, RNG_LEFT, n);
            let count = match n >> 1 {
                0 => 0x10000u32,
                c => c as u32,
            };
            let mut di = 0u16;
            for _ in 0..count {
                v = self.cr_step_in(seg);
                self.mem.w16(tseg, di, v);
                di = di.wrapping_add(2);
            }
        }
        v
    }

    /// 0000:6EEA with DS = `seg`: [lo, hi), None on a zero span.
    fn cr_range_in(&mut self, seg: u16, lo: u16, hi: u16) -> Option<u16> {
        let r = self.cr_next_in(seg);
        let span = hi.wrapping_sub(lo);
        if span == 0 {
            return None;
        }
        Some((r % span).wrapping_add(lo))
    }

    /// Runs `attempt` on the DGROUP RNG until it succeeds. The original
    /// loops forever when the LCG cycles without success (seed 0 and the
    /// other multiples of 2^30 are fixed points); that returns None.
    fn cr_retry<T>(&mut self, mut attempt: impl FnMut(&mut Self) -> Option<Option<T>>) -> Option<T> {
        let mut cycle = CycleCheck::new(&self.rng);
        loop {
            if let Some(v) = attempt(self)? {
                return Some(v);
            }
            if cycle.cycled(&self.rng) {
                return None;
            }
        }
    }

    /// 1194:5572: next() % 100.
    fn cr_percent(&mut self) -> u16 {
        self.rng.next() % 100
    }

    /// 1194:391F. NPC level: the first of the three cumulative chances at
    /// DGROUP 419A (summed in 8 bits, compared signed) that d% reaches;
    /// 0..3.
    pub fn npc_roll_level(&mut self) -> u16 {
        let r = self.cr_percent();
        let mut sum = 0u8;
        for i in 0..3u16 {
            sum = sum.wrapping_add(self.mem.r8(DS, table::NPC_LEVEL + i));
            if r as i16 <= sum as i16 {
                return i;
            }
        }
        3
    }

    /// 1194:3937. NPC class byte: d% against 18 cumulative chances at
    /// 3276:6157; past the last it reads the byte after the class list.
    pub fn npc_roll_class(&mut self) -> u8 {
        let r = self.cr_percent();
        let mut sum = 0u8;
        let mut i = 0u16;
        while i < 18 {
            sum = sum.wrapping_add(self.mem.r8(DATA, data::NPC_CLASS + i));
            if r as i16 <= sum as i16 {
                break;
            }
            i += 1;
        }
        self.mem.r8(DS, crate::character::CLASS_IDS + i)
    }

    /// 1194:3965. 1 (female) a quarter of the time.
    pub fn npc_roll_sex(&mut self) -> u8 {
        (self.cr_percent() < 25) as u8
    }

    /// 1194:3973. An INT roll for an NPC seed: dice(20) (no +1, unlike
    /// player creation) on the 256 scale plus the race/sex base INT.
    fn cr_roll_int(&mut self, race: u16, sex: u8) -> Option<u8> {
        let mut base = data::RACE_BASE.wrapping_add(race << 4);
        if sex != 0 {
            base = base.wrapping_add(8);
        }
        // AL = 20; AH is the high byte of race << 4.
        let n = ((race << 4) & 0xFF00) | 0x14;
        let r = dice(&mut self.rng, n)?;
        Some((to256(r as i16) as u8).wrapping_add(self.mem.r8(DATA, base.wrapping_add(1))))
    }

    /// 0000:0250. A random byte other than 0xFF.
    fn cr_byte_not_ff(&mut self) -> Option<u8> {
        self.cr_retry(|m| {
            let v = m.rng.next() as u8;
            Some((v != 0xFF).then_some(v))
        })
    }

    /// 1194:38B9. Fills the 16-byte NPC seed record at seg:off. `level` 0
    /// rolls the level and then always rolls the class, ignoring `class`;
    /// otherwise `class` 0xFFFF rolls it.
    pub fn npc_seed(&mut self, seg: u16, off: u16, class: u16, level: u16) -> Option<()> {
        let w8 = |m: &mut Self, f: u16, v: u8| m.mem.w8(seg, off.wrapping_add(f), v);
        let class_byte = if level == 0 {
            let l = self.npc_roll_level();
            w8(self, seed::LEVEL, l as u8);
            None
        } else {
            w8(self, seed::LEVEL, level as u8);
            (class != 0xFFFF).then_some(class as u8)
        };
        let c = match class_byte {
            Some(c) => c,
            None => self.npc_roll_class(),
        };
        w8(self, seed::CLASS, c);
        self.mem.w16(DS, var::A898, 0);
        self.mem.w32(seg, off.wrapping_add(seed::SEED), self.rng.seed);
        let sex = self.npc_roll_sex();
        w8(self, seed::SEX, sex);
        let race = roll(&mut self.rng, 8)?;
        w8(self, seed::RACE, race as u8);
        let int = self.cr_roll_int(race, sex)?;
        w8(self, seed::INT, int);
        self.mem.w32(seg, off.wrapping_add(seed::D_0C), 0);
        let b = self.cr_byte_not_ff()?;
        w8(self, seed::B_0B, b);
        self.mem.w16(DS, var::A886, 1);
        Some(())
    }

    /// 1194:35FF. Portrait: race × 32 + roll(32), roll(31) for race 7,
    /// with the RNG state of the record's segment.
    pub(crate) fn cr_portrait(&mut self, seg: u16, rec: u16) -> Option<()> {
        let race = self.mem.r8(seg, rec.wrapping_add(RACE));
        let n = if race == 7 { 31 } else { 32 };
        let r = self.cr_range_in(seg, 0, n)?;
        let v = 0x20u16.wrapping_mul(race as u16).wrapping_add(r);
        self.mem.w8(seg, rec.wrapping_add(rec::PORTRAIT), v as u8);
        Some(())
    }

    /// 1194:3829 (far, 1194:3825 entry is retf). Rolls 0..9 until
    /// race × 10 + roll (low byte) is none of the faces at DGROUP 0FD4
    /// (0xFF entries are skipped). Leaves race × 10 in 128A and the roll in
    /// 128C, and returns the roll.
    pub fn npc_face(&mut self, race: u16) -> Option<u16> {
        let base = race.wrapping_mul(10);
        self.mem.w16(DS, var::FACE_BASE, base);
        self.cr_retry(|m| {
            let r = roll(&mut m.rng, 10)?;
            m.mem.w16(DS, var::FACE_ROLL, r);
            let al = r.wrapping_add(m.mem.r16(DS, var::FACE_BASE)) as u8;
            let taken = (0..9).any(|i| {
                let f = m.mem.r8(DS, table::FACES + i);
                f != 0xFF && f == al
            });
            Some((!taken).then_some(r))
        })
    }

    /// 1194:379F. Derived stats of a record in any segment: level-tier
    /// values, spell points from INT, attribute modifiers, hit points
    /// (1194:374E: hit die + HP modifier, at least 1 unsigned) and
    /// fatigue (STR + END) << 6.
    pub fn derive_record_stats(&mut self, seg: u16, rec: u16) -> Option<()> {
        self.level_tier_stats(seg, rec);
        let int = to100(self.mem.r8(seg, rec.wrapping_add(ATTRS + 1)) as i16) as u16;
        let (max, cur) = self.spell_points_at(seg, rec, int);
        self.mem.w16(seg, rec.wrapping_add(SPELL_POINTS), cur);
        self.mem.w16(seg, rec.wrapping_add(MAX_SPELL_POINTS), max);
        self.attribute_modifiers_at(seg, rec);
        let die = self.hit_die_at(seg, rec);
        let mut hp = dice(&mut self.rng, die)?.wrapping_add(self.mem.r16(seg, rec.wrapping_add(HP_MOD)));
        if hp < 1 {
            hp = 1;
        }
        self.mem.w16(seg, rec.wrapping_add(MAX_HP), hp);
        self.mem.w16(seg, rec.wrapping_add(HP), hp);
        let str = self.mem.r8(seg, rec.wrapping_add(ATTRS)) as u16;
        let end = self.mem.r8(seg, rec.wrapping_add(ATTRS + 5)) as u16;
        self.mem.w16(seg, rec.wrapping_add(FATIGUE), (str + end) << 6);
        Some(())
    }

    /// 1194:3869..38B8. Attributes for an NPC record from its seed record:
    /// rolled for race and sex, copied to the base attributes, then INT
    /// replaced by the seed's INT roll (after the copy, so base INT keeps
    /// the rolled value). Then derived stats, and hit points as one hit
    /// die (at least 1) per level counting level 0, plus 25.
    fn cr_npc_attributes(&mut self, seed_seg: u16, seed_off: u16, seg: u16, rec: u16) -> Option<()> {
        self.roll_attributes_at(seg, rec)?;
        for i in 0..8 {
            let v = self.mem.r8(seg, rec.wrapping_add(ATTRS + i));
            self.mem.w8(seg, rec.wrapping_add(rec::BASE_ATTRS + i), v);
        }
        let int = self.mem.r8(seed_seg, seed_off.wrapping_add(seed::INT));
        self.mem.w8(seg, rec.wrapping_add(ATTRS + 1), int);
        self.derive_record_stats(seg, rec)?;
        self.mem.w16(seg, rec.wrapping_add(MAX_HP), 0);
        self.mem.w16(seg, rec.wrapping_add(HP), 0);
        let n = self.mem.r8(seg, rec.wrapping_add(LEVEL)) as u16 + 1;
        for _ in 0..n {
            let die = self.hit_die_at(seg, rec);
            let hd = dice(&mut self.rng, die)?.max(1);
            let v = self.mem.r16(seg, rec.wrapping_add(MAX_HP)).wrapping_add(hd);
            self.mem.w16(seg, rec.wrapping_add(MAX_HP), v);
        }
        let v = self.mem.r16(seg, rec.wrapping_add(MAX_HP)).wrapping_add(25);
        self.mem.w16(seg, rec.wrapping_add(MAX_HP), v);
        self.mem.w16(seg, rec.wrapping_add(HP), v);
        Some(())
    }

    /// 1194:399A. XP an NPC is worth: level² × the multiplier for its class
    /// byte (DGROUP 419D: below 6, below 12, else; the raw byte with its
    /// flag bits, so spellcasters count as the last group), each product
    /// kept to 16 bits.
    pub fn npc_xp_value(&mut self, seg: u16, rec: u16) {
        let class = self.mem.r8(seg, rec.wrapping_add(CLASS));
        let group = match class {
            0..=5 => 0,
            6..=11 => 1,
            _ => 2,
        };
        let mul = self.mem.r8(DS, table::NPC_XP + group) as u16;
        let level = self.mem.r8(seg, rec.wrapping_add(LEVEL)) as u16;
        let xp = level.wrapping_mul(level).wrapping_mul(mul);
        self.mem.w32(seg, rec.wrapping_add(XP), xp as u32);
    }

    /// 1194:351D. Expands the NPC seed record at seed_seg:seed_off into a
    /// full record at seg:rec. The caller's RNG seed is restored at the
    /// end. None where the original faults (a hit die of 0) or hangs.
    pub fn expand_npc(&mut self, seed_seg: u16, seed_off: u16, seg: u16, rec: u16) -> Option<()> {
        self.cr_expand(None, seed_seg, seed_off, seg, rec)
    }

    /// `expand_npc` with the name (1194:080B, `npc_name`) written as the
    /// original does: from the record's race, sex and seed, before byte
    /// 0x28 is cleared, which cuts names at 31 characters. None also
    /// where the name can't be made (see `npc_name`).
    pub fn expand_npc_named(&mut self, files: &mut Files, seed_seg: u16, seed_off: u16, seg: u16, rec: u16) -> Option<()> {
        self.cr_expand(Some(files), seed_seg, seed_off, seg, rec)
    }

    fn cr_expand(&mut self, files: Option<&mut Files>, seed_seg: u16, seed_off: u16, seg: u16, rec: u16) -> Option<()> {
        let saved = self.rng.seed;
        for i in 0..RECORD_SIZE {
            self.mem.w8(seg, rec.wrapping_add(i), 0);
        }
        for i in 0..8 {
            let v = self.mem.r8(seed_seg, seed_off.wrapping_add(i));
            self.mem.w8(seg, rec.wrapping_add(i), v);
        }
        let race = self.mem.r8(seg, rec.wrapping_add(RACE)) & 0x7f;
        self.mem.w8(seg, rec.wrapping_add(RACE), race);
        self.rng.seed = self.mem.r32(seg, rec.wrapping_add(seed::SEED));
        self.cr_portrait(seg, rec)?;
        let face = self.npc_face(race as u16)?;
        self.mem.w8(seg, rec.wrapping_add(rec::FACE), face as u8);
        self.rng.seed = self.mem.r32(seg, rec.wrapping_add(seed::SEED));
        if let Some(files) = files {
            let sex = self.mem.r8(seg, rec.wrapping_add(SEX));
            let seed = self.mem.r32(seg, rec.wrapping_add(seed::SEED));
            self.npc_name(files, race, sex, seed, seg, rec.wrapping_add(NAME))?;
        }
        self.mem.w8(seg, rec.wrapping_add(rec::B_28), 0);
        self.cr_npc_attributes(seed_seg, seed_off, seg, rec)?;
        self.npc_xp_value(seg, rec);
        self.rng.seed = saved;
        Some(())
    }

    /// 1194:8EA9. Starts the effect a monster type carries (flag 0x80, one
    /// pulse every tick for `duration` pulses, then endless phase B) and
    /// stores `value` at +0E of the slot, or of the record when the effect
    /// was not applied (the original writes through whatever DS:SI the
    /// effect routine returned).
    fn cr_monster_effect(&mut self, seg: u16, rec: u16, value: u16, duration: u16) {
        match self.apply_effect(seg, rec, 0x80, 1, duration, 1, 0xFFFB) {
            Applied::Slot(Some(s)) => self.mem.w16(DS, s.wrapping_add(0x0E), value),
            Applied::Slot(None) => unreachable!("flag 0x80 always takes a slot"),
            Applied::Refused | Applied::AlreadyActive => self.mem.w16(seg, rec.wrapping_add(0x0E), value),
        }
    }

    /// 1194:8ED4. Type 2A: four words at 0x40 past the far pointer at
    /// DGROUP 1020, six special-attack bytes from DGROUP 2DCD, kind 6.
    pub(crate) fn cr_type_2a(&mut self, seg: u16, rec: u16) {
        let (off, s) = (self.mem.r16(DS, var::P_1020), self.mem.r16(DS, var::P_1020 + 2));
        let p = off.wrapping_add(0x40);
        for (f, v) in [(0x2C, 0xFFFF), (0x2E, 0x19), (0x30, 0xFFFF), (0x32, 0x32)] {
            self.mem.w16(s, p.wrapping_add(f), v);
        }
        for i in 0..6 {
            let v = self.mem.r8(DS, table::TYPE_2A_DATA + i);
            self.mem.w8(seg, rec.wrapping_add(rec::SPECIAL_DATA + i), v);
        }
        self.mem.w8(seg, rec.wrapping_add(rec::SPECIAL), 6);
    }

    /// 1194:8CEE. Builds a monster of type `ty` (1-based) at offset `rec`
    /// of the creature segment (DGROUP 1042) from the per-type tables:
    /// level, XP, attributes (3276:3B2B), damage, hit points (a range),
    /// spell flags and special attacks. Types 0E, 16 and 17 start an
    /// effect. Type 2A is built as type 18 with extra data. None where the
    /// original faults (an empty HP range).
    pub fn create_monster(&mut self, ty: u16, rec: u16) -> Option<()> {
        let seg = self.mem.r16(DS, CREATURE_SEG);
        let (f74, f75) = match ty {
            0x0C => (0x9E, 0x9E),
            0x0F => (0x9E, 0x9F),
            _ => (0, 0),
        };
        self.mem.w8(DS, var::F74, f74);
        self.mem.w8(DS, var::F75, f75);
        for i in 0..RECORD_SIZE {
            self.mem.w8(seg, rec.wrapping_add(i), 0);
        }
        let w8 = |m: &mut Self, f: u16, v: u8| m.mem.w8(seg, rec.wrapping_add(f), v);
        w8(self, RACE, ty as u8);
        let mut bx = ty;
        if ty as u8 == 0x2A {
            self.cr_type_2a(seg, rec);
            bx = (bx & 0xFF00) | 0x18;
        }
        let bx = bx.wrapping_sub(1);
        let f = self.mem.r16(seg, rec.wrapping_add(FLAGS)) | FLAG_CREATURE;
        self.mem.w16(seg, rec.wrapping_add(FLAGS), f);
        let ds8 = |m: &Self, t: u16, i: u16| m.mem.r8(DS, t.wrapping_add(i));
        w8(self, LEVEL, ds8(self, table::M_LEVEL, bx));
        let xp = self.mem.r32(DS, table::M_XP.wrapping_add(bx << 2));
        self.mem.w32(seg, rec.wrapping_add(XP), xp);
        let v = self.mem.r16(DS, table::M_DBE.wrapping_add(bx << 1));
        self.mem.w16(DS, var::DBE, v);
        self.mem.w8(DS, var::B_100D, ds8(self, table::M_100D, bx));
        self.mem.w16(DS, var::FF3, ds8(self, table::M_FF3, bx) as i8 as u16);
        self.mem.w8(DS, var::F83, ds8(self, table::M_F83, bx));
        for i in 0..8 {
            let v = self.mem.r8(DATA, data::M_ATTRS.wrapping_add(bx << 3).wrapping_add(i));
            w8(self, ATTRS + i, v);
        }
        for i in 0..8 {
            let v = self.mem.r8(seg, rec.wrapping_add(ATTRS + i));
            w8(self, rec::BASE_ATTRS + i, v);
        }
        self.derive_record_stats(seg, rec)?;
        w8(self, CREATURE_MIN_DAMAGE, ds8(self, table::M_DAMAGE, bx << 1));
        w8(self, CREATURE_MAX_DAMAGE, ds8(self, table::M_DAMAGE.wrapping_add(1), bx << 1));
        let lo = self.mem.r16(DS, table::M_HP.wrapping_add(bx << 2));
        let hi = self.mem.r16(DS, table::M_HP.wrapping_add(2).wrapping_add(bx << 2)).wrapping_add(1);
        let hp = self.rng.range(lo, hi)?;
        self.mem.w16(seg, rec.wrapping_add(HP), hp);
        self.mem.w16(seg, rec.wrapping_add(MAX_HP), hp);
        // 16-bit product, added to the dword.
        let per = ds8(self, table::M_XP_PER_HP, bx) as u16;
        let xp = self.mem.r32(seg, rec.wrapping_add(XP)).wrapping_add(per.wrapping_mul(hp) as u32);
        self.mem.w32(seg, rec.wrapping_add(XP), xp);
        let flags = self.mem.r16(seg, rec.wrapping_add(0x41A)) | self.mem.r16(DS, table::M_SPELL_FLAGS.wrapping_add(bx << 1));
        self.mem.w16(seg, rec.wrapping_add(0x41A), flags);
        if bx as u8 == 0x17 {
            return Some(());
        }
        match self.mem.r8(seg, rec.wrapping_add(RACE)) {
            0x16 => self.cr_monster_effect(seg, rec, 1, 1),
            0x0E => self.cr_monster_effect(seg, rec, 1, 2),
            0x17 => self.cr_monster_effect(seg, rec, 2, 3),
            _ => {}
        }
        w8(self, rec::SPECIAL, 0);
        for (t, data, portrait, class) in [(4u16, 0x0Cu8, 0xFFu8, 6u8), (0x0B, 0x3F, 0xFF, 5), (0x0A, 0x10, 0xFF, 6)] {
            if bx == t {
                w8(self, rec::SPECIAL, 1);
                w8(self, rec::SPECIAL_DATA, data);
                w8(self, rec::PORTRAIT, portrait);
                w8(self, CLASS, class);
            }
        }
        if bx >= 0x0E {
            let i = bx - 0x0E;
            w8(self, rec::SPECIAL, ds8(self, table::M_SPECIAL, i));
            w8(self, rec::SPECIAL_DATA, ds8(self, table::M_SPECIAL_DATA, i));
            w8(self, rec::PORTRAIT, ds8(self, table::M_PORTRAIT, i));
            w8(self, CLASS, ds8(self, table::M_CLASS, i));
        }
        Some(())
    }
}

/// DGROUP variables of the corpse loot dispatcher (segment 24FA).
mod loot_var {
    /// Far pointer to the corpse being looted (segment, offset).
    pub const CORPSE_SEG: u16 = 0x4EFE;
    pub const CORPSE_OFF: u16 = 0x4F00;
    /// Four loot flags: gold, trinket, equipment, magic equipment.
    pub const FLAGS: u16 = 0x4F02;
    /// Condition steps (vs roll(7)) and the percentage for each.
    pub const COND_STEPS: u16 = 0x4F07;
    pub const COND_PCT: u16 = 0x4F0A;
    /// Item level for the generators.
    pub const ITEM_LEVEL: u16 = 0x13CC;
    /// Item record the generators build.
    pub const ITEM: u16 = 0x13DA;
    pub const ITEM_HEALTH: u16 = 0x13E0;
    pub const ITEM_MAX_HEALTH: u16 = 0x13E2;
    pub const ITEM_FLAGS: u16 = 0x13E8;
    /// Magic-item template of 1194:B848.
    pub const MAGIC_TEMPLATE: u16 = 0x7033;
    /// Forced armour kind for 1194:E889 (0xFF = none).
    pub const FORCE_KIND: u16 = 0x101E;
    pub const W_17E7: u16 = 0x17E7;
}

/// 3276:4EA5: loot chances, 4 per creature type.
const LOOT_CHANCE: u16 = 0x4EA5;

impl Machine {
    /// 24FA:0233..0263. Scales the item's condition: max health / 100 ×
    /// a percentage picked by roll(7) against the steps at DGROUP 4F07,
    /// at least 1, into health and max health. None where the step scan
    /// would never end.
    fn cr_loot_condition(&mut self) -> Option<()> {
        let r = self.rng.range(0, 7)? as u8;
        let mut i = 0u16;
        let mut n = 0u32;
        while r > self.mem.r8(DS, loot_var::COND_STEPS.wrapping_add(i)) {
            i = i.wrapping_add(1);
            n += 1;
            if n > 0x10000 {
                return None;
            }
        }
        let pct = self.mem.r8(DS, loot_var::COND_PCT.wrapping_add(i)) as u16;
        let mut v = (self.mem.r16(DS, loot_var::ITEM_MAX_HEALTH) / 100).wrapping_mul(pct);
        if v == 0 {
            v = 1;
        }
        self.mem.w16(DS, loot_var::ITEM_MAX_HEALTH, v);
        self.mem.w16(DS, loot_var::ITEM_HEALTH, v);
        Some(())
    }

    fn cr_corpse(&self) -> (u16, u16) {
        (self.mem.r16(DS, loot_var::CORPSE_SEG), self.mem.r16(DS, loot_var::CORPSE_OFF))
    }

    /// 24FA:02C4. Copies 19 bytes from DGROUP `src` into the corpse.
    fn cr_loot_add(&mut self, src: u16) {
        let (seg, rec) = self.cr_corpse();
        self.add_item(DS, src, seg, rec);
    }

    fn cr_corpse_class(&self) -> u16 {
        let (seg, rec) = self.cr_corpse();
        self.mem.r8(seg, rec.wrapping_add(CLASS)) as u16
    }

    /// 24FA:028F. A magic trinket (1194:B848) or a potion (B7FA), half
    /// each. When B848 makes nothing (kind 1 below item level 7) it leaves
    /// SI on its template at 7033, and those 19 bytes are what the corpse
    /// receives.
    fn cr_loot_trinket(&mut self) -> Option<()> {
        let src = if self.rng.range(0, 2)? != 0 {
            let level = self.mem.r8(DS, loot_var::ITEM_LEVEL);
            let kind = self.rng.clone().range(0, 3);
            self.shop_magic_item(level, 0xFF)?;
            if kind == Some(1) && level < 7 { loot_var::MAGIC_TEMPLATE } else { loot_var::ITEM }
        } else {
            self.shop_potion()?;
            loot_var::ITEM
        };
        let v = self.mem.r16(DS, loot_var::W_17E7) | 2;
        self.mem.w16(DS, loot_var::W_17E7, v);
        self.cr_loot_add(src);
        Some(())
    }

    /// 24FA:01C4. Up to 20 tries at a plain weapon or armour (half each)
    /// the corpse's class can use. Armour is skipped for a character
    /// class that wears none; creatures are not checked. Each item's
    /// condition is scaled. If all 20 are unusable the last one is added
    /// anyway; if the last try was skipped armour, nothing is.
    fn cr_loot_equipment(&mut self) -> Option<()> {
        let (seg, rec) = self.cr_corpse();
        for left in (1..=20u16).rev() {
            if self.rng.range(0, 2)? == 0 {
                let creature = self.mem.r16(seg, rec.wrapping_add(FLAGS)) & FLAG_CREATURE != 0;
                if !creature && self.class_wears_no_armor(self.cr_corpse_class()) {
                    continue;
                }
                self.mem.w8(DS, loot_var::FORCE_KIND, 0xFF);
                let level = self.mem.r8(DS, loot_var::ITEM_LEVEL);
                self.loot_armor(level, 0xFF);
            } else {
                let level = self.mem.r8(DS, loot_var::ITEM_LEVEL);
                self.loot_weapon(level, 0xFF)?;
            }
            self.cr_loot_condition()?;
            if self.class_can_use(DS, loot_var::ITEM, self.cr_corpse_class())? || left == 1 {
                self.cr_loot_add(loot_var::ITEM);
                return Some(());
            }
        }
        Some(())
    }

    /// 24FA:0150. Up to 20 tries at a magic weapon or armour (half each)
    /// the corpse's class can use. The item level is forced to 16 (the
    /// corpse's level is computed and overwritten).
    fn cr_loot_magic_equipment(&mut self) -> Option<()> {
        for _ in 0..20 {
            self.mem.w8(DS, loot_var::ITEM_LEVEL, 0x10);
            if self.rng.range(0, 2)? != 0 {
                self.loot_magic_weapon(0x10, 0xFFFF)?;
            } else {
                self.loot_magic_armor(0x10, 0xFFFF)?;
            }
            let class = self.cr_corpse_class();
            let f = self.mem.r8(DS, loot_var::ITEM_FLAGS) | 2;
            self.mem.w8(DS, loot_var::ITEM_FLAGS, f);
            if self.class_can_use(DS, loot_var::ITEM, class)? {
                self.cr_loot_add(loot_var::ITEM);
                return Some(());
            }
        }
        Some(())
    }

    /// 24FA:0006. Loots the corpse at seg:rec: records it as the corpse
    /// (DGROUP 4EFE), rolls the four loot flags by creature type (d100 at
    /// most the chance at 3276:4EA5), then with item level = level + 1
    /// (8-bit) runs the handlers: gold (`roll_corpse_gold`), a trinket
    /// above item level 3, equipment, and magic equipment above item
    /// level 7. Returns whether any flag was set (the original's CF
    /// clear). None where a generator faults or hangs.
    pub fn loot_corpse(&mut self, seg: u16, rec: u16) -> Option<bool> {
        self.mem.w16(DS, loot_var::CORPSE_SEG, seg);
        self.mem.w16(DS, loot_var::CORPSE_OFF, rec);
        let chances = LOOT_CHANCE.wrapping_add((self.mem.r8(seg, rec.wrapping_add(RACE)) as u16) << 2);
        self.mem.w32(DS, loot_var::FLAGS, 0);
        for i in 0..4 {
            let r = self.d100() as u8;
            if r <= self.mem.r8(DATA, chances.wrapping_add(i)) {
                self.mem.w8(DS, loot_var::FLAGS + i, 1);
            }
        }
        let level = self.mem.r8(seg, rec.wrapping_add(LEVEL)).wrapping_add(1);
        self.mem.w8(DS, loot_var::ITEM_LEVEL, level);
        if self.mem.r32(DS, loot_var::FLAGS) == 0 {
            return Some(false);
        }
        let flag = |m: &Self, i: u16| m.mem.r8(DS, loot_var::FLAGS + i) != 0;
        let item_level = |m: &Self| m.mem.r8(DS, loot_var::ITEM_LEVEL);
        if flag(self, 0) {
            self.roll_corpse_gold();
        }
        if flag(self, 1) && item_level(self) > 3 {
            self.cr_loot_trinket()?;
        }
        if flag(self, 2) {
            self.cr_loot_equipment()?;
        }
        if flag(self, 3) && item_level(self) > 7 {
            self.cr_loot_magic_equipment()?;
        }
        Some(true)
    }
}

/// DGROUP 0C57: the active-creature list, 8 entries of 1C bytes; entry
/// i's record is at i × 41E in the creature segment.
pub const ACTIVE_LIST: u16 = 0x0C57;
const ACTIVE_ENTRY: u16 = 0x1C;
/// Entry flags (word at +0F): 0x4000 usable, 0x100 taken.
const ACTIVE_FLAGS: u16 = 0x0F;

impl Machine {
    /// 25CA:018F. Takes the first entry of the active list that is usable
    /// (0x4000) and not taken (0x100), marking it taken: Ok((record
    /// offset, entry)). With none, Err(SI): the original sets CF and
    /// leaves SI just past the list (0D37), which callers use as a record
    /// offset anyway.
    pub fn alloc_creature_slot(&mut self) -> Result<(u16, u16), u16> {
        let mut si = ACTIVE_LIST;
        let mut dx = 0u16;
        for _ in 0..8 {
            let f = self.mem.r16(DS, si + ACTIVE_FLAGS);
            if f & 0x4000 != 0 && f & 0x100 == 0 {
                self.mem.w16(DS, si + ACTIVE_FLAGS, f | 0x100);
                return Ok((dx, si));
            }
            dx = dx.wrapping_add(RECORD_SIZE);
            si += ACTIVE_ENTRY;
        }
        Err(si)
    }

    /// 25CA:01E6. Entries of the active list marked usable (0x4000),
    /// whether taken or not.
    pub fn usable_creature_slots(&self) -> u16 {
        (0..8).filter(|&i| self.mem.r16(DS, ACTIVE_LIST + i * ACTIVE_ENTRY + ACTIVE_FLAGS) & 0x4000 != 0).count() as u16
    }

    /// 1194:8BEF. Spawns `count` monsters of type `ty` (`count` 0 means
    /// 65536, as `loop` counts): each takes a creature slot, is built by
    /// `create_monster` and gets its loot. A failed slot allocation is
    /// not checked, so the monster is built at offset 0D37.
    pub fn spawn_monsters(&mut self, ty: u16, count: u16) -> Option<()> {
        let n = if count == 0 { 0x10000u32 } else { count as u32 };
        for _ in 0..n {
            let rec = match self.alloc_creature_slot() {
                Ok((rec, _)) => rec,
                Err(si) => si,
            };
            self.create_monster(ty, rec)?;
            let seg = self.mem.r16(DS, CREATURE_SEG);
            self.loot_corpse(seg, rec)?;
        }
        Some(())
    }

    /// 1194:8CA5. Raises an NPC record's attributes by level + 11 (on the
    /// 256 scale), each capped at 255, then recomputes its modifiers.
    pub fn npc_level_boost(&mut self, seg: u16, rec: u16) {
        let level = self.mem.r8(seg, rec.wrapping_add(LEVEL)) as u16;
        let add = to256(level.wrapping_add(11) as i16) as u16;
        for i in 0..8 {
            let v = (self.mem.r8(seg, rec.wrapping_add(ATTRS + i)) as u16).wrapping_add(add);
            self.mem.w8(seg, rec.wrapping_add(ATTRS + i), v.min(0xFF) as u8);
        }
        self.attribute_modifiers_at(seg, rec);
    }

    /// 1194:8CDF. A Sorcerer (class byte exactly 23h) starts with half
    /// its maximum spell points.
    pub fn npc_sorcerer_spell_points(&mut self, seg: u16, rec: u16) {
        if self.mem.r8(seg, rec.wrapping_add(CLASS)) == 0x23 {
            let v = self.mem.r16(seg, rec.wrapping_add(MAX_SPELL_POINTS)) >> 1;
            self.mem.w16(seg, rec.wrapping_add(SPELL_POINTS), v);
        }
    }

    /// 24FA:004D for a record in any segment (`Machine::roll_creature_gold`
    /// handles DGROUP records): gold carried, by class byte (80h set / 20h
    /// set / other) a d100 against DGROUP 4F0D, then 1..=50.
    pub fn creature_gold_at(&mut self, seg: u16, rec: u16) -> Option<()> {
        let class = self.mem.r8(seg, rec.wrapping_add(CLASS));
        let kind = if class & 0x80 != 0 {
            0
        } else if class & 0x20 != 0 {
            2
        } else {
            1
        };
        let chance = self.mem.r8(DS, 0x4F0D + kind) as u16;
        let gold = if self.d100() > chance { 0 } else { self.rng.range(1, 0x33)? };
        self.mem.w16(seg, rec.wrapping_add(CREATURE_GOLD), gold);
        Some(())
    }
}
