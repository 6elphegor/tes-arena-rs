//! Resting (1194:424E..4880, the regeneration at 2901:016C, the camp
//! check at 2433:12F5).
//!
//! Camping (`camp_check`) is refused with enemies near (FFB), in the
//! wild's dungeons (C39A bit 10h), in a city's streets (C398 bit 4) and
//! in most buildings; a tavern's room (D48 4) needs hours left on it
//! (1194:B9E4). The rest then starts (`rest_start`) from a menu: rest
//! until healed (`HOURS` counting up, COUNTDOWN 0), or for a number of
//! hours typed in (counting down, COUNTDOWN 1). Each frame (`rest_frame`,
//! 1194:4635, from the frame body at 0000:48C9) a step passes 20 minutes;
//! every third step is an hour: the player regenerates (`rest_heal`) and
//! the hours shown change. Enemies arriving (FFB, set by the clock's
//! encounter checks) or a key (AFF9) end the rest. Once rested, the
//! main quest's dream may come (1194:D340, D3AF).
//!
//! In a rented tavern room the hours pass at once (1194:437F, 4757), an
//! hour at a time against the room's hours (1019), which the room's file
//! keeps (1194:BA91, `RestHooks::room_file`).
//!
//! The menus, the typed hours, the texts and the screen are `RestHooks`;
//! the clock is the game's (`Machine::advance_minutes`) with the front
//! end's `ClockHooks`, whose encounters interrupt the rest.

use crate::bsa::Files;
use crate::character::PLAYER;
use crate::clock::ClockHooks;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Resting (byte).
    pub const RESTING: u16 = 0x1013;
    /// The fatigue the rest aims at: (strength + endurance) × 64.
    pub const FATIGUE_LIMIT: u16 = 0x1014;
    /// Hours rested (until healed), or hours left (counting down).
    pub const HOURS: u16 = 0x1016;
    /// 1: counting down the hours typed in; 0: until healed.
    pub const COUNTDOWN: u16 = 0x1018;
    /// The rented room's hours left (word).
    pub const ROOM_HOURS: u16 = 0x1019;
    /// The 20-minute steps of the hour (byte, 0..2).
    pub const STEP: u16 = 0x49E6;
    /// Which regeneration bonus (2901:016C: the signed bytes at 21FD):
    /// 0 resting, 1 collapsed.
    pub const REGEN_KIND: u16 = 0x13C8;
    /// Enemies near (word) and its partner FF9.
    pub const ENEMIES: u16 = 0x0FFB;
    pub const ENEMIES_2: u16 = 0x0FF9;
    /// A key was pressed (byte).
    pub const KEY: u16 = 0xAFF9;
    /// The hours typed in (4 bytes of text).
    pub const TYPED: u16 = 0x4980;
    /// The text the hours are formatted into.
    pub const TEXT: u16 = 0x1044;
    /// The text colour.
    pub const COLOUR: u16 = 0x9288;
    /// The room's end (dword, in the clock's minutes), its flags, and the
    /// hours asked for.
    pub const ROOM_END: u16 = 0x12CA;
    pub const ROOM_FLAG: u16 = 0x12BC;
    pub const ROOM_FLAG_2: u16 = 0x12BE;
    pub const ASKED: u16 = 0x12C4;
    /// The main quest's dream: pending, shown, count.
    pub const DREAM: u16 = 0x0F76;
    pub const DREAM_COUNT: u16 = 0x0F77;
    pub const DREAM_SHOWN: u16 = 0x0F7A;
}
use var::*;

/// Texts (DGROUP).
mod text {
    pub const HOURS_PASSED: u16 = 0x48E3;
    pub const HOURS_PASSED_N: u16 = 0x48F6;
    pub const HOURS_LEFT: u16 = 0x4909;
    pub const HOURS_LEFT_N: u16 = 0x491E;
    /// The rest menu: its title, keys and entries.
    pub const MENU: (u16, u16, u16) = (0x4934, 0x4945, 0x4948);
    pub const NO_NEED: u16 = 0x4A05;
    pub const RESTED: u16 = 0x49B1;
    pub const DONE: u16 = 0x49C3;
    pub const ENEMIES: u16 = 0x49E9;
    pub const ROOM_GONE: u16 = 0x747A;
    pub const NO_ROOM: u16 = 0x74DF;
    /// "You only have %d remaining hours. Rest this long?", its menu.
    pub const ONLY: u16 = 0x7444;
    pub const ONLY_MENU: (u16, u16) = (0x72EA, 0x72ED);
    pub const ENEMIES_NEAR: u16 = 0x5CD2;
    pub const NOT_HERE: u16 = 0x5CF8;
}

/// What resting leaves to the front end.
pub trait RestHooks: ClockHooks {
    /// 1194:020E: a menu, its title at DS:`si`, its keys at `bx`, its
    /// entries at `di`. The pick (AX; negative: none).
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        0xFFFF
    }
    /// 1194:4800: the hours typed (up to 3 characters, NUL-terminated)
    /// into DS:TYPED, which is zeroed before, the font and tiles 46BF
    /// selects selected. Nothing typed ends the rest.
    fn type_hours(&mut self, _m: &mut Machine) {}
    /// 1194:59C8 (59CC): a message box with the text at DS:`si`.
    fn message(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:4659 / 452F: the hours (TEXT, formatted) drawn
    /// (`Machine::rest_hours_draw`).
    fn hours_text(&mut self, m: &mut Machine) {
        m.rest_hours_draw();
    }
    /// The colours shown (6-bit), which A611 reads to A39E and D45D to
    /// 9A6E; None leaves them as they are.
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        None
    }
    /// 0000:A611: the world kept before the dream (`Machine::world_keep`,
    /// the colours `dac`).
    fn dream_keep(&mut self, m: &mut Machine, files: &mut Files) {
        let dac = self.dac();
        m.world_keep(files, dac.as_ref());
    }
    /// 1194:D45D: the dream's movie (DS:`di`, its song `si`) with its text
    /// number `ax` (`Machine::vision`, its pages clicked through).
    fn dream(&mut self, m: &mut Machine, files: &mut Files, ax: u16, si: u16, di: u16) -> Option<()> {
        let dac = self.dac();
        m.vision(files, &mut crate::vision::ClickThrough::default(), dac.as_ref(), ax, si, di)
    }
    /// 1194:55B2 between the first level's two dreams: the colours at 929E
    /// black and the page cleared (the fade the front end's).
    fn dream_black(&mut self, m: &mut Machine) {
        m.sheet_black();
    }
    /// 0000:A6B5 (`Machine::world_back`) and 2542:0446 (the colours of
    /// the file named at DS:1304 read to 929E, 0000:7530; their setting
    /// the front end's): the world given back after the dream.
    fn dream_back(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.world_back(files)?;
        m.colours_read(files, 0x1304)
    }
    /// 0000:9A98 (the clip the whole screen) and 9AE6 ([1026]'s font) as
    /// the dream ends.
    fn dream_screen(&mut self, m: &mut Machine) {
        m.clip_whole();
        m.select_font(0x1026);
    }
    /// 2294:0341 (after its window is reset): the sky for the new hour.
    fn sky(&mut self, m: &mut Machine) {
        m.sky_step();
    }
    /// 1194:C04C: the rented room's file read (ROOM_HOURS, ROOM_END).
    /// The carry: no room.
    fn room_load(&mut self, m: &mut Machine, files: &mut Files) -> bool {
        m.room_read(files)
    }
    /// 1194:BA91: the room's file written with ROOM_HOURS and ROOM_END
    /// (deleted once no hours are left).
    fn room_file(&mut self, m: &mut Machine, files: &mut Files) {
        m.room_write(files)
    }
    /// The screen redrawn after the rest (1194:55B2, 0000:4EFD, 2294:10EC,
    /// 2542:0446, 0000:9426, 9466) or a wait (0000:6BDE).
    fn screen(&mut self, _m: &mut Machine) {}
    /// 0000:142D: `minutes` minutes pass. Return false to have the game's
    /// clock run them (with these hooks as its `ClockHooks`); true when
    /// the front end did.
    fn advance(&mut self, _m: &mut Machine, _files: &mut Files, _minutes: u32) -> Option<bool> {
        Some(false)
    }
}

/// `RestHooks` that leave everything as it is (no menu picked).
pub struct NoRestHooks;

impl ClockHooks for NoRestHooks {}
impl RestHooks for NoRestHooks {}

impl Machine {
    fn r_g8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }
    fn r_g16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }
    fn r_or8(&mut self, o: u16, v: u8) {
        let b = self.mem.r8(DS, o) | v;
        self.mem.w8(DS, o, b);
    }

    /// 0000:142D for `minutes`, through the hooks.
    fn rest_advance(&mut self, files: &mut Files, hooks: &mut dyn RestHooks, minutes: u32) -> Option<()> {
        if !hooks.advance(self, files, minutes)? {
            self.advance_minutes(files, minutes, hooks)?;
        }
        Some(())
    }

    /// 1194:42EE / 2433:12E2, with AX [1026] as every caller loads it: the
    /// message's font FONT_A.DAT (0000:9AE6: B036), its box tiles [1036]
    /// (9AF8: B038) and the colour byte 60h.
    fn rest_colours(&mut self) {
        let font = self.r_g16(0x1026);
        self.mem.w16(DS, 0xB036, font);
        self.mem.w8(DS, COLOUR, 0x60);
        let tiles = self.r_g16(0x1036);
        self.mem.w16(DS, 0xB038, tiles);
    }

    /// 1194:59CC with the colours first.
    fn rest_message(&mut self, hooks: &mut dyn RestHooks, si: u16) {
        hooks.message(self, si);
    }

    /// 2901:016C. `hours` hours of rest: the attributes recover
    /// (2901:026E); health comes back by max health × hours × (heal
    /// modifier × 5 + 60, or + 80 for class 24h) / 1000, plus (for class
    /// 0Fh its heal modifier, if positive) + the bonus of REGEN_KIND, per
    /// hour; at least 1, at most the maximum; at a tenth of it or less the
    /// screen is to be redrawn (D39 bit 80h). Fatigue comes back by
    /// (strength + endurance) × 64 × hours / 1000 × (heal modifier × 5 +
    /// 70), up to (strength + endurance) × 64; spell points (classes with
    /// bit 20h but 23h) by max × hours / 8. The multiplications keep 16
    /// bits; None where a division overflows (the original's divide
    /// fault).
    pub fn rest_heal(&mut self, hours: u16) -> Option<()> {
        self.rest_attributes();
        let class = self.r_g8(0x01A9);
        let bonus = if class == 0x24 { 0x14 } else { 0 };
        self.mem.w16(DS, 0xA888, bonus);
        self.mem.w16(DS, 0xA886, hours);
        let max_hp = self.r_g16(0x01FF);
        let heal = self.r_g16(0x01F7);
        let ax = max_hp.wrapping_mul(hours);
        let bx = heal.wrapping_mul(5).wrapping_add(0x3C).wrapping_add(bonus);
        let q = (ax as u32 * bx as u32) / 1000;
        if q > 0xFFFF {
            return crate::giveup::original("2901:016C: the healing's divide overflows");
        }
        let mut cx = q as u16;
        let mut ax = 0u16;
        if class == 0x0F {
            ax = heal;
            if (ax as i16) <= 0 {
                ax = 0;
            }
        }
        let kind = self.r_g8(REGEN_KIND) as u16;
        ax = ax.wrapping_add(self.r_g8(0x21FD_u16.wrapping_add(kind)) as i8 as i16 as u16);
        cx = cx.wrapping_add(ax.wrapping_mul(hours));
        if (cx as i16) <= 0 {
            cx = 1;
        }
        cx = cx.wrapping_add(self.r_g16(0x01FD));
        if (cx as i16) > (max_hp as i16) {
            cx = max_hp;
        }
        self.mem.w16(DS, 0x01FD, cx);
        if self.r_g16(0x01FD) <= max_hp / 10 {
            self.r_or8(0x0D39, 0x80);
        }
        let limit = self.rest_fatigue_limit();
        self.mem.w16(DS, 0x12D0, limit);
        let q = (limit as u32 * hours as u32) / 1000;
        if q > 0xFFFF {
            return crate::giveup::original("2901:016C: the healing's divide overflows");
        }
        let ax = (q as u16).wrapping_mul(heal.wrapping_mul(5).wrapping_add(0x46));
        let mut f = self.r_g16(0x0201).wrapping_add(ax);
        if f > limit {
            f = limit;
        }
        self.mem.w16(DS, 0x0201, f);
        if class & 0x20 != 0 && class != 0x23 && self.r_g16(0x020A) < self.r_g16(0x020C) {
            let max = self.r_g16(0x020C);
            let mut sp = (max.wrapping_mul(hours) >> 3).wrapping_add(self.r_g16(0x020A));
            if sp > max {
                sp = max;
            }
            self.mem.w16(DS, 0x020A, sp);
        }
        Some(())
    }

    /// 2901:026E. Each attribute below its base rises by endurance / 10
    /// (a byte add: past 255 it is the base), not above the base; then
    /// the player's modifiers (1194:361D).
    fn rest_attributes(&mut self) {
        let dl = self.r_g8(0x01D2) / 10;
        for i in 0..8u16 {
            let base = self.r_g8(0x01D5 + i);
            let cur = self.r_g8(0x01CD + i);
            if cur >= base {
                continue;
            }
            let (v, carry) = cur.overflowing_add(dl);
            self.mem.w8(DS, 0x01CD + i, if carry || v > base { base } else { v });
        }
        self.attribute_modifiers(PLAYER);
    }

    /// 1194:4687. (strength + endurance) × 64.
    pub fn rest_fatigue_limit(&self) -> u16 {
        (self.r_g8(0x01CD) as u16 + self.r_g8(0x01D2) as u16) << 6
    }

    /// 1194:4345's test (also inline at 44D0): fatigue within 60 of
    /// FATIGUE_LIMIT, full health, and full spell points for the classes
    /// with bit 20h but 23h.
    fn rest_full(&self) -> bool {
        if self.r_g16(0x0201).wrapping_add(0x3C) < self.r_g16(FATIGUE_LIMIT) {
            return false;
        }
        if self.r_g16(0x01FD) < self.r_g16(0x01FF) {
            return false;
        }
        let class = self.r_g8(0x01A9);
        if class & 0x20 != 0 && class != 0x23 && self.r_g16(0x020A) < self.r_g16(0x020C) {
            return false;
        }
        true
    }

    /// 1194:4345. Rested (the carry): the rest is over (RESTING and STEP
    /// cleared).
    fn rest_rested(&mut self) -> bool {
        if !self.rest_full() {
            return false;
        }
        self.mem.w8(DS, RESTING, 0);
        self.mem.w8(DS, STEP, 0);
        true
    }

    /// 1194:4301. The rest ends: the screen to be redrawn (D39 bit 80h),
    /// RESTING, HOURS and STEP cleared.
    fn rest_end(&mut self) {
        self.r_or8(0x0D39, 0x80);
        self.mem.w8(DS, RESTING, 0);
        self.mem.w16(DS, HOURS, 0);
        self.mem.w8(DS, STEP, 0);
    }

    /// 1194:42B9. No enemies (FF9, FFB); every actor slot inactive, the
    /// creatures (0C57..) not already so forgotten first (0000:269E).
    fn rest_clear_actors(&mut self) {
        self.mem.w16(DS, ENEMIES_2, 0);
        self.mem.w16(DS, ENEMIES, 0);
        for i in 0..0x17u16 {
            let si = 0x0AB3 + i * 0x1C;
            if si >= 0x0C57 {
                if self.r_g16(si + 0xF) & 0x4000 != 0 {
                    continue;
                }
                self.actor_forget(si);
            }
            let f = self.r_g16(si + 0xF) | 0x4000;
            self.mem.w16(DS, si + 0xF, f);
        }
    }

    /// In a tavern's room (5E6 4, D48 4).
    fn rest_in_room(&self) -> bool {
        self.r_g8(0x05E6) == 4 && self.r_g8(0x0D48) == 4
    }

    /// 1194:4659 (`passed` false) / 1194:452F (true): the hours (HOURS, `%u`)
    /// formatted with the text `si` into TEXT and drawn. 452F picks the
    /// text itself, "hours passed".
    fn rest_hours_text(&mut self, hooks: &mut dyn RestHooks, si: u16) {
        let n = self.r_g16(HOURS);
        self.sprintf((DS, TEXT), (DS, si), &[Arg::Int(n)]);
        self.mem.w8(DS, COLOUR, 0x3E);
        hooks.hours_text(self);
    }

    /// 1194:466B..4681: [1026]'s font selected (9AE6) and the hours at
    /// TEXT drawn centred on x A0h at row 14h (0000:5364) in COLOUR, into
    /// the page where it is the game's own block.
    pub fn rest_hours_draw(&mut self) {
        self.select_font(0x1026);
        self.text_centred((DS, TEXT), 0xA0, 0x14);
    }

    fn rest_passed_text(&self) -> u16 {
        if self.r_g16(HOURS) == 1 { text::HOURS_PASSED } else { text::HOURS_PASSED_N }
    }

    fn rest_left_text(&self) -> u16 {
        if self.r_g16(HOURS) == 1 { text::HOURS_LEFT } else { text::HOURS_LEFT_N }
    }

    /// 2433:12F5. Whether camping is refused here (the carry), with its
    /// message: enemies near; in the wild's dungeons (C39A bit 10h) or a
    /// city's streets (C398 bit 4) "You can't camp here."; in the wild
    /// (C39A bit 200h) it is allowed, and inside (5E6 4) in buildings of
    /// kind 0Bh and 8, a rented room (D48 4, `room_camp`), or when
    /// 191D is set.
    pub fn camp_check(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> bool {
        let refuse = |m: &mut Machine, hooks: &mut dyn RestHooks, si: u16| {
            m.rest_colours();
            m.rest_message(hooks, si);
            true
        };
        if self.r_g16(ENEMIES) != 0 {
            return refuse(self, hooks, text::ENEMIES_NEAR);
        }
        if self.r_g16(0xC39A) & 0x10 != 0 {
            return refuse(self, hooks, text::NOT_HERE);
        }
        if self.r_g16(0xC39A) & 0x200 != 0 {
            return false;
        }
        if self.r_g16(0xC398) & 4 != 0 {
            return refuse(self, hooks, text::NOT_HERE);
        }
        if self.r_g8(0x05E6) != 4 {
            return false;
        }
        match self.r_g8(0x0D48) {
            0x0B | 8 => false,
            4 => self.room_camp(files, hooks),
            _ if self.r_g8(0x191D) != 0 => false,
            _ => refuse(self, hooks, text::NOT_HERE),
        }
    }

    /// 1194:B9E4. A tavern's room: allowed (no carry) while its file is
    /// read and hours are left on it; otherwise "no room" refuses.
    fn room_camp(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> bool {
        if !hooks.room_load(self, files) && self.r_g16(ROOM_HOURS) != 0 {
            return false;
        }
        self.mem.w8(DS, COLOUR, 0x60);
        self.rest_message(hooks, text::NO_ROOM);
        true
    }

    /// 2433:12D6. Camping: the check, then the rest.
    pub fn camp(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        if self.camp_check(files, hooks) {
            return Some(());
        }
        self.rest_start(files, hooks)
    }

    /// 1194:424E. The rest begins: FATIGUE_LIMIT set, HOURS 0, then the
    /// menu. None picked (negative) ends it; the first entry (0) asks
    /// for the hours (`rest_hours`, counting down); another rests until
    /// healed (HOURS 1, COUNTDOWN 0): "You don't need to rest." when
    /// rested already; otherwise no enemies and no actors, and in a
    /// rented room the hours pass at once (`room_rest`), elsewhere the
    /// first step is taken. Throughout, the clip's bottom (A91A) is 92h,
    /// the view's (4252), the boxes centred above the bar; it is put back
    /// on every way out (42B0, 4274).
    pub fn rest_start(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        let bottom = self.mem.r16(DS, 0xA91A);
        self.mem.w16(DS, 0xA91A, 0x92);
        let r = self.rest_start_body(files, hooks);
        self.mem.w16(DS, 0xA91A, bottom);
        r
    }

    fn rest_start_body(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        let limit = self.rest_fatigue_limit();
        self.mem.w16(DS, FATIGUE_LIMIT, limit);
        self.mem.w16(DS, HOURS, 0);
        // 1194:4317: FONT_A.DAT ([1026]) and the tiles [1036] selected
        // (0000:9AE6, 9AF8), then the menu.
        self.select_font(crate::shops::font::A);
        self.select_tiles(0x1036);
        let (si, bx, di) = text::MENU;
        let pick = hooks.menu(self, si, bx, di);
        if (pick as i16) < 0 {
            return Some(());
        }
        if pick == 0 {
            self.rest_clear_actors();
            self.mem.w8(DS, COUNTDOWN, 1);
            return self.rest_hours(files, hooks);
        }
        let h = self.r_g16(HOURS).wrapping_add(1);
        self.mem.w16(DS, HOURS, h);
        self.mem.w8(DS, COUNTDOWN, 0);
        if self.rest_rested() {
            self.rest_colours();
            self.rest_message(hooks, text::NO_NEED);
            return Some(());
        }
        self.rest_clear_actors();
        if self.rest_in_room() {
            return self.room_rest(files, hooks);
        }
        self.mem.w8(DS, RESTING, 1);
        self.rest_step(files, hooks)
    }

    /// 1194:46BF. The hours to rest: typed until they are only digits
    /// (1194:485C); nothing typed, or 0, ends the rest. In a rented room
    /// they pass at once (`room_hours`); elsewhere HOURS counts them
    /// down, after an hour's healing at once.
    fn rest_hours(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        let n = loop {
            // 46BF: ARENAFNT.DAT ([102C]) and the tiles [1036] selected
            // (0000:9AE6, 9AF8) round the field's zeroing.
            self.select_font(crate::shops::font::ARENA);
            self.mem.w32(DS, TYPED, 0);
            self.select_tiles(0x1036);
            self.mem.w8(DS, COLOUR, 0x60);
            hooks.type_hours(self);
            if self.r_g8(TYPED) == 0 {
                self.rest_end();
                return Some(());
            }
            if self.rest_digits(TYPED) {
                break self.info_atoi(DS, TYPED);
            }
        };
        if n == 0 {
            self.rest_end();
            return Some(());
        }
        if self.rest_in_room() {
            return self.room_hours(files, hooks, n);
        }
        self.mem.w16(DS, HOURS, n);
        self.mem.w8(DS, REGEN_KIND, 0);
        self.rest_heal(1)?;
        let si = self.rest_left_text();
        self.rest_hours_text(hooks, si);
        self.mem.w8(DS, RESTING, 1);
        Some(())
    }

    /// 1194:485C. The text at DS:`si` is digits only, and not empty.
    pub(crate) fn rest_digits(&self, mut si: u16) -> bool {
        let mut n = 0u16;
        loop {
            let c = self.r_g8(si);
            if c == 0 {
                return n != 0;
            }
            if !c.is_ascii_digit() {
                return false;
            }
            si = si.wrapping_add(1);
            n = n.wrapping_add(1);
        }
    }

    /// 1194:4635. A frame of rest, while RESTING: enemies near end it
    /// ("Enemies are nearby", 1194:4697), as does a key (AFF9); otherwise
    /// a step, counting down the hours typed (COUNTDOWN) or until healed.
    pub fn rest_frame(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        if self.r_g8(RESTING) == 0 {
            return Some(());
        }
        if self.r_g16(ENEMIES) != 0 {
            self.rest_colours();
            self.rest_message(hooks, text::ENEMIES);
            self.rest_end();
            return Some(());
        }
        if self.r_g8(KEY) != 0 {
            self.rest_end();
            return Some(());
        }
        if self.r_g8(COUNTDOWN) == 1 {
            self.rest_count_step(files, hooks)
        } else {
            self.rest_step(files, hooks)
        }
    }

    /// 1194:4430. A step until healed. In a rented room the third step
    /// ends it (`room_rest` passed the hours): "rested" when it shows,
    /// otherwise the room is gone. Elsewhere 20 minutes pass, the hour's
    /// third step heals (`rest_heal`, HOURS one more), and once rested
    /// the rest ends; the dream may come, then "rested".
    fn rest_step(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        if self.rest_in_room() {
            let s = self.r_g8(STEP).wrapping_add(1);
            self.mem.w8(DS, STEP, s);
            if s != 3 {
                self.mem.w8(DS, RESTING, 1);
                return Some(());
            }
            let si = self.rest_passed_text();
            self.rest_hours_text(hooks, si);
            hooks.screen(self);
            self.rest_colours();
            let si = if self.rest_rested() {
                text::RESTED
            } else {
                self.mem.w16(DS, ROOM_HOURS, 0);
                text::ROOM_GONE
            };
            self.rest_message(hooks, si);
            self.mem.w8(DS, RESTING, 0);
            self.mem.w8(DS, STEP, 0);
            return Some(());
        }
        let s = self.r_g8(STEP).wrapping_add(1);
        self.mem.w8(DS, STEP, s);
        let si = self.rest_passed_text();
        self.rest_hours_text(hooks, si);
        self.rest_advance(files, hooks, 0x14)?;
        if self.r_g8(STEP) != 3 {
            self.mem.w8(DS, RESTING, 1);
            return Some(());
        }
        self.mem.w8(DS, STEP, 0);
        self.mem.w8(DS, REGEN_KIND, 0);
        self.rest_heal(1)?;
        let si = self.rest_passed_text();
        self.rest_hours_text(hooks, si);
        let h = self.r_g16(HOURS).wrapping_add(1);
        self.mem.w16(DS, HOURS, h);
        if !self.rest_full() {
            self.mem.w8(DS, RESTING, 1);
            return Some(());
        }
        self.rest_end();
        if !self.dream_check() {
            self.dream_show(files, hooks)?;
            self.mem.w16(DS, crate::sky::var::WINDOW, 0xFFFF);
            hooks.sky(self);
        }
        self.rest_colours();
        self.rest_message(hooks, text::RESTED);
        self.mem.w8(DS, RESTING, 0);
        Some(())
    }

    /// 1194:456F. A step counting down. In a rented room the third step
    /// ends it ("done"). Elsewhere 20 minutes pass, the hour's third step
    /// heals and counts an hour down; at 0 the rest ends after a wait,
    /// the dream may come, then "done".
    fn rest_count_step(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        if self.rest_in_room() {
            let s = self.r_g8(STEP).wrapping_add(1);
            self.mem.w8(DS, STEP, s);
            if s != 3 {
                self.mem.w8(DS, RESTING, 1);
                return Some(());
            }
            let si = self.rest_passed_text();
            self.rest_hours_text(hooks, si);
            hooks.screen(self);
            self.rest_colours();
            self.rest_message(hooks, text::DONE);
            self.mem.w8(DS, RESTING, 0);
            self.mem.w8(DS, STEP, 0);
            return Some(());
        }
        let s = self.r_g8(STEP).wrapping_add(1);
        self.mem.w8(DS, STEP, s);
        let si = self.rest_left_text();
        self.rest_hours_text(hooks, si);
        self.rest_advance(files, hooks, 0x14)?;
        if self.r_g8(STEP) != 3 {
            return Some(());
        }
        self.mem.w8(DS, STEP, 0);
        self.mem.w8(DS, REGEN_KIND, 0);
        self.rest_heal(1)?;
        let si = self.rest_left_text();
        self.rest_hours_text(hooks, si);
        let h = self.r_g16(HOURS).wrapping_sub(1);
        self.mem.w16(DS, HOURS, h);
        if h != 0 {
            self.mem.w8(DS, RESTING, 1);
            return Some(());
        }
        hooks.screen(self);
        if !self.dream_check() {
            self.dream_show(files, hooks)?;
            self.mem.w16(DS, crate::sky::var::WINDOW, 0xFFFF);
            hooks.sky(self);
        }
        self.rest_colours();
        self.rest_message(hooks, text::DONE);
        self.rest_end();
        Some(())
    }

    /// 1194:437F. Resting until healed in a rented room: an hour at a
    /// time against the room's hours (a CX loop: 0 is 65536), healing
    /// each (REGEN_KIND as it is), until rested or enemies come. The
    /// room's file is written (`room_file`); with no enemies the dream may
    /// come; RESTING stays set for the steps that end it.
    fn room_rest(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        self.mem.w8(DS, COUNTDOWN, 0);
        let n = match self.r_g16(ROOM_HOURS) {
            0 => 0x10000u32,
            n => n as u32,
        };
        self.mem.w16(DS, HOURS, 0);
        for _ in 0..n {
            self.rest_advance(files, hooks, 0x3C)?;
            let h = self.r_g16(HOURS).wrapping_add(1);
            self.mem.w16(DS, HOURS, h);
            self.rest_heal(1)?;
            if self.r_g16(ENEMIES) != 0 {
                self.mem.w8(DS, RESTING, 0);
                self.r_or8(0x0D39, 0x80);
                return Some(());
            }
            let r = self.r_g16(ROOM_HOURS).wrapping_sub(1);
            self.mem.w16(DS, ROOM_HOURS, r);
            if self.rest_rested() {
                break;
            }
        }
        self.room_save(files, hooks);
        if self.r_g16(ENEMIES) == 0 && !self.dream_check() {
            hooks.screen(self);
            self.mem.w8(DS, 0xC994, 1);
            self.dream_show(files, hooks)?;
        } else {
            hooks.screen(self);
        }
        self.mem.w8(DS, RESTING, 1);
        self.mem.w8(DS, COUNTDOWN, 0);
        self.r_or8(0x0D39, 0x80);
        hooks.screen(self);
        Some(())
    }

    /// 1194:4757. `hours` typed in a rented room: checked against the
    /// room (`room_hours_check`), then they pass an hour at a time
    /// (healing, the room's hours less one each) unless enemies come; the
    /// room's file is written, the dream may come.
    fn room_hours(&mut self, files: &mut Files, hooks: &mut dyn RestHooks, hours: u16) -> Option<()> {
        let Some(n) = self.room_hours_check(hooks, hours) else {
            self.r_or8(0x0D39, 0x80);
            hooks.screen(self);
            return Some(());
        };
        self.mem.w8(DS, RESTING, 1);
        self.mem.w16(DS, HOURS, 0);
        let n = if n == 0 { 0x10000u32 } else { n as u32 };
        let mut enemies = false;
        for _ in 0..n {
            self.rest_advance(files, hooks, 0x3C)?;
            self.rest_heal(1)?;
            let r = self.r_g16(ROOM_HOURS).wrapping_sub(1);
            self.mem.w16(DS, ROOM_HOURS, r);
            if self.r_g16(ENEMIES) != 0 {
                enemies = true;
                break;
            }
            let h = self.r_g16(HOURS).wrapping_add(1);
            self.mem.w16(DS, HOURS, h);
        }
        if enemies {
            self.mem.w8(DS, RESTING, 0);
        } else {
            if self.r_g16(ENEMIES) == 0 && !self.dream_check() {
                hooks.screen(self);
                self.mem.w8(DS, 0xC994, 1);
                self.dream_show(files, hooks)?;
            } else {
                hooks.screen(self);
            }
            self.mem.w8(DS, RESTING, 1);
            self.mem.w8(DS, COUNTDOWN, 1);
        }
        self.room_save(files, hooks);
        self.r_or8(0x0D39, 0x80);
        hooks.screen(self);
        Some(())
    }

    /// 1194:BA91 (the file is the hooks'; nothing when no hours are
    /// left but its deletion).
    fn room_save(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) {
        hooks.room_file(self, files);
    }

    /// 1194:BE84. The hours to rest in the room: when counting down
    /// (COUNTDOWN), `hours` asked (ASKED) checked against the room
    /// (1194:C1A8); otherwise the room's hours. None: not resting.
    fn room_hours_check(&mut self, hooks: &mut dyn RestHooks, hours: u16) -> Option<u16> {
        if self.r_g8(COUNTDOWN) == 0 {
            return Some(self.r_g16(ROOM_HOURS));
        }
        self.mem.w16(DS, ASKED, hours);
        let ax = self.room_time(hooks, hours)?;
        self.mem.w16(DS, ASKED, ax);
        Some(ax)
    }

    /// 1194:C1A8. The room's time left (ROOM_END less the clock, in hours
    /// rounded up): none, the room is gone ("ROOM_GONE", its flags and
    /// hours cleared). Asked for more than that: "You only have %d
    /// remaining hours. Rest this long?" with the lesser of it and the
    /// room's hours; any pick but the first (0) cancels. Asked for more
    /// than the room's hours: the same question with those.
    fn room_time(&mut self, hooks: &mut dyn RestHooks, asked: u16) -> Option<u16> {
        let left = self.mem.r32(DS, ROOM_END).wrapping_sub(self.mem.r32(DS, 0x05CC));
        let gone = |m: &mut Machine, hooks: &mut dyn RestHooks| {
            m.mem.w8(DS, ROOM_FLAG, 0);
            m.mem.w8(DS, ROOM_FLAG_2, 0);
            // 1194:C222: ARENAFNT ([102C]) through 42EE.
            m.message_fonts(0x102C);
            m.rest_message(hooks, text::ROOM_GONE);
            m.mem.w16(DS, ROOM_HOURS, 0);
            None
        };
        if (left as i32) < 0 {
            return gone(self, hooks);
        }
        let mut ax = left / 0x3C;
        if ax == 0 {
            return gone(self, hooks);
        }
        if left % 0x3C != 0 {
            ax += 1;
        }
        let ax = ax as u16;
        if asked > ax {
            let cx = if ax < self.r_g16(ROOM_HOURS) { ax } else { self.r_g16(ROOM_HOURS) };
            // 1194:C1E3: 42EE with FONT_A ([1026]).
            self.rest_colours();
            self.sprintf((DS, TEXT), (DS, text::ONLY), &[Arg::Int(cx)]);
            self.mem.w8(DS, COLOUR, 0x60);
            let (bx, di) = text::ONLY_MENU;
            if hooks.menu(self, TEXT, bx, di) != 0 {
                return None;
            }
            return Some(cx);
        }
        let ax = self.r_g16(ASKED);
        if ax <= self.r_g16(ROOM_HOURS) {
            return Some(ax);
        }
        let n = self.r_g16(ROOM_HOURS);
        // 1194:BEC5: the text, then FONT_A ([1026]) selected (BED6).
        self.sprintf((DS, TEXT), (DS, text::ONLY), &[Arg::Int(n)]);
        self.select_font(0x1026);
        let (bx, di) = text::ONLY_MENU;
        if hooks.menu(self, TEXT, bx, di) != 0 {
            return None;
        }
        Some(self.r_g16(ROOM_HOURS))
    }

    /// 1194:D340. The main quest's dream: none (the carry) when one is
    /// pending already or the player's level byte is 0; otherwise it is
    /// pending (DREAM set, DREAM_SHOWN cleared), shown while fewer than
    /// 8 were (DREAM_COUNT).
    fn dream_check(&mut self) -> bool {
        if self.r_g8(DREAM) != 0 || self.r_g8(0x01AA) < 1 {
            return true;
        }
        self.mem.w8(DS, DREAM, 1);
        self.mem.w8(DS, DREAM_SHOWN, 0);
        self.r_g8(DREAM_COUNT) >= 8
    }

    /// 1194:D3AF. A pending dream not yet shown: the world kept (A611);
    /// at level 1 the first (text 5DCh on CHAOSVSN.FLC, DI 4E16h) and the
    /// colours black (55B2), then the one for DREAM_COUNT (50Eh + it, a
    /// byte add, on VISION.FLC, DI 8085h); the world given back (A6B5,
    /// 2542:0446), A91A as it was, the clip and [1026]'s font (9A98, 9AE6).
    /// None where the vision or the world's return gives up.
    pub fn dream_show(&mut self, files: &mut Files, hooks: &mut dyn RestHooks) -> Option<()> {
        if self.r_g8(DREAM) == 0 || self.r_g8(DREAM_SHOWN) != 0 {
            return Some(());
        }
        let a91a = self.r_g16(0xA91A);
        hooks.dream_keep(self, files);
        let a902 = 0xA902;
        let flags = |m: &mut Machine| {
            let f = (m.r_g16(a902) | 0x200) & 0xFEFF;
            m.mem.w16(DS, a902, f);
        };
        if self.r_g8(0x01AA) == 1 {
            self.mem.w8(DS, 0x803C, 0);
            flags(self);
            self.mem.w8(DS, COLOUR, 0x33);
            hooks.dream(self, files, 0x5DC, 0x809A, 0x4E16)?;
            hooks.dream_black(self);
        }
        self.mem.w8(DS, 0x803C, 0);
        let text = 0x0500 | 0x0Eu8.wrapping_add(self.r_g8(DREAM_COUNT)) as u16;
        flags(self);
        self.mem.w8(DS, COLOUR, 0x2F);
        hooks.dream(self, files, text, 0x809A, 0x8085)?;
        self.mem.w8(DS, 0xC994, 1);
        hooks.dream_back(self, files)?;
        self.mem.w16(DS, 0xA91A, a91a);
        hooks.dream_screen(self);
        self.r_or8(0x0D39, 0x80);
        self.mem.w8(DS, DREAM_SHOWN, 1);
        self.mem.w8(DS, DREAM, 1);
        Some(())
    }
}
