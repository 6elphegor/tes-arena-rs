//! The equipment store (1194:F48E, `equipment_store`), the shop of
//! building kind 6 (the table at 869F: 4 B93C the tavern, 5 C258 the
//! temple, 7 A73E the mages' guild, 6 F48E this).
//!
//! Unless a fight is on (FFB) or the guards are coming (680F), the store
//! is set up (1194:FA03), then its menu (Buy, Sell, Repair, Steal, Exit)
//! runs until "Exit", nothing picked, or a routine returns the carry (the
//! steal that called the guards). On leaving (1194:F526, run even when
//! the store wasn't set up) its stock and its repairs are written to
//! their files.
//!
//! - The set-up (1194:FA03): the shop's scratch cleared (1194:FDDD), the
//!   screen (`ShopHooks::shop_screen`), the greeting ("%en " expanded,
//!   `ShopHooks::expand`, and shown, `StoreHooks::greeting`), the quests' check (`ShopHooks::quests`),
//!   today a holiday for this shop or not (13C9), the files' names made
//!   from the place's seed (1194:FBA2: the seed's digits and ".64", the
//!   first two letters "eq" for the stock, "re" for the repairs, each
//!   in the save directory, 25FD:0844), the repairs read (1194:C8C0) and
//!   the stock: read from its file (1194:FB14) when the file is there and
//!   less than a day old (its first dword, the time it was written), the
//!   level and price factors rolled from the shop's seed (1194:FB70);
//!   otherwise made anew (1194:FAD9, `Machine::shop_open`).
//! - Buy (1194:F7A1): with a free slot, from the weapons (EMS frame:0,
//!   12C6 of them) or the armour (frame:7D0, 12CF), asked when there are
//!   both (1194:FB89); the list's lines are the front end's
//!   (`StoreHooks::weapon_lines`, `armor_lines`: 1194:F142, F204). The
//!   record picked becomes the template (13F3), its price (on a holiday
//!   for this shop, 1194:4A6B with service 2 or 3) the price asked, its
//!   name made (1194:F768). Haggled over (1194:8020, mode 1, with the
//!   shopkeeper's name from the place's seed, 1194:080B, and the player's
//!   personality, `StoreHooks::haggle`), paid if the gold is there and
//!   the item not too heavy (1194:F961, 4D99); a magic record leaves its
//!   list (1194:5ED1), plain ones stay; the item is made into the free
//!   slot (1194:F031 or EEE2 through SI, with the price the haggle
//!   agreed), and the items sorted (1194:7911).
//! - Sell (1194:F5DB): one of the player's items (listed up to the first
//!   empty slot, 1194:FE0E), not a special one (type 80h up) nor a broken
//!   weapon or armour, an equipped one only if confirmed (1194:F6BC). Its
//!   price: for weapons and armour the price times the condition over
//!   its maximum, for kinds 2 and 3 a quarter of the price (kind 3 any
//!   price makes 100), at least 1; haggled over (1194:09FC, mode 4). Sold:
//!   it is no longer occupied, the weapon class made again if it was
//!   equipped, the gold added; a magic weapon or armour joins the store's
//!   list (1194:FC02, F426, F6EC: its own price from the tables at
//!   22D4/24F4, sorted in, 1194:F37B), and it leaves the inventory
//!   (1194:788D).
//! - Repair (1194:C6AC, `store_repair`): the repairs, five records of
//!   24 bytes at REPAIRS (+0 taken, +1 the dword minute it is ready, +5
//!   the item, 13h bytes). With one on file, newold.img: "Which job do
//!   you wish to inspect?" (a yes or no: 0 a new one, 2 leave). An old
//!   one picked from their names' list (1194:CA02): not ready, the time
//!   left told (1194:C8FB: days at a day or more, hours past 6 hours,
//!   else "any minute"; texts 58Ch, 58Dh, 58Bh); ready, "Your %s is
//!   ready.", its condition made whole (its maximum, +8, into +6; for an
//!   item of category 2 and kind 0, +6's low byte into +3), or a quest
//!   piece made again (2686:06E1: 1194:D316, D1F1); taken if not too
//!   heavy (4D99, below type 8Dh) into the first free slot (copied, from
//!   ITEM for a quest piece, unequipped), its record removed (1194:C9CC)
//!   and the items sorted. A new one: with a record free ("Sorry, I
//!   already have my hands full..." else), an item picked (1194:FE0E);
//!   one already whole "does not need any repairing" (CA7F); else priced
//!   (1194:C83B, its +0Ah price: a tenth in gold for 10 days, or 1 gold
//!   for price days under 10; text 590h with %a the gold, %t the days and
//!   %i its name, 1194:CCFD) and agreed (yes: paid, 1194:CAF6, ready
//!   days * 5A0h minutes on, a 16-bit product), or a menu of three: the
//!   gold typed (CB40: the days the price over it), the days typed (CBD3:
//!   the gold the price over them), or no (CC77); filed (CA68) and the
//!   item taken out of the inventory (the weapon class made again when
//!   it was equipped), the items sorted.
//! - Steal (1194:FCC6): with a free slot, the lockpicking roll against
//!   the shop's level (1194:53F9); done, a random record of the weapons
//!   or the armour (0000:6EEA) is made an item, named ("You successfully
//!   stole a %s."), made again into the free slot and the items sorted
//!   (the record stays in the list); failed, "You are unsuccessful...",
//!   and the guards' roll may call them (0000:3F6E, 3EBE), the building
//!   broken into (191D) and its people gone (0000:50B9): the store left.
//!
//! The screen's routines are the front end's (nothing in the port; the
//! generator makes them return at once: 0000:8FC4, 77EC, 9466, 541E,
//! 9AE6, 9AF8, 1194:5496, 54B3, 4B60, 5F8C, 545C, 5FC0, 5E21), as are the
//! menus, lists, dialogs, messages and the gold left (545C, called where
//! the original calls it) (`ShopHooks`) and the greeting,
//! the message boxes, the haggle, the lists' lines, the repairs'
//! pictures and texts (`StoreHooks`). The files are the port's
//! (`Files`: kept in memory).
//! Assumed: on entering AX and DI are 0 (the bank put back on leaving and
//! where 1194:FDDD's fill starts are what the caller left there); EMS is
//! off or the frame's bank is the store's (0000:5C86 then changes
//! nothing the vectors see).
//!
//! Bugs kept: a record's price for selling back (1194:FC78) with no
//! enchantment (+0Fh FFh) is the type byte, or the material's scaled
//! price, not the table's price plus it; a stolen record stays for sale.

use crate::bsa::Files;
use crate::format::Arg;
use crate::loot::{ITEM, TEMPLATE, template as t};
use crate::machine::Machine;
use crate::mem::DS;
use crate::shops::font;
use crate::shops::ShopHooks;

/// What the store leaves to the front end, besides `ShopHooks`.
pub trait StoreHooks: ShopHooks {
    /// 1194:4BFF: the greeting at DS:`si` shown, on the screen
    /// `ShopHooks::shop_screen` drew.
    fn greeting(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:59CC: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:8020: the price `eax` haggled over in `mode` (1 buying, 4
    /// selling), the player's personality `per` (0..100), the
    /// shopkeeper's name at DS:`name`, the shop's quality and factor at
    /// 13CA, 13CB: the price agreed, or the carry when the player walks
    /// away.
    fn haggle(&mut self, _m: &mut Machine, _mode: u8, eax: u32, _per: u16, _name: u16) -> (u32, bool) {
        (eax, true)
    }
    /// 1194:F142: the weapons' lines for the list (at frame:FA0;
    /// `Machine::store_lines`).
    fn weapon_lines(&mut self, m: &mut Machine) -> Option<()> {
        m.store_lines(true)
    }
    /// 1194:F204: the armour's lines for the list (at frame:2710).
    fn armor_lines(&mut self, m: &mut Machine) -> Option<()> {
        m.store_lines(false)
    }
    /// 1194:C6AC in a generator that stubs it: Some(the carry) takes the
    /// repairs' place; None (the default) runs them (`store_repair`).
    fn repair_instead(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<bool> {
        None
    }
    /// 1194:A374 with AX 2: the text numbered at 137A written to DS:`di`
    /// (no substitutions, 101C FFh): the text port (`Machine::text`).
    fn text(&mut self, m: &mut Machine, files: &mut Files, di: u16) -> Option<()> {
        m.text(files, &mut crate::text::NoTextHooks, 2, DS, di).map(|_| ())
    }
}

/// `StoreHooks` that do nothing: every menu left at once.
pub struct NoStoreHooks;

impl ShopHooks for NoStoreHooks {}
impl StoreHooks for NoStoreHooks {}

/// DGROUP variables.
pub mod var {
    /// The menu's title, keys, entries and routines; Buy's.
    pub const MENU_TITLE: u16 = 0x8A74;
    pub const MENU_KEYS: u16 = 0x8A85;
    pub const MENU_ENTRIES: u16 = 0x8A8B;
    pub const ROUTINES: u16 = 0x8ACD;
    pub const BUY_TITLE: u16 = 0x8AD5;
    pub const BUY_KEYS: u16 = 0x8AE5;
    pub const BUY_ENTRIES: u16 = 0x8AE8;
    /// The EMS bank mapped before (put back on leaving).
    pub const BANK: u16 = 0x8B27;
    /// The files' names: the extension, the stock's and the repairs'.
    pub const EXTENSION: u16 = 0x8B05;
    pub const STOCK_FILE: u16 = 0x8B09;
    pub const REPAIRS_FILE: u16 = 0x8B18;
    /// The repairs: five records of 24 bytes.
    pub const REPAIRS: u16 = 0x76E3;
    /// The weapons' and armour's counts, and their copies.
    pub const WEAPONS: u16 = 0x12C6;
    pub const ARMOR: u16 = 0x12CF;
    pub const WEAPONS_READ: u16 = 0x12DB;
    pub const ARMOR_READ: u16 = 0x12DC;
    /// Buying a weapon (1) or armour (0).
    pub const BUYING_WEAPON: u16 = 0x12DE;
    /// The pick, today a holiday (1; FFh not), the shop kind.
    pub const PICK: u16 = 0x13C8;
    pub const HOLIDAY: u16 = 0x13C9;
    pub const SHOP: u16 = 0x137F;
    /// The shop's seed (dword).
    pub const SEED: u16 = 0x12E0;
    /// The name shown (a pointer), its buffer; the haggles' names.
    pub const NAME_AT: u16 = 0x1376;
    pub const NAME: u16 = 0x1395;
    pub const BUY_NAME: u16 = 0x11D4;
    pub const SELL_NAME: u16 = 0x1A4C;
    /// The price asked (dword); a kind 2 or 3 item's resale price.
    pub const PRICE: u16 = 0x1391;
    pub const SALE: u16 = 0x8A70;
    /// The player's items' names for Sell's list, and where the next goes.
    pub const SELL_LIST: u16 = 0x9A6E;
    pub const SELL_NEXT: u16 = 0x12D5;
    /// The template's category while a sold item is recorded.
    pub const CATEGORY: u16 = 0x13F2;
}
use var::*;

const GOLD: u16 = 0x05C2;
const TIME: u16 = 0x05CC;
const PLAYER: u16 = crate::character::PLAYER;
const COLOUR: u16 = 0x9288;
const SCRATCH: u16 = 0x1044;
const FRAME: u16 = 0x9030;
const ENTRY: u16 = 0x100C;
const WEAPON_LIST: u16 = 0x0000;
const ARMOR_LIST: u16 = 0x07D0;
/// The seed's digits and extension, before they are copied.
const DIGITS: u16 = 0x10A8;

/// Texts.
mod text {
    pub const GREETING: u16 = 0x929E;
    pub const NOT_ENOUGH: u16 = 0x198C;
    pub const INVENTORY_FULL: u16 = 0x19DE;
    pub const INVENTORY_EMPTY: u16 = 0x19C5;
    pub const NOT_INTERESTED: u16 = 0x1A0D;
    pub const UNSUCCESSFUL: u16 = 0x20FA;
    pub const STOLE: u16 = 0x8B29;
    pub const STILL_SELL: u16 = 0x8B46;
    pub const YES_NO: u16 = 0x8B7D;
    pub const YES_NO_ENTRIES: u16 = 0x8B80;
}

/// `LootHooks` for 1194:4D99's message box, through the store's.
struct Heavy<'a>(&'a mut dyn StoreHooks);

impl crate::loot_window::LootHooks for Heavy<'_> {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.0.message_box(m, si);
    }
}

impl Machine {
    /// 1194:F48E. See the module's notes. None where the original faults
    /// or hangs, or a routine isn't ported.
    pub fn equipment_store(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<()> {
        // Where the port gives up (a hook with no answer, a routine not
        // ported), the end's state is left all the same: no shop, no
        // holiday, the clip as it was, and the bank mapped before the
        // store's (kept at BANK) mapped back, so the store's bank (18h)
        // doesn't stand in for it. Its stock file isn't written. The
        // original has no such exit.
        let saved = self.mem.r16(DS, 0xA91A);
        let entered = self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x680F) == 0;
        let r = self.equipment_store_body(files, hooks);
        if r.is_none() {
            self.mem.w8(DS, SHOP, 0xFF);
            self.mem.w8(DS, HOLIDAY, 0xFF);
            self.mem.w16(DS, 0xA91A, saved);
            if entered {
                let bank = self.mem.r16(DS, BANK);
                self.map_bank(bank, 0);
            }
        }
        r
    }

    fn equipment_store_body(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<()> {
        let saved = self.mem.r16(DS, 0xA91A);
        self.mem.w8(DS, SHOP, 6);
        if self.mem.r16(DS, 0x0FFB) == 0 && self.mem.r8(DS, 0x680F) == 0 {
            hooks.screen_saved(self, files);
            let bank = self.map_bank(0x18, 0);
            self.mem.w16(DS, BANK, bank);
            self.store_enter(files, hooks)?;
            loop {
                self.map_bank(0x18, 0);
                // 1194:F4D5: FONT_A and the tiles at 1036 selected.
                self.select_font(font::A);
                self.select_tiles(0x1036);
                let pick = hooks.menu(self, MENU_TITLE, MENU_KEYS, MENU_ENTRIES);
                if (pick as i16) < 0 || pick == 4 {
                    break;
                }
                let cf = match self.mem.r16(DS, ROUTINES.wrapping_add(pick.wrapping_add(pick))) {
                    0xF7A1 => self.store_buy(files, hooks)?,
                    0xF5DB => self.store_sell(files, hooks)?,
                    0xC6AC => match hooks.repair_instead(self, files) {
                        Some(cf) => cf,
                        None => self.store_repair(files, hooks)?,
                    },
                    0xFCC6 => self.store_steal(hooks)?,
                    _ => return crate::giveup::not_ported("1194:F48E: a store choice not in the table"),
                };
                if cf {
                    break;
                }
            }
        }
        self.store_leave(files)?;
        self.mem.w8(DS, SHOP, 0xFF);
        self.mem.w8(DS, HOLIDAY, 0xFF);
        self.mem.w16(DS, 0xA91A, saved);
        hooks.screen_back(self, files);
        let bank = self.mem.r16(DS, BANK);
        self.map_bank(bank, 0);
        Some(())
    }

    /// 0000:9CA0 within DGROUP.
    fn store_strcpy(&mut self, mut si: u16, mut di: u16) -> u16 {
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return di;
            }
        }
    }

    /// 1194:FA03. See the module's notes.
    fn store_enter(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<()> {
        // 1194:FDDD: the scratch cleared (the frame's fill from DI, 0 here).
        for at in [0x12D0, 0x12DB, 0x12DC, 0x13F1, 0x13EF, 0x13F0] {
            self.mem.w8(DS, at, 0);
        }
        let frame = self.mem.r16(DS, FRAME);
        for i in 0..0x2710u16 {
            self.mem.w8(frame, i, 0);
        }
        for i in 0..0xC8u16 {
            self.mem.w8(DS, SCRATCH.wrapping_add(i), 0);
        }
        self.mem.w8(DS, 0x717F, 0);
        // 1194:FA06: the screen (4B60), then "%en " made and on the
        // banner (4BFF), as the temple's.
        hooks.shop_screen(self);
        self.mem.w32(DS, text::GREETING, 0x206E_6525);
        self.mem.w8(DS, text::GREETING + 4, 0);
        hooks.expand(self, text::GREETING);
        hooks.greeting(self, text::GREETING);
        hooks.quests(self, files)?;
        self.map_bank(0x18, 0);
        // 1194:FA38: ARENAFNT selected.
        self.select_font(font::ARENA);
        self.mem.w8(DS, HOLIDAY, 0xFF);
        if self.shop_holiday() {
            self.mem.w8(DS, HOLIDAY, 1);
        }
        let si = self.store_file_name();
        self.store_strcpy(si, STOCK_FILE);
        self.mem.w16(DS, STOCK_FILE, 0x7165);
        let si = self.store_file_name();
        self.store_strcpy(si, REPAIRS_FILE);
        self.mem.w16(DS, REPAIRS_FILE, 0x6572);
        // 1194:C8C0: the repairs read, when their file is there.
        let si = self.save_path(REPAIRS_FILE);
        if self.file_find(files, si) {
            let si = self.save_path(REPAIRS_FILE);
            self.file_open(files, si);
            self.file_load(files, si, DS, REPAIRS);
        }
        let si = self.save_path(STOCK_FILE);
        if self.file_find(files, si) {
            let si = self.save_path(STOCK_FILE);
            self.file_open(files, si);
            self.file_read(files, DS, SCRATCH, 4);
            let written = self.mem.r32(DS, SCRATCH);
            if self.mem.r32(DS, TIME).wrapping_sub(written) < 0x5A0 {
                self.store_read(files);
                self.file_close(files);
                // 1194:FB70: the level and factors from the shop's seed.
                let saved = self.rng.seed;
                self.rng.seed = self.mem.r32(DS, SEED);
                // The game's seed back even where the port gives up.
                let r = self.shop_roll_level().and_then(|level| self.shop_price_factors(level));
                self.rng.seed = saved;
                r?;
                return Some(());
            }
            self.file_close(files);
        }
        self.shop_open()
    }

    /// 1194:FBA2. The place's seed (the word of the places' list entry for
    /// FD2's low byte halved, 1194:3EB1) into SEED, its digits and the
    /// extension (five bytes from 8B05) at 10A8; returns 10A8.
    fn store_file_name(&mut self) -> u16 {
        let key = self.mem.r16(DS, 0x0FD2);
        let key = (key & 0xFF00) | ((key & 0xFF) >> 1);
        let (off, seg) = (self.mem.r16(DS, 0x1008), self.mem.r16(DS, 0x100A));
        let si = self.place_lookup(seg, off, key, self.mem.r16(DS, 0x138B));
        let seed = self.mem.r16(seg, si) as u32;
        self.mem.w32(DS, SEED, seed);
        self.dun_ultoa(seed, DIGITS);
        let mut di = DIGITS;
        let mut n = 0x64u16;
        while n != 0 {
            n -= 1;
            let c = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        let di = di.wrapping_sub(1);
        for i in 0..5 {
            let c = self.mem.r8(DS, EXTENSION.wrapping_add(i));
            self.mem.w8(DS, di.wrapping_add(i), c);
        }
        DIGITS
    }

    /// 1194:FB14. The stock read from the open file: the armour's count
    /// and records, then the weapons'.
    fn store_read(&mut self, files: &mut Files) {
        self.map_bank(0x18, 0);
        let frame = self.mem.r16(DS, FRAME);
        self.file_read(files, DS, ARMOR, 1);
        let n = self.mem.r8(DS, ARMOR);
        self.mem.w8(DS, ARMOR_READ, n);
        self.file_read(files, frame, ARMOR_LIST, (n as u16).wrapping_mul(13));
        self.file_read(files, DS, WEAPONS, 1);
        let n = self.mem.r8(DS, WEAPONS);
        self.mem.w8(DS, WEAPONS_READ, n);
        self.file_read(files, frame, WEAPON_LIST, (n as u16).wrapping_mul(13));
        self.file_close(files);
    }

    /// 1194:C8E5: whether any repair is on file (a record's first byte).
    fn store_repairs(&self) -> bool {
        (0..5u16).any(|i| self.mem.r8(DS, REPAIRS + i * 0x18) != 0)
    }

    /// 1194:F526. The stock written (the time, the armour, the weapons;
    /// over the file from its start, created when it isn't there), then
    /// the repairs written and cleared, or their file deleted when there
    /// are none.
    fn store_leave(&mut self, files: &mut Files) -> Option<()> {
        self.map_bank(0x18, 0);
        let si = self.save_path(STOCK_FILE);
        if !self.file_open_or_device(files, si)? {
            self.file_create(files, si);
        }
        let frame = self.mem.r16(DS, FRAME);
        self.file_put(files, DS, TIME, 4);
        self.file_put(files, DS, ARMOR, 1);
        let n = (self.mem.r8(DS, ARMOR) as u16).wrapping_mul(13);
        self.file_put(files, frame, ARMOR_LIST, n);
        self.file_put(files, DS, WEAPONS, 1);
        let n = (self.mem.r8(DS, WEAPONS) as u16).wrapping_mul(13);
        self.file_put(files, frame, WEAPON_LIST, n);
        self.file_close(files);
        let any = self.store_repairs();
        let si = self.save_path(REPAIRS_FILE);
        if !any {
            self.file_delete(files, si);
            return Some(());
        }
        if !self.file_open_or_device(files, si)? {
            let si = self.save_path(REPAIRS_FILE);
            self.file_create(files, si);
        }
        // 1194:C8A0.
        if self.store_repairs() {
            self.file_put(files, DS, REPAIRS, 0x78);
            for i in 0..0x78u16 {
                self.mem.w8(DS, REPAIRS + i, 0);
            }
        }
        self.file_close(files);
        Some(())
    }

    /// 1194:F7A1. Buying; see the module's notes. The carry (never set).
    fn store_buy(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<bool> {
        if self.free_slot(DS, PLAYER).is_err() {
            hooks.message_box(self, text::INVENTORY_FULL);
            return Some(false);
        }
        // 1194:F7AE: the gold left.
        hooks.gold_left(self);
        let weapons = if self.mem.r8(DS, ARMOR) == 0 {
            true
        } else if self.mem.r8(DS, WEAPONS) == 0 {
            false
        } else {
            let pick = hooks.menu(self, BUY_TITLE, BUY_KEYS, BUY_ENTRIES);
            if (pick as i16) < 0 {
                // 1194:F954
                hooks.strip_back(self);
                return Some(false);
            }
            pick == 0
        };
        let (list, count, service) = if weapons { (WEAPON_LIST, WEAPONS, 2) } else { (ARMOR_LIST, ARMOR, 3) };
        self.mem.w8(DS, BUYING_WEAPON, weapons as u8);
        if weapons {
            hooks.weapon_lines(self)?;
        } else {
            hooks.armor_lines(self)?;
        }
        let n = self.mem.r8(DS, count) as u16;
        // 1194:F7D7, F854: TEENYFNT selected.
        self.select_font(font::TEENY);
        let lines = if weapons { 0x0FA0 } else { 0x2710 };
        // AX: 1 the weapons' list (1194:F7E9), 2 the armour's (F866).
        self.list_kind = if weapons { 1 } else { 2 };
        // DI as 1194:F142 or F204 leaves it: the lines' end (12CB, 12CD).
        let di = self.mem.r16(DS, if weapons { 0x12CB } else { 0x12CD });
        let pick = hooks.list(self, lines, n, di);
        if (pick as i16) < 0 {
            // 1194:F954
            hooks.strip_back(self);
            return Some(false);
        }
        self.mem.w8(DS, PICK, pick as u8);
        self.map_bank(0x18, pick);
        let frame = self.mem.r16(DS, FRAME);
        let si = list.wrapping_add(pick.wrapping_mul(13));
        self.memmove(frame, si, DS, TEMPLATE, 13);
        let price = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, PRICE, price);
        let price = if self.mem.r8(DS, HOLIDAY) == 1 { self.holiday_price(price, service) } else { price };
        self.mem.w32(DS, PRICE, price);
        self.store_template_name()?;
        // 1194:F8C3
        hooks.strip_back(self);
        let name = self.shopkeeper_name(files, BUY_NAME)?;
        self.mem.w16(DS, NAME_AT, NAME);
        let per = self.store_personality();
        let price = self.mem.r32(DS, PRICE);
        let (eax, cf) = hooks.haggle(self, 1, price, per, name);
        if cf {
            return Some(false);
        }
        if self.mem.r32(DS, GOLD) < eax {
            hooks.message(self, text::NOT_ENOUGH);
            return Some(false);
        }
        let w = self.store_weight();
        if self.carry_check(&mut Heavy(hooks), w)? {
            return Some(false);
        }
        let gold = self.mem.r32(DS, GOLD).wrapping_sub(eax);
        self.mem.w32(DS, GOLD, gold);
        self.mem.w32(DS, TEMPLATE + t::PRICE, eax);
        let pick = self.mem.r8(DS, PICK) as u16;
        // 1194:F9A2 / F9D2: a magic record leaves its list.
        let si = list.wrapping_add(pick.wrapping_mul(13));
        if self.mem.r8(frame, si.wrapping_add(t::MAGIC)) == 1 {
            let n = self.mem.r8(DS, count) as u16;
            self.mem.w8(DS, ENTRY, 13);
            self.table_remove(si, n);
            let n = self.mem.r8(DS, count).wrapping_sub(1);
            self.mem.w8(DS, count, n);
        }
        let slot = self.free_slot(DS, PLAYER).map_or_else(|end| end, |(s, _)| s);
        self.mem.w8(DS, 0x13EF, 0);
        if weapons {
            // 1194:F36C.
            self.finish_weapon_at(slot);
        } else {
            // 1194:F35A, which sorts the items too.
            self.finish_armor_at(slot);
            self.sort_inventory();
        }
        self.sort_inventory();
        Some(false)
    }

    /// 1194:F768. The template's name at NAME: a magic one's made
    /// (27CE:000C), a plain one's from the weapons' list (2204) or its
    /// armour kind's (the lists at 2688), by type.
    fn store_template_name(&mut self) -> Option<()> {
        let tb = |m: &Self, f: u16| m.mem.r8(DS, TEMPLATE + f);
        if tb(self, t::MAGIC) != 0 {
            let dl = tb(self, t::WEAPON);
            if dl == 2 {
                // Not a weapon or armour: not in the store's lists.
                return None;
            }
            let (al, bx, cl) = (tb(self, t::MATERIAL), tb(self, t::TYPE) as u16, tb(self, t::ENCHANT));
            return self.item_name_format(DS, dl, al, 0, bx, cl, cl);
        }
        let ty = tb(self, t::TYPE) as u16;
        let list = if tb(self, t::WEAPON) != 0 {
            0x2204
        } else {
            self.mem.r16(DS, 0x2688u16.wrapping_add((tb(self, t::ARMOR_KIND) as u16).wrapping_mul(2)))
        };
        let si = self.skip_strings(DS, list, ty);
        self.store_strcpy(si, NAME);
        Some(())
    }

    /// The shopkeeper's name (1194:080B, set A46, sex 0) from the place's
    /// seed (the dword of its entry, 1194:3EB1), at DS:`di`; returns `di`.
    fn shopkeeper_name(&mut self, files: &mut Files, di: u16) -> Option<u16> {
        let key = self.mem.r16(DS, 0x0FD2);
        let key = (key & 0xFF00) | ((key & 0xFF) >> 1);
        let (off, seg) = (self.mem.r16(DS, 0x1008), self.mem.r16(DS, 0x100A));
        let si = self.place_lookup(seg, off, key, self.mem.r16(DS, 0x138B));
        let seed = self.mem.r32(seg, si);
        let kind = self.mem.r8(DS, 0x0A46);
        self.npc_name(files, kind, 0, seed, DS, di)?;
        Some(di)
    }

    /// 1194:5B07 on the player's personality (1D3): 0..100.
    fn store_personality(&self) -> u16 {
        crate::scale::to100(self.mem.r8(DS, 0x01D3) as i16) as u16
    }

    /// 1194:F961. The template's weight: a weapon's by type (22B0), an
    /// armour's by kind and type (1194:EB95: 252B plate, 2515 chain, 24FF
    /// leather).
    fn store_weight(&self) -> u16 {
        let ty = self.mem.r8(DS, TEMPLATE + t::TYPE) as u16;
        if self.mem.r8(DS, TEMPLATE + t::WEAPON) != 0 {
            return self.mem.r16(DS, 0x22B0u16.wrapping_add(ty.wrapping_mul(2)));
        }
        let table = match self.mem.r8(DS, TEMPLATE + t::ARMOR_KIND) {
            0 => 0x252B,
            1 => 0x2515,
            _ => 0x24FF,
        };
        self.mem.r16(DS, table + ty.wrapping_mul(2))
    }

    /// 1194:F5DB. Selling; see the module's notes. The carry (never set).
    fn store_sell(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<bool> {
        // 1194:F5DE: the gold left.
        hooks.gold_left(self);
        let picked = self.store_pick_item(hooks)?;
        // 1194:F5E9, or F6B7 when none was picked.
        hooks.strip_back(self);
        let Some(si) = picked else {
            return Some(false);
        };
        let name = self.any_item_name(si)?;
        self.mem.w16(DS, NAME_AT, name);
        let cat = self.item_category(DS, si);
        if self.mem.r8(DS, si) >= 0x80 || (cat != 3 && self.mem.r16(DS, si.wrapping_add(6)) == 0) {
            hooks.message(self, text::NOT_INTERESTED);
            return Some(false);
        }
        if cat != 3 && self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x80 != 0 {
            // 1194:F6BC: an equipped item, only if confirmed.
            let name = self.any_item_name(si)?;
            self.sprintf((DS, SCRATCH), (DS, text::STILL_SELL), &[Arg::Str(DS, name)]);
            // 1194:F6D2: ARENAFNT selected.
            self.select_font(font::ARENA);
            if hooks.menu(self, SCRATCH, text::YES_NO, text::YES_NO_ENTRIES) != 0 {
                return Some(false);
            }
        }
        let ebx = self.mem.r32(DS, si.wrapping_add(0x0A));
        let eax = match cat {
            3 | 2 => {
                let ebx = if cat == 3 && ebx != 0 { 0x64 } else { ebx };
                let v = ebx >> 2;
                self.mem.w32(DS, SALE, v);
                v
            }
            _ => {
                let p = (self.mem.r16(DS, si.wrapping_add(6)) as u64) * ebx as u64;
                let d = self.mem.r16(DS, si.wrapping_add(8)) as u64;
                if d == 0 || p / d > 0xFFFF_FFFF {
                    return crate::giveup::original("1194:F6BC: the price's divide overflows");
                }
                (p / d) as u32
            }
        };
        let eax = if eax == 0 { 1 } else { eax };
        self.mem.w32(DS, PRICE, eax);
        // 1194:09FC.
        let name = self.shopkeeper_name(files, SELL_NAME)?;
        let per = self.store_personality();
        let (eax, cf) = hooks.haggle(self, 4, eax, per, name);
        if cf {
            return Some(false);
        }
        let f = self.mem.r8(DS, si.wrapping_add(0x0E)) & 0xBF;
        self.mem.w8(DS, si.wrapping_add(0x0E), f);
        if f & 0x80 != 0 {
            self.player_weapon_class();
        }
        let gold = self.mem.r32(DS, GOLD).wrapping_add(eax);
        self.mem.w32(DS, GOLD, gold);
        let cat = self.item_category(DS, si);
        if cat != 2 && cat != 3 && self.mem.r8(DS, si.wrapping_add(0x0E)) & 1 != 0 {
            self.store_resale_price(si, cat);
            self.store_record(si);
            self.store_add_record();
        }
        // 13C8 read again: a long name made at 1395 runs on over it.
        let pick = self.mem.r8(DS, PICK) as u16;
        self.remove_item(DS, si, pick);
        Some(false)
    }

    /// 1194:FE0E. The player's items named for a list (up to the first
    /// empty slot) and one picked: its slot (PICK set), or None (nothing
    /// to sell, "Your inventory is empty.", or nothing picked).
    fn store_pick_item(&mut self, hooks: &mut dyn StoreHooks) -> Option<Option<u16>> {
        self.mem.w16(DS, SELL_NEXT, SELL_LIST);
        let mut n = 0u16;
        let mut si = crate::inventory::PLAYER_SLOTS;
        while n < 0x28 && self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x40 != 0 {
            let name = self.any_item_name(si)?;
            // 1194:FBE9.
            let di = self.mem.r16(DS, SELL_NEXT);
            self.store_strcpy(name, di);
            let mut di = self.mem.r16(DS, SELL_NEXT);
            let mut k = 0xC8u16;
            while k != 0 {
                k -= 1;
                let c = self.mem.r8(DS, di);
                di = di.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
            self.mem.w16(DS, SELL_NEXT, di);
            si = si.wrapping_add(0x13);
            n += 1;
        }
        if n == 0 {
            hooks.message(self, text::INVENTORY_EMPTY);
            return Some(None);
        }
        // 1194:FE3E: TEENYFNT selected.
        self.select_font(font::TEENY);
        // DI: where 1194:FBE9 left it, past the last name.
        let di = self.mem.r16(DS, SELL_NEXT);
        // AX Bh (1194:FE4E): the list's kind (1194:5734).
        self.list_kind = 0xB;
        let pick = hooks.list(self, SELL_LIST, n, di);
        if (pick as i16) < 0 {
            return Some(None);
        }
        self.mem.w8(DS, PICK, pick as u8);
        let pick = self.mem.r8(DS, PICK) as u16;
        Some(Some(crate::inventory::PLAYER_SLOTS.wrapping_add(pick.wrapping_mul(0x13))))
    }

    /// 1194:FC02. The price a sold item is put up for again: a weapon's
    /// (1194:FC78 on the tables at 22D4, 23F7) or an armour's (24F4,
    /// 2623) by type, material and enchantment. Into PRICE.
    fn store_resale_price(&mut self, si: u16, cat: u16) {
        let eax = if cat != 0 {
            self.mem.w8(DS, BUYING_WEAPON, 0);
            self.store_price_from(si, 0x22D4, 0x23F7)
        } else {
            self.store_price_from(si, 0x24F4, 0x2623)
        };
        self.mem.w32(DS, PRICE, eax);
    }

    /// 1194:FC78: the price byte at `prices` + type; with a material
    /// (+10h not FFh) times its word at 2882 over four (16 bits); with an
    /// enchantment (+0Fh not FFh) its word at `enchant` plus that. With
    /// none, AX as it was left: the type, or the material's product.
    fn store_price_from(&self, si: u16, prices: u16, enchant: u16) -> u32 {
        let ty = self.mem.r8(DS, si) as u16;
        let mut ax = ty;
        let mut cx = self.mem.r8(DS, prices.wrapping_add(ty)) as u16;
        let material = self.mem.r8(DS, si.wrapping_add(0x10));
        if material != 0xFF {
            let w = self.mem.r16(DS, 0x2882u16.wrapping_add((material as u16).wrapping_mul(2)));
            ax = w.wrapping_mul(cx) >> 2;
            cx = ax;
        }
        let e = self.mem.r8(DS, si.wrapping_add(0x0F));
        if e != 0xFF {
            let w = self.mem.r16(DS, enchant.wrapping_add((e as u16).wrapping_mul(2)));
            return w as u32 + cx as u32;
        }
        ax as u32
    }

    /// 1194:F426. A sold magic item (+0Eh bit 1) recorded in the template:
    /// magic, weapon or armour, its material, type, enchantment and the
    /// enchantment's two bytes; its category kept (13F2).
    fn store_record(&mut self, si: u16) {
        self.clear_template();
        if self.mem.r8(DS, si.wrapping_add(0x0E)) & 1 == 0 {
            // Not reached: Sell records only magic items.
            self.mem.w8(DS, TEMPLATE + t::MAGIC, 0);
            if self.mem.r8(DS, CATEGORY) != 1 {
                let kind = (self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x30) >> 4;
                let kind = if kind > 1 { 2 } else { kind };
                self.mem.w8(DS, TEMPLATE + t::ARMOR_KIND, kind);
            }
            let ty = self.mem.r8(DS, si);
            self.mem.w8(DS, TEMPLATE + t::TYPE, ty);
            return;
        }
        self.mem.w8(DS, TEMPLATE + t::MAGIC, 1);
        let cat = self.item_category(DS, si);
        self.mem.w8(DS, TEMPLATE + t::WEAPON, (cat != 0) as u8);
        self.mem.w8(DS, CATEGORY, cat as u8);
        let b = |m: &Self, f: u16| m.mem.r8(DS, si.wrapping_add(f));
        let (material, ty, e, e12, e11) = (b(self, 0x10), b(self, 0), b(self, 0x0F), b(self, 0x12), b(self, 0x11));
        self.mem.w8(DS, TEMPLATE + t::MATERIAL, material);
        self.mem.w8(DS, TEMPLATE + t::TYPE, ty);
        self.mem.w8(DS, TEMPLATE + t::ENCHANT, e);
        self.mem.w8(DS, TEMPLATE + t::ENCHANT_SPELL, e12);
        self.mem.w8(DS, TEMPLATE + t::ENCHANT_VALUE, e11);
    }

    /// 1194:F6EC. The template, priced at PRICE, appended to the weapons
    /// (category 1) or the armour and the list sorted (1194:F37B).
    fn store_add_record(&mut self) {
        let (list, count) = if self.mem.r8(DS, CATEGORY) == 1 { (WEAPON_LIST, WEAPONS) } else { (ARMOR_LIST, ARMOR) };
        let n = self.mem.r8(DS, count);
        let frame = self.mem.r16(DS, FRAME);
        let di = list.wrapping_add((n as u16).wrapping_mul(13));
        let price = self.mem.r32(DS, PRICE);
        self.mem.w32(DS, TEMPLATE + t::PRICE, price);
        self.memmove(DS, TEMPLATE, frame, di, 13);
        let n = n.wrapping_add(1);
        self.mem.w8(DS, count, n);
        self.shop_sort(list, n);
    }

    /// 1194:FCC6. Stealing; see the module's notes. The carry: the guards
    /// were called (the store left).
    fn store_steal(&mut self, hooks: &mut dyn StoreHooks) -> Option<bool> {
        if self.free_slot(DS, PLAYER).is_err() {
            // 1194:FCFF (42EE with ARENAFNT).
            self.message_fonts(font::ARENA);
            hooks.message_box(self, text::INVENTORY_FULL);
            return Some(false);
        }
        let (ok, chance) = self.lockpick()?;
        if !ok {
            // 1194:FCD7 (42EE with FONT_A).
            self.message_fonts(font::A);
            hooks.message_box(self, text::UNSUCCESSFUL);
            let cx = 100u16.wrapping_sub(chance);
            if self.rng.next() % 100 >= cx {
                return Some(false);
            }
            self.call_guards();
            self.mem.w8(DS, 0x191D, 1);
            self.clear_dungeon_flats();
            return Some(true);
        }
        // 1194:FD0C.
        let weapons = self.rng.range(0, 2)? != 0;
        let (list, count) = if weapons { (WEAPON_LIST, WEAPONS) } else { (ARMOR_LIST, ARMOR) };
        let n = self.mem.r8(DS, count) as u16;
        let pick = self.rng.range(0, n)?;
        let frame = self.mem.r16(DS, FRAME);
        let si = list.wrapping_add(pick.wrapping_mul(13));
        self.memmove(frame, si, DS, TEMPLATE, 13);
        self.mem.w8(DS, 0x13EF, 1);
        if weapons {
            self.finish_weapon_at(ITEM);
        } else {
            self.finish_armor_at(ITEM);
        }
        self.mem.w8(DS, 0x13EF, 0);
        let name = self.any_item_name(ITEM)?;
        // 1194:FDB6.
        self.sprintf((DS, SCRATCH), (DS, text::STOLE), &[Arg::Str(DS, name)]);
        // 1194:FDC8 (42EE with ARENAFNT).
        self.message_fonts(font::ARENA);
        hooks.message_box(self, SCRATCH);
        self.map_bank(0x18, 0);
        let price = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, PRICE, price);
        let slot = self.free_slot(DS, PLAYER).map_or_else(|end| end, |(s, _)| s);
        self.mem.w8(DS, 0x13EF, 0);
        if weapons {
            self.finish_weapon_at(slot);
        } else {
            self.finish_armor_at(slot);
        }
        self.sort_inventory();
        Some(false)
    }
}

/// The repairs' variables (DGROUP).
pub mod repair {
    /// The gold asked and the days (words), the price as first priced
    /// (dword), the state of the asking (FFh: done), where the next name
    /// goes in the list, the number typed.
    pub const COST: u16 = 0x785F;
    pub const DAYS: u16 = 0x785D;
    pub const ASKED: u16 = 0x7861;
    pub const STATE: u16 = 0x7867;
    pub const NEXT: u16 = 0x7868;
    pub const TYPED: u16 = 0x76DE;
    /// The text's number, its wrap width, no substitutions (FFh), the
    /// dialog's kind.
    pub const TEXT: u16 = 0x137A;
    pub const WIDTH: u16 = 0x137E;
    pub const NO_EXPAND: u16 = 0x101C;
    pub const DIALOG: u16 = 0x13D3;
    /// The formatted texts.
    pub const SAID: u16 = 0x1116;
    pub const READY_TEXT: u16 = 0x93CA;
    /// Texts: the question, "Your %s is ready.", "...does not need any
    /// repairing.", "...my hands full...", the pictures, the three
    /// reasons (a list of strings).
    pub const WHICH: u16 = 0x780A;
    pub const READY: u16 = 0x782C;
    pub const WHOLE: u16 = 0x77AD;
    pub const FULL: u16 = 0x77D2;
    pub const NEW_OLD: u16 = 0x1346;
    pub const YES_NO: u16 = 0x132F;
    pub const REASONS: u16 = 0x7761;
}

impl Machine {
    /// 1194:C6AC. The repairs (see the module's notes). The carry (never
    /// set). None where the original faults or reads on through memory,
    /// or a text or an item's name can't be made.
    pub fn store_repair(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<bool> {
        use repair::*;
        if self.store_repairs() {
            hooks.picture(self, NEW_OLD);
            self.mem.w8(DS, WIDTH, 3);
            self.split_lines(WHICH);
            self.mem.w8(DS, DIALOG, 1);
            let ax = hooks.dialog_at(self, 1, WHICH, 0);
            if ax == 2 {
                return Some(false);
            }
            if ax != 0 {
                if let Some(pick) = self.repair_list(hooks)? {
                    if (pick as i16) < 0 {
                        return Some(false);
                    }
                    return self.repair_collect(files, hooks, pick);
                }
            }
        }
        self.repair_new(files, hooks)
    }

    /// 1194:CA02. The jobs' names (each record's item, 1194:7388) one
    /// after another from 1044, up to the first record not taken, listed
    /// (1194:5730): the pick, or None (the carry) when the first isn't
    /// taken.
    fn repair_list(&mut self, hooks: &mut dyn StoreHooks) -> Option<Option<u16>> {
        use repair::*;
        self.mem.w16(DS, NEXT, SCRATCH);
        let mut bx = 0u16;
        for i in 0..5u16 {
            let si = REPAIRS + i * 0x18;
            if self.mem.r8(DS, si) == 0 {
                if bx == 0 {
                    return Some(None);
                }
                break;
            }
            bx += 1;
            let name = self.any_item_name(si + 5)?;
            let di = self.mem.r16(DS, NEXT);
            let di = self.store_strcpy(name, di);
            self.mem.w16(DS, NEXT, di);
        }
        let di = self.mem.r16(DS, NEXT);
        // AX Bh (1194:CA4D): the list's kind (1194:5734).
        self.list_kind = 0xB;
        Some(Some(hooks.list(self, SCRATCH, bx, di)))
    }

    /// 1194:C6E7..C7AF. Job `pick` (PICK) collected: told the time left
    /// when it isn't ready, else made whole and taken.
    fn repair_collect(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks, pick: u16) -> Option<bool> {
        use repair::*;
        self.mem.w8(DS, PICK, pick as u8);
        let si = REPAIRS.wrapping_add((pick & 0xFF).wrapping_mul(0x18));
        let at = self.mem.r32(DS, si.wrapping_add(1));
        if at > self.mem.r32(DS, TIME) {
            self.repair_not_ready(files, hooks, si, at)?;
            return Some(false);
        }
        let it = si.wrapping_add(5);
        if self.mem.r8(DS, it) & 0x80 != 0 {
            // 2686:06E1: a quest piece made again at ITEM.
            let kept = self.mem.r8(DS, PICK);
            let step = self.quest_step_of(self.mem.r8(DS, it));
            self.quest_artifact(step)?;
            self.mem.w8(DS, PICK, kept);
        } else if self.item_category(DS, it) == 2 && self.armor_kind(DS, it) == 0 {
            let v = self.mem.r8(DS, it.wrapping_add(6));
            self.mem.w8(DS, it.wrapping_add(3), v);
        } else {
            let v = self.mem.r16(DS, it.wrapping_add(8));
            self.mem.w16(DS, it.wrapping_add(6), v);
        }
        let name = self.any_item_name(it)?;
        self.store_strcpy(name, NAME);
        self.sprintf((DS, READY_TEXT), (DS, READY), &[Arg::Str(DS, NAME)]);
        hooks.message(self, READY_TEXT);
        if self.mem.r8(DS, it) < 0x8D {
            let w = self.mem.r16(DS, it.wrapping_add(1));
            if self.carry_check(&mut Heavy(hooks), w)? {
                return Some(false);
            }
        }
        let Ok((slot, _)) = self.free_slot(DS, PLAYER) else {
            hooks.message(self, text::INVENTORY_FULL);
            return Some(false);
        };
        let f = self.mem.r8(DS, slot.wrapping_add(0x0E)) | 0x40;
        self.mem.w8(DS, slot.wrapping_add(0x0E), f);
        let f = self.mem.r8(DS, it.wrapping_add(0x0E));
        if f & 0x80 != 0 {
            self.mem.w8(DS, it.wrapping_add(0x0E), f & 0x7F);
        }
        // 2686:06F9: a quest piece from ITEM.
        let from = if self.mem.r8(DS, it) & 0x80 != 0 { ITEM } else { it };
        self.memmove(DS, from, DS, slot, 0x13);
        self.repair_remove();
        self.sort_inventory();
        hooks.picture(self, YES_NO);
        Some(false)
    }

    /// 1194:C8FB. How long the job at `si` (ready at minute `at`) still
    /// takes: a day or more, its days (text 58Ch); more than 168h minutes,
    /// its hours (58Dh); else "any minute" (58Bh, the minutes passed on
    /// unused). Its item's name (1194:C971, through ITEM) and the number
    /// formatted into the text (C992), shown as a box.
    fn repair_not_ready(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks, si: u16, at: u32) -> Option<()> {
        use repair::*;
        let left = at.wrapping_sub(self.mem.r32(DS, TIME));
        let (number, n) = if left >= 0x5A0 {
            (0x58C, left / 0x5A0)
        } else if left > 0x168 {
            (0x58D, left / 0x3C)
        } else {
            (0x58B, left)
        };
        self.mem.w16(DS, TEXT, number);
        self.repair_text(files, hooks)?;
        // 1194:C971.
        self.memmove(DS, si.wrapping_add(5), DS, ITEM, 0x13);
        let name = self.any_item_name(ITEM)?;
        self.store_strcpy(name, NAME);
        // 1194:C992.
        self.sprintf((DS, READY_TEXT), (DS, SCRATCH), &[Arg::Str(DS, NAME), Arg::Int(n as u16)]);
        self.repair_spaces(READY_TEXT);
        self.mem.w8(DS, WIDTH, 0);
        self.repair_wrap(READY_TEXT)?;
        self.mem.w8(DS, WIDTH, 3);
        self.split_lines(READY_TEXT);
        self.mem.w8(DS, DIALOG, 0);
        hooks.dialog_at(self, 0, READY_TEXT, 0);
        Some(())
    }

    /// 1194:CC61. The text numbered at TEXT written to 1044 (no
    /// substitutions, no wrap).
    fn repair_text(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<()> {
        use repair::*;
        self.mem.w8(DS, WIDTH, 0);
        self.mem.w8(DS, NO_EXPAND, 0xFF);
        hooks.text(self, files, SCRATCH)
    }

    /// Line feeds and carriage returns at DS:`si` made spaces (C9A5,
    /// CD62).
    fn repair_spaces(&mut self, mut si: u16) {
        loop {
            match self.mem.r8(DS, si) {
                0 => return,
                0x0A | 0x0D => self.mem.w8(DS, si, 0x20),
                _ => {}
            }
            si = si.wrapping_add(1);
        }
    }

    /// 1194:49FF. The text at `si` wrapped at WIDTH characters (3Ch when
    /// 0): a line's last space before the width made a line feed. None
    /// where none is found going back (the original reads on through
    /// memory).
    fn repair_wrap(&mut self, si: u16) -> Option<()> {
        use repair::*;
        let mut ax = 0u16;
        while self.mem.r8(DS, si.wrapping_add(ax)) != 0 {
            ax = ax.wrapping_add(1);
        }
        let mut si = si;
        loop {
            if self.mem.r8(DS, WIDTH) == 0 {
                self.mem.w8(DS, WIDTH, 0x3C);
            }
            let bx = self.mem.r8(DS, WIDTH) as u16;
            if ax <= bx {
                return Some(());
            }
            si = si.wrapping_add(bx);
            ax = ax.wrapping_sub(bx);
            let mut steps = 0u32;
            loop {
                match self.mem.r8(DS, si) {
                    0 => return Some(()),
                    0x20 => {
                        self.mem.w8(DS, si, 0x0A);
                        si = si.wrapping_add(1);
                        break;
                    }
                    _ => {
                        si = si.wrapping_sub(1);
                        ax = ax.wrapping_add(1);
                        steps += 1;
                        if steps > 0x10000 {
                            return crate::giveup::original("1194:49FF: the wrap never ends: the original loops for ever");
                        }
                    }
                }
            }
        }
    }

    /// 1194:C9CC. Job PICK's record removed: those after it moved down
    /// one, the last cleared.
    fn repair_remove(&mut self) {
        let pick = self.mem.r8(DS, PICK) as u16;
        let mut si = REPAIRS.wrapping_add(pick * 0x18);
        let mut di = si;
        let n = 4u16.wrapping_sub(pick);
        if n != 0 {
            si = si.wrapping_add(0x18);
            for _ in 0..n {
                (si, di) = self.memmove(DS, si, DS, di, 0x18);
            }
        }
        for i in 0..0x18u16 {
            self.mem.w8(DS, di.wrapping_add(i), 0);
        }
    }

    /// 1194:C7B2. A new job: a record free, an item picked (1194:FE0E),
    /// not already whole (CA7F), priced and agreed (1194:C83B), filed
    /// (CA68) and taken out of the inventory.
    fn repair_new(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<bool> {
        use repair::*;
        // 1194:C7B2: the gold left.
        hooks.gold_left(self);
        if self.repair_free().is_none() {
            // 1194:C82B
            hooks.strip_back(self);
            hooks.message(self, FULL);
            return Some(false);
        }
        let picked = self.store_pick_item(hooks)?;
        // 1194:C7BF, or C836 when none was picked.
        hooks.strip_back(self);
        let Some(si) = picked else {
            return Some(false);
        };
        let whole = if self.item_category(DS, si) == 2 && self.armor_kind(DS, si) == 0 {
            self.mem.r8(DS, si.wrapping_add(3)) == self.mem.r8(DS, si.wrapping_add(8))
        } else {
            self.mem.r16(DS, si.wrapping_add(6)) == self.mem.r16(DS, si.wrapping_add(8))
        };
        if whole {
            // 1194:CA7F.
            let name = self.any_item_name(si)?;
            self.store_strcpy(name, NAME);
            self.sprintf((DS, SCRATCH), (DS, WHOLE), &[Arg::Str(DS, NAME)]);
            hooks.message(self, SCRATCH);
            return Some(false);
        }
        let Some(di) = self.repair_free() else {
            hooks.message(self, FULL);
            return Some(false);
        };
        let eax = self.mem.r32(DS, si.wrapping_add(0x0A));
        let Some(at) = self.repair_offer(files, hooks, si, eax)? else {
            return Some(false);
        };
        // 1194:CA68.
        self.mem.w8(DS, di, 1);
        self.mem.w32(DS, di.wrapping_add(1), at);
        self.memmove(DS, si, DS, di.wrapping_add(5), 0x13);
        let pick = self.mem.r8(DS, PICK) as u16;
        let equipped = self.mem.r8(DS, si.wrapping_add(0x0E)) & 0x80 != 0;
        self.remove_item(DS, si, pick);
        if equipped {
            self.player_weapon_class();
        }
        self.sort_inventory();
        Some(false)
    }

    /// 1194:CA53. The first record not taken, or None (the carry).
    fn repair_free(&self) -> Option<u16> {
        (0..5u16).map(|i| REPAIRS + i * 0x18).find(|&di| self.mem.r8(DS, di) == 0)
    }

    /// 1194:C83B. The job on the item at `si`, priced `eax`: offered
    /// (text 590h) and, refused, asked again from the three reasons
    /// (REASONS: 1194:CB40, 1194:CBD3, 1194:CC77) until agreed or left. The minute
    /// it will be ready (1194:CAF6, paid), or None (left, or not enough
    /// gold).
    fn repair_offer(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks, si: u16, eax: u32) -> Option<Option<u32>> {
        use repair::*;
        self.mem.w32(DS, PRICE, eax);
        self.mem.w32(DS, ASKED, eax);
        let name = self.any_item_name(si)?;
        self.store_strcpy(name, NAME);
        self.mem.w16(DS, NAME_AT, NAME);
        // 1194:CAB8.
        let p = self.mem.r32(DS, PRICE);
        if p >= 10 {
            let q = p / 10;
            self.mem.w16(DS, COST, q as u16);
            self.mem.w16(DS, DAYS, 10);
            self.mem.w32(DS, PRICE, q.wrapping_mul(10));
        } else {
            self.mem.w16(DS, COST, 1);
            self.mem.w16(DS, DAYS, p as u16);
        }
        self.repair_ask_text(files, hooks, 0)?;
        self.repair_format()?;
        match self.repair_yes_no(hooks) {
            0 => return self.repair_pay(files, hooks),
            2 => return Some(None),
            _ => {}
        }
        loop {
            self.mem.w8(DS, STATE, 0);
            // 1194:C874: the gold left.
            hooks.gold_left(self);
            // 1194:CC94: ARENAFNT selected.
            self.select_font(font::ARENA);
            // AX Bh (1194:CCA5): the list's kind (1194:5734).
            self.list_kind = 0xB;
            let ax = hooks.list(self, REASONS, 3, REASONS);
            // 1194:C87E
            hooks.strip_back(self);
            if (ax as i16) < 0 {
                return Some(None);
            }
            let cf = match self.mem.r16(DS, 0x775Bu16.wrapping_add(ax.wrapping_add(ax))) {
                0xCB40 => self.repair_reason(files, hooks, false)?,
                0xCBD3 => self.repair_reason(files, hooks, true)?,
                0xCC77 => {
                    self.repair_ask_text(files, hooks, 4)?;
                    self.mem.w8(DS, WIDTH, 3);
                    self.split_lines(SCRATCH);
                    self.mem.w8(DS, DIALOG, 0);
                    hooks.dialog_at(self, 0, SCRATCH, 0);
                    self.mem.w8(DS, STATE, 0xFF);
                    false
                }
                _ => return crate::giveup::not_ported("0000:9AE6: a repair reason not in the table"),
            };
            if cf {
                continue;
            }
            if self.mem.r8(DS, STATE) == 0xFF {
                return Some(None);
            }
            return self.repair_pay(files, hooks);
        }
    }

    /// 1194:CB40 (`days` false: the gold typed, text 58Ah, then 592h) and
    /// CBD3 (true: the days, 589h, then 591h): the number typed until it is
    /// one (nothing typed, or 0, leaves: STATE FFh); the other from the
    /// first price over it (1 when that is 0 in 16 bits); offered again.
    /// The carry: refused (asked again).
    fn repair_reason(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks, days: bool) -> Option<bool> {
        use repair::*;
        self.mem.w32(DS, TYPED, 0);
        loop {
            self.mem.w16(DS, TEXT, if days { 0x589 } else { 0x58A });
            self.repair_text(files, hooks)?;
            self.mem.w8(DS, WIDTH, 3);
            self.split_lines(SCRATCH);
            self.mem.w8(DS, DIALOG, 2);
            hooks.dialog_at(self, 2, SCRATCH, TYPED);
            if self.mem.r8(DS, TYPED) == 0 {
                self.mem.w8(DS, STATE, 0xFF);
                return Some(false);
            }
            if !self.not_number(TYPED) {
                break;
            }
        }
        let ax = self.parse_number(TYPED);
        if ax == 0 {
            self.mem.w8(DS, STATE, 0xFF);
            return Some(false);
        }
        let (set, other) = if days { (DAYS, COST) } else { (COST, DAYS) };
        self.mem.w16(DS, set, ax);
        let q = (self.mem.r32(DS, ASKED) / ax as u32) as u16;
        self.mem.w16(DS, other, if q != 0 { q } else { 1 });
        self.repair_ask_text(files, hooks, if days { 1 } else { 2 })?;
        self.repair_format()?;
        match self.repair_yes_no(hooks) {
            0 => {
                self.mem.w8(DS, STATE, 0);
                Some(false)
            }
            2 => {
                self.mem.w8(DS, STATE, 0xFF);
                Some(false)
            }
            _ => {
                if !days {
                    self.mem.w8(DS, STATE, 0);
                }
                Some(true)
            }
        }
    }

    /// 1194:CAF6. Agreed: text 593h shown; the gold paid (COST) and the
    /// minute it will be ready, DAYS * 5A0h (in 16 bits) on from now; or
    /// "You don't have enough money." (None). STATE FFh either way.
    fn repair_pay(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks) -> Option<Option<u32>> {
        use repair::*;
        self.repair_ask_text(files, hooks, 3)?;
        hooks.message(self, SCRATCH);
        self.mem.w8(DS, DIALOG, 0);
        let cost = self.mem.r16(DS, COST) as u32;
        let at = if self.mem.r32(DS, GOLD) < cost {
            hooks.message(self, text::NOT_ENOUGH);
            None
        } else {
            let gold = self.mem.r32(DS, GOLD) - cost;
            self.mem.w32(DS, GOLD, gold);
            let ax = self.mem.r16(DS, DAYS).wrapping_mul(0x5A0);
            Some((ax as u32).wrapping_add(self.mem.r32(DS, TIME)))
        };
        self.mem.w8(DS, STATE, 0xFF);
        Some(at)
    }

    /// 1194:CAA7. Text 590h + `cx` written to 1044 (CC61).
    fn repair_ask_text(&mut self, files: &mut Files, hooks: &mut dyn StoreHooks, cx: u16) -> Option<()> {
        self.mem.w16(DS, repair::TEXT, 0x590 + cx);
        self.repair_text(files, hooks)
    }

    /// 1194:CCFD. The text at 1044 formatted into SAID: going back from
    /// its end to a NUL (past its start while the bytes before it aren't
    /// one), each "%a" made "%u" with the gold, "%t" "%u" with the days,
    /// "%i" "%s" with the item's name (NAME_AT); the line breaks made
    /// spaces and the text wrapped at 3Ch (49FF). The original saves SP at
    /// 7865 meanwhile, left out. None where no NUL comes going back.
    fn repair_format(&mut self) -> Option<()> {
        use repair::*;
        let mut si = SCRATCH;
        while self.mem.r8(DS, si) != 0 {
            si = si.wrapping_add(1);
        }
        si = si.wrapping_sub(1);
        let mut pushed = Vec::new();
        let mut steps = 0u32;
        loop {
            let c = self.mem.r8(DS, si);
            if c == 0 {
                break;
            }
            if c == b'%' {
                let next = si.wrapping_add(1);
                match self.mem.r8(DS, next) {
                    b'a' => {
                        self.mem.w8(DS, next, b'u');
                        pushed.push(Arg::Int(self.mem.r16(DS, COST)));
                    }
                    b't' => {
                        self.mem.w8(DS, next, b'u');
                        pushed.push(Arg::Int(self.mem.r16(DS, DAYS)));
                    }
                    b'i' => {
                        self.mem.w8(DS, next, b's');
                        pushed.push(Arg::Str(DS, self.mem.r16(DS, NAME_AT)));
                    }
                    _ => {}
                }
            }
            si = si.wrapping_sub(1);
            steps += 1;
            if steps > 0x10000 {
                return crate::giveup::original("1194:CCFD: the format never ends: the original loops for ever");
            }
        }
        // The last pushed is the first argument.
        pushed.reverse();
        self.sprintf((DS, SAID), (DS, SCRATCH), &pushed);
        self.repair_spaces(SAID);
        self.mem.w8(DS, WIDTH, 0);
        self.repair_wrap(SAID)
    }

    /// 1194:CCA9. SAID asked as a yes or no (yesno.img, colour 60h): its
    /// AX (0 yes, 2 leave).
    fn repair_yes_no(&mut self, hooks: &mut dyn StoreHooks) -> u16 {
        use repair::*;
        hooks.picture(self, YES_NO);
        self.mem.w8(DS, COLOUR, 0x60);
        self.mem.w8(DS, WIDTH, 3);
        self.split_lines(SAID);
        self.mem.w8(DS, DIALOG, 1);
        hooks.dialog_at(self, 1, SAID, 0)
    }

    /// 1194:F142 (`weapons`) and F204. The list's lines, one per record,
    /// written one after another (NUL-terminated) into the EMS frame from
    /// FA0 (weapons, from the records at 0, 12C6 of them) or 2710 (armour,
    /// from 7D0, 12CF), the read and write offsets kept at 12C7 and 12CB
    /// (12C9, 12CD). Each record is copied to the template (13F3, cleared
    /// first, 1194:E970) and named at 1395: a magic one (+1: 1 for a
    /// weapon, any for armour) by 27CE:000C (1194:F126, its +2, +4, +5,
    /// +6), a plain weapon from the list at 2204, plain armour from its
    /// kind's list (2688) by its type (+5). A weapon's line (the format at
    /// 86BD): the name, its hands (the table at 229E, kept at A874), its
    /// weight (1194:EBBE: the word at 22B0 by type, a whole number of
    /// pounds from 100h up written by 0000:7024 with its bytes swapped,
    /// else the quarter (".25", ".5", ".75" for 40h, 80h, C0h, 1194:F2FC;
    /// another value is searched for on through DGROUP, its index then a
    /// string past those three) at 10A8) and its price; armour's (86DD):
    /// the name, its kind's name
    /// (the list at 2491, at 10A8), its weight (the high byte of the word
    /// by type in the kind's table: 252B, 2515, 24FF; at A874) and its
    /// price. The price on the shop's holiday (13C9) is 1194:4A6B's with
    /// service 2 (weapons) or 3. None where the original loops for ever (a
    /// weight under 100h found nowhere in DGROUP) or runs wild: no records
    /// at all (CX 0 runs the loop 65536 times through the frame), or a
    /// name that fails (`item_name_format`).
    pub fn store_lines(&mut self, weapons: bool) -> Option<()> {
        let (count, src, src0, dst, dst0) =
            if weapons { (WEAPONS, 0x12C7, WEAPON_LIST, 0x12CB, 0x0FA0) } else { (ARMOR, 0x12C9, ARMOR_LIST, 0x12CD, 0x2710) };
        let n = self.mem.r8(DS, count) as u16;
        self.mem.w16(DS, src, src0);
        self.mem.w16(DS, dst, dst0);
        if n == 0 {
            return None;
        }
        let frame = self.mem.r16(DS, FRAME);
        for _ in 0..n {
            for i in 0..13u16 {
                self.mem.w8(DS, TEMPLATE + i, 0);
            }
            let si = self.mem.r16(DS, src);
            self.memmove(frame, si, DS, TEMPLATE, 13);
            self.mem.w16(DS, src, si.wrapping_add(13));
            let tb = |m: &Self, f: u16| m.mem.r8(DS, TEMPLATE + f);
            let magic = if weapons { tb(self, 1) == 1 } else { tb(self, 1) != 0 };
            if magic {
                // CX is the byte zero-extended: weapons and armour take it
                // whole as the suffix, an accessory (kind 2) its CH, 0.
                let (dl, cl) = (tb(self, 2), tb(self, 6));
                let suffix = if dl == 2 { 0 } else { cl };
                self.item_name_format(DS, dl, tb(self, 4), 0, tb(self, 5) as u16, cl, suffix)?;
            } else {
                let list = if weapons {
                    0x2204
                } else {
                    self.mem.r16(DS, 0x2688u16.wrapping_add((tb(self, 3) as u16).wrapping_mul(2)))
                };
                let si = self.skip_strings(DS, list, tb(self, 5) as u16);
                self.store_strcpy(si, NAME);
            }
            let ty = tb(self, 5) as u16;
            let (fmt, args_weight_str) = if weapons {
                let w = self.mem.r16(DS, 0x22B0u16.wrapping_add(ty.wrapping_mul(2)));
                if w >= 0x100 {
                    self.mem.w32(DS, DIGITS, 0);
                    self.store_utoa(w.swap_bytes(), DIGITS);
                } else {
                    // 1194:F2FC: the words from 2413 searched for it (40h,
                    // 80h, C0h first, then on through DGROUP), its index
                    // the quarter's among ".25", ".5", ".75" and beyond.
                    let mut si = 0x2413u16;
                    let mut i = 0u16;
                    let mut found = false;
                    for _ in 0..0x10000u32 {
                        if self.mem.r16(DS, si) == w {
                            found = true;
                            break;
                        }
                        si = si.wrapping_add(2);
                        i = i.wrapping_add(1);
                    }
                    if !found {
                        return None;
                    }
                    let si = self.skip_strings(DS, 0x2419, i);
                    self.store_strcpy(si, DIGITS);
                }
                let hands = self.mem.r8(DS, 0x229Eu16.wrapping_add(ty)) as u16;
                self.mem.w16(DS, 0xA874, hands);
                (0x86BDu16, true)
            } else {
                let si = self.skip_strings(DS, 0x2491, ty);
                self.store_strcpy(si, DIGITS);
                let table = match tb(self, 3) {
                    0 => 0x252Bu16,
                    1 => 0x2515,
                    _ => 0x24FF,
                };
                let w = self.mem.r16(DS, table.wrapping_add(ty.wrapping_mul(2)));
                self.mem.w16(DS, 0xA874, w >> 8);
                (0x86DDu16, false)
            };
            let mut price = self.mem.r32(DS, TEMPLATE + 7);
            if self.mem.r8(DS, HOLIDAY) == 1 {
                price = self.holiday_price(price, if weapons { 2 } else { 3 });
            }
            let at = self.mem.r16(DS, dst);
            let a874 = self.mem.r16(DS, 0xA874) as u32;
            let args: [Arg; 4] = if args_weight_str {
                [Arg::Str(DS, NAME), Arg::Int(a874 as u16), Arg::Str(DS, DIGITS), Arg::Long(price)]
            } else {
                [Arg::Str(DS, NAME), Arg::Str(DS, DIGITS), Arg::Int(a874 as u16), Arg::Long(price)]
            };
            let end = self.sprintf((frame, at), (DS, fmt), &args);
            let di = if end.wrapping_sub(at) < 0x4E20 { end.wrapping_add(1) } else { at.wrapping_add(0x4E20) };
            self.mem.w8(frame, di.wrapping_sub(1), 0);
            self.mem.w16(DS, dst, di);
        }
        Some(())
    }

    /// 0000:7024. `v` in decimal at DS:`di` (no NUL), its digits made
    /// backwards from DGROUP A933 down first (they stay there).
    pub(crate) fn store_utoa(&mut self, v: u16, di: u16) {
        let mut at = 0xA933u16;
        let mut v = v;
        loop {
            self.mem.w8(DS, at, (v % 10) as u8 + b'0');
            v /= 10;
            at = at.wrapping_sub(1);
            if v == 0 {
                break;
            }
        }
        let n = 0xA933u16.wrapping_sub(at);
        for i in 0..n {
            let b = self.mem.r8(DS, at.wrapping_add(1 + i));
            self.mem.w8(DS, di.wrapping_add(i), b);
        }
    }
}
