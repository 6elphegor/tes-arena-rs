//! Saved games (segment 25FD): file names, paths, and the engine save
//! (SAVEENGN.nn).
//!
//! SAVEENGN.nn is four blocks read back to back (17,983 bytes): 0xE4F
//! bytes of DGROUP from 01A4 (the player's record and the game state
//! after it), XOR-encrypted (`save_decrypt`); the creature records
//! (0x20F0 bytes, at the segment in DGROUP 1042); 0x15E0 bytes at offset
//! A000 of the buffer segment (9030); and 0x120 bytes at the far pointer
//! in DGROUP 1020.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The save slot (byte) and the file names that carry its two digits
    /// at +9 (SAVEGAME.nn, SAVEENGN.nn) or +8 (WILD001.nn).
    pub const SLOT: u16 = 0x1A6C;
    pub const SAVEGAME: u16 = 0x5E92;
    pub const SAVEENGN: u16 = 0x5E9E;
    pub const WILD: u16 = 0x5EC6;
    /// The game's directory ("C:\\ARENA_CD"), the two path buffers used
    /// in turn, and which one is next (0 the first).
    pub const DIR: u16 = 0x1AB9;
    pub const PATH_A: u16 = 0x1AEB;
    pub const PATH_B: u16 = 0x1B3B;
    pub const PATH_TOGGLE: u16 = 0x1AEA;
    /// Flags: bit 2 of ENGINE_FLAGS survives the load (it is ORed back).
    pub const ENGINE_FLAGS: u16 = 0x05C8;
    pub const ENGINE_FLAGS2: u16 = 0x05CA;
    /// The first block and its size.
    pub const STATE: u16 = 0x01A4;
    pub const STATE_BYTES: u16 = 0x0E4F;
    /// Far pointer to the last block.
    pub const EXTRA: u16 = 0x1020;
    /// "spellsg.nn" (the spellbook, 128 spells) and "spells.nn" (32
    /// more), loaded whole into segment 3276 at 0000 and 2A80.
    pub const SPELLBOOK: u16 = 0x7F0C;
    pub const SPELLS: u16 = 0x7F17;
}
use var::*;

const CREATURE_SEG: u16 = 0x1042;
const BUFFER_SEG: u16 = 0x9030;
const CREATURE_BYTES: u16 = 0x20F0;
const BUFFER_OFF: u16 = 0xA000;
const BUFFER_BYTES: u16 = 0x15E0;
const EXTRA_BYTES: u16 = 0x120;
/// EMS page mapped while the buffer block is read.
const BUFFER_PAGE: u16 = 6;

/// Spell records: 0x55 bytes each, the base cost (word) at +0x32 and the
/// name at +0x34.
pub mod spell {
    pub const SIZE: u16 = 0x55;
    pub const COST: u16 = 0x32;
    pub const NAME: u16 = 0x34;
    /// Where the two files are loaded.
    pub const SEG: u16 = 0x3276;
    pub const SECOND: u16 = 0x2A80;
}

impl Machine {
    /// 29CF:0691: loads the current slot's spell files into segment
    /// 3276 (their digits come from SLOT). A load that fails is skipped,
    /// as in the original; None only where the file error hook is set
    /// (not ported).
    pub fn spells_load(&mut self, files: &mut Files) -> Option<()> {
        let slot = self.mem.r8(DS, SLOT);
        let digits = (((slot % 10) as u16) << 8 | (slot / 10) as u16).wrapping_add(0x3030);
        self.mem.w16(DS, SPELLS + 7, digits);
        self.mem.w16(DS, SPELLBOOK + 8, digits);
        for (name, off) in [(SPELLBOOK, 0), (SPELLS, spell::SECOND)] {
            let path = self.save_path(name);
            if self.file_load(files, path, spell::SEG, off).is_none()
                && self.mem.r16(DS, crate::bsa::var::ERROR_HOOK + 2) != 0
            {
                return None;
            }
        }
        Some(())
    }

    /// 29CF:0679: the record of spell `index` (bit 7 picks the second
    /// file), as (segment, offset).
    pub fn spell_record(&self, index: u8) -> (u16, u16) {
        let base = if index & 0x80 != 0 { spell::SECOND } else { 0 };
        (spell::SEG, base.wrapping_add((index & 0x7F) as u16 * spell::SIZE))
    }

    /// 25FD:028D..029F: makes `slot` current and writes its two digits
    /// (slot / 10, slot % 10, as ASCII; a slot above 99 gives a first
    /// "digit" past '9') into the save file names.
    pub fn save_set_slot(&mut self, slot: u8) {
        self.mem.w8(DS, SLOT, slot);
        let digits = (((slot % 10) as u16) << 8 | (slot / 10) as u16).wrapping_add(0x3030);
        for name in [SAVEGAME, SAVEENGN] {
            self.mem.w16(DS, name + 9, digits);
        }
        self.mem.w16(DS, WILD + 8, digits);
    }

    /// 25FD:0844: the path of the file named at DS:`name` in the game's
    /// directory (DIR + '\\' + name), built in the next of the two path
    /// buffers; returns its offset.
    pub fn save_path(&mut self, name: u16) -> u16 {
        let toggle = self.mem.r8(DS, PATH_TOGGLE);
        let dest = if toggle != 0 { PATH_B } else { PATH_A };
        let di = self.save_append(dest, DIR);
        self.mem.w8(DS, di, b'\\');
        let di = self.save_append(di.wrapping_add(1), name);
        self.mem.w8(DS, di, 0);
        self.mem.w8(DS, PATH_TOGGLE, toggle ^ 0xFF);
        dest
    }

    /// Copies the string at DS:`si` to DS:`di` without its NUL; returns
    /// the offset after it.
    fn save_append(&mut self, mut di: u16, mut si: u16) -> u16 {
        loop {
            let c = self.mem.r8(DS, si);
            si = si.wrapping_add(1);
            if c == 0 {
                return di;
            }
            self.mem.w8(DS, di, c);
            di = di.wrapping_add(1);
        }
    }

    /// 25FD:0832: XORs the `n` bytes at seg:off in place, byte i with the
    /// low byte of (n - i) rotated right by ((n - i) & 15).
    pub fn save_decrypt(&mut self, seg: u16, off: u16, n: u16) {
        let mut si = off;
        // `loop` with CX = 0 runs 65536 times.
        let count = if n == 0 { 0x10000u32 } else { n as u32 };
        for k in 0..count {
            let cx = (count - k) as u16;
            let key = cx.rotate_right((cx & 15) as u32) as u8;
            let b = self.mem.r8(seg, si) ^ key;
            self.mem.w8(seg, si, b);
            si = si.wrapping_add(1);
        }
    }

    /// 25FD:028D..029F then 035A..03F8: loads SAVEENGN for `slot` (the
    /// teardown before it, 034B..0359, and the rebuild after it, from
    /// 03F9, are the caller's). Then clears a few engine flags and
    /// DGROUP 1294..12BB. None where the file is missing or EMS is on.
    pub fn save_load_engine(&mut self, files: &mut Files, slot: u8) -> Option<()> {
        self.save_set_slot(slot);
        let path = self.save_path(SAVEENGN);
        self.file_open(files, path)?;
        let keep = self.mem.r16(DS, ENGINE_FLAGS) & 4;
        self.file_read(files, DS, STATE, STATE_BYTES);
        self.save_decrypt(DS, STATE, STATE_BYTES);
        let f = self.mem.r16(DS, ENGINE_FLAGS) | keep;
        self.mem.w16(DS, ENGINE_FLAGS, f);
        let f = self.mem.r16(DS, ENGINE_FLAGS2) | 0x4000;
        self.mem.w16(DS, ENGINE_FLAGS2, f);
        let seg = self.mem.r16(DS, CREATURE_SEG);
        self.file_read(files, seg, 0, CREATURE_BYTES);
        self.ems_map(BUFFER_PAGE)?;
        let seg = self.mem.r16(DS, BUFFER_SEG);
        self.file_read(files, seg, BUFFER_OFF, BUFFER_BYTES);
        let (off, seg) = (self.mem.r16(DS, EXTRA), self.mem.r16(DS, EXTRA + 2));
        self.file_read(files, seg, off, EXTRA_BYTES);
        self.file_close(files);
        let f = self.mem.r16(DS, ENGINE_FLAGS) & 0xF9FF;
        self.mem.w16(DS, ENGINE_FLAGS, f);
        let b = self.mem.r8(DS, 0x0F81) & 0x7F;
        self.mem.w8(DS, 0x0F81, b);
        self.mem.w16(DS, 0x0D56, 0xFFFF);
        self.mem.w8(DS, 0x427D, 0xFF);
        self.mem.w16(DS, 0x061E, 0);
        for i in 0..0x28 {
            self.mem.w8(DS, 0x1294 + i, 0);
        }
        Some(())
    }
}
