//! Using items in play: the "use" button's list of items with an effect
//! and what using one does (24FA:07CD, 29CF:07C3), the two once-a-day
//! items (2294:1051, 0FFA), and the inventory screen's rules for putting
//! on, taking off (1194:1AFB, 7605) and dropping (1194:18F6) an item,
//! a dropped item landing in the level's item list (2433:09E1).
//!
//! Using an item (29CF:07C3) casts its spell (+11h) on the player:
//! - a stack (a potion) is drunk: spell FFh heals 25 points (to the
//!   maximum), another is cast at level byte 0Fh; one is taken from the
//!   stack (or the slot emptied) and the inventory sorted;
//! - armour and weapons pay the spell's cost (+32h of its record) out of
//!   their condition: a special item (type bit 80h) half the cost, cast
//!   at level byte 13h (and F84 set to 8000h), and it breaks once its
//!   condition goes negative, after the cast; another a fifteenth, cast
//!   at level byte 0Eh, breaking instead of casting when that is more
//!   than it has;
//! - accessories by the table at 7F81 (per type): of armour kind 0 a
//!   charge (+3) each use, breaking with none left; of other kinds as
//!   armour, the table's value the divisor; special ones as armour.
//!
//! The level byte (player +6, 1AA) is set around the cast to the cast's
//! level less one, as the caster's level is read from it.
//!
//! The inventory screen's drawing, its lists and prompts are the front
//! end's (`UseHooks`); the routines here start from the slot the screen
//! has selected (index at 1296 for dropping) or that a click picked.
//!
//! The segment registers the original leaves to its callers (ES in
//! 1194:5074, 2433:0201 and the item naming, FS in 1194:7527) are DGROUP
//! there; the port takes them so.

use crate::bsa::Files;
use crate::character::PLAYER;
use crate::format::Arg;
use crate::inventory::{category, item, LAST_COUNT, PLAYER_SLOTS, SLOT_SIZE};
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::text::TextHooks;

/// DGROUP variables.
pub mod var {
    /// The list of usable items: its count, the next name's place, and
    /// the names (NUL-terminated, one after another).
    pub const USABLE_COUNT: u16 = 0x127C;
    pub const USABLE_NEXT: u16 = 0x127E;
    pub const USABLE_NAMES: u16 = 0x9686;
    /// The level byte of the player's record (player +6).
    pub const LEVEL: u16 = 0x01AA;
    /// Health and its maximum (player +66h, +68h).
    pub const HEALTH: u16 = 0x020A;
    pub const MAX_HEALTH: u16 = 0x020C;
    /// Per accessory type: the level byte its cast is made at, plus one,
    /// and (for kinds other than 0) the divisor of its cost.
    pub const ACCESSORY_LEVEL: u16 = 0x7F81;
    /// A waiting Open spell's power (7D0h: sure).
    pub const OPEN_SPELL: u16 = 0x0DC4;
    /// The game's minutes (dword) and the player's gold (dword).
    pub const MINUTES: u16 = 0x05CC;
    pub const GOLD: u16 = 0x05C2;
    /// The inventory screen: the selected slot (FFh none), the first
    /// line shown, whether the player's own items are shown (else the
    /// other list's count at 50A), the other list's flag (1299), a
    /// shop's or a looting's flag (1270) and the line count (126C).
    pub const SELECTED: u16 = 0x1296;
    pub const FIRST_LINE: u16 = 0x1297;
    pub const OWN_ITEMS: u16 = 0x129A;
    pub const OTHER_LIST: u16 = 0x1299;
    pub const NOT_OWN: u16 = 0x1270;
    pub const LINES: u16 = 0x126C;
    pub const OTHER_COUNT: u16 = 0x050A;
    /// The item being dropped: its slot number and a copy.
    pub const DROPPED_INDEX: u16 = 0x13C8;
    pub const DROPPED: u16 = 0x13DA;
    /// The record a dropped item is placed with (1Ch bytes), the level's
    /// item count (byte) and its list (in the EMS frame, from A000h).
    pub const FLOOR_ITEM: u16 = 0x4BF6;
    pub const LEVEL_ITEMS: u16 = 0x0FF2;
    pub const LEVEL_ITEM_LIST: u16 = 0xA000;
    /// The last special item put on (1194:5074).
    pub const SPECIAL_WORN: u16 = 0x1AA4;
    /// Where the player stands, as 1194:5F13 found it before the
    /// character sheet: 5E0 bit 20h no room to drop there, and DROP_LOST
    /// (12D0, a byte many routines borrow) an item dropped there lost.
    pub const PLAYER_FLAGS: u16 = 0x05E0;
    pub const NO_ROOM: u16 = 0x20;
    pub const DROP_LOST: u16 = 0x12D0;
    /// The drop button's texts and questions go to DGROUP 1044, in the
    /// colour at 9288 and the font at B036 (the fonts' records at 1026
    /// and 102C).
    pub const DROP_TEXT: u16 = 0x1044;
}

/// The drop button's texts (segment 3276): no room, the item lost
/// (asked), not the player's to drop, nothing selected, and the question
/// ("Do you wish to drop\rthe %s?\r").
pub mod text {
    pub const NO_ROOM: u16 = 0x5A4F;
    pub const LOST: u16 = 0x5A24;
    pub const CANT_DROP: u16 = 0x5AAE;
    pub const DROP_WHAT: u16 = 0x5ACB;
    pub const DROP_FORMAT: u16 = 0x3C08;
}

/// What the inventory's drop button (1194:17D8) came to; its text, if
/// any, is at DROP_TEXT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropButton {
    /// No room where the player stands: "There is no room to drop this
    /// here."
    NoRoom,
    /// Nothing done: the list isn't the player's own, nothing is carried,
    /// or the question was answered no.
    Nothing,
    /// The list is a shop's or a looting's: "This item can't be dropped."
    CantDrop,
    /// Nothing selected: "Drop what?"
    DropWhat,
    /// The selected item dropped (`drop_item`).
    Dropped,
}
use var::*;

/// The EMS bank the level's item list lives in.
const ITEMS_BANK: u16 = 6;

/// What the front end does for the inventory screen and the use button.
pub trait UseHooks {
    /// 29CF:00FA with SI the player and FS DGROUP: the player casts spell
    /// `spell` (the level byte at 1AA as the caller set it).
    fn cast(&mut self, m: &mut Machine, files: &mut Files, spell: u8) -> Option<()> {
        // On the player (no target is asked): no quest target to give up.
        m.player_cast(files, spell, &mut |_, _| {})
    }
    /// 2AA1:021B: the player's item at DGROUP `si` broke.
    fn broken(&mut self, m: &mut Machine, si: u16) -> Option<()> {
        m.item_broken(si)
    }
    /// 24FA:0B78: the list of usable items shown (`count` names from
    /// USABLE_NAMES). The pick, or FFFFh (any negative) for none.
    fn pick(&mut self, _m: &mut Machine, _count: u16) -> u16 {
        0xFFFF
    }
    /// 1194:59C8: a message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:1BE0: the item at DGROUP `si` (category `cat`) can't be used
    /// by the player's class, or (NOT_OWN set) items aren't put on here.
    fn cant_use(&mut self, _m: &mut Machine, _si: u16, _cat: u16) {}
    /// 1194:1B78: the accessory at DGROUP `si` clashes with one worn (its
    /// base type `base`; the kind looked for is at 12A0).
    fn clash(&mut self, _m: &mut Machine, _si: u16, _base: u16) {}
}

/// `UseHooks` with the defaults: casts and breakages ported, no picks.
pub struct NoUseHooks;

impl UseHooks for NoUseHooks {}

/// What putting an item on or taking it off did (1194:1AFB).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Equip {
    /// Put on or taken off (`equip_toggle`).
    Toggled,
    /// Refused: the class can't use it (`UseHooks::cant_use`).
    CantUse,
    /// Refused: an accessory of its kind is worn (`UseHooks::clash`).
    Clash,
}

impl Machine {
    /// 24FA:07CD. The use button: the player's items with an effect are
    /// listed (`usable_list`) and one picked; type 8Fh is the Open charm
    /// (`open_charm`), 8Dh the gold charm (`gold_charm`), any other is
    /// used (`item_cast`). None where naming an item, a cast or a
    /// breakage fails.
    pub fn use_item(&mut self, files: &mut Files, hooks: &mut dyn UseHooks) -> Option<()> {
        let n = self.usable_list(PLAYER)?;
        let ax = if n == 0 { 0xFFFF } else { hooks.pick(self, n) };
        if (ax as i16) < 0 {
            return Some(());
        }
        let si = self.nth_usable_item(DS, PLAYER, ax);
        match self.mem.r8(DS, si) {
            0x8F => {
                self.open_charm(hooks, si);
                Some(())
            }
            0x8D => self.gold_charm(hooks, si),
            _ => self.item_cast(files, hooks, si),
        }
    }

    /// 24FA:0AFE. The usable items of record `rec` (all 40 slots, those
    /// with an effect, 1194:7A86) named one after another at
    /// USABLE_NAMES (each at most 19 characters, cut short without its
    /// NUL then given one; a second NUL is written after each, which the
    /// next name covers). Returns their count, kept at USABLE_COUNT.
    pub fn usable_list(&mut self, rec: u16) -> Option<u16> {
        self.mem.w16(DS, USABLE_COUNT, 0);
        self.mem.w16(DS, USABLE_NEXT, USABLE_NAMES);
        let mut si = rec.wrapping_add(crate::inventory::SLOTS);
        for _ in 0..0x28 {
            if self.item_has_effect(DS, si) {
                let mut name = self.display_name(si)?;
                let mut di = self.mem.r16(DS, USABLE_NEXT);
                let mut ended = false;
                for _ in 0..0x13 {
                    let b = self.mem.r8(DS, name);
                    name = name.wrapping_add(1);
                    self.mem.w8(DS, di, b);
                    di = di.wrapping_add(1);
                    if b == 0 {
                        ended = true;
                        break;
                    }
                }
                if !ended {
                    self.mem.w8(DS, di, 0);
                    di = di.wrapping_add(1);
                }
                self.mem.w16(DS, USABLE_NEXT, di);
                self.mem.w8(DS, di, 0);
                let n = self.mem.r16(DS, USABLE_COUNT).wrapping_add(1);
                self.mem.w16(DS, USABLE_COUNT, n);
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        Some(self.mem.r16(DS, USABLE_COUNT))
    }

    /// 1194:7388. An item's name as lists show it: a special item's (type
    /// bit 80h) from the list at 35F0 by its type, any other's
    /// `item_name`.
    pub fn display_name(&mut self, it: u16) -> Option<u16> {
        let ty = self.mem.r8(DS, it);
        if ty & 0x80 != 0 {
            return Some(self.skip_strings(DS, 0x35F0, (ty & 0x7F) as u16));
        }
        self.item_name(DS, it)
    }

    /// 29CF:07C3. The item at DGROUP `si` is used (see the module notes).
    /// The waiting spells' state (DC0, DC2, DC4, DC8) is cleared first.
    /// None where an accessory's divisor is 0 (the original's divide
    /// fault), or a cast or breakage fails.
    pub fn item_cast(&mut self, files: &mut Files, hooks: &mut dyn UseHooks, si: u16) -> Option<()> {
        for o in [0x0DC0, 0x0DC2, OPEN_SPELL, 0x0DC8] {
            self.mem.w16(DS, o, 0);
        }
        let mut cx = 0x0Fu16;
        let ty = self.mem.r8(DS, si);
        match self.item_category(DS, si) {
            category::STACK => return self.drink(files, hooks, si),
            category::ACCESSORY if ty & 0x80 == 0 => {
                cx = self.mem.r8(DS, ACCESSORY_LEVEL.wrapping_add(ty as u16)) as u16;
                if self.armor_kind(DS, si) == 0 {
                    let charges = self.mem.r8(DS, si.wrapping_add(item::HANDS));
                    if charges == 0 {
                        return hooks.broken(self, si);
                    }
                    self.mem.w8(DS, si.wrapping_add(item::HANDS), charges - 1);
                    let spell = self.mem.r8(DS, si.wrapping_add(item::ENCHANT_VALUE));
                    return self.item_spell(files, hooks, spell, cx as u8);
                }
            }
            _ => {}
        }
        // 29CF:081F: the spell's cost out of the item's condition.
        let spell = self.mem.r8(DS, si.wrapping_add(item::ENCHANT_VALUE));
        let (gs, rec) = self.spell_record(spell);
        let cost = self.mem.r16(gs, rec.wrapping_add(0x32));
        let health = self.mem.r16(DS, si.wrapping_add(item::HEALTH));
        if ty & 0x80 != 0 {
            let left = health.wrapping_sub(cost >> 1);
            self.mem.w16(DS, si.wrapping_add(item::HEALTH), left);
            self.mem.w16(DS, 0x0F84, 0x8000);
            self.item_spell(files, hooks, spell, 0x14)?;
            if (left as i16) >= 0 {
                return Some(());
            }
        } else {
            if cx == 0 {
                return crate::giveup::original("29CF:081F: a divide by zero");
            }
            let bx = cost / cx;
            self.mem.w16(DS, si.wrapping_add(item::HEALTH), health.wrapping_sub(bx));
            if health >= bx {
                return self.item_spell(files, hooks, spell, cx as u8);
            }
        }
        // 29CF:085E
        self.mem.w16(DS, si.wrapping_add(item::HEALTH), 0);
        hooks.broken(self, si)
    }

    /// 29CF:08BE. Spell `spell` cast by the player with the level byte
    /// at `cl` less one meanwhile.
    fn item_spell(&mut self, files: &mut Files, hooks: &mut dyn UseHooks, spell: u8, cl: u8) -> Option<()> {
        let level = self.mem.r8(DS, LEVEL);
        self.mem.w8(DS, LEVEL, cl.wrapping_sub(1));
        let r = hooks.cast(self, files, spell);
        self.mem.w8(DS, LEVEL, level);
        r
    }

    /// 29CF:0869. A potion at `si` is drunk: spell FFh heals 25 points,
    /// another is cast at level byte 0Fh; then one is taken from the
    /// stack (1194:7889) and the inventory sorted (1194:790D).
    fn drink(&mut self, files: &mut Files, hooks: &mut dyn UseHooks, si: u16) -> Option<()> {
        let spell = self.mem.r8(DS, si.wrapping_add(item::ENCHANT_VALUE));
        if spell == 0xFF {
            let hp = self.mem.r16(DS, HEALTH).wrapping_add(0x19).min(self.mem.r16(DS, MAX_HEALTH));
            self.mem.w16(DS, HEALTH, hp);
        } else {
            let level = self.mem.r8(DS, LEVEL);
            self.mem.w8(DS, LEVEL, 0x0F);
            let r = hooks.cast(self, files, spell);
            self.mem.w8(DS, LEVEL, level);
            r?;
        }
        let index = si.wrapping_sub(PLAYER_SLOTS) / SLOT_SIZE;
        self.remove_item(DS, si, index);
        self.sort_inventory();
        Some(())
    }

    /// 2294:1085. The day of the year (0..359) of the game's minutes.
    fn charm_day(&self) -> u16 {
        let minutes = self.mem.r32(DS, MINUTES);
        ((minutes % 0x7E900) / 0x5A0) as u16
    }

    /// 2294:1067. "Already used today" (41E8), in a message box.
    fn charm_used(&mut self, hooks: &mut dyn UseHooks) {
        let v = self.mem.r16(DS, 0x1036);
        self.mem.w16(DS, 0xB038, v);
        self.mem.w8(DS, 0x9288, 0xD4);
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        hooks.message_box(self, 0x41E8);
    }

    /// 2294:1051. Type 8Fh, once a day (the day kept at +1): a sure Open
    /// spell waits (OPEN_SPELL 7D0h).
    pub fn open_charm(&mut self, hooks: &mut dyn UseHooks, si: u16) {
        let day = self.charm_day();
        if day == self.mem.r16(DS, si.wrapping_add(1)) {
            self.charm_used(hooks);
            return;
        }
        self.mem.w16(DS, si.wrapping_add(1), day);
        self.mem.w16(DS, OPEN_SPELL, 0x7D0);
    }

    /// 2294:0FFA. Type 8Dh, once a day (the day kept at +1): 10..100 gold
    /// out of its condition, "You find %d gold." for 15 frames; it
    /// breaks when its condition goes negative, giving none. None where
    /// the breakage fails.
    pub fn gold_charm(&mut self, hooks: &mut dyn UseHooks, si: u16) -> Option<()> {
        let day = self.charm_day();
        if day == self.mem.r16(DS, si.wrapping_add(1)) {
            self.charm_used(hooks);
            return Some(());
        }
        self.mem.w16(DS, si.wrapping_add(1), day);
        let gold = self.rng.range(0x0A, 0x65)?;
        let left = self.mem.r16(DS, si.wrapping_add(item::HEALTH)).wrapping_sub(gold);
        self.mem.w16(DS, si.wrapping_add(item::HEALTH), left);
        if (left as i16) < 0 {
            self.mem.w16(DS, si.wrapping_add(item::HEALTH), 0);
            return hooks.broken(self, si);
        }
        let g = self.mem.r32(DS, GOLD).wrapping_add(gold as u32);
        self.mem.w32(DS, GOLD, g);
        self.sprintf((DS, 0x7979), (DS, 0x41C7), &[Arg::Int(gold)]);
        self.mem.w16(DS, 0x061E, 0x0F);
        self.mem.w16(DS, 0x061A, 0x7979);
        self.mem.w16(DS, 0x061C, DS);
        Some(())
    }

    /// 1194:1AFB..1B37. A click puts on or takes off the player's item in
    /// slot `index` (the line under the cursor): not while another list
    /// is shown (NOT_OWN), nor what the class can't use (1194:7772), nor
    /// an accessory clashing with one worn (1194:7804); otherwise it is
    /// toggled (`equip_toggle`), the weapon class (1194:731A) and the
    /// armour class (1194:7719, in the redraw 23B1) made again. None
    /// where the class's list has no end. With NOT_OWN set the original
    /// names the category of the item its caller's SI points at; the
    /// port passes the clicked one.
    pub fn equip_click(&mut self, hooks: &mut dyn UseHooks, index: u16) -> Option<Equip> {
        let si = PLAYER_SLOTS.wrapping_add(index.wrapping_mul(SLOT_SIZE));
        if self.mem.r16(DS, NOT_OWN) != 0 {
            let cat = self.item_category(DS, si);
            hooks.cant_use(self, si, cat);
            return Some(Equip::CantUse);
        }
        let class = (self.mem.r8(DS, PLAYER + crate::character::field::CLASS) & 0x1F) as u16;
        if !self.class_can_use(DS, si, class)? {
            let cat = self.item_category(DS, si);
            hooks.cant_use(self, si, cat);
            return Some(Equip::CantUse);
        }
        if self.accessory_clash(si) {
            let (base, _) = self.base_type(self.mem.r8(DS, si));
            hooks.clash(self, si, base);
            return Some(Equip::Clash);
        }
        self.equip_toggle(si);
        self.player_weapon_class();
        self.player_armor_class();
        Some(Equip::Toggled)
    }

    /// 1194:7605. The player's item at DGROUP `si` is put on or taken
    /// off (flag 80h flipped). Special items 80h..88h lose their armour
    /// kind first. Taking off gives back its enchantment (1194:511A);
    /// putting on takes off what it can't be worn with (`unequip_clashes`),
    /// applies its enchantment (`equip_enchantment`) and remakes the
    /// player's attribute modifiers (1194:3621).
    pub fn equip_toggle(&mut self, si: u16) {
        let ty = self.mem.r8(DS, si);
        if ty & 0x80 != 0 && ty <= 0x88 {
            let f = self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & 0xCF;
            self.mem.w8(DS, si.wrapping_add(item::FLAGS), f);
        }
        if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::EQUIPPED != 0 {
            self.unequip_enchantment(si);
        } else {
            self.unequip_clashes(si);
            self.equip_enchantment(si);
            self.attribute_modifiers(PLAYER);
        }
        self.toggle_equipped(si);
    }

    fn toggle_equipped(&mut self, si: u16) {
        let f = self.mem.r8(DS, si.wrapping_add(item::FLAGS)) ^ item::EQUIPPED;
        self.mem.w8(DS, si.wrapping_add(item::FLAGS), f);
    }

    /// 1194:511A then the flag flipped: a worn item taken off.
    fn take_off(&mut self, si: u16) {
        self.unequip_enchantment(si);
        self.toggle_equipped(si);
    }

    /// 1194:7644. Before the item at `si` is put on: armour of the same
    /// location (base type) worn among the LAST_COUNT slots comes off, a
    /// shield takes off a shield or a two-handed weapon
    /// (`shield_or_two_hander`); a weapon takes off the worn weapon
    /// (`worn_weapon`) and, two-handed (+3 not 1), a shield too.
    fn unequip_clashes(&mut self, si: u16) {
        match self.item_category(DS, si) {
            category::ARMOR => {
                let (base, _) = self.base_type(self.mem.r8(DS, si));
                if Self::is_shield(base) {
                    self.take_off_shield();
                    return;
                }
                let cx = self.mem.r8(DS, LAST_COUNT) as u16;
                if let (true, found) = self.find_equipped(DS, PLAYER_SLOTS, cx, base, category::ARMOR, false) {
                    self.take_off(found);
                }
            }
            category::WEAPON => {
                let hands = (self.mem.r8(DS, si.wrapping_add(item::HANDS)) as u16).wrapping_sub(1);
                if let Some(w) = self.worn_weapon() {
                    self.take_off(w);
                }
                if hands != 0 {
                    self.take_off_shield();
                }
            }
            _ => {}
        }
    }

    /// 1194:769F
    fn take_off_shield(&mut self) {
        if let Some(s) = self.shield_or_two_hander() {
            self.take_off(s);
        }
    }

    /// 1194:7527. The first worn weapon among the player's LAST_COUNT
    /// slots.
    fn worn_weapon(&self) -> Option<u16> {
        let mut si = PLAYER_SLOTS;
        for _ in 0..self.mem.r8(DS, LAST_COUNT) {
            let f = self.mem.r8(DS, si.wrapping_add(item::FLAGS));
            if f & item::EQUIPPED != 0 && self.item_category(DS, si) == category::WEAPON {
                return Some(si);
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        None
    }

    /// 1194:7548. The first worn shield or two-handed weapon (the table
    /// at 229E per base type holds 2) among the player's LAST_COUNT
    /// slots; special items mapping to a negative base type and
    /// accessories are passed over.
    fn shield_or_two_hander(&self) -> Option<u16> {
        let mut si = PLAYER_SLOTS;
        for _ in 0..self.mem.r8(DS, LAST_COUNT) {
            if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::EQUIPPED != 0 {
                let cat = self.item_category(DS, si);
                let (base, negative) = self.base_type(self.mem.r8(DS, si));
                if !negative {
                    let hit = match cat {
                        category::ARMOR => Self::is_shield(base),
                        category::WEAPON => self.mem.r8(DS, 0x229E_u16.wrapping_add(base)) == 2,
                        _ => false,
                    };
                    if hit {
                        return Some(si);
                    }
                }
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        None
    }

    /// 1194:5074. An item put on applies its enchantment: a special item
    /// is recorded (SPECIAL_WORN); an enchanted one with an enchantment
    /// (+F not FFh) raises the attribute +12h by +11h's place: an
    /// accessory of kind 1 by its type's amount (table 4AFE), others by
    /// 10, in percent, the amount kept at +11h; reaching 100 the
    /// attribute is FFh and +11h what was given. With no attribute
    /// (+12h FFh), value C1h sets 5CA bit 1000h.
    fn equip_enchantment(&mut self, si: u16) {
        let ty = self.mem.r8(DS, si);
        if ty & 0x80 != 0 {
            self.mem.w8(DS, SPECIAL_WORN, ty);
            return;
        }
        let b = |m: &Self, f: u16| m.mem.r8(DS, si.wrapping_add(f));
        if self.mem.r8(DS, si.wrapping_add(item::FLAGS)) & item::ENCHANTED == 0 || b(self, item::ENCHANT) == 0xFF {
            return;
        }
        if b(self, item::ENCHANT_ATTR) == 0xFF {
            if b(self, item::ENCHANT_VALUE) == 0xC1 {
                let f = self.mem.r16(DS, 0x05CA) | 0x1000;
                self.mem.w16(DS, 0x05CA, f);
            }
            return;
        }
        let at = PLAYER.wrapping_add(crate::character::field::ATTRS).wrapping_add(b(self, item::ENCHANT_ATTR) as u16);
        let pct = crate::scale::to100(self.mem.r8(DS, at) as i16) as u16;
        let ring = self.item_category(DS, si) == category::ACCESSORY && self.armor_kind(DS, si) == 1;
        let (sum, gain) = if ring {
            let gain = self.mem.r8(DS, 0x4AFE_u16.wrapping_add(ty as u16));
            (pct & 0xFF00 | (pct as u8).wrapping_add(gain) as u16, gain)
        } else {
            (pct.wrapping_add(0x0A), 0x0A)
        };
        if sum >= 0x64 {
            self.mem.w8(DS, at, 0xFF);
            self.mem.w8(DS, si.wrapping_add(item::ENCHANT_VALUE), 0x64u16.wrapping_sub(pct) as u8);
            return;
        }
        self.mem.w8(DS, si.wrapping_add(item::ENCHANT_VALUE), gain);
        let v = crate::scale::to256(pct.wrapping_add(gain as u16) as i16);
        self.mem.w8(DS, at, v as u8);
    }

    /// 1194:18F6..195C. The selected item (SELECTED, slot number; FFh is
    /// none, which shows a message the port leaves to the front end) is
    /// dropped: copied to DROPPED (its slot number at DROPPED_INDEX),
    /// taken out of the inventory (1194:788D) and the weapon class made
    /// again; nothing is selected; the line count is taken again
    /// (1194:1CF0: the player's occupied slots, or the other list's
    /// count) unless the other list is up (OTHER_LIST), with the armour
    /// class (the redraw's 7719), and the first line shown kept in
    /// range; the copy is no longer worn and lands on the floor
    /// (`drop_to_floor`). Returns whether an item was dropped.
    pub fn drop_item(&mut self) -> bool {
        self.drop_item_drawn(&mut |m| {
            m.player_armor_class();
            Some(())
        }) == Some(true)
    }

    /// `drop_item` with `drawn` for 1CF0's redraw of the screen (23B1,
    /// whose 7719 makes the armour class again) after the line count is
    /// taken again. None where `drawn` gives up.
    pub fn drop_item_drawn(&mut self, drawn: &mut dyn FnMut(&mut Machine) -> Option<()>) -> Option<bool> {
        let index = self.mem.r8(DS, SELECTED);
        if index == 0xFF {
            return Some(false);
        }
        let si = PLAYER_SLOTS.wrapping_add((index as u16).wrapping_mul(SLOT_SIZE));
        self.mem.w8(DS, DROPPED_INDEX, index);
        self.memmove(DS, si, DS, DROPPED, SLOT_SIZE);
        self.remove_item(DS, si, index as u16);
        self.player_weapon_class();
        self.mem.w8(DS, SELECTED, 0xFF);
        if self.mem.r8(DS, OTHER_LIST) == 0 {
            let mut al = self.mem.r8(DS, OTHER_COUNT);
            if self.mem.r8(DS, OWN_ITEMS) != 0 {
                al = self.count_items(DS, PLAYER_SLOTS).0 as u8;
            }
            self.mem.w8(DS, LAST_COUNT, al);
            drawn(self)?;
        }
        let ax = (self.mem.r8(DS, LAST_COUNT) as u16).wrapping_add(self.mem.r16(DS, LINES)).wrapping_sub(7);
        if self.mem.r8(DS, FIRST_LINE) >= ax as u8 {
            let al = ax.wrapping_add(6) as u8;
            if self.mem.r8(DS, FIRST_LINE) > al {
                self.mem.w8(DS, FIRST_LINE, al);
            }
        }
        let f = self.mem.r8(DS, DROPPED.wrapping_add(item::FLAGS)) & 0x7F;
        self.mem.w8(DS, DROPPED.wrapping_add(item::FLAGS), f);
        self.drop_to_floor(DROPPED);
        Some(true)
    }

    /// 1194:5F13, before the character sheet (2542:00E5): whether an
    /// item can be dropped where the player stands. No room (PLAYER_FLAGS
    /// bit NO_ROOM set, DROP_LOST kept) when the player's square (CB90)
    /// has a byte in the plane at [D806]; else lost (DROP_LOST 1) when
    /// C39A has 200h or C398 4; else no room when the square is one of
    /// the 17h actors' squares at 1951; else lost when C39A has 10h or
    /// the square's word in the plane at [CCD0] less the word at 9030 is
    /// C00h or more (DROP_LOST 0 otherwise). Lost or not, the bit is
    /// cleared. (The actors' squares are read at ES:1951, ES DGROUP from
    /// 2542:00BE.)
    pub fn drop_place(&mut self) {
        let sq = self.mem.r16(DS, 0xCB90);
        let plane = self.mem.r16(DS, 0xD806);
        let flags = self.mem.r16(DS, PLAYER_FLAGS);
        let (c39a, c398) = (self.mem.r16(DS, 0xC39A), self.mem.r16(DS, 0xC398));
        let busy = self.mem.r8(plane, sq) != 0
            || (c39a & 0x200 == 0 && c398 & 4 == 0 && (0..0x17u16).any(|i| self.mem.r16(DS, 0x1951 + i * 2) == sq));
        if busy {
            self.mem.w16(DS, PLAYER_FLAGS, flags | NO_ROOM);
            return;
        }
        let lost = c39a & 0x210 != 0 || c398 & 4 != 0 || {
            let floor = self.mem.r16(DS, 0xCCD0);
            self.mem.r16(floor, sq).wrapping_sub(self.mem.r16(DS, 0x9030)) >= 0xC00
        };
        self.mem.w8(DS, DROP_LOST, lost as u8);
        self.mem.w16(DS, PLAYER_FLAGS, flags & !NO_ROOM);
    }

    /// 1194:17D8, the inventory's drop button. `ask` is the yes-or-no
    /// question (1194:48E9, its text at DROP_TEXT), true for yes. In
    /// order: no room (`drop_place`) says so; the list not the player's
    /// own does nothing; a shop's or a looting's list (NOT_OWN) can't be
    /// dropped from; where an item would be lost (DROP_LOST) the player
    /// is asked whether to drop it anyway; else with nothing carried
    /// nothing happens (the count kept at 129D otherwise), with nothing
    /// selected "Drop what?", and the selected item is asked about by
    /// name. A yes drops it (1194:18F6, `drop_item`, which with nothing
    /// selected says "Drop what?" too). The texts are copied to DROP_TEXT
    /// (1194:1BCE); the colour at 9288 is FEh for them and A0h after the
    /// two refusals; the font (B036) is 1026's for a question and 102C's
    /// after it (between them 102C's for a question wider than 37h, which
    /// only its drawing sees). With "Drop what?" up the original waits
    /// and starts over if an item was selected meanwhile; the port
    /// returns. None where the item's name gives up.
    pub fn drop_button(&mut self, ask: &mut dyn FnMut(&mut Machine) -> bool) -> Option<DropButton> {
        self.drop_button_drawn(ask, &mut |m| {
            m.player_armor_class();
            Some(())
        })
    }

    /// `drop_button` with `drawn` the redraw after a drop (`drop_item_drawn`).
    pub fn drop_button_drawn(&mut self, ask: &mut dyn FnMut(&mut Machine) -> bool, drawn: &mut dyn FnMut(&mut Machine) -> Option<()>) -> Option<DropButton> {
        let colour = |m: &mut Machine, c: u8| m.mem.w8(DS, 0x9288, c);
        if self.mem.r16(DS, PLAYER_FLAGS) & NO_ROOM != 0 {
            colour(self, 0xFE);
            self.drop_text(text::NO_ROOM);
            colour(self, 0xA0);
            return Some(DropButton::NoRoom);
        }
        if self.mem.r8(DS, OWN_ITEMS) == 0 {
            return Some(DropButton::Nothing);
        }
        colour(self, 0xFE);
        if self.mem.r16(DS, NOT_OWN) != 0 {
            self.drop_text(text::CANT_DROP);
            colour(self, 0xA0);
            return Some(DropButton::CantDrop);
        }
        if self.mem.r8(DS, DROP_LOST) != 0 {
            self.mem.w16(DS, 0xB036, self.mem.r16(DS, 0x1026));
            self.drop_text(text::LOST);
            let yes = ask(self);
            self.mem.w16(DS, 0xB036, self.mem.r16(DS, 0x102C));
            return if yes { self.drop_selected(drawn) } else { Some(DropButton::Nothing) };
        }
        let (count, _) = self.count_items(DS, PLAYER_SLOTS);
        if count == 0 {
            return Some(DropButton::Nothing);
        }
        self.mem.w8(DS, 0x129D, count as u8);
        let sel = self.mem.r8(DS, SELECTED);
        if sel == 0xFF {
            return Some(self.drop_what());
        }
        let it = PLAYER_SLOTS.wrapping_add((sel as u16).wrapping_mul(SLOT_SIZE));
        let name = self.display_name(it)?;
        self.sprintf((DS, DROP_TEXT), (DATA, text::DROP_FORMAT), &[Arg::Str(DS, name)]);
        colour(self, 0xFE);
        self.mem.w16(DS, 0xB036, self.mem.r16(DS, 0x1026));
        let yes = ask(self);
        self.mem.w16(DS, 0xB036, self.mem.r16(DS, 0x102C));
        if yes { self.drop_selected(drawn) } else { Some(DropButton::Nothing) }
    }

    /// 1194:18F6 from the drop button: the selected item dropped, or with
    /// none "Drop what?".
    fn drop_selected(&mut self, drawn: &mut dyn FnMut(&mut Machine) -> Option<()>) -> Option<DropButton> {
        if self.mem.r8(DS, SELECTED) == 0xFF {
            return Some(self.drop_what());
        }
        self.drop_item_drawn(drawn)?;
        Some(DropButton::Dropped)
    }

    /// 1194:18CA: "Drop what?" in the colour FEh.
    fn drop_what(&mut self) -> DropButton {
        self.drop_text(text::DROP_WHAT);
        self.mem.w8(DS, 0x9288, 0xFE);
        DropButton::DropWhat
    }

    /// 1194:1BCE: a text of segment 3276 copied to DROP_TEXT (0000:9CA0).
    fn drop_text(&mut self, at: u16) {
        let t = self.mem.cstr(DATA, at).to_vec();
        for (i, &c) in t.iter().chain(std::iter::once(&0)).enumerate() {
            self.mem.w8(DS, DROP_TEXT.wrapping_add(i as u16), c);
        }
    }

    /// 2433:09E1. The item at DGROUP `si` placed on the player's square:
    /// a record at FLOOR_ITEM (1, the kind of place `floor_kind`, the
    /// square (row, column doubled), the weather byte BB0A, FFFFh, FFh,
    /// 1, the item) appended to the level's item list in EMS bank 6
    /// (mapped meanwhile, when EMS is on with that bank). A record with
    /// FFFFh, FFh and a price of 0 gets 0Fh at +5 (2433:027D, after an
    /// INT 3 the game's DOS handler ignores).
    pub fn drop_to_floor(&mut self, si: u16) {
        use crate::level::var::{EMS_MAPPED, EMS_ON, EMS_PAGES};
        self.memmove(DS, si, DS, FLOOR_ITEM + 9, SLOT_SIZE);
        let mapped = self.mem.r8(DS, EMS_ON) != 0 && ITEMS_BANK < self.mem.r16(DS, EMS_PAGES);
        let old = self.mem.r16(DS, EMS_MAPPED) >> 2;
        self.ems_map(ITEMS_BANK);
        self.mem.w16(DS, FLOOR_ITEM + 5, 0xFFFF);
        self.mem.w8(DS, FLOOR_ITEM + 7, 0xFF);
        self.mem.w8(DS, FLOOR_ITEM, 1);
        let row = (self.mem.r16(DS, crate::movement::var::Y) >> 7) as u8;
        let col = ((self.mem.r16(DS, crate::movement::var::X) >> 7) as u8).wrapping_add((self.mem.r16(DS, crate::movement::var::X) >> 7) as u8);
        self.mem.w16(DS, FLOOR_ITEM + 2, (row as u16) << 8 | col as u16);
        self.mem.w8(DS, FLOOR_ITEM + 8, 1);
        let kind = self.floor_kind();
        self.mem.w8(DS, FLOOR_ITEM + 1, kind);
        let w = self.mem.r8(DS, 0xBB0A);
        self.mem.w8(DS, FLOOR_ITEM + 4, w);
        // 2433:0258
        let frame = self.mem.r16(DS, crate::movement::var::FRAME_SEG);
        let di = (self.mem.r8(DS, LEVEL_ITEMS) as u16).wrapping_mul(0x1C).wrapping_add(LEVEL_ITEM_LIST);
        if self.mem.r16(DS, FLOOR_ITEM + 5) == 0xFFFF
            && self.mem.r8(DS, FLOOR_ITEM + 7) == 0xFF
            && self.mem.r32(DS, FLOOR_ITEM + 0x13) == 0
        {
            self.mem.w16(DS, FLOOR_ITEM + 5, 0x0F);
        }
        self.memmove(DS, FLOOR_ITEM, frame, di, 0x1C);
        let n = self.mem.r8(DS, LEVEL_ITEMS).wrapping_add(1);
        self.mem.w8(DS, LEVEL_ITEMS, n);
        if mapped {
            self.ems_map(old);
        }
    }

    /// 2433:0201. The kind of place an item lies in: 5 in the wild, 3
    /// outdoors or anywhere but a building (5E6 not 4), else by the
    /// building (D48): 1 for 1, 2 for 3, 4 for 8 and 0Bh, 0 otherwise.
    pub fn floor_kind(&self) -> u8 {
        if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x200 != 0 {
            return 5;
        }
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 4 != 0 || self.mem.r8(DS, 0x05E6) != 4 {
            return 3;
        }
        match self.mem.r8(DS, 0x0D48) {
            1 => 1,
            3 => 2,
            8 | 0x0B => 4,
            _ => 0,
        }
    }
}

/// The item description's DGROUP variables (1194:1988, 7CBA).
pub mod info {
    /// The text built (lines ended by CR).
    pub const TEXT: u16 = 0x1044;
    /// What `sprintf` writes before it is appended.
    pub const SCRATCH: u16 = 0x929E;
    /// The lines to draw (1988 sets 3), and the word 1988 clears that the
    /// formatting reuses for its arguments' size (0 leaves the first line
    /// in 55h).
    pub const LINES: u16 = 0x12B0;
    pub const ARGS: u16 = 0x12B2;
    /// The weight's format ("%d kgs", segment 3276), whose conversion
    /// letter 7EAF rewrites ('d' or 's').
    pub const WEIGHT: u16 = 0x6CC3;
}

impl Machine {
    /// 0000:9BD0: the string at `seg`:`si` appended to DS:TEXT; the
    /// offset after its NUL.
    fn info_append(&mut self, seg: u16, mut si: u16) -> u16 {
        let mut di = info::TEXT;
        for _ in 0..0x10000u32 {
            if self.mem.r8(DS, di) == 0 {
                break;
            }
            di = di.wrapping_add(1);
        }
        loop {
            let c = self.mem.r8(seg, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return di;
            }
        }
    }

    /// `sprintf` into DS:SCRATCH, appended to the text.
    fn info_line(&mut self, fmt: u16, args: &[Arg]) -> u16 {
        self.sprintf((DS, info::SCRATCH), (DATA, fmt), args);
        self.info_append(DS, info::SCRATCH)
    }

    /// 1194:7CBA. The description of the item at DGROUP `it` that a right
    /// click on the equipment list shows (1194:1988), as CR-ended lines at
    /// TEXT: its name, then by its kind (1194:748E) its use (7D79, 7DE1,
    /// 7CF8), its weight and condition, its enchantment; LINES counts an
    /// enchantment's line. A special item's (type bit 80h) is then shown
    /// in a message box of its own (7E44), and the carry (true) tells
    /// 1988 to draw nothing more. None where the original loops for ever
    /// (a weight under 100h found nowhere in DGROUP, 1194:F2FC; a text's
    /// key not in the index) or the text gives up.
    pub fn item_info(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, it: u16) -> Option<bool> {
        let name = self.display_name(it)?;
        // 0000:9CA0, then its NUL made a CR.
        let mut di = info::TEXT;
        let mut si = name;
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        self.mem.w8(DS, di.wrapping_sub(1), 0x0D);
        self.mem.w8(DS, di, 0);
        match self.item_category(DS, it) {
            category::ACCESSORY => self.info_accessory(it)?,
            category::STACK => {
                if self.inv_flags(DS, it) & 2 != 0 {
                    self.info_potion(files, hooks, it)?;
                } else {
                    self.info_append(DATA, 0x6D53);
                }
            }
            _ => self.info_gear(it)?,
        }
        if self.mem.r8(DS, it) & 0x80 != 0 {
            self.info_special(files, hooks, it)?;
            return Some(true);
        }
        Some(false)
    }

    /// 1194:7CF8 for an identified stack: its text (597h past its type,
    /// 1194:A36C mode 2, wrapped at 21h, with 91F6 its +4 for the
    /// substitutions and the random seed) in place of the name: TEXT cleared for C8h bytes
    /// first, its line feeds made CRs (1194:49DF) and its NUL a CR; the
    /// first line in FEh (ARGS 1) and one line more.
    fn info_potion(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, it: u16) -> Option<()> {
        self.mem.w16(DS, info::ARGS, 1);
        let n = self.mem.r16(DS, info::LINES).wrapping_add(1);
        self.mem.w16(DS, info::LINES, n);
        for i in 0..0xC8u16 {
            self.mem.w8(DS, info::TEXT + i, 0);
        }
        // The random seed (91F6) the item's +4 while the text is made, so
        // that its variant is the item's own; the game's put back after.
        self.store_rng();
        let saved = self.mem.r32(DS, 0x91F6);
        let amount = self.mem.r16(DS, it.wrapping_add(4)) as i16 as i32 as u32;
        self.mem.w32(DS, 0x91F6, amount);
        self.load_rng();
        let t = (self.mem.r8(DS, it) as u16).wrapping_add(0x597);
        self.mem.w16(DS, crate::text::var::TEXT, t);
        self.mem.w8(DS, crate::text::var::WIDTH, 0x21);
        let made = self.text(files, hooks, 2, DS, info::TEXT);
        self.mem.w32(DS, 0x91F6, saved);
        self.load_rng();
        made?;
        // 0000:9AF8.
        let font = self.mem.r16(DS, 0x103A);
        self.mem.w16(DS, 0xB038, font);
        // 1194:49DF over the text's length (0000:9CAC), stopping at a NUL.
        let mut si = info::TEXT;
        let len = self.mem.cstr(DS, si).len();
        for _ in 0..len {
            match self.mem.r8(DS, si) {
                0 => break,
                0x0A => self.mem.w8(DS, si, 0x0D),
                _ => {}
            }
            si = si.wrapping_add(1);
        }
        // 1194:559F: past the first string (3000 bytes at most).
        let mut si = info::TEXT;
        for k in 1..=0xBB8 {
            let c = self.mem.r8(DS, si);
            si = si.wrapping_add(1);
            if c == 0 || k == 0xBB8 {
                break;
            }
        }
        self.mem.w8(DS, si.wrapping_sub(1), 0x0D);
        self.mem.w8(DS, si, 0);
        Some(())
    }

    /// 1194:7E44. A special item's box: a block of 7530h bytes taken from
    /// the heap's top (0000:9CD2, its segment kept at 12B4), the text so
    /// far copied there and the special item's own text appended (1F9h
    /// past its place in the 16 types at 78C3, 1194:D316; 1194:A36C mode
    /// 2, wrapped at 3Ch), shown in a message box (1194:59CC) and the
    /// block freed (0000:6C32); AFF9 2.
    fn info_special(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, it: u16) -> Option<()> {
        let seg = self.heap_alloc_high(0x7530)?;
        if seg == 0 {
            return crate::giveup::not_ported("0000:9CD2: no room for 7E44's block: the original writes from 0:0");
        }
        self.mem.w16(DS, 0x12B4, seg);
        let ty = self.mem.r8(DS, it);
        let mut k = 0u16;
        while k < 0x10 && self.mem.r8(DS, 0x78C3 + k) != ty {
            k += 1;
        }
        self.mem.w16(DS, crate::text::var::TEXT, k.wrapping_add(0x1F9));
        let (mut si, mut di) = (info::TEXT, 0u16);
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(seg, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        self.mem.w8(DS, crate::text::var::WIDTH, 0x3C);
        self.text(files, hooks, 2, seg, di.wrapping_sub(1))?;
        let font = self.mem.r16(DS, 0x103A);
        self.mem.w16(DS, 0xB038, font);
        hooks.message_box_far(self, seg, 0);
        self.heap_free(seg);
        self.mem.w8(DS, 0xAFF9, 2);
        Some(())
    }

    /// 1194:7D79. A weapon's damage (+4, +5) or armour's protection (+4
    /// over 5), then the weight (7EAF), the condition (7BE7) and the
    /// enchantment (7F2E), which counts a line.
    fn info_gear(&mut self, it: u16) -> Option<()> {
        let b = |m: &Self, f: u16| m.mem.r8(DS, it.wrapping_add(f)) as u16;
        if self.item_category(DS, it) == 1 {
            self.mem.w16(DS, info::ARGS, 0x0C);
            let (lo, hi) = (b(self, 4), b(self, 5));
            self.info_line(0x6CDA, &[Arg::Int(lo), Arg::Int(hi)]);
        } else {
            self.mem.w16(DS, info::ARGS, 0x0A);
            let v = b(self, 4) / 5;
            self.info_line(0x6CF4, &[Arg::Int(v)]);
        }
        self.info_weight(it)?;
        let cond = self.condition_name(DS, it);
        self.info_line(0x6CCB, &[Arg::Str(DS, cond)]);
        if self.info_enchantment(it) {
            let n = self.mem.r16(DS, info::LINES).wrapping_add(1);
            self.mem.w16(DS, info::LINES, n);
        }
        Some(())
    }

    /// 1194:7EAF. "%d kgs" for a weight (+1) of 100h and up (its bytes
    /// swapped), else "%s kgs" with its quarter's string (1194:F2FC: the
    /// words from 2413 searched for it, its index the string's past 2419);
    /// the format's letter written as chosen.
    fn info_weight(&mut self, it: u16) -> Option<()> {
        let w = self.mem.r16(DS, it.wrapping_add(1));
        let arg = if w >= 0x100 {
            self.mem.w16(DS, info::ARGS, 0x0A);
            self.mem.w8(DATA, info::WEIGHT + 1, b'd');
            Arg::Int(w.swap_bytes())
        } else {
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
                return crate::giveup::original("1194:F2FC: a weight found nowhere loops for ever");
            }
            self.mem.w16(DS, info::ARGS, 0x0C);
            self.mem.w8(DATA, info::WEIGHT + 1, b's');
            Arg::Str(DS, self.skip_strings(DS, 0x2419, i))
        };
        self.info_line(info::WEIGHT, &[arg]);
        Some(())
    }

    /// 1194:7DE1. An accessory: of armour kind 0 its charges (+3) unless
    /// special; of kind 2 its protection; another its enchantment (7F2E,
    /// its line not counted).
    fn info_accessory(&mut self, it: u16) -> Option<()> {
        match self.armor_kind(DS, it) {
            0 => {
                if self.mem.r8(DS, it) & 0x80 == 0 {
                    let n = self.mem.r8(DS, it.wrapping_add(3)) as u16;
                    self.info_line(0x6D09, &[Arg::Int(n)]);
                }
            }
            2 => {
                let v = self.mem.r8(DS, it.wrapping_add(4)) as u16 / 5;
                self.info_line(0x6D27, &[Arg::Int(v)]);
            }
            _ => {
                self.info_enchantment(it);
            }
        }
        Some(())
    }

    /// 1194:7F2E. An identified enchanted item's line (the carry): "+%d to
    /// %s" with its attribute's name (+12h among the strings at 2B00) and
    /// 0Ah (an accessory's type times 5 more), a CR more after it; with no
    /// attribute (+12h FFh) its uses left (+6 over 15), not for a special
    /// item. None for an item not identified (+0Eh bit 1), with no
    /// enchantment (+0Fh FFh), or for gear with bit 2.
    fn info_enchantment(&mut self, it: u16) -> bool {
        let cat = self.item_category(DS, it);
        let flags = self.inv_flags(DS, it);
        let b = |m: &Self, f: u16| m.mem.r8(DS, it.wrapping_add(f));
        if (cat != category::ACCESSORY && flags & 2 != 0) || flags & 1 == 0 || b(self, 0x0F) == 0xFF {
            return false;
        }
        if b(self, 0x12) != 0xFF {
            let v = if cat == category::ACCESSORY { (b(self, 0) as u16).wrapping_mul(5).wrapping_add(0x0A) } else { 0x0A };
            let s = self.skip_strings(DS, 0x2B00, b(self, 0x12) as u16);
            let di = self.info_line(0x6D1C, &[Arg::Int(v), Arg::Str(DS, s)]);
            self.mem.w8(DS, di.wrapping_sub(1), 0x0D);
            self.mem.w8(DS, di, 0);
            return true;
        }
        if b(self, 0) & 0x80 != 0 {
            return false;
        }
        let n = self.mem.r16(DS, it.wrapping_add(6)) / 0x0F;
        self.info_line(0x6D40, &[Arg::Int(n)]);
        true
    }
}
