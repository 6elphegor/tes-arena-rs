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
