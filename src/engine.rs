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
//! doors (`view_walk_doors`) and the changed squares it drew, which
//! `EngineHooks::draw` leaves to the front end, called at that point. The front end draws the view from the
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
    /// `temp_wall_drawn`). Called only without a page: with one, the
    /// drawing's own walk (`render::Walked`) is taken instead.
    fn draw(&mut self, _m: &mut Machine) {}
    /// The door hook (CD46, 0000:A8EF): a door's sound, as the walk
    /// steps it (`Machine::view_walk_doors`), when the hook is set, at the
    /// door's square (0000:CDD8, CX the door's map offset). The hook the
    /// 3D mode sets is 23BA:002A (`Machine::sound_at`).
    fn sound(&mut self, m: &mut Machine, sound: u16, at: u16) {
        let _ = m.sound_at(sound, at);
    }
    /// A step of the frame taken over: true where the hooks did it (a
    /// test's stand-in), false to let the engine run it.
    fn step(&mut self, _m: &mut Machine, _step: Step) -> bool {
        false
    }
    /// 0000:D4B4: the drawing's set-up (the floor's half, 140h, then the
    /// ceiling's, FEC0h, unless outdoors or C398 bit 40h), which in fog
    /// runs the fog's step (2294:0B01); the renderer's.
    fn draw_setup(&mut self, _m: &mut Machine, _half: u16) {}
    /// 0000:CF96, the hook CD56 (25CA:0271, `Machine::flat_creature`) for
    /// each creature's flat the page's drawing reached, in its walk's
    /// order, called on the machine as the drawing ends (the drawing ran
    /// on a copy). Nothing by default: the flats stay in
    /// `Engine::page_creatures`.
    fn creature_flats(&mut self, _m: &mut Machine, _files: &mut Files, _reached: &[crate::render::Creature]) -> Option<()> {
        Some(())
    }
    /// The hook CD1A, 2294:0A5D: the sky's objects drawn (`sky_draw`,
    /// with the renderer's SkyHooks).
    fn sky(&mut self, _m: &mut Machine) {}
    /// 0000:E26B, E1A2: the snow's or the rain's drops moved (the drops
    /// are the renderer's).
    fn snow(&mut self, _m: &mut Machine) {}
    fn rain(&mut self, _m: &mut Machine) {}
    /// 0000:A7B4: the frame's picture put on the screen (shaken by CB68).
    fn present(&mut self, _m: &mut Machine) {}
    /// The page as the frame's drawing left it (its 3D view, the snow's or
    /// the rain's drops), before the physics: what the page holds for
    /// what runs from there in the frame (a click's box saving the screen
    /// it covers, 0000:8F2A, saves this frame's view, not the last).
    fn drawn(&mut self, _m: &mut Machine, _page: &[u8]) {}
}

/// The world loop's stage (BA4B), which it marks as it goes.
const STAGE: u16 = 0xBA4B;

/// The steps of a frame that `EngineHooks::step` may take over (a test
/// stands in for them; by default the engine runs them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The hooks C3A4 (the actors, 0000:47FD), CD16, CD66, CD1A (the sky,
    /// 2294:0A5D) and C3A8 (the frame body, 0000:481E).
    Actors,
    HookCD16,
    HookCD66,
    Sky,
    FrameBody,
    /// 0000:FCF6, the walk's bob and its footstep.
    WalkBob,
    /// 0000:E12E, the level's lights near the eye.
    LightsNear,
    /// 0000:DFDB with SI: the textures put back.
    Textures(u16),
    /// 0000:5C8A with BX: the level's bank mapped (C8F0).
    MapBank(u16),
    /// 0000:E106, the moving lights packed.
    MovingLights,
    /// 0000:D4B4 with its argument: the drawing's set-up.
    DrawSetup(u16),
    /// 0000:C923, the view drawn (`view_begin`, the walk's doors,
    /// `EngineHooks::draw`, the trigger squares).
    View,
    /// 0000:B4F5 and B358: the draw lists, drawing only.
    DrawLists,
    DrawFlats,
    /// 0000:E26B, E1A2: the snow's or the rain's drops moved.
    Snow,
    Rain,
    /// 0000:F57C, the physics.
    Physics,
    /// 0000:D476, the changed squares.
    TempWalls,
    /// 0000:EC03, a level's INF waiting (CB84).
    InfoLoad,
    /// 0000:A7B4, the picture put on the screen.
    Present,
}

impl Machine {
    /// 0000:A7B4..A827, the state the picture's putting on the screen
    /// leaves when it is shaken (CB68, the bob in water, not 0): by its
    /// size `n` (CB68 or its negative), the step BA70 (2^n / 2, negative
    /// with BA6E FFFFh and BA6C 140h for a negative shake, else BA6E 1 and
    /// BA6C FEC0h), the view's rows (A916..A91A) and quarter columns
    /// (A914..A918) over 2^n at BA68 and BA6A, and 2^n's low byte at BA4A.
    /// The copy to the screen after (A82A..) is the front end's.
    pub fn present_shake(&mut self) {
        let shake = self.mem.r16(DS, 0xCB68);
        if shake == 0 {
            return;
        }
        let r = |m: &Machine, at: u16| m.mem.r16(DS, at);
        let n = (if (shake as i16) < 0 { shake.wrapping_neg() } else { shake }) as u8 & 0x1F;
        let pow = if n < 16 { 1u16 << n } else { 0 };
        let half = pow >> 1;
        if (shake as i16) < 0 {
            self.mem.w16(DS, 0xBA70, half.wrapping_neg());
            self.mem.w16(DS, 0xBA6E, 0xFFFF);
            self.mem.w16(DS, 0xBA6C, 0x140);
        } else {
            self.mem.w16(DS, 0xBA6E, 1);
            self.mem.w16(DS, 0xBA6C, 0xFEC0);
            self.mem.w16(DS, 0xBA70, half);
        }
        let shr = |v: u16| if n < 16 { v >> n } else { 0 };
        let rows = shr(r(self, 0xA91A).wrapping_sub(r(self, 0xA916)).wrapping_add(1));
        self.mem.w16(DS, 0xBA68, rows);
        let cols = shr(r(self, 0xA918).wrapping_sub(r(self, 0xA914)).wrapping_add(1)) >> 2;
        self.mem.w16(DS, 0xBA6A, cols);
        self.mem.w8(DS, 0xBA4A, pow as u8);
    }
}

/// `n` words copied in DGROUP from `from` to `to`.
fn copy_words(m: &mut Machine, from: u16, to: u16, n: u16) {
    for i in 0..n {
        let w = m.mem.r16(DS, from + 2 * i);
        m.mem.w16(DS, to + 2 * i, w);
    }
}

/// 0000:A606: the clip (A914..A91B) put back from BA58.
fn clip_back(m: &mut Machine) {
    for i in 0..8u16 {
        let b = m.mem.r8(DS, 0xBA58 + i);
        m.mem.w8(DS, 0xA914 + i, b);
    }
}

impl Machine {
    /// 0000:DFDB: the water's (or the lava's) animation, the .RCI named at
    /// DS:`si` (waterani.rci at BA17, lavaani.rci at BA24: five frames of
    /// 320 x 100), read into EMS banks 4, 5 and 6 (FA00h, FA00h and 7D00h
    /// bytes, through the frame at 902E) unless they have none (C8F2's
    /// sign) or hold that file already (CD00). The floor's half is filled
    /// from them (0000:DEA8). Where the file isn't there the original
    /// reads on from a device that gives nothing (`file_open_or_device`).
    /// None where the open's error hook is set.
    pub fn water_frames(&mut self, files: &mut Files, si: u16) -> Option<()> {
        if self.mem.r8(DS, 0xC8F2) & 0x80 != 0 || self.mem.r16(DS, 0xCD00) == si {
            return Some(());
        }
        self.mem.w16(DS, 0xCD00, si);
        self.file_open_or_device(files, si)?;
        for (bank, n) in [(4u16, 0xFA00u16), (5, 0xFA00), (6, 0x7D00)] {
            self.ems_map(bank);
            let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
            self.file_read(files, seg, off, n);
        }
        self.file_close(files);
        Some(())
    }
}

/// A hook of the world loop called through 0000:A8EF: nothing when unset,
/// the ported routine its value names, None for another.
fn world_hook(m: &mut Machine, files: &mut Files, hooks: &mut dyn EngineHooks, step: Step, at: u16) -> Option<()> {
    if !m.hook_set(at) || hooks.step(m, step) {
        return Some(());
    }
    match (at, m.mem.r32(DS, at)) {
        (0xC3A4, 0x47FD) => {
            // 47FD keeps its return address (A8EF's, A8F9 in the world
            // loop's segment) at 127A for its stack's check. Its segment
            // word, 127C, is the controls' refresh (2826:018A, `REFRESH`),
            // never 0 on a PC, so the next input step makes the row tables
            // again: as the harness loads the image, 1000h.
            m.mem.w32(DS, 0x127A, 0x1000_A8F9);
            m.actors_update(files)?;
            m.projectiles_update(&mut crate::weapon::Kill { files, hooks })
        }
        (0xC3A8, 0x481E) => m.frame_body(files, hooks),
        (0xCD1A, 0x2294_0A5D) => {
            hooks.sky(m);
            Some(())
        }
        _ => crate::giveup::not_ported("0000:A8EF: a world-loop hook set to a routine not ported"),
    }
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
    /// With a page, each frame draws the 3D view into it as the original
    /// does (`render::draw`, at the start of the drawing stages), over
    /// the last frame's; the paths it met that aren't ported in
    /// `page_gaps`.
    pub page: Option<Vec<u8>>,
    pub page_gaps: Vec<&'static str>,
    /// DGROUP's bytes the drawing has written on its copy, frame after
    /// frame, the last write's (before, after) for each: the state the
    /// original's drawing leaves in DGROUP and the engine's doesn't (for
    /// the state comparison, `page_state`).
    pub page_wrote: Vec<Option<(u8, u8)>>,
    /// The EMS bytes the drawing has changed on its copy, likewise (the
    /// bank times 10000h plus the offset: `page_ems`).
    pub page_ems_wrote: std::collections::HashMap<u32, (u8, u8)>,
    /// `page_ems_wrote` as it stood before the last frame's drawing: the
    /// EMS that drawing started from (`render::draw`'s `prior_ems`), for
    /// a front end's own pass standing in for the drawing's sky hook.
    pub page_ems_prior: std::collections::HashMap<u32, (u8, u8)>,
    /// The creatures' flats the last page's drawing reached
    /// (`render::Creature`, CF96), for the hook CD56 on the machine.
    pub page_creatures: Vec<crate::render::Creature>,
    /// What this frame's drawing left the game (`render::Walked`), taken
    /// at the walk (A367) in place of the front end's `EngineHooks::draw`.
    walked: Option<crate::render::Walked>,
}

/// The flags the frame sets and clears bits of (C398, CCCA), whose
/// drawing's bits the machine's read-modify-write keeps in the original.
const DRAWN_BITS: [u16; 4] = [0xC398, 0xC399, 0xCCCA, 0xCCCB];

impl Engine {
    pub fn new(m: Machine, files: Files) -> Self {
        Self { m, files, page: None, page_gaps: Vec::new(), page_wrote: vec![None; 0x10000], page_ems_wrote: Default::default(), page_ems_prior: Default::default(), page_creatures: Vec::new(), walked: None }
    }

    /// The drawing's own state taken over from `from`, the engine this
    /// one replaces, its machine copied from `from`'s: the page the next
    /// frame draws over, and the bytes its copies wrote (`page_wrote`,
    /// `page_ems_wrote`), which the original, drawing on the machine,
    /// still holds where nothing has written them since (the machine's
    /// writes since the last drawing, a load's among them, are dropped
    /// from them at the next, as every frame's are).
    pub fn keep_drawing(&mut self, from: &mut Engine) {
        self.page = from.page.take();
        self.page_wrote = std::mem::take(&mut from.page_wrote);
        self.page_ems_wrote = std::mem::take(&mut from.page_ems_wrote);
        self.page_ems_prior = std::mem::take(&mut from.page_ems_prior);
    }

    /// The BIOS timer advances `n` ticks (18.2 a second).
    pub fn tick(&mut self, n: u16) {
        let t = self.m.mem.r16(DS, crate::frame::var::TICKS).wrapping_add(n);
        self.m.mem.w16(DS, crate::frame::var::TICKS, t);
        self.m.bios_ticks = self.m.bios_ticks.wrapping_add(n as u32);
    }

    /// How many timer ticks the original gives a frame: two (about 9
    /// frames a second), one on levels with flag C39A bit 400h, plus the
    /// detail setting.
    pub fn frame_ticks(&self) -> u16 {
        let base = if self.m.mem.r16(DS, LEVEL_FLAGS) & 0x400 != 0 { 1 } else { 2 };
        base + self.m.mem.r8(DS, crate::frame::var::DETAIL) as u16
    }

    /// One frame of the world, the world loop's body 0000:A2AC..A453 in
    /// its order. None where ported code meets something it cannot do (an
    /// unported hook, a fault or hang of the original).
    ///
    /// The player's position (C3D0..C3D5) is the eye's for most of the
    /// frame: the real one is kept at BA62 and put back only around the
    /// physics (A3AA..A3BE); after them the eye moves 1Eh back along the
    /// heading (A919) until the next physics. The waits for the timer
    /// (A428..A444) are the front end's (`frame_ticks`, `tick`); the extra
    /// tick of a level without C39A bit 400h shows in BA60.
    pub fn frame(&mut self, input: Input, hooks: &mut dyn EngineHooks) -> Option<Frame> {
        self.walked = None;
        // (A frame that gave up last time left its tracking on.)
        self.m.mem.untrack();
        self.m.mem.frame_written();
        let (m, files) = (&mut self.m, &mut self.files);
        let r = |m: &Machine, at: u16| m.mem.r16(DS, at);
        m.mem.w8(DS, STAGE, 0);
        let t = r(m, crate::frame::var::TICKS);
        m.mem.w16(DS, 0xBA60, t);
        // A2BD: the hook C3A4, the actors and the projectiles.
        world_hook(m, files, hooks, Step::Actors, 0xC3A4)?;
        m.mem.w8(DS, STAGE, 1);
        m.view_window();
        if !hooks.step(m, Step::WalkBob) {
            if let Some((s, at)) = m.walk_bob() {
                hooks.sound(m, s, at);
            }
        }
        world_hook(m, files, hooks, Step::HookCD16, 0xCD16)?;
        // A2EC: the player's light at the eye.
        let (x, y) = (r(m, X), r(m, Y));
        m.mem.w16(DS, 0xC3DE, x);
        m.mem.w16(DS, 0xC3E0, y);
        m.mem.w8(DS, STAGE, 2);
        if !hooks.step(m, Step::LightsNear) {
            m.lights_near();
        }
        // B4E8: the draw lists emptied.
        m.mem.w16(DS, 0xBAC8, 0);
        m.mem.w16(DS, 0xCD7A, 0);
        // A483: the textures put back (0000:DFDB), from the copy at BA17
        // outdoors or on dry ground, else from BA24's.
        let si = if r(m, MAP_FLAGS) & 4 != 0 || m.mem.r8(DS, 0xCB5B) == 1 { 0xBA17 } else { 0xBA24 };
        if !hooks.step(m, Step::Textures(si)) {
            m.water_frames(files, si)?;
        }
        let bank = m.mem.r8(DS, 0xC8F0) as u16;
        if !hooks.step(m, Step::MapBank(bank)) {
            m.ems_map(bank);
        }
        if !hooks.step(m, Step::MovingLights) {
            m.moving_lights();
        }
        m.mem.w8(DS, STAGE, 3);
        if m.rows_drawn.is_some() {
            let v = crate::render::rows_read(m);
            m.rows_drawn.as_mut().unwrap().push(v);
        }
        if let Some(page) = self.page.as_mut() {
            // The moons' phases for a new day, which the drawing's E41A
            // loads from their files (its copy has none).
            m.moon_phases(files)?;
            // What the machine wrote since the last drawing (`watch_drawn`)
            // is its own: the earlier drawings' bytes there are gone in the
            // original, even where the machine wrote the byte they started
            // from, and this drawing doesn't find them (`render::draw`'s
            // `prior`). Not the flags the frame sets and clears bits of
            // (`after_drawing`).
            for o in m.mem.drawn_written() {
                if !DRAWN_BITS.contains(&o) {
                    self.page_wrote[o as usize] = None;
                }
            }
            // An EMS byte the machine has written since the drawing changed
            // it on its copy is the machine's (as `page_wrote`'s are).
            self.page_ems_wrote.retain(|&o, &mut (before, _)| {
                let bank = (o >> 16) as u16;
                let i = (o & 0xFFFF) as u16;
                let mapped = m.mem.r16(DS, crate::level::var::EMS_MAPPED) >> 2 == bank && m.mem.r8(DS, crate::level::var::EMS_ON) != 0;
                let now = if mapped { m.mem.r8(m.mem.r16(DS, crate::mif::var::BUFFER_SEG), i) } else { m.ems.get(bank as usize).map_or(0, |b| b[i as usize]) };
                now == before
            });
            self.page_ems_prior = self.page_ems_wrote.clone();
            let (p, gaps, wrote, creatures, walked, ems) = crate::render::draw(m, page, &crate::render::Lgts::of(files), &self.page_wrote, &self.page_ems_prior);
            for (o, before, after) in ems {
                self.page_ems_wrote.insert(o, (before, after));
            }
            *page = p;
            self.walked = Some(walked);
            self.page_gaps = gaps;
            if self.page_gaps.is_empty() {
                hooks.creature_flats(m, files, &creatures)?;
            }
            self.page_creatures = creatures;
            for (o, before, after) in wrote {
                // Written back to the machine's own byte: the original's
                // holds the machine's there now, not an earlier frame's.
                self.page_wrote[o as usize] = (before != after).then_some((before, after));
            }
            // The fog's step (2294:0B01) runs in the drawing's set-up
            // (0000:D4B4 at A322, the sky's hook), on the machine in the
            // original and before any more of the frame: its flags, the
            // daylight's off (C398 bit 8000h, which the near lights read,
            // 0000:E12E) and the fog's on or off (C39A bit 20h, which the
            // next set-up's sky reads, D502), are the machine's.
            for (o, bit) in [(0xC399u16, 0x80u8), (0xC39A, 0x20)] {
                let Some((_, after)) = self.page_wrote[o as usize] else { continue };
                let b = (m.mem.r8(DS, o) & !bit) | (after & bit);
                m.mem.w8(DS, o, b);
                self.page_wrote[o as usize] = (b != after).then_some((b, after));
            }
            m.mem.watch_drawn();
            // The bank the drawing left mapped is the machine's from here:
            // the original's draws on the machine, and what runs after it in
            // the frame (a way out's 2433:0199 keeping the bank mapped)
            // finds that bank in the frame.
            let mapped = crate::level::var::EMS_MAPPED;
            let byte = |o: u16| self.page_wrote[o as usize].map_or(m.mem.r8(DS, o), |(_, after)| after);
            let drawn = byte(mapped) as u16 | (byte(mapped + 1) as u16) << 8;
            if drawn != m.mem.r16(DS, mapped) {
                m.ems_map(drawn >> 2);
                self.page_wrote[mapped as usize] = None;
                self.page_wrote[mapped as usize + 1] = None;
            }
        }
        // A322: the drawing's set-up (0000:D4B4, the floor's half).
        if !hooks.step(m, Step::DrawSetup(0x140)) {
            hooks.draw_setup(m, 0x140);
        }
        world_hook(m, files, hooks, Step::HookCD66, 0xCD66)?;
        if m.mem.r8(DS, 0xCB5C) != 0 {
            // The sky's hook runs on the machine too (its fog step,
            // 2294:0B01): what it writes there is the machine's, not the
            // drawing's copy's run of it.
            let watched = self.page.is_some();
            if watched {
                m.mem.track();
            }
            let r = world_hook(m, files, hooks, Step::Sky, 0xCD1A);
            if watched {
                for o in m.mem.written_offsets() {
                    self.page_wrote[o as usize] = None;
                }
                m.mem.untrack();
            }
            r?;
        } else {
            m.mem.w8(DS, STAGE, 4);
            if r(m, MAP_FLAGS) & 0x44 == 0 && !hooks.step(m, Step::DrawSetup(0xFEC0)) {
                hooks.draw_setup(m, 0xFEC0);
            }
        }
        m.mem.w8(DS, STAGE, 6);
        // A367..A393: the view drawn (0000:C923), twice as far when the
        // eye is below the floor; else C398 bit 20h set for the draw.
        let reach = r(m, 0xC394);
        m.mem.w16(DS, 0xC394, reach << 1);
        if (r(m, HEIGHT) as i16) >= 0 {
            // shl then shr: bit 15 is lost for the draw.
            m.mem.w16(DS, 0xC394, (reach << 1) >> 1);
            let f = r(m, MAP_FLAGS) | 0x20;
            m.mem.w16(DS, MAP_FLAGS, f);
        }
        m.mem.w8(DS, STAGE, 7);
        if !hooks.step(m, Step::View) {
            m.view_begin();
            for (s, at) in m.view_walk_doors() {
                hooks.sound(m, s, at);
            }
            match self.walked.take() {
                // The drawing's own: the objects it stepped (C23A), what
                // it left under the cursor and the changed squares' steps.
                Some(w) => {
                    for &si in &w.stepped {
                        m.drawn_step(si);
                    }
                    for &(o, b) in &w.left {
                        m.mem.w8(DS, o, b);
                        // The machine's own now (`page_state`).
                        self.page_wrote[o as usize] = None;
                    }
                }
                None => hooks.draw(m),
            }
            // 0000:C9E8: the trigger squares at the end of the walk, while
            // the eye isn't below the floor.
            if (r(m, HEIGHT) as i16) >= 0 {
                if let Some((s, at)) = m.trigger_squares()? {
                    hooks.sound(m, s, at);
                }
            }
        }
        hooks.step(m, Step::DrawLists);
        // The drawing over (the view stage above is the machine's part of
        // it): what the machine writes from here to the frame's end comes
        // after it in the original, so it, not the copy's write, is what
        // the original holds (`after_drawing`).
        if self.page.is_some() {
            m.mem.track();
            let frame = r(m, crate::mif::var::BUFFER_SEG);
            m.mem.watch_frame(frame, r(m, crate::level::var::EMS_MAPPED) >> 2);
        }
        m.mem.w8(DS, STAGE, 8);
        hooks.step(m, Step::DrawFlats);
        m.mem.w16(DS, 0xC394, reach);
        m.mem.w8(DS, STAGE, 9);
        // A8FB: outdoors, the snow (BB09 82h) or the rain (81h) moves.
        if r(m, MAP_FLAGS) & 4 != 0 {
            match m.mem.r8(DS, 0xBB09) {
                0x82 => {
                    if !hooks.step(m, Step::Snow) {
                        m.snow_drops(self.page.as_deref_mut())?;
                        hooks.snow(m);
                    }
                }
                0x81 => {
                    if !hooks.step(m, Step::Rain) {
                        m.rain_drops(self.page.as_deref_mut())?;
                        hooks.rain(m);
                    }
                }
                _ => {}
            }
        }
        m.mem.w8(DS, STAGE, 0x0A);
        if let Some(page) = self.page.as_deref() {
            // The drawing's page is the original's draw page: its 3D view
            // there, for what the screens save of it (`page_view_put`).
            m.page_view_put(page);
            hooks.drawn(m, page);
        }
        // A3AA..A3BE: the physics from the real position.
        copy_words(m, 0xBA62, X, 3);
        if !hooks.step(m, Step::Physics) {
            m.physics_frame(files, &mut Physics { input: &input, hooks })?;
        }
        copy_words(m, X, 0xBA62, 3);
        if r(m, MAP_FLAGS) & 0x80 != 0 {
            // Left in the physics (a door, a gate): what the machine wrote
            // since the drawing is its own as at a frame's end.
            self.after_drawing();
            return Some(Frame::Left);
        }
        m.mem.w8(DS, STAGE, 0x0B);
        // 0000:A919: the eye 1Eh behind the player (the sine table at B03C, the
        // cosine's at B13C; the high word of each product).
        let b = (r(m, HEADING).wrapping_add(0x100) & 0x1FF).wrapping_mul(2);
        let s = r(m, 0xB03C_u16.wrapping_add(b)) as i16 as i32;
        let c = r(m, 0xB13C_u16.wrapping_add(b)) as i16 as i32;
        let x = r(m, X).wrapping_add(((0x1E * s) >> 16) as u16);
        let y = r(m, Y).wrapping_add(((0x1E * c) >> 16) as u16);
        m.mem.w16(DS, X, x);
        m.mem.w16(DS, Y, y);
        clip_back(m);
        if !hooks.step(m, Step::TempWalls) {
            m.temp_walls_frame();
        }
        m.mem.w8(DS, STAGE, 0x0C);
        world_hook(m, files, hooks, Step::FrameBody, 0xC3A8)?;
        m.mem.w8(DS, STAGE, 0x0D);
        m.view_window();
        let info = r(m, crate::info::var::INFO_PTR);
        if info != 0 && !hooks.step(m, Step::InfoLoad) {
            m.info_load(files, info)?;
        }
        if !hooks.step(m, Step::Present) {
            m.present_shake();
            hooks.present(m);
        }
        m.mem.w16(DS, 0xCB5E, 0);
        clip_back(m);
        m.mem.w8(DS, STAGE, 0x0E);
        // A431..A444: the second tick waited (the front end's), counted.
        if r(m, LEVEL_FLAGS) & 0x400 == 0 {
            let t = r(m, 0xBA60).wrapping_add(1);
            m.mem.w16(DS, 0xBA60, t);
        }
        let c = r(m, 0xCCBC).wrapping_add(1);
        m.mem.w16(DS, 0xCCBC, c);
        // 0000:A44A: the objects' lists emptied for the next frame (C019;
        // its sprites file themselves again); without it the node pool
        // fills up and filing a projectile (0000:BFC0) runs away.
        m.object_lists_clear();
        self.after_drawing();
        let m = &self.m;
        Some(if r(m, MAP_FLAGS) & 0x80 != 0 { Frame::Left } else { Frame::Running })
    }

    /// The machine's writes since the drawing (tracked from its end) made
    /// the machine's own in `page_state`: the original did them after its
    /// drawing, over what the drawing left (the bank mapped last, 8FA8, the
    /// faces' 19h). Not the flags the frame sets and
    /// clears bits of (C398, CCCA): the machine's read-modify-write there
    /// worked on its own bytes, without the drawing's bits, which the
    /// original's kept.
    fn after_drawing(&mut self) {
        if self.page.is_none() {
            return;
        }
        for o in self.m.mem.written_offsets() {
            if !DRAWN_BITS.contains(&o) {
                self.page_wrote[o as usize] = None;
            }
        }
        self.m.mem.untrack();
        // The EMS frame's likewise, by the bank mapped as each was written
        // (the sky's images read again into bank 0Eh by a relight in the
        // frame body, over what the drawing's sky hook changed in them).
        for o in self.m.mem.frame_written() {
            self.page_ems_wrote.remove(&o);
        }
    }

    /// The end of the world's entry and the loop's start: the view's
    /// window (0000:A5D1, which both ways in call, A250 and A28D) and the
    /// clip put back (A606 at A2A0), then the player's position kept
    /// (A2A3: BA62), which each frame's physics start from.
    pub fn world_loop_start(&mut self) {
        self.m.view_window();
        clip_back(&mut self.m);
        self.world_loop_kept();
    }

    /// 0000:A2A3: the player's position kept (BA62), which each frame's
    /// physics start from.
    pub fn world_loop_kept(&mut self) {
        copy_words(&mut self.m, X, 0xBA62, 3);
    }

    /// 0000:A26B..A297 and the state part of A4ED: entering a 3D area
    /// whose level is loaded (`level_load`), with its INF named at DS:`inf`
    /// (0 none). The timer restarts, the ceiling is 64h above the floor
    /// (D80A, D80C), the player stands (3Ch) at the base walking speed,
    /// the EMS banks are freed but the level's (5D74), floors from C00h
    /// past the EMS frame are deep (CCCE), no squares are changing, the
    /// player is on dry ground (CB5B), rain or snow gets its 100 drops
    /// in the view (E322, weather 82h), the buffer block is allocated
    /// (A516, `buffer_block_alloc`), and the INF loads. The rest of
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
        m.buffer_block_alloc()?;
        m.draw_buffers_alloc()?;
        m.light_table_make()?;
        // A523: the row tables made (DA8E), before the INF's ceiling.
        m.rows_note();
        let info = m.mem.r16(DS, crate::info::var::INFO_PTR);
        if info != 0 {
            m.info_load(&mut self.files, info)?;
            m.mem.w16(DS, crate::info::var::INFO_PTR, 0);
        }
        self.world_loop_start();
        Some(())
    }

    /// What the renderer needs most often.
    /// DGROUP as the original's would hold it, its drawing done on the
    /// machine: the machine's bytes, with the drawing's copy's last write
    /// where the machine's byte is still what the copy started from.
    pub fn page_state(&self) -> Vec<u8> {
        (0..=0xFFFFu16)
            .map(|o| {
                let b = self.m.mem.r8(DS, o);
                match self.page_wrote[o as usize] {
                    Some((before, after)) if b == before => after,
                    _ => b,
                }
            })
            .collect()
    }

    /// EMS bank `b` as the original's would hold it, its drawing done on
    /// the machine (`page_state`'s rule): the machine's bytes, with the
    /// drawing's copy's last change where the machine's byte is still what
    /// the copy started from.
    pub fn page_ems(&self, b: u16) -> Vec<u8> {
        let mut bank = self.m.ems_bank(b);
        for (i, x) in bank.iter_mut().enumerate() {
            if let Some(&(before, after)) = self.page_ems_wrote.get(&((b as u32) << 16 | i as u32)) {
                if *x == before {
                    *x = after;
                }
            }
        }
        bank
    }

    pub fn view(&self) -> View {
        View::of(&self.m)
    }
}

impl View {
    /// The view of the machine's state (`Engine::view`).
    pub fn of(m: &Machine) -> View {
        let r = |at| m.mem.r16(DS, at);
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

    /// 0000:0896. Outdoors (C398 bit 4), the INF to load (CB84): in the
    /// wilderness (C39A bit 200h) the one for the climate (DA8: 0, 4 or
    /// 8) and weather (BB09, low 7 bits), from the table at 3CF8 (eight
    /// bytes a name, three weathers a climate); in a city the one at
    /// 3C98 by the weather plus the climate (four a climate), weather 1
    /// with BB09 bit 7 taken as 3. Returns whether the townspeople's
    /// graphics then load (a city: 0000:17F9, the front end's).
    pub fn outdoor_inf(&mut self) -> bool {
        if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 {
            return false;
        }
        let climate = self.mem.r16(DS, 0x0DA8);
        let weather = self.mem.r8(DS, 0xBB09);
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            // mul cl: AL = 3 x (DA8 >> 2) low byte, AH the high.
            let ax = 3u16.wrapping_mul((climate >> 2) & 0xFF);
            let i = ax.wrapping_add((weather & 0x7F) as u16);
            self.mem.w16(DS, crate::info::var::INFO_PTR, (i << 3).wrapping_add(0x3CF8));
            return false;
        }
        let mut al = (weather & 0x7F) as u16;
        if weather & 0x80 != 0 && al == 1 {
            al = 3;
        }
        let i = al.wrapping_add(climate);
        self.mem.w16(DS, crate::info::var::INFO_PTR, (i << 3).wrapping_add(0x3C98));
        true
    }

    /// 0000:C980, the walk over the squares in view, for the doors (the
    /// rest it does is drawing, and the changed squares it steps are
    /// `EngineHooks::draw`'s), with C398 bit 20h as the world loop sets
    /// it (0000:A36F: when the player isn't below the floor, height C3D2
    /// negative). Every square in the map within C394 squares of the
    /// player's (CCA4, CCA6) is met once, ring by ring from the farthest
    /// (`walk_order`). A door (kind 0Bh) is met twice, from its side
    /// facing the player's row and then its column (0000:CB3F), each
    /// unless the neighbour that side is a full wall (both bytes alike,
    /// positive or kind 0Dh; 0000:CCC3), and steps each time it is met
    /// (`door_drawn`). Not with level flag C39A bit 4, whose squares the
    /// original handles otherwise (0000:CC76). Returns the sounds the
    /// door hook was called with, in order, each with its door's square. A reach of 0 isn't handled
    /// (the original walks 65535 rings).
    pub fn view_walk_doors(&mut self) -> Vec<(u16, u16)> {
        let mut sounds = Vec::new();
        let reach = self.mem.r16(DS, 0xC394);
        if (self.mem.r16(DS, HEIGHT) as i16) < 0 || self.mem.r16(DS, LEVEL_FLAGS) & 4 != 0 || reach == 0 {
            return sounds;
        }
        let (w, d) = (self.mem.r16(DS, 0xC357), self.mem.r16(DS, 0xC359));
        let (pc, pr) = (self.mem.r16(DS, 0xCCA4), self.mem.r16(DS, 0xCCA6));
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let mut last = None;
        for (dc, dr) in walk_order(reach) {
            let (col, row) = ((dc as u16).wrapping_add(pc), (dr as u16).wrapping_add(pr));
            if (col as i16) < 0 || (row as i16) < 0 || col >= w || row >= d || last == Some((col, row)) {
                continue;
            }
            last = Some((col, row));
            let sq = (row << 8).wrapping_add(col << 1);
            let v = self.mem.r16(walls, sq);
            if v >> 12 != 0xB {
                continue;
            }
            let by_row = if row < pr { 0x100 } else { 0xFF00 };
            let by_col = if col < pc { 2 } else { 0xFFFE };
            for side in [by_row, by_col] {
                let n = self.mem.r16(walls, sq.wrapping_add(side));
                if n as u8 == (n >> 8) as u8 && ((n as i16) > 0 || n >> 12 == 0xD) {
                    continue;
                }
                sounds.extend(self.door_drawn(sq).map(|s| (s, sq)));
            }
        }
        sounds
    }
}

/// The order 0000:C980 meets the squares in, as (column, row) from the
/// player's: each ring from `reach` in to 1, then the player's square. A
/// ring goes by k from 0 to r, a = k - r: (a, -r), (-a, -r), (-a, r),
/// (a, r), then unless a = -r (r, a) mirrored likewise, (-r, a), (r, a),
/// and unless a = 0, (r, -a), (-r, -a). The squares met twice running
/// (a = 0) are skipped where they are met (CD14, D744).
pub fn walk_order(reach: u16) -> Vec<(i16, i16)> {
    let mut v = Vec::new();
    for r in (1..=reach as i16).rev() {
        for k in 0..=r {
            let a = k - r;
            v.extend([(a, -r), (-a, -r), (-a, r), (a, r)]);
            if a != -r {
                v.extend([(-r, a), (r, a)]);
                if a != 0 {
                    v.extend([(r, -a), (-r, -a)]);
                }
            }
        }
    }
    v.push((0, 0));
    v
}
