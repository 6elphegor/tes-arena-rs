//! The interface's pixels on the draw page ([A900], 320 by 200, a byte a
//! pixel), as the original puts them there: the frame's 3D view (the
//! drawing's, `Engine::page`), the bar's name, condition picture, face
//! and stat bars (0000:4DD6, 2901:00A8), the keys held (4D2F), the
//! level's text (05B5), the message line (0511), the compass (4D7C), an
//! effect's words (5011), the row under the view cleared (4CC0) and the
//! view's window cleared (4EFD); the hands are `weapon`'s (3B8E). What
//! the page holds is saved whole to SCREENSV.$$2 under the screens
//! (0000:A69E), so it must be the original's.
//!
//! Only where the page is the game's own heap block (`page_is_block`): a
//! game the port starts without one keeps A000h there, over its heap's
//! blocks. What the drawings leave in DGROUP is left either way.

use crate::machine::Machine;
use crate::mem::DS;

/// The page's rows' offsets (a word a row).
const ROWS: u16 = 0xA6A0;

impl Machine {
    /// The draw page's segment, where it is the game's own block.
    fn page_seg(&self) -> Option<u16> {
        self.page_is_block().then(|| self.mem.r16(DS, 0xA900))
    }

    /// The frame's 3D view (`view`, the drawing's page: 320 by 200) put
    /// into the draw page over the view's window (C39C..C3A2), where the
    /// original's drawing draws it.
    pub fn page_view_put(&mut self, view: &[u8]) {
        let Some(seg) = self.page_seg() else { return };
        let r = |o: u16| self.mem.r16(DS, o) as usize;
        let (left, top, right, bottom) = (r(0xC39C), r(0xC39E), r(0xC3A0), r(0xC3A2));
        let page = self.mem.segment_mut(seg);
        for y in top..=bottom.min(199) {
            let row = y * 320;
            page[row + left..=row + right.min(319)].copy_from_slice(&view[row + left..=row + right.min(319)]);
        }
    }

    /// 2901:00A8: the three stat bars (health 1FD of 1FF at x 39h, colour
    /// 1Ah; fatigue 201 of (1CD + 1D2) x 40h at 43h, 9Fh; spell points 20A
    /// of 20C at 4Dh, 3Eh), each 4 wide and 1Ah rows from row A8h (page
    /// offset D200h): the value (0 under 0, the maximum past it) times
    /// 100h over the maximum, times 1Ah, over 100h rows of the colour at
    /// the bottom, black above; all black with a maximum of 0. Each leaves
    /// its x at 126C and, with a maximum not 0, its colour at 9288. (The
    /// screen's copy, A000h, is the front end's.)
    pub fn bars_draw(&mut self) {
        let fatigue = (self.mem.r8(DS, 0x01CD) as u16 + self.mem.r8(DS, 0x01D2) as u16) << 6;
        let page = self.page_seg();
        for (x, v, max, colour) in [(0x39u16, self.mem.r16(DS, 0x01FD), self.mem.r16(DS, 0x01FF), 0x1Au8), (0x43, self.mem.r16(DS, 0x0201), fatigue, 0x9F), (0x4D, self.mem.r16(DS, 0x020A), self.mem.r16(DS, 0x020C), 0x3E)] {
            let mut ax = if (v as i16) < 0 { 0 } else { v };
            if ax > max {
                ax = max;
            }
            self.mem.w16(DS, 0x126C, x);
            let full = if max == 0 {
                0
            } else {
                self.mem.w8(DS, 0x9288, colour);
                let q = ((ax as u32) << 8) / max as u32;
                ((q * 0x1A) >> 8) as u16
            };
            let Some(seg) = page else { continue };
            let mut di = 0xD200u16.wrapping_add(x);
            for row in 0..0x1Au16 {
                let c = if row < 0x1A - full { 0 } else { colour };
                for k in 0..4 {
                    self.mem.w8(seg, di.wrapping_add(k), c);
                }
                di = di.wrapping_add(0x140);
            }
        }
    }

    /// 0000:4CC0..4CDA: the row under the view (C3A2) cleared, its 140h
    /// bytes from the row table's offset (A6A0).
    pub fn view_row_clear(&mut self) {
        let Some(seg) = self.page_seg() else { return };
        let y = self.mem.r16(DS, 0xC3A2);
        let di = self.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y)));
        for k in 0..0x140u16 {
            self.mem.w8(seg, di.wrapping_add(k), 0);
        }
    }

    /// 0000:4EFD: the view's window (C39C..C3A2) filled with colour 0
    /// (9288 0, then 9A72 with its corner, width and height).
    pub fn view_clear(&mut self) {
        self.mem.w8(DS, 0x9288, 0);
        let Some(seg) = self.page_seg() else { return };
        let r = |m: &Machine, o: u16| m.mem.r16(DS, o);
        let (x, y) = (r(self, 0xC39C), r(self, 0xC39E));
        let w = r(self, 0xC3A0).wrapping_sub(x).wrapping_add(1);
        let h = r(self, 0xC3A2).wrapping_sub(y).wrapping_add(1);
        // (A window of no rows, which would run 9A72's `loop` on through
        // memory, is left alone.)
        if (h as i16) <= 0 {
            return;
        }
        let mut di = self.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
        for _ in 0..h {
            for k in 0..w {
                self.mem.w8(seg, di.wrapping_add(k), 0);
            }
            di = di.wrapping_add(0x140);
        }
    }

    /// 0000:4D2F: the keys held (the 0Ch bytes at 652, C5h none), each the
    /// ((key - 1) mod 0Ch)'th picture of the keys' file at [43D4]:0 drawn
    /// whole at x 8 (0000:5B08), from row 10h down 0Ah a key drawn. None
    /// for a key of 0, whose `div` faults in the original.
    pub fn keys_draw(&mut self) -> Option<()> {
        let mut y = 0x10u16;
        for i in 0..0x0Cu16 {
            let b = self.mem.r8(DS, 0x0652 + i);
            if b == 0xC5 {
                continue;
            }
            if b == 0 {
                return crate::giveup::original("0000:4D46: a key of 0: (0 - 1) / 0Ch doesn't fit AL");
            }
            let n = (b as u16 - 1) % 0x0C;
            let seg = self.mem.r16(DS, 0x43D4);
            let mut si = 0u16;
            for _ in 0..n {
                si = si.wrapping_add(self.mem.r16(seg, si.wrapping_add(0xA))).wrapping_add(0xC);
            }
            let (w, h) = (self.mem.r16(seg, si.wrapping_add(4)), self.mem.r16(seg, si.wrapping_add(6)));
            self.page_whole(8, y, w, h, seg, si.wrapping_add(0xC));
            y = y.wrapping_add(0xA);
        }
        Some(())
    }

    /// 0000:5B08 into the draw page: `w` by `h` from seg:off at (`x`,
    /// `y`), its 0s left out, not cut.
    fn page_whole(&mut self, x: u16, y: u16, w: u16, h: u16, seg: u16, off: u16) {
        let Some(p) = self.page_seg() else { return };
        let mut page = self.mem.segment(p)[..0x10000].to_vec();
        crate::render::whole_page(self, &mut page, x, y, w, h, seg, off);
        self.mem.segment_mut(p)[..0x10000].copy_from_slice(&page);
    }

    /// 0000:4D7C: while the compass is shown (5C8 bit 2000h) and not held
    /// back (1013), seven rows of 20h bytes of the slider ([438F]:, from
    /// ((heading + E0h) & 1FFh) / 2, 121h a row) put at (90h, 7), then
    /// the compass's frame (the image at the far pointer 4391) drawn
    /// whole at its header's place (0000:5B08).
    pub fn compass_draw(&mut self) {
        if self.mem.r16(DS, 0x05C8) & 0x2000 == 0 || self.mem.r8(DS, 0x1013) != 0 {
            return;
        }
        let Some(page) = self.page_seg() else { return };
        let slider = self.mem.r16(DS, 0x438F);
        let mut si = (self.mem.r16(DS, crate::movement::var::HEADING).wrapping_add(0xE0) & 0x1FF) >> 1;
        let mut di = 0x950u16;
        for _ in 0..7 {
            for _ in 0..0x20 {
                let b = self.mem.r8(slider, si);
                self.mem.w8(page, di, b);
                si = si.wrapping_add(1);
                di = di.wrapping_add(1);
            }
            di = di.wrapping_add(0x120);
            si = si.wrapping_add(0x101);
        }
        let (si, seg) = (self.mem.r16(DS, 0x4391), self.mem.r16(DS, 0x4393));
        let r = |m: &Machine, k: u16| m.mem.r16(seg, si.wrapping_add(k));
        let (x, y, w, h) = (r(self, 0), r(self, 2), r(self, 4), r(self, 6));
        self.page_whole(x, y, w, h, seg, si.wrapping_add(0xC));
    }

    /// 0000:051D..054D: the message line (the text at the far pointer
    /// 61A) drawn centred on x A0h at row 14h (0000:5368) in the font
    /// selected, in colour 70h, then one pixel right in 9Fh.
    pub fn message_line_draw(&mut self) {
        let text = (self.mem.r16(DS, 0x061C), self.mem.r16(DS, 0x061A));
        for (x, colour) in [(0xA0u16, 0x70u8), (0xA1, 0x9F)] {
            self.mem.w8(DS, 0x9288, colour);
            self.text_centred(text, x, 0x14);
        }
    }

    /// 0000:5011's drawing (5027..5052): an effect's words (1405) in the
    /// font from 102C (9AEA) centred on x A0h at the view's bottom (C3A2)
    /// less 0Bh in colour FDh, then one pixel right in FCh.
    pub fn effect_words_draw(&mut self) {
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        let y = self.mem.r16(DS, 0xC3A2).wrapping_sub(0xB);
        for (x, colour) in [(0xA0u16, 0xFDu8), (0xA1, 0xFC)] {
            self.mem.w8(DS, 0x9288, colour);
            self.text_centred((DS, crate::effects::var::MESSAGE_TEXT), x, y);
        }
    }

    /// 0000:05B5: the level's text shown (at [3D42] in the EMS frame,
    /// [9030], its bank mapped) drawn line by line from the view's bottom
    /// (C3A2) less its height (3D40), 0Ah apart: each line copied to DGROUP
    /// 929E up to a CR (the next line) or a 0 (the last), a `%` (and the
    /// three bytes after it) the player's name (01AD, its 0 and all: the
    /// rest of the line is copied after that 0), and a `:` after a 0 or a
    /// CR the end with that line not drawn; each line centred on x A0h in
    /// colour 70h, then one pixel right in F0h (0605).
    pub fn level_text_lines(&mut self) {
        let seg = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let mut si = self.mem.r16(DS, crate::squares::var::SHOWN);
        let mut y = self.mem.r16(DS, 0xC3A2).wrapping_sub(self.mem.r16(DS, 0x3D40));
        'lines: loop {
            let mut di = 0x929Eu16;
            loop {
                let al = self.mem.r8(seg, si);
                si = si.wrapping_add(1);
                if al == b':' {
                    let before = self.mem.r8(seg, si.wrapping_sub(2));
                    if before == 0 || before == 0x0D {
                        return;
                    }
                } else if al == b'%' {
                    si = si.wrapping_add(3);
                    let mut from = 0x01ADu16;
                    loop {
                        let c = self.mem.r8(DS, from);
                        from = from.wrapping_add(1);
                        self.mem.w8(DS, di, c);
                        di = di.wrapping_add(1);
                        if c == 0 {
                            break;
                        }
                    }
                    continue;
                }
                if al == 0 {
                    self.mem.w8(DS, di, 0);
                    self.level_text_line(y);
                    return;
                }
                if al == 0x0D {
                    self.mem.w8(DS, di, 0);
                    self.level_text_line(y);
                    y = y.wrapping_add(0xA);
                    continue 'lines;
                }
                self.mem.w8(DS, di, al);
                di = di.wrapping_add(1);
            }
        }
    }

    /// 0000:0605: the line at 929E centred on x A0h at row `y` in colour
    /// 70h, then one pixel right in F0h.
    fn level_text_line(&mut self, y: u16) {
        for (x, colour) in [(0xA0u16, 0x70u8), (0xA1, 0xF0)] {
            self.mem.w8(DS, 0x9288, colour);
            self.text_centred((DS, 0x929E), x, y);
        }
    }

    /// 0000:5368: the text at `seg`:`si` drawn centred on x `x` at row `y`
    /// (its width, 66F6, halved off: 9E0A), into the page where it is the
    /// game's own block; its state either way (`text_state_far`).
    fn text_centred(&mut self, text: (u16, u16), x: u16, y: u16) {
        let w = self.text_width_far(text);
        let x = x.wrapping_sub(w >> 1);
        if self.page_is_block() {
            self.page_text(text, x, y);
        } else {
            self.text_state_far(x, y, text);
        }
    }
}
