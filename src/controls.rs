//! The player's controls (2826:0002, the physics frame's input hook C3B4).
//!
//! The original steers by the mouse: the view (320 x 147) is a 3 x 3
//! grid of regions (`REGIONS`, 79D8), and holding the left button in one
//! runs its handler: forward (top centre), back (bottom centre), turn
//! left and right (middle row), strafe (bottom corners), or forward while
//! turning (top corners). How far the cursor sits towards the region's
//! outer edge sets the speed. The centre is for clicking (`view_click`),
//! and a right click there looks at the wall (`look`). The arrow keys
//! work by calling the same handlers with the cursor pushed to the edge
//! of the region (Ctrl turns left/right into strafing), and J (or both
//! buttons) jumps.
//!
//! Each handler comes down to two primitives, which a modern front end
//! can drive directly: `walk` (a speed and a heading, with the
//! original's acceleration) and `turn`. The cursor's speed fraction is
//! 0..256 (`fraction` below).
//!
//! The mouse driver's state is input: the front end writes the cursor
//! (8F92, 8F94, in 320 x 200 pixels) and the buttons (AFF9: bit 0 left,
//! bit 1 right) with `set_mouse`, and the keys (one byte per scan code
//! at ACB5) with `set_key`. The cursor drawing (0000:541E, 77EC), the
//! and the stat bars' pixels (2901:00A8: their state is
//! `ControlHooks::bars`) are the renderer's; the view refreshes
//! (0000:DA8A) note the row tables' inputs (`Machine::rows_note`). The
//! Logitech CyberMan (2F26, when 8BEC is set) is not ported.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{CEILING, HEADING, HEIGHT, LEVEL_FLAGS, MAP_FLAGS, X, Y};

/// DGROUP variables.
pub mod var {
    /// The mouse driver's cursor (pixels) and buttons, the buttons last
    /// frame, and the releases seen this frame (left, right).
    pub const MOUSE_X: u16 = 0x8F92;
    pub const MOUSE_Y: u16 = 0x8F94;
    pub const BUTTONS: u16 = 0xAFF9;
    pub const BUTTONS_LAST: u16 = 0x7926;
    pub const RELEASED_LEFT: u16 = 0x7924;
    pub const RELEASED_RIGHT: u16 = 0x7925;
    /// The cursor's limits (x min, x max, y min, y max).
    pub const MOUSE_LIMITS: u16 = 0x8F74;
    /// Key states: one byte per scan code, set while held.
    pub const KEYS: u16 = 0xACB5;
    /// The region handler under the cursor (77Eh: the centre, or the
    /// cursor is outside the view) and the last movement handler.
    pub const HANDLER: u16 = 0x7A54;
    pub const LAST_HANDLER: u16 = 0x7A56;
    /// The speed: the handler's (CCBA) and the acceleration factor
    /// (0..100h, 7A46). The base walking and turning speeds.
    pub const SPEED: u16 = 0xCCBA;
    pub const ACCEL: u16 = 0x7A46;
    pub const WALK: u16 = 0xCCBE;
    pub const TURN: u16 = 0xC392;
    /// The jump counter (byte, 10 down to 0) and its height steps.
    pub const JUMP: u16 = 0x05E3;
    pub const JUMP_STEPS: u16 = 0x7A58;
    /// The levitation bob: step (0..7) and offsets.
    pub const BOB_STEP: u16 = 0x7A6C;
    pub const BOB_TABLE: u16 = 0x7A6E;
    /// Set when the view must be refreshed.
    pub const REFRESH: u16 = 0x127C;
    /// The view's height in pixels (the cursor below it is on the
    /// interface bar).
    pub const VIEW_HEIGHT: u16 = 0xA91A;
    /// The cursor shown: position, size and image (far pointer), and the
    /// image list's segment. 7A48 is FFFFh when the arrow is not shown.
    pub const CURSOR: u16 = 0x7A48;
    pub const CURSOR_IMAGES: u16 = 0x1002;
    /// The map square under the cursor (set by the renderer), and the
    /// player's.
    pub const PICKED: u16 = 0xCB92;
    pub const SQUARE: u16 = 0xCB90;
    /// A pending teleport (x, y; x nonzero).
    pub const TELEPORT: u16 = 0x43CA;
    /// The CyberMan is present.
    pub const CYBERMAN: u16 = 0x8BEC;
    pub const CYBERMAN_ACTIVE: u16 = 0x8F36;
}
use var::*;

/// The region table: 9 records of (handler, cursor image, x0, y0, x1,
/// y1), rows top to bottom.
pub const REGIONS: u16 = 0x79D8;

/// Scan codes the controls read.
pub mod key {
    pub const CTRL: u8 = 0x1D;
    pub const J: u8 = 0x24;
    pub const LSHIFT: u8 = 0x2A;
    pub const RSHIFT: u8 = 0x36;
    pub const UP: u8 = 0x48;
    pub const LEFT: u8 = 0x4B;
    pub const RIGHT: u8 = 0x4D;
    pub const DOWN: u8 = 0x50;
}

/// The region handlers (2826 offsets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    ForwardLeft,
    Forward,
    ForwardRight,
    TurnLeft,
    Centre,
    TurnRight,
    StrafeLeft,
    Back,
    StrafeRight,
}

impl Handler {
    pub fn from_offset(o: u16) -> Option<Handler> {
        Some(match o {
            0x0750 => Handler::ForwardLeft,
            0x055A => Handler::Forward,
            0x0768 => Handler::ForwardRight,
            0x06A0 => Handler::TurnLeft,
            0x077E => Handler::Centre,
            0x06FA => Handler::TurnRight,
            0x05FE => Handler::StrafeLeft,
            0x05A9 => Handler::Back,
            0x0651 => Handler::StrafeRight,
            _ => return None,
        })
    }
}

const CENTRE: u16 = 0x077E;

/// What the controls leave to code not ported here (and, through the
/// walk, the stairs' hooks).
pub trait ControlHooks: crate::hooks::StairsHooks {
    /// 2826:0008, 2901:00A8: the stat bars drawn, the state they leave
    /// (`Machine::bars_state`: 126C, 9288). Nothing by default, as
    /// gen_input.py stubs it.
    fn bars(&mut self, _m: &mut Machine) {}
    /// 2826:0B52: the map square (offset) of the floor under the cursor,
    /// None when there is none: the cursor projected onto the floor
    /// (`Machine::floor_pick`).
    fn floor_pick(&mut self, m: &mut Machine) -> Option<u16> {
        match m.floor_pick() {
            Some(v) => v,
            None => crate::giveup::original("2826:0B52: the floor's projection divides past a word (0000:DB43)"),
        }
    }
    /// With the steering controls (`Machine::player_steer`), a left click
    /// in the view the front end has waiting, taken where the classic
    /// controls take theirs (2826:0128, `view_click`): the front end sets
    /// the mouse and calls `Machine::view_click`. By default none.
    fn take_click(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:43B1: a click on a wall may run its trigger (texts, riddles).
    fn wall_trigger(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 29CF:0000: the spell waiting for a target is cast (`cast::
    /// resolve_cast`; only aimed and area spells come this way, so the
    /// touch is never used).
    fn cast_at_target(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.resolve_cast(files, crate::cast::Touch::Record(crate::mem::DS, crate::character::PLAYER)).map(|_| ())
    }
    /// 1194:42EA, 59C8: the text at DGROUP `text` in a message box.
    fn message_box(&mut self, _m: &mut Machine, _text: u16) {}
    /// 2826:0C15: a right click in the centre: looking at the square
    /// `ax` (CB92 with its low byte halved).
    fn look(&mut self, _m: &mut Machine, _ax: u16) -> Option<()> {
        Some(())
    }
    /// 2542:00BA: a click on the interface bar.
    fn bar_click(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 217B:0265: the wilderness moves its chunks when the player
    /// crosses one.
    fn wild_recentre(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 2826:0BB2: the counters of a spell waiting for a target, drawn
    /// over the view.
    fn spell_counters(&mut self, _m: &mut Machine) {}
}

/// What a modern front end asks of the player in a frame, in place of
/// the cursor regions and the arrow keys. Speeds are fractions of full
/// speed, -256..=256, as the original's cursor position gives them, so
/// the original's speeds, acceleration, and wilderness and race rules
/// still apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Intent {
    /// Forward (+) or back (-).
    pub forward: i16,
    /// Strafing right (+) or left (-).
    pub strafe: i16,
    /// Turning right (+) or left (-) at the original's turning speed.
    pub turn: i16,
    /// Turning by this many heading units as well (512 a full turn):
    /// mouse look.
    pub look: i16,
    /// Jump; with `run`, a running jump.
    pub jump: bool,
    pub run: bool,
}

/// `ControlHooks` that do nothing.
pub struct NoControlHooks;

impl crate::hooks::StairsHooks for NoControlHooks {}

impl ControlHooks for NoControlHooks {
    fn cast_at_target(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
}

/// Classic controls as the physics frame's input hook.
pub struct Classic<'a>(pub &'a mut dyn ControlHooks);

impl crate::movement::PhysicsHooks for Classic<'_> {
    fn input(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.player_input(files, self.0)
    }
}

impl Machine {
    /// The mouse driver's report: the cursor (pixels) and the buttons.
    pub fn set_mouse(&mut self, x: u16, y: u16, buttons: u8) {
        self.mem.w16(DS, MOUSE_X, x);
        self.mem.w16(DS, MOUSE_Y, y);
        self.mem.w8(DS, BUTTONS, buttons);
    }

    /// A key (scan code) pressed or released.
    pub fn set_key(&mut self, scan: u8, down: bool) {
        self.mem.w8(DS, KEYS.wrapping_add(scan as u16), down as u8);
    }

    fn key(&self, scan: u8) -> bool {
        self.mem.r8(DS, KEYS.wrapping_add(scan as u16)) != 0
    }

    fn region(&self, i: u16, field: u16) -> u16 {
        self.mem.r16(DS, REGIONS + i * 0xC + field)
    }

    /// 2826:0002. The player's input and movement for a frame. None where
    /// a move fails (`movement::move_step`), a speed divides by zero, a
    /// handler is unknown, or the CyberMan is present.
    pub fn player_input(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        self.player_frame(files, hooks, None)
    }

    /// The player's input and movement for a frame from a modern front
    /// end: `player_input` with `Intent` in place of the cursor regions
    /// and the arrow keys (see `intent_step`). The front end resolves
    /// clicks itself (picking what is under the cursor) and calls
    /// `view_click`.
    pub fn player_steer(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks, intent: &Intent) -> Option<()> {
        self.player_frame(files, hooks, Some(intent))
    }

    fn player_frame(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks, intent: Option<&Intent>) -> Option<()> {
        self.mem.w16(DS, CURSOR, 0xFFFF);
        hooks.bars(self);
        if self.mem.r16(DS, 0x061E) == 0xFFFE {
            self.mem.w16(DS, 0x061E, 0);
        }
        if self.mem.r16(DS, 0x0DC4) != 0 {
            // A spell waiting for a target: its prompt stays up.
            self.mem.w16(DS, 0x061E, 0xFFFE);
            self.mem.w16(DS, 0x061A, 0x7F35);
            self.mem.w16(DS, 0x061C, DS);
        }
        let v = self.mem.r16(DS, 0x05E0) & !0x40;
        self.mem.w16(DS, 0x05E0, v);
        self.mouse_poll();
        if self.mem.r16(DS, LEVEL_FLAGS) & 1 != 0 {
            self.levitation_bob();
        }
        let f = self.mem.r16(DS, MAP_FLAGS) & !0x200;
        self.mem.w16(DS, MAP_FLAGS, f);
        'body: {
            if self.jump_step(files, hooks)? {
                break 'body;
            }
            let f = self.mem.r16(DS, LEVEL_FLAGS) & !0x80;
            self.mem.w16(DS, LEVEL_FLAGS, f);
            let f = self.mem.r16(DS, MAP_FLAGS);
            if f & 0x100 != 0 {
                self.mem.w16(DS, MAP_FLAGS, f & !0x100);
                break 'body;
            }
            if f & 0x800 != 0 || self.mem.r8(DS, JUMP) != 0 {
                // 2826:0468: falling or jumping keeps the last speed.
                let (speed, heading) = (self.mem.r16(DS, SPEED), self.mem.r16(DS, HEADING));
                self.walk(files, hooks, speed, heading)?;
                return self.input_tail(files, hooks);
            }
            match intent {
                None => self.classic_step(files, hooks)?,
                Some(it) => self.intent_step(files, hooks, it)?,
            }
        }
        if self.mem.r16(DS, REFRESH) != 0 {
            self.mem.w16(DS, REFRESH, 0);
            // 0198: the row tables made (0000:DA8A).
            self.rows_note();
        }
        self.input_tail(files, hooks)
    }

    /// 2826:00AD..0182. The classic controls: the arrow keys, then the
    /// cursor's region (`pick_region`) and its handler while the left
    /// button is down (or the speed has not run out); in the centre a
    /// left click (`view_click`) or a right click (`look`); J or both
    /// buttons jump.
    // The jump's two tests are the original's two branches (2826), kept
    // apart as it has them.
    #[allow(clippy::if_same_then_else)]
    fn classic_step(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        self.mouse_clamp();
        self.arrow_keys(files, hooks)?;
        if self.mem.r8(DS, CYBERMAN) != 0 {
            return crate::giveup::not_ported("the CyberMan's input (segment 2F26) not ported");
        }
        if self.pick_region(files, hooks)? {
            // The cursor is on the interface bar: the handler is
            // the centre now, so nothing moves.
            return Some(());
        }
        let handler = self.mem.r16(DS, HANDLER);
        if handler == CENTRE {
            let v = self.mem.r16(DS, 0x05E0) | 0x40;
            self.mem.w16(DS, 0x05E0, v);
            let picked = self.mem.r16(DS, PICKED);
            if self.mem.r8(DS, RELEASED_RIGHT) != 0 && (picked as i16) > 0 {
                hooks.look(self, picked & 0xFF00 | (picked as u8 >> 1) as u16)?;
            }
            if self.mem.r8(DS, RELEASED_LEFT) == 0 {
                self.mem.w16(DS, SPEED, 0);
            } else if self.mem.r16(DS, 0x05E0) & 0x2000 == 0 {
                self.view_click(files, hooks)?;
            }
        } else if self.mem.r16(DS, ACCEL) != 0 || self.mem.r8(DS, BUTTONS) & 1 != 0 {
            if self.mem.r16(DS, 0x05E0) & 0x2000 == 0 {
                self.run_handler(files, hooks, handler)?;
            }
        } else {
            self.mem.w16(DS, SPEED, 0);
        }
        if self.key(key::J) {
            self.jump(self.key(key::LSHIFT) || self.key(key::RSHIFT));
        } else if self.mem.r16(DS, 0x05E0) & 0x10 == 0 && self.mem.r8(DS, BUTTONS) == 3 {
            self.jump(self.key(key::LSHIFT) || self.key(key::RSHIFT));
        }
        Some(())
    }

    /// The modern counterpart of `classic_step`: turning, then one walk
    /// along the combined direction of `forward` and `strafe` at the
    /// larger of their speeds (the original walks one region's direction
    /// a frame) as if the left button were down, so the acceleration
    /// builds; going back is at half speed. The speed is 0 after, moving
    /// or not, as the keys with the cursor in the centre leave it.
    fn intent_step(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks, it: &Intent) -> Option<()> {
        // Moving or turning holds 5E0 bit 2000h as the arrow keys do, so
        // the frame body doesn't brake the acceleration.
        let held = it.forward != 0 || it.strafe != 0 || it.turn != 0;
        let f = self.mem.r16(DS, 0x05E0) & !0x2000 | if held { 0x2000 } else { 0 };
        // The cursor is the view's centre's (the mouse held there, as the
        // classic controls' centre handler sets 5E0 bit 40h, 2826:00F6):
        // the bow (0000:392A) shoots only with it set.
        self.mem.w16(DS, 0x05E0, f | 0x40);
        if it.look != 0 {
            self.turn(it.look as u16);
        }
        if it.turn != 0 {
            let d = self.turn_speed(it.turn.unsigned_abs().min(0x100));
            self.turn(if it.turn < 0 { d.wrapping_neg() } else { d });
        }
        if it.forward != 0 || it.strafe != 0 {
            let fraction = it.forward.unsigned_abs().max(it.strafe.unsigned_abs()).min(0x100);
            let speed = self.walk_speed(fraction, it.forward < 0);
            // The direction in heading units (512 a turn, + to the right).
            let a = (it.strafe as f64).atan2(it.forward as f64);
            let offset = (a * 256.0 / std::f64::consts::PI).round() as i32 as u16;
            let h = self.mem.r16(DS, HEADING).wrapping_add(offset) & 0x1FF;
            self.mem.w16(DS, SPEED, speed);
            if it.strafe == 0 {
                self.mem.w16(DS, 0x05DE, h);
            }
            let buttons = self.mem.r8(DS, BUTTONS);
            self.mem.w8(DS, BUTTONS, buttons | 1);
            let r = self.walk(files, hooks, speed, h);
            self.mem.w8(DS, BUTTONS, buttons);
            r?;
            // As the arrow keys leave it (2826:0473 walks, then the
            // cursor's handler, the view's centre with no click, puts the
            // speed to 0: 015C): a fall or a jump after keeps none
            // (0468, 0226).
            self.mem.w16(DS, SPEED, 0);
        } else {
            self.mem.w16(DS, SPEED, 0);
        }
        // Where the classic controls' centre handler takes a left click
        // (2826:00F6..0128, after the moves): the front end's click, if one
        // is waiting, in this frame's input step as the original's.
        hooks.take_click(self, files)?;
        if it.jump {
            self.jump(it.run);
        }
        Some(())
    }

    /// 2826:019D..01D5.
    fn input_tail(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 && self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
            hooks.wild_recentre(self, files)?;
        }
        hooks.spell_counters(self);
        let tx = self.mem.r16(DS, TELEPORT);
        if tx != 0 {
            self.mem.w16(DS, X, tx);
            let ty = self.mem.r16(DS, TELEPORT + 2);
            self.mem.w16(DS, Y, ty);
            self.mem.w16(DS, HEIGHT, 0xFFEC);
            self.mem.w16(DS, TELEPORT, 0);
        }
        Some(())
    }

    /// 2826:0D85. Reads the mouse; a button's release this frame (it was
    /// down last frame) sets RELEASED_LEFT / RIGHT.
    fn mouse_poll(&mut self) {
        self.mouse_clamp();
        let now = self.mem.r8(DS, BUTTONS);
        let last = self.mem.r8(DS, BUTTONS_LAST);
        self.mem.w8(DS, BUTTONS_LAST, now);
        for (bit, at) in [(1, RELEASED_LEFT), (2, RELEASED_RIGHT)] {
            let released = now & bit == 0 && last & bit != 0;
            self.mem.w8(DS, at, released as u8);
        }
    }

    /// 0000:8FC8 after the driver: the cursor kept within its limits.
    fn mouse_clamp(&mut self) {
        let lim = |i: u16| self.mem.r16(DS, MOUSE_LIMITS + 2 * i);
        let (x0, x1, y0, y1) = (lim(0), lim(1), lim(2), lim(3));
        let clamp = |v: u16, lo: u16, hi: u16| {
            let v = if (v as i16) < 0 { 0 } else { v };
            let v = if v < lo { lo } else { v };
            if v >= hi { hi } else { v }
        };
        let x = clamp(self.mem.r16(DS, MOUSE_X), x0, x1);
        let y = clamp(self.mem.r16(DS, MOUSE_Y), y0, y1);
        self.mem.w16(DS, MOUSE_X, x);
        self.mem.w16(DS, MOUSE_Y, y);
    }

    /// 2826:0042. Levitating: the height bobs by an 8-step table, and
    /// never stays below standing height.
    fn levitation_bob(&mut self) {
        let step = self.mem.r16(DS, BOB_STEP).wrapping_add(1) & 7;
        self.mem.w16(DS, BOB_STEP, step);
        let d = self.mem.r16(DS, BOB_TABLE + 2 * step);
        let mut h = self.mem.r16(DS, HEIGHT).wrapping_add(d);
        self.mem.w16(DS, HEIGHT, h);
        if (h as i16) < 0x3C {
            h = h.wrapping_add(8);
            self.mem.w16(DS, HEIGHT, h);
        }
        self.mem.w16(DS, REFRESH, 1);
        if self.mem.r8(DS, crate::movement::var::CLIMB) == 0 && h as i16 <= 0x46 {
            self.mem.w8(DS, crate::movement::var::CLIMB, 1);
        }
    }

    /// 2826:026A. A jump in progress: the height follows the jump's
    /// steps (up to the ceiling) and the player keeps moving at the
    /// jump's speed scaled by agility (the player's byte 1CD). The
    /// counter runs down each frame, every other frame with feather fall
    /// (5CA bit 1000h) or as a race C9h. True while jumping.
    fn jump_step(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<bool> {
        let f = self.mem.r16(DS, LEVEL_FLAGS) & 0x7FFF;
        self.mem.w16(DS, LEVEL_FLAGS, f);
        let f = self.mem.r16(DS, MAP_FLAGS) & !0x100;
        self.mem.w16(DS, MAP_FLAGS, f);
        let j = self.mem.r8(DS, JUMP);
        if j == 0 {
            return Some(false);
        }
        let f = self.mem.r16(DS, LEVEL_FLAGS) | 0x8000;
        self.mem.w16(DS, LEVEL_FLAGS, f);
        let step = (j as u16).wrapping_sub(1).wrapping_mul(2);
        let d = self.mem.r16(DS, JUMP_STEPS.wrapping_add(step));
        let h = self.mem.r16(DS, HEIGHT).wrapping_add(d);
        self.mem.w16(DS, HEIGHT, h);
        let ceiling = self.mem.r16(DS, CEILING);
        if h as i16 >= ceiling as i16 {
            self.mem.w16(DS, HEIGHT, ceiling);
        }
        self.mem.w16(DS, REFRESH, 0);
        // 02A5.
        self.rows_note();
        let heading = self.mem.r16(DS, HEADING);
        let speed = (self.mem.r16(DS, SPEED) as u8 as u16 * self.mem.r8(DS, 0x01CD) as u16) >> 7;
        self.walk(files, hooks, speed, heading)?;
        let f = self.mem.r16(DS, MAP_FLAGS) | 0x100;
        self.mem.w16(DS, MAP_FLAGS, f);
        let slow = self.mem.r16(DS, 0x05CA) & 0x1000 != 0 || self.mem.r8(DS, 0x01A9) == 0xC9;
        let d = if slow && self.mem.r16(DS, 0xCCBC) & 1 != 0 { 0 } else { 0xFF };
        let j = self.mem.r8(DS, JUMP).wrapping_add(d);
        self.mem.w8(DS, JUMP, j);
        Some(true)
    }

    /// 2826:01DB. Starting a jump (J, or both buttons): not while
    /// jumping or climbing, nor below standing height; on a square with
    /// no wall, only if its floor is solid. Levitating, the jump instead
    /// ends the levitation (2826:0244). Holding Shift makes it a running
    /// jump.
    fn jump(&mut self, run: bool) {
        if self.mem.r8(DS, JUMP) != 0 || self.mem.r8(DS, crate::movement::var::CLIMB) != 0 {
            return;
        }
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x4000 != 0 {
            self.end_levitation();
            self.mem.w8(DS, JUMP, 0x0A);
            return;
        }
        if (self.mem.r16(DS, HEIGHT) as i16) < 0x3C {
            return;
        }
        let sq = self.mem.r16(DS, SQUARE);
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        if self.mem.r16(walls, sq) == 0 {
            let floors = self.mem.r16(DS, crate::movement::var::FLOOR_SEG);
            if self.mem.r16(floors, sq) >= self.mem.r16(DS, crate::movement::var::FLOOR_SOLID) {
                return;
            }
        }
        self.mem.w8(DS, JUMP, 0x0A);
        if !run {
            return;
        }
        if self.mem.r16(DS, SPEED) >= 0x1E {
            return;
        }
        self.mem.w16(DS, SPEED, 0x1E);
        self.mem.w16(DS, ACCEL, 0xC0);
    }

    /// 2826:0244. The levitation ends: the square under the player gets
    /// a wall of type 80h (the level's C942 + 1).
    fn end_levitation(&mut self) {
        let f = self.mem.r16(DS, LEVEL_FLAGS);
        if f & 0x4000 == 0 {
            return;
        }
        self.mem.w16(DS, LEVEL_FLAGS, f & !0x4000);
        let v = self.mem.r16(DS, 0x05CA) & !0x10;
        self.mem.w16(DS, 0x05CA, v);
        let sq = self.mem.r16(DS, SQUARE);
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let al = self.mem.r8(DS, 0xC942).wrapping_add(1);
        self.mem.w16(walls, sq, 0x8000 | al as u16);
    }

    /// 2826:0473. The arrow keys, unless the left button is down: each
    /// runs its region's handler with the cursor at the region's edge
    /// (full speed) and the left button held. Up and Down set the last
    /// handler (LAST_HANDLER).
    fn arrow_keys(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        let f = self.mem.r16(DS, 0x05E0) & !0x2000;
        self.mem.w16(DS, 0x05E0, f);
        let buttons = self.mem.r8(DS, BUTTONS);
        if buttons & 1 != 0 {
            return Some(());
        }
        self.mem.w8(DS, BUTTONS, 1);
        let held = |m: &mut Machine| {
            let f = m.mem.r16(DS, 0x05E0) | 0x2000;
            m.mem.w16(DS, 0x05E0, f);
        };
        if self.key(key::UP) {
            held(self);
            self.with_cursor(MOUSE_Y, 0, |m| m.forward(files, hooks))?;
            self.mem.w16(DS, LAST_HANDLER, 0x055A);
        } else if self.key(key::DOWN) {
            held(self);
            self.with_cursor(MOUSE_Y, 0x90, |m| m.back(files, hooks))?;
            self.mem.w16(DS, LAST_HANDLER, 0x05A9);
        }
        let ctrl = self.key(key::CTRL);
        if self.key(key::LEFT) {
            held(self);
            if ctrl {
                self.with_cursor(MOUSE_X, 0, |m| m.strafe_left(files, hooks))?;
            } else {
                self.with_cursor(MOUSE_X, 0, |m| m.turn_left(0x28, 0x2C))?;
            }
        } else if self.key(key::RIGHT) {
            held(self);
            if ctrl {
                self.with_cursor(MOUSE_X, 0x13C, |m| m.strafe_right(files, hooks))?;
            } else {
                self.with_cursor(MOUSE_X, 0x13C, |m| m.turn_right(0x40, 0x44))?;
            }
        }
        self.mem.w8(DS, BUTTONS, buttons);
        Some(())
    }

    fn with_cursor<T>(&mut self, at: u16, v: u16, f: impl FnOnce(&mut Machine) -> Option<T>) -> Option<T> {
        let saved = self.mem.r16(DS, at);
        self.mem.w16(DS, at, v);
        let r = f(self);
        self.mem.w16(DS, at, saved);
        r
    }

    /// 2826:02E2. A right click cancels a spell waiting for a target
    /// (the prompt's spell points come back unless F84 is 8000h). Then
    /// the region under the cursor becomes the handler (the previous
    /// movement handler is kept in LAST_HANDLER while the left button is
    /// down) and its cursor image is chosen. True (the original's carry)
    /// when the cursor is on the interface bar: the handler is then the
    /// centre, and a click goes to the bar.
    fn pick_region(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<bool> {
        if self.mem.r8(DS, RELEASED_RIGHT) != 0 {
            self.cancel_target();
        }
        let (x, y) = (self.mem.r16(DS, MOUSE_X), self.mem.r16(DS, MOUSE_Y));
        if y >= self.mem.r16(DS, VIEW_HEIGHT) {
            self.mem.w16(DS, HANDLER, CENTRE);
            self.mem.w8(DS, 0x79A9, 1);
            if self.mem.r8(DS, RELEASED_LEFT) | self.mem.r8(DS, RELEASED_RIGHT) != 0 {
                hooks.bar_click(self, files)?;
            }
            return Some(true);
        }
        self.mem.w8(DS, 0x79AA, 0);
        if self.mem.r8(DS, 0x79A9) != 0 {
            // (0000:541E removes the arrow cursor: the renderer's.)
            self.mem.w8(DS, 0x79A9, 0);
        }
        let (xs, ys) = (x as i16, y as i16);
        let Some(i) = (0..9).find(|&i| {
            let r = |f| self.region(i, f) as i16;
            xs >= r(4) && xs <= r(8) && ys >= r(6) && ys <= r(0xA)
        }) else {
            return Some(false);
        };
        let old = self.mem.r16(DS, HANDLER);
        self.mem.w16(DS, HANDLER, self.region(i, 0));
        if old != CENTRE && self.mem.r8(DS, BUTTONS) & 1 != 0 {
            self.mem.w16(DS, LAST_HANDLER, old);
        }
        // The cursor image: the list at [1002]:0 is records of 12 bytes
        // plus their data (length at +A); the size is at +4/+6.
        let seg = self.mem.r16(DS, CURSOR_IMAGES);
        let mut si = 0u16;
        for _ in 0..self.region(i, 2) {
            si = si.wrapping_add(self.mem.r16(seg, si.wrapping_add(0xA))).wrapping_add(0xC);
        }
        let (w, h) = (self.mem.r16(seg, si.wrapping_add(4)), self.mem.r16(seg, si.wrapping_add(6)));
        for (k, v) in [x, y, w, h, si.wrapping_add(0xC), seg].into_iter().enumerate() {
            self.mem.w16(DS, CURSOR + 2 * k as u16, v);
        }
        Some(false)
    }

    /// 2826:02EC..0374. A right click while a spell waits for a target.
    fn cancel_target(&mut self) {
        let r = |m: &Machine, o| m.mem.r16(DS, o);
        'cancel: {
            if r(self, 0x0DC4) == 0x7D0 {
                self.mem.w16(DS, 0x0DC4, 0);
                break 'cancel;
            }
            if r(self, 0x0DC0) != 0 || r(self, 0x0DC2) != 0 {
                self.mem.w16(DS, 0x0F84, 0);
            } else if r(self, 0x0DC4) == 0 && r(self, 0x0DC8) == 0 && r(self, 0x0620) == 0 {
                break 'cancel;
            }
            for o in [0x0DC4, 0x0DC8, 0x0DC0, 0x0DC2] {
                self.mem.w16(DS, o, 0);
            }
            let refund = r(self, 0x0F84);
            if refund != 0x8000 {
                let sp = r(self, 0x020A).wrapping_add(refund);
                self.mem.w16(DS, 0x020A, sp);
            }
            self.mem.w16(DS, 0x0620, 0);
            self.mem.w8(DS, 0x0A21, 0);
            let b = self.mem.r8(DS, 0x7DBA);
            self.mem.w8(DS, 0x05E0, b);
            self.mem.w8(DS, 0x1012, 0xFF);
            self.mem.w16(DS, 0x1947, 0x1E);
            self.player_weapon_class();
        }
        // 2542:028F: the message goes (when 5CA has any bit but 0 set;
        // bit 0 is the one cleared).
        let f = r(self, 0x05CA);
        if f & 0xFFFE != 0 {
            self.mem.w16(DS, 0x061E, 0);
            self.mem.w16(DS, 0x05CA, f & 0xFFFE);
        }
    }

    fn run_handler(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks, at: u16) -> Option<()> {
        match Handler::from_offset(at)? {
            Handler::ForwardLeft => {
                self.turn_left(4, 8)?;
                self.forward(files, hooks)
            }
            Handler::Forward => self.forward(files, hooks),
            Handler::ForwardRight => {
                self.turn_right(0x1C, 0x20)?;
                self.forward(files, hooks)
            }
            Handler::TurnLeft => self.turn_left(0x28, 0x2C),
            Handler::Centre => self.view_click(files, hooks),
            Handler::TurnRight => self.turn_right(0x40, 0x44),
            Handler::StrafeLeft => self.strafe_left(files, hooks),
            Handler::Back => self.back(files, hooks),
            Handler::StrafeRight => self.strafe_right(files, hooks),
        }
    }

    /// (ax << 8) / cx, the cursor's place in its region as a fraction of
    /// 256; None where the region is empty (the original's divide
    /// fault).
    fn fraction(ax: u16, cx: u16) -> Option<u16> {
        if cx == 0 {
            return crate::giveup::original("a divide by zero");
        }
        Some((ax << 8) / cx)
    }

    /// The walking speed for a fraction: WALK (halved going back),
    /// quartered in the wilderness (C39A bit 10h) unless levitating
    /// (4000h), where race 7 gets half.
    fn walk_speed(&self, fraction: u16, back: bool) -> u16 {
        let mut dx = self.mem.r16(DS, WALK);
        if back {
            dx >>= 1;
        }
        let f = self.mem.r16(DS, LEVEL_FLAGS);
        if f & 0x10 != 0 && f & 0x4000 == 0 {
            dx >>= 2;
            if self.mem.r8(DS, 0x01A8) == 7 {
                dx = dx.wrapping_add(dx);
            }
        }
        ((fraction as u32 * dx as u32) >> 8) as u16
    }

    /// The turning speed for a fraction: TURN, halved in the wilderness
    /// except for race 7.
    fn turn_speed(&self, fraction: u16) -> u16 {
        let mut dx = self.mem.r16(DS, TURN);
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x10 != 0 && self.mem.r8(DS, 0x01A8) != 7 {
            dx >>= 1;
        }
        ((fraction as u32 * dx as u32) >> 8) as u16
    }

    /// 2826:055A. Forward: faster towards the top of region 1.
    fn forward(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        let (y0, y1) = (self.region(1, 6), self.region(1, 0xA));
        let ax = y1.wrapping_sub(self.mem.r16(DS, MOUSE_Y));
        let speed = self.walk_speed(Self::fraction(ax, y1.wrapping_sub(y0))?, false);
        self.mem.w16(DS, SPEED, speed);
        let h = self.mem.r16(DS, HEADING);
        self.mem.w16(DS, 0x05DE, h);
        self.walk(files, hooks, speed, h)
    }

    /// 2826:05A9. Back: faster towards the bottom of region 7.
    fn back(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        let (y0, y1) = (self.region(7, 6), self.region(7, 0xA));
        let ax = self.mem.r16(DS, MOUSE_Y).wrapping_sub(y0);
        let speed = self.walk_speed(Self::fraction(ax, y1.wrapping_sub(y0))?, true);
        self.mem.w16(DS, SPEED, speed);
        let h = self.mem.r16(DS, HEADING).wrapping_add(0x100) & 0x1FF;
        self.mem.w16(DS, 0x05DE, h);
        self.walk(files, hooks, speed, h)
    }

    /// 2826:05FE. Strafing left: faster towards the left of region 6.
    fn strafe_left(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        let (x0, x1) = (self.region(6, 4), self.region(6, 8));
        let ax = x1.wrapping_sub(self.mem.r16(DS, MOUSE_X));
        let speed = self.walk_speed(Self::fraction(ax, x1.wrapping_sub(x0))?, false);
        self.mem.w16(DS, SPEED, speed);
        let h = self.mem.r16(DS, HEADING).wrapping_sub(0x80) & 0x1FF;
        self.walk(files, hooks, speed, h)
    }

    /// 2826:0651. Strafing right: faster towards the right of region 8.
    fn strafe_right(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        let (x0, x1) = (self.region(8, 4), self.region(8, 8));
        let ax = self.mem.r16(DS, MOUSE_X).wrapping_sub(x0);
        let speed = self.walk_speed(Self::fraction(ax, x1.wrapping_sub(x0))?, false);
        self.mem.w16(DS, SPEED, speed);
        let h = self.mem.r16(DS, HEADING).wrapping_add(0x80) & 0x1FF;
        self.walk(files, hooks, speed, h)
    }

    /// 2826:06A0 (and 06B1 from 0750). Turning left, only with the left
    /// button down: faster towards the left of the region whose x range
    /// is at REGIONS + `x0` / `x1`.
    fn turn_left(&mut self, x0: u16, x1: u16) -> Option<()> {
        let (x0, x1) = (self.mem.r16(DS, REGIONS + x0), self.mem.r16(DS, REGIONS + x1));
        if self.mem.r8(DS, BUTTONS) & 1 == 0 {
            return Some(());
        }
        let ax = x1.wrapping_sub(self.mem.r16(DS, MOUSE_X));
        let d = self.turn_speed(Self::fraction(ax, x1.wrapping_sub(x0))?);
        self.turn(d.wrapping_neg());
        Some(())
    }

    /// 2826:06FA (and 0709 from 0768). Turning right, faster towards the
    /// right of the region.
    fn turn_right(&mut self, x0: u16, x1: u16) -> Option<()> {
        let (x0, x1) = (self.mem.r16(DS, REGIONS + x0), self.mem.r16(DS, REGIONS + x1));
        if self.mem.r8(DS, BUTTONS) & 1 == 0 {
            return Some(());
        }
        let ax = self.mem.r16(DS, MOUSE_X).wrapping_sub(x0);
        let d = self.turn_speed(Self::fraction(ax, x1.wrapping_sub(x0))?);
        self.turn(d);
        Some(())
    }

    /// Turning by `d` (heading units, 512 a full turn; negative is
    /// left), unless the player is held (5B1 bit 40h).
    pub fn turn(&mut self, d: u16) {
        let mut h = self.mem.r16(DS, HEADING);
        if self.mem.r16(DS, 0x05B1) & 0x40 == 0 {
            h = h.wrapping_add(d);
        }
        self.mem.w16(DS, HEADING, h & 0x1FF);
        self.mem.w16(DS, REFRESH, 0);
        // 06F4 (074A): the row tables made.
        self.rows_note();
    }

    /// 2826:0D20. Walking at `speed` towards `heading`, unless held (5B1
    /// bit 40h). Outside a jump, the acceleration factor (ACCEL, 0..100h)
    /// grows by 20h while the left button is down and falls by 40h
    /// otherwise; the step is speed x ACCEL / 256 (the product's low
    /// word). See `movement::move_step`.
    pub fn walk(&mut self, files: &mut Files, hooks: &mut dyn crate::hooks::StairsHooks, speed: u16, heading: u16) -> Option<()> {
        if self.mem.r16(DS, 0x05B1) & 0x40 != 0 {
            return Some(());
        }
        if self.mem.r8(DS, JUMP) == 0 {
            let a = self.mem.r16(DS, ACCEL);
            if self.mem.r8(DS, BUTTONS) & 1 == 0 {
                let a = a.wrapping_sub(0x40);
                self.mem.w16(DS, ACCEL, if (a as i16) < 0 { 0 } else { a });
            } else if a < 0x100 {
                self.mem.w16(DS, ACCEL, a + 0x20);
            }
        }
        let step = speed.wrapping_mul(self.mem.r16(DS, ACCEL)) >> 8;
        self.move_step(files, hooks, step, heading)?;
        Some(())
    }
}
