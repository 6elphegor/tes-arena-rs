//! The piles of treasure made (2433:0000).
//!
//! A pile is a 1Ch-byte record in a table in EMS bank 6 (at A000 in the
//! frame, PILE_COUNT of them, at most C8h): +0 in use, +1 the kind of
//! place, +2 the square, +4 the level, +5 gold (FFFFh none), +7 a name
//! (FFh none), +9 an item (13h bytes). The record being made is PILE
//! (4BF6).
//!
//! Two callers set MAKING (4C91) first:
//! - the loot window (2433:0583), clicking a square with no pile: one
//!   round, MAKING 0, the piles going to the clicked square (12C0);
//! - a dungeon's level entered (0000:44D7), MAKING FFh: as many rounds
//!   as the level has spots for piles (C765) times 50%, each at a spot
//!   not yet taken this time (the table of spots at C6E5) and, unless the
//!   level already has as many piles as that (2433:0101), marked on the
//!   map with a flat. Not in the town's buildings but the palace (8) and
//!   0Bh, unless they were broken into (191D); not with C8h piles in use
//!   or fewer than two spots.
//!
//! Each round rolls for each of four kinds of treasure (2433:029F: d100
//! at most the table at 4D5A for the kind of place) and appends a pile
//! for each that comes (2433:0258), the same record reused:
//! - gold (2433:0358), an amount by the kind of place (the table of
//!   routines at 4D22);
//! - an item (036C): on an even RNG seed a potion (1194:B7FA), else a
//!   magic item (B848) of the level the table at 4D2C gives (none below
//!   3, nor when B848 makes none);
//! - plain armour or a weapon (041C, a coin), level 10h, its condition a
//!   share (25%, 75% or 100%) of its maximum;
//! - a magic weapon or armour (04C8, a coin), level 10h.
//!
//! Original quirks kept: the gold routine halving by `shr` (climate 2)
//! leaves an odd amount's low bit in the carry, which drops that pile;
//! the level handler's result is unused by the two last kinds, but its
//! BX (the span of its roll, or the doubled kind) is the forced type the
//! magic generators then take (so such piles' types are fixed by the kind
//! of place, at level 14h); the full-level test reads its limit through
//! the frame's segment (frame:4D76, not DGROUP's); in the wild (kind 5)
//! the routine tables are overread, and an item, a weapon or a magic
//! pile there calls into a string (2433:7525): None, as the original
//! runs off into data.

use crate::loot_window::var::{MAKING, PILE, PILE_COUNT, ROLLS};
use crate::machine::Machine;
use crate::mem::DS;
use crate::random::CycleCheck;

/// DGROUP variables.
pub mod var {
    /// The piles' limit for a level (2433:02C3), the bank to put back,
    /// and the spots already taken this time (19h words, the spot + 64h
    /// as bytes).
    pub const LIMIT: u16 = 0x4D76;
    pub const BANK: u16 = 0x12C4;
    pub const TAKEN: u16 = 0x1395;
    /// The level's spots for piles: their count and (column, row) pairs.
    pub const SPOTS: u16 = 0xC765;
    pub const SPOT: u16 = 0xC6E5;
    /// The flats piles are shown with on the map (by kind, 3..6).
    pub const FLATS: u16 = 0xC934;
    /// The roll tables: the four kinds' chances by the kind of place
    /// (4 bytes each), the routines for gold and for an item's level (by
    /// the kind of place), the conditions' shares.
    pub const CHANCES: u16 = 0x4D5A;
    pub const GOLD_ROUTINES: u16 = 0x4D22;
    pub const LEVEL_ROUTINES: u16 = 0x4D2C;
    pub const FINE: u16 = 0x4D6E;
    pub const SHARES: u16 = 0x4D73;
    pub const GOLD_BY_CLIMATE: u16 = 0x4BEE;
}
use var::*;

/// Fields of PILE.
const KIND: u16 = PILE + 1;
const SQUARE: u16 = PILE + 2;
const LEVEL_AT: u16 = PILE + 4;
const GOLD: u16 = PILE + 5;
const NAME: u16 = PILE + 7;
const ITEM_AT: u16 = PILE + 9;
/// The item made (13h bytes at 13DA): its condition and maximum, flags.
const ITEM: u16 = 0x13DA;
const ITEM_LEVEL: u16 = 0x13CC;

const FRAME_SEG: u16 = 0x9030;
const LEVEL: u16 = 0xBB0A;
const CLIMATE: u16 = 0x0A47;
const MAP_SEG: u16 = 0xD806;
const CLICKED_SQUARE: u16 = 0x12C0;
const TABLE: u16 = 0xA000;

impl Machine {
    /// 2433:0000. See the module's notes. None where the original hangs
    /// (no free spot left), faults (a roll over an empty range) or runs
    /// into data.
    pub fn piles(&mut self) -> Option<()> {
        for i in 0..0x19u16 {
            self.mem.w16(DS, TAKEN + 2 * i, 0);
        }
        let making = self.mem.r8(DS, MAKING) != 0;
        let mut cx: u32 = 1;
        if making {
            if self.mem.r8(DS, PILE_COUNT) == 0xC8 {
                return Some(());
            }
            let d48 = self.mem.r8(DS, 0x0D48);
            if self.mem.r8(DS, 0x05E6) == 4 && d48 != 8 && d48 != 0xB && self.mem.r8(DS, 0x191D) == 0 {
                return Some(());
            }
            if self.mem.r8(DS, SPOTS) as i8 <= 1 {
                return Some(());
            }
            let n = self.pile_limit();
            cx = if n == 0 { 0x10000 } else { n as u32 };
            let old = self.map_bank(6, n);
            self.mem.w16(DS, BANK, old);
        }
        for _ in 0..cx {
            self.map_bank(6, 0);
            if self.mem.r8(DS, PILE_COUNT) == 0 {
                let frame = self.mem.r16(DS, FRAME_SEG);
                for i in 0..0x15E0u16 {
                    self.mem.w8(frame, TABLE + i, 0);
                }
            }
            let place = self.pile_place();
            let level = self.mem.r8(DS, LEVEL);
            self.mem.w8(DS, LEVEL_AT, level);
            self.mem.w8(DS, KIND, place);
            if making && self.level_full() {
                continue;
            }
            self.pile_rolls()?;
            if self.mem.r32(DS, ROLLS) != 0 {
                self.mem.w8(DS, PILE, 1);
                if making {
                    self.pile_spot()?;
                }
            }
            for i in 0..4u16 {
                if self.mem.r8(DS, ROLLS + i) == 0 {
                    continue;
                }
                if self.pile_treasure(i)? {
                    continue;
                }
                if !making {
                    let sq = self.mem.r16(DS, CLICKED_SQUARE);
                    self.mem.w16(DS, SQUARE, sq);
                    let level = self.mem.r8(DS, LEVEL);
                    self.mem.w8(DS, LEVEL_AT, level);
                }
                self.pile_add();
            }
            if making {
                self.pile_flat()?;
            }
            self.mem.w32(DS, ROLLS, 0);
        }
        let bank = self.mem.r16(DS, BANK);
        self.map_bank(bank, 0);
        Some(())
    }

    /// 2433:02C3. The level's limit: half its spots (8-bit product, then
    /// / 100 of the x 50), at least 1, as LIMIT's byte; returned.
    fn pile_limit(&mut self) -> u16 {
        let ax = self.mem.r8(DS, SPOTS) as u16 * 0x32 / 0x64;
        let ax = if ax == 0 { 1 } else { ax };
        self.mem.w8(DS, LIMIT, ax as u8);
        ax
    }

    /// 2433:0101. Whether the level has its limit of piles already:
    /// counting the table's piles of this level, the count's low byte
    /// against the limit read through the frame's segment.
    fn level_full(&self) -> bool {
        let frame = self.mem.r16(DS, FRAME_SEG);
        let level = self.mem.r8(DS, LEVEL);
        let limit = self.mem.r8(frame, LIMIT);
        let mut ax = 0u16;
        let mut si = TABLE;
        for _ in 0..self.mem.r8(DS, PILE_COUNT) {
            if self.mem.r8(frame, si.wrapping_add(4)) == level {
                ax = ax.wrapping_add(1);
                if ax as u8 >= limit {
                    return true;
                }
            }
            si = si.wrapping_add(0x1C);
        }
        false
    }

    /// 2433:029F. A d100 (1..100) for each kind of treasure: at most its
    /// chance (the kind of place's four bytes at CHANCES) marks it in
    /// ROLLS; a kind that misses keeps what ROLLS held.
    fn pile_rolls(&mut self) -> Option<()> {
        let si = CHANCES + 4 * self.mem.r8(DS, KIND) as u16;
        for i in 0..4u16 {
            let roll = self.rng.range(1, 0x65)?;
            if roll as u8 <= self.mem.r8(DS, si + i) {
                self.mem.w8(DS, ROLLS + i, 1);
            }
        }
        Some(())
    }

    /// 2433:02F8. A spot not yet taken this time (2433:02DE: TAKEN's
    /// bytes, the spot + 64h, end at a zero word), whose table entry isn't
    /// FFh: its square (the column doubled in 8 bits, the row) to SQUARE.
    /// A taken spot is rolled again; one whose entry is FFh is taken and
    /// rolled again. None where no spot is left: the original rolls for
    /// ever.
    fn pile_spot(&mut self) -> Option<()> {
        let n = self.mem.r8(DS, SPOTS) as u16;
        if !self.spot_left(n) {
            return None;
        }
        let mut check = CycleCheck::new(&self.rng);
        loop {
            let ax = self.rng.range(0, n.wrapping_sub(1))?;
            let al = (ax as u8).wrapping_add(0x64);
            let mut si = TAKEN;
            let mut taken = false;
            for _ in 0..0x10000u32 {
                if self.mem.r16(DS, si) == 0 {
                    self.mem.w8(DS, si, al);
                    break;
                }
                if self.mem.r8(DS, si) == al {
                    taken = true;
                    break;
                }
                si = si.wrapping_add(1);
            }
            if !taken {
                let bx = ax.wrapping_mul(2);
                let col = self.mem.r8(DS, SPOT.wrapping_add(bx));
                if col != 0xFF {
                    let row = self.mem.r8(DS, SPOT.wrapping_add(bx).wrapping_add(1));
                    self.mem.w16(DS, SQUARE, (row as u16) << 8 | col.wrapping_add(col) as u16);
                    return Some(());
                }
            }
            if check.cycled(&self.rng) {
                return crate::giveup::original("2433:02F8: no spot left: the original rolls for ever");
            }
        }
    }

    /// Whether a roll of 2433:02F8 can still end: a spot among the
    /// `n` - 1 it rolls from that isn't taken (TAKEN's bytes up to its
    /// first zero word) and whose entry isn't FFh. (A taken spot, or one
    /// with FFh, which is taken when rolled, never ends it.)
    fn spot_left(&self, n: u16) -> bool {
        let mut taken = Vec::new();
        let mut si = TAKEN;
        for _ in 0..0x10000u32 {
            if self.mem.r16(DS, si) == 0 {
                break;
            }
            taken.push(self.mem.r8(DS, si));
            si = si.wrapping_add(1);
        }
        (0..n.wrapping_sub(1)).any(|i| {
            !taken.contains(&(i as u8).wrapping_add(0x64)) && self.mem.r8(DS, SPOT.wrapping_add(i.wrapping_mul(2))) != 0xFF
        })
    }

    /// The handler for kind `i` (the table at 4D1A: 0358, 036C, 041C,
    /// 04C8). Whether it returns the carry (no pile).
    fn pile_treasure(&mut self, i: u16) -> Option<bool> {
        match i {
            0 => {
                let (ax, cf) = self.pile_gold()?;
                self.mem.w8(DS, NAME, 0xFF);
                self.mem.w16(DS, GOLD, ax);
                Some(cf)
            }
            1 => self.pile_potion_or_magic(),
            2 => self.pile_weapon().map(|_| false),
            _ => self.pile_magic().map(|_| false),
        }
    }

    /// The routine at word `table` + 2 x the kind of place, by its
    /// address in segment 2433. None for one that isn't code.
    fn pile_routine(&self, table: u16) -> u16 {
        let k = self.mem.r8(DS, KIND) as u16;
        self.mem.r16(DS, table.wrapping_add(k.wrapping_add(k)))
    }

    /// 2433:0358's routine from GOLD_ROUTINES: the amount, and the carry
    /// it leaves. Each stores the amount at GOLD too.
    fn pile_gold(&mut self) -> Option<(u16, bool)> {
        let (ax, cf) = match self.pile_routine(GOLD_ROUTINES) {
            0x052A => self.gold_small()?,
            0x0546 => {
                let a = self.mem.r8(DS, CLIMATE) as u16;
                (self.mem.r16(DS, GOLD_BY_CLIMATE.wrapping_add(a.wrapping_add(a))), false)
            }
            0x0555 => {
                let (ax, _) = self.gold_small()?;
                let p = ax as u32 * 10;
                (p as u16, p > 0xFFFF)
            }
            0x0561 | 0x0573 => {
                let roll = self.rng.range(1, 0x65)?;
                let l = self.mem.r8(DS, LEVEL) as u16;
                let (s, cf) = l.wrapping_mul(l).overflowing_add(roll);
                (s, cf)
            }
            // The level routines, read past the table (the wild).
            0x03D5 => (self.rng.range(1, 6)?, false),
            _ => return crate::giveup::original("2433:0358: a level past the table: the original jumps into a string"),
        };
        self.mem.w16(DS, GOLD, ax);
        Some((ax, cf))
    }

    /// 2433:052A. 2..10, halved (`shr`, the low bit to the carry) in
    /// climate 2, else doubled.
    fn gold_small(&mut self) -> Option<(u16, bool)> {
        let ax = self.rng.range(2, 0xB)?;
        let r = if self.mem.r8(DS, CLIMATE) == 2 { (ax >> 1, ax & 1 != 0) } else { (ax.wrapping_add(ax), false) };
        self.mem.w16(DS, GOLD, r.0);
        Some(r)
    }

    /// The routine from LEVEL_ROUTINES: an item's level, and the BX it
    /// leaves (6EEA leaves the span of its roll).
    fn pile_level(&mut self) -> Option<(u8, u16)> {
        let k = self.mem.r8(DS, KIND) as u16;
        Some(match self.pile_routine(LEVEL_ROUTINES) {
            0x03D5 => (self.rng.range(1, 6)? as u8, 5),
            0x03ED => {
                let ax = match self.mem.r8(DS, CLIMATE) {
                    0 => 0x10,
                    1 => 0x0E,
                    _ => 0x0C,
                };
                (ax, k.wrapping_add(k))
            }
            0x03E1 => (self.rng.range(2, 0xB)? as u8, 9),
            0x0405 => ((self.mem.r8(DS, LEVEL) as u16 + 1).wrapping_mul(5) as u8, 5),
            0x0411 => ((self.mem.r8(DS, LEVEL) as u16).wrapping_mul(3) as u8, 3),
            _ => return crate::giveup::original("a level past the table: the original jumps into a string"),
        })
    }

    /// 2433:036C. A potion on an even seed, else a magic item of the
    /// level (none below 3, nor when 1194:B848 makes none: the carry);
    /// the item flagged (+0Eh bit 2) and copied to the pile.
    fn pile_potion_or_magic(&mut self) -> Option<bool> {
        self.mem.w16(DS, GOLD, 0xFFFF);
        let (level, _) = self.pile_level()?;
        self.mem.w8(DS, ITEM_LEVEL, level);
        if self.rng.seed & 1 == 0 {
            self.mem.w8(DS, NAME, 0xFF);
            self.mem.w16(DS, GOLD, 0xFFFF);
            self.shop_potion()?;
        } else {
            if level < 3 {
                return Some(true);
            }
            if !self.shop_magic_item_made(level, 0xFF)? {
                return Some(true);
            }
            let f = self.mem.r8(DS, ITEM + 0xE) | 2;
            self.mem.w8(DS, ITEM + 0xE, f);
            self.mem.w8(DS, NAME, 0xFF);
        }
        let f = self.mem.r8(DS, ITEM + 0xE) | 2;
        self.mem.w8(DS, ITEM + 0xE, f);
        self.pile_take_item();
        Some(false)
    }

    /// 2433:041C. Plain armour or a weapon (a coin) of level 10h, its
    /// condition (+6) a share of its maximum (+8): 25%, 75% or 100% (the
    /// shares at SHARES, by a roll of 0..2, or 1..3 with 3 made 2 where
    /// the kind of place's FINE byte is set), at least 1.
    fn pile_weapon(&mut self) -> Option<()> {
        self.pile_level()?;
        self.mem.w8(DS, ITEM_LEVEL, 0x10);
        self.mem.w8(DS, NAME, 0xFF);
        self.mem.w16(DS, GOLD, 0xFFFF);
        if self.rng.next() & 1 == 0 {
            self.mem.w8(DS, 0x101E, 0xFF);
            self.loot_armor(0x10, 0xFF);
        } else {
            self.loot_weapon(0x10, 0xFF)?;
        }
        let k = self.mem.r8(DS, KIND) as u16;
        let i = if self.mem.r8(DS, FINE.wrapping_add(k)) != 0 {
            let a = self.rng.range(1, 4)?;
            if a == 3 { 2 } else { a }
        } else {
            self.rng.range(0, 3)?
        };
        let share = self.mem.r8(DS, SHARES.wrapping_add(i)) as u32;
        let q = share * self.mem.r16(DS, ITEM + 8) as u32 / 0x64;
        if q > 0xFFFF {
            // DIV's quotient overflows: the original faults.
            return crate::giveup::original("2433:041C: the share's divide overflows");
        }
        let q = if q == 0 { 1 } else { q as u16 };
        self.mem.w16(DS, ITEM + 6, q);
        self.pile_take_item();
        Some(())
    }

    /// 2433:04C8. A magic weapon or armour (a coin), level 10h; its forced
    /// type is the BX the level routine left. Flagged (+0Eh bit 2) unless
    /// +0Fh is FFh.
    fn pile_magic(&mut self) -> Option<()> {
        let (_, bx) = self.pile_level()?;
        self.mem.w8(DS, ITEM_LEVEL, 0x10);
        self.mem.w8(DS, NAME, 0xFF);
        self.mem.w16(DS, GOLD, 0xFFFF);
        if self.rng.next() & 1 != 0 {
            self.loot_magic_weapon(0x10, bx)?;
        } else {
            self.loot_magic_armor(0x10, bx)?;
        }
        if self.mem.r8(DS, ITEM + 0xF) != 0xFF {
            let f = self.mem.r8(DS, ITEM + 0xE) | 2;
            self.mem.w8(DS, ITEM + 0xE, f);
        }
        self.pile_take_item();
        Some(())
    }

    /// The item (13h bytes at 13DA) copied into the pile (0000:7732).
    fn pile_take_item(&mut self) {
        for i in 0..0x13u16 {
            let b = self.mem.r8(DS, ITEM + i);
            self.mem.w8(DS, ITEM_AT + i, b);
        }
    }

    /// 2433:0258. PILE appended to the table (after 2433:027D's fix):
    /// at A000 + 1Ch x PILE_COUNT (8-bit product) in the frame; the count
    /// up by one.
    fn pile_add(&mut self) {
        let frame = self.mem.r16(DS, FRAME_SEG);
        let n = self.mem.r8(DS, PILE_COUNT);
        let di = TABLE.wrapping_add(0x1C * n as u16);
        self.pile_fix(DS, PILE);
        for i in 0..0x1Cu16 {
            let b = self.mem.r8(DS, PILE + i);
            self.mem.w8(frame, di.wrapping_add(i), b);
        }
        self.mem.w8(DS, PILE_COUNT, n.wrapping_add(1));
    }

    /// 2433:01C8. The pile shown on the map at SQUARE: a flat from FLATS
    /// (3 for a pile of only the fourth kind, else 4..6 at random) with
    /// the high byte 80h.
    fn pile_flat(&mut self) -> Option<()> {
        let ax = if self.mem.r32(DS, ROLLS) == 0x0100_0000 { 3 } else { self.rng.range(4, 7)? };
        let al = self.mem.r8(DS, FLATS.wrapping_add(ax));
        let map = self.mem.r16(DS, MAP_SEG);
        let sq = self.mem.r16(DS, SQUARE);
        self.mem.w16(map, sq, 0x8000 | al as u16);
        Some(())
    }
}

