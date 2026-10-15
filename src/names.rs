//! Random names (1194:080B), from NAMECHNK.DAT.
//!
//! The file is a run of chunks, each a word length (counting itself), a
//! count byte and that many NUL-terminated strings. A name set's chunks
//! are found by skipping the chunks before it; a name is built from one
//! random string of each chunk it uses, some chunks being skipped at
//! random. Name sets 0..7 are the races (6, 5 then six chunks each, so
//! race 1 has one fewer); 8..16, 17..21 and above use the chunks from
//! 0x2F, 0x32 and 0x36 on.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables: the name set and sex (bytes), the file handle, and
/// the chunk buffer (the length word, then the count and strings).
pub mod var {
    pub const KIND: u16 = 0x98DE;
    pub const SEX: u16 = 0x98DF;
    pub const HANDLE: u16 = 0x98E0;
    pub const CHUNK: u16 = 0x98E2;
}
use var::*;

/// "namechnk.dat", the race sets' chunk counts and the suffix race 2 gets.
const FILE_NAME: u16 = 0x3978;
const RACE_CHUNKS: u16 = 0x3985;
const SUFFIX: u16 = 0x398D;

/// Output position: segment and offset of the next character.
struct Out {
    seg: u16,
    di: u16,
    /// AL as the routine's last step leaves it: 0 after a string copied
    /// (0000:9CA0 stops on the NUL), the new position's low byte after a
    /// chunk skipped (0000:6D52, DOS seek).
    al: u8,
}

impl Machine {
    /// 1194:080B. Writes a name of set `kind` for sex `sex` (0 male) at
    /// seg:off, NUL-terminated, with the RNG seeded from `seed`; the
    /// caller's seed is restored. Returns DI after, as the original
    /// leaves it (mostly at the NUL; past it after race 2's suffix). None
    /// where the file is missing or a chunk's count is 0 (the original
    /// divides by zero).
    pub fn npc_name(&mut self, files: &mut Files, kind: u8, sex: u8, seed: u32, seg: u16, off: u16) -> Option<u16> {
        self.npc_name_al(files, kind, sex, seed, seg, off).map(|(di, _)| di)
    }

    /// `npc_name`, with AL as 1194:080B returns it (AH is left 3Eh by the
    /// file's close, DOS 3Eh, which changes no other register): 0, but
    /// for race 1 (`names_set_1`) when its last step skips a chunk, then
    /// the file position's low byte. BX is left the file's handle
    /// (HANDLE, its high word the caller's).
    pub fn npc_name_al(&mut self, files: &mut Files, kind: u8, sex: u8, seed: u32, seg: u16, off: u16) -> Option<(u16, u8)> {
        let saved = self.rng.seed;
        self.mem.w8(DS, KIND, kind);
        self.mem.w8(DS, SEX, sex);
        self.rng.seed = seed;
        self.mem.w8(seg, off, 0);
        // Where the port gives up (the names file missing, or a pick
        // where the original faults) the game's seed is put back and the
        // file closed, as the routine's end does; left as they were, every
        // later roll would follow the name's seed. The original has no
        // such exit.
        let Some(handle) = self.file_open_handle(files, FILE_NAME) else {
            self.rng.seed = saved;
            return crate::giveup::error("1194:080B: the names file missing");
        };
        self.mem.w16(DS, HANDLE, handle);
        let mut o = Out { seg, di: off, al: 0 };
        let r = self.names_by_kind(files, kind, &mut o);
        self.file_close(files);
        self.rng.seed = saved;
        r?;
        Some((o.di, o.al))
    }

    /// `npc_name_al`'s names by kind, from the open names file.
    fn names_by_kind(&mut self, files: &mut Files, kind: u8, o: &mut Out) -> Option<()> {
        match kind {
            0..=7 => {
                if kind > 0 {
                    let mut n = 0u8;
                    for i in 0..kind as u16 {
                        n = n.wrapping_add(self.mem.r8(DS, RACE_CHUNKS + i));
                    }
                    // `loop` with CX = 0 runs 65536 times.
                    self.names_skip_n(files, if n == 0 { 0x10000 } else { n as u32 });
                }
                match kind {
                    1 => self.names_set_1(files, o)?,
                    2 => {
                        self.names_race(files, o)?;
                        self.names_copy(SUFFIX, o);
                        // strcpy leaves DI past the NUL.
                    }
                    _ => self.names_race(files, o)?,
                }
            }
            8..=16 => {
                self.names_skip_n(files, 0x2F);
                self.names_pick(files, o)?;
                self.names_one_or_two(files, o)?;
            }
            17..=21 => {
                self.names_skip_n(files, 0x32);
                self.names_pick(files, o)?;
                if kind == 0x15 {
                    self.names_skip(files);
                    for _ in 0..2 {
                        self.names_pick(files, o)?;
                    }
                } else {
                    self.names_one_or_two(files, o)?;
                }
            }
            _ => {
                self.names_skip_n(files, 0x36);
                let take = kind == 0x16 && self.rng.range(0, 100)? <= 0x19;
                if take {
                    self.names_pick(files, o)?;
                    self.mem.w8(o.seg, o.di, b' ');
                    o.di = o.di.wrapping_add(1);
                } else {
                    self.names_skip(files);
                }
                for _ in 0..3 {
                    self.names_pick(files, o)?;
                }
            }
        }
        Some(())
    }

    /// 1194:0940: race names. A male name takes the first two chunks and
    /// skips the next two, a female one the reverse; then a space and
    /// two more chunks.
    fn names_race(&mut self, files: &mut Files, o: &mut Out) -> Option<()> {
        if self.mem.r8(DS, SEX) == 0 {
            self.names_pick(files, o)?;
            self.names_pick(files, o)?;
            self.names_skip(files);
            self.names_skip(files);
        } else {
            self.names_skip(files);
            self.names_skip(files);
            self.names_pick(files, o)?;
            self.names_pick(files, o)?;
        }
        self.mem.w8(o.seg, o.di, b' ');
        o.di = o.di.wrapping_add(1);
        self.names_pick(files, o)?;
        self.names_pick(files, o)
    }

    /// 1194:096D: race 1. Three chunks, then (one in four) a fourth
    /// skipped or (three in four) used, and one more for women.
    fn names_set_1(&mut self, files: &mut Files, o: &mut Out) -> Option<()> {
        for _ in 0..3 {
            self.names_pick(files, o)?;
        }
        let mut n = 0u16;
        if self.rng.range(0, 100)? > 0x4B {
            o.al = self.names_skip(files) as u8;
        } else {
            n += 1;
        }
        if self.mem.r8(DS, SEX) != 0 {
            n += 1;
        }
        for _ in 0..n {
            self.names_pick(files, o)?;
        }
        Some(())
    }

    /// The end of 1194:09A3 and 09CB: a roll of range(0, 100); above
    /// 0x4B skips a chunk and takes one, otherwise takes two.
    fn names_one_or_two(&mut self, files: &mut Files, o: &mut Out) -> Option<()> {
        let n = if self.rng.range(0, 100)? > 0x4B {
            self.names_skip(files);
            1
        } else {
            2
        };
        for _ in 0..n {
            self.names_pick(files, o)?;
        }
        Some(())
    }

    /// 1194:08DE: skips a chunk (its length counts the length word; under
    /// 2 seeks on by nearly 64 KiB).
    fn names_skip(&mut self, files: &mut Files) -> u32 {
        self.file_read(files, DS, CHUNK, 2);
        let len = self.mem.r16(DS, CHUNK).wrapping_sub(2);
        self.file_seek(files, 1, len as u32)
    }

    fn names_skip_n(&mut self, files: &mut Files, n: u32) {
        for _ in 0..n {
            self.names_skip(files);
        }
    }

    /// 1194:0901: reads a chunk over the buffer and appends one of its
    /// strings, chosen by range(0, count). Skipping to string k steps one
    /// byte, then to the next NUL, k times, so an empty string counts as
    /// part of the one after it. The output ends on the copied NUL.
    fn names_pick(&mut self, files: &mut Files, o: &mut Out) -> Option<()> {
        self.file_read(files, DS, CHUNK, 2);
        let len = self.mem.r16(DS, CHUNK).wrapping_sub(2);
        self.file_read(files, DS, CHUNK, len);
        let count = self.mem.r8(DS, CHUNK) as u16;
        let k = self.rng.range(0, count)?;
        let mut si = CHUNK + 1;
        if k != 0 {
            for _ in 0..k {
                loop {
                    si = si.wrapping_add(1);
                    if self.mem.r8(DS, si) == 0 {
                        break;
                    }
                }
            }
            si = si.wrapping_add(1);
        }
        self.names_copy(si, o);
        o.di = o.di.wrapping_sub(1);
        o.al = 0;
        Some(())
    }

    /// 0000:9CA0: strcpy from DS:si to the output, NUL included; the
    /// output position ends past the NUL.
    fn names_copy(&mut self, mut si: u16, o: &mut Out) {
        loop {
            let b = self.mem.r8(DS, si);
            self.mem.w8(o.seg, o.di, b);
            si = si.wrapping_add(1);
            o.di = o.di.wrapping_add(1);
            if b == 0 {
                break;
            }
        }
    }
}
