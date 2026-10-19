//! The automap screen's run as the game's state sees it (0000:2CFE..2F0F,
//! the map's key M, 2542:045A): its start (the screen saved to
//! SCREENSV.$$2, the colours read, the pen's buffer, FONT_S.DAT and its
//! second font, AUTOMAP.IMG into the EMS frame and POINTER.IMG on the
//! heap, the map drawn into the page), a turn of its loop (the mouse read,
//! the pen drawn where it is), and its end (the fonts and blocks freed,
//! the screen read back). The fades (0000:5E3E), the texts' pixels
//! (5368, 9E0A) and the page's showing (946A) are the front end's.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mapdraw::var::{A889, NOTE_FLAGS, PAGE};
use crate::mapnotes::NoteHooks;
use crate::mem::DS;

/// What the screen asks of the front end.
pub trait AutomapScreenHooks: NoteHooks {
    /// 0000:6E3C with DI `di`: the VGA's colours (256 x 3 six-bit values)
    /// read to DS:`di`.
    fn palette(&mut self, _m: &mut Machine, _di: u16) {}
}

/// `AutomapScreenHooks` that leave everything as it is.
pub struct NoAutomapScreenHooks;

impl NoteHooks for NoAutomapScreenHooks {}
impl AutomapScreenHooks for NoAutomapScreenHooks {}

/// SCREENSV.$$2's name (BA3D): the screen saved under the screens.
const SCREEN_FILE: u16 = 0xBA3D;
/// The two fonts (FONT_S.DAT, 6652; 665D), AUTOMAP.IMG (6646) and
/// POINTER.IMG (6668).
const FONT_ONE: u16 = 0x6652;
const FONT_TWO: u16 = 0x665D;
const PICTURE: u16 = 0x6646;
const POINTER: u16 = 0x6668;
/// Their far pointers and segments: the pen's background's buffer (A874),
/// the fonts (A86C, A86E), the picture (A868), the pen (A88E).
const BUFFER: u16 = 0xA874;
const FONT_A: u16 = 0xA86C;
const FONT_B: u16 = 0xA86E;
const PICTURE_AT: u16 = 0xA868;
const POINTER_AT: u16 = 0xA88E;
/// Set by the loop's turns after the first (2E05): the map moved since.
const MOVED: u16 = 0x6700;

impl Machine {
    /// 0000:A69E: the page ([A8FE], FA00h bytes) written to SCREENSV.$$2
    /// in the game's directory (25FD:0844, 0000:9DD6).
    pub fn screen_save(&mut self, files: &mut Files) {
        let path = self.save_path(SCREEN_FILE);
        let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, PAGE));
        self.file_write(files, path, seg, off, 0xFA00);
    }

    /// 0000:A7A0: SCREENSV.$$2 read back into the page.
    pub fn screen_restore(&mut self, files: &mut Files) {
        let path = self.save_path(SCREEN_FILE);
        let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, PAGE));
        let _ = self.file_load(files, path, seg, off);
    }

    /// 0000:2CFE..2DB4: the automap screen started, up to its loop.
    pub fn automap_screen_begin(&mut self, files: &mut Files, hooks: &mut dyn AutomapScreenHooks) -> Option<()> {
        use crate::mapdraw::var::{EXIT, STAIRS};
        let v = self.mem.r8(DS, 0xC932);
        self.mem.w8(DS, STAIRS, v);
        let v = self.mem.r8(DS, 0xC933);
        self.mem.w8(DS, EXIT, v);
        let f = self.mem.r8(DS, NOTE_FLAGS) & !2;
        self.mem.w8(DS, NOTE_FLAGS, f);
        self.automap_write(files);
        let v = self.mem.r8(DS, 0xBB0A);
        self.mem.w8(DS, A889, v);
        self.screen_save(files);
        self.automap_view_open();
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        hooks.palette(self, 0x999E);
        for i in 0..0x300u16 {
            self.mem.w8(DS, 0x929E + i, 0);
        }
        let (seg, off) = self.heap_alloc_bytes(0x640)?;
        self.mem.w16(DS, BUFFER, off);
        self.mem.w16(DS, BUFFER + 2, seg);
        let f = self.font_load(files, FONT_ONE);
        self.mem.w16(DS, FONT_A, f);
        let f = self.font_load(files, FONT_TWO);
        self.mem.w16(DS, FONT_B, f);
        let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        let (s, o) = self.file_load(files, PICTURE, seg, off).unwrap_or((0, 0));
        self.mem.w16(DS, PICTURE_AT, o);
        self.mem.w16(DS, PICTURE_AT + 2, s);
        let seg = self.load_alloc(files, POINTER);
        self.mem.w16(DS, POINTER_AT, 0);
        self.mem.w16(DS, POINTER_AT + 2, seg);
        self.automap_screen_draw(files, hooks);
        Some(())
    }

    /// 0000:2F10: the map drawn: AUTOMAP.IMG into the page (2FFD, 582C)
    /// with the place's name over it in [A86E]'s font (not in a dungeon,
    /// 5E6 3), [A86C]'s selected again; the squares (2F20..2F47, bank 11h
    /// mapped); the notes (32FE), not where 5E6 is 4 nor in the
    /// wilderness.
    pub fn automap_screen_draw(&mut self, files: &mut Files, hooks: &mut dyn NoteHooks) {
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        let (o, s) = (self.mem.r16(DS, PICTURE_AT), self.mem.r16(DS, PICTURE_AT + 2));
        self.page_image(s, o);
        if self.mem.r8(DS, 0x05E6) != 3 {
            let f = self.mem.r16(DS, FONT_B);
            self.mem.w16(DS, crate::mapnotes::var::FONT + 2, f);
            self.mem.w8(DS, 0x9288, 0x18);
        }
        let f = self.mem.r16(DS, FONT_A);
        self.mem.w16(DS, crate::mapnotes::var::FONT + 2, f);
        self.map_bank(crate::automap::var::BANK, 0);
        let page_seg = self.mem.r16(DS, PAGE);
        let mut page: Vec<u8> = (0..0x10000u32).map(|i| self.mem.r8(page_seg, i as u16)).collect();
        let frame = self.mem.r16(DS, 0x9030);
        let seen: Vec<u8> = (0..0x10000u32).map(|i| self.mem.r8(frame, i as u16)).collect();
        self.automap_squares(&|o| seen[o as usize], &mut page);
        // Not over a heap's blocks: a game the port starts without a page
        // block has A000h there (`page_is_block`; a playtest's notes from
        // CITY:1:2 broke the heap's chain).
        if self.page_is_block() {
            for (i, &b) in page.iter().enumerate() {
                self.mem.w8(page_seg, i as u16, b);
            }
        }
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        let wild = self.mem.r16(DS, 0xC398) & 4 != 0 && self.mem.r16(DS, 0xC39A) & 0x200 != 0;
        if self.mem.r8(DS, 0x05E6) != 4 && !wild {
            self.notes_draw(files, hooks);
        }
    }

    /// 0000:2DB5..2E1D: a turn of the screen's loop, the mouse read at
    /// (`x`, `y`) (8FC8: 8F92, 8F94): after the first (`first`), the map
    /// counted moved (6700 1); the pen drawn there (36B6).
    pub fn automap_screen_turn(&mut self, x: u16, y: u16, first: bool) {
        use crate::controls::var::{MOUSE_X, MOUSE_Y};
        self.mem.w16(DS, MOUSE_X, x);
        self.mem.w16(DS, MOUSE_Y, y);
        if !first {
            self.mem.w16(DS, MOVED, 1);
        }
        self.automap_pen();
    }

    /// 0000:36B6: the pen (POINTER.IMG, [A88E]) at the mouse, its bottom
    /// left there: the background under it before (A878..A87E, kept in
    /// [A874] by 8F2A) put back first where one is kept (A888 bit 2,
    /// 3698), its box pulled inside the clip (A918, A91A), the pen drawn
    /// (5A6A) and A888 bit 2 set.
    fn automap_pen(&mut self) {
        use crate::controls::var::{MOUSE_X, MOUSE_Y};
        if self.mem.r8(DS, NOTE_FLAGS) & 2 != 0 {
            let f = self.mem.r8(DS, NOTE_FLAGS) & !2;
            self.mem.w8(DS, NOTE_FLAGS, f);
        }
        let (pseg, poff) = (self.mem.r16(DS, POINTER_AT + 2), self.mem.r16(DS, POINTER_AT));
        let ax = self.mem.r16(DS, MOUSE_X);
        let cx = self.mem.r16(pseg, poff.wrapping_add(4));
        let dx = self.mem.r16(pseg, poff.wrapping_add(6));
        let bx = self.mem.r16(DS, MOUSE_Y).wrapping_sub(dx);
        let mut x = if (ax as i16) < 0 { 0 } else { ax };
        let mut y = if (bx as i16) < 0 { 0 } else { bx };
        let over = x.wrapping_add(cx).wrapping_sub(1).wrapping_sub(self.mem.r16(DS, 0xA918));
        if (over as i16) > 0 {
            x = x.wrapping_sub(over);
        }
        let over = y.wrapping_add(dx).wrapping_sub(self.mem.r16(DS, 0xA91A));
        if (over as i16) > 0 {
            y = y.wrapping_sub(over);
        }
        self.mem.w16(DS, 0xA878, x);
        self.mem.w16(DS, 0xA87A, y);
        self.mem.w16(DS, 0xA87C, cx);
        self.mem.w16(DS, 0xA87E, dx);
        self.mem.w16(pseg, poff, ax);
        self.mem.w16(pseg, poff.wrapping_add(2), bx);
        let f = self.mem.r8(DS, NOTE_FLAGS) | 2;
        self.mem.w8(DS, NOTE_FLAGS, f);
    }

    /// 0000:2EAA..2F0F: the screen left: the fonts dropped (930E), the
    /// pen's buffer and POINTER.IMG's block freed, the game's font [102C]
    /// selected (9AEA), 929E cleared, the screen read back (A7A0), the
    /// mouse's motion forgotten (8F96, 8F98) and BB0A as it was (A889).
    pub fn automap_screen_end(&mut self, files: &mut Files) {
        let f = self.mem.r16(DS, FONT_A);
        self.font_free(f);
        let f = self.mem.r16(DS, FONT_B);
        self.font_free(f);
        let s = self.mem.r16(DS, BUFFER + 2);
        self.heap_free(s);
        let s = self.mem.r16(DS, POINTER_AT + 2);
        self.heap_free(s);
        let f = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, crate::mapnotes::var::FONT + 2, f);
        for i in 0..0x300u16 {
            self.mem.w8(DS, 0x929E + i, 0);
        }
        self.screen_restore(files);
        self.mem.w16(DS, 0x8F96, 0);
        self.mem.w16(DS, 0x8F98, 0);
        let v = self.mem.r8(DS, A889);
        self.mem.w8(DS, 0xBB0A, v);
    }
}
