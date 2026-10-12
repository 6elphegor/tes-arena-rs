//! The wilderness around a location (segment 217B): a 64 x 64 grid of
//! block numbers at offset F000 of the buffer segment, each naming a
//! block file (wildNNN.rmd), with the location itself as blocks 1..4 at
//! the centre.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables and tables.
pub mod var {
    /// The current location record; its first four bytes (the start of
    /// its name) seed the grid.
    pub const LOCATION: u16 = 0x0A22;
    /// The location's template file name ("cityw1.mif").
    pub const TEMPLATE: u16 = 0x0A36;
    /// The seed used (dword), and the window of blocks loaded (FFFF: none).
    pub const SEED: u16 = 0x0616;
    pub const WINDOW: u16 = 0x0604;
    /// Block lists: (weight, list) word pairs; a list is a count byte
    /// and that many block numbers.
    pub const LISTS: u16 = 0x3900;
    /// Walled-template quadrant masks by template kind, and the
    /// quadrants' offsets.
    pub const MASKS: u16 = 0x3882;
    pub const QUADRANTS: u16 = 0x38F8;
}
use var::*;

/// Where the grid starts in the buffer segment, and its size.
pub const GRID: u16 = 0xF000;
pub const GRID_SIZE: u16 = 64;
/// The block that marks a walled city's ground.
const CITY_GROUND: u8 = 0x38;
/// EMS page holding the grid.
const GRID_PAGE: u16 = 0x11;

impl Machine {
    /// 217B:0004. Generates the wilderness grid for the current location,
    /// seeded from its record's first four bytes (the RNG is restored
    /// afterwards): every cell a block from the weighted lists, then the
    /// walled template's quadrants (see `wild_footprint`), then blocks 1
    /// to 4 at the centre. None where the original divides by zero or EMS
    /// is on.
    pub fn wild_generate(&mut self) -> Option<()> {
        self.mem.w16(DS, WINDOW, 0xFFFF);
        let saved = self.rng.seed;
        let seed = self.mem.r32(DS, LOCATION);
        self.rng.seed = seed;
        self.mem.w32(DS, SEED, seed);
        self.ems_map(GRID_PAGE)?;
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        for i in 0..GRID_SIZE * GRID_SIZE {
            let b = self.wild_pick()?;
            self.mem.w8(buf, GRID + i, b);
        }
        self.wild_footprint(buf);
        for (off, b) in [(0xF7DF, 1), (0xF7E0, 2), (0xF81F, 3), (0xF820, 4)] {
            self.mem.w8(buf, off, b);
        }
        self.rng.seed = saved;
        Some(())
    }

    /// 217B:0506: a block number. A random word picks a list by the
    /// running sum of weights (they pass 65535 by the fifth list), a
    /// random byte modulo the list's count picks the block. None where
    /// a count is 0.
    pub fn wild_pick(&mut self) -> Option<u8> {
        let mut r = self.rng.next();
        let mut si = LISTS;
        loop {
            let w = self.mem.r16(DS, si);
            if r < w {
                break;
            }
            r -= w;
            si = si.wrapping_add(4);
        }
        let list = self.mem.r16(DS, si.wrapping_add(2));
        let n = self.mem.r8(DS, list);
        let k = self.rng.next() as u8;
        if n == 0 {
            return None;
        }
        Some(self.mem.r8(DS, list.wrapping_add(1 + (k % n) as u16)))
    }

    /// 217B:052C. For a walled template (a 'w' after "city", "town" or
    /// "village"; the letter checked is at +4 for a name starting 'c' or
    /// 't', at +7 otherwise), the digit after the 'w' picks a mask whose
    /// low four bits mark quadrants to fill with CITY_GROUND, 32 x 32
    /// bytes each. The quadrant offsets are taken from offset 0 of the
    /// buffer segment, not the grid at F000, so the grid is never
    /// touched.
    fn wild_footprint(&mut self, buf: u16) {
        let (masks, at) = match self.mem.r8(DS, TEMPLATE) {
            b'c' => (MASKS, 4),
            b't' => (MASKS + 3, 4),
            _ => (MASKS + 5, 7),
        };
        if self.mem.r8(DS, TEMPLATE + at) != b'w' {
            return;
        }
        let digit = (self.mem.r8(DS, TEMPLATE + at + 1) as u16).wrapping_sub(b'1' as u16) & 0xFF;
        let mask = self.mem.r8(DS, masks.wrapping_add(digit));
        for q in 0..4 {
            if mask >> q & 1 == 0 {
                continue;
            }
            let mut di = self.mem.r16(DS, QUADRANTS + q * 2);
            for _ in 0..32 {
                for x in 0..32u16 {
                    self.mem.w8(buf, di.wrapping_add(x), CITY_GROUND);
                }
                di = di.wrapping_add(0x40);
            }
        }
    }
}
