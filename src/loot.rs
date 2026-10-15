//! Loot and item generation (segments 1194 and 24FA).
//!
//! Items are built in two steps. A 13-byte template at DGROUP 13F3 holds
//! the rolled type, material, enchantment and price; a finisher then turns
//! it into the 19-byte item record at 13DA that goes into an inventory.
//! Only the loot paths are ported: they run with DGROUP 13F0 = 1, which
//! skips the shop-only steps (duplicate check, class check, and copying
//! into the shop list in far memory).
//!
//! Several selection loops reroll until a minimum level is met and never
//! end if it can't be (e.g. enchantments below level 7); the port loops
//! the same way.

use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// Item record being built (19 bytes).
pub const ITEM: u16 = 0x13DA;
/// Item template (13 bytes).
pub const TEMPLATE: u16 = 0x13F3;

/// Item record fields (offsets from `ITEM`): the shared layout.
pub use crate::item;

/// Template fields (offsets from `TEMPLATE`).
pub mod template {
    pub const MAGIC: u16 = 0x01;
    pub const WEAPON: u16 = 0x02;
    /// 0 plate, 1 chain, 2 leather.
    pub const ARMOR_KIND: u16 = 0x03;
    pub const MATERIAL: u16 = 0x04;
    pub const TYPE: u16 = 0x05;
    pub const ENCHANT: u16 = 0x06;
    /// Dword.
    pub const PRICE: u16 = 0x07;
    pub const ENCHANT_SPELL: u16 = 0x0B; // ?
    pub const ENCHANT_VALUE: u16 = 0x0C; // ?
}

use item as it;
use template as t;

/// DGROUP variables.
pub const LEVEL: u16 = 0x13CC;
const FORCED_TYPE: u16 = 0x86B0;
const FORCED_ARMOR_KIND: u16 = 0x101E;
const BASE_PRICE: u16 = 0x1391;
const ARMOR_PASS: u16 = 0x13EF;
const LOOT_MODE: u16 = 0x13F0;
/// Running key used by the shop duplicate check; loot updates it too.
const DEDUP_KEY: u16 = 0x12D0;
const RETRIES: u16 = 0x1294;

/// DGROUP tables.
const W_HANDS: u16 = 0x229E;
const W_CONDITION: u16 = 0x22B0;
const W_PRICE: u16 = 0x22D4;
const W_MIN_LEVEL: u16 = 0x22E6;
const QUALITY_STEPS: u16 = 0x22F8;
const W_DAMAGE: u16 = 0x22FB;
const W_ENCH_SPELL: u16 = 0x23DB;
const W_ENCH_MIN_LEVEL: u16 = 0x23E9;
const W_ENCH_PRICE: u16 = 0x23F7;
const A_PLATE_PRICE: u16 = 0x24F4;
const A_LEATHER_CONDITION: u16 = 0x24FF;
const A_CHAIN_CONDITION: u16 = 0x2515;
const A_PLATE_CONDITION: u16 = 0x252B;
const A_MIN_LEVEL: u16 = 0x2541;
const A_ENCH_SPELL: u16 = 0x2607;
const A_ENCH_MIN_LEVEL: u16 = 0x2615;
const A_ENCH_PRICE: u16 = 0x2623;
const MATERIAL_BONUS: u16 = 0x2678;
const MATERIAL_STEPS: u16 = 0x2680;
const MATERIAL_PRICE: u16 = 0x2882;
const SHIELD_RATING: u16 = 0x86FD;

/// Per armour kind (plate, chain, leather): price and min-level tables.
const ARMOR_KINDS: [(u16, u16); 3] = [(0x24F4, 0x2725), (0x24E9, 0x27C7), (0x24DE, 0x2877)];

/// 24FA: offsets of the corpse being looted (far pointer), loot flags,
/// and the per-creature-type chance table in 3276.
const CORPSE_SEG: u16 = 0x4EFE;
const CORPSE_OFF: u16 = 0x4F00;
pub const LOOT_FLAGS: u16 = 0x4F02;
const LOOT_CHANCE: u16 = 0x4EA5;
const MONSTER_GOLD_CHANCE: u16 = 0x4F0D;
pub use crate::character::field::CREATURE_GOLD;

impl Machine {
    fn b(&self, off: u16) -> u8 {
        self.mem.r8(DS, off)
    }
    fn setb(&mut self, off: u16, v: u8) {
        self.mem.w8(DS, off, v)
    }
    fn tb(&self, f: u16) -> u8 {
        self.b(TEMPLATE + f)
    }
    fn set_tb(&mut self, f: u16, v: u8) {
        self.setb(TEMPLATE + f, v)
    }
    fn iat(&self, at: u16, f: u16) -> u8 {
        self.b(at.wrapping_add(f))
    }
    fn set_iat(&mut self, at: u16, f: u16, v: u8) {
        self.setb(at.wrapping_add(f), v)
    }
    fn or_iat(&mut self, at: u16, f: u16, v: u8) {
        self.set_iat(at, f, self.iat(at, f) | v)
    }
    fn and_iat(&mut self, at: u16, f: u16, v: u8) {
        self.set_iat(at, f, self.iat(at, f) & v)
    }
    fn add_key(&mut self, v: u8) {
        self.setb(DEDUP_KEY, self.b(DEDUP_KEY).wrapping_add(v))
    }
    fn rand(&mut self, lo: u16, hi: u16) -> u16 {
        self.rng.range(lo, hi).expect("nonzero span")
    }
    /// Whether a reroll loop over `0..n` can ever stop: some entry of the
    /// min-level `table` is at or below the item level. The original loops
    /// forever when none is; the port returns None instead.
    fn any_allowed(&self, table: u16, n: u16) -> bool {
        (0..n).any(|i| self.b(table.wrapping_add(i)) <= self.b(LEVEL))
    }

    /// Number of leading `steps` entries that `v` reaches.
    fn steps(&self, table: u16, n: u16, v: u8) -> u16 {
        (0..n).take_while(|&i| v >= self.b(table + i)).count() as u16
    }

    /// 1194:E970. Clears the template.
    pub fn clear_template(&mut self) {
        for i in 0..13 {
            self.setb(TEMPLATE + i, 0);
        }
    }

    /// 1194:E97B (and the 0000:77C8 fill in `finish_weapon`).
    pub fn clear_item(&mut self) {
        for i in 0..19 {
            self.setb(ITEM + i, 0);
        }
    }

    /// 1194:EDBC. Rolls a material and scales the template price.
    pub fn roll_material(&mut self) {
        self.add_key(10);
        self.set_tb(t::ENCHANT, 0xFF);
        // 1..=99: the top threshold (101) can never be reached either.
        let r = self.rand(1, 100) as u8;
        let m = self.steps(MATERIAL_STEPS, 7, r);
        self.add_key(m as u8);
        self.set_tb(t::MATERIAL, m as u8);
        let mul = self.mem.r16(DS, MATERIAL_PRICE + 2 * m) as u32;
        let price = self.mem.r32(DS, TEMPLATE + t::PRICE) & 0xFFFF;
        let p = match mul.wrapping_mul(price) >> 2 {
            0 => 1,
            p => p,
        };
        self.mem.w32(DS, TEMPLATE + t::PRICE, p);
    }

    /// 1194:EE1D. Rolls an enchantment the item level allows. Without
    /// `keep_material` the item is marked as having none. None where the
    /// original hangs: no enchantment is allowed at this level (below 7).
    pub fn roll_enchantment(&mut self, keep_material: bool) -> Option<()> {
        if !keep_material {
            self.set_tb(t::MATERIAL, 0xFF);
            self.add_key(0x14);
        }
        let weapon = self.b(TEMPLATE + t::WEAPON) == 1;
        let (min_level, price, spell) = if weapon {
            (W_ENCH_MIN_LEVEL, W_ENCH_PRICE, W_ENCH_SPELL)
        } else {
            (A_ENCH_MIN_LEVEL, A_ENCH_PRICE, A_ENCH_SPELL)
        };
        if !self.any_allowed(min_level, 14) {
            return crate::giveup::original("1194:EE1D: no enchantment allowed: the original rolls for ever");
        }
        let e = loop {
            let e = self.rand(0, 14);
            if self.b(min_level + e) <= self.b(LEVEL) {
                break e;
            }
        };
        self.add_key(e as u8);
        self.set_tb(t::ENCHANT, e as u8);
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE).wrapping_add(self.mem.r16(DS, price + 2 * e) as u32);
        self.mem.w32(DS, TEMPLATE + t::PRICE, p);
        match self.b(spell + e) {
            0xFF => self.set_tb(t::ENCHANT_SPELL, e as u8),
            v => {
                self.set_tb(t::ENCHANT_VALUE, v);
                self.set_tb(t::ENCHANT_SPELL, 0xFF);
            }
        }
        Some(())
    }

    /// 1194:EC67..EC84 (and ED43..ED60): d10 against 22F8 picks material
    /// only, enchantment only, or both (handler table at 86B1).
    fn roll_quality(&mut self) -> Option<()> {
        let r = self.rand(1, 11) as u8;
        match self.steps(QUALITY_STEPS, 2, r) {
            0 => self.roll_material(),
            1 => self.roll_enchantment(false)?,
            _ => {
                // 1194:EEC3
                self.roll_material();
                self.roll_enchantment(true)?;
            }
        }
        Some(())
    }

    /// 1194:EC02, loot mode only. `forced` is a weapon type or 0xFFFF.
    /// None where the original hangs (no type or enchantment allowed).
    pub fn magic_weapon_template(&mut self, forced: u16) -> Option<()> {
        assert_eq!(self.b(LOOT_MODE), 1, "shop path not ported");
        let saved = self.mem.r16(DS, RETRIES);
        self.mem.w16(DS, RETRIES, 0);
        self.clear_template();
        self.set_tb(t::WEAPON, 1);
        let mut ty = forced;
        let mut forced_try = forced != 0xFFFF;
        loop {
            if !forced_try {
                if !self.any_allowed(W_MIN_LEVEL, 18) {
                    return crate::giveup::original("1194:EC02: no weapon allowed: the original rolls for ever");
                }
                self.setb(DEDUP_KEY, 10);
                ty = self.rand(0, 18);
            }
            forced_try = false;
            if self.b(LEVEL) >= self.b(W_MIN_LEVEL.wrapping_add(ty)) {
                break;
            }
        }
        self.add_key(ty as u8);
        self.set_tb(t::MAGIC, 1);
        self.set_tb(t::WEAPON, 1);
        self.set_tb(t::TYPE, ty as u8);
        let price = self.b(W_PRICE.wrapping_add(ty)) as u32;
        self.mem.w32(DS, TEMPLATE + t::PRICE, price);
        self.mem.w32(DS, BASE_PRICE, price);
        self.roll_quality()?;
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.mem.w16(DS, RETRIES, saved);
        Some(())
    }

    /// 1194:ECED, loot mode only. `forced` is an armour type or 0xFFFF.
    /// None where the original hangs (no type or enchantment allowed).
    pub fn magic_armor_template(&mut self, forced: u16) -> Option<()> {
        assert_eq!(self.b(LOOT_MODE), 1, "shop path not ported");
        self.clear_template();
        self.set_tb(t::WEAPON, 0);
        let mut ty = forced;
        let mut forced_try = forced != 0xFFFF;
        loop {
            if !forced_try {
                if !self.any_allowed(A_MIN_LEVEL, 11) {
                    return crate::giveup::original("1194:ECED: no armour allowed: the original rolls for ever");
                }
                ty = self.rand(0, 11);
            }
            forced_try = false;
            self.setb(DEDUP_KEY, 100);
            if self.b(A_MIN_LEVEL.wrapping_add(ty)) <= self.b(LEVEL) {
                break;
            }
        }
        self.add_key(ty as u8);
        self.set_tb(t::TYPE, ty as u8);
        let price = self.b(A_PLATE_PRICE.wrapping_add(ty)) as u32;
        self.mem.w32(DS, TEMPLATE + t::PRICE, price);
        self.set_tb(t::MAGIC, 1);
        self.roll_quality()?;
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        Some(())
    }

    /// 1194:F312. Health from a condition code: 0x80 -> 50, 0x40 -> 25,
    /// otherwise the high byte times 100. Magic items get their template
    /// price instead when it is higher, capped at 50000. Into the item at
    /// `at` (the original's SI).
    pub fn set_condition(&mut self, code: u16) {
        self.set_condition_at(ITEM, code);
    }

    /// `set_condition` into the item at `at`.
    pub fn set_condition_at(&mut self, at: u16, code: u16) {
        let mut h = match code {
            0x80 => 50,
            0x40 => 25,
            _ => (code >> 8) * 100,
        };
        let store = |m: &mut Self, h: u16| {
            m.mem.w16(DS, at + it::HEALTH, h);
            m.mem.w16(DS, at + it::MAX_HEALTH, h);
        };
        store(self, h);
        if self.tb(t::MAGIC) != 0 {
            let price = self.mem.r32(DS, TEMPLATE + t::PRICE);
            if price > 50000 {
                store(self, 50000);
            } else if price as u16 > h {
                h = price as u16;
                store(self, h);
            }
        }
    }

    /// Price of a finished item: three quarters of the base price, at
    /// least 1 (dword, overflow wraps).
    fn item_price(&self) -> u32 {
        match self.mem.r32(DS, BASE_PRICE).wrapping_mul(3) >> 2 {
            0 => 1,
            p => p,
        }
    }

    /// 1194:F031. Turns the template into a weapon item at ITEM (13DA).
    pub fn finish_weapon(&mut self) {
        self.finish_weapon_at(ITEM);
    }

    /// 1194:F031 with SI `at`: ITEM (13DA) cleared, then the weapon's
    /// fields set (or OR'd into) the item at `at`, as the original writes
    /// through SI.
    pub fn finish_weapon_at(&mut self, at: u16) {
        self.clear_item();
        self.or_iat(at, it::FLAGS, 0x40);
        self.or_iat(at, it::FLAGS, 4);
        // 1194:F0DA
        let ty = self.tb(t::TYPE) as u16;
        let dmg = self.mem.r16(DS, W_DAMAGE + 2 * ty);
        self.mem.w16(DS, at + it::DAMAGE_MIN, dmg);
        self.set_iat(at, it::HANDS, self.b(W_HANDS + ty));
        if self.tb(t::MAGIC) != 0 {
            self.or_iat(at, it::FLAGS, 1);
            let m = self.tb(t::MATERIAL);
            self.set_iat(at, it::MATERIAL, m);
            if m == 0 {
                self.set_iat(at, it::DAMAGE_MAX, self.iat(at, it::DAMAGE_MAX).wrapping_sub(2));
            } else if m != 0xFF {
                // Sign-extended, then divided unsigned.
                let bonus = self.b(MATERIAL_BONUS + m as u16) as i8 as i16 as u16;
                let v = (bonus / 5) as u8;
                self.set_iat(at, it::DAMAGE_MIN, self.iat(at, it::DAMAGE_MIN).wrapping_add(v));
                self.set_iat(at, it::DAMAGE_MAX, self.iat(at, it::DAMAGE_MAX).wrapping_add(v));
            }
            self.set_iat(at, it::ENCHANT, self.tb(t::ENCHANT));
            self.set_iat(at, it::ENCHANT_SPELL, self.tb(t::ENCHANT_SPELL));
            self.set_iat(at, it::ENCHANT_VALUE, self.tb(t::ENCHANT_VALUE));
        } else {
            self.and_iat(at, it::FLAGS, 0xFE);
            self.set_iat(at, it::MATERIAL, 0xFF);
        }
        let p = self.item_price();
        self.mem.w32(DS, at + it::PRICE, p);
        self.mem.w32(DS, BASE_PRICE, p);
        self.set_iat(at, it::TYPE, ty as u8);
        let code = self.mem.r16(DS, W_CONDITION + 2 * ty);
        self.mem.w16(DS, at + it::CONDITION_CODE, code);
        self.set_condition_at(at, code);
    }

    /// 1194:EEE2. Turns the template into an armour item at ITEM (13DA).
    pub fn finish_armor(&mut self) {
        self.finish_armor_at(ITEM);
    }

    /// 1194:EEE2 with SI `at`: ITEM (13DA) cleared (E97B), then the
    /// armour's fields set (or OR'd into) the item at `at`.
    pub fn finish_armor_at(&mut self, at: u16) {
        self.clear_item();
        self.or_iat(at, it::FLAGS, 0x40);
        self.and_iat(at, it::FLAGS, 0xCF);
        let ty = self.tb(t::TYPE);
        self.set_iat(at, it::TYPE, ty);
        let p = self.item_price();
        self.mem.w32(DS, at + it::PRICE, p);
        let ty = ty as u16;
        if self.tb(t::MAGIC) == 1 {
            // Magic armour always uses the plate condition table.
            let code = self.mem.r16(DS, A_PLATE_CONDITION + 2 * ty);
            self.mem.w16(DS, at + it::CONDITION_CODE, code);
            self.set_condition_at(at, code);
            self.or_iat(at, it::FLAGS, 1);
            self.set_iat(at, it::DAMAGE_MIN, 0x2D);
            let m = self.tb(t::MATERIAL);
            self.set_iat(at, it::MATERIAL, m);
            if m != 0xFF {
                let r = self.iat(at, it::DAMAGE_MIN).wrapping_add(self.b(MATERIAL_BONUS + m as u16));
                self.set_iat(at, it::DAMAGE_MIN, r);
                self.or_iat(at, it::FLAGS, 1);
            }
            self.set_iat(at, it::ENCHANT, self.tb(t::ENCHANT));
            self.set_iat(at, it::ENCHANT_SPELL, self.tb(t::ENCHANT_SPELL));
            self.set_iat(at, it::ENCHANT_VALUE, self.tb(t::ENCHANT_VALUE));
        } else {
            self.and_iat(at, it::FLAGS, 0xFE);
            let (kind_bits, table, kind, rating) = match self.tb(t::ARMOR_KIND) {
                0 => (0x00, A_PLATE_CONDITION, 0, 0x2D),
                1 => (0x10, A_CHAIN_CONDITION, 1, 0x1E),
                _ => (0x20, A_LEATHER_CONDITION, 2, 0x0F),
            };
            self.or_iat(at, it::FLAGS, kind_bits);
            let code = self.mem.r16(DS, table + 2 * ty);
            self.mem.w16(DS, at + it::CONDITION_CODE, code);
            self.set_condition_at(at, code);
            self.set_iat(at, it::MATERIAL, kind);
            self.set_iat(at, it::DAMAGE_MIN, rating);
        }
        // 1194:7502: types 7..=10 are shields, with a fixed rating.
        let ty = self.iat(at, it::TYPE) as u16;
        if (7..=10).contains(&ty) {
            self.set_iat(at, it::DAMAGE_MIN, self.b(SHIELD_RATING + ty - 7));
            self.and_iat(at, it::FLAGS, 0xCF);
            let m = self.iat(at, it::MATERIAL) as i8;
            if self.iat(at, it::FLAGS) & 1 != 0 && m != -1 {
                let bonus = self.b(MATERIAL_BONUS.wrapping_add(m as i16 as u16));
                let r = self.iat(at, it::DAMAGE_MIN).wrapping_add(bonus);
                self.set_iat(at, it::DAMAGE_MIN, r.min(0x2D));
            }
        }
    }

    /// 1194:E9C4. Picks a type of one armour kind from its price and
    /// min-level tables. Unforced, it rolls once and then walks up the
    /// table until the level allows one; a forced type that fails leaves
    /// the template untouched.
    pub fn pick_armor(&mut self, prices: u16, min_levels: u16) {
        let forced = self.b(FORCED_TYPE);
        let mut ty = if forced != 0xFF {
            self.setb(LEVEL, 0x14);
            forced as u16
        } else {
            self.rand(0, 11)
        };
        loop {
            if self.b(LEVEL) >= self.b(min_levels.wrapping_add(ty)) {
                self.set_tb(t::TYPE, ty as u8);
                let p = self.b(prices.wrapping_add(ty)) as u32;
                self.mem.w32(DS, TEMPLATE + t::PRICE, p);
                self.mem.w32(DS, BASE_PRICE, p);
                return;
            }
            if forced != 0xFF {
                return;
            }
            ty = ty.wrapping_add(1);
        }
    }

    /// 1194:E986 with the loot flag (13EF) set. A kind is rolled but
    /// discarded unless DGROUP 101E forces one, so loot armour is plate.
    fn pick_armor_kind(&mut self) {
        self.set_tb(t::WEAPON, 0);
        assert_ne!(self.b(ARMOR_PASS), 0, "shop path not ported");
        let rolled = self.rand(0, 3);
        let _ = rolled;
        let kind = match self.b(FORCED_ARMOR_KIND) {
            0xFF => 0,
            k => k as u16,
        };
        // Handlers 1194:EA83 / EAA0 / EABD.
        self.set_tb(t::ARMOR_KIND, kind as u8);
        let (prices, min_levels) = match kind {
            0..=2 => ARMOR_KINDS[kind as usize],
            _ => panic!("armour kind {kind} jumps through a bad table entry"),
        };
        self.pick_armor(prices, min_levels);
    }

    /// 1194:E889. Plain (non-magic) armour of the given level; `forced`
    /// is a type or 0xFF.
    pub fn loot_armor(&mut self, level: u8, forced: u8) {
        self.setb(FORCED_TYPE, forced);
        self.setb(LEVEL, level);
        self.clear_item();
        self.clear_template();
        self.setb(ARMOR_PASS, 1);
        self.pick_armor_kind();
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.finish_armor();
        self.setb(ARMOR_PASS, 0);
    }

    /// 1194:E8B2. Plain weapon of the given level; `forced` is a type or
    /// 0xFF (forcing raises the level to 20).
    pub fn loot_weapon(&mut self, level: u8, forced: u8) -> Option<()> {
        self.setb(FORCED_TYPE, forced);
        self.setb(LEVEL, level);
        self.clear_item();
        self.clear_template();
        loop {
            let ty = if self.b(FORCED_TYPE) == 0xFF {
                if !self.any_allowed(W_MIN_LEVEL, 18) {
                    return crate::giveup::original("1194:E8B2: no weapon allowed: the original rolls for ever");
                }
                self.rand(0, 18)
            } else {
                self.setb(LEVEL, 0x14);
                self.b(FORCED_TYPE) as u16
            };
            self.set_tb(t::WEAPON, 1);
            if self.b(LEVEL) >= self.b(W_MIN_LEVEL + ty) {
                self.set_tb(t::TYPE, ty as u8);
                let p = self.b(W_PRICE + ty) as u32;
                self.mem.w32(DS, TEMPLATE + t::PRICE, p);
                self.mem.w32(DS, BASE_PRICE, p);
                break;
            }
            if self.b(FORCED_TYPE) != 0xFF {
                // The forced type is retried unchanged: the original hangs.
                return crate::giveup::original("1194:E8B2: the forced type retried unchanged: the original hangs");
            }
        }
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.setb(ARMOR_PASS, 1);
        self.finish_weapon();
        Some(())
    }

    /// 1194:E922. Magic weapon; `forced` is a type or 0xFFFF (forcing
    /// raises the level to 20).
    pub fn loot_magic_weapon(&mut self, level: u8, forced: u16) -> Option<()> {
        self.setb(LEVEL, level);
        self.clear_item();
        self.setb(LOOT_MODE, 1);
        if forced != 0xFFFF {
            self.setb(LEVEL, 0x14);
        }
        self.magic_weapon_template(forced)?;
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.finish_weapon();
        self.setb(LOOT_MODE, 0);
        Some(())
    }

    /// 1194:E94E. Magic armour; `forced` is a type or 0xFFFF.
    pub fn loot_magic_armor(&mut self, level: u8, forced: u16) -> Option<()> {
        self.setb(LEVEL, level);
        self.clear_item();
        self.setb(LOOT_MODE, 1);
        self.magic_armor_template(forced)?;
        let p = self.mem.r32(DS, TEMPLATE + t::PRICE);
        self.mem.w32(DS, BASE_PRICE, p);
        self.finish_armor();
        self.setb(LOOT_MODE, 0);
        Some(())
    }

    /// 24FA:0300. 1..=100.
    pub fn d100(&mut self) -> u16 {
        self.rand(1, 101)
    }

    /// 24FA:0009..003C. Rolls which kinds of loot a corpse of creature
    /// record `rec` (in DGROUP) has: four flags at DGROUP 4F02 for gold,
    /// magic item, weapon or armour, and a fourth kind, each set when
    /// d100 <= the creature type's chance in 3276:4EA5.
    pub fn roll_loot_flags(&mut self, rec: u16) {
        let chances = LOOT_CHANCE.wrapping_add((self.b(rec.wrapping_add(4)) as u16) << 2);
        self.mem.w32(DS, LOOT_FLAGS, 0);
        for i in 0..4 {
            let r = self.d100() as u8;
            if r <= self.mem.r8(DATA, chances.wrapping_add(i)) {
                self.setb(LOOT_FLAGS + i, 1);
            }
        }
    }

    /// 24FA:009C. Gold on the corpse at the far pointer in DGROUP
    /// 4EFE:4F00. The chance check is inverted relative to the flag roll:
    /// gold is given when 0..=100 is at least the chance. Returns false
    /// (carry set) when there is none; then nothing is written.
    pub fn roll_corpse_gold(&mut self) -> bool {
        let (seg, rec) = (self.mem.r16(DS, CORPSE_SEG), self.mem.r16(DS, CORPSE_OFF));
        let kind = self.mem.r8(seg, rec.wrapping_add(4)) as u16;
        let chance = self.mem.r8(DATA, LOOT_CHANCE.wrapping_add(kind << 2)) as u16;
        let r = self.rand(0, 101);
        if chance > r {
            return false;
        }
        let g = self.rand(1, 11);
        let level = self.mem.r8(seg, rec.wrapping_add(6)) as u16 + 1;
        self.mem.w16(seg, rec.wrapping_add(CREATURE_GOLD), g.wrapping_mul(level));
        true
    }

    /// 24FA:004D. Gold a creature (record at DGROUP `rec`) carries: by
    /// class byte (0x80 set / 0x20 set / other), a d100 against 4F0D,
    /// then 1..=50 gold.
    pub fn roll_creature_gold(&mut self, rec: u16) {
        let class = self.b(rec.wrapping_add(5));
        let kind = if class & 0x80 != 0 {
            0
        } else if class & 0x20 != 0 {
            2
        } else {
            1
        };
        let chance = self.b(MONSTER_GOLD_CHANCE + kind) as u16;
        let gold = if self.d100() > chance { 0 } else { self.rand(1, 0x33) };
        self.mem.w16(DS, rec.wrapping_add(CREATURE_GOLD), gold);
    }
}
