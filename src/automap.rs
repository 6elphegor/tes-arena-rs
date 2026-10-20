//! The automap: which squares of a city's or a dungeon's level the player
//! has seen (segments 0000 and 23F1).
//!
//! The map is 128 x 128 bytes in EMS bank 11h (at the frame, the far
//! pointer 902E): 0 unseen, then 7, 0Eh and 15h nearer the player's path.
//! Each frame (0000:49D6, `automap_mark`) the squares around the player
//! are raised to the 9 x 9 pattern at MARKS. A city's map is shown whole
//! (all 15h), and so are an interior's and the wilderness's, which
//! aren't kept.
//!
//! AUTOMAP.nn (the slot's digits, from 1A6C when it is loaded) holds 16
//! records of 2004h bytes: a location's key (`automap_key`), then 2000h
//! bytes whose second half is its map, four squares a byte. Entering an
//! area (0000:3503, `automap_load`) finds its record by its key, or takes
//! the next of the 16 in turn (5DA) for it; leaving it (23F1:013C,
//! `automap_keep`) writes its map back. A file of another size (or none)
//! is made again: from the walls plane's bytes, as the original writes
//! them, so its keys are whatever those bytes are.
//!
//! The packing puts a group's first square in the top bits and the
//! unpacking takes it from the bottom ones: each group of four squares
//! comes back reversed (kept, as the original does it).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{LEVEL_FLAGS, MAP_FLAGS};
use crate::trigger::var::AREA;

/// DGROUP variables.
pub mod var {
    /// The automap screen's centre (map units; also the talk's flat kind
    /// and its neighbour when no map is up) and its top left square.
    pub const MAP_X: u16 = 0x126C;
    pub const MAP_Y: u16 = 0x126E;
    pub const MAP_LEFT: u16 = 0xA880;
    pub const MAP_TOP: u16 = 0xA882;
    /// "automap.00", its digits at +8.
    pub const NAME: u16 = 0x663B;
    /// The record's key as read or written (dword), its position in the
    /// file (dword), and whether a record is in use (byte).
    pub const KEY: u16 = 0x6637;
    pub const POSITION: u16 = 0x66C6;
    pub const IN_USE: u16 = 0x6636;
    /// The record a new location takes next (0..15).
    pub const NEXT: u16 = 0x05DA;
    /// The 9 x 9 marks around the player.
    pub const MARKS: u16 = 0x6674;
    /// The four values a packed square unpacks to (0, 7, 0Eh, 15h).
    pub const CODES: u16 = 0x4730;
    /// The frame's far pointer (offset, then the segment, 9030).
    pub const FRAME: u16 = 0x902E;
    /// The EMS bank the map is kept in.
    pub const BANK: u16 = 0x11;
}
use var::*;

/// The file's size: 16 records of 2004h bytes.
const FILE_SIZE: usize = 0x20040;
const RECORD: usize = 0x2004;

impl Machine {
    fn frame(&self) -> (u16, u16) {
        (self.mem.r16(DS, FRAME + 2), self.mem.r16(DS, FRAME))
    }

    /// 23F1:0149. Which map the area keeps: 1 a city (outdoors, not the
    /// wilderness), 2 a dungeon (indoors, not an interior), else 0.
    pub fn automap_kind(&self) -> u16 {
        if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 {
            if self.mem.r8(DS, AREA) == 4 { 0 } else { 2 }
        } else if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            0
        } else {
            1
        }
    }

    /// 0000:386F. The location's key: C0000000h in the start dungeon (5E4
    /// bit 1); otherwise the high word (type A47, or 3 in a dungeon's
    /// level indoors, times 8 plus the province A46, times 8, or the
    /// level BB0A) and the location (A4C), all in 8 bits; the low word
    /// the wilderness's block (60C, 60E, each one more past 2000h of
    /// 610, 612) for a place entered from it (5C8 bit 4000h), else 0.
    pub fn automap_key(&self) -> u32 {
        if self.mem.r8(DS, 0x05E4) & 1 != 0 {
            return 0xC000_0000;
        }
        let mut bl = self.mem.r8(DS, 0x060C);
        let mut bh = self.mem.r8(DS, 0x060E);
        if self.mem.r16(DS, 0x0610) >= 0x2000 {
            bl = bl.wrapping_add(1);
        }
        if self.mem.r16(DS, 0x0612) >= 0x2000 {
            bh = bh.wrapping_add(1);
        }
        let al = self.mem.r8(DS, 0x0A4C);
        let mut ah = self.mem.r8(DS, 0x0A47);
        if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 && self.mem.r8(DS, AREA) == 3 {
            ah = 3;
        }
        let ah = ((ah << 3).wrapping_add(self.mem.r8(DS, 0x0A46)) << 3) | self.mem.r8(DS, 0xBB0A);
        let low = if self.mem.r16(DS, 0x05C8) & 0x4000 != 0 { (bh as u32) << 8 | bl as u32 } else { 0 };
        (ah as u32) << 24 | (al as u32) << 16 | low
    }

    /// 0000:3491. The squares around the player seen: but in an interior
    /// or the wilderness, the map's bank mapped and each of the 9 x 9
    /// squares around the player's (the row by 80h, wrapping in the
    /// frame) raised to its mark at MARKS where that is not 0.
    pub fn automap_mark(&mut self) {
        if self.mem.r8(DS, AREA) == 4 {
            return;
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 && self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            return;
        }
        self.ems_map(BANK);
        let row = self.mem.r16(DS, 0xC3D4) & 0xFF80;
        let col = (self.mem.r16(DS, 0xC3D0) >> 7) & 0x7F;
        let seg = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let mut di = col.wrapping_add(row).wrapping_sub(0x204);
        let mut si = MARKS;
        for _ in 0..9 {
            let mut d = di;
            for _ in 0..9 {
                let al = self.mem.r8(DS, si);
                si = si.wrapping_add(1);
                if al != 0 && self.mem.r8(seg, d) < al {
                    self.mem.w8(seg, d, al);
                }
                d = d.wrapping_add(1);
            }
            di = di.wrapping_add(0x80);
        }
    }

    /// 0000:67C8 (and 67C4) on the file named at DS:`path`: its name
    /// (A908), its size (A90C) or, missing, the BSA looked in and the
    /// error reported (`file_open_or_device`). Its bytes, where it is a
    /// file, closed (the port reads them whole). Missing, nothing is
    /// closed: 3546 goes on to make it (355D's carry, 677E), and what DOS
    /// has on handle 2 stays (CON at first).
    fn automap_open(&mut self, files: &mut Files, path: u16) -> Option<std::rc::Rc<[u8]>> {
        if self.file_open_or_device(files, path) != Some(true) {
            return None;
        }
        self.file_close(files);
        files.data(self.mem.cstr(DS, path))
    }

    /// 0000:34F1: the map cleared (4000h bytes at the frame).
    fn automap_clear(&mut self) {
        let (seg, off) = self.frame();
        for i in 0..0x4000u16 {
            self.mem.w8(seg, off.wrapping_add(i), 0);
        }
    }

    /// 0000:3503. Entering an area: AUTOMAP.nn named by the slot (1A6C),
    /// the map's bank mapped and cleared; for a city or a dungeon its
    /// record found or made (0000:3546); a city's map then all seen (15h,
    /// 0000:3536), and so is the map of an area that keeps none (an
    /// interior, the wilderness).
    pub fn automap_load(&mut self, files: &mut Files) {
        let slot = self.mem.r8(DS, crate::save::var::SLOT);
        let digits = (((slot % 10) as u16) << 8 | (slot / 10) as u16).wrapping_add(0x3030);
        self.mem.w16(DS, NAME + 8, digits);
        self.ems_map(BANK);
        self.automap_clear();
        let kind = self.automap_kind();
        if kind != 0 {
            self.automap_record(files);
        }
        if kind == 0 || self.automap_kind() == 1 {
            let (seg, off) = self.frame();
            for i in 0..0x4000u16 {
                self.mem.w8(seg, off.wrapping_add(i), 0x15);
            }
        }
    }

    /// 0000:677E: a file created (its name kept at OPEN_NAME), from the
    /// bytes at DS:0 of the walls plane's segment as 0000:3546 writes them:
    /// FFFFh bytes twice, then 42h.
    fn automap_create(&mut self, files: &mut Files) -> Vec<u8> {
        let path = self.save_path(NAME);
        self.mem.w16(DS, crate::bsa::var::OPEN_NAME, path);
        self.mem.w16(DS, crate::bsa::var::OPEN_NAME + 2, DS);
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let block = |n: u16| (0..n).map(|i| self.mem.r8(walls, i)).collect::<Vec<u8>>();
        let mut data = block(0xFFFF);
        data.extend(block(0xFFFF));
        data.extend(block(0x42));
        let name = self.mem.cstr(DS, path).to_vec();
        files.store(&name, data.clone());
        data
    }

    /// 0000:3546. The area's record in AUTOMAP.nn: NEXT kept below 10h;
    /// the file opened, or made when missing or of another size than
    /// 16 records (deleted first); the keys read in turn (KEY) to the
    /// area's (`automap_key`): found, its position kept and its map read
    /// and unpacked (23F1:037A); not found, the record NEXT taken (NEXT
    /// moves on), its key and 2000h zeros written, the map cleared and
    /// 837A FFFFh. Either way IN_USE.
    fn automap_record(&mut self, files: &mut Files) {
        if self.mem.r8(DS, NEXT) >= 0x10 {
            self.mem.w8(DS, NEXT, 0);
        }
        let path = self.save_path(NAME);
        let name = self.mem.cstr(DS, path).to_vec();
        let mut data = match self.automap_open(files, path) {
            Some(d) if d.len() == FILE_SIZE => d.to_vec(),
            Some(_) => {
                // Closed, deleted (its path made again), then made.
                self.save_path(NAME);
                self.automap_create(files)
            }
            None => self.automap_create(files),
        };
        let key = self.automap_key();
        let mut pos = 0usize;
        loop {
            let got = data.get(pos..(pos + 4).min(data.len())).unwrap_or(&[]).to_vec();
            for (i, &b) in got.iter().enumerate() {
                self.mem.w8(DS, KEY + i as u16, b);
            }
            pos += got.len();
            if got.is_empty() {
                break;
            }
            if self.mem.r32(DS, KEY) == key {
                // 0000:3640: found.
                self.mem.w8(DS, IN_USE, 1);
                self.mem.w32(DS, POSITION, (pos - 4) as u32);
                pos += 0x1000;
                let (seg, off) = self.frame();
                let packed = data.get(pos.min(data.len())..(pos + 0x1000).min(data.len())).unwrap_or(&[]).to_vec();
                for (i, &b) in packed.iter().enumerate() {
                    self.mem.w8(seg, off.wrapping_add(0x4000).wrapping_add(i as u16), b);
                }
                self.automap_unpack();
                return;
            }
            pos += 0x2000;
        }
        // 0000:35E5: not found.
        let next = self.mem.r8(DS, NEXT);
        let at = next as usize * RECORD;
        self.mem.w8(DS, NEXT, next.wrapping_add(1) & 0xF);
        self.mem.w32(DS, POSITION, at as u32);
        self.mem.w8(DS, IN_USE, 1);
        self.mem.w32(DS, KEY, key);
        let (seg, off) = self.frame();
        for i in 0..0x2000u16 {
            self.mem.w8(seg, off.wrapping_add(i), 0);
        }
        let end = at + 4 + 0x2000;
        if data.len() < end {
            data.resize(end, 0);
        }
        data[at..at + 4].copy_from_slice(&key.to_le_bytes());
        data[at + 4..end].fill(0);
        files.store(&name, data);
        self.automap_clear();
        self.mem.w16(DS, 0x837A, 0xFFFF);
    }

    /// 23F1:037A. The map unpacked: each of the 1000h bytes at frame +4000h
    /// to four squares, its bottom two bits first, through CODES.
    fn automap_unpack(&mut self) {
        let (seg, off) = self.frame();
        for k in 0..0x1000u16 {
            let mut b = self.mem.r8(seg, off.wrapping_add(0x4000).wrapping_add(k));
            for j in 0..4u16 {
                let v = self.mem.r8(DS, CODES + (b & 3) as u16);
                self.mem.w8(seg, off.wrapping_add(k * 4 + j), v);
                b >>= 2;
            }
        }
    }

    /// 23F1:034E. The map packed to frame +4000h: four squares a byte, the
    /// first in the top bits, each 0 (0), 1 (7), 2 (0Eh) or 3 (anything
    /// else).
    fn automap_pack(&mut self) {
        let (seg, off) = self.frame();
        for k in 0..0x1000u16 {
            let mut ah = 0u8;
            for j in 0..4u16 {
                let al = self.mem.r8(seg, off.wrapping_add(k * 4 + j));
                let dl = match al {
                    0 => 0,
                    7 => 1,
                    0xE => 2,
                    _ => 3,
                };
                ah = ah << 2 | dl;
            }
            self.mem.w8(seg, off.wrapping_add(0x4000).wrapping_add(k), ah);
        }
    }

    /// 23F1:013C. Leaving an area: its map written back (23F1:0178) and
    /// no record in use. 0178: for a city or a dungeon with a record in
    /// use, 837A FFFFh, the map's bank mapped, the map packed and written
    /// at its record's position + 1004h (the file opened by the name as
    /// it stands; nothing where it is missing).
    pub fn automap_keep(&mut self, files: &mut Files) {
        self.automap_write(files);
        self.mem.w8(DS, IN_USE, 0);
    }

    /// 23F1:0145 (0178), as saving a game writes the map: `automap_keep`
    /// without clearing IN_USE.
    pub fn automap_write(&mut self, files: &mut Files) {
        if self.automap_kind() == 0 || self.mem.r8(DS, IN_USE) == 0 {
            return;
        }
        self.mem.w16(DS, 0x837A, 0xFFFF);
        self.ems_map(BANK);
        let path = self.save_path(NAME);
        let name = self.mem.cstr(DS, path).to_vec();
        let found = self.automap_open(files, path);
        let at = self.mem.r32(DS, POSITION).wrapping_add(0x1004) as usize;
        self.automap_pack();
        let Some(d) = found else { return };
        let mut data = d.to_vec();
        if data.len() < at + 0x1000 {
            data.resize(at + 0x1000, 0);
        }
        let (seg, off) = self.frame();
        for i in 0..0x1000usize {
            data[at + i] = self.mem.r8(seg, off.wrapping_add(0x4000).wrapping_add(i as u16));
        }
        files.store(&name, data);
    }

    /// 292B:0A2B. Whether the level's locks and triggers are kept in
    /// STATES.nn: a dungeon (A47 3) of the first two of a province's
    /// (A4C & 1Fh below 2: the main quest's).
    pub fn states_kept(&self) -> bool {
        self.mem.r8(DS, 0x0A47) == 3 && self.mem.r8(DS, 0x0A4C) & 0x1F < 2
    }

    /// 292B:0A0E. The level's record in STATES.nn: ((A46 x 2 + (A4C & 1Fh))
    /// x 4 + the level BB0A, in 8 bits) x 1C2h.
    fn states_at(&self) -> usize {
        let al = self.mem.r8(DS, 0x0A46).wrapping_mul(2).wrapping_add(self.mem.r8(DS, 0x0A4C) & 0x1F);
        let al = al.wrapping_mul(4).wrapping_add(self.mem.r8(DS, 0xBB0A));
        al as usize * 0x1C2
    }

    /// The four blocks of a STATES record: the triggers' count and
    /// squares, the locks' count and locks.
    const STATES_BLOCKS: [(u16, usize); 4] = [(0xC866, 1), (0xC766, 0x100), (0xC6E4, 1), (0xC624, 0xC0)];

    /// 292B:09AC. A main quest dungeon's level (`states_kept`) entered: its
    /// record read from STATES.nn (named as it stands at 7D06): its first
    /// byte to A836; 0 (never kept) writes the level's own instead
    /// (`states_keep`); otherwise the triggers' count, their squares, the
    /// locks' count and the locks read. Nothing where the file is missing.
    pub fn states_load(&mut self, files: &mut Files) {
        if !self.states_kept() {
            return;
        }
        let path = self.save_path(0x7D06);
        let Some(data) = self.automap_open(files, path) else { return };
        let mut pos = self.states_at();
        let get = |pos: usize, n: usize| data.get(pos.min(data.len())..(pos + n).min(data.len())).unwrap_or(&[]).to_vec();
        if let Some(&b) = get(pos, 1).first() {
            self.mem.w8(DS, 0xA836, b);
        }
        pos += 1;
        let first = self.mem.r8(DS, 0xA836);
        if first == 0 {
            self.states_keep(files);
            return;
        }
        self.mem.w8(DS, 0xC866, first);
        for (at, n) in &Self::STATES_BLOCKS[1..] {
            for (i, b) in get(pos, *n).into_iter().enumerate() {
                self.mem.w8(DS, at + i as u16, b);
            }
            pos += n;
        }
    }

    /// 292B:095E. A main quest dungeon's level left (or first entered):
    /// its triggers' count, their squares, its locks' count and its locks
    /// written to its record in STATES.nn. Nothing where the file is
    /// missing.
    pub fn states_keep(&mut self, files: &mut Files) {
        if !self.states_kept() {
            return;
        }
        let path = self.save_path(0x7D06);
        let name = self.mem.cstr(DS, path).to_vec();
        let Some(d) = self.automap_open(files, path) else { return };
        let mut data = d.to_vec();
        let mut pos = self.states_at();
        for (at, n) in Self::STATES_BLOCKS {
            if data.len() < pos + n {
                data.resize(pos + n, 0);
            }
            for i in 0..n {
                data[pos + i] = self.mem.r8(DS, at + i as u16);
            }
            pos += n;
        }
        files.store(&name, data);
    }

    /// 0000:3796. The automap screen's centre (MAP_X, MAP_Y, in map units)
    /// kept inside the level when it is at least 60 x 42 squares: x from
    /// F00h (signed) to under (width << 7) - E80h, where it is put 80h less
    /// than that bound; y from A7Fh to under (depth << 7) - A00h, likewise.
    pub fn automap_centre(&mut self) {
        let w = self.mem.r16(DS, 0xC357);
        let d = self.mem.r16(DS, 0xC359);
        if (w as i16) < 0x3C || (d as i16) < 0x2A {
            return;
        }
        if (self.mem.r16(DS, MAP_X) as i16) < 0xF00 {
            self.mem.w16(DS, MAP_X, 0xF00);
        }
        if (self.mem.r16(DS, MAP_Y) as i16) < 0xA7F {
            self.mem.w16(DS, MAP_Y, 0xA7F);
        }
        let ax = (w << 7).wrapping_sub(0xE80);
        if (self.mem.r16(DS, MAP_X) as i16) >= ax as i16 {
            self.mem.w16(DS, MAP_X, ax.wrapping_sub(0x80));
        }
        let ay = (d << 7).wrapping_sub(0xA00);
        if (self.mem.r16(DS, MAP_Y) as i16) >= ay as i16 {
            self.mem.w16(DS, MAP_Y, ay.wrapping_sub(0x80));
        }
    }

    /// 0000:377D. The automap screen's top left square from its centre:
    /// (x >> 7) - 1Eh, (y >> 7) - 15h (MAP_LEFT, MAP_TOP).
    pub fn automap_origin(&mut self) {
        let x = (self.mem.r16(DS, MAP_X) >> 7).wrapping_sub(0x1E);
        let y = (self.mem.r16(DS, MAP_Y) >> 7).wrapping_sub(0x15);
        self.mem.w16(DS, MAP_LEFT, x);
        self.mem.w16(DS, MAP_TOP, y);
    }

    /// 0000:2D1F..2D2E. The automap screen opened: its centre on the
    /// player, kept inside the level, and its top left square.
    pub fn automap_view_open(&mut self) {
        let (x, y) = (self.mem.r16(DS, 0xC3D0), self.mem.r16(DS, 0xC3D4));
        self.mem.w16(DS, MAP_X, x);
        self.mem.w16(DS, MAP_Y, y);
        self.automap_centre();
        self.automap_origin();
    }

    /// The automap screen's buttons (the table at 66CA, handlers at 66F2):
    /// 0 up (0000:37ED, y less 80h), 1 down (0000:37F7), 2 the right arrow
    /// (0000:3801, x less 80h: the map is drawn with x falling to the right),
    /// 3 the left arrow (0000:380B), 4 the centre (0000:3815, back on the player);
    /// each then kept inside the level (3796). The screen's loop makes the
    /// top left square again after (377D, `automap_origin`, 0000:2E75).
    pub fn automap_view_button(&mut self, b: u16) {
        let add = |m: &mut Machine, at: u16, v: u16| {
            let x = m.mem.r16(DS, at).wrapping_add(v);
            m.mem.w16(DS, at, x);
        };
        match b {
            0 => add(self, MAP_Y, 0xFF80),
            1 => add(self, MAP_Y, 0x80),
            2 => add(self, MAP_X, 0xFF80),
            3 => add(self, MAP_X, 0x80),
            _ => {
                let (x, y) = (self.mem.r16(DS, 0xC3D0), self.mem.r16(DS, 0xC3D4));
                self.mem.w16(DS, MAP_X, x);
                self.mem.w16(DS, MAP_Y, y);
            }
        }
        self.automap_centre();
    }
}
