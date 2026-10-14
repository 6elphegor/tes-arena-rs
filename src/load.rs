//! Loading a saved game (segment 25FD): after the player picks a slot
//! (25FD:0242, the menu), the game's state is put back and its world
//! rebuilt around it.
//!
//! In play (2542:061E, 5C8 bit 4 clear) the load (`game_load`) is:
//! the slot's digits in the save files' names, SAVEGAME looked for, the
//! light table made again (2294:0B8A), the slot's files copied to the
//! working ones (25FD:06A1), then SAVEGAME (`save_load_level`, 0000:A9A6,
//! and its world, `save_level_world`), SAVEENGN and the rebuild
//! (`save_restore`, 25FD:034B), and the area's entered hook (CD42).
//!
//! From the title screen (0000:119D sets 5C8 bit 4) the same runs in
//! another order: the hook after SAVEGAME's planes (BA76) is set to the
//! 3D mode (2294:0006), which runs the rebuild first, and the world loop
//! is entered afresh (0000:A232) to go back, when the area ends, to the
//! outer loop's code after that area's run (25FD:0539 sets it at BA7A).
//!
//! The rebuild after SAVEENGN (03F9..051E) puts back what the files
//! don't hold: the fight under way (1194:867E, `fight_restore`), the
//! effects' slots' segments (2EBD:054C), the spells (29CF:0691), the
//! province's data (1194:2E14), the 3D mode in play, the area as the
//! outer loop knows it (4BD0), the wilderness's grid and blocks, the
//! city's location record, the dungeon's automap, the level hooks
//! (0000:0B15 or 0B4F) and the working slot (1A80) again.
//!
//! What draws, plays or copies files is `LoadHooks`.

use crate::bsa::Files;
use crate::encounter::FightHooks;
use crate::light::LightHooks;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{LEVEL_FLAGS, MAP_FLAGS};
use crate::save::var::{SAVEGAME, SLOT};
use crate::transit::AreaHooks;
use crate::trigger::var::AREA;

/// DGROUP variables.
pub mod var {
    /// The working slot: the files the game reads and writes while it
    /// plays carry its digits (save slots are copied to and from it).
    pub const WORKING_SLOT: u16 = 0x1A80;
    /// "*.00": the working slot's files, its digits at +2; "*.00" again for
    /// the slot's (SLOT), its digits at +2.
    pub const WORKING_FILES: u16 = 0x5EAA;
    pub const SLOT_FILES: u16 = 0x5EB5;
    /// "wild001.00" (the slot's, digits at +8, the block's at +6) and
    /// "wild001.rmd" (the working copy, the block's digit at +6).
    pub const WILD_SAVED: u16 = 0x5EC6;
    pub const WILD_WORKING: u16 = 0x5EBA;
    /// Where the world loop goes back to when the area ends, as a far
    /// pointer (offset, segment): after a title-screen load, the outer
    /// loop's code after the area's run (0000:0D89 a city, 0DBF the
    /// wilderness, 0E32 a dungeon, 0EBB the start dungeon, 0E7D an
    /// interior).
    pub const RETURN: u16 = 0xBA7A;
    /// The SAVEGAME hooks: after the state block (BA72) and after the
    /// planes (BA76).
    pub const LEVEL_HOOK: u16 = 0xBA76;
    /// The level's INF, as the saved state names it.
    pub const SAVED_INF: u16 = 0xC2B5;
    /// The message shown when the slot is empty.
    pub const NO_SAVE: u16 = 0x5F0F;
}
use var::*;

/// What loading a game leaves to the front end and to code not ported.
pub trait LoadHooks: AreaHooks + FightHooks + LightHooks {
    /// 23BA:000C: the sound stopped.
    fn sound_off(&mut self, _m: &mut Machine) {}
    /// 0000:600C: whether the file at DS:`path` exists (DOS find first).
    /// The default asks `files`.
    fn exists(&mut self, m: &mut Machine, files: &mut Files, path: u16) -> bool {
        m.file_exists(files, path)
    }
    /// 1194:59C8 with the palette set (102C, 1036): the message at DS:`si`
    /// (the slot is empty).
    fn message(&mut self, _m: &mut Machine, _si: u16) {}
    /// 23F1:0178 (in 013C, which then clears 6636): the current area's
    /// automap written back.
    fn automap_keep(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 25FD:0803: "Loading..." shown.
    fn loading(&mut self, _m: &mut Machine) {}
    /// 25FD:06A1: the slot's files (*.nn) copied to the working ones. The
    /// default copies through `files` (`Machine::slot_files_in`).
    fn slot_files(&mut self, m: &mut Machine, files: &mut Files) {
        m.slot_files_in(files);
    }
    /// 242A:000E: CITYDATA.
    fn citydata(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 0000:A5D1: the view's screen set up.
    fn screen(&mut self, _m: &mut Machine) {}
    /// 0000:DA8E: the view updated.
    fn view_update(&mut self, _m: &mut Machine) {}
    /// 0000:E5C9: outdoors, the sky's images loaded into the EMS bank
    /// C8F3.
    fn sky_graphics(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 0000:AAA2: the renderer's set-up (the light table at CCC2 made).
    fn renderer(&mut self, _m: &mut Machine) {}
    /// 25FD:056F: the file at DS:`from` copied to DS:`to` (deleted, made
    /// again). The default copies through `files`.
    fn copy_file(&mut self, m: &mut Machine, files: &mut Files, from: u16, to: u16) {
        m.file_copy(files, from, to);
    }
    /// 292B:09AC: the level's automap (STATES.nn) loaded.
    fn level_automap(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 25FD:051F: in an interior, 2F9F:001F with the text at DS:[FF0].
    fn interior_text(&mut self, _m: &mut Machine) {}
    /// 2294:0006, the 3D mode in play: true where the hook did it (the
    /// vectors stub it); by default not, and `Machine::mode_3d` runs.
    fn mode_3d(&mut self, _m: &mut Machine) -> bool {
        false
    }
}

/// `LoadHooks` that do nothing (and copy through `Files`).
pub struct NoLoadHooks;

impl AreaHooks for NoLoadHooks {}
impl FightHooks for NoLoadHooks {}
impl LightHooks for NoLoadHooks {}
impl LoadHooks for NoLoadHooks {}

/// How a load went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loaded {
    /// The game was put back; play goes on in its area.
    Done,
    /// The slot's SAVEGAME isn't there: the message was shown and the
    /// menu comes back.
    Missing,
}

impl Machine {
    fn load_or16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) | v;
        self.mem.w16(DS, at, x);
    }

    fn load_and16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) & v;
        self.mem.w16(DS, at, x);
    }

    /// The two digits of `slot` as the file names carry them.
    fn slot_digits(slot: u8) -> u16 {
        (((slot % 10) as u16) << 8 | (slot / 10) as u16).wrapping_add(0x3030)
    }

    /// 25FD:028D..0349: a game loaded from save slot `slot` in play (5C8
    /// bit 4 clear; see the module's notes): the slot set (1A6C and the
    /// digits of SAVEGAME, SAVEENGN and WILD001), the sound stopped; with
    /// no SAVEGAME, the message (`Loaded::Missing`). Otherwise 2F9F:0438,
    /// the automap kept (23F1:013C), the light table made again (2294:0B8A), and,
    /// unless 5C8 bit 4, the light table's block freed (CCC2 0); then
    /// "Loading...", the working slot's digits set (25FD:022A), the slot's
    /// files copied, CITYDATA, SAVEGAME and its world, SAVEENGN with the
    /// rebuild, 5CA bit 4000h (just loaded) and the hook CD42 (0000:468B,
    /// `AreaHooks::entered`). None where a part fails (a file missing,
    /// the heap), or 5C8 bit 4 is set (the title screen's way, which
    /// enters the world loop afresh: not ported).
    pub fn game_load(&mut self, files: &mut Files, slot: u8, hooks: &mut dyn LoadHooks) -> Option<Loaded> {
        self.save_set_slot(slot);
        hooks.sound_off(self);
        let path = self.save_path(SAVEGAME);
        if !hooks.exists(self, files, path) {
            hooks.message(self, NO_SAVE);
            return Some(Loaded::Missing);
        }
        hooks.sync(self);
        hooks.automap_keep(self, files);
        self.mem.w8(DS, 0x6636, 0);
        self.light_table_reset(hooks);
        let title = self.mem.r16(DS, 0x05C8) & 4 != 0;
        if !title {
            let t = self.mem.r16(DS, 0xCCC2);
            self.heap_free(t);
            self.mem.w16(DS, 0xCCC2, 0);
        }
        hooks.loading(self);
        self.working_files_digits();
        hooks.slot_files(self, files);
        hooks.citydata(self, files);
        if title {
            return None;
        }
        self.save_load_level(files)?;
        self.save_level_world(files, hooks)?;
        self.save_restore(files, hooks)?;
        self.load_or16(0x05CA, 0x4000);
        if self.mem.r32(DS, crate::transit::var::ENTERED_HOOK) != 0x468B {
            return None;
        }
        hooks.entered(self);
        Some(Loaded::Done)
    }

    /// 25FD:06A1. The slot's files ("*.nn", the slot at SLOT, its digits
    /// at SLOT_FILES +2) copied to the working slot's: each name up to its
    /// '.' and then the working slot's digits (WORKING_SLOT). The files are
    /// found through `Files` (DOS find first/next, 0000:600C, 6028: in
    /// directory order there; the copies don't depend on it). The last
    /// file's names stay at 1044 (found) and 1051 (made), as the original
    /// leaves them; its DTA pointer (92A2) isn't kept.
    pub fn slot_files_in(&mut self, files: &mut Files) {
        let slot = self.mem.r8(DS, SLOT);
        let w = self.mem.r8(DS, WORKING_SLOT);
        self.mem.w16(DS, SLOT_FILES + 2, Self::slot_digits(slot));
        self.copy_slot_files(files, slot, w);
    }

    /// 25FD:0612. The working slot's files ("*.ww", digits at
    /// WORKING_FILES +2) copied to the slot's (SLOT), as `slot_files_in`
    /// the other way; 5E42 is set meanwhile and 929E holds the slot's
    /// digits. Returns the names written (to keep them, a front end
    /// writes them out).
    pub fn slot_files_out(&mut self, files: &mut Files) -> Vec<Vec<u8>> {
        let slot = self.mem.r8(DS, SLOT);
        let w = self.mem.r8(DS, WORKING_SLOT);
        self.mem.w8(DS, 0x5E42, 1);
        self.mem.w16(DS, WORKING_FILES + 2, Self::slot_digits(w));
        self.mem.w16(DS, 0x929E, Self::slot_digits(slot));
        self.mem.w8(DS, 0x92A0, 0);
        let out = self.copy_slot_files(files, w, slot);
        self.mem.w8(DS, 0x5E42, 0);
        out
    }

    /// The copies of 06A1 and 0612: each "*.`from`" to the same name with
    /// `to`'s digits. Returns the names made.
    fn copy_slot_files(&mut self, files: &mut Files, from: u8, to: u8) -> Vec<Vec<u8>> {
        let ext = Self::slot_digits(from).to_le_bytes();
        let digits = Self::slot_digits(to).to_le_bytes();
        let mut made = Vec::new();
        for name in files.with_ext(&ext) {
            let stem = &name[..name.iter().rposition(|&c| c == b'.').unwrap_or(name.len())];
            let mut new = stem.to_vec();
            new.push(b'.');
            new.extend_from_slice(&digits);
            for (at, s) in [(0x1044u16, &name), (0x1051, &new)] {
                for (i, &c) in s.iter().chain(&[0]).enumerate() {
                    self.mem.w8(DS, at + i as u16, c);
                }
            }
            if files.copy(&name, &new) {
                made.push(new);
            }
        }
        made
    }

    /// 25FD:022A: the working slot's digits in "*.00" (WORKING_FILES +2).
    pub fn working_files_digits(&mut self) {
        let slot = self.mem.r8(DS, WORKING_SLOT);
        self.mem.w16(DS, WORKING_FILES + 2, Self::slot_digits(slot));
    }

    /// 0000:AA4B..AA5C, the end of SAVEGAME's load (after
    /// `save_load_level`): the view's screen, the level's INF (named in
    /// the saved state, SAVED_INF) loaded, the view updated and outdoors
    /// the sky's images. None where the INF load fails.
    pub fn save_level_world(&mut self, files: &mut Files, hooks: &mut dyn LoadHooks) -> Option<()> {
        hooks.screen(self);
        self.info_load(files, SAVED_INF)?;
        hooks.view_update(self);
        hooks.sky_graphics(self, files);
        Some(())
    }

    /// 25FD:034B..051E. SAVEENGN read back and the game rebuilt around
    /// it: the spells' state cleared (29CF:0CEF), the moving lights'
    /// list (D330, 80h words) and the lights' records' +4 (80h of 6
    /// bytes from D43C) cleared (0000:C015), the 17h actor records at
    /// 5F6A made inactive (0000:27AE); SAVEENGN (`save_read_engine`);
    /// then (03F9):
    /// - the fight under way (`fight_restore`), the effects' slots
    ///   (`fix_slot_segments`), the spells (`spells_load`), the province
    ///   (A46) loaded (`city_load_province`), the sky's images;
    /// - in play (5C8 bit 4 clear) the 3D mode (`mode_3d`);
    /// - CB5C bit 80h clear, ENTERED (4BD0) the area; with no light
    ///   table (CCC2 0) and not full daylight, the renderer's set-up;
    /// - in the wilderness (C39A bit 200h): the area 1, its grid made
    ///   (217B:0004, `wild_generate`), and its four blocks' saved files
    ///   copied to the working ones (WILD00n.nn to WILD00n.RMD);
    /// - outdoors not in the wilderness, the city's location record
    ///   (1194:3AE8, A4C);
    /// - 5C8 bit 800h, the level's automap (292B:09AC); from the title
    ///   screen (5C8 bit 4) the world loop's way back (25FD:0539);
    /// - outdoors 5C8 bit 800h clear again; indoors the level hooks, the
    ///   wilderness dungeon's (0000:0B4F) for the start dungeon with 5E4
    ///   bit 1 or with 5C8 bit 80h, else 0B15;
    /// - the slot back to the working slot, an interior's text (25FD:051F),
    ///   6636 0, the automap (0000:3503), 5E0 bit 200h clear, 1404 0Ah, 191D
    ///   5E0 bit 80h.
    ///
    /// None where SAVEENGN is missing or a part fails.
    pub fn save_restore(&mut self, files: &mut Files, hooks: &mut dyn LoadHooks) -> Option<()> {
        self.spells_clear();
        for i in 0..0x80u16 {
            self.mem.w16(DS, 0xD330 + 2 * i, 0);
            self.mem.w16(DS, 0xD43C + 6 * i + 4, 0);
        }
        for i in 0..0x17u16 {
            self.load_or16(0x5F6A + 0x1C * i + 0xF, 0x4000);
        }
        self.save_read_engine(files)?;
        self.save_rebuild(files, hooks)
    }

    /// 25FD:03F9..051E, the rebuild after SAVEENGN (see `save_restore`).
    pub fn save_rebuild(&mut self, files: &mut Files, hooks: &mut dyn LoadHooks) -> Option<()> {
        self.fight_restore(files, hooks)?;
        self.fix_slot_segments();
        self.spells_load(files)?;
        let province = self.mem.r8(DS, 0x0A46);
        self.city_load_province(files, province)?;
        hooks.sky_graphics(self, files);
        let title = self.mem.r16(DS, 0x05C8) & 4 != 0;
        if !title && !hooks.mode_3d(self) {
            self.mode_3d(hooks);
        }
        let v = self.mem.r8(DS, 0xCB5C) & 0x7F;
        self.mem.w8(DS, 0xCB5C, v);
        let area = self.mem.r8(DS, AREA);
        self.mem.w8(DS, crate::transit::var::ENTERED, area);
        if self.mem.r16(DS, 0xCCC2) == 0 && self.mem.r16(DS, MAP_FLAGS) & 0x8000 == 0 {
            hooks.renderer(self);
        }
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            self.mem.w8(DS, AREA, 1);
            self.mem.w8(DS, crate::transit::var::ENTERED, 1);
            self.wild_generate()?;
            self.mem.w8(DS, WILD_SAVED + 6, b'1');
            self.mem.w8(DS, WILD_WORKING + 6, b'1');
            for _ in 0..4 {
                let from = self.save_path(WILD_SAVED);
                let to = self.save_path(WILD_WORKING);
                hooks.copy_file(self, files, from, to);
                for at in [WILD_WORKING + 6, WILD_SAVED + 6] {
                    let c = self.mem.r8(DS, at).wrapping_add(1);
                    self.mem.w8(DS, at, c);
                }
            }
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 && self.mem.r16(DS, LEVEL_FLAGS) & 0x200 == 0 {
            let id = self.mem.r8(DS, 0x0A4C) as u16;
            hooks.location(self, id);
        }
        self.load_or16(0x05C8, 0x800);
        hooks.level_automap(self, files);
        if title {
            self.load_return();
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
            self.load_and16(0x05C8, 0xF7FF);
        } else {
            let start = self.mem.r8(DS, AREA) == 2 && self.mem.r8(DS, 0x05E4) & 1 != 0;
            let wild = start || self.mem.r16(DS, 0x05C8) & 0x80 != 0;
            self.level_hooks(if wild { 0x44BD } else { 0x4537 });
        }
        let w = self.mem.r8(DS, WORKING_SLOT);
        self.mem.w8(DS, SLOT, w);
        if self.mem.r8(DS, AREA) == 4 {
            hooks.interior_text(self);
        }
        self.mem.w8(DS, 0x6636, 0);
        hooks.border(self);
        self.load_and16(0x05E0, 0xFDFF);
        self.mem.w8(DS, 0x1404, 0x0A);
        let b = (self.mem.r16(DS, 0x05E0) & 0x80 != 0) as u8;
        self.mem.w8(DS, 0x191D, b);
        Some(())
    }

    /// 25FD:0539: the world loop's way back after a title-screen load,
    /// the outer loop's code after the area's run (RETURN, segment 0).
    pub fn load_return(&mut self) {
        let off = match self.mem.r8(DS, AREA) {
            0 => 0x0D89,
            1 => 0x0DBF,
            3 => 0x0E32,
            2 => 0x0EBB,
            _ => 0x0E7D,
        };
        self.mem.w16(DS, RETURN, off);
        self.mem.w16(DS, RETURN + 2, 0);
    }

    /// 1194:867E. The fight saved with the game set up again, from the
    /// first creature record (at 0 in the creature segment, 1042): its
    /// type (type 2Ah as 18h, its extra data built, 1194:8ED4) picks the
    /// music (100D, FFh for a person: record +40Dh bit 1000h clear) and
    /// the flyers' height (FF3); no fight is on (FF9 0); the fight's text
    /// (1194:8B12) and its set-up (25CA:0000) follow; FFB is whether any
    /// of the eight creature slots (from 0C57) is in use.
    pub fn fight_restore(&mut self, files: &mut Files, hooks: &mut dyn FightHooks) -> Option<()> {
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        let flags = self.mem.r16(seg, 0x40D);
        let mut bx = self.mem.r8(seg, 4) as u16;
        if bx == 0x2A {
            self.cr_type_2a(seg, 0);
            bx = 0x18;
        }
        let bx = bx.wrapping_sub(1);
        self.mem.w8(DS, 0x100D, 0xFF);
        if flags & 0x1000 != 0 {
            let v = self.mem.r8(DS, 0x2BF2u16.wrapping_add(bx));
            self.mem.w8(DS, 0x100D, v);
        }
        let ax = self.mem.r8(DS, 0x2C0Au16.wrapping_add(bx)) as i8 as u16;
        self.mem.w16(DS, 0x0FF3, ax);
        self.mem.w16(DS, 0x0FF9, 0);
        let people = self.encounter_text(ax);
        self.fight_prepare(files, hooks, people as u8)?;
        let any = (0..8u16).any(|i| self.mem.r16(DS, 0x0C57 + 0x1C * i + 0xF) & 0x4000 == 0);
        self.mem.w16(DS, 0x0FFB, any as u16);
        Some(())
    }
}
