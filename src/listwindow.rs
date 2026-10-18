//! The shops' list windows as the game's state sees them: 1194:5734 (the
//! screen kept in the load bank, the panel's picture, the window's record
//! filled) and the window library's list (3159:06A0: its record's words,
//! its buffers on the heap, its last drawing's scratch).
//!
//! The record (DS:`tmpl`, 55h bytes, five of them from 5006) holds four
//! boxes (x, y, width, height: the arrows up and down at +0 and +8, the
//! scroll bar at +10, the lines at +18), the flags at +20 (1: put back
//! what it covers on leaving; 1000h, 2000h: the lines centred, or ended,
//! on the box; 4000h: a right click leaves) and, made here: the texts'
//! far pointer (+22), their count (+26), the colours (+28, +2B..+2E), the
//! rows shown (+37), the line picked (+39), the first line shown (+3B),
//! the lines in all (+3D), the lines' x (+3F), their drawing (+41: one of
//! 9E06, 5364, 9370 by the flags, from the table at B7FE), the scroll
//! bar's background's buffer (+45) and the lines' (+49), the bar's knob
//! (+4D: x, y, width, height).
//!
//! The library's loop (its mouse regions, keys and scrolling) is the front
//! end's: it answers with the line picked (`pick`, FFFFh none), which the
//! port takes as the original's loop leaves it for a pick made among the
//! lines first shown (the line clicked made the record's pick, 3159:0A1D,
//! 0BCA; the loop's last drawing, 0A58, then the window's end, 07D3). The
//! lines' and the panel's pixels are the front end's too, but for the
//! panel's picture, which the page keeps.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// The library's flags (B7C4): bit 0 the line under a click was the one
/// picked already.
const FLAGS: u16 = 0xB7C4;
/// The window's record's far pointer (B80A).
const RECORD: u16 = 0xB80A;
/// The font's height (B7F4), the regions' routines (B7F6..B7FD), the
/// lines' drawings (B7FE: three far pointers).
const ROW_HEIGHT: u16 = 0xB7F4;
const REGIONS: u16 = 0xB7F6;
const DRAWINGS: u16 = 0xB7FE;
/// What the last drawing scrolled (B7F2: 0 none).
const SCROLLED: u16 = 0xB7F2;

impl Machine {
    /// The strings' segment for a list of kind `kind`: the frame's
    /// ([9030]) for the store's lists of weapons and armour (kinds 1 and
    /// 2, 1194:F7E9 and F866) and the guild's of spells (7, 1194:B351),
    /// whose lines are made there; DGROUP for the others.
    pub fn list_segment(&self, kind: u16) -> u16 {
        match kind {
            1 | 2 | 7 => self.mem.r16(DS, crate::mif::var::BUFFER_SEG),
            _ => DS,
        }
    }

    /// 1194:5734: a list of kind `kind` (AX) of `cx` strings at `ds`:`si`,
    /// its title at `ds`:`di` (A872, written with the caller's DS), the
    /// line picked `pick` (the library's loop's answer; FFFFh none).
    /// Returns AX as the original leaves it. None where the port gives up
    /// (a kind not ported, no room on the heap).
    pub fn list_window(&mut self, files: &mut Files, kind: u16, (ds, si): (u16, u16), cx: u16, di: u16, pick: u16) -> Option<u16> {
        self.mem.w16(ds, 0xA872, di);
        let flags = self.mem.r8(DS, FLAGS);
        self.mem.w8(DS, FLAGS, 0);
        self.mem.w16(DS, 0xA892, si);
        let load = self.mem.r16(DS, 0x1030);
        let before = self.map_bank(load, 0);
        self.mem.w16(DS, 0xA894, before);
        // The page kept in the load bank (3E80h dwords from [A8FE]).
        self.page_copy(true);
        let (panel, record, bx) = match kind {
            3 => {
                let f = self.mem.r16(DS, 0x5006 + 0x20) | 0x4000;
                self.mem.w16(DS, 0x5006 + 0x20, f);
                (0x4F4C, 0x5006, 0xD5)
            }
            5 => {
                let f = self.mem.r16(DS, 0x5006 + 0x20) | 0x4000;
                self.mem.w16(DS, 0x5006 + 0x20, f);
                (0x4F82, 0x5006, 0xFD)
            }
            0 => (0x4F4C, 0x5006, 0xFD),
            2 => (0x4F6C, 0x50B0, 0xFD),
            4 => (0x4F77, 0x5006, 0xFD),
            7 => (0x4F8F, 0x50B0, 0xFD),
            8 => (0x4F9A, 0x5006, 0xEE),
            0xB => (0x4FB1, 0x5006, 0xFD),
            1 => (0x4F61, 0x50B0, 0xFD),
            _ => return crate::giveup::not_ported("1194:5734: a list kind with a title or a picture of its own (6, 9, Ah)"),
        };
        // 5938: the panel's picture into bank 17h at the frame's start and
        // drawn (0000:56C8 with BX 14h).
        self.map_bank(0x17, 0);
        self.panel_load(files, panel)?;
        // 594A: the bank before the list mapped; the record filled, +2B
        // DL as the panel's drawing leaves DX (3C0h for each panel the
        // shops show, measured against the original; kind 8's F0h set
        // before it is lost).
        let a894 = self.mem.r16(DS, 0xA894);
        self.map_bank(a894, 0);
        let dl = 0xC0u8;
        self.mem.w16(DS, record + 0x22, si);
        self.mem.w16(DS, record + 0x24, ds);
        self.mem.w16(DS, record + 0x26, cx);
        self.mem.w8(DS, record + 0x28, bx as u8);
        self.mem.w8(DS, record + 0x2B, dl);
        self.mem.w8(DS, record + 0x2C, 0xF9);
        self.mem.w8(DS, record + 0x2D, 0xF2);
        self.mem.w8(DS, record + 0x2E, 0xF5);
        let ax = self.window_list((DS, record), pick)?;
        // 5988: a pick past the count (or none) the last line's.
        let mut al = ax as u8;
        let count = cx as u8;
        if count <= al {
            al = count.wrapping_sub(1);
        }
        let ax = (ax & 0xFF00) | al as u16;
        // 5996: the page back from the load bank, the bank before mapped.
        let old = self.map_bank(load, 0);
        self.page_copy(false);
        self.map_bank(old, 0);
        self.mem.w8(DS, FLAGS, flags);
        Some(ax)
    }

    /// 2542:06F8 (24FA:0B78 the same): the list in the bar's corner over
    /// the record at 5AF4 of `cx` strings at `seg`:`si` (the use list's,
    /// the spellbook's), the line `pick` picked (the library's loop's
    /// answer; FFFFh none): the record's texts and colours (+22..+2E:
    /// FDh, FEh, F9h, F2h, F5h), TEENYFNT ([102E]) selected, the cursor
    /// put back (541E), bank [1032] mapped and the page kept in it, the
    /// list (3159:06A0), the bank mapped again and the page put back,
    /// A902 bit 0 off. Returns AX, the record's pick. `screen`: the page
    /// as the front end last showed it (the port's page holds no frame's
    /// pixels), kept in the bank in its place. The page's showing (9466)
    /// is the front end's. None where the list gives up.
    pub fn bar_list(&mut self, screen: Option<&[u8]>, (seg, si): (u16, u16), cx: u16, pick: u16) -> Option<u16> {
        const BAR: u16 = 0x5AF4;
        self.mem.w16(DS, BAR + 0x22, si);
        self.mem.w16(DS, BAR + 0x24, seg);
        self.mem.w16(DS, BAR + 0x26, cx);
        for (o, v) in [(0x28u16, 0xFDu8), (0x2B, 0xFE), (0x2C, 0xF9), (0x2D, 0xF2), (0x2E, 0xF5)] {
            self.mem.w8(DS, BAR + o, v);
        }
        self.select_font(0x102E);
        let f = self.mem.r16(DS, 0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        let bank = self.mem.r16(DS, 0x1032);
        self.map_bank(bank, 0);
        match screen {
            Some(px) => {
                let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
                for (i, &b) in px.iter().take(0xFA00).enumerate() {
                    self.mem.w8(frame, i as u16, b);
                }
            }
            None => self.page_copy(true),
        }
        let ax = self.window_list((DS, BAR), pick)?;
        self.map_bank(bank, 0);
        self.page_copy(false);
        let f = self.mem.r16(DS, 0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        Some(ax)
    }

    /// The page ([A8FE], FA00h bytes) to the frame's start (`to_frame`), or
    /// back.
    fn page_copy(&mut self, to_frame: bool) {
        let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, 0xA900));
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        for i in 0..0xFA00u16 {
            if to_frame {
                let b = self.mem.r8(seg, off.wrapping_add(i));
                self.mem.w8(frame, i, b);
            } else {
                let b = self.mem.r8(frame, i);
                self.mem.w8(seg, off.wrapping_add(i), b);
            }
        }
    }

    /// 0000:56C8 with BX 14h: the picture named at DS:`si` loaded to the
    /// frame's start (0000:742C), its header's words to A836..A83C, drawn
    /// into the page (582C; its colours not taken: bit 10h).
    pub(crate) fn panel_load(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        self.file_load(files, si, frame, 0)?;
        for k in 0..4u16 {
            let v = self.mem.r16(frame, 2 * k);
            self.mem.w16(DS, 0xA836 + 2 * k, v);
        }
        self.page_image(frame, 0);
        Some(())
    }

    /// 3159:06A0: the window library's list over the record at
    /// `seg`:`rec` (B80A), the line `pick` picked among the lines first
    /// shown (FFFFh: left). AX: the record's pick (+39).
    fn window_list(&mut self, (seg, rec): (u16, u16), pick: u16) -> Option<u16> {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        self.mem.w16(DS, RECORD, rec);
        self.mem.w16(DS, RECORD + 2, seg);
        let f = self.mem.r8(DS, FLAGS) & 0xFE;
        self.mem.w8(DS, FLAGS, f);
        // 0718: the regions' routines, the pick and the first line, the
        // lines' x and drawing, the rows and the lines in all.
        for (k, v) in [0x09C2u16, 0x09E8, 0x0D95, 0x0A1D].into_iter().enumerate() {
            self.mem.w16(DS, REGIONS + 2 * k as u16, v);
        }
        self.mem.w16(seg, rec + 0x3B, 0);
        self.mem.w16(seg, rec + 0x39, 0xFFFF);
        let flags = r16(self, 0x20);
        let mut x = r16(self, 0x18);
        let mut table = 0u16;
        if flags & 0x1000 != 0 {
            table = 4;
            x = x.wrapping_add(r16(self, 0x1C) >> 1);
        } else if flags & 0x2000 != 0 {
            table = 8;
            x = x.wrapping_add(r16(self, 0x1C));
        }
        self.mem.w16(seg, rec + 0x3F, x);
        let draw = self.mem.r32(DS, DRAWINGS + table);
        self.mem.w32(seg, rec + 0x41, draw);
        let (foff, fseg) = (self.mem.r16(DS, 0xB034), self.mem.r16(DS, 0xB036));
        let height = self.mem.r8(fseg, foff) as u16;
        self.mem.w16(DS, ROW_HEIGHT, height);
        let rows = if height == 0 { return crate::giveup::original("3159:0789: a font of no height (a divide by 0)") } else { r16(self, 0x1E) / height };
        if rows > 0xFF {
            return crate::giveup::original("3159:0789: the rows overflow AL (a divide overflow)");
        }
        self.mem.w16(seg, rec + 0x37, rows & 0xFF);
        let total = r16(self, 0x26).wrapping_add(self.mem.r16(seg, rec.wrapping_add(0x29)));
        self.mem.w16(seg, rec + 0x3D, total);
        let (bx, bw, bh) = (r16(self, 0x1A), r16(self, 0x1C), r16(self, 0x1E));
        if bw == 0 {
            // 07A9: no box: carry set, the loop never runs.
            return crate::giveup::not_ported("3159:07D1: a list window with no box");
        }
        // 07B4: what the lines' box covers kept on the heap (8F2A, DI
        // FFFEh).
        let lines = self.page_rect_heap(r16(self, 0x18), bx, bw, bh)?;
        self.mem.w16(seg, rec + 0x49, 0);
        self.mem.w16(seg, rec + 0x4B, lines);
        // 0C75: the scroll bar's background kept and its knob made.
        if r16(self, 0x14) != 0 {
            let (w, h) = (r16(self, 0x14), r16(self, 0x16));
            let buf = self.heap_alloc_high(w.wrapping_mul(h)).filter(|&s| s != 0).or_else(|| crate::giveup::not_ported("0000:9CD2: no room for the scroll bar's background"))?;
            self.mem.w16(seg, rec + 0x45, 0);
            self.mem.w16(seg, rec + 0x47, buf);
            let (kx, ky) = (r16(self, 0x10), r16(self, 0x12));
            self.mem.w16(seg, rec + 0x4D, kx);
            self.mem.w16(seg, rec + 0x4F, ky);
            self.mem.w16(seg, rec + 0x51, w);
            self.mem.w16(seg, rec + 0x53, h);
            self.page_rect_save((buf, 0), kx, ky, w, h);
            let (total, rows) = (r16(self, 0x3D), r16(self, 0x37));
            if total > rows {
                let knob = ((h as u32 / total as u32) as u16).wrapping_mul(rows);
                let rem = (h as u32 % total as u32) as u16;
                let extra = ((rem as u32 * rows as u32) / total as u32) as u16;
                let v = extra.wrapping_add(knob).max(3);
                self.mem.w16(seg, rec + 0x53, v);
            }
        }
        // 07C6: the first line picked (0BCA: no routine at +2F here).
        self.list_pick(seg, rec, 0)?;
        // The loop: the line clicked picked (0A1D, 0BCA), the last
        // drawing (0A58) and the end (07D3, 710E).
        self.mem.w8(DS, SCROLLED, 0);
        let ax = if pick == 0xFFFF {
            self.mem.w16(seg, rec + 0x39, 0xFFFF);
            0xFFFF
        } else {
            self.list_pick(seg, rec, pick)?;
            pick
        };
        self.list_drawn(seg, rec);
        self.list_end(seg, rec);
        self.key_flush();
        Some(ax)
    }

    /// 3159:0BCA: the record's pick `ax` (+39), its routine at +2F called
    /// where it changes.
    fn list_pick(&mut self, seg: u16, rec: u16, ax: u16) -> Option<()> {
        if self.mem.r16(seg, rec + 0x39) == ax {
            return Some(());
        }
        self.mem.w16(seg, rec + 0x39, ax);
        if self.mem.r32(seg, rec + 0x2F) != 0 {
            return crate::giveup::not_ported("3159:0BDF: a list's routine for a new pick");
        }
        Some(())
    }

    /// 3159:0A58, the state its drawing leaves: the lines' box put back
    /// (599C, its pixels the front end's), and of the lines shown from the
    /// first (+3B, at most the rows), each one's tab stops (9 and three
    /// digits) written at A836 of the texts' segment (0B07: DS is theirs),
    /// the last stop's left. The colour (9288) and the texts' drawing are
    /// the front end's.
    fn list_drawn(&mut self, seg: u16, rec: u16) {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        let (tseg, mut at) = (r16(self, 0x24), r16(self, 0x22));
        let colour = if r16(self, 0x26) == 0 { self.mem.r8(seg, rec + 0x2B) } else { self.mem.r8(seg, rec + 0x28) };
        self.mem.w8(DS, 0x9288, colour);
        for _ in 0..r16(self, 0x3B) {
            while self.mem.r8(tseg, at) != 0 {
                at = at.wrapping_add(1);
            }
            at = at.wrapping_add(1);
        }
        let mut n = r16(self, 0x3D).wrapping_sub(r16(self, 0x3B));
        if r16(self, 0x37) < n {
            n = r16(self, 0x37);
        }
        for _ in 0..n {
            let mut p = at;
            loop {
                let c = self.mem.r8(tseg, p);
                if c == 0 {
                    break;
                }
                if c == 9 {
                    for k in 0..3u16 {
                        let d = self.mem.r8(tseg, p.wrapping_add(1 + k));
                        self.mem.w8(tseg, 0xA836 + k, d);
                    }
                    self.mem.w8(tseg, 0xA839, 0);
                    p = p.wrapping_add(4);
                    continue;
                }
                p = p.wrapping_add(1);
            }
            at = p.wrapping_add(1);
        }
    }

    /// 3159:07D3: the window's end: with flag 1 what the lines' box and
    /// the scroll bar covered put back (599C), the bar's buffer and the
    /// lines' freed (0000:6C32).
    fn list_end(&mut self, seg: u16, rec: u16) {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        let bar = r16(self, 0x14) != 0;
        if r16(self, 0x20) & 1 != 0 {
            let (x, y, w, h) = (r16(self, 0x18), r16(self, 0x1A), r16(self, 0x1C), r16(self, 0x1E));
            let buf = r16(self, 0x4B);
            self.page_rect_put((buf, r16(self, 0x49)), x, y, w, h);
            if bar {
                let (x, y, w, h) = (r16(self, 0x10), r16(self, 0x12), r16(self, 0x14), r16(self, 0x16));
                let buf = r16(self, 0x47);
                self.page_rect_put((buf, r16(self, 0x45)), x, y, w, h);
            }
        }
        if bar || r16(self, 0x20) & 1 == 0 {
            let s = r16(self, 0x47);
            self.heap_free(s);
        }
        let s = r16(self, 0x4B);
        self.heap_free(s);
    }

    /// 0000:8F2A with DI FFFEh: a heap block (9CD6) for the page's `w` by
    /// `h` box at (`x`, `y`), the rows copied into it (A8FC 1). Its
    /// segment.
    fn page_rect_heap(&mut self, x: u16, y: u16, w: u16, h: u16) -> Option<u16> {
        let seg = self.heap_alloc_high(w.wrapping_mul(h)).filter(|&s| s != 0).or_else(|| crate::giveup::not_ported("0000:9CD6: no room for a list's background"))?;
        self.page_rect_save((seg, 0), x, y, w, h);
        self.mem.w16(DS, 0xA8FC, 1);
        Some(seg)
    }
}
