//! City layout generation (segment 1194, 0292..04DC).
//!
//! A city is a square plan of `dim` x `dim` blocks at DGROUP 486B, one
//! byte per block: 0 empty, 1 reserved (kept from the caller), 2.. a
//! block type + 2. The RNG is seeded from the city's map coordinates,
//! six fixed blocks are placed, the rest are filled from a probability
//! table, and each block then gets a .MIF file chosen by random
//! rotation and variation. The original then loads each file and copies
//! its floor into the city map; that I/O is not ported here, and
//! `generate_city` returns the file names in load order instead.
//!
//! Block types (index into the code table): 0 equipment "eq", 1 mages'
//! guild "mg", 2 noble house "nb", 3 temple "tp", 4 tavern "tv",
//! 5 "ts" (spacer), 6 houses "bs".

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Location type; 8 skips generation (no random layout).
    pub const LOCATION_TYPE: u16 = 0x0A46;
    /// City grid dimension (blocks per side).
    pub const DIM: u16 = 0x0A43;
    /// Map offset of the first block, copied to `WRITE_POS`.
    pub const START_POS: u16 = 0x0A44;
    /// City coordinates; the seed is (X << 16) | Y.
    pub const X: u16 = 0x0A48;
    pub const Y: u16 = 0x0A4A;
    /// Working copies: dimension, dim*dim, current map write offset.
    pub const WORK_DIM: u16 = 0xA886;
    pub const WORK_CELLS: u16 = 0xA88C;
    pub const WRITE_POS: u16 = 0xA888;
    /// One byte per block, row-major.
    pub const PLAN: u16 = 0x486B;
    pub const RNG_SEED: u16 = 0x91F6;
}

/// DGROUP tables.
pub mod table {
    /// Variation count per block type (words).
    pub const VARIATIONS: u16 = 0x36FE;
    /// "xxbdxxx.mif": two-digit variations (10 and up).
    pub const NAME_LONG: u16 = 0x370C;
    /// "xxbdxx.mif": one-digit variations.
    pub const NAME_SHORT: u16 = 0x3718;
    /// Two-letter code per block type.
    pub const CODES: u16 = 0x3723;
    /// (threshold word, type byte) entries; the first threshold at or
    /// above the random value picks the type. Ends with 0xFFFF.
    pub const FILL: u16 = 0x3731;
}

use table::*;
use var::*;

/// Map write offset step per block across, and per row of blocks.
const BLOCK_STEP: u16 = 0x28;
const ROW_STEP: u16 = 0x1400;

impl Machine {
    fn city_next(&mut self) -> u16 {
        self.rng.next()
    }

    /// 1194:04BE. Puts block `ty` (stored as ty + 2) on a random empty
    /// cell. A cell is empty when its byte, read signed, is below 1.
    /// None where the original faults (no cells) or spins forever (no
    /// empty cell).
    pub fn city_place_block(&mut self, ty: u8) -> Option<()> {
        let cells = self.mem.r16(DS, WORK_CELLS);
        if cells == 0 {
            return None;
        }
        if !(0..cells).any(|i| (self.mem.r8(DS, PLAN.wrapping_add(i)) as i8) < 1) {
            return None;
        }
        loop {
            let i = self.city_next() % cells;
            if (self.mem.r8(DS, PLAN.wrapping_add(i)) as i8) < 1 {
                self.mem.w8(DS, PLAN.wrapping_add(i), ty.wrapping_add(2));
                return Some(());
            }
        }
    }

    /// 1194:0316..036A. Places the six fixed blocks (types 0-5), then
    /// one block for every cell that was exactly 0 at that point, its
    /// type from the fill table. With no such cell the original's
    /// `loop` runs 65536 times.
    pub fn city_plan(&mut self) -> Option<()> {
        for ty in 0..6 {
            self.city_place_block(ty)?;
        }
        let cells = self.mem.r16(DS, WORK_CELLS);
        let zeros = (0..cells).filter(|&i| self.mem.r8(DS, PLAN.wrapping_add(i)) == 0).count() as u16;
        let n = if zeros == 0 { 0x10000 } else { zeros as u32 };
        for _ in 0..n {
            let r = self.city_next();
            let mut e = FILL;
            while r > self.mem.r16(DS, e) {
                e = e.wrapping_add(3);
            }
            let ty = self.mem.r8(DS, e.wrapping_add(2));
            self.city_place_block(ty)?;
        }
        Some(())
    }

    /// 1194:03A2..03FA. Chooses the .MIF file for a plan byte: writes the
    /// code, rotation letter ('a'+(r&3)) and variation (r % count) into
    /// both name templates and returns the offset of the one to load:
    /// the long name for variations above 9. Variation 0 is spelled "1"
    /// in the short name. Some(None) for empty or reserved cells (no
    /// block); None where the original divides by zero.
    pub fn city_block_name(&mut self, cell: u8) -> Option<Option<u16>> {
        let Some(t) = (cell as u16).checked_sub(2) else { return Some(None) };
        let si = t * 2;
        let code = self.mem.r16(DS, CODES.wrapping_add(si));
        self.mem.w16(DS, NAME_LONG, code);
        self.mem.w16(DS, NAME_SHORT, code);
        let rot = (self.city_next() as u8 & 3).wrapping_add(b'a');
        self.mem.w8(DS, NAME_LONG + 6, rot);
        self.mem.w8(DS, NAME_SHORT + 5, rot);
        let r = self.city_next();
        let count = self.mem.r16(DS, VARIATIONS.wrapping_add(si));
        if count == 0 {
            return None;
        }
        let var = r % count;
        // AL = tens (low byte of the quotient), AH = units, then a 16-bit
        // add of '00': a tens byte above 0xCF carries into the units.
        let digits = (((var % 10) << 8) | (var / 10) & 0xff).wrapping_add(0x3030);
        self.mem.w16(DS, NAME_LONG + 4, digits);
        let mut units = (digits >> 8) as u8;
        if units == b'0' {
            units = b'1';
        }
        self.mem.w8(DS, NAME_SHORT + 4, units);
        Some(Some(if var > 9 { NAME_LONG } else { NAME_SHORT }))
    }

    /// 1194:0292 without its I/O: the screen/loader calls (0000:5C86,
    /// 0000:E87C, 1194:05A9) and the per-block file load after 03FA.
    /// Returns, in load order, each block's map write offset and file
    /// name; None where the original faults or hangs. The RNG seed is
    /// saved and restored around the whole routine, as in the original.
    pub fn generate_city(&mut self) -> Option<Vec<(u16, Vec<u8>)>> {
        let seg = self.mem.r16(DS, 0x9030);
        self.mem.w32(DS, 0xBA54, (seg as u32) << 16);
        let f = self.mem.r16(DS, 0xC398) | 0x8000;
        self.mem.w16(DS, 0xC398, f);
        let saved = self.rng.seed;
        let mut out = Vec::new();
        if self.mem.r8(DS, LOCATION_TYPE) != 8 {
            self.rng.seed = (self.mem.r16(DS, X) as u32) << 16 | self.mem.r16(DS, Y) as u32;
            let dim = self.mem.r8(DS, DIM) as u16;
            self.mem.w16(DS, WORK_DIM, dim);
            let cells = dim.wrapping_mul(dim);
            self.mem.w16(DS, WORK_CELLS, cells);
            // `loop` with CX=0 runs 65536 times.
            let n = if cells == 0 { 0x10000 } else { cells as u32 };
            for i in 0..n {
                let p = PLAN.wrapping_add(i as u16);
                if self.mem.r8(DS, p) != 1 {
                    self.mem.w8(DS, p, 0);
                }
            }
            let start = self.mem.r16(DS, START_POS);
            self.mem.w16(DS, WRITE_POS, start);
            self.city_plan()?;
            let rows = if dim == 0 { 0x10000 } else { dim as u32 };
            let mut cell = PLAN;
            for _ in 0..rows {
                for _ in 0..rows {
                    let b = self.mem.r8(DS, cell);
                    if let Some(name) = self.city_block_name(b)? {
                        let pos = self.mem.r16(DS, WRITE_POS);
                        out.push((pos, self.mem.cstr(DS, name).to_vec()));
                    }
                    let pos = self.mem.r16(DS, WRITE_POS).wrapping_add(BLOCK_STEP);
                    self.mem.w16(DS, WRITE_POS, pos);
                    cell = cell.wrapping_add(1);
                }
                let back = BLOCK_STEP.wrapping_mul(dim);
                let pos = self.mem.r16(DS, WRITE_POS).wrapping_sub(back).wrapping_add(ROW_STEP);
                self.mem.w16(DS, WRITE_POS, pos);
            }
        }
        self.rng.seed = saved;
        Some(out)
    }
}
