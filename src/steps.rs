//! The walk's bob and its footsteps (0000:FCF6, called by the world loop
//! at 0000:A2D7, after the player's physics and before the drawing).
//!
//! Each time the player has gone more than 46h from where the last step
//! was counted (the game's distance: the larger axis plus a quarter of
//! the smaller), a step is counted: the step's place kept, the bob's
//! position (0..7) moved on. The bob runs on by itself while its table
//! (E7A8, eight words) is not 0 there, and its value is added to the view's
//! horizon (C3D8) unless C39A bit 40h. A step counted makes a footstep
//! (0000:FD77) unless C398 bit 800h (no bob at all) or 400h: the sound
//! C3C4 (none when negative) through the sound hook (CD46) at the player's
//! square; every other step is silent when in water (C39A 10h) with 4000h.

use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{LEVEL_FLAGS, MAP_FLAGS, SOUND_HOOK, X, Y};

/// DGROUP variables.
pub mod var {
    /// Where the last step was counted (x, y), and the bob's position.
    pub const STEP_X: u16 = 0xE7A2;
    pub const STEP_Y: u16 = 0xE7A4;
    pub const BOB_AT: u16 = 0xE7A6;
    /// The bob's table: eight words added to the horizon.
    pub const BOB_TABLE: u16 = 0xE7A8;
    /// The view's horizon row.
    pub const HORIZON: u16 = 0xC3D8;
    /// The footstep's sound (a signed byte) and the step's parity.
    pub const FOOTSTEP: u16 = 0xC3C4;
    pub const PARITY: u16 = 0xE39D;
    /// The player's square (a wall plane offset).
    pub const SQUARE: u16 = 0xCB90;
}
use var::*;

impl Machine {
    /// 0000:A5D1. The view's window for the frame: the clip (A914..A91A)
    /// saved at BA58 and set from the window (C39C..C3A2), the centre
    /// column (C3D6) its middle plus CB86, the horizon (C3D8) its middle
    /// row, which the walk's bob then moves.
    pub fn view_window(&mut self) {
        for i in 0..8u16 {
            let b = self.mem.r8(DS, 0xA914 + i);
            self.mem.w8(DS, 0xBA58 + i, b);
        }
        for i in 0..8u16 {
            let b = self.mem.r8(DS, 0xC39C + i);
            self.mem.w8(DS, 0xA914 + i, b);
        }
        let r = |m: &Machine, at: u16| m.mem.r16(DS, at);
        let c = (r(self, 0xA918).wrapping_add(r(self, 0xA914)) >> 1).wrapping_add(r(self, 0xCB86));
        self.mem.w16(DS, 0xC3D6, c);
        let h = r(self, 0xA91A).wrapping_add(r(self, 0xA916)) >> 1;
        self.mem.w16(DS, HORIZON, h);
    }

    /// 0000:FCF6..FDA9. The walk's bob for a frame; Some((sound, square))
    /// for a footstep the sound hook (CD46, 0000:A8EF) is called with,
    /// when it is set. (BX is 1Eh at that call; 23BA:002A doesn't read
    /// it.)
    pub fn walk_bob(&mut self) -> Option<(u16, u16)> {
        let r = |m: &Machine, at: u16| m.mem.r16(DS, at);
        let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
        let mut ax = abs(r(self, STEP_X).wrapping_sub(r(self, X)));
        let mut bx = abs(r(self, STEP_Y).wrapping_sub(r(self, Y)));
        if ax >= bx {
            std::mem::swap(&mut ax, &mut bx);
        }
        let d = (ax >> 2).wrapping_add(bx);
        let mut stepped = false;
        if d > 0x46 {
            let b = r(self, BOB_AT).wrapping_add(1) & 7;
            self.mem.w16(DS, BOB_AT, b);
            let (x, y) = (r(self, X), r(self, Y));
            self.mem.w16(DS, STEP_X, x);
            self.mem.w16(DS, STEP_Y, y);
            stepped = true;
        }
        if r(self, MAP_FLAGS) & 0x800 != 0 {
            return None;
        }
        let mut v = r(self, BOB_TABLE.wrapping_add(r(self, BOB_AT).wrapping_mul(2)));
        if v != 0 {
            let b = r(self, BOB_AT).wrapping_add(1) & 7;
            self.mem.w16(DS, BOB_AT, b);
            v = r(self, BOB_TABLE.wrapping_add(b.wrapping_mul(2)));
        }
        if r(self, LEVEL_FLAGS) & 0x40 == 0 {
            let h = r(self, HORIZON).wrapping_add(v);
            self.mem.w16(DS, HORIZON, h);
        }
        if !stepped || r(self, MAP_FLAGS) & 0x400 != 0 {
            return None;
        }
        self.footstep()
    }

    /// 0000:FD77. The footstep: sound C3C4 (none when negative); the
    /// parity flips, and with it set a step in water (C39A 10h) with
    /// 4000h makes none.
    fn footstep(&mut self) -> Option<(u16, u16)> {
        let sound = self.mem.r8(DS, FOOTSTEP) as i8 as i16 as u16;
        if (sound as i16) < 0 {
            return None;
        }
        let p = self.mem.r8(DS, PARITY) ^ 1;
        self.mem.w8(DS, PARITY, p);
        if p != 0 {
            let f = self.mem.r16(DS, LEVEL_FLAGS);
            if f & 0x10 != 0 && f & 0x4000 != 0 {
                return None;
            }
        }
        if !self.hook_set(SOUND_HOOK) {
            return None;
        }
        Some((sound, self.mem.r16(DS, SQUARE)))
    }
}
