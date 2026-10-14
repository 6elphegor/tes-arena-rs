//! The tavern (1194:B93C, `tavern_shop`), the shop of building kind 4.
//!
//! Unless a fight is on (FFB) or the guards are coming (680F), the
//! tavern's state is set up (1194:C012: no room yet, the shop's level and
//! price factors, 1194:4AA7 and 4AF8, today a holiday for taverns or not,
//! 13C9, 1194:4A4A; the room's file read, `TavernHooks::room_load`), then
//! a menu runs until "Exit" (or nothing picked): with a room rented or on
//! file (12BE, 12BC) "Buy Drinks, Rumors, Exit", otherwise "Buy Drinks,
//! Get a Room, Sneak into a Room, Rumors, Exit". A failed sneak that
//! calls the guards ends it at once. On leaving the room's file is
//! written (1194:BA91).
//!
//! - Drinks (1194:BB83): ten of the 16 drinks, picked from the place's
//!   seed (its record's first dword in the places' list at 1AB5,
//!   1194:3EB1; the game's seed is put back after, 1194:BEED), each with
//!   its price (21E8; a holiday's price, 1194:4A6B with service 2). A
//!   drink bought costs its price; the drinks' timer (1378) is set to 60
//!   and their count (1379) goes up. More than the player's +2Eh byte
//!   over 25 (+4 set) or 12 makes the player drunk (1194:BFB1): the
//!   effect 20h for 60 (or 120) ticks, its attribute deltas the price of
//!   the drink at the list's position the quotient gives, times the
//!   table at 7349 (then negated for its end), and the attributes made
//!   again (1194:3621).
//! - A room (1194:BD51): one of five kinds, for the days typed (digits
//!   only, 1194:485C, asked again otherwise; 0 or nothing cancels), its
//!   price (the low byte of 28C3's word times the days, in 8 bits, as
//!   the original's `mul bl`), free on a holiday whose factor makes it 0,
//!   else haggled over (1194:C14E, `TavernHooks::haggle`); paid, the
//!   room is the player's (12BE) for the days (hours 1019 = days x 24,
//!   the end 12CA = days x 1440 + the time).
//! - Sneaking into a room (1194:BC99): asked, then the lockpicking roll
//!   (1194:53F9) against the lock tried (13CC). Done: a room of a random
//!   kind for a day (1194:BCEA; its hours, 1019, the low word of the end
//!   time, the original's bug). Failed: "You are unsuccessful...", and
//!   the guards' roll (0000:3F6E: d100 under 100 less the chance) may
//!   call them (0000:3EBE), the building broken into (191D) and its
//!   people gone (0000:50B9).
//! - Rumors: 1194:E3F7 (`TavernHooks::rumours`).
//!
//! The screen's routines are the front end's (nothing in the port), as
//! are the menus, lists, dialogs and messages (`ShopHooks`) and the
//! greeting, the quests' checks, the room's file, the haggle, the sneak's
//! picture and the rumours (`TavernHooks`). Assumed: the message box
//! returns the carry clear and keeps BX (the chance the guards' roll
//! reads).

use crate::bsa::Files;
use crate::effects::Applied;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::DS;
use crate::random::CycleCheck;
use crate::shops::ShopHooks;

/// What the tavern leaves to the front end, besides `ShopHooks`.
pub trait TavernHooks: ShopHooks {
    /// 1194:C0B1: the rooms' files (old ones deleted).
    fn files(&mut self, _m: &mut Machine) {}
    /// 1194:C12A: the innkeeper's greeting (2DA2's %nt).
    fn greeting(&mut self, _m: &mut Machine) {}
    /// 1194:CED7: the main quest's informant (`Machine::quest_informant`).
    fn quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_informant(files, &mut crate::quest::NoQuestHooks).map(|_| ())
    }
    /// 1194:8F1E: the quests the talk ends or offers (`Machine::quests_talk`).
    fn quests_talk(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_talk(files, &mut crate::quest::NoQuestHooks)
    }
    /// 1194:90B9: another queued quest's text taken (`Machine::quests_take`).
    fn tavern_quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_take(files, &mut crate::quest::NoQuestHooks)
    }
    /// 1194:C04C: the room's file read: 12BC set (1) with ROOM_HOURS
    /// (1019) and ROOM_END (12CA) when there is one, else 12BC 0 and the
    /// carry (returned).
    fn room_load(&mut self, m: &mut Machine) -> bool {
        m.mem.w8(DS, 0x12BC, 0);
        true
    }
    /// 1194:BA91: the room's file written (deleted once no hours are
    /// left).
    fn room_file(&mut self, _m: &mut Machine) {}
    /// 1194:59CC: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:4B8E: the picture named at DS:`si` shown.
    fn picture(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:C14E: the room's price (1391) haggled over with the
    /// innkeeper (1194:8020): the price agreed, or the carry when the
    /// player walks away.
    fn haggle(&mut self, m: &mut Machine) -> (u32, bool) {
        (m.mem.r32(DS, PRICE), true)
    }
    /// 1194:E3F7: the rumours.
    fn rumours(&mut self, _m: &mut Machine) {}
}

/// `TavernHooks` that do nothing: every menu left at once.
pub struct NoTavernHooks;

impl ShopHooks for NoTavernHooks {}
impl TavernHooks for NoTavernHooks {}

/// DGROUP variables.
pub mod var {
    /// The menus' title, keys and entries (with a room; without).
    pub const MENU_TITLE: u16 = 0x723C;
    pub const KEYS_ROOM: u16 = 0x72B8;
    pub const ENTRIES_ROOM: u16 = 0x72BC;
    pub const ROUTINES_ROOM: u16 = 0x72B4;
    pub const KEYS: u16 = 0x724D;
    pub const ENTRIES: u16 = 0x7253;
    pub const ROUTINES: u16 = 0x72AC;
    /// Today is the taverns' holiday.
    pub const HOLIDAY: u16 = 0x13C9;
    /// The pick.
    pub const PICK: u16 = 0x13C8;
    /// A room: on file, rented now, its days, its hours and end.
    pub const ROOM_FILED: u16 = 0x12BC;
    pub const ROOM_DAYS: u16 = 0x12BD;
    pub const ROOM_RENTED: u16 = 0x12BE;
    pub const ROOM_HOURS: u16 = 0x1019;
    pub const ROOM_END: u16 = 0x12CA;
    /// The days typed.
    pub const TYPED: u16 = 0x12C6;
    /// The drinks: the list's text, the drinks listed, those taken, the
    /// timer and the count.
    pub const DRINKS_TEXT: u16 = 0x1044;
    pub const DRINKS: u16 = 0x732F;
    pub const TAKEN: u16 = 0x9302;
    pub const TIMER: u16 = 0x1378;
    pub const COUNT: u16 = 0x1379;
}
use var::*;

const PRICE: u16 = 0x1391;
const GOLD: u16 = 0x05C2;
const TIME: u16 = 0x05CC;
const COLOUR: u16 = 0x9288;
const DRINK_NAMES: u16 = 0x2112;
const DRINK_PRICES: u16 = 0x21E8;
const ROOM_NAMES: u16 = 0x2892;
const ROOM_PRICES: u16 = 0x28C3;
const NAME: u16 = 0x110C;
const DRUNK_FACTORS: u16 = 0x7349;
const PLAYER: u16 = 0x01A4;

/// Texts.
mod text {
    pub const NOT_ENOUGH: u16 = 0x198C;
    pub const DRINK: u16 = 0x74AF;
    pub const DRINK_LINE: u16 = 0x7411;
    pub const ROOMS: u16 = 0x7351;
    pub const HOW_MANY_DAYS: u16 = 0x739E;
    pub const FREE_ROOM: u16 = 0x74FB;
    pub const SNEAK_PICTURE: u16 = 0x132F;
    pub const SNEAK: u16 = 0x7379;
    pub const SNEAK_FAILED: u16 = 0x7361;
    pub const SNEAK_ROOM: u16 = 0x73C6;
}

impl Machine {
    /// 1194:B93C. See the module's notes. None where the original faults
    /// or hangs.
    pub fn tavern_shop(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<()> {
        let saved = self.mem.r16(DS, 0xA91A);
        self.mem.w8(DS, 0x137F, 4);
        if self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x680F) == 0 {
            hooks.files(self);
            self.tavern_enter(files, hooks)?;
            loop {
                if self.mem.r8(DS, ROOM_RENTED) != 0 || self.mem.r8(DS, ROOM_FILED) != 0 {
                    let pick = hooks.menu(self, MENU_TITLE, KEYS_ROOM, ENTRIES_ROOM);
                    if (pick as i16) < 0 || pick == 2 {
                        break;
                    }
                    self.tavern_choice(hooks, ROUTINES_ROOM, pick)?;
                } else {
                    let pick = hooks.menu(self, MENU_TITLE, KEYS, ENTRIES);
                    if (pick as i16) < 0 || pick == 4 {
                        break;
                    }
                    if self.tavern_choice(hooks, ROUTINES, pick)? {
                        break;
                    }
                }
            }
        }
        hooks.room_file(self);
        self.mem.w8(DS, 0x137F, 0xFF);
        self.mem.w16(DS, 0xA91A, saved);
        Some(())
    }

    /// 1194:C012: the state set up on entering.
    fn tavern_enter(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<()> {
        self.mem.w32(DS, ROOM_END, 0);
        self.mem.w16(DS, ROOM_HOURS, 0);
        self.mem.w8(DS, ROOM_RENTED, 0);
        self.mem.w8(DS, 0x137F, 4);
        hooks.greeting(self);
        hooks.quest(self, files)?;
        hooks.quests_talk(self, files)?;
        hooks.tavern_quest(self, files)?;
        let level = self.shop_roll_level()?;
        self.shop_price_factors(level)?;
        self.mem.w8(DS, HOLIDAY, 1);
        if !self.shop_holiday() {
            self.mem.w8(DS, HOLIDAY, 0);
        }
        hooks.room_load(self);
        Some(())
    }

    /// The menu's routine for `pick`, by its address in the table at
    /// `table`. The carry it returns (only the sneak's failure sets it).
    fn tavern_choice(&mut self, hooks: &mut dyn TavernHooks, table: u16, pick: u16) -> Option<bool> {
        match self.mem.r16(DS, table.wrapping_add(pick.wrapping_add(pick))) {
            0xBB83 => self.tavern_drinks(hooks).map(|_| false),
            0xBC8B => {
                self.tavern_room(hooks)?;
                hooks.room_file(self);
                Some(false)
            }
            0xBC99 => self.tavern_sneak(hooks),
            0xBD4C => {
                hooks.rumours(self);
                Some(false)
            }
            _ => None,
        }
    }

    /// The price of the drink at `i` in the drinks listed (a holiday's,
    /// service 2).
    fn drink_price(&self, kind: u8) -> u32 {
        let eax = self.mem.r8(DS, DRINK_PRICES.wrapping_add(kind as u16)) as u32;
        if self.mem.r8(DS, HOLIDAY) == 1 { self.holiday_price(eax, 2) } else { eax }
    }

    /// 1194:BB83. A drink bought; see the module's notes.
    fn tavern_drinks(&mut self, hooks: &mut dyn TavernHooks) -> Option<()> {
        self.drinks_list()?;
        let pick = hooks.list(self, DRINKS_TEXT + 1, 10, 0);
        self.mem.w8(DS, PICK, pick as u8);
        if (pick as i16) < 0 {
            return Some(());
        }
        let kind = self.mem.r8(DS, DRINKS.wrapping_add(pick));
        let eax = self.drink_price(kind);
        self.mem.w32(DS, PRICE, eax);
        let gold = self.mem.r32(DS, GOLD);
        if gold < eax {
            hooks.message(self, text::NOT_ENOUGH);
            return Some(());
        }
        self.mem.w32(DS, GOLD, gold.wrapping_sub(eax));
        let kind = self.mem.r8(DS, DRINKS.wrapping_add(self.mem.r8(DS, PICK) as u16));
        let name = self.skip_strings(DS, DRINK_NAMES, kind as u16);
        self.sprintf((DS, DRINKS_TEXT), (DS, text::DRINK), &[Arg::Str(DS, name)]);
        self.mem.w8(DS, COLOUR, 0x60);
        hooks.message_box(self, DRINKS_TEXT);
        self.mem.w8(DS, TIMER, 0x3C);
        let n = self.mem.r8(DS, COUNT).wrapping_add(1);
        self.mem.w8(DS, COUNT, n);
        let bx = if self.mem.r8(DS, PLAYER + 4) != 0 { 0x19 } else { 0x0C };
        let ax = self.mem.r8(DS, PLAYER + 0x2E) as u16 / bx;
        if ax as u8 >= n {
            return Some(());
        }
        let kind = self.mem.r8(DS, DRINKS.wrapping_add(ax));
        let ax = self.mem.r8(DS, DRINK_PRICES.wrapping_add(kind as u16)) as u16;
        self.drunk(ax)?;
        self.attribute_modifiers(PLAYER);
        Some(())
    }

    /// 1194:BEED. The drinks listed: ten different kinds (of 16; 0 is
    /// never counted as taken, 1194:BFFF) rolled from the place's seed,
    /// each a line "%u gp\n%s" from 1045 on.
    fn drinks_list(&mut self) -> Option<()> {
        for i in 0..10u16 {
            self.mem.w8(DS, TAKEN + i, 0);
        }
        for i in 0..0x64u16 {
            self.mem.w16(DS, DRINKS_TEXT + 2 * i, 0);
        }
        let ax = self.mem.r16(DS, 0x0FD2);
        let key = (ax & 0xFF00) | ((ax as u8) >> 1) as u16;
        let (seg, si) = (self.mem.r16(DS, 0x1AB7), self.mem.r16(DS, 0x1AB5));
        let si = self.place_lookup(seg, si, key, self.mem.r16(DS, 0x1387));
        let saved = self.rng.seed;
        self.rng.seed = self.mem.r32(seg, si);
        self.mem.w16(DS, 0x12C0, DRINKS_TEXT);
        self.mem.w16(DS, 0x12C2, DRINKS);
        for _ in 0..10 {
            let mut check = CycleCheck::new(&self.rng);
            let kind = loop {
                let k = self.rng.range(0, 0x10)? as u8;
                if !self.drink_taken(k) {
                    break k;
                }
                if check.cycled(&self.rng) {
                    return None;
                }
            };
            let di = self.mem.r16(DS, 0x12C2);
            self.mem.w8(DS, di, kind);
            let name = self.skip_strings(DS, DRINK_NAMES, kind as u16);
            self.tavern_strcpy(name, NAME);
            let eax = self.drink_price(kind);
            self.mem.w32(DS, PRICE, eax);
            let mut si = self.mem.r16(DS, 0x12C0);
            while self.mem.r8(DS, si) != 0 {
                si = si.wrapping_add(1);
            }
            si = si.wrapping_add(1);
            self.mem.w16(DS, 0x12C0, si);
            self.sprintf((DS, si), (DS, text::DRINK_LINE), &[Arg::Int(eax as u16), Arg::Str(DS, NAME)]);
            self.mem.w16(DS, 0x12C2, di.wrapping_add(1));
        }
        self.rng.seed = saved;
        Some(())
    }

    /// 1194:BFFF. Whether drink `al` is among those taken (TAKEN, to its
    /// first 0); a new one is added there.
    fn drink_taken(&mut self, al: u8) -> bool {
        let mut si = TAKEN;
        loop {
            let b = self.mem.r8(DS, si);
            if b == 0 {
                self.mem.w8(DS, si, al);
                return false;
            }
            if b == al {
                return true;
            }
            si = si.wrapping_add(1);
        }
    }

    /// 1194:3EB1. The entry (4 bytes) whose word is `key` among `cx` at
    /// seg:`si`: its offset, 0 when none (`si` itself when `cx` is 0).
    pub fn place_lookup(&self, seg: u16, mut si: u16, key: u16, cx: u16) -> u16 {
        if cx == 0 {
            return si;
        }
        for _ in 0..cx {
            if self.mem.r16(seg, si) == key {
                return si;
            }
            si = si.wrapping_add(4);
        }
        0
    }

    /// 0000:9CA0: the string at DS:`si` copied to DS:`di` with its NUL.
    fn tavern_strcpy(&mut self, mut si: u16, mut di: u16) {
        loop {
            let b = self.mem.r8(DS, si);
            self.mem.w8(DS, di, b);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if b == 0 {
                break;
            }
        }
    }

    /// 1194:BFB1. The player drunk: the effect 20h (period, pulses and
    /// phase B's period 60, or 120 with the player's +4 clear; phase B
    /// endless), its attribute deltas `ax` times the factors at 7349, then
    /// the same negated for its end.
    fn drunk(&mut self, ax: u16) -> Option<()> {
        let cx = if self.mem.r8(DS, PLAYER + 4) != 0 { 0x3C } else { 0x78 };
        let Applied::Slot(Some(s)) = self.apply_effect(DS, PLAYER, 0x20, cx, cx, cx, 0xFFFF) else {
            return None;
        };
        let (mut ax, mut si) = (ax, s);
        for _ in 0..2 {
            for i in 0..8u16 {
                let f = self.mem.r8(DS, DRUNK_FACTORS + i) as u16;
                self.mem.w8(DS, si.wrapping_add(6 + i), ax.wrapping_mul(f) as u8);
            }
            ax = ax.wrapping_neg();
            si = si.wrapping_add(0x14);
        }
        Some(())
    }

    /// 1194:BD51. A room rented; see the module's notes. The room's file
    /// written (1194:BA91) when it is.
    fn tavern_room(&mut self, hooks: &mut dyn TavernHooks) -> Option<()> {
        self.mem.w32(DS, TYPED, 0);
        let pick = hooks.list(self, ROOM_NAMES, 5, text::ROOMS);
        self.mem.w8(DS, PICK, pick as u8);
        if (pick as i16) < 0 {
            return Some(());
        }
        let holiday = self.mem.r8(DS, HOLIDAY) == 1;
        let mut free = holiday && self.holiday_price(0x64, 3) == 0;
        let mut eax = 0;
        if free {
            self.mem.w8(DS, ROOM_DAYS, 1);
            self.mem.w16(DS, ROOM_HOURS, 0x18);
        } else {
            loop {
                self.mem.w32(DS, TYPED, 0);
                self.mem.w8(DS, 0x137E, 3);
                self.split_lines(text::HOW_MANY_DAYS);
                self.mem.w8(DS, 0x13D3, 2);
                hooks.dialog(self, 2, TYPED);
                if self.rest_digits(TYPED) {
                    break;
                }
            }
            if self.mem.r8(DS, TYPED) == 0 {
                return self.room_cancelled();
            }
            let ax = self.info_atoi(DS, TYPED);
            if ax == 0 {
                return self.room_cancelled();
            }
            let days = ax as u8;
            self.mem.w8(DS, ROOM_DAYS, days);
            self.mem.w16(DS, ROOM_HOURS, days as u16 * 0x18);
            let price = self.mem.r16(DS, ROOM_PRICES.wrapping_add(2 * (pick as u8 as u16)));
            eax = (price as u8 as u16 * days as u16) as u32;
            if holiday {
                eax = self.holiday_price(eax, 3);
                free = eax == 0;
            }
        }
        if free {
            self.mem.w8(DS, COLOUR, 0x60);
            hooks.message_box(self, text::FREE_ROOM);
        } else {
            self.mem.w32(DS, PRICE, eax);
            let (agreed, walked) = hooks.haggle(self);
            if walked {
                return self.room_cancelled();
            }
            eax = agreed;
            if self.mem.r32(DS, GOLD) < eax {
                hooks.message(self, text::NOT_ENOUGH);
                return self.room_cancelled();
            }
        }
        let gold = self.mem.r32(DS, GOLD).wrapping_sub(eax);
        self.mem.w32(DS, GOLD, gold);
        self.mem.w8(DS, ROOM_RENTED, 1);
        let end = (self.mem.r8(DS, ROOM_DAYS) as u32 * 0x5A0).wrapping_add(self.mem.r32(DS, TIME));
        self.mem.w32(DS, ROOM_END, end);
        hooks.room_file(self);
        Some(())
    }

    /// 1194:BE78: no room (its hours and the file's mark cleared).
    fn room_cancelled(&mut self) -> Option<()> {
        self.mem.w16(DS, ROOM_HOURS, 0);
        self.mem.w8(DS, ROOM_FILED, 0);
        Some(())
    }

    /// 1194:BC99. Sneaking into a room; see the module's notes. The carry
    /// (the tavern left): the guards called.
    fn tavern_sneak(&mut self, hooks: &mut dyn TavernHooks) -> Option<bool> {
        hooks.picture(self, text::SNEAK_PICTURE);
        self.mem.w8(DS, 0x137E, 3);
        self.split_lines(text::SNEAK);
        self.mem.w8(DS, 0x13D3, 1);
        if hooks.dialog(self, 1, 0) != 0 {
            return Some(false);
        }
        let (ok, chance) = self.lockpick()?;
        if ok {
            self.sneak_room(hooks)?;
            return Some(false);
        }
        self.mem.w8(DS, COLOUR, 0x60);
        hooks.message_box(self, text::SNEAK_FAILED);
        let cx = 100u16.wrapping_sub(chance);
        if self.rng.next() % 100 >= cx {
            return Some(false);
        }
        self.call_guards();
        self.mem.w8(DS, 0x191D, 1);
        self.clear_dungeon_flats();
        Some(true)
    }

    /// 1194:BCEA. A room of a random kind (0..4) for a day; its hours
    /// are the end time's low word (the original's bug).
    fn sneak_room(&mut self, hooks: &mut dyn TavernHooks) -> Option<()> {
        let kind = self.rng.range(0, 5)? as u8;
        self.mem.w8(DS, PICK, kind);
        self.mem.w8(DS, ROOM_RENTED, 1);
        self.mem.w8(DS, ROOM_DAYS, 1);
        let ax = (self.mem.r8(DS, ROOM_DAYS) as u32 * 0x5A0) & 0xFFFF;
        let end = ax.wrapping_add(self.mem.r32(DS, TIME));
        self.mem.w32(DS, ROOM_END, end);
        self.mem.w16(DS, ROOM_HOURS, end as u16);
        hooks.room_file(self);
        let name = self.skip_strings(DS, ROOM_NAMES, self.mem.r8(DS, PICK) as u16);
        self.sprintf((DS, NAME), (DS, text::SNEAK_ROOM), &[Arg::Str(DS, name)]);
        self.mem.w8(DS, COLOUR, 0x60);
        hooks.message_box(self, NAME);
        Some(())
    }
}
