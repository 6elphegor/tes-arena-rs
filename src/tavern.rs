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
use crate::shops::font;
use crate::mem::DS;
use crate::random::CycleCheck;
use crate::shops::ShopHooks;

/// What the tavern leaves to the front end, besides `ShopHooks`.
pub trait TavernHooks: ShopHooks {
    /// 1194:C0B1: the rooms' files whose time is up deleted
    /// (`Machine::room_files_expire`; by default left, as the vectors
    /// stub it).
    fn files(&mut self, _m: &mut Machine, _files: &mut Files) {}
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
    fn room_load(&mut self, m: &mut Machine, files: &mut Files) -> bool {
        m.room_read(files)
    }
    /// 1194:BA91: the room's file written (deleted once no hours are
    /// left; `Machine::room_write`).
    fn room_file(&mut self, m: &mut Machine, files: &mut Files) {
        m.room_write(files)
    }
    /// 1194:59CC: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:C14E: the room's price (1391) haggled over with the
    /// innkeeper (`Machine::tavern_room_haggle`, 1194:8020 in mode 3):
    /// the price agreed, or the carry when the player walks away. By
    /// default the player walks away (`haggle::Leave`); a front end
    /// answers for them. None where the original faults or hangs.
    fn haggle(&mut self, m: &mut Machine, files: &mut Files) -> Option<(u32, bool)> {
        m.tavern_room_haggle(files, &mut crate::haggle::Leave)
    }
    /// 1194:E3F7: the rumours told by the front end instead: whether it
    /// did. By default the port (`Machine::tavern_rumours`) runs, its
    /// menu and boxes through these hooks.
    fn rumours(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<bool> {
        Some(false)
    }
    /// 1194:A374 (mode `mode`, into 1044) done by the front end instead:
    /// whether it was. By default the text port runs.
    fn text(&mut self, _m: &mut Machine, _files: &mut Files, _mode: u16) -> Option<bool> {
        Some(false)
    }
}

/// The text's own boxes and menus through the tavern's hooks.
struct TavernText<'a>(&'a mut dyn TavernHooks);

impl crate::text::TextHooks for TavernText<'_> {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.0.message_box(m, si);
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.0.menu(m, si, bx, di)
    }
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
    /// The room's file name (1194:BA44) and the extension it takes (".64"
    /// and its NUL, as the store's).
    pub const ROOM_NAME: u16 = 0x733A;
    pub const ROOM_EXTENSION: u16 = 0x8B05;
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
/// The innkeeper's name (1194:C14E).
const INNKEEPER: u16 = 0x7311;
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
        // Where the port gives up, the end's state is left all the same:
        // no tavern (137F FFh), the clip as it was. The original has no
        // such exit.
        let saved = self.mem.r16(DS, 0xA91A);
        let r = self.tavern_shop_body(files, hooks);
        if r.is_none() {
            self.mem.w8(DS, 0x137F, 0xFF);
            self.mem.w16(DS, 0xA91A, saved);
        }
        r
    }

    fn tavern_shop_body(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<()> {
        let saved = self.mem.r16(DS, 0xA91A);
        self.mem.w8(DS, 0x137F, 4);
        if self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x680F) == 0 {
            hooks.files(self, files);
            self.tavern_enter(files, hooks)?;
            loop {
                // 1194:B975: FONT_A and the tiles at 1036 selected.
                self.select_font(font::A);
                self.select_tiles(0x1036);
                if self.mem.r8(DS, ROOM_RENTED) != 0 || self.mem.r8(DS, ROOM_FILED) != 0 {
                    let pick = hooks.menu(self, MENU_TITLE, KEYS_ROOM, ENTRIES_ROOM);
                    if (pick as i16) < 0 || pick == 2 {
                        break;
                    }
                    self.tavern_choice(files, hooks, ROUTINES_ROOM, pick)?;
                } else {
                    let pick = hooks.menu(self, MENU_TITLE, KEYS, ENTRIES);
                    if (pick as i16) < 0 || pick == 4 {
                        break;
                    }
                    if self.tavern_choice(files, hooks, ROUTINES, pick)? {
                        break;
                    }
                }
            }
        }
        hooks.room_file(self, files);
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
        hooks.room_load(self, files);
        Some(())
    }

    /// The menu's routine for `pick`, by its address in the table at
    /// `table`. The carry it returns (only the sneak's failure sets it).
    fn tavern_choice(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks, table: u16, pick: u16) -> Option<bool> {
        match self.mem.r16(DS, table.wrapping_add(pick.wrapping_add(pick))) {
            0xBB83 => self.tavern_drinks(hooks).map(|_| false),
            0xBC8B => {
                // 1194:BC8E: the gold left.
                hooks.gold_left(self);
                self.tavern_room(files, hooks)?;
                hooks.room_file(self, files);
                Some(false)
            }
            0xBC99 => self.tavern_sneak(files, hooks),
            0xBD4C => {
                if !hooks.rumours(self, files)? {
                    self.tavern_rumours(files, hooks)?;
                }
                Some(false)
            }
            _ => crate::giveup::not_ported("a tavern choice not in the table"),
        }
    }

    /// 1194:E3F7. The rumours: the menu "Rumor Type" (general, work; the
    /// seed taken from 151 first). General: by the parity of the clock's
    /// low word plus the player's x (C3D0), over 8 (the low byte's bits
    /// even), the quest being told (F3E +4 bits 6-7) says where it stands
    /// (A1h + its kind, +4 bits 9-11) and the record is cleared (34h
    /// bytes), or nothing is heard (B9h); odd, a plain "nothing" (9Fh,
    /// colour C0h first). Work: the first of the four quests from E6E in
    /// state 2 (+4 bits 6-7) is told of (C5h + its kind), else C4h. Each
    /// text through 1194:E4E3 (`tavern_say`). None where the text fails.
    pub fn tavern_rumours(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<()> {
        self.mem.w32(DS, 0x91F6, self.mem.r16(DS, 0x0151) as u32);
        self.load_rng();
        let ax = hooks.menu(self, 0x857B, 0x8589, 0x858C);
        let clock = self.mem.r32(DS, 0x05CC);
        let bx = ((clock as u16).wrapping_add(self.mem.r16(DS, 0xC3D0)) >> 3) as u8;
        if (ax as i16) < 0 {
            return Some(());
        }
        if ax == 0 {
            if bx.count_ones() % 2 != 0 {
                self.mem.w8(DS, 0x9288, 0xC0);
                return self.tavern_say(files, hooks, 0x9F);
            }
            let q = self.mem.r16(DS, 0x0F3E + 4);
            if (q & 0xC0) >> 6 == 0 {
                return self.tavern_say(files, hooks, 0xB9);
            }
            self.mem.w8(DS, 0x9288, 0x60);
            self.tavern_say(files, hooks, ((q & 0xE00) >> 9).wrapping_add(0xA1))?;
            for i in 0..0x34u16 {
                self.mem.w8(DS, 0x0F3E + i, 0);
            }
            return Some(());
        }
        let mut si = 0x0E6Eu16;
        for _ in 0..4 {
            let w = self.mem.r16(DS, si.wrapping_add(4));
            if (w & 0xC0) >> 6 == 2 {
                return self.tavern_say(files, hooks, ((w & 0xE00) >> 9).wrapping_add(0xC5));
            }
            si = si.wrapping_add(0x34);
        }
        self.tavern_say(files, hooks, 0xC4)
    }

    /// 1194:E4E3. Text `ax` said in a box: the font (B038) from 1036, the
    /// text (1194:A374, mode 2, no wrap width, substituted) into 1044,
    /// then the box in colour 60h.
    fn tavern_say(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks, ax: u16) -> Option<()> {
        self.mem.w16(DS, 0x137A, ax);
        let f = self.mem.r16(DS, 0x1036);
        self.mem.w16(DS, 0xB038, f);
        self.mem.w8(DS, 0x137E, 0);
        self.mem.w8(DS, 0x101C, 0);
        if !hooks.text(self, files, 2)? {
            self.text(files, &mut TavernText(hooks), 2, DS, 0x1044)?;
        }
        self.mem.w8(DS, 0x9288, 0x60);
        hooks.message_box(self, 0x1044);
        Some(())
    }

    /// The price of the drink at `i` in the drinks listed (a holiday's,
    /// service 2).
    fn drink_price(&self, kind: u8) -> u32 {
        let eax = self.mem.r8(DS, DRINK_PRICES.wrapping_add(kind as u16)) as u32;
        if self.mem.r8(DS, HOLIDAY) == 1 { self.holiday_price(eax, 2) } else { eax }
    }

    /// 1194:BB83. A drink bought; see the module's notes.
    fn tavern_drinks(&mut self, hooks: &mut dyn TavernHooks) -> Option<()> {
        // 1194:BB86: the gold left; ARENAFNT selected (BB89).
        hooks.gold_left(self);
        self.select_font(font::ARENA);
        self.drinks_list()?;
        // AX Bh (1194:BB9E): the list's kind (1194:5734).
        self.list_kind = 0xB;
        let pick = hooks.list(self, DRINKS_TEXT + 1, 10, 0);
        // 1194:BBA5
        hooks.strip_back(self);
        self.mem.w8(DS, PICK, pick as u8);
        if (pick as i16) < 0 {
            return Some(());
        }
        let kind = self.mem.r8(DS, DRINKS.wrapping_add(pick));
        let eax = self.drink_price(kind);
        self.mem.w32(DS, PRICE, eax);
        let gold = self.mem.r32(DS, GOLD);
        if gold < eax {
            // 1194:BC19: the tiles at 1034.
            self.select_tiles(0x1034);
            hooks.message(self, text::NOT_ENOUGH);
            return Some(());
        }
        self.mem.w32(DS, GOLD, gold.wrapping_sub(eax));
        let kind = self.mem.r8(DS, DRINKS.wrapping_add(self.mem.r8(DS, PICK) as u16));
        let name = self.skip_strings(DS, DRINK_NAMES, kind as u16);
        self.sprintf((DS, DRINKS_TEXT), (DS, text::DRINK), &[Arg::Str(DS, name)]);
        // 1194:BC01: ARENAFNT (AX then [1036], with no call).
        self.select_font(font::ARENA);
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
        self.select_font(font::ARENA);
        // Where the port gives up, the game's seed is put back as the
        // end puts it, not left at the one this rolled with. The
        // original has no such exit.
        let saved = self.rng.seed;
        let r = self.drinks_list_seeded();
        self.seed_back(saved, r)
    }

    fn drinks_list_seeded(&mut self) -> Option<()> {
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
                    return crate::giveup::original("1194:BEED: no drink left: the original rolls for ever");
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

    /// 1194:BA44. The rented room's file name at ROOM_NAME: the digits of
    /// the tavern's seed (1194:FBA2: its entry among the town's taverns,
    /// 1AB5, for FD2's low byte halved, into 12E0) with the extension
    /// (8B05), or with 5C8 bit 4000h the digits of the wild square's seed
    /// (2DA2:092B) cut at eight bytes before the extension; its first two
    /// bytes "in". Returns its path (25FD:0844).
    pub fn room_file_name(&mut self) -> u16 {
        if self.mem.r16(DS, 0x05C8) & 0x4000 == 0 {
            let key = self.mem.r16(DS, 0x0FD2);
            let key = (key & 0xFF00) | ((key & 0xFF) >> 1);
            let (off, seg) = (self.mem.r16(DS, 0x1AB5), self.mem.r16(DS, 0x1AB7));
            let si = self.place_lookup(seg, off, key, self.mem.r16(DS, 0x1387));
            let seed = self.mem.r16(seg, si) as u32;
            self.mem.w32(DS, 0x12E0, seed);
            self.dun_ultoa(seed, 0x10A8);
            let di = self.room_name_end(0x10A8, 0x64);
            for i in 0..5 {
                let c = self.mem.r8(DS, ROOM_EXTENSION.wrapping_add(i));
                self.mem.w8(DS, di.wrapping_add(i), c);
            }
            self.tavern_strcpy(0x10A8, ROOM_NAME);
        } else {
            let (cx, dx) = (self.mem.r16(DS, 0x0610), self.mem.r16(DS, 0x0612));
            let hi = self.mem.r16(DS, 0x060A).wrapping_add((dx >= 0x2000) as u16);
            let lo = self.mem.r16(DS, 0x0608).wrapping_add((cx >= 0x2000) as u16);
            self.dun_ultoa((hi as u32) << 16 | lo as u32, ROOM_NAME);
            let di = self.room_name_end(ROOM_NAME, 8);
            self.tavern_strcpy(ROOM_EXTENSION, di);
        }
        self.mem.w16(DS, ROOM_NAME, 0x6E69);
        self.save_path(ROOM_NAME)
    }

    /// 1194:C0B1. The slot's rented rooms' files ("in*.%d", 754C, by the
    /// slot 1A6C, made at ROOM_NAME and its path, 25FD:0844) found one by
    /// one (DOS find first and next, 0000:600C, 6028): each loaded whole
    /// to 929E (0000:7428, its size at A90C), and deleted (0000:9DC2)
    /// unless the end it holds (the dword at +2, 92A0) is past the time
    /// (5CC, signed). Here the files are found through `Files` in name
    /// order, and each one's path made from ROOM_NAME as it stands (as
    /// `delete_matching` does): the path buffers' toggle ends as the
    /// original's, not their contents. Without it nothing in play
    /// differs (`room_read` takes a room whose time is up as none), but
    /// the files would stay in the save's folder.
    pub fn room_files_expire(&mut self, files: &mut Files) {
        let slot = self.mem.r8(DS, 0x1A6C) as u16;
        self.sprintf((DS, ROOM_NAME), (DS, 0x754C), &[Arg::Int(slot)]);
        self.save_path(ROOM_NAME);
        let pattern = self.mem.cstr(DS, ROOM_NAME).to_ascii_uppercase();
        let Some(star) = pattern.iter().position(|&c| c == b'*') else { return };
        let (prefix, rest) = (pattern[..star].to_vec(), &pattern[star + 1..]);
        let ext = rest.strip_prefix(b".").unwrap_or(rest).to_vec();
        for name in files.with_ext_where(|e| e == &ext[..]) {
            if !name.starts_with(&prefix) {
                continue;
            }
            self.save_path(ROOM_NAME);
            let Some(data) = files.data(&name) else { continue };
            if data.is_empty() || data.len() > 0xFFFF {
                // 7428 fails (an empty file, or 64 KiB): the original goes
                // on to the next with the buffer as it was.
            } else {
                self.mem.w16(DS, crate::bsa::var::OPEN_SIZE, data.len() as u16);
                for (i, &b) in data.iter().enumerate() {
                    self.mem.w8(DS, 0x929E_u16.wrapping_add(i as u16), b);
                }
            }
            if self.mem.r32(DS, 0x92A0) as i32 <= self.mem.r32(DS, 0x05CC) as i32 {
                files.delete(&name);
            }
        }
    }

    /// `repne scasb` for the NUL from `di`, at most `cx` bytes: the byte
    /// before where it stopped.
    fn room_name_end(&self, mut di: u16, cx: u16) -> u16 {
        for _ in 0..cx {
            let c = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        di.wrapping_sub(1)
    }

    /// 1194:C04C (C048 far). The rented room's file read: with none, 12BC
    /// 0 and the carry (true); else 12BC 1, ROOM_HOURS (2 bytes) and
    /// ROOM_END (4) read from it, and both cleared again (ROOM_HOURS 0,
    /// 12BC 0) when the room's end is past or within 3Ch minutes.
    pub fn room_read(&mut self, files: &mut Files) -> bool {
        self.room_file_name();
        let path = self.save_path(ROOM_NAME);
        if !self.file_find(files, path) {
            self.mem.w8(DS, ROOM_FILED, 0);
            return true;
        }
        self.mem.w8(DS, ROOM_FILED, 1);
        let path = self.save_path(ROOM_NAME);
        if self.file_open(files, path).is_none() {
            // (Found a moment before: the open can't miss.)
            self.mem.w8(DS, ROOM_FILED, 0);
            return true;
        }
        self.file_read(files, DS, ROOM_HOURS, 2);
        self.file_read(files, DS, ROOM_END, 4);
        let left = self.mem.r32(DS, ROOM_END).wrapping_sub(self.mem.r32(DS, 0x05CC)) as i32;
        if left <= 0x3C {
            self.mem.w16(DS, ROOM_HOURS, 0);
            self.mem.w8(DS, ROOM_FILED, 0);
        }
        self.file_close(files);
        false
    }

    /// 1194:BA91. The rented room's file written: with no hours left
    /// deleted (the name at ROOM_NAME as it stands: not made again, the
    /// last one made); else made if missing (0000:677A) and ROOM_HOURS
    /// (2 bytes) and ROOM_END (4) written over its start.
    pub fn room_write(&mut self, files: &mut Files) {
        if self.mem.r16(DS, ROOM_HOURS) == 0 {
            let path = self.save_path(ROOM_NAME);
            self.file_delete(files, path);
            return;
        }
        let mut path = self.room_file_name();
        if !self.file_find(files, path) {
            path = self.save_path(ROOM_NAME);
            self.file_create(files, path);
        }
        if self.file_open(files, path).is_none() {
            return;
        }
        self.file_put(files, DS, ROOM_HOURS, 2);
        self.file_put(files, DS, ROOM_END, 4);
        self.file_close(files);
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
    fn tavern_room(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<()> {
        self.mem.w32(DS, TYPED, 0);
        // AX Ah (1194:BD66): the list's kind (1194:5734).
        self.list_kind = 0xA;
        let pick = hooks.list(self, ROOM_NAMES, 5, text::ROOMS);
        // 1194:BD6D
        hooks.strip_back(self);
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
                // 1194:BD9D: the tiles at 1034.
                self.select_tiles(0x1034);
                self.mem.w32(DS, TYPED, 0);
                self.mem.w8(DS, 0x137E, 3);
                self.split_lines(text::HOW_MANY_DAYS);
                self.mem.w8(DS, 0x13D3, 2);
                hooks.dialog_at(self, 2, text::HOW_MANY_DAYS, TYPED);
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
            // 1194:BE18 (42EE with ARENAFNT).
            self.message_fonts(font::ARENA);
            hooks.message_box(self, text::FREE_ROOM);
        } else {
            self.mem.w32(DS, PRICE, eax);
            let (agreed, walked) = hooks.haggle(self, files)?;
            if walked {
                return self.room_cancelled();
            }
            eax = agreed;
            if self.mem.r32(DS, GOLD) < eax {
                // 1194:BE38: the tiles at 1034.
                self.select_tiles(0x1034);
                hooks.message(self, text::NOT_ENOUGH);
                return self.room_cancelled();
            }
        }
        let gold = self.mem.r32(DS, GOLD).wrapping_sub(eax);
        self.mem.w32(DS, GOLD, gold);
        self.mem.w8(DS, ROOM_RENTED, 1);
        let end = (self.mem.r8(DS, ROOM_DAYS) as u32 * 0x5A0).wrapping_add(self.mem.r32(DS, TIME));
        self.mem.w32(DS, ROOM_END, end);
        hooks.room_file(self, files);
        Some(())
    }

    /// 1194:C14E. The room's price (PRICE) haggled over with the
    /// innkeeper, 1194:8020 in mode 3 (TAVERN.DAT's lines):
    /// - his place's record (`place_lookup` on 0FD2's key, as the drinks
    ///   find it) gives the seed (its dword) his name is made from, set
    ///   [A46], male, at INNKEEPER (1194:080B);
    /// - the room's kind (13C8) picks the item named in the lines (%i,
    ///   1376) from the list at ROOM_NAMES (1194:5585);
    /// - a second name, at 1A4C, the lines' %n, is made with AL and BX as
    ///   the first left them: AL 0 but on one of race 1's paths
    ///   (`npc_name_al`), BX the name file's handle (98E0) under the
    ///   seed's high word;
    /// - the player's personality from 1D3 (1194:5B07), the shop's
    ///   quality and mood (13CA, 13CB).
    ///
    /// The price agreed, or true when the player walked away. DL, which
    /// the haggle keeps at 6598 (for %c, which TAVERN.DAT has none of), is
    /// what the names left in DX: not modelled, taken as 0. None where a
    /// name can't be made or the haggle stops.
    pub fn tavern_room_haggle(&mut self, files: &mut Files, io: &mut impl crate::haggle::HaggleIo) -> Option<(u32, bool)> {
        let ax = self.mem.r16(DS, 0x0FD2);
        let key = (ax & 0xFF00) | ((ax as u8) >> 1) as u16;
        let (seg, si) = (self.mem.r16(DS, 0x1AB7), self.mem.r16(DS, 0x1AB5));
        let si = self.place_lookup(seg, si, key, self.mem.r16(DS, 0x1387));
        let ebx = self.mem.r32(seg, si);
        let kind = self.mem.r8(DS, 0x0A46);
        let (_, al) = self.npc_name_al(files, kind, 0, ebx, DS, INNKEEPER)?;
        let item = self.skip_strings(DS, ROOM_NAMES, self.mem.r8(DS, 0x13C8) as u16);
        self.mem.w16(DS, crate::haggle::var::ITEM, item);
        let seed = (ebx & 0xFFFF_0000) | self.mem.r16(DS, crate::names::var::HANDLE) as u32;
        self.npc_name(files, al, 0, seed, DS, 0x1A4C)?;
        let per = crate::scale::to100(self.mem.r8(DS, PLAYER + 0x2F) as i16) as u16;
        let (price, quality, mood) = (self.mem.r32(DS, PRICE), self.mem.r8(DS, 0x13CA), self.mem.r8(DS, 0x13CB));
        Some(match self.haggle(files, io, price, per, quality, mood, 0, 3, 0x1A4C)? {
            crate::haggle::Haggle::Deal(p) => (p, false),
            crate::haggle::Haggle::NoDeal => (price, true),
        })
    }

    /// 1194:BE78: no room (its hours and the file's mark cleared).
    fn room_cancelled(&mut self) -> Option<()> {
        self.mem.w16(DS, ROOM_HOURS, 0);
        self.mem.w8(DS, ROOM_FILED, 0);
        Some(())
    }

    /// 1194:BC99. Sneaking into a room; see the module's notes. The carry
    /// (the tavern left): the guards called.
    fn tavern_sneak(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<bool> {
        hooks.picture(self, text::SNEAK_PICTURE);
        self.mem.w8(DS, 0x137E, 3);
        self.split_lines(text::SNEAK);
        self.mem.w8(DS, 0x13D3, 1);
        if hooks.dialog_at(self, 1, text::SNEAK, 0) != 0 {
            return Some(false);
        }
        let (ok, chance) = self.lockpick()?;
        if ok {
            self.sneak_room(files, hooks)?;
            return Some(false);
        }
        // 1194:BCC1 (42EE with FONT_A).
        self.message_fonts(font::A);
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
    fn sneak_room(&mut self, files: &mut Files, hooks: &mut dyn TavernHooks) -> Option<()> {
        let kind = self.rng.range(0, 5)? as u8;
        self.mem.w8(DS, PICK, kind);
        self.mem.w8(DS, ROOM_RENTED, 1);
        self.mem.w8(DS, ROOM_DAYS, 1);
        let ax = (self.mem.r8(DS, ROOM_DAYS) as u32 * 0x5A0) & 0xFFFF;
        let end = ax.wrapping_add(self.mem.r32(DS, TIME));
        self.mem.w32(DS, ROOM_END, end);
        self.mem.w16(DS, ROOM_HOURS, end as u16);
        hooks.room_file(self, files);
        let name = self.skip_strings(DS, ROOM_NAMES, self.mem.r8(DS, PICK) as u16);
        self.sprintf((DS, NAME), (DS, text::SNEAK_ROOM), &[Arg::Str(DS, name)]);
        // 1194:BD3F (42EE with ARENAFNT).
        self.message_fonts(font::ARENA);
        hooks.message_box(self, NAME);
        Some(())
    }
}
