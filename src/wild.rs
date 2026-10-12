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
    /// "wild001.rmd" (digits at +4..+6), the window positions' map
    /// offsets (0, 80h, 4000h, 4080h), and where blocks decompress to
    /// (offset; the segment is the buffer segment's).
    pub const BLOCK_NAME: u16 = 0x3889;
    pub const WINDOW_OFFSETS: u16 = 0x3963;
    pub const UNPACK_OFF: u16 = 0x902E;
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
        self.ems_map(GRID_PAGE);
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

    /// 217B:0189. Loads block `block` (wildNNN.rmd) into window position
    /// `slot` (0..3: the quarters of the 128 x 128 map): the file is
    /// loaded whole into the block at BUFFER_BLOCK, decompressed
    /// (`wild_unpack`) into the buffer segment, and its three 64 x 64
    /// planes copied to the floor (plus the buffer segment number),
    /// walls and ceiling. Blocks 1..4, the location's own, are opened
    /// through the game's directory, where a save's copies are put; the
    /// others come from GLOBAL.BSA. A file that fails to load leaves the
    /// block as it was, and is unpacked and copied anyway. Between the
    /// steps the heap is checked (2294:11F2); None where a check fails
    /// (the original stops), the unpacking runs away, or EMS is on.
    pub fn wild_load_block(&mut self, files: &mut crate::bsa::Files, slot: u8, block: u8) -> Option<()> {
        use crate::level::var::BUFFER_BLOCK;
        use crate::mif::var::{BUFFER_SEG, FLOOR_SEG, MAP1_SEG, MAP2_SEG};
        self.heap_check().then_some(())?;
        let dest = self.mem.r16(DS, WINDOW_OFFSETS.wrapping_add(slot as u16 * 2));
        let digits = [block / 100 % 10, block / 10 % 10, block % 10].map(|d| b'0' + d);
        for (i, &d) in digits.iter().enumerate() {
            self.mem.w8(DS, BLOCK_NAME + 4 + i as u16, d);
        }
        self.ems_map(self.mem.r16(DS, 0x1030));
        let (bseg, boff) = (self.mem.r16(DS, BUFFER_BLOCK + 2), self.mem.r16(DS, BUFFER_BLOCK));
        let mut name = BLOCK_NAME;
        if digits[0] == b'0' && digits[1] == b'0' && digits[2] < b'5' {
            name = self.save_path(BLOCK_NAME);
        }
        self.file_load(files, name, bseg, boff);
        for _ in 0..2 {
            self.heap_check().then_some(())?;
        }
        let buf = self.mem.r16(DS, BUFFER_SEG);
        let out = self.mem.r16(DS, UNPACK_OFF);
        self.wild_unpack(bseg, boff, buf, out)?;
        for _ in 0..2 {
            self.heap_check().then_some(())?;
        }
        let mut si = 0u16;
        for (var, add) in [(FLOOR_SEG, buf), (MAP1_SEG, 0), (MAP2_SEG, 0)] {
            let plane = self.mem.r16(DS, var);
            let mut di = dest;
            for _ in 0..64 {
                for _ in 0..64 {
                    let w = self.mem.r16(buf, si).wrapping_add(add);
                    self.mem.w16(plane, di, w);
                    si = si.wrapping_add(2);
                    di = di.wrapping_add(2);
                }
                di = di.wrapping_add(0x80);
            }
        }
        self.heap_check().then_some(())
    }

    /// 2294:0CBD. Word run-length decoding from src_seg:src to dst_seg:dst:
    /// a total in words, then runs of a count word, negative for that
    /// many copies of the next word, positive for that many literal
    /// words, until the total reaches exactly 0. Offsets wrap within the
    /// segments. None where the total never lands on 0 (the original
    /// reads on for ever; cut off after 0x20000 runs).
    pub fn wild_unpack(&mut self, src_seg: u16, src: u16, dst_seg: u16, dst: u16) -> Option<()> {
        let (mut si, mut di) = (src, dst);
        let read = |m: &Self, si: &mut u16| {
            let w = m.mem.r16(src_seg, *si);
            *si = si.wrapping_add(2);
            w
        };
        let mut total = read(self, &mut si);
        for _ in 0..0x20000 {
            let n = read(self, &mut si);
            if (n as i16) < 0 {
                let count = n.wrapping_neg();
                total = total.wrapping_sub(count);
                let w = read(self, &mut si);
                for _ in 0..count {
                    self.mem.w16(dst_seg, di, w);
                    di = di.wrapping_add(2);
                }
            } else {
                total = total.wrapping_sub(n);
                for _ in 0..n {
                    let w = read(self, &mut si);
                    self.mem.w16(dst_seg, di, w);
                    di = di.wrapping_add(2);
                }
            }
            if total == 0 {
                return Some(());
            }
        }
        None
    }
}
