//! The message box's state (0000:965A): what laying a text out in a box
//! of tiles leaves in DGROUP, its pixels left to the front end. The box
//! keeps its working values at A836..A858:
//!
//! - A836, A838 the place asked (AX, BX), A84C the flags (CX: 1 the
//!   background kept, 2 centred across the clip, 4 centred down it, 8
//!   the width from FS, 10h the height from GS; 20h set while the
//!   background's buffer is given, 40h a centred line, 80h the drawing
//!   pass), A84E and A850 the text's left and next row (the margin DX
//!   first), A83A and A83C the box's width and height (twice the margin
//!   first), A852 its centre across, A858 a centred line's end;
//! - A844, A846 the text (DS:SI), A848, A84A where the background goes
//!   (ES:DI, or the heap's block for DI FFFFh / FFFEh);
//! - A856, A854 the tiles' width and height ([B038]'s first two words),
//!   A83E a tile's size, A840 the tiles' row step (A6A0's row for their
//!   height).
//!
//! The texts' drawing (0000:9E0A) leaves ADB6 at the end of the text it
//! drew and ADB8 at its row (`text_state`); a text's width is 0000:66F6
//! (`text_width`, in mapnotes.rs).

use crate::machine::Machine;
use crate::mem::DS;

/// The rows' offsets on the 320-wide page (y x 140h, a word a row).
const ROWS: u16 = 0xA6A0;
/// The fonts' segments by number (a font code 0Ch n picks n - 1).
const FONTS: u16 = 0x91AC;
/// The line being laid out (0000:98A0 copies each line here).
const LINE: u16 = 0x929E;

impl Machine {
    /// 0000:9E06's state: a text drawn at (`x`, `y`) in the font at
    /// [B034] leaves ADB6 at its end and ADB8 at `y`. From each byte of
    /// the text at DS:`si` 20h is taken: 0 (a space) moves on the font's
    /// second byte and one more, a signed result below 0 is passed over,
    /// any other draws the character and moves on its width.
    pub fn text_state(&mut self, x: u16, y: u16, si: u16) {
        self.text_state_far(x, y, (DS, si));
    }

    /// `text_state` of the text at `tseg`:`si`.
    pub fn text_state_far(&mut self, x: u16, y: u16, (tseg, si): (u16, u16)) {
        let (off, seg) = (self.mem.r16(DS, 0xB034), self.mem.r16(DS, 0xB036));
        let mut x = x;
        self.mem.w16(DS, 0xADB8, y);
        let mut at = si;
        loop {
            let c = self.mem.r8(tseg, at);
            at = at.wrapping_add(1);
            if c == 0 {
                break;
            }
            let c = c.wrapping_sub(0x20);
            if c == 0 {
                x = x.wrapping_add(self.mem.r8(seg, off.wrapping_add(1)) as u16 + 1);
            } else if (c as i8) > 0 {
                x = x.wrapping_add(self.mem.r8(seg, off.wrapping_add(c as u16)) as u16);
            }
        }
        self.mem.w16(DS, 0xADB6, x);
    }

    /// 0000:965A's state: the text at `seg`:`si` in a box at (`ax`,
    /// `bx`) with flags `cx` and margin `dx` (see the module's notes), its
    /// background kept at `es`:`di` with flag 1 (`fs`, `gs` the width and
    /// height for flags 8 and 10h). Measured (98A0), placed, the box
    /// (9798) and the text laid out again to draw (98A0 with 80h).
    /// `screen` is the draw page as the front end has it (a byte a pixel,
    /// 320 wide; none: black), for the background. Returns DX:AX, where
    /// the background went.
    #[allow(clippy::too_many_arguments)]
    pub fn message_box(&mut self, (seg, si): (u16, u16), (ax, bx): (u16, u16), cx: u16, dx: u16, (es, di): (u16, u16), (fs, gs): (u16, u16), screen: Option<&[u8]>) -> Option<(u16, u16)> {
        let w = |m: &mut Machine, o: u16, v: u16| m.mem.w16(DS, o, v);
        let r = |m: &Machine, o: u16| m.mem.r16(DS, o);
        w(self, 0xA846, seg);
        w(self, 0xA844, si);
        w(self, 0xA848, di);
        w(self, 0xA84A, es);
        w(self, 0xA836, ax);
        w(self, 0xA838, bx);
        w(self, 0xA84C, cx);
        w(self, 0xA850, dx);
        w(self, 0xA84E, dx);
        w(self, 0xA83A, dx.wrapping_add(dx));
        w(self, 0xA83C, dx.wrapping_add(dx));
        let tiles = r(self, 0xB038);
        let (tw, th) = (r(self, tiles), r(self, tiles.wrapping_add(2)));
        w(self, 0xA856, tw);
        w(self, 0xA83E, tw.wrapping_mul(th));
        w(self, 0xA854, th);
        let step = r(self, ROWS.wrapping_add(th.wrapping_add(th)));
        w(self, 0xA840, step);
        self.box_lines();
        if r(self, 0xA84C) & 8 != 0 {
            w(self, 0xA83A, fs);
        }
        if r(self, 0xA84C) & 0x10 != 0 {
            w(self, 0xA83C, gs);
        }
        let width = r(self, 0xA83A);
        let mut left = r(self, 0xA836);
        if r(self, 0xA84C) & 2 != 0 {
            let mid = r(self, 0xA918).wrapping_add(r(self, 0xA914)) >> 1;
            left = mid.wrapping_sub(width >> 1);
        }
        let v = r(self, 0xA84E).wrapping_add(left);
        w(self, 0xA84E, v);
        if r(self, 0xA84C) & 4 == 0 {
            let v = r(self, 0xA850).wrapping_add(r(self, 0xA838));
            w(self, 0xA850, v);
        }
        w(self, 0xA852, (width >> 1).wrapping_add(left));
        self.box_frame(screen)?;
        let v = r(self, 0xA84C) | 0x80;
        w(self, 0xA84C, v);
        self.box_lines();
        Some((r(self, 0xA84A), r(self, 0xA848)))
    }

    /// 0000:9798: the box's size rounded up to whole tiles (half the
    /// width's rounding added to the text's left, half of one more than
    /// the height's to its row), placed (centred where its flags say),
    /// and with flag 1 its background kept: a buffer from the heap for
    /// A848 FFFEh (0000:9CD6) or FFFFh (51EC), then the box's x (bit 15
    /// with the buffer given), y, width and height and its rows from the
    /// page (8F2A, A8FC 0 for a buffer given). The tiles' drawing (9727)
    /// is the front end's. None where the heap has no room.
    fn box_frame(&mut self, screen: Option<&[u8]>) -> Option<()> {
        let w = |m: &mut Machine, o: u16, v: u16| m.mem.w16(DS, o, v);
        let r = |m: &Machine, o: u16| m.mem.r16(DS, o);
        let (tw, th) = (r(self, 0xA856), r(self, 0xA854));
        let rem = r(self, 0xA83A) % tw;
        if rem != 0 {
            let d = tw - rem;
            let v = r(self, 0xA83A).wrapping_add(d);
            w(self, 0xA83A, v);
            let v = r(self, 0xA84E).wrapping_add(d >> 1);
            w(self, 0xA84E, v);
        }
        let rem = r(self, 0xA83C) % th;
        if rem != 0 {
            let d = th - rem;
            let v = r(self, 0xA83C).wrapping_add(d);
            w(self, 0xA83C, v);
            let v = r(self, 0xA850).wrapping_add(d.wrapping_add(1) >> 1);
            w(self, 0xA850, v);
        }
        let flags = r(self, 0xA84C);
        let mut x = r(self, 0xA836);
        if flags & 2 != 0 {
            let mid = r(self, 0xA914).wrapping_add(r(self, 0xA918)) >> 1;
            x = mid.wrapping_sub(r(self, 0xA83A) >> 1);
        }
        let mut y = r(self, 0xA838);
        if flags & 4 != 0 {
            let mid = r(self, 0xA91A).wrapping_add(r(self, 0xA916)) >> 1;
            y = mid.wrapping_sub(r(self, 0xA83C) >> 1);
            let v = r(self, 0xA850).wrapping_add(y);
            w(self, 0xA850, v);
        }
        if flags & 1 == 0 {
            return Some(());
        }
        w(self, 0xA84C, flags | 0x20);
        let to = r(self, 0xA848);
        if to == 0xFFFE || to == 0xFFFF {
            let size = r(self, 0xA83A).wrapping_mul(r(self, 0xA83C)).wrapping_add(8);
            let (seg, off) = if to == 0xFFFE {
                match self.heap_alloc_high(size)? {
                    0 => return crate::giveup::not_ported("0000:9798: no room for the box's background: it would go to 0:0"),
                    seg => (seg, 0),
                }
            } else {
                self.heap_alloc_bytes(size)?
            };
            w(self, 0xA848, off);
            w(self, 0xA84A, seg);
            let v = r(self, 0xA84C) & !0x20;
            w(self, 0xA84C, v);
        }
        let (bw, bh) = (r(self, 0xA83A), r(self, 0xA83C));
        let (off, seg) = (r(self, 0xA848), r(self, 0xA84A));
        let given = r(self, 0xA84C) & 0x20 != 0;
        self.box_background(seg, off, (x, y, bw, bh), screen);
        // The head's x as 9876 writes it: 8000h set with the buffer given,
        // as it is from the heap (a box left of the screen, x negative,
        // has it set too, and 933C then doesn't free its block).
        if !given {
            self.mem.w16(seg, off, x);
        }
        self.mem.w16(DS, 0xA8FC, 0);
        Some(())
    }

    /// 0000:98A0: the box's text laid out line by line (to a NUL, CR or
    /// LF; a CR's line centred, A84C bit 40h), each copied to 929E with
    /// its codes (0Ch n: font n's segment from 91AC into B036; 09h c: the
    /// colour c for what follows), its width measured (`text_width`, the
    /// widest kept) and with A84C bit 80h drawn: from A84E, or centred
    /// on A852 (A858 its pieces' starts), at row A850, which then moves
    /// down the font's height and one. The lines stop at a NUL, or where
    /// the line's end is followed by one. Then the widest is added to
    /// A83A, the lines times the last font's height and one to A83C, and
    /// the font put back.
    pub(crate) fn box_lines(&mut self) {
        let w = |m: &mut Machine, o: u16, v: u16| m.mem.w16(DS, o, v);
        let r = |m: &Machine, o: u16| m.mem.r16(DS, o);
        let font = r(self, 0xB036);
        let mut widest = 0u16;
        let mut lines = 0u16;
        let tseg = r(self, 0xA846);
        let mut si = r(self, 0xA844);
        loop {
            let mut di = LINE;
            let f = r(self, 0xA84C) & !0x40;
            w(self, 0xA84C, f);
            loop {
                let al = self.mem.r8(tseg, si);
                si = si.wrapping_add(1);
                self.mem.w8(DS, di, al);
                di = di.wrapping_add(1);
                match al {
                    0x0C => {
                        let n = self.mem.r8(tseg, si);
                        self.mem.w8(DS, di, n);
                        si = si.wrapping_add(1);
                        di = di.wrapping_add(1);
                        let bx = (n.wrapping_sub(1) as u16).wrapping_add(n.wrapping_sub(1) as u16);
                        let v = r(self, FONTS.wrapping_add(bx));
                        w(self, 0xB036, v);
                    }
                    0x09 => {
                        let c = self.mem.r8(tseg, si);
                        self.mem.w8(DS, di, c);
                        si = si.wrapping_add(1);
                        di = di.wrapping_add(1);
                    }
                    0 | 0x0A => break,
                    0x0D => {
                        let f = r(self, 0xA84C) | 0x40;
                        w(self, 0xA84C, f);
                        break;
                    }
                    _ => {}
                }
            }
            self.mem.w8(DS, di.wrapping_sub(1), 0);
            let ax = self.text_width(LINE);
            lines = lines.wrapping_add(1);
            widest = widest.max(ax);
            if r(self, 0xA84C) & 0x80 != 0 {
                self.box_line_draw(ax);
            }
            let ends = self.mem.r8(tseg, si) == 0 || self.mem.r8(tseg, si.wrapping_sub(1)) == 0;
            if ends {
                break;
            }
        }
        let v = r(self, 0xA83A).wrapping_add(widest);
        w(self, 0xA83A, v);
        let (off, seg) = (r(self, 0xB034), r(self, 0xB036));
        let h = self.mem.r8(seg, off).wrapping_add(1) as u16;
        let v = r(self, 0xA83C).wrapping_add(lines.wrapping_mul(h));
        w(self, 0xA83C, v);
        w(self, 0xB036, font);
    }

    /// 0000:990D..99D6: the line at 929E (`ax` wide) drawn: centred, its
    /// start A852 less half its width (A858); piece by piece between its
    /// colour codes (9288 the code's colour after each piece) and font
    /// codes; then the row on by the font's height and one.
    fn box_line_draw(&mut self, ax: u16) {
        let w = |m: &mut Machine, o: u16, v: u16| m.mem.w16(DS, o, v);
        let r = |m: &Machine, o: u16| m.mem.r16(DS, o);
        if r(self, 0xA84C) & 0x40 != 0 {
            let v = r(self, 0xA852).wrapping_sub(ax >> 1);
            w(self, 0xA858, v);
        }
        // One piece from `di` (the line's bytes up to a NUL) drawn.
        let piece = |m: &mut Machine, di: u16| {
            let (x, y) = if r(m, 0xA84C) & 0x40 != 0 {
                let end = m.text_width(di).wrapping_add(r(m, 0xA858));
                let start = r(m, 0xA858);
                w(m, 0xA858, end);
                (start, r(m, 0xA850))
            } else {
                (r(m, 0xA84E), r(m, 0xA850))
            };
            m.text_state(x, y, di);
        };
        let mut si = LINE;
        let mut di = si;
        loop {
            let c = self.mem.r8(DS, si);
            if c == 9 {
                let colour = self.mem.r8(DS, si.wrapping_add(1));
                self.mem.w8(DS, si, 0);
                piece(self, di);
                self.mem.w8(DS, 0x9288, colour);
                si = si.wrapping_add(2);
                di = si;
            } else if c == 0x0C {
                let n = self.mem.r8(DS, si.wrapping_add(1)).wrapping_sub(1) as u16;
                let v = r(self, FONTS.wrapping_add(n.wrapping_add(n)));
                w(self, 0xB036, v);
                si = si.wrapping_add(2);
                di = si;
            } else if c != 0 {
                si = si.wrapping_add(1);
            } else {
                piece(self, di);
                let h = self.mem.r8(r(self, 0xB036), 0).wrapping_add(1) as u16;
                let v = r(self, 0xA850).wrapping_add(h);
                w(self, 0xA850, v);
                return;
            }
        }
    }

    /// 25FD:0803: "Loading..." (5F26) shown: the tiles from 1036 (B038),
    /// the font from 102C (B036), colour D4h, 5E0 bit 800h (the box not
    /// waited on) round 1194:59C8 (`message_wait_box`), the cursor's
    /// background flag (A902 bit 1) cleared. None where the box's
    /// background finds no room (it never asks the heap here).
    pub fn loading_shown(&mut self, screen: Option<&[u8]>) -> Option<()> {
        let v = self.mem.r16(DS, 0x1036);
        self.mem.w16(DS, 0xB038, v);
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        self.mem.w8(DS, 0x9288, 0xD4);
        let f = self.mem.r16(DS, 0x05E0);
        self.mem.w16(DS, 0x05E0, f | 0x800);
        self.message_wait_box((DS, 0x5F26), screen)?;
        let f = self.mem.r16(DS, 0x05E0);
        self.mem.w16(DS, 0x05E0, f & !0x800);
        let f = self.mem.r16(DS, 0xA902);
        self.mem.w16(DS, 0xA902, f & !1);
        Some(())
    }

    /// 0000:4B02..4B21: the town's welcome: the file named at 130C
    /// (ENTERCTY.$$$) loaded by its path in the save's folder (25FD:0844,
    /// 7428) to 1044, then its box (965E: CX 7, DX 5, centred, what it
    /// covers kept at the frame's start, [9030]:0, in the bank the caller
    /// mapped, [1030]). AX and BX are what came before leaves: the load's
    /// offset and the bank (4AFB). The page's showing (946A) and the wait
    /// for a key (1194:5532) are the front end's. None where the file
    /// can't be loaded.
    pub fn town_welcome_box(&mut self, files: &mut crate::bsa::Files, screen: Option<&[u8]>) -> Option<()> {
        let path = self.save_path(0x130C);
        let (_, off) = self.file_load(files, path, DS, 0x1044)?;
        let bank = self.mem.r16(DS, 0x1030);
        let frame = self.mem.r16(DS, 0x9030);
        self.message_box((DS, 0x1044), (off, bank), 7, 5, (frame, 0), (0, 0), screen)?;
        Some(())
    }

    /// 1194:59C8 whole: the box shown (`message_wait_box`) and, unless 5E0
    /// bit 800h, once the front end's wait for a click or a key (1194:5536)
    /// is over, put away (`message_box_closed`) and the buttons let go: its
    /// last loop (5E21) reads the mouse until AFF9 is 0, so a click's
    /// release (or a press left from before, which 59E5 waits out) is the
    /// box's, not the next frame's. The mouse's other reads on the way
    /// (8FC4: 8F92, 8F94) are the front end's.
    pub fn message_shown(&mut self, text: (u16, u16), screen: Option<&[u8]>) -> Option<()> {
        self.message_wait_box(text, screen)?;
        if self.mem.r16(DS, 0x05E0) & 0x800 == 0 {
            self.message_box_closed();
            self.mem.w8(DS, 0xAFF9, 0);
        }
        Some(())
    }

    /// 1194:59C8 up to 5A23 (all of it with 5E0 bit 800h set): bank 18h
    /// mapped (11h in a dungeon, 5E6 4), the box (965A: CX 4007h, DX 5,
    /// centred in the clip, its background at [902E], the frame's start)
    /// for the text at `text`. The mouse's wait (8FC4), the sound's poll
    /// (2F26:02A1), the keys' flush (710E) and the page's showing (9466)
    /// are the front end's; of the dissolve that shows it, its generator
    /// (`dissolve`).
    pub fn message_wait_box(&mut self, text: (u16, u16), screen: Option<&[u8]>) -> Option<()> {
        let bank = if self.mem.r8(DS, 0x05E6) == 4 { 0x11 } else { 0x18 };
        self.ems_map(bank);
        let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        // AX and BX as the mouse's read (8FC4) leaves them: the cursor's
        // x and y (8F92, 8F94), which 965A keeps at A836 and A838.
        let at = (self.mem.r16(DS, 0x8F92), self.mem.r16(DS, 0x8F94));
        self.message_box(text, at, 0x4007, 5, (seg, off), (0, 0), screen)?;
        self.dissolve();
        Some(())
    }

    /// 1194:5A2E..5A6B, after the wait: the background put back (933C,
    /// `box_restore`, from the segment 965A returned in DX, A84A), the
    /// dissolve's generator stepped again, and in a dungeon (5E6 4) bank
    /// 11h mapped and the 4000h bytes at [902E] filled with 15h. The
    /// page's showing (9466) and the wait for the buttons' release (5E21)
    /// are the front end's.
    pub fn message_box_closed(&mut self) {
        let seg = self.mem.r16(DS, 0xA84A);
        self.box_restore(seg);
        self.dissolve();
        if self.mem.r8(DS, 0x05E6) == 4 {
            self.ems_map(0x11);
            let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
            for i in 0..0x4000u16 {
                self.mem.w8(seg, off.wrapping_add(i), 0x15);
            }
        }
    }

    /// 0000:933C: a box's background put back from the record at `seg`:0
    /// (its x, y, width and height, the pixels after; the page is the
    /// front end's), and unless its x has bit 15 (a buffer given, or a
    /// box left of the screen) the heap's block at `seg` freed (6C36).
    /// Returns (x less bit 15, y, width, height) for the front end.
    pub fn box_restore(&mut self, seg: u16) -> (u16, u16, u16, u16) {
        let x = self.mem.r16(seg, 0);
        let rect = (x & 0x7FFF, self.mem.r16(seg, 2), self.mem.r16(seg, 4), self.mem.r16(seg, 6));
        if x & 0x8000 == 0 {
            self.heap_free(seg);
        }
        rect
    }

    /// 1194:4800 up to its field (9EE5): `cx` bytes at `es`:`di` (the
    /// field's text) cleared, the keys flushed (710E), the prompt at
    /// `text` in its box (965A: CX 7, DX 0Ah, centred, its background in
    /// the heap's top, DI FFFEh), with AX's low byte the stosb's 0 (AH and
    /// BX what the caller left: the field's place, which the field takes
    /// over at once), the background's segment kept at 1044 and ARENAFNT
    /// selected ([102C] into B036) for the field. None where the heap has
    /// no room.
    pub fn prompt_shown(&mut self, text: (u16, u16), (ax, bx): (u16, u16), (es, di): (u16, u16), cx: u16, screen: Option<&[u8]>) -> Option<()> {
        for i in 0..cx {
            self.mem.w8(es, di.wrapping_add(i), 0);
        }
        self.key_flush();
        self.prompt_box(text, (ax & 0xFF00, bx), es, screen)?;
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        Some(())
    }

    /// 965A with CX 7, DX 0Ah, DI FFFEh as 1194:4800 and 5C34 call it,
    /// then 1044 the background's segment (DX). (4800 then selects
    /// ARENAFNT, 482B; 5C34 leaves the font its caller chose.)
    fn prompt_box(&mut self, text: (u16, u16), at: (u16, u16), es: u16, screen: Option<&[u8]>) -> Option<()> {
        let (seg, _) = self.message_box(text, at, 7, 0x0A, (es, 0xFFFE), (0, 0), screen)?;
        self.mem.w16(DS, 0x1044, seg);
        Some(())
    }

    /// 1194:4800 after its field: the box's background put back (933C,
    /// `box_restore`, its segment from 1044). The page's showing (9466)
    /// is the front end's.
    pub fn prompt_closed(&mut self) {
        let seg = self.mem.r16(DS, 0x1044);
        self.box_restore(seg);
    }

    /// 1194:5C34 up to its field (9EE5): the keys flushed (710E), the
    /// prompt at `text` written to 1044 with a line of room under it
    /// (8D14 with 4FBC "%s\r" and 25h spaces, "\r"), its box as 4800's
    /// (`prompt_box`; ES the formatter's, DGROUP), and the dissolve
    /// (4FF7 over the whole screen, `dissolve`). AX is what the
    /// formatter leaves: its %s's length (9CB0) with the last byte read,
    /// 0, in AL; BX, its arguments' pointer, is an offset on the stack
    /// (`bx`; the field overwrites both at once). None where the heap has
    /// no room.
    pub fn named_shown(&mut self, text: (u16, u16), bx: u16, screen: Option<&[u8]>) -> Option<()> {
        self.key_flush();
        let mut len = 0u16;
        while self.mem.r8(text.0, text.1.wrapping_add(len)) != 0 {
            len = len.wrapping_add(1);
        }
        self.sprintf((DS, 0x1044), (DS, 0x4FBC), &[crate::format::Arg::Str(text.0, text.1)]);
        self.prompt_box((DS, 0x1044), (len & 0xFF00, bx), DS, screen)?;
        self.dissolve();
        Some(())
    }

    /// 1194:5C34 after its field: the background put back (933C from
    /// 1044) and the dissolve again. The page's showing (9466) is the
    /// front end's.
    pub fn named_closed(&mut self) {
        self.prompt_closed();
        self.dissolve();
    }

    /// 2542:051A..052B: the status box (the text at 941E, `status_box`)
    /// in a box centred both ways, its background not kept (965A: CX 6,
    /// DX 5, DI FFFFh, ES DGROUP as the text's formatting leaves it). AX
    /// and BX are what came before leaves: [1036] from the tiles' choice
    /// (9AF8), and the formatter's arguments' pointer, an offset on the
    /// stack (`bx`). The page's showing (9466) and the wait for a click or
    /// a key are the front end's.
    pub fn status_shown(&mut self, bx: u16, screen: Option<&[u8]>) -> Option<()> {
        let ax = self.mem.r16(DS, 0x1036);
        self.message_box((DS, 0x941E), (ax, bx), 6, 5, (DS, 0xFFFF), (0, 0), screen)?;
        Some(())
    }

    /// 0000:53AE: the mouse's limits, x from `ax` over `cx` and y from
    /// `bx` over `dx` (8F74 left, 8F76 right, 8F78 top, 8F7A bottom, the
    /// last pixel in). Giving them to the driver (int 33h 7, 8) and its
    /// cursor put at their middle when outside are the front end's.
    pub fn mouse_limits(&mut self, ax: u16, bx: u16, cx: u16, dx: u16) {
        self.mem.w16(DS, 0x8F78, bx);
        self.mem.w16(DS, 0x8F7A, dx.wrapping_add(bx).wrapping_sub(1));
        self.mem.w16(DS, 0x8F74, ax);
        self.mem.w16(DS, 0x8F76, cx.wrapping_add(ax).wrapping_sub(1));
    }

    /// 1194:4FF7's state (its pixels the front end's, view/src/film.rs):
    /// unless 5CA bit 2, the dissolve's generator (4B8E, 1194:5065: x
    /// 5E5h + 29h) stepped FA00h times, once a pixel.
    pub fn dissolve(&mut self) {
        if self.mem.r16(DS, 0x05CA) & 2 != 0 {
            return;
        }
        let mut r = self.mem.r16(DS, 0x4B8E);
        for _ in 0..0xFA00u32 {
            r = r.wrapping_mul(0x5E5).wrapping_add(0x29);
        }
        self.mem.w16(DS, 0x4B8E, r);
    }
}
