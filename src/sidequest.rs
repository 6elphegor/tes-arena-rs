//! Side quests made (segment 1194, 9358..98E7 and 9D64..9EFC): the
//! records a place offers, made when the player comes to a new place.
//!
//! The records are those of `quest`: four of 34h bytes at QUESTS (a
//! place's own quests: a person or thing to find, an escort, a delivery
//! across the province) and four of 29h at OTHERS (errands within the
//! town, to one of its buildings). A record's word at +4 holds its kind
//! (bits 6-7: 0 free, 1 under way, 2 offered, 3 failed), its stage
//! (bits 9-11: here the sort of quest it is, 0..6), bit 8 (an errand)
//! and the sort of building its goal is (bits 12-15: 0 a tavern's list,
//! 1 a temple's, 3 an equipment store's, 2 the palace, 6 none, 7 a
//! person); +0 the place's seed it was made with, +6 the amount (people,
//! days, what is asked), +8 the time it began (a day's noon), +0Ch its
//! deadline, +10h who gives it, +14h its goal (a list's entry dword, the
//! index in its low word), +18h the reward in gold, +1Ch its item (FFh
//! none), +1Eh a seed for its people's names, +22h, +24h the place it
//! leads to and +25h the place it is given, +26h..+33h by sort.
//!
//! `quests_place` (9358) runs on entering a place: when it is not the
//! place last seen (FC8), the place's buildings are listed (1194:3EBF,
//! `QuestHooks::city_info`), the records not under way and not failed
//! are cleared, failed ones forgotten when their time is up (9AEE), and
//! the RNG is seeded from the PC's timer (0000:9D60). Outside the
//! Imperial Province, with a d20 below the player's level one quest
//! (QUESTS, 9462) is made, otherwise as many errands (OTHERS, 9D64) as
//! the town has taverns' entries left, four at most. The seed is put
//! back after.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::quest::var::{OTHERS, PLACES, QUESTS, TIME};
use crate::quest::QuestHooks;
use crate::random::CycleCheck;

/// DGROUP variables.
pub mod var {
    /// The place now (its building's door square, 1194:3E..), and the
    /// one the records were made for.
    pub const PLACE: u16 = 0xCCA8;
    pub const PLACE_SEEN: u16 = 0x0FC8;
    /// The record being made, and the seed they were made with.
    pub const CURRENT: u16 = 0x0F72;
    pub const SEED: u16 = 0xA836;
    /// The place (+24h of the location record, A22): its id (the high
    /// three bits the province), its province.
    pub const LOCATION: u16 = 0x0A4C;
    pub const PROVINCE: u16 = 0x0A46;
    /// The player's level (byte).
    pub const LEVEL: u16 = 0x01AA;
    /// The place's lists of buildings (far pointers to dwords; their
    /// counts): taverns', temples', equipment stores'; and 138D, the
    /// palace's word.
    pub const LIST_TAVERNS: u16 = 0x1AB5;
    pub const LIST_TEMPLES: u16 = 0x1008;
    pub const LIST_STORES: u16 = 0x1004;
    pub const COUNT_TAVERNS: u16 = 0x1387;
    pub const COUNT_TEMPLES: u16 = 0x138B;
    pub const COUNT_STORES: u16 = 0x1389;
    pub const PALACE: u16 = 0x138D;
    /// 2E69's inputs and result: the place's square, its kind; the
    /// province here; the days the journey takes.
    pub const TRAVEL_X: u16 = 0x193E;
    pub const TRAVEL_Y: u16 = 0x1940;
    pub const TRAVEL_KIND: u16 = 0x1944;
    pub const TRAVEL_FROM: u16 = 0x193D;
    pub const TRAVEL_DAYS: u16 = 0x1927;
}
use var::*;

/// A record's size: a quest's, an errand's.
const QUEST: u16 = 0x34;
const ERRAND: u16 = 0x29;
/// A day in minutes.
const DAY: u32 = 0x5A0;

/// DGROUP tables.
mod table {
    /// The quest makers by sort (words), the errand makers.
    pub const QUEST_MAKERS: u16 = 0x6CC4;
    pub const ERRAND_MAKERS: u16 = 0x6DCC;
    /// NUL-ended lists `pick` draws from.
    pub const AMOUNTS: u16 = 0x6CE2;
    pub const SIZES: u16 = 0x6CE8;
    /// (threshold, building sort) pairs over a d25.
    pub const GOALS: u16 = 0x6CF1;
    /// Levels, and the pay per day by level and a d3.
    pub const LEVELS: u16 = 0x6CFB;
    pub const PAY: u16 = 0x6CFF;
    /// (level, value) pairs; values by a d14.
    pub const RANKS: u16 = 0x6D0B;
    pub const ITEMS: u16 = 0x6D0C;
    /// The temples' kinds no quest goes to (7 bytes).
    pub const EXCLUDED: u16 = 0x6D67;
    /// d20 thresholds (3, or 5 for a size of 7) and what they give.
    pub const ODDS: u16 = 0x6DA4;
    pub const ODDS7: u16 = 0x6DA7;
    pub const SIZED: u16 = 0x6DB0;
}
use table::*;

impl Machine {
    fn sq8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }
    fn sq16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }
    fn sq32(&self, o: u16) -> u32 {
        self.mem.r32(DS, o)
    }

    /// 1194:55A3 / 55A7 (0000:6EEA from 0): [0, n). None where the
    /// original divides by zero.
    fn sq_roll(&mut self, n: u16) -> Option<u16> {
        self.rng.range(0, n)
    }

    /// 1194:5572: a d100, 0..99.
    fn sq_d100(&mut self) -> u16 {
        self.rng.next() % 100
    }

    /// 1194:9358 (far). The records made for the place now, if it isn't
    /// the one they were made for (see the module's notes). None where
    /// the original faults or never ends, or a hook fails.
    pub fn quests_place(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        let saved = self.rng.seed;
        self.quests_place_in(files, hooks)?;
        self.rng.seed = saved;
        Some(())
    }

    fn quests_place_in(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        if self.sq16(PLACE) == self.sq16(PLACE_SEEN) {
            return Some(());
        }
        let here = self.sq8(LOCATION);
        hooks.city_info(self, files, here)?;
        self.quests_clear();
        let place = self.sq16(PLACE);
        self.mem.w16(DS, PLACE_SEEN, place);
        let ticks = hooks.bios_ticks(self);
        self.rng.seed = ticks;
        self.mem.w32(DS, SEED, ticks);
        if self.sq8(PROVINCE) == 8 {
            return Some(());
        }
        let r = self.sq_roll(0x14)?;
        if (r as u8) < self.sq8(LEVEL) {
            if let Some(si) = self.sq_free(QUESTS, QUEST) {
                self.mem.w16(DS, CURRENT, si);
                self.sq_start(si)?;
                let seed = self.sq32(SEED);
                self.mem.w32(DS, si, seed);
                return self.quest_make(files, hooks, si);
            }
        }
        // 93CA: as many errands as taverns not already given one, four
        // with four or more.
        let n = self.sq16(COUNT_TAVERNS);
        let count = if n >= 4 {
            4
        } else {
            let mut bx = n;
            for i in 0..4 {
                if matches!(self.quest_kind(OTHERS + i * ERRAND), 1 | 2) {
                    bx = bx.wrapping_sub(1);
                }
            }
            bx
        };
        for _ in 0..count {
            let Some(si) = self.sq_free(OTHERS, ERRAND) else { break };
            self.mem.w16(DS, CURRENT, si);
            self.sq_start(si)?;
            let seed = self.sq32(SEED);
            self.mem.w32(DS, si, seed);
            self.errand_make(si)?;
        }
        Some(())
    }

    /// 1194:941F. The records neither under way nor failed cleared
    /// (943B: their kind word, then all their bytes; ES is taken to be
    /// DS), the failed quests whose time is up forgotten (9AEE), and F42
    /// cleared.
    fn quests_clear(&mut self) {
        for (base, size) in [(OTHERS, ERRAND), (QUESTS, QUEST)] {
            for i in 0..4 {
                let si = base + i * size;
                if matches!(self.quest_kind(si), 1 | 3) {
                    continue;
                }
                for k in 0..size {
                    self.mem.w8(DS, si + k, 0);
                }
            }
        }
        self.quests_forget();
        self.mem.w16(DS, 0x0F42, 0);
    }

    /// 1194:9AEE. Each entry of the failed quests' table (FAILED: its
    /// stage, its place, the time it is forgotten) whose time has come
    /// (TIME at or past it) clears the failed quest (kind 3) of that
    /// place and stage (9B26), if there is one. The table's entry stays.
    fn quests_forget(&mut self) {
        for i in 0..8u16 {
            let di = crate::quest::var::FAILED + i * 6;
            if self.sq32(TIME) < self.sq32(di + 2) {
                continue;
            }
            let stage = self.sq8(di) as u16;
            let place = self.sq8(di + 1);
            if let Some(si) = self.quest_failed_at(stage, place) {
                for k in 0..QUEST {
                    self.mem.w8(DS, si + k, 0);
                }
            }
        }
    }

    /// 1194:9B26. The failed quest (kind 3) given at `place` (+25h) of
    /// stage `stage` (FFFFh: any), the first of the four. Bug kept: a
    /// failed quest of the place with another stage leaves its stage in
    /// AX, so the records after it are compared with that stage.
    pub fn quest_failed_at(&self, stage: u16, place: u8) -> Option<u16> {
        let mut ax = stage;
        for i in 0..4 {
            let si = QUESTS + i * QUEST;
            if self.sq8(si + 0x25) != place || self.quest_kind(si) != 3 {
                continue;
            }
            let dx = ax;
            ax = self.quest_stage(si);
            if dx == 0xFFFF || ax == dx {
                return Some(si);
            }
        }
        None
    }

    /// 1194:9B7C. The first free record (kind 0) of the four from `si`.
    fn sq_free(&self, si: u16, size: u16) -> Option<u16> {
        (0..4).map(|i| si.wrapping_add(i * size)).find(|&si| self.quest_kind(si) == 0)
    }

    /// 1194:9B8F. The record begins at noon today: +8 the day's start
    /// (TIME less its part of a day) plus 2D0h. None where the original's
    /// 16-bit divide overflows (TIME past FFFFh days).
    fn sq_start(&mut self, si: u16) -> Option<()> {
        let days = self.sq32(TIME) / DAY;
        if days > 0xFFFF {
            return None;
        }
        self.mem.w32(DS, si + 8, (days * DAY).wrapping_add(0x2D0));
        Some(())
    }

    /// 1194:9B70. The record's stage (+4 bits 9-11).
    fn sq_stage_set(&mut self, si: u16, stage: u16) {
        let v = self.sq16(si + 4) & 0xF1FF | stage << 9;
        self.mem.w16(DS, si + 4, v);
    }

    /// The goal's sort (+4 bits 12-15).
    fn sq_goal_set(&mut self, si: u16, goal: u16) {
        let v = self.sq16(si + 4) & 0x0FFF | goal << 12;
        self.mem.w16(DS, si + 4, v);
    }

    /// 1194:9BD6. A place's size from its index in the province (the
    /// low five bits): 2 for 0..8 (the city-states, and the first town),
    /// 1 for 9..16, 0 after.
    fn sq_size(place: u8) -> u16 {
        match place % 0x20 {
            0..=8 => 2,
            9..=16 => 1,
            _ => 0,
        }
    }

    /// 1194:9BF4. The days the journey to place `place` takes: its
    /// province's data loaded (2E18), the place's square and kind (from
    /// its record among PLACES, by the low four bits of the id) and the
    /// province here set as 2E69's inputs, 2E69 run
    /// (`QuestHooks::travel`); its result at TRAVEL_DAYS.
    fn sq_days_to(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, place: u8) -> Option<u16> {
        hooks.province_load(self, place >> 5);
        let rec = PLACES + (place & 0xF) as u16 * 0x19;
        let x = self.sq16(rec + 0x14);
        self.mem.w16(DS, TRAVEL_X, x);
        let y = self.sq16(rec + 0x16);
        self.mem.w16(DS, TRAVEL_Y, y);
        let kind = self.sq8(rec + 0x18);
        self.mem.w8(DS, TRAVEL_KIND, kind);
        let p = self.sq8(PROVINCE);
        self.mem.w8(DS, TRAVEL_FROM, p);
        hooks.travel(self, files)?;
        Some(self.sq16(TRAVEL_DAYS))
    }

    /// 1194:9C30. The deadline (+0Ch): the journey's days to +24h
    /// (doubled when `twice`, there and back) plus the level + 1, in days
    /// from +8.
    fn sq_deadline(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16, twice: bool) -> Option<()> {
        let place = self.sq8(si + 0x24);
        let mut days = self.sq_days_to(files, hooks, place)?;
        if twice {
            days = days.wrapping_add(days);
        }
        days = days.wrapping_add(self.sq8(LEVEL) as u16 + 1);
        let t = (days as u32).wrapping_mul(DAY).wrapping_add(self.sq32(si + 8));
        self.mem.w32(DS, si + 0xC, t);
        Some(())
    }

    /// 1194:9C57. A random entry's index from the NUL-ended list at
    /// `list` (rising bytes): a roll below the last entry (the byte
    /// before the NUL), then the first entry at or above it.
    fn sq_pick(&mut self, list: u16) -> Option<u16> {
        let mut end = list;
        let mut n = 0u32;
        while self.sq8(end) != 0 {
            end = end.wrapping_add(1);
            n += 1;
            if n >= 0xFFFE {
                return None;
            }
        }
        // `repne scasb` leaves DI past the NUL.
        let last = self.sq8(end.wrapping_sub(1)) as u16;
        let r = self.sq_roll(last)? as u8;
        let mut cx = 0u16;
        while r > self.sq8(list.wrapping_add(cx)) {
            cx = cx.wrapping_add(1);
        }
        Some(cx)
    }

    /// 1194:0ABF. A random place of the province (its id, province
    /// times 20h plus a d32) that isn't the one here (compared in 8
    /// bits). None where the RNG never gives one.
    fn sq_other_place(&mut self) -> Option<u16> {
        let mut cycle = CycleCheck::new(&self.rng);
        loop {
            let r = self.sq_roll(0x20)?;
            let v = r.wrapping_add((self.sq8(PROVINCE) as u16) << 5);
            if v as u8 != self.sq8(LOCATION) {
                return Some(v);
            }
            if cycle.cycled(&self.rng) {
                return None;
            }
        }
    }

    /// 1194:9462. A quest made at `si`: offered (kind 2), no item, given
    /// here, to lead to the next place of the province by default, its
    /// names' seed from A52, no giver (AAAAAAAAh); a random sort but 2
    /// made by its maker (the table at QUEST_MAKERS), then 1404 10.
    fn quest_make(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<()> {
        self.mem.w16(DS, si + 4, 0x80);
        self.mem.w8(DS, si + 0x1C, 0xFF);
        let here = self.sq8(LOCATION);
        self.mem.w8(DS, si + 0x25, here);
        let next = here.wrapping_add(1) & 0x1F | self.sq8(PROVINCE) << 5;
        self.mem.w8(DS, si + 0x24, next);
        let seed = self.sq32(0x0A52);
        self.mem.w32(DS, si + 0x1E, seed);
        self.mem.w32(DS, si + 0x10, 0xAAAA_AAAA);
        let mut cycle = CycleCheck::new(&self.rng);
        let sort = loop {
            let r = self.sq_roll(7)?;
            if r != 2 {
                break r;
            }
            if cycle.cycled(&self.rng) {
                return None;
            }
        };
        match self.sq16(QUEST_MAKERS + sort * 2) {
            0x94B7 => self.quest_make_find(files, hooks, si, 0, false)?,
            0x98CD => self.quest_make_find(files, hooks, si, 1, true)?,
            0x96FE => self.quest_make_escort(files, hooks, si)?,
            0x97F1 => self.quest_make_fetch(files, hooks, si)?,
            0x9786 => self.quest_make_rescue(files, hooks, si)?,
            0x9849 => self.quest_make_guard(files, hooks, si)?,
            _ => return None,
        }
        self.mem.w8(DS, 0x1404, 0xA);
        Some(())
    }

    /// 1194:94B7 (stage 0) and 98CD (stage 1, the deadline counting the
    /// way back): a size (94D9), a goal in another town (9511), an amount
    /// (9683), the deadline, +22h (9692) and the reward (96B2).
    fn quest_make_find(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16, stage: u16, back: bool) -> Option<()> {
        self.sq_stage_set(si, stage);
        self.quest_size(si)?;
        self.quest_goal(files, hooks, si)?;
        let amount = self.sq_pick(AMOUNTS)?;
        self.mem.w16(DS, si + 6, amount);
        self.sq_deadline(files, hooks, si, back)?;
        self.quest_sized(si)?;
        self.quest_reward(si)
    }

    /// 1194:96FE (stage 3): the pay (9719), a size and goal (94D2), the
    /// deadline, +22h, the pay for the days (9760).
    fn quest_make_escort(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<()> {
        self.sq_stage_set(si, 3);
        self.quest_pay(si)?;
        self.quest_size(si)?;
        self.quest_goal(files, hooks, si)?;
        self.sq_deadline(files, hooks, si, false)?;
        self.quest_sized(si)?;
        self.quest_daily(si)
    }

    /// 1194:97F1 (stage 4): an item's number (9815), a person (97C0), the
    /// deadline there and back, a size, the reward by the place's size
    /// (9825), an item (97AD), then +24h the place here.
    fn quest_make_fetch(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<()> {
        self.sq_stage_set(si, 4);
        let r = self.sq_roll(0x12)? as u8;
        self.mem.w8(DS, si + 0x2F, r.wrapping_add(0x18));
        self.quest_whom(si)?;
        self.sq_deadline(files, hooks, si, true)?;
        self.quest_size(si)?;
        let size = Self::sq_size(self.sq8(si + 0x25)) as u32;
        let level = self.sq8(LEVEL).wrapping_add(1) as u32;
        self.mem.w32(DS, si + 0x18, (size + 3) * 10 * level);
        self.quest_item(si)?;
        let here = self.sq8(LOCATION);
        self.mem.w8(DS, si + 0x24, here);
        Some(())
    }

    /// 1194:9786 (stage 5): the pay, a person, a size, the deadline
    /// there and back, +22h, an item, the pay for the days, then +24h the
    /// place here.
    fn quest_make_rescue(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<()> {
        self.sq_stage_set(si, 5);
        self.quest_pay(si)?;
        self.quest_whom(si)?;
        self.quest_size(si)?;
        self.sq_deadline(files, hooks, si, true)?;
        self.quest_sized(si)?;
        self.quest_item(si)?;
        self.quest_daily(si)?;
        let here = self.sq8(LOCATION);
        self.mem.w8(DS, si + 0x24, here);
        Some(())
    }

    /// 1194:9849 (stage 6): a rank by level (9867), the goal a person of
    /// the second kind (9895), the deadline, the reward by the place's
    /// size (98A9), then +24h the place here.
    fn quest_make_guard(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<()> {
        self.sq_stage_set(si, 6);
        self.quest_rank(si);
        self.sq_goal_set(si, 7);
        self.mem.w16(DS, si + 0x32, 2);
        self.sq_deadline(files, hooks, si, false)?;
        let size = Self::sq_size(self.sq8(si + 0x25)) as u32;
        let level = (self.sq8(LEVEL) as u32) + 1;
        self.mem.w32(DS, si + 0x18, (size * 25 + 50).wrapping_mul(level) << 1);
        let here = self.sq8(LOCATION);
        self.mem.w8(DS, si + 0x24, here);
        Some(())
    }

    /// 1194:94D9. The size (+2Eh), an index from SIZES.
    fn quest_size(&mut self, si: u16) -> Option<()> {
        let i = self.sq_pick(SIZES)?;
        self.mem.w8(DS, si + 0x2E, i as u8);
        Some(())
    }

    /// 1194:9511. The goal in another place of the province (0ABF, to
    /// +24h): its buildings listed (3EC3, `QuestHooks::city_info`) into a
    /// block of 280h bytes from the heap (taverns' at 0, stores' at 100h,
    /// temples' at 180h), the lists here kept and put back after. A d100
    /// against five times the level + 1 (in 8 bits) sets +29h (1 at or
    /// under it); a d25 over the pairs at GOALS gives the goal's sort
    /// (+4 bits 12-15): for a tavern, store or temple a random entry of
    /// its list (a temple's not of a kind in EXCLUDED, unless all are),
    /// +14h its dword with its index in the low word; for the palace (2)
    /// or none (6) +0's high word and 1. The block is freed. None where
    /// the heap has no room (see below).
    fn quest_goal(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<()> {
        let place = self.sq_other_place()?;
        self.mem.w8(DS, si + 0x24, place as u8);
        let words = [COUNT_TEMPLES, COUNT_TAVERNS, COUNT_STORES, PALACE, 0x138F].map(|o| (o, self.sq16(o)));
        let dwords = [LIST_TEMPLES, LIST_TAVERNS, LIST_STORES].map(|o| (o, self.sq32(o)));
        // With no room the original's EAX keeps a high word left from
        // earlier code: not followed (None).
        let seg = self.heap_alloc_high(0x280)?;
        if seg == 0 {
            return None;
        }
        let block = (seg as u32) << 16;
        self.mem.w32(DS, LIST_TAVERNS, block);
        self.mem.w32(DS, LIST_STORES, block | 0x100);
        self.mem.w32(DS, LIST_TEMPLES, block | 0x180);
        let to = self.sq8(si + 0x24);
        hooks.city_info(self, files, to)?;
        let d = self.sq_d100();
        let limit = (self.sq8(LEVEL).wrapping_add(1) as u16) * 5;
        let near = ((d as i16) <= (limit as i16)) as u8;
        let r = self.sq_roll(0x19)? as u8;
        let mut di = GOALS;
        while r > self.sq8(di) {
            di = di.wrapping_add(2);
        }
        let goal = self.sq8(di.wrapping_add(1)) as u16;
        self.sq_goal_set(si, goal);
        self.mem.w8(DS, si + 0x29, near);
        let list = match goal {
            0 => Some((self.sq16(COUNT_TAVERNS), self.sq32(LIST_TAVERNS))),
            3 => Some((self.sq16(COUNT_STORES), self.sq32(LIST_STORES))),
            1 => Some((self.sq16(COUNT_TEMPLES), self.sq32(LIST_TEMPLES))),
            2 | 6 => None,
            // The table gives no other sort; the original would draw from
            // whatever GS:DI held.
            _ => return None,
        };
        match list {
            None => {
                let v = self.sq32(si) & 0xFFFF_0000 | 1;
                self.mem.w32(DS, si + 0x14, v);
            }
            Some((count, far)) => {
                let (seg, off) = ((far >> 16) as u16, far as u16);
                let excluded = |m: &Machine, i: u16| {
                    let kind = m.mem.r8(seg, off.wrapping_add(i.wrapping_mul(4)).wrapping_add(2));
                    (0..7).any(|k| m.sq8(EXCLUDED + k) == kind)
                };
                // A temple's list: whether any entry may be drawn (`loop`
                // over the count, 65536 for 0).
                let check = goal == 1 && {
                    let n = if count == 0 { 0x10000 } else { count as u32 };
                    (0..n).any(|i| !excluded(self, i as u16))
                };
                let mut cycle = CycleCheck::new(&self.rng);
                let i = loop {
                    let i = self.sq_roll(count)?;
                    if !check || !excluded(self, i) {
                        break i;
                    }
                    if cycle.cycled(&self.rng) {
                        return None;
                    }
                };
                let v = self.mem.r32(seg, off.wrapping_add(i.wrapping_mul(4))) & 0xFFFF_0000 | i as u32;
                self.mem.w32(DS, si + 0x14, v);
            }
        }
        let block = self.sq16(LIST_TAVERNS + 2);
        self.heap_free(block);
        for (o, v) in words {
            self.mem.w16(DS, o, v);
        }
        for (o, v) in dwords {
            self.mem.w32(DS, o, v);
        }
        Some(())
    }

    /// 1194:9692. +22h from SIZED by the size (+2Eh) times three plus a
    /// d20's step (94E8).
    fn quest_sized(&mut self, si: u16) -> Option<()> {
        let size = self.sq8(si + 0x2E) as u16;
        let step = self.quest_odds(size)?;
        let v = self.sq8(SIZED.wrapping_add(size.wrapping_mul(3)).wrapping_add(step));
        self.mem.w16(DS, si + 0x22, v as u16);
        Some(())
    }

    /// 1194:94E8 (far). The first of the thresholds at ODDS (three; five
    /// at ODDS7 for size 7) a d20 is below, 0 for none.
    fn quest_odds(&mut self, size: u16) -> Option<u16> {
        let r = self.sq_roll(0x14)? as u8;
        let (table, n) = if size == 7 { (ODDS7, 5) } else { (ODDS, 3) };
        Some((0..n).find(|&i| r < self.sq8(table + i)).unwrap_or(0))
    }

    /// 1194:96B2. The reward (+18h): a roll from (amount + 1) x 100 to
    /// that plus (amount + 2) x 25 (amounts in 8 bits), times the level
    /// + 1 in 16 bits, times 140h (E6h, C0h for A47 1, other), over 256.
    fn quest_reward(&mut self, si: u16) -> Option<()> {
        let amount = self.sq16(si + 6);
        let lo = (amount.wrapping_add(1) as u8 as u16).wrapping_mul(100);
        let hi = (amount.wrapping_add(2) as u8 as u16).wrapping_mul(25).wrapping_add(lo).wrapping_add(1);
        let r = self.rng.range(lo, hi)?;
        let ax = r.wrapping_mul(self.sq8(LEVEL) as u16 + 1);
        let cx = match self.sq8(0x0A47) {
            0 => 0x140,
            1 => 0xE6,
            _ => 0xC0,
        };
        let v = ((ax as u32 * cx) >> 8) as u16;
        self.mem.w32(DS, si + 0x18, v as u32);
        Some(())
    }

    /// 1194:9719. A d100 of 30 or more sets +28h; the level's row of
    /// LEVELS (the first it reaches) and a d3 pick the pay per day (+26h,
    /// from PAY); +6 and +27h the pick doubled plus +28h (in 8 bits).
    fn quest_pay(&mut self, si: u16) -> Option<()> {
        let d = self.sq_d100();
        let flag = (d >= 0x1E) as u8;
        self.mem.w8(DS, si + 0x28, flag);
        let level = self.sq8(LEVEL);
        let mut row = 0u16;
        while level < self.sq8(LEVELS + row) {
            row += 1;
        }
        let r = self.sq_roll(3)?;
        let bx = row.wrapping_mul(3).wrapping_add(r);
        let pay = self.sq8(PAY.wrapping_add(bx));
        self.mem.w8(DS, si + 0x26, pay);
        let al = (bx as u8).wrapping_add(bx as u8).wrapping_add(flag);
        self.mem.w16(DS, si + 6, al as u16 | (bx.wrapping_add(bx) & 0xFF00));
        self.mem.w8(DS, si + 0x27, al);
        Some(())
    }

    /// 1194:9760. The reward (+18h): the days from +8 to the deadline
    /// times the pay per day (+26h) times the level + 1. None where the
    /// days overflow the original's 16-bit divide.
    fn quest_daily(&mut self, si: u16) -> Option<()> {
        let span = self.sq32(si + 0xC).wrapping_sub(self.sq32(si + 8));
        let days = span / DAY;
        if days > 0xFFFF {
            return None;
        }
        let level = (self.sq8(LEVEL) as u32) + 1;
        let v = days.wrapping_mul(self.sq8(si + 0x26) as u32).wrapping_mul(level);
        self.mem.w32(DS, si + 0x18, v);
        Some(())
    }

    /// 1194:97C0. The goal a person (+4 bits 12-15 7), +31h a d8, +32h 1
    /// unless bit 1 of the seed is set (then 0).
    fn quest_whom(&mut self, si: u16) -> Option<()> {
        self.sq_goal_set(si, 7);
        let r = self.sq_roll(8)?;
        self.mem.w8(DS, si + 0x31, r as u8);
        let v = (self.rng.seed & 2 == 0) as u16;
        self.mem.w16(DS, si + 0x32, v);
        Some(())
    }

    /// 1194:97AD. +30h an item from ITEMS by a d14 (every other byte).
    fn quest_item(&mut self, si: u16) -> Option<()> {
        let r = self.sq_roll(0xE)?;
        let v = self.sq8(ITEMS + r * 2);
        self.mem.w8(DS, si + 0x30, v);
        Some(())
    }

    /// 1194:9867. +30h from the pairs at RANKS: the level + 1 (in 8
    /// bits, 13h from 14h on, compared signed) against each pair's first
    /// byte until it is no more; on a tie the next pair when the seed's
    /// low byte has odd parity.
    fn quest_rank(&mut self, si: u16) {
        let mut al = self.sq8(LEVEL).wrapping_add(1) as i8;
        if al >= 0x14 {
            al = 0x13;
        }
        let mut di = RANKS;
        while al > self.sq8(di) as i8 {
            di = di.wrapping_add(2);
        }
        if al == self.sq8(di) as i8 && (self.rng.seed as u8).count_ones() % 2 == 1 {
            di = di.wrapping_add(2);
        }
        let v = self.sq8(di.wrapping_add(1));
        self.mem.w8(DS, si + 0x30, v);
    }

    /// 1194:9D64. An errand made at `si`: offered (kind 2) with bit 8,
    /// no item; its giver (+10h) a random entry of the taverns' list no
    /// errand has (the record's own +10h cleared first, so not 0); given
    /// and leading here; a random sort but 2 made by its maker (the
    /// table at ERRAND_MAKERS).
    fn errand_make(&mut self, si: u16) -> Option<()> {
        self.mem.w16(DS, si + 4, 0x180);
        self.mem.w8(DS, si + 0x1C, 0xFF);
        self.mem.w32(DS, si + 0x10, 0);
        let count = self.sq16(COUNT_TAVERNS);
        let far = self.sq32(LIST_TAVERNS);
        let entry = |m: &Machine, r: u16| m.mem.r32((far >> 16) as u16, (far as u16).wrapping_add(r.wrapping_mul(4)));
        let taken = |m: &Machine, v: u32| (0..4).any(|i| m.sq32(OTHERS + i * ERRAND + 0x10) == v);
        let mut cycle = CycleCheck::new(&self.rng);
        let giver = loop {
            let r = self.sq_roll(count)?;
            let v = entry(self, r);
            if !taken(self, v) {
                break v;
            }
            // Every entry taken: the original draws for ever.
            if (0..count).all(|r| taken(self, entry(self, r))) || cycle.cycled(&self.rng) {
                return None;
            }
        };
        self.mem.w32(DS, si + 0x10, giver);
        let here = self.sq8(LOCATION);
        self.mem.w8(DS, si + 0x25, here);
        self.mem.w8(DS, si + 0x24, here);
        let mut cycle = CycleCheck::new(&self.rng);
        let sort = loop {
            let r = self.sq_roll(4)?;
            if r != 2 {
                break r;
            }
            if cycle.cycled(&self.rng) {
                return None;
            }
        };
        match self.sq16(ERRAND_MAKERS + sort * 2) {
            0x9DCF => self.errand_make_plain(si, 0),
            0x9EAB => self.errand_make_plain(si, 1),
            0x9EC4 => self.errand_make_escort(si),
            _ => None,
        }
    }

    /// 1194:9DCF (stage 0) and 9EAB (stage 1): the names' seed (9E61), a
    /// goal in town (9DE8), an amount (9E7F), a day's deadline (9E52), a
    /// reward (9E8D).
    fn errand_make_plain(&mut self, si: u16, stage: u16) -> Option<()> {
        self.sq_stage_set(si, stage);
        self.errand_seed(si);
        self.errand_goal(si)?;
        let r = self.sq_roll(0xA)?;
        self.mem.w16(DS, si + 6, r);
        self.errand_deadline(si);
        self.errand_reward(si)
    }

    /// 1194:9EC4 (stage 3): the names' seed, the escort's size (9EDD), a
    /// goal in town, a day's deadline, a reward.
    fn errand_make_escort(&mut self, si: u16) -> Option<()> {
        self.sq_stage_set(si, 3);
        self.errand_seed(si);
        let d = self.sq_d100();
        let flag = (d <= 0x46) as u16;
        self.mem.w8(DS, si + 0x28, flag as u8);
        let r = self.sq_roll(0xC)?;
        let v = r.wrapping_add(r).wrapping_add(flag);
        self.mem.w16(DS, si + 6, v);
        self.mem.w8(DS, si + 0x27, v as u8);
        self.mem.w8(DS, si + 0x26, 0);
        self.errand_goal(si)?;
        self.errand_deadline(si);
        self.errand_reward(si)
    }

    /// 1194:9E61. +1Eh: the seed (after a d100) shifted left twice, bit 0
    /// set when the d100 was 40 or under.
    fn errand_seed(&mut self, si: u16) {
        let d = self.sq_d100();
        let v = self.rng.seed << 2 | (d <= 0x28) as u32;
        self.mem.w32(DS, si + 0x1E, v);
    }

    /// 1194:9DE8. The goal: a random sort (+4 bits 12-15) of 0..3; the
    /// palace (2) gives +14h the word at PALACE, the others a random
    /// entry of their list, all drawn again when it is the giver (+10h).
    fn errand_goal(&mut self, si: u16) -> Option<()> {
        let mut cycle = CycleCheck::new(&self.rng);
        loop {
            let goal = self.sq_roll(4)?;
            self.sq_goal_set(si, goal);
            let (count, far) = match goal {
                3 => (self.sq16(COUNT_STORES), self.sq32(LIST_STORES)),
                1 => (self.sq16(COUNT_TEMPLES), self.sq32(LIST_TEMPLES)),
                0 => (self.sq16(COUNT_TAVERNS), self.sq32(LIST_TAVERNS)),
                _ => {
                    let v = self.sq16(PALACE) as u32;
                    self.mem.w32(DS, si + 0x14, v);
                    return Some(());
                }
            };
            let r = self.sq_roll(count)?;
            let v = self.mem.r32((far >> 16) as u16, (far as u16).wrapping_add(r.wrapping_mul(4)));
            if self.sq32(si + 0x10) != v {
                self.mem.w32(DS, si + 0x14, v);
                return Some(());
            }
            if cycle.cycled(&self.rng) {
                return None;
            }
        }
    }

    /// 1194:9E52. The deadline a day after +8.
    fn errand_deadline(&mut self, si: u16) {
        let t = self.sq32(si + 8).wrapping_add(DAY);
        self.mem.w32(DS, si + 0xC, t);
    }

    /// 1194:9E8D. The reward: a roll of 15..20 times the level + 1 (in 8
    /// bits).
    fn errand_reward(&mut self, si: u16) -> Option<()> {
        let r = self.rng.range(0xF, 0x15)? as u32;
        let level = self.sq8(LEVEL).wrapping_add(1) as u32;
        self.mem.w32(DS, si + 0x18, r * level);
        Some(())
    }
}
