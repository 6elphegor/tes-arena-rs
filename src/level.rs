//! The .MIF level loader (0000:E87C, near entry E880): city templates,
//! interiors and dungeons all load through it.
//!
//! A .MIF file is a 4-byte tag ("MHDR"), a word length and that many
//! header bytes (read to DGROUP C342: start positions, the starting
//! level, the map width and depth in words), then one or more levels.
//! Each level is a "LEVL" tag and length followed by chunks of a 4-byte
//! tag and a word length:
//! - FLOR, MAP1, MAP2: a plane (floor, walls, ceiling), LZHUF-compressed
//!   (see `mif`). It is decompressed to the end of its 32 KB plane and
//!   copied to rows of 128 words, adding the buffer segment number to
//!   every floor word; the rest of the plane is then filled with that
//!   segment number (in walls and ceiling too).
//! - LOCK, TRIG, TARG, LITE, NAME: read to fixed DGROUP buffers, with a
//!   record count derived from the length (3, 4, 2 and 6 bytes per
//!   record; NAME has none).
//! - TEXT: read to the buffer segment at the offset in CB88.
//! - INFO: read to E2BE, unless one was already loaded (CB84 set).
//! - anything else is skipped.
//!
//! The chunk loop ends at end of file or at the next LEVL tag.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::mif::var::{BUFFER_SEG, CHUNK_LEN, FLOOR_SEG, MAP1_SEG, MAP2_SEG, TAG};

/// DGROUP variables.
pub mod var {
    /// The open file's handle (see `Machine::file_open_handle`).
    pub const HANDLE: u16 = 0xA848;
    /// Length of the previous level header (CHUNK_LEN + 4: the 6-byte
    /// tag-and-length read leaves it there).
    pub const LEVEL_LEN: u16 = 0xA83A;
    /// Header body; START_LEVEL is its byte 0x12, MAP_WIDTH and
    /// MAP_DEPTH (words) bytes 0x15 and 0x17.
    pub const HEADER: u16 = 0xC342;
    pub const START_X: u16 = 0xC344;
    pub const START_Y: u16 = 0xC34C;
    pub const START_LEVEL: u16 = 0xC354;
    pub const MAP_WIDTH: u16 = 0xC357;
    pub const MAP_DEPTH: u16 = 0xC359;
    /// Level loaded (byte), and a level to load instead of START_LEVEL
    /// (0xFF: none; cleared after use).
    pub const LEVEL: u16 = 0xBB0A;
    pub const LEVEL_OVERRIDE: u16 = 0xBB0B;
    /// Chunk buffers and their record counts (bytes, LITE a word).
    pub const LOCKS: u16 = 0xC624;
    pub const LOCK_COUNT: u16 = 0xC6E4;
    pub const TARGETS: u16 = 0xC6E5;
    pub const TARGET_COUNT: u16 = 0xC765;
    pub const TRIGGERS: u16 = 0xC766;
    pub const TRIGGER_COUNT: u16 = 0xC866;
    pub const LIGHTS: u16 = 0xBC90;
    pub const LIGHT_COUNT: u16 = 0xBC8E;
    pub const LEVEL_NAME: u16 = 0xC294;
    pub const INFO: u16 = 0xE2BE;
    /// Offset of the loaded INFO text; 0 when none is loaded.
    pub const INFO_PTR: u16 = 0xCB84;
    /// TEXT goes to the buffer segment at this offset, after mapping
    /// this EMS page.
    pub const TEXT_OFF: u16 = 0xCB88;
    pub const TEXT_PAGE: u16 = 0xC8FE;
    /// Bit 2 is set when the level has a ceiling (MAP2).
    pub const MAP_FLAGS: u16 = 0xC398;
    /// Bit 9 keeps the player's position.
    pub const FLAGS: u16 = 0xC39A;
    /// Player position, and the position used with LEVEL_OVERRIDE.
    pub const PLAYER_X: u16 = 0xC3D0;
    pub const PLAYER_Y: u16 = 0xC3D4;
    pub const RETURN_X: u16 = 0xC3CC;
    pub const RETURN_Y: u16 = 0xC3CE;
    /// Set to 0xFFFF on every load.
    pub const W_C8ED: u16 = 0xC8ED;
    /// The level file's name (kept by the caller), and a far pointer
    /// called after a level change, when set.
    pub const LEVEL_FILE: u16 = 0xC37F;
    pub const CHANGE_HOOK: u16 = 0xCD32;
    /// Far pointer called after a load with LEVEL_OVERRIDE, when set.
    pub const LEVEL_HOOK: u16 = 0xCD36;
    /// Set by the reload path (0000:A548): each plane then frees the
    /// buffer block (far pointer BUFFER_BLOCK, normally the buffer
    /// segment) first and allocates a new one of BUFFER_SIZE bytes after.
    pub const RELOAD: u16 = 0xBA16;
    pub const BUFFER_BLOCK: u16 = 0xBA54;
    pub const BUFFER_SIZE: u16 = 0xCB8E;
    /// EMS: enabled flag and page count (0000:5C8A).
    pub const EMS_ON: u16 = 0x8FA6;
    pub const EMS_PAGES: u16 = 0x9032;
    /// The bank mapped, times 4 (an index into the driver's page maps).
    pub const EMS_MAPPED: u16 = 0x8FA8;
    /// Free bytes in each bank (words; see `Machine::ems_alloc`).
    pub const BANK_FREE: u16 = 0x8FAA;
}
use var::*;

/// Bytes in a plane: 128 x 128 words.
const PLANE_BYTES: u16 = 0x8000;
/// Chunks read before the loop is taken to hang. Only a level past the
/// last gets near it: in a BSA entry the position then runs past the
/// entry's end, where a read returns its count even with nothing left in
/// the file, so the loop skips the same stale chunk until the 32-bit
/// position wraps around GLOBAL.BSA and real bytes are read again. That
/// can end (on a LEVL tag, or the position landing on the entry's end)
/// after hundreds of thousands of chunks, or never.
const CHUNK_LIMIT: u32 = 1 << 26;
/// EMS page mapped before a plane is read.
const PLANE_PAGE: u16 = 0x1F;

impl Machine {
    /// 0000:E87C. Loads the level file named at DS:`name`: the level is
    /// LEVEL_OVERRIDE if set, otherwise the header's starting level. New
    /// floor and wall planes are allocated on every call; a ceiling plane
    /// only if there is none yet. Afterwards the player is put at the
    /// first start position (or the return position with an override)
    /// unless FLAGS bit 9 is set.
    ///
    /// Assumes ES = DGROUP on entry, as from both callers seen. None
    /// where the heap reports an error, the original faults (a LOCK
    /// chunk of 768 bytes or more) or hangs (see CHUNK_LIMIT), the file
    /// is missing, or it reaches code not ported: the level and open
    /// hooks.
    pub fn level_load(&mut self, files: &mut Files, name: u16) -> Option<()> {
        self.mem.w8(DS, TARGET_COUNT, 0);
        let handle = self.file_open_handle(files, name)?;
        self.mem.w16(DS, HANDLE, handle);
        self.mem.w16(DS, W_C8ED, 0xFFFF);
        for i in 0..0x600 {
            self.mem.w8(DS, LIGHTS + i, 0);
        }
        let seg = self.level_alloc(PLANE_BYTES)?;
        self.mem.w16(DS, MAP1_SEG, seg);
        let seg = self.level_alloc(PLANE_BYTES)?;
        self.mem.w16(DS, FLOOR_SEG, seg);
        self.file_read(files, DS, CHUNK_LEN, 4);
        self.file_read(files, DS, CHUNK_LEN, 2);
        let len = self.mem.r16(DS, CHUNK_LEN);
        self.file_read(files, DS, HEADER, len);
        let over = self.mem.r8(DS, LEVEL_OVERRIDE);
        let level = if over == 0xFF { self.mem.r8(DS, START_LEVEL) } else { over };
        self.mem.w8(DS, LEVEL, level);
        self.level_skip(files, level as u16);
        self.level_chunks(files)?;
        if self.mem.r8(DS, LEVEL_OVERRIDE) != 0xFF && self.mem.r16(DS, LEVEL_HOOK + 2) != 0 {
            return None;
        }
        self.file_close(files);
        if self.mem.r16(DS, FLAGS) & 0x200 == 0 {
            let (x, y) = if self.mem.r8(DS, LEVEL_OVERRIDE) == 0xFF {
                (self.mem.r16(DS, START_X), self.mem.r16(DS, START_Y))
            } else {
                self.mem.w8(DS, LEVEL_OVERRIDE, 0xFF);
                (self.mem.r16(DS, RETURN_X), self.mem.r16(DS, RETURN_Y))
            };
            self.mem.w16(DS, PLAYER_X, x);
            self.mem.w16(DS, PLAYER_Y, y);
        }
        Some(())
    }

    /// 0000:533C. Allocates `bytes` bytes on the heap and clears them;
    /// returns the segment. None where the heap reports an error.
    pub fn level_alloc(&mut self, bytes: u16) -> Option<u16> {
        let (seg, off) = self.heap_alloc_bytes(bytes)?;
        for i in 0..bytes {
            self.mem.w8(seg, off.wrapping_add(i), 0);
        }
        Some(seg)
    }

    /// 0000:5C8A. With EMS on, maps 64 KB bank `page` into the frame (the
    /// buffer segment, 9030) if it exists and isn't mapped already; the
    /// bank mapped is kept at EMS_MAPPED, times 4. The original asks the
    /// EMS driver (INT 67h 4E02) to switch four 16 KB pages; here the
    /// frame's bytes are swapped with the stored banks, which reads the
    /// same as long as the frame segment doesn't change.
    pub fn ems_map(&mut self, page: u16) {
        if self.mem.r8(DS, EMS_ON) == 0 || page >= self.mem.r16(DS, EMS_PAGES) {
            return;
        }
        let old = self.mem.r16(DS, EMS_MAPPED);
        let new = page.wrapping_shl(2);
        if old == new {
            return;
        }
        self.mem.w16(DS, EMS_MAPPED, new);
        let frame = self.mem.r16(DS, BUFFER_SEG);
        let (old, page) = ((old >> 2) as usize, page as usize);
        if self.ems.len() <= old.max(page) {
            self.ems.resize(old.max(page) + 1, vec![0; 0x10000]);
        }
        for o in 0..=0xFFFFu16 {
            self.ems[old][o as usize] = self.mem.r8(frame, o);
            self.mem.w8(frame, o, self.ems[page][o as usize]);
        }
    }

    /// 0000:5C26. Allocates `size` bytes of EMS: from the first bank
    /// whose free count (a word per bank at BANK_FREE, counting down from
    /// 0xFFFF) is at least `size`, at offset 0xFFFF - free; the bank is
    /// mapped. Returns (offset, bank). None with EMS off or no room (the
    /// original calls the hook at 8F70, if set, and returns 0).
    pub fn ems_alloc(&mut self, size: u16) -> Option<(u16, u16)> {
        if self.mem.r8(DS, EMS_ON) == 0 {
            return None;
        }
        let banks = self.mem.r16(DS, EMS_PAGES);
        for i in 0..if banks == 0 { 0x10000 } else { banks as u32 } {
            let at = BANK_FREE.wrapping_add((i as u16).wrapping_mul(2));
            let free = self.mem.r16(DS, at);
            if free >= size {
                self.mem.w16(DS, at, free - size);
                let bank = i as u16 & 0xFF;
                self.ems_map(bank);
                return Some((!free, bank));
            }
        }
        None
    }

    /// Bank `page` as the program sees it: the frame when it is mapped.
    pub fn ems_bank(&self, page: u16) -> Vec<u8> {
        if self.mem.r16(DS, EMS_MAPPED) >> 2 == page && self.mem.r8(DS, EMS_ON) != 0 {
            let frame = self.mem.r16(DS, BUFFER_SEG);
            return (0..=0xFFFFu16).map(|o| self.mem.r8(frame, o)).collect();
        }
        self.ems.get(page as usize).cloned().unwrap_or_else(|| vec![0; 0x10000])
    }

    /// 0000:EBB5. Reloads the chunks of level LEVEL from the level file
    /// named at LEVEL_FILE (the planes into the planes there are), as
    /// stairs do: the file is reopened, read from 0x43 (past a header of
    /// the usual 0x3D bytes), and the levels before skipped. A name
    /// starting `#` (a generated dungeon) only calls the hook at CD32.
    /// None where the file is missing, the chunk loop fails (see
    /// `level_load`) or that hook is set.
    pub fn level_change(&mut self, files: &mut Files) -> Option<()> {
        self.mem.w8(DS, TRIGGER_COUNT, 0);
        if self.mem.r8(DS, LEVEL_FILE) != b'#' {
            let handle = self.file_open_handle(files, LEVEL_FILE)?;
            self.mem.w16(DS, HANDLE, handle);
            self.file_seek(files, 0, 0x43);
            self.level_skip(files, self.mem.r8(DS, LEVEL) as u16);
            self.level_chunks(files)?;
            self.file_close(files);
        }
        if self.mem.r16(DS, CHANGE_HOOK + 2) != 0 {
            return None;
        }
        Some(())
    }

    /// 0000:EB88. Reads a level header, then skips `n` levels, reading
    /// the next header after each.
    pub(crate) fn level_skip(&mut self, files: &mut Files, n: u16) {
        self.file_read(files, DS, CHUNK_LEN, 6);
        for _ in 0..n {
            let len = self.mem.r16(DS, LEVEL_LEN);
            self.file_seek(files, 1, len as u32);
            self.file_read(files, DS, CHUNK_LEN, 6);
        }
    }

    /// 0000:E930. The chunk loop.
    fn level_chunks(&mut self, files: &mut Files) -> Option<()> {
        for _ in 0..CHUNK_LIMIT {
            if self.file_read(files, DS, TAG, 4) == 0 {
                return Some(());
            }
            match &self.mem.r32(DS, TAG).to_le_bytes() {
                b"LEVL" => return Some(()),
                b"LOCK" => {
                    let n = self.level_read(files, DS, LOCKS);
                    // div bl: a quotient above 0xFF is a divide error.
                    let q = n / 3;
                    if q > 0xFF {
                        return None;
                    }
                    self.mem.w8(DS, LOCK_COUNT, q as u8);
                }
                b"TRIG" => {
                    let n = self.level_read(files, DS, TRIGGERS);
                    self.mem.w8(DS, TRIGGER_COUNT, (n >> 2) as u8);
                }
                b"TARG" => {
                    let n = self.level_read(files, DS, TARGETS);
                    self.mem.w8(DS, TARGET_COUNT, (n >> 1) as u8);
                }
                b"TEXT" => {
                    self.ems_map(self.mem.r8(DS, TEXT_PAGE) as u16);
                    let (seg, off) = (self.mem.r16(DS, BUFFER_SEG), self.mem.r16(DS, TEXT_OFF));
                    self.level_read(files, seg, off);
                }
                b"LITE" => {
                    let n = self.level_read(files, DS, LIGHTS);
                    self.mem.w16(DS, LIGHT_COUNT, n / 6);
                }
                b"FLOR" => {
                    let seg = self.mem.r16(DS, FLOOR_SEG);
                    let add = self.mem.r16(DS, BUFFER_SEG);
                    self.level_plane(files, seg, add)?;
                    self.level_fill(seg);
                }
                b"MAP1" => {
                    let seg = self.mem.r16(DS, MAP1_SEG);
                    self.level_plane(files, seg, 0)?;
                    self.level_fill(seg);
                }
                b"MAP2" => {
                    let f = self.mem.r16(DS, MAP_FLAGS) | 4;
                    self.mem.w16(DS, MAP_FLAGS, f);
                    if self.mem.r16(DS, MAP2_SEG) == 0 {
                        let seg = self.level_alloc(PLANE_BYTES)?;
                        self.mem.w16(DS, MAP2_SEG, seg);
                    }
                    let seg = self.mem.r16(DS, MAP2_SEG);
                    self.level_plane(files, seg, 0)?;
                    self.level_fill(seg);
                }
                b"NAME" => {
                    self.level_read(files, DS, LEVEL_NAME);
                }
                b"INFO" if self.mem.r16(DS, INFO_PTR) == 0 => {
                    self.file_read(files, DS, CHUNK_LEN, 2);
                    let len = self.mem.r16(DS, CHUNK_LEN);
                    self.file_read(files, DS, INFO, len);
                    self.mem.w16(DS, INFO_PTR, INFO);
                }
                _ => {
                    self.file_read(files, DS, CHUNK_LEN, 2);
                    let len = self.mem.r16(DS, CHUNK_LEN);
                    self.file_seek(files, 1, len as u32);
                }
            }
        }
        None
    }

    /// 0000:EAC6. Reads a chunk's length and then its bytes to
    /// `seg`:`off`; returns the length.
    fn level_read(&mut self, files: &mut Files, seg: u16, off: u16) -> u16 {
        self.file_read(files, DS, CHUNK_LEN, 2);
        let len = self.mem.r16(DS, CHUNK_LEN);
        self.file_read(files, seg, off, len);
        len
    }

    /// 0000:EAE4. Reads a compressed plane chunk to the buffer,
    /// decompresses it to the end of the plane at `seg` (the
    /// decoded size is the chunk's first word) and copies it, MAP_DEPTH
    /// rows of MAP_WIDTH words, to rows of 0x100 bytes from offset 0,
    /// adding `add` to every word. Source and destination share the
    /// segment, so a large map overwrites source words before reading
    /// them, as in the original.
    ///
    /// With RELOAD set the buffer block is freed first, yet the chunk is
    /// still read to the buffer segment: the decompressor's tables may
    /// then be allocated over the input it is reading.
    fn level_plane(&mut self, files: &mut Files, seg: u16, add: u16) -> Option<()> {
        let reload = self.mem.r8(DS, RELOAD) != 0;
        if reload {
            self.heap_free(self.mem.r16(DS, BUFFER_BLOCK + 2));
        }
        self.file_read(files, DS, CHUNK_LEN, 2);
        self.ems_map(PLANE_PAGE);
        let buf = self.mem.r16(DS, BUFFER_SEG);
        let len = self.mem.r16(DS, CHUNK_LEN);
        self.file_read(files, buf, 0, len);
        let start = PLANE_BYTES.wrapping_sub(self.mem.r16(buf, 0));
        self.mif_decompress(buf, 0, seg, start, len)?;
        let width = self.mem.r16(DS, MAP_WIDTH);
        let depth = self.mem.r16(DS, MAP_DEPTH);
        // `loop` with CX = 0 runs 65536 times.
        let count = |n: u16| if n == 0 { 0x10000u32 } else { n as u32 };
        let (mut si, mut row) = (start, 0u16);
        for _ in 0..count(depth) {
            let mut di = row;
            for _ in 0..count(width) {
                let w = self.mem.r16(seg, si).wrapping_add(add);
                self.mem.w16(seg, di, w);
                si = si.wrapping_add(2);
                di = di.wrapping_add(2);
            }
            row = row.wrapping_add(0x100);
        }
        if reload {
            let (s, o) = self.heap_alloc_bytes(self.mem.r16(DS, BUFFER_SIZE))?;
            self.mem.w16(DS, BUFFER_BLOCK, o);
            self.mem.w16(DS, BUFFER_BLOCK + 2, s);
        }
        Some(())
    }

    /// 0000:F321. Fills the plane at `seg` outside the map with the
    /// buffer segment number: the rest of each of MAP_DEPTH rows (none
    /// when the width is exactly 128, a count wrapping past 0xFFFF words
    /// when wider), then the rows below the map up to 0x4000 words.
    fn level_fill(&mut self, seg: u16) {
        let v = self.mem.r16(DS, BUFFER_SEG);
        let width = self.mem.r16(DS, MAP_WIDTH);
        let depth = self.mem.r16(DS, MAP_DEPTH);
        let rest = 0x80u16.wrapping_sub(width);
        if rest != 0 {
            let mut di = 0u16;
            for _ in 0..if depth == 0 { 0x10000 } else { depth as u32 } {
                di = di.wrapping_add(width.wrapping_mul(2));
                for _ in 0..rest {
                    self.mem.w16(seg, di, v);
                    di = di.wrapping_add(2);
                }
            }
        }
        if depth != 0x80 {
            let below = depth << 7;
            let mut di = below.wrapping_mul(2);
            for _ in 0..0x4000u16.wrapping_sub(below) {
                self.mem.w16(seg, di, v);
                di = di.wrapping_add(2);
            }
        }
    }
}
