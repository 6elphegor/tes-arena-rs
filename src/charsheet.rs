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
//!   equipment page (0D67, 16BD), Esc (or the click at 0, AAh, 1Fh
//!   x 11h) leaves (3159:0006). Then the colours black again (55B6),
//!   12BA 0, B7C4 as it was, CD00 FFFFh.
//! - The equipment page (16E0): the page cleared, CHARSHET.$$$ and
//!   EQUIP.IMG on it, the page kept in bank 5, the doll's place in bank 4
//!   (C350h) and the message's in bank [1032] and BACKMSG.$$$ (1738); then
//!   drawn (23B1): the doll, the name, race, class and level, the list of
//!   the items and the quests' lines (1DC8), the armour's classes (76B4).
//!   Its own buttons' loop (5CF4 over 3276:57D8's six) with B7C4 bit 4;
//!   its exit (the fifth, Esc) goes back to the stats' page (0D8E). The
//!   others are the original's handlers: the list's clicks (1AC6: a line
//!   marked, worn or taken off, a quest's line's note; a right click's
//!   description, 1988), the arrows (1A4A, 1A94), Drop (17D8) and Spells
//!   (2365, the spells' list 1D5C). What they say goes in the list's
//!   panel (223A), the wait there the front end's. A marked spell's line
//!   clicked again opens its page (1B3C: `sheet_spell_page`). Drop with
//!   no line marked says "Drop what?" (18CA): 223A's wait is then the
//!   page's own loop again over its first five records (2325), where a
//!   line clicked is marked and the drop starts over (`drop_what_loop`;
//!   D within it, the drop within the drop, isn't ported).
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
const EQUIP_IMG: u16 = 0x3E59;
const BACKMSG: u16 = 0x1AA5;
/// The equipment list's own lines shown (1194:1DC8's count).
const LIST_SHOWN: u16 = 0x126E;

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
/// The front end's wait (1194:5536, after 5E21) while the list's panel
/// shows its lines (1194:223A): each line's text and colour, and the
/// first's row.
pub type PanelWait<'a> = dyn FnMut(&mut Machine, &[(Vec<u8>, u8)], u16) + 'a;

/// What 223A's DI runs after its lines are shown (`list_panel_then`).
type PanelThen<'a> = dyn FnMut(&mut Machine, &[(Vec<u8>, u8)], u16) -> Option<()> + 'a;

/// What the front end gives a turn of the equipment page's loop (5DE2):
/// a key typed (its character) or a left click (x, y).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageInput {
    Key(u8),
    Click(u16, u16),
}

/// `equip_click`'s refusals as the page's click (1AFB) takes them: the
/// item and its category (1BE0's) or the clashing kind's base (1B78's),
/// said in the list's panel by the page itself.
#[derive(Default)]
struct Refusal {
    cant_use: Option<(u16, u16)>,
    clash: Option<u16>,
}

impl crate::use_item::UseHooks for Refusal {
    fn cant_use(&mut self, _m: &mut Machine, si: u16, cat: u16) {
        self.cant_use = Some((si, cat));
    }
    fn clash(&mut self, _m: &mut Machine, _si: u16, base: u16) {
        self.clash = Some(base);
    }
}

/// The equipment page's buttons (3276:57D8: 62h bytes each; x, y, width
/// and height words, the key at +59h, the handler at +5Eh).
const PAGE_BUTTONS: u16 = 0x57D8;

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
        self.equip_frames(files)?;
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

    /// The sheet's loop on N (5CF4's first record, 3276:52B0), `key` its
    /// key typed (`n`; None, a click on it): 1194:0D67's turn to the
    /// equipment page, 129B 1, and 16BD up to its own buttons' loop
    /// (`equipment_setup`, then B7C4 bit 4: no waits for the buttons'
    /// release).
    pub fn sheet_equipment(&mut self, files: &mut Files, key: Option<u8>) -> Option<()> {
        let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        self.mem.w16(DS, LOOP_MATCH, 0);
        // 5DE2: the keys flushed, the key read; the first record matched.
        self.key_flush();
        if let Some(k) = key {
            self.ring_typed(k);
        }
        let v = self.mem.r8(DS, STATS) ^ 1;
        self.mem.w8(DS, STATS, v);
        self.equipment_setup(files)?;
        let f = self.mem.r8(DS, LOOP_FLAGS) | 4;
        self.mem.w8(DS, LOOP_FLAGS, f);
        // 5D08..5D1E: its loop's start, the keys flushed before its wait
        // (5DE2: a key typed comes after).
        let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        self.mem.w16(DS, LOOP_MATCH, 0);
        self.key_flush();
        Some(())
    }

    /// The equipment page's loop left by its exit (3276:57D8's fifth
    /// record, 3159:0006), `key` its key typed (Esc, 1Bh; None, a click):
    /// 16D7 B7C4 bit 4 off, back in 0D80 129B as it was, B7C4 0 and the
    /// stats' page drawn again (0D8E); the sheet's loop goes on.
    pub fn sheet_equipment_close(&mut self, files: &mut Files, key: Option<u8>) -> Option<()> {
        self.key_flush();
        if let Some(k) = key {
            self.ring_typed(k);
        }
        self.mem.w16(DS, LOOP_MATCH, 4);
        let f = self.mem.r8(DS, LOOP_FLAGS) | 1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        let f = self.mem.r8(DS, LOOP_FLAGS) & !4;
        self.mem.w8(DS, LOOP_FLAGS, f);
        let v = self.mem.r8(DS, STATS) ^ 1;
        self.mem.w8(DS, STATS, v);
        self.mem.w8(DS, LOOP_FLAGS, 0);
        self.sheet_stats(files)?;
        // 5D08..5D1E: the sheet's loop's next turn, the keys flushed
        // before its wait.
        self.mem.w16(DS, LOOP_MATCH, 0);
        self.key_flush();
        Some(())
    }

    /// A turn of the equipment page's loop (5CF4 over 3276:57D8) up to
    /// the handler of record `rec`: the keys flushed, `key` typed (None,
    /// a click), the records before it passed over (B7C6).
    fn equipment_turn(&mut self, key: Option<u8>, rec: u16) {
        self.key_flush();
        if let Some(k) = key {
            self.ring_typed(k);
        }
        self.mem.w16(DS, LOOP_MATCH, rec);
    }

    /// After a handler that leaves the loop going: its next turn's start
    /// (5D12..5D1E), B7C6 0 and the keys flushed before its wait.
    pub(crate) fn equipment_turn_end(&mut self) {
        self.mem.w16(DS, LOOP_MATCH, 0);
        self.key_flush();
    }

    /// The equipment page's arrows: '+' (3276:57D8's second record,
    /// 1194:1A4A; `on`) shows the list one line on while the items and the
    /// quests' lines (129D, 126C; the spells' count at 50A on their page)
    /// leave seven or more, unless the last seven are shown already; '-'
    /// (the third, 1A94) one back unless at the first. A move puts the
    /// list's background back (21C2) and draws the list again (1DC8;
    /// 0000:6BDE's ten waits for the retrace between, no state). `key`
    /// its key typed, None a click.
    pub fn sheet_equipment_scroll(&mut self, files: &mut Files, key: Option<u8>, on: bool) -> Option<()> {
        self.equipment_turn(key, if on { 1 } else { 2 });
        self.equipment_scrolled(files, on)?;
        self.equipment_turn_end();
        Some(())
    }

    /// 1194:1A4A (`on`) or 1A94: the arrows' handlers.
    fn equipment_scrolled(&mut self, files: &mut Files, on: bool) -> Option<()> {
        let first = self.mem.r8(DS, 0x1297);
        let moved = if on {
            let mut ax = (self.mem.r8(DS, 0x129D) as u16).wrapping_add(self.mem.r16(DS, 0x126C));
            if self.mem.r8(DS, 0x129A) == 0 {
                ax = self.mem.r8(DS, 0x050A) as u16;
            }
            ax >= 7 && first != ax.wrapping_sub(7) as u8
        } else {
            first != 0
        };
        if moved {
            self.mem.w8(DS, 0x1297, if on { first.wrapping_add(1) } else { first.wrapping_sub(1) });
            self.list_back();
            self.list_again(files)?;
        }
        Some(())
    }

    /// A left click on the equipment page's list (3276:57D8's first
    /// record, 1194:1AC6) at row `y`: with another list up (1299) the
    /// page's loop is left (3159:0006), which the port doesn't take (it
    /// can't be: the page's setup clears 1299, 16E0, and only "Drop
    /// what?"'s loop sets it, which takes its list's record itself,
    /// `drop_what_loop`); else the line under it marked (1C5E, `list_mark`). The line
    /// marked already, clicked again, puts its item on or takes it off
    /// (1AFB: `equip_click`, its refusals said in the list's panel: 1BE0,
    /// `cant_equip_said`, and 1B78, `one_only_said`),
    /// the weapon's class made again (731A) and the page drawn again
    /// (23B1). A quest's line marked already, clicked again, says so in
    /// the list's panel (1B6F: 1BE0, 223A; `wait` the front end's wait
    /// for a click or a key, 5536). On the spells' list (129A 0) the line
    /// marked already opens its spell's page (1B3C): false, the caller
    /// runs `sheet_spell_page`; true otherwise.
    pub fn sheet_equipment_click(&mut self, files: &mut Files, y: u16, wait: &mut PanelWait<'_>) -> Option<bool> {
        self.equipment_turn(None, 0);
        if self.mem.r8(DS, 0x1299) != 0 {
            return crate::giveup::not_ported("1194:1ACD: the list's click with another list up (the page's loop left)");
        }
        if !self.equipment_list_click(files, y, wait)? {
            return Some(false);
        }
        self.equipment_turn_end();
        Some(true)
    }

    /// 1194:1AD6..1BCD, the list's left click with no other list up: the
    /// line marked, and the line marked already clicked again (its item
    /// put on or taken off, a refusal or a quest's line said in the
    /// list's panel); false for a spell's line marked already (1B3C).
    fn equipment_list_click(&mut self, files: &mut Files, y: u16, wait: &mut PanelWait<'_>) -> Option<bool> {
        let old = self.mem.r8(DS, 0x1296);
        let slot = self.list_mark(files, y)?;
        let mark = self.mem.r8(DS, 0x1296);
        if mark != 0xFF && mark == old {
            if self.mem.r8(DS, 0x129A) == 0 {
                // 1B3C: the spell's page, which ends the turn
                // (`sheet_spell_page`, the caller's).
                return Some(false);
            }
            if self.mem.r16(DS, 0x1270) != 0 {
                // 1B6F: 748E (its kind, which 1BE0 doesn't read with
                // 1270 set), then 1BE0: "This item is needed..."'s text
                // (3276:5A90) copied to 1044 (1BCE) and shown in the
                // list's panel (223A) in A0h at row A0h.
                self.text_copied((DATA, 0x5A90), NUMBER);
                self.mem.w8(DS, 0x9288, 0xA0);
                self.list_panel_text(NUMBER, 1, 0xA0, 0xA0, wait);
                return Some(true);
            }
            let mut refused = Refusal::default();
            match self.equip_click(&mut refused, slot)? {
                crate::use_item::Equip::Toggled => self.equipment_drawn(files)?,
                crate::use_item::Equip::CantUse => {
                    let (si, cat) = refused.cant_use.unwrap_or_default();
                    self.cant_equip_said(si, cat, wait);
                }
                crate::use_item::Equip::Clash => {
                    let base = refused.clash.unwrap_or_default();
                    self.one_only_said(base, wait);
                }
            }
        }
        Some(true)
    }

    /// A right click on the equipment page's list (3276:57D8's first
    /// record, 1194:1AC6, with AFF9 2) at row `y`: the line under it
    /// marked (1C5E), and the items' list's marked line described
    /// (1CEC: 1988, `item_described`; `hooks` a special item's own box,
    /// `wait` the front end's wait for a click or a key). A right click
    /// never puts an item on (1AF4). With another list up the page's loop
    /// is left (not ported); on the spells' list a line marked already
    /// opens its page (1B3C): false, the caller runs `sheet_spell_page`.
    pub fn sheet_equipment_describe(&mut self, files: &mut Files, hooks: &mut dyn crate::text::TextHooks, y: u16, wait: &mut PanelWait<'_>) -> Option<bool> {
        self.equipment_turn(None, 0);
        if self.mem.r8(DS, 0x1299) != 0 {
            return crate::giveup::not_ported("1194:1ACD: the list's click with another list up (the page's loop left)");
        }
        let old = self.mem.r8(DS, 0x1296);
        // (1C5E describes only from a line: below them it returns first.)
        let mut cx = (self.mem.r8(DS, 0x129D) as u16).wrapping_add(self.mem.r16(DS, 0x126C));
        if self.mem.r8(DS, 0x129A) == 0 {
            cx = self.mem.r8(DS, 0x050A) as u16;
        }
        let hit = (0..cx.min(7)).any(|i| y <= 0x3D + 0x0B * i);
        self.list_mark(files, y)?;
        if hit && self.mem.r8(DS, 0x129A) != 0 {
            self.item_described(files, hooks, wait)?;
        }
        let mark = self.mem.r8(DS, 0x1296);
        if mark != 0xFF && mark == old && self.mem.r8(DS, 0x129A) == 0 {
            // 1B3C, as a left click's (`sheet_spell_page`, the caller's).
            return Some(false);
        }
        self.equipment_turn_end();
        Some(true)
    }

    /// 1194:1988: the marked line described in the list's panel: 12B2 0,
    /// 12B0 3 (the lines), 9288 FEh, TEENYFNT ([102E]). A quest's line
    /// (1270): "This item is needed..." (3276:5AF3) copied to 1044, or
    /// with the staff's pieces (1272) "You currently have %d piece(s)..."
    /// (5B36 with F77), 12B2 1. Else, with items (129D their count, 7472)
    /// and a line marked, its item's description (7CBA, `item_info`)
    /// unless a special item's own box (the carry) took it. Then [12B0]
    /// lines in the list's panel (223A) from row 90h (95h unless four),
    /// the first in 55h unless 12B2, the rest FEh; AFF9 2, B7C4 bit 1
    /// off and ARENAFNT ([102C]) again.
    fn item_described(&mut self, files: &mut Files, hooks: &mut dyn crate::text::TextHooks, wait: &mut PanelWait<'_>) -> Option<()> {
        use crate::use_item::info;
        self.mem.w16(DS, info::ARGS, 0);
        self.mem.w16(DS, info::LINES, 3);
        self.mem.w8(DS, 0x9288, 0xFE);
        self.select_font(0x102E);
        if self.mem.r16(DS, 0x1270) != 0 {
            self.mem.w16(DS, info::ARGS, 1);
            if self.mem.r16(DS, 0x1272) != 0 {
                let pieces = self.mem.r8(DS, 0x0F77) as u16;
                self.sprintf((DS, NUMBER), (DATA, 0x5B36), &[crate::format::Arg::Int(pieces)]);
            } else {
                let mut si = 0x5AF3u16;
                let mut di = NUMBER;
                loop {
                    let c = self.mem.r8(DATA, si);
                    self.mem.w8(DS, di, c);
                    if c == 0 {
                        break;
                    }
                    si = si.wrapping_add(1);
                    di = di.wrapping_add(1);
                }
            }
        } else {
            let (n, _) = self.count_items(DS, ITEMS);
            if n == 0 {
                return Some(());
            }
            self.mem.w8(DS, WORN, n as u8);
            let mark = self.mem.r8(DS, 0x1296);
            if mark == 0xFF {
                return Some(());
            }
            let it = ITEMS.wrapping_add((mark as u16).wrapping_mul(SLOT));
            if self.item_info(files, hooks, it)? {
                self.page_di = None;
                let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
                self.mem.w8(DS, LOOP_FLAGS, f);
                self.select_font(0x102C);
                return Some(());
            }
        }
        let lines = self.mem.r16(DS, info::LINES);
        let y = if lines == 4 { 0x90 } else { 0x95 };
        if self.mem.r16(DS, info::ARGS) == 0 {
            self.mem.w8(DS, 0x9288, 0x55);
        }
        self.list_panel_text(NUMBER, lines, y, 0xFE, wait);
        self.mem.w8(DS, 0xAFF9, 2);
        let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        self.select_font(0x102C);
        Some(())
    }

    /// 1194:1B6F..1B76 then 1BE0 for an item the class can't use (1B1F):
    /// 9288 FEh; for an accessory or a stack (748E's 2 or more, `cat`) or
    /// an item whose word at +6 is 0, "This item can't be equipped."
    /// (3276:5A90, copied to 1044 by 1BCE); otherwise "%ss cannot equip
    /// this item." (3276:3BEB) made at 1044 with the class's name (the
    /// thief's own at 3E82 for class 0Ah, CX as 1B10 left it; else
    /// 2686:0645's, `class_name`). Either in the list's panel (223A) in
    /// A0h at row A0h, DI 5536: the wait.
    fn cant_equip_said(&mut self, si: u16, cat: u16, wait: &mut PanelWait<'_>) {
        self.mem.w8(DS, 0x9288, 0xFE);
        if cat >= 2 || self.mem.r16(DS, si.wrapping_add(6)) == 0 {
            self.text_copied((DATA, 0x5A90), NUMBER);
        } else {
            let class = (self.mem.r8(DS, crate::character::PLAYER + crate::character::field::CLASS) & 0x1F) as u16;
            let (seg, at) = if class == 0x0A { (DS, 0x3E82) } else { self.class_name(DS, crate::character::PLAYER) };
            self.sprintf((DS, NUMBER), (DATA, 0x3BEB), &[crate::format::Arg::Str(seg, at)]);
        }
        self.mem.w8(DS, 0x9288, 0xA0);
        self.list_panel_text(NUMBER, 1, 0xA0, 0xA0, wait);
    }

    /// 1194:1B78 for an accessory clashing with one worn (1B24): its type's
    /// base (7596, `base`) names it in the rings' list (3BA0:1DD1; the
    /// bracelets' at 2028 with 12A0 set; 559F), "You can only equip one
    /// %s." (3276:5A74) made at 1044 and said in the list's panel (223A)
    /// in A0h at row A0h, DI 5536: the wait.
    fn one_only_said(&mut self, base: u16, wait: &mut PanelWait<'_>) {
        let list = if self.mem.r16(DS, 0x12A0) != 0 { 0x2028 } else { 0x1DD1 };
        let at = self.skip_strings(DS, list, base);
        self.sprintf((DS, NUMBER), (DATA, 0x5A74), &[crate::format::Arg::Str(DS, at)]);
        self.mem.w8(DS, 0x9288, 0xA0);
        self.list_panel_text(NUMBER, 1, 0xA0, 0xA0, wait);
    }

    /// 0000:9CA0 (as 1BCE calls it): the text at `from` copied to DS:`to`
    /// with its NUL.
    fn text_copied(&mut self, from: (u16, u16), to: u16) {
        let (seg, mut si) = from;
        let mut di = to;
        loop {
            let c = self.mem.r8(seg, si);
            self.mem.w8(DS, di, c);
            if c == 0 {
                break;
            }
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
        }
    }

    /// 1194:223A: `cx` lines of the text at DS:`si` in the list's panel:
    /// ARENAFNT ([102C]); each line's CR (looked for in its first 1F4h
    /// bytes) made a NUL and the line drawn centred on x 55h at row `bx`
    /// (0000:5364), 0Bh lower each, in 9288 (the caller's for the first,
    /// `dl` after). The page shown (9466); the wait for the buttons'
    /// release (5E21) and for a click or a key (DI 5536) are `wait`'s.
    /// Then the panel's message place put back from bank [1032] (599C:
    /// 0, 8Fh; ABh by 2Ch, as 1738 kept it), the bank before mapped
    /// again, and the page shown.
    fn list_panel_text(&mut self, si: u16, cx: u16, bx: u16, dl: u8, wait: &mut PanelWait<'_>) {
        // (Its callers here, 1A0A, 1C0A, 1815, 1981, give it DI 5536, its
        // wait, which it keeps for theirs: `page_di`.)
        self.page_di = Some(0x5536);
        let _ = self.list_panel_then(si, cx, bx, dl, &mut |m, lines, top| {
            wait(m, lines, top);
            Some(())
        });
    }

    /// `list_panel_text` with `then` its DI's routine after the lines are
    /// shown (5E21, CALL DI; 5536 the wait, 2325 "Drop what?"'s loop).
    fn list_panel_then(&mut self, si: u16, cx: u16, bx: u16, dl: u8, then: &mut PanelThen<'_>) -> Option<()> {
        self.select_font(0x102C);
        let top = bx;
        let mut lines = Vec::new();
        let (mut si, mut bx) = (si, bx);
        for _ in 0..cx {
            let mut di = si;
            for _ in 0..0x1F4 {
                let c = self.mem.r8(DS, di);
                di = di.wrapping_add(1);
                if c == 0x0D {
                    break;
                }
            }
            self.mem.w8(DS, di.wrapping_sub(1), 0);
            lines.push((self.mem.cstr(DS, si).to_vec(), self.mem.r8(DS, 0x9288)));
            let w = self.text_width(si);
            si = self.page_text_at(0x55u16.wrapping_sub(w >> 1), bx, (DS, si));
            bx = bx.wrapping_add(0x0B);
            self.mem.w8(DS, 0x9288, dl);
        }
        then(self, &lines, top)?;
        let bank = self.mem.r16(DS, 0x1032);
        let old = self.map_bank(bank, 0);
        if self.page_is_block() {
            let frame = self.mem.r16(DS, FRAME);
            self.page_rect_put((frame, 0), 0, 0x8F, 0xAB, 0x2C);
        }
        self.map_bank(old, 0);
        Some(())
    }

    /// D or the Drop button (3276:57D8's fourth record, 1194:17D8), `key`
    /// its key typed (None, a click): the drop button's rules
    /// (`drop_button`), its question the yes-or-no menu (48E9: 020E with
    /// the text at 1044 and the entries at 4B63, `menu_boxes` over
    /// `screen` and `menu_closed`, `pick` the front end's answer, 0 yes);
    /// a drop draws the page again (1CF0's 23B1). Its refusals (no room,
    /// a quest's item) are said in the list's panel (223A, `wait` the
    /// front end's). With no line marked, "Drop what?" (18CA) is said
    /// there (in FEh at row A0h, `wait` shown it) and its DI, 2325, runs
    /// the page's loop over its first five records with 1299 set, `what`
    /// the front end's inputs (`drop_what_loop`, whose D runs this again
    /// within); a line marked by it starts the drop over (18F3).
    pub fn sheet_equipment_drop(&mut self, files: &mut Files, key: Option<u8>, screen: Option<&[u8]>, pick: &mut dyn FnMut(&mut Machine) -> u16, wait: &mut PanelWait<'_>, what: &mut dyn FnMut(&mut Machine) -> PageInput) -> Option<()> {
        self.equipment_turn(key, 3);
        self.equipment_dropped(files, screen, pick, wait, what)?;
        self.equipment_turn_end();
        Some(())
    }

    /// 1194:17D8, the drop button's handler, as the page's loop or "Drop
    /// what?"'s loop within (D there) calls it.
    fn equipment_dropped(&mut self, files: &mut Files, screen: Option<&[u8]>, pick: &mut dyn FnMut(&mut Machine) -> u16, wait: &mut PanelWait<'_>, what: &mut dyn FnMut(&mut Machine) -> PageInput) -> Option<()> {
        use crate::use_item::DropButton;
        loop {
            let mut failed = false;
            let r = self.drop_button_drawn(
                &mut |m| {
                    let Some(boxes) = m.menu_boxes(crate::use_item::var::DROP_TEXT, 0x4B63, screen) else {
                        failed = true;
                        return false;
                    };
                    let answer = if boxes.is_some() { pick(m) } else { 0xFFFF };
                    m.menu_closed(boxes.as_ref());
                    answer == 0
                },
                &mut |m| m.equipment_drawn(files),
            )?;
            if failed {
                return None;
            }
            match r {
                DropButton::NoRoom | DropButton::CantDrop => {
                    // 1962, 17FB: the refusal's text (at 1044) in the list's
                    // panel (223A) in A0h at row A0h, DI 5536: the wait.
                    self.list_panel_text(crate::use_item::var::DROP_TEXT, 1, 0xA0, 0xA0, wait);
                }
                DropButton::DropWhat => {
                    // 18CA: the text in FEh at row A0h (the colour after
                    // FEh too), DI 2325 (kept after: `page_di`).
                    self.list_panel_then(crate::use_item::var::DROP_TEXT, 1, 0xA0, 0xFE, &mut |m, lines, top| {
                        wait(m, lines, top);
                        m.drop_what_loop(files, screen, pick, wait, what)
                    })?;
                    self.page_di = Some(0x2325);
                    if self.mem.r8(DS, 0x1296) != 0xFF {
                        continue;
                    }
                }
                DropButton::Nothing | DropButton::Dropped => {}
            }
            return Some(());
        }
    }

    /// 1194:2325, "Drop what?"'s DI: 1299 1 (another list up) and the
    /// page's loop (5CF4: B7C4 bit 0 off, then turns until a handler sets
    /// it) over 3276:57D8's first five records, `what` each turn's input
    /// (5D18, 5DE2: B7C6 0, the keys flushed, the input read; a key put in
    /// the ring whether a record takes it or not). The list's record
    /// (a click on it, or Space: 1AC6) with 1299 set waits for the
    /// buttons' release and leaves (3159:0006); with 1299 clear, as a drop
    /// within leaves it, it is the list's click (`equipment_list_click`).
    /// The arrows' ('+', '-': `equipment_scrolled`) go on; the drop's (D:
    /// 17D8 within, `equipment_dropped`, itself saying "Drop what?" with
    /// this loop again while no line is marked) goes on unless it left
    /// B7C4 bit 0 set; the exit's (Esc, 3159:0006) leaves. Then no line
    /// marked, and unless the record that left was the exit's (B7C6 4)
    /// the line under the cursor marked (1C5E: the click's row; a key's,
    /// the cursor's at 8F94); 1299 0, B7C4 bit 0 off.
    fn drop_what_loop(&mut self, files: &mut Files, screen: Option<&[u8]>, pick: &mut dyn FnMut(&mut Machine) -> u16, wait: &mut PanelWait<'_>, what: &mut dyn FnMut(&mut Machine) -> PageInput) -> Option<()> {
        self.mem.w8(DS, 0x1299, 1);
        let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        let y = loop {
            self.mem.w16(DS, LOOP_MATCH, 0);
            self.key_flush();
            let input = what(self);
            if let PageInput::Key(k) = input {
                self.ring_typed(k);
            }
            // 5D24..5D39: each record in turn, its key (a letter made a
            // capital, 5D75) or its rectangle (5D8D, edges included); B7C6
            // counts the records passed over.
            let hit = (0..5u16).find(|&i| {
                let r = PAGE_BUTTONS.wrapping_add(0x62 * i);
                match input {
                    PageInput::Key(k) => k != 0 && k.to_ascii_uppercase() == self.mem.r8(DATA, r.wrapping_add(0x59)),
                    PageInput::Click(x, y) => {
                        let w = |o: u16| self.mem.r16(DATA, r.wrapping_add(o));
                        x >= w(0) && y >= w(2) && x <= w(0).wrapping_add(w(4)) && y <= w(2).wrapping_add(w(6))
                    }
                }
            });
            let Some(rec) = hit else {
                self.mem.w16(DS, LOOP_MATCH, 5);
                continue;
            };
            self.mem.w16(DS, LOOP_MATCH, rec);
            let row = match input {
                PageInput::Click(_, y) => y,
                PageInput::Key(_) => self.mem.r16(DS, 0x8F94),
            };
            match rec {
                0 if self.mem.r8(DS, 0x1299) != 0 => {
                    let f = self.mem.r8(DS, LOOP_FLAGS) | 1;
                    self.mem.w8(DS, LOOP_FLAGS, f);
                }
                0 => {
                    // (A spell's page, 1B3C, can't be: "Drop what?" is only
                    // said on the own items' list, 129A 1, `drop_button`.)
                    if !self.equipment_list_click(files, row, wait)? {
                        return crate::giveup::not_ported("1194:1B3C within the loop of \"Drop what?\" (a spell's page)");
                    }
                }
                1 | 2 => self.equipment_scrolled(files, rec == 1)?,
                3 => self.equipment_dropped(files, screen, pick, wait, what)?,
                _ => {
                    let f = self.mem.r8(DS, LOOP_FLAGS) | 1;
                    self.mem.w8(DS, LOOP_FLAGS, f);
                }
            }
            if self.mem.r8(DS, LOOP_FLAGS) & 1 != 0 {
                break row;
            }
        };
        self.mem.w8(DS, 0x1296, 0xFF);
        if self.mem.r16(DS, LOOP_MATCH) != 4 {
            self.list_mark(files, y)?;
        }
        self.mem.w8(DS, 0x1299, 0);
        let f = self.mem.r8(DS, LOOP_FLAGS) & !1;
        self.mem.w8(DS, LOOP_FLAGS, f);
        Some(())
    }

    /// S or the Spells button (3276:57D8's sixth record, 1194:2365), `key`
    /// its key typed (None, a click): a caster (1A9 bit 20h) turns the
    /// list to the spells' (129A flipped) or back to the items', the
    /// first line marked and shown first, and the page drawn again (23B1);
    /// anyone else gets the box of 3276:5AD7 (1194:59CC, `message_shown`
    /// over `screen`).
    pub fn sheet_equipment_spells(&mut self, files: &mut Files, key: Option<u8>, screen: Option<&[u8]>) -> Option<()> {
        self.equipment_turn(key, 5);
        if self.mem.r8(DS, 0x01A9) & 0x20 != 0 {
            let v = self.mem.r8(DS, 0x129A) ^ 1;
            self.mem.w8(DS, 0x129A, v);
            self.mem.w8(DS, 0x1296, 0);
            self.mem.w8(DS, 0x1297, 0);
            self.equipment_drawn(files)?;
        } else {
            self.message_shown((DATA, 0x5AD7), screen)?;
        }
        self.equipment_turn_end();
        Some(())
    }

    /// 1194:1C5E: the line a click at row `y` marks (1296): of the lines
    /// shown (129D and 126C, or the spells' 50A; at most seven) the first
    /// whose bottom (3Dh, then each 0Bh lower) is at or below it, plus
    /// the first shown (1297); none (FFh) below them or with no lines. A
    /// new mark puts the list's background back and draws the list again
    /// (21C2, 1DC8). Then 1270 and 1272 0, and a mark past the items is a
    /// quest's line (1270 1; 1F56, `staff_line`). Returns the line's number (DX) for the click's equipping.
    fn list_mark(&mut self, files: &mut Files, y: u16) -> Option<u16> {
        let mut cx = (self.mem.r8(DS, 0x129D) as u16).wrapping_add(self.mem.r16(DS, 0x126C));
        if self.mem.r8(DS, 0x129A) == 0 {
            cx = self.mem.r8(DS, 0x050A) as u16;
        }
        let shown = cx.min(7);
        let Some(row) = (0..shown).find(|&i| y <= 0x3D + 0x0B * i) else {
            self.mem.w8(DS, 0x1296, 0xFF);
            return Some(shown);
        };
        let dl = (row as u8).wrapping_add(self.mem.r8(DS, 0x1297));
        let dx = dl as u16;
        if self.mem.r8(DS, 0x1296) != dl {
            self.mem.w8(DS, 0x1296, dl);
            self.list_back();
            self.list_again(files)?;
        }
        self.mem.w16(DS, 0x1270, 0);
        self.mem.w16(DS, 0x1272, 0);
        if self.mem.r8(DS, 0x1296) >= self.mem.r8(DS, 0x129D) {
            self.mem.w16(DS, 0x1270, 1);
            self.staff_line(files)?;
        }
        Some(dx)
    }

    /// 1194:1F56: with the staff's pieces held (F77), the quests' lines
    /// counted again (126C: A206 and 2433:083D into DS:DI, 1272 0) and
    /// the marked line the staff's (1272 1) when it is the last: the
    /// count plus the items' (in AL alone) one past the mark. Its DI is
    /// the one the page's loop carries (`page_di`): a new mark's 1DC8
    /// left it at 1044h (1EB8, the quests' lines drawn), but the same
    /// line clicked again after a description or a note in the list's
    /// panel finds 5536h, their 223A's, and the lines go over DGROUP
    /// there (bug kept). Both leave DI as they found it.
    fn staff_line(&mut self, files: &mut Files) -> Option<()> {
        if self.mem.r8(DS, 0x0F77) == 0 {
            return Some(());
        }
        let Some(di) = self.page_di else {
            return crate::giveup::not_ported("1194:1F56: its DI as a handler the port doesn't follow left it");
        };
        self.mem.w16(DS, 0x1272, 0);
        let n = self.quest_lines_counted_at(files, di)?;
        let al = (n as u8).wrapping_add(self.mem.r8(DS, 0x129D));
        let ax = n & 0xFF00 | al as u16;
        if ax == self.mem.r8(DS, 0x1296) as u16 + 1 {
            self.mem.w16(DS, 0x1272, 1);
        }
        Some(())
    }

    /// 1194:21C2: the list's background (0Ch, 2Fh; 92h by 58h) put back
    /// from bank 4 at C350h (599C), the bank mapped before put back.
    pub(crate) fn list_back(&mut self) {
        let old = self.map_bank(4, 0);
        if self.page_is_block() {
            let frame = self.mem.r16(DS, FRAME);
            self.page_rect_put((frame, 0xC350), 0x0C, 0x2F, 0x92, 0x58);
        }
        self.map_bank(old, 0);
    }

    /// 1194:1DC8 with SI the list 129A picks: the player's items (212h)
    /// or the spells (50Bh).
    pub(crate) fn list_again(&mut self, files: &mut Files) -> Option<()> {
        self.equipment_list(files)
    }

    /// 1194:1D5C: the spells' list (129A 0), the spells' numbers at 50Bh
    /// ([50A] of them): up to seven lines from [1297] (from 9Fh on, A0h
    /// less it), each at (0Eh, 32h + 0Bh a line) in 53h, the marked one
    /// ([1296]) boxed first in A0h (22AF), its name (29CF:0650) at 1044.
    fn sheet_spell_list(&mut self) {
        let n = self.mem.r8(DS, 0x050A) as u16;
        if n == 0 {
            return;
        }
        let first = self.mem.r8(DS, 0x1297);
        let mut cx = n;
        if cx >= 7 {
            cx = 7;
            if first >= 0x9F {
                cx = 0xA0u8.wrapping_sub(first) as u16;
            }
        }
        let (mut ax, mut bx) = (first as u16, 0x32u16);
        let mut si = 0x050Bu16.wrapping_add(first as u16);
        for _ in 0..cx {
            self.mem.w8(DS, 0x9288, 0x53);
            if self.mem.r8(DS, 0x1296) == ax as u8 {
                self.mem.w8(DS, 0x9288, 0xA0);
                self.line_box(bx);
                self.mem.w8(DS, 0x9288, 0x53);
            }
            let spell = self.mem.r8(DS, si);
            self.spell_name_copy(spell, NUMBER);
            self.page_text_at(0x0E, bx, (DS, NUMBER));
            si = si.wrapping_add(1);
            bx = bx.wrapping_add(0x0B);
            ax = ax.wrapping_add(1);
        }
    }

    /// 1194:16E0: the equipment page made: the clothes' names (1D22), no
    /// other list (1299 0, 1270, 1272 0), the quests' lines counted at
    /// 126C (A206 and 2433:083D into 1044), the own items' list (129A 1)
    /// with its first line selected and shown first (1296, 1297 0), the
    /// page's back (1738) and the page drawn (23B1).
    fn equipment_setup(&mut self, files: &mut Files) -> Option<()> {
        self.sheet_clothes();
        self.mem.w8(DS, 0x1299, 0);
        self.mem.w16(DS, 0x1270, 0);
        self.mem.w16(DS, 0x1272, 0);
        self.mem.w8(DS, NUMBER, 0);
        self.quest_lines_counted(files)?;
        self.mem.w8(DS, 0x9288, 0xFE);
        self.mem.w8(DS, 0x129A, 1);
        self.mem.w8(DS, 0x1296, 0);
        self.mem.w8(DS, 0x1297, 0);
        self.equipment_back(files)?;
        self.equipment_drawn(files)
    }

    /// 1194:A206 and 2433:083D into 1044, their lines counted at 126C.
    fn quest_lines_counted(&mut self, files: &mut Files) -> Option<u16> {
        self.quest_lines_counted_at(files, NUMBER)
    }

    /// `quest_lines_counted` into DS:`di`.
    fn quest_lines_counted_at(&mut self, files: &mut Files, di: u16) -> Option<u16> {
        use crate::questitems::var::COUNT;
        self.mem.w16(DS, COUNT, 0);
        let more = self.quest_item_names(files, di)?;
        let n = self.mem.r16(DS, COUNT).wrapping_add(more);
        self.mem.w16(DS, COUNT, n);
        Some(n)
    }

    /// 1194:1738: the page cleared (9426), CHARSHET.$$$ over it (1647),
    /// the page copied to bank 5 (7702), EQUIP.IMG drawn (5CD9), the doll's
    /// place (0Ch, 2Fh; 92h by 58h) kept in bank 4 at C350h and the
    /// message's (0, 8Fh; ABh by 2Ch) in bank [1032] and written to
    /// BACKMSG.$$$ (8F26, 9DD2), bank 5 left mapped; CHARSHT.COL's colours
    /// (1695).
    fn equipment_back(&mut self, files: &mut Files) -> Option<()> {
        self.page_clear();
        self.sheet_back(files)?;
        self.map_bank(5, 0);
        let frame = self.mem.r16(DS, FRAME);
        let page = self.mem.r16(DS, 0xA900);
        for i in 0..0xFA00u16 {
            let b = self.mem.r8(page, i);
            self.mem.w8(frame, i, b);
        }
        self.sheet_image(files, EQUIP_IMG)?;
        let old = self.map_bank(4, 0);
        let frame = self.mem.r16(DS, FRAME);
        self.page_rect_save((frame, 0xC350), 0x0C, 0x2F, 0x92, 0x58);
        self.map_bank(old, 0);
        let bank = self.mem.r16(DS, 0x1032);
        let old = self.map_bank(bank, 0);
        let frame = self.mem.r16(DS, FRAME);
        self.page_rect_save((frame, 0), 0, 0x8F, 0xAB, 0x2C);
        let path = self.save_path(BACKMSG);
        self.file_write(files, path, frame, 0, 0x1D64);
        self.map_bank(old, 0);
        self.sheet_colours(files)
    }

    /// 1194:23B1: the page given back from bank 5 (21F1: 5B04, its 0s left
    /// out) and the doll's place from bank 4 (21C2: 599C), EQUIP.IMG drawn
    /// again (the spells' pictures too with the spells' list, 129A 0),
    /// the doll (23E5), the name, race, class and level (22F5), the list
    /// (1DC8) and the armour's classes (76B4).
    fn equipment_drawn(&mut self, files: &mut Files) -> Option<()> {
        let old = self.map_bank(5, 0);
        let frame = self.mem.r16(DS, FRAME);
        let block = self.page_is_block();
        if block {
            let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, 0xA900));
            for i in 0..0xFA00u16 {
                let b = self.mem.r8(frame, i);
                if b != 0 {
                    self.mem.w8(seg, off.wrapping_add(i), b);
                }
            }
        }
        let back = self.map_bank(4, 0);
        let frame = self.mem.r16(DS, FRAME);
        if block {
            self.page_rect_put((frame, 0xC350), 0x0C, 0x2F, 0x92, 0x58);
        }
        self.map_bank(back, 0);
        self.map_bank(old, 0);
        self.sheet_image(files, EQUIP_IMG)?;
        if self.mem.r8(DS, 0x129A) == 0 {
            self.sheet_image(files, 0x3E94)?;
            self.sheet_image(files, 0x3E89)?;
        }
        self.doll_drawn(files)?;
        // 22F5 (0E19): the name, the race, the class.
        self.mem.w8(DS, 0x9288, 0xFE);
        self.page_text_at(0x0B, 0x07, (DS, PLAYER + 9));
        let race = self.mem.r8(DS, PLAYER + 4) as u16;
        let si = self.skip_strings(DS, 0x2B94, race);
        self.page_text_at(0x0B, 0x10, (DS, si));
        let class = self.class_name(DS, PLAYER);
        self.page_text_at(0x0B, 0x19, class);
        // 5634: the level, at 929E (7052).
        let level = self.mem.r8(DS, PLAYER + 6) as u16 + 1;
        self.utoa_nul(level, SCRATCH);
        self.page_text_at(0x80, 0x17, (DS, SCRATCH));
        self.equipment_list(files)?;
        // 76B4 with BX the sex (+7): the armour's classes (7719) at the
        // places of DGROUP's 5D8D (5DA9 for a woman) seven (48F5), in
        // 8Ah, then again one to the left in C3h. (DS is DGROUP here,
        // from 23E4: the segment 3276h's words there are 0s.)
        let woman = self.mem.r8(DS, PLAYER + 7) != 0;
        self.player_armor_class();
        let table = if woman { 0x5DA9 } else { 0x5D8D };
        for (colour, dx) in [(0x8Au8, 0u16), (0xC3, 1)] {
            self.mem.w8(DS, 0x9288, colour);
            for i in 0..7u16 {
                let cx = self.mem.r8(DS, crate::inventory::ARMOR_CLASS + i) as i8 as i16 as u16;
                let x = self.mem.r16(DS, table + 4 * i).wrapping_sub(dx);
                let y = self.mem.r16(DS, table + 4 * i + 2);
                self.sheet_signed(cx, 0);
                self.page_text_at(x, y, (DS, NUMBER));
            }
        }
        // (DI as 76B4's drawing leaves it, which the port doesn't follow.)
        self.page_di = None;
        Some(())
    }

    /// 1194:1DC8 for the own items (129A 1): no other list (1270, 1272
    /// 0), TEENYFNT ([102E]); up to seven lines from [1297] (fewer past
    /// the 28h slots), each slot in use (+0Eh 40h) counted at 126E and
    /// drawn at (0Eh, 32h + 0Bh a line) in A0h, 58h one the class can use
    /// (7772), 55h worn, the selected one ([1296]) boxed (22AF) and an
    /// enchanted known one (+0Eh 1 and 2) 68h, 65h worn; a stack's (kind
    /// 3) unknown name with its count (1F30), any other's 7388. Under
    /// seven, the quests' lines after them (1EB8: A206 and 2433:083D into
    /// 1044 again) in A0h, the selected one boxed. Then [102C]'s font.
    fn equipment_list(&mut self, files: &mut Files) -> Option<()> {
        self.mem.w16(DS, 0x1270, 0);
        self.mem.w16(DS, 0x1272, 0);
        self.select_font(0x102E);
        // (Its DI after: 1044h where it draws the quests' lines, 1EB8;
        // else as its drawing leaves it, which the port doesn't follow.)
        self.page_di = None;
        if self.mem.r8(DS, 0x129A) == 0 {
            self.sheet_spell_list();
            self.select_font(0x102C);
            return Some(());
        }
        let first = self.mem.r8(DS, 0x1297) as u16;
        let mut cx = 7u16;
        if first + 7 > 0x28 {
            cx = 0x28u8.wrapping_sub(first as u8) as u16;
        }
        let mut si = ITEMS.wrapping_add(first.wrapping_mul(SLOT));
        let (mut ax, mut bx) = (first, 0x32u16);
        self.mem.w16(DS, LIST_SHOWN, 0);
        for _ in 0..cx {
            let flags = self.mem.r8(DS, si.wrapping_add(0x0E));
            if flags & 0x40 != 0 {
                let n = self.mem.r16(DS, LIST_SHOWN).wrapping_add(1);
                self.mem.w16(DS, LIST_SHOWN, n);
                self.mem.w8(DS, 0x9288, 0xA0);
                let class = (self.mem.r8(DS, 0x01A9) & 0x1F) as u16;
                if self.class_can_use(DS, si, class)? {
                    self.mem.w8(DS, 0x9288, if flags & 0x80 != 0 { 0x55 } else { 0x58 });
                }
                if self.mem.r8(DS, 0x1296) == ax as u8 {
                    let c = self.mem.r8(DS, 0x9288);
                    self.mem.w8(DS, 0x9288, 0xA0);
                    self.line_box(bx);
                    self.mem.w8(DS, 0x9288, c);
                }
                if flags & 1 != 0 && flags & 2 != 0 {
                    self.mem.w8(DS, 0x9288, if flags & 0x80 != 0 { 0x65 } else { 0x68 });
                }
                let name = if self.item_category(DS, si) == 3 && flags & 2 == 0 {
                    // 1F30: "%s (%d)" (3276:5B6D), the name and +1 plus one.
                    let count = self.mem.r16(DS, si.wrapping_add(1)).wrapping_add(1);
                    let name = self.display_name(si)?;
                    self.sprintf((DS, NUMBER), (DATA, 0x5B6D), &[crate::format::Arg::Str(DS, name), crate::format::Arg::Int(count)]);
                    NUMBER
                } else {
                    self.display_name(si)?
                };
                self.page_text_at(0x0E, bx, (DS, name));
            }
            si = si.wrapping_add(SLOT);
            bx = bx.wrapping_add(0x0B);
            ax = ax.wrapping_add(1);
        }
        if self.mem.r16(DS, LIST_SHOWN) != 7 {
            self.page_di = Some(NUMBER);
            let n = self.quest_lines_counted(files)?;
            if n != 0 {
                self.mem.w8(DS, 0x9288, 0xA0);
                let shown = self.mem.r16(DS, LIST_SHOWN);
                let room = 7u16.wrapping_sub(shown);
                let cx = if room < n { room } else { n };
                let mut si = NUMBER;
                let mut bx = shown.wrapping_mul(0x0B).wrapping_add(0x33);
                for _ in 0..cx {
                    si = self.page_text_at(0x0E, bx, (DS, si));
                    let dl = (self.mem.r16(DS, LIST_SHOWN) as u8).wrapping_add(self.mem.r8(DS, 0x1297));
                    if dl == self.mem.r8(DS, 0x1296) {
                        self.line_box(bx);
                    }
                    let v = self.mem.r16(DS, LIST_SHOWN).wrapping_add(1);
                    self.mem.w16(DS, LIST_SHOWN, v);
                    bx = bx.wrapping_add(0x0B);
                }
            }
        }
        self.select_font(0x102C);
        Some(())
    }

    /// 1194:22AF: the box round a selected line at row `bx` (0000:7334's
    /// four lines from (0Ch, `bx` - 2) by 9Dh and 9Ch, `bx` + 0Ah), ADB6
    /// and ADB8 left at the last line's end (each line starts at them).
    fn line_box(&mut self, bx: u16) {
        let y = bx.wrapping_sub(2);
        self.mem.w16(DS, 0xADB6, 0x0C);
        self.mem.w16(DS, 0xADB8, y);
        self.line_to(0x9D, y);
        self.line_to(0x9C, y.wrapping_add(0x0C));
        let v = self.mem.r16(DS, 0xADB8).wrapping_sub(1);
        self.mem.w16(DS, 0xADB8, v);
        self.line_to(0x0B, y.wrapping_add(0x0C));
        let v = self.mem.r16(DS, 0xADB8).wrapping_sub(1);
        self.mem.w16(DS, 0xADB8, v);
        self.line_to(0x0C, y);
    }

    /// 0000:7334: a line from (ADB6, ADB8) to (`ax`, `bx`) in [9288],
    /// drawn from whichever end is higher on the page (the lower y, the
    /// other where they're level) one pixel a step of the longer axis,
    /// that many steps (the far end left undrawn; scaled by ADBA / 100h
    /// where ADBA is at most FFh, 73E6), the shorter axis stepped as the
    /// error passes 0; then ADB6 and ADB8 at (`ax`, `bx`). A line of no
    /// length draws nothing and leaves them. (Drawn where the page is the
    /// game's own block.)
    fn line_to(&mut self, ax: u16, bx: u16) {
        let (mut cx, mut dx) = (self.mem.r16(DS, 0xADB6), self.mem.r16(DS, 0xADB8));
        let (mut x, mut y) = (ax, bx);
        if y >= dx {
            std::mem::swap(&mut cx, &mut x);
            std::mem::swap(&mut dx, &mut y);
        }
        let dy = dx.wrapping_sub(y);
        let wide = cx.wrapping_sub(x);
        if wide == 0 && dy == 0 {
            return;
        }
        let back = (wide as i16) < 0;
        let wide = if back { wide.wrapping_neg() } else { wide };
        let (major, minor, x_major) = if wide >= dy { (wide, dy, true) } else { (dy, wide, false) };
        // 73E6.
        let scale = self.mem.r16(DS, 0xADBA);
        let mut n = major;
        if (scale as i16) <= 0xFF {
            n = ((n as u32 * scale as u32) >> 8) as u16;
        }
        if self.page_is_block() {
            let page = self.mem.r16(DS, 0xA900);
            let colour = self.mem.r8(DS, 0x9288);
            let mut di = self.mem.r16(DS, 0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
            let step = if back { 0xFFFFu16 } else { 1 };
            let mut si = major.wrapping_neg();
            // (LOOP: a count scaled to 0 runs 10000h times.)
            for _ in 0..if n == 0 { 0x10000 } else { n as u32 } {
                self.mem.w8(page, di, colour);
                di = di.wrapping_add(if x_major { step } else { 0x140 });
                si = si.wrapping_add(minor);
                if (si as i16) >= 0 {
                    si = si.wrapping_sub(major);
                    di = di.wrapping_add(if x_major { 0x140 } else { step });
                }
            }
        }
        self.mem.w16(DS, 0xADB6, ax);
        self.mem.w16(DS, 0xADB8, bx);
    }

    /// 0000:9E06: the text at `seg`:`si` drawn at (`x`, `y`) (into the page
    /// where it is the game's own block, else its state alone:
    /// `Machine::text_state`); SI past its NUL.
    pub(crate) fn page_text_at(&mut self, x: u16, y: u16, (seg, si): (u16, u16)) -> u16 {
        if self.page_is_block() {
            self.page_text((seg, si), x, y);
        } else {
            self.text_state_far(x, y, (seg, si));
        }
        let mut at = si;
        while self.mem.r8(seg, at) != 0 {
            at = at.wrapping_add(1);
        }
        at.wrapping_add(1)
    }

    /// 1194:55B6: the colours at 929E black (the fade to them, 5E3E, the
    /// front end's) and the page cleared (55D9: 9426; the screen's own
    /// clearing the front end's).
    pub(crate) fn sheet_black(&mut self) {
        for i in 0..0x300u16 {
            self.mem.w8(DS, SCRATCH + i, 0);
        }
        self.page_clear();
    }

    /// 0000:9426: the page ([A8FE], FA00h bytes) cleared unless A902 bit
    /// 4 (here only where the page is the game's own block).
    pub(crate) fn page_clear(&mut self) {
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
    pub(crate) fn sheet_keep(&mut self, files: &mut Files) -> Option<()> {
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
    pub(crate) fn cif_frame(&self, k: u16) -> u16 {
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
    pub(crate) fn sheet_image(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let bank = self.mem.r16(DS, 0x1030);
        let old = self.map_bank(bank, 0);
        self.sheet_image_here(files, si)?;
        self.map_bank(old, 0);
        Some(())
    }

    /// 0000:56C8 with BX 4 or 14h: the picture named at DS:`si` loaded to
    /// the EMS frame's start (742C), its header's words to A836..A83C,
    /// drawn into the page (582C).
    pub(crate) fn sheet_image_here(&mut self, files: &mut Files, si: u16) -> Option<()> {
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
        self.sheet_back(files)?;
        self.sheet_doll(files)?;
        self.sheet_image(files, CHARSTAT)?;
        self.sheet_texts();
        self.sheet_image(files, PAGE2)?;
        self.sheet_colours(files)
    }

    /// 1194:1647: CHARSHET.$$$ loaded into bank [1030] and drawn over
    /// the page (5B04, its 0s left out), the bank before mapped again.
    pub(crate) fn sheet_back(&mut self, files: &mut Files) -> Option<()> {
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
        Some(())
    }

    /// 1194:1695: CHARSHT.COL's colours read to 929E (0000:7530: a file
    /// of other than 300h bytes its colours past an 8-byte header, made
    /// 6-bit); their setting (9B4E) the front end's.
    pub(crate) fn sheet_colours(&mut self, files: &mut Files) -> Option<()> {
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
    pub(crate) fn sheet_doll(&mut self, files: &mut Files) -> Option<()> {
        self.sheet_clothes();
        self.doll_drawn(files)
    }

    /// 1194:1D22: the clothes' names by sex and class (SHIRT's two
    /// letters, the trousers' and the helmet's first).
    pub fn sheet_clothes(&mut self) {
        let bl = if self.mem.r8(DS, PLAYER + 5) & 0x20 != 0 { b'r' } else { b's' };
        let al = if self.mem.r8(DS, PLAYER + 7) != 0 { b'f' } else { b'm' };
        self.mem.w8(DS, SHIRT, al);
        self.mem.w8(DS, SHIRT + 1, bl);
        self.mem.w8(DS, PANTS, al);
        self.mem.w8(DS, HELMET, al);
    }

    /// 1194:23E5: the doll (see `sheet_doll`).
    fn doll_drawn(&mut self, files: &mut Files) -> Option<()> {
        self.map_bank(0x17, 0);
        let (n, _) = self.count_items(DS, ITEMS);
        self.mem.w8(DS, WORN, n as u8);
        if n == 0 || self.worn_find(5, 0, n).is_none() {
            let path = self.save_path(SAVEFACE);
            self.sheet_image(files, path)?;
        }
        self.sheet_image(files, PANTS)?;
        let n = self.mem.r8(DS, WORN) as u16;
        // (Each picture 5CD9 draws leaves DI 0; each item's DI goes on to
        // the next: `sheet_item`.)
        if let Some(si) = self.worn_find(2, 0, n) {
            self.sheet_item(files, si, 0)?;
        }
        self.sheet_image(files, SHIRT)?;
        let (mut si, mut di) = (ITEMS, 0);
        for _ in 0..self.mem.r8(DS, WORN) {
            if self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x80 != 0 && !(self.mem.r8(DS, si) == 2 && self.item_category(DS, si) == 0) {
                di = self.sheet_item(files, si, di)?;
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

    /// 1194:2462: the equipment's frames (nEQUIP.CIF, n the player's +7)
    /// loaded into bank 17h, left mapped.
    pub fn equip_frames(&mut self, files: &mut Files) -> Option<()> {
        let n = self.mem.r8(DS, 0x01AB).wrapping_add(b'0');
        self.mem.w8(DS, EQUIP, n);
        self.map_bank(0x17, 0);
        let frame = self.mem.r16(DS, FRAME);
        self.file_load(files, EQUIP, frame, 0)?;
        Some(())
    }

    /// 1194:1F8D: the worn item at DS:`si` on the doll, bank 17h mapped
    /// (`equip_frames`): its frame there (5CCC: weapons by type, the
    /// armour's by type and kind, 21A8; none for kind 2), drawn as it is
    /// or, coloured (+0Eh bit 1, its colour +10h at 12A4), its header
    /// copied to 12A8 and its frame to 929E and recoloured through bank 4
    /// (`item_coloured`); after any but a coloured one, a helmet (type 5,
    /// kind 0) by the race (`sheet_helmet`). `di` is DI as 1F8D finds it,
    /// which race 7's helmet loads its picture at; DI after it (C350h
    /// after a coloured one, 929Eh after races 1, 3 and 4's helmet, else
    /// as it was).
    pub fn sheet_item(&mut self, files: &mut Files, si: u16, di: u16) -> Option<u16> {
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
                // (No helmet after a coloured item: 201C's RET.)
                return self.item_coloured().map(|_| 0xC350);
            }
            if self.page_is_block() {
                self.page_image(frame, img);
            }
        }
        if self.mem.r8(DS, si) == 5 && self.item_category(DS, si) == 0 {
            return self.sheet_helmet(files, si, di);
        }
        Some(di)
    }

    /// 1194:20BF: the worn item's frame copied to 929E recoloured: bank 4
    /// mapped and its first C350h bytes cleared, the frame unpacked to
    /// its start (5828 with [8F9C] the EMS frame: `image_capture`), 929E
    /// made a table of each colour to itself but from 938C the item's
    /// colours: 13 from 3EA0's byte for its colour, or for colour 2 seven
    /// then 80h..84h (12A6 counting the second part); the bank's C350h
    /// bytes mapped through the table and its first [12ACh] x [12AEh]
    /// drawn at (12A8, 12AA), 0s left out (5B04, where the page is the
    /// game's own block); the bank before mapped again.
    fn item_coloured(&mut self) -> Option<()> {
        let old = self.map_bank(4, 0);
        let frame = self.mem.r16(DS, FRAME);
        for i in 0..0xC350u16 {
            self.mem.w8(frame, i, 0);
        }
        self.mem.w16(DS, 0x8F9C, frame);
        self.image_capture(DS, SCRATCH)?;
        self.mem.w32(DS, 0x8F9A, 0);
        for i in 0..0x100u16 {
            self.mem.w8(DS, SCRATCH + i, i as u8);
        }
        let colour = self.mem.r16(DS, ITEM_COLOUR);
        self.mem.w16(DS, ITEM_STEPS, 0);
        let mut al = self.mem.r8(DS, 0x3EA0u16.wrapping_add(colour));
        let mut di = 0x938Cu16;
        let mut cx = 0x0Du16;
        if colour == 2 {
            self.mem.w16(DS, ITEM_STEPS, 1);
            cx = 7;
        }
        loop {
            for _ in 0..cx {
                self.mem.w8(DS, di, al);
                di = di.wrapping_add(1);
                al = al.wrapping_add(1);
            }
            if self.mem.r16(DS, ITEM_STEPS) == 0 {
                break;
            }
            cx = 5;
            al = 0x80;
            let v = self.mem.r16(DS, ITEM_STEPS).wrapping_sub(1);
            self.mem.w16(DS, ITEM_STEPS, v);
            if colour != 2 {
                break;
            }
        }
        for i in 0..0xC350u16 {
            let b = self.mem.r8(frame, i);
            let c = self.mem.r8(DS, SCRATCH.wrapping_add(b as u16));
            self.mem.w8(frame, i, c);
        }
        if self.page_is_block() {
            let r = |m: &Self, o: u16| m.mem.r16(DS, ITEM_HEADER + o);
            let (x, y, w, h) = (r(self, 0), r(self, 2), r(self, 4), r(self, 6));
            let page = self.mem.r16(DS, 0xA900);
            let mut di = self.mem.r16(DS, 0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
            let mut si = 0u16;
            for _ in 0..h {
                for k in 0..w {
                    let b = self.mem.r8(frame, si);
                    si = si.wrapping_add(1);
                    if b != 0 {
                        self.mem.w8(page, di.wrapping_add(k), b);
                    }
                }
                di = di.wrapping_add(0x140);
            }
        }
        self.map_bank(old, 0);
        Some(())
    }

    /// 0000:5828 with the far pointer at 8F9A set: the image whose header
    /// is at seg:off unpacked to [8F9C]:[8F9A] rather than drawn (flags 1
    /// runs, 8 LZHUF, 4 LZSS; an unpacked one writes nothing). Flags 2
    /// draws by runs (5B9A), not ported here.
    fn image_capture(&mut self, seg: u16, off: u16) -> Option<()> {
        let flags = self.mem.r16(seg, off.wrapping_add(8));
        let (bs, bo) = (self.mem.r16(DS, 0x8F9C), self.mem.r16(DS, 0x8F9A));
        if flags & 1 != 0 {
            self.rle_unpack(seg, off, bs, bo)?;
        } else if flags & 2 != 0 {
            return crate::giveup::not_ported("0000:5897: an image by runs to the capture buffer (5B9A)");
        } else if flags & 8 != 0 {
            let len = self.mem.r16(seg, off.wrapping_add(0xA));
            self.mif_decompress(seg, off.wrapping_add(0xC), bs, bo, len)?;
        } else if flags & 4 != 0 {
            self.lzss_unpack(seg, off.wrapping_add(0xA), bs, bo)?;
        }
        Some(())
    }

    /// 1194:2020: a worn helmet (type 5, kind 0) by the race at 1A8:
    /// 7, the helmet's own picture by its armour's kind (20B0: the digit
    /// of mliz?.img, 5CD9); 1, 3 and 4, the head's colours remapped (929E
    /// a table of each colour to itself but the 1Eh at 3276:5750's places
    /// from the race's list at 3276:576E) over (F4h, 13h; 13h by 17h), or
    /// a woman's (ECh, 12h; Fh by Fh) on the page (9D84, where the page is
    /// the game's own block); others nothing.
    fn sheet_helmet(&mut self, files: &mut Files, si: u16, di: u16) -> Option<u16> {
        let race = self.mem.r8(DS, PLAYER + 4) as u16;
        match race {
            7 => {
                let k = self.armor_kind(DS, si) as u8;
                self.mem.w8(DS, HELMET + 4, k.wrapping_add(b'0'));
                self.helmet_picture(files, di)?;
                Some(di)
            }
            1 | 3 | 4 => {
                let list = self.mem.r16(DATA, 0x576E + race * 2);
                for i in 0..0x100u16 {
                    self.mem.w8(DS, SCRATCH + i, i as u8);
                }
                for k in 0..0x1Eu16 {
                    let at = self.mem.r8(DATA, 0x5750 + k) as u16;
                    let b = self.mem.r8(DATA, list.wrapping_add(k));
                    self.mem.w8(DS, SCRATCH.wrapping_add(at), b);
                }
                let (x, y, w, h) = if self.mem.r8(DS, PLAYER + 7) != 0 { (0xEC, 0x12, 0x0F, 0x0F) } else { (0xF4, 0x13, 0x13, 0x17) };
                if self.page_is_block() {
                    let page = self.mem.r16(DS, 0xA900);
                    let mut di = self.mem.r16(DS, 0xA6A0u16 + 2 * y).wrapping_add(x);
                    for _ in 0..h {
                        for k in 0..w {
                            let b = self.mem.r8(page, di.wrapping_add(k));
                            let c = self.mem.r8(DS, SCRATCH.wrapping_add(b as u16));
                            self.mem.w8(page, di.wrapping_add(k), c);
                        }
                        di = di.wrapping_add(0x140);
                    }
                }
                Some(SCRATCH)
            }
            _ => Some(di),
        }
    }

    /// 1194:5CD9 with BX 7 (from 20B0): bank [1030] mapped and 0000:56C8
    /// with BX 7: bit 2 keeps the caller's ES:DI, so the picture named at
    /// HELMET is loaded into DGROUP at `di` (over whatever is there: from
    /// the doll's loop DGROUP's start), its header's words to A836..A83C;
    /// bit 1 keeps what it covers: a heap block (9CD6) of its width times
    /// its height plus 8 at A83E (EAX: 0, the segment), the header's four
    /// words and the page's rows there (8F2A), never freed; then the
    /// picture drawn (582C, where the page is the game's own block). Its
    /// colours (9B56) are the VGA's. The bank before mapped again.
    fn helmet_picture(&mut self, files: &mut Files, di: u16) -> Option<()> {
        let bank = self.mem.r16(DS, 0x1030);
        let old = self.map_bank(bank, 0);
        self.file_load(files, HELMET, DS, di)?;
        let h: Vec<u16> = (0..4u16).map(|k| self.mem.r16(DS, di.wrapping_add(2 * k))).collect();
        for (k, &v) in h.iter().enumerate() {
            self.mem.w16(DS, 0xA836 + 2 * k as u16, v);
        }
        let (x, y, w, hh) = (h[0], h[1], h[2], h[3]);
        let seg = self.heap_alloc_high(w.wrapping_mul(hh).wrapping_add(8))?;
        if seg == 0 {
            return crate::giveup::original("0000:573C: no room for the helmet's background: the original writes it at 0:0");
        }
        self.mem.w16(DS, 0xA83E, 0);
        self.mem.w16(DS, 0xA840, seg);
        for (k, &v) in h.iter().enumerate() {
            self.mem.w16(seg, 2 * k as u16, v);
        }
        self.page_rect_save((seg, 8), x, y, w, hh);
        if self.page_is_block() {
            self.page_image(DS, di);
        }
        self.map_bank(old, 0);
        Some(())
    }

    /// 1194:0DE1: the stats' texts, their state: the numbers made (0000:
    /// 7028's digits at A931.., the formatted at 1044: hit points,
    /// fatigue, experience and the bonuses), and the player's derived
    /// numbers (+39h, 16 words) scaled and divided in place for their
    /// texts and put back from their copy at 10A8 (0F08). Their drawing
    /// is the front end's.
    pub(crate) fn sheet_texts(&mut self) {
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
