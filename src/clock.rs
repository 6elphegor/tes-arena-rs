//! The game clock (0000:142D): minutes pass one at a time, the calendar
//! fields are recomputed (0000:1570) and each one that changed runs its
//! handler; the time of day for the sky follows (0000:16F7).
//!
//! The minute handler (0000:1644) is the game's timed tick: auras, timed
//! spell flags, effects, the drain check, a few timers, the level-up
//! check, fatigue and the encounter checks. The day handler checks quest
//! deadlines, rolls the weather and lets Knights repair their gear; the
//! hour handler runs the hourly quest checks and encounters.
//!
//! Quests (1194:99F8, 98E8), random encounters (1194:887A, 86F2), the
//! level-up screen (1194:1034) and what happens when the player runs out
//! of fatigue (2901:0014..) are reached through `ClockHooks`.

use crate::bsa::Files;
use crate::character::{field, PLAYER};
use crate::machine::Machine;
use crate::mem::DS;
use crate::strike::GameHooks;

/// DGROUP variables.
pub mod var {
    /// Minutes since the start of the calendar (dword).
    pub const MINUTES: u16 = 0x05CC;
    /// The calendar: year, month, day, hour, minute.
    pub const YEAR: u16 = 0x05D0;
    pub const MONTH: u16 = 0x05D2;
    pub const DAY: u16 = 0x05D4;
    pub const HOUR: u16 = 0x05D6;
    pub const MINUTE: u16 = 0x05D8;
    /// Month lengths in minutes (dwords).
    pub const MONTHS: u16 = 0x4ECE;
    /// The week number's low byte, and what a new week resets.
    pub const WEEK: u16 = 0x0A20;
    pub const WEEKLY: u16 = 0x05C6;
    /// The time of day for the sky: day of the month & 1Fh and the
    /// minute of the day × 1024 / 1440 (or 2D0h minutes when CB5C is
    /// set).
    pub const SKY_DAY: u16 = 0xCCA8;
    pub const SKY_TIME: u16 = 0xCCFE;
    pub const FIXED_TIME: u16 = 0xCB5C;
    /// Bit 0x80: the player is dead or leaving the area (the clock stops).
    pub const AREA_FLAGS: u16 = 0xC398;
    pub const LEVEL_FLAGS: u16 = 0xC39A;
    /// Four countdown pairs (bytes, overlapping).
    pub const TIMERS: u16 = 0x1378;
}

/// The handlers the clock calls that are not ported here.
pub trait ClockHooks {
    /// 1194:99F8, daily: the deadlines of the records at 0DCA
    /// (`quest::quests_daily`, its texts and files left out; override to
    /// show them).
    fn quests_daily(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        m.quests_daily(&mut crate::quest::NoQuestHooks);
        Some(())
    }
    /// 1194:98E8, hourly: the quests' deadlines (`quest::quests_hourly`,
    /// likewise).
    fn quests_hourly(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_hourly(files, &mut crate::quest::NoQuestHooks);
        Some(())
    }
    /// 1194:887A: a random encounter check (`encounter::random_encounter`,
    /// with the fight's graphics, music and text left out; override to
    /// show them).
    fn encounter(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.random_encounter(files, &mut crate::encounter::NoFightHooks)
    }
    /// 1194:86F2: the per-minute encounter check (also while resting): a
    /// quest's ambush (`sidequest::quest_ambush`, the fight's graphics,
    /// music and text left out; override to show them).
    fn encounter_minute(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_ambush(files, &mut crate::encounter::NoFightHooks, &mut crate::quest::NoQuestHooks)
    }
    /// 2901:0014..: the player's fatigue went below 0. Whether the port
    /// runs it (`Machine::exhaustion`: in a fight they die, in water they
    /// drown, elsewhere they collapse and rest an hour); false where the
    /// harness stubs it.
    fn exhausted(&mut self, _m: &mut Machine) -> Option<bool> {
        Some(true)
    }
    /// 1194:59C8 from the exhaustion: its text at DS:`si` (7B6A dead in a
    /// fight, 7B2F drowned, 7AE0 collapsed).
    fn exhaustion_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 0000:4EFD's fill (9A72): the view filled with colour 0 (9288,
    /// which the port sets).
    fn view_cleared(&mut self, _m: &mut Machine) {}
    /// 2686:0319..0331: the level-up screen (1194:1034,
    /// `Machine::level_up_screen`), after `levels` levels were gained,
    /// with the text segment B038 set from 103A around it (0000:9AF8) and
    /// from 1036 after. By default its points are spent in turn
    /// (`levelup::NoLevelUpHooks`); front ends show it.
    fn level_up_screen(&mut self, m: &mut Machine, levels: u16) -> Option<()> {
        crate::levelup::level_up_around(m, levels, &mut crate::levelup::NoLevelUpHooks)
    }
}

impl Machine {
    /// 0000:142D. `minutes` minutes pass, one at a time, unless the area
    /// is being left (C398 bit 0x80), checked before each. The count
    /// runs in CX, so only its low 16 bits count, and 0 means 65536 (a
    /// whole dword of 0 does nothing).
    pub fn advance_minutes(&mut self, files: &mut Files, minutes: u32, hooks: &mut dyn ClockHooks) -> Option<()> {
        if minutes == 0 {
            return Some(());
        }
        let n = match minutes & 0xFFFF {
            0 => 0x10000,
            n => n,
        };
        for _ in 0..n {
            if self.mem.r16(DS, var::AREA_FLAGS) & 0x80 != 0 {
                return Some(());
            }
            self.minute_passes(files, hooks)?;
        }
        Some(())
    }

    /// 0000:1459. One minute: the calendar, the sky, and the week.
    fn minute_passes(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        let t = self.mem.r32(DS, var::MINUTES).wrapping_add(1);
        self.mem.w32(DS, var::MINUTES, t);
        self.calendar(files, t, hooks)?;
        self.sky_time()?;
        let week = (self.mem.r32(DS, var::MINUTES) / 10080) as u8;
        if self.mem.r8(DS, var::WEEK) != week {
            self.mem.w8(DS, var::WEEK, week);
            self.mem.w16(DS, var::WEEKLY, 0);
        }
        Some(())
    }

    /// 0000:1570. Splits `t` into year (from 389), month, day, hour and
    /// minute, and runs the handler of each field that changed (year and
    /// month have none).
    fn calendar(&mut self, files: &mut Files, t: u32, hooks: &mut dyn ClockHooks) -> Option<()> {
        let year = ((t / 518_400) as u16).wrapping_add(0x185);
        self.mem.w16(DS, var::YEAR, year);
        let mut rem = t % 518_400;
        let mut month = 0u16;
        let mut at = var::MONTHS;
        loop {
            let len = self.mem.r32(DS, at);
            if rem < len {
                break;
            }
            rem -= len;
            at = at.wrapping_add(4);
            month = month.wrapping_add(1);
        }
        self.mem.w16(DS, var::MONTH, month);
        let day = (rem / 1440) as u16;
        let rem = rem % 1440;
        let changed = |m: &mut Self, at: u16, v: u16| {
            let old = m.mem.r16(DS, at);
            m.mem.w16(DS, at, v);
            old != v
        };
        if changed(self, var::DAY, day) {
            self.day_passes(files, hooks)?;
        }
        if changed(self, var::HOUR, (rem / 60) as u16) {
            self.hour_passes(files, hooks)?;
        }
        if changed(self, var::MINUTE, (rem % 60) as u16) {
            self.minute_tick(files, hooks)?;
        }
        Some(())
    }

    /// 0000:16F7. The time of day for the sky. None where the original's
    /// division overflows (more than 65535 days).
    pub fn sky_time(&mut self) -> Option<()> {
        let t = if self.mem.r8(DS, var::FIXED_TIME) != 0 { 0x2D0 } else { self.mem.r32(DS, var::MINUTES) };
        let days = u16::try_from(t / 1440).ok().or_else(|| crate::giveup::original("0000:16F7: more than 65535 days: the divide overflows"))?;
        self.mem.w16(DS, var::SKY_DAY, days & 0x1F);
        let of_day = t % 1440;
        self.mem.w16(DS, var::SKY_TIME, ((of_day << 10) / 1440) as u16);
        Some(())
    }

    /// 0000:1607. A new day: quest deadlines, flag 5E0 bit 0x100 cleared,
    /// the weather, and a Knight (class byte 11h) repairs their gear.
    fn day_passes(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        hooks.quests_daily(self, files)?;
        let f = self.mem.r16(DS, 0x05E0) & !0x100;
        self.mem.w16(DS, 0x05E0, f);
        self.weather_roll()?;
        if self.mem.r8(DS, PLAYER + field::CLASS) == 0x11 {
            self.knight_repair();
        }
        Some(())
    }

    /// 0000:172E. Each of the player's items that is not an accessory or
    /// a stack regains up to (level + 1) × 10 condition, walking as many
    /// consecutive slots as are occupied.
    fn knight_repair(&mut self) {
        let amount = (self.mem.r8(DS, PLAYER + field::LEVEL) as u16 + 1).wrapping_mul(10);
        let mut si = crate::inventory::PLAYER_SLOTS;
        let (n, _) = self.count_items(DS, si);
        for _ in 0..n {
            let cat = (self.mem.r8(DS, si.wrapping_add(crate::item::FLAGS)) & 0xC) >> 2;
            if cat != 2 && cat != 3 {
                let cur = self.mem.r16(DS, si.wrapping_add(crate::item::HEALTH));
                let gap = self.mem.r16(DS, si.wrapping_add(crate::item::MAX_HEALTH)).wrapping_sub(cur);
                if gap != 0 {
                    self.mem.w16(DS, si.wrapping_add(crate::item::HEALTH), cur.wrapping_add(gap.min(amount)));
                }
            }
            si = si.wrapping_add(crate::item::SIZE);
        }
    }

    /// 0000:1622. A new hour: quest progress, then an encounter check
    /// (unless 5C8 bit 0x20) in areas with level flag 200h or while 1013
    /// is set.
    fn hour_passes(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        hooks.quests_hourly(self, files)?;
        if self.mem.r16(DS, 0x05C8) & 0x20 != 0 {
            return Some(());
        }
        if self.mem.r16(DS, var::LEVEL_FLAGS) & 0x200 != 0 || self.mem.r8(DS, 0x1013) != 0 {
            hooks.encounter(self, files)?;
        }
        Some(())
    }

    /// 0000:1644. The minute tick.
    fn minute_tick(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        self.auras();
        self.tick_spell_flags();
        self.tick_effects(&mut GameHooks);
        self.drain_death_check(&mut GameHooks);
        self.countdowns();
        let busy = self.mem.r8(DS, 0x05E6) == 4 && self.mem.r8(DS, 0x137F) != 0xFF;
        if !busy && self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x1013) == 0 && self.mem.r16(DS, 0x05CA) & 0x100 != 0 {
            if let Some(n) = self.level_up()? {
                hooks.level_up_screen(self, n)?;
            }
        }
        if self.mem.r16(DS, 0x05C8) & 0x20 != 0 {
            return Some(());
        }
        self.fatigue_minute(files, hooks)?;
        let flags = self.mem.r16(DS, var::LEVEL_FLAGS);
        if flags & 0x10 != 0 && flags & 0x4000 == 0 {
            let n = if self.mem.r8(DS, PLAYER + field::RACE) == 7 { 0xC } else { 0x1E };
            for _ in 0..n {
                self.fatigue_minute(files, hooks)?;
                if self.mem.r16(DS, 0x05B1) & 0x100 != 0 {
                    return Some(());
                }
            }
        }
        hooks.encounter_minute(self, files)?;
        let mode = self.mem.r8(DS, 0x05E6);
        let check = mode == 0
            || (self.mem.r16(DS, var::AREA_FLAGS) & 4 == 0 && (mode == 4 || self.mem.r8(DS, 0x1013) == 0));
        if check {
            hooks.encounter(self, files)?;
        }
        Some(())
    }

    /// 2901:0002. Fatigue drops by 11 a minute (not while 1013 is set);
    /// below 0 the player is exhausted.
    pub fn fatigue_minute(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        if self.mem.r8(DS, 0x1013) != 0 {
            return Some(());
        }
        let at = PLAYER + field::FATIGUE;
        let v = self.mem.r16(DS, at).wrapping_sub(0xB);
        self.mem.w16(DS, at, v);
        if (v as i16) < 0 && hooks.exhausted(self)? {
            self.exhaustion(files, hooks)?;
        }
        Some(())
    }

    /// 29CF:0C81. Creatures of the aura types (2DE6 entry FEh) burn the
    /// player: element 4, 3 points before resistance. "In range" is
    /// 0000:1242, which measures the creature's map coordinates, not its
    /// distance to the player: within C8h of the map's corner. The first
    /// person or other creature "in range" ends the check. Records are
    /// read through FS, which 29CF:0B35 leaves at DGROUP once it gets
    /// past its first step: after such a burn the next records are read
    /// from DGROUP.
    fn auras(&mut self) {
        let mut seg = self.mem.r16(DS, 0x1042);
        for k in 0..8u16 {
            let si = 0x0C57 + k * 0x1C;
            if self.mem.r16(DS, si + 0xF) & 0x4000 != 0 || self.mem.r8(DS, si + 0xC) == 0xC5 {
                continue;
            }
            let (x, y) = (self.mem.r16(DS, si), self.mem.r16(DS, si + 4));
            if crate::actors::approx(x, y) > 0xC8 {
                continue;
            }
            let rec = self.mem.r16(DS, si + 0x12);
            if self.mem.r16(seg, rec.wrapping_add(field::FLAGS)) & field::FLAG_CREATURE == 0 {
                return;
            }
            let ty = self.mem.r8(seg, rec.wrapping_add(field::RACE)) as u16;
            if self.mem.r8(DS, 0x2DE6u16.wrapping_add(ty).wrapping_sub(1)) != 0xFE {
                return;
            }
            if 3u16.wrapping_sub(self.mem.r8(DS, PLAYER + 0x2B) as u16 / 0x1A) as i16 > 0 {
                seg = DS;
            }
            let d = self.resist(DS, PLAYER, 3, 4);
            let hp = self.mem.r16(DS, PLAYER + field::HP).wrapping_sub(d);
            self.mem.w16(DS, PLAYER + field::HP, hp);
        }
    }

    /// 1194:BC6A. Four countdowns at 1378 (byte steps over byte pairs):
    /// one that reaches 0 counts the next byte down too.
    fn countdowns(&mut self) {
        for b in 0..4u16 {
            let at = var::TIMERS + b;
            let v = self.mem.r8(DS, at);
            if v == 0 {
                continue;
            }
            self.mem.w8(DS, at, v - 1);
            if v == 1 {
                let w = self.mem.r8(DS, at + 1);
                if w != 0 {
                    self.mem.w8(DS, at + 1, w - 1);
                }
            }
        }
    }
}

/// `ClockHooks` that do nothing (the harness stubs those routines).
pub struct NoClockHooks;

impl ClockHooks for NoClockHooks {
    fn exhausted(&mut self, _m: &mut Machine) -> Option<bool> {
        Some(false)
    }
    fn level_up_screen(&mut self, _m: &mut Machine, _levels: u16) -> Option<()> {
        Some(())
    }
    fn encounter(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_daily(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_hourly(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn encounter_minute(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
}
