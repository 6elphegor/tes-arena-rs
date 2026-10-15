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

/// A block to load, as named by `city_block_name`.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// Map write offset.
    pub pos: u16,
    /// The name template it was loaded through (NAME_LONG or NAME_SHORT).
    pub name_off: u16,
    /// File name.
    pub name: Vec<u8>,
}

/// DGROUP variables.
pub mod var {
    /// The current location's template file name (record +14).
    pub const TEMPLATE: u16 = 0x0A36;
    /// Province of the current location (record +24). Province 8, the
    /// Imperial Province, has a fixed city and skips generation.
    pub const PROVINCE: u16 = 0x0A46;
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

/// City-info pass (1194:3EF4): record far pointer (segment, offset),
/// saved seed, dimension, cell count, and three counters it clears
/// (written in the order 138B, 1389, 1387).
const INFO_REC: u16 = 0x124E;
const INFO_SEED: u16 = 0x8B98;
const INFO_DIM: u16 = 0x113B;
const INFO_CELLS: u16 = 0x1141;
const INFO_COUNTS: [u16; 3] = [0x138B, 0x1389, 0x1387];

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
    /// empty cell, or none the RNG gives in 2^20 rolls).
    pub fn city_place_block(&mut self, ty: u8) -> Option<()> {
        self.city_place_in(WORK_CELLS, ty)
    }

    /// `city_place_block` with the cell count taken from DGROUP
    /// `cells_var` (1194:421C uses 1141).
    fn city_place_in(&mut self, cells_var: u16, ty: u8) -> Option<()> {
        let cells = self.mem.r16(DS, cells_var);
        if cells == 0 {
            return None;
        }
        if !(0..cells).any(|i| (self.mem.r8(DS, PLAN.wrapping_add(i)) as i8) < 1) {
            return None;
        }
        // An empty cell the RNG never lands on keeps the original rolling
        // forever; past 2^20 rolls (beyond the emulator's budget for the
        // vectors) the port gives up.
        for _ in 0..1u32 << 20 {
            let i = self.city_next() % cells;
            if (self.mem.r8(DS, PLAN.wrapping_add(i)) as i8) < 1 {
                self.mem.w8(DS, PLAN.wrapping_add(i), ty.wrapping_add(2));
                return Some(());
            }
        }
        None
    }

    /// 1194:0316..036A. Places the six fixed blocks (types 0-5), then
    /// one block for every cell that was exactly 0 at that point, its
    /// type from the fill table. With no such cell the original's
    /// `loop` runs 65536 times.
    pub fn city_plan(&mut self) -> Option<()> {
        self.city_plan_in(WORK_CELLS)
    }

    /// `city_plan` with the cell count in DGROUP `cells_var`.
    fn city_plan_in(&mut self, cells_var: u16) -> Option<()> {
        for ty in 0..6 {
            self.city_place_in(cells_var, ty)?;
        }
        let cells = self.mem.r16(DS, cells_var);
        let zeros = (0..cells).filter(|&i| self.mem.r8(DS, PLAN.wrapping_add(i)) == 0).count() as u16;
        let n = if zeros == 0 { 0x10000 } else { zeros as u32 };
        for _ in 0..n {
            let r = self.city_next();
            let mut e = FILL;
            while r > self.mem.r16(DS, e) {
                e = e.wrapping_add(3);
            }
            let ty = self.mem.r8(DS, e.wrapping_add(2));
            self.city_place_in(cells_var, ty)?;
        }
        Some(())
    }

    /// 1194:3EF4..3F96: the plan as built for a location record by the
    /// city-info pass (which then reads each block's .MIF header and sums
    /// three counts into 1387/138B/1389; that file I/O is not ported).
    /// Seeds the RNG from the record's (X << 16) | Y, also kept at 8B98,
    /// with the dimension from +21 squared in 8 bits, and the cell count
    /// at 113B/1141 instead of A886/A88C.
    pub fn city_plan_for_record(&mut self, seg: u16, rec: u16) -> Option<()> {
        self.mem.w16(DS, INFO_REC, seg);
        self.mem.w16(DS, INFO_REC + 2, rec);
        let seed = (self.mem.r16(seg, rec.wrapping_add(record::X)) as u32) << 16
            | self.mem.r16(seg, rec.wrapping_add(record::Y)) as u32;
        self.rng.seed = seed;
        self.mem.w32(DS, RNG_SEED, seed);
        self.mem.w32(DS, INFO_SEED, seed);
        let dim = self.mem.r8(seg, rec.wrapping_add(record::DIM));
        self.mem.w16(DS, INFO_DIM, dim as u16);
        let cells = dim as u16 * dim as u16;
        self.mem.w16(DS, INFO_CELLS, cells);
        for c in INFO_COUNTS {
            self.mem.w16(DS, c, 0);
        }
        // `loop` with CX=0 runs 65536 times.
        let n = if cells == 0 { 0x10000 } else { cells as u32 };
        for i in 0..n {
            let p = PLAN.wrapping_add(i as u16);
            if self.mem.r8(DS, p) != 1 {
                self.mem.w8(DS, p, 0);
            }
        }
        self.city_plan_in(INFO_CELLS)
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
    /// Returns the blocks in load order; None where the original faults
    /// or hangs. The RNG seed is saved and restored around the whole
    /// routine, as in the original.
    pub fn generate_city(&mut self) -> Option<Vec<Block>> {
        // Where the port gives up, the game's seed is put back as the
        // end puts it, not left at the one this rolled with. The
        // original has no such exit.
        let saved = self.rng.seed;
        let r = self.generate_city_seeded();
        self.seed_back(saved, r)
    }

    fn generate_city_seeded(&mut self) -> Option<Vec<Block>> {
        let seg = self.mem.r16(DS, 0x9030);
        self.mem.w32(DS, 0xBA54, (seg as u32) << 16);
        let f = self.mem.r16(DS, 0xC398) | 0x8000;
        self.mem.w16(DS, 0xC398, f);
        let saved = self.rng.seed;
        let mut out = Vec::new();
        if self.mem.r8(DS, PROVINCE) != 8 {
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
                        out.push(Block { pos, name_off: name, name: self.mem.cstr(DS, name).to_vec() });
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

/// Location records (0x38 bytes; the current one is DGROUP 0A22) and
/// the province data they come from (1194:3B84 and helpers).
///
/// The save's per-province `citydata` block is loaded at DGROUP 1429:
/// 20-byte name, bounds words at 143D (x offset), 143F (y offset), 1441
/// (width), 1443 (height), then 25-byte location entries from 1445 (a
/// 20-byte name, x and y words, a flags byte). Location ids are
/// province * 32 + local index; 0x100 means the Imperial City.
pub mod record {
    pub const NAME: u16 = 0x00;
    /// Template file name, e.g. "city3.mif".
    pub const TEMPLATE: u16 = 0x14;
    /// Blocks per side: 6 - KIND.
    pub const DIM: u16 = 0x21;
    /// Map offset of the first block.
    pub const START_POS: u16 = 0x22;
    pub const PROVINCE: u16 = 0x24;
    /// 0 city-state, 1 town, 2 village.
    pub const KIND: u16 = 0x25;
    pub const X: u16 = 0x26;
    pub const Y: u16 = 0x28;
    pub const ID: u16 = 0x2A;
    /// Location entry flags; the high nibble scales the world position.
    pub const FLAGS: u16 = 0x2B;
    /// Map row from `city_map_y`.
    pub const MAP_Y: u16 = 0x2C;
    /// Byte read from a map file by 1194:3D91 (climate).
    pub const CLIMATE: u16 = 0x2E;
    /// Quadrant of the province map plus province * 4.
    pub const QUADRANT: u16 = 0x2F;
    /// World position, scaled by the flags.
    pub const WORLD_X: u16 = 0x30;
    pub const WORLD_Y: u16 = 0x32;
    /// World y * province (16-bit) and (world x & 0xFF) * province.
    pub const W34: u16 = 0x34;
    pub const W36: u16 = 0x36;
}

/// "citydata.nn", a province's block in it and where it is loaded.
const CITYDATA_NAME: u16 = 0x135E;
const PROVINCE_BYTES: u16 = 0x4CC;
const PROVINCE_DATA: u16 = 0x1429;

/// Province data in DGROUP.
const PROV_BOUNDS: u16 = 0x143D;
const PROV_LOCATIONS: u16 = 0x1445;
const LOCATION_SIZE: u16 = 25;
/// Scales computed by `city_local_to_global`.
const SCALE_X: u16 = 0x3FFC;
const SCALE_Y: u16 = 0x3FFE;
/// Location ids that use the plain templates (58 bytes); all others use
/// the "w" variants.
const PLAIN_IDS: u16 = 0x47E2;
/// Reserved block lists: NUL-terminated byte strings of plan indices.
const RESERVED_LISTS: u16 = 0x48C8;
const IMPERIAL_MIF: u16 = 0x48BB;

/// Result of 1194:3D05: which family of city templates a location uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Template {
    /// DGROUP list of (x, y) start positions, one pair per variant (AX).
    pub starts: u16,
    /// DGROUP printf format of the file name (SI).
    pub format: u16,
    /// 0 when the id is in the plain-template list (DX).
    pub flag: u16,
    /// 0 city-state, 1 town, 2 village (BX).
    pub kind: u16,
    /// Number of variants (CX).
    pub count: u16,
}

impl Machine {
    /// 1194:3183 without the province load (1194:2E18) it starts with.
    /// Converts a location's province-map position to world coordinates
    /// from the province bounds; stores the scales at 3FFC/3FFE. None
    /// where a quotient overflows 16 bits (divide error).
    pub fn city_local_to_global(&mut self, x: u16, y: u16) -> Option<(u16, u16)> {
        let div = |n: u32, d: u32| u16::try_from(n / d).ok();
        let sx = div(self.mem.r16(DS, PROV_BOUNDS + 4) as u32 * 100, 320)?;
        self.mem.w16(DS, SCALE_X, sx);
        let sy = div(self.mem.r16(DS, PROV_BOUNDS + 6) as u32 * 100, 200)?;
        self.mem.w16(DS, SCALE_Y, sy);
        let gy = div(y as u32 * sy as u32, 100)?.wrapping_add(self.mem.r16(DS, PROV_BOUNDS + 2));
        let gx = div(x as u32 * sx as u32, 100)?.wrapping_add(self.mem.r16(DS, PROV_BOUNDS));
        Some((gx, gy))
    }

    /// 1194:3E7C. (x >= 160) + 2 * (y >= 100), compared signed, plus
    /// `province` * 4; the low byte.
    pub fn city_quadrant(&self, x: u16, y: u16, province: u8) -> u8 {
        let q = (x as i16 >= 0xA0) as u16 + 2 * ((y as i16) >= 0x64) as u16 + ((province as u16) << 2);
        q as u8
    }

    /// 1194:3E9E. World position, with y turned into a map row:
    /// (0x200 - (y - 100)) & 0x1FF. Returns (AX, BX).
    pub fn city_map_y(&mut self, x: u16, y: u16) -> Option<(u16, u16)> {
        let (gx, gy) = self.city_local_to_global(x, y)?;
        Some((gx, gy.wrapping_sub(100).wrapping_neg().wrapping_add(0x200) & 0x1FF))
    }

    /// 1194:3D05. Template family for location `id` (its low byte).
    pub fn city_template(&self, id: u8) -> Template {
        if self.mem.r8(DS, PROVINCE) == 8 {
            return Template { starts: 0, format: IMPERIAL_MIF, flag: 0, kind: 0, count: 1 };
        }
        let listed = (0..0x3A).any(|i| self.mem.r8(DS, PLAIN_IDS + i) == id);
        let flag = !listed as u16;
        let local = id & 0x1F;
        let kind = if local < 8 { 0 } else if local < 16 { 1 } else { 2 };
        let (count, format, starts) = match (kind, listed) {
            (0, false) => (5, 0x484F, 0x48AB),
            (0, true) => (3, 0x485A, 0x48B5),
            (1, false) => (5, 0x481C, 0x488F),
            (1, true) => (2, 0x4827, 0x4899),
            (_, false) => (5, 0x4833, 0x489D),
            (_, true) => (2, 0x4841, 0x48A7),
        };
        Template { starts, format, flag, kind, count }
    }

    /// 1194:3CD3. Clears the first 36 plan bytes, then adds 1 to every
    /// plan index in reserved list `n` (n strings into the lists).
    pub fn city_reserve_blocks(&mut self, n: u16) {
        for i in 0..0x24 {
            self.mem.w8(DS, PLAN + i, 0);
        }
        let mut p = self.skip_strings(DS, RESERVED_LISTS, n);
        loop {
            let b = self.mem.r8(DS, p);
            p = p.wrapping_add(1);
            if b == 0 {
                break;
            }
            let c = PLAN.wrapping_add(b as u16);
            self.mem.w8(DS, c, self.mem.r8(DS, c).wrapping_add(1));
        }
    }

    /// 1194:2E18 (far 2E14). Loads province `province`'s block of
    /// CITYDATA.nn (the current save slot's digits from DGROUP 1A6C) to
    /// DGROUP 1429: 0x4CC bytes at province * 0x4CC. Returns the block's
    /// first byte (its name's). None where the file is missing (the
    /// original goes on reading from whatever handle is current).
    pub fn city_load_province(&mut self, files: &mut crate::bsa::Files, province: u8) -> Option<u8> {
        self.mem.w8(DS, 0x191E, province);
        let slot = self.mem.r8(DS, 0x1A6C);
        let digits = (((slot % 10) as u16) << 8 | (slot / 10) as u16).wrapping_add(0x3030);
        self.mem.w16(DS, CITYDATA_NAME + 9, digits);
        let path = self.save_path(CITYDATA_NAME);
        self.file_open(files, path)?;
        if province != 0 {
            self.file_seek(files, 0, province as u32 * PROVINCE_BYTES as u32);
        }
        self.file_read(files, DS, PROVINCE_DATA, PROVINCE_BYTES);
        self.file_close(files);
        Some(self.mem.r8(DS, PROVINCE_DATA))
    }

    /// 1194:3B84. Fills the location record at seg:rec for location `id`
    /// (0x100 for the Imperial City) from the loaded province data. The
    /// province load (1194:2E18) is assumed done, and the climate byte
    /// that 1194:3D91 reads from a map file is passed in. Reserved blocks
    /// are set up only when DGROUP 0A46 (the current record's province)
    /// is not 8. None where the original faults.
    pub fn city_build_location(&mut self, seg: u16, rec: u16, id: u16, climate: u8) -> Option<()> {
        self.city_build_location_with(seg, rec, id, &mut |_| Some(climate))
    }

    /// `city_build_location` with the climate made by `climate` where the
    /// original makes it (1194:3D91, after the quadrant, before the map
    /// row), as `Machine::city_location` does from the terrain map.
    pub(crate) fn city_build_location_with(&mut self, seg: u16, rec: u16, id: u16, climate: &mut dyn FnMut(&mut Self) -> Option<u8>) -> Option<()> {
        use record::*;
        let w8 = |m: &mut Self, f: u16, v: u8| m.mem.w8(seg, rec.wrapping_add(f), v);
        let w16 = |m: &mut Self, f: u16, v: u16| m.mem.w16(seg, rec.wrapping_add(f), v);
        let r8 = |m: &Self, f: u16| m.mem.r8(seg, rec.wrapping_add(f));
        let r16 = |m: &Self, f: u16| m.mem.r16(seg, rec.wrapping_add(f));

        let al = if id == 0x100 {
            w8(self, ID, 0);
            w8(self, PROVINCE, 8);
            0
        } else {
            w8(self, ID, id as u8);
            w8(self, PROVINCE, (id as u8) >> 5);
            id as u8
        };
        let t = self.city_template(al);
        w8(self, KIND, t.kind as u8);
        let v = if r8(self, PROVINCE) != 8 { r8(self, ID) as u16 } else { 0x100 };
        let variant = v % t.count;
        let list = variant + if t.flag == 0 { 5 } else { 0 };
        if self.mem.r8(DS, var::PROVINCE) != 8 {
            self.city_reserve_blocks(list);
        }
        // sprintf(rec + TEMPLATE, format, variant + 1); formats hold one %d.
        let fmt = self.mem.cstr(DS, t.format).to_vec();
        let mut name = Vec::new();
        let mut i = 0;
        while i < fmt.len() {
            if fmt[i] == b'%' && fmt.get(i + 1) == Some(&b'd') {
                let digits = self.utoa(variant + 1);
                name.extend_from_slice(&digits);
                i += 2;
            } else {
                name.push(fmt[i]);
                i += 1;
            }
        }
        name.push(0);
        for (k, &c) in name.iter().enumerate() {
            w8(self, TEMPLATE + k as u16, c);
        }
        let p = t.starts.wrapping_add(variant * 2);
        let start = ((self.mem.r8(DS, p) as u16) << 1).wrapping_add((self.mem.r8(DS, p.wrapping_add(1)) as u16) << 8);
        w16(self, START_POS, start);

        let entry = PROV_LOCATIONS.wrapping_add((r8(self, ID) & 0x1F) as u16 * LOCATION_SIZE);
        w16(self, X, self.mem.r16(DS, entry.wrapping_add(0x14)));
        w16(self, Y, self.mem.r16(DS, entry.wrapping_add(0x16)));
        w8(self, FLAGS, self.mem.r8(DS, entry.wrapping_add(0x18)));
        // strcpy(rec, entry name)
        let mut k = 0u16;
        loop {
            let c = self.mem.r8(DS, entry.wrapping_add(k));
            self.mem.w8(seg, rec.wrapping_add(k), c);
            k = k.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        w8(self, DIM, 6u8.wrapping_sub(r8(self, KIND)));
        let (x, y, prov) = (r16(self, X), r16(self, Y), r8(self, PROVINCE));
        let q = self.city_quadrant(x, y, prov);
        w8(self, QUADRANT, q);
        let climate = climate(self)?;
        w8(self, CLIMATE, climate);
        let (_, row) = self.city_map_y(x, y)?;
        w16(self, MAP_Y, row);
        let (gx, gy) = self.city_local_to_global(x, y)?;
        // SHL by 2 * (flags >> 4); 16-bit shifts of 16 or more give 0.
        let sh = (r8(self, FLAGS) >> 4) as u32 * 2;
        let shl = |v: u16| if sh >= 16 { 0 } else { v << sh };
        w16(self, WORLD_X, shl(gx));
        w16(self, WORLD_Y, shl(gy));
        let prov = r8(self, PROVINCE);
        w16(self, W34, gy.wrapping_mul(prov as u16));
        w16(self, W36, (gx as u8 as u16) * prov as u16);
        Some(())
    }
}
