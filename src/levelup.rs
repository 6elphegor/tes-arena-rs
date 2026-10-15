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
}
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

/// What the level-up screen leaves to the front end.
pub trait LevelUpHooks {
    /// 1194:5E33: the level's notice (a sound, its text in a box), unless
    /// the main quest is over.
    fn notice(&mut self, _m: &mut Machine) {}
    /// 1194:158F: the screen kept (CHARSHET.$$$), unless 5E0 bit 200h.
    fn keep_screen(&mut self, _m: &mut Machine) {}
    /// The screen drawn: 0000:541E, A611, the view filled (9A98), 9426,
    /// the palette black (1194:55B2), the body's picture (2462).
    fn screen(&mut self, _m: &mut Machine) {}
    /// 1194:122C: the player spends the points (POINTS) on the attributes
    /// (at the player's +29h, percent), as on the creation's screen
    /// (`Machine::creation_select`, `creation_arrow`). By default they go
    /// to the attributes in turn, one at a time.
    fn spend(&mut self, m: &mut Machine) {
        spend_in_turn(m);
    }
    /// 1194:59CC: a message box with the text at DS:`si` (3E17: points
    /// are left to spend).
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 0000:541E, A6B5: the game's screen back.
    fn restore(&mut self, _m: &mut Machine) {}
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
            m.creation_arrow(0);
        }
        if m.mem.r8(DS, POINTS) == before {
            return;
        }
    }
}

/// 2686:0319..0331: B038 set from 103A (0000:9AF8), the level-up screen
/// (`Machine::level_up_screen`), then B038 from 1036.
pub fn level_up_around(m: &mut Machine, levels: u16, hooks: &mut dyn LevelUpHooks) -> Option<()> {
    let v = m.mem.r16(DS, 0x103A);
    m.mem.w16(DS, TEXT_B038, v);
    m.level_up_screen(levels, hooks)?;
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
    /// faults.
    pub fn level_up_screen(&mut self, levels: u16, hooks: &mut dyn LevelUpHooks) -> Option<()> {
        let (sp, fatigue) = (self.l16(0x020A), self.l16(0x0201));
        let finale = self.l8(0x0FDD) == 0x8E;
        self.mem.w16(DS, FINALE, finale as u16);
        if !finale {
            hooks.notice(self);
        }
        if self.mem.r32(DS, BASE) == 0xFFFF_FFFF && self.mem.r32(DS, BASE + 4) == 0xFFFF_FFFF {
            let max = self.l16(PLAYER + field::MAX_HP);
            self.mem.w16(DS, PLAYER + field::HP, max);
            self.mem.w16(DS, 0x0201, fatigue);
            self.mem.w16(DS, 0x020A, sp);
            return Some(());
        }
        if self.l16(0x05E0) & 0x200 == 0 {
            hooks.keep_screen(self);
        }
        self.worn_off();
        self.copy8(ATTRS, OLD_ATTRS);
        crate::scale::attrs_to100(&mut self.mem, BASE, BASE, 8);
        self.copy8(BASE, ATTRS);
        hooks.screen(self);
        self.mem.w16(DS, SHEET, 1);
        let f = self.l16(0x05C8) | 0x10;
        self.mem.w16(DS, 0x05C8, f);
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
            hooks.spend(self);
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
        hooks.restore(self);
        self.mem.w16(DS, 0x0201, fatigue);
        self.mem.w16(DS, 0x020A, sp);
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
