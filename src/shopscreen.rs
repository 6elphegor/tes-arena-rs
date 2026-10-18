//! The shops' screen as the game state sees it: the screen's picture
//! (1194:4B60), the banner with the shop's name (4BFF) and a picture on
//! it (4B8E). Each loads its files, takes and frees heap blocks and leaves
//! words behind; the images are drawn into the draw page ([A900]) as
//! 0000:5828 draws them (`render::image_page`), so what is later saved
//! from the page is the original's. The texts' drawing (0000:5364) and the
//! page's showing (9466) are the front end's.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// FONT_C.DAT's name, the banner's font (4B03).
const FONT_C: u16 = 0x4B03;
/// MENU_CTR.IMG, MENU_LFT.IMG, MENU_RT.IMG: the banner's tiles.
const MENU_CTR: u16 = 0x4AD8;
const MENU_LFT: u16 = 0x4AF1;
const MENU_RT: u16 = 0x4AE5;
/// The banner's font's segment (4C08).
pub const BANNER_FONT: u16 = 0x102A;
/// The banner's name's width in FONT_C (4C1D).
pub const BANNER_WIDTH: u16 = 0x12BC;
/// Where 1194:5496 reads the colours (9A6E, 300h bytes).
pub const PALETTE: u16 = 0x9A6E;
/// The page's rows' offsets (a word a row).
const ROWS: u16 = 0xA6A0;

impl Machine {
    /// 0000:742C with DI FFFEh: the file named at DS:`name` loaded whole
    /// into the last heap block it fits (0000:9CD6); its segment (DX), 0
    /// where it fails.
    pub fn load_alloc_high(&mut self, files: &mut Files, name: u16) -> u16 {
        if self.file_open_handle(files, name).is_none() {
            return 0;
        }
        let size = self.file_seek(files, 2, 0);
        if size > 0xFFFF {
            self.file_close(files);
            return (size >> 16) as u16;
        }
        self.mem.w16(DS, crate::bsa::var::OPEN_SIZE, size as u16);
        self.file_seek(files, 0, 0);
        let seg = match self.heap_alloc_high(size as u16) {
            Some(s) if s != 0 => s,
            _ => {
                self.file_close(files);
                return 0;
            }
        };
        let n = self.file_read(files, seg, 0, size as u16);
        self.file_close(files);
        if n == 0 {
            self.heap_free(seg);
            return 0;
        }
        seg
    }

    /// 0000:930E: the font whose segment is `seg` dropped from the fonts'
    /// table (91AC, 8 words) and its block freed, where the table holds it.
    pub fn font_free(&mut self, seg: u16) {
        if let Some(k) = (0..8u16).find(|&k| self.mem.r16(DS, 0x91AC + 2 * k) == seg) {
            self.mem.w16(DS, 0x91AC + 2 * k, 0);
            self.heap_free(seg);
        }
    }

    /// 0000:5828: the image whose header is at seg:off drawn into the draw
    /// page ([A900], 64000 bytes).
    pub fn page_image(&mut self, seg: u16, off: u16) {
        let page_seg = self.mem.r16(DS, 0xA900);
        let mut page: Vec<u8> = (0..64000u32).map(|i| self.mem.r8(page_seg, i as u16)).collect();
        crate::render::image_page(self, &mut page, seg, off);
        for (i, &b) in page.iter().enumerate() {
            self.mem.w8(page_seg, i as u16, b);
        }
    }

    /// 1194:4B60: the shop's screen, the picture named at DS:`si`
    /// (MENUSCRN.IMG), loaded into the EMS frame with bank [1030] mapped
    /// and drawn; the bank before mapped again.
    pub fn shop_screen(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let old = self.map_bank(self.mem.r16(DS, 0x1030), 0);
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let r = self.file_load(files, si, frame, 0);
        if r.is_some() {
            self.page_image(frame, 0);
        }
        self.map_bank(old, 0);
        r.map(|_| ())
    }

    /// 1194:4BFF: the banner with the shop's name at DS:`si`: FONT_C.DAT
    /// loaded whole (0000:7504: [102A]) and selected, the name measured
    /// (12BC; past 118h in [102C]'s font instead: A874), MENU_CTR.IMG's
    /// tiles (A878 of them, 16 wide: A876) from x A0h less half of that
    /// (A87C) at row 0, the end at A880, MENU_LFT.IMG 5 before them and
    /// MENU_RT.IMG after, each loaded high (A872) and freed after its
    /// drawing; the colour 70h, the name (A87E) drawn, the font freed.
    pub fn shop_banner(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let seg = self.font_load(files, FONT_C);
        self.mem.w16(DS, BANNER_FONT, seg);
        self.mem.w16(DS, 0xB036, seg);
        self.mem.w16(DS, 0xA87E, si);
        let mut w = self.text_width(si);
        self.mem.w16(DS, BANNER_WIDTH, w);
        if w > 0x118 {
            let f = self.mem.r16(DS, 0x102C);
            self.mem.w16(DS, 0xB036, f);
            w = self.text_width(si);
        }
        self.mem.w16(DS, 0xA874, w);
        let tiles = w.wrapping_add(0xF) >> 4;
        self.mem.w16(DS, 0xA878, tiles);
        let span = tiles.wrapping_mul(0x10);
        self.mem.w16(DS, 0xA876, span);
        let left = 0xA0u16.wrapping_sub(span >> 1);
        self.mem.w16(DS, 0xA87C, left);
        let img = self.load_alloc_high(files, MENU_CTR);
        self.mem.w16(DS, 0xA872, img);
        let mut x = left;
        for _ in 0..tiles {
            self.mem.w16(img, 2, 0);
            self.mem.w16(img, 0, x);
            self.page_image(img, 0);
            x = x.wrapping_add(0x10);
        }
        self.mem.w16(DS, 0xA880, x);
        self.heap_free(img);
        for (name, x) in [(MENU_LFT, left.wrapping_sub(5)), (MENU_RT, 0xA0u16.wrapping_add(span >> 1))] {
            let img = self.load_alloc_high(files, name);
            self.mem.w16(DS, 0xA872, img);
            self.mem.w16(img, 0, x);
            self.mem.w16(img, 2, 0);
            self.page_image(img, 0);
            self.heap_free(img);
        }
        self.mem.w8(DS, 0x9288, 0x70);
        let f = self.mem.r16(DS, BANNER_FONT);
        self.font_free(f);
        Some(())
    }

    /// Whether the draw page ([A900], from [A8FE] 0) is the game's own heap
    /// block, as a loaded game's is: a game the port starts without one
    /// keeps A000h there over its heap's blocks, where the screens' page
    /// writes would break the heap.
    pub fn page_is_block(&self) -> bool {
        self.mem.r16(DS, 0xA8FE) == 0 && self.heap_block_used(self.mem.r16(DS, 0xA900), 0xFA0)
    }

    /// 1194:5496: on entering a shop, the colours (`dac`, the VGA's, 6-bit
    /// values) read to 9A6E (0000:6E3C), the page written to SCREENSV.$$2
    /// (A69E) and the clip the whole screen (9A98: 0, 0, 140h x C8h).
    /// (None: the colours left as they are, for a front end without them.)
    pub fn shop_screen_saved(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>) {
        for (i, c) in dac.iter().flat_map(|d| d.iter()).enumerate() {
            for (k, &v) in c.iter().enumerate() {
                self.mem.w8(DS, PALETTE.wrapping_add((i * 3 + k) as u16), v);
            }
        }
        self.screen_save(files);
        self.clip_whole();
    }

    /// 1194:54B3: on leaving a shop, SCREENSV.$$2 read back into the page
    /// (A7A0), the clip the whole screen (9A98) and 0D39 bit 80h set: the
    /// next frame makes the interface again (`interface_setup`). The
    /// page's showing (9466) is the front end's.
    pub fn shop_screen_back(&mut self, files: &mut Files) {
        self.screen_restore(files);
        self.clip_whole();
        let f = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, f);
    }

    /// 1194:5E49: the buffer at [BA54] freed (0000:6C32 with its segment,
    /// BA56).
    pub fn buffer_free(&mut self) {
        let seg = self.mem.r16(DS, 0xBA56);
        self.heap_free(seg);
    }

    /// 1194:5E54: [CB8E] bytes taken from the heap (0000:51E2), their far
    /// pointer at BA54. None where the heap has no room.
    pub fn buffer_make(&mut self) -> Option<()> {
        let n = self.mem.r16(DS, 0xCB8E);
        let (seg, off) = self.heap_alloc_bytes(n)?;
        self.mem.w16(DS, 0xBA54, off);
        self.mem.w16(DS, 0xBA56, seg);
        Some(())
    }

    /// 0000:9A98 with AX 0, BX 0, CX 140h, DX C8h: the clip the whole
    /// screen.
    fn clip_whole(&mut self) {
        for (o, v) in [(0xA91Eu16, 0x140u16), (0xA91C, 0xC8), (0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0xC7)] {
            self.mem.w16(DS, o, v);
        }
    }

    /// 0000:9E0A: the text at `seg`:`si` drawn into the draw page ([A900])
    /// from (`x`, `y`) in colour [9288] and the font at [B034]: ADB6 the x
    /// (moved on by each glyph's width, a space by the first glyph's and
    /// one), ADB8 the row. Bytes under 20h are passed over; each glyph's
    /// rows are words, their bits from the top the pixels set.
    pub fn page_text(&mut self, (seg, si): (u16, u16), x: u16, y: u16) {
        self.mem.w16(DS, 0xADB6, x);
        self.mem.w16(DS, 0xADB8, y);
        let row = self.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y)));
        let page = self.mem.r16(DS, 0xA900);
        let (foff, fseg) = (self.mem.r16(DS, 0xB034), self.mem.r16(DS, 0xB036));
        let colour = self.mem.r8(DS, 0x9288);
        let mut at = si;
        loop {
            let c = self.mem.r8(seg, at);
            at = at.wrapping_add(1);
            if c == 0 {
                break;
            }
            let al = c.wrapping_sub(0x20);
            if al == 0 {
                let w = self.mem.r8(fseg, foff.wrapping_add(1)) as u16 + 1;
                let v = self.mem.r16(DS, 0xADB6).wrapping_add(w);
                self.mem.w16(DS, 0xADB6, v);
                continue;
            }
            if (al as i8) < 0 {
                continue;
            }
            let g = al.wrapping_sub(1) as u16;
            let h = self.mem.r8(fseg, foff) as u16;
            let w = self.mem.r8(fseg, foff.wrapping_add(g).wrapping_add(1)) as u16;
            let mut src = foff.wrapping_add((g.wrapping_mul(2) as u8 as u16).wrapping_mul(h)).wrapping_add(0x5F);
            let mut di = row.wrapping_add(self.mem.r16(DS, 0xADB6));
            for _ in 0..h {
                let mut bits = self.mem.r16(fseg, src);
                src = src.wrapping_add(2);
                for k in 0..w {
                    if bits & 0x8000 != 0 {
                        self.mem.w8(page, di.wrapping_add(k), colour);
                    }
                    bits <<= 1;
                }
                di = di.wrapping_add(0x140);
            }
            let v = self.mem.r16(DS, 0xADB6).wrapping_add(w);
            self.mem.w16(DS, 0xADB6, v);
        }
    }

    /// 0000:5368: the text at DS:`si` drawn centred on x `ax` at row `bx`
    /// (its width, 0000:66F6, halved off: `page_text`).
    pub fn page_text_centred(&mut self, si: u16, ax: u16, bx: u16) {
        let w = self.text_width(si);
        self.page_text((DS, si), ax.wrapping_sub(w >> 1), bx);
    }

    /// 1194:5F8C: the strip under the shop's menu (0Bh, 91h; 12Ah by 1Fh)
    /// kept in bank 18h at [9030]:7530 (0000:8F2A: A8FC 0, the rows from
    /// the page), the bank before mapped again.
    pub fn shop_strip_save(&mut self) {
        let old = self.map_bank(0x18, 0);
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        self.page_rect_save((frame, 0x7530), 0x0B, 0x91, 0x12A, 0x1F);
        self.map_bank(old, 0);
    }

    /// 1194:5FC0: the strip 5F8C kept put back into the page (0000:599C)
    /// from bank 18h, the bank before mapped again. The page's showing
    /// (9466) and the wait for the buttons' release (5E21) are the front
    /// end's.
    pub fn shop_strip_back(&mut self) {
        let old = self.map_bank(0x18, 0);
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        self.page_rect_put((frame, 0x7530), 0x0B, 0x91, 0x12A, 0x1F);
        self.map_bank(old, 0);
    }

    /// 1194:4D31: a message with the text at DS:`si`: [102E]'s font
    /// selected (9AE6), the strip kept (5F8C), the text drawn in colour C0h
    /// at (23h, 91h) and, after the player's click (the front end's), the
    /// strip put back (5FC0).
    pub fn shop_message(&mut self, si: u16) {
        self.select_font(crate::shops::font::TEENY);
        self.shop_strip_save();
        self.mem.w8(DS, 0x9288, 0xC0);
        self.page_text((DS, si), 0x23, 0x91);
        self.shop_strip_back();
    }

    /// 1194:4E88's state: the dialog's lines (LINES of them, 0FF5, each
    /// ending at a NUL from DS:`si`, as 1194:4E54 split them, [137E] to a
    /// page: 4E54's BX the pages) drawn page by page in [102E]'s font and
    /// colour C0h from (20h, 91h), 9 rows apart, each page's strip kept
    /// first (5F8C): A872 the lines, A87E the lines drawn on the page. A
    /// page with more after it ends with "...More" (4B1B, colour 4Bh at
    /// (11Dh, A9h)) and, after the player's click, its strip put back
    /// (5FC0) and A87E 0. The answer (kind [13D3]: 0 a click, 1 yes or
    /// no, 6001; 2 a number typed, 9EE5) is the front end's; the strip is
    /// put back after it (5FC0).
    pub fn shop_dialog(&mut self, si: u16) {
        let lines = self.mem.r16(DS, 0x0FF5);
        let per = self.mem.r8(DS, 0x137E) as u16;
        let mut pages = 1u16;
        for k in 1..lines {
            if per != 0 && k % per == 0 {
                pages += 1;
            }
        }
        self.mem.w16(DS, 0xA872, lines);
        self.mem.w16(DS, 0xA87E, 0);
        // The registers as 4E95 leaves them: CX the pages, BX the lines.
        let (mut cx, mut bx) = (pages, lines);
        let mut si = si;
        loop {
            // 4E97
            let mut dx = 0x91u16;
            let pushed = cx;
            self.shop_strip_save();
            let mut c = bx;
            let more = loop {
                if self.mem.r16(DS, 0xA87E) == 3 {
                    break Some(c);
                }
                let v = self.mem.r16(DS, 0xA87E).wrapping_add(1);
                self.mem.w16(DS, 0xA87E, v);
                self.select_font(crate::shops::font::TEENY);
                self.mem.w8(DS, 0x9288, 0xC0);
                self.page_text((DS, si), 0x20, dx);
                si = si.wrapping_add(self.mem.cstr(DS, si).len() as u16 + 1);
                dx = dx.wrapping_add(9);
                c = c.wrapping_sub(1);
                if c == 0 {
                    // 4ED0: the page's lines done; another page?
                    let left = pushed.wrapping_sub(1);
                    if left < 1 {
                        break None;
                    }
                    break Some(left);
                }
            };
            let Some(left) = more else { break };
            // 4ED8: "...More", the click, the strip back; 4F11.
            self.mem.w8(DS, 0x9288, 0x4B);
            self.page_text((DS, 0x4B1B), 0x11D, 0xA9);
            self.mem.w8(DS, 0x9288, 0xC0);
            self.shop_strip_back();
            self.mem.w16(DS, 0xA87E, 0);
            bx = left;
            cx = pushed.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
    }

    /// 0000:8F2A with DI not FFFFh or FFFEh: the page's `w` by `h` box at
    /// (`x`, `y`) copied row by row to `es`:`di` (A8FC 0). The page is
    /// [8F9C]'s where the far pointer at 8F9A is set, else [A900]'s.
    pub fn page_rect_save(&mut self, (es, di): (u16, u16), x: u16, y: u16, w: u16, h: u16) {
        self.mem.w16(DS, 0xA8FC, 0);
        let page = if self.mem.r32(DS, 0x8F9A) != 0 { self.mem.r16(DS, 0x8F9C) } else { self.mem.r16(DS, 0xA900) };
        let mut src = self.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
        let mut dst = di;
        for _ in 0..h {
            for k in 0..w {
                let b = self.mem.r8(page, src.wrapping_add(k));
                self.mem.w8(es, dst.wrapping_add(k), b);
            }
            dst = dst.wrapping_add(w);
            src = src.wrapping_add(0x140);
        }
    }

    /// 0000:599C: `w` by `h` bytes from `ds`:`si` put into the page
    /// ([A900]) at (`x`, `y`), row by row.
    pub fn page_rect_put(&mut self, (ds, si): (u16, u16), x: u16, y: u16, w: u16, h: u16) {
        let page = self.mem.r16(DS, 0xA900);
        let mut dst = self.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
        let mut src = si;
        for _ in 0..h {
            for k in 0..w {
                let b = self.mem.r8(ds, src.wrapping_add(k));
                self.mem.w8(page, dst.wrapping_add(k), b);
            }
            src = src.wrapping_add(w);
            dst = dst.wrapping_add(0x140);
        }
    }

    /// 1194:4B8E: the picture named at DS:`si` loaded high (A872), put at
    /// (59h, 7Fh), drawn and freed.
    pub fn shop_picture(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let img = self.load_alloc_high(files, si);
        self.mem.w16(DS, 0xA872, img);
        self.mem.w16(img, 0, 0x59);
        self.mem.w16(img, 2, 0x7F);
        self.page_image(img, 0);
        self.heap_free(img);
        Some(())
    }
}
