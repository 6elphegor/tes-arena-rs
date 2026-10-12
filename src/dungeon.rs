//! Dungeon generation: dungeon location records, the dungeon layout from
//! its seed, and main-quest dungeon selection.
//!
//! A dungeon is a grid of 32x32 chunks (2x1 or 2x2) on several levels.
//! Each level gets one block group of `random1.mif` (8 levels of that
//! file per group), and every chunk of the grid copies a random level of
//! the group. Stairs connect consecutive levels at random chunks: the
//! up stairs of a level sit where the previous level's down stairs were.
//! Reading `random1.mif` and copying its floors into the map is file I/O
//! and is not ported; `dungeon_level` returns which level of the file
//! each chunk loads instead, like `generate_city` does for city blocks.
//!
//! Interiors use the 64 KiB map segment whose segment is in DGROUP D806:
//! a 128x128 grid of words, cleared to 0x0101. Stairs are marked with
//! two bytes 0x1D (up) or 0x1C (down) at an offset from table 6564.

use crate::city::record;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The current location record (0x38 bytes), as in `city::record`.
    pub const LOCATION: u16 = 0x0A22;
    /// The dungeon seed, record +34 of the current location.
    pub const LOCATION_SEED: u16 = 0x0A56;
    /// Destination name, position and province from the travel screen.
    pub const DEST_NAME: u16 = 0x1929;
    pub const DEST_X: u16 = 0x193E;
    pub const DEST_Y: u16 = 0x1940;
    pub const DEST_PROVINCE: u16 = 0x47E1;
    /// Game flags: 0x80 adds the custom seed parts, 0x4000 makes the
    /// grid 2x2.
    pub const GAME_FLAGS: u16 = 0x05C8;
    /// Added to the seed with flag 0x80: (B << 6) + A, in 16 bits.
    pub const SEED_A: u16 = 0x0608;
    pub const SEED_B: u16 = 0x060A;
    pub const FLAGS_C398: u16 = 0xC398;
    pub const FLAGS_C39A: u16 = 0xC39A;
    /// Seed of the dungeon being built (the RNG seed at setup).
    pub const DUNGEON_SEED: u16 = 0x05FC;
    /// Stair cells: the start cell, then for each level its down cell,
    /// which is also the next level's up cell.
    pub const STAIRS: u16 = 0x05E8;
    /// Map size in tiles (chunks * 32) and the level count.
    pub const WIDTH: u16 = 0xC357;
    pub const HEIGHT: u16 = 0xC359;
    pub const LEVELS: u16 = 0xC355;
    /// The current level.
    pub const LEVEL: u16 = 0xBB0A;
    /// Segment of the map (word).
    pub const MAP_SEG: u16 = 0xD806;
    /// File handle of random1.mif while a level is built.
    pub const HANDLE: u16 = 0xA848;
    /// random1.mif level of the first block in this level's group.
    pub const GROUP: u16 = 0x6596;
    /// Player start position.
    pub const START_X: u16 = 0xC3D0;
    pub const START_Y: u16 = 0xC3D4;
    /// Main quest: quest step, dungeon (local id, province) slots, flags
    /// and target.
    pub const QUEST_STEP: u16 = 0x0FDD;
    pub const QUEST_LOCAL: u16 = 0x0FE2;
    pub const QUEST_PROVINCE: u16 = 0x0FE3;
    pub const QUEST2_LOCAL: u16 = 0x0FE4;
    pub const QUEST2_PROVINCE: u16 = 0x0FE5;
    pub const QUEST_FLAGS: u16 = 0x0FE6;
    pub const QUEST_TARGET: u16 = 0x0FEE;
    /// Results of `dungeon_pick_entry`.
    pub const PICK_NAME: u16 = 0x12A0;
    pub const PICK_MODE: u16 = 0x1298;
    pub const PICK_INDEX: u16 = 0x1296;
    pub const PICK_PROVINCE: u16 = 0x129E;
    pub const PICK_X: u16 = 0x129A;
    pub const PICK_Y: u16 = 0x129C;
    /// Current save slot, and where its two ASCII digits go in the save
    /// file name.
    pub const SAVE_SLOT: u16 = 0x1A6C;
    pub const SAVE_DIGITS: u16 = 0x1367;
}

/// DGROUP tables.
pub mod table {
    /// Map offset of each stair cell; indexed by the cell byte, which is
    /// row * 8 + column * 2 (a word table for a 4x4 grid of chunks).
    pub const STAIR_OFFSETS: u16 = 0x6564;
    /// ".mif".
    pub const MIF_EXT: u16 = 0x4866;
    /// Quest dungeon province per quest step.
    pub const QUEST_PROVINCES: u16 = 0x7883;
    /// Province dungeon entries: 25 bytes each (20-byte name, x, y,
    /// flags), entries 0 and 1 fixed, then 14 random ones. Part of the
    /// province data loaded at 1429.
    pub const DUNGEON_ENTRIES: u16 = 0x1765;
}

use table::*;
use var::*;

/// Buffer that 0000:75B0 writes decimal digits into, backwards from here.
const ULTOA_END: u16 = 0x94F6;
const ENTRY_SIZE: u16 = 25;
/// Entry flag: the dungeon has been handed out.
const ENTRY_USED: u8 = 2;
const STAIRS_UP: u8 = 0x1D;
const STAIRS_DOWN: u8 = 0x1C;

/// A chunk of a dungeon level: its map offset (row * 0x2000 + column *
/// 0x40, the original's BP) and the level of random1.mif it loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub offset: u16,
    pub block: u16,
}

/// Result of `dungeon_pick_entry`, the original's (AX, BX, CX).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub index: u16,
    pub x: u16,
    pub y: u16,
}

impl Machine {
    /// Brent's cycle check over the RNG seed, for reroll loops that only
    /// end when the RNG yields a different value. Returns true once the
    /// seed has come back to a saved one, i.e. the loop would repeat the
    /// same values forever. Without the RNG table the seed is the whole
    /// RNG state.
    fn dun_cycled(&self, saved: &mut u32, power: &mut u32, lam: &mut u32) -> bool {
        if self.rng.table.is_some() {
            return false;
        }
        *lam += 1;
        if self.rng.seed == *saved {
            return true;
        }
        if *lam == *power {
            *saved = self.rng.seed;
            *power *= 2;
            *lam = 0;
        }
        false
    }

    /// 0000:75B0. Writes `v` in decimal at DS:di, NUL-terminated; the
    /// digits are first written backwards into DGROUP ending at 94F6.
    fn dun_ultoa(&mut self, v: u32, di: u16) {
        let mut p = ULTOA_END;
        let mut v = v;
        loop {
            self.mem.w8(DS, p, b'0' + (v % 10) as u8);
            p = p.wrapping_sub(1);
            v /= 10;
            if v == 0 {
                break;
            }
        }
        let n = ULTOA_END.wrapping_sub(p);
        for i in 0..n {
            let c = self.mem.r8(DS, p.wrapping_add(1).wrapping_add(i));
            self.mem.w8(DS, di.wrapping_add(i), c);
        }
        self.mem.w8(DS, di.wrapping_add(n), 0);
    }

    /// 0000:9CA4: strcpy within DGROUP.
    fn dun_strcpy(&mut self, mut di: u16, mut si: u16) {
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
    }

    /// 1194:3B0C. Fills the current location record (0A22) for a dungeon
    /// with local id `id`, from the travel destination at 1929 (name),
    /// 193E/1940 (x, y) and 47E1 (province). The climate byte that
    /// 1194:3D91 reads from a map file is passed in.
    ///
    /// The seed (record +34) is !rol(Y << 16 | X with the province added
    /// to the low byte, no carry, 5). The record's file name (+14) is the
    /// seed in decimal, cut to 8 digits, plus ".mif". None where the
    /// original faults (the map row's division).
    pub fn dungeon_build_location(&mut self, id: u8, climate: u8) -> Option<()> {
        use record::*;
        let rec = LOCATION;
        for i in 0..0x38 {
            self.mem.w8(DS, rec + i, 0);
        }
        self.mem.w8(DS, rec + ID, id);
        self.mem.w8(DS, rec + KIND, 3);
        let x = self.mem.r16(DS, DEST_X);
        let y = self.mem.r16(DS, DEST_Y);
        self.mem.w16(DS, rec + X, x);
        self.mem.w16(DS, rec + Y, y);
        let province = self.mem.r8(DS, DEST_PROVINCE);
        self.mem.w8(DS, rec + PROVINCE, province);
        let (_, row) = self.city_map_y(x, y)?;
        self.mem.w16(DS, rec + MAP_Y, row);
        self.mem.w8(DS, rec + CLIMATE, climate);
        self.dun_strcpy(rec, DEST_NAME);
        let xy = self.mem.r32(DS, rec + X);
        let low = (xy as u8).wrapping_add(self.mem.r8(DS, rec + PROVINCE));
        let seed = !((xy & !0xFF) | low as u32).rotate_left(5);
        self.mem.w32(DS, rec + W34, seed);
        let name = rec + TEMPLATE;
        self.dun_ultoa(seed, name);
        self.mem.w8(DS, name + 8, 0);
        // strcat(name, ".mif")
        let end = (0..=0xFFFFu16).map(|i| name.wrapping_add(i)).find(|&p| self.mem.r8(DS, p) == 0)?;
        self.dun_strcpy(end, MIF_EXT);
        Some(())
    }

    /// 2294:0ED4. Number of levels: 1 or 2, or 4 for the current location
    /// (record province and local id) when it is one of the two main-quest
    /// dungeon slots. The roll is made either way.
    pub fn dungeon_level_count(&mut self) -> u16 {
        let n = self.rng.range(1, 3).expect("nonzero span");
        let province = self.mem.r8(DS, LOCATION + record::PROVINCE);
        let local = self.mem.r8(DS, LOCATION + record::ID) & 0x1F;
        let slots = [(QUEST_PROVINCE, QUEST_LOCAL), (QUEST2_PROVINCE, QUEST2_LOCAL)];
        if slots.iter().any(|&(p, l)| province == self.mem.r8(DS, p) && local == self.mem.r8(DS, l)) {
            4
        } else {
            n
        }
    }

    /// 0000:2939. A random stair cell of a `w` x `h` chunk grid:
    /// ((r % h) << 3) + 2 * (r % w), all in 8 bits. None where a size is
    /// 0 (divide error).
    fn dun_cell(&mut self, w: u16, h: u16) -> Option<u8> {
        let r = self.rng.next();
        let row = (r.checked_rem(h)? as u8) << 3;
        let r = self.rng.next();
        let col = r.checked_rem(w)? as u8;
        Some(row.wrapping_add(col.wrapping_add(col)))
    }

    /// 0000:28BC..2938, without the load of blank.mif (0000:E880) at the
    /// end. Sets up a dungeon of `ax` + 1 by `bx` + 1 chunks (1x1 becomes
    /// 2x1) and `levels` levels from the current RNG seed, which becomes
    /// the dungeon seed, and rolls the stair cells: a start cell, then a
    /// cell for each level that differs from the one before it.
    ///
    /// The sizes are stored as tiles (<< 5) and shifted back for the
    /// rolls, so their top 5 bits are lost. Each new cell is stored twice
    /// (the level's down stairs and the next level's up stairs) except
    /// the last. None where the original faults (a size of 0) or loops
    /// forever (the RNG never yields a different cell), and for `levels`
    /// 0 or above 0x4000, where the stair list runs on through DGROUP
    /// into the RNG's own state (not modelled).
    pub fn dungeon_setup(&mut self, ax: u16, bx: u16, levels: u16) -> Option<()> {
        if levels == 0 || levels > 0x4000 {
            return None;
        }
        let mut w = ax.wrapping_add(1);
        let h = bx.wrapping_add(1);
        if w.wrapping_add(h) == 2 {
            w = w.wrapping_add(1);
        }
        let f = self.mem.r16(DS, FLAGS_C398) & 0x7FFB;
        self.mem.w16(DS, FLAGS_C398, f);
        let f = self.mem.r16(DS, FLAGS_C39A) | 0x0800;
        self.mem.w16(DS, FLAGS_C39A, f);
        self.mem.w32(DS, DUNGEON_SEED, self.rng.seed);
        self.mem.w16(DS, WIDTH, w << 5);
        self.mem.w16(DS, HEIGHT, h << 5);
        self.mem.w16(DS, LEVELS, levels);
        self.mem.w8(DS, LEVEL, 0);
        let (w, h) = ((w << 5) >> 5, (h << 5) >> 5);

        let mut di = STAIRS;
        let mut cell = self.dun_cell(w, h)?;
        self.mem.w8(DS, di, cell);
        di += 1;
        for left in (1..=levels).rev() {
            let prev = self.mem.r8(DS, di - 1);
            let (mut saved, mut power, mut lam) = (self.rng.seed, 1, 0);
            loop {
                cell = self.dun_cell(w, h)?;
                if cell != prev {
                    break;
                }
                if self.dun_cycled(&mut saved, &mut power, &mut lam) {
                    return None;
                }
            }
            self.mem.w8(DS, di, cell);
            di += 1;
            if left == 1 {
                break;
            }
            self.mem.w8(DS, di, cell);
            di += 1;
        }
        Some(())
    }

    /// 0000:0DE6..0E2C: the dungeon part of entering a location, without
    /// the load of blank.mif. Seeds the RNG with the location's dungeon
    /// seed (plus (B << 6) + A in 16 bits when game flag 0x80 is set,
    /// which also sets flag 0x4000), rolls the level count and sets up a
    /// 2x1 dungeon, or 2x2 with flag 0x4000.
    pub fn dungeon_enter(&mut self) -> Option<()> {
        let mut seed = self.mem.r32(DS, LOCATION_SEED);
        if self.mem.r16(DS, GAME_FLAGS) & 0x80 != 0 {
            let extra = (self.mem.r16(DS, SEED_B) << 6).wrapping_add(self.mem.r16(DS, SEED_A));
            seed = (extra as u32).wrapping_add(seed);
            let f = self.mem.r16(DS, GAME_FLAGS) | 0x4000;
            self.mem.w16(DS, GAME_FLAGS, f);
        }
        self.rng.seed = seed;
        let f = self.mem.r16(DS, FLAGS_C39A) & 0xFDFF;
        self.mem.w16(DS, FLAGS_C39A, f);
        let levels = self.dungeon_level_count();
        let bx = (self.mem.r16(DS, GAME_FLAGS) & 0x4000 != 0) as u16;
        self.dungeon_setup(1, bx, levels)
    }

    /// 0000:299F without its file I/O. Builds level `level` of the current
    /// dungeon: clears the map to 0x0101, picks a group of random1.mif
    /// with the RNG seeded from the dungeon seed + level, picks a level of
    /// the group for each chunk (rows, then columns), then marks the
    /// stairs. The RNG seed is restored afterwards. `handle` is the file
    /// handle the original stores at A848.
    ///
    /// Returns the chunks in load order. None where a grid dimension is
    /// 0: the original's LOOP then runs 65536 times (not modelled).
    pub fn dungeon_level(&mut self, level: u8, handle: u16) -> Option<Vec<Chunk>> {
        self.mem.w8(DS, LEVEL, level);
        self.mem.w16(DS, HANDLE, handle);
        let rows = self.mem.r16(DS, HEIGHT) >> 5;
        if rows == 0 || self.mem.r16(DS, WIDTH) >> 5 == 0 {
            return None;
        }
        let map = self.mem.r16(DS, MAP_SEG);
        for i in 0..0x4000u16 {
            self.mem.w16(map, i * 2, 0x0101);
        }
        let saved = self.rng.seed;
        self.rng.seed = self.mem.r32(DS, DUNGEON_SEED).wrapping_add(level as u32);
        let group = (self.rng.next() & 3) << 3;
        self.mem.w16(DS, GROUP, group);
        let mut chunks = Vec::new();
        let mut row_off = 0u16;
        for _ in 0..rows {
            // The column count is read again for every row.
            let cols = self.mem.r16(DS, WIDTH) >> 5;
            if cols == 0 {
                return None;
            }
            let mut off = row_off;
            for _ in 0..cols {
                let block = (self.rng.next() & 7).wrapping_add(self.mem.r16(DS, GROUP));
                chunks.push(Chunk { offset: off, block });
                off = off.wrapping_add(0x40);
            }
            row_off = row_off.wrapping_add(0x2000);
        }
        self.rng.seed = saved;

        let si = STAIRS.wrapping_add((level as u16) * 2);
        let mark = |m: &mut Self, cell: u8, v: u8| {
            let off = m.mem.r16(DS, STAIR_OFFSETS.wrapping_add(cell as u16));
            m.mem.w8(map, off, v);
            m.mem.w8(map, off.wrapping_add(1), v);
        };
        let up = self.mem.r8(DS, si);
        mark(self, up, STAIRS_UP);
        let last = self.mem.r16(DS, LEVELS).wrapping_sub(1) as u8;
        if last != level {
            let down = self.mem.r8(DS, si.wrapping_add(1));
            mark(self, down, STAIRS_DOWN);
        }
        Some(chunks)
    }

    /// 0000:2978..299E. The player's start position on the first level,
    /// from the start cell's map offset: x = ((low byte >> 1) << 7) + 0x40,
    /// y = (high byte << 7) + 0xC0.
    pub fn dungeon_start_position(&mut self) -> (u16, u16) {
        let cell = self.mem.r8(DS, STAIRS);
        let off = self.mem.r16(DS, STAIR_OFFSETS.wrapping_add(cell as u16));
        let x = (((off as u8) >> 1) as u16).wrapping_shl(7).wrapping_add(0x40);
        let y = (off >> 8).wrapping_shl(7).wrapping_add(0xC0);
        self.mem.w16(DS, START_X, x);
        self.mem.w16(DS, START_Y, y);
        (x, y)
    }

    /// 1194:CE1B. Main quest: the quest dungeon's province from the quest
    /// step (table 7883), and a second province rolled from 0..7 until it
    /// differs. The second slot's local id is cleared (0xFF). None where
    /// the original loops forever (the RNG never yields a different one).
    pub fn dungeon_pick_quest_provinces(&mut self) -> Option<()> {
        let step = self.mem.r8(DS, QUEST_STEP);
        let p = self.mem.r8(DS, QUEST_PROVINCES.wrapping_add(step as u16));
        self.mem.w8(DS, QUEST_PROVINCE, p);
        let (mut saved, mut power, mut lam) = (self.rng.seed, 1, 0);
        let other = loop {
            let r = self.rng.range(0, 8).expect("nonzero span") as u8;
            if r != self.mem.r8(DS, QUEST_PROVINCE) {
                break r;
            }
            if self.dun_cycled(&mut saved, &mut power, &mut lam) {
                return None;
            }
        };
        self.mem.w8(DS, QUEST2_PROVINCE, other);
        self.mem.w8(DS, QUEST2_LOCAL, 0xFF);
        Some(())
    }

    /// 1194:CE40. Main quest: sets quest flag 2, the target 30 * step +
    /// 500 (the product in 16 bits), and the quest dungeon's province.
    pub fn dungeon_set_quest_target(&mut self) {
        let f = self.mem.r8(DS, QUEST_FLAGS) | 2;
        self.mem.w8(DS, QUEST_FLAGS, f);
        let step = self.mem.r8(DS, QUEST_STEP);
        self.mem.w16(DS, QUEST_TARGET, (step as u16 * 0x1E).wrapping_add(0x1F4));
        let p = self.mem.r8(DS, QUEST_PROVINCES.wrapping_add(step as u16));
        self.mem.w8(DS, QUEST_PROVINCE, p);
    }

    /// 26F7:0D32. A random one of the 14 random dungeon entries: returns
    /// its offset; the index stored at 1296 counts the two fixed ones.
    fn dun_random_entry(&mut self) -> u16 {
        let r = self.rng.range(0, 14).expect("nonzero span");
        self.mem.w16(DS, PICK_INDEX, r.wrapping_add(2));
        DUNGEON_ENTRIES + 0x32 + r * ENTRY_SIZE
    }

    /// 26F7:0C0A without its I/O: the province load (1194:2E14) and the
    /// write of the changed entry back to the save are skipped, so the
    /// province's data must already be at 1429.
    ///
    /// Picks a dungeon entry of the province: `mode` 4 takes entry
    /// `index`, 2 the first fixed one, 3 or more the second; 0 and 1 a
    /// random unused one of the 14 random entries, or any of them when
    /// all are used. Mode 1 swaps names with DS:`name` (the entry takes
    /// the name if it has none, else gives it); other modes mark the
    /// entry used. None where the original loops forever.
    pub fn dungeon_pick_entry(&mut self, province: u16, mode: u16, index: u16, name: u16) -> Option<Pick> {
        self.mem.w16(DS, PICK_NAME, name);
        self.mem.w16(DS, PICK_MODE, mode);
        self.mem.w16(DS, PICK_INDEX, index);
        self.mem.w16(DS, PICK_PROVINCE, province);
        let mut si = DUNGEON_ENTRIES;
        let used = |m: &Self, si: u16| m.mem.r8(DS, si.wrapping_add(0x18)) & ENTRY_USED != 0;
        if mode == 4 {
            si = si.wrapping_add(index.wrapping_mul(ENTRY_SIZE));
        } else if mode > 2 {
            si += ENTRY_SIZE;
        } else if mode < 2 {
            let first = DUNGEON_ENTRIES + 0x32;
            if (0..14).all(|i| used(self, first + i * ENTRY_SIZE)) {
                si = self.dun_random_entry();
            } else {
                let (mut saved, mut power, mut lam) = (self.rng.seed, 1, 0);
                loop {
                    si = self.dun_random_entry();
                    if !used(self, si) {
                        break;
                    }
                    if self.dun_cycled(&mut saved, &mut power, &mut lam) {
                        return None;
                    }
                }
            }
        }
        let x = self.mem.r16(DS, si.wrapping_add(0x14));
        let y = self.mem.r16(DS, si.wrapping_add(0x16));
        self.mem.w16(DS, PICK_X, x);
        self.mem.w16(DS, PICK_Y, y);
        if mode != 1 {
            let f = self.mem.r8(DS, si.wrapping_add(0x18)) | ENTRY_USED;
            self.mem.w8(DS, si.wrapping_add(0x18), f);
        } else if self.mem.r8(DS, si) == 0 {
            self.mem.w8(DS, name.wrapping_add(0x13), 0);
            self.dun_strcpy(si, name);
        } else {
            self.dun_strcpy(name, si);
        }
        // The save file's name gets the save slot's two digits.
        let slot = self.mem.r8(DS, SAVE_SLOT);
        self.mem.w8(DS, SAVE_DIGITS, b'0' + slot / 10);
        self.mem.w8(DS, SAVE_DIGITS + 1, b'0' + slot % 10);
        Some(Pick { index: self.mem.r16(DS, PICK_INDEX), x, y })
    }
}
