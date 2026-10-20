//! The player's progress and end: the level-up screen (1194:1034) and
//! exhaustion (2901:0014..00A7).
//!
//! A level gained (2686:02D8, `progression::level_up`, from the minute
//! tick) brings up the level-up screen: the attributes are shown as
//! percentages of their base (1194:1007), the player gets 3..6 points a
//! level (50 once the main quest is over, 0FDD 8Eh) to spend on them,
//! as on the character creation's screen (1194:14C5, 138E: the points at
//! 129C, the rolled count at 9686), and can't leave while points remain
//! that the attributes have room for. The gain, back on the 256 scale
//! (1194:1012), is added to the attributes as they were; the worn items
//! are taken off first and put back on after (1194:7FD3, 800E), so their
//! enchantments are counted again.
//!
//! Fatigue below 0 (2901:0002, the minute tick) exhausts the player: in a
//! fight they die, in water (C39A bit 10h, unless 4000h) they drown, and
//! otherwise they collapse: an hour's rest (2901:016C with 13C8 1) and an
//! hour passes (0000:142D).
//!
//! The player's death (2686:02B4, `Machine::player_dies`) leaves the area;
//! the outer loop then ends the game with its text (`transit::Run::End`).

use crate::bsa::Files;
use crate::character::{field, PLAYER};
use crate::clock::ClockHooks;
use crate::machine::Machine;
use crate::mem::DS;
use crate::newgame::var::{POINTS, POINTS_ROLLED};

/// DGROUP variables.
pub mod var {
    /// The attributes as they were before the screen (8 bytes).
    pub const OLD_ATTRS: u16 = 0x12B2;
    /// The gain per attribute (8 bytes).
    pub const GAIN: u16 = 0x10A8;
    /// The worn items taken off (their slots' offsets, 0 after the last).
    pub const WORN: u16 = 0x1244;
    /// The main quest is over: the screen gives 50 points.
    pub const FINALE: u16 = 0x12AE;
    /// Points have been offered (the screen's second pass on).
    pub const OFFERED: u16 = 0x1299;
    /// A screen of the character's sheet is up.
    pub const SHEET: u16 = 0x12BA;
    /// The text colour, and the text segments set through 0000:9AE6 and
    /// 9AF8 (from 102C, 1036, 103A).
    pub const COLOUR: u16 = 0x9288;
    pub const TEXT_B036: u16 = 0xB036;
    pub const TEXT_B038: u16 = 0xB038;
    /// The window loop's flags (bit 4: no waits for the buttons'
    /// release).
    pub const LOOP_FLAGS: u16 = 0xB7C4;
}

/// The EMS frame's segment.
const FRAME: u16 = 0x9030;
/// "charstat.img", "bonus.img", "updown.img".
const CHARSTAT: u16 = 0x3DE2;
const BONUS: u16 = 0x3DFC;
const UPDOWN: u16 = 0x3E06;
/// Where UPDOWN.IMG is loaded in bank [1032].
const ARROWS_PIC: u16 = 0x67B9;
/// The attribute screen's arrows region's y (its record in the region
/// table at 3276:5312, record 9).
const ARROWS_Y: u16 = 0x5312 + 0x372 + 2;
use var::*;

/// How many collapses may be nested (each in the hour of the one
/// before) before the port gives up. Not the original's: its stack (in
/// DGROUP, down from 974Eh, 0000:85C6) takes 2Ah bytes a collapse and
/// runs on into its data. harness/gen_levelup.py stops there too.
pub const EXHAUSTION_DEPTH: u32 = 200;

thread_local! {
    static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// The attributes, the base attributes (DGROUP).
const ATTRS: u16 = PLAYER + 0x29;
const BASE: u16 = PLAYER + 0x31;
const ITEMS: u16 = 0x0212;

/// What the level-up screen leaves to the front end, and the game's own
/// steps around it that the stubbed vectors (harness/gen_levelup.py) stub:
/// those default to the game's (`Machine::world_keep`, `sheet_keep`,
/// `equip_frames`, `world_back`).
pub trait LevelUpHooks {
    /// 23BA:012B: the sound named at DS:`si` played (the notice's
    /// FANFARE1.VOC).
    fn sound(&mut self, m: &mut Machine, files: &mut Files, si: u16) -> Option<()> {
        m.sound_file(files, si)
    }
    /// The colours shown (6-bit), which A611 reads to A39E; None leaves
    /// A39E as it is.
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        None
    }
    /// 0000:541E, A611: the cursor hidden and the world kept
    /// (`Machine::world_keep`, the colours `dac`).
    fn world_keep(&mut self, m: &mut Machine, files: &mut Files) {
        let dac = self.dac();
        m.cursor_hidden();
        m.world_keep(files, dac.as_ref());
    }
    /// 1194:158F: the screen kept (CHARSHET.$$$, SAVEFACE.$$$:
    /// `Machine::sheet_keep`), unless 5E0 bit 200h.
    fn keep_screen(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.sheet_keep(files)
    }
    /// 0000:9A98 (the clip the whole screen), 9426 (the page cleared) and
    /// 1194:55B2 (the colours at 929E black, the page cleared again; the
    /// fade to them is the front end's).
    fn screen(&mut self, m: &mut Machine) {
        m.clip_whole();
        m.page_clear();
        m.sheet_black();
    }
    /// 1194:2462: the equipment's frames loaded into bank 17h
    /// (`Machine::equip_frames`), for the body the screen draws.
    fn body(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.equip_frames(files)
    }
    /// 1194:124A: the attribute screen set up for the points
    /// (`Machine::points_screen`).
    fn points_screen(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.points_screen(files)
    }
    /// 1194:122C's loop (5CF4 over the regions at 3276:5312): the player
    /// spends the points (POINTS) on the attributes (at the player's +29h,
    /// percent), as on the creation's screen (`Machine::creation_select`,
    /// `creation_arrow`). By default they go to the attributes in turn,
    /// one at a time.
    fn spend(&mut self, m: &mut Machine) {
        spend_in_turn(m);
    }
    /// 1194:1241..1249: the loop left (`Machine::points_screen_end`).
    fn points_screen_end(&mut self, m: &mut Machine) {
        m.points_screen_end();
    }
    /// 1194:59CC: a message box with the text at DS:`si` (4F25 the
    /// level's notice, 3E17 points left to spend).
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 0000:541E, A6B5: the cursor hidden and the world given back
    /// (`Machine::world_back`). None where it gives up.
    fn restore(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.cursor_hidden();
        m.world_back(files)
    }
}

/// `LevelUpHooks` that show nothing and spend the points in turn.
pub struct NoLevelUpHooks;

impl LevelUpHooks for NoLevelUpHooks {}

/// The points spent one at a time on the attributes in turn, as clicks on
/// the creation's screen would (`creation_select`, then the up arrow),
/// until none are left or no attribute takes one.
pub fn spend_in_turn(m: &mut Machine) {
    loop {
        let before = m.mem.r8(DS, POINTS);
        for region in 1..=8u16 {
            if m.mem.r8(DS, POINTS) == 0 {
                return;
            }
            m.creation_select(region);
            m.attribute_arrow(0);
        }
        if m.mem.r8(DS, POINTS) == before {
            return;
        }
    }
}

/// 2686:0319..0331: B038 set from 103A (0000:9AF8), the level-up screen
/// (`Machine::level_up_screen`), then B038 from 1036.
pub fn level_up_around(m: &mut Machine, files: &mut Files, levels: u16, hooks: &mut dyn LevelUpHooks) -> Option<()> {
    let v = m.mem.r16(DS, 0x103A);
    m.mem.w16(DS, TEXT_B038, v);
    m.level_up_screen(levels, files, hooks)?;
    let v = m.mem.r16(DS, 0x1036);
    m.mem.w16(DS, TEXT_B038, v);
    Some(())
}

impl Machine {
    fn l8(&self, at: u16) -> u8 {
        self.mem.r8(DS, at)
    }

    fn l16(&self, at: u16) -> u16 {
        self.mem.r16(DS, at)
    }

    fn copy8(&mut self, from: u16, to: u16) {
        for i in 0..8 {
            let b = self.l8(from + i);
            self.mem.w8(DS, to + i, b);
        }
    }

    /// 1194:1034. The level-up screen after `levels` levels (CX as
    /// 2686:02D8 leaves it, the count with the new level's high byte
    /// above; 0 counts 65536). Fatigue (201) and spell points (20A) are as
    /// they were after it. If the base attributes' first eight bytes are
    /// all FFh (1194:11C6) only health is made full. None where a roll
    /// faults or a hook gives up.
    pub fn level_up_screen(&mut self, levels: u16, files: &mut Files, hooks: &mut dyn LevelUpHooks) -> Option<()> {
        let (sp, fatigue) = (self.l16(0x020A), self.l16(0x0201));
        let finale = self.l8(0x0FDD) == 0x8E;
        self.mem.w16(DS, FINALE, finale as u16);
        if !finale {
            self.level_notice(files, hooks)?;
        }
        if self.mem.r32(DS, BASE) == 0xFFFF_FFFF && self.mem.r32(DS, BASE + 4) == 0xFFFF_FFFF {
            let max = self.l16(PLAYER + field::MAX_HP);
            self.mem.w16(DS, PLAYER + field::HP, max);
            self.mem.w16(DS, 0x0201, fatigue);
            self.mem.w16(DS, 0x020A, sp);
            return Some(());
        }
        hooks.world_keep(self, files);
        if self.l16(0x05E0) & 0x200 == 0 {
            hooks.keep_screen(self, files)?;
        }
        self.worn_off();
        self.copy8(ATTRS, OLD_ATTRS);
        crate::scale::attrs_to100(&mut self.mem, BASE, BASE, 8);
        self.copy8(BASE, ATTRS);
        self.mem.w16(DS, SHEET, 1);
        let f = self.l16(0x05C8) | 0x10;
        self.mem.w16(DS, 0x05C8, f);
        hooks.screen(self);
        let points_were = self.l16(POINTS);
        let mut bx = 0u16;
        let n = if levels == 0 { 0x10000 } else { levels as u32 };
        for _ in 0..n {
            let ax = crate::dice::roll(&mut self.rng, 4)?.wrapping_add(3);
            bx = bx.wrapping_add(ax);
        }
        self.mem.w8(DS, POINTS, bx as u8);
        self.mem.w16(DS, POINTS_ROLLED, bx);
        let v = self.l16(0x102C);
        self.mem.w16(DS, TEXT_B036, v);
        hooks.body(self, files)?;
        if finale {
            self.mem.w8(DS, POINTS, 0x32);
            self.mem.w16(DS, POINTS_ROLLED, 0x32);
            self.mem.w8(DS, OFFERED, 0);
        }
        loop {
            // 1194:11A9: the room the attributes have (100 less each, in
            // 16 bits, stopping once it reaches 256).
            let mut room = 0u16;
            for i in 0..8 {
                room = room.wrapping_add(0x64u16.wrapping_sub(self.l8(ATTRS + i) as u16) & 0xFF);
                if room >> 8 != 0 {
                    break;
                }
            }
            if room >> 8 == 0 && (room as u8) < self.l8(POINTS) {
                self.mem.w8(DS, POINTS, room as u8);
            }
            hooks.points_screen(self, files)?;
            hooks.spend(self);
            hooks.points_screen_end(self);
            self.mem.w8(DS, OFFERED, 1);
            if self.l8(POINTS) == 0 {
                break;
            }
            hooks.message_box(self, 0x3E17);
        }
        crate::scale::attrs_to256(&mut self.mem, ATTRS, ATTRS, 8);
        crate::scale::attrs_to256(&mut self.mem, OLD_ATTRS, OLD_ATTRS, 8);
        for i in 0..8 {
            let g = self.l8(ATTRS + i).wrapping_sub(self.l8(OLD_ATTRS + i));
            self.mem.w8(DS, GAIN + i, g);
        }
        self.copy8(ATTRS, BASE);
        self.copy8(OLD_ATTRS, ATTRS);
        for i in 0..8 {
            let v = self.l8(ATTRS + i).wrapping_add(self.l8(GAIN + i));
            self.mem.w8(DS, ATTRS + i, v);
        }
        self.worn_on();
        self.attribute_modifiers(PLAYER);
        self.mem.w16(DS, POINTS, points_were);
        let f = self.l16(0x05C8) & 0xFFEF;
        self.mem.w16(DS, 0x05C8, f);
        self.mem.w16(DS, SHEET, 0);
        self.mem.w16(DS, 0xCD00, 0xFFFF);
        hooks.restore(self, files)?;
        self.mem.w16(DS, 0x0201, fatigue);
        self.mem.w16(DS, 0x020A, sp);
        Some(())
    }

    /// 1194:124A..12BD. The attribute screen for the points, its game's
    /// side: the buttons' release waited for (5E21, the front end's), the
    /// cursor hidden (541E), the colour FEh, B7C4 bit 4 set (the loop's
    /// no waits for the release); on its first pass (1299 0) only, the
    /// screen made: 1297 38h and 1296 0, CHARSTAT.IMG and BONUS.IMG drawn
    /// (5CD9 with BX 10h, `sheet_image`), the page's top left A7h x 9Fh
    /// kept in bank [1032] (12BE: 8F26), the page cleared (9426), the two
    /// pictures again round the stats' texts (0DE1), the points' number
    /// (12E6: 5634, its drawing the front end's), the arrows (12F5:
    /// UPDOWN.IMG into bank [1032] at 67B9, the page under them, 8 x 48h
    /// at (26h, 30h), kept in bank 5, 1297 38h, their region at y 30h;
    /// 1354 draws them), the sheet (0DC7: 541E, CHARSHET.$$$ under the
    /// page, 1647, and the doll, 16AA, 9686 kept round it) and
    /// CHARSHT.COL's colours (1695). None where a file is missing or the
    /// heap has no room.
    pub fn points_screen(&mut self, files: &mut Files) -> Option<()> {
        self.cursor_hidden();
        self.mem.w8(DS, COLOUR, 0xFE);
        let f = self.l8(LOOP_FLAGS) | 4;
        self.mem.w8(DS, LOOP_FLAGS, f);
        if self.l8(OFFERED) != 0 {
            return Some(());
        }
        self.mem.w8(DS, 0x1297, 0x38);
        self.mem.w8(DS, 0x1296, 0);
        self.sheet_image(files, CHARSTAT)?;
        self.sheet_image(files, BONUS)?;
        self.corner_kept();
        self.page_clear();
        self.sheet_image(files, CHARSTAT)?;
        self.sheet_texts();
        self.sheet_image(files, BONUS)?;
        self.points_drawn();
        self.arrows_made(files)?;
        let rolled = self.l16(POINTS_ROLLED);
        self.cursor_hidden();
        self.sheet_back(files)?;
        self.sheet_doll(files)?;
        self.mem.w16(DS, POINTS_ROLLED, rolled);
        self.sheet_colours(files)
    }

    /// 1194:12BE: the page's top left, A7h by 9Fh, kept in bank [1032]
    /// at the frame's start (8F26), the bank mapped before put back.
    pub(crate) fn corner_kept(&mut self) {
        let bank = self.l16(0x1032);
        let frame = self.l16(FRAME);
        let old = self.map_bank(bank, 0);
        self.page_rect_save((frame, 0), 0, 0, 0xA7, 0x9F);
        self.map_bank(old, 0);
    }

    /// 1194:12F5: the arrows: UPDOWN.IMG into bank [1032] at 67B9, the
    /// page under them (8 by 48h at 26h, 30h) kept in bank 5, the bank
    /// mapped before put back; 1297 38h, their region at y 30h, and the
    /// arrows drawn there (1354, `arrows_drawn`). None where the file is
    /// missing.
    pub(crate) fn arrows_made(&mut self, files: &mut Files) -> Option<()> {
        let bank = self.l16(0x1032);
        let frame = self.l16(FRAME);
        let old = self.map_bank(bank, 0);
        self.file_load(files, UPDOWN, frame, ARROWS_PIC)?;
        self.map_bank(5, 0);
        self.page_rect_save((frame, 0), 0x26, 0x30, 8, 0x48);
        self.map_bank(old, 0);
        self.mem.w8(DS, 0x1297, 0x38);
        self.mem.w16(crate::mem::DATA, ARROWS_Y, 0x30);
        self.arrows_drawn();
        Some(())
    }

    /// 1194:1354: UPDOWN.IMG's 8 x 10h at 67B9 in bank [1032] drawn at x
    /// 26h, their region's y (5312's +372h +2), its 0s left out (5B04,
    /// into the page where it is the game's own block), the bank before
    /// mapped again.
    pub(crate) fn arrows_drawn(&mut self) {
        let bank = self.l16(0x1032);
        let old = self.map_bank(bank, 0);
        if self.page_is_block() {
            let (frame, page) = (self.l16(FRAME), self.l16(0xA900));
            let y = self.mem.r16(crate::mem::DATA, ARROWS_Y);
            let mut di = self.l16(0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(0x26);
            let mut si = ARROWS_PIC;
            for _ in 0..0x10 {
                for k in 0..8u16 {
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
    }

    /// 1194:12E6: the points left (129C) drawn at (5Ch, 71h) (5634: their
    /// digits at 929E, 9E06).
    pub(crate) fn points_drawn(&mut self) {
        let points = self.l8(crate::newgame::var::POINTS) as u16;
        self.number_at(0x5C, 0x71, points);
    }

    /// 1194:1241..1249: the loop (5CF4) left by its first region, the
    /// button (B7C6 0, its handler 3159:0006 setting B7C4 bit 0), B7C4 bit
    /// 4 clear again, the buttons' release waited for (5E21, the front
    /// end's).
    pub fn points_screen_end(&mut self) {
        self.mem.w16(DS, crate::newgame::var::REGION, 0);
        let f = (self.l8(LOOP_FLAGS) | 1) & !4;
        self.mem.w8(DS, LOOP_FLAGS, f);
    }

    /// 1194:5E33. The level's notice: its sound, then (1194:42EE) the
    /// text segments B036 (102C) and B038 (1036) and colour 60h set, and
    /// "You have gained a level of experience!" (4F25) in a box.
    fn level_notice(&mut self, files: &mut Files, hooks: &mut dyn LevelUpHooks) -> Option<()> {
        hooks.sound(self, files, 0x4F18)?;
        let v = self.l16(0x102C);
        self.mem.w16(DS, TEXT_B036, v);
        let v = self.l16(0x1036);
        self.mem.w16(DS, TEXT_B038, v);
        self.mem.w8(DS, COLOUR, 0x60);
        hooks.message_box(self, 0x4F25);
        Some(())
    }

    /// 1194:7FD3. The worn items taken off, listed at WORN: as many slots
    /// from the first as there are occupied ones (1194:7472; so worn
    /// items past that many slots stay on), each occupied (flag 40h),
    /// worn (80h), with bit 1 and +12h not FFh.
    fn worn_off(&mut self) {
        let (n, _) = self.count_items(DS, ITEMS);
        let mut di = WORN;
        for k in 0..n {
            let si = ITEMS + k * 0x13;
            let f = self.l8(si + 0xE);
            if f & 0x40 != 0 && f & 0x80 != 0 && f & 1 != 0 && self.l8(si + 0x12) != 0xFF {
                self.mem.w16(DS, di, si);
                di = di.wrapping_add(2);
                self.equip_toggle(si);
            }
        }
        // xor eax, eax; stosw.
        self.mem.w16(DS, di, 0);
    }

    /// 1194:800E. The items listed at WORN put back on.
    fn worn_on(&mut self) {
        let mut at = WORN;
        loop {
            let si = self.l16(at);
            if si == 0 {
                return;
            }
            at = at.wrapping_add(2);
            self.equip_toggle(si);
        }
    }

    /// 2901:0014..00A7. Fatigue fell below 0 (2901:0002 took 11 from it):
    /// it is 0 again, and in a fight (0FFB) the player dies of it; else,
    /// the view cleared (0000:4EFD), in water (C39A bit 10h, unless
    /// 4000h) they drown; else they collapse, rest an hour (2901:016C with
    /// 13C8 1) and an hour passes (0000:142D, the clock's own hooks).
    ///
    /// That hour runs the minute tick, whose fatigue may run out again:
    /// the exhaustion is then called inside itself, sometimes for good
    /// (an hour's rest giving back less than an hour takes); past
    /// `EXHAUSTION_DEPTH` nested collapses the port gives None. None also where the rest or the
    /// clock does.
    pub fn exhaustion(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        let depth = DEPTH.with(|d| {
            d.set(d.get() + 1);
            d.get()
        });
        let r = if depth > EXHAUSTION_DEPTH { None } else { self.exhaustion_body(files, hooks) };
        DEPTH.with(|d| d.set(d.get() - 1));
        r
    }

    fn exhaustion_body(&mut self, files: &mut Files, hooks: &mut dyn ClockHooks) -> Option<()> {
        self.mem.w16(DS, PLAYER + field::FATIGUE, 0);
        if self.l16(0x0FFB) != 0 {
            self.mem.w8(DS, COLOUR, 0x60);
            let v = self.l16(0x1036);
            self.mem.w16(DS, TEXT_B038, v);
            let v = self.l16(0x102C);
            self.mem.w16(DS, TEXT_B036, v);
            hooks.exhaustion_box(self, 0x7B6A);
            self.player_dies();
            return Some(());
        }
        // 0000:4EFD: colour 0, then the view filled (9A72).
        self.mem.w8(DS, COLOUR, 0);
        hooks.view_cleared(self);
        let flags = self.l16(crate::movement::var::LEVEL_FLAGS);
        if flags & 0x10 != 0 && flags & 0x4000 == 0 {
            let v = self.l16(0x102C);
            self.mem.w16(DS, TEXT_B036, v);
            let v = self.l16(0x1036);
            self.mem.w16(DS, TEXT_B038, v);
            self.mem.w8(DS, COLOUR, 0x60);
            hooks.exhaustion_box(self, 0x7B2F);
            self.player_dies();
            return Some(());
        }
        self.mem.w8(DS, COLOUR, 0x60);
        hooks.exhaustion_box(self, 0x7AE0);
        self.mem.w8(DS, crate::rest::var::REGEN_KIND, 1);
        self.rest_heal(1)?;
        self.advance_minutes(files, 0x3C, hooks)
    }
}
