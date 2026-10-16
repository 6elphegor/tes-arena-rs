//! The mages' guild (1194:A73E, `mages_guild`), the shop of building
//! kind 7 (the table at 869B maps kinds 4, 5, 7, 6 to the tavern, the
//! temple, the guild and the equipment store, 1194:F48E).
//!
//! Unless a fight is on (FFB) or the guards are coming (680F), bank 18h
//! is mapped (the bank before kept at 12CC) and the guild's state set up
//! (1194:AD39): today a holiday for the guild or not (13C9: 1, or FFh;
//! 1194:4A4A), the first 1388h bytes of the frame (9030) cleared, then
//! the stock (`stock_load`, 1194:ADA4): the file "gl" + the digits of the
//! player's x + y (C3D0 + C3D2, the first two digits overwritten) +
//! ".64" (`GuildHooks::stock_file`), when it was written less than 1440
//! minutes ago (its first dword, a time, against 5CC), gives the stock's
//! count (12BE) and records (0Bh bytes each, the magic-item template's
//! layout, from frame:0); otherwise the stock is made afresh from the
//! place's seed (A4C + 1000; 1 for 0, the game's seed put back after):
//! the shop's level and price factors (1194:4AA7, 4AF8), then one magic
//! item a level (`stock_make`, 1194:AE8F). A menu then runs until "Exit"
//! (or nothing picked): for a spellcaster (class byte bit 20h) "Buy,
//! Detect Magic, Spellmaker, Steal", otherwise without the spellmaker.
//! A steal that calls the guards ends it at once. On leaving (1194:
//! A7F1) the stock is written to the file (1194:A82D: the time, the
//! count and the records) and the bank put back.
//!
//! - Buy (1194:AA8C): a menu of potions, magic items (with any stock)
//!   and spells (casters). Without stock a caster's "Spells" runs the
//!   magic items' routine on the empty stock (the table at 7149 is
//!   indexed by the pick: the original's bug).
//!   - Potions (1194:AFF9): fifteen, their prices at 2A20 (a holiday's,
//!     service 5); how many typed (1194:B0F8: digits only, asked again
//!     otherwise; 0 cancels; its low byte kept, 7138); the price times
//!     that in 16 bits, haggled over (1194:B51A: the guild's own quality
//!     and factor are replaced by 44h each, the original's bug), paid,
//!     and the potions added (1194:B158: onto an identified stack of the
//!     type, 1194:75B8, or into a free slot, a refund when there is
//!     none).
//!   - Magic items (1194:B327): one of the stock, haggled over with the
//!     guild's quality and factor (a holiday's price, service 4), paid
//!     when it fits into a free slot, and taken out of the stock.
//!   - Spells (1194:B1EA): the guild's spells (the first 2Fh with a name
//!     whose price, twice their cost, is at most the shop's level times
//!     250; 1194:A87E), sorted by price (1194:AA07), one picked; its
//!     screen (1194:68A3: the spell's record copied to 57E6, the rest the
//!     front end's), whose buttons are clicked: "back" lists them again,
//!     "buy" buys it (1194:A8E4: its price doubled, on a holiday its
//!     undiscounted cost, haggled over as potions are, paid, and the
//!     spell added to the player's list at 50A), "exit" leaves. On a
//!     holiday the record's cost is the holiday's (service 2) while the
//!     screen is up.
//! - Detect Magic (1194:AB39): an item picked; one not yet identified is
//!   identified for a tenth of its value (at least 1; a holiday's price,
//!   service 3, free when 0), asked first; a potion identified joins an
//!   identified stack of its type, if any.
//! - Spellmaker: the front end's (0000:A611, 1194:6064, 0000:A6B5).
//! - Steal (1194:AC2A): a potion or a magic item (asked when there is
//!   stock); the lockpicking roll (1194:53F9) against the lock tried
//!   (13CC, the shop's level). Done: a random potion, or a random item of
//!   the stock (taken out of it); failed: the guards' roll may call them.
//!
//! The screens, menus, lists, dialogs and messages are the front end's
//! (`ShopHooks`, `GuildHooks`), as are the stock's file, the haggle, the
//! spell's screen and its clicks, and the spellmaker. Assumed: the
//! message box returns the carry clear and keeps BX (the chance the
//! guards' roll reads), as the tavern does; the screens' buffers (1194:
//! 5E49, 5E54 and the spell screen's) are the front end's. With EMS off
//! the lists' clearing (1194:AE68) fills with the caller's AX's high
//! byte, taken here as the original leaves it where the code shows it
//! and 0 after the sorts.

use crate::bsa::Files;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::shops::ShopHooks;

/// What the guild leaves to the front end, besides `ShopHooks`.
pub trait GuildHooks: ShopHooks {
    /// 1194:4B60 and 4BFF: the menu's screen (MENUSCRN.IMG, 12EB) and its
    /// title ("Mages Guild", 6F11).
    fn screen(&mut self, _m: &mut Machine) {}
    /// 25FD:0844, 0000:600C, 67C4: the stock's file named at DS:`name`,
    /// its bytes when there is one.
    fn stock_file(&mut self, _m: &mut Machine, _name: u16) -> Option<Vec<u8>> {
        None
    }
    /// 1194:A82D's writes (0000:677A, 6DDC, 6742): the stock's file
    /// named at DS:`name` made with `bytes`.
    fn stock_write(&mut self, _m: &mut Machine, _name: u16, _bytes: &[u8]) {}
    /// 1194:59CC: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:5734 over the stock's texts: a list of `cx` strings at
    /// `seg`:`si`. The pick (FFFFh: none).
    fn stock_list(&mut self, _m: &mut Machine, _seg: u16, _si: u16, _cx: u16) -> u16 {
        0xFFFF
    }
    /// 1194:8020: `price` haggled over with the shop's `quality` and
    /// `factor` and the player's personality as a percentage `per`
    /// (1194:5B07), the seller's name at 1A4C: the price agreed, or the
    /// carry when the player walks away.
    fn haggle(&mut self, _m: &mut Machine, price: u32, _quality: u16, _factor: u16, _per: u16) -> (u32, bool) {
        (price, false)
    }
    /// 1194:68A3's screen (the record copied to 57E6 already).
    fn spell_screen(&mut self, _m: &mut Machine) {}
    /// A poll of the mouse while the spell's screen is up (0000:5454,
    /// 8FC4, 77EC, 9466, 541E): its buttons (AFF9), x and y (8F92, 8F94).
    /// By default "exit" clicked.
    fn click(&mut self, _m: &mut Machine) -> (u8, u16, u16) {
        (1, 0x130, 0xC0)
    }
    /// The spellmaker (0000:A611, 1194:6064 with SI the player, 0000:A6B5;
    /// the far calls keep and give back the world around the screen, the
    /// front end's). By default 6064 runs (`Machine::spellmaker`) without
    /// its screen: the first click leaves.
    fn spellmaker(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.spellmaker(files, &mut crate::spellmaker::NoSpellmakerHooks, 0x01A4)
    }
}

/// `GuildHooks` that do nothing: every menu left at once, nothing typed
/// (the count asked for is "0").
pub struct NoGuildHooks;

impl ShopHooks for NoGuildHooks {
    fn dialog(&mut self, m: &mut Machine, kind: u8, di: u16) -> u16 {
        if kind == 2 {
            m.mem.w8(DS, di, b'0');
            m.mem.w8(DS, di.wrapping_add(1), 0);
            return 0;
        }
        1
    }
}
impl GuildHooks for NoGuildHooks {}

/// DGROUP variables.
pub mod var {
    /// The menus: title, keys and entries (casters; others).
    pub const MENU_TITLE: u16 = 0x6EAE;
    pub const KEYS: u16 = 0x6EBF;
    pub const ENTRIES: u16 = 0x6EC5;
    pub const KEYS_PLAIN: u16 = 0x6F27;
    pub const ENTRIES_PLAIN: u16 = 0x6F1D;
    pub const ROUTINES: u16 = 0x713B;
    pub const ROUTINES_PLAIN: u16 = 0x7143;
    /// The buy menu: title; keys and entries with no stock (casters),
    /// with stock (casters; others); its routines.
    pub const BUY_TITLE: u16 = 0x6FCD;
    pub const BUY_KEYS: u16 = 0x7011;
    pub const BUY_ENTRIES: u16 = 0x7014;
    pub const BUY_KEYS_STOCK: u16 = 0x6FDB;
    pub const BUY_ENTRIES_STOCK: u16 = 0x6FDF;
    pub const BUY_KEYS_PLAIN: u16 = 0x7113;
    pub const BUY_ENTRIES_PLAIN: u16 = 0x7116;
    pub const BUY_ROUTINES: u16 = 0x7149;
    /// Today is the guild's holiday (1; FFh not).
    pub const HOLIDAY: u16 = 0x13C9;
    /// The pick.
    pub const PICK: u16 = 0x13C8;
    /// The stock: its count, the next record's offset, the next text's.
    pub const STOCK: u16 = 0x12BE;
    pub const NEXT: u16 = 0x12C5;
    pub const TEXT: u16 = 0x12C7;
    /// The magic item being made (and its running key; the keys made).
    pub const TEMPLATE: u16 = 0x7033;
    pub const KEY: u16 = 0x12C9;
    pub const KEYS_MADE: u16 = 0x1044;
    /// Failed tries at making the stock.
    pub const TRIES: u16 = 0x717F;
    /// How many potions; the most a spell can cost.
    pub const COUNT: u16 = 0x7138;
    /// The spells sold, their count, the one bought, its cost saved.
    pub const SPELLS: u16 = 0x7088;
    pub const SPELL_COUNT: u16 = 0x713A;
    pub const BOUGHT: u16 = 0x7108;
    pub const SAVED_COST: u16 = 0x717D;
    /// The bank mapped before; the stock's file's name.
    pub const SAVED_BANK: u16 = 0x12CC;
    pub const FILE_NAME: u16 = 0x6FBE;
    /// The spell's record while its screen is up.
    pub const SPELL_COPY: u16 = 0x57E6;
}
use var::*;

const SHOP: u16 = 0x137F;
const LEVEL: u16 = 0x13CC;
const QUALITY: u16 = 0x13CA;
const FACTOR: u16 = 0x13CB;
const PRICE: u16 = 0x1391;
const NAME_PTR: u16 = 0x1376;
const GOLD: u16 = 0x05C2;
const TIME: u16 = 0x05CC;
const FRAME: u16 = 0x9030;
const COLOUR: u16 = 0x9288;
const MAKING: u16 = 0x7109;
const SORT_I: u16 = 0x12BC;
const PLAYER: u16 = 0x01A4;
const ITEMS: u16 = 0x0212;
const ITEM: u16 = 0x13DA;
const POTION_NAMES: u16 = 0x28CD;
const POTION_PRICES: u16 = 0x2A20;
const POTION_SPELLS: u16 = 0x2A11;
const SPELL_KNOWN: u16 = 0x050A;
const ITEM_NAME: u16 = 0x1395;
const SELLER: u16 = 0x1A4C;
const TEXTS: u16 = 0x987A;
const NAMES: u16 = 0x929E;
const POTION_TEXTS: u16 = 0x9686;
const PICK_TEXTS: u16 = 0x9A6E;
const PICK_NEXT: u16 = 0x12D5;
const IDENTIFY_ITEM: u16 = 0xA868;

/// Texts.
mod text {
    pub const NOT_ENOUGH: u16 = 0x198C;
    pub const FULL: u16 = 0x19DE;
    pub const NO_ITEMS: u16 = 0x19C5;
    pub const KNOWN: u16 = 0x6F5C;
    pub const IDENTIFIED: u16 = 0x714F;
    pub const FREE: u16 = 0x7210;
    pub const DETECT: u16 = 0x6F7B;
    pub const PRICE_LINE: u16 = 0x706E;
    pub const STOCK_LINE: u16 = 0x705C;
    pub const HOW_MANY: u16 = 0x71BF;
    pub const DETECT_PICTURE: u16 = 0x132F;
    pub const STEAL_FAILED: u16 = 0x20FA;
    pub const STOLEN: u16 = 0x8B29;
    pub const EXTENSION: u16 = 0x8B05;
}

impl Machine {
    /// 1194:A73E. See the module's notes. None where the original faults
    /// or hangs.
    pub fn mages_guild(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        // Where the port gives up (a hook with no answer, a routine not
        // ported), the end's state is left all the same (1194:A7F1): the
        // clip as it was, no shop, no holiday, and the bank mapped before
        // the guild's (SAVED_BANK) mapped back. Its stock file isn't
        // written. The original has no such exit.
        let saved = self.mem.r16(DS, 0xA91A);
        let entered = self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x680F) == 0;
        let r = self.mages_guild_body(files, hooks);
        if r.is_none() {
            self.mem.w16(DS, 0xA91A, saved);
            self.mem.w8(DS, SHOP, 0xFF);
            self.mem.w8(DS, HOLIDAY, 0xFF);
            if entered {
                let bank = self.mem.r16(DS, SAVED_BANK);
                self.map_bank(bank, 0);
            }
        }
        r
    }

    fn mages_guild_body(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        let saved = self.mem.r16(DS, 0xA91A);
        if self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x680F) == 0 {
            self.mem.w8(DS, SHOP, 7);
            let bank = self.map_bank(0x18, 0);
            self.mem.w16(DS, SAVED_BANK, bank);
            self.guild_enter(files, hooks)?;
            loop {
                self.map_bank(0x18, 0);
                let caster = self.mem.r8(DS, 0x01A9) & 0x20 != 0;
                let (bx, di) = if caster { (KEYS, ENTRIES) } else { (KEYS_PLAIN, ENTRIES_PLAIN) };
                let pick = hooks.menu(self, MENU_TITLE, bx, di);
                if (pick as i16) < 0 {
                    break;
                }
                let table = if caster {
                    if pick == 4 {
                        break;
                    }
                    ROUTINES
                } else {
                    if pick == 3 {
                        break;
                    }
                    ROUTINES_PLAIN
                };
                if self.guild_choice(files, hooks, table, pick)? {
                    break;
                }
            }
        }
        self.mem.w16(DS, 0xA91A, saved);
        // 1194:A7F1
        self.mem.w8(DS, SHOP, 0xFF);
        self.stock_save(hooks);
        self.mem.w8(DS, HOLIDAY, 0xFF);
        let bank = self.mem.r16(DS, SAVED_BANK);
        self.map_bank(bank, 0);
        Some(())
    }

    /// The menu's routine for `pick`, by its address in the table at
    /// `table`. The carry it returns (only a steal's guards set it).
    fn guild_choice(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks, table: u16, pick: u16) -> Option<bool> {
        match self.mem.r16(DS, table.wrapping_add(pick.wrapping_add(pick))) {
            0xAA8C => self.guild_buy(files, hooks).map(|_| false),
            0xAB39 => self.guild_detect(hooks).map(|_| false),
            0xA80D => {
                // 1194:A80D: A902 bit 0 cleared around the spellmaker.
                let f = self.mem.r16(DS, 0xA902) & !1;
                self.mem.w16(DS, 0xA902, f);
                hooks.spellmaker(self, files)?;
                let f = self.mem.r16(DS, 0xA902) & !1;
                self.mem.w16(DS, 0xA902, f);
                Some(false)
            }
            0xAC2A => self.guild_steal(hooks),
            _ => crate::giveup::not_ported("1194:A80D: a guild choice not in the table"),
        }
    }

    /// 1194:AD39: the state set up on entering.
    fn guild_enter(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        hooks.screen(self);
        hooks.quest_check(self);
        hooks.quests(self, files)?;
        self.map_bank(0x18, 0);
        self.mem.w8(DS, HOLIDAY, 0xFF);
        if self.shop_holiday() {
            self.mem.w8(DS, HOLIDAY, 1);
        }
        // 1194:AFE6
        let frame = self.mem.r16(DS, FRAME);
        for i in 0..0x1388u16 {
            self.mem.w8(frame, i, 0);
        }
        if self.stock_load(hooks)? {
            let mut eax = self.mem.r32(DS, 0x0A4C).wrapping_add(1000);
            if eax == 0 {
                eax = 1;
            }
            let saved = self.rng.seed;
            self.rng.seed = eax;
            // The game's seed back even where the port gives up.
            let r = self.shop_roll_level().and_then(|level| self.shop_price_factors(level)).and_then(|()| self.stock_make());
            self.rng.seed = saved;
            r?;
        }
        self.mem.w8(DS, 0x12CB, 0);
        Some(())
    }

    /// 1194:ADA4. The stock's file read; true (the carry) when it isn't,
    /// or is a day old or more, and the stock is to be made.
    fn stock_load(&mut self, hooks: &mut dyn GuildHooks) -> Option<bool> {
        // 1194:AE39: the digits of x + y, then ".64".
        let v = self.mem.r16(DS, 0xC3D0).wrapping_add(self.mem.r16(DS, 0xC3D2));
        self.dun_ultoa(v as u32, 0x10A8);
        let mut di = 0x10A8u16;
        for _ in 0..0x64 {
            let b = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if b == 0 {
                break;
            }
        }
        di = di.wrapping_sub(1);
        for i in 0..5 {
            let b = self.mem.r8(DS, text::EXTENSION + i);
            self.mem.w8(DS, di.wrapping_add(i), b);
        }
        self.guild_strcpy(0x10A8, FILE_NAME);
        self.mem.w16(DS, FILE_NAME, 0x6C67);
        let Some(bytes) = hooks.stock_file(self, FILE_NAME) else {
            return Some(true);
        };
        let mut file = bytes.into_iter();
        let mut read = |m: &mut Self, seg: u16, si: u16, n: u16| {
            let mut k = 0u16;
            while k < n {
                let Some(b) = file.next() else { break };
                m.mem.w8(seg, si.wrapping_add(k), b);
                k += 1;
            }
            k
        };
        read(self, DS, 0x1044, 4);
        let eax = self.mem.r32(DS, 0x1044);
        if self.mem.r32(DS, TIME).wrapping_sub(eax) >= 0x5A0 {
            return Some(true);
        }
        // 1194:AE02
        self.map_bank(0x18, 0);
        read(self, DS, STOCK, 1);
        let n = self.mem.r8(DS, STOCK);
        if n != 0 {
            let frame = self.mem.r16(DS, FRAME);
            let ax = read(self, frame, 0, (n as u16).wrapping_mul(0x0B));
            self.stock_texts(ax)?;
        }
        Some(false)
    }

    /// 1194:A82D. The stock's file written: the time, the count and the
    /// records (bank 18h mapped meanwhile).
    fn stock_save(&mut self, hooks: &mut dyn GuildHooks) {
        let mut bytes = self.mem.r32(DS, TIME).to_le_bytes().to_vec();
        let n = self.mem.r8(DS, STOCK);
        bytes.push(n);
        let old = self.map_bank(0x18, 0);
        let frame = self.mem.r16(DS, FRAME);
        bytes.extend((0..(n as u16).wrapping_mul(0x0B)).map(|i| self.mem.r8(frame, i)));
        self.map_bank(old, 0);
        hooks.stock_write(self, FILE_NAME, &bytes);
    }

    /// 1194:AE8F. The stock made: from the shop's level 3 up, one magic
    /// item a level (1194:B566, B617, B6B0 by a third each), its key new
    /// to the stock (1194:AF3B); a failed one is tried again, up to 200
    /// failures. Then sorted and its texts made.
    fn stock_make(&mut self) -> Option<()> {
        let ax = self.mem.r16(DS, 0x1034);
        self.stock_clear(ax);
        self.mem.w8(DS, PICK, 0xFF);
        if self.mem.r8(DS, LEVEL) < 3 {
            return Some(());
        }
        let mut cx = self.mem.r8(DS, LEVEL) as u16;
        self.mem.w8(DS, STOCK, 0);
        loop {
            let kind = self.rng.range(0, 3)?;
            for i in 0..0x0Bu16 {
                self.mem.w8(DS, TEMPLATE + i, 0);
            }
            self.mem.w32(DS, TEMPLATE + 5, 0);
            let made = match kind {
                0 => self.shop_magic_kind0().map(|_| true)?,
                1 => self.shop_magic_kind1()?,
                _ => self.shop_magic_kind2().map(|_| true)?,
            };
            if made && self.stock_key_new() {
                let n = self.mem.r8(DS, STOCK).wrapping_add(1);
                self.mem.w8(DS, STOCK, n);
                let di = self.mem.r16(DS, NEXT);
                let frame = self.mem.r16(DS, FRAME);
                let (_, di) = self.memmove(DS, TEMPLATE, frame, di, 0x0B);
                self.mem.w16(DS, NEXT, di);
            } else {
                let t = self.mem.r8(DS, TRIES).wrapping_add(1);
                self.mem.w8(DS, TRIES, t);
                cx = cx.wrapping_add(1);
                if t >= 0xC8 {
                    break;
                }
            }
            self.mem.w16(DS, KEY, 0);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
        let n = self.mem.r8(DS, STOCK);
        self.stock_sort(n);
        self.stock_texts(0)
    }

    /// 1194:AF3B. Whether the item's key (KEY) is new to the keys made
    /// (words from 1044 to a 0), which it joins.
    fn stock_key_new(&mut self) -> bool {
        let ax = self.mem.r16(DS, KEY);
        let mut si = KEYS_MADE;
        loop {
            let v = self.mem.r16(DS, si);
            if v == 0 {
                self.mem.w16(DS, si, ax);
                return true;
            }
            if v == ax {
                return false;
            }
            si = si.wrapping_add(2);
        }
    }

    /// 1194:AE68: the tries, the stock's next record and next text reset
    /// (0, 7D0h), bank 18h mapped, and the 200 bytes of keys from 1044
    /// filled with the word AX the mapping leaves (AL 0): `ax` the
    /// caller's, which it keeps with EMS off.
    fn stock_clear(&mut self, ax: u16) {
        self.mem.w8(DS, TRIES, 0);
        let ax = self.map_bank(0x18, ax);
        self.mem.w16(DS, NEXT, 0);
        self.mem.w16(DS, TEXT, 0x7D0);
        let ah = (ax >> 8) as u8;
        for i in 0..0xC8u16 {
            self.mem.w8(DS, KEYS_MADE + i, if i & 1 == 0 { 0 } else { ah });
        }
    }

    /// 1194:B893. The stock sorted by price, as the original's exchange
    /// sort runs (its counter at 12BC; each exchange through the
    /// template at 7033).
    fn stock_sort(&mut self, dl: u8) {
        if dl <= 1 {
            return;
        }
        let frame = self.mem.r16(DS, FRAME);
        self.mem.w8(DS, SORT_I, 0);
        let mut dl = dl - 1;
        let mut cx = dl as u16;
        let mut si = 0u16;
        loop {
            let mut di = si;
            let mut ebx = self.mem.r32(frame, si.wrapping_add(5));
            loop {
                di = di.wrapping_add(0x0B);
                if self.mem.r32(frame, di.wrapping_add(5)) < ebx {
                    self.memmove(frame, si, DS, TEMPLATE, 0x0B);
                    self.memmove(frame, di, frame, si, 0x0B);
                    self.memmove(DS, TEMPLATE, frame, di, 0x0B);
                    ebx = self.mem.r32(frame, si.wrapping_add(5));
                }
                let i = self.mem.r8(DS, SORT_I).wrapping_add(1);
                self.mem.w8(DS, SORT_I, i);
                if i < dl {
                    continue;
                }
                break;
            }
            self.mem.w8(DS, SORT_I, 0);
            dl = dl.wrapping_sub(1);
            si = si.wrapping_add(0x0B);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                return;
            }
        }
    }

    /// 1194:AF54. The stock's texts made for the list: `stock_clear`
    /// (with AX `ax`), then for each record (a count of 0 runs 65536
    /// times) its name (1194:AFD9) and price (a holiday's, service 4) as
    /// "%s\n%lu gp" from frame:7D0. None where a record's kind names
    /// through a routine that isn't (3 and up).
    fn stock_texts(&mut self, ax: u16) -> Option<()> {
        self.stock_clear(ax);
        let frame = self.mem.r16(DS, FRAME);
        let n = self.mem.r8(DS, STOCK) as u32;
        let mut si = 0u16;
        for _ in 0..if n == 0 { 0x10000 } else { n } {
            self.memmove(frame, si, DS, TEMPLATE, 0x0B);
            let mut eax = self.mem.r32(DS, TEMPLATE + 5);
            if self.mem.r8(DS, HOLIDAY) == 1 {
                eax = self.holiday_price(eax, 4);
            }
            self.mem.w32(DS, PRICE, eax);
            self.stock_name()?;
            // 1194:AFAA
            let di = self.mem.r16(DS, TEXT);
            let end = self.sprintf((frame, di), (DS, text::STOCK_LINE), &[Arg::Str(DS, ITEM_NAME), Arg::Long(self.mem.r32(DS, PRICE))]);
            self.mem.w16(DS, TEXT, self.guild_scan(frame, di, end));
            si = si.wrapping_add(0x0B);
        }
        Some(())
    }

    /// 1194:5EA5 after a sprintf from `di` (its NUL at `end`): DI past
    /// the NUL (its scan stops after 4E20h bytes).
    fn guild_scan(&self, seg: u16, di: u16, end: u16) -> u16 {
        let _ = seg;
        if end.wrapping_sub(di) < 0x4E20 { end.wrapping_add(1) } else { di.wrapping_add(0x4E20) }
    }

    /// 1194:AFD9. The template's item named at 1395 (27CE:000C) by its
    /// kind: a trinket (B7AD), a magic weapon or armour (B7C4), or a
    /// bracelet of a material (B7D8: a sub-kind from 4 up is taken as 3,
    /// after the INT 3 DOS ignores, and stored so; it sets only AL, AH
    /// is DGROUP's segment's high byte, as its callers leave AX, and
    /// 27CE:000C keeps it at 1258).
    fn stock_name(&mut self) -> Option<()> {
        let t = |m: &Self, f: u16| m.mem.r8(DS, TEMPLATE + f);
        let kind = t(self, 0);
        match kind {
            0 => self.item_name_format(DS, 2, 0, t(self, 2), 0, t(self, 1), t(self, 3)),
            1 => self.item_name_format(DS, 2, 1, 0, 2, t(self, 1), t(self, 3)),
            2 => {
                let mut cl = t(self, 1);
                if cl >= 4 {
                    self.mem.w8(DS, TEMPLATE + 1, 3);
                    cl = 3;
                }
                self.item_name_format(DS, 2, 2, (DS >> 8) as u8, t(self, 4) as u16, cl, 0)
            }
            _ => None,
        }
    }

    /// 0000:9CA0: the string at DS:`si` copied to DS:`di` with its NUL;
    /// DI past it.
    fn guild_strcpy(&mut self, mut si: u16, mut di: u16) -> u16 {
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

    /// 1194:AA8C. The buy menu (see the module's notes).
    fn guild_buy(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        // 1194:AA8F: the gold left.
        hooks.gold_left(self);
        let stock = self.mem.r8(DS, STOCK) >= 1;
        let caster = self.mem.r8(DS, 0x01A9) & 0x20 != 0;
        let menu = match (stock, caster) {
            (true, true) => Some((BUY_KEYS_STOCK, BUY_ENTRIES_STOCK)),
            (true, false) => Some((BUY_KEYS_PLAIN, BUY_ENTRIES_PLAIN)),
            (false, true) => Some((BUY_KEYS, BUY_ENTRIES)),
            (false, false) => None,
        };
        let pick = match menu {
            Some((bx, di)) => {
                let pick = hooks.menu(self, BUY_TITLE, bx, di);
                if (pick as i16) < 0 {
                    // 1194:AADD
                    hooks.strip_back(self);
                    return Some(());
                }
                pick
            }
            None => 0,
        };
        match self.mem.r16(DS, BUY_ROUTINES.wrapping_add(pick.wrapping_add(pick))) {
            0xAFF9 => self.guild_potions(files, hooks),
            0xB327 => self.guild_items(files, hooks),
            0xB1EA => self.guild_spells(files, hooks),
            _ => crate::giveup::not_ported("1194:AA8C: a guild purchase not in the table"),
        }
    }

    /// The window colour of a message box (1194:42EE: the fonts set, the
    /// front end's).
    fn guild_colour(&mut self) {
        self.mem.w8(DS, COLOUR, 0x60);
    }

    /// 1194:AFF9. Potions bought (see the module's notes).
    fn guild_potions(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        self.mem.w16(DS, TEXT, POTION_TEXTS);
        for i in 0..0x0Fu16 {
            let name = self.skip_strings(DS, POTION_NAMES, i);
            let ax = self.potion_price(i);
            let di = self.mem.r16(DS, TEXT);
            let end = self.sprintf((DS, di), (DS, text::PRICE_LINE), &[Arg::Int(ax), Arg::Str(DS, name)]);
            self.mem.w16(DS, TEXT, self.guild_scan(DS, di, end));
        }
        // AX Bh (1194:B06B): the list's kind (1194:5734).
        self.list_kind = 0xB;
        let pick = hooks.list(self, POTION_TEXTS, 0x0F, 0);
        self.mem.w8(DS, PICK, pick as u8);
        // 1194:B072
        hooks.strip_back(self);
        if (pick as i16) < 0 {
            return Some(());
        }
        if !self.potion_count(hooks) {
            return Some(());
        }
        let ty = self.mem.r8(DS, PICK) as u16;
        let ax = self.potion_price(ty);
        let eax = ax.wrapping_mul(self.mem.r16(DS, COUNT)) as u32;
        self.mem.w32(DS, PRICE, eax);
        let name = self.skip_strings(DS, POTION_NAMES, ty);
        self.mem.w16(DS, NAME_PTR, name);
        let (eax, walked) = self.guild_haggle(files, hooks)?;
        if walked {
            return Some(());
        }
        if self.mem.r32(DS, GOLD) < eax {
            hooks.message(self, text::NOT_ENOUGH);
            return Some(());
        }
        let g = self.mem.r32(DS, GOLD).wrapping_sub(eax);
        self.mem.w32(DS, GOLD, g);
        self.mem.w32(DS, PRICE, eax);
        self.mem.w8(DS, MAKING, 0);
        if self.potions_add().is_some() {
            self.sort_inventory();
        } else {
            hooks.message(self, text::FULL);
            let g = self.mem.r32(DS, GOLD).wrapping_add(self.mem.r32(DS, PRICE));
            self.mem.w32(DS, GOLD, g);
        }
        Some(())
    }

    /// A potion's price (2A20), a holiday's (service 5), in 16 bits.
    fn potion_price(&self, ty: u16) -> u16 {
        let ax = self.mem.r16(DS, POTION_PRICES.wrapping_add(ty.wrapping_add(ty)));
        if self.mem.r8(DS, HOLIDAY) == 1 { self.holiday_price(ax as u32, 5) as u16 } else { ax }
    }

    /// 1194:B0F8. How many potions: typed until digits (an empty text
    /// asks again); its low byte at COUNT. False (the carry) for 0.
    fn potion_count(&mut self, hooks: &mut dyn GuildHooks) -> bool {
        loop {
            let name = self.skip_strings(DS, POTION_NAMES, self.mem.r8(DS, PICK) as u16);
            self.sprintf((DS, 0x10A8), (DS, text::HOW_MANY), &[Arg::Str(DS, name)]);
            self.mem.w32(DS, 0x1044, 0);
            self.mem.w8(DS, 0x137E, 3);
            self.split_lines(0x10A8);
            self.mem.w8(DS, 0x13D3, 2);
            hooks.dialog(self, 2, 0x1044);
            if self.rest_digits(0x1044) {
                break;
            }
        }
        if self.mem.r8(DS, 0x1044) == 0 {
            return false;
        }
        let ax = self.parse_number(0x1044);
        if ax == 0 {
            return false;
        }
        self.mem.w16(DS, COUNT, ax & 0xFF);
        true
    }

    /// 1194:B51A. The price (PRICE) haggled over (1194:8020) with the
    /// seller named (1194:080B, at 1A4C), the quality and factor 44h each
    /// (the original's bug); the holiday's price it works out first is
    /// lost. The price agreed and whether the player walked away.
    fn guild_haggle(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<(u32, bool)> {
        self.guild_seller(files)?;
        let per = Self::guild_percent(self.mem.r8(DS, 0x01D3) as u16);
        let price = self.mem.r32(DS, PRICE);
        Some(hooks.haggle(self, price, 0x44, 0x44, per))
    }

    /// 1194:080B for the seller: a name of set A46 (male) from the seed
    /// A56 + 0Ch, at 1A4C.
    fn guild_seller(&mut self, files: &mut Files) -> Option<()> {
        let seed = self.mem.r32(DS, 0x0A56).wrapping_add(0x0C);
        let kind = self.mem.r8(DS, 0x0A46);
        self.npc_name(files, kind, 0, seed, DS, SELLER).map(|_| ())
    }

    /// 1194:5B07: `ax` × 100 / 256, rounded half away from zero.
    fn guild_percent(ax: u16) -> u16 {
        crate::scale::to100(ax as i16) as u16
    }

    /// 1194:B158. COUNT potions of type PICK added: onto an identified
    /// stack of the type, or into a free slot (its price, with MAKING,
    /// the type's; its value three quarters of it; condition 80h,
    /// 1194:F312). None (the carry) when there is no free slot; else
    /// the slot.
    fn potions_add(&mut self) -> Option<u16> {
        let cx = self.mem.r16(DS, COUNT);
        let ty = self.mem.r8(DS, PICK);
        if let Some(si) = self.find_stack(ty as u16) {
            let n = self.mem.r16(DS, si.wrapping_add(1)).wrapping_add(cx);
            self.mem.w16(DS, si.wrapping_add(1), n);
            self.mem.w16(DS, COUNT, 0);
            return Some(si);
        }
        let Ok((si, _)) = self.free_slot(DS, PLAYER) else { return None };
        for i in 0..0x13u16 {
            self.mem.w8(DS, si.wrapping_add(i), 0);
        }
        self.mem.w16(DS, si.wrapping_add(1), cx.wrapping_sub(1));
        let f = self.mem.r8(DS, si.wrapping_add(0x0E)) | 0x40 | 1 | 0x0C;
        self.mem.w8(DS, si.wrapping_add(0x0E), f);
        self.mem.w8(DS, si, ty);
        let eax = if self.mem.r8(DS, MAKING) != 0 { self.mem.r16(DS, POTION_PRICES.wrapping_add(ty as u16 * 2)) as u32 } else { 0 };
        self.mem.w32(DS, PRICE, eax);
        self.mem.w32(DS, si.wrapping_add(0x0A), (eax & 0xFFFF).wrapping_mul(3) >> 2);
        let spell = self.mem.r8(DS, POTION_SPELLS.wrapping_add(ty as u16));
        self.mem.w8(DS, si.wrapping_add(0x11), spell);
        self.mem.w8(DS, si.wrapping_add(0x12), 0xFF);
        self.guild_condition(si, 0x80);
        self.mem.w16(DS, COUNT, 0);
        Some(si)
    }

    /// 1194:F312 for the item at `si`: health and its maximum from the
    /// condition `code`; with a magic template up (13F4) its price (13FA)
    /// instead when higher, capped at 50000.
    fn guild_condition(&mut self, si: u16, code: u16) {
        let ax = match code {
            0x80 => 0x32,
            0x40 => 0x19,
            _ => (code >> 8).wrapping_mul(100),
        };
        let mut store = ax;
        let mut set = true;
        self.mem.w16(DS, si.wrapping_add(6), ax);
        self.mem.w16(DS, si.wrapping_add(8), ax);
        if self.mem.r8(DS, 0x13F4) != 0 {
            if self.mem.r32(DS, 0x13FA) > 0xC350 {
                store = 0xC350;
            } else if self.mem.r16(DS, 0x13FA) > ax {
                store = self.mem.r16(DS, 0x13FA);
            } else {
                set = false;
            }
            if set {
                self.mem.w16(DS, si.wrapping_add(6), store);
                self.mem.w16(DS, si.wrapping_add(8), store);
            }
        }
    }

    /// 1194:75B8. The player's first identified stack (category 3) of
    /// type `ty` (types from 80h mapped through 36DD, 1194:7596).
    pub(crate) fn find_stack(&self, ty: u16) -> Option<u16> {
        let (mut si, mut cx) = (ITEMS, 0x28u16);
        loop {
            // 1194:75D3
            let found = loop {
                if cx == 0 {
                    return None;
                }
                if self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x40 != 0
                    && self.shop_map_type(self.mem.r8(DS, si)) == ty
                    && self.item_category(DS, si) == 3
                {
                    cx -= 1;
                    break si;
                }
                si = si.wrapping_add(0x13);
                cx -= 1;
            };
            if self.mem.r8(DS, found.wrapping_add(0x0E)) & 2 == 0 {
                return Some(found);
            }
            si = found.wrapping_add(0x13);
        }
    }

    /// 1194:B327. A magic item of the stock bought (see the module's
    /// notes). Its list is shown with DS the frame; 1194:5734 returns with
    /// DS and ES DGROUP.
    fn guild_items(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        if self.free_slot(DS, PLAYER).is_err() {
            self.guild_colour();
            hooks.message_box(self, text::FULL);
            return Some(());
        }
        let old = self.map_bank(0x18, 0);
        let frame = self.mem.r16(DS, FRAME);
        let n = self.mem.r8(DS, STOCK) as u16;
        // AX 7h (1194:B351): the list's kind (1194:5734).
        self.list_kind = 0x7;
        let pick = hooks.stock_list(self, frame, 0x7D0, n);
        self.mem.w8(DS, PICK, pick as u8);
        // 1194:B358
        hooks.strip_back(self);
        if (pick as i16) >= 0 {
            self.guild_item_bought(files, hooks, pick)?;
        }
        self.map_bank(old, 0);
        Some(())
    }

    /// 1194:B362..B419 after the pick: the record copied out (1194:B4DB),
    /// named, haggled over, paid, made in a free slot and taken out of the
    /// stock.
    fn guild_item_bought(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks, pick: u16) -> Option<()> {
        // 1194:B4DB
        self.map_bank(0x18, 0);
        let frame = self.mem.r16(DS, FRAME);
        let rec = pick.wrapping_mul(0x0B);
        self.memmove(frame, rec, DS, TEMPLATE, 0x0B);
        self.stock_name()?;
        self.mem.w16(DS, NAME_PTR, ITEM_NAME);
        let p = self.mem.r32(DS, TEMPLATE + 5);
        self.mem.w32(DS, PRICE, p);
        self.guild_seller(files)?;
        let mut eax = self.mem.r32(DS, PRICE);
        if self.mem.r8(DS, HOLIDAY) != 0xFF {
            eax = self.holiday_price(eax, 4);
        }
        let quality = self.mem.r8(DS, QUALITY) as u16;
        let factor = self.mem.r8(DS, FACTOR) as u16;
        let per = Self::guild_percent(self.mem.r8(DS, 0x01D3) as u16);
        let (eax, walked) = hooks.haggle(self, eax, quality, factor, per);
        if walked {
            return Some(());
        }
        if self.mem.r32(DS, GOLD) < eax {
            hooks.message(self, text::NOT_ENOUGH);
            return Some(());
        }
        self.map_bank(0x18, 0);
        self.mem.w32(DS, PRICE, eax);
        let si = (self.mem.r8(DS, PICK) as u16).wrapping_mul(0x0B);
        if !self.guild_take_item() {
            return Some(());
        }
        let g = self.mem.r32(DS, GOLD).wrapping_sub(self.mem.r32(DS, PRICE));
        self.mem.w32(DS, GOLD, g);
        self.stock_remove(si)?;
        Some(())
    }

    /// The stock's record at frame:`si` (the pick's) taken out (1194:5ED1
    /// with entries of 0Bh bytes), and the texts made again while any is
    /// left (AX 3BA0h). None where DGROUP's pick (13C8) isn't below the
    /// count (`stock_take_out`).
    fn stock_remove(&mut self, si: u16) -> Option<()> {
        let cx = self.mem.r8(DS, STOCK) as u16;
        self.stock_take_out(si, cx)?;
        let n = self.mem.r8(DS, STOCK).wrapping_sub(1);
        self.mem.w8(DS, STOCK, n);
        if n != 0 {
            self.stock_texts(0x3BA0)?;
        }
        Some(())
    }

    /// 1194:5ED1 for the stock (entries of 0Bh bytes at 100C): the entry
    /// PICK of the `cx` at frame:`si` taken out. None where PICK isn't
    /// below `cx`: the entries after it number 65536 less, and moving them
    /// runs 4-byte copies (0000:7736) across the frame's end, a general
    /// protection fault on the CPU.
    fn stock_take_out(&mut self, si: u16, cx: u16) -> Option<()> {
        if self.mem.r8(DS, PICK) as u16 >= cx {
            return crate::giveup::original("1194:5ED1: a pick past the list: the original faults (#GP)");
        }
        self.mem.w8(DS, 0x100C, 0x0B);
        self.table_remove(si, cx);
        Some(())
    }

    /// 1194:B427. The template's item made in the player's first free
    /// slot (1194:B436) and the items sorted; false (the carry) when
    /// there is none.
    fn guild_take_item(&mut self) -> bool {
        let Ok((si, _)) = self.free_slot(DS, PLAYER) else { return false };
        self.guild_item_record(si);
        self.sort_inventory();
        true
    }

    /// 1194:B436. The item record at DS:`si` made from the template (the
    /// record at 13DA is what is cleared first, whatever `si` is; the
    /// flags are ORed into what is there).
    fn guild_item_record(&mut self, si: u16) {
        for i in 0..0x13u16 {
            self.mem.w8(DS, ITEM + i, 0);
        }
        let t = |m: &Self, f: u16| m.mem.r8(DS, TEMPLATE + f);
        let or = |m: &mut Self, v: u8| {
            let f = m.mem.r8(DS, si.wrapping_add(0x0E)) | v;
            m.mem.w8(DS, si.wrapping_add(0x0E), f);
        };
        or(self, 0x40 | 1 | 8);
        self.mem.w16(DS, si.wrapping_add(6), 1);
        self.mem.w16(DS, si.wrapping_add(8), 1);
        match t(self, 0) {
            0 => {
                self.mem.w8(DS, si.wrapping_add(0x10), t(self, 2));
                let v = t(self, 4);
                self.mem.w8(DS, si.wrapping_add(3), v);
                self.mem.w16(DS, si.wrapping_add(6), v as u16);
                self.mem.w8(DS, si.wrapping_add(0x0F), t(self, 3));
            }
            2 => {
                or(self, 0x20);
                let g = t(self, 4);
                self.mem.w8(DS, si.wrapping_add(0x10), g);
                let v = self.mem.r8(DS, 0x2678u16.wrapping_add(g as u16 + 3));
                self.mem.w8(DS, si.wrapping_add(4), v);
                self.mem.w8(DS, si.wrapping_add(0x0F), 0xFF);
            }
            _ => {
                or(self, 0x10);
                self.mem.w8(DS, si.wrapping_add(0x0F), t(self, 3));
            }
        }
        self.mem.w8(DS, si, t(self, 1));
        let p = (self.mem.r32(DS, TEMPLATE + 5) >> 2).wrapping_mul(3);
        self.mem.w32(DS, si.wrapping_add(0x0A), p);
        self.mem.w8(DS, si.wrapping_add(0x12), t(self, 9));
        self.mem.w8(DS, si.wrapping_add(0x11), t(self, 0x0A));
    }

    /// 1194:B1EA. A spell bought (see the module's notes).
    fn guild_spells(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        hooks.strip_back(self);
        'list: loop {
            // 1194:B1F4 (the loop's B1ED): the gold left.
            hooks.gold_left(self);
            let picked = self.spell_list(hooks);
            // 1194:B1FE, or B31F when none was picked.
            hooks.strip_back(self);
            if !picked {
                return Some(());
            }
            // 1194:A933
            let spell = self.mem.r8(DS, SPELLS.wrapping_add(self.mem.r8(DS, PICK) as u16));
            self.mem.w8(DS, BOUGHT, spell);
            let si = (spell as u16).wrapping_mul(0x55);
            let holiday = self.mem.r8(DS, HOLIDAY) != 0xFF;
            if holiday {
                let ax = self.mem.r16(DATA, si.wrapping_add(0x32));
                self.mem.w16(DS, SAVED_COST, ax);
                let p = self.holiday_price(ax as u32, 2) as u16;
                self.mem.w16(DATA, si.wrapping_add(0x32), p);
            }
            self.guild_colour_102c();
            self.mem.w8(DS, 0x101D, 0xFF);
            // 1194:68A3
            for i in 0..0x55u16 {
                let b = self.mem.r8(DATA, si.wrapping_add(i));
                self.mem.w8(DS, SPELL_COPY + i, b);
            }
            hooks.spell_screen(self);
            self.mem.w8(DS, 0x101D, 0);
            let restore = |m: &mut Self| {
                if holiday {
                    let v = m.mem.r16(DS, SAVED_COST);
                    m.mem.w16(DATA, si.wrapping_add(0x32), v);
                }
            };
            loop {
                let (b, x, y) = hooks.click(self);
                self.mem.w8(DS, 0xAFF9, b);
                self.mem.w16(DS, 0x8F92, x);
                self.mem.w16(DS, 0x8F94, y);
                if b == 0 || !(5..=0x136).contains(&x) || y < 0xB8 {
                    continue;
                }
                if x <= 0x69 {
                    restore(self);
                    continue 'list;
                }
                if x < 0x78 {
                    continue;
                }
                if x <= 0xBE {
                    restore(self);
                    return self.spell_buy(files, hooks);
                }
                if x < 0x11D {
                    continue;
                }
                restore(self);
                return Some(());
            }
        }
    }

    /// The text colours 102C set (0000:9AE6, the front end's).
    fn guild_colour_102c(&mut self) {}

    /// 1194:A87E. The spells sold, sorted, listed and one picked; false
    /// (the carry) when none is.
    fn spell_list(&mut self, hooks: &mut dyn GuildHooks) -> bool {
        let max = (self.mem.r8(DS, LEVEL) as u16).wrapping_mul(0xFA);
        self.mem.w16(DS, COUNT, max);
        let (mut si, mut n) = (SPELLS, 0u8);
        for al in 0..0x80u16 {
            let di = al.wrapping_mul(0x55);
            if self.mem.r8(DATA, di.wrapping_add(0x34)) != 0
                && al <= 0x2E
                && self.mem.r16(DATA, di.wrapping_add(0x32)).wrapping_shl(1) <= max
            {
                n = n.wrapping_add(1);
                self.mem.w8(DS, si, al as u8);
                si = si.wrapping_add(1);
            }
        }
        self.mem.w8(DS, SPELL_COUNT, n);
        self.spell_sort(n);
        let cx = self.mem.r8(DS, SPELL_COUNT) as u16;
        // 1194:A952. With none it returns at once (JCXZ), and the caller
        // tests the sign flag, which the sort's CMP DL, 1 left set: as if
        // nothing were picked.
        if cx == 0 {
            return false;
        }
        let mut di = NAMES;
        for i in 0..cx {
            let al = self.mem.r8(DS, SPELLS.wrapping_add(i));
            di = self.spell_name_copy(al, di);
        }
        self.mem.w8(DS, di, 0);
        self.mem.w16(DS, TEXT, TEXTS);
        for i in 0..cx {
            let spell = self.mem.r8(DS, SPELLS.wrapping_add(i)) as u16;
            let mut eax = self.mem.r16(DATA, spell.wrapping_mul(0x55).wrapping_add(0x32)).wrapping_shl(1) as u32;
            if self.mem.r8(DS, HOLIDAY) != 0xFF {
                eax = self.holiday_price(eax, 2);
            }
            let name = self.skip_strings(DS, NAMES, i);
            let di = self.mem.r16(DS, TEXT);
            let end = self.sprintf((DS, di), (DS, text::PRICE_LINE), &[Arg::Int(eax as u16), Arg::Str(DS, name)]);
            self.mem.w16(DS, TEXT, self.guild_scan(DS, di, end));
        }
        // AX Bh (1194:A9F9): the list's kind (1194:5734).
        self.list_kind = 0xB;
        let pick = hooks.list(self, TEXTS, cx, 0);
        self.mem.w8(DS, PICK, pick as u8);
        (pick as i16) >= 0
    }

    /// 29CF:0650: spell `al`'s name (3276:34h + 55h × number; 2AB4h for
    /// numbers from 80h, less 80h) copied to DS:`di`; DI past its NUL.
    fn spell_name_copy(&mut self, al: u8, mut di: u16) -> u16 {
        let (base, n) = if al & 0x80 != 0 { (0x2AB4u16, al & 0x7F) } else { (0x34, al) };
        let mut si = base.wrapping_add((n as u16).wrapping_mul(0x55));
        loop {
            let b = self.mem.r8(DATA, si);
            self.mem.w8(DS, di, b);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if b == 0 {
                return di;
            }
        }
    }

    /// 1194:AA07. The spells sold sorted by price (twice their cost, in
    /// 16 bits), as the original's exchange sort runs (its counter at
    /// 12BC).
    fn spell_sort(&mut self, dl: u8) {
        if dl <= 1 {
            return;
        }
        let price = |m: &Self, at: u16| {
            let s = m.mem.r8(DS, at) as u16;
            m.mem.r16(DATA, s.wrapping_mul(0x55).wrapping_add(0x32)).wrapping_shl(1)
        };
        self.mem.w8(DS, SORT_I, 0);
        let mut dl = dl - 1;
        let mut cx = dl as u16;
        let mut si = SPELLS;
        loop {
            let mut di = si;
            let mut bx = price(self, si);
            loop {
                di = di.wrapping_add(1);
                if price(self, di) < bx {
                    let (a, b) = (self.mem.r8(DS, si), self.mem.r8(DS, di));
                    self.mem.w8(DS, si, b);
                    self.mem.w8(DS, di, a);
                    bx = price(self, si);
                }
                let i = self.mem.r8(DS, SORT_I).wrapping_add(1);
                self.mem.w8(DS, SORT_I, i);
                if i < dl {
                    continue;
                }
                break;
            }
            self.mem.w8(DS, SORT_I, 0);
            dl = dl.wrapping_sub(1);
            si = si.wrapping_add(1);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                return;
            }
        }
    }

    /// 1194:A8E4. The spell on the screen bought: its price twice its
    /// cost (57E6 + 32h), on a holiday the undiscounted cost (also put
    /// back there); haggled over, paid, and added to the player's list.
    fn spell_buy(&mut self, files: &mut Files, hooks: &mut dyn GuildHooks) -> Option<()> {
        let mut ax = self.mem.r16(DS, SPELL_COPY + 0x32).wrapping_shl(1);
        if self.mem.r8(DS, HOLIDAY) != 0xFF {
            ax = self.mem.r16(DS, SAVED_COST);
            self.mem.w16(DS, SPELL_COPY + 0x32, ax);
        }
        self.mem.w32(DS, PRICE, ax as u32);
        self.mem.w16(DS, NAME_PTR, SPELL_COPY + 0x34);
        let (eax, walked) = self.guild_haggle(files, hooks)?;
        if walked {
            return Some(());
        }
        if self.mem.r32(DS, GOLD) < eax {
            hooks.message(self, text::NOT_ENOUGH);
            return Some(());
        }
        let g = self.mem.r32(DS, GOLD).wrapping_sub(eax);
        self.mem.w32(DS, GOLD, g);
        let n = self.mem.r8(DS, SPELL_KNOWN);
        let b = self.mem.r8(DS, BOUGHT);
        self.mem.w8(DS, SPELL_KNOWN + 1 + n as u16, b);
        self.mem.w8(DS, SPELL_KNOWN, n.wrapping_add(1));
        Some(())
    }

    /// 1194:FE0E. An item of the player's picked: their names listed
    /// from 9A6E (to the first empty slot); "nothing" (19C5) without any.
    /// The item's offset, or None (the carry). None outside (`Err`)
    /// where a name can't be made.
    fn guild_pick_item(&mut self, hooks: &mut dyn GuildHooks) -> Result<Option<u16>, ()> {
        self.mem.w16(DS, PICK_NEXT, PICK_TEXTS);
        let mut si = ITEMS;
        let mut n = 0u16;
        for _ in 0..0x28 {
            if self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x40 == 0 {
                break;
            }
            let name = self.display_name(si).ok_or(())?;
            let di = self.mem.r16(DS, PICK_NEXT);
            self.guild_strcpy(name, di);
            // 1194:FBE9: the scan from DI for the NUL (200 bytes at most).
            let mut d = di;
            for _ in 0..0xC8 {
                let b = self.mem.r8(DS, d);
                d = d.wrapping_add(1);
                if b == 0 {
                    break;
                }
            }
            self.mem.w16(DS, PICK_NEXT, d);
            si = si.wrapping_add(0x13);
            n += 1;
        }
        if n == 0 {
            hooks.message(self, text::NO_ITEMS);
            return Ok(None);
        }
        // AX Bh (1194:FE4E): the list's kind (1194:5734).
        self.list_kind = 0xB;
        let pick = hooks.list(self, PICK_TEXTS, n, 0);
        if (pick as i16) < 0 {
            return Ok(None);
        }
        self.mem.w8(DS, PICK, pick as u8);
        Ok(Some(ITEMS.wrapping_add((self.mem.r8(DS, PICK) as u16).wrapping_mul(0x13))))
    }

    /// 1194:AB39. Detect Magic (see the module's notes).
    fn guild_detect(&mut self, hooks: &mut dyn GuildHooks) -> Option<()> {
        let Some(si) = self.guild_pick_item(hooks).ok()? else { return Some(()) };
        self.mem.w16(DS, IDENTIFY_ITEM, si);
        if self.mem.r8(DS, si.wrapping_add(0x0E)) & 2 == 0 {
            hooks.message(self, text::KNOWN);
            return Some(());
        }
        let mut eax = self.mem.r32(DS, si.wrapping_add(0x0A));
        eax = if eax > 0x0A { eax / 0x0A } else { 1 };
        let mut pay = true;
        if self.mem.r8(DS, HOLIDAY) == 1 {
            eax = self.holiday_price(eax, 3);
            if eax == 0 {
                self.guild_colour();
                hooks.message_box(self, text::FREE);
                pay = false;
            }
        }
        if pay {
            self.mem.w32(DS, PRICE, eax);
            self.sprintf((DS, 0x1044), (DS, text::DETECT), &[Arg::Long(self.mem.r32(DS, PRICE))]);
            hooks.picture(self, text::DETECT_PICTURE);
            self.mem.w8(DS, 0x137E, 3);
            self.split_lines(0x1044);
            self.mem.w8(DS, 0x13D3, 1);
            if hooks.dialog(self, 1, 0x1044) != 0 {
                return Some(());
            }
            let eax = self.mem.r32(DS, PRICE);
            if self.mem.r32(DS, GOLD) < eax {
                hooks.message(self, text::NOT_ENOUGH);
                return Some(());
            }
            let g = self.mem.r32(DS, GOLD).wrapping_sub(eax);
            self.mem.w32(DS, GOLD, g);
        }
        // 1194:ABD0
        self.guild_colour();
        hooks.message_box(self, text::IDENTIFIED);
        let si = self.mem.r16(DS, IDENTIFY_ITEM);
        let flag = |m: &mut Self, set: bool| {
            let f = m.mem.r8(DS, si.wrapping_add(0x0E));
            m.mem.w8(DS, si.wrapping_add(0x0E), if set { f | 2 } else { f & !2 });
        };
        flag(self, false);
        if self.item_category(DS, si) != 3 {
            return Some(());
        }
        flag(self, true);
        match self.find_stack(self.mem.r8(DS, si) as u16) {
            Some(found) => {
                let n = self.mem.r16(DS, found.wrapping_add(1)).wrapping_add(1);
                self.mem.w16(DS, found.wrapping_add(1), n);
                let index = si.wrapping_sub(ITEMS) / 0x13;
                self.remove_item(DS, si, index);
            }
            None => flag(self, false),
        }
        Some(())
    }

    /// 1194:AC2A. Stealing (see the module's notes). The carry: the
    /// guards called.
    fn guild_steal(&mut self, hooks: &mut dyn GuildHooks) -> Option<bool> {
        let mut pick = 0u16;
        if self.mem.r8(DS, STOCK) != 0 {
            pick = hooks.menu(self, BUY_TITLE, BUY_KEYS_PLAIN, BUY_ENTRIES_PLAIN);
        }
        self.mem.w8(DS, PICK, pick as u8);
        if (pick as i16) < 0 {
            return Some(false);
        }
        let (ok, chance) = self.lockpick()?;
        if ok {
            self.steal_take(hooks)?;
            return Some(false);
        }
        self.guild_colour();
        hooks.message_box(self, text::STEAL_FAILED);
        let cx = 100u16.wrapping_sub(chance);
        if self.rng.next() % 100 >= cx {
            return Some(false);
        }
        self.call_guards();
        self.mem.w8(DS, 0x191D, 1);
        self.clear_dungeon_flats();
        Some(true)
    }

    /// 1194:AC79. A potion of a random type, or a random item of the
    /// stock (taken out of it), stolen: "You stole %s" (1194:FDB6), or
    /// the inventory's full.
    fn steal_take(&mut self, hooks: &mut dyn GuildHooks) -> Option<()> {
        self.map_bank(0x18, 0);
        if self.mem.r8(DS, PICK) == 0 {
            // 1194:ACFE
            self.mem.w16(DS, COUNT, 1);
            let ty = self.rng.range(0, 0x0F)?;
            self.mem.w8(DS, PICK, ty as u8);
            self.mem.w8(DS, MAKING, 1);
            let added = self.potions_add();
            self.mem.w8(DS, MAKING, 0);
            let Some(si) = added else {
                self.guild_colour();
                hooks.message_box(self, text::FULL);
                return Some(());
            };
            self.stolen(hooks, si)?;
            self.sort_inventory();
            return Some(());
        }
        let n = self.mem.r8(DS, STOCK) as u16;
        let ax = self.rng.range(0, n)?;
        self.mem.w8(DS, PICK, ax as u8);
        let frame = self.mem.r16(DS, FRAME);
        let rec = ax.wrapping_mul(0x0B);
        self.memmove(frame, rec, DS, TEMPLATE, 0x0B);
        self.mem.w8(DS, MAKING, 1);
        self.guild_item_record(ITEM);
        self.mem.w8(DS, MAKING, 0);
        self.stolen(hooks, ITEM)?;
        if !self.guild_take_item() {
            self.guild_colour();
            hooks.message_box(self, text::FULL);
            return Some(());
        }
        let n = self.mem.r8(DS, STOCK) as u16;
        self.stock_take_out(rec, n)?;
        let n = self.mem.r8(DS, STOCK).wrapping_sub(1);
        self.mem.w8(DS, STOCK, n);
        if n != 0 {
            let ax = self.mem.r16(DS, FRAME);
            self.stock_texts(ax)?;
        }
        Some(())
    }

    /// 1194:7388 then FDB6: "You stole %s" for the item at `si`, in a
    /// message box; bank 18h mapped.
    fn stolen(&mut self, hooks: &mut dyn GuildHooks, si: u16) -> Option<()> {
        let name = self.display_name(si)?;
        self.sprintf((DS, 0x1044), (DS, text::STOLEN), &[Arg::Str(DS, name)]);
        self.guild_colour();
        hooks.message_box(self, 0x1044);
        self.map_bank(0x18, 0);
        Some(())
    }
}
