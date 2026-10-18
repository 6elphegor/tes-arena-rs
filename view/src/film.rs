//! What the original shows while a screen changes, frame by frame
//! (`Film`): the screen (the VGA's memory) and the colours (the DAC) at
//! each wait for the retrace, built from the routines' own steps: the
//! palette's fade (0000:5E50, the far 5E3E), its colours set (9B56) and
//! the dissolve (1194:4FF7).

use crate::hud::{H, W};
use arena::machine::Machine;
use arena::mem::DS;

/// The dissolve's generator (1194:5065: x 5E5h + 29h, 16 bits).
pub const DISSOLVE_SEED: u16 = 0x4B8E;
/// 4FF7's pixels (FA00h, whatever the rectangle).
pub const DISSOLVE_STEPS: u32 = 0xFA00;
/// A retrace, in milliseconds (70 Hz).
pub const RETRACE_MS: u32 = 14;
pub const FULL: (usize, usize, usize, usize) = (0, 0, W, H);

fn dissolve_next(r: u16) -> u16 {
    r.wrapping_mul(0x5E5).wrapping_add(0x29)
}

/// The colours as the DAC holds them (8-bit, as `Screen` keeps them).
pub type Dac = [[u8; 4]; 256];

/// A colour of the DAC (6-bit) as `Screen` keeps it.
pub fn dac8(v: u8) -> u8 {
    let v = v & 0x3F;
    v << 2 | v >> 4
}

/// What the original shows between two screens settled, frame by frame:
/// the screen (the VGA's memory) and the colours (the DAC) at each wait
/// for the retrace. Built from the routines' own steps, each on the
/// screen and colours the last left.
pub struct Film {
    pub vga: Vec<u8>,
    pub dac: Dac,
    pub frames: Vec<(Vec<u8>, Dac)>,
}

impl Film {
    pub fn new(vga: Vec<u8>, dac: Dac) -> Film {
        Film { vga, dac, frames: Vec::new() }
    }

    /// A wait for the retrace: the screen shown as it is.
    pub fn wait(&mut self) {
        self.frames.push((self.vga.clone(), self.dac));
    }

    /// The screen drawn at once (9466: the page shown whole, or drawn
    /// straight to the screen).
    pub fn whole(&mut self, page: &[u8]) {
        self.vga.copy_from_slice(page);
    }

    /// 1194:4FF7: FA00h times the generator's next value (DGROUP 4B8E,
    /// 1194:5065: x 5E5h + 29h), its remainder by the rectangle's size, a
    /// pixel there (row, column) copied from the page to the screen; a
    /// wait after the 201h-th and every 400h after (62); then the page
    /// shown whole (9466). With 5CA bit 2 (F4's) none of it: the page
    /// shown whole.
    pub fn dissolve(&mut self, m: &mut Machine, page: &[u8], rect: (usize, usize, usize, usize)) {
        if m.mem.r16(DS, 0x05CA) & 2 != 0 {
            return self.whole(page);
        }
        let (x0, y0, w, h) = rect;
        let size = (w * h) as u32;
        let mut r = m.mem.r16(DS, DISSOLVE_SEED);
        for n in 1..=DISSOLVE_STEPS {
            r = dissolve_next(r);
            if size != 0 {
                let off = (r as u32 % size) as usize;
                let i = (y0 + off / w) * W + x0 + off % w;
                if let (Some(o), Some(&t)) = (self.vga.get_mut(i), page.get(i)) {
                    *o = t;
                }
            }
            if n >= 0x201 && (n - 0x201) % 0x400 == 0 {
                self.wait();
            }
        }
        m.mem.w16(DS, DISSOLVE_SEED, r);
        self.whole(page);
    }

    /// 0000:9B56: the colours set, from entry 0 on, a wait before the
    /// first, the 55h-th and the ABh-th.
    pub fn colours(&mut self, to: &Dac) {
        for (lo, hi) in [(0, 0x55), (0x55, 0xAB), (0xAB, 0x100)] {
            self.wait();
            self.dac[lo..hi].copy_from_slice(&to[lo..hi]);
        }
    }

    /// 0000:5E50: the colours taken to `to` in `steps` (1 or less: set at
    /// once). The DAC read first (6E3C: a wait for the retrace after its
    /// first colour, its ABh-th from the end and its 56h-th); each
    /// component as 8.8 (the DAC's 6 bits), the step its difference over
    /// `steps` (idiv), added `steps` - 1 times (below 0: 0), each set
    /// (9B56); then `to` set.
    pub fn fade(&mut self, to: &Dac, steps: i16) {
        if steps > 1 {
            for _ in 0..3 {
                self.wait();
            }
            let mut acc = [0i16; 0x300];
            let mut step = [0i16; 0x300];
            for i in 0..0x300 {
                let (from, want) = ((self.dac[i / 3][i % 3] >> 2) as i16, (to[i / 3][i % 3] >> 2) as i16);
                acc[i] = from << 8;
                step[i] = ((want << 8) - (from << 8)) / steps;
            }
            for _ in 1..steps {
                let mut d = self.dac;
                for i in 0..0x300 {
                    acc[i] = acc[i].wrapping_add(step[i]);
                    if acc[i] < 0 {
                        acc[i] = 0;
                    }
                    d[i / 3][i % 3] = dac8((acc[i] as u16 >> 8) as u8);
                }
                self.colours(&d);
            }
        }
        self.colours(to);
    }
}
