//! The automap's notes (segment 0000, 31FE..347F, and 23F1:000E/00A3).
//!
//! Each location's record in AUTOMAP.nn (`automap`) begins with 40h notes
//! of 40h bytes after its 4-byte key: the note's place (two words: map
//! square x, y; or, x with bit 15 set, the screen's x and y), then 3Ch
//! bytes of text. A note whose x word is 0 is free.
//!
//! A click on the automap screen (0000:31FE, `note_click`) inside the map
//! (346B) finds the note under it (33BF) or starts a new one there; the
//! player types its text (9EE9, the front end's), and it is written back
//! in place (an emptied note freed) or added in the first free note
//! unless the same note is already there (23F1:00A3 with 000E). Drawing
//! the screen (0000:32FE, `notes_draw`) shows each note in the window, its
//! text at the screen's place (3763) or, for a screen note, where it was
//! clicked.
//!
//! The screen's drawing (the region put back by 3698, the map redrawn by
//! 2F10, the text drawn by 9E0A) and the typing are `NoteHooks`.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The click on the screen (pixels).
    pub const CLICK_X: u16 = 0x8F92;
    pub const CLICK_Y: u16 = 0x8F94;
    /// The click's map square (3745).
    pub const CLICK_COL: u16 = 0xA884;
    pub const CLICK_ROW: u16 = 0xA886;
    /// The note read or being made: its place (two words).
    pub const NOTE_X: u16 = 0xA88A;
    pub const NOTE_Y: u16 = 0xA88C;
    /// The note found: its position in the file (dword).
    pub const NOTE_POS: u16 = 0xA892;
    /// Bit 0: a found note being edited; bit 1: the screen to put back;
    /// bit 2: the note being edited met while drawing.
    pub const NOTE_FLAGS: u16 = 0xA888;
    /// The note to add, and the text read or typed (3Ch bytes), the text
    /// compared with it, and whether the place matched (23F1:000E).
    pub const NEW_X: u16 = 0xADB6;
    pub const NEW_Y: u16 = 0xADB8;
    pub const TEXT: u16 = 0x1044;
    pub const OTHER_TEXT: u16 = 0x1144;
    pub const SAME_PLACE: u16 = 0xA864;
    /// The text kept while the map is redrawn.
    pub const KEPT_TEXT: u16 = 0x929E;
    /// The current font's far pointer (its widths from +0).
    pub const FONT: u16 = 0xB034;
    /// The message colour.
    pub const COLOUR: u16 = 0x9288;
}
use var::*;

/// What the notes leave to the front end.
pub trait NoteHooks {
    /// 0000:3698's drawing (59A4): the screen region saved at A874.. put
    /// back.
    fn restore(&mut self, _m: &mut Machine) {}
    /// 0000:2F10: the map redrawn (around editing a note).
    fn redraw(&mut self, _m: &mut Machine) {}
    /// 0000:9EE9: the text at TEXT edited in place at the screen's (x, y),
    /// the cursor `cursor` pixels in, at most `max` characters, colour 6
    /// (COLOUR).
    fn type_text(&mut self, _m: &mut Machine, _x: u16, _y: u16, _cursor: u16, _max: u16) {}
    /// 0000:9E0A: the text at DS:`si` drawn at the screen's (x, y).
    fn draw_text(&mut self, _m: &mut Machine, _x: u16, _y: u16, _si: u16) {}
}

/// `NoteHooks` that do nothing.
pub struct NoNoteHooks;

impl NoteHooks for NoNoteHooks {}

impl Machine {
    /// 0000:3745. A screen point to the map's square: the map is drawn
    /// mirrored, 3 pixels a square from x CDh and y 28h, from the window's
    /// corner (MAP_LEFT, MAP_TOP); unsigned divisions.
    pub fn note_to_map(&self, x: u16, y: u16) -> (u16, u16) {
        use crate::automap::var::{MAP_LEFT, MAP_TOP};
        let col = 0xCDu16.wrapping_sub(x) / 3;
        let row = y.wrapping_sub(0x28) / 3;
        (col.wrapping_add(self.mem.r16(DS, MAP_LEFT)), row.wrapping_add(self.mem.r16(DS, MAP_TOP)))
    }

    /// 0000:3763. A map square to its screen point (the 16-bit products).
    pub fn note_to_screen(&self, col: u16, row: u16) -> (u16, u16) {
        use crate::automap::var::{MAP_LEFT, MAP_TOP};
        let c = col.wrapping_sub(self.mem.r16(DS, MAP_LEFT)).wrapping_mul(3);
        let r = row.wrapping_sub(self.mem.r16(DS, MAP_TOP)).wrapping_mul(3);
        (0xCDu16.wrapping_sub(c), r.wrapping_add(0x28))
    }

    /// 0000:66F6. The width of the text at DS:`si` in the current font
    /// (FONT): each character's width byte (signed) at +(c - 20h), a
    /// space its +1 byte plus 1; characters below 20h or from 80h count 0.
    pub fn text_width(&self, si: u16) -> u16 {
        let (off, seg) = (self.mem.r16(DS, FONT), self.mem.r16(DS, FONT + 2));
        let mut dx = 0u16;
        let mut si = si;
        loop {
            let c = self.mem.r8(DS, si);
            if c == 0 {
                return dx;
            }
            if c == 0x20 {
                dx = dx.wrapping_add(self.mem.r8(seg, off.wrapping_add(1)) as i8 as u16).wrapping_add(1);
            } else if c & 0x80 == 0 && c >= 0x20 {
                dx = dx.wrapping_add(self.mem.r8(seg, off.wrapping_add((c - 0x20) as u16)) as i8 as u16);
            }
            si = si.wrapping_add(1);
        }
    }

    /// 0000:346B. Whether the click is inside the map (x 19h..117h, y
    /// 23h..B7h).
    pub fn note_click_in_map(&self) -> bool {
        let (x, y) = (self.mem.r16(DS, CLICK_X), self.mem.r16(DS, CLICK_Y));
        (0x19..0x118).contains(&x) && (0x23..0xB8).contains(&y)
    }

    /// The notes' file opened (25FD:0844's path of automap.nn, 0000:67C8)
    /// and placed at the record's notes (POSITION + 4).
    fn notes_open(&mut self, files: &mut Files) {
        let path = self.save_path(crate::automap::var::NAME);
        let _ = self.file_open_or_device(files, path);
        let at = self.mem.r32(DS, crate::automap::var::POSITION).wrapping_add(4);
        self.file_seek(files, 0, at);
    }

    /// 0000:343C. Whether the note at (x, y) with its text at DS:`si`
    /// is under the click's square: x (bit 15 dropped) at least the
    /// click's column (signed), the click's row from y to y + 2, and the
    /// click's column at least x less the text's width / 3 (`div cl`:
    /// a quotient past FFh faults, None).
    fn note_hit(&self, x: u16, y: u16, si: u16) -> Option<bool> {
        let x = x & 0x7FFF;
        let col = self.mem.r16(DS, CLICK_COL) as i16;
        let row = self.mem.r16(DS, CLICK_ROW) as i16;
        if col > x as i16 || row < y as i16 || row > y.wrapping_add(2) as i16 {
            return Some(false);
        }
        let q = self.text_width(si) / 3;
        if q > 0xFF {
            return None;
        }
        Some(col >= x.wrapping_sub(q) as i16)
    }

    /// 0000:33BF. The note under the click: CLICK_COL/ROW from it (3745);
    /// each of the 40h notes read (NOTE_X/Y, its text to TEXT; none left
    /// to read ends the search) and tested (343C); the one found has its
    /// place kept at NOTE_POS. Whether one was found; None where 343C
    /// faults.
    pub fn note_find(&mut self, files: &mut Files) -> Option<bool> {
        let (col, row) = self.note_to_map(self.mem.r16(DS, CLICK_X), self.mem.r16(DS, CLICK_Y));
        self.mem.w16(DS, CLICK_COL, col);
        self.mem.w16(DS, CLICK_ROW, row);
        self.notes_open(files);
        let mut found = false;
        for _ in 0..0x40 {
            if self.file_read(files, DS, NOTE_X, 4) == 0 {
                break;
            }
            self.file_read(files, DS, TEXT, 0x3C);
            let hit = self.note_hit(self.mem.r16(DS, NOTE_X), self.mem.r16(DS, NOTE_Y), TEXT);
            let hit = match hit {
                Some(h) => h,
                None => return self.original_fault("a note's hit test divides past FFh (0000:343C's div cl)"),
            };
            if hit {
                let pos = self.file_seek(files, 1, 0xFFFF_FFC0);
                self.mem.w32(DS, NOTE_POS, pos);
                found = true;
                break;
            }
        }
        self.file_close(files);
        Some(found)
    }

    /// 23F1:000E. Whether the note to add (NEW_X/Y, its text at TEXT) is
    /// there already: the same place (SAME_PLACE) and text (0000:9C2C,
    /// letters compared in upper case; read to OTHER_TEXT). A free note's
    /// text is skipped. None where the file is missing (0000:67C4 reports
    /// the error).
    pub fn note_duplicate(&mut self, files: &mut Files) -> Option<bool> {
        let path = self.save_path(crate::automap::var::NAME);
        self.file_open(files, path)?;
        let at = self.mem.r32(DS, crate::automap::var::POSITION).wrapping_add(4);
        self.file_seek(files, 0, at);
        for _ in 0..0x40 {
            self.file_read(files, DS, NOTE_X, 4);
            if self.mem.r16(DS, NOTE_X) == 0 {
                self.file_seek(files, 1, 0x3C);
                continue;
            }
            let same = self.mem.r16(DS, NEW_X) == self.mem.r16(DS, NOTE_X) && self.mem.r16(DS, NEW_Y) == self.mem.r16(DS, NOTE_Y);
            self.mem.w8(DS, SAME_PLACE, same as u8);
            self.file_read(files, DS, OTHER_TEXT, 0x3C);
            if self.text_equal(TEXT, OTHER_TEXT) && same {
                self.file_close(files);
                return Some(true);
            }
        }
        self.file_close(files);
        Some(false)
    }

    /// 0000:9C2C. Whether the texts at DS:`a` and DS:`b` are equal, letters
    /// a..z compared as A..Z.
    fn text_equal(&self, a: u16, b: u16) -> bool {
        let up = |c: u8| if c.is_ascii_lowercase() { c - 0x20 } else { c };
        let (mut a, mut b) = (a, b);
        loop {
            let (x, y) = (up(self.mem.r8(DS, a)), up(self.mem.r8(DS, b)));
            if x != y {
                return false;
            }
            if x == 0 {
                return true;
            }
            a = a.wrapping_add(1);
            b = b.wrapping_add(1);
        }
    }

    /// 23F1:00A3. A note at (x, y) with the text at DS:`si` (3Ch bytes
    /// copied to TEXT) added in the first free note (its x word 0), unless
    /// it is there already (`note_duplicate`); with none free, nothing.
    /// None where the file is missing.
    pub fn note_add(&mut self, files: &mut Files, x: u16, y: u16, si: u16) -> Option<()> {
        self.mem.w16(DS, NEW_X, x);
        self.mem.w16(DS, NEW_Y, y);
        for i in 0..0x3Cu16 {
            let b = self.mem.r8(DS, si.wrapping_add(i));
            self.mem.w8(DS, TEXT + i, b);
        }
        if self.note_duplicate(files)? {
            return Some(());
        }
        let path = self.save_path(crate::automap::var::NAME);
        self.file_open(files, path)?;
        let at = self.mem.r32(DS, crate::automap::var::POSITION).wrapping_add(4);
        self.file_seek(files, 0, at);
        for _ in 0..0x40 {
            self.file_read(files, DS, NOTE_X, 2);
            if self.mem.r16(DS, NOTE_X) == 0 {
                self.file_seek(files, 1, 0xFFFF_FFFE);
                let (nx, ny) = (self.mem.r16(DS, NEW_X), self.mem.r16(DS, NEW_Y));
                self.mem.w16(DS, NOTE_X, nx);
                self.mem.w16(DS, NOTE_Y, ny);
                self.file_put(files, DS, NOTE_X, 4);
                self.file_put(files, DS, TEXT, 0x3C);
                self.file_close(files);
                return Some(());
            }
            self.file_seek(files, 1, 0x3E);
        }
        self.file_close(files);
        Some(())
    }

    /// 0000:31FE. A click on the automap screen inside the map: the screen
    /// put back (3698), the note under it found (33BF) and edited, or a new
    /// one begun there (its text cleared; a map square's place from
    /// 3745, or past x CDh the screen's x with bit 15 and y); the text
    /// typed (9EE9, `NoteHooks::type_text`), then written back in place
    /// (an emptied note freed: x 0) or added (23F1:00A3). None where the
    /// original faults (343C) or reports a missing file.
    pub fn note_click(&mut self, files: &mut Files, hooks: &mut dyn NoteHooks) -> Option<()> {
        if !self.note_click_in_map() {
            return Some(());
        }
        // 3698.
        hooks.restore(self);
        let f = self.mem.r8(DS, NOTE_FLAGS) & !2;
        self.mem.w8(DS, NOTE_FLAGS, f);
        let (cx, x, y) = if !self.note_find(files)? {
            for i in 0..0x50u16 {
                self.mem.w8(DS, TEXT + i, 0);
            }
            let (ax, bx) = (self.mem.r16(DS, CLICK_X), self.mem.r16(DS, CLICK_Y));
            if ax > 0xCD {
                self.mem.w16(DS, NOTE_X, 0x8000 | ax);
                self.mem.w16(DS, NOTE_Y, bx);
            } else {
                let (col, row) = self.note_to_map(ax, bx);
                self.mem.w16(DS, NOTE_X, col);
                self.mem.w16(DS, NOTE_Y, row);
            }
            let f = self.mem.r8(DS, NOTE_FLAGS) & !1;
            self.mem.w8(DS, NOTE_FLAGS, f);
            (0, ax, bx)
        } else {
            self.copy_text(TEXT, KEPT_TEXT);
            let (nx, ny) = (self.mem.r16(DS, NOTE_X), self.mem.r16(DS, NOTE_Y));
            let f = self.mem.r8(DS, NOTE_FLAGS) | 1;
            self.mem.w8(DS, NOTE_FLAGS, f);
            hooks.redraw(self);
            self.copy_text(KEPT_TEXT, TEXT);
            self.mem.w16(DS, NOTE_X, nx);
            self.mem.w16(DS, NOTE_Y, ny);
            let (ax, bx) = self.note_to_screen(nx, ny);
            (self.mem.r16(DS, CLICK_X).wrapping_sub(ax), ax, bx)
        };
        self.mem.w8(DS, COLOUR, 6);
        let di = 0x140u16.wrapping_sub(self.mem.r16(DS, CLICK_X));
        let max = (di >> 2).min(0x3B);
        hooks.type_text(self, x, y, cx, max);
        if self.mem.r8(DS, NOTE_FLAGS) & 1 != 0 {
            if self.mem.r8(DS, TEXT) == 0 {
                self.mem.w16(DS, NOTE_X, 0);
            }
            let path = self.save_path(crate::automap::var::NAME);
            let _ = self.file_open_or_device(files, path);
            let pos = self.mem.r32(DS, NOTE_POS);
            self.file_seek(files, 0, pos);
            self.file_put(files, DS, NOTE_X, 4);
            self.file_put(files, DS, TEXT, 0x3C);
            self.file_close(files);
            let f = self.mem.r8(DS, NOTE_FLAGS) & !1;
            self.mem.w8(DS, NOTE_FLAGS, f);
            return Some(());
        }
        let (nx, ny) = (self.mem.r16(DS, NOTE_X), self.mem.r16(DS, NOTE_Y));
        self.note_add(files, nx, ny, TEXT)
    }

    /// 0000:9CA4. The text at DS:`from` copied to DS:`to` with its NUL.
    fn copy_text(&mut self, from: u16, to: u16) {
        let mut i = 0u16;
        loop {
            let b = self.mem.r8(DS, from.wrapping_add(i));
            self.mem.w8(DS, to.wrapping_add(i), b);
            if b == 0 {
                return;
            }
            i = i.wrapping_add(1);
        }
    }

    /// 0000:32FE. The notes drawn (colour 18h): each of the 40h notes read
    /// (NEW_X/Y, its text to TEXT; none left to read ends it); the one
    /// being edited (NOTE_FLAGS bit 0, at NOTE_POS: bit 2) and free ones
    /// skipped; a screen note (bit 15) drawn where it was clicked, a map
    /// note when its square is in the window (MAP_LEFT/TOP, 3Ch x 2Ah
    /// squares) at its screen point (3763), x with bit 15 dropped.
    pub fn notes_draw(&mut self, files: &mut Files, hooks: &mut dyn NoteHooks) {
        use crate::automap::var::{MAP_LEFT, MAP_TOP};
        self.mem.w8(DS, COLOUR, 0x18);
        self.notes_open(files);
        for _ in 0..0x40 {
            let f = self.mem.r8(DS, NOTE_FLAGS) & !4;
            self.mem.w8(DS, NOTE_FLAGS, f);
            if f & 1 != 0 {
                let here = self.file_seek(files, 1, 0);
                if here == self.mem.r32(DS, NOTE_POS) {
                    self.mem.w8(DS, NOTE_FLAGS, f | 4);
                }
            }
            self.mem.w16(DS, NEW_X, 0);
            self.mem.w16(DS, NEW_Y, 0);
            if self.file_read(files, DS, NEW_X, 4) == 0 {
                break;
            }
            self.file_read(files, DS, TEXT, 0x3C);
            if self.mem.r8(DS, NOTE_FLAGS) & 4 != 0 {
                continue;
            }
            let (ax, bx) = (self.mem.r16(DS, NEW_X), self.mem.r16(DS, NEW_Y));
            if ax == 0 {
                continue;
            }
            let (x, y) = if ax & 0x8000 != 0 {
                (ax, bx)
            } else {
                let (l, t) = (self.mem.r16(DS, MAP_LEFT) as i16, self.mem.r16(DS, MAP_TOP) as i16);
                let (a, b) = (ax as i16, bx as i16);
                if a < l || b < t || a >= l.wrapping_add(0x3C) || b >= t.wrapping_add(0x2A) {
                    continue;
                }
                self.note_to_screen(ax, bx)
            };
            hooks.draw_text(self, x & 0x7FFF, y, TEXT);
        }
        self.file_close(files);
    }
}
