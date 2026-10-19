//! The list windows as the game's state sees them: 1194:5734 (the screen
//! kept in the load bank, the panel's picture or a list's own title and
//! picture, the window's record filled) and the window library's list (3159:06A0: its record's words,
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

/// One read of the mouse by the list's loop (0000:8FC4): its buttons
/// (AFF9: bit 0 left, 1 right), the cursor's place (8F92, 8F94), and the
/// list's keys held then (`list_keys`), as the front end saw them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListRead {
    pub buttons: u8,
    pub x: u16,
    pub y: u16,
    pub keys: u8,
}

/// The keys the list's loop looks at in the key table (ACB5 by scan
/// code): `ListRead::keys`' bits and their scan codes.
pub mod list_keys {
    pub const UP: (u8, u8) = (1, 0x48);
    pub const DOWN: (u8, u8) = (2, 0x50);
    pub const PGUP: (u8, u8) = (4, 0x49);
    pub const PGDN: (u8, u8) = (8, 0x51);
    pub const HOME: (u8, u8) = (0x10, 0x47);
    pub const END: (u8, u8) = (0x20, 0x4F);
    pub const ENTER: (u8, u8) = (0x40, 0x1C);
    pub const ESC: (u8, u8) = (0x80, 0x01);
    pub const SCANS: [(u8, u8); 8] = [UP, DOWN, PGUP, PGDN, HOME, END, ENTER, ESC];
}

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
    /// Kinds 0..5, 7, 8 and Bh (any past Bh is 1's) are the shops' panels
    /// over the records at 5006 and 50B0; 6 (the creation's classes), 9
    /// ("Where is", with a picture) and Ah (a room, a cure) draw their own
    /// titles. Returns AX as the original leaves it. None where the port
    /// gives up (no room on the heap; kind 9's picture packed or cut by
    /// the clip).
    pub fn list_window(&mut self, files: &mut Files, kind: u16, (ds, si): (u16, u16), cx: u16, di: u16, pick: u16) -> Option<u16> {
        self.list_window_scrolled(files, kind, (ds, si), cx, di, pick, 0)
    }

    /// `list_window`, the library's loop having scrolled its lines to
    /// first show line `top` (+3B) before the pick (`pick` among all
    /// the lines).
    #[allow(clippy::too_many_arguments)]
    pub fn list_window_scrolled(&mut self, files: &mut Files, kind: u16, (ds, si): (u16, u16), cx: u16, di: u16, pick: u16, top: u16) -> Option<u16> {
        self.mem.w16(ds, 0xA872, di);
        let flags = self.mem.r8(DS, FLAGS);
        self.mem.w8(DS, FLAGS, 0);
        self.mem.w16(DS, 0xA892, si);
        let load = self.mem.r16(DS, 0x1030);
        let before = self.map_bank(load, 0);
        self.mem.w16(DS, 0xA894, before);
        // The page kept in the load bank (3E80h dwords from [A8FE]).
        self.page_copy(true);
        let (record, bx, dl) = match kind {
            6 => {
                // 58D7: POPUP2's panel (its bank 17h mapped and the bank
                // before again), the title (A872) in colour 1 centred on
                // x A0h at row 20h in the caller's font (5364), the lines'
                // font [1026]; DL FDh.
                let old = self.map_bank(0x17, 0);
                self.panel_load(files, 0x4F56)?;
                self.map_bank(old, 0);
                self.mem.w8(DS, 0x9288, 1);
                let title = self.mem.r16(DS, 0xA872);
                self.title_centred(title, 0xA0, 0x20);
                self.select_font(0x1026);
                (0x515A, 0xFD, 0xFD)
            }
            9 => {
                // 5884: the picture named at 4FA5 loaded into bank 19h at
                // [902E] (0000:7428) and drawn (5828); the bank 19h's
                // mapping found made [1030] and A894 both (the load bank:
                // bug kept, the list's end leaves it mapped, not the bank
                // before), 5CA bit 2000h off, D39 bit 80h on. The lines'
                // colour 65h; DL as that drawing leaves DX.
                let old = self.map_bank(0x19, 0);
                let (frame, at) = (self.mem.r16(DS, 0x9030), self.mem.r16(DS, 0x902E));
                self.file_load(files, 0x4FA5, frame, at)?;
                let dx = self.image_drawn_dx(frame, at)?;
                self.page_image(frame, at);
                self.mem.w16(DS, 0x1030, old);
                self.mem.w16(DS, 0xA894, old);
                let f = self.mem.r16(DS, 0x05CA) & 0xDFFF;
                self.mem.w16(DS, 0x05CA, f);
                let f = self.mem.r8(DS, 0x0D39) | 0x80;
                self.mem.w8(DS, 0x0D39, f);
                (0x5105, 0x65, dx as u8)
            }
            0xA => {
                // 57F5: the title's font [102C] ([1026] with 137F 4), the
                // panel at 4FB1 (bank 17h mapped and the bank before
                // again), the title in colour 60h centred on x A0h at row
                // 1Fh, the lines' font the same; DL as 56C8 leaves DX (its
                // last background's segment, A840: 5C86, 5364, 9466 and
                // 9AE6 keep it).
                let dungeon = self.mem.r8(DS, 0x137F) == 4;
                self.select_font(0x1026);
                if !dungeon {
                    self.select_font(0x102C);
                }
                let old = self.map_bank(0x17, 0);
                self.panel_load(files, 0x4FB1)?;
                self.map_bank(old, 0);
                self.mem.w8(DS, 0x9288, 0x60);
                let title = self.mem.r16(DS, 0xA872);
                self.title_centred(title, 0xA0, 0x1F);
                self.select_font(0x102C);
                if dungeon {
                    self.select_font(0x1026);
                }
                (0x505B, 0xFD, self.mem.r8(DS, 0xA840))
            }
            _ => {
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
                    _ => (0x4F61, 0x50B0, 0xFD),
                };
                // 5938: the panel's picture into bank 17h at the frame's
                // start and drawn (0000:56C8 with BX 14h); DL as 56C8
                // leaves DX: the high word of A83E's dword (A840, the last
                // background it kept; kind 8's F0h set before it is lost).
                self.map_bank(0x17, 0);
                self.panel_load(files, panel)?;
                (record, bx, self.mem.r8(DS, 0xA840))
            }
        };
        // 594A: the bank before the list (A894) mapped; the record filled.
        let a894 = self.mem.r16(DS, 0xA894);
        self.map_bank(a894, 0);
        self.mem.w16(DS, record + 0x22, si);
        self.mem.w16(DS, record + 0x24, ds);
        self.mem.w16(DS, record + 0x26, cx);
        self.mem.w8(DS, record + 0x28, bx as u8);
        self.mem.w8(DS, record + 0x2B, dl);
        self.mem.w8(DS, record + 0x2C, 0xF9);
        self.mem.w8(DS, record + 0x2D, 0xF2);
        self.mem.w8(DS, record + 0x2E, 0xF5);
        let ax = self.window_list((DS, record), pick, top)?;
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
    /// as the frame's input step finds it, from the front end (the
    /// port's page holds no frame's pixels: the drawing's rows over the
    /// interface, before the compass), kept in the bank in its place. The
    /// page's showing (9466) is the front end's. None where the list gives
    /// up.
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
        let ax = self.window_list((DS, BAR), pick, 0)?;
        self.map_bank(bank, 0);
        self.page_copy(false);
        let f = self.mem.r16(DS, 0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        Some(ax)
    }

    /// 2542:056A..05B1 (24FA:09EA the same) by its list, up to the cast:
    /// 100E 9, the book's names listed (29CF:060B) and the line `pick`
    /// picked (FFFFh none; `screen` as `bar_list`'s), and the spell picked
    /// kept at 5CAE. Returns the spell's number for the cast (29CF:071F,
    /// `Machine::cast_spell`), None where nothing was picked. (Shift+C,
    /// the last spell again without the list, is `spellbook_last`.) None
    /// outside where the list gives up.
    pub fn spellbook_list(&mut self, files: &mut Files, screen: Option<&[u8]>, pick: u16) -> Option<Option<u8>> {
        const BOOK: u16 = 0x01A4 + 0x367;
        self.mem.w8(DS, 0x100E, 9);
        let cx = self.mem.r8(DS, 0x01A4 + 0x366) as u16;
        let ax = self.book_names_listed(files, screen, BOOK, cx, pick)?;
        if (ax as i16) < 0 {
            return Some(None);
        }
        self.mem.w8(DS, 0x5CAE, ax as u8);
        Some(Some(self.mem.r8(DS, BOOK.wrapping_add(ax))))
    }

    /// 2542:056A..059B (24FA:09EA the same) without the list: 100E 9; with
    /// C held and a Shift (the key table at ACB5: 2Eh with 2Ah or 36h), or
    /// the right button's release (7925, which the caller gives as 2826's
    /// poll leaves it where the bar's click is taken: `released_right`),
    /// the spell picked last (5CAE) again unless none (FFh). Every caller
    /// leaves AH 0 (the C key's AX 2Eh from 05B8, the bar's AX the click's
    /// x, 5Bh..72h), so the pick is 5CAE alone: kept at 5CAE and its
    /// spell's number returned for the cast (29CF:071F). None: the list is
    /// asked for (`spellbook_list`).
    pub fn spellbook_last(&mut self, released_right: bool) -> Option<u8> {
        const BOOK: u16 = 0x01A4 + 0x367;
        use crate::controls::var::KEYS;
        self.mem.w8(DS, 0x100E, 9);
        let held = |m: &Machine, scan: u16| m.mem.r8(DS, KEYS + scan) != 0;
        let shifted = held(self, 0x2E) && (held(self, 0x2A) || held(self, 0x36));
        if !shifted && !released_right {
            return None;
        }
        let al = self.mem.r8(DS, 0x5CAE);
        if al == 0xFF {
            return None;
        }
        self.mem.w8(DS, 0x5CAE, al);
        Some(self.mem.r8(DS, BOOK.wrapping_add(al as u16)))
    }

    /// 29CF:060B: the names of the `cx` spells numbered at DS:`si` copied
    /// to 929E (29CF:0654: from the spells' records, 3276:34h on, the
    /// player's own, 80h and up, from 2AB4h, 55h bytes each; a NUL after
    /// the last), then with [8036] 9 sorted (0000:0B9C) and listed in the
    /// bar's corner (2542:06F8), the line picked read back through the
    /// order 0B9C left at 1044; otherwise the list of kind [8036]
    /// (1194:5734) over them as they are. FFFFh with no spells.
    fn book_names_listed(&mut self, files: &mut Files, screen: Option<&[u8]>, si: u16, cx: u16, pick: u16) -> Option<u16> {
        if cx == 0 {
            return Some(0xFFFF);
        }
        let mut di = 0x929Eu16;
        for k in 0..cx {
            let n = self.mem.r8(DS, si.wrapping_add(k));
            let (seg, rec) = if n & 0x80 != 0 { (0x3276u16, 0x2AB4u16) } else { (0x3276, 0x0034) };
            let mut from = rec.wrapping_add((n & 0x7F) as u16 * 0x55);
            loop {
                let c = self.mem.r8(seg, from);
                self.mem.w8(DS, di, c);
                from = from.wrapping_add(1);
                di = di.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        }
        self.mem.w8(DS, di, 0);
        let kind = self.mem.r16(DS, 0x8036);
        if kind != 9 {
            return self.list_window(files, kind, (DS, 0x929E), cx, di, pick);
        }
        let names = self.names_sorted(0x929E, cx);
        let ax = self.bar_list(screen, (DS, names), cx, pick)?;
        if (ax as i16) < 0 {
            return Some(ax);
        }
        Some(self.mem.r8(DS, 0x1044u16.wrapping_add(ax)) as u16)
    }

    /// 0000:0B9C: the `cx` names at DS:`si` sorted: with more than one,
    /// 1044.. their order (0, 1, ...), a table of pointers to them at
    /// 929E+800h (0C36), the table and the order swapped as 0BC6's
    /// exchange sort compares the names (9C0A, A85E and A860 its two
    /// places), then the names copied in the table's order after the
    /// table. Returns where they are (with one, `si` itself, 1044 as it
    /// was: bug kept, the pick read through it unsorted).
    fn names_sorted(&mut self, si: u16, cx: u16) -> u16 {
        if cx <= 1 {
            return si;
        }
        let table = 0x929Eu16 + 0x800;
        for k in 0..cx {
            self.mem.w8(DS, 0x1044 + k, k as u8);
        }
        let mut at = si;
        for k in 0..cx {
            self.mem.w16(DS, table + 2 * k, at);
            while self.mem.r8(DS, at) != 0 {
                at = at.wrapping_add(1);
            }
            at = at.wrapping_add(1);
        }
        let end = table + 2 * cx;
        self.mem.w16(DS, 0xA85E, 0);
        for i in 0..cx - 1 {
            self.mem.w16(DS, 0xA860, i + 1);
            for j in i + 1..cx {
                let a = self.mem.r16(DS, table + 2 * i);
                let b = self.mem.r16(DS, table + 2 * j);
                if self.name_after(a, b) {
                    self.mem.w16(DS, table + 2 * i, b);
                    self.mem.w16(DS, table + 2 * j, a);
                    let (x, y) = (self.mem.r8(DS, 0x1044 + i), self.mem.r8(DS, 0x1044 + j));
                    self.mem.w8(DS, 0x1044 + i, y);
                    self.mem.w8(DS, 0x1044 + j, x);
                }
                self.mem.w16(DS, 0xA860, j + 1);
            }
            self.mem.w16(DS, 0xA85E, i + 1);
        }
        let mut di = end;
        for k in 0..cx {
            let mut from = self.mem.r16(DS, table + 2 * k);
            loop {
                let c = self.mem.r8(DS, from);
                self.mem.w8(DS, di, c);
                from = from.wrapping_add(1);
                di = di.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        }
        end
    }

    /// 0000:9C0A with SI `a`, DI `b`: their bytes compared over `b`'s
    /// length and its NUL; whether the first that differ has `a`'s the
    /// greater, as a byte's difference with its sign clear (AX FFFFh).
    fn name_after(&self, a: u16, b: u16) -> bool {
        let mut n = 0u16;
        while self.mem.r8(DS, b.wrapping_add(n)) != 0 {
            n = n.wrapping_add(1);
        }
        for k in 0..=n {
            let (x, y) = (self.mem.r8(DS, a.wrapping_add(k)), self.mem.r8(DS, b.wrapping_add(k)));
            if x != y {
                let d = x.wrapping_sub(y);
                return d & 0x80 == 0;
            }
        }
        false
    }

    /// 0000:5364: the title at DS:`si` centred on x `ax` at row `bx` (its
    /// width, 66F6, halved off) in colour [9288] and the font selected
    /// (9E0A: into the page where it is the game's own block).
    fn title_centred(&mut self, si: u16, ax: u16, bx: u16) {
        let x = ax.wrapping_sub(self.text_width(si) >> 1);
        self.page_text_at(x, bx, (DS, si));
    }

    /// DX as 0000:582C leaves it, drawing the picture whose header is at
    /// `seg`:`off`: with the capture pointer (8F9A) set, its segment
    /// (8F9C); else, unpacked, its width where it is drawn (9207 set, or
    /// wholly inside the clip, A914..A91A: 59A4 and 5B08 swap it into DX)
    /// and its height where it lies wholly outside. None (not ported) for
    /// a packed one (its buffer's 51EC and 6C36 leave DX) or one the clip
    /// cuts (59D8, 5A6A).
    fn image_drawn_dx(&mut self, seg: u16, off: u16) -> Option<u16> {
        if self.mem.r32(DS, 0x8F9A) != 0 {
            return Some(self.mem.r16(DS, 0x8F9C));
        }
        let r = |m: &Machine, o: u16| m.mem.r16(seg, off.wrapping_add(o));
        if r(self, 8) & 0xF != 0 {
            return crate::giveup::not_ported("0000:582C: DX after a packed picture's drawing (its buffer's 51EC and 6C36)");
        }
        let (x, y, w, h) = (r(self, 0) as i16, r(self, 2) as i16, r(self, 4), r(self, 6));
        let clip = |m: &Machine, o: u16| m.mem.r16(DS, o) as i16;
        if self.mem.r8(DS, 0x9207) != 0 {
            return Some(w);
        }
        let (right, bottom) = (x.wrapping_add(w as i16), y.wrapping_add(h as i16));
        if x > clip(self, 0xA918) || y > clip(self, 0xA91A) || bottom < clip(self, 0xA916) || right < clip(self, 0xA914) {
            return Some(h);
        }
        if y >= clip(self, 0xA916) && bottom <= clip(self, 0xA91A) && x >= clip(self, 0xA914) && right <= clip(self, 0xA918) {
            return Some(w);
        }
        crate::giveup::not_ported("0000:582C: DX after a picture the clip cuts (59D8, 5A6A)")
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
    /// `seg`:`rec` (B80A), its lines scrolled to first show line `top`,
    /// the line `pick` picked (FFFFh: left). AX: the record's pick (+39).
    fn window_list(&mut self, (seg, rec): (u16, u16), pick: u16, top: u16) -> Option<u16> {
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
        // The loop itself, over the front end's reads where it gave them
        // (run on a copy: reads that end before the loop does leave the
        // pick as given, below).
        if let Some(reads) = self.list_reads.take() {
            let mut run = self.clone();
            let ax = run.list_loop(seg, rec, &reads);
            if std::env::var_os("ARENA_LIST_DEBUG").is_some() {
                eprintln!("list loop over {} reads: {:?} (pick given {:04X}, top {})", reads.len(), ax, pick, top);
            }
            if let Some(ax) = ax {
                *self = run;
                return Some(ax);
            }
        }
        // The loop's scrolling (the arrows' 09C2 and 09E8, the keys' and
        // the bar's, each through 0E68) as it leaves the record: the first
        // line shown, the pick kept among the lines shown, the knob.
        if top != 0 {
            self.list_scrolled(seg, rec, top)?;
        }
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

    /// 3159:07CB..0717: the list's loop over the front end's `reads`, from
    /// its first line picked: 0834 once as the start runs it (what it
    /// comes to not taken), then each pass 0834 and, until it ends, the
    /// pass's end (06EB: the mouse read, 8FC4; the cursor drawn and put
    /// back, 5454 and 541E; the page shown, 9466, and two retraces waited,
    /// 6BDE, the front end's). Its end (0709): the window's (07D3) and
    /// the keys flushed (710E). AX: the record's pick (FFFFh left). None
    /// where the reads end before the loop does, or the port gives up.
    fn list_loop(&mut self, seg: u16, rec: u16, reads: &[ListRead]) -> Option<u16> {
        let mut reads = reads.iter();
        self.list_input(seg, rec, &mut reads)?;
        let ax = loop {
            if let Some(ax) = self.list_input(seg, rec, &mut reads)? {
                break ax;
            }
            self.list_read(reads.next()?);
            self.cursor_drawn();
            self.cursor_hidden();
        };
        self.list_end(seg, rec);
        self.key_flush();
        Some(ax)
    }

    /// 0000:8FC4 as the list's loop reads the mouse: the buttons (AFF9) and
    /// the cursor's place (8F92, 8F94, kept within its limits) as `r`
    /// has them, and the list's keys held (the key table at ACB5 by scan
    /// code, as the key interrupt has left it by then).
    fn list_read(&mut self, r: &ListRead) {
        self.mem.w8(DS, 0xAFF9, r.buttons);
        self.mem.w16(DS, 0x8F92, r.x);
        self.mem.w16(DS, 0x8F94, r.y);
        self.mouse_clamp();
        for (bit, scan) in list_keys::SCANS {
            self.mem.w8(DS, 0xACB5 + scan as u16, (r.keys & bit != 0) as u8);
        }
    }

    /// Whether the list's key `scan` is held (the key table at ACB5).
    fn list_key_held(&self, scan: u8) -> bool {
        self.mem.r8(DS, 0xACB5 + scan as u16) != 0
    }

    /// 3159:0834: one pass's input. With the left button down (AFF9 bit
    /// 0), the first of the record's four boxes the cursor is in (their
    /// right and bottom edges in: the arrows, the bar, the lines) has its
    /// region's routine (B7F6: 09C2, 09E8, 0D95, 0A1D), and B7C4 bit 0
    /// set by it the line picked. Otherwise B7F2 cleared and the keys:
    /// Down and Up as the arrows; PgDn (08A3) and PgUp (08E7) a page less
    /// one on (their ten retraces waited, the front end's); Home and End
    /// the first and the last; Enter (its release waited) the line marked
    /// picked; the right button or Esc (with the record's flag 4000h) the
    /// list left, once they and the CyberMan's button are let go (8FC4
    /// read meanwhile). Each pass ends in the lines' drawing (0A58).
    /// Some(Some(AX)) the loop's end (its pick, FFFFh left), Some(None)
    /// on; None where the reads end inside a wait, or the port gives up.
    fn list_input(&mut self, seg: u16, rec: u16, reads: &mut std::slice::Iter<ListRead>) -> Option<Option<u16>> {
        use list_keys::*;
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        if self.mem.r8(DS, 0xAFF9) & 1 != 0 {
            let (mx, my) = (self.mem.r16(DS, 0x8F92) as i16, self.mem.r16(DS, 0x8F94) as i16);
            for k in 0..4u16 {
                let (x, y, w, h) = (r16(self, 8 * k) as i16, r16(self, 8 * k + 2) as i16, r16(self, 8 * k + 4) as i16, r16(self, 8 * k + 6) as i16);
                if mx < x || mx > x.wrapping_add(w) || my < y || my > y.wrapping_add(h) {
                    continue;
                }
                match k {
                    0 => self.list_up(seg, rec)?,
                    1 => self.list_down(seg, rec)?,
                    2 => self.list_bar(seg, rec)?,
                    _ => self.list_line_clicked(seg, rec, reads)?,
                }
                let f = self.mem.r8(DS, FLAGS);
                if f & 1 != 0 {
                    self.mem.w8(DS, FLAGS, f & 0xFE);
                    return Some(Some(self.list_done(seg, rec)));
                }
                self.list_drawn(seg, rec);
                return Some(None);
            }
        }
        self.mem.w8(DS, SCROLLED, 0);
        if self.list_key_held(DOWN.1) {
            self.list_down(seg, rec)?;
        } else if self.list_key_held(UP.1) {
            self.list_up(seg, rec)?;
        } else if self.list_key_held(PGDN.1) {
            let ax = r16(self, 0x3B).wrapping_add(r16(self, 0x37)).wrapping_sub(1);
            self.list_lowest(seg, rec, ax)?;
        } else if self.list_key_held(PGUP.1) || self.list_key_held(HOME.1) {
            let ax = if self.list_key_held(PGUP.1) { r16(self, 0x3B).wrapping_sub(r16(self, 0x37)).wrapping_add(1) } else { 0 };
            let ax = if (ax as i16) < 0 { 0 } else { ax };
            self.mem.w16(seg, rec + 0x39, ax);
            self.list_scrolled(seg, rec, ax)?;
        } else if self.list_key_held(END.1) {
            let last = r16(self, 0x3D).wrapping_sub(1);
            self.mem.w16(seg, rec + 0x39, last);
            let ax = last.wrapping_add(1).wrapping_sub(r16(self, 0x37));
            self.list_lowest(seg, rec, ax)?;
        } else if self.list_key_held(ENTER.1) {
            // 0932: its release waited for (the key interrupt's).
            self.mem.w8(DS, 0xACB5 + ENTER.1 as u16, 0);
            return Some(Some(self.list_done(seg, rec)));
        } else if (self.mem.r8(DS, 0xAFF9) & 2 != 0 || self.list_key_held(ESC.1)) && r16(self, 0x20) & 0x4000 != 0 {
            // 0975: the mouse read until Esc and the right button are let
            // go (the CyberMan's button 20h none).
            loop {
                let r = reads.next()?;
                self.list_read(r);
                if !self.list_key_held(ESC.1) && self.mem.r8(DS, 0xAFF9) & 2 == 0 {
                    break;
                }
            }
            self.mem.w16(seg, rec + 0x39, 0xFFFF);
            return Some(Some(self.list_done(seg, rec)));
        }
        self.list_drawn(seg, rec);
        Some(None)
    }

    /// 3159:099C: the lines drawn (0A58), the loop's end with the record's
    /// pick (+39).
    fn list_done(&mut self, seg: u16, rec: u16) -> u16 {
        self.list_drawn(seg, rec);
        self.mem.r16(seg, rec + 0x39)
    }

    /// 3159:08BB: the first line shown made `ax`, but no lower than with
    /// the last at the foot (the last then marked), or the top while the
    /// lines all fit (the last marked); 0E68.
    fn list_lowest(&mut self, seg: u16, rec: u16, ax: u16) -> Option<()> {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        let bx = r16(self, 0x3D).wrapping_sub(r16(self, 0x37));
        let last = r16(self, 0x3D).wrapping_sub(1);
        let ax = if (bx as i16) < 0 {
            self.mem.w16(seg, rec + 0x39, last);
            0
        } else if ax > bx {
            self.mem.w16(seg, rec + 0x39, last);
            bx
        } else {
            ax
        };
        self.list_scrolled(seg, rec, ax)
    }

    /// 3159:09C2: the up arrow (B7F2 1): the line above marked, the lines
    /// a line up where it was the first shown.
    fn list_up(&mut self, seg: u16, rec: u16) -> Option<()> {
        self.mem.w8(DS, SCROLLED, 1);
        let ax = self.mem.r16(seg, rec + 0x39).wrapping_sub(1);
        if ax == 0xFFFF {
            return Some(());
        }
        let top = self.mem.r16(seg, rec + 0x3B);
        self.list_pick(seg, rec, ax)?;
        if ax < top {
            self.list_scrolled(seg, rec, top.wrapping_sub(1))?;
        }
        Some(())
    }

    /// 3159:09E8: the down arrow (B7F2 2): the line below marked (not past
    /// the last), the lines a line down where it would leave them, but
    /// not once the last shows.
    fn list_down(&mut self, seg: u16, rec: u16) -> Option<()> {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        self.mem.w8(DS, SCROLLED, 2);
        let mut ax = r16(self, 0x39).wrapping_add(1);
        if ax >= r16(self, 0x3D) {
            ax = ax.wrapping_sub(1);
        }
        let bx = r16(self, 0x3B).wrapping_add(r16(self, 0x37));
        if ax < bx {
            return self.list_pick(seg, rec, ax);
        }
        if bx.wrapping_add(1) > r16(self, 0x3D) {
            return Some(());
        }
        self.list_pick(seg, rec, ax)?;
        let top = r16(self, 0x3B).wrapping_add(1);
        self.list_scrolled(seg, rec, top)
    }

    /// 3159:0D95: the bar clicked: the knob's middle put at the cursor's
    /// y, kept to the bar (+4F; drawn, 0CFF), then the lines shown from
    /// its share of the bar (0DCE: in 1/1024ths, times one more than the
    /// lines, no lower than with the last at the foot; 0E68).
    fn list_bar(&mut self, seg: u16, rec: u16) -> Option<()> {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        if r16(self, 0x14) == 0 {
            return Some(());
        }
        let mut ax = self.mem.r16(DS, 0x8F94).wrapping_sub(r16(self, 0x53) >> 1);
        let top = r16(self, 0x12);
        if (ax as i16) <= (top as i16) {
            ax = top;
        }
        let foot = top.wrapping_add(r16(self, 0x16)).wrapping_sub(r16(self, 0x53));
        if ax > foot {
            ax = foot;
        }
        self.mem.w16(seg, rec + 0x4F, ax);
        let h = r16(self, 0x16) as u32;
        if h == 0 {
            return crate::giveup::original("3159:0DED: a bar of no height (a divide by 0)");
        }
        let share = ((ax.wrapping_sub(top) as u32) << 10) / h;
        let t = (share.wrapping_mul(r16(self, 0x3D) as u32 + 1) >> 10) as u16;
        let most = r16(self, 0x3D).wrapping_sub(r16(self, 0x37));
        self.list_scrolled(seg, rec, if t > most { most } else { t })
    }

    /// 3159:0A1D: the lines clicked: the line under the cursor's y counted
    /// from the first shown over the lines in all less one (its rows from
    /// the box's top, +1A, each the font's height; with one line it is
    /// taken as picked), B7C4 bit 0 set where it is the line marked
    /// already; it marked (0BCA); the mouse read until the left button
    /// is let go.
    fn list_line_clicked(&mut self, seg: u16, rec: u16, reads: &mut std::slice::Iter<ListRead>) -> Option<()> {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        let y = self.mem.r16(DS, 0x8F94);
        let cx = r16(self, 0x3D).wrapping_sub(1);
        let mut row = r16(self, 0x1A);
        let mut dx = r16(self, 0x3B);
        let height = self.mem.r16(DS, ROW_HEIGHT);
        if cx == 0 || {
            for _ in 0..cx {
                row = row.wrapping_add(height);
                if y > row {
                    dx = dx.wrapping_add(1);
                }
            }
            dx == r16(self, 0x39)
        } {
            let f = self.mem.r8(DS, FLAGS) | 1;
            self.mem.w8(DS, FLAGS, f);
        }
        self.list_pick(seg, rec, dx)?;
        loop {
            let r = reads.next()?;
            self.list_read(r);
            if self.mem.r8(DS, 0xAFF9) & 1 == 0 {
                return Some(());
            }
        }
    }

    /// 3159:0E68: the record's first line shown (+3B) made `ax`, the pick
    /// moved into the lines shown (0BCA: to the first, or the last), and
    /// the knob placed (0E11): at the first line's share of the bar's
    /// height (in 1/1024ths, from the bar's y, +12), or at the bar's foot
    /// with the last lines shown; changed, it is drawn (0CFF: 9288 and
    /// the texts' ADB6/ADB8 as its lines leave them, the front end's
    /// pixels; the loop's last drawing, 0A58, sets 9288 again).
    fn list_scrolled(&mut self, seg: u16, rec: u16, ax: u16) -> Option<()> {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        self.mem.w16(seg, rec + 0x3B, ax);
        let last = ax.wrapping_add(r16(self, 0x37)).wrapping_sub(1);
        if r16(self, 0x39) < ax {
            self.list_pick(seg, rec, ax)?;
        } else if r16(self, 0x39) > last {
            self.list_pick(seg, rec, last)?;
        }
        if r16(self, 0x14) == 0 {
            return Some(());
        }
        let total = r16(self, 0x3D) as u64;
        if total == 0 {
            return crate::giveup::original("3159:0E35: no lines (a divide by 0)");
        }
        let q = (((ax as u64) << 10) * r16(self, 0x16) as u64) / total;
        if q > 0xFFFF_FFFF {
            return crate::giveup::original("3159:0E35: the knob's place overflows EAX (a divide overflow)");
        }
        let y = (((q as u32) >> 10) as u16).wrapping_add(r16(self, 0x12));
        if r16(self, 0x4F) != y {
            let y = if ax == r16(self, 0x3D).wrapping_sub(r16(self, 0x37)) {
                r16(self, 0x12).wrapping_add(r16(self, 0x16)).wrapping_sub(r16(self, 0x53))
            } else {
                y
            };
            self.mem.w16(seg, rec + 0x4F, y);
        }
        Some(())
    }

    /// 3159:0BCA: the record's pick `ax` (+39), its routine at +2F called
    /// where it changes. (No record the game fills has one: 5006, 505B,
    /// 50B0, 5105, 515A and 5AF4 keep +2F 0; 0BDF isn't ported.)
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
    /// the last stop's left; the colour (9288) as the lines' colour codes
    /// and their count leave it. The texts' drawing is the front end's.
    fn list_drawn(&mut self, seg: u16, rec: u16) {
        let r16 = |m: &Machine, o: u16| m.mem.r16(seg, rec.wrapping_add(o));
        let (tseg, mut at) = (r16(self, 0x24), r16(self, 0x22));
        let colour = if r16(self, 0x26) == 0 { self.mem.r8(seg, rec + 0x2B) } else { self.mem.r8(seg, rec + 0x28) };
        self.mem.w8(DS, 0x9288, colour);
        // 0BAE after each line passed or drawn: BP, from the texts'
        // count (+26), counted down, and where it reaches 0 the colour
        // after them (+2B) taken.
        let mut bp = r16(self, 0x26);
        let mut line_done = |m: &mut Machine| {
            bp = bp.wrapping_sub(1);
            if bp == 0 {
                let c = m.mem.r8(seg, rec + 0x2B);
                m.mem.w8(DS, 0x9288, c);
            }
        };
        for _ in 0..r16(self, 0x3B) {
            while self.mem.r8(tseg, at) != 0 {
                at = at.wrapping_add(1);
            }
            at = at.wrapping_add(1);
            line_done(self);
        }
        let mut n = r16(self, 0x3D).wrapping_sub(r16(self, 0x3B));
        if r16(self, 0x37) < n {
            n = r16(self, 0x37);
        }
        for _ in 0..n {
            // 0AE2 by its segments from SI: a tab stop (9 and three digits)
            // at a segment's start written at A836 of the texts' segment;
            // the bytes after the segment's first (and after a stop's
            // digits, its fourth) looked at: 1 a colour code (its colour
            // taken, the segment after it next), Ah, Dh and 8 a segment
            // drawn (Dh Ah the pair passed), 0 the line's end, drawn
            // through +41 from the last segment's start, SI past its NUL.
            let mut start = at;
            loop {
                let mut p = start;
                if self.mem.r8(tseg, p) == 9 {
                    for k in 0..3u16 {
                        let d = self.mem.r8(tseg, p.wrapping_add(1 + k));
                        self.mem.w8(tseg, 0xA836 + k, d);
                    }
                    self.mem.w8(tseg, 0xA839, 0);
                    p = p.wrapping_add(4);
                }
                let next = loop {
                    p = p.wrapping_add(1);
                    match self.mem.r8(tseg, p) {
                        0 => break None,
                        1 => {
                            let colour = self.mem.r8(tseg, p.wrapping_add(1));
                            self.mem.w8(DS, 0x9288, colour);
                            break Some(p.wrapping_add(2));
                        }
                        0x0A | 8 => break Some(p.wrapping_add(1)),
                        0x0D if self.mem.r8(tseg, p.wrapping_add(1)) == 0x0A => break Some(p.wrapping_add(2)),
                        0x0D => break Some(p.wrapping_add(1)),
                        _ => {}
                    }
                };
                match next {
                    Some(s) => start = s,
                    None => break,
                }
            }
            while self.mem.r8(tseg, start) != 0 {
                start = start.wrapping_add(1);
            }
            at = start.wrapping_add(1);
            line_done(self);
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
