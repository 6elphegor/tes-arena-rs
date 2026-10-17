//! The game's file layer (segment 0000, 67C4..6F87): loose files in the
//! game directory, falling back to entries of GLOBAL.BSA.
//!
//! GLOBAL.BSA is a word entry count, the entries' data back to back, and
//! a directory of `count` 18-byte records at the end of the file: a
//! 14-byte NUL-padded name and a dword size. An entry's data starts at 2
//! plus the sizes of all entries before it. Entries named *.INF or *.TXT
//! are XOR-encrypted with an 8-byte key (DGROUP 91A4) and the position.
//!
//! The original keeps its state in DGROUP and in two code-segment bytes;
//! the port keeps the same variables in `Machine` memory and only the
//! open files' bytes and DOS file positions in `Files`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::level::var::EMS_MAPPED;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// GLOBAL.BSA handle (0xFFFF: none), directory position (dword),
    /// entry count.
    pub const BSA_HANDLE: u16 = 0xA920;
    pub const BSA_DIR: u16 = 0xA922;
    pub const BSA_COUNT: u16 = 0xA926;
    /// "global.bsa".
    pub const BSA_NAME: u16 = 0x91FA;
    /// Directory record buffer (18 bytes); +14 is the size dword.
    pub const DIR_ENTRY: u16 = 0x9176;
    pub const DIR_SIZE: u16 = 0x9184;
    /// "INF\0TXT\0": extensions of encrypted entries.
    pub const ENCRYPTED_EXTS: u16 = 0x9188;
    /// Current entry: encrypted flag (byte), position, data offset,
    /// size, bytes remaining (dwords).
    pub const ENCRYPTED: u16 = 0x9190;
    pub const POS: u16 = 0x9194;
    pub const DATA_OFFSET: u16 = 0x9198;
    pub const SIZE: u16 = 0x919C;
    pub const REMAINING: u16 = 0x91A0;
    /// 8-byte XOR key.
    pub const KEY: u16 = 0x91A4;
    /// In-memory directory handle; 0xFFFF scans the directory on disk.
    pub const DIR_CACHE: u16 = 0x9192;
    /// Name of the last opened file (far pointer) and low word of its size.
    pub const OPEN_NAME: u16 = 0xA908;
    pub const OPEN_SIZE: u16 = 0xA90C;
    /// Far pointer called before every open when its segment is set.
    pub const OPEN_HOOK: u16 = 0xA904;
    /// Far pointer called on a file error when set (0000:74E5).
    pub const ERROR_HOOK: u16 = 0xA90E;
}
use var::*;

/// Code-segment variables in segment 0000: the open file's handle and
/// whether it is a BSA entry.
const CS_HANDLE: u16 = 0x6837;
const CS_IN_BSA: u16 = 0x6839;

/// DOS's error code for an invalid handle (a closed one), which a read,
/// write, seek or close on it returns in AX with the carry set.
const INVALID_HANDLE: u16 = 6;

/// A file open on a DOS handle: its bytes, position and upper-case name
/// (writes to it are kept under the name, `file_put`).
#[derive(Clone)]
struct OpenFile {
    data: Rc<[u8]>,
    pos: u32,
    name: Vec<u8>,
}

/// What a DOS handle is: a file, one of the character devices DOS opens a
/// program with (still open), or nothing (free, or closed).
enum On<'a> {
    File(&'a mut OpenFile),
    Device(u16),
    Free,
}

/// DOS's handles for the program (the 20 of its PSP). DOS starts a
/// program with its character devices on the first five, CON on 0, 1
/// and 2 (one open file: one position), AUX on 3, PRN on 4; each is open
/// until the program closes it, and an open or create takes the lowest
/// free handle. The game closes 2 itself where it goes on after a missing
/// file with the open's error code, 2 ("file not found"), as its handle:
/// from then on its next file is opened on 2, and an operation on 2 with
/// nothing open there fails as an invalid handle (MS-DOS 4.0's
/// HANDLE.ASM: $Close frees the JFN even with CON's other references).
#[derive(Clone)]
struct Dos {
    /// Bit h set: device handle h (0..4) still open.
    devices: u8,
    /// CON's position (its size is 0: a seek from the end is the offset).
    con_pos: u32,
    files: std::collections::BTreeMap<u16, OpenFile>,
    /// GLOBAL.BSA's handle (kept open from the start, read through `bsa`).
    bsa: Option<u16>,
    /// The handle the port's file calls work on: the last opened or
    /// made, or the error code a failed open left (CS:6837 for the
    /// routines that keep it there).
    cur: u16,
}

impl Default for Dos {
    fn default() -> Self {
        Self { devices: 0x1F, con_pos: 0, files: Default::default(), bsa: None, cur: 0xFFFF }
    }
}

impl Dos {
    /// The handle the next open or create gets: the lowest free.
    fn free(&self) -> u16 {
        (0..20).find(|&h| !self.on_device(h) && !self.files.contains_key(&h) && self.bsa != Some(h)).unwrap_or(0xFFFF)
    }

    fn on_device(&self, h: u16) -> bool {
        h < 5 && self.devices & (1 << h) != 0
    }

    /// A file opened (or made) on the lowest free handle, made current.
    fn open(&mut self, data: Rc<[u8]>, name: Vec<u8>) -> u16 {
        let h = self.free();
        self.files.insert(h, OpenFile { data, pos: 0, name });
        self.cur = h;
        h
    }

    fn on(&mut self, h: u16) -> On<'_> {
        if self.on_device(h) {
            return On::Device(h);
        }
        match self.files.get_mut(&h) {
            Some(f) => On::File(f),
            None => On::Free,
        }
    }

    /// DOS read (3Fh) of up to `n` bytes on `h`: the bytes, or Err(AX)
    /// with the carry set. A count of 0 returns at once, on a device too
    /// (DISK.ASM's DOS_READ: "no bytes to read - fast return"); more from
    /// CON waits for a line typed at the keyboard ($STD_CON_STRING_INPUT),
    /// which the port can't give: it reads nothing, noted as where the
    /// original would wait.
    fn read(&mut self, h: u16, n: u16) -> Result<Vec<u8>, u16> {
        match self.on(h) {
            On::File(f) => {
                let start = (f.pos as usize).min(f.data.len());
                let end = (start + n as usize).min(f.data.len());
                f.pos = f.pos.wrapping_add((end - start) as u32);
                Ok(f.data[start..end].to_vec())
            }
            On::Device(_) => {
                if n != 0 {
                    let _: Option<()> = crate::giveup::original("DOS read from a character device: waits for the keyboard");
                }
                Ok(Vec::new())
            }
            On::Free => Err(INVALID_HANDLE),
        }
    }

    /// DOS seek (42h) on `h` by `off` from the start (0), the position
    /// (1) or the end (any other, as the game only passes 0..2): the new
    /// position, or Err(AX) with the carry set. A device's position is
    /// its file's, its size 0 ($LSEEK has no case for devices).
    fn seek(&mut self, h: u16, whence: u8, off: u32) -> Result<u32, u16> {
        let base = |pos: u32, size: u32| match whence {
            0 => 0,
            1 => pos,
            _ => size,
        };
        match self.on(h) {
            On::File(f) => {
                f.pos = base(f.pos, f.data.len() as u32).wrapping_add(off);
                Ok(f.pos)
            }
            On::Device(d) => {
                let p = if d <= 2 { &mut self.con_pos } else { return Ok(off) };
                *p = base(*p, 0).wrapping_add(off);
                Ok(*p)
            }
            On::Free => Err(INVALID_HANDLE),
        }
    }

    /// DOS close (3Eh) of `h`: false (the carry set, AX 6) where nothing
    /// is open on it.
    fn close(&mut self, h: u16) -> bool {
        if self.on_device(h) {
            self.devices &= !(1 << h);
            return true;
        }
        self.files.remove(&h).is_some()
    }
}

/// The game directory: loose files (matched case-insensitively, as DOS
/// does) and DOS's handles with the files open on them. Cloning is cheap:
/// file contents are shared.
#[derive(Clone, Default)]
pub struct Files {
    names: HashMap<Vec<u8>, PathBuf>,
    bsa: Option<(Rc<[u8]>, u32)>,
    dos: Dos,
    /// Files the game wrote (`Machine::file_write`), by upper-case name:
    /// kept here, not on disk, and found before the directories' files.
    written: HashMap<Vec<u8>, Rc<[u8]>>,
    /// Files the game deleted (`Machine::file_delete`): hidden from then
    /// on, the directories' files too (nothing is deleted on disk).
    deleted: std::collections::HashSet<Vec<u8>>,
}

impl Files {
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        let mut files = Self { names: HashMap::new(), bsa: None, dos: Dos::default(), written: HashMap::new(), deleted: Default::default() };
        files.add_dir(dir)?;
        Ok(files)
    }

    /// The game directory as `other` left it: the files it wrote and
    /// deleted. The original's writes go to disk and outlive a return to
    /// the title screen (a load from the main menu, a new game), so a
    /// fresh engine for one carries them on (a city's wilderness blocks,
    /// wild00N.rmd, written at its gate and read on the way back out of a
    /// building there).
    pub fn disk_from(&mut self, other: &Files) {
        self.written = other.written.clone();
        self.deleted = other.deleted.clone();
    }

    /// A file the game wrote, by DOS name (its last component).
    pub fn written(&self, name: &[u8]) -> Option<&[u8]> {
        self.written.get(&Self::base(name).to_ascii_uppercase()).map(|d| &d[..])
    }

    /// The upper-case names of the files whose extension is `ext` (DOS's
    /// find first/next on "*.EXT"), the directories' and the written ones,
    /// sorted (DOS gives directory order).
    pub fn with_ext(&self, ext: &[u8]) -> Vec<Vec<u8>> {
        let ext = ext.to_ascii_uppercase();
        self.with_ext_where(|e| e == &ext[..])
    }

    /// As `with_ext`, the extensions (upper case) that `fits` takes.
    pub fn with_ext_where(&self, fits: impl Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let mut v: Vec<Vec<u8>> = self
            .names
            .keys()
            .chain(self.written.keys())
            .filter(|n| !self.deleted.contains(*n))
            .filter(|n| n.contains(&b'.') && n.rsplit(|&c| c == b'.').next().is_some_and(&fits))
            .cloned()
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// The names of the files written (upper case, sorted).
    pub fn written_names(&self) -> Vec<Vec<u8>> {
        let mut v: Vec<Vec<u8>> = self.written.keys().cloned().collect();
        v.sort();
        v
    }

    /// A file's bytes by DOS name (written first, then the directories').
    pub fn data(&self, name: &[u8]) -> Option<Rc<[u8]>> {
        self.read_loose(name)
    }

    /// A file written (by DOS name) with `data`, replacing any.
    pub fn store(&mut self, name: &[u8], data: Vec<u8>) {
        let base = Self::base(name).to_ascii_uppercase();
        self.deleted.remove(&base);
        self.written.insert(base, Rc::from(data));
    }

    /// The file named `from` copied to `to` (by DOS name) as a written
    /// file. False where `from` doesn't exist.
    pub fn copy(&mut self, from: &[u8], to: &[u8]) -> bool {
        let Some(data) = self.read_loose(from) else { return false };
        let base = Self::base(to).to_ascii_uppercase();
        self.deleted.remove(&base);
        self.written.insert(base, data);
        true
    }

    /// Puts a file (by DOS name) among those the game wrote: a saved
    /// game's files, or a test's.
    pub fn put(&mut self, name: &[u8], data: &[u8]) {
        let base = Self::base(name).to_ascii_uppercase();
        self.deleted.remove(&base);
        self.written.insert(base, Rc::from(data));
    }

    /// The file (by DOS name) deleted, as DOS 41h deletes it.
    pub fn delete(&mut self, name: &[u8]) {
        let base = Self::base(name).to_ascii_uppercase();
        self.written.remove(&base);
        self.deleted.insert(base);
    }

    /// The names of the files the game deleted and hasn't made again
    /// (upper case, sorted).
    pub fn deleted_names(&self) -> Vec<Vec<u8>> {
        let mut v: Vec<Vec<u8>> = self.deleted.iter().cloned().collect();
        v.sort();
        v
    }

    /// Whether the game deleted the file (by DOS name).
    pub fn was_deleted(&self, name: &[u8]) -> bool {
        self.deleted.contains(&Self::base(name).to_ascii_uppercase())
    }

    /// Whether a loose file of that name exists (DOS find first).
    pub fn exists(&self, name: &[u8]) -> bool {
        let base = Self::base(name).to_ascii_uppercase();
        !self.deleted.contains(&base) && (self.written.contains_key(&base) || self.names.contains_key(&base))
    }

    fn base(name: &[u8]) -> &[u8] {
        name.rsplit(|&c| c == b'\\' || c == b':').next().unwrap_or(name)
    }

    /// Makes the files in `dir` available too (a save game's folder, for
    /// instance); a name found in both is taken from `dir`.
    pub fn add_dir(&mut self, dir: &Path) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            self.names.insert(e.file_name().to_string_lossy().to_uppercase().into_bytes(), e.path());
        }
        Ok(())
    }

    /// A loose file by DOS name. A path ("C:\ARENA_CD\SAVEENGN.01") is
    /// looked up by its last component: the game's directories all map
    /// to the directories given here.
    fn read_loose(&self, name: &[u8]) -> Option<Rc<[u8]>> {
        let base = Self::base(name).to_ascii_uppercase();
        if self.deleted.contains(&base) {
            return None;
        }
        if let Some(d) = self.written.get(&base) {
            return Some(d.clone());
        }
        std::fs::read(self.names.get(&base)?).ok().map(Rc::from)
    }

    /// DOS read: bytes from `pos`, fewer at the end of the file.
    fn dos_read(file: &mut (Rc<[u8]>, u32), n: u16) -> Vec<u8> {
        let start = (file.1 as usize).min(file.0.len());
        let end = (start + n as usize).min(file.0.len());
        file.1 = file.1.wrapping_add((end - start) as u32);
        file.0[start..end].to_vec()
    }
}

impl Machine {
    fn bsa_cs(&self, off: u16) -> u8 {
        self.mem.r8(0, off)
    }

    /// 0000:6F3A..6F87 (from startup). Opens GLOBAL.BSA: reads the entry
    /// count and puts the directory 18 * count bytes before the end (32
    /// bits, wrapping). False if the file is missing or shorter than 2
    /// bytes; the handle stays 0xFFFF and every lookup then fails.
    pub fn bsa_open(&mut self, files: &mut Files) -> bool {
        self.mem.w16(DS, BSA_HANDLE, 0xFFFF);
        let name = self.mem.cstr(DS, BSA_NAME).to_vec();
        let Some(bytes) = files.read_loose(&name) else { return false };
        let mut f = (bytes, 0u32);
        let head = Files::dos_read(&mut f, 2);
        self.mem.w8(DS, BSA_COUNT, head.first().copied().unwrap_or(self.mem.r8(DS, BSA_COUNT)));
        if head.len() > 1 {
            self.mem.w8(DS, BSA_COUNT + 1, head[1]);
        }
        if head.len() != 2 {
            return false;
        }
        let h = files.dos.free();
        files.dos.bsa = Some(h);
        self.mem.w16(DS, BSA_HANDLE, h);
        let count = self.mem.r16(DS, BSA_COUNT) as u32;
        let dir = (f.0.len() as u32).wrapping_sub(count * 18);
        self.mem.w32(DS, BSA_DIR, dir);
        f.1 = f.0.len() as u32;
        files.bsa = Some(f);
        true
    }

    /// 0000:692E. Looks up the NUL-terminated name at DS:`name` and, if
    /// found, makes it the current entry: in the directory on disk, or
    /// with DIR_CACHE set (not 0xFFFF), in the copy `bsa_cache_dir` read
    /// to that EMS bank (0000:6AC1; the bank is mapped for the search and
    /// the previous one mapped back). Names compare up to the first NUL
    /// (0000:69F1), with only the requested name's letters made upper
    /// case; a name of 14 or more characters runs on into the record's
    /// size bytes. The entry is encrypted when the last three characters
    /// of the requested name (read from the 4 bytes before its end, so
    /// they may precede a short name) are INF or TXT. A count of 0 scans
    /// 65536 records.
    pub fn bsa_find(&mut self, files: &mut Files, name: u16) -> bool {
        if (self.mem.r16(DS, BSA_HANDLE) as i16) < 0 {
            return false;
        }
        let count = self.mem.r16(DS, BSA_COUNT);
        let n = if count == 0 { 0x10000 } else { count as u32 };
        let mut offset = 0u32;
        let cache = self.mem.r16(DS, DIR_CACHE);
        if cache != 0xFFFF {
            let mapped = self.mem.r16(DS, EMS_MAPPED);
            self.ems_map(cache);
            let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
            let mut di = 0u16;
            let mut found = None;
            for _ in 0..n {
                if let Some(end) = self.bsa_dir_match(name, frame, di) {
                    for i in 0..18 {
                        self.mem.w8(DS, DIR_ENTRY + i, self.mem.r8(frame, di.wrapping_add(i)));
                    }
                    found = Some(end);
                    break;
                }
                offset = offset.wrapping_add(self.mem.r32(frame, di.wrapping_add(0xE)));
                di = di.wrapping_add(18);
            }
            self.ems_map(mapped >> 2);
            let Some(end) = found else { return false };
            self.bsa_select(files, offset, end);
            return true;
        }
        let Some(bsa) = files.bsa.as_mut() else { return false };
        bsa.1 = self.mem.r32(DS, BSA_DIR);
        for _ in 0..n {
            let rec = Files::dos_read(bsa, 18);
            for (i, &b) in rec.iter().enumerate() {
                self.mem.w8(DS, DIR_ENTRY + i as u16, b);
            }
            if let Some(end) = self.bsa_dir_match(name, DS, DIR_ENTRY) {
                self.bsa_select(files, offset, end);
                return true;
            }
            offset = offset.wrapping_add(self.mem.r32(DS, DIR_SIZE));
        }
        false
    }

    /// 0000:6A61. With a BSA open and no cached directory, reads the
    /// directory (count x 18 bytes, the product in 16 bits) to the start
    /// of EMS bank `page` and sets DIR_CACHE to `page`, so lookups search
    /// memory; the bank mapped before is mapped back.
    pub fn bsa_cache_dir(&mut self, files: &mut Files, page: u16) {
        if (self.mem.r16(DS, BSA_HANDLE) as i16) < 0 || self.mem.r16(DS, DIR_CACHE) != 0xFFFF {
            return;
        }
        self.mem.w16(DS, DIR_CACHE, page);
        let mapped = self.mem.r16(DS, EMS_MAPPED);
        self.ems_map(page);
        let n = self.mem.r16(DS, BSA_COUNT).wrapping_mul(18);
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        if let Some(bsa) = files.bsa.as_mut() {
            bsa.1 = self.mem.r32(DS, BSA_DIR);
            for (i, &b) in Files::dos_read(bsa, n).iter().enumerate() {
                self.mem.w8(frame, i as u16, b);
            }
        }
        self.ems_map(mapped >> 2);
    }

    /// 0000:69F1: compares the name at DS:`a` with a directory name at
    /// `seg`:`b`, making only the former's letters upper case; on a match
    /// returns the offset just past `a`'s NUL.
    fn bsa_dir_match(&self, mut a: u16, seg: u16, mut b: u16) -> Option<u16> {
        loop {
            let x = self.mem.r8(DS, a).to_ascii_uppercase();
            let y = self.mem.r8(seg, b);
            a = a.wrapping_add(1);
            b = b.wrapping_add(1);
            if x != y {
                return None;
            }
            if x == 0 {
                return Some(a);
            }
        }
    }

    /// 0000:9C30: compares DS:`a` with DS:`b` ignoring case; on a match
    /// returns the offset just past `a`'s NUL.
    fn bsa_names_match(&self, mut a: u16, mut b: u16) -> Option<u16> {
        let up = |c: u8| if c.is_ascii_lowercase() { c - 0x20 } else { c };
        loop {
            let (x, y) = (up(self.mem.r8(DS, a)), up(self.mem.r8(DS, b)));
            a = a.wrapping_add(1);
            b = b.wrapping_add(1);
            if x != y {
                return None;
            }
            if x == 0 {
                return Some(a);
            }
        }
    }

    /// 0000:6A15 and 69AE: the matched entry becomes current.
    fn bsa_select(&mut self, files: &mut Files, offset: u32, name_end: u16) {
        self.mem.w8(DS, ENCRYPTED, 0);
        let ext = name_end.wrapping_sub(4);
        for e in [ENCRYPTED_EXTS, ENCRYPTED_EXTS + 4] {
            if self.bsa_names_match(ext, e).is_some() {
                self.mem.w8(DS, ENCRYPTED, 1);
            }
        }
        self.mem.w8(0, CS_IN_BSA, 1);
        let data = offset.wrapping_add(2);
        self.mem.w32(DS, DATA_OFFSET, data);
        self.mem.w32(DS, POS, 0);
        if let Some(bsa) = files.bsa.as_mut() {
            bsa.1 = data;
        }
        let size = self.mem.r32(DS, DIR_SIZE);
        self.mem.w16(DS, OPEN_SIZE, size as u16);
        self.mem.w32(DS, REMAINING, size);
        self.mem.w32(DS, SIZE, size);
    }

    /// 0000:9DD2. A new file named at DS:`name` (DOS create, 3Ch) with the
    /// `n` bytes at `seg`:`off` written, then closed; AX 1. The port
    /// keeps written files in `Files` (later opens find them) and never
    /// fails, where the original reports a failed create through 0000:74E5.
    pub fn file_write(&mut self, files: &mut Files, name: u16, seg: u16, off: u16, n: u16) {
        let data: Vec<u8> = (0..n).map(|i| self.mem.r8(seg, off.wrapping_add(i))).collect();
        let base = Files::base(self.mem.cstr(DS, name)).to_ascii_uppercase();
        files.deleted.remove(&base);
        files.written.insert(base, Rc::from(data));
    }

    /// Whether a file named at DS:`name` exists, loose or written (0000:600C,
    /// DOS find first, as the save menu asks).
    pub fn file_exists(&mut self, files: &mut Files, name: u16) -> bool {
        files.exists(self.mem.cstr(DS, name))
    }

    /// 25FD:056F: the file named at DS:`from` copied to DS:`to` (kept in
    /// `Files` as a written file, as `file_write` does), through the EMS
    /// banks: `from` opened (0000:6B22, its handle at 126C and A836), `to`
    /// deleted (9DC2) and made (67A2, its handle at 126E and A838; both
    /// DOS's lowest free handles), then (05A5) up to FFFFh bytes read
    /// into the frame ([902E]) with each of the four banks of the table
    /// at 5F5A mapped in turn (1Fh, 17h, 18h, 12h; AX, the count read, at
    /// the entry's +2), until a read comes short, and written out again
    /// bank by bank, until a write comes short; four full banks read
    /// again. Both handles are closed (0000:6762), `from`'s first. The
    /// banks keep what the copy left there, the last bank written stays
    /// mapped. With `from` missing (or only in GLOBAL.BSA: 6B22 opens
    /// DOS's files alone) its handle is DOS's error, 2, and the copy reads
    /// what DOS has on 2: CON at first, which reads nothing; once the game
    /// has closed 2, `to` itself, made there, empty (the copy leaves it
    /// so, whatever it held before; its second close fails).
    pub fn file_copy(&mut self, files: &mut Files, from: u16, to: u16) {
        let found = files.read_loose(self.mem.cstr(DS, from));
        const TABLE: u16 = 0x5F5A;
        self.mem.w8(0, CS_IN_BSA, 0);
        self.mem.w16(DS, OPEN_NAME, from);
        self.mem.w16(DS, OPEN_NAME + 2, DS);
        let src = match found {
            Some(d) => {
                // 6B77: the size (the seek to the end) at A90C.
                self.mem.w16(DS, OPEN_SIZE, d.len() as u16);
                let name = Files::base(self.mem.cstr(DS, from)).to_ascii_uppercase();
                files.dos.open(d, name)
            }
            None => 2,
        };
        self.mem.w16(DS, 0x126C, src);
        self.file_delete(files, to);
        self.mem.w16(DS, OPEN_NAME, to);
        let dst = self.dos_create(files, to);
        self.mem.w16(DS, 0x126E, dst);
        self.mem.w16(DS, 0xA836, src);
        self.mem.w16(DS, 0xA838, dst);
        'copy: loop {
            for k in 0..4u16 {
                let bank = self.mem.r8(DS, TABLE + 4 * k) as u16;
                self.ems_map(bank);
                let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
                let n = self.dos_read(files, src, seg, off, 0xFFFF);
                self.mem.w16(DS, TABLE + 4 * k + 2, n);
                if n != 0xFFFF {
                    break;
                }
            }
            for k in 0..4u16 {
                let bank = self.mem.r8(DS, TABLE + 4 * k) as u16;
                self.ems_map(bank);
                let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
                let n = self.mem.r16(DS, TABLE + 4 * k + 2);
                if self.dos_write(files, dst, seg, off, n) != 0xFFFF {
                    break 'copy;
                }
            }
        }
        files.dos.close(src);
        files.dos.close(dst);
    }

    /// 0000:677A. Creates (or empties) the file named at DS:`name` (DOS
    /// create, 3Ch, on the lowest free handle) and makes it the open file;
    /// the original keeps the handle at CS:6837 (its error code on a
    /// failure, which the port never has). OPEN_NAME is set as for an
    /// open.
    pub fn file_create(&mut self, files: &mut Files, name: u16) {
        self.mem.w16(DS, OPEN_NAME, name);
        self.mem.w16(DS, OPEN_NAME + 2, DS);
        let h = self.dos_create(files, name);
        self.mem.w8(0, CS_IN_BSA, 0);
        self.mem.w16(0, CS_HANDLE, h);
    }

    /// DOS create (3Ch) of the file named at DS:`name`: made empty (kept
    /// as written) and opened on the lowest free handle, which it returns.
    fn dos_create(&mut self, files: &mut Files, name: u16) -> u16 {
        let base = Files::base(self.mem.cstr(DS, name)).to_ascii_uppercase();
        let empty: Rc<[u8]> = Rc::from(Vec::new());
        files.deleted.remove(&base);
        files.written.insert(base.clone(), empty.clone());
        files.dos.open(empty, base)
    }

    /// 0000:6DDC. Writes `n` bytes from `seg`:`off` to the open file at
    /// its position (DOS write, 40h: over what is there, the file growing
    /// past its end, never shortened). Returns AX, the count written (6,
    /// an invalid handle, with nothing open on the handle).
    pub fn file_put(&mut self, files: &mut Files, seg: u16, off: u16, n: u16) -> u16 {
        let h = files.dos.cur;
        self.dos_write(files, h, seg, off, n)
    }

    /// DOS write (40h) of `n` bytes from `seg`:`off` on handle `h`: AX,
    /// the count (a device takes them all), or 6 where nothing is open.
    fn dos_write(&mut self, files: &mut Files, h: u16, seg: u16, off: u16, n: u16) -> u16 {
        let bytes: Vec<u8> = (0..n).map(|i| self.mem.r8(seg, off.wrapping_add(i))).collect();
        let f = match files.dos.on(h) {
            On::File(f) => f,
            On::Device(_) => return n,
            On::Free => return INVALID_HANDLE,
        };
        let mut data = f.data.to_vec();
        let pos = f.pos as usize;
        if data.len() < pos + n as usize {
            data.resize(pos + n as usize, 0);
        }
        data[pos..pos + n as usize].copy_from_slice(&bytes);
        let rc: Rc<[u8]> = Rc::from(data);
        f.data = rc.clone();
        f.pos = f.pos.wrapping_add(n as u32);
        let name = f.name.clone();
        files.written.insert(name, rc);
        n
    }

    /// 0000:9DC2. Deletes the file named at DS:`name` (DOS 41h).
    pub fn file_delete(&mut self, files: &mut Files, name: u16) {
        let base = Files::base(self.mem.cstr(DS, name)).to_ascii_uppercase();
        files.written.remove(&base);
        files.deleted.insert(base);
    }

    /// 0000:600C with CX 0: whether a file named at DS:`name` exists (DOS
    /// find first, 4Eh; the carry clear when found). The DTA's address it
    /// returns in DX:AX is left out.
    pub fn file_find(&self, files: &Files, name: u16) -> bool {
        files.exists(self.mem.cstr(DS, name))
    }

    /// 0000:67C4. Opens the file named at DS:`name`: a loose file if one
    /// exists, otherwise the BSA entry. None where neither exists (the
    /// original reports the error through 0000:74E5) or where the open
    /// hook at DGROUP A904 is set (not ported).
    pub fn file_open(&mut self, files: &mut Files, name: u16) -> Option<()> {
        let (handle, found) = self.file_open_any(files, name)?;
        self.mem.w16(0, CS_HANDLE, handle);
        found.then_some(())
    }

    /// `file_open` for callers that go on after a failed open (0000:67C8
    /// returns with carry set after 0000:74E5): Some(false) when nothing
    /// was found, and the handle left at CS:6837 is then DOS's error code
    /// 2: what DOS has on handle 2 is read, seeked and closed instead
    /// (CON at first, which reads nothing and seeks to 0; once closed, an
    /// invalid handle, or a file opened there since; see `Dos`). None
    /// where the open or the error hook is set.
    pub fn file_open_or_device(&mut self, files: &mut Files, name: u16) -> Option<bool> {
        let (handle, found) = self.file_open_any(files, name)?;
        self.mem.w16(0, CS_HANDLE, handle);
        if !found {
            if self.mem.r32(DS, ERROR_HOOK) != 0 {
                return crate::giveup::error("0000:67C8: a file missing, the error hook set");
            }
        }
        Some(found)
    }

    /// 0000:6B26. `file_open` for callers that keep the handle themselves:
    /// returns it instead of storing it at CS:6837. For a BSA entry it is
    /// DOS's "file not found" code, 2, which the directory scan preserves.
    /// None where nothing is found (the original returns with carry set
    /// and its callers read from handle 2) or the open hook is set.
    pub fn file_open_handle(&mut self, files: &mut Files, name: u16) -> Option<u16> {
        let (handle, found) = self.file_open_any(files, name)?;
        found.then_some(handle)
    }

    /// The part of 67C4 and 6B26 they share: (handle, found).
    fn file_open_any(&mut self, files: &mut Files, name: u16) -> Option<(u16, bool)> {
        self.mem.w8(0, CS_IN_BSA, 0);
        self.mem.w16(DS, OPEN_NAME, name);
        self.mem.w16(DS, OPEN_NAME + 2, DS);
        if self.hook_set(OPEN_HOOK) {
            return crate::giveup::not_ported("0000:67C4: the open hook set to a routine not ported");
        }
        let raw = self.mem.cstr(DS, name).to_vec();
        if let Some(bytes) = files.read_loose(&raw) {
            self.mem.w16(DS, OPEN_SIZE, bytes.len() as u16);
            let h = files.dos.open(bytes, Files::base(&raw).to_ascii_uppercase());
            return Some((h, true));
        }
        // DOS's error code, 2, is the handle from here on.
        files.dos.cur = 2;
        Some((2, self.bsa_find(files, name)))
    }

    /// 0000:6BEC. Reads up to `n` bytes to DS... at `seg`:`off`; returns
    /// the original's AX. In a BSA entry the count is first capped at the
    /// bytes remaining; a capped count of 0 returns 0, any other returns
    /// `n` itself (not the bytes read), and an encrypted entry decrypts
    /// all `n` buffer bytes, read or not. The position advances by the
    /// capped count. Elsewhere it is DOS's read on the handle: the count
    /// read, or 6 with nothing open on the handle (DOS's error, in AX).
    pub fn file_read(&mut self, files: &mut Files, seg: u16, off: u16, n: u16) -> u16 {
        if self.bsa_cs(CS_IN_BSA) == 0 {
            let h = files.dos.cur;
            return self.dos_read(files, h, seg, off, n);
        }
        let remaining = self.mem.r32(DS, REMAINING);
        let capped = (n as u32).min(remaining);
        self.mem.w32(DS, REMAINING, remaining - capped);
        if capped == 0 {
            return 0;
        }
        if let Some(bsa) = files.bsa.as_mut() {
            let data = Files::dos_read(bsa, capped as u16);
            for (i, &b) in data.iter().enumerate() {
                self.mem.w8(seg, off.wrapping_add(i as u16), b);
            }
        }
        let pos = self.mem.r32(DS, POS);
        self.mem.w32(DS, POS, pos.wrapping_add(capped));
        if self.mem.r8(DS, ENCRYPTED) != 0 {
            let (mut dl, mut bx) = (pos as u8, (pos & 7) as u16);
            for i in 0..n {
                let o = off.wrapping_add(i);
                let k = dl.wrapping_add(self.mem.r8(DS, KEY.wrapping_add(bx)));
                self.mem.w8(seg, o, self.mem.r8(seg, o) ^ k);
                dl = dl.wrapping_add(1);
                bx = (bx + 1) & 7;
            }
        }
        n
    }

    /// 0000:6D32. Seeks by `off` from the start (0), the current position
    /// (1) or the end (2, or any other value in a BSA entry). Returns the
    /// new position. In a BSA entry the remaining count becomes size -
    /// position in 32 bits, so a position past the end lets later reads
    /// run on into the following entries.
    pub fn file_seek(&mut self, files: &mut Files, whence: u8, off: u32) -> u32 {
        if self.bsa_cs(CS_IN_BSA) == 0 {
            // A failed seek leaves DX the offset's low word (6D42 passed
            // it in DX) and AX the error.
            return files.dos.seek(files.dos.cur, whence, off).unwrap_or_else(|e| (off << 16) | e as u32);
        }
        let size = self.mem.r32(DS, SIZE);
        let pos = match whence {
            0 => off,
            1 => size.wrapping_sub(self.mem.r32(DS, REMAINING)).wrapping_add(off),
            _ => size.wrapping_add(off),
        };
        self.mem.w32(DS, POS, pos);
        self.mem.w32(DS, REMAINING, size.wrapping_sub(pos));
        let data = self.mem.r32(DS, DATA_OFFSET);
        if let Some(bsa) = files.bsa.as_mut() {
            bsa.1 = pos.wrapping_add(data);
        }
        pos
    }

    /// 0000:7428 with ES:DI a buffer (DI not 0xFFFD..0xFFFF, which
    /// allocate): loads the whole file named at DS:`name` to seg:off,
    /// leaves its size at OPEN_SIZE and returns the buffer (DX:AX). None
    /// where the file is missing, is 64 KiB or larger, or reads as 0
    /// bytes: the original then returns 0:0 with carry set, after
    /// 0000:74E5, which only calls the hook at ERROR_HOOK if set. A
    /// segment of 0 reads nothing, which also counts as 0 bytes.
    pub fn file_load(&mut self, files: &mut Files, name: u16, seg: u16, off: u16) -> Option<(u16, u16)> {
        self.file_open_handle(files, name)?;
        let size = self.file_seek(files, 2, 0);
        if size > 0xFFFF {
            self.file_close(files);
            return crate::giveup::error("0000:7428: a file of 64 KiB or more");
        }
        self.mem.w16(DS, OPEN_SIZE, size as u16);
        self.file_seek(files, 0, 0);
        // AX after the close is the read's result (0 if nothing was read).
        let n = if seg != 0 { self.file_read(files, seg, off, size as u16) } else { 0 };
        self.file_close(files);
        (n != 0).then_some((seg, off)).or_else(|| crate::giveup::error("0000:7428: an empty file, or no room to read it"))
    }

    /// `file_load` for callers that go on after a failed load (the carry
    /// set): Some(None) where the file is missing, reads as 0 bytes (both
    /// after 0000:74E5) or is 64 KiB or larger (closed, carry set, no
    /// 74E5). None where the open hook is set, or 74E5 would call the
    /// error hook (ERROR_HOOK, not ported).
    pub fn file_load_or_carry(&mut self, files: &mut Files, name: u16, seg: u16, off: u16) -> Option<Option<(u16, u16)>> {
        let failed = |m: &mut Machine| {
            if m.mem.r32(DS, ERROR_HOOK) != 0 {
                return crate::giveup::error("0000:74E5: a file not loaded, the error hook set");
            }
            Some(None)
        };
        let (_, found) = self.file_open_any(files, name)?;
        if !found {
            return failed(self);
        }
        let size = self.file_seek(files, 2, 0);
        if size > 0xFFFF {
            self.file_close(files);
            return Some(None);
        }
        self.mem.w16(DS, OPEN_SIZE, size as u16);
        self.file_seek(files, 0, 0);
        let n = if seg != 0 { self.file_read(files, seg, off, size as u16) } else { 0 };
        self.file_close(files);
        if n == 0 {
            return failed(self);
        }
        Some(Some((seg, off)))
    }

    /// 0000:6742. Outside a BSA entry, DOS's close of the handle, which
    /// fails (false: the carry set, AX 6), changing nothing, where nothing
    /// is open on it.
    pub fn file_close(&mut self, files: &mut Files) -> bool {
        if self.bsa_cs(CS_IN_BSA) != 0 {
            self.mem.w8(0, CS_IN_BSA, 0);
            true
        } else {
            files.dos.close(files.dos.cur)
        }
    }

    /// DOS read (3Fh) of up to `n` bytes on handle `h` to `seg`:`off`:
    /// AX, the count read, or 6 (nothing read) with nothing open on `h`.
    fn dos_read(&mut self, files: &mut Files, h: u16, seg: u16, off: u16, n: u16) -> u16 {
        match files.dos.read(h, n) {
            Ok(data) => {
                for (i, &b) in data.iter().enumerate() {
                    self.mem.w8(seg, off.wrapping_add(i as u16), b);
                }
                data.len() as u16
            }
            Err(e) => e,
        }
    }
}
