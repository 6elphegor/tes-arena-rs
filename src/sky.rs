//! The sky's colours (segment 2294, 0341..047A).
//!
//! Outdoors the game keeps a .COL file at segment 43C0: DAYTIME.COL, or
//! DREARY.COL in any weather but clear. It is not a palette but a run of
//! sky colours through the day (RGB, eight bits). Palette entries 1..8
//! are set to eight of them in a row, a window that slides with the time
//! of day (CCFE, 0..3FFh: from 100h to 300h, half a colour a unit; the
//! first eight otherwise), and the sky is drawn with those entries
//! through a dither pattern (DITHER.IMG before noon, DITHER2.IMG after,
//! at CD04: 40 x 20 indices running 1..8 down the sky, or 8..1). As the
//! entries are the palette's, anything else drawn in colours 1..8
//! changes with them.
//!
//! At night the textures darken: the light level (0000:0390, `sky_light`)
//! is a row of the light table they are remapped through, 10 at night,
//! stepping to none over dawn and dusk.
//!
//! The file loads and the palette writes are the front end's: these
//! routines keep the game's record of what is loaded and say what to do.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The .COL loaded (its name's offset; FFFFh none), the dither image
    /// loaded, and the window's offset into the .COL (three bytes a
    /// colour; FFFFh none).
    pub const COLOURS: u16 = 0x3C85;
    pub const DITHER: u16 = 0x3C89;
    pub const WINDOW: u16 = 0x3C8B;
    /// The names.
    pub const DREARY: u16 = 0x42A2;
    pub const DAYTIME: u16 = 0x42AD;
    pub const DITHER_MORNING: u16 = 0x428B;
    pub const DITHER_AFTERNOON: u16 = 0x4296;
    /// The light level outdoors (a row of the light table, in the high
    /// byte; FFFFh full day), and the dawn and dusk steps (16 words).
    pub const LIGHT: u16 = 0x3C87;
    pub const LIGHT_STEPS: u16 = 0x3D46;
    /// The time of day (0..3FFh) and the weather.
    pub const TIME: u16 = 0xCCFE;
    pub const WEATHER: u16 = 0xBB09;
}
use var::*;

/// What the front end does after `sky_step`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkyStep {
    /// Set palette entries 1..8 from the .COL at the window (WINDOW).
    pub palette: bool,
    /// Load this dither image (its name's offset) to CD04.
    pub dither: Option<u16>,
}

impl Machine {
    /// 2294:0428. Outdoors (C398 bit 4), the .COL for the weather:
    /// DAYTIME.COL when clear (BB09 0), else DREARY.COL. Returns its
    /// name when it isn't the one loaded (recorded at COLOURS): the front
    /// end loads it to 43C0, copies its first eight colours past its end
    /// (+300h, where the window runs on to), and sets palette entries
    /// 1..8 from the window.
    pub fn sky_colours(&mut self) -> Option<u16> {
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 4 == 0 {
            return None;
        }
        let name = if self.mem.r8(DS, WEATHER) == 0 { DAYTIME } else { DREARY };
        if self.mem.r16(DS, COLOURS) == name {
            return None;
        }
        self.mem.w16(DS, COLOURS, name);
        Some(name)
    }

    /// 0000:0390's light level for the time of day, stored at LIGHT and
    /// returned (indoors it is left as it is): FFFFh by day (110h..2F0h), at
    /// dawn (100h..10Fh) and dusk (2F1h..300h) the step at LIGHT_STEPS for
    /// the time from sunrise or to sunset (0A00h down to 0), 0A00h at
    /// night. While it isn't FFFFh the textures are remapped through
    /// that row of the light table (0000:02F2, 2294:032E), as the front
    /// end does.
    pub fn sky_light(&mut self) -> u16 {
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 4 == 0 {
            return self.mem.r16(DS, LIGHT);
        }
        let t = self.mem.r16(DS, TIME);
        let level = if !(0x100..=0x300).contains(&t) {
            0xA00
        } else if t < 0x110 {
            self.mem.r16(DS, LIGHT_STEPS + 2 * (t - 0x100))
        } else if t > 0x2F0 {
            self.mem.r16(DS, LIGHT_STEPS + 2 * (0x300 - t))
        } else {
            0xFFFF
        };
        self.mem.w16(DS, LIGHT, level);
        level
    }

    /// 2294:0341. The window for the time of day, and the dither for the
    /// half of the day.
    pub fn sky_step(&mut self) -> SkyStep {
        let t = self.mem.r16(DS, TIME);
        let window = if t > 0x100 && t < 0x300 { ((t - 0x100) >> 1).wrapping_mul(3) } else { 0 };
        let mut step = SkyStep::default();
        if self.mem.r16(DS, WINDOW) != window {
            self.mem.w16(DS, WINDOW, window);
            step.palette = true;
        }
        let dither = if t < 0x200 { DITHER_MORNING } else { DITHER_AFTERNOON };
        if self.mem.r16(DS, DITHER) != dither {
            self.mem.w16(DS, DITHER, dither);
            step.dither = Some(dither);
        }
        step
    }
}
