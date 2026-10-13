//! One frame of the 3D world (the world loop 0000:A240..A44D) without its
//! drawing, for a front end to drive.
//!
//! A frame, in the original's order: the actors and the projectiles (the
//! hook C3A4, 0000:47FD), the player's physics with the input step (the
//! hook C3B4, `controls`) and the level transitions (C3AC), the changed
//! squares of the wall spells (0000:D476), the frame body (the hook
//! C3A8, `frame`), a level's INF waiting to load (CB84), and the frame
//! counter (CCBC).
//!
//! The original drew the view between the physics and the changed
//! squares. The start of its drawing set state the game reads (the
//! player's square: `view_begin`); the walk over the squares stepped the
//! changed squares it drew, which `EngineHooks::draw` leaves to the front
//! end, called at that point. The front end draws the view from the
//! state after the frame (through `Machine` and the `View` helpers),
//! advances the BIOS timer (`tick`: 18.2 ticks a second; the clock runs
//! on it), and paces frames (`frame_ticks`).

use crate::bsa::Files;
use crate::controls::{ControlHooks, Intent};
use crate::frame::FrameHooks;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::PhysicsHooks;
use crate::movement::var::{HEADING, HEIGHT, LEVEL_FLAGS, MAP_FLAGS, X, Y};

/// Where a frame's input comes from.
pub enum Input<'a> {
    /// The original's controls: the mouse and keys the front end set
    /// (`Machine::set_mouse`, `set_key`, `mouse_motion`).
    Classic,
    /// Modern controls (`controls::Intent`); the mouse and keys still feed
    /// the swing and the frame body's keys.
    Steer(&'a Intent),
}

/// Everything the engine leaves to the front end or to code not ported.
pub trait EngineHooks: ControlHooks + FrameHooks {
    /// The hook C3AC (0000:4470): a trigger (CB6A) taking the player
    /// elsewhere.
    fn transition(&mut self, _m: &mut Machine, _files: &mut Files, _trigger: u16) -> Option<()> {
        Some(())
    }
    /// 0000:C980, the walk over the squares in view (C394 squares out),
    /// back to front: for the game's state, each square drawn steps if it
    /// is changing (`Machine::temp_walls_begin_draw` then
    /// `temp_wall_drawn`).
    fn draw(&mut self, _m: &mut Machine) {}
}

/// How a frame ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The world goes on.
    Running,
    /// The area is being left (C398 bit 80h): death, an exit, or a change
    /// of mode. The front end decides what comes next.
    Left,
}

/// The physics frame's hooks for one frame's input.
struct Physics<'a, 'b> {
    input: &'a Input<'b>,
    hooks: &'a mut dyn EngineHooks,
}

impl PhysicsHooks for Physics<'_, '_> {
    fn input(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        match self.input {
            Input::Classic => m.player_input(files, self.hooks),
            Input::Steer(it) => m.player_steer(files, self.hooks, it),
        }
    }
    fn transition(&mut self, m: &mut Machine, files: &mut Files, trigger: u16) -> Option<()> {
        self.hooks.transition(m, files, trigger)
    }
}

/// The game's state and files.
pub struct Engine {
    pub m: Machine,
    pub files: Files,
}

impl Engine {
    pub fn new(m: Machine, files: Files) -> Self {
        Self { m, files }
    }

    /// The BIOS timer advances `n` ticks (18.2 a second).
    pub fn tick(&mut self, n: u16) {
        let t = self.m.mem.r16(DS, crate::frame::var::TICKS).wrapping_add(n);
        self.m.mem.w16(DS, crate::frame::var::TICKS, t);
    }

    /// How many timer ticks the original gives a frame: two (about 9
    /// frames a second), one on levels with flag C39A bit 400h, plus the
    /// detail setting.
    pub fn frame_ticks(&self) -> u16 {
        let base = if self.m.mem.r16(DS, LEVEL_FLAGS) & 0x400 != 0 { 1 } else { 2 };
        base + self.m.mem.r8(DS, crate::frame::var::DETAIL) as u16
    }

    /// One frame of the world. None where ported code meets something it
    /// cannot do (an unported hook, a fault or hang of the original).
    pub fn frame(&mut self, input: Input, hooks: &mut dyn EngineHooks) -> Option<Frame> {
        let (m, files) = (&mut self.m, &mut self.files);
        let t = m.mem.r16(DS, crate::frame::var::TICKS);
        m.mem.w16(DS, 0xBA60, t);
        m.actors_update(files)?;
        m.projectiles_update()?;
        m.physics_frame(files, &mut Physics { input: &input, hooks })?;
        if m.mem.r16(DS, MAP_FLAGS) & 0x80 != 0 {
            return Some(Frame::Left);
        }
        m.view_begin();
        hooks.draw(m);
        m.temp_walls_frame();
        m.frame_body(files, hooks)?;
        let info = m.mem.r16(DS, crate::info::var::INFO_PTR);
        if info != 0 {
            m.info_load(files, info)?;
            m.mem.w16(DS, crate::info::var::INFO_PTR, 0);
        }
        m.mem.w16(DS, 0xCB5E, 0);
        let c = m.mem.r16(DS, 0xCCBC).wrapping_add(1);
        m.mem.w16(DS, 0xCCBC, c);
        Some(if m.mem.r16(DS, MAP_FLAGS) & 0x80 != 0 { Frame::Left } else { Frame::Running })
    }

    /// 0000:A26B..A297 and the state part of A4ED: entering a 3D area
    /// whose level is loaded (`level_load`), with its INF named at DS:`inf`
    /// (0 none). The timer restarts, the ceiling is 64h above the floor
    /// (D80A, D80C), the player stands (3Ch) at the base walking speed,
    /// the EMS banks are freed but the level's (5D74), floors from C00h
    /// past the EMS frame are deep (CCCE), no squares are changing, the
    /// player is on dry ground (CB5B), rain or snow gets its 100 drops
    /// in the view (E322, weather 82h), and the INF loads. The rest of
    /// A4ED sets up the renderer.
    pub fn enter_area(&mut self, inf: u16) -> Option<()> {
        let m = &mut self.m;
        m.mem.w16(DS, crate::info::var::INFO_PTR, inf);
        m.mem.w16(DS, crate::frame::var::TICKS, 0);
        m.mem.w16(DS, 0xD80A, 0xFF9C);
        m.mem.w16(DS, crate::movement::var::CEILING, 0x64);
        let v = m.mem.r16(DS, 0xC390);
        m.mem.w16(DS, crate::controls::var::WALK, v);
        m.mem.w16(DS, HEIGHT, 0x3C);
        m.ems_reset();
        let v = m.mem.r16(DS, crate::movement::var::FRAME_SEG).wrapping_add(0xC00);
        m.mem.w16(DS, crate::movement::var::FLOOR_SOLID, v);
        for i in 0..0x20 {
            m.mem.w16(DS, crate::click::var::TEMP_WALLS + 2 * i, 0xFFFF);
        }
        m.weather_drops()?;
        m.mem.w8(DS, 0xCB5B, 1);
        let info = m.mem.r16(DS, crate::info::var::INFO_PTR);
        if info != 0 {
            m.info_load(&mut self.files, info)?;
            m.mem.w16(DS, crate::info::var::INFO_PTR, 0);
        }
        Some(())
    }

    /// What the renderer needs most often.
    pub fn view(&self) -> View {
        let r = |at| self.m.mem.r16(DS, at);
        View {
            x: r(X),
            y: r(Y),
            height: r(HEIGHT) as i16,
            heading: r(HEADING),
            light: r(crate::frame::var::LIGHT),
            time_of_day: r(0xCCFE),
            bob: r(crate::movement::var::BOB) as i16,
            hand_frame: r(crate::weapon::var::FRAME),
            message: (r(0x061E) != 0).then(|| (r(0x061C), r(0x061A))),
        }
    }
}

/// A snapshot of the player's view for drawing. The map planes, actors
/// (0AB3, 23 slots of 1Ch) and projectiles are read from `Engine::m`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    /// Position in map units (128 a square), height above the floor (3Ch
    /// standing), heading (0..511, a full turn).
    pub x: u16,
    pub y: u16,
    pub height: i16,
    pub heading: u16,
    /// The light radius (C3E2) and the time of day (CCFE, 0..3FFh).
    pub light: u16,
    pub time_of_day: u16,
    /// The view's bob or shake (CB68).
    pub bob: i16,
    /// The hands' animation frame (1947).
    pub hand_frame: u16,
    /// The message over the view (far pointer, segment then offset),
    /// while it shows.
    pub message: Option<(u16, u16)>,
}

impl Machine {
    /// 0000:C923, but for the walk over the squares (C980): the state the
    /// original sets as it starts drawing the view. The player's column
    /// and row (A854, A856; the renderer's CCA4, CCA6) and square (CB90,
    /// a wall plane offset), the wall word there (CB6C); the draw's
    /// resets (CB92, CCCA FFFFh, CD14 F830h, CCB2, CCB4 0), and C398 bit
    /// 20h (the frame sets it for the draw) clear.
    pub fn view_begin(&mut self) {
        self.mem.w16(DS, 0xCB92, 0xFFFF);
        self.mem.w16(DS, 0xCCCA, 0xFFFF);
        self.mem.w16(DS, 0xCD14, 0xF830);
        self.mem.w16(DS, 0xCCB2, 0);
        self.mem.w16(DS, 0xCCB4, 0);
        let col = self.mem.r16(DS, 0xC3D0) >> 7;
        let row = self.mem.r16(DS, 0xC3D4) >> 7;
        self.mem.w16(DS, 0xA854, col);
        self.mem.w16(DS, 0xCCA4, col);
        self.mem.w16(DS, 0xA856, row);
        self.mem.w16(DS, 0xCCA6, row);
        // 0000:F565: the offset of (x, y) in the wall plane.
        let sq = (row << 8).wrapping_add(col << 1);
        self.mem.w16(DS, 0xCB90, sq);
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let w = self.mem.r16(walls, sq);
        self.mem.w16(DS, 0xCB6C, w);
        let f = self.mem.r16(DS, MAP_FLAGS) & !0x20;
        self.mem.w16(DS, MAP_FLAGS, f);
    }
}
