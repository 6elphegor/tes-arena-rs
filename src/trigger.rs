//! A wall's trigger (0000:43B1): clicking a building's door to go in, a
//! city's gate to go out, an interior's way out.
//!
//! A click on a wall (the square under the cursor, CB92), or a break-in
//! by the swing (0000:3D43), runs the wall's trigger when the square's
//! corner is within DCh of the player and the wall (a plain one, its two
//! bytes alike, or any with bit 8000h) is one of the level's 16 trigger
//! walls (C917; the INF's `MENU n`). The trigger's handler is the n-th of
//! a table by where the player is:
//!
//! - indoors (89CB): every trigger leaves the interior (0000:44BD);
//! - in a city (898B): 0 the equipment store, 1 the tavern, 2 the mages'
//!   guild, 3 the temple, 4..6 houses, 7 and 8 the gates out to the
//!   wilderness, 9 a noble's house, 11..13 the palace;
//! - in the wilderness (894B): 1 a crypt, 2 a house, 3 a tavern, 4 a
//!   temple, 5 a tower, 6 and 7 a city's gates, 8 and 9 a dungeon.
//!
//! The handlers of buildings set the interior's kind (D48) and music
//! (FF0) and go in through 0000:4054: the interior's file (1351) is
//! `<kind>N.MIF`, N from the square (1 + a hash of it, mod 8); a closed
//! building (houses always, shops by night, holidays: 1194:4FB8) is
//! locked, unless broken into (5CA bit 800h) or opened by a spell (DC4):
//! then a lockpicking attempt if armed (5CA bit 0), whose failure may
//! bring the guards. Going in, the area is left (C398 bit 80h) with the
//! player's place kept (FCE, FD0) for coming out.
//!
//! The screen, the music, the palace's audiences and the area changes
//! themselves are `TriggerHooks`.

use crate::bsa::Files;
use crate::click::var::{MESSAGE_TIME, OPEN_SPELL, PICK_FAILED};
use crate::controls::var::PICKED;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{MAP_FLAGS, TRIGGERS, X, Y};

/// DGROUP variables.
pub mod var {
    /// The handler tables (16 far pointers each): indoors, in a city, in
    /// the wilderness.
    pub const INDOORS: u16 = 0x89CB;
    pub const CITY: u16 = 0x898B;
    pub const WILD: u16 = 0x894B;
    /// The square whose trigger ran.
    pub const TRIGGERED: u16 = 0x0FD2;
    /// The interior's kind (1 the palace .. 0Bh a tower), its music (a
    /// name's offset) and its file's name.
    pub const KIND: u16 = 0x0D48;
    pub const MUSIC: u16 = 0x0FF0;
    pub const INTERIOR: u16 = 0x1351;
    /// Where the player stood outside.
    pub const OUTSIDE_X: u16 = 0x0FCE;
    pub const OUTSIDE_Y: u16 = 0x0FD0;
    /// The area being entered (0 a city, 1 the wilderness, 3 a dungeon,
    /// 4 an interior).
    pub const AREA: u16 = 0x05E6;
    /// A scratch block (64 bytes) cleared for the handler, kept at
    /// E2CC meanwhile.
    pub const SCRATCH: u16 = 0x8FAA;
    pub const SCRATCH_SAVED: u16 = 0xE2CC;
}
use var::*;

/// Music names (DGROUP offsets).
const PALACE_XMI: u16 = 0x87B3;
const TEMPLE_XMI: u16 = 0x87BE;
const MAGIC_XMI: u16 = 0x87C9;
const EQUIPMENT_XMI: u16 = 0x87D5;
const TAVERN_XMI: u16 = 0x87E2;
const SNEAKING_XMI: u16 = 0x87ED;
const SQUARE_XMI: u16 = 0x87FA;
const DUNGEON_XMI: u16 = 0x8805;

/// What the triggers leave to the front end and to code not ported.
pub trait TriggerHooks {
    /// 1194:59C8: the text at DGROUP `text` in a message box.
    fn message_box(&mut self, _m: &mut Machine, _text: u16) {}
    /// 23BA:002A: sound `sound` at the place `at`.
    fn sound(&mut self, m: &mut Machine, sound: u16, at: u16) {
        let _ = m.sound_at(sound, at);
    }
    /// 0000:3EBE: the guards are called (`Machine::call_guards`).
    fn guards(&mut self, m: &mut Machine) {
        m.call_guards();
    }
    /// 1194:C048: the tavern's room (looked for on disk: a room rented
    /// still, 1019, 12BC, 12CA).
    fn tavern_room(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.room_read(files);
        Some(())
    }
    /// 0000:41A4..4247: the Imperial palace with the main quest at its
    /// stage 8 (F77): the audience with the Emperor, then the palace
    /// (IMPPAL.MIF) entered.
    fn palace_audience(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.emperor_audience(files, &mut crate::quest::NoQuestHooks)
    }
    /// 1194:A36C (AX 0): at stage 9 the audience that starts the quest
    /// for the artifact (137A 5DDh).
    fn artifact_audience(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 2F9F:0438: the sounds playing stopped.
    fn sounds_off(&mut self, _m: &mut Machine) {}
    /// 1194:55B2: the palette (929E) black and set, the screen cleared.
    fn blackout(&mut self, _m: &mut Machine) {}
    /// 23F1:0145 and 013C (their 0178): the area's automap written back
    /// (`Machine::automap_write`) as the building is entered, before and
    /// after AREA is set; 013C's clearing of 6636 follows in `go_in`.
    fn automap_write(&mut self, m: &mut Machine, files: &mut Files) {
        m.automap_write(files);
    }
    /// 1194:D55F and 2433:0199: an interior is left for the outside
    /// (`Machine::staff_leave`, then `Machine::piles_forget` with AX as
    /// 4D91 leaves it: 0 in the port).
    fn leave_interior(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.staff_leave(files);
        m.piles_forget(0);
        Some(())
    }
    /// 217B:027A: a city's gate out to the wilderness: the city written
    /// as the wilderness's blocks 1..4 (`Machine::wild_leave_city`).
    fn leave_city(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.wild_leave_city(files, &mut crate::wild::PortWild);
        Some(())
    }
    /// 0000:4EFD and 946A: a city's gate from the wilderness (the view's
    /// window and the screen cleared; the city loads after).
    fn enter_city(&mut self, _m: &mut Machine) {}
    /// 217B:04DC: a dungeon's entrance in the wilderness.
    fn enter_dungeon(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
}

/// `TriggerHooks` that do nothing but call the guards.
pub struct NoTriggerHooks;

impl TriggerHooks for NoTriggerHooks {}

impl Machine {
    fn set_flag(&mut self, at: u16, set: u16, clear: u16) {
        let v = self.mem.r16(DS, at) & !clear | set;
        self.mem.w16(DS, at, v);
    }

    /// 0000:43B1. The trigger of the wall under the cursor (PICKED), if
    /// it is near (see the module's notes; the distance is 0000:1242 from
    /// the square's corner).
    pub fn wall_trigger(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks) -> Option<()> {
        let sq = self.mem.r16(DS, PICKED);
        if sq == 0xFFFF || self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x10 != 0 {
            return Some(());
        }
        let dx = (((sq as u8) >> 1) as u16).wrapping_shl(7).wrapping_sub(self.mem.r16(DS, X));
        let dy = (sq >> 8).wrapping_shl(7).wrapping_sub(self.mem.r16(DS, Y));
        if crate::actors::approx(dx, dy) > 0xDC {
            return Some(());
        }
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let w = self.mem.r16(walls, sq);
        if (w as i16) >= 0 && (w >> 8) as u8 != w as u8 {
            return Some(());
        }
        let al = w as u8 & 0x3F;
        let Some(i) = (0..0x10).find(|&i| self.mem.r8(DS, TRIGGERS + i) == al) else {
            return Some(());
        };
        let table = if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 {
            INDOORS
        } else if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x200 != 0 {
            WILD
        } else {
            CITY
        };
        for k in 0..0x40 {
            let b = self.mem.r8(DS, SCRATCH + k);
            self.mem.w8(DS, SCRATCH_SAVED + k, b);
        }
        let dl = (((sq.rotate_right(4) ^ sq) & 0xFF) % 8) as u8;
        for k in 0..0x40 {
            self.mem.w8(DS, SCRATCH + k, 0);
        }
        self.mem.w16(DS, TRIGGERED, sq);
        self.trigger_handler(files, hooks, table, i, dl)?;
        self.mem.w16(DS, 0x0FF7, 0);
        for k in 0..0x40 {
            let b = self.mem.r8(DS, SCRATCH_SAVED + k);
            self.mem.w8(DS, SCRATCH + k, b);
        }
        Some(())
    }

    /// The handler `i` of `table` (the far pointers at 89CB, 898B, 894B).
    fn trigger_handler(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks, table: u16, i: u16, dl: u8) -> Option<()> {
        match (table, i) {
            (INDOORS, _) => {
                // 0000:44BD
                self.set_flag(0xC398, 0x80, 0);
                hooks.leave_interior(self, files)
            }
            // 0000:4322
            (CITY, 0) => self.building(files, hooks, dl, Some((1, 4)), EQUIPMENT_XMI, 6),
            (CITY, 1) | (WILD, 3) => self.tavern(files, hooks, dl),
            // 0000:4341
            (CITY, 2) => self.building(files, hooks, dl, Some((1, 1)), MAGIC_XMI, 7),
            // 0000:4303
            (CITY, 3) | (WILD, 4) => self.building(files, hooks, dl, Some((1, 3)), TEMPLE_XMI, 5),
            // 0000:42B4
            (CITY, 4..=6) | (WILD, 2) => self.building(files, hooks, dl, None, SNEAKING_XMI, 2),
            // 0000:42C3
            (CITY, 9) => self.building(files, hooks, dl, None, SNEAKING_XMI, 3),
            // 0000:4360
            (WILD, 1) => self.building(files, hooks, dl, Some((0, 0xFFFF)), DUNGEON_XMI, 8),
            // 0000:4374
            (WILD, 5) => self.building(files, hooks, dl, Some((0, 0xFFFF)), DUNGEON_XMI, 0xB),
            (CITY, 7 | 8) => {
                // 0000:43A0
                self.set_flag(0xC398, 0x80, 0);
                self.mem.w8(DS, AREA, 1);
                hooks.leave_city(self, files)
            }
            (WILD, 6 | 7) => {
                // 0000:429B
                self.set_flag(0xC398, 0x80, 0);
                self.set_flag(crate::movement::var::LEVEL_FLAGS, 0, 0x200);
                self.mem.w8(DS, AREA, 0);
                hooks.enter_city(self);
                Some(())
            }
            (WILD, 8 | 9) => {
                // 0000:4388, calling 0000:08EC
                hooks.enter_dungeon(self, files)?;
                self.set_flag(0x05C8, 0x80, 0);
                self.set_flag(0xC398, 0x80, 0);
                self.set_flag(crate::movement::var::LEVEL_FLAGS, 0, 0x200);
                self.mem.w8(DS, AREA, 3);
                Some(())
            }
            (CITY, 11..=13) => self.palace(files, hooks),
            // 0000:439F: a bare retf.
            _ => Some(()),
        }
    }

    /// A building's handler: 191F and 137A (FFFFh: left), its music and
    /// kind, then in (0000:4054).
    #[allow(clippy::too_many_arguments)]
    fn building(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks, dl: u8, f: Option<(u8, u16)>, music: u16, kind: u8) -> Option<()> {
        if let Some((a, b)) = f {
            self.mem.w8(DS, 0x191F, a);
            if b != 0xFFFF {
                self.mem.w16(DS, 0x137A, b);
                self.mem.w8(DS, 0x137E, 0);
            }
        }
        self.mem.w16(DS, MUSIC, music);
        self.mem.w8(DS, KIND, kind);
        self.enter_building(files, hooks, dl)
    }

    /// 0000:42D2. The tavern: its room looked for, then its music is the
    /// square's (SQUARE.XMI) or the tavern's by a random word's sign.
    fn tavern(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks, dl: u8) -> Option<()> {
        hooks.tavern_room(self, files)?;
        self.mem.w8(DS, 0x191F, 1);
        self.mem.w16(DS, 0x137A, 2);
        self.mem.w8(DS, 0x137E, 0);
        let music = if (self.rng.next() as i16) < 0 { SQUARE_XMI } else { TAVERN_XMI };
        self.mem.w16(DS, MUSIC, music);
        self.mem.w8(DS, KIND, 4);
        self.enter_building(files, hooks, dl)
    }

    /// 0000:4192. A palace: outside the Imperial City (A46 not 8) a
    /// town's or village's (by A47), its number A53 mod 3; in it, by the
    /// main quest's stage (F77): 8 the audience, 9 the artifact's,
    /// otherwise "The Emperor has no time."
    fn palace(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks) -> Option<()> {
        if self.mem.r8(DS, 0x0A46) != 8 {
            self.mem.w8(DS, 0x191F, 1);
            self.mem.w16(DS, 0x137A, 0);
            self.mem.w8(DS, 0x137E, 0);
            self.mem.w16(DS, MUSIC, PALACE_XMI);
            self.mem.w8(DS, KIND, 1);
            let dl = (self.mem.r16(DS, 0x0A53) % 3) as u8;
            return self.enter_building(files, hooks, dl);
        }
        match self.mem.r8(DS, 0x0F77) {
            8 => hooks.palace_audience(self, files),
            9 => {
                self.mem.w16(DS, 0x137A, 0x5DD);
                self.mem.w8(DS, 0x101C, 0);
                self.mem.w8(DS, 0x137E, 0);
                hooks.artifact_audience(self, files)
            }
            _ => {
                hooks.message_box(self, 0x8888);
                Some(())
            }
        }
    }

    /// 0000:4054. Going into the building of kind KIND (see the module's
    /// notes), `dl` its number less 1.
    fn enter_building(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks, dl: u8) -> Option<()> {
        self.mem.w8(DS, 0x191D, 0);
        // 0000:506D: the interior's file, "%s%d.mif".
        let kind = self.mem.r8(DS, KIND);
        let mut name = kind as u16;
        let town = self.mem.r8(DS, 0x0A47);
        if name == 1 && town != 0 {
            name = if town == 1 { 9 } else { 10 };
        }
        let si = self.skip_strings(DS, 0x876E, name.wrapping_sub(1));
        use crate::format::Arg;
        self.sprintf((DS, INTERIOR), (DS, 0x871C), &[Arg::Str(DS, si), Arg::Int(dl as u16 + 1)]);
        // 0000:50A9: houses and nobles' are always closed.
        let closed = matches!(kind, 2 | 3) || self.building_closed(kind);
        if !closed {
            return self.go_in(files, hooks);
        }
        if kind == 1 {
            self.mem.w8(DS, 0x191F, 0);
            let v = self.mem.r16(DS, 0x1036);
            self.mem.w16(DS, 0xB038, v);
            let v = self.mem.r16(DS, 0x102C);
            self.mem.w16(DS, 0xB036, v);
            self.mem.w8(DS, 0x9288, 0x60);
            hooks.message_box(self, 0x88D0);
            return Some(());
        }
        if self.mem.r16(DS, 0x05CA) & 0x800 != 0 || self.open_spell(kind) {
            return self.broken_in(files, hooks);
        }
        // 0000:402F: the lock's level, 1..10 by the square.
        let saved = self.rng.seed;
        let sq = self.mem.r16(DS, PICKED) as u32;
        self.rng.seed = sq << 16 | sq;
        let level = self.rng.range(0, 0xA)? as u8 + 1;
        self.rng.seed = saved;
        self.mem.w8(DS, crate::click::var::LOCK_TRIED, level);
        self.mem.w8(DS, 0x191F, 0);
        if self.mem.r16(DS, 0x05CA) & 1 == 0 {
            return self.locked_message(level);
        }
        // 2542:028F
        let f = self.mem.r16(DS, 0x05CA);
        if f & 0xFFFE != 0 {
            self.mem.w16(DS, MESSAGE_TIME, 0);
            self.mem.w16(DS, 0x05CA, f & 0xFFFE);
        }
        let picked = self.mem.r16(DS, PICKED);
        let odds = if (0..0x20).any(|i| self.mem.r16(DS, PICK_FAILED + 2 * i) == picked) {
            0x32
        } else {
            let (ok, chance) = self.lockpick()?;
            if ok {
                return self.broken_in(files, hooks);
            }
            if let Some(i) = (0..0x20).find(|&i| self.mem.r16(DS, PICK_FAILED + 2 * i) == 0xFFFF) {
                self.mem.w16(DS, PICK_FAILED + 2 * i, picked);
            }
            chance
        };
        // 0000:3F6E: seen, under 100 less the odds.
        let roll = self.rng.next() % 100;
        if roll < 100u16.wrapping_sub(odds) {
            hooks.guards(self);
        } else {
            self.outcome(false);
        }
        Some(())
    }

    /// 0000:414E. An Open spell waiting (DC4) opens the building: 7D0h
    /// always, otherwise a d100 at most its power plus (the player's
    /// level byte 1AA less the kind) x 5; the spell is used either way.
    /// Chests' locks too (`talk`).
    pub(crate) fn open_spell(&mut self, kind: u8) -> bool {
        let power = self.mem.r16(DS, OPEN_SPELL);
        if power == 0 {
            return false;
        }
        if power == 0x7D0 {
            self.mem.w16(DS, OPEN_SPELL, 0);
            return true;
        }
        let d = self.mem.r8(DS, 0x01AA).wrapping_sub(kind) as i8 as i16;
        let bx = (d * 5) as u16;
        let bx = bx.wrapping_add(power);
        let roll = self.rng.next() % 100;
        self.mem.w16(DS, OPEN_SPELL, 0);
        roll as i16 <= bx as i16
    }

    /// 0000:40FD. A lock broken: its sound, "Success...", then in.
    fn broken_in(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks) -> Option<()> {
        let at = self.mem.r16(DS, 0xCB90);
        hooks.sound(self, 0x15, at);
        self.mem.w8(DS, 0x191F, 0);
        self.mem.w8(DS, 0x191D, 1);
        self.outcome(true);
        self.go_in(files, hooks)
    }

    /// 0000:4118. In: the break-in's mark cleared, the street's automap
    /// written back (23F1:0145), the area left (C398 bit 80h) for the
    /// interior (AREA 4, its file at D58) with the player's place kept, and
    /// the automap written again and let go (23F1:013C: C398 bit 4 still
    /// the street's, so it writes).
    fn go_in(&mut self, files: &mut Files, hooks: &mut dyn TriggerHooks) -> Option<()> {
        self.set_flag(0x05CA, 0, 0x800);
        hooks.sounds_off(self);
        hooks.blackout(self);
        hooks.automap_write(self, files);
        self.mem.w8(DS, AREA, 4);
        let (x, y) = (self.mem.r16(DS, X), self.mem.r16(DS, Y));
        self.mem.w16(DS, OUTSIDE_X, x);
        self.mem.w16(DS, OUTSIDE_Y, y);
        self.set_flag(0xC398, 0x80, 0);
        self.mem.w16(DS, 0x0D58, INTERIOR);
        hooks.automap_write(self, files);
        self.mem.w8(DS, 0x6636, 0);
        Some(())
    }
}
