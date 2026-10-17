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

use crate::bsa::Files;
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
    /// The segments of the day's colours (318h bytes: 300h and the first
    /// eight colours again) and of the dither image (8 x 100).
    pub const COLOURS_SEG: u16 = 0x43C0;
    pub const DITHER_SEG: u16 = 0xCD04;
    /// "dither.img", the first dither the game loads.
    pub const DITHER_FIRST: u16 = 0x4381;
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
    /// 0000:068A..06B2, the sky's state forgotten on entering an area
    /// (2294:0057): 427D FFh, 3C8D FFFFh, 8702 1, the .COL, the dither,
    /// the window and the light level loaded none (FFFFh), D56 FFFFh; the
    /// .COL for the weather then chosen again (06B8, 2294:0428:
    /// `sky_colours`, which the frame runs too).
    pub fn sky_reset(&mut self) {
        self.mem.w8(DS, 0x427D, 0xFF);
        self.mem.w16(DS, 0x3C8D, 0xFFFF);
        self.mem.w8(DS, 0x8702, 1);
        for at in [COLOURS, DITHER, WINDOW, 0x0D56, LIGHT] {
            self.mem.w16(DS, at, 0xFFFF);
        }
    }

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

    /// 2294:01F1's sky part (from the game's set-up, 21EE:016C): 318h
    /// bytes for the day's colours (0000:51E2, the segment at
    /// COLOURS_SEG), and dither.img loaded where 0000:7428 allocates it
    /// (DI FFFFh: 51EC of the file's size), its segment at DITHER_SEG.
    /// (Before them the routine loads the compass, the slider, the arrows
    /// and the keys, which are the front end's.) None where an allocation
    /// or the load fails. The loads below do nothing until it has run.
    pub fn sky_buffers(&mut self, files: &mut Files) -> Option<()> {
        let (seg, _) = self.heap_alloc_bytes(0x318)?;
        self.mem.w16(DS, COLOURS_SEG, seg);
        self.file_open_handle(files, DITHER_FIRST)?;
        let size = self.file_seek(files, 2, 0);
        self.file_close(files);
        let (seg, off) = self.heap_alloc_bytes(size as u16)?;
        self.file_load(files, DITHER_FIRST, seg, off)?;
        self.mem.w16(DS, DITHER_SEG, seg);
        Some(())
    }

    /// 2294:0341's load: the dither named at DS:`name` (`SkyStep::dither`)
    /// read to DITHER_SEG:0 (0000:7428).
    pub fn sky_dither_load(&mut self, files: &mut Files, name: u16) -> Option<()> {
        let seg = self.mem.r16(DS, DITHER_SEG);
        if seg == 0 {
            return Some(());
        }
        self.file_load(files, name, seg, 0).map(|_| ())
    }

    /// 2294:0428's load: the .COL named at DS:`name` (`sky_colours`) read
    /// as 0000:7530 reads one, to COLOURS_SEG:0: the file to 929E (7428),
    /// then 300h bytes, its own if it is 300h long, else past an 8-byte
    /// header each shifted right by 2; then its first 18h bytes again at
    /// +300h.
    pub fn sky_colours_load(&mut self, files: &mut Files, name: u16) -> Option<()> {
        if self.mem.r16(DS, COLOURS_SEG) == 0 {
            return Some(());
        }
        let (seg, _) = self.file_load(files, name, DS, 0x929E)?;
        let size = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE);
        let to = self.mem.r16(DS, COLOURS_SEG);
        for i in 0..0x300u16 {
            let b = if size == 0x300 {
                self.mem.r8(seg, 0x929E + i)
            } else {
                self.mem.r8(seg, 0x929E + 8 + i) >> 2
            };
            self.mem.w8(to, i, b);
        }
        for i in 0..0x18u16 {
            let b = self.mem.r8(to, i);
            self.mem.w8(to, 0x300 + i, b);
        }
        Some(())
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
