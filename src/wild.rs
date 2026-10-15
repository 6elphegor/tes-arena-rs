//! The wilderness around a location (segment 217B): a 64 x 64 grid of
//! block numbers at offset F000 of the buffer segment, each naming a
//! block file (wildNNN.rmd), with the location itself as blocks 1..4 at
//! the centre.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{HEADING, LEVEL_FLAGS, X, Y};

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
    /// The window's origin in blocks (x, y), and the player's position
    /// when it was last checked (x, y): a quarter changed is a block
    /// crossed.
    pub const ORIGIN_X: u16 = 0x0608;
    pub const ORIGIN_Y: u16 = 0x060A;
    pub const LAST_X: u16 = 0x0600;
    pub const LAST_Y: u16 = 0x0602;
    /// Where the wilderness was left (217B:04DC): the origin (x, y), the
    /// player's x, y and heading.
    pub const SAVED_ORIGIN: u16 = 0x060C;
    pub const SAVED_POS: u16 = 0x0610;
    /// The shift the actors got this frame (x, y).
    pub const SHIFT_X: u16 = 0xA836;
    pub const SHIFT_Y: u16 = 0xA838;
    /// The saved INF's name (from C2B5), and C39A's bit for coming back
    /// to the wilderness (out of a dungeon there).
    pub const SAVED_INF: u16 = 0x3895;
    pub const BACK: u16 = 0x2000;
    /// Set while the wilderness doesn't move its chunks.
    pub const FROZEN: u16 = 0xACC2;
}
use var::*;

/// Where the grid starts in the buffer segment, and its size.
pub const GRID: u16 = 0xF000;
pub const GRID_SIZE: u16 = 64;
/// The block that marks a walled city's ground.
const CITY_GROUND: u8 = 0x38;
/// EMS page holding the grid.
const GRID_PAGE: u16 = 0x11;

/// What the wilderness leaves to the front end and the files.
pub trait WildHooks {
    /// 217B:0189: block `block` loaded into window position `slot`
    /// (`Machine::wild_load_block`).
    fn load_block(&mut self, m: &mut Machine, files: &mut Files, slot: u8, block: u8) -> Option<()> {
        m.wild_load_block(files, slot, block)
    }
    /// 0000:9DD2: `n` bytes at `seg`:`off` written to a new file named at
    /// DS:`name` (`Machine::file_write`).
    fn write(&mut self, m: &mut Machine, files: &mut Files, name: u16, seg: u16, off: u16, n: u16) {
        m.file_write(files, name, seg, off, n);
    }
    /// 0000:0F05: the weather set (`Machine::weather_set`; its light
    /// tables through `LightHooks`, none here).
    fn weather(&mut self, m: &mut Machine) {
        m.weather_set(&mut crate::light::NoLightHooks);
    }
}

/// `WildHooks` that run the ports.
pub struct PortWild;

impl WildHooks for PortWild {}

/// `loop` with CX = 0 runs 65536 times.
fn count(n: u16) -> u32 {
    if n == 0 { 0x10000 } else { n as u32 }
}

impl Machine {
    /// 217B:0004. Generates the wilderness grid for the current location,
    /// seeded from its record's first four bytes (the RNG is restored
    /// afterwards): every cell a block from the weighted lists, then the
    /// walled template's quadrants (see `wild_footprint`), then blocks 1
    /// to 4 at the centre. None where the original divides by zero or EMS
    /// is on.
    pub fn wild_generate(&mut self) -> Option<()> {
        // Where the port gives up, the game's seed is put back as the
        // end puts it, not left at the one this rolled with. The
        // original has no such exit.
        let saved = self.rng.seed;
        let r = self.wild_generate_seeded();
        self.seed_back(saved, r)
    }

    fn wild_generate_seeded(&mut self) -> Option<()> {
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

    /// 217B:0084. The wilderness set up for a run: no window loaded; the
    /// grid made again (as 217B:0004) from the seed, the location's
    /// (A22, kept at SEED) or, coming back out of a dungeon there (C39A
    /// bit 2000h), the one kept, with the saved INF (CB84 SAVED_INF) and
    /// 5C8 bit 40h (the window to be loaded); the EMS banks freed first
    /// (0000:5D70). Coming back, the bit clears and the origin and the
    /// player's place are the saved ones; otherwise the origin is block
    /// (1Fh, 1Fh) and the player is moved by 2000h into the middle half
    /// of the window (the origin following), then away from the gate
    /// (`wild_nudge`). The seed is put back, then the weather set
    /// (0000:0F05, `WildHooks::weather`). None where a block list is
    /// empty (a division by 0).
    pub fn wild_setup(&mut self, hooks: &mut dyn WildHooks) -> Option<()> {
        // Where the port gives up, the game's seed is put back as the
        // end puts it, not left at the one this rolled with. The
        // original has no such exit.
        let saved = self.rng.seed;
        let r = self.wild_setup_seeded(hooks);
        self.seed_back(saved, r)
    }

    fn wild_setup_seeded(&mut self, hooks: &mut dyn WildHooks) -> Option<()> {
        self.mem.w16(DS, WINDOW, 0xFFFF);
        let saved = self.rng.seed;
        let back = self.mem.r16(DS, LEVEL_FLAGS) & BACK != 0;
        let seed = if back {
            self.mem.w16(DS, crate::info::var::INFO_PTR, SAVED_INF);
            self.add16_or(0x05C8, 0x40);
            self.mem.r32(DS, SEED)
        } else {
            let s = self.mem.r32(DS, LOCATION);
            self.mem.w32(DS, SEED, s);
            s
        };
        self.rng.seed = seed;
        self.ems_reset();
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
        if back {
            let f = self.mem.r16(DS, LEVEL_FLAGS) & !BACK;
            self.mem.w16(DS, LEVEL_FLAGS, f);
            for (to, from) in [(ORIGIN_X, SAVED_ORIGIN), (ORIGIN_Y, SAVED_ORIGIN + 2), (X, SAVED_POS), (Y, SAVED_POS + 2), (HEADING, SAVED_POS + 4)] {
                let v = self.mem.r16(DS, from);
                self.mem.w16(DS, to, v);
            }
        } else {
            self.mem.w16(DS, ORIGIN_X, 0x1F);
            self.mem.w16(DS, ORIGIN_Y, 0x1F);
            for (pos, origin) in [(X, ORIGIN_X), (Y, ORIGIN_Y)] {
                if self.mem.r16(DS, pos) < 0x1000 {
                    self.add16(pos, 0x2000);
                    self.add16(origin, 0xFFFF);
                }
                if self.mem.r16(DS, pos) > 0x3000 {
                    self.add16(pos, 0xE000);
                    self.add16(origin, 1);
                }
            }
            self.wild_nudge();
        }
        self.rng.seed = saved;
        hooks.weather(self);
        Some(())
    }

    fn add16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at).wrapping_add(v);
        self.mem.w16(DS, at, x);
    }

    fn add16_or(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) | v;
        self.mem.w16(DS, at, x);
    }

    /// 217B:058F. The player moved 12Ch off the city's gate, by where
    /// the gate square (CB90) is from the place kept outside (FD2): of
    /// their difference (signed) the size, unsigned, above 100h moves
    /// both axes back, exactly 100h y back, above 80h x on and y back,
    /// else x back; back and on swap when the difference is negative.
    fn wild_nudge(&mut self) {
        let mut cx: u16 = 0x12C;
        let mut ax = self.mem.r16(DS, 0xCB90).wrapping_sub(self.mem.r16(DS, 0x0FD2));
        if (ax as i16) < 0 {
            cx = cx.wrapping_neg();
            ax = ax.wrapping_neg();
        }
        let back = cx.wrapping_neg();
        if ax > 0x100 {
            self.add16(X, back);
            self.add16(Y, back);
        } else if ax == 0x100 {
            self.add16(Y, back);
        } else if ax > 0x80 {
            self.add16(X, cx);
            self.add16(Y, back);
        } else {
            self.add16(X, back);
        }
    }

    /// 217B:0265 (each frame of the input step in the wilderness
    /// outdoors, 2826:01AD; through 217B:0184 for the place hook): unless
    /// FROZEN, the window follows the player (`wild_recentre`) and the
    /// blocks under it are loaded where they changed (`wild_window`).
    pub fn wild_shift(&mut self, files: &mut Files, hooks: &mut dyn WildHooks) -> Option<()> {
        if self.mem.r8(DS, FROZEN) != 0 {
            return Some(());
        }
        self.wild_recentre();
        let (x, y) = (self.mem.r16(DS, ORIGIN_X), self.mem.r16(DS, ORIGIN_Y));
        self.wild_window(files, hooks, x, y)
    }

    /// 217B:0434. A quarter of the window crossed (the position's top
    /// two bits changed since LAST_X / LAST_Y): into the first quarter
    /// the player moves 2000h on and the origin a block back, into the
    /// last 2000h back and the origin on, the shift kept (SHIFT_X,
    /// SHIFT_Y); then all 23 actors move by the shift.
    fn wild_recentre(&mut self) {
        self.mem.w16(DS, SHIFT_X, 0);
        self.mem.w16(DS, SHIFT_Y, 0);
        for (pos, last, origin, shift) in [(X, LAST_X, ORIGIN_X, SHIFT_X), (Y, LAST_Y, ORIGIN_Y, SHIFT_Y)] {
            let q = self.mem.r16(DS, pos) >> 12;
            if q != self.mem.r16(DS, last) >> 12 {
                if q == 0 {
                    self.mem.w16(DS, shift, 0x2000);
                    self.add16(pos, 0x2000);
                    self.add16(origin, 0xFFFF);
                } else if q == 3 {
                    self.mem.w16(DS, shift, 0xE000);
                    self.add16(pos, 0xE000);
                    self.add16(origin, 1);
                }
            }
            let v = self.mem.r16(DS, pos);
            self.mem.w16(DS, last, v);
        }
        let (dx, dy) = (self.mem.r16(DS, SHIFT_X), self.mem.r16(DS, SHIFT_Y));
        for i in 0..0x17u16 {
            let si = 0x0AB3 + i * 0x1C;
            self.add16(si, dx);
            self.add16(si + 4, dy);
        }
    }

    /// 217B:03C0. The window's four blocks, the grid's words at the
    /// origin (x, y) and the row under it, compared with those loaded
    /// (WINDOW, four bytes); where they changed, or 5C8 bit 40h asks, the
    /// four are loaded (`WildHooks::load_block`, slots 0..3) with 5CA bit
    /// 4 set and 8848 cleared, then 5C8 bits 40h, 200h and 400h clear.
    fn wild_window(&mut self, files: &mut Files, hooks: &mut dyn WildHooks, x: u16, y: u16) -> Option<()> {
        let at = (y << 6).wrapping_add(x).wrapping_add(GRID);
        self.ems_map(GRID_PAGE);
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let mut changed = false;
        for (k, at) in [(0u16, at), (2, at.wrapping_add(0x40))] {
            let w = self.mem.r16(buf, at);
            if self.mem.r16(DS, WINDOW + k) != w {
                self.mem.w16(DS, WINDOW + k, w);
                changed = true;
            }
        }
        if self.mem.r16(DS, 0x05C8) & 0x40 == 0 && !changed {
            return Some(());
        }
        let v = self.mem.r16(DS, 0x05C8) & !0x40;
        self.mem.w16(DS, 0x05C8, v);
        self.add16_or(0x05CA, 4);
        self.mem.w16(DS, 0x8848, 0);
        for slot in 0..4u16 {
            let block = self.mem.r8(DS, WINDOW + slot);
            hooks.load_block(self, files, slot as u8, block)?;
        }
        let v = self.mem.r16(DS, 0x05C8) & 0xF9FF;
        self.mem.w16(DS, 0x05C8, v);
        Some(())
    }

    /// 217B:04DC. The wilderness left: the player's x, y and heading and
    /// the origin kept (SAVED_POS, SAVED_ORIGIN), and the INF's name
    /// (C2B5) copied to SAVED_INF (0000:9CA0).
    pub fn wild_done(&mut self) {
        for (to, from) in [(SAVED_POS, X), (SAVED_POS + 2, Y), (SAVED_POS + 4, HEADING), (SAVED_ORIGIN, ORIGIN_X), (SAVED_ORIGIN + 2, ORIGIN_Y)] {
            let v = self.mem.r16(DS, from);
            self.mem.w16(DS, to, v);
        }
        let (mut si, mut di) = (0xC2B5u16, SAVED_INF);
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

    /// 0000:08EC. A dungeon's entrance in the wilderness taken: the
    /// wilderness left (`wild_done`), the dungeon is the wilderness's (5C8
    /// bit 80h), the area left (C398 bit 80h), not the wilderness (C39A
    /// bit 200h), the area to enter a dungeon (5E6 3). The wall triggers
    /// 8 and 9 there (`Machine::wall_trigger`) run the same with
    /// `wild_done` as their hook; clicking a dungeon's keeper in the
    /// wilderness (`talk_to`, `TalkHooks::dungeon_entrance`) runs it after
    /// clearing the view's window (0000:4EFD).
    pub fn wild_dungeon_entry(&mut self) {
        self.wild_done();
        let v = self.mem.r16(DS, 0x05C8) | 0x80;
        self.mem.w16(DS, 0x05C8, v);
        let v = self.mem.r16(DS, 0xC398) | 0x80;
        self.mem.w16(DS, 0xC398, v);
        let v = self.mem.r16(DS, LEVEL_FLAGS) & 0xFDFF;
        self.mem.w16(DS, LEVEL_FLAGS, v);
        self.mem.w8(DS, crate::trigger::var::AREA, 3);
    }

    /// 217B:027A. A city's gate taken out to the wilderness: the city
    /// becomes blocks 1..4 of its wilderness (the four quarters of the
    /// 128 x 128 map, at WINDOW_OFFSETS), written as wild001.rmd ..
    /// wild004.rmd in the game's directory (25FD:0844, 0000:9DD2), where
    /// `wild_load_block` finds them. The map's rows past its depth (C359)
    /// and columns past its width (C357) are cleared first (bank [1030]
    /// mapped). A block is three 64 x 64 planes made in the buffer
    /// segment from offset 0: the floor (less the buffer segment's
    /// number, 12 bits), at 2000h the walls and at 4000h the ceiling,
    /// from the city's walls: of the words with both bytes equal, a
    /// wall of type Ah (bit 15 set, its low six bits 33h..35h) becomes
    /// 3030h with a ceiling B0B0h; 2Fh, 2Dh and 2Eh (bit 15 clear) become
    /// 3030h, 2F2Fh and 2E2Eh with a ceiling 3030h plus the bits 8080h of
    /// the ceiling word one square on (the original reads past the one
    /// it converts); anything else is 0. The three planes (6000h bytes
    /// at the buffer's UNPACK_OFF) are packed (`wild_compress`) to 6000h
    /// past it, and written from buffer:6000h whatever UNPACK_OFF is.
    pub fn wild_leave_city(&mut self, files: &mut Files, hooks: &mut dyn WildHooks) {
        use crate::mif::var::{BUFFER_SEG, FLOOR_SEG, MAP1_SEG, MAP2_SEG};
        let page = self.mem.r16(DS, 0x1030);
        self.ems_map(page);
        let walls = self.mem.r16(DS, MAP1_SEG);
        let depth = self.mem.r16(DS, 0xC359);
        if depth != 0x80 {
            let di = depth << 8;
            for i in 0..0x80u16.wrapping_sub(depth) << 8 {
                self.mem.w8(walls, di.wrapping_add(i), 0);
            }
        }
        let width = self.mem.r16(DS, 0xC357);
        if width != 0x80 {
            let n = 0x80u16.wrapping_sub(width);
            let mut di = width.wrapping_add(width);
            for _ in 0..count(self.mem.r16(DS, 0xC359)) {
                for i in 0..n {
                    self.mem.w16(walls, di.wrapping_add(i.wrapping_mul(2)), 0);
                }
                di = di.wrapping_add(0x100);
            }
        }
        for (i, c) in [(4, b'0'), (5, b'0'), (6, b'0')] {
            self.mem.w8(DS, BLOCK_NAME + i, c);
        }
        for k in 0..4u16 {
            let d = self.mem.r8(DS, BLOCK_NAME + 6).wrapping_add(1);
            self.mem.w8(DS, BLOCK_NAME + 6, d);
            let mut si = self.mem.r16(DS, WINDOW_OFFSETS + 2 * k);
            let bp = self.mem.r16(DS, BUFFER_SEG);
            let (ceiling, floor, es) = (self.mem.r16(DS, MAP2_SEG), self.mem.r16(DS, FLOOR_SEG), bp);
            let mut di = 0u16;
            for _ in 0..0x40 {
                let row = si;
                for _ in 0..0x40 {
                    let ax = self.mem.r16(walls, si);
                    si = si.wrapping_add(2);
                    let (lo, hi) = (ax as u8, (ax >> 8) as u8);
                    let (a, mut b, merge) = if lo != hi || ax == 0 {
                        (0, 0, false)
                    } else if ax & 0x8000 != 0 {
                        let c = lo & 0x3F;
                        if (ax >> 12) & 0xFF == 0xA && (0x33..=0x35).contains(&c) { (0x3030, 0xB0B0, false) } else { (0, 0, false) }
                    } else {
                        match lo {
                            0x2F => (0x3030, 0x3030, true),
                            0x2D => (0x2F2F, 0x3030, true),
                            0x2E => (0x2E2E, 0x3030, true),
                            _ => (0, 0, false),
                        }
                    };
                    if merge {
                        b |= self.mem.r16(ceiling, si) & 0x8080;
                    }
                    self.mem.w16(es, di.wrapping_add(0x2000), a);
                    self.mem.w16(es, di.wrapping_add(0x4000), b);
                    let f = self.mem.r16(floor, si.wrapping_sub(2)).wrapping_sub(bp) & 0xFFF;
                    self.mem.w16(es, di, f);
                    di = di.wrapping_add(2);
                }
                si = row.wrapping_add(0x100);
            }
            let off = self.mem.r16(DS, UNPACK_OFF);
            let bx = off.wrapping_add(0x6000);
            let n = self.wild_compress(bp, off, bp, bx, 0x6000, bx);
            let name = self.save_path(BLOCK_NAME);
            let seg = self.mem.r16(DS, BUFFER_SEG);
            hooks.write(self, files, name, seg, 0x6000, n);
        }
    }

    /// 2294:0D03. Packs the `len` bytes at src_seg:src as words into
    /// dst_seg:dst in `wild_unpack`'s form: the word count (len / 2),
    /// then runs: a negative count and a word repeated, or a count and
    /// that many words. Returns the size written in bytes. Faithful to
    /// its quirks: a run is found by comparing each word with the next
    /// (the word past the end is read), a run closed at the end repeats
    /// the word BX held (`bx` on entry, then the last word compared),
    /// and the routine ends when the next word would start at or past
    /// src + len.
    pub fn wild_compress(&mut self, src_seg: u16, src: u16, dst_seg: u16, dst: u16, len: u16, bx: u16) -> u16 {
        let (mut si, mut di, mut bx) = (src, dst, bx);
        let end = len.wrapping_add(si);
        let w16 = |m: &mut Self, di: &mut u16, v: u16| {
            m.mem.w16(dst_seg, *di, v);
            *di = di.wrapping_add(2);
        };
        w16(self, &mut di, len >> 1);
        let mut size: u16 = 2;
        // [bp-2]: 1 a literal run, 2 a repeat (0 before the first).
        let mut mode = 0u16;
        let mut lit = 0u16;
        let mut cx: u16;
        loop {
            cx = 1;
            if si >= end {
                break;
            }
            mode = 2;
            let mut ax = self.mem.r16(src_seg, si);
            si = si.wrapping_add(2);
            if self.mem.r16(src_seg, si) != ax {
                lit = si.wrapping_sub(2);
                mode = 1;
            }
            let ended = loop {
                // 2294:0DC5: the next word, or the end.
                if si >= end {
                    break true;
                }
                bx = ax;
                ax = self.mem.r16(src_seg, si);
                si = si.wrapping_add(2);
                if ax != bx {
                    if mode == 1 {
                        cx = cx.wrapping_add(1);
                        continue;
                    }
                    if mode == 2 {
                        w16(self, &mut di, cx.wrapping_neg());
                        w16(self, &mut di, bx);
                        size = size.wrapping_add(4);
                    }
                } else {
                    if mode == 2 {
                        cx = cx.wrapping_add(1);
                        continue;
                    }
                    if mode == 1 {
                        w16(self, &mut di, cx);
                        for i in 0..cx {
                            let w = self.mem.r16(src_seg, lit.wrapping_add(i.wrapping_mul(2)));
                            w16(self, &mut di, w);
                        }
                        size = size.wrapping_add(2).wrapping_add(cx.wrapping_add(cx));
                    }
                }
                si = si.wrapping_sub(2);
                break false;
            };
            if ended {
                break;
            }
        }
        if mode == 2 {
            w16(self, &mut di, cx.wrapping_neg());
            w16(self, &mut di, bx);
            size = size.wrapping_add(4);
        } else {
            w16(self, &mut di, cx);
            for i in 0..cx {
                let w = self.mem.r16(src_seg, lit.wrapping_add(i.wrapping_mul(2)));
                w16(self, &mut di, w);
            }
            size = size.wrapping_add(2).wrapping_add(cx.wrapping_add(cx));
        }
        size
    }
}
