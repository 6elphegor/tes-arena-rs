//! Saving a game (25FD:0113..01CB, 0000:A94E): the files the original
//! writes, byte for byte, as the loaders in src/save.rs read them back.
//!
//! - NAMES.DAT: the slots' names, 0x1E0 bytes from 9686 (`save_names`).
//! - SAVEGAME.nn (`save_level`): the screen (0xFA00 bytes at the far
//!   pointer A8FE), the palette (0x300 bytes at 929E, read from the VGA
//!   first), the level's state (0xDE7 bytes from BB08), and the three
//!   planes (32 KB each at D806, CCD0, D748: walls, floor, ceiling). The
//!   floor's words go out with the buffer segment (9030) taken off, as
//!   the loader adds it back. The ceiling goes out even when the level
//!   has none: its segment is then 0, and the original writes 32 KB of
//!   low memory (the interrupt table on), which a port can't reproduce.
//! - SAVEENGN.nn (`save_engine`): the game state (0xE4F bytes from 01A4)
//!   encrypted (25FD:0832, the same XOR as the load), the creatures
//!   (0x20F0 bytes at 1042), 0x15E0 bytes of EMS bank 6 at offset A000 of
//!   the frame, and 0x120 bytes at the far pointer 1020.
//!
//! The slot's files copied (25FD:0612, 056F) are `load.rs`'s, through
//! `Files`; the automap kept and loaded (23F1:0145, 0000:3503) is
//! `automap.rs`'s; the save's end (25FD:01CB on) is `save_finish` below.
//! Not ported: the menu and the name typed (25FD:004F..00D0) and the
//! CITYDATA check (242A:000E), left to the front end.

use crate::machine::Machine;
use crate::mem::DS;
use crate::save::var::{EXTRA, SAVEENGN, SAVEGAME, SLOT, STATE, STATE_BYTES};

/// DGROUP variables.
pub mod var {
    /// NAMES.DAT's name and the slots' names it holds.
    pub const NAMES: u16 = 0x5E88;
    pub const SLOT_NAMES: u16 = 0x9686;
    pub const SLOT_NAMES_BYTES: u16 = 0x01E0;
    /// The level's state block, the palette, the screen (far pointer).
    pub const LEVEL_STATE: u16 = 0xBB08;
    pub const LEVEL_STATE_BYTES: u16 = 0x0DE7;
    pub const PALETTE: u16 = 0x929E;
    pub const SCREEN: u16 = 0xA8FE;
    /// The saved position (x, z, y) and the player's, which saving a level
    /// sets from it.
    pub const SAVED_POSITION: u16 = 0xBA62;
    pub const POSITION: u16 = 0xC3D0;
    /// The last file created: its name's far pointer (offset, segment).
    pub const CREATED: u16 = 0xA908;
}
use var::*;

/// The file system, which the port's `Files` doesn't write.
pub trait SaveHooks {
    /// 0000:9DC6 (int 21h 41h): the file at the path at DS:`path`
    /// deleted, if there is one.
    fn delete(&mut self, _m: &Machine, _path: u16) {}
    /// int 21h 3Ch, 40h, 3Eh: the file at the path at DS:`path` created
    /// (or emptied) with `data`.
    fn write(&mut self, _m: &Machine, _path: u16, _data: &[u8]) {}
    /// 0000:6E3C with DI 929E: the VGA's palette (256 × 3 six-bit values)
    /// read into DS:PALETTE.
    fn palette(&mut self, _m: &mut Machine) {}
}

/// `SaveHooks` that write nothing.
pub struct NoSaveHooks;

impl SaveHooks for NoSaveHooks {}

const CREATURE_SEG: u16 = 0x1042;
const BUFFER_SEG: u16 = 0x9030;
const CREATURE_BYTES: u16 = 0x20F0;
const BUFFER_OFF: u16 = 0xA000;
const BUFFER_BYTES: u16 = 0x15E0;
const BUFFER_PAGE: u16 = 6;
const EXTRA_BYTES: u16 = 0x0120;
const PLANES: [u16; 3] = [0xD806, 0xCCD0, 0xD748];

impl Machine {
    fn save_bytes(&self, seg: u16, off: u16, n: u16) -> Vec<u8> {
        (0..n).map(|i| self.mem.r8(seg, off.wrapping_add(i))).collect()
    }

    /// 0000:677E: a file created; its name's far pointer kept at CREATED.
    fn save_create(&mut self, path: u16) {
        self.mem.w16(DS, CREATED, path);
        self.mem.w16(DS, CREATED + 2, DS);
    }

    /// 25FD:0113..01CB: a game saved to the slot at SLOT: NAMES.DAT, the
    /// slot's digits in the file names (`save_set_slot`), the slot's
    /// SAVEGAME and SAVEENGN deleted, then written (`save_level`,
    /// `save_engine`).
    pub fn save_game(&mut self, hooks: &mut dyn SaveHooks) {
        self.save_names(hooks);
        let slot = self.mem.r8(DS, SLOT);
        self.save_set_slot(slot);
        let p = self.save_path(SAVEGAME);
        hooks.delete(self, p);
        let p = self.save_path(SAVEENGN);
        hooks.delete(self, p);
        let p = self.save_path(SAVEGAME);
        self.save_level(hooks, p);
        self.save_engine(hooks);
    }

    /// 25FD:01CB..020C, after the save: the wilderness's four blocks
    /// (wild00N.rmd, the name at 5EBA) copied to the slot's (wild00N.nn
    /// at 5EC6, its digits `save_set_slot`'s; 25FD:056F, `file_copy`),
    /// the slot set back to the working slot (1A80 to SLOT: later reads,
    /// CITYDATA's among them, name the working files again), and in an
    /// interior (5E6 = 4) its automap loaded (0000:3503). 242A:000E (a
    /// check of CITYDATA) is the caller's. Returns the slot's file names
    /// made, for a front end to keep with the save.
    pub fn save_finish(&mut self, files: &mut crate::bsa::Files) -> Vec<Vec<u8>> {
        const FROM: u16 = 0x5EBA;
        const TO: u16 = 0x5EC6;
        let mut made = Vec::new();
        self.mem.w8(DS, TO + 6, b'1');
        self.mem.w8(DS, FROM + 6, b'1');
        for _ in 0..4 {
            let from = self.save_path(FROM);
            let to = self.save_path(TO);
            if files.data(self.mem.cstr(DS, from)).is_some() {
                made.push(self.mem.cstr(DS, TO).to_ascii_uppercase());
            }
            self.file_copy(files, from, to);
            for at in [FROM + 6, TO + 6] {
                let c = self.mem.r8(DS, at).wrapping_add(1);
                self.mem.w8(DS, at, c);
            }
        }
        let w = self.mem.r8(DS, 0x1A80);
        self.mem.w8(DS, SLOT, w);
        if self.mem.r8(DS, crate::trigger::var::AREA) == 4 {
            self.automap_load(files);
        }
        made
    }

    /// 25FD:0114..0121 (0000:9DD6): NAMES.DAT written with the slots'
    /// names.
    pub fn save_names(&mut self, hooks: &mut dyn SaveHooks) {
        let p = self.save_path(NAMES);
        let data = self.save_bytes(DS, SLOT_NAMES, SLOT_NAMES_BYTES);
        hooks.write(self, p, &data);
    }

    /// 0000:A94E. The level saved to the file at DS:`path`: the player's
    /// position set from the saved one first (SAVED_POSITION, which the
    /// original pushes and pops around the save, unchanged); the file
    /// deleted, then written (see the module's notes).
    pub fn save_level(&mut self, hooks: &mut dyn SaveHooks, path: u16) {
        for i in 0..3 {
            let w = self.mem.r16(DS, SAVED_POSITION + 2 * i);
            self.mem.w16(DS, POSITION + 2 * i, w);
        }
        hooks.delete(self, path);
        self.save_create(path);
        let (off, seg) = (self.mem.r16(DS, SCREEN), self.mem.r16(DS, SCREEN + 2));
        let mut data = self.save_bytes(seg, off, 0xFA00);
        hooks.palette(self);
        data.extend(self.save_bytes(DS, PALETTE, 0x300));
        data.extend(self.save_bytes(DS, LEVEL_STATE, LEVEL_STATE_BYTES));
        // 0000:A665: the planes, the floor's words less the buffer segment.
        let buffer = self.mem.r16(DS, BUFFER_SEG);
        for (i, var) in PLANES.into_iter().enumerate() {
            let seg = self.mem.r16(DS, var);
            if i == 1 {
                data.extend((0..0x4000u16).flat_map(|k| self.mem.r16(seg, 2 * k).wrapping_sub(buffer).to_le_bytes()));
            } else {
                data.extend(self.save_bytes(seg, 0, 0x8000));
            }
        }
        hooks.write(self, path, &data);
    }

    /// 25FD:015F..01C6. SAVEENGN for the slot written (see the module's
    /// notes); the game state is encrypted in place for the write and
    /// decrypted after. EMS bank 6 is mapped for its block and the bank
    /// mapped before put back (0000:5C86 returns it), when EMS is on and
    /// has that bank.
    pub fn save_engine(&mut self, hooks: &mut dyn SaveHooks) {
        let p = self.save_path(SAVEENGN);
        self.save_create(p);
        self.save_decrypt(DS, STATE, STATE_BYTES);
        let mut data = self.save_bytes(DS, STATE, STATE_BYTES);
        self.save_decrypt(DS, STATE, STATE_BYTES);
        let seg = self.mem.r16(DS, CREATURE_SEG);
        data.extend(self.save_bytes(seg, 0, CREATURE_BYTES));
        use crate::level::var::{EMS_MAPPED, EMS_ON, EMS_PAGES};
        let mapped = self.mem.r8(DS, EMS_ON) != 0 && BUFFER_PAGE < self.mem.r16(DS, EMS_PAGES);
        let old = self.mem.r16(DS, EMS_MAPPED) >> 2;
        self.ems_map(BUFFER_PAGE);
        let seg = self.mem.r16(DS, BUFFER_SEG);
        data.extend(self.save_bytes(seg, BUFFER_OFF, BUFFER_BYTES));
        if mapped {
            self.ems_map(old);
        }
        let (off, seg) = (self.mem.r16(DS, EXTRA), self.mem.r16(DS, EXTRA + 2));
        data.extend(self.save_bytes(seg, off, EXTRA_BYTES));
        hooks.write(self, p, &data);
    }
}
