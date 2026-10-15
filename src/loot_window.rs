//! The loot window (2433:0583, reached through 1194:7B16): what clicking
//! a corpse, a key, the quest's staff piece or a pile of treasure puts
//! into the player's hands.
//!
//! The click (292B:06DC, `talk::object_click`) passes the kind in AH, a
//! value in AL, a record at FS:SI and a square in BX (all kept at
//! 12BC..12C2):
//!
//! - AH 3, a key (AL 1..): it goes in the first free key byte (C5h) of
//!   the twelve at 652, scanning on through DGROUP if there is none, and
//!   "You found the %s key" is shown (2433:0AA1);
//! - AH 0Fh with AL the staff's flat (C941): the keys are taken away, the
//!   piece's sound plays, and either the pieces count (F77) moves on and
//!   its text is shown (2433:0BF4) with the flat taken from the map, or,
//!   holding all eight, the quest's end (1194:D986) begins and the area
//!   is left;
//! - AH 0 or 1, a corpse (creature or person, the record at FS:SI): its
//!   gold (+407h) goes to the player's (5C2), then a list of its items
//!   (the occupied slots up to the first empty one) is shown until the
//!   player leaves it or nothing is left; the item picked moves into the
//!   player's first free slot if it isn't too heavy (`carry_check`);
//! - any other (AH 0Fh with another flat): the piles of treasure at square
//!   BX (EMS bank 6 at A000: 200 records of 1Ch, FF2 in use; a level's
//!   piles keep their level, +4, the kind of place, +1, and the square,
//!   +2). With none there the piles are made first (2433:0000,
//!   `LootHooks::piles`). The list shows each pile's gold, item or name;
//!   the one picked goes to the player (gold, or an item: onto a stack of
//!   the same type, or into a free slot), and its record is taken out of
//!   the table; the last pile takes the flat off the map.
//!
//! The windows, lists, texts and sounds are `LootHooks`. Bugs kept:
//! after a corpse's gold is shown, the flag that says so (4BF4) is
//! cleared in the corpse's segment, not DGROUP's, so an empty corpse that
//! had gold says nothing; the pile picked is found again by counting the
//! piles at the square and level without the kind of place, so a pile of
//! another kind there shifts the pick; the window maps bank 6 without
//! saving what was mapped, then puts back the bank saved at 4D77, which
//! the key and corpse paths never set.
//!
//! With EMS off the bank mapping does nothing and returns AX, so the bank
//! saved at 4D77 is AX as earlier code left it; the port takes the click's
//! AX there. The message box is taken as leaving SI as it was (it is the
//! front end's), which matters after a corpse's gold is shown.

use crate::bsa::Files;
use crate::format::Arg;
use crate::inventory::{SLOTS, SLOT_COUNT, SLOT_SIZE, category};
use crate::item;
use crate::machine::Machine;
use crate::mem::DS;

/// What the loot window leaves to the front end.
pub trait LootHooks {
    /// 1194:59CC: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:5734: a list of `cx` strings at DS:`si`. The pick (FFFFh:
    /// none).
    fn list(&mut self, _m: &mut Machine, _si: u16, _cx: u16) -> u16 {
        0xFFFF
    }
    /// 1194:A374 (through A36C): text number TEXT (137A) shown (`mode`
    /// 0) or written to a buffer (2).
    fn text(&mut self, _m: &mut Machine, _mode: u16) {}
    /// 23BA:012B: the sound named at DS:`si`.
    fn sound(&mut self, m: &mut Machine, si: u16) {
        m.sound_file(si);
    }
    /// 1194:D363: whether the staff's text was shown already (the carry).
    fn piece_told(&mut self, _m: &mut Machine) -> bool {
        false
    }
    /// 1194:D986: the eighth piece: the quest's end begins.
    fn last_piece(&mut self, _m: &mut Machine) {}
    /// 2433:0000: the piles of treasure made (`Machine::piles`).
    fn piles(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        m.piles()
    }
}

/// `LootHooks` that do nothing.
pub struct NoLootHooks;

impl LootHooks for NoLootHooks {}

/// DGROUP variables.
pub mod var {
    /// The click's AL, AH, record (offset, then segment) and square.
    pub const AL: u16 = 0x12BC;
    pub const AH: u16 = 0x12BD;
    pub const REC: u16 = 0x12BE;
    pub const SQUARE: u16 = 0x12C0;
    pub const REC_SEG: u16 = 0x12C2;
    /// The corpse's gold was shown; how many of its items are left in
    /// the list.
    pub const GOLD_SHOWN: u16 = 0x4BF4;
    pub const ITEMS_LEFT: u16 = 0x4BF5;
    /// The pile being looked at (1Ch): +0 in use, +1 the kind of place,
    /// +2 the square, +4 the level, +5 gold (FFFFh none), +7 a name from
    /// the list at 2052 (FFh none), +9 an item (13h).
    pub const PILE: u16 = 0x4BF6;
    /// The four kinds of treasure rolled (2433:029F).
    pub const ROLLS: u16 = 0x4C8D;
    /// Making the level's piles (rather than one at the square).
    pub const MAKING: u16 = 0x4C91;
    /// Where the next pile's name goes in the list.
    pub const LIST_AT: u16 = 0x4C8B;
    /// The bank put back at the end, and the piles left in the list.
    pub const BANK: u16 = 0x4D77;
    pub const PILES_LEFT: u16 = 0x4D79;
    /// The pick, and where the next item's name goes.
    pub const PICK: u16 = 0x13C8;
    pub const NAMES_AT: u16 = 0x13ED;
    /// An item copied for its name.
    pub const ITEM: u16 = 0x13DA;
    /// The keys (12 bytes, C5h free), the staff's flat, the pieces held.
    pub const KEYS: u16 = 0x0652;
    pub const STAFF: u16 = 0xC941;
    pub const PIECES: u16 = 0x0F77;
    /// The player's gold (dword).
    pub const GOLD: u16 = 0x05C2;
    /// The piles in use, and the size of a table entry for 1194:5ED1.
    pub const PILE_COUNT: u16 = 0x0FF2;
    pub const ENTRY: u16 = 0x100C;
    /// How heavily the player is loaded (0 not at all).
    pub const LOAD: u16 = 0x05BD;
}
use var::*;

/// The piles' table in bank 6, and its length.
const BANK_PILES: u16 = 6;
const TABLE: u16 = 0xA000;
const TABLE_LEN: u16 = 200;
const PILE_SIZE: u16 = 0x1C;

const FRAME_SEG: u16 = 0x9030;
const LEVEL: u16 = 0xBB0A;
const LEVEL_FLAGS: u16 = 0xC39A;
const MAP_FLAGS: u16 = 0xC398;
const MAP_SEG: u16 = 0xD806;
const FLOOR_SEG: u16 = 0xCCD0;
const PLAYER: u16 = 0x01A4;
const PLAYER_ITEMS: u16 = 0x0212;
/// The player's strength (current).
const STRENGTH: u16 = 0x01CD;
/// The window colour the message boxes take (1194:42EE).
const COLOUR: u16 = 0x9288;

impl Machine {
    /// 0000:5C86 with BX `page`: bank `page` mapped (`ems_map`); returns
    /// the bank mapped before (AX `ax` unchanged with EMS off or no such
    /// bank).
    pub(crate) fn map_bank(&mut self, page: u16, ax: u16) -> u16 {
        if self.mem.r8(DS, 0x8FA6) == 0 || page >= self.mem.r16(DS, 0x9032) {
            return ax;
        }
        let old = self.mem.r16(DS, 0x8FA8);
        self.ems_map(page);
        old >> 2
    }

    /// 0000:9CA0: the string at seg:si copied to es:di with its NUL; DI
    /// after the NUL.
    fn copy_str(&mut self, seg: u16, mut si: u16, es: u16, mut di: u16) -> u16 {
        loop {
            let b = self.mem.r8(seg, si);
            self.mem.w8(es, di, b);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if b == 0 {
                return di;
            }
        }
    }

    /// 1194:42EE (through 42EA): the fonts set (the front end's) and the
    /// window colour 60h.
    fn window_colour(&mut self) {
        self.mem.w8(DS, COLOUR, 0x60);
    }

    /// 2433:0738: a message box (the fonts, 1194:42EA) with the text at
    /// `si`.
    fn loot_message(&mut self, hooks: &mut dyn LootHooks, si: u16) {
        self.window_colour();
        hooks.message_box(self, si);
    }

    /// 1194:7388 (through 7384): an item's name, the item at es:`it`
    /// (DGROUP): a special item (type bit 80h) by its number in the list
    /// at 35F0, any other through `item_name`. None where that is.
    pub fn any_item_name(&mut self, it: u16) -> Option<u16> {
        let ty = self.mem.r8(DS, it);
        if ty & 0x80 != 0 {
            return Some(self.skip_strings(DS, 0x35F0, (ty & 0x7F) as u16));
        }
        self.item_name(DS, it)
    }

    /// 1194:5B07: `ax` * 100 / 256, rounded half away from zero (signed).
    fn percent_of_256(ax: u16) -> u16 {
        let p = (ax as i16 as i32) * 100;
        let (q, r) = (p / 256, p % 256);
        let q = q as u16;
        if r.unsigned_abs() >= 0x80 {
            return if (ax as i16) < 0 { q.wrapping_sub(1) } else { q.wrapping_add(1) };
        }
        q
    }

    /// 1194:5B34: `ax` * 256 / 100, rounded half away from zero (signed).
    /// None where the quotient overflows (the original's divide fault).
    fn of_100_to_256(ax: u16) -> Option<u16> {
        let p = (ax as i16 as i32) * 256;
        let (q, r) = (p / 100, p % 100);
        let q = i16::try_from(q).ok().or_else(|| crate::giveup::original("1194:5B34: the divide by 100 overflows"))? as u16;
        if r.unsigned_abs() >= 0x32 {
            return Some(if (ax as i16) < 0 { q.wrapping_sub(1) } else { q.wrapping_add(1) });
        }
        Some(q)
    }

    /// 1194:4E1B: what the player carries: the weights (the item's +1
    /// word with its bytes swapped, 0 under 100h) of the occupied slots up
    /// to the first empty one, accessories and stacks weighing nothing.
    pub fn carried_weight(&self) -> u16 {
        let mut ax = 0u16;
        let mut si = PLAYER_ITEMS;
        for _ in 0..SLOT_COUNT {
            if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::OCCUPIED == 0 {
                break;
            }
            let cat = self.item_category(DS, si);
            if cat != category::STACK && cat != category::ACCESSORY {
                let w = self.mem.r16(DS, si.wrapping_add(1));
                ax = ax.wrapping_add(if w < 0x100 { 0 } else { w.rotate_left(8) });
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        ax
    }

    /// 1194:4D99 (through 4D95): whether the player can take an item of
    /// weight word `ax` (the item's +1, its high byte the weight; 80h
    /// and 40h weigh nothing). The strength (1CD) makes the limit, s =
    /// strength * 100 / 256: up to 1.5 s the player isn't loaded (5BD 0);
    /// up to 2 s the load is the excess over 1.5 s against 0.75 s, in
    /// 256ths turned to percent (5BD); past that "too heavy" is shown and
    /// the item stays (the carry). None where the original divides
    /// wrongly.
    pub fn carry_check(&mut self, hooks: &mut dyn LootHooks, ax: u16) -> Option<bool> {
        let ax = if ax == 0x80 || ax == 0x40 { 0 } else { ax };
        let cx = ax.rotate_left(8);
        let total = self.carried_weight();
        let s = Self::percent_of_256(self.mem.r8(DS, STRENGTH) as u16);
        let bx = s.wrapping_add(s >> 1);
        let ax = total.wrapping_add(cx);
        if ax <= bx {
            self.mem.w8(DS, LOAD, 0);
            return Some(false);
        }
        let dx = bx.wrapping_add(s >> 1);
        if ax > dx {
            self.window_colour();
            hooks.message_box(self, 0x4B33);
            return Some(true);
        }
        let q = (ax.wrapping_sub(bx) << 8).checked_div(bx >> 1).or_else(|| crate::giveup::original("1194:4D99: the weight limit's divide by zero"))?;
        let load = Self::of_100_to_256(q)?;
        self.mem.w8(DS, LOAD, load as u8);
        Some(false)
    }

    /// 2433:0201: the kind of place the piles belong to: 5 in the wild,
    /// 3 outdoors or outside the buildings (5E6 not 4), else by the
    /// building (D48): 1 gives 1, 3 gives 2, 8 or 0Bh give 4, others 0.
    pub fn pile_place(&self) -> u8 {
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            return 5;
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 || self.mem.r8(DS, 0x05E6) != 4 {
            return 3;
        }
        match self.mem.r8(DS, 0x0D48) {
            1 => 1,
            3 => 2,
            8 | 0xB => 4,
            _ => 0,
        }
    }

    /// 2433:027D: a pile at seg:`si` with neither gold nor a name, whose
    /// item's price (dword at its +0Ah) is 0, gets 0Fh gold (after an
    /// INT 3, which DOS ignores).
    pub(crate) fn pile_fix(&mut self, seg: u16, si: u16) {
        if self.mem.r16(seg, si.wrapping_add(5)) == 0xFFFF
            && self.mem.r8(seg, si.wrapping_add(7)) == 0xFF
            && self.mem.r32(seg, si.wrapping_add(0x13)) == 0
        {
            self.mem.w16(seg, si.wrapping_add(5), 0x0F);
        }
    }

    /// 2433:0152: the name of the pile at `si` (DGROUP) after `pile_fix`:
    /// its item's, the name from the list at 2052, or "%d gold" (929E).
    /// None where an item's name is.
    fn pile_name(&mut self, si: u16) -> Option<u16> {
        self.pile_fix(DS, si);
        let gold = self.mem.r16(DS, si.wrapping_add(5));
        if gold == 0xFFFF {
            let name = self.mem.r8(DS, si.wrapping_add(7));
            if name == 0xFF {
                let r = self.any_item_name(si.wrapping_add(9));
                return r;
            }
            return Some(self.skip_strings(DS, 0x2052, name as u16));
        }
        let fmt = if gold == 1 { 0x4D36 } else { 0x4D44 };
        self.sprintf((DS, 0x929E), (DS, fmt), &[Arg::Int(gold)]);
        Some(0x929E)
    }

    /// 2433:07EA: whether there is a pile at the square (SQUARE) on this
    /// level (none at all: no; bank 6 is mapped and put back while
    /// looking).
    fn pile_here(&mut self, ax: u16) -> bool {
        if self.mem.r8(DS, PILE_COUNT) == 0 {
            return false;
        }
        let saved = self.map_bank(BANK_PILES, ax);
        self.mem.w16(DS, BANK, saved);
        let frame = self.mem.r16(DS, FRAME_SEG);
        let (sq, level) = (self.mem.r16(DS, SQUARE), self.mem.r8(DS, LEVEL));
        let found = (0..TABLE_LEN).any(|i| {
            let si = TABLE + i * PILE_SIZE;
            self.mem.r16(frame, si.wrapping_add(2)) == sq && self.mem.r8(frame, si.wrapping_add(4)) == level
        });
        let bank = self.mem.r16(DS, BANK);
        self.map_bank(bank, saved);
        found
    }

    /// 2433:0883: the pick (PICK) found in the table: counting the piles
    /// at the square and level (the kind of place not checked, as the
    /// list does), the pick-th, whose index replaces the pick and which is
    /// copied to PILE. Past the piles in use the original runs an INT 3
    /// (DOS ignores it) and goes on through the frame, the count
    /// wrapping.
    fn pile_find(&mut self, ax: u16) -> Option<()> {
        let saved = self.map_bank(BANK_PILES, ax);
        self.mem.w16(DS, BANK, saved);
        let frame = self.mem.r16(DS, FRAME_SEG);
        let sq = self.mem.r16(DS, SQUARE);
        let level = self.mem.r8(DS, LEVEL);
        let pick = self.mem.r8(DS, PICK);
        let mut cx = self.mem.r8(DS, PILE_COUNT) as u16;
        let (mut si, mut dx, mut bl) = (TABLE, 0u16, 0u8);
        // The loop's states: at 08A5 (the square compared) or 08B1 (the
        // level), as `loop` falls through to 08B1 when CX runs out.
        let mut at_level = false;
        let mut steps = 0u32;
        loop {
            // Past CX's wrap the scan goes on through the frame until
            // something matches; past four wraps (two states an entry)
            // the port takes it as hanging.
            steps += 1;
            if steps > 0x80000 {
                return crate::giveup::original("2433:0883: no pile found: the original searches for ever");
            }
            if !at_level {
                if self.mem.r16(frame, si.wrapping_add(2)) == sq {
                    at_level = true;
                    continue;
                }
            } else {
                at_level = false;
                if self.mem.r8(frame, si.wrapping_add(4)) == level {
                    if bl == pick {
                        break;
                    }
                    bl = bl.wrapping_add(1);
                }
            }
            si = si.wrapping_add(PILE_SIZE);
            dx = dx.wrapping_add(1);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                // 08B0: INT 3, then 08B1.
                at_level = true;
            }
        }
        self.mem.w8(DS, PICK, dx as u8);
        for i in 0..PILE_SIZE {
            let b = self.mem.r8(frame, si.wrapping_add(i));
            self.mem.w8(DS, PILE + i, b);
        }
        let bank = self.mem.r16(DS, BANK);
        self.map_bank(bank, saved);
        Some(())
    }

    /// 2433:0972: the item a pile gives: the one in PILE (+9), or in the
    /// wild the one at 13DA.
    fn pile_item(&self) -> u16 {
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 { ITEM } else { PILE + 9 }
    }

    /// 2433:08E2: the pile in PILE taken: its gold (not in the wild), or
    /// its item: onto the player's stack of the same type whose flags
    /// lack bit 1 (the count capped at 7Fh when it overflows), else into
    /// a free slot if not too heavy (`carry_check`, stacks always), the
    /// player's items then sorted. Whether it stayed (the carry): no free
    /// slot ("full", 19DE) or too heavy. None where `carry_check` is.
    fn pile_take(&mut self, hooks: &mut dyn LootHooks) -> Option<bool> {
        let gold = self.mem.r16(DS, PILE + 5);
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 == 0 && gold != 0xFFFF {
            let g = self.mem.r32(DS, GOLD).wrapping_add(gold as u32);
            self.mem.w32(DS, GOLD, g);
            return Some(false);
        }
        let si = self.pile_item();
        if self.item_category(DS, si) == category::STACK {
            let ty = self.mem.r8(DS, si) as u16;
            let (mut found, mut at, mut cx) = self.find_item(DS, PLAYER_ITEMS, SLOT_COUNT, ty, category::STACK);
            while found && self.mem.r8(DS, at.wrapping_add(item::FLAGS)) & 2 != 0 {
                (found, at, cx) = self.find_item(DS, at.wrapping_add(SLOT_SIZE), cx, ty, category::STACK);
            }
            if found {
                let n = self.mem.r16(DS, at.wrapping_add(1)).wrapping_add(1);
                self.mem.w16(DS, at.wrapping_add(1), if (n as i16) < 0 { 0x7F } else { n });
                return Some(false);
            }
        }
        let Ok((slot, _)) = self.free_slot(DS, PLAYER) else {
            self.loot_message(hooks, 0x19DE);
            return Some(true);
        };
        let si = self.pile_item();
        if self.item_category(DS, si) != category::STACK {
            let w = self.mem.r16(DS, si.wrapping_add(1));
            if self.carry_check(hooks, w)? {
                return Some(true);
            }
        }
        let si = self.pile_item();
        self.memmove(DS, si, DS, slot, SLOT_SIZE);
        self.sort_inventory();
        Some(false)
    }

    /// 2433:07B9: the square's flat taken off the map: a flat in the map
    /// plane (its word's top nibble 8) cleared there, else the floor
    /// plane's byte.
    fn pile_flat_gone(&mut self) {
        let bx = self.mem.r16(DS, SQUARE);
        let map = self.mem.r16(DS, MAP_SEG);
        if self.mem.r16(map, bx) >> 12 == 8 {
            self.mem.w16(map, bx, 0);
        } else {
            let floor = self.mem.r16(DS, FLOOR_SEG);
            self.mem.w8(floor, bx, 0);
        }
    }

    /// 1194:5ED1 (through 5ECD): the entry PICK of the table at
    /// frame:`si` (the pick's own) taken out: the `cx` - 1 - PICK entries
    /// after it (ENTRY bytes each) move down one and the last is cleared.
    pub(crate) fn table_remove(&mut self, si: u16, cx: u16) {
        let frame = self.mem.r16(DS, FRAME_SEG);
        let size = self.mem.r8(DS, ENTRY) as u16;
        let pick = self.mem.r8(DS, PICK) as u16;
        let mut k = cx.wrapping_sub(1).wrapping_sub(pick);
        let mut di = si;
        if k != 0 {
            let mut si = si.wrapping_add(size);
            while k != 0 {
                (si, di) = self.memmove(frame, si, frame, di, size);
                k = k.wrapping_sub(1);
            }
        }
        for i in 0..size {
            self.mem.w8(frame, di.wrapping_add(i), 0);
        }
    }

    /// 2433:0981: the corpse's item at REC_SEG:`si` (slot PICK) taken:
    /// into the player's first free slot ("full" otherwise) if not too
    /// heavy; the player's items sorted, and the item taken out of the
    /// corpse (`remove_item`). None where `carry_check` is.
    fn corpse_take(&mut self, hooks: &mut dyn LootHooks, si: u16) -> Option<()> {
        let Ok((slot, _)) = self.free_slot(DS, PLAYER) else {
            self.loot_message(hooks, 0x19DE);
            return Some(());
        };
        let seg = self.mem.r16(DS, REC_SEG);
        let w = self.mem.r16(seg, si.wrapping_add(1));
        if self.carry_check(hooks, w)? {
            return Some(());
        }
        self.memmove(seg, si, DS, slot, SLOT_SIZE);
        self.sort_inventory();
        let pick = self.mem.r8(DS, PICK) as u16;
        self.remove_item(seg, si, pick);
        Some(())
    }

    /// 2433:0BA2: an empty corpse: "%s has nothing" with its class or
    /// creature's name (2686:0645).
    fn corpse_empty(&mut self, hooks: &mut dyn LootHooks) {
        let (seg, rec) = (self.mem.r16(DS, REC_SEG), self.mem.r16(DS, REC));
        let (ns, no) = self.class_name(seg, rec);
        self.sprintf((DS, 0x1044), (DS, 0x4C70), &[Arg::Str(ns, no)]);
        self.loot_message(hooks, 0x1044);
    }

    /// 2433:0AFF: the corpse's list: the names of the occupied slots at
    /// REC_SEG:`si` + 6Eh up to the first empty one (each copied to ITEM
    /// for its name) at 1044. Empty: "has nothing" unless its gold was just shown (the
    /// flag read in DGROUP), and done. Otherwise the list; leaving it is
    /// done, a pick (PICK; one fewer left, ITEMS_LEFT) is taken
    /// (`corpse_take`). Whether it is done (the carry). None where an
    /// item's name is.
    fn corpse_list(&mut self, hooks: &mut dyn LootHooks, si: u16) -> Option<bool> {
        let seg = self.mem.r16(DS, REC_SEG);
        self.mem.w16(DS, NAMES_AT, 0x1044);
        let mut si = si.wrapping_add(SLOTS);
        let mut bx = 0u16;
        for _ in 0..SLOT_COUNT {
            if self.mem.r8(seg, si.wrapping_add(item::FLAGS)) & item::OCCUPIED == 0 {
                break;
            }
            self.memmove(seg, si, DS, ITEM, SLOT_SIZE);
            let name = self.any_item_name(ITEM)?;
            let di = self.mem.r16(DS, NAMES_AT);
            let di = self.copy_str(DS, name, DS, di);
            self.mem.w16(DS, NAMES_AT, di);
            si = si.wrapping_add(SLOT_SIZE);
            bx = bx.wrapping_add(1);
        }
        if bx == 0 {
            if self.mem.r8(DS, GOLD_SHOWN) == 0 {
                self.corpse_empty(hooks);
            }
            return Some(true);
        }
        self.mem.w8(DS, ITEMS_LEFT, bx as u8);
        let pick = hooks.list(self, 0x1044, bx);
        if (pick as i16) < 0 {
            return Some(true);
        }
        self.mem.w8(DS, PICK, pick as u8);
        let left = self.mem.r8(DS, ITEMS_LEFT).wrapping_sub(1);
        self.mem.w8(DS, ITEMS_LEFT, left);
        let rec = self.mem.r16(DS, REC);
        let si = rec.wrapping_add(SLOTS).wrapping_add((pick as u8 as u16).wrapping_mul(SLOT_SIZE));
        self.corpse_take(hooks, si)?;
        Some(false)
    }

    /// 2433:0748: a corpse: its gold (+407h) to the player's, shown
    /// ("%u gold pieces", 9686; GOLD_SHOWN set), then its list until it
    /// is done or nothing is left. Before each list the flag is cleared at
    /// REC_SEG:4BF4 (the original's DS there). The first list after the
    /// gold is shown reads the slots at 9686h + 6Eh, SI still the
    /// message's (the message box taken as leaving SI as it is), though
    /// the item taken is the record's; later lists read the record's.
    fn corpse_loot(&mut self, hooks: &mut dyn LootHooks) -> Option<()> {
        let (seg, rec) = (self.mem.r16(DS, REC_SEG), self.mem.r16(DS, REC));
        let gold = self.mem.r16(seg, rec.wrapping_add(0x407));
        if gold != 0 {
            self.mem.w16(seg, rec.wrapping_add(0x407), 0);
            let g = self.mem.r32(DS, GOLD).wrapping_add(gold as u32);
            self.mem.w32(DS, GOLD, g);
            self.sprintf((DS, 0x9686), (DS, 0x4CFA), &[Arg::Int(gold)]);
            self.window_colour();
            hooks.message_box(self, 0x9686);
            self.mem.w8(DS, GOLD_SHOWN, 1);
        }
        let mut si = if gold != 0 { 0x9686 } else { rec };
        loop {
            let seg = self.mem.r16(DS, REC_SEG);
            self.mem.w8(seg, GOLD_SHOWN, 0);
            if self.corpse_list(hooks, si)? || self.mem.r8(DS, ITEMS_LEFT) == 0 {
                return Some(());
            }
            si = self.mem.r16(DS, REC);
        }
    }

    /// 2433:0AA1 with AH 1: "You found the %s key", the key AL (1..)
    /// named from the list at 342B by (AL - 1) mod 12, at 7979: shown in
    /// a box, or with AL 0 (12BC) as the message line for 1Eh frames.
    /// None where the original divides wrongly (AL 0).
    fn key_found(&mut self, hooks: &mut dyn LootHooks, al: u8, ah: u8) -> Option<()> {
        self.mem.w8(DS, 0x12CE, ah);
        let ax = (al as u16).wrapping_sub(1);
        if ax / 12 > 0xFF {
            return crate::giveup::original("2433:0AA1: the key's divide overflows");
        }
        let k = ax % 12;
        let name = self.skip_strings(DS, 0x342B, k);
        let fmt = if ah != 0 { 0x4CA5 } else { 0x4CBC };
        self.sprintf((DS, 0x7979), (DS, fmt), &[Arg::Str(DS, name)]);
        if self.mem.r8(DS, AL) == 0 {
            self.mem.w16(DS, 0x061A, 0x7979);
            self.mem.w16(DS, 0x061C, DS);
            self.mem.w16(DS, 0x061E, 0x1E);
            return Some(());
        }
        self.window_colour();
        hooks.message_box(self, 0x7979);
        Some(())
    }

    /// 2433:0BF4: the staff's text: with 0F7D set, "piece %d" (4C92) and,
    /// unless already told (1194:D363), text 55Bh + pieces - 1; else
    /// (first time) text 553h + pieces, and 0F7D, 0F7F set.
    fn piece_text(&mut self, hooks: &mut dyn LootHooks) {
        self.mem.w8(DS, 0x12CC, 0);
        let pieces = self.mem.r8(DS, PIECES) as u16;
        let cx;
        if self.mem.r8(DS, 0x0F7D) != 0 {
            self.sprintf((DS, 0x1044), (DS, 0x4C92), &[Arg::Int(pieces.wrapping_add(1))]);
            self.mem.w8(DS, 0x0F7E, 1);
            if hooks.piece_told(self) {
                return;
            }
            self.mem.w8(DS, 0x12CC, 1);
            cx = 0x55B;
        } else {
            self.mem.w8(DS, 0x0F7B, 0);
            cx = 0x553;
            self.mem.w8(DS, 0x0F7D, 1);
            self.mem.w8(DS, 0x0F7F, 1);
        }
        let mut ax = self.mem.r8(DS, PIECES) as u16;
        if self.mem.r8(DS, 0x12CC) != 0 {
            ax = ax.wrapping_sub(1);
        }
        self.mem.w16(DS, 0x137A, ax.wrapping_add(cx));
        self.mem.w8(DS, 0x137E, 0);
        self.mem.w8(DS, 0x101C, 0);
        hooks.text(self, 0);
    }

    /// 2433:0583 (through 1194:7B16). The loot window for the click: AH
    /// the kind, AL its value, the record at `seg`:`si`, the square `bx`
    /// (see the module). None where the original faults or hangs (a key
    /// with no free key byte anywhere in DGROUP; key 0), or an item's
    /// name does.
    #[allow(clippy::too_many_arguments)]
    pub fn loot_window(&mut self, files: &mut Files, hooks: &mut dyn LootHooks, ax: u16, si: u16, seg: u16, bx: u16) -> Option<()> {
        let (al, ah) = (ax as u8, (ax >> 8) as u8);
        // 2433:0328.
        self.mem.w8(DS, AL, al);
        self.mem.w8(DS, AH, ah);
        self.mem.w16(DS, REC, si);
        self.mem.w16(DS, REC_SEG, seg);
        self.mem.w16(DS, SQUARE, bx);
        self.mem.w8(DS, ROLLS, 0);
        for i in 0..PILE_SIZE {
            self.mem.w8(DS, PILE + i, 0);
        }
        self.mem.w8(DS, GOLD_SHOWN, 0);
        self.map_bank(BANK_PILES, ax);
        if ah == 3 {
            let mut at = KEYS;
            let mut n = 0u32;
            while self.mem.r8(DS, at) != 0xC5 {
                at = at.wrapping_add(1);
                n += 1;
                if n > 0xFFFF {
                    return crate::giveup::original("2433:0328: the list has no end: the original reads on for ever");
                }
            }
            self.mem.w8(DS, at, al);
            self.key_found(hooks, al, 1)?;
        } else if ah == 0x0F && al == self.mem.r8(DS, STAFF) {
            for i in 0..12 {
                self.mem.w8(DS, KEYS + i, 0xC5);
            }
            hooks.sound(self, 0x4C63);
            if self.mem.r8(DS, PIECES) == 8 {
                self.mem.w8(DS, PIECES, 9);
                hooks.last_piece(self);
                let f = self.mem.r16(DS, MAP_FLAGS) | 0x80;
                self.mem.w16(DS, MAP_FLAGS, f);
                self.mem.w8(DS, 0x0F76, 0);
                self.mem.w8(DS, 0x05E6, 0);
                self.mem.w16(DS, 0x0FCE, 0);
            } else {
                self.piece_text(hooks);
                let bx = self.mem.r16(DS, SQUARE);
                let floor = self.mem.r16(DS, FLOOR_SEG);
                if self.mem.r8(floor, bx) != 0 {
                    self.mem.w8(floor, bx, 0);
                } else {
                    let map = self.mem.r16(DS, MAP_SEG);
                    self.mem.w16(map, bx, 0);
                }
            }
        } else if ah == 0 || ah == 1 {
            self.corpse_loot(hooks)?;
        } else {
            if !self.pile_here(ax) {
                self.mem.w8(DS, MAKING, 0);
                hooks.piles(self, files)?;
            }
            self.pile_list(hooks, ax)?;
        }
        let bank = self.mem.r16(DS, BANK);
        self.map_bank(bank, ax);
        Some(())
    }

    /// 2433:0637..072C: the piles at the square listed (their names at
    /// 9302) until the player leaves the list or none is left: the one
    /// picked found (`pile_find`) and taken (`pile_take`, which may leave
    /// it); the last one takes the flat off the map; the pile's entry is
    /// taken out of the table. With none there, "nothing here" (4CD7).
    /// None where a name is.
    fn pile_list(&mut self, hooks: &mut dyn LootHooks, ax: u16) -> Option<()> {
        loop {
            let saved = self.map_bank(BANK_PILES, ax);
            self.mem.w16(DS, BANK, saved);
            self.mem.w16(DS, LIST_AT, 0x9302);
            let frame = self.mem.r16(DS, FRAME_SEG);
            let mut bx = 0u16;
            for i in 0..TABLE_LEN {
                let si = TABLE + i * PILE_SIZE;
                for k in 0..PILE_SIZE {
                    let b = self.mem.r8(frame, si.wrapping_add(k));
                    self.mem.w8(DS, PILE + k, b);
                }
                if self.mem.r8(DS, LEVEL) == self.mem.r8(DS, PILE + 4)
                    && self.pile_place() == self.mem.r8(DS, PILE + 1)
                    && self.mem.r16(DS, SQUARE) == self.mem.r16(DS, PILE + 2)
                {
                    let name = self.pile_name(PILE)?;
                    let di = self.mem.r16(DS, LIST_AT);
                    let di = self.copy_str(DS, name, DS, di);
                    self.mem.w16(DS, LIST_AT, di);
                    bx = bx.wrapping_add(1);
                }
            }
            if bx == 0 {
                self.loot_message(hooks, 0x4CD7);
                return Some(());
            }
            self.mem.w8(DS, PILES_LEFT, bx as u8);
            let pick = hooks.list(self, 0x9302, bx);
            if (pick as i16) < 0 {
                return Some(());
            }
            self.mem.w8(DS, PICK, pick as u8);
            self.pile_find(ax)?;
            self.pile_fix(DS, PILE);
            if self.pile_take(hooks)? {
                return Some(());
            }
            if bx == 1 {
                self.pile_flat_gone();
            }
            let si = TABLE.wrapping_add((self.mem.r8(DS, PICK) as u16).wrapping_mul(PILE_SIZE));
            self.mem.w8(DS, ENTRY, PILE_SIZE as u8);
            let cx = self.mem.r8(DS, PILE_COUNT) as u16;
            self.table_remove(si, cx);
            let n = self.mem.r8(DS, PILE_COUNT).wrapping_sub(1);
            self.mem.w8(DS, PILE_COUNT, n);
            let left = self.mem.r8(DS, PILES_LEFT).wrapping_sub(1);
            self.mem.w8(DS, PILES_LEFT, left);
            if left == 0 {
                return Some(());
            }
        }
    }
}
