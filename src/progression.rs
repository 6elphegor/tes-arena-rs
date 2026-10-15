//! Experience, levels and level-dependent stats (segment 2686, plus XP
//! awards elsewhere).
//!
//! The player's level is the record byte +6 (DGROUP 01AA, 0-based) and
//! XP is the record dword +409 (DGROUP 05AD). Awarding XP sets bit 0x100
//! of DGROUP 05CA; `level_up` clears it and checks for new levels.

use crate::character::PLAYER;
use crate::dice::roll;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

use crate::character::field::*;

/// DGROUP dword: player XP (PLAYER + XP).
pub const PLAYER_XP: u16 = 0x05AD;
/// DGROUP word: bit 0x20 disables levelling.
pub const GAME_FLAGS: u16 = 0x05C8;
/// DGROUP word: bit 0x100 means "XP changed, check for a level".
pub const PENDING_FLAGS: u16 = 0x05CA;
/// 3276:47AE: XP for the first level, one word per class index.
pub const BASE_XP: u16 = 0x47AE;
/// Level-threshold scratch: multiplier and current threshold (dwords).
const THRESH_MUL: u16 = 0xA836;
const THRESH: u16 = 0xA844;
/// Tables read by `level_tier_stats`, 8 tiers for each of 3 class groups.
const TIER_A: u16 = 0x47D2;
const TIER_B_TAB: u16 = 0x47EA;
const TIER_C_TAB: u16 = 0x4802;
/// Main-quest state in DGROUP: in progress, pieces found, piece ready.
const STAFF_ACTIVE: u16 = 0x0F76;
const STAFF_PIECES: u16 = 0x0F77;
const STAFF_F7A: u16 = 0x0F7A;
const STAFF_READY: u16 = 0x0F7C;
const STAFF_ASKED: u16 = 0x0F79;
const STAFF_F7E: u16 = 0x0F7E;

impl Machine {
    fn set_pending_level_check(&mut self) {
        let f = self.mem.r16(DS, PENDING_FLAGS) | 0x100;
        self.mem.w16(DS, PENDING_FLAGS, f);
    }

    /// 2686:039E. Starting from `level`, counts thresholds `xp` has passed,
    /// growing the threshold by mul/16 each time (32-bit, low half kept,
    /// so it can wrap). Stops when the count reaches exactly 9; the count
    /// is 16-bit, so beyond 9 it only stops at the threshold or after
    /// wrapping round to 9.
    fn count_levels(&mut self, xp: u32, mut level: u16) -> u16 {
        loop {
            if xp < self.mem.r32(DS, THRESH) {
                return level;
            }
            level = level.wrapping_add(1);
            if level == 9 {
                return level;
            }
            let t = self.mem.r32(DS, THRESH).wrapping_mul(self.mem.r32(DS, THRESH_MUL)) >> 4;
            self.mem.w32(DS, THRESH, t);
        }
    }

    /// 2686:0333. Works out the player's level from XP and stores it if it
    /// rose. Returns the original's AX when it did (CF set): AL = levels
    /// gained, AH = high byte of the new level. The level byte is compared
    /// and stored as 8 bits.
    pub fn check_level_up(&mut self) -> Option<u16> {
        self.mem.w32(DS, THRESH_MUL, 30);
        let class = (self.mem.r8(DS, PLAYER + CLASS) & 0x1f) as u16;
        let base = self.mem.r16(DATA, BASE_XP + class * 2);
        self.mem.w32(DS, THRESH, base as u32);
        let xp = self.mem.r32(DS, PLAYER_XP);
        let mut level = self.count_levels(xp, 0);
        if level as u8 >= 9 {
            self.mem.w32(DS, THRESH_MUL, 24);
            let t = self.mem.r32(DS, THRESH).wrapping_mul(24) >> 4;
            self.mem.w32(DS, THRESH, t);
            level = self.count_levels(self.mem.r32(DS, PLAYER_XP), 9);
        }
        let old = self.mem.r8(DS, PLAYER + LEVEL);
        if old >= level as u8 {
            return None;
        }
        self.mem.w8(DS, PLAYER + LEVEL, level as u8);
        Some((level & 0xff00) | (level as u8).wrapping_sub(old) as u16)
    }

    /// 2686:02D8 without its UI calls (0000:9AF8 and the level-up screen
    /// 1194:1034, which gets the returned count in CX).
    ///
    /// Returns Some(None) if there was no level up, Some(Some(n)) after
    /// applying n levels (n is the AX from `check_level_up`), and None
    /// where the original faults (hit die 0).
    pub fn level_up(&mut self) -> Option<Option<u16>> {
        if self.mem.r16(DS, GAME_FLAGS) & 0x20 != 0 {
            return Some(None);
        }
        let f = self.mem.r16(DS, PENDING_FLAGS) & 0xfeff;
        self.mem.w16(DS, PENDING_FLAGS, f);
        let Some(n) = self.check_level_up() else { return Some(None) };
        for _ in 0..n {
            let class = (self.mem.r8(DS, PLAYER + CLASS) & 0x1f) as u16;
            let die = self.mem.r8(DS, crate::character::HIT_DIE + class) as u16;
            let gain = roll(&mut self.rng, die)?
                .wrapping_add(1)
                .wrapping_add(self.mem.r16(DS, PLAYER + HP_MOD));
            let max = self.mem.r16(DS, PLAYER + MAX_HP).wrapping_add(gain);
            self.mem.w16(DS, PLAYER + MAX_HP, max);
            self.level_tier_stats(DS, PLAYER);
        }
        Some(Some(n))
    }

    /// 2686:03C9. Sets five record bytes from tables indexed by level tier
    /// (one per 3 levels, up to 7) and class group.
    ///
    /// The tables are read from `seg` (the original's DS, which is also
    /// the record's segment). They live in 3276, but level-up calls this
    /// with DS = DGROUP, so for the player they come from DGROUP 47D2..
    /// instead. Class groups are 0-6, 7-12 and 13+ (creatures count as
    /// group 0) and step by 6 although the tables hold 8 tiers per group.
    pub fn level_tier_stats(&mut self, seg: u16, rec: u16) {
        let r8 = |m: &Self, f: u16| m.mem.r8(seg, rec.wrapping_add(f));
        let mut group = r8(self, CLASS) & 0x1f;
        if self.mem.r16(seg, rec.wrapping_add(FLAGS)) & FLAG_CREATURE != 0 {
            group = 0;
        }
        let level = r8(self, LEVEL);
        let mut tier = 0u16;
        while tier < 7 && level > (tier * 3) as u8 {
            tier += 1;
        }
        if group > 6 {
            tier += 6;
            if group > 12 {
                tier += 6;
            }
        }
        let a = self.mem.r8(seg, TIER_A + tier);
        for f in [TIER_A1, TIER_A2, TIER_A3] {
            self.mem.w8(seg, rec.wrapping_add(f), a);
        }
        let b = self.mem.r8(seg, TIER_B_TAB + tier);
        self.mem.w8(seg, rec.wrapping_add(TIER_B), b);
        let c = self.mem.r8(seg, TIER_C_TAB + tier);
        self.mem.w8(seg, rec.wrapping_add(TIER_C), c);
        // Barbarian (class byte exactly 0x0F).
        if r8(self, CLASS) == 0x0F {
            self.mem.w8(seg, rec.wrapping_add(TIER_B), 0);
        }
    }

    /// 2686:02CE. Is the player's class byte `class`?
    pub fn player_class_is(&self, class: u8) -> bool {
        self.mem.r8(DS, PLAYER + CLASS) == class
    }

    /// 1194:5585. Skips `n` NUL-terminated strings at seg:off, scanning at
    /// most 3000 bytes for each terminator; returns the new offset.
    pub fn skip_strings(&self, seg: u16, mut off: u16, n: u16) -> u16 {
        for _ in 0..n {
            for _ in 0..3000 {
                let b = self.mem.r8(seg, off);
                off = off.wrapping_add(1);
                if b == 0 {
                    break;
                }
            }
        }
        off
    }

    /// 2686:0645. Name of a record's class, or for creatures the name of
    /// its type (by the race byte). Returns (segment, offset).
    pub fn class_name(&self, seg: u16, rec: u16) -> (u16, u16) {
        if self.mem.r16(seg, rec.wrapping_add(FLAGS)) & FLAG_CREATURE != 0 {
            let kind = self.mem.r8(seg, rec.wrapping_add(RACE)) as u16;
            (DATA, self.skip_strings(DATA, 0x465D, kind))
        } else {
            let class = (self.mem.r8(seg, rec.wrapping_add(CLASS)) & 0x1f) as u16;
            (DS, self.skip_strings(DS, 0x2A62, class))
        }
    }

    /// 2AA1:06D5..06E8. Adds XP (e.g. for a kill) to the player.
    pub fn award_xp(&mut self, xp: u32) {
        let v = self.mem.r32(DS, PLAYER_XP).wrapping_add(xp);
        self.mem.w32(DS, PLAYER_XP, v);
        self.set_pending_level_check();
    }

    /// 1194:9099..90B8. Quest reward: (level + 1) × `per_level` XP in 16
    /// bits (500 for main quests, 50 for minor ones), at least 200.
    pub fn award_quest_xp(&mut self, per_level: u16) {
        let level = self.mem.r8(DS, PLAYER + LEVEL) as u16 + 1;
        let xp = level.wrapping_mul(per_level).max(200);
        self.award_xp(xp as u32);
    }

    /// 1194:D340. Starts looking for the next staff piece. True (CF clear)
    /// when none is in progress, the player is level 1 or more (0-based)
    /// and fewer than 8 pieces are found. Writes the flags even when the
    /// piece count then refuses.
    pub fn start_staff_piece(&mut self) -> bool {
        if self.mem.r8(DS, STAFF_ACTIVE) != 0 || self.mem.r8(DS, PLAYER + LEVEL) < 1 {
            return false;
        }
        self.mem.w8(DS, STAFF_ACTIVE, 1);
        self.mem.w8(DS, STAFF_F7A, 0);
        self.mem.r8(DS, STAFF_PIECES) < 8
    }

    /// 1194:D363. Completes a staff piece: clears the 11 bytes of quest
    /// state at DGROUP 0F76 (through 0000:77C8 with ES = DGROUP), counts
    /// the piece and awards 1000 × (level + 1) XP, computed in 16 bits and
    /// capped at 65000. False (CF set) if no piece is ready.
    pub fn complete_staff_piece(&mut self) -> bool {
        if self.mem.r8(DS, STAFF_READY) == 0 {
            return false;
        }
        let pieces = self.mem.r8(DS, STAFF_PIECES);
        for i in 0..11 {
            self.mem.w8(DS, STAFF_ACTIVE + i, 0);
        }
        self.mem.w8(DS, STAFF_PIECES, pieces.wrapping_add(1));
        let f = self.mem.r16(DS, GAME_FLAGS) | 0x8000;
        self.mem.w16(DS, GAME_FLAGS, f);
        let level = self.mem.r8(DS, PLAYER + LEVEL) as u16 + 1;
        let xp = 1000u16.wrapping_mul(level).min(65000);
        self.award_xp(xp as u32);
        true
    }

    /// 1194:D55F, leaving an interior for the outside (0000:44BD): with a
    /// staff piece sought (0F76), 0F7E set, and with the person's quest
    /// taken (0F79) the piece ready (0F7C, which `complete_staff_piece`
    /// asks for); then the stores' and guilds' stock files deleted
    /// ("eq*.??", 5CC4; "gl*.??", 5CCB; 1194:4D91).
    pub fn staff_leave(&mut self, files: &mut crate::bsa::Files) {
        self.staff_leave_with(&mut |m, si| m.delete_matching(files, si));
    }

    /// `staff_leave` with the deletes (SI the pattern) left to `delete`.
    pub fn staff_leave_with(&mut self, delete: &mut dyn FnMut(&mut Machine, u16)) {
        if self.mem.r8(DS, STAFF_ACTIVE) != 0 {
            self.mem.w8(DS, STAFF_F7E, 1);
            if self.mem.r8(DS, STAFF_ASKED) != 0 {
                self.mem.w8(DS, STAFF_READY, 1);
            }
        }
        delete(self, 0x5CC4);
        delete(self, 0x5CCB);
    }

    /// 1194:3893..38B0. Hit points for a character created at a level
    /// (e.g. an NPC): one hit die per level (`roll_hit_die`, at least 1),
    /// counting level 0, plus 25.
    pub fn roll_level_hit_points(&mut self, rec: u16) -> Option<()> {
        self.mem.w16(DS, rec.wrapping_add(MAX_HP), 0);
        self.mem.w16(DS, rec.wrapping_add(HP), 0);
        let n = self.mem.r8(DS, rec.wrapping_add(LEVEL)) as u16 + 1;
        for _ in 0..n {
            let hp = self.roll_hit_die(rec)?;
            let v = self.mem.r16(DS, rec.wrapping_add(MAX_HP)).wrapping_add(hp);
            self.mem.w16(DS, rec.wrapping_add(MAX_HP), v);
        }
        let v = self.mem.r16(DS, rec.wrapping_add(MAX_HP)).wrapping_add(25);
        self.mem.w16(DS, rec.wrapping_add(MAX_HP), v);
        Some(())
    }
}
