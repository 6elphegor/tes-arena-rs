//! The buildings' services (the routines of the table at 869F, run by
//! `talk` for the building's keeper): so far the temple (1194:C258).
//!
//! A temple offers, by its menu (keys B, C, H, E):
//!
//! - healing (1194:C2C7): 15 gold a level (plus one), health (1FD) back
//!   to its maximum (1FF);
//! - curing (1194:C36E): of the player's afflictions (+40Dh bits 1, 2,
//!   4: disease, poison, paralysis), each priced from the table at 76A3
//!   (25, 50, 75) times the level plus one (at most 20) plus the maximum
//!   health, one picked from a list ("Cure all" with more than one); the
//!   cure ends the affliction (2EBD:0004, `cure_effect`);
//! - blessing (1194:C58B): a donation typed in, at least five gold a level
//!   (plus one); a day's first donation of at least that adds
//!   donation / minimum, in percent turned to 256ths (1194:5B34), to the
//!   blessing (5C6).
//!
//! On a holiday whose shop kind (the table at 3353) is the temple's (5),
//! prices are scaled by the holiday's factor in quarters (1194:4A6B).
//! Prices are paid from the player's gold (5C2; "not enough" otherwise).
//!
//! The menus, dialogs, lists, messages and the greeting are
//! `ShopHooks`, as are the quests' checks on entering (1194:D961,
//! 8F1E). Bugs kept: "Cure all" pays the list's index (its EAX), cures
//! the first affliction, and then calls through the table at 7620 with
//! the record's offset plus one as the index (2EBD:0004 leaves it in AX),
//! which is not code: the port returns None there; a second donation
//! within a day of the first is taken but blesses nothing; the
//! single cure's price is added to the list's total (12BF) again.

use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::DS;

/// What the services leave to the front end.
pub trait ShopHooks {
    /// 1194:4D31: a message box with the text at DS:`si`.
    fn message(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:4B60 and 4BFF on entering: the shop's screen (MENUSCRN.IMG)
    /// and its name at DS:`si` on a banner across its top (FONT_C.DAT on
    /// MENU_CTR.IMG's tiles between MENU_LFT and MENU_RT).
    fn banner(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:4B8E: the picture named at DS:`si` shown on the shop's screen
    /// (the temple's healing: YESNO.IMG under its question; the store's
    /// repairs NEWOLD.IMG and YESNO.IMG; the guild's and the tavern's
    /// theirs).
    fn picture(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:545C: the gold left shown under the shop's menu, after the
    /// strip there is saved (1194:5F8C; the next 5FC0 puts it back):
    /// `Machine::gold_left`'s text at 1044 drawn centred on x A0h at row
    /// 9Ch (0000:5364) and shown (9466).
    fn gold_left(&mut self, _m: &mut Machine) {}
    /// 1194:5FC0: the strip under the menu (0Bh, 91h; 12Ah by 1Fh) put
    /// back as 5F8C saved it (0000:599C from [9030]:7530) and shown: the
    /// gold left's line gone. Called where it can follow `gold_left`.
    fn strip_back(&mut self, _m: &mut Machine) {}
    /// 1194:020E: a menu, its title at `si`, its keys at `bx`, its
    /// entries' table at `di`. The pick (FFFFh: none).
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        0xFFFF
    }
    /// 1194:4E88: a dialog of kind `kind` (13D3: 1 yes or no, AX 0 yes;
    /// 2 a number typed into DS:`di`). Its AX.
    fn dialog(&mut self, _m: &mut Machine, _kind: u8, _di: u16) -> u16 {
        1
    }
    /// `dialog` with its text: 1194:4E88 shows the lines 1194:4E54 split
    /// at DS:`si` (LINES of them, each ending at a NUL), as SI holds them.
    fn dialog_at(&mut self, m: &mut Machine, kind: u8, _si: u16, di: u16) -> u16 {
        self.dialog(m, kind, di)
    }
    /// 1194:5734: a list of `cx` strings at DS:`si` (its title at `di`).
    /// The pick (FFFFh: none).
    fn list(&mut self, _m: &mut Machine, _si: u16, _cx: u16, _di: u16) -> u16 {
        0xFFFF
    }
    /// 2DA2:000A: the text at DS:`di` has its substitutions made; returns
    /// DI after (at its NUL).
    fn expand(&mut self, m: &mut Machine, di: u16) -> u16 {
        let mut di = di;
        while m.mem.r8(DS, di) != 0 {
            di = di.wrapping_add(1);
        }
        di
    }
    /// 1194:D961: the main quest's person, when the stage's province is
    /// this one (`Machine::main_quest_check`, 1194:D593).
    fn quest_check(&mut self, _m: &mut Machine, _files: &mut crate::bsa::Files) -> Option<()> {
        Some(())
    }
    /// 1194:8F1E: the quests the talk ends or offers on entering
    /// (`Machine::quests_talk`).
    fn quests(&mut self, m: &mut Machine, files: &mut crate::bsa::Files) -> Option<()> {
        m.quests_talk(files, &mut crate::quest::NoQuestHooks)
    }
    /// 1194:5496 on entering: the colours read, the screen saved, the clip
    /// the whole screen (`Machine::shop_screen_saved`).
    fn screen_saved(&mut self, _m: &mut Machine, _files: &mut crate::bsa::Files) {}
    /// 1194:54B3 on leaving: the screen read back, the clip the whole
    /// screen, the interface made again by the next frame
    /// (`Machine::shop_screen_back`).
    fn screen_back(&mut self, _m: &mut Machine, _files: &mut crate::bsa::Files) {}
}

/// `ShopHooks` that do nothing (every menu left at once).
pub struct NoShopHooks;

/// 1194:545C's text: "Gold left : " (4B0E), with a space for its NUL.
const GOLD_TEXT: u16 = 0x4B0E;
/// Where 545C builds it (also the spell list's order, 0000:0B9C's).
pub const GOLD_LINE: u16 = 0x1044;

/// The fonts' records the screens select from (0000:9AE6 with AX one of
/// these words): FONT_A.DAT, ARENAFNT.DAT, TEENYFNT.DAT.
pub mod font {
    pub const A: u16 = 0x1026;
    pub const ARENA: u16 = 0x102C;
    pub const TEENY: u16 = 0x102E;
}

impl Machine {
    /// 0000:9AE6 with AX the word at DS:`at` (`font`): that font selected
    /// for the text drawn after (B036). What is drawn is the front end's;
    /// the selection is the game's state its boxes and lists read.
    pub fn select_font(&mut self, at: u16) {
        let v = self.mem.r16(DS, at);
        self.mem.w16(DS, 0xB036, v);
    }

    /// 1194:42EE with AX the word at DS:`at` (`font`): before a message
    /// box, the font selected, the tiles at 1036 and the colour 60h.
    pub fn message_fonts(&mut self, at: u16) {
        self.select_font(at);
        self.select_tiles(0x1036);
        self.mem.w8(DS, 0x9288, 0x60);
    }

    /// 0000:9AF8 with AX the word at DS:`at` (1034h or 1036h): the boxes'
    /// tiles selected (B038).
    pub fn select_tiles(&mut self, at: u16) {
        let v = self.mem.r16(DS, at);
        self.mem.w16(DS, 0xB038, v);
    }

    /// 1194:545C, the state it leaves: the text colour 41h (9288), the
    /// font [1026] selected (0000:9AE6: B036), and at 1044 "Gold left : "
    /// (4B0E, 0000:9CA0) with a space over its NUL and the gold (5C2) in
    /// decimal after it (0000:75B0). Its drawing is the front end's
    /// (`ShopHooks::gold_left`). DI after the number's NUL.
    pub fn gold_left(&mut self) -> u16 {
        self.mem.w8(DS, 0x9288, 0x41);
        let font = self.mem.r16(DS, 0x1026);
        self.mem.w16(DS, 0xB036, font);
        let mut di = GOLD_LINE;
        let mut si = GOLD_TEXT;
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        self.mem.w8(DS, di.wrapping_sub(1), 0x20);
        let gold = self.mem.r32(DS, GOLD);
        self.dun_ultoa(gold, di);
        let digits = if gold == 0 { 1 } else { gold.ilog10() as u16 + 1 };
        di.wrapping_add(digits).wrapping_add(1)
    }
}

impl ShopHooks for NoShopHooks {}

/// DGROUP variables.
pub mod var {
    /// The kind of shop being visited (FFh none): 5 a temple.
    pub const SHOP: u16 = 0x137F;
    /// A holiday discount applies (1; FFh not).
    pub const HOLIDAY: u16 = 0x13C9;
    /// The price asked (dword).
    pub const PRICE: u16 = 0x1391;
    /// The player's gold (dword), level (byte), health and its maximum,
    /// name, afflictions (+40Dh) and blessing.
    pub const GOLD: u16 = 0x05C2;
    pub const LEVEL: u16 = 0x01AA;
    pub const HEALTH: u16 = 0x01FD;
    pub const MAX_HEALTH: u16 = 0x01FF;
    pub const NAME: u16 = 0x01AD;
    pub const AFFLICTIONS: u16 = 0x05B1;
    pub const BLESSING: u16 = 0x05C6;
    /// The time (minutes, dword).
    pub const TIME: u16 = 0x05CC;
    /// The cures on offer (3 bytes), their list's total, where the next
    /// entry goes, their count, and the picks' afflictions (4 bytes).
    pub const CURES: u16 = 0x12C9;
    pub const TOTAL: u16 = 0x12BF;
    pub const ENTRY_AT: u16 = 0x12C1;
    pub const COUNT: u16 = 0x12BE;
    pub const PICKS: u16 = 0x76DA;
    /// The pick.
    pub const PICK: u16 = 0x13C8;
    /// The donation: the least, the last donation, its time, and the one
    /// given.
    pub const LEAST: u16 = 0x12C3;
    pub const DONATED: u16 = 0x13D1;
    pub const DONATED_AT: u16 = 0x13CD;
    pub const GIVEN: u16 = 0x12C5;
    /// The donation's typed digits (5 bytes).
    pub const TYPED: u16 = 0x75F1;
    /// The lines a text was split into, and the line length mark.
    pub const LINES: u16 = 0x0FF5;
    pub const LINE_MARK: u16 = 0x137E;
    pub const DIALOG: u16 = 0x13D3;
}
use var::*;

/// The holidays' days (words, rising) and their shops and factors (6
/// bytes a holiday: the shop kind, then factors by the service's index).
const HOLIDAYS: u16 = 0x33AD;
const HOLIDAY_SHOPS: u16 = 0x3353;
/// The temple's menu, cure handlers, prices and texts.
const CURE_HANDLERS: u16 = 0x7620;
const CURE_PRICES: u16 = 0x76A3;
const NAMES: u16 = 0x1395;

impl Machine {
    /// 2DA2:111C: whether the day of the year of time `eax` (minutes; 360
    /// days of 1440) is a holiday (the list at 33AD): Some(its index), or
    /// None. Past day 344 (unsigned) the original reads a word through an
    /// unset SI; it only says no.
    pub fn holiday(&self, eax: u32) -> Option<u16> {
        let (cx, found) = self.holiday_search(eax);
        found.then_some(cx)
    }

    /// 2DA2:111C's CX and carry: the holiday's index and true, or where
    /// the search stopped (the first holiday after the day; 0 past day
    /// 158h) and false.
    fn holiday_search(&self, eax: u32) -> (u16, bool) {
        let day = (eax % 0x7E900) / 0x5A0;
        if day > 0x158 {
            return (0, false);
        }
        let mut si = HOLIDAYS;
        let mut cx = 0u16;
        loop {
            let d = self.mem.r16(DS, si) as i16;
            if d == day as i16 {
                return (cx, true);
            }
            if d > day as i16 {
                return (cx, false);
            }
            si = si.wrapping_add(2);
            cx = cx.wrapping_add(1);
        }
    }

    /// 1194:4A4A: whether today's holiday is this shop's (SHOP).
    pub fn shop_holiday(&self) -> bool {
        let Some(cx) = self.holiday(self.mem.r32(DS, TIME)) else { return false };
        self.mem.r8(DS, HOLIDAY_SHOPS.wrapping_add(cx.wrapping_mul(6))) == self.mem.r8(DS, SHOP)
    }

    /// 1194:4A6B: price `eax` on today's holiday for service `bx`: times
    /// the holiday's factor (byte `bx` of its entry) over four (the low
    /// dword), at least 1 when the factor isn't 0. Not a holiday: the
    /// entry the search stopped at (the first holiday after today; 0 past
    /// day 158h), as the original's CX.
    pub(crate) fn holiday_price(&self, eax: u32, bx: u16) -> u32 {
        let (cx, _) = self.holiday_search(self.mem.r32(DS, TIME));
        let f = self.mem.r8(DS, HOLIDAY_SHOPS.wrapping_add(cx.wrapping_mul(6)).wrapping_add(bx)) as u32;
        let p = eax.wrapping_mul(f) >> 2;
        if p == 0 && f != 0 { 1 } else { p }
    }

    /// 1194:4E54: the text at `si` split at its line feeds: each becomes
    /// a NUL, and every LINE_MARK-th line ends at the NUL after it too;
    /// the lines counted at LINES.
    pub fn split_lines(&mut self, si: u16) {
        let mark = self.mem.r8(DS, LINE_MARK);
        let (mut si, mut cx, mut dl) = (si, 0u16, 0u8);
        while self.mem.r8(DS, si) != 0 {
            if self.mem.r8(DS, si) == 0x0A {
                self.mem.w8(DS, si, 0);
                cx = cx.wrapping_add(1);
                dl = dl.wrapping_add(1);
                if dl == mark {
                    self.mem.w8(DS, si, 0);
                    dl = 0;
                }
            }
            si = si.wrapping_add(1);
        }
        self.mem.w16(DS, LINES, cx.wrapping_add(1));
    }

    /// 1194:C572: price `eax` paid from the player's gold, or "not enough"
    /// (198C) and nothing paid (the carry).
    fn pay(&mut self, hooks: &mut dyn ShopHooks, eax: u32) -> bool {
        let gold = self.mem.r32(DS, GOLD);
        if gold < eax {
            hooks.message(self, 0x198C);
            return true;
        }
        self.mem.w32(DS, GOLD, gold - eax);
        false
    }

    /// The text `fmt` with the player's name (1AD) at `dst`, shown.
    fn shop_say_name(&mut self, hooks: &mut dyn ShopHooks, fmt: u16, dst: u16) {
        self.sprintf((DS, dst), (DS, fmt), &[Arg::Str(DS, NAME)]);
        hooks.message(self, dst);
    }

    /// 0000:9CA0: the string at DS:si copied to DS:di with its NUL.
    fn shop_copy(&mut self, mut si: u16, mut di: u16) -> u16 {
        loop {
            let b = self.mem.r8(DS, si);
            self.mem.w8(DS, di, b);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if b == 0 {
                return di;
            }
        }
    }

    /// 1194:C2C7. Healing, unless the player's health is full ("in
    /// perfect condition"): the price (15 a level plus one, the holiday's
    /// factor 2) asked (yes or no) and paid, then "thou art healed" and
    /// health to its maximum; free on a holiday that makes it 0.
    fn temple_heal(&mut self, hooks: &mut dyn ShopHooks) {
        let max = self.mem.r16(DS, MAX_HEALTH);
        if self.mem.r16(DS, HEALTH) == max {
            self.shop_say_name(hooks, 0x75D3, 0x10A8);
            return;
        }
        let level = self.mem.r8(DS, LEVEL) as u32 + 1;
        let mut eax = (15 * level) & 0xFFFF;
        if self.mem.r8(DS, HOLIDAY) == 1 {
            eax = self.holiday_price(eax, 2);
        }
        if eax != 0 {
            self.mem.w32(DS, PRICE, eax);
            self.sprintf((DS, 0x93CA), (DS, 0x76A7), &[Arg::Int(eax as u16)]);
            // C30D: YESNO.IMG (132F) under the question.
            hooks.picture(self, 0x132F);
            self.mem.w8(DS, LINE_MARK, 3);
            self.split_lines(0x93CA);
            self.mem.w8(DS, DIALOG, 1);
            if hooks.dialog_at(self, 1, 0x93CA, 0) != 0 {
                return;
            }
            let p = self.mem.r32(DS, PRICE);
            if self.pay(hooks, p) {
                return;
            }
        }
        self.shop_say_name(hooks, 0x75BC, 0x10A8);
        let max = self.mem.r16(DS, MAX_HEALTH);
        self.mem.w16(DS, HEALTH, max);
    }

    /// 1194:C4FF: the price of cure `bx`: its factor (76A3) times the
    /// level plus one (at most 20), in a byte multiply, plus the maximum
    /// health; added to TOTAL; the holiday's factor 3; kept at PRICE.
    fn cure_price(&mut self, bx: u16) -> u32 {
        let f = self.mem.r8(DS, CURE_PRICES.wrapping_add(bx)) as u16;
        let l = (self.mem.r8(DS, LEVEL).wrapping_add(1)).min(0x14) as u16;
        let ax = (f * l).wrapping_add(self.mem.r16(DS, MAX_HEALTH));
        let t = self.mem.r16(DS, TOTAL).wrapping_add(ax);
        self.mem.w16(DS, TOTAL, t);
        let mut eax = ax as u32;
        if self.mem.r8(DS, HOLIDAY) == 1 {
            eax = self.holiday_price(eax, 3);
        }
        self.mem.w32(DS, PRICE, eax);
        eax
    }

    /// 2EBD:0004. Effect `ax` (an affliction's bit) ended on the record
    /// at DS:`si`: its bit cleared (+40Dh); unless 100h, the kind's count
    /// (+40Fh, kinds under 9) lowered and the effect slots (16 of 3Ch at
    /// 660) targeting the record with that kind (any 0Ah or more for
    /// kind 0) cleared, and if any was, the kind's count zeroed. Returns
    /// AX: the record's offset after the slots (100h: 100h).
    pub fn cure_effect(&mut self, ax: u16, si: u16) -> u16 {
        let f = self.mem.r16(DS, si.wrapping_add(0x40D)) & !ax;
        self.mem.w16(DS, si.wrapping_add(0x40D), f);
        if ax == 0x100 {
            return ax;
        }
        let bx = self.effect_kind(ax as u8);
        if bx < 9 {
            let at = bx.wrapping_add(si).wrapping_add(0x40F);
            let v = self.mem.r8(DS, at).wrapping_sub(1);
            self.mem.w8(DS, at, v);
        }
        let target = (DS as u32) << 16 | si as u32;
        self.mem.w16(DS, 0x1294, 0);
        let bl = bx as u8;
        for k in 0..0x10u16 {
            let s = 0x0660 + k * 0x3C;
            if self.mem.r32(DS, s.wrapping_add(0x28)) != target {
                continue;
            }
            let kind = self.mem.r8(DS, s.wrapping_add(0x3A));
            if (bl == 0 && kind >= 0x0A) || kind == bl {
                self.mem.w16(DS, 0x1294, 1);
                self.clear_slot(s);
            }
        }
        if self.mem.r16(DS, 0x1294) != 0 {
            self.mem.w8(DS, bx.wrapping_add(si).wrapping_add(0x40F), 0);
        }
        si
    }

    /// 1194:C36E. Curing: the player's afflictions (+40Dh bits 1, 2, 4)
    /// listed with their prices ("%u gp\n%s" at 110C on), "Cure all" with
    /// their total when more than one; none: "we cannot cure". The pick
    /// is priced again, paid, and cured; "Cure all" pays the pick's index
    /// and goes wrong (see the module). None there.
    fn temple_cure(&mut self, hooks: &mut dyn ShopHooks) -> Option<()> {
        self.mem.w32(DS, CURES, 0);
        self.mem.w32(DS, PICKS, 0);
        self.mem.w16(DS, TOTAL, 0);
        self.mem.w16(DS, ENTRY_AT, 0x110C);
        self.shop_copy(NAME, NAMES);
        self.sprintf((DS, 0x1044), (DS, 0x7626), &[Arg::Str(DS, NAMES)]);
        let ill = self.mem.r16(DS, 0x01A4 + 0x40D);
        for (bit, at) in [(1u16, 0u16), (2, 1), (4, 2)] {
            if ill & bit != 0 {
                self.mem.w8(DS, CURES + at, 1);
            }
        }
        if self.mem.r32(DS, CURES) == 0 {
            self.shop_copy(NAME, NAMES);
            self.sprintf((DS, 0x11E8), (DS, 0x766C), &[Arg::Str(DS, NAMES)]);
            hooks.message(self, 0x11E8);
            return Some(());
        }
        self.mem.w8(DS, COUNT, 0);
        for bx in 0..3u16 {
            if self.mem.r8(DS, CURES + bx) == 0 {
                continue;
            }
            let n = self.mem.r8(DS, COUNT).wrapping_add(1);
            self.mem.w8(DS, COUNT, n);
            let name = self.skip_strings(DS, 0x7653, bx);
            self.shop_copy(name, NAMES);
            let ax = self.cure_price(bx) as u16;
            let at = self.mem.r16(DS, ENTRY_AT);
            self.sprintf((DS, at), (DS, 0x7633), &[Arg::Int(ax), Arg::Str(DS, NAMES)]);
            // 1194:C4E0: past the entry's NUL (at most 100 bytes).
            let mut di = at;
            for _ in 0..0x64 {
                let b = self.mem.r8(DS, di);
                di = di.wrapping_add(1);
                if b == 0 {
                    break;
                }
            }
            self.mem.w16(DS, ENTRY_AT, di);
        }
        let count = self.mem.r8(DS, COUNT);
        if count > 1 {
            let at = self.mem.r16(DS, ENTRY_AT);
            let total = self.mem.r16(DS, TOTAL);
            self.sprintf((DS, at), (DS, 0x7640), &[Arg::Int(total)]);
            self.mem.w8(DS, COUNT, count + 1);
        }
        let count = self.mem.r8(DS, COUNT);
        // AX Ah (1194:C4F6): the list's kind (1194:5734).
        self.list_kind = 0xA;
        let pick = hooks.list(self, 0x110C, count as u16, 0x1044);
        self.mem.w8(DS, PICK, pick as u8);
        if (pick as i16) < 0 {
            return Some(());
        }
        // 1194:C41E: ARENAFNT selected.
        self.select_font(font::ARENA);
        if count == 1 || pick as u8 != count.wrapping_sub(1) {
            // 1194:C48D: the afflictions present, in order, at PICKS.
            let mut di = PICKS;
            for bl in 0..3u8 {
                if self.mem.r8(DS, CURES + bl as u16) != 0 {
                    self.mem.w8(DS, di, bl);
                    di += 1;
                }
            }
            let bx = self.mem.r8(DS, PICKS.wrapping_add(pick)) as u16;
            let eax = self.cure_price(bx) & 0xFFFF;
            if self.pay(hooks, eax) {
                return Some(());
            }
            let mask = self.cure_handler(bx)?;
            self.cure_effect(mask, 0x01A4);
            return Some(());
        }
        if self.pay(hooks, pick as u32) {
            return Some(());
        }
        let mut ax = 0u16;
        loop {
            let mask = self.cure_handler(ax)?;
            ax = self.cure_effect(mask, 0x01A4).wrapping_add(1);
            if ax == 4 {
                return Some(());
            }
        }
    }

    /// The cure handler `bx` of the table at 7620 (1194:C69F, C6A3,
    /// C6A7: AX 1, 2, 4). None where the entry isn't one of them.
    fn cure_handler(&self, bx: u16) -> Option<u16> {
        match self.mem.r16(DS, CURE_HANDLERS.wrapping_add(bx.wrapping_mul(2))) {
            0xC69F => Some(1),
            0xC6A3 => Some(2),
            0xC6A7 => Some(4),
            _ => None,
        }
    }

    /// 1194:485C: whether the text at `si` is all digits and not empty
    /// (the carry: not).
    pub(crate) fn not_number(&self, si: u16) -> bool {
        let (mut si, mut cx) = (si, 0u16);
        loop {
            let b = self.mem.r8(DS, si);
            if b == 0 {
                return cx == 0;
            }
            if !b.is_ascii_digit() {
                return true;
            }
            si = si.wrapping_add(1);
            cx = cx.wrapping_add(1);
        }
    }

    /// 0000:52BA: the number at `si` (a sign, then digits, in 16 bits).
    pub(crate) fn parse_number(&self, si: u16) -> u16 {
        let (mut si, mut neg) = (si, false);
        match self.mem.r8(DS, si) {
            b'-' => {
                si = si.wrapping_add(1);
                neg = true;
            }
            b'+' => si = si.wrapping_add(1),
            _ => {}
        }
        let mut dx = 0u16;
        loop {
            let b = self.mem.r8(DS, si);
            si = si.wrapping_add(1);
            if !b.is_ascii_digit() {
                break;
            }
            dx = dx.wrapping_mul(10).wrapping_add((b - b'0') as u16);
        }
        if neg { dx.wrapping_neg() } else { dx }
    }

    /// 1194:C58B. Blessing: the least donation (five gold a level plus
    /// one, the holiday's factor 5) at LEAST; a donation typed in until it
    /// is a number (nothing typed, or 0, leaves); less than the least:
    /// only "receive our blessings"; else paid, and the day's first
    /// donation (DONATED_AT 0, or more than a day ago) adds donation /
    /// least, from percent to 256ths, to the blessing. None where that
    /// divides wrongly.
    fn temple_bless(&mut self, hooks: &mut dyn ShopHooks) -> Option<()> {
        let level = self.mem.r8(DS, LEVEL).wrapping_add(1) as u32;
        let mut eax = level * 5;
        if self.mem.r8(DS, HOLIDAY) == 1 {
            eax = self.holiday_price(eax, 5);
        }
        self.mem.w16(DS, LEAST, eax as u16);
        let ax = loop {
            for i in 0..5 {
                self.mem.w8(DS, TYPED + i, 0);
            }
            self.mem.w8(DS, LINE_MARK, 3);
            self.split_lines(0x75F7);
            self.mem.w8(DS, DIALOG, 2);
            hooks.dialog_at(self, 2, 0x75F7, TYPED);
            if self.mem.r8(DS, TYPED) == 0 {
                return Some(());
            }
            if !self.not_number(TYPED) {
                break self.parse_number(TYPED);
            }
        };
        if ax == 0 {
            return Some(());
        }
        if ax >= self.mem.r16(DS, LEAST) {
            if self.pay(hooks, ax as u32) {
                return Some(());
            }
            self.mem.w16(DS, DONATED, ax);
            let bless = if self.mem.r32(DS, DONATED_AT) == 0 {
                let t = self.mem.r32(DS, TIME);
                self.mem.w32(DS, DONATED_AT, t);
                true
            } else if self.mem.r32(DS, TIME).wrapping_sub(self.mem.r32(DS, DONATED_AT)) > 0x5A0 {
                self.mem.w32(DS, DONATED_AT, 0);
                true
            } else {
                // ax <= DONATED (it was just set to ax).
                false
            };
            if bless {
                self.mem.w16(DS, GIVEN, ax);
                let q = ax.checked_div(self.mem.r16(DS, LEAST)).or_else(|| crate::giveup::original("1194:C58B: a least donation of 0: the divide by zero"))?;
                let p = (q as i16 as i32) * 256;
                let r = p % 100;
                let mut v = i16::try_from(p / 100).ok().or_else(|| crate::giveup::original("1194:C58B: the blessing's divide by 100 overflows"))? as u16;
                if r.unsigned_abs() >= 0x32 {
                    v = if (q as i16) < 0 { v.wrapping_sub(1) } else { v.wrapping_add(1) };
                }
                let b = self.mem.r16(DS, BLESSING).wrapping_add(v);
                self.mem.w16(DS, BLESSING, b);
            }
        }
        hooks.message(self, 0x75A3);
        Some(())
    }

    /// 1194:C258. The temple (SHOP 5): on entering, "%met" made (the
    /// greeting's substitution), the quests' checks, and the holiday's
    /// discount (HOLIDAY 1, else FFh); then its menu until it is left (3
    /// or none), its pick's service run each time. On leaving SHOP FFh,
    /// D39 bit 80h set and HOLIDAY FFh. None where a service is.
    pub fn temple(&mut self, files: &mut crate::bsa::Files, hooks: &mut dyn ShopHooks) -> Option<()> {
        // Where the port gives up (a hook with no answer, a routine not
        // ported), the end's state is left all the same: no shop (SHOP
        // FFh), no holiday, the clip as it was. The original has no such
        // exit.
        let a91a = self.mem.r16(DS, 0xA91A);
        let r = self.temple_body(files, hooks);
        if r.is_none() {
            self.mem.w8(DS, SHOP, 0xFF);
            self.mem.w16(DS, 0xA91A, a91a);
            self.mem.w8(DS, HOLIDAY, 0xFF);
        }
        r
    }

    fn temple_body(&mut self, files: &mut crate::bsa::Files, hooks: &mut dyn ShopHooks) -> Option<()> {
        let a91a = self.mem.r16(DS, 0xA91A);
        self.mem.w8(DS, SHOP, 5);
        hooks.screen_saved(self, files);
        // 1194:C65F.
        self.mem.w32(DS, 0x929E, 0x6D65_7425);
        self.mem.w16(DS, 0x92A2, 0);
        hooks.expand(self, 0x929E);
        hooks.banner(self, 0x929E);
        hooks.quest_check(self, files)?;
        hooks.quests(self, files)?;
        let h = if self.shop_holiday() { 1 } else { 0xFF };
        self.mem.w8(DS, HOLIDAY, h);
        loop {
            // 1194:C27D: FONT_A and the tiles at 1036 selected.
            self.select_font(font::A);
            self.select_tiles(0x1036);
            let pick = hooks.menu(self, 0x7554, 0x7565, 0x756A);
            if (pick as i16) < 0 || pick == 3 {
                break;
            }
            match self.mem.r16(DS, 0x759D + pick.wrapping_mul(2)) {
                0xC58B => self.temple_bless(hooks)?,
                0xC36E => self.temple_cure(hooks)?,
                0xC2C7 => self.temple_heal(hooks),
                _ => return crate::giveup::not_ported("1194:C65F: a temple choice not in the table"),
            }
        }
        self.mem.w8(DS, SHOP, 0xFF);
        self.mem.w16(DS, 0xA91A, a91a);
        hooks.screen_back(self, files);
        let f = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, f);
        self.mem.w8(DS, HOLIDAY, 0xFF);
        Some(())
    }
}
