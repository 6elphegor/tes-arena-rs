//! A spell of the player's own looked at: the equipment screen's spells
//! page (1194:1B3C, from a left click on the line already marked) shows
//! the marked spell's page (1194:51D1) and stays on it until Exit.
//!
//! The page (1194:68A3 with 1001 FFh: CHARSPEL.IMG) is the spellmaker's
//! screen for the spell's record (DATA: its number on the player's list
//! at 50B, times 55h), copied to 57E6; 68A3 also takes 528h bytes of the
//! heap for the name's background (A890, kept at 12BC and freed before
//! the next page and at the end) and sets the level the costs divide by
//! (A898, A89A: the player's plus one). Its clicks (8F92, 8F94 when AFF9
//! says a button):
//! - the name (45h..83h, 30h..39h): typed over (1194:65FD into 581A) and
//!   copied to the record's name (+34h); both spells' files written
//!   (29CF:06F5, the 2A80h bytes from DATA:0; 06CB, the player's AA0h
//!   from DATA:2A80, each named by the save slot 1A6C);
//! - on the bottom row (from B8h; x 5 is no button): the next spell
//!   (x up to 3Ch; none past the last), the one before (62h..A5h; none
//!   before the first), Delete (CEh..103h), Exit (118h on);
//! - Delete asks "Delete spell / NAME / from your spellbook?" (4BAA in a
//!   yes-or-no menu, 1194:48E9, with spparts.rci registered around it,
//!   0000:756C and 9326, its slot's offset left at 1284 and in B038):
//!   yes takes the spell off the list (the bytes after it moved down to
//!   5ABh, the count less one), a spell of the player's own making (its
//!   number 80h and up) has its record's name cleared, the list's mark
//!   goes back one (not below 0), and then the next spell is shown as
//!   the button would. Bug kept: with the last line deleted the mark is
//!   already the last and nothing is shown again, the deleted spell's
//!   page staying up.
//!
//! Exit fades to black (1194:55B2), frees the block and puts the
//! equipment screen back (the screen kept in SPELBOOK.$$$ at 51B8,
//! EQUIP.IMG's palette, CHARBK00.IMG); 1B46 then leaves the list's top
//! and mark at 0 and B038 from 103A, and redraws the list (21C2, 1DC8).
//! The screen's file and the palette 6E38 keeps at 929E are the front
//! end's.

use crate::bsa::Files;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// DGROUP variables.
pub mod var {
    /// The player's spells: their count, then their numbers.
    pub const COUNT: u16 = 0x050A;
    pub const LIST: u16 = 0x050B;
    /// The list's top line and its mark (the spells' index).
    pub const TOP: u16 = 0x1297;
    pub const MARK: u16 = 0x1296;
    /// The mark as the page keeps it.
    pub const SHOWN: u16 = 0x13C8;
    /// The record shown (DATA offset, segment).
    pub const RECORD: u16 = 0x1280;
    /// The page's name background block (12BC; 68A3's A890).
    pub const BLOCK: u16 = 0x12BC;
    pub const NAME_BACK: u16 = 0xA890;
    /// "the player's spell" for 68D7 (CHARSPEL.IMG).
    pub const OWN: u16 = 0x1001;
    /// The Delete question's resource slot.
    pub const PARTS: u16 = 0x1284;
}

/// What the page leaves to the front end.
pub trait SpellPageHooks {
    /// 1194:68D7: the page drawn for the record at 57E6 (1001 FFh).
    fn page(&mut self, _m: &mut Machine) {}
    /// 0000:8FC4: the buttons (AFF9) and the cursor (8F92, 8F94); by
    /// default Exit.
    fn click(&mut self, _m: &mut Machine) -> (u8, u16, u16) {
        (1, 0x120, 0xC0)
    }
    /// 1194:65FD: the name typed into 581A (0000:9EE5 at 45h, 30h, 83h
    /// wide, the cursor at the click's x less 45h, at most 20h bytes).
    fn type_name(&mut self, _m: &mut Machine) {}
    /// 1194:48E9: the yes-or-no menu of the text at DS:`si`; 0 yes.
    fn confirm(&mut self, _m: &mut Machine, _si: u16) -> u16 {
        1
    }
    /// 1194:55B2 and 53E3..9466: faded out, the equipment screen back.
    fn leave(&mut self, _m: &mut Machine) {}
    /// 25FD:0844, 0000:9DD2: `n` bytes of DATA from `off` written to the
    /// file named at DS:`name`.
    fn write(&mut self, m: &mut Machine, files: &mut Files, name: u16, off: u16, n: u16) {
        m.file_write(files, name, DATA, off, n);
    }
}

/// `SpellPageHooks` that draw nothing and leave at once.
pub struct NoSpellPageHooks;

impl SpellPageHooks for NoSpellPageHooks {}

impl Machine {
    /// 1194:1B3C: the marked spell's page (51D1), then the list's top and
    /// mark 0, the list's background put back and the list drawn again
    /// (21C2, 1DC8 with the spells' 50Bh) and B038 from 103A.
    pub fn spell_page(&mut self, files: &mut Files, hooks: &mut dyn SpellPageHooks) -> Option<()> {
        let mark = self.mem.r8(DS, var::MARK);
        self.spell_page_run(files, hooks, mark)?;
        self.mem.w8(DS, var::TOP, 0);
        self.mem.w8(DS, var::MARK, 0);
        self.list_back();
        self.list_again(files)?;
        // 1B63: 5E21's wait for the buttons' release, which leaves AFF9 0
        // after the page's last click.
        self.mem.w8(DS, 0xAFF9, 0);
        let font = self.mem.r16(DS, 0x103A);
        self.mem.w16(DS, 0xB038, font);
        Some(())
    }

    /// `spell_page` as the equipment page's list's click runs it (its
    /// handler 1AC6 left there, `sheet_equipment_click` and
    /// `sheet_equipment_describe` answering false): then the page's loop
    /// on to its next turn.
    pub fn sheet_spell_page(&mut self, files: &mut Files, hooks: &mut dyn SpellPageHooks) -> Option<()> {
        self.spell_page(files, hooks)?;
        self.equipment_turn_end();
        Some(())
    }

    /// 1194:51D1 with AL `mark`.
    fn spell_page_run(&mut self, files: &mut Files, hooks: &mut dyn SpellPageHooks, mark: u8) -> Option<()> {
        self.mem.w16(DS, var::BLOCK, 0);
        self.spell_page_show(hooks, mark)?;
        loop {
            let (b, x, y) = hooks.click(self);
            self.mem.w8(DS, 0xAFF9, b);
            self.mem.w16(DS, 0x8F92, x);
            self.mem.w16(DS, 0x8F94, y);
            if b == 0 {
                continue;
            }
            let (sx, sy) = (x as i16, y as i16);
            if (0x45..=0x83).contains(&sx) && (0x30..=0x39).contains(&sy) {
                self.spell_page_rename(files, hooks);
                continue;
            }
            if x == 5 || y < 0xB8 {
                continue;
            }
            let next = if x <= 0x3C {
                true
            } else if x < 0x62 {
                continue;
            } else if x <= 0xA5 {
                // 1194:539B: the one before.
                let m = self.mem.r8(DS, var::MARK);
                if m == 0 {
                    continue;
                }
                self.mem.w8(DS, var::MARK, m - 1);
                self.spell_page_show(hooks, m - 1)?;
                continue;
            } else if x <= 0xCD {
                continue;
            } else if x <= 0x103 {
                if !self.spell_page_delete(files, hooks)? {
                    continue;
                }
                true
            } else if x < 0x118 {
                continue;
            } else {
                false
            };
            // 1194:5376: the next, or Exit with none left.
            if next && self.mem.r8(DS, var::COUNT) != 0 {
                let m = self.mem.r8(DS, var::MARK);
                if m == self.mem.r8(DS, var::COUNT).wrapping_sub(1) {
                    continue;
                }
                self.mem.w8(DS, var::MARK, m.wrapping_add(1));
                self.spell_page_show(hooks, m.wrapping_add(1))?;
                continue;
            }
            // 1194:53B5.
            hooks.leave(self);
            let block = self.mem.r16(DS, var::BLOCK);
            self.heap_free(block);
            return Some(());
        }
    }

    /// 1194:51E6: spell `mark` of the list shown (68A3).
    fn spell_page_show(&mut self, hooks: &mut dyn SpellPageHooks, mark: u8) -> Option<()> {
        self.mem.w8(DS, var::MARK, mark);
        self.mem.w8(DS, var::SHOWN, mark);
        let n = self.mem.r8(DS, var::LIST.wrapping_add(mark as u16)) as u16;
        let si = n.wrapping_mul(0x55);
        self.mem.w16(DS, var::RECORD, si);
        self.mem.w16(DS, var::RECORD + 2, DATA);
        self.mem.w8(DS, var::OWN, 0xFF);
        let block = self.mem.r16(DS, var::BLOCK);
        if block != 0 {
            self.heap_free(block);
        }
        // 1194:68A3.
        for i in 0..0x55u16 {
            let b = self.mem.r8(DATA, si.wrapping_add(i));
            self.mem.w8(DS, crate::spellmaker::var::SPELL + i, b);
        }
        let (seg, _) = self.heap_alloc_bytes(0x528)?;
        self.mem.w16(DS, var::NAME_BACK, seg);
        let level = (self.mem.r8(DS, 0x01AA) as u16).wrapping_add(1);
        self.mem.w16(DS, crate::spellmaker::var::PLAYER, level);
        self.mem.w16(DS, crate::spellmaker::var::LEVEL, level);
        hooks.page(self);
        self.mem.w16(DS, var::BLOCK, seg);
        self.mem.w8(DS, var::OWN, 0);
        Some(())
    }

    /// 1194:5271: the name typed and copied to the record, both spells'
    /// files written.
    fn spell_page_rename(&mut self, files: &mut Files, hooks: &mut dyn SpellPageHooks) {
        hooks.type_name(self);
        let at = self.mem.r16(DS, var::RECORD).wrapping_add(0x34);
        let seg = self.mem.r16(DS, var::RECORD + 2);
        let mut si = crate::spellmaker::var::NAME;
        let mut di = at;
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(seg, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        // 29CF:06F5 and 06CB: the slot's two digits (AL the tens, AH the
        // units) in each name.
        let slot = self.mem.r8(DS, 0x1A6C);
        let digits = u16::from_le_bytes([b'0'.wrapping_add(slot / 10), b'0'.wrapping_add(slot % 10)]);
        self.mem.w16(DS, 0x7F14, digits);
        hooks.write(self, files, 0x7F0C, 0, 0x2A80);
        self.mem.w16(DS, 0x7F1E, digits);
        hooks.write(self, files, 0x7F17, 0x2A80, 0xAA0);
    }

    /// 1194:52DB: Delete asked; true when it was answered yes and the
    /// spell taken off (5337..5375).
    fn spell_page_delete(&mut self, files: &mut Files, hooks: &mut dyn SpellPageHooks) -> Option<bool> {
        let at = self.mem.r16(DS, var::RECORD).wrapping_add(0x34);
        let seg = self.mem.r16(DS, var::RECORD + 2);
        self.sprintf((DS, 0x1044), (DS, 0x4BAA), &[Arg::Str(seg, at)]);
        self.mem.w8(DS, 0x9288, 0xFE);
        let parts = self.resource_add(files, 0x5AE8, 3, 3)?;
        self.mem.w16(DS, var::PARTS, parts);
        self.mem.w16(DS, 0xB038, parts);
        self.select_font(0x102C);
        let no = hooks.confirm(self, 0x1044);
        self.resource_drop(self.mem.r16(DS, var::PARTS));
        if no != 0 {
            return Some(false);
        }
        let bx = self.mem.r8(DS, var::MARK) as u16;
        let m = self.mem.r8(DS, var::MARK).wrapping_sub(1);
        self.mem.w8(DS, var::MARK, if m & 0x80 != 0 { 0 } else { m });
        let n = self.mem.r8(DS, var::LIST.wrapping_add(bx));
        let c = self.mem.r8(DS, var::COUNT).wrapping_sub(1);
        self.mem.w8(DS, var::COUNT, c);
        let di = var::LIST.wrapping_add(bx);
        let si = di.wrapping_add(1);
        let count = 0x5ABu16.wrapping_sub(si);
        for i in 0..count {
            let b = self.mem.r8(DS, si.wrapping_add(i));
            self.mem.w8(DS, di.wrapping_add(i), b);
        }
        if n & 0x80 != 0 {
            self.mem.w8(seg, at, 0);
        }
        Some(true)
    }
}
