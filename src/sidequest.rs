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
//!
//! `quests_talk` (8F1E) runs when the player talks to someone who may
//! give or end a quest (from the talk, the shops and the palace): offers
//! for the person (errands whose giver it is, 8F50; in a palace the
//! first quest offered, 8F7B) and the quests under way it ends (8F99:
//! a goal reached turns the quest back to its giver; at the giver, or
//! for stages 0 and 3 at once, it is done, 9041: gold, experience, its
//! item freed) are queued as texts (QUEUE, 9D2C), then one is taken
//! (90B9): a failed quest scolded in a palace, a message, or the offer
//! itself, answered (yes: under way and in the log; no: forgotten).

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
    /// The texts queued (byte text, word record, 4 bytes each; negative
    /// texts are messages) and their count; the offer's text and record,
    /// its goal's sort.
    pub const QUEUE: u16 = 0x6CA2;
    pub const QUEUED: u16 = 0x0AB0;
    pub const OFFER_TEXT: u16 = 0x1252;
    pub const OFFER: u16 = 0x1250;
    pub const OFFER_GOAL: u16 = 0x1254;
    /// The person talked to (its word, the low byte doubled), the
    /// building's kind (1 a palace), the area's (4 a building), the talk's
    /// (4 a tavern's keeper).
    pub const TALKER: u16 = 0x0FD2;
    pub const BUILDING: u16 = 0x0D48;
    pub const AREA: u16 = 0x05E6;
    pub const TALK: u16 = 0x137F;
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

    /// The person talked to, as the records name a giver or goal: FD2's
    /// word with its low byte halved.
    fn sq_talker(&self) -> u16 {
        let w = self.sq16(TALKER);
        w & 0xFF00 | (w as u8 >> 1) as u16
    }

    /// 1194:9D2C. Text `ax` (its low byte) for record `si` queued.
    fn quest_queue(&mut self, ax: u16, si: u16) {
        let bx = self.sq16(QUEUED).wrapping_mul(4);
        self.mem.w16(DS, QUEUE.wrapping_add(bx), ax);
        self.mem.w16(DS, QUEUE.wrapping_add(bx).wrapping_add(1), si);
        let n = self.sq16(QUEUED).wrapping_add(1);
        self.mem.w16(DS, QUEUED, n);
    }

    /// 1194:8F1E. A talk's quests (see the module's notes). None where
    /// the original faults or never ends, or a hook fails.
    pub fn quests_talk(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        self.mem.w16(DS, QUEUED, 0);
        if self.sq16(0x05C8) & 0x4000 != 0 {
            return Some(());
        }
        if self.sq8(TALK) == 4 {
            // 8F50: the errands offered whose giver this is.
            for i in 0..4 {
                let si = OTHERS + i * ERRAND;
                if self.quest_kind(si) == 2 && self.sq_talker() == self.sq16(si + 0x10) {
                    self.mem.w16(DS, CURRENT, si);
                    self.quest_queue(self.quest_stage(si) + 0x57, si);
                }
            }
        } else if self.sq8(AREA) == 4 && self.sq8(BUILDING) == 1 {
            // 8F7B: the first quest offered, in a palace.
            if let Some(si) = (0..4).map(|i| QUESTS + i * QUEST).find(|&si| self.quest_kind(si) == 2) {
                self.quest_queue(self.quest_stage(si) + 5, si);
            }
        }
        self.quests_progress();
        self.quests_take(files, hooks)
    }

    /// 1194:8F99. The quests under way (kind 1; the errands first) whose
    /// place (+24h) is here and whose person (+10h once the goal is
    /// reached, +14h's low word before; in a palace AAAAh) is the one
    /// talked to: stage 1 not yet there marks the goal reached and turns
    /// back to the giver's place (text 2Eh, 5Dh for an errand); stages 0
    /// and 3, and the others with the goal reached, are done (9041). The
    /// texts are queued as messages (low byte negated).
    fn quests_progress(&mut self) {
        for (base, size) in [(OTHERS, ERRAND), (QUESTS, QUEST)] {
            for i in 0..4 {
                let si = base + i * size;
                if self.quest_kind(si) != 1 || self.sq8(LOCATION) != self.sq8(si + 0x24) {
                    continue;
                }
                let who = if self.sq8(BUILDING) == 1 { 0xAAAA } else { self.sq_talker() };
                let f = self.sq16(si + 4);
                let person = if f & 0x20 != 0 { self.sq16(si + 0x10) } else { self.sq16(si + 0x14) };
                if who != person {
                    continue;
                }
                let stage = self.quest_stage(si);
                let text = if stage == 1 && f & 0x20 == 0 {
                    self.mem.w16(DS, si + 4, f | 0x20);
                    let back = self.sq8(si + 0x25);
                    self.mem.w8(DS, si + 0x24, back);
                    if f & 0x100 != 0 { 0x5D } else { 0x2E }
                } else if stage == 1 || stage == 3 || stage == 0 || f & 0x20 != 0 {
                    self.quest_done(si, stage)
                } else {
                    continue;
                };
                self.quest_queue(text & 0xFF00 | (text as u8).wrapping_neg() as u16, si);
            }
        }
    }

    /// 1194:9041. Quest `si` (of stage `stage`) done: its goal marked,
    /// its reward (+18h) to the gold (5C2), a quest (not an errand)
    /// copied to F3E with 8A56 set; its item freed for stages 3..5; the
    /// record freed (kind 0); experience (5AD) the level + 1 times 1F4h
    /// (32h for an errand), at least C8h; 5CA bit 100h. Returns the
    /// text, from 6D99 by the stage (+7 for an errand).
    fn quest_done(&mut self, si: u16, stage: u16) -> u16 {
        let f = self.sq16(si + 4) | 0x20;
        self.mem.w16(DS, si + 4, f);
        let gold = self.sq32(0x05C2).wrapping_add(self.sq32(si + 0x18));
        self.mem.w32(DS, 0x05C2, gold);
        if f & 0x100 == 0 {
            for k in 0..QUEST {
                let b = self.sq8(si.wrapping_add(k));
                self.mem.w8(DS, 0x0F3E + k, b);
            }
            self.mem.w8(DS, 0x8A56, 1);
        }
        let (bx, cx) = if f & 0x100 != 0 { (stage + 7, 0x32u16) } else { (stage, 0x1F4) };
        let text = self.sq8(0x6D99 + bx) as u16;
        if (3..=5).contains(&stage) {
            self.quest_item_free(si);
        }
        let k = self.sq16(si + 4) & 0xFF3F;
        self.mem.w16(DS, si + 4, k);
        let ax = (self.sq8(LEVEL) as u16 + 1).wrapping_mul(cx).max(0xC8);
        let xp = self.sq32(0x05AD).wrapping_add(ax as u32);
        self.mem.w32(DS, 0x05AD, xp);
        let v = self.sq16(0x05CA) | 0x100;
        self.mem.w16(DS, 0x05CA, v);
        text
    }

    /// 1194:90B9. One of the queued texts taken (see the module's
    /// notes), 8A56 kept meanwhile.
    pub fn quests_take(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        let saved = self.sq8(0x8A56);
        self.mem.w8(DS, 0x8A56, 0);
        self.quests_take_in(files, hooks)?;
        self.mem.w8(DS, 0x8A56, saved);
        Some(())
    }

    fn quests_take_in(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        if self.sq8(BUILDING) == 1 {
            if let Some(si) = self.quest_failed_at(0xFFFF, self.sq8(LOCATION)) {
                // A palace's failed quest: its scolding, in its giver's
                // words (the names' seed meanwhile).
                self.mem.w16(DS, CURRENT, si);
                let saved = self.rng.seed;
                self.rng.seed = self.sq32(si + 0x1E);
                self.quest_message(hooks, self.quest_stage(si) + 0x10, 0);
                self.rng.seed = saved;
                return Some(());
            }
        }
        let q = self.sq16(QUEUED).wrapping_sub(1);
        self.mem.w16(DS, QUEUED, q);
        if (q as i16) < 0 {
            if self.sq8(AREA) == 4 && self.sq8(BUILDING) == 1 {
                self.quest_message(hooks, 0xD, 0);
            }
            return Some(());
        }
        // 9D40.
        let bx = q.wrapping_mul(4);
        let ax = self.sq8(QUEUE.wrapping_add(bx)) as u16;
        let si = self.sq16(QUEUE.wrapping_add(bx).wrapping_add(1));
        self.mem.w16(DS, CURRENT, si);
        if (ax as u8 as i8) < 0 {
            self.quest_message(hooks, (ax as u8).wrapping_neg() as u16, 0);
            return Some(());
        }
        self.mem.w16(DS, OFFER_TEXT, ax);
        self.mem.w16(DS, OFFER, si);
        if self.sq8(TALK) != 4 && self.sq8(AREA) == 4 {
            if self.sq8(BUILDING) != 1 {
                return Some(());
            }
            if self.quest_ask(hooks, 0xE) != 0 {
                self.quest_message(hooks, 0xF, 0);
                return Some(());
            }
        } else {
            self.quest_message(hooks, 0x56, 0);
        }
        self.quest_offer_made(files, hooks)
    }

    /// 1194:9CD8. Text `text` written (A374, mode 2; width 28h with 5C8
    /// bit 20h) and offered with its yes and no (1194:020E, the keys at
    /// 6D6E, the entries at 6D71). The pick.
    fn quest_ask(&mut self, hooks: &mut dyn QuestHooks, text: u16) -> u16 {
        use crate::quest::var::{TEXT, TEXT_COLOUR, TEXT_FLAG, TEXT_FLAG2};
        self.mem.w16(DS, TEXT, text);
        self.mem.w8(DS, TEXT_FLAG, 0);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        let wide = self.sq16(0x05C8) & 0x20 != 0;
        if wide {
            self.mem.w8(DS, TEXT_FLAG, 0x28);
        }
        hooks.text(self);
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        let ax = self.sq16(0x102C);
        hooks.font(self, 0, ax);
        if wide {
            let ax = self.sq16(0x1038);
            hooks.font(self, 1, ax);
        }
        hooks.menu(self, 0x1044, 0x6D6E, 0x6D71)
    }

    /// 1194:9162..9321. The offer of record CURRENT: the way to its place
    /// (8A52, 8A54) when not an errand; for a person to find (goal 7) its
    /// name (the +32h-th of the strings at 6D27, its substitutions made,
    /// at 6D49) given to a random dungeon of the province (26F7:0C0A,
    /// mode 1), the journey there, and the deadline again: the days twice
    /// unless the stage is 6, plus the level + 1. Bug kept: that stage is
    /// read through SI as 26F7:0C0A leaves it, the name's buffer, so from
    /// the name's fifth and sixth characters. The offer's text
    /// (OFFER_TEXT) is then asked (A374 mode 1, the player put at the
    /// place's square meanwhile, 9D51): no frees the record (text 5Bh, Fh
    /// in a palace); yes gives a stage 3 quest its item (1194:3829, bit
    /// 80h with +28h), takes the dungeon (mode 4, its index to +14h),
    /// shows the acceptance (56Dh, 56Ch with a dungeon, 56Fh for an
    /// errand), logs the quest (stage + 27h, + 4D6h for an errand) and
    /// puts it under way.
    fn quest_offer_made(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        use crate::quest::var::{ITEMS, TEXT, TEXT_FLAG, TEXT_FLAG2};
        let si = self.sq16(CURRENT);
        if self.sq16(si + 4) & 0x100 == 0 {
            let here = self.sq8(PROVINCE);
            let to = self.sq8(si + 0x24);
            hooks.province_load(self, to >> 5);
            let rec = PLACES + (to & 0x1F) as u16 * 0x19;
            let (x, y) = (self.sq16(rec + 0x14), self.sq16(rec + 0x16));
            self.mem.w16(DS, 0x8A52, x);
            self.mem.w16(DS, 0x8A54, y);
            hooks.province_load(self, here);
        }
        let goal = (self.sq16(si + 4) & 0xF000) >> 12;
        self.mem.w16(DS, OFFER_GOAL, goal);
        if goal == 7 {
            let n = self.sq16(si + 0x32);
            let name = self.skip_strings(DS, 0x6D27, n);
            self.quest_strcpy(name, 0x6D49);
            hooks.substitute(self, files, 0x6D49)?;
            let province = self.sq8(PROVINCE) as u16;
            let ax = self.quest_dungeon_entry(hooks, province, 1, 0, 0x6D49)?;
            let (bx, cx) = (self.sq16(0x129A), self.sq16(0x129C));
            self.mem.w16(DS, 0x8A52, bx);
            self.mem.w16(DS, 0x8A54, cx);
            self.mem.w16(DS, 0x126C, ax);
            self.mem.w16(DS, TRAVEL_X, bx);
            self.mem.w16(DS, TRAVEL_Y, cx);
            hooks.travel(self, files)?;
            let mut days = self.sq16(TRAVEL_DAYS);
            if self.quest_stage(0x6D49) != 6 {
                days = days.wrapping_add(days);
            }
            let days = (self.sq8(LEVEL) as u16 + 1).wrapping_add(days);
            let si = self.sq16(CURRENT);
            let t = (days as u32).wrapping_mul(DAY).wrapping_add(self.sq32(si + 8));
            self.mem.w32(DS, si + 0xC, t);
        }
        self.mem.w8(DS, TEXT_FLAG, 0);
        let text = self.sq16(OFFER_TEXT);
        self.mem.w16(DS, TEXT, text);
        let answer = self.sq_at_place(|m| {
            m.mem.w8(DS, TEXT_FLAG, 0);
            m.mem.w8(DS, TEXT_FLAG2, 0);
            hooks.text_at(m, files, 1, 0x1044)
        })?;
        if answer != 0 {
            let si = self.sq16(CURRENT);
            let k = self.sq16(si + 4) & 0xFF3F;
            self.mem.w16(DS, si + 4, k);
            let text = if self.sq8(AREA) == 4 && self.sq8(BUILDING) == 1 { 0xF } else { 0x5B };
            self.quest_message(hooks, text, 0);
            return Some(());
        }
        self.mem.w8(DS, TEXT_FLAG, 0);
        let si = self.sq16(CURRENT);
        if self.quest_stage(si) == 3 {
            let (item, at) = self.pick_quest_item(self.sq8(PROVINCE) as u16)?;
            let mut al = item as u8;
            if self.sq8(si + 0x28) != 0 {
                al |= 0x80;
            }
            self.mem.w8(DS, ITEMS.wrapping_add(at), al);
            self.mem.w8(DS, si + 0x1C, al);
            let d = self.sq8(0x0D39) | 0x80;
            self.mem.w8(DS, 0x0D39, d);
        }
        self.mem.w16(DS, TEXT, 0x56D);
        if self.sq16(OFFER_GOAL) == 7 {
            let province = self.sq8(PROVINCE) as u16;
            let index = self.sq16(0x126C);
            let ax = self.quest_dungeon_entry(hooks, province, 4, index, 0x6D49)?;
            let (bx, cx) = (self.sq16(0x129A), self.sq16(0x129C));
            self.mem.w16(DS, 0x8A52, bx);
            self.mem.w16(DS, 0x8A54, cx);
            let si = self.sq16(CURRENT);
            self.mem.w32(DS, si + 0x14, ax as u32);
            self.mem.w16(DS, TEXT, 0x56C);
        }
        self.mem.w8(DS, TEXT_FLAG, 0);
        let si = self.sq16(CURRENT);
        if self.sq16(si + 4) & 0x100 != 0 {
            self.mem.w16(DS, TEXT, 0x56F);
        }
        hooks.text_shown(self);
        let si = self.sq16(CURRENT);
        let mut t = self.quest_stage(si) + 0x27;
        if self.sq16(si + 4) & 0x100 != 0 {
            t = t.wrapping_add(0x4D6);
        }
        self.mem.w16(DS, TEXT, t);
        self.sq_at_place(|m| {
            m.mem.w8(DS, TEXT_FLAG2, 0);
            hooks.text_at(m, files, 2, 0x929E)?;
            hooks.log_entry(m, 0x929E);
            Some(())
        })?;
        let si = self.sq16(CURRENT);
        let k = self.sq16(si + 4) & 0xFF3F | 0x40;
        self.mem.w16(DS, si + 4, k);
        Some(())
    }

    /// 1194:CED7. The main quest's informant (0FDE the person, 0FE6 bit 2
    /// the informant met, bit 4 told, bit 1 paid already): when it is the
    /// person talked to (and the player isn't wanted, 5C8 bit 4000h),
    /// with days left (0FED less the days since 0FE9) a deal is made: paid
    /// for (bit 1: D019) or haggled over (8020 in mode 5, 6 once met,
    /// `QuestHooks::informant_haggle`); a deal tells the next dungeon
    /// (CFAF) and marks it told. With no days left the informant is gone
    /// (bits 1, 2 and 3 cleared). AB2 is 0 meanwhile, FFh after. The
    /// carry: set when it isn't the informant.
    pub fn quest_informant(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<bool> {
        let f = self.sq8(0x0FE6);
        if self.sq16(0x05C8) & 0x4000 != 0 || f & 4 != 0 || f & 2 == 0 || self.sq_talker() != self.sq16(0x0FDE) {
            self.mem.w8(DS, 0x0AB2, 0xFF);
            return Some(true);
        }
        self.mem.w8(DS, 0x0AB2, 0);
        if (self.informant_days() as i16) <= 0 {
            let f = self.sq8(0x0FE6) & !2 & !4 & !8;
            self.mem.w8(DS, 0x0FE6, f);
            self.mem.w8(DS, 0x0AB2, 0xFF);
            return Some(false);
        }
        let no_deal = if self.sq8(0x0FE6) & 1 != 0 {
            self.informant_paid(files, hooks)?
        } else {
            let per = crate::scale::to100(self.sq8(0x01D3) as i16) as u16;
            let mode = if self.sq8(0x0FE6) & 8 != 0 { 6 } else { 5 };
            let price = self.sq16(0x0FEE) as u32;
            hooks.informant_haggle(self, files, price, per, mode)?
        };
        if !no_deal {
            let f = self.sq8(0x0FE6) | 8 | 4;
            self.mem.w8(DS, 0x0FE6, f);
            self.informant_next(files, hooks)?;
        }
        let f = self.sq8(0x0FE6) | 8;
        self.mem.w8(DS, 0x0FE6, f);
        self.mem.w8(DS, 0x0AB2, 0xFF);
        Some(false)
    }

    /// 1194:CFF7. The informant's days left: 0FED less the whole days
    /// since 0FE9.
    fn informant_days(&self) -> u16 {
        let days = self.sq32(TIME).wrapping_sub(self.sq32(0x0FE9)) / DAY;
        (self.sq8(0x0FED) as u16).wrapping_sub(days as u16)
    }

    /// 0000:5C86 with BX `page`: bank `page` mapped (`ems_map`); returns
    /// the bank mapped before (AX `ax` with EMS off or no such bank).
    fn sq_map_bank(&mut self, page: u16, ax: u16) -> u16 {
        if self.sq8(0x8FA6) == 0 || page >= self.sq16(0x9032) {
            return ax;
        }
        let old = self.sq16(0x8FA8);
        self.ems_map(page);
        old >> 2
    }

    /// 1194:D019. The informant paid for: the days left at 7880 (the
    /// dialogue file's name, 7873), its bank (1032) mapped and the file
    /// loaded into the EMS frame, the picture shown, the offer's line
    /// (row 0, `informant_line`) answered: yes pays the price (0FEE) if
    /// the gold is there (row 1 said, no carry) or tells it isn't
    /// (198Ch); leaving (2) says nothing; no says row 3. The bank is put
    /// back. Bug kept: with no days left the informant is forgotten
    /// (0FE6 bits 1, 2) and the return address (CF92h) popped as the bank
    /// to map, none being that high; it returns past CF58 as its RET
    /// would. The carry.
    fn informant_paid(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<bool> {
        let left = self.informant_days();
        if (left as i16) <= 0 {
            let f = self.sq8(0x0FE6) & !2 & !4;
            self.mem.w8(DS, 0x0FE6, f);
            self.sq_map_bank(0xCF92, 0);
            return Some(true);
        }
        self.mem.w8(DS, 0x7880, left as u8);
        let page = self.sq16(0x1032);
        let old = self.sq_map_bank(page, left);
        hooks.frame_load(self, files, 0x7873)?;
        hooks.picture(self, 0x132F);
        self.informant_line(files, hooks, 0)?;
        self.mem.w8(DS, 0x13D3, 1);
        let answer = hooks.dialog(self, 1);
        let no_deal = match answer {
            0 => {
                let fee = self.sq16(0x0FEE) as u32;
                let gold = self.sq32(0x05C2);
                if gold < fee {
                    hooks.notice(self, 0x198C);
                    true
                } else {
                    self.mem.w32(DS, 0x05C2, gold - fee);
                    self.informant_line(files, hooks, 1)?;
                    self.mem.w8(DS, 0x13D3, 0);
                    hooks.dialog(self, 0);
                    false
                }
            }
            2 => true,
            _ => {
                self.informant_line(files, hooks, 3)?;
                self.mem.w8(DS, 0x13D3, 0);
                hooks.dialog(self, 0);
                true
            }
        };
        self.sq_map_bank(old, 0);
        Some(no_deal)
    }

    /// 1194:D105, then the line made ready (D0E3..D0F9): the informant's
    /// line from the file in the EMS frame, the (step x 15 + `row` x 3
    /// + a d3, unless row 1, rolled with the frame's bytes as the RNG's)-th
    /// of its strings, copied to 929E, its
    /// substitutions made, its line feeds and returns made spaces and
    /// its lines wrapped at 3Ch (D2F3, 49FF), then split at 3 lines a
    /// page (4E54).
    fn informant_line(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, row: u16) -> Option<()> {
        let frame = self.sq16(0x9030);
        let step = self.sq8(0x0FDD) as u16;
        let mut n = step * 15 + row * 3;
        if row != 1 {
            // Bug kept: DS is the frame here, so 0000:6EEA's RNG is the
            // one at frame:91F0..91F8, made from the file's bytes, and
            // stepped there. With its table on (91F0) not followed.
            if self.mem.r16(frame, 0x91F0) != 0 {
                return None;
            }
            let mut rng = crate::random::ArenaRandom::new(self.mem.r32(frame, 0x91F6));
            let r = rng.range(0, 3)?;
            self.mem.w32(frame, 0x91F6, rng.seed);
            n = n.wrapping_add(r);
        }
        let si = self.skip_strings(frame, 0, n);
        let mut k = 0u16;
        loop {
            let b = self.mem.r8(frame, si.wrapping_add(k));
            self.mem.w8(DS, 0x929E_u16.wrapping_add(k), b);
            if b == 0 {
                break;
            }
            k = k.wrapping_add(1);
            if k == 0 {
                return None;
            }
        }
        hooks.substitute(self, files, 0x929E)?;
        let mut si = 0x929E_u16;
        while self.sq8(si) != 0 {
            if matches!(self.sq8(si), 0x0A | 0x0D) {
                self.mem.w8(DS, si, 0x20);
            }
            si = si.wrapping_add(1);
        }
        self.mem.w8(DS, crate::quest::var::TEXT_FLAG, 0x3C);
        self.sq_wrap(0x929E)?;
        self.mem.w8(DS, crate::quest::var::TEXT_FLAG, 3);
        self.sq_split(0x929E);
        Some(())
    }

    /// 1194:49FF. The text at `si` wrapped at 137E characters (3Ch when
    /// 0): from each line's end back to a space, made a line feed. None
    /// where no space is found going back (the original reads on through
    /// memory).
    fn sq_wrap(&mut self, si: u16) -> Option<()> {
        let mut ax = 0u16;
        while self.sq8(si.wrapping_add(ax)) != 0 {
            ax = ax.wrapping_add(1);
        }
        let mut si = si;
        loop {
            if self.sq8(crate::quest::var::TEXT_FLAG) == 0 {
                self.mem.w8(DS, crate::quest::var::TEXT_FLAG, 0x3C);
            }
            let bx = self.sq8(crate::quest::var::TEXT_FLAG) as u16;
            if ax <= bx {
                return Some(());
            }
            si = si.wrapping_add(bx);
            ax = ax.wrapping_sub(bx);
            let mut steps = 0u32;
            loop {
                match self.sq8(si) {
                    0 => return Some(()),
                    0x20 => break,
                    _ => {
                        si = si.wrapping_sub(1);
                        ax = ax.wrapping_add(1);
                        steps += 1;
                        if steps > 0x10000 {
                            return None;
                        }
                    }
                }
            }
            self.mem.w8(DS, si, 0x0A);
            si = si.wrapping_add(1);
        }
    }

    /// 1194:4E54. The text at `si` split at its line feeds (each made a
    /// NUL; 137E lines a page), the lines counted (plus one) at FF5. ES is
    /// taken to be DS.
    fn sq_split(&mut self, si: u16) {
        let mark = self.sq8(crate::quest::var::TEXT_FLAG);
        let (mut si, mut cx, mut dl) = (si, 0u16, 0u8);
        while self.sq8(si) != 0 {
            if self.sq8(si) == 0x0A {
                self.mem.w8(DS, si, 0);
                cx = cx.wrapping_add(1);
                dl = dl.wrapping_add(1);
                if dl == mark {
                    dl = 0;
                }
            }
            si = si.wrapping_add(1);
        }
        self.mem.w16(DS, 0x0FF5, cx.wrapping_add(1));
    }

    /// 1194:CFAF. The informant tells: text 1F6h logged (CF93: written
    /// to 1044, then 27E3), "%amn" made the next dungeon's name at 929E,
    /// a random dungeon of its province (0FE3) given it (26F7:0C0A mode
    /// 1), its index to 0FE2, 0FE6 bit 4, then taken (mode 4).
    fn informant_next(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        use crate::quest::var::{TEXT, TEXT_FLAG, TEXT_FLAG2};
        self.mem.w16(DS, TEXT, 0x1F6);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.mem.w8(DS, TEXT_FLAG, 0);
        hooks.text_at(self, files, 2, 0x1044)?;
        hooks.log_entry(self, 0x1044);
        self.mem.w32(DS, 0x929E, u32::from_le_bytes(*b"%amn"));
        self.mem.w8(DS, 0x92A2, 0);
        hooks.substitute(self, files, 0x929E)?;
        let province = self.sq8(0x0FE3) as u16;
        let ax = self.quest_dungeon_entry(hooks, province, 1, 0, 0x929E)?;
        self.mem.w8(DS, 0x0FE2, ax as u8);
        let f = self.sq8(0x0FE6) | 4;
        self.mem.w8(DS, 0x0FE6, f);
        let province = self.sq8(0x0FE3) as u16;
        self.quest_dungeon_entry(hooks, province, 4, ax, 0x929E)?;
        Some(())
    }

    /// 1194:D593. The main quest's person in a town: the stage's (0F77)
    /// province (80ECh), town (the stage-th string at 80F4h, against the
    /// place's name at A22) and building (8223h: 0 the palace, 7 a mages'
    /// guild, 5 a temple whose name, "%tem", is the stage-th at 822Bh).
    /// Not the town: away from buildings (5E6 not 4) text 519h. The town
    /// but not the person: away from buildings a d100 against (level +
    /// 1) x 10 (at most 90, D8B7) gives a hint (51Bh + stage) or not
    /// (519h). The person: `quest_person_met` (D67F). None where a hook
    /// fails.
    pub fn quest_person_talk(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        let stage = self.sq8(0x0F77) as u16;
        let away = self.sq8(AREA) != 4;
        if self.sq8(PROVINCE) != self.sq8(0x80EC + stage) {
            if away {
                self.quest_say(files, hooks, 0x519)?;
            }
            return Some(());
        }
        let town = self.skip_strings(DS, 0x80F4, stage);
        if self.sq_streq(town, 0x0A22).is_none() {
            if away {
                let text = if self.quest_chance()? { 0x51A } else { 0x519 };
                self.quest_say(files, hooks, text)?;
            }
            return Some(());
        }
        let met = if self.sq16(0xC398) & 4 != 0 {
            false
        } else {
            match self.sq8(0x8223 + stage) {
                0 => self.sq8(BUILDING) == 1,
                7 => self.sq8(BUILDING) == 7,
                _ if self.sq8(BUILDING) == 5 => {
                    self.mem.w32(DS, 0x929E, u32::from_le_bytes(*b"%tem"));
                    self.mem.w8(DS, 0x92A2, 0);
                    hooks.substitute(self, files, 0x929E)?;
                    let temple = self.skip_strings(DS, 0x822B, stage);
                    self.sq_streq(temple, 0x929E).is_some()
                }
                _ => false,
            }
        };
        if met {
            return self.quest_person_met(files, hooks);
        }
        if away {
            let text = if self.quest_chance()? { stage + 0x51B } else { 0x519 };
            self.quest_say(files, hooks, text)?;
        }
        Some(())
    }

    /// 0000:9C2C. The strings at DS:`si` and DS:`di` equal, ignoring the
    /// case of a..z: Some(SI past the NUL) when equal.
    fn sq_streq(&self, mut si: u16, mut di: u16) -> Option<u16> {
        let up = |b: u8| if b.is_ascii_lowercase() { b - 0x20 } else { b };
        loop {
            let (a, b) = (up(self.sq8(si)), up(self.sq8(di)));
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if a != b {
                return None;
            }
            if a == 0 {
                return Some(si);
            }
        }
    }

    /// 1194:D8B7. A d100 (1..100) at or under (level + 1) x 10 (8 bits),
    /// at most 90.
    fn quest_chance(&mut self) -> Option<bool> {
        let limit = (self.sq8(LEVEL).wrapping_add(1) as u16 * 10).min(0x5A);
        let r = self.rng.range(1, 0x65)?;
        Some(r <= limit)
    }

    /// 1194:D80B. Text `text` shown (A374, mode 0).
    fn quest_say(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, text: u16) -> Option<()> {
        use crate::quest::var::{TEXT, TEXT_FLAG, TEXT_FLAG2};
        self.mem.w8(DS, TEXT_FLAG, 0);
        self.mem.w16(DS, TEXT, text);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        hooks.text_at(self, files, 0, 0).map(|_| ())
    }

    /// 1194:D66F. The stage's texts begin at 523h + stage x 6.
    fn quest_texts(&self) -> u16 {
        (self.sq8(0x0F77) as u16).wrapping_mul(6).wrapping_add(0x523)
    }

    /// 1194:D67F. The main quest's person met (the fonts set, colour
    /// 60h), by the flags of the stage: 0F7D (the person is the one the
    /// quest names), 0F7B (spoken to), 0F79 (asked), 0F7C. A named person
    /// first met gives the stage's place (26F7:0C0A mode 2), says text
    /// +4, logs 20Dh + stage (in 8 bits); then in a temple or guild
    /// nothing, elsewhere 80C9h's message. Otherwise the question (+0, or
    /// +3 once spoken to) is asked over pages (D863) and answered: yes
    /// logs 217h + stage, sets 0F79 and 0F78, says +1 and takes the
    /// stage's dungeon (mode 3); no clears 0F79 and says +2. Once asked
    /// (0F79): with 0F7C +5, else as for a named person met. 26F7:0C0A
    /// records its caller's CX and SI (1296, 12A0), left by the text
    /// routine here: 0 in the port.
    fn quest_person_met(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<()> {
        use crate::quest::var::{TEXT, TEXT_COLOUR, TEXT_FLAG, TEXT_FLAG2};
        let ax = self.sq16(0x1036);
        hooks.font(self, 1, ax);
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        let ax = self.sq16(0x102E);
        hooks.font(self, 0, ax);
        let province = |m: &Machine| m.sq8(0x80EC + m.sq8(0x0F77) as u16) as u16;
        let done_here = |m: &mut Machine, hooks: &mut dyn QuestHooks| {
            if !matches!(m.sq8(BUILDING), 7 | 5) {
                hooks.message_box(m, 0x80C9);
            }
        };
        if self.sq8(0x0F7D) != 0 {
            if self.sq8(0x0F7B) != 0 {
                done_here(self, hooks);
                return Some(());
            }
            self.quest_dungeon_entry(hooks, province(self), 2, 0, 0)?;
            self.mem.w8(DS, 0x0F7B, 1);
            self.mem.w8(DS, 0x0F7F, 0);
            self.quest_say(files, hooks, self.quest_texts() + 4)?;
            let t = 0x200 | (0x0D_u8.wrapping_add(self.sq8(0x0F77))) as u16;
            self.mem.w16(DS, TEXT, t);
            self.mem.w8(DS, TEXT_FLAG2, 0);
            self.mem.w8(DS, TEXT_FLAG, 0);
            hooks.text_at(self, files, 2, 0x1044)?;
            hooks.log_entry(self, 0x1044);
            return Some(());
        }
        let question = if self.sq8(0x0F7B) != 0 {
            if self.sq8(0x0F79) != 0 {
                if self.sq8(0x0F7C) == 0 {
                    done_here(self, hooks);
                } else {
                    self.quest_say(files, hooks, self.quest_texts() + 5)?;
                }
                return Some(());
            }
            self.quest_texts() + 3
        } else {
            self.quest_texts()
        };
        self.mem.w16(DS, TEXT, question);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        hooks.text_at(self, files, 2, 0x9622)?;
        self.mem.w8(DS, TEXT_FLAG, 0xA);
        let yes = self.quest_pages(hooks) == 0;
        self.mem.w8(DS, 0x0F7B, 1);
        if !yes {
            self.mem.w8(DS, 0x0F79, 0);
            return self.quest_say(files, hooks, self.quest_texts() + 2);
        }
        let t = 0x200 | (0x17_u8.wrapping_add(self.sq8(0x0F77))) as u16;
        self.mem.w16(DS, TEXT, t);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.mem.w8(DS, TEXT_FLAG, 0);
        hooks.text_at(self, files, 2, 0x1044)?;
        hooks.log_entry(self, 0x1044);
        self.mem.w8(DS, 0x0F79, 1);
        self.mem.w8(DS, 0x0F78, 1);
        self.quest_say(files, hooks, self.quest_texts() + 1)?;
        self.quest_dungeon_entry(hooks, province(self), 3, 0, 0)?;
        Some(())
    }

    /// 1194:D863. The text at 9622 shown in pages: split (D8DE, 0Ah
    /// lines a page, each page's last line feed or return a NUL), the
    /// fonts set, colour 60h, each page but the last in a message box,
    /// the last offered with its yes and no (020E, the keys at 8256h, the
    /// entries at 8259h). The pick.
    fn quest_pages(&mut self, hooks: &mut dyn QuestHooks) -> u16 {
        use crate::quest::var::{TEXT_COLOUR, TEXT_FLAG};
        self.mem.w8(DS, TEXT_FLAG, 0xA);
        let mark = self.sq8(TEXT_FLAG);
        let (mut si, mut pages, mut dl) = (0x9622_u16, 1u16, 0u8);
        while self.sq8(si) != 0 {
            if matches!(self.sq8(si), 0x0D | 0x0A) {
                dl = dl.wrapping_add(1);
                if dl == mark {
                    pages = pages.wrapping_add(1);
                    self.mem.w8(DS, si, 0);
                    dl = 0;
                }
            }
            si = si.wrapping_add(1);
        }
        let ax = self.sq16(0x102E);
        hooks.font(self, 0, ax);
        let ax = self.sq16(0x1036);
        hooks.font(self, 1, ax);
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        let mut si = 0x9622_u16;
        for _ in 1..pages {
            hooks.message_box(self, si);
            // `repne scasb` over at most BB8h bytes: past the NUL, or
            // BB8h on.
            si = match (0..0xBB8u16).find(|&k| self.sq8(si.wrapping_add(k)) == 0) {
                Some(k) => si.wrapping_add(k + 1),
                None => si.wrapping_add(0xBB8),
            };
        }
        hooks.menu(self, si, 0x8256, 0x8259)
    }

    /// 1194:9F8A. The first quest under way (kind 1), unless its stage
    /// is 6 (the carry).
    pub fn quest_under_way(&self) -> Option<u16> {
        let si = (0..4).map(|i| QUESTS + i * QUEST).find(|&si| self.quest_kind(si) == 1)?;
        (self.quest_stage(si) != 6).then_some(si)
    }

    /// 1194:86F2, every minute from the clock: a quest's ambush. Not
    /// during a scene (5CA bit 200h), a fight (0FF7, 0FFB), on a
    /// building's square, or inside most buildings (a dungeon, D48 8 and
    /// up, or a tavern, or once broken into, 191D, are open to it); only
    /// with a quest under way (9F8A) and when the countdown at 1404 runs
    /// out. Then a d100 at or under the chance at 681Ah (681Bh with an
    /// overlay up, 1013; 681Ch outside a tavern, or in a tavern in a
    /// dungeon or the wild, also making it night, 1238) picks an entry of
    /// the quest's row (+22h) of the table at 3276:4C65 (45h bytes a row,
    /// three an entry) by the level (at most 20) plus a d4: its count,
    /// type and level. By day a type in the list at 681Dh doesn't come;
    /// nor one of the first creature's type (8AF0). Otherwise the fight
    /// begins (89A7, 0FFB), with text 506h + the row. The countdown is set
    /// again to 10..29 after a roll.
    pub fn quest_ambush(&mut self, files: &mut Files, fight: &mut dyn crate::encounter::FightHooks, hooks: &mut dyn QuestHooks) -> Option<()> {
        if self.sq16(0x05CA) & 0x200 != 0 || self.sq16(0x0FF7) != 0 || self.sq16(0x0FFB) != 0 || self.in_building() {
            return Some(());
        }
        let kind = self.sq8(BUILDING);
        if self.sq8(AREA) == 4 && self.sq8(0x191D) == 0 && kind != 4 && kind < 8 {
            return Some(());
        }
        let Some(si) = self.quest_under_way() else { return Some(()) };
        let n = self.sq8(0x1404).wrapping_sub(1);
        self.mem.w8(DS, 0x1404, n);
        if n != 0 {
            return Some(());
        }
        self.mem.w16(DS, CURRENT, si);
        let row = self.sq16(si + 0x22);
        self.mem.w16(DS, 0x123A, row);
        let night = self.night();
        self.mem.w16(DS, 0x1238, night);
        let d = self.sq_d100();
        let mut bx = (self.sq8(0x1013) != 0) as u16;
        if self.sq8(0x191D) == 0 && (kind != 4 || matches!(self.sq8(AREA), 2 | 3)) {
            bx = 2;
            self.mem.w16(DS, 0x1238, 1);
        }
        if d as u8 <= self.sq8(0x681A + bx) {
            self.quest_ambush_comes(files, fight, hooks)?;
        }
        let r = self.rng.range(0xA, 0x1E)?;
        self.mem.w8(DS, 0x1404, r as u8);
        Some(())
    }

    /// 1194:8795..8869: the ambush's entry, and the fight.
    fn quest_ambush_comes(&mut self, files: &mut Files, fight: &mut dyn crate::encounter::FightHooks, hooks: &mut dyn QuestHooks) -> Option<()> {
        self.mem.w16(DS, 0x123C, 0);
        let a = self.sq_roll(4)?;
        let level = (self.sq8(LEVEL) as u16).min(0x14);
        let at = level.wrapping_add(a).wrapping_mul(3).wrapping_add(self.sq16(0x123A).wrapping_mul(0x45));
        let data = crate::mem::DATA;
        let ty = self.mem.r8(data, 0x4C65_u16.wrapping_add(at));
        let count = self.mem.r8(data, 0x4C66_u16.wrapping_add(at));
        let lv = self.mem.r8(data, 0x4C67_u16.wrapping_add(at));
        if self.sq16(0x1238) == 0 {
            self.mem.w16(DS, 0x123C, 1);
            let mut k = 0u16;
            while self.sq8(0x681D + k) != 0 {
                if self.sq8(0x681D + k) == ty {
                    return Some(());
                }
                k += 1;
            }
        }
        self.mem.w8(DS, 0x929E, count);
        self.mem.w8(DS, 0x929F, ty);
        self.mem.w8(DS, 0x92A0, lv);
        if ty == self.encounter_current() {
            return Some(());
        }
        let f = self.sq16(0x05C8) & !0x1000;
        self.mem.w16(DS, 0x05C8, f);
        self.mem.w8(DS, 0x1013, 0);
        self.mem.w16(DS, 0x1294, 1);
        self.quest_encounter(files, fight, count, ty, lv)?;
        self.mem.w16(DS, 0x0FFB, 1);
        let ax = self.sq16(0x102C);
        hooks.font(self, 0, ax);
        let text = self.sq16(0x123A).wrapping_add(0x506);
        self.mem.w16(DS, crate::quest::var::TEXT, text);
        self.mem.w8(DS, crate::quest::var::TEXT_FLAG, 0x3C);
        self.mem.w8(DS, 0x7387, 2);
        hooks.text_at(self, files, 0, 0)?;
        Some(())
    }

    /// `f` run with the player put at the place's square (9D51: A48, A4A
    /// in squares to C3D0, C3D4), then put back.
    fn sq_at_place<T>(&mut self, f: impl FnOnce(&mut Machine) -> Option<T>) -> Option<T> {
        let (x, y) = (self.sq16(0xC3D0), self.sq16(0xC3D4));
        let (px, py) = (self.sq16(0x0A48) << 7, self.sq16(0x0A4A) << 7);
        self.mem.w16(DS, 0xC3D0, px);
        self.mem.w16(DS, 0xC3D4, py);
        let r = f(self)?;
        self.mem.w16(DS, 0xC3D4, y);
        self.mem.w16(DS, 0xC3D0, x);
        Some(r)
    }
}
