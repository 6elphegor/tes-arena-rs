//! The character sheet's run as the game's state sees it (F1 or the
//! portrait, 2542:00BE..00FF, around 1194:0CF8), and the world kept and
//! given back around a screen (0000:A611, A6B5).
//!
//! - The world kept (A611): the renderer's buffers freed (A5A0), the
//!   floor, walls and ceiling written to TEMPSAVE.$$$ (A665: the
//!   ceiling's words less the frame's segment, [9030], while written)
//!   and freed (C8FE), the mouse's box kept at BA4C (backwards), the
//!   colours read to A39E (6E3C) and the page written to SCREENSV.$$2
//!   (A69E).
//! - The sheet (0CF8): unless kept already (5E0 bit 200h), the body's
//!   picture (CHRBKF00.IMG or CHARBK00.IMG, the race's digit before the
//!   '.') drawn on a cleared page and the page written to CHARSHET.$$$,
//!   the face (FACES0r.CIF, '1' for the screen at 12BA, the player's
//!   frame +6Bh) written to SAVEFACE.$$$ (158F); the equipment's frames
//!   (nEQUIP.CIF, n at 1AB) into bank 17h (2462); the fonts, the colours
//!   black (55B2); the page drawn (0D8E): CHARSHET.$$$ into bank [1030]
//!   (1647), the doll (16AA: the clothes' names by sex, the face unless
//!   a helmet is worn, the trousers, the worn items from bank 17h, the
//!   shirt), CHARSTAT.IMG, the texts (0DE1: the numbers made at 1044 and
//!   the attributes' copy at 10A8 their only state), PAGE2.IMG and
//!   CHARSHT.COL's colours (1695).
//! - Its buttons (5CF4 over 3276:52B0's two records): N turns to the
//!   equipment page (the front end's), Esc (or the click at 0, AAh, 1Fh
//!   x 11h) leaves (3159:0006). Then the colours black again (55B6),
//!   12BA 0, B7C4 as it was, CD00 FFFFh.
//! - The world given back (A6B5): the buffer block, the planes read back
//!   from TEMPSAVE.$$$, the renderer's buffers (A561), SCREENSV.$$2, the
//!   mouse's box (53AE), the row tables (DA8E) and outdoors the sky's
//!   images (E5C9); then the piles taken marked on the map (2433:0A39).
//!
//! The page's pixels (drawn only where the page is the game's own heap
//! block, `Machine::page_is_block`), the fades (5E3E) and the showing
//! (946A) are the front end's.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// The player's record and their items (40 slots of 13h bytes).
const PLAYER: u16 = 0x01A4;
const ITEMS: u16 = 0x0212;
const SLOT: u16 = 0x13;

/// The files' names.
const TEMPSAVE: u16 = 0xBA30;
const CHARSHET: u16 = 0x3DBE;
const SAVEFACE: u16 = 0x3DCB;
const PAGE2: u16 = 0x3DD8;
const CHARSTAT: u16 = 0x3DE2;
const COLOURS: u16 = 0x3DB2;
const BODY: u16 = 0x4B90;
const BODY_F: u16 = 0x3DEF;
const FACES: u16 = 0x3EFD;
const FACES_F: u16 = 0x3F09;
const EQUIP: u16 = 0x3E6D;
const SHIRT: u16 = 0x3E42;
const PANTS: u16 = 0x3E4E;
const HELMET: u16 = 0x3E78;

/// The planes' segments: the ceiling's (CCD0), the walls' (D806), the
/// floor's (D748).
const CEILING: u16 = 0xCCD0;
const WALLS: u16 = 0xD806;
const FLOOR: u16 = 0xD748;
/// The EMS frame's segment.
const FRAME: u16 = 0x9030;
/// A screen of the sheet is up.
const SHEET: u16 = 0x12BA;
/// The page shown is the stats' (1), not the equipment's.
const STATS: u16 = 0x129B;
/// The buttons' loop's flags (bit 1 leave, 4 no wait for the buttons'
/// release) and the record matched.
const LOOP_FLAGS: u16 = 0xB7C4;
const LOOP_MATCH: u16 = 0xB7C6;
/// The worn items' count (1194:7472 of the player's).
const WORN: u16 = 0x129D;
/// A worn item's colour (+10h with +0Eh bit 1; FFFFh none), a counter of
/// its recolouring and its frame's header.
const ITEM_COLOUR: u16 = 0x12A4;
const ITEM_STEPS: u16 = 0x12A6;
const ITEM_HEADER: u16 = 0x12A8;
/// The scratch the numbers are made in (1044) and the colours (929E).
const NUMBER: u16 = 0x1044;
const SCRATCH: u16 = 0x929E;

impl Machine {
    /// 0000:A611: the world kept while a screen is up. `dac` the VGA's
    /// colours (6-bit) 6E3C reads to A39E (None: left as they are).
    pub fn world_keep(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>) {
        // A5A0: the renderer's buffers and the buffer block freed; DE9B
        // the compiled stores'; AB2A the light table.
        if self.mem.r16(DS, 0xCB8C) != 0 {
            let s = self.mem.r16(DS, 0xCD78);
            self.heap_free(s);
        }
        for at in [0xCD84u16, 0xCD80, 0xCD88, 0xBA56, 0xCD8C, 0xCD90, 0xCCC2] {
            let s = self.mem.r16(DS, at);
            self.heap_free(s);
        }
        self.mem.w16(DS, 0xCCC2, 0);
        let path = self.save_path(TEMPSAVE);
        self.file_create(files, path);
        // A665.
        let walls = self.mem.r16(DS, WALLS);
        self.file_put(files, walls, 0, 0x8000);
        let frame = self.mem.r16(DS, FRAME);
        self.plane_shift(frame.wrapping_neg());
        let ceiling = self.mem.r16(DS, CEILING);
        self.file_put(files, ceiling, 0, 0x8000);
        self.plane_shift(frame);
        let floor = self.mem.r16(DS, FLOOR);
        self.file_put(files, floor, 0, 0x8000);
        self.file_close(files);
        // C8FE.
        for at in [CEILING, WALLS, FLOOR] {
            let s = self.mem.r16(DS, at);
            self.heap_free(s);
            self.mem.w16(DS, at, 0);
        }
        // A628..A644: 8F74..8F7A pushed and popped to BA4C..BA52.
        for (k, from) in [0x8F7Au16, 0x8F78, 0x8F76, 0x8F74].into_iter().enumerate() {
            let v = self.mem.r16(DS, from);
            self.mem.w16(DS, 0xBA4C + 2 * k as u16, v);
        }
        for (i, c) in dac.iter().flat_map(|d| d.iter()).enumerate() {
            for (k, &v) in c.iter().enumerate() {
                self.mem.w8(DS, 0xA39E + (i * 3 + k) as u16, v);
            }
        }
        self.screen_save(files);
    }

    /// 0000:A653: `ax` added to each of the ceiling's 4000h words.
    fn plane_shift(&mut self, ax: u16) {
        let seg = self.mem.r16(DS, CEILING);
        for i in 0..0x4000u16 {
            let v = self.mem.r16(seg, 2 * i).wrapping_add(ax);
            self.mem.w16(seg, 2 * i, v);
        }
    }

    /// 0000:A6B5 to its fade: the world given back as `world_keep` kept
    /// it. None where the heap has no room or TEMPSAVE.$$$ is missing.
    pub fn world_back(&mut self, files: &mut Files) -> Option<()> {
        let n = self.mem.r16(DS, 0xCB8E);
        let seg = self.heap_alloc_high(n)?;
        self.mem.w16(DS, 0xBA54, 0);
        self.mem.w16(DS, 0xBA56, seg);
        let path = self.save_path(TEMPSAVE);
        let h = self.file_open_handle(files, path)?;
        self.mem.w16(DS, 0xBA6C, h);
        // A72E.
        let walls = self.level_alloc(0x8000)?;
        self.mem.w16(DS, WALLS, walls);
        self.file_read(files, walls, 0, 0x8000);
        let (ceiling, _) = self.heap_alloc_bytes(0x8000)?;
        self.mem.w16(DS, CEILING, ceiling);
        self.file_read(files, ceiling, 0, 0x8000);
        let frame = self.mem.r16(DS, FRAME);
        self.plane_shift(frame);
        if self.mem.r16(DS, 0xC398) & 4 != 0 {
            let (floor, _) = self.heap_alloc_bytes(0x8000)?;
            self.mem.w16(DS, FLOOR, floor);
            self.file_read(files, floor, 0, 0x8000);
        } else {
            self.file_seek(files, 1, 0x8000);
        }
        self.file_close(files);
        // A561.
        self.draw_buffers_alloc()?;
        self.light_table_make()?;
        for i in 0..0x300u16 {
            self.mem.w8(DS, SCRATCH + i, 0);
        }
        self.screen_restore(files);
        self.mem.w8(DS, 0xC994, 0);
        // A70D..A722: the mouse's box back (53AE).
        let (dx, bx, cx, ax) = (self.mem.r16(DS, 0xBA4C), self.mem.r16(DS, 0xBA4E), self.mem.r16(DS, 0xBA50), self.mem.r16(DS, 0xBA52));
        let (w, h) = (cx.wrapping_sub(ax).wrapping_add(1), dx.wrapping_sub(bx).wrapping_add(1));
        self.mem.w16(DS, 0x8F78, bx);
        self.mem.w16(DS, 0x8F7A, bx.wrapping_add(h).wrapping_sub(1));
        self.mem.w16(DS, 0x8F74, ax);
        self.mem.w16(DS, 0x8F76, ax.wrapping_add(w).wrapping_sub(1));
        self.row_tables();
        self.sky_images(files)?;
        Some(())
    }

    /// 2542:00E0..00F0 and 1194:0CF8..0D4A: the character sheet opened
    /// (the place checked for the drop button, 1194:5F13, is the menu
    /// key's: `Machine::portrait`), up to its buttons' loop. Returns B7C4
    /// as it was, for `sheet_close`. None where a file is missing or the
    /// heap has no room.
    pub fn sheet_open(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>) -> Option<u8> {
        self.clip_whole();
        self.world_keep(files, dac);
        let saved = self.mem.r8(DS, LOOP_FLAGS);
        if self.mem.r16(DS, 0x05E0) & 0x200 == 0 {
            self.sheet_keep(files)?;
        }
        // 2462: the equipment's frames into bank 17h (left mapped).
        let n = self.mem.r8(DS, 0x01AB).wrapping_add(b'0');
        self.mem.w8(DS, EQUIP, n);
        self.map_bank(0x17, 0);
        let frame = self.mem.r16(DS, FRAME);
        self.file_load(files, EQUIP, frame, 0)?;
        self.mem.w16(DS, SHEET, 1);
        self.select_font(0x102C);
        self.select_tiles(0x103A);
        self.sheet_black();
        self.mem.w8(DS, STATS, 1);
        // 0D67: the page turned to the stats' (129B 1 xor 1).
        self.mem.w8(DS, STATS, 0);
        self.sheet_stats(files)
            .map(|_| saved)
    }

    /// The sheet's loop left (5CF4 with the exit's record, `key` its key
    /// typed: Esc, 1Bh; None, a click), 1194:0D4E..0D66 and 2542:00F5..00FA:
    /// the world given back (`world_back`) and the piles taken marked
    /// (2433:0A39). `saved` B7C4 as `sheet_open` found it.
    pub fn sheet_close(&mut self, files: &mut Files, saved: u8, key: Option<u8>) -> Option<()> {
        let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        self.mem.w16(DS, LOOP_MATCH, 0);
        // 5DE2: the keys flushed, the key read.
        self.key_flush();
        if let Some(k) = key {
            self.ring_typed(k);
        }
        // The first record (N) passed over, the second's handler
        // (3159:0006): leave.
        self.mem.w16(DS, LOOP_MATCH, 1);
        let f = self.mem.r8(DS, LOOP_FLAGS) | 1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        self.sheet_black();
        self.mem.w16(DS, SHEET, 0);
        self.mem.w8(DS, LOOP_FLAGS, saved);
        self.mem.w16(DS, 0xCD00, 0xFFFF);
        self.world_back(files)?;
        self.piles_mark();
        Some(())
    }

    /// 1194:55B6: the colours at 929E black (the fade to them, 5E3E, the
    /// front end's) and the page cleared (55D9: 9426; the screen's own
    /// clearing the front end's).
    fn sheet_black(&mut self) {
        for i in 0..0x300u16 {
            self.mem.w8(DS, SCRATCH + i, 0);
        }
        self.page_clear();
    }

    /// 0000:9426: the page ([A8FE], FA00h bytes) cleared unless A902 bit
    /// 4 (here only where the page is the game's own block).
    fn page_clear(&mut self) {
        if self.mem.r16(DS, 0xA902) & 4 != 0 || !self.page_is_block() {
            return;
        }
        let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, 0xA900));
        for i in 0..0xFA00u16 {
            self.mem.w8(seg, off.wrapping_add(i), 0);
        }
    }

    /// 1194:158F: the body drawn on the cleared page and the page written
    /// to CHARSHET.$$$ (with bank 5 mapped first), the face written to
    /// SAVEFACE.$$$; 5E0 bit 200h set: kept.
    fn sheet_keep(&mut self, files: &mut Files) -> Option<()> {
        let bank = self.mem.r16(DS, 0x1030);
        let old = self.map_bank(bank, 0);
        let page = self.mem.r16(DS, 0xA900);
        self.page_clear();
        // 161A.
        let si = if self.mem.r8(DS, PLAYER + 7) != 0 { BODY_F } else { BODY };
        let race = self.mem.r8(DS, PLAYER + 4) as u16;
        let di = self.name_dot(si, 0x0B).wrapping_sub(2);
        self.store_utoa(race, di);
        self.sheet_image_here(files, si)?;
        self.map_bank(5, 0);
        let path = self.save_path(CHARSHET);
        self.file_write(files, path, page, 0, 0xFA00);
        self.mem.w16(DS, SHEET, 1);
        let face = self.sheet_face(files)?;
        self.map_bank(0x17, 0);
        let frame = self.mem.r16(DS, FRAME);
        let n = self.mem.r16(frame, face.wrapping_add(0x0A)).wrapping_add(0x0C);
        let path = self.save_path(SAVEFACE);
        self.file_write(files, path, frame, face, n);
        self.mem.w16(DS, 0xA900, page);
        self.mem.w16(DS, SHEET, 0);
        let f = self.mem.r16(DS, 0x05E0) | 0x200;
        self.mem.w16(DS, 0x05E0, f);
        self.map_bank(old, 0);
        Some(())
    }

    /// `repne scasb` for '.' over `n` bytes from DS:`si`: DI after it (or
    /// after the `n` bytes).
    fn name_dot(&self, si: u16, n: u16) -> u16 {
        for i in 0..n {
            if self.mem.r8(DS, si.wrapping_add(i)) == b'.' {
                return si.wrapping_add(i + 1);
            }
        }
        si.wrapping_add(n)
    }

    /// 1194:2D60 with DX 1 (2DAF): the player's faces (FACES0r.CIF or
    /// the female's, '1' with a sheet's screen up) loaded into bank 17h;
    /// the offset of the player's face (+6Bh) among its frames (5CCC).
    fn sheet_face(&mut self, files: &mut Files) -> Option<u16> {
        let si = if self.mem.r8(DS, PLAYER + 7) != 0 { FACES_F } else { FACES };
        let di = self.name_dot(si, 0x14);
        let c = if self.mem.r16(DS, SHEET) != 0 { b'1' } else { b'0' };
        self.mem.w8(DS, di.wrapping_sub(3), c);
        let di = self.name_dot(si, 0x14).wrapping_sub(2);
        let race = self.mem.r8(DS, PLAYER + 4) as u16;
        self.store_utoa(race, di);
        let old = self.map_bank(0x17, 0);
        let frame = self.mem.r16(DS, FRAME);
        self.file_load(files, si, frame, 0)?;
        self.map_bank(old, 0);
        let k = self.mem.r8(DS, PLAYER + 0x6B) as u16;
        let old = self.map_bank(0x17, 0);
        let face = self.cif_frame(k);
        self.map_bank(old, 0);
        Some(face)
    }

    /// 1194:5CCC: the offset of frame `k` of the CIF in the EMS frame.
    fn cif_frame(&self, k: u16) -> u16 {
        let frame = self.mem.r16(DS, FRAME);
        let mut si = 0u16;
        for _ in 0..k {
            si = si.wrapping_add(self.mem.r16(frame, si.wrapping_add(0x0A))).wrapping_add(0x0C);
        }
        si
    }

    /// 1194:5CD9 (BX without bit 10h left aside: the picture's own colours
    /// are the front end's): the picture named at DS:`si` loaded into
    /// bank [1030] and drawn (`sheet_image_here`), the bank before
    /// mapped again.
    fn sheet_image(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let bank = self.mem.r16(DS, 0x1030);
        let old = self.map_bank(bank, 0);
        self.sheet_image_here(files, si)?;
        self.map_bank(old, 0);
        Some(())
    }

    /// 0000:56C8 with BX 4 or 14h: the picture named at DS:`si` loaded to
    /// the EMS frame's start (742C), its header's words to A836..A83C,
    /// drawn into the page (582C).
    fn sheet_image_here(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let frame = self.mem.r16(DS, FRAME);
        self.file_load(files, si, frame, 0)?;
        for k in 0..4u16 {
            let v = self.mem.r16(frame, 2 * k);
            self.mem.w16(DS, 0xA836 + 2 * k, v);
        }
        if self.page_is_block() {
            self.page_image(frame, 0);
        }
        Some(())
    }

    /// 1194:0D8E: the stats' page: CHARSHET.$$$ into bank [1030] and onto
    /// the page (1647), the doll (16AA), CHARSTAT.IMG, the texts (0DE1),
    /// PAGE2.IMG and CHARSHT.COL's colours read to 929E (1695: 7530).
    fn sheet_stats(&mut self, files: &mut Files) -> Option<()> {
        self.mem.w8(DS, 0x9288, 0xFE);
        let bank = self.mem.r16(DS, 0x1030);
        let old = self.map_bank(bank, 0);
        let path = self.save_path(CHARSHET);
        let frame = self.mem.r16(DS, FRAME);
        self.file_load(files, path, frame, 0)?;
        if self.page_is_block() {
            // 5B04: the frame over the page, its 0s left out.
            let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, 0xA900));
            for i in 0..0xFA00u16 {
                let b = self.mem.r8(frame, i);
                if b != 0 {
                    self.mem.w8(seg, off.wrapping_add(i), b);
                }
            }
        }
        self.map_bank(old, 0);
        self.sheet_doll(files)?;
        self.sheet_image(files, CHARSTAT)?;
        self.sheet_texts();
        self.sheet_image(files, PAGE2)?;
        let n = self.file_load(files, COLOURS, DS, SCRATCH).map(|_| self.mem.r16(DS, crate::bsa::var::OPEN_SIZE))?;
        if n != 0x300 {
            for i in 0..0x300u16 {
                let b = self.mem.r8(DS, SCRATCH + 8 + i) >> 2;
                self.mem.w8(DS, SCRATCH + i, b);
            }
        }
        Some(())
    }

    /// 1194:16AA: the clothes' names by sex (1D22) and the doll (23E5):
    /// bank 17h mapped, the worn items counted (129D), the face unless a
    /// helmet (type 5, kind 0) is worn (1687: SAVEFACE.$$$), the trousers,
    /// a worn shirt's item (type 2, kind 0: 2187) under the shirt, then
    /// each worn item of the first [129D] slots but such shirts (2451).
    fn sheet_doll(&mut self, files: &mut Files) -> Option<()> {
        let bl = if self.mem.r8(DS, PLAYER + 5) & 0x20 != 0 { b'r' } else { b's' };
        let al = if self.mem.r8(DS, PLAYER + 7) != 0 { b'f' } else { b'm' };
        self.mem.w8(DS, SHIRT, al);
        self.mem.w8(DS, SHIRT + 1, bl);
        self.mem.w8(DS, PANTS, al);
        self.mem.w8(DS, HELMET, al);
        self.map_bank(0x17, 0);
        let (n, _) = self.count_items(DS, ITEMS);
        self.mem.w8(DS, WORN, n as u8);
        if n == 0 || self.worn_find(5, 0, n).is_none() {
            let path = self.save_path(SAVEFACE);
            self.sheet_image(files, path)?;
        }
        self.sheet_image(files, PANTS)?;
        let n = self.mem.r8(DS, WORN) as u16;
        if let Some(si) = self.worn_find(2, 0, n) {
            self.sheet_item(files, si)?;
        }
        self.sheet_image(files, SHIRT)?;
        let mut si = ITEMS;
        for _ in 0..self.mem.r8(DS, WORN) {
            if self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x80 != 0 && !(self.mem.r8(DS, si) == 2 && self.item_category(DS, si) == 0) {
                self.sheet_item(files, si)?;
            }
            si = si.wrapping_add(SLOT);
        }
        Some(())
    }

    /// 1194:745A with AX `ty`, BX `kind`, CX `n` from 0212: a worn item of
    /// the base type and kind among the slots 75D3 looks through (CX
    /// counts the slots looked at, not the items: bug kept), its offset.
    fn worn_find(&self, ty: u16, kind: u16, n: u16) -> Option<u16> {
        let (mut cx, mut si) = (n, ITEMS);
        loop {
            // 75D3.
            if cx == 0 {
                return None;
            }
            loop {
                let flags = self.mem.r8(DS, si.wrapping_add(0x0E));
                if flags & 0x40 != 0 && self.base_type(self.mem.r8(DS, si)).0 == ty && self.item_category(DS, si) == kind {
                    cx -= 1;
                    break;
                }
                si = si.wrapping_add(SLOT);
                cx -= 1;
                if cx == 0 {
                    return None;
                }
            }
            if self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x80 != 0 {
                return Some(si);
            }
            si = si.wrapping_add(SLOT);
        }
    }

    /// 1194:1F8D: the worn item at DS:`si` on the doll: its frame in bank
    /// 17h (5CCC: weapons by type, the armour's by type and kind, 21A8;
    /// none for kind 2), drawn as it is or, coloured (+0Eh bit 1, its
    /// colour +10h at 12A4), its header copied to 12A8 and its frame to
    /// 929E and recoloured through bank 4 (20BF: bank 4's first C350h
    /// bytes cleared, the frame drawn there and mapped through the
    /// colours; that drawing the front end's); a helmet (type 5, kind 0)
    /// then for the classes that show one (2020).
    fn sheet_item(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let colour = if self.mem.r8(DS, si.wrapping_add(0x0E)) & 1 != 0 { self.mem.r8(DS, si.wrapping_add(0x10)) as i8 as i16 as u16 } else { 0xFFFF };
        self.mem.w16(DS, ITEM_COLOUR, colour);
        let ty = self.mem.r8(DS, si);
        let cx = if ty & 0x80 != 0 { self.base_type(ty).0 } else { ty as u16 };
        let kind = self.item_category(DS, si);
        if kind != 2 {
            let k = if kind == 1 {
                cx
            } else {
                let armour = self.armor_kind(DS, si);
                let mut c = cx;
                if armour != 0 {
                    c = c.wrapping_add(0x0B);
                    if armour != 1 {
                        c = c.wrapping_add(7);
                    }
                }
                c.wrapping_add(0x12)
            };
            let img = self.cif_frame(k);
            let frame = self.mem.r16(DS, FRAME);
            if colour != 0xFFFF {
                for i in 0..8u16 {
                    let b = self.mem.r8(frame, img.wrapping_add(i));
                    self.mem.w8(DS, ITEM_HEADER + i, b);
                }
                let n = self.mem.r16(frame, img.wrapping_add(0x0A)).wrapping_add(0x0C);
                for i in 0..n {
                    let b = self.mem.r8(frame, img.wrapping_add(i));
                    self.mem.w8(DS, SCRATCH.wrapping_add(i), b);
                }
                // 20BF.
                let old = self.map_bank(4, 0);
                let frame = self.mem.r16(DS, FRAME);
                for i in 0..0xC350u16 {
                    self.mem.w8(frame, i, 0);
                }
                self.mem.w32(DS, 0x8F9A, 0);
                for i in 0..0x100u16 {
                    self.mem.w8(DS, SCRATCH + i, i as u8);
                }
                self.mem.w16(DS, ITEM_STEPS, 0);
                self.map_bank(old, 0);
                return Some(());
            }
            if self.page_is_block() {
                self.page_image(frame, img);
            }
        }
        if self.mem.r8(DS, si) == 5 && self.item_category(DS, si) == 0 {
            // 2020: by the class (1A8): 7, the helmet's picture by its kind.
            let class = self.mem.r8(DS, 0x01A8);
            if class == 7 {
                return crate::giveup::not_ported("1194:20B0: a helmet's picture for class 7 (0000:56C8 with BX 7)");
            }
            let _ = files;
        }
        Some(())
    }

    /// 1194:0DE1: the stats' texts, their state: the numbers made (0000:
    /// 7028's digits at A931.., the formatted at 1044: hit points,
    /// fatigue, experience and the bonuses), and the player's derived
    /// numbers (+39h, 16 words) scaled and divided in place for their
    /// texts and put back from their copy at 10A8 (0F08). Their drawing
    /// is the front end's.
    fn sheet_texts(&mut self) {
        let gold = self.mem.r32(DS, 0x05C2);
        self.dun_ultoa(gold, SCRATCH);
        let raw = self.mem.r16(DS, 0x05C8) & 0x10 != 0;
        // 0E53: the attributes.
        for i in 0..8u16 {
            let mut cx = self.mem.r8(DS, PLAYER + 0x29 + i) as u16;
            if !raw {
                cx = crate::scale::to100(cx as i16) as u16;
            }
            self.utoa_nul(cx, SCRATCH);
        }
        // 0E86.
        let p = PLAYER;
        let cx = self.mem.r16(DS, p + 0x5B);
        let dx = if raw { cx } else { self.mem.r16(DS, p + 0x59) };
        self.sheet_ratio(dx, cx);
        let mut ax = (self.mem.r8(DS, p + 0x29) as u16).wrapping_add(self.mem.r8(DS, p + 0x2E) as u16);
        if !raw {
            ax = crate::scale::to100_u(ax);
        }
        let dx = crate::scale::to100_u(self.mem.r16(DS, p + 0x5D) >> 6);
        self.sheet_ratio(dx, ax);
        let (cx, dx) = (self.mem.r16(DS, p + 0x68), self.mem.r16(DS, p + 0x66));
        self.sheet_ratio(dx, cx);
        let level = self.mem.r8(DS, p + 6) as u16 + 1;
        self.utoa_nul(level, SCRATCH);
        let exp = self.mem.r32(DS, p + 0x409);
        self.dun_ultoa(exp, SCRATCH);
        // 0F08.
        for i in 0..0x20u16 {
            let b = self.mem.r8(DS, p + 0x39 + i);
            self.mem.w8(DS, 0x10A8 + i, b);
        }
        // (The cap at 1FEh is overwritten: the number shown as it is.)
        let v = crate::scale::to100_u(self.mem.r16(DS, p + 0x3B));
        self.utoa_nul(v, SCRATCH);
        for off in [0x39u16, 0x4F, 0x53] {
            let v = self.mem.r16(DS, p + off);
            self.sheet_signed(v, 0);
        }
        for i in 0..16u16 {
            let at = p + 0x39 + 2 * i;
            let v = crate::scale::to100(self.mem.r16(DS, at) as i16) as u16;
            self.mem.w16(DS, at, v);
        }
        for i in 0..16u16 {
            let at = p + 0x39 + 2 * i;
            let v = (self.mem.r16(DS, at) as i16 / 5) as u16;
            self.mem.w16(DS, at, v);
        }
        let v = self.mem.r16(DS, p + 0x43).wrapping_neg();
        self.sheet_signed(v, 0);
        for off in [0x47u16, 0x49, 0x55] {
            let v = self.mem.r16(DS, p + off);
            self.sheet_signed(v, 0);
        }
        for i in 0..0x20u16 {
            let b = self.mem.r8(DS, 0x10A8 + i);
            self.mem.w8(DS, p + 0x39 + i, b);
        }
    }

    /// 0000:7052: `v` in decimal at DS:`di` (7028), NUL-terminated.
    fn utoa_nul(&mut self, v: u16, di: u16) {
        self.store_utoa(v, di);
        let mut n = 1u16;
        let mut x = v / 10;
        while x != 0 {
            n += 1;
            x /= 10;
        }
        self.mem.w8(DS, di.wrapping_add(n), 0);
    }

    /// 1194:0FD8: "%d/%d" (3E11) of `dx` and `cx` at 1044.
    fn sheet_ratio(&mut self, dx: u16, cx: u16) {
        use crate::format::Arg;
        self.sprintf((DS, NUMBER), (DS, 0x3E11), &[Arg::Int(dx), Arg::Int(cx)]);
    }

    /// 1194:48F5: `cx` at 1044 as "+%u" (3276:3A5C), or "%d" (3A60) with
    /// DX set or CX negative.
    fn sheet_signed(&mut self, cx: u16, dx: u16) {
        use crate::format::Arg;
        let fmt = if dx != 0 || (cx as i16) < 0 { 0x3A60 } else { 0x3A5C };
        self.sprintf((DS, NUMBER), (DATA, fmt), &[Arg::Int(cx)]);
    }

    /// 2433:0A39: outside the wilderness, the piles taken (+8 1, in the
    /// table at the frame's A000h with bank 6 mapped, [FF2] of 1Ch bytes)
    /// marked untaken, each copied to 4BF6 and its square on the walls'
    /// map (at [4BF8]) set to 80h:[C937].
    pub fn piles_mark(&mut self) {
        if self.mem.r16(DS, 0xC39A) & 0x200 != 0 {
            return;
        }
        let old = self.map_bank(6, 0);
        let frame = self.mem.r16(DS, FRAME);
        let mut si = 0xA000u16;
        for _ in 0..self.mem.r8(DS, 0x0FF2) {
            if self.mem.r8(frame, si.wrapping_add(8)) == 1 {
                self.mem.w8(frame, si.wrapping_add(8), 0);
                for i in 0..0x1Cu16 {
                    let b = self.mem.r8(frame, si.wrapping_add(i));
                    self.mem.w8(DS, 0x4BF6 + i, b);
                }
                let (seg, bx) = (self.mem.r16(DS, WALLS), self.mem.r16(DS, 0x4BF8));
                let v = 0x8000 | self.mem.r8(DS, 0xC937) as u16;
                self.mem.w16(seg, bx, v);
            }
            si = si.wrapping_add(0x1C);
        }
        self.map_bank(old, 0);
    }
}
