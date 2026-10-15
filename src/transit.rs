//! Moving between areas: the game's outer loop (0000:0CAD) and the
//! routines that enter and leave an area around the world loop.
//!
//! The 3D world runs inside an area: a city (0), the wilderness (1), the
//! start dungeon (2), a dungeon (3) or an interior (4), the byte at 5E6
//! (`trigger::var::AREA`). An area ends when C398 bit 80h is set (a
//! wall's trigger, `Machine::wall_trigger`; the way out of a level, the
//! exit hook CD4E, `area_exit`; death and the like). The outer loop then
//! goes on after the area it ran (0000:0ED5 dispatches by 4BD0, the area
//! as it was entered), and picks the next by 5E6 (`area_next`):
//!
//! - a city: its location record built (1194:3AE8), the weather set
//!   (0000:0F05), the city built (1194:0292), then run (0000:0A83);
//! - the wilderness: likewise, its chunks set up (217B:0084), run
//!   (0000:090A);
//! - a dungeon: its seed (91F6) from the location's (A56), plus the
//!   wilderness dungeon's place when 5C8 bit 80h, made (2294:0ED4,
//!   0000:28BC), run (0000:09FA);
//! - an interior, when one is named (D58, the name's offset, taken):
//!   run (0000:0936); otherwise the start dungeon (0000:0993).
//!
//! An area's run (`area_begin`, the world loop, `area_end`) sets its
//! kind's state and the level hooks (CD32, CD4E, CD52: 0000:0B15 or
//! 0B4F), then runs the 3D world (0000:0AA9: `world_begin`, the world
//! loop 0000:A26F, `world_end`). Leaving an interior, the outer loop
//! goes back to the city (or the wilderness, 5C8 bit 4000h), where the
//! player comes out at the door (FCE, FD0: `area_place`, the hook CD2A).
//!
//! What draws, loads files or asks the player is `AreaHooks`.

use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{LEVEL, LEVEL_FLAGS, MAP_FLAGS, X, Y};
use crate::trigger::var::{AREA, OUTSIDE_X, OUTSIDE_Y};

/// DGROUP variables.
pub mod var {
    /// The area as the outer loop entered it.
    pub const ENTERED: u16 = 0x4BD0;
    /// The interior to enter (its name's offset; 0 none).
    pub const INTERIOR: u16 = 0x0D58;
    /// The level hooks an area sets: after the INF loads (CD32), the
    /// way out (CD4E), and CD52.
    pub const INF_HOOK: u16 = 0xCD32;
    pub const EXIT_HOOK: u16 = 0xCD4E;
    pub const HOOK_CD52: u16 = 0xCD52;
    /// The level's name as the world loop loads it ('#' first: none).
    pub const LEVEL_NAME: u16 = 0xC37F;
    /// The hooks of entering the 3D world: C3B0 (0000:4632), CD2A
    /// (0000:45B2, `area_place`), CD42 (0000:468B).
    pub const ENTRY_HOOK: u16 = 0xC3B0;
    pub const PLACE_HOOK: u16 = 0xCD2A;
    pub const ENTERED_HOOK: u16 = 0xCD42;
}
use var::*;

/// The area runs `area_next` picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    /// The game is left (5CA bit 8000h).
    Quit,
    /// The main quest is over (5B1 bit 100h): the ending (1194:D459)
    /// was shown, unless F77 is 9.
    End,
    /// 0000:0A83, the city built.
    City,
    /// 0000:090A, the wilderness set up.
    Wild,
    /// 0000:09FA, the dungeon made.
    Dungeon,
    /// 0000:0936, the interior whose .MIF is named at DS:offset.
    Interior(u16),
    /// 0000:0993, the start dungeon (its name at 0A36).
    Start,
}

/// What the area changes leave to the front end and to code not ported.
pub trait AreaHooks {
    /// 2F9F:0438: the song playing faded out and stopped (the AIL call;
    /// by default `Song::Stop` on `Machine::songs`).
    fn sync(&mut self, m: &mut Machine) {
        m.music_stop();
    }
    /// 1194:D459: the ending, its text set (AX, SI, DI) and the view
    /// cleared to the colour at 9288.
    fn ending(&mut self, _m: &mut Machine, _ax: u16, _si: u16, _di: u16) {}
    /// 0000:742C: NORMAL.LGT (named at 1A6D) loaded to 3AAC:0240.
    fn normal_lgt(&mut self, _m: &mut Machine) {}
    /// 1194:3AE8: the location record for location `id` (100h the
    /// Imperial City) built (`Machine::city_build_location`).
    fn location(&mut self, _m: &mut Machine, _id: u16) {}
    /// 0000:0F05: the weather set (`Machine::weather_set`).
    fn weather(&mut self, _m: &mut Machine) {}
    /// 1194:0292: the city built (`Machine::mif_build_city`).
    fn build_city(&mut self, _m: &mut Machine) {}
    /// 217B:0084: the wilderness set up.
    fn wild_setup(&mut self, _m: &mut Machine) {}
    /// 217B:04DC: after the wilderness.
    fn wild_done(&mut self, _m: &mut Machine) {}
    /// 2294:0ED4 and 0000:28BC (AX 1, BX `wild`): the dungeon made.
    fn build_dungeon(&mut self, _m: &mut Machine, _wild: bool) {}
    /// 0000:17F9: the townspeople's graphics loaded.
    fn townsfolk_graphics(&mut self, _m: &mut Machine) {}
    /// 2294:11F2, around a city's run.
    fn city_screen(&mut self, _m: &mut Machine) {}
    /// 292B:095E: a main quest dungeon's level's locks and triggers kept
    /// in STATES.nn (`Machine::states_keep`, which needs the files).
    fn automap(&mut self, _m: &mut Machine) {}
    /// 26F7:0000, asked on the way out of a dungeon: whether to leave
    /// (the view saved and put back around it, 0000:A611/A606/A5D1, and
    /// redrawn after, A6B5, 2542:0446). The default stays.
    fn ask_leave(&mut self, _m: &mut Machine) -> bool {
        false
    }
    /// The screen after leaving (0000:9426, 9466).
    fn leave_screen(&mut self, _m: &mut Machine) {}
    /// 1194:D55F and 2433:0199, leaving an interior.
    fn leave_interior(&mut self, _m: &mut Machine) {}
    /// 2433:0199, leaving a dungeon.
    fn leave_dungeon(&mut self, _m: &mut Machine) {}
    /// 0000:1793 with BX 0Ah: `n` townspeople put around the player
    /// (`Machine::spawn_actors(n, 0xA)`).
    fn spawn(&mut self, _m: &mut Machine, _n: u16) {}
    /// 0000:464A..467C: the sky's objects made (`Machine::area_sky`), or
    /// where that answers false the palette named at 1304 loaded.
    fn sky(&mut self, _m: &mut Machine) {}
    /// 0000:2974.
    fn hook_2974(&mut self, _m: &mut Machine) {}
    /// 217B:0184 then 217B:0058: whether the wilderness has no
    /// townspeople here (the carry).
    fn wild_empty(&mut self, _m: &mut Machine) -> bool {
        false
    }
    /// 0000:8FC4: the mouse's buttons waited for (AFF9 clear).
    fn mouse_wait(&mut self, _m: &mut Machine) {}
    /// 0000:3503: the automap loaded (`Machine::automap_load`, which
    /// needs the files).
    fn border(&mut self, _m: &mut Machine) {}
    /// 25FD:034B and 0000:16F7 (5C8 bit 4: a saved game's level).
    fn restore_level(&mut self, _m: &mut Machine) {}
    /// 25CA:01E6: the music's number.
    fn music(&mut self, _m: &mut Machine) -> u16 {
        0
    }
    /// 0000:068A: the hands' graphics.
    fn hands(&mut self, _m: &mut Machine) {}
    /// 0000:2631.
    fn hook_2631(&mut self, _m: &mut Machine) {}
    /// 0000:5D70: EMS banks freed.
    fn ems_free(&mut self, _m: &mut Machine) {}
    /// After the world loop (0000:0AD7..0B07): 23F1:013C, the palette
    /// faded to black (5E3E), 4EFD and 946A.
    fn world_screen(&mut self, _m: &mut Machine) {}
    /// 0000:A5A0: the renderer's buffers freed (CD78, CD84, CD80, CD88,
    /// the buffer at BA56), 0000:DE9B and AB2A.
    fn world_free(&mut self, _m: &mut Machine) {}
    /// 0000:468B, the hook CD42 once the INF is loaded: the flats' flags,
    /// outdoors the light (0000:0390) and the city's people, indoors the
    /// level's lights from its flats (0000:478F, `Machine::flat_lights`)
    /// and the interior's people (2433:0000).
    fn entered(&mut self, _m: &mut Machine) {}
}

/// `AreaHooks` that do nothing.
pub struct NoAreaHooks;

impl AreaHooks for NoAreaHooks {}

impl Machine {
    fn or16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) | v;
        self.mem.w16(DS, at, x);
    }

    fn and16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) & v;
        self.mem.w16(DS, at, x);
    }

    /// 0000:0ED5 then 0000:0CAD: after the area entered as ENTERED (its
    /// run returned), the outer loop's next area. Each area's tail:
    /// - the wilderness (DC2): 217B:04DC; an interior entered from it
    ///   marks 5C8 bit 4000h; out of the wilderness (C39A bit 200h);
    /// - a dungeon (E35): to the wilderness (1) if it was one there (5C8
    ///   bit 80h), else as 5E6 says, the wilderness becoming a city; then
    ///   5C8 bits 4000h, 80h and C39A bit 800h clear, and into the
    ///   wilderness C39A bit 2000h and no outside place (FCE);
    /// - the start dungeon (EBE): 5C8 bit 80h clear, a city next with
    ///   5E4 bit 1;
    /// - an interior (or anything else, E80): 2F9F:0438, then the city,
    ///   or the wilderness when entered from it (5C8 bit 4000h, cleared;
    ///   C39A bit 2000h).
    ///
    /// Then (0CAD) the area's state reset (AB2, BC8C, 5CA bit 200h, 5E4
    /// bit 1, the level BB0A), ENTERED set to 5E6, 2F9F:0438, D56 FFFFh;
    /// out on 5CA bit 8000h (`Run::Quit`) or 5B1 bit 100h (`Run::End`);
    /// NORMAL.LGT loaded, and the next area by 5E6 (see the module's
    /// notes).
    pub fn area_next(&mut self, hooks: &mut dyn AreaHooks) -> Run {
        match self.mem.r8(DS, ENTERED) {
            0 => {}
            1 => {
                hooks.wild_done(self);
                if self.mem.r8(DS, AREA) == 4 {
                    self.or16(0x05C8, 0x4000);
                }
                self.and16(LEVEL_FLAGS, 0xFDFF);
            }
            3 => {
                let mut al = 1;
                if self.mem.r16(DS, 0x05C8) & 0x80 == 0 {
                    al = self.mem.r8(DS, AREA);
                    if al == 1 {
                        al = 0;
                    }
                }
                self.mem.w8(DS, AREA, al);
                self.and16(0x05C8, 0xBF7F);
                self.and16(LEVEL_FLAGS, 0xF7FF);
                if al == 1 {
                    self.or16(LEVEL_FLAGS, 0x2000);
                    self.mem.w16(DS, OUTSIDE_X, 0);
                }
            }
            2 => {
                self.and16(0x05C8, 0xFF7F);
                if self.mem.r8(DS, 0x05E4) & 1 != 0 {
                    self.mem.w8(DS, AREA, 0);
                }
            }
            _ => {
                hooks.sync(self);
                let mut al = 0;
                if self.mem.r16(DS, 0x05C8) & 0x4000 != 0 {
                    al = 1;
                    self.and16(0x05C8, 0xBFFF);
                }
                self.mem.w8(DS, AREA, al);
                if al == 1 {
                    self.or16(LEVEL_FLAGS, 0x2000);
                }
            }
        }
        self.mem.w8(DS, 0x0AB2, 0xFF);
        self.mem.w16(DS, 0xBC8C, 0);
        self.and16(0x05CA, 0xFDFF);
        let v = self.mem.r8(DS, 0x05E4) & 0xFE;
        self.mem.w8(DS, 0x05E4, v);
        self.mem.w8(DS, LEVEL, 0);
        let area = self.mem.r8(DS, AREA);
        self.mem.w8(DS, ENTERED, area);
        hooks.sync(self);
        self.mem.w16(DS, 0x0D56, 0xFFFF);
        if self.mem.r16(DS, 0x05CA) & 0x8000 != 0 {
            return Run::Quit;
        }
        if self.mem.r16(DS, 0x05B1) & 0x100 != 0 {
            for (at, v) in [(0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0xC7)] {
                self.mem.w16(DS, at, v);
            }
            let (ax, si, di) = if self.mem.r8(DS, 0x0F77) != 0 {
                self.mem.w8(DS, 0x9288, 0xF5);
                (0x57B, 0x80A5, 0x8090)
            } else {
                self.mem.w8(DS, 0x9288, 0x2F);
                (0x57A, 0x809A, 0x8085)
            };
            if self.mem.r8(DS, 0x0F77) != 9 {
                self.mem.w8(DS, 0x803C, 0);
                self.or16(0xA902, 0x200);
                self.and16(0xA902, 0xFEFF);
                hooks.ending(self, ax, si, di);
            }
            return Run::End;
        }
        hooks.normal_lgt(self);
        let location = |m: &mut Machine| {
            if m.mem.r8(DS, 0x0A46) == 8 { 0x100 } else { m.mem.r8(DS, 0x0A4C) as u16 }
        };
        match self.mem.r8(DS, AREA) {
            0 => {
                self.mem.w8(DS, 0x0A47, 0);
                self.and16(0x05C8, 0xFDFF);
                let id = location(self);
                hooks.location(self, id);
                hooks.weather(self);
                hooks.build_city(self);
                Run::City
            }
            1 => {
                let id = location(self);
                hooks.location(self, id);
                hooks.weather(self);
                self.or16(LEVEL_FLAGS, 0x200);
                hooks.wild_setup(self);
                Run::Wild
            }
            3 => {
                let mut seed = self.mem.r32(DS, 0x0A56);
                if self.mem.r16(DS, 0x05C8) & 0x80 != 0 {
                    // movzx eax, [60A]; shl ax, 6; add ax, [608]: 16-bit.
                    let ax = (self.mem.r16(DS, 0x060A) << 6).wrapping_add(self.mem.r16(DS, 0x0608));
                    seed = (ax as u32).wrapping_add(seed);
                    self.or16(0x05C8, 0x4000);
                }
                self.mem.w32(DS, 0x91F6, seed);
                self.and16(LEVEL_FLAGS, 0xFDFF);
                let wild = self.mem.r16(DS, 0x05C8) & 0x4000 != 0;
                hooks.build_dungeon(self, wild);
                Run::Dungeon
            }
            _ => {
                let name = self.mem.r16(DS, INTERIOR);
                if name != 0 {
                    self.mem.w16(DS, INTERIOR, 0);
                    return Run::Interior(name);
                }
                self.and16(MAP_FLAGS, 0x7FFB);
                Run::Start
            }
        }
    }

    /// 0000:0B15 (`exit` 4537h) and 0B4F (`exit` 44BDh): the level
    /// hooks: after the INF, 0000:2957 in a dungeon (5E6 3), else 0B0B
    /// (the level's lights from its flats); the way out; CD52 0000:0628.
    pub(crate) fn level_hooks(&mut self, exit: u16) {
        let inf = if self.mem.r8(DS, AREA) == 3 { 0x2957 } else { 0x0B0B };
        self.mem.w32(DS, INF_HOOK, inf);
        self.mem.w32(DS, EXIT_HOOK, exit as u32);
        self.mem.w32(DS, HOOK_CD52, 0x0628);
    }

    /// 0000:0997, the start dungeon's run after its level hooks (0993
    /// sets 0B15's first; a new game's start, 0000:0C66, 0B4F's):
    /// NORMAL.LGT, the indoor state, no INF, 427D FFh, 430E 0. Returns the
    /// level's name (0A36).
    pub fn start_run(&mut self, hooks: &mut dyn AreaHooks) -> u16 {
        hooks.normal_lgt(self);
        self.indoor_state();
        self.mem.w16(DS, crate::info::var::INFO_PTR, 0);
        self.mem.w8(DS, 0x427D, 0xFF);
        self.mem.w8(DS, 0x430E, 0);
        0x0A36
    }

    /// 0000:0B4F: the level hooks with the way out 44BDh.
    pub(crate) fn start_level_hooks(&mut self) {
        self.level_hooks(0x44BD);
    }

    /// The state an indoor run sets (0000:0939.., 099A.., 09FD..): no fog,
    /// C8FC 0Dh, C8FD 2, 5C8 bit 800h (entering), indoors (C398 bits
    /// 8000h, 4 clear), not the wilderness, the fill colours 70h.
    fn indoor_state(&mut self) {
        self.mem.w8(DS, 0x0F81, 0);
        self.mem.w8(DS, 0xC8FC, 0xD);
        self.mem.w8(DS, 0xC8FD, 2);
        self.or16(0x05C8, 0x800);
        self.and16(MAP_FLAGS, 0x7FFB);
        self.and16(LEVEL_FLAGS, 0xFDFF);
        self.mem.w8(DS, 0xC3C6, 0x70);
        self.mem.w8(DS, 0xC3C7, 0x70);
    }

    /// An area's run up to its world (0000:0AA9): what `run` sets.
    /// Returns the level's name for `world_begin` (DS offset).
    /// - an interior (0936): NORMAL.LGT, the indoor state, the level
    ///   hooks (0B15), no INF (CB84);
    /// - the start dungeon (0993): the level hooks, NORMAL.LGT, the indoor
    ///   state, no INF, 427D FFh, 430E 0;
    /// - a dungeon (09FA): NORMAL.LGT, the indoor state, the level hooks
    ///   (0B4F when it is the wilderness's, 5C8 bit 80h), then the INF
    ///   hook 0000:2957 and CD52 0628 with this segment (0 here), the INF
    ///   at 4718h, 427D FFh, 430E 0;
    /// - a city (0A83): 5C8 bits 200h..800h clear, C8FC 0Dh, C8FD 1,
    ///   outdoors (C398 bit 4), its INF (`outdoor_inf`; the townspeople's
    ///   graphics), 2294:11F2;
    /// - the wilderness (090A): 7A46 0, 5C8 bits 200h, 400h clear, C8FC
    ///   0Dh, C8FD 1, C39A bit 200h, outdoors, its INF.
    pub fn area_begin(&mut self, run: Run, hooks: &mut dyn AreaHooks) -> u16 {
        match run {
            Run::Interior(name) => {
                hooks.normal_lgt(self);
                self.indoor_state();
                self.level_hooks(0x4537);
                self.mem.w16(DS, crate::info::var::INFO_PTR, 0);
                name
            }
            Run::Start => {
                self.level_hooks(0x4537);
                self.start_run(hooks)
            }
            Run::Dungeon => {
                hooks.normal_lgt(self);
                self.indoor_state();
                let exit = if self.mem.r16(DS, 0x05C8) & 0x80 == 0 { 0x4537 } else { 0x44BD };
                self.level_hooks(exit);
                self.mem.w32(DS, INF_HOOK, 0x2957);
                self.mem.w16(DS, crate::info::var::INFO_PTR, 0x4718);
                self.mem.w32(DS, HOOK_CD52, 0x0628);
                self.mem.w8(DS, 0x427D, 0xFF);
                self.mem.w8(DS, 0x430E, 0);
                0x1385
            }
            Run::City => {
                self.and16(0x05C8, 0xF1FF);
                self.mem.w8(DS, 0xC8FC, 0xD);
                self.mem.w8(DS, 0xC8FD, 1);
                self.or16(MAP_FLAGS, 4);
                if self.outdoor_inf() {
                    hooks.townsfolk_graphics(self);
                }
                hooks.city_screen(self);
                0x1385
            }
            Run::Wild => {
                self.mem.w16(DS, 0x7A46, 0);
                self.and16(0x05C8, 0xF9FF);
                self.mem.w8(DS, 0xC8FC, 0xD);
                self.mem.w8(DS, 0xC8FD, 1);
                self.or16(LEVEL_FLAGS, 0x200);
                self.or16(MAP_FLAGS, 4);
                if self.outdoor_inf() {
                    hooks.townsfolk_graphics(self);
                }
                0x1319
            }
            Run::Quit | Run::End => 0,
        }
    }

    /// An area's run after its world: indoors the level hooks cleared
    /// and 5C8 bit 800h; a city's 2294:11F2 again.
    pub fn area_end(&mut self, run: Run, hooks: &mut dyn AreaHooks) {
        match run {
            Run::Interior(_) | Run::Start | Run::Dungeon => {
                for at in [INF_HOOK, EXIT_HOOK, HOOK_CD52] {
                    self.mem.w32(DS, at, 0);
                }
                self.and16(0x05C8, 0xF7FF);
            }
            Run::City => hooks.city_screen(self),
            _ => {}
        }
    }

    /// 0000:0AA9 up to the world loop (0000:A26F): the mouse's buttons
    /// released, D39 bit 80h, C866 0, the level's name (at DS:`name`)
    /// copied to LEVEL_NAME, the 3D mode set (`mode_3d`), the actors
    /// cleared (0000:1776), the view's border (0000:3503).
    pub fn world_begin(&mut self, name: u16, hooks: &mut dyn AreaHooks) {
        hooks.mouse_wait(self);
        let v = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, v);
        self.mem.w8(DS, 0xC866, 0);
        // 0000:9CA4: the string copied with its 0.
        let (mut si, mut di) = (name, LEVEL_NAME);
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        self.mode_3d(hooks);
        self.actors_clear();
        hooks.border(self);
    }

    /// 0000:0AD7.. after the world loop: D39 bit 80h, no CD2A hook, the
    /// palette buffer (929E, 300h bytes) cleared and the screens
    /// (`AreaHooks::world_screen`).
    pub fn world_end(&mut self, hooks: &mut dyn AreaHooks) {
        let v = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, v);
        self.mem.w32(DS, PLACE_HOOK, 0);
        for i in 0..0x300 {
            self.mem.w8(DS, 0x929E + i, 0);
        }
        hooks.world_screen(self);
    }

    /// 0000:1776. The actor slots (23 of 1Ch at 0AB3) inactive (flag
    /// 4000h), from the sixteenth also 8000h.
    pub fn actors_clear(&mut self) {
        for i in 0..0x17u16 {
            let v = if i >= 0xF { 0xC000 } else { 0x4000 };
            self.mem.w16(DS, 0x0AB3 + i * 0x1C + 0xF, v);
        }
    }

    /// 2294:025B. The 3D mode set: the hands' frame (1947) and C38C 0,
    /// 2294:0006 (the frame count, a saved game's level, the music; the
    /// world loop's hooks: C3A4 the actors, C3A8 the frame, C3B0 the
    /// entry 0000:4632, C3B4 the input 2826:0002, C3AC 0000:4470, C3B8
    /// 44D7, CD6E 51B8, CD1A the sky 2294:0A5D, CD22, CD26, CD2A the
    /// place 45B2, CD3A, CD42 468B, CD6A 4009, CD2E 4531; the view's reach
    /// from D37, the near lights' reach C396 800h, C39A bits 400h, 40h,
    /// 2; the player's square CB90), 0000:5D70, A902 bit 1 clear, the
    /// tables at 622 (18h words) and F88 (20h words) FFFFh.
    pub fn mode_3d(&mut self, hooks: &mut dyn AreaHooks) {
        self.mem.w16(DS, 0x1947, 0);
        self.mem.w16(DS, 0xC38C, 0);
        self.mem.w8(DS, 0x3C82, 0);
        self.mem.w8(DS, 0x680F, 0);
        self.mem.w16(DS, 0xCCBC, 0);
        if self.mem.r16(DS, 0x05C8) & 4 != 0 {
            self.and16(0x05C8, 0xFFFB);
            self.mem.w16(DS, 0xBA78, 0);
            hooks.restore_level(self);
        }
        if hooks.music(self) == 8 && self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
            hooks.townsfolk_graphics(self);
        }
        let v = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, v);
        if self.mem.r16(DS, 0x05C8) & 0x800 == 0 {
            hooks.hands(self);
        }
        self.or16(LEVEL_FLAGS, 0x40);
        self.mem.w16(DS, 0xD434, 0xC8);
        self.or16(LEVEL_FLAGS, 2);
        self.mem.w8(DS, 0x1012, 0xFF);
        self.mem.w32(DS, 0xCD66, 0);
        for (at, v) in [(0xCD4A, 0x23BA_0217u32), (0xCD46, 0x23BA_002A), (0xCD56, 0x25CA_0271), (0xCD5A, 0x292B_0028), (0xCD62, 0x2826_0AAB)] {
            self.mem.w32(DS, at, v);
        }
        self.and16(MAP_FLAGS, 0xFF7F);
        let bytes: [(u16, u8); 14] = [
            (0xC3BC, 5), (0xC3BD, 0xFF), (0xC3BE, 0xFF), (0xC3BF, 0xFF), (0xC3C8, 0xFF), (0xC3C9, 0xE), (0xC3CA, 0xF),
            (0xC3CB, 0xFF), (0xC3C0, 6), (0xC3C1, 0xE), (0xC3C2, 0xF), (0xC3C3, 0xFF), (0xC3C4, 0), (0xC3C5, 1),
        ];
        for (at, v) in bytes {
            self.mem.w8(DS, at, v);
        }
        let v = self.mem.r16(DS, 0x0D37);
        self.mem.w16(DS, 0xC394, v);
        for (at, v) in [
            (0xC3A8, 0x481Eu32), (0xC3A4, 0x47FD), (0xC3B0, 0x4632), (0xC3B4, 0x2826_0002), (0xC3AC, 0x4470), (0xC3B8, 0x44D7),
            (0xCD6E, 0x51B8),
        ] {
            self.mem.w32(DS, at, v);
        }
        self.mem.w16(DS, crate::light::var::REACH, 0x800);
        for (at, v) in [
            (0xCD1A, 0x2294_0A5Du32), (0xCD22, 0x2294_03A1), (0xCD26, 0x2294_03D7), (0xCD2A, 0x45B2), (0xCD3A, 0x2294_047B),
            (0xCD42, 0x468B), (0xCD6A, 0x4009), (0xCD2E, 0x4531),
        ] {
            self.mem.w32(DS, at, v);
        }
        self.mem.w16(DS, 0x8848, 0);
        self.mem.w16(DS, 0x7A46, 0);
        self.or16(LEVEL_FLAGS, 0x400);
        let col = self.mem.r16(DS, X) >> 7;
        let row = self.mem.r16(DS, Y) >> 7;
        self.mem.w16(DS, 0xCB90, (row & 0xFF) << 8 | (col.wrapping_add(col) & 0xFF));
        hooks.hook_2631(self);
        hooks.ems_free(self);
        self.and16(0xA902, 0xFFFE);
        for i in 0..0x18 {
            self.mem.w16(DS, 0x0622 + 2 * i, 0xFFFF);
        }
        for i in 0..0x20 {
            self.mem.w16(DS, 0x0F88 + 2 * i, 0xFFFF);
        }
    }

    /// The way out of a level (the hook CD4E), when the stairs lead past
    /// the first level or the last:
    /// - 0000:4537 (interiors, dungeons): the automap kept (with the
    ///   level one more meanwhile); an interior, the wilderness's dungeon
    ///   (5C8 bit 80h) or one entered by a trigger (5C8 bit 4000h, now
    ///   clear) is left; otherwise the player is asked (`ask_leave`), and
    ///   on yes C994 is set, the screens shown and the area left;
    /// - 0000:44BD (the wilderness's dungeons): left, 1194:D55F and
    ///   2433:0199.
    ///
    /// Leaving sets C398 bit 80h. None for another hook.
    pub fn area_exit(&mut self, hooks: &mut dyn AreaHooks) -> Option<()> {
        match self.mem.r32(DS, EXIT_HOOK) {
            0x4537 => {
                let l = self.mem.r8(DS, LEVEL).wrapping_add(1);
                self.mem.w8(DS, LEVEL, l);
                hooks.automap(self);
                let l = self.mem.r8(DS, LEVEL).wrapping_sub(1);
                self.mem.w8(DS, LEVEL, l);
                let f = self.mem.r16(DS, 0x05C8);
                if self.mem.r8(DS, AREA) != 4 && f & 0x80 == 0 {
                    if f & 0x4000 != 0 {
                        self.and16(0x05C8, 0xBFFF);
                    } else {
                        self.mem.w8(DS, 0x1A81, 1);
                        let leave = hooks.ask_leave(self);
                        self.mem.w8(DS, 0x1A81, 0);
                        if !leave {
                            return Some(());
                        }
                        self.mem.w8(DS, 0xC994, 1);
                        hooks.leave_screen(self);
                        hooks.leave_dungeon(self);
                    }
                }
                self.or16(MAP_FLAGS, 0x80);
                Some(())
            }
            0x44BD => {
                self.or16(MAP_FLAGS, 0x80);
                hooks.leave_interior(self);
                Some(())
            }
            _ => None,
        }
    }

    /// 0000:4632, the hook C3B0 entering the 3D world: C398 bit 1 for a
    /// player who sees in the dark (byte 1A9 C9h, or race 1A8 6), then
    /// the sky (0000:464A, `AreaHooks::sky`); 884A the timer.
    pub fn area_entry(&mut self, hooks: &mut dyn AreaHooks) {
        self.and16(MAP_FLAGS, 0xFFFE);
        if self.mem.r8(DS, 0x01A9) == 0xC9 || self.mem.r8(DS, 0x01A8) == 6 {
            self.or16(MAP_FLAGS, 1);
        }
        hooks.sky(self);
        let t = self.mem.r16(DS, crate::frame::var::TICKS);
        self.mem.w16(DS, 0x884A, t);
    }

    /// 0000:45B2, the hook CD2A once the level is loaded: but in an
    /// interior or the start dungeon, the player is put back where they
    /// stood outside (FCE, taken, and FD0) if anywhere; then entering
    /// (5C8 bit 800h) 0000:2974 when C39A bit 800h; otherwise outdoors,
    /// by day (CCFE 100h..300h), townspeople come (15, or 7 in the
    /// wilderness unless it has none here) with their graphics.
    pub fn area_place(&mut self, hooks: &mut dyn AreaHooks) {
        let area = self.mem.r8(DS, AREA);
        if area != 4 && area != 2 && self.mem.r16(DS, OUTSIDE_X) != 0 {
            let x = self.mem.r16(DS, OUTSIDE_X);
            self.mem.w16(DS, OUTSIDE_X, 0);
            self.mem.w16(DS, X, x);
            let y = self.mem.r16(DS, OUTSIDE_Y);
            self.mem.w16(DS, Y, y);
        }
        if self.mem.r16(DS, 0x05C8) & 0x800 != 0 {
            // (0000:1793 with no actors: nothing.)
            hooks.spawn(self, 0);
            if self.mem.r16(DS, LEVEL_FLAGS) & 0x800 != 0 {
                hooks.hook_2974(self);
            }
            return;
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 {
            return;
        }
        let mut n = 0xF;
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            if hooks.wild_empty(self) {
                return;
            }
            n = 7;
        }
        let t = self.mem.r16(DS, 0xCCFE);
        if !(0x100..=0x300).contains(&t) {
            return;
        }
        hooks.spawn(self, n);
        hooks.townsfolk_graphics(self);
    }
}

impl Machine {
    /// 0000:A45D..A47F, the world loop's end: the planes freed (0000:C8FE:
    /// the floor, walls and ceiling, their segments cleared), the actors'
    /// flags past the flats' (from CA97 + C38E, to 9Eh bytes, its length
    /// kept at D432 and its start at CCC0) cleared, then the renderer's
    /// buffers freed (0000:A5A0, `AreaHooks::world_free`).
    pub fn world_leave(&mut self, hooks: &mut dyn AreaHooks) {
        for at in [crate::mif::var::FLOOR_SEG, crate::mif::var::MAP1_SEG, crate::mif::var::MAP2_SEG] {
            let seg = self.mem.r16(DS, at);
            self.heap_free(seg);
            self.mem.w16(DS, at, 0);
        }
        let n = self.mem.r16(DS, 0xC38E);
        self.mem.w16(DS, 0xD432, n);
        let at = n.wrapping_add(0xCA97);
        self.mem.w16(DS, 0xCCC0, at);
        // rep stosb with CX 9Eh - n (65536 - .. past 9Eh).
        let cx = 0x9Eu16.wrapping_sub(n);
        for i in 0..cx {
            self.mem.w8(DS, at.wrapping_add(i), 0);
        }
        hooks.world_free(self);
    }
}

impl crate::engine::Engine {
    /// 0000:A26F..A2A0 without the drawing: entering the 3D world whose
    /// level is named at LEVEL_NAME, after `world_begin`. The timer, the
    /// ceiling 64h, the walking speed, standing (`enter_area`'s); the EMS
    /// banks but the level's freed, the deep floors, no changing squares,
    /// the rain's drops, no moving lights (0000:E122), on dry ground
    /// (0000:A4ED); the entry hook (C3B0: 0000:4632, `area_entry`); the
    /// level loaded unless its name starts with `#` (a city built, a
    /// dungeon made) or the hook CD5E is set; the place hook (CD2A:
    /// 0000:45B2, `area_place`); the INF loaded (0000:EC03); the entered
    /// hook (CD42: 0000:468B, `AreaHooks::entered`).
    ///
    /// The hooks are recognised by their value: the port loads the image
    /// at segment 0, so the original's hooks into segment 0000 read as
    /// unset to the original's test of the segment (0000:A8EF).
    pub fn world_enter(&mut self, hooks: &mut dyn AreaHooks) -> Option<()> {
        let m = &mut self.m;
        m.mem.w16(DS, crate::frame::var::TICKS, 0);
        m.mem.w16(DS, 0xD80A, 0xFF9C);
        m.mem.w16(DS, crate::movement::var::CEILING, 0x64);
        let v = m.mem.r16(DS, 0xC390);
        m.mem.w16(DS, crate::controls::var::WALK, v);
        m.mem.w16(DS, crate::movement::var::HEIGHT, 0x3C);
        m.ems_reset();
        let v = m.mem.r16(DS, crate::movement::var::FRAME_SEG).wrapping_add(0xC00);
        m.mem.w16(DS, crate::movement::var::FLOOR_SOLID, v);
        for i in 0..0x20 {
            m.mem.w16(DS, crate::click::var::TEMP_WALLS + 2 * i, 0xFFFF);
        }
        m.weather_drops()?;
        for i in 0..0x60 {
            m.mem.w16(DS, 0xBB0C + 2 * i, 0x8000);
        }
        m.mem.w8(DS, 0xCB5B, 1);
        if m.mem.r32(DS, ENTRY_HOOK) == 0x4632 {
            m.area_entry(hooks);
        }
        if m.mem.r16(DS, 0xCD60) != 0 {
            return None;
        }
        if m.mem.r8(DS, LEVEL_NAME) != b'#' {
            m.level_load(&mut self.files, LEVEL_NAME)?;
        }
        let m = &mut self.m;
        if m.mem.r32(DS, PLACE_HOOK) == 0x45B2 {
            m.area_place(hooks);
        }
        let info = m.mem.r16(DS, crate::info::var::INFO_PTR);
        if info != 0 {
            m.info_load(&mut self.files, info)?;
            self.m.mem.w16(DS, crate::info::var::INFO_PTR, 0);
        }
        if self.m.mem.r32(DS, ENTERED_HOOK) == 0x468B {
            hooks.entered(&mut self.m);
        }
        Some(())
    }
}
