//! Status effects: the timed-effect engine (segment 2EBD), diseases,
//! monster special attacks and weapon enchantments (2AA1), and the
//! attribute-drain death check (2686).
//!
//! Active effects live in 16 slots of 0x3C bytes at DGROUP 0660. A slot
//! points at its target record (far pointer) and runs in two phases, each
//! with a pulse counter, a remaining count, a period and a block of
//! deltas (8 attributes, HP, SP, fatigue) applied on every pulse:
//!
//! - phase A (+00) runs while its remaining count is non-zero, applying
//!   its deltas towards fixed bounds (0 below, 255 or the maximum above);
//! - phase B (+14) then pulses towards the snapshot of the target taken
//!   when the effect began, restoring what phase A took, and frees the
//!   slot once every delta has reached its bound.
//!
//! Phase B's remaining count doubles as a mode: -1 pulses every tick
//! until restored, -3 frees the slot at once, -4 never ends (the flag
//! stays set), -5 makes phase A run forever.
//!
//! The record flags word (+40D) carries one bit per effect kind, and
//! +40F.. counts how many slots hold each kind (see `EFFECT_BITS`).
//!
//! The kill handler (2AA1:06A0) and the player-death handler
//! (2686:02B4) are not ported; they are reached through `FxHooks`.

use crate::character::field::*;
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::scale::{to100, to256};

pub const SLOTS: u16 = 0x0660;
pub const SLOT_SIZE: u16 = 0x3C;
pub const NSLOTS: u16 = 16;

/// Slot layout.
pub mod slot {
    /// Phase blocks; offsets within a block are in `block`.
    pub const PHASE_A: u16 = 0x00;
    pub const PHASE_B: u16 = 0x14;
    pub const TARGET_OFF: u16 = 0x28;
    pub const TARGET_SEG: u16 = 0x2A;
    /// The target's 8 attributes, HP, SP and fatigue when the effect began.
    pub const SNAPSHOT: u16 = 0x2C;
    pub const SNAP_HP: u16 = 0x34;
    pub const SNAP_SP: u16 = 0x36;
    pub const SNAP_FATIGUE: u16 = 0x38;
    /// Effect kind (bit position in `EFFECT_BITS`, 9 = none, or a disease id).
    pub const KIND: u16 = 0x3A;
    /// 0xFF = free; otherwise a pulse counter that skips 0xFF.
    pub const STATE: u16 = 0x3B;
}

/// Offsets within a phase block.
pub mod block {
    pub const COUNT: u16 = 0x00;
    pub const LEFT: u16 = 0x02;
    pub const PERIOD: u16 = 0x04;
    pub const ATTRS: u16 = 0x06;
    pub const HP: u16 = 0x0E;
    pub const SP: u16 = 0x10;
    pub const FATIGUE: u16 = 0x12;
}

/// Phase B "remaining" values with special meanings.
pub const B_FOREVER: u16 = 0xFFFF;
pub const B_FREE_AT_ONCE: u16 = 0xFFFD;
pub const B_HOLD: u16 = 0xFFFC;
pub const B_ENDLESS_A: u16 = 0xFFFB;

/// Record fields used here (offsets from a character/creature record).
pub mod rec {
    /// Per-kind effect counters, one byte for each of kinds 0..8.
    pub const EFFECT_COUNTS: u16 = 0x40F;
    /// Magic shield points; absorb HP damage first (not for creatures).
    pub const SHIELD: u16 = 0x41C;
    /// Current fatigue, bounded by (STR + END) << 6.
    pub const FATIGUE: u16 = 0x5D;
    /// Base attributes (the creation roll, see `create_player_stats`).
    pub const BASE_ATTRS: u16 = 0x31;
    /// First equipped item of the player (0212 in DGROUP).
    pub const ITEMS: u16 = 0x6E;
}

/// DGROUP 2BE7: effect flag bits by kind: 1, 2, 4 .. 80, FF, FE, 0.
pub const EFFECT_BITS: u16 = 0x2BE7;
/// Effect flag bits seen in the code.
pub const DISEASE: u16 = 0x01;
pub const PARALYSIS: u16 = 0x40;
/// Flag 0x80 marks a record whose HP changes move its maximum too
/// (unless 0x4000 is also set).
pub const FLAG_DRAINS_MAX: u16 = 0x80;
pub const FLAG_PLAYER_LIKE: u16 = 0x4000;
/// Kills are recorded as 0x100 (see progression).
pub const FLAG_DEAD: u16 = 0x100;

/// DGROUP variables.
pub mod var {
    pub const CREATURE_SEG: u16 = 0x1042;
    pub const DEFENDER: u16 = 0x1402;
    pub const CREATURE_COUNT: u16 = 0x0FF9;
    /// The player's record flags (01A4 + 40D): bit 0x10 is effect kind 4
    /// (drain); with bit 0x100 the drain check also covers creatures.
    pub const STATE_5B1: u16 = 0x05B1;
    /// The player's spell flags (01A4 + 41A): bit 0x40 stops paralysis and
    /// disease.
    pub const IMMUNE_5BE: u16 = 0x05BE;
    /// Bit 0x80 is set whenever effects change (redraw).
    pub const CHANGED: u16 = 0x0D39;
    /// Set to 0x0F when an effect message was written to 1405.
    pub const MESSAGE: u16 = 0x1428;
    pub const SCRATCH_1296: u16 = 0x1296;
    /// Level + 1 of the spell's caster, it seems (resistance and absorb).
    pub const CASTER: u16 = 0x1292;
    /// Tick bookkeeping: deltas applied, and "in phase B".
    pub const APPLIED: u16 = 0xA836;
    pub const RESTORING: u16 = 0xA838;
    /// Per creature type (from 1), percent chance of a special attack.
    pub const SPECIAL_CHANCE: u16 = 0x2DE6;
    /// Creature types whose disease armour stops (7 bytes).
    pub const ARMOUR_STOPS: u16 = 0x8298;
    /// 4 creature types that spread disease, and their disease lists.
    pub const DISEASE_TYPES: u16 = 0x82BB;
    pub const DISEASE_LISTS: u16 = 0x82BF;
    /// 3 diseases that run 3..17 days; others are permanent.
    pub const TIMED_DISEASES: u16 = 0x8BE8;
    /// Per disease (from id 10), the range of each delta.
    pub const DISEASE_RANGES: u16 = 0x8BBC;
}

/// 3276:78B0: per disease (from id 10), a pointer to the list of block
/// fields it changes, ending in 0xFF.
pub const DISEASE_FIELDS: u16 = 0x78B0;

/// Timed spell flags in record +41A (16 bits, see `spell_flag`), with a
/// far table at DGROUP 1020 of 4-byte entries per flag (timer, strength):
/// 16 for the player, then 16 for each of the 8 active creatures listed
/// at DGROUP 0C57 (1Ch bytes each: +0F flags, +12 record offset).
pub mod spell_flag {
    pub const DRUNK: u16 = 1 << 0; // ? order of the status strings at 6862
    pub const ABSORB: u16 = 1 << 10;
    pub const REFLECT: u16 = 1 << 11;
    pub const RESIST_SPELLS: u16 = 1 << 12;
    pub const TABLE: u16 = 0x1020;
    pub const ACTIVE: u16 = 0x0C57;
    pub const ACTIVE_SIZE: u16 = 0x1C;
    pub const ACTIVE_COUNT: u16 = 8;
}

/// Size of a creature record in the creature segment.
pub const CREATURE_SIZE: u16 = 0x41E;

/// Handlers the effect code calls but that are not ported here.
pub trait FxHooks {
    /// 2AA1:06A0, the kill handler, after a record's HP changed (it
    /// checks for death and awards XP). Returns its carry flag.
    fn vitals_changed(&mut self, m: &mut Machine, seg: u16, rec: u16) -> bool;
    /// 2686:02B4, the player died.
    fn player_died(&mut self, m: &mut Machine);
    /// 0000:27E5 with AX = 19h, after a creature reflected a spell; `actor`
    /// is its entry in the actor table (DGROUP 0AB3, 1Ch bytes each).
    fn spell_reflected(&mut self, _m: &mut Machine, _actor: u16) {}
}

/// Outcome of `apply_effect`, mirroring the original's flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// The target is outside DGROUP and the creature segment, and the
    /// creature segment's number is odd (ZF clear); nothing happened.
    Refused,
    /// The flag was already set (CF set).
    AlreadyActive,
    /// Applied; the slot used, or None for flag 0x100 (no slot).
    Slot(Option<u16>),
}

impl Machine {
    fn fx_or8(&mut self, seg: u16, off: u16, v: u8) {
        let x = self.mem.r8(seg, off) | v;
        self.mem.w8(seg, off, x);
    }

    fn fx_add16(&mut self, seg: u16, off: u16, v: u16) -> u16 {
        let x = self.mem.r16(seg, off).wrapping_add(v);
        self.mem.w16(seg, off, x);
        x
    }

    fn fx_target(&self, s: u16) -> (u16, u16) {
        (self.mem.r16(DS, s + slot::TARGET_SEG), self.mem.r16(DS, s + slot::TARGET_OFF))
    }

    fn fx_flags(&self, seg: u16, off: u16) -> u16 {
        self.mem.r16(seg, off.wrapping_add(FLAGS))
    }

    /// 2EBD:01F3. Kind of an effect flag: its position in `EFFECT_BITS`
    /// (11 bytes), or 10 if absent.
    pub fn effect_kind(&self, flag: u8) -> u16 {
        (0..11).find(|&i| self.mem.r8(DS, EFFECT_BITS + i) == flag).unwrap_or(10)
    }

    /// 2EBD:01B0. Takes a slot for an effect on seg:off: counts the kind
    /// in the target (kinds 9 and up are stored as 9, uncounted) and uses
    /// the first free slot, or the last slot when all are taken.
    fn fx_alloc_slot(&mut self, seg: u16, off: u16, flag: u8) -> u16 {
        let mut kind = self.effect_kind(flag);
        if kind >= 9 {
            kind = 9;
        } else {
            let at = off.wrapping_add(kind).wrapping_add(rec::EFFECT_COUNTS);
            self.mem.w8(seg, at, self.mem.r8(seg, at).wrapping_add(1));
        }
        let s = (0..NSLOTS)
            .map(|i| SLOTS + i * SLOT_SIZE)
            .find(|&s| self.mem.r8(DS, s + slot::STATE) == 0xFF)
            .unwrap_or(SLOTS + (NSLOTS - 1) * SLOT_SIZE);
        self.mem.w8(DS, s + slot::KIND, kind as u8);
        self.fx_or8(DS, s + 0x38, 0xCC);
        self.mem.w8(DS, s + 0x39, 1);
        s
    }

    /// 2EBD:0102. Starts an effect on the record at seg:off.
    ///
    /// `flag` is set in the record's flags (flags other than 0x10, 0x20 and
    /// 0x100 refuse to stack). Phase A pulses every `period` ticks for
    /// `duration` pulses (at least 1); phase B pulses every `b_period`
    /// with `b_left` remaining (or one of the `B_*` modes). The deltas
    /// start at whatever the slot held before.
    ///
    /// The effect message (into DGROUP 1405) is not formatted; only its
    /// flag at 1428 is set.
    pub fn apply_effect(
        &mut self, seg: u16, off: u16, flag: u16, period: u16, duration: u16, b_period: u16, b_left: u16,
    ) -> Applied {
        // 2EBD:00E5
        if seg != DS {
            let creatures = self.mem.r16(DS, var::CREATURE_SEG);
            if seg != creatures && creatures & 1 != 0 {
                return Applied::Refused;
            }
        }
        if flag != 0 {
            let stacks = matches!(flag, 0x100 | 0x20 | 0x10);
            let flags = self.fx_flags(seg, off);
            if !stacks && flags & flag != 0 {
                return Applied::AlreadyActive;
            }
            self.mem.w16(seg, off.wrapping_add(FLAGS), flags | flag);
        }
        if flag == 0x100 {
            // DS is still the target's segment here.
            self.fx_or8(seg, var::CHANGED, 0x80);
            return Applied::Slot(None);
        }
        let s = self.fx_alloc_slot(seg, off, flag as u8);
        let w = |m: &mut Self, f: u16, v: u16| m.mem.w16(DS, s + f, v);
        w(self, slot::PHASE_A + block::COUNT, 1);
        w(self, slot::PHASE_A + block::LEFT, duration.max(1));
        w(self, slot::PHASE_A + block::PERIOD, period);
        w(self, slot::PHASE_B + block::COUNT, 1);
        w(self, slot::PHASE_B + block::LEFT, b_left);
        w(self, slot::PHASE_B + block::PERIOD, b_period);
        w(self, slot::TARGET_SEG, seg);
        w(self, slot::TARGET_OFF, off);
        self.mem.w8(DS, s + slot::STATE, 0);
        for i in 0..8 {
            let a = self.mem.r8(seg, off.wrapping_add(ATTRS + i));
            self.mem.w8(DS, s + slot::SNAPSHOT + i, a);
        }
        for (from, to) in [(HP, slot::SNAP_HP), (SPELL_POINTS, slot::SNAP_SP), (rec::FATIGUE, slot::SNAP_FATIGUE)] {
            let v = self.mem.r16(seg, off.wrapping_add(from));
            self.mem.w16(DS, s + to, v);
        }
        // 2EBD:0202: the message, unless the kind is 9.
        if self.mem.r8(DS, s + slot::KIND) != 9 {
            self.mem.w8(DS, var::MESSAGE, 0x0F);
        }
        self.fx_or8(DS, var::CHANGED, 0x80);
        Applied::Slot(Some(s))
    }

    /// 2EBD:00B4. Ends a slot's effect on its target: clears the kind's
    /// flag (even if other slots hold the same kind) and decrements its
    /// counter. Kind 9 has neither; kinds 10 and up count as kind 0.
    pub fn end_effect(&mut self, s: u16) {
        if self.mem.r32(DS, s + slot::TARGET_OFF) == 0 {
            return;
        }
        let mut kind = self.mem.r8(DS, s + slot::KIND) as u16;
        if kind == 9 {
            return;
        }
        if kind >= 10 {
            kind = 0;
        }
        let mask = !(self.mem.r8(DS, EFFECT_BITS + kind) as u16);
        let (seg, off) = self.fx_target(s);
        let f = self.fx_flags(seg, off) & mask;
        self.mem.w16(seg, off.wrapping_add(FLAGS), f);
        let at = off.wrapping_add(kind).wrapping_add(rec::EFFECT_COUNTS);
        self.mem.w8(seg, at, self.mem.r8(seg, at).wrapping_sub(1));
    }

    /// 2EBD:0526. Frees a slot (offsets wrap within DGROUP).
    pub fn clear_slot(&mut self, s: u16) {
        for i in 0..SLOT_SIZE {
            self.mem.w8(DS, s.wrapping_add(i), 0);
        }
        self.mem.w8(DS, s.wrapping_add(slot::STATE), 0xFF);
        self.mem.w8(DS, s.wrapping_add(slot::KIND), 0xFF);
        self.fx_or8(DS, var::CHANGED, 0x80);
    }

    /// 2EBD:0084. Frees every slot aimed at seg:off (free slots included,
    /// by their zeroed pointer) and clears the low byte of its flags. The
    /// per-kind counters are left as they are.
    pub fn clear_effects_on(&mut self, seg: u16, off: u16) {
        for i in 0..NSLOTS {
            let s = SLOTS + i * SLOT_SIZE;
            if self.fx_target(s) == (seg, off) {
                self.clear_slot(s);
            }
        }
        let f = self.fx_flags(seg, off) & 0xFF00;
        self.mem.w16(seg, off.wrapping_add(FLAGS), f);
    }

    /// 2EBD:0190. The slot holding effect `kind` on seg:off, if any.
    pub fn find_effect_slot(&self, seg: u16, off: u16, kind: u8) -> Option<u16> {
        (0..NSLOTS).map(|i| SLOTS + i * SLOT_SIZE).find(|&s| {
            self.mem.r8(DS, s + slot::STATE) != 0xFF
                && self.fx_target(s) == (seg, off)
                && self.mem.r8(DS, s + slot::KIND) == kind
        })
    }

    /// 2EBD:054C. After loading a game: points used slots at DGROUP if
    /// they target offset 01A4 (the player), else at the creature segment.
    pub fn fix_slot_segments(&mut self) {
        for i in 0..NSLOTS {
            let s = SLOTS + i * SLOT_SIZE;
            if self.mem.r8(DS, s + slot::STATE) == 0xFF {
                continue;
            }
            self.fx_or8(DS, s + 0x38, 0xCC);
            let seg = if self.mem.r16(DS, s + slot::TARGET_OFF) == crate::character::PLAYER {
                DS
            } else {
                self.mem.r16(DS, var::CREATURE_SEG)
            };
            self.mem.w16(DS, s + slot::TARGET_SEG, seg);
        }
    }

    /// 2EBD:0268. One tick of every active effect.
    pub fn tick_effects(&mut self, hooks: &mut dyn FxHooks) {
        for i in 0..NSLOTS {
            self.fx_tick_slot(SLOTS + i * SLOT_SIZE, hooks);
        }
    }

    /// 2EBD:028A.
    fn fx_tick_slot(&mut self, s: u16, hooks: &mut dyn FxHooks) {
        if self.mem.r8(DS, s + slot::STATE) == 0xFF {
            return;
        }
        // 2EBD:0689 checks byte +38 of the *target* (its base luck) where
        // the 0xCC marker was put in the slot. When the test fails it
        // executes INT 3 (a no-op under DOS) and frees the slot, then
        // carries on ticking the emptied slot.
        let (seg, off) = self.fx_target(s);
        if self.mem.r8(seg, off.wrapping_add(0x38)) & 0xCC == 0 {
            self.clear_slot(s);
        }
        if self.mem.r16(DS, s + block::LEFT) != 0 {
            self.fx_phase_a(s, hooks);
        } else {
            self.fx_phase_b(s, hooks);
        }
    }

    /// 2EBD:02A4.
    fn fx_phase_a(&mut self, s: u16, hooks: &mut dyn FxHooks) {
        self.mem.w16(DS, var::RESTORING, 0);
        let ended = self.mem.r16(DS, s + slot::PHASE_B + block::LEFT) != B_ENDLESS_A
            && self.fx_add16(DS, s + block::LEFT, 0xFFFF) == 0;
        if !ended {
            if self.fx_add16(DS, s + block::COUNT, 0xFFFF) != 0 {
                return;
            }
            let period = self.mem.r16(DS, s + block::PERIOD);
            self.mem.w16(DS, s + block::COUNT, period);
            self.fx_next_state(s);
            self.fx_apply_block(s, s + slot::PHASE_A, hooks);
            if self.mem.r16(DS, s + block::LEFT) != 0 {
                return;
            }
        }
        if self.mem.r16(DS, s + slot::PHASE_B + block::LEFT) == B_HOLD {
            return;
        }
        self.end_effect(s);
    }

    /// 2EBD:02E4.
    fn fx_phase_b(&mut self, s: u16, hooks: &mut dyn FxHooks) {
        let b = s + slot::PHASE_B;
        let left = self.mem.r16(DS, b + block::LEFT);
        if left == B_HOLD {
            return;
        }
        self.mem.w16(DS, var::RESTORING, 1);
        let free = if left == B_FREE_AT_ONCE {
            Some(s)
        } else if left != B_FOREVER
            && (self.fx_add16(DS, b + block::LEFT, 0xFFFF), self.fx_add16(DS, b + block::COUNT, 0xFFFF)).1 != 0
        {
            None
        } else {
            let period = self.mem.r16(DS, b + block::PERIOD);
            self.mem.w16(DS, b + block::COUNT, period);
            self.fx_next_state(s);
            let (done, si) = self.fx_apply_block(s, b, hooks);
            if done {
                // SI may be FFFF here (a death during the HP step).
                Some(si)
            } else if si == 0xFFFF || self.mem.r16(DS, si + slot::PHASE_B + block::LEFT) != 0 {
                None
            } else {
                Some(si)
            }
        };
        if let Some(si) = free {
            self.clear_slot(si);
        }
        self.mem.w16(DS, var::RESTORING, 0);
    }

    fn fx_next_state(&mut self, s: u16) {
        let mut st = self.mem.r8(DS, s + slot::STATE).wrapping_add(1);
        if st == 0xFF {
            st = 0;
        }
        self.mem.w8(DS, s + slot::STATE, st);
    }

    /// 2EBD:0337. Applies a phase block to the slot's target; returns
    /// (every applied delta reached its bound, SI afterwards: the slot,
    /// or FFFF after a death).
    fn fx_apply_block(&mut self, s: u16, blk: u16, hooks: &mut dyn FxHooks) -> (bool, u16) {
        self.mem.w16(DS, var::APPLIED, 0);
        let mut bounded = 0u16;
        let (seg, off) = self.fx_target(s);
        for i in 0..8 {
            let snap = self.mem.r8(DS, s + slot::SNAPSHOT + i) as u16;
            let delta = self.mem.r8(DS, blk + block::ATTRS + i) as i8;
            let at = off.wrapping_add(ATTRS + i);
            let v = self.fx_attr_step(delta, snap, self.mem.r8(seg, at), &mut bounded);
            self.mem.w8(seg, at, v);
        }
        // 1194:361D on the record as it is: player attributes are in
        // percent, not the 256 scale that routine expects.
        self.fx_attribute_modifiers(seg, off);
        self.level_tier_stats(seg, off);

        let d = self.mem.r16(DS, blk + block::SP);
        if d != 0 {
            let bound = if (d as i16) < 0 { 0 } else { self.mem.r16(seg, off.wrapping_add(MAX_SPELL_POINTS)) };
            let v = (self.mem.r16(seg, off.wrapping_add(SPELL_POINTS)) as u32).wrapping_add(d as i16 as i32 as u32);
            let v = self.fx_clamp(v, bound, d, &mut bounded);
            self.mem.w16(seg, off.wrapping_add(SPELL_POINTS), v);
        }
        // Fatigue deltas are scaled by 64 in 32 bits; the direction is the
        // sign of the low 16 bits of the result.
        let e = ((self.mem.r16(DS, blk + block::FATIGUE) as i16 as i32) << 6) as u32;
        let ax = e as u16;
        if ax != 0 {
            let bound = if (ax as i16) < 0 {
                0
            } else {
                let str = self.mem.r8(seg, off.wrapping_add(ATTRS)) as u16;
                let end = self.mem.r8(seg, off.wrapping_add(ATTRS + 5)) as u16;
                (str + end) << 6
            };
            let v = (self.mem.r16(seg, off.wrapping_add(rec::FATIGUE)) as u32).wrapping_add(e);
            let v = self.fx_clamp(v, bound, ax, &mut bounded);
            self.mem.w16(seg, off.wrapping_add(rec::FATIGUE), v);
        }
        let si = self.fx_hp_step(s, blk, &mut bounded, hooks);
        (self.mem.r16(DS, var::APPLIED) == bounded, si)
    }

    /// 2EBD:0478. One attribute: the delta (percent) is scaled to 256ths
    /// and added. Phase A is bounded by 0 and 255, phase B by the
    /// snapshot (both ways).
    fn fx_attr_step(&mut self, delta: i8, snap: u16, attr: u8, bounded: &mut u16) -> u8 {
        let ax = to256(delta as i16) as u16;
        let restoring = self.mem.r16(DS, var::RESTORING) != 0;
        let mut bound = if restoring { snap } else { 0 };
        if ax == 0 {
            return attr;
        }
        if (ax as i16) > 0 && !restoring {
            bound = 0xFF;
        }
        let v = (attr as u16).wrapping_add(ax) as i16 as i32 as u32;
        self.fx_clamp(v, bound, ax, bounded) as u8
    }

    /// 2EBD:04AB. Clamps a 32-bit value moving in direction `ax` to
    /// `bound`, counting the delta in DGROUP A836 and, if the bound is
    /// reached (or passed), in `bounded`.
    fn fx_clamp(&mut self, v: u32, bound: u16, ax: u16, bounded: &mut u16) -> u16 {
        if ax == 0 {
            return v as u16;
        }
        let n = self.mem.r16(DS, var::APPLIED).wrapping_add(1);
        self.mem.w16(DS, var::APPLIED, n);
        let hit = if (ax as i16) > 0 {
            v >= bound as u32
        } else {
            (v as u16 as i16) < 0 || v as u16 <= bound
        };
        if hit {
            *bounded = bounded.wrapping_add(1);
            bound
        } else {
            v as u16
        }
    }

    /// 2EBD:03E9. HP: damage goes through the magic shield first; records
    /// with flag 0x80 (and not 0x4000) take it on their maximum as well.
    fn fx_hp_step(&mut self, s: u16, blk: u16, bounded: &mut u16, hooks: &mut dyn FxHooks) -> u16 {
        let mut ax = self.mem.r16(DS, blk + block::HP);
        if ax == 0 {
            return s;
        }
        let (seg, off) = self.fx_target(s);
        if (ax as i16) < 0 && self.mem.r16(seg, off.wrapping_add(rec::SHIELD)) != 0 {
            ax = self.absorb_damage(seg, off, ax.wrapping_neg()).wrapping_neg();
        }
        let flags = self.fx_flags(seg, off);
        if flags & FLAG_DRAINS_MAX != 0 && flags & FLAG_PLAYER_LIKE == 0 {
            // 2EBD:044B
            if (ax as i16) < 0 {
                self.fx_add16(seg, off.wrapping_add(MAX_HP), ax);
            }
            let hp = self.fx_add16(seg, off.wrapping_add(HP), ax);
            let max = self.mem.r16(seg, off.wrapping_add(MAX_HP));
            if (ax as i16) >= 0 && (hp as i16) > (max as i16) {
                self.mem.w16(seg, off.wrapping_add(HP), max);
            }
            return if hooks.vitals_changed(self, seg, off) { 0xFFFF } else { s };
        }
        let bound = if (ax as i16) < 0 { 0 } else { self.mem.r16(seg, off.wrapping_add(MAX_HP)) };
        let v = (self.mem.r16(seg, off.wrapping_add(HP)) as i16 as i32 as u32).wrapping_add(ax as i16 as i32 as u32);
        let v = self.fx_clamp(v, bound, ax, bounded);
        self.mem.w16(seg, off.wrapping_add(HP), v);
        // 2EBD:04D6. HP is compared unsigned, so only exactly 0 is death.
        if self.mem.r16(seg, off.wrapping_add(HP)) != 0 {
            if self.fx_flags(seg, off) & FLAG_CREATURE != 0 {
                hooks.vitals_changed(self, seg, off);
            }
            return s;
        }
        if self.fx_flags(seg, off) & FLAG_PLAYER_LIKE == 0 {
            hooks.vitals_changed(self, seg, off);
        } else {
            hooks.player_died(self);
        }
        // Makes the caller's "all bounded" test fail.
        self.mem.w16(DS, var::APPLIED, *bounded);
        *bounded = bounded.wrapping_sub(1);
        0xFFFF
    }

    /// 29CF:0C4F. Takes `damage` off the record's magic shield (+41C)
    /// first, unless it is a creature; returns what gets through, with
    /// the sign of `damage`.
    pub fn absorb_damage(&mut self, seg: u16, off: u16, damage: u16) -> u16 {
        let negative = (damage as i16) < 0;
        let mag = if negative { damage.wrapping_neg() } else { damage };
        let mut through = mag;
        let shield = off.wrapping_add(rec::SHIELD);
        if self.fx_flags(seg, off) & FLAG_CREATURE == 0 && self.mem.r16(seg, shield) != 0 {
            through = 0;
            let left = self.mem.r16(seg, shield).wrapping_sub(mag);
            self.mem.w16(seg, shield, left);
            if (left as i16) < 0 {
                self.mem.w16(seg, shield, 0);
                through = left.wrapping_neg();
            }
        }
        if negative { through.wrapping_neg() } else { through }
    }

    /// 1194:3621 (via 361D) on a record in any segment; the same formulas
    /// as `attribute_modifiers`, which only handles DGROUP records.
    pub fn fx_attribute_modifiers(&mut self, seg: u16, off: u16) {
        if seg == DS {
            self.attribute_modifiers(off);
            return;
        }
        let r = |f: u16| off.wrapping_add(f);
        let b: [u8; 8] = std::array::from_fn(|i| self.mem.r8(seg, r(ATTRS + i as u16)));
        let [str, int, wil, agi, spd, end, per, _lck] = b.map(|v| v as i16);
        let w = |m: &mut Self, f: u16, v: i16| m.mem.w16(seg, r(f), v as u16);
        w(self, DAMAGE_MOD, (str - 128 + 6) / 13);
        w(self, MAX_WEIGHT, str * 2);
        let max_sp = self.fx_spell_points(seg, off, to100(int) as u16);
        w(self, MAX_SPELL_POINTS, max_sp as i16);
        w(self, MAX_SPELL_POINTS_2, max_sp as i16);
        w(self, INT_MOD, int - 128);
        w(self, MAGIC_DEF, -(wil - 128));
        w(self, WIL_MOD_HALF, ((wil - 128) as u16 >> 1) as i16);
        for f in [ATTACK_MOD, DEFENCE_MOD, AGI_MOD_4B] {
            w(self, f, agi - 128);
        }
        w(self, SPD_MOD, (256 - spd + 12) / 25);
        let hp_mod = (end - 128 + 12) / 25;
        w(self, HP_MOD, hp_mod);
        w(self, HEAL_MOD, hp_mod);
        w(self, END_X2, end * 2);
        w(self, PER_MOD, per - 128);
        w(self, LCK_MOD, 50);
        self.mem.w8(seg, r(B_5F), ((b[2].wrapping_add(b[5]) as u16 + 0x200) / 0x400) as u8);
        self.mem.w8(seg, r(B_6A), ((b[0].wrapping_add(b[1]) as u16 + 0x400) / 0x800) as u8);
        self.mem.w8(seg, r(TIER_B), (end >> 2) as u8);
        self.mem.w8(seg, r(TIER_C), self.mem.r16(seg, r(MAGIC_DEF)) as u8);
    }

    /// 1194:37D3 for a record in any segment; returns the maximum.
    fn fx_spell_points(&self, seg: u16, off: u16, int: u16) -> u16 {
        let class = self.mem.r8(seg, off.wrapping_add(CLASS));
        let sp = if class & 0x20 == 0 || class == 0x23 {
            0
        } else if class == 0xE6 {
            int
        } else {
            match self.mem.r8(DATA, crate::character::SPELL_KIND.wrapping_add((class & 0x1f) as u16)) {
                2 => int.wrapping_add(int),
                0 => int.wrapping_add(int >> 1),
                _ => int.wrapping_add(int >> 2).wrapping_add(int >> 1),
            }
        };
        if class == 0x23 { int.wrapping_mul(3) } else { sp }
    }

    /// 2EBD:0597. Disease timing: (period, duration, phase B mode). The
    /// three timed diseases last 3..17 days of 1440 minutes; the rest
    /// pulse daily forever.
    fn fx_disease_timing(&mut self, id: u8) -> Option<(u16, u16, u16)> {
        if (0..3).any(|k| self.mem.r8(DS, var::TIMED_DISEASES + k) == id) {
            let days = self.rng.range(3, 18)?;
            Some((0x5A0, days.wrapping_mul(0x5A0), B_FREE_AT_ONCE))
        } else {
            Some((0x5A0, 0x5A0, B_ENDLESS_A))
        }
    }

    /// 2EBD:0573. Gives the record at seg:off disease `id` (10 and up):
    /// flag 1, then its kind and phase A deltas.
    ///
    /// When `apply_effect` refuses (target outside the known segments),
    /// the original carries on with DS:SI still at the target, so the
    /// kind and deltas are written into the target record itself.
    pub fn apply_disease(&mut self, seg: u16, off: u16, id: u16) -> Option<Applied> {
        self.mem.w16(DS, var::SCRATCH_1296, id);
        let (period, duration, mode) = self.fx_disease_timing(id as u8)?;
        let res = self.apply_effect(seg, off, DISEASE, period, duration, 1, mode);
        let (bseg, base) = match res {
            Applied::AlreadyActive => return Some(res),
            Applied::Slot(Some(s)) => (DS, s),
            Applied::Slot(None) | Applied::Refused => (seg, off),
        };
        self.mem.w8(bseg, base.wrapping_add(slot::KIND), id as u8);
        self.fx_disease_deltas(bseg, base, id.wrapping_sub(10))?;
        Some(res)
    }

    /// 2EBD:05E7. Rolls each listed phase A delta (fields below 8 are
    /// attributes, bytes; the rest words) as minus range(lo, hi).
    fn fx_disease_deltas(&mut self, bseg: u16, base: u16, d: u16) -> Option<()> {
        let d2 = d.wrapping_add(d);
        let mut p = self.mem.r16(DATA, DISEASE_FIELDS.wrapping_add(d2));
        for _ in 0..0x10000 {
            let f = self.mem.r8(DATA, p) as u16;
            if f == 0xFF {
                return Some(());
            }
            // 2EBD:05CC
            let lo = self.mem.r8(DS, var::DISEASE_RANGES.wrapping_add(d2)) as u16;
            let hi = self.mem.r8(DS, var::DISEASE_RANGES.wrapping_add(d2).wrapping_add(1)) as u16;
            let v = self.rng.range(lo, hi)?.wrapping_neg();
            let at = base.wrapping_add(f).wrapping_add(block::ATTRS);
            if f < 8 {
                self.mem.w8(bseg, at, v as u8);
            } else {
                self.mem.w16(bseg, at, v);
            }
            p = p.wrapping_add(1);
        }
        None // no terminator anywhere in the segment: the original hangs
    }

    /// 2AA1:035D. After a creature (seg:off) hits the player: by creature
    /// type, a chance of a special attack, which paralyses (types 3 and 9)
    /// or else may infect. `armoured` is 1194:7449 for the hit location
    /// (an item worn there), which stops disease from the creature types
    /// at DGROUP 8298. None where the original faults.
    pub fn creature_special_attack(&mut self, seg: u16, off: u16, armoured: bool) -> Option<()> {
        let ty = self.mem.r8(seg, off.wrapping_add(RACE)) as u16;
        let chance = if ty == 0x16 {
            5
        } else {
            let c = self.mem.r8(DS, var::SPECIAL_CHANCE.wrapping_add(ty.wrapping_sub(1))) as i8;
            if c <= 0 {
                return Some(());
            }
            c as u16
        };
        if self.percent() > chance {
            return Some(());
        }
        let roll = self.percent() as u8;
        if self.fx_paralyse(roll, ty)? {
            return Some(());
        }
        self.fx_infect(roll, ty, armoured)
    }

    /// 2AA1:03B3. Paralysis by creature types 3 and 9 if `roll` is under
    /// the player's TIER_B; High Elves (race 4) and class byte 0x11 are
    /// immune. True (CF) if it struck.
    fn fx_paralyse(&mut self, roll: u8, ty: u16) -> Option<bool> {
        let p = crate::character::PLAYER;
        if self.mem.r16(DS, var::IMMUNE_5BE) & 0x40 != 0
            || self.mem.r8(DS, p + RACE) == 4
            || self.mem.r8(DS, p + CLASS) == 0x11
            || (ty != 3 && ty != 9)
            || roll >= self.mem.r8(DS, p + TIER_B)
        {
            return Some(false);
        }
        let turns = crate::dice::roll(&mut self.rng, 6)? + 1;
        self.apply_effect(DS, p, PARALYSIS, 1, turns, 1, B_FREE_AT_ONCE);
        Some(true)
    }

    /// 2AA1:03FC. Disease from the four disease-carrying creature types,
    /// one picked from the type's list, if `roll` is under the player's
    /// TIER_B; Barbarians (class byte 0x0F) are immune.
    fn fx_infect(&mut self, roll: u8, ty: u16, armoured: bool) -> Option<()> {
        let p = crate::character::PLAYER;
        if self.mem.r16(DS, var::IMMUNE_5BE) & 0x40 != 0
            || self.mem.r8(DS, p + CLASS) == 0x0F
            || roll >= self.mem.r8(DS, p + TIER_B)
        {
            return Some(());
        }
        let t = ty as u8;
        if armoured && (0..7).any(|k| self.mem.r8(DS, var::ARMOUR_STOPS + k) == t) {
            return Some(());
        }
        let Some(k) = (0..4).find(|&k| self.mem.r8(DS, var::DISEASE_TYPES + k) == t) else {
            return Some(());
        };
        let list = self.mem.r16(DS, var::DISEASE_LISTS + 2 * k);
        let n = (0..0x10000u32).find(|&i| self.mem.r8(DS, list.wrapping_add(i as u16)) == 0xFF)? as u16;
        let r = crate::dice::roll(&mut self.rng, n)?;
        let id = self.mem.r8(DS, list.wrapping_add(r)) as u16;
        self.apply_disease(DS, p, id).map(drop)
    }

    /// 2AA1:08DC. Player weapon enchantments on a hit for `damage`:
    /// `has_85`/`has_88` are 1194:7C69 for enchantments 0x85 and 0x88 on
    /// the equipped items. Either triples the damage; 0x88 also tries to
    /// paralyse the defender (DGROUP 1402 in the creature segment) for 10
    /// on a percent roll under the player's TIER_C, costing the weapon at
    /// DGROUP 0212 800 condition (a broken-weapon message is not shown).
    pub fn weapon_enchantments(&mut self, damage: u16, has_85: bool, has_88: bool) -> u16 {
        if !has_85 {
            if !has_88 {
                return damage;
            }
            let seg = self.mem.r16(DS, var::CREATURE_SEG);
            let off = self.mem.r16(DS, var::DEFENDER);
            let flags = self.fx_flags(seg, off);
            let immune = self.mem.r8(seg, off.wrapping_add(CLASS)) == 0x11 && flags & FLAG_CREATURE == 0;
            if !immune && (self.percent() as u8) < self.mem.r8(DS, crate::character::PLAYER + TIER_C) {
                self.apply_effect(seg, off, PARALYSIS, 1, 10, 1, B_FREE_AT_ONCE);
                let weapon = crate::character::PLAYER + rec::ITEMS;
                self.fx_add16(DS, weapon + 6, 800u16.wrapping_neg());
            }
        }
        damage.wrapping_mul(3)
    }

    /// 2AA1:0956. With enchantment 0x84 (`has_84`), the player heals half
    /// the damage dealt (arithmetic shift), up to the maximum (unsigned).
    pub fn vampiric_heal(&mut self, damage: u16, has_84: bool) {
        if !has_84 {
            return;
        }
        let p = crate::character::PLAYER;
        let hp = self.fx_add16(DS, p + HP, ((damage as i16) >> 1) as u16);
        let max = self.mem.r16(DS, p + MAX_HP);
        if hp > max {
            self.mem.w16(DS, p + HP, max);
        }
    }

    /// 2686:0004. Each tick after the effects: the player, and with
    /// DGROUP 05B1 bit 0x100 every live creature, dies if any attribute
    /// has drained to 0.
    pub fn drain_death_check(&mut self, hooks: &mut dyn FxHooks) {
        self.fx_check_drained(DS, crate::character::PLAYER, hooks);
        if self.mem.r16(DS, var::STATE_5B1) & 0x100 == 0 {
            return;
        }
        let n = self.mem.r16(DS, var::CREATURE_COUNT);
        let seg = self.mem.r16(DS, var::CREATURE_SEG);
        let mut off = 0u16;
        for _ in 0..n {
            if self.fx_flags(seg, off) & FLAG_DEAD == 0 {
                self.fx_check_drained(seg, off, hooks);
            }
            off = off.wrapping_add(CREATURE_SIZE);
        }
    }

    /// 2686:0036. Offset 01A4 counts as the player in any segment.
    fn fx_check_drained(&mut self, seg: u16, off: u16, hooks: &mut dyn FxHooks) {
        if (0..8).all(|i| self.mem.r8(seg, off.wrapping_add(ATTRS + i)) != 0) {
            return;
        }
        if off == crate::character::PLAYER {
            hooks.player_died(self);
        } else {
            self.mem.w16(seg, off.wrapping_add(HP), 0);
            hooks.vitals_changed(self, seg, off);
        }
    }

    /// 2EBD:0617. With drain effects active (05B1 bit 0x10), lowers the
    /// snapshot of attribute `attr` in every kind-4 slot that drains it by
    /// `value` (256 scale), but not below the player's base attribute. Run
    /// when the player raises an attribute.
    pub fn rebase_drain_snapshots(&mut self, value: u16, attr: u16) {
        if self.mem.r16(DS, var::STATE_5B1) & 0x10 == 0 {
            return;
        }
        for i in 0..NSLOTS {
            let s = SLOTS + i * SLOT_SIZE;
            if self.mem.r8(DS, s + slot::KIND) != 4 || self.mem.r8(DS, attr.wrapping_add(s + block::ATTRS)) == 0 {
                continue;
            }
            let at = attr.wrapping_add(s + slot::SNAPSHOT);
            let mut v = (self.mem.r8(DS, at) as u16).wrapping_sub(value);
            if (v as i16) < 0 {
                v &= 0xFF00;
            }
            let base = self.mem.r8(DS, attr.wrapping_add(crate::character::PLAYER + rec::BASE_ATTRS));
            self.mem.w8(DS, at, (v as u8).max(base));
        }
    }

    /// 2EBD:0657. Whether a kind-4 slot drains attribute `attr` (only
    /// while 05B1 bit 0x10 is set).
    pub fn attr_drained(&self, attr: u16) -> bool {
        self.mem.r16(DS, var::STATE_5B1) & 0x10 != 0
            && (0..NSLOTS).any(|i| {
                let s = SLOTS + i * SLOT_SIZE;
                self.mem.r8(DS, s + slot::KIND) == 4 && self.mem.r8(DS, attr.wrapping_add(s + block::ATTRS)) != 0
            })
    }

    /// 2CDD:0964. One tick of the timed spell flags (+41A): the player's,
    /// then each active creature's whose entry lacks flag 4000h. Creature
    /// k's timers are block k + 1 of the table, or block k when the player
    /// has no flags (unlike `spell_flag_table`).
    pub fn tick_spell_flags(&mut self) {
        let (tseg, mut toff) = (self.mem.r16(DS, spell_flag::TABLE + 2), self.mem.r16(DS, spell_flag::TABLE));
        // The player's block is only stepped over when the player has
        // flags; otherwise the creatures' blocks start one too early.
        if self.mem.r16(DS, crate::character::PLAYER + 0x41A) != 0 {
            self.fx_tick_flags(DS, crate::character::PLAYER, tseg, toff);
            toff = toff.wrapping_add(0x40);
        }
        for k in 0..spell_flag::ACTIVE_COUNT {
            let e = spell_flag::ACTIVE + k * spell_flag::ACTIVE_SIZE;
            if self.mem.r16(DS, e + 0x0F) & 0x4000 == 0 {
                let seg = self.mem.r16(DS, var::CREATURE_SEG);
                let off = self.mem.r16(DS, e + 0x12);
                self.fx_tick_flags(seg, off, tseg, toff);
            }
            toff = toff.wrapping_add(0x40);
        }
    }

    /// 2CDD:0999. Counts down the timer of each set flag and clears the
    /// flag when it reaches 0 (or was 0); timers of clear flags are zeroed.
    fn fx_tick_flags(&mut self, seg: u16, off: u16, tseg: u16, mut t: u16) {
        let at = off.wrapping_add(0x41A);
        let flags = self.mem.r16(seg, at);
        if flags == 0 {
            return;
        }
        for i in 0..16 {
            if flags & (1 << i) != 0 {
                let timer = self.mem.r16(tseg, t);
                let expired = timer == 0 || {
                    self.mem.w16(tseg, t, timer - 1);
                    timer == 1
                };
                if expired {
                    let f = self.mem.r16(seg, at) & !(1u16 << i);
                    self.mem.w16(seg, at, f);
                    self.fx_or8(DS, var::CHANGED, 0x80);
                }
            } else {
                self.mem.w16(tseg, t, 0);
            }
            t = t.wrapping_add(4);
        }
    }

    /// 2CDD:09E0. The spell-flag table entries (segment, offset) of the
    /// record at seg:off: the player's (DGROUP:01A4), or an active
    /// creature's (in the creature segment). For anyone else the original
    /// hits an INT 3 and carries on with the record's own segment and
    /// offset 0D37.
    pub fn spell_flag_table(&self, seg: u16, off: u16) -> (u16, u16) {
        let (tseg, toff) = (self.mem.r16(DS, spell_flag::TABLE + 2), self.mem.r16(DS, spell_flag::TABLE));
        if seg == DS && off == crate::character::PLAYER {
            return (tseg, toff);
        }
        for k in 0..spell_flag::ACTIVE_COUNT {
            let e = spell_flag::ACTIVE + k * spell_flag::ACTIVE_SIZE;
            if self.mem.r16(DS, e + 0x12) == off && seg == self.mem.r16(DS, var::CREATURE_SEG) {
                return (tseg, toff.wrapping_add(0x40).wrapping_add(k << 6));
            }
        }
        (seg, spell_flag::ACTIVE + spell_flag::ACTIVE_COUNT * spell_flag::ACTIVE_SIZE)
    }

    /// 2CDD:0B46. Whether the record at seg:off resists through spell flag
    /// entry `entry` (flag number × 4): a percent roll up to the flag's
    /// strength plus 5 per level the target has over DGROUP 1292 (computed
    /// in 8 bits).
    fn fx_resist_roll(&mut self, seg: u16, off: u16, entry: u16) -> bool {
        let lvl = (self.mem.r8(seg, off.wrapping_add(LEVEL)) as u16 + 1).wrapping_sub(self.mem.r16(DS, var::CASTER));
        let bonus = (lvl as u8 as i8 as i16 * 5) as u16;
        let (tseg, toff) = self.spell_flag_table(seg, off);
        let chance = self.mem.r16(tseg, entry.wrapping_add(toff).wrapping_add(2)).wrapping_add(bonus);
        self.percent() < chance.wrapping_add(1)
    }

    /// 2CDD:0A38. Whether the record at seg:off stops a spell costing
    /// `cost` (spell +32): Sorcerers (class byte 23h) absorb it on a random
    /// byte under (INT + WIL) / 2 (at most C0h), regaining cost / (level +
    /// 1) SP; then resist magic (flag 1000h), absorb (400h, regaining
    /// cost / [1292] SP) and reflect (800h, queueing the projectile at
    /// [A886] back). Absorb and reflect are only tried if one of 1C00h is
    /// set. None where the original faults ([1292] = 0).
    pub fn spell_stopped(&mut self, seg: u16, off: u16, cost: u16, hooks: &mut dyn FxHooks) -> Option<bool> {
        let r = |f: u16| off.wrapping_add(f);
        let max_sp = |m: &mut Self, gain: u16| {
            let sp = m.fx_add16(seg, r(SPELL_POINTS), gain);
            let max = m.mem.r16(seg, r(MAX_SPELL_POINTS));
            if sp > max {
                m.mem.w16(seg, r(SPELL_POINTS), max);
            }
        };
        if self.mem.r8(seg, r(CLASS)) == 0x23 {
            let a = self.mem.r8(seg, r(ATTRS + 1)) as u16 + self.mem.r8(seg, r(ATTRS + 2)) as u16;
            let limit = ((a >> 1) as u8).min(0xC0);
            if (self.rng.next() as u8) < limit {
                let level = self.mem.r8(seg, r(LEVEL)) as u16 + 1;
                max_sp(self, cost / level);
                return Some(true);
            }
        }
        let flags = self.mem.r16(seg, r(0x41A));
        if flags & 0x1C7C == 0 || flags & 0x1C00 == 0 {
            return Some(false);
        }
        if flags & spell_flag::RESIST_SPELLS != 0 && self.fx_resist_roll(seg, off, 0x30) {
            return Some(true);
        }
        if self.mem.r16(seg, r(0x41A)) & spell_flag::ABSORB != 0 && self.fx_resist_roll(seg, off, 0x28) {
            let div = self.mem.r16(DS, var::CASTER);
            if div == 0 {
                return None;
            }
            max_sp(self, cost / div);
            return Some(true);
        }
        if self.mem.r16(seg, r(0x41A)) & spell_flag::REFLECT == 0 || !self.fx_resist_roll(seg, off, 0x2C) {
            return Some(false);
        }
        let projectile = self.mem.r16(DS, 0xA886);
        let source = self.mem.r32(DS, projectile.wrapping_add(4));
        let at = (self.mem.r16(DS, 0x0D5A) << 2).wrapping_add(0x0D5C);
        self.mem.w32(DS, at, source);
        self.fx_add16(DS, 0x1276, 1);
        let caster = source as u16;
        if caster != crate::character::PLAYER {
            hooks.spell_reflected(self, (caster / CREATURE_SIZE).wrapping_mul(0x1C).wrapping_add(0x0AB3));
        }
        Some(true)
    }
}
