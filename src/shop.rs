//! Equipment shop stock, potions and magic items (segment 1194).
//!
//! A shop's stock is generated from the shop's own seed (DGROUP 12E0):
//! the RNG seed is swapped for it and restored afterwards, so a shop
//! always offers the same goods. The stock is a list of item templates
//! (13 bytes each, the layout of `loot::template`) in far memory at the
//! segment in DGROUP 9030: weapons from offset 0, armour from 07D0. Every
//! plain weapon and armour the shop level allows is listed, followed by
//! one random magic weapon or armour per shop level from level 7 up.
//! Items the player's class can't use are left out, and magic items are
//! rerolled until their running key (DGROUP 12D0) is new to the shop.
//!
//! Plain items are finished (`finish_weapon`/`finish_armor`) only to run
//! the class check; the finishers read the base price at 1391, which the
//! shop loops never set, so that value carries over from item to item.
//! The listed templates keep their table prices.

use crate::loot::{ITEM, LEVEL, TEMPLATE, template as t};
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// DGROUP variables.
pub const SHOP_SEED: u16 = 0x12E0;
/// Far segment of the stock lists.
pub const LIST_SEG: u16 = 0x9030;
/// Next free offsets in the weapon and armour lists (and two more lists
/// the generator resets but doesn't fill).
pub const WEAPON_NEXT: u16 = 0x12C7;
pub const ARMOR_NEXT: u16 = 0x12C9;
const LIST_C_NEXT: u16 = 0x12CB;
const LIST_D_NEXT: u16 = 0x12CD;
pub const WEAPON_LIST: u16 = 0x0000;
pub const ARMOR_LIST: u16 = 0x07D0;
/// Item counts while generating, and the final counts kept for the shop.
pub const WEAPON_COUNT: u16 = 0x12DB;
pub const ARMOR_COUNT: u16 = 0x12DC;
pub const WEAPON_TOTAL: u16 = 0x12C6;
pub const ARMOR_TOTAL: u16 = 0x12CF;
/// Running keys of the magic items already in the shop, NUL-terminated.
pub const KEYS: u16 = 0x1044;
/// Shop quality and price factors set by `shop_price_factors`.
pub const QUALITY: u16 = 0x13CA;
pub const PRICE_FACTOR: u16 = 0x13CB;
/// Location type: 4 and 6 get their own factor ranges.
const LOCATION: u16 = 0x137F;
/// City record; its byte +25 selects the shop level table.
const CITY: u16 = 0x0A22;

const KEY: u16 = 0x12D0;
const RETRIES: u16 = 0x1294;
const ARMOR_PASS: u16 = 0x13EF;
const SORT_I: u16 = 0x13F1;
const BASE_PRICE: u16 = 0x1391;
const SHOP_FLAG: u16 = 0x13C8;
const PLAYER_CLASS: u16 = 0x01A9;

const W_PRICE: u16 = 0x22D4;
const W_MIN_LEVEL: u16 = 0x22E6;
const QUALITY_STEPS: u16 = 0x22F8;
const A_PLATE_PRICE: u16 = 0x24F4;
const A_MIN_LEVEL: u16 = 0x2541;
/// Per armour kind (plate, chain, leather): price and min-level tables.
const ARMOR_KINDS: [(u16, u16); 3] = [(0x24F4, 0x2725), (0x24E9, 0x27C7), (0x24DE, 0x2877)];
/// 4B72: d101 thresholds; 4B76/4B7E/4B86: level ranges per city type.
const LEVEL_STEPS: u16 = 0x4B72;
const LEVEL_RANGES: [u16; 3] = [0x4B76, 0x4B7E, 0x4B86];

/// 3276 tables for the class check.
const TYPE_MAP: u16 = 0x36DD;
const WEAPON_LISTS: u16 = 0x3C25;
const SHIELD_LISTS: u16 = 0x3C71;
const ARMOR_MIN_KIND: u16 = 0x3CA4;

/// Magic item template (11 bytes): +0 kind, +1 sub-kind, +2 variant,
/// +3 type, +4 rolled value, +5 price (dword), +9/+A spells.
pub const MAGIC_TEMPLATE: u16 = 0x7033;
/// Set while generating a magic item.
const MAGIC_ACTIVE: u16 = 0x7109;
/// Running key of the magic item (shares the word at 12C9 with the
/// armour list pointer).
const MAGIC_KEY: u16 = 0x12C9;

/// Potion tables (DGROUP).
const POTION_PRICE: u16 = 0x2A20;
const POTION_SPELL: u16 = 0x2A11;

/// Item record fields used here (see `loot::item`).
mod it {
    pub const TYPE: u16 = 0x00;
    pub const VALUE: u16 = 0x04;
    pub const HEALTH: u16 = 0x06;
    pub const PRICE: u16 = 0x0A;
    pub const FLAGS: u16 = 0x0E;
    pub const SPELL: u16 = 0x11;
    pub const SPELL_2: u16 = 0x12;
}

impl Machine {
    fn shop_b(&self, off: u16) -> u8 {
        self.mem.r8(DS, off)
    }
    fn shop_setb(&mut self, off: u16, v: u8) {
        self.mem.w8(DS, off, v)
    }
    fn shop_tb(&mut self, f: u16, v: u8) {
        self.shop_setb(TEMPLATE + f, v)
    }
    fn shop_inc(&mut self, off: u16, by: u8) {
        self.shop_setb(off, self.shop_b(off).wrapping_add(by))
    }
    fn shop_rand(&mut self, lo: u16, hi: u16) -> Option<u16> {
        self.rng.range(lo, hi)
    }
    fn shop_any_allowed(&self, table: u16, n: u16) -> bool {
        (0..n).any(|i| self.shop_b(table.wrapping_add(i)) <= self.shop_b(LEVEL))
    }

    /// 0000:7732: memmove of `n` bytes; the result is as if copied
    /// through a temporary buffer.
    fn shop_move(&mut self, from: (u16, u16), to: (u16, u16), n: u16) {
        let buf: Vec<u8> = (0..n).map(|i| self.mem.r8(from.0, from.1.wrapping_add(i))).collect();
        for (i, v) in buf.into_iter().enumerate() {
            self.mem.w8(to.0, to.1.wrapping_add(i as u16), v);
        }
    }

    /// 1194:7596. Item types with bit 7 set map through the signed table
    /// at DGROUP 36DD.
    fn shop_map_type(&self, ty: u8) -> u16 {
        if ty & 0x80 != 0 {
            self.mem.r8(DS, TYPE_MAP + (ty & 0x7f) as u16) as i8 as i16 as u16
        } else {
            ty as u16
        }
    }

    /// 1194:77F2. Is `v` in the 0xFF-terminated list at 3276:`list`?
    /// None where the original scans forever (no terminator anywhere).
    fn shop_in_list(&self, mut list: u16, v: u8) -> Option<bool> {
        for _ in 0..0x10000 {
            match self.mem.r8(DATA, list) {
                0xFF => return Some(false),
                b if b == v => return Some(true),
                _ => list = list.wrapping_add(1),
            }
        }
        None
    }

    /// 1194:7772. Can class index `class` use the item record at DGROUP
    /// `item`? Unusable: types 8D and up, no health, category 3. Weapons
    /// are checked against the class's list (no list: anything); shields
    /// (types 7..=10) against the class's shield list; other armour by
    /// kind (plate 0, chain 1, leather 2) against the class's lowest
    /// allowed kind, and Knights (class 17) can't wear leather.
    pub fn shop_class_can_use(&self, class: u16, item: u16) -> Option<bool> {
        let ty = self.mem.r8(DS, item + it::TYPE);
        if ty >= 0x8D || self.mem.r16(DS, item + it::HEALTH) == 0 {
            return Some(false);
        }
        let flags = self.mem.r8(DS, item + it::FLAGS);
        let usable = match (flags & 0x0C) >> 2 {
            3 => false,
            0 => {
                let t = self.shop_map_type(ty);
                if (7..=10).contains(&t) {
                    let list = self.mem.r16(DATA, SHIELD_LISTS.wrapping_add(class.wrapping_mul(2)));
                    self.shop_in_list(list, t as u8)?
                } else {
                    let kind = (flags & 0x30) >> 4;
                    let min = self.mem.r8(DATA, ARMOR_MIN_KIND.wrapping_add(class));
                    kind >= min && !(class == 0x11 && kind == 2)
                }
            }
            1 => {
                let t = self.shop_map_type(ty);
                match self.mem.r16(DATA, WEAPON_LISTS.wrapping_add(class.wrapping_mul(2))) {
                    0 => true,
                    list => self.shop_in_list(list, t as u8)?,
                }
            }
            _ => true,
        };
        Some(usable)
    }

    fn shop_player_can_use(&self) -> Option<bool> {
        let class = (self.shop_b(PLAYER_CLASS) & 0x1f) as u16;
        self.shop_class_can_use(class, ITEM)
    }

    /// 1194:EECB. Is the template's running key already in the shop? If
    /// not it is appended (without writing a new terminator). A key of 0
    /// is never found. None where the original scans forever.
    pub fn shop_key_is_duplicate(&mut self) -> Option<bool> {
        let key = self.shop_b(KEY);
        let mut p = KEYS;
        for _ in 0..0x10000 {
            match self.shop_b(p) {
                0 => {
                    self.shop_setb(p, key);
                    return Some(false);
                }
                b if b == key => return Some(true),
                _ => p = p.wrapping_add(1),
            }
        }
        None
    }

    /// 1194:EB3B / EB56. Appends the template to a stock list.
    fn shop_list_add(&mut self, next: u16) {
        let seg = self.mem.r16(DS, LIST_SEG);
        let off = self.mem.r16(DS, next);
        self.shop_move((DS, TEMPLATE), (seg, off), 13);
        self.mem.w16(DS, next, off.wrapping_add(13));
    }

    /// d10 against 22F8: material, enchantment, or both (1194:EC67).
    fn shop_roll_quality(&mut self) -> Option<()> {
        let r = self.shop_rand(1, 11)? as u8;
        let steps = (0..2).take_while(|&i| r >= self.shop_b(QUALITY_STEPS + i)).count();
        match steps {
            0 => self.roll_material(),
            1 => self.roll_enchantment(false)?,
            _ => {
                self.roll_material();
                self.roll_enchantment(true)?;
            }
        }
        Some(())
    }

    /// One retry of a rejected magic item; true when the 200 retries
    /// (counted at DGROUP 1294) are used up.
    fn shop_retry(&mut self) -> bool {
        self.clear_template();
        let n = self.mem.r16(DS, RETRIES).wrapping_add(1);
        self.mem.w16(DS, RETRIES, n);
        n >= 200
    }

    /// 1194:EC02 with DGROUP 13F0 = 0: a magic weapon for the shop.
    /// Rerolled while its key is a duplicate or the player's class can't
    /// use it, up to 200 times; then nothing is added.
    pub fn shop_magic_weapon(&mut self) -> Option<()> {
        let saved = self.mem.r16(DS, RETRIES);
        self.mem.w16(DS, RETRIES, 0);
        self.clear_template();
        self.shop_tb(t::WEAPON, 1);
        loop {
            if !self.shop_any_allowed(W_MIN_LEVEL, 18) {
                return None;
            }
            let ty = loop {
                self.shop_setb(KEY, 10);
                let ty = self.shop_rand(0, 18)?;
                if self.shop_b(LEVEL) >= self.shop_b(W_MIN_LEVEL + ty) {
                    break ty;
                }
            };
            self.shop_inc(KEY, ty as u8);
            self.shop_tb(t::MAGIC, 1);
            self.shop_tb(t::WEAPON, 1);
            self.shop_tb(t::TYPE, ty as u8);
            let price = self.shop_b(W_PRICE + ty) as u32;
            self.mem.w32(DS, TEMPLATE + t::PRICE, price);
            self.mem.w32(DS, BASE_PRICE, price);
            self.shop_roll_quality()?;
            if self.shop_key_is_duplicate()? {
                if self.shop_retry() {
                    break;
                }
                continue;
            }
            self.shop_setb(ARMOR_PASS, 1);
            self.finish_weapon();
            self.shop_setb(ARMOR_PASS, 0);
            if self.shop_player_can_use()? {
                self.shop_inc(WEAPON_COUNT, 1);
                self.shop_list_add(WEAPON_NEXT);
                self.mem.w16(DS, RETRIES, saved);
                return Some(());
            }
            if self.shop_retry() {
                break;
            }
        }
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.mem.w16(DS, RETRIES, saved);
        Some(())
    }

    /// 1194:ECED with DGROUP 13F0 = 0: a magic armour for the shop. The
    /// retry count (1294) is not reset first, and an armour the class
    /// can't use is dropped without a retry.
    pub fn shop_magic_armor(&mut self) -> Option<()> {
        let saved = self.mem.r16(DS, RETRIES);
        self.clear_template();
        self.shop_tb(t::WEAPON, 0);
        loop {
            if !self.shop_any_allowed(A_MIN_LEVEL, 11) {
                return None;
            }
            let ty = loop {
                let ty = self.shop_rand(0, 11)?;
                self.shop_setb(KEY, 100);
                if self.shop_b(A_MIN_LEVEL + ty) <= self.shop_b(LEVEL) {
                    break ty;
                }
            };
            self.shop_inc(KEY, ty as u8);
            self.shop_tb(t::TYPE, ty as u8);
            let price = self.shop_b(A_PLATE_PRICE + ty) as u32;
            self.mem.w32(DS, TEMPLATE + t::PRICE, price);
            self.shop_tb(t::MAGIC, 1);
            self.shop_roll_quality()?;
            if self.shop_key_is_duplicate()? {
                if self.shop_retry() {
                    break;
                }
                continue;
            }
            self.shop_setb(ARMOR_PASS, 1);
            self.finish_armor();
            self.shop_setb(ARMOR_PASS, 0);
            if self.shop_player_can_use()? {
                self.shop_inc(ARMOR_COUNT, 1);
                self.shop_list_add(ARMOR_NEXT);
                self.mem.w16(DS, RETRIES, saved);
                return Some(());
            }
            self.clear_template();
            break;
        }
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.mem.w16(DS, RETRIES, saved);
        Some(())
    }

    /// 1194:EA26. Lists every armour of one kind the shop level allows.
    /// The base price (1391) is not set, so `finish_armor` uses whatever
    /// is there; and an unusable armour stays in the template.
    fn shop_list_armor_kind(&mut self, prices: u16, min_levels: u16) -> Option<()> {
        for ty in 0..11u16 {
            if self.shop_b(LEVEL) < self.shop_b(min_levels + ty) {
                continue;
            }
            self.shop_tb(t::TYPE, ty as u8);
            self.shop_inc(ARMOR_COUNT, 1);
            let price = self.shop_b(prices + ty) as u32;
            self.mem.w32(DS, TEMPLATE + t::PRICE, price);
            self.shop_setb(ARMOR_PASS, 1);
            self.finish_armor();
            self.shop_setb(ARMOR_PASS, 0);
            if self.shop_player_can_use()? {
                self.shop_list_add(ARMOR_NEXT);
            } else {
                self.shop_inc(ARMOR_COUNT, 0xFF);
            }
        }
        Some(())
    }

    /// 1194:EADA. Lists every weapon the shop level allows; an unusable
    /// weapon clears the template.
    fn shop_list_weapons(&mut self) -> Option<()> {
        for ty in 0..18u16 {
            self.shop_tb(t::WEAPON, 1);
            if self.shop_b(LEVEL) < self.shop_b(W_MIN_LEVEL + ty) {
                continue;
            }
            self.shop_inc(WEAPON_COUNT, 1);
            self.shop_tb(t::TYPE, ty as u8);
            let price = self.shop_b(W_PRICE + ty) as u32;
            self.mem.w32(DS, TEMPLATE + t::PRICE, price);
            self.shop_setb(ARMOR_PASS, 1);
            self.finish_weapon();
            self.shop_setb(ARMOR_PASS, 0);
            if self.shop_player_can_use()? {
                self.shop_list_add(WEAPON_NEXT);
            } else {
                self.shop_inc(WEAPON_COUNT, 0xFF);
                self.clear_template();
            }
        }
        Some(())
    }

    /// 1194:F37B. Sorts `n` templates at LIST_SEG:`list` by price,
    /// cheapest first, swapping through the template at 13F3 (which is
    /// left holding the last swapped entry).
    pub fn shop_sort(&mut self, list: u16, n: u8) {
        if n <= 1 {
            return;
        }
        let seg = self.mem.r16(DS, LIST_SEG);
        let price = |m: &Self, off: u16| m.mem.r32(seg, off.wrapping_add(t::PRICE));
        self.shop_setb(SORT_I, 0);
        let mut dl = n - 1;
        let mut si = list;
        for _ in 0..dl {
            let mut di = si;
            let mut lowest = price(self, si);
            loop {
                di = di.wrapping_add(13);
                if price(self, di) < lowest {
                    self.shop_move((seg, si), (DS, TEMPLATE), 13);
                    self.shop_move((seg, di), (seg, si), 13);
                    self.shop_move((DS, TEMPLATE), (seg, di), 13);
                    lowest = price(self, si);
                }
                self.shop_inc(SORT_I, 1);
                if self.shop_b(SORT_I) >= dl {
                    break;
                }
            }
            self.shop_setb(SORT_I, 0);
            dl = dl.wrapping_sub(1);
            si = si.wrapping_add(13);
        }
    }

    /// 1194:E826. Generates the stock for the level at 13CC.
    pub fn shop_generate_stock(&mut self) -> Option<()> {
        self.shop_setb(SHOP_FLAG, 0xFF);
        // 1194:F2CE
        self.mem.w16(DS, WEAPON_NEXT, WEAPON_LIST);
        self.mem.w16(DS, ARMOR_NEXT, ARMOR_LIST);
        self.mem.w16(DS, LIST_C_NEXT, 0x0FA0);
        self.mem.w16(DS, LIST_D_NEXT, 0x2710);
        self.shop_setb(WEAPON_COUNT, 0);
        self.shop_setb(ARMOR_COUNT, 0);
        self.shop_setb(SORT_I, 0);
        // 1194:F2E7, and E986 with 13EF = 0: all three armour kinds.
        self.shop_tb(t::MAGIC, 0);
        self.clear_template();
        self.shop_tb(t::WEAPON, 0);
        for (kind, &(prices, mins)) in ARMOR_KINDS.iter().enumerate() {
            self.shop_tb(t::ARMOR_KIND, kind as u8);
            self.shop_list_armor_kind(prices, mins)?;
        }
        self.clear_template();
        self.shop_list_weapons()?;
        let level = self.shop_b(LEVEL);
        if level >= 7 {
            for _ in 0..level {
                self.shop_setb(KEY, 0);
                if self.shop_rand(0, 2)? != 0 {
                    self.shop_magic_weapon()?;
                } else {
                    self.shop_magic_armor()?;
                }
            }
        }
        let (w, a) = (self.shop_b(WEAPON_COUNT), self.shop_b(ARMOR_COUNT));
        self.shop_sort(WEAPON_LIST, w);
        self.shop_sort(ARMOR_LIST, a);
        let (w, a) = (self.shop_b(WEAPON_COUNT), self.shop_b(ARMOR_COUNT));
        self.shop_setb(ARMOR_TOTAL, a);
        self.shop_setb(WEAPON_TOTAL, w);
        Some(())
    }

    /// 1194:4AA7. Rolls the shop level into 13CC: d101 picks one of four
    /// ranges, from the table for the city type (city byte +25: 0, 1 or
    /// other). Returns the level (the original's AX).
    pub fn shop_roll_level(&mut self) -> Option<u16> {
        let r = self.shop_rand(0, 0x65)? as u8;
        let mut i = 0u16;
        while r > self.shop_b(LEVEL_STEPS + i) {
            i = i.wrapping_add(1);
        }
        let table = match self.shop_b(CITY + 0x25) {
            0 => LEVEL_RANGES[0],
            1 => LEVEL_RANGES[1],
            _ => LEVEL_RANGES[2],
        };
        let at = table.wrapping_add(i.wrapping_mul(2));
        let (lo, hi) = (self.shop_b(at) as u16, self.shop_b(at.wrapping_add(1)) as u16);
        let level = self.shop_rand(lo, hi)?;
        self.shop_setb(LEVEL, level as u8);
        Some(level)
    }

    /// 1194:4AF8. Shop quality (13CA) and price factor (13CB), both offset
    /// by `base` (the shop level) in 8 bits.
    pub fn shop_price_factors(&mut self, base: u16) -> Option<()> {
        let (a, b) = match self.shop_b(LOCATION) {
            4 => ((1, 0x65), (0x28, 0x3D)),
            6 => ((1, 0x65), (0x1E, 0x47)),
            _ => ((0x32, 0x65), (0x28, 0x51)),
        };
        let cx = self.shop_rand(a.0, a.1)?;
        let ax = self.shop_rand(b.0, b.1)?;
        self.shop_setb(QUALITY, ax.wrapping_add(base) as u8);
        self.shop_setb(PRICE_FACTOR, base.wrapping_add(cx) as u8);
        Some(())
    }

    /// 1194:FAD9. An equipment shop: with the RNG seeded from the shop's
    /// seed (12E0, or 1 if that is 0), rolls the shop level and price
    /// factors, clears the key list, and generates the stock; then the
    /// RNG seed is put back.
    pub fn shop_open(&mut self) -> Option<()> {
        let saved = self.rng.seed;
        self.rng.seed = match self.mem.r32(DS, SHOP_SEED) {
            0 => 1,
            s => s,
        };
        let level = self.shop_roll_level()?;
        self.shop_price_factors(level)?;
        for i in 0..200 {
            self.shop_setb(KEYS + i, 0);
        }
        self.shop_generate_stock()?;
        self.rng.seed = saved;
        Some(())
    }

    /// 1194:5572. d100 from the RNG's next value: 0..=99.
    fn shop_d100(&mut self) -> u16 {
        self.rng.next() % 100
    }

    /// 1194:B5AB. Index of the first entry of the table at `table` that
    /// is above `v`. None where the original scans forever.
    fn shop_step(&self, table: u16, v: u8) -> Option<u16> {
        (0..=0xFFFFu16).find(|&i| v < self.shop_b(table.wrapping_add(i)))
    }

    /// Rerolls `roll(n)` until the item level reaches the entry's minimum
    /// in `mins`. None where no entry allows it (the original hangs).
    fn shop_roll_allowed(&mut self, n: u16, mins: u16) -> Option<u16> {
        if !self.shop_any_allowed(mins, n) {
            return None;
        }
        loop {
            let r = self.shop_rand(0, n)?;
            if self.shop_b(LEVEL) >= self.shop_b(mins + r) {
                return Some(r);
            }
        }
    }

    fn shop_mt(&mut self, f: u16, v: u8) {
        self.shop_setb(MAGIC_TEMPLATE + f, v)
    }

    fn shop_add_key16(&mut self, v: u16) {
        let k = self.mem.r16(DS, MAGIC_KEY).wrapping_add(v);
        self.mem.w16(DS, MAGIC_KEY, k);
    }

    /// 1194:B566. Trinket (kind 0): a sub-kind by d100 against 1DEB (or
    /// forced by 13C8), then one of three variants (B718/B74B/B77E) each
    /// with its own type table, then the price (B5B5).
    fn shop_magic_kind0(&mut self) -> Option<()> {
        self.shop_add_key16(10);
        self.shop_mt(0, 0);
        let forced = self.shop_b(SHOP_FLAG);
        let sub = if self.shop_b(MAGIC_ACTIVE) == 1 && forced != 0xFF {
            forced as u16
        } else {
            let r = self.shop_d100() as u8;
            self.shop_step(0x1DEB, r)?
        };
        self.shop_add_key16(sub);
        self.shop_mt(1, sub as u8);
        let (key, variant, n, mins, spells) = match self.shop_rand(0, 3)? {
            0 => (5, 0, 15, 0x1EDD, 0x1ECE),
            1 => (10, 1, 9, 0x1F84, 0x1F7B),
            _ => (15, 2, 8, 0x2010, 0x2008),
        };
        self.shop_add_key16(key);
        self.shop_mt(2, variant);
        let ty = self.shop_roll_allowed(n, mins)?;
        // The third variant doesn't add its type to the key.
        if variant != 2 {
            self.shop_add_key16(ty);
        }
        self.shop_mt(3, ty as u8);
        let spell = self.shop_b(spells + ty);
        self.shop_mt(0x0A, spell);
        self.shop_mt(0x09, 0xFF);
        // 1194:B5B5
        let sub = self.shop_b(MAGIC_TEMPLATE + 1) as u16;
        let price = self.mem.r16(DS, 0x1DF3 + sub * 2) as u32;
        self.mem.w32(DS, MAGIC_TEMPLATE + 5, price);
        let table = match self.shop_b(MAGIC_TEMPLATE + 2) {
            0 => 0x1EEC,
            1 => 0x1F8D,
            _ => 0x2018,
        };
        let ty = self.shop_b(MAGIC_TEMPLATE + 3) as u16;
        let per = self.mem.r16(DS, table + ty * 2);
        self.mem.w16(DS, 0xA840, per);
        let (lo, hi) = (self.shop_b(0x1DFB + sub * 2) as u16, self.shop_b(0x1DFC + sub * 2) as u16);
        let v = self.shop_rand(lo, hi)?;
        self.shop_mt(4, v as u8);
        // 16-bit product; only its low word is added.
        let add = v.wrapping_mul(per) as u32;
        let p = self.mem.r32(DS, MAGIC_TEMPLATE + 5).wrapping_add(add);
        self.mem.w32(DS, MAGIC_TEMPLATE + 5, p);
        Some(())
    }

    /// 1194:B617. Kind 1, only from item level 7; Some(false) (CF set)
    /// below that.
    fn shop_magic_kind1(&mut self) -> Option<bool> {
        if self.shop_b(LEVEL) < 7 {
            return Some(false);
        }
        self.shop_mt(0, 1);
        self.shop_add_key16(100);
        let forced = self.shop_b(SHOP_FLAG);
        let sub = if self.shop_b(MAGIC_ACTIVE) == 1 && forced != 0xFF {
            forced as u16
        } else {
            let r = self.shop_d100() as u8;
            self.shop_step(0x2042, r)?
        };
        self.shop_mt(1, sub as u8);
        self.shop_add_key16(sub);
        let ty = self.shop_roll_allowed(8, 0x20B3)?;
        self.shop_add_key16(ty);
        self.shop_mt(3, ty as u8);
        match self.shop_b(0x20CB + ty) {
            0xFF => self.shop_mt(9, ty as u8),
            v => {
                self.shop_mt(0x0A, v);
                self.shop_mt(9, 0xFF);
            }
        }
        // 1194:B68C
        let mut p = self.mem.r32(DS, MAGIC_TEMPLATE + 5);
        p = p.wrapping_add(self.mem.r16(DS, 0x20BB + ty * 2) as u32);
        p = p.wrapping_add(self.mem.r16(DS, 0x204A + sub * 2) as u32);
        self.mem.w32(DS, MAGIC_TEMPLATE + 5, p);
        Some(true)
    }

    /// 1194:B6B0. Kind 2: a sub-kind of 4, and a grade from 76..=99
    /// against 20D3; the price is the grade's table entry × 50, cut to
    /// 16 bits.
    fn shop_magic_kind2(&mut self) -> Option<()> {
        self.shop_add_key16(1000);
        self.shop_mt(0, 2);
        // B848 always sets 7109 = 1, so the sub-kind is always rolled.
        let sub = self.shop_rand(0, 4)?;
        self.shop_mt(1, sub as u8);
        self.shop_add_key16(sub);
        let r = self.shop_rand(0x4C, 0x64)? as u8;
        let grade = self.shop_step(0x20D3, r)?;
        self.shop_mt(4, grade as u8);
        self.shop_mt(3, 0xFF);
        self.shop_add_key16(grade);
        // 1194:B702
        let p = self.mem.r16(DS, 0x20D8 + grade * 2).wrapping_mul(50) as u32;
        self.mem.w32(DS, MAGIC_TEMPLATE + 5, p);
        Some(())
    }

    /// 1194:B436. Builds the item record at 13DA from the magic template
    /// at 7033.
    fn shop_magic_item_record(&mut self) {
        self.clear_item();
        let t = |m: &Self, f: u16| m.shop_b(MAGIC_TEMPLATE + f);
        let mut flags = 0x40 | 1 | 8;
        self.mem.w16(DS, ITEM + it::HEALTH, 1);
        self.mem.w16(DS, ITEM + 8, 1);
        match t(self, 0) {
            0 => {
                self.shop_setb(ITEM + 0x10, t(self, 2));
                self.shop_setb(ITEM + 3, t(self, 4));
                self.mem.w16(DS, ITEM + it::HEALTH, t(self, 4) as u16);
                self.shop_setb(ITEM + 0x0F, t(self, 3));
            }
            2 => {
                flags |= 0x20;
                let g = t(self, 4);
                self.shop_setb(ITEM + 0x10, g);
                let v = self.shop_b(0x2678u16.wrapping_add(g as u16 + 3));
                self.shop_setb(ITEM + it::VALUE, v);
                self.shop_setb(ITEM + 0x0F, 0xFF);
            }
            _ => {
                flags |= 0x10;
                self.shop_setb(ITEM + 0x0F, t(self, 3));
            }
        }
        self.shop_setb(ITEM + it::FLAGS, flags);
        self.shop_setb(ITEM + it::TYPE, t(self, 1));
        let p = (self.mem.r32(DS, MAGIC_TEMPLATE + 5) >> 2).wrapping_mul(3);
        self.mem.w32(DS, ITEM + it::PRICE, p);
        self.shop_setb(ITEM + it::SPELL_2, t(self, 9));
        self.shop_setb(ITEM + it::SPELL, t(self, 0x0A));
    }

    /// 1194:B848. A random magic item of item level `level` into the item
    /// record at 13DA (kind 1 needs level 7; below that the record is
    /// left as it was). `forced` (13C8) fixes the trinket sub-kind unless
    /// it is 0xFF. The RNG seed is copied to 7238 first.
    pub fn shop_magic_item(&mut self, level: u8, forced: u8) -> Option<()> {
        self.mem.w32(DS, 0x7238, self.rng.seed);
        self.shop_setb(LEVEL, level);
        self.shop_setb(SHOP_FLAG, forced);
        self.shop_setb(0x12BE, 1);
        self.shop_setb(MAGIC_ACTIVE, 1);
        let kind = self.shop_rand(0, 3)?;
        for i in 0..11 {
            self.shop_mt(i, 0);
        }
        self.mem.w32(DS, MAGIC_TEMPLATE + 5, 0);
        let made = match kind {
            0 => self.shop_magic_kind0().map(|_| true)?,
            1 => self.shop_magic_kind1()?,
            _ => self.shop_magic_kind2().map(|_| true)?,
        };
        if made {
            self.shop_magic_item_record();
        }
        self.shop_setb(MAGIC_ACTIVE, 0);
        Some(())
    }

    /// 1194:B7FA. A random potion into the item record at 13DA: one of 15
    /// types with its price and spell from tables; +4 is DGROUP 05CC +
    /// 05CE (16-bit).
    pub fn shop_potion(&mut self) -> Option<()> {
        self.clear_item();
        let ty = self.shop_rand(0, 15)?;
        self.shop_setb(ITEM + it::TYPE, ty as u8);
        let price = self.mem.r16(DS, POTION_PRICE + ty * 2) as u32;
        self.mem.w32(DS, ITEM + it::PRICE, price);
        let f = self.shop_b(ITEM + it::FLAGS) | 0x40 | 1 | 0x0C;
        self.shop_setb(ITEM + it::FLAGS, f);
        let spell = self.shop_b(POTION_SPELL + ty);
        self.shop_setb(ITEM + it::SPELL, spell);
        self.shop_setb(ITEM + it::SPELL_2, 0xFF);
        let v = self.mem.r16(DS, 0x05CC).wrapping_add(self.mem.r16(DS, 0x05CE));
        self.mem.w16(DS, ITEM + it::VALUE, v);
        Some(())
    }
}

