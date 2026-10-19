//! The window library (segment 3159) as the game's state sees it: a
//! window run over a picture from a .MNU file, its regions (each a
//! rectangle and a bit mask of the pixels that are its own) and its
//! buttons (rectangles), each with a handler.
//!
//! A window's record (26h bytes, copied to the run's stack by 3159:000C
//! and 0494): +0 the .MNU's name (far), +4 the regions' records (far,
//! 62h bytes each: x, y, width, height words, the key at +59h, the
//! mask's far pointer at +5Ah, the handler at +5Eh), +0Ch the buttons'
//! (far, 64h bytes each), +10h the regions' count, +14h the buttons',
//! +16h a routine run once the window is up, +1Ah one run every pass of
//! its loop, +1Eh one for a click outside every region and button, +22h
//! one run as the window ends.
//!
//! - Its start (3159:0494, `window_open`): the counts and lists to
//!   B7CC..B7D7, the colours read to a block from the heap's top (A842,
//!   0000:6E3C), unless B7E4 the screen faded to black, nothing under
//!   the cursor (B7C6, B7C8 FFFFh); the .MNU read into the buffer at
//!   B7E6 (or a block from the heap's top of FDE8h bytes, freed after),
//!   cleared first: its 12h bytes' header, then from its start the
//!   header and the picture (+0Ah its size), drawn (0000:5828), its
//!   colours after it into A842's block where its flags (+8) have bit
//!   100h; B7E6 cleared; the regions' masks read into one heap block
//!   (51E2, B7EA), each region's +5Ah pointing into it; the colours'
//!   block freed after the fade in, B7C4 bit 0 and 9207 cleared.
//! - Each pass (3159:000C's loop) finds what is under the cursor
//!   (3159:019A, `window_regions`), runs a click's handler (00F3), the
//!   pass's routine, and stops once B7C4 bit 0 is set (3159:0006,
//!   `window_close`): its last routine, the masks' block freed (046E,
//!   `window_masks_free`), the keys flushed.
//! - 3159:0E8A and 0EB6 (`window_save`, `window_restore`) keep B7DC..B7E3
//!   and B7C4, B7C5 in a record and put them back: a window run from a
//!   handler of another's.
//!
//! The window's picture is drawn into the page where the page is the
//! heap's block (`page_is_block`); the fades and the cursor are the front
//! end's.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The loop's flags (bit 0 leave, bit 2 no wait for the buttons'
    /// release), and a byte kept with them.
    pub const FLAGS: u16 = 0xB7C4;
    /// The region and the button under the cursor (FFFFh none).
    pub const REGION: u16 = 0xB7C6;
    pub const BUTTON: u16 = 0xB7C8;
    /// The buttons' and the regions' counts, and their lists (far).
    pub const BUTTONS: u16 = 0xB7CC;
    pub const REGIONS: u16 = 0xB7CE;
    pub const BUTTON_LIST: u16 = 0xB7D0;
    pub const REGION_LIST: u16 = 0xB7D4;
    /// What 3159:0E8A keeps and 0EB6 puts back.
    pub const KEPT: u16 = 0xB7DC;
    /// No fades (nonzero).
    pub const NO_FADE: u16 = 0xB7E4;
    /// The .MNU's buffer (far; 0 a block from the heap).
    pub const BUFFER: u16 = 0xB7E6;
    /// The regions' masks' block (far).
    pub const MASKS: u16 = 0xB7EA;
    /// The colours' block (far).
    pub const COLOURS: u16 = 0xA842;
    /// A window is opening.
    pub const OPENING: u16 = 0x9207;
}
use var::*;

/// A region's record's size, and a button's.
const REGION_SIZE: u16 = 0x62;
const BUTTON_SIZE: u16 = 0x64;

impl Machine {
    /// 3159:0494 up to the routine run once it is up (+16h, the
    /// caller's), and after it: the window whose record is at DS:`rec`
    /// opened (see the module's notes). `dac` the colours on the screen
    /// (6-bit) that 0000:6E3C reads into the colours' block (None: left
    /// as allocated). None where a file can't be read or the heap has no
    /// room.
    pub fn window_open(&mut self, files: &mut Files, rec: u16, dac: Option<&[[u8; 3]; 256]>) -> Option<()> {
        let r = |m: &Machine, o: u16| m.mem.r16(DS, rec.wrapping_add(o));
        let v = r(self, 0x14);
        self.mem.w16(DS, BUTTONS, v);
        let v = self.mem.r32(DS, rec.wrapping_add(0xC));
        self.mem.w32(DS, BUTTON_LIST, v);
        let v = r(self, 0x10);
        self.mem.w16(DS, REGIONS, v);
        let v = self.mem.r32(DS, rec.wrapping_add(4));
        self.mem.w32(DS, REGION_LIST, v);
        self.mem.w8(DS, OPENING, 1);
        self.key_flush();
        for i in 0..0x300u16 {
            self.mem.w8(DS, 0x929E + i, 0);
        }
        // 0000:6E3C with DI FFFEh: 300h bytes from the heap's top.
        let colours = match self.heap_alloc_high(0x300)? {
            0 => return crate::giveup::not_ported("0000:6E3C: no room for the colours: they would go to 0:0"),
            s => s,
        };
        for (i, c) in dac.iter().flat_map(|d| d.iter()).enumerate() {
            for (k, &v) in c.iter().enumerate() {
                self.mem.w8(colours, (i * 3 + k) as u16, v);
            }
        }
        self.mem.w32(DS, COLOURS, (colours as u32) << 16);
        self.mem.w16(DS, BUTTON, 0xFFFF);
        self.mem.w16(DS, REGION, 0xFFFF);
        self.mem.w32(DS, MASKS, 0);
        let given = self.mem.r16(DS, BUFFER.wrapping_add(2)) != 0;
        let (bseg, boff) = if given {
            (self.mem.r16(DS, BUFFER.wrapping_add(2)), self.mem.r16(DS, BUFFER))
        } else {
            match self.heap_alloc_high(0xFDE8)? {
                0 => return crate::giveup::not_ported("0000:9CD2: no room for the window's picture"),
                s => (s, 0),
            }
        };
        for i in 0..0xFDE8u16 {
            self.mem.w8(bseg, boff.wrapping_add(i), 0);
        }
        let (name_off, name_seg) = (r(self, 0), r(self, 2));
        if name_seg != DS {
            return crate::giveup::not_ported("3159:054F: a window's file named outside DGROUP");
        }
        self.file_open_handle(files, name_off)?;
        self.file_read(files, bseg, boff, 0x12);
        self.file_seek(files, 0, 0);
        let n = self.mem.r16(bseg, boff.wrapping_add(0xA)).wrapping_add(0xC);
        self.file_read(files, bseg, boff, n);
        // 057E: the picture drawn into the page (5828), where the page is
        // the heap's block.
        if self.page_is_block() {
            self.page_image(bseg, boff);
        }
        if self.mem.r16(bseg, boff.wrapping_add(8)) & 0x100 != 0 {
            self.file_read(files, colours, 0, 0x300);
        }
        if !given {
            self.heap_free(bseg);
        }
        self.mem.w32(DS, BUFFER, 0);
        let regions = r(self, 0x10);
        if regions.wrapping_add(r(self, 0x14)) != 0 && regions != 0 {
            let (lseg, loff) = (self.mem.r16(DS, REGION_LIST.wrapping_add(2)), self.mem.r16(DS, REGION_LIST));
            let mask_size = |m: &Machine, at: u16| ((m.mem.r16(lseg, at.wrapping_add(4)).wrapping_add(7)) >> 3).wrapping_mul(m.mem.r16(lseg, at.wrapping_add(6)));
            let mut size = 0u16;
            for i in 0..regions {
                size = size.wrapping_add(mask_size(self, loff.wrapping_add(i.wrapping_mul(REGION_SIZE))));
            }
            let (mseg, moff) = self.heap_alloc_bytes(size)?;
            self.mem.w16(DS, MASKS, moff);
            self.mem.w16(DS, MASKS.wrapping_add(2), mseg);
            let mut di = 0u16;
            for i in 0..regions {
                let at = loff.wrapping_add(i.wrapping_mul(REGION_SIZE));
                self.mem.w16(lseg, at.wrapping_add(0x5A), di);
                self.mem.w16(lseg, at.wrapping_add(0x5C), mseg);
                di = di.wrapping_add(mask_size(self, at));
            }
            self.file_read(files, mseg, moff, di);
        }
        self.file_close(files);
        Some(())
    }

    /// 3159:0682..0694, after the window's routine and fade in: the
    /// colours' block freed, B7C4 bit 0 and 9207 cleared.
    pub fn window_opened(&mut self) {
        let seg = self.mem.r16(DS, COLOURS.wrapping_add(2));
        self.heap_free(seg);
        let f = self.mem.r8(DS, FLAGS) & 0xFE;
        self.mem.w8(DS, FLAGS, f);
        self.mem.w8(DS, OPENING, 0);
    }

    /// 3159:019A: the region and the button under the cursor (8F92,
    /// 8F94), FFFFh for none: the last region whose rectangle holds it
    /// (signed, its right and bottom edges in) and whose mask has its
    /// pixel's bit, the last button whose rectangle does.
    pub fn window_regions(&mut self) {
        self.mem.w16(DS, REGION, 0xFFFF);
        self.mem.w16(DS, BUTTON, 0xFFFF);
        let (x, y) = (self.mem.r16(DS, 0x8F92), self.mem.r16(DS, 0x8F94));
        let inside = |m: &Machine, seg: u16, at: u16| {
            let r = |o: u16| m.mem.r16(seg, at.wrapping_add(o)) as i16;
            let (x, y) = (x as i16, y as i16);
            x >= r(0) && x <= r(0).wrapping_add(r(4)) && y >= r(2) && y <= r(2).wrapping_add(r(6))
        };
        let (lseg, loff) = (self.mem.r16(DS, REGION_LIST.wrapping_add(2)), self.mem.r16(DS, REGION_LIST));
        for bx in 0..self.mem.r16(DS, REGIONS) {
            let at = loff.wrapping_add(bx.wrapping_mul(REGION_SIZE));
            if !inside(self, lseg, at) {
                continue;
            }
            let r = |o: u16| self.mem.r16(lseg, at.wrapping_add(o));
            let dx = x.wrapping_sub(r(0));
            // div cl: AH the remainder of AX / 8.
            let bit = 0x80u16 >> (dx % 8);
            let row = ((r(4).wrapping_add(7)) >> 3).wrapping_mul(y.wrapping_sub(r(2)));
            let si = r(0x5A).wrapping_add(row).wrapping_add(dx >> 3);
            if self.mem.r8(r(0x5C), si) as u16 & bit != 0 {
                self.mem.w16(DS, REGION, bx);
            }
        }
        let n = self.mem.r16(DS, BUTTONS);
        if n == 0 {
            return;
        }
        self.mem.w16(DS, BUTTON, 0xFFFF);
        let (lseg, loff) = (self.mem.r16(DS, BUTTON_LIST.wrapping_add(2)), self.mem.r16(DS, BUTTON_LIST));
        for bx in 0..n {
            if inside(self, lseg, loff.wrapping_add(bx.wrapping_mul(BUTTON_SIZE))) {
                self.mem.w16(DS, BUTTON, bx);
            }
        }
    }

    /// 3159:0006: the window's loop to end (B7C4 bit 0).
    pub fn window_close(&mut self) {
        let f = self.mem.r8(DS, FLAGS) | 1;
        self.mem.w8(DS, FLAGS, f);
    }

    /// 3159:046E: the masks' block freed (its segment, where set: 3159:0483), B7C4
    /// bit 0 and the block's pointer cleared.
    pub fn window_masks_free(&mut self) {
        let seg = self.mem.r16(DS, MASKS.wrapping_add(2));
        if seg != 0 {
            self.heap_free(seg);
        }
        let f = self.mem.r8(DS, FLAGS) & 0xFE;
        self.mem.w8(DS, FLAGS, f);
        self.mem.w32(DS, MASKS, 0);
    }

    /// 3159:00D0..00EA: the loop's end, B7C4 bit 0 found set: the
    /// window's last routine (+22h, the caller's), the masks' block freed
    /// (`window_masks_free`), the keys flushed.
    pub fn window_ended(&mut self) {
        self.window_masks_free();
        self.key_flush();
    }

    /// 3159:0E8A: the masks' block freed (`window_masks_free`), B7DC..B7E3
    /// copied to DS:`at`, B7C4 and B7C5 after them.
    pub fn window_save(&mut self, at: u16) {
        self.window_masks_free();
        for i in 0..8u16 {
            let b = self.mem.r8(DS, KEPT + i);
            self.mem.w8(DS, at.wrapping_add(i), b);
        }
        let (a, b) = (self.mem.r8(DS, FLAGS), self.mem.r8(DS, FLAGS + 1));
        self.mem.w8(DS, at.wrapping_add(8), a);
        self.mem.w8(DS, at.wrapping_add(9), b);
    }

    /// 3159:0EB6: what `window_save` kept at DS:`at` put back.
    pub fn window_restore(&mut self, at: u16) {
        for i in 0..8u16 {
            let b = self.mem.r8(DS, at.wrapping_add(i));
            self.mem.w8(DS, KEPT + i, b);
        }
        let (a, b) = (self.mem.r8(DS, at.wrapping_add(8)), self.mem.r8(DS, at.wrapping_add(9)));
        self.mem.w8(DS, FLAGS + 1, b);
        self.mem.w8(DS, FLAGS, a);
    }

    /// 1194:4D74: EMS bank `bx` mapped (0000:5C86), the window's buffer
    /// (B7E6) the frame's start, 8FAA..8FE9 cleared.
    pub fn screen_bank(&mut self, bx: u16) {
        self.ems_map(bx);
        let frame = self.mem.r16(DS, 0x9030);
        self.mem.w32(DS, BUFFER, (frame as u32) << 16);
        for i in 0..0x40u16 {
            self.mem.w8(DS, 0x8FAA + i, 0);
        }
    }
}
