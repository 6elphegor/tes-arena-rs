//! The save and load menus (segment 25FD): the slot picked on the
//! LOADSAVE.IMG screen (25FD:0718), the save's name typed, and the game
//! saved (25FD:0002) or loaded (25FD:0242).
//!
//! - The slot picker (0718) loads NAMES.DAT's ten names (30h bytes each)
//!   to 9686 and draws them, then takes clicks (1194:564D) until one
//!   falls on a slot's row: x 5..13Ah, row (y - 4) / 0Eh at most 9. A
//!   negative AX (Esc, the right button) cancels. Its row's place is
//!   kept at 127A/127C for the name's box.
//! - Saving (0002) is refused in a tavern, a temple or a store (in an
//!   interior, 5E6 = 4, of building kind D48 4, 5 or 6: "You cannot save
//!   in a tavern, store or temple..."). Otherwise a main quest dungeon's
//!   locks are kept (292B:095E), the slot picked, and its name typed in a
//!   box over its row (0000:9EE5 edits a copy at 1044; Esc clears A902
//!   bit 1000h and the menu starts again). Then the name goes back into
//!   9686, "Kindly waiteth..." (0803), 5E0 bit 80h from 191D, the slot's
//!   re*.nn and in*.nn deleted (1194:4D91), the automap kept (23F1:0145)
//!   and the working files copied to the slot's (0612): the rest is
//!   `save_write.rs`'s (`save_game`, `save_finish`) and the CITYDATA
//!   check (242A:000E).
//! - Loading (0242) deletes log.64, re*.64 and in*.64, clears the
//!   message (61E) and 7A46, waits for the buttons, sets C995 and picks a
//!   slot; the load itself is `load.rs`'s `game_load`. A slot without a
//!   SAVEGAME brings the message, then the picker again.
//!
//! The pictures, fonts, boxes and the typing are `MenuHooks`.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The slots' names (ten of 30h bytes, NAMES.DAT's) and the copy the
    /// name is typed in.
    pub const NAMES: u16 = 0x9686;
    pub const TYPED: u16 = 0x1044;
    /// The picked slot's box: x, y.
    pub const BOX_X: u16 = 0x127A;
    pub const BOX_Y: u16 = 0x127C;
    /// The slot (1A6C) and the mouse's buttons (AFF9).
    pub const SLOT: u16 = 0x1A6C;
    pub const BUTTONS: u16 = 0xAFF9;
    /// A902: bit 0 cleared around the menu, bit 1000h while a name is
    /// typed (cleared by Esc).
    pub const FLAGS: u16 = 0xA902;
    /// "loadsave.img", "names.dat", the refusal, "Kindly waiteth...".
    pub const PICTURE: u16 = 0x5E7B;
    pub const NAMES_FILE: u16 = 0x5E88;
    pub const REFUSED: u16 = 0x5ED1;
    pub const WAIT: u16 = 0x5F26;
    /// "re*.00" and "in*.00" (the slot's digits at +4); log.64, re*.64,
    /// in*.64.
    pub const RE_SLOT: u16 = 0x5F01;
    pub const IN_SLOT: u16 = 0x5F08;
    pub const LOG_WORKING: u16 = 0x5CAF;
    pub const RE_WORKING: u16 = 0x5CB6;
    pub const IN_WORKING: u16 = 0x5CBD;
}
use var::*;

/// What the menus leave to the front end.
pub trait MenuHooks {
    /// 0000:8FC4 (and 2F26:02A1): the mouse's buttons waited for; AFF9
    /// clear after it. The default clears it.
    fn mouse_wait(&mut self, m: &mut Machine) {
        m.mem.w8(DS, BUTTONS, 0);
    }
    /// 0000:9AE6 (`text` false) and 9AF8 (true): a font's record (AX)
    /// made current for boxes and texts.
    fn font(&mut self, _m: &mut Machine, _ax: u16, _text: bool) {}
    /// 0000:5C86: EMS bank `bank` mapped (by default `Machine::ems_map`).
    fn map_bank(&mut self, m: &mut Machine, bank: u16) {
        m.ems_map(bank);
    }
    /// 0000:7428 then 5828: LOADSAVE.IMG (named at DS:`name`) loaded to
    /// the buffer at [902E] and drawn.
    fn picture(&mut self, _m: &mut Machine, _files: &mut Files, _name: u16) {}
    /// 0000:7428: NAMES.DAT (named at DS:`path`, the path made) loaded to
    /// NAMES. By default through `files` (`Machine::file_load`); a missing
    /// file leaves NAMES as it was.
    fn names(&mut self, m: &mut Machine, files: &mut Files, path: u16) {
        let _ = m.file_load(files, path, DS, NAMES);
    }
    /// 0000:9A6E: a box filled in colour 9288; 0000:5364: the text at
    /// DS:`si` centred on x `ax` at y `bx`. Drawing only.
    fn fill(&mut self, _m: &mut Machine, _ax: u16, _bx: u16, _cx: u16, _dx: u16) {}
    fn centred(&mut self, _m: &mut Machine, _si: u16, _ax: u16, _bx: u16) {}
    /// 0000:9466 and 541E: the screen shown.
    fn show(&mut self, _m: &mut Machine) {}
    /// 1194:564D: a click's x (AX) and y (BX); a negative AX cancels.
    fn click(&mut self, _m: &mut Machine) -> (u16, u16) {
        (0xFFFF, 0)
    }
    /// 0000:9EE5: the name at TYPED edited (2Fh characters at most) in
    /// the box at BOX_X + 0Ah, BOX_Y; Esc clears FLAGS bit 1000h.
    fn type_name(&mut self, _m: &mut Machine) {}
    /// 1194:59C8: a message box with the text at DS:`si`.
    fn message(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:4D91: the files matching the pattern at DS:`si` deleted (by
    /// default `Machine::delete_matching`).
    fn delete_matching(&mut self, m: &mut Machine, files: &mut Files, si: u16) {
        m.delete_matching(files, si);
    }
    /// 292B:095E: a main quest dungeon's level's locks kept (by default
    /// `Machine::states_keep`).
    fn states_keep(&mut self, m: &mut Machine, files: &mut Files) {
        m.states_keep(files);
    }
    /// 23F1:0145: the automap written (by default `Machine::automap_write`).
    fn automap_keep(&mut self, m: &mut Machine, files: &mut Files) {
        m.automap_write(files);
    }
    /// 2294:11F2: the load menu's screen made ready.
    fn load_screen(&mut self, _m: &mut Machine) {}
}

/// `MenuHooks` that cancel every menu.
pub struct NoMenuHooks;

impl MenuHooks for NoMenuHooks {}

/// How the save menu ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveChoice {
    /// Saving isn't allowed here (the message was shown).
    Refused,
    /// The picker was left.
    Cancelled,
    /// Slot `n` (its name typed) is to be saved: the caller writes it.
    Slot(u8),
}

impl Machine {
    fn menu_and16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) & v;
        self.mem.w16(DS, at, x);
    }

    fn menu_or16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) | v;
        self.mem.w16(DS, at, x);
    }

    /// 25FD:0002 waiting for the buttons, as the menu's start does (and
    /// 026B..027A): until AFF9 is clear.
    fn menu_wait(&mut self, hooks: &mut dyn MenuHooks) {
        while self.mem.r8(DS, BUTTONS) != 0 {
            hooks.mouse_wait(self);
        }
    }

    /// 25FD:0718. The slot picker: the screen (LOADSAVE.IMG, NAMES.DAT's
    /// names centred at x A0h from y 4, 0Eh apart, each over a box filled
    /// with colour 0), then clicks until one picks a row (see the
    /// module's notes); 5C8 bit 1 set before each. Returns the row, its
    /// box kept at BOX_X/BOX_Y; None where the click cancels. (The row's
    /// remainder test, at most 0Eh after a divide by 0Eh, always passes.)
    pub fn slot_pick(&mut self, files: &mut Files, hooks: &mut dyn MenuHooks) -> Option<u8> {
        self.menu_and16(FLAGS, 0xFFFE);
        let ax = self.mem.r16(DS, 0x102C);
        hooks.font(self, ax, false);
        let bank = self.mem.r16(DS, 0x1030);
        hooks.map_bank(self, bank);
        hooks.picture(self, files, PICTURE);
        let path = self.save_path(NAMES_FILE);
        hooks.names(self, files, path);
        let (mut si, mut bx) = (NAMES, 4u16);
        for _ in 0..10 {
            if self.mem.r8(DS, si) != 0 {
                self.mem.w8(DS, 0x9288, 0);
                hooks.fill(self, 5, bx, 0x136, 6);
                self.mem.w8(DS, 0x9288, 0x0F);
                hooks.centred(self, si, 0xA0, bx);
            }
            si += 0x30;
            bx += 0x0E;
        }
        hooks.show(self);
        self.menu_and16(FLAGS, 0xFFFE);
        loop {
            self.menu_or16(0x05C8, 2);
            let (ax, by) = hooks.click(self);
            if (ax as i16) < 0 {
                return None;
            }
            if !(5..=0x13A).contains(&ax) || by < 4 {
                continue;
            }
            let row = (by - 4) / 0x0E;
            while self.mem.r8(DS, BUTTONS) & 3 != 0 {
                hooks.mouse_wait(self);
            }
            if row > 9 {
                continue;
            }
            self.mem.w16(DS, BOX_X, 5);
            self.mem.w16(DS, BOX_Y, row * 0x0E + 4);
            return Some(row as u8);
        }
    }

    /// 25FD:0803. "Kindly waiteth. Ye olde diske needs be accessed...":
    /// the fonts, colour D4h, 5E0 bit 800h around the message, A902 bit 0
    /// cleared.
    fn menu_wait_message(&mut self, hooks: &mut dyn MenuHooks) {
        let ax = self.mem.r16(DS, 0x1036);
        hooks.font(self, ax, true);
        let ax = self.mem.r16(DS, 0x102C);
        hooks.font(self, ax, false);
        self.mem.w8(DS, 0x9288, 0xD4);
        self.menu_or16(0x05E0, 0x800);
        hooks.message(self, WAIT);
        self.menu_and16(0x05E0, 0xF7FF);
        self.menu_and16(FLAGS, 0xFFFE);
    }

    /// 25FD:0002..010F: the save menu up to the writing (see the module's
    /// notes). With `Slot`, the caller saves (0110 on: the working files
    /// copied to the slot's, `save_game`, `save_finish`, the CITYDATA
    /// check).
    pub fn save_menu(&mut self, files: &mut Files, hooks: &mut dyn MenuHooks) -> SaveChoice {
        loop {
            self.menu_wait(hooks);
            if self.mem.r8(DS, 0x05E6) == 4 && matches!(self.mem.r8(DS, 0x0D48), 4..=6) {
                let ax = self.mem.r16(DS, 0x1036);
                hooks.font(self, ax, true);
                let ax = self.mem.r16(DS, 0x102C);
                hooks.font(self, ax, false);
                self.mem.w8(DS, 0x9288, 0xD4);
                hooks.message(self, REFUSED);
                return SaveChoice::Refused;
            }
            hooks.states_keep(self, files);
            let Some(slot) = self.slot_pick(files, hooks) else {
                // 0212: the slot back to the working one.
                let w = self.mem.r8(DS, 0x1A80);
                self.mem.w8(DS, SLOT, w);
                return SaveChoice::Cancelled;
            };
            self.mem.w8(DS, SLOT, slot);
            let at = NAMES + slot as u16 * 0x30;
            for i in 0..0x30 {
                let b = self.mem.r8(DS, at + i);
                self.mem.w8(DS, TYPED + i, b);
            }
            hooks.show(self);
            let (x, y) = (self.mem.r16(DS, BOX_X), self.mem.r16(DS, BOX_Y));
            self.mem.w8(DS, 0x9288, 0);
            hooks.fill(self, x, y, 0x135, 9);
            self.menu_or16(FLAGS, 0x1000);
            self.mem.w8(DS, 0x9288, 0x0F);
            hooks.type_name(self);
            if self.mem.r16(DS, FLAGS) & 0x1000 == 0 {
                continue;
            }
            self.menu_and16(FLAGS, 0xEFFF);
            for i in 0..0x30 {
                let b = self.mem.r8(DS, TYPED + i);
                self.mem.w8(DS, at + i, b);
            }
            self.menu_wait_message(hooks);
            self.menu_and16(0x05E0, 0xFF7F);
            if self.mem.r8(DS, 0x191D) != 0 {
                self.menu_or16(0x05E0, 0x80);
            }
            let s = self.mem.r8(DS, SLOT);
            let digits = (((s % 10) as u16) << 8 | (s / 10) as u16).wrapping_add(0x3030);
            self.mem.w16(DS, RE_SLOT + 4, digits);
            self.mem.w16(DS, IN_SLOT + 4, digits);
            hooks.delete_matching(self, files, RE_SLOT);
            hooks.delete_matching(self, files, IN_SLOT);
            hooks.automap_keep(self, files);
            return SaveChoice::Slot(slot);
        }
    }

    /// 25FD:0242..028C: the load menu up to the load: log.64, re*.64 and
    /// in*.64 deleted, the message's time (61E) and 7A46 cleared, the
    /// screen made ready (2294:11F2), then (026B, where an empty slot
    /// comes back to) the buttons waited for, C995 1, the screen shown
    /// and a slot picked. None where the picker is left (034A).
    pub fn load_menu(&mut self, files: &mut Files, hooks: &mut dyn MenuHooks) -> Option<u8> {
        for si in [IN_WORKING, LOG_WORKING, RE_WORKING] {
            hooks.delete_matching(self, files, si);
        }
        self.mem.w16(DS, 0x061E, 0);
        self.mem.w16(DS, 0x7A46, 0);
        hooks.load_screen(self);
        self.load_menu_again(files, hooks)
    }

    /// 25FD:026B..028C: the load menu again (after an empty slot).
    pub fn load_menu_again(&mut self, files: &mut Files, hooks: &mut dyn MenuHooks) -> Option<u8> {
        loop {
            hooks.mouse_wait(self);
            if self.mem.r8(DS, BUTTONS) == 0 {
                break;
            }
        }
        self.mem.w8(DS, 0xC995, 1);
        hooks.show(self);
        self.slot_pick(files, hooks)
    }

    /// 1194:4D91 (4F89): the files matching the pattern at DS:`si` (a
    /// prefix, '*', then the extension) deleted. The original makes the
    /// pattern's path, then each found file's (25FD:0844, which flips the
    /// path buffers each time) from the DTA, and deletes it (0000:9DC2).
    /// Here the files are found through `Files`, and each match flips the
    /// buffers with the pattern's path instead: the toggle ends as the
    /// original's, but the buffers hold the pattern's path, not the last
    /// file's.
    pub fn delete_matching(&mut self, files: &mut Files, si: u16) {
        self.save_path(si);
        let pattern = self.mem.cstr(DS, si).to_ascii_uppercase();
        let Some(star) = pattern.iter().position(|&c| c == b'*') else {
            // No wildcard: find first finds the file itself, if it's there.
            if files.exists(&pattern) {
                self.save_path(si);
                files.delete(&pattern);
            }
            return;
        };
        let (prefix, rest) = (&pattern[..star], &pattern[star + 1..]);
        let ext = rest.strip_prefix(b".").unwrap_or(rest);
        // DOS's '?' takes any character or none (1194:D55F's "eq*.??").
        let fits = |e: &[u8]| {
            e.len() <= ext.len() && ext.iter().enumerate().all(|(i, &c)| e.get(i).map_or(c == b'?', |&d| c == b'?' || c == d))
        };
        for name in files.with_ext_where(fits) {
            if !name.starts_with(prefix) {
                continue;
            }
            self.save_path(si);
            files.delete(&name);
        }
    }
}
