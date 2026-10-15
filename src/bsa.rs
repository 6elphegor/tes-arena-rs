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

const BSA_DOS_HANDLE: u16 = 5;
const LOOSE_DOS_HANDLE: u16 = 6;

/// The game directory: loose files (matched case-insensitively, as DOS
/// does) and the DOS file positions of open files. Cloning is cheap: file
/// contents are shared.
#[derive(Clone, Default)]
pub struct Files {
    names: HashMap<Vec<u8>, PathBuf>,
    bsa: Option<(Rc<[u8]>, u32)>,
    loose: Option<(Rc<[u8]>, u32)>,
    /// Files the game wrote (`Machine::file_write`), by upper-case name:
    /// kept here, not on disk, and found before the directories' files.
    written: HashMap<Vec<u8>, Rc<[u8]>>,
}

impl Files {
    pub fn new(dir: &Path) -> std::io::Result<Self> {
        let mut files = Self { names: HashMap::new(), bsa: None, loose: None, written: HashMap::new() };
        files.add_dir(dir)?;
        Ok(files)
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
        let mut v: Vec<Vec<u8>> = self
            .names
            .keys()
            .chain(self.written.keys())
            .filter(|n| n.rsplit(|&c| c == b'.').next() == Some(&ext[..]) && n.contains(&b'.'))
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
        self.written.insert(Self::base(name).to_ascii_uppercase(), Rc::from(data));
    }

    /// The file named `from` copied to `to` (by DOS name) as a written
    /// file. False where `from` doesn't exist.
    pub fn copy(&mut self, from: &[u8], to: &[u8]) -> bool {
        let Some(data) = self.read_loose(from) else { return false };
        self.written.insert(Self::base(to).to_ascii_uppercase(), data);
        true
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
        self.mem.w16(DS, BSA_HANDLE, BSA_DOS_HANDLE);
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
        files.written.insert(base, Rc::from(data));
    }

    /// Whether a file named at DS:`name` exists, loose or written (0000:600C,
    /// DOS find first, as the save menu asks).
    pub fn file_exists(&mut self, files: &mut Files, name: u16) -> bool {
        files.read_loose(self.mem.cstr(DS, name)).is_some()
    }

    /// 25FD:056F: the file named at DS:`from` copied to DS:`to` (kept in
    /// `Files` as a written file, as `file_write` does). Nothing is
    /// copied when `from` is missing.
    pub fn file_copy(&mut self, files: &mut Files, from: u16, to: u16) {
        if let Some(data) = files.read_loose(self.mem.cstr(DS, from)) {
            let base = Files::base(self.mem.cstr(DS, to)).to_ascii_uppercase();
            files.written.insert(base, data);
        }
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
    /// 2, a character device, which reads nothing and seeks to 0 (as
    /// `file_read` and `file_seek` do with no file open). None where the
    /// open or the error hook is set.
    pub fn file_open_or_device(&mut self, files: &mut Files, name: u16) -> Option<bool> {
        let (handle, found) = self.file_open_any(files, name)?;
        self.mem.w16(0, CS_HANDLE, handle);
        if !found {
            files.loose = None;
            if self.mem.r32(DS, ERROR_HOOK) != 0 {
                return None;
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
            return None;
        }
        let raw = self.mem.cstr(DS, name).to_vec();
        if let Some(bytes) = files.read_loose(&raw) {
            self.mem.w16(DS, OPEN_SIZE, bytes.len() as u16);
            files.loose = Some((bytes, 0));
            return Some((LOOSE_DOS_HANDLE, true));
        }
        Some((2, self.bsa_find(files, name)))
    }

    /// 0000:6BEC. Reads up to `n` bytes to DS... at `seg`:`off`; returns
    /// the original's AX. In a BSA entry the count is first capped at the
    /// bytes remaining; a capped count of 0 returns 0, any other returns
    /// `n` itself (not the bytes read), and an encrypted entry decrypts
    /// all `n` buffer bytes, read or not. The position advances by the
    /// capped count.
    pub fn file_read(&mut self, files: &mut Files, seg: u16, off: u16, n: u16) -> u16 {
        if self.bsa_cs(CS_IN_BSA) == 0 {
            let Some(f) = files.loose.as_mut() else { return 0 };
            let data = Files::dos_read(f, n);
            for (i, &b) in data.iter().enumerate() {
                self.mem.w8(seg, off.wrapping_add(i as u16), b);
            }
            return data.len() as u16;
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
            let Some(f) = files.loose.as_mut() else { return 0 };
            let base = match whence {
                0 => 0,
                1 => f.1,
                _ => f.0.len() as u32,
            };
            f.1 = base.wrapping_add(off);
            return f.1;
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
            return None;
        }
        self.mem.w16(DS, OPEN_SIZE, size as u16);
        self.file_seek(files, 0, 0);
        // AX after the close is the read's result (0 if nothing was read).
        let n = if seg != 0 { self.file_read(files, seg, off, size as u16) } else { 0 };
        self.file_close(files);
        (n != 0).then_some((seg, off))
    }

    /// 0000:6742.
    pub fn file_close(&mut self, files: &mut Files) {
        if self.bsa_cs(CS_IN_BSA) != 0 {
            self.mem.w8(0, CS_IN_BSA, 0);
        } else {
            files.loose = None;
        }
    }
}
