//! The options screen Esc brings up in play (2542:000A, from the key
//! check 2542:0100: key 1 latched, or the CyberMan's button 20h).
//!
//! OP.IMG (named at 5B49) is loaded and drawn over the view, the sound
//! and music volumes (D3B, D3A: 0..64h) in their boxes (5C9A, 5C92) and
//! the detail bar (D37, 6..19h) drawn, and clicks are taken (1194:564D)
//! until one leaves. A click is matched against the table at 5C2C: ten
//! bytes an entry (x0, y0, x1, y1, signed, inclusive; the handler, near in
//! 2542; FFFFh closes the screen; x0 FFFFh ends the table). The bottom
//! row is New (0627), Load (061E), Save (0617), Exit (05DB) and Resume
//! (FFFFh); above them the volumes' arrows (0785/07C6 the sound up and
//! down, 079C/07DE the music, which also tells the music driver,
//! 0FFD:0DE8) and the detail bar (0653: the click's x through [5C86]
//! makes D37). A handler that leaves the carry set closes the screen
//! (New, Load, Save); the others leave it clear, the drawing they end
//! with (0000:5364's 9E0A, 9A6E) clearing it.
//!
//! A click that matches nothing (Esc's FFFFh among them: 564D returns AX
//! FFFFh for it, and every x0 is at least 0) waits for the next one. The
//! CyberMan's button leaves without the `popa` (2542:0062 jumps past
//! 0064), so the original's `ret` returns into the pushed registers: the
//! port gives up there.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The click table and the screen's picture's name.
    pub const CLICKS: u16 = 0x5C2C;
    pub const PICTURE: u16 = 0x5B49;
    /// The sound and the music volumes (0..64h) and their boxes (x0, y0,
    /// x1, y1).
    pub const SOUND: u16 = 0x0D3B;
    pub const MUSIC: u16 = 0x0D3A;
    pub const SOUND_BOX: u16 = 0x5C9A;
    pub const MUSIC_BOX: u16 = 0x5C92;
    /// The view's reach (the detail), 6..19h, and the detail bar's left
    /// (the table's entry at 5C86 starts with it: CBh).
    pub const DETAIL: u16 = 0x0D37;
    pub const DETAIL_LEFT: u16 = 0x5C86;
    /// The mouse's x and y.
    pub const MOUSE_X: u16 = 0x8F92;
    pub const MOUSE_Y: u16 = 0x8F94;
    /// The number drawn in a box, the colour boxes and texts take.
    pub const NUMBER: u16 = 0x929E;
    pub const COLOUR: u16 = 0x9288;
    /// The music driver's two words passed with the volume.
    pub const DRIVER_A: u16 = 0xEB93;
    pub const DRIVER_B: u16 = 0xEB00;
    /// "Are you sure ?" (its keys at 4B60, its entries at 4B63).
    pub const SURE: u16 = 0x4B50;
    /// The working slot's files New deletes.
    pub const LOG_WORKING: u16 = 0x5CAF;
    pub const RE_WORKING: u16 = 0x5CB6;
    pub const IN_WORKING: u16 = 0x5CBD;
}
use var::*;

/// What the screen leaves to the front end.
pub trait OptionsHooks {
    /// 0000:9AE6 (`text` false) and 9AF8 (true): a font's record (AX)
    /// made current. By default as the original does it
    /// (`Machine::font_selected`).
    fn font(&mut self, m: &mut Machine, ax: u16, text: bool) {
        m.font_selected(ax, text);
    }
    /// 23F1:0145: the automap written (by default `Machine::automap_write`).
    fn automap_write(&mut self, m: &mut Machine, files: &mut Files) {
        m.automap_write(files);
    }
    /// 0000:5C86: EMS bank `bank` mapped (by default `Machine::ems_map`).
    fn map_bank(&mut self, m: &mut Machine, bank: u16) {
        m.ems_map(bank);
    }
    /// 0000:7428 then 5828: the picture named at DS:`name` (OP.IMG) loaded
    /// to [902E] and drawn (by default `Machine::options_picture`).
    fn picture(&mut self, m: &mut Machine, files: &mut Files, _name: u16) {
        m.options_picture(files);
    }
    /// 0000:9A6E: a box (x `ax`, y `bx`, `cx` wide, `dx` high) filled in
    /// colour 9288. 0000:5364: the text at DS:`si` centred on x `ax` at y
    /// `bx`. Drawing only.
    fn fill(&mut self, _m: &mut Machine, _ax: u16, _bx: u16, _cx: u16, _dx: u16) {}
    fn centred(&mut self, _m: &mut Machine, _si: u16, _ax: u16, _bx: u16) {}
    /// 0000:541E (the mouse hidden), 5454 (shown), 9466 (the screen shown).
    fn hide(&mut self, _m: &mut Machine) {}
    fn unhide(&mut self, _m: &mut Machine) {}
    fn show(&mut self, _m: &mut Machine) {}
    /// 1194:564D: a click's x (AX) and y (BX), as the mouse's (MOUSE_X,
    /// MOUSE_Y) stand at it; FFFFh for Esc. None: none comes (the front
    /// end left the screen), which the port takes as Resume.
    fn click(&mut self, _m: &mut Machine) -> Option<(u16, u16)> {
        None
    }
    /// 2F26:04E2 with AL 20h: the CyberMan's button 20h down (the carry).
    fn cyber(&mut self, _m: &mut Machine) -> bool {
        false
    }
    /// 25FD:0242 and 0002: the load and the save menus (`Machine::
    /// load_menu`, `save_menu` with the front end's `MenuHooks`). Their
    /// outcome doesn't change the screen: it closes after either.
    fn load_menu(&mut self, _m: &mut Machine, _files: &mut Files) {}
    fn save_menu(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 1194:48DF: "Are you sure ?" asked (1194:020E with SURE, its keys
    /// and entries); the pick (0 yes, 1 no, FFFFh Esc).
    fn confirm(&mut self, _m: &mut Machine) -> u16 {
        1
    }
    /// 1194:4D91: the files matching the pattern at DS:`si` deleted (by
    /// default `Machine::delete_matching`).
    fn delete_matching(&mut self, m: &mut Machine, files: &mut Files, si: u16) {
        m.delete_matching(files, si);
    }
    /// 0FFD:0DE8: the music driver's volume set to `volume` (with the
    /// driver's words at DRIVER_A and DRIVER_B, and 0, the call's other
    /// arguments; by default kept with a music driver's replay,
    /// `Machine::music_volume`).
    fn music_volume(&mut self, m: &mut Machine, volume: u16) {
        m.music_volume(volume);
    }
}

/// `OptionsHooks` that leave the screen at once (Resume).
pub struct NoOptionsHooks;

impl OptionsHooks for NoOptionsHooks {}

/// How the options screen ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsEnd {
    /// Resume: back to the game.
    Resume,
    /// New (2542:0627): the area left (C398 bit 80h) with 5CA bit 8000h
    /// for the outer loop, the working slot's log and files deleted.
    NewGame,
    /// The load or the save menu ran (`OptionsHooks::load_menu`,
    /// `save_menu`).
    Load,
    Save,
    /// Exit confirmed (2542:05DB): 21EE:01ED shuts the program down (it
    /// doesn't come back).
    Quit,
}

impl Machine {
    fn opt_or16(&mut self, at: u16, v: u16) {
        let x = self.mem.r16(DS, at) | v;
        self.mem.w16(DS, at, x);
    }

    /// 2542:0627, New: FCE 0, the area left (C398 bit 80h) with 5CA bit
    /// 8000h for the outer loop, the working slot's log and files deleted
    /// (1194:4D91 on each pattern).
    pub fn options_new(&mut self, files: &mut Files, hooks: &mut dyn OptionsHooks) {
        self.mem.w16(DS, 0x0FCE, 0);
        self.opt_or16(crate::movement::var::MAP_FLAGS, 0x80);
        self.opt_or16(0x05CA, 0x8000);
        for si in [LOG_WORKING, RE_WORKING, IN_WORKING] {
            hooks.delete_matching(self, files, si);
        }
    }

    /// 2542:000A. The options screen; see the module's notes. None where
    /// the original returns into its pushed registers (the CyberMan's
    /// button) or a handler isn't one of the table's.
    pub fn options_screen(&mut self, files: &mut Files, hooks: &mut dyn OptionsHooks) -> Option<OptionsEnd> {
        let ax = self.mem.r16(DS, 0x102C);
        hooks.font(self, ax, false);
        hooks.automap_write(self, files);
        let bank = self.mem.r16(DS, 0x1030);
        hooks.map_bank(self, bank);
        hooks.picture(self, files, PICTURE);
        let v = self.mem.r8(DS, SOUND) as u16;
        self.opt_box(hooks, v, SOUND_BOX);
        let v = self.mem.r8(DS, MUSIC) as u16;
        self.opt_box(hooks, v, MUSIC_BOX);
        self.opt_detail_bar(hooks);
        let end = 'wait: loop {
            let Some((ax, bx)) = hooks.click(self) else {
                break 'wait OptionsEnd::Resume;
            };
            let f = self.mem.r16(DS, 0x05C8) & !2;
            self.mem.w16(DS, 0x05C8, f);
            if hooks.cyber(self) {
                return crate::giveup::original("2542:000A: the CyberMan's button: the original returns into its pushed registers");
            }
            let mut si = CLICKS;
            loop {
                let x0 = self.mem.r16(DS, si);
                if x0 == 0xFFFF {
                    continue 'wait;
                }
                let r = |o: u16| self.mem.r16(DS, si.wrapping_add(o)) as i16;
                let (x, y) = (ax as i16, bx as i16);
                if x < x0 as i16 || x > r(4) || y < r(2) || y > r(6) {
                    si = si.wrapping_add(10);
                    continue;
                }
                let handler = self.mem.r16(DS, si.wrapping_add(8));
                if handler == 0xFFFF {
                    break 'wait OptionsEnd::Resume;
                }
                hooks.hide(self);
                let ended = match handler {
                    0x0627 => {
                        self.options_new(files, hooks);
                        Some(OptionsEnd::NewGame)
                    }
                    0x061E => {
                        hooks.load_menu(self, files);
                        Some(OptionsEnd::Load)
                    }
                    0x0617 => {
                        hooks.save_menu(self, files);
                        Some(OptionsEnd::Save)
                    }
                    0x05DB => {
                        let ax = self.mem.r16(DS, 0x1026);
                        hooks.font(self, ax, false);
                        let ax = self.mem.r16(DS, 0x1036);
                        hooks.font(self, ax, true);
                        self.mem.w8(DS, COLOUR, 0xC0);
                        if hooks.confirm(self) == 0 {
                            return Some(OptionsEnd::Quit);
                        }
                        None
                    }
                    0x0785 | 0x07C6 | 0x079C | 0x07DE => {
                        let (at, at_box) = if matches!(handler, 0x0785 | 0x07C6) { (SOUND, SOUND_BOX) } else { (MUSIC, MUSIC_BOX) };
                        let v = self.mem.r8(DS, at);
                        let up = matches!(handler, 0x0785 | 0x079C);
                        let moved = if up { v < 0x64 } else { v > 0 };
                        if moved {
                            let v = if up { v + 1 } else { v - 1 };
                            self.mem.w8(DS, at, v);
                            self.opt_box(hooks, v as u16, at_box);
                            if at == MUSIC {
                                hooks.music_volume(self, v as u16);
                            }
                        }
                        None
                    }
                    0x0653 => {
                        // The mouse's x less the bar's left, `div cl` by 5:
                        // a quotient past FFh faults.
                        let d = self.mem.r16(DS, MOUSE_X).wrapping_sub(self.mem.r16(DS, DETAIL_LEFT));
                        if d / 5 > 0xFF {
                            return crate::giveup::original("2542:0653: the detail bar's div cl by 5 overflows");
                        }
                        self.mem.w16(DS, DETAIL, d / 5 + 6);
                        self.opt_detail_bar(hooks);
                        None
                    }
                    _ => return crate::giveup::not_ported("2542:000A: an options handler not in the table"),
                };
                match ended {
                    Some(e) => break 'wait e,
                    None => {
                        hooks.unhide(self);
                        continue 'wait;
                    }
                }
            }
        };
        hooks.hide(self);
        hooks.show(self);
        let f = self.mem.r16(DS, 0xA902) & !1;
        self.mem.w16(DS, 0xA902, f);
        Some(end)
    }

    /// 2542:0020..0030: OP.IMG loaded to [902E] (0000:7428) and drawn
    /// into the page there (5828) where the page is the heap's block.
    pub fn options_picture(&mut self, files: &mut Files) {
        let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        if self.file_load(files, PICTURE, seg, off).is_some() && self.page_is_block() {
            self.page_image(seg, off);
        }
    }

    /// 2542:0808. A volume's box at DS:`si` (x0, y0, x1, y1) filled in
    /// colour 70h, `v` written in decimal at NUMBER (0000:7052) and
    /// centred over the box in colour A5h.
    fn opt_box(&mut self, hooks: &mut dyn OptionsHooks, v: u16, si: u16) {
        let r = |o: u16| self.mem.r16(DS, si + o);
        let (x0, y0, x1, y1) = (r(0), r(2), r(4), r(6));
        self.mem.w8(DS, COLOUR, 0x70);
        hooks.fill(self, x0, y0, x1.wrapping_sub(x0).wrapping_add(1), y1.wrapping_sub(y0).wrapping_add(1));
        self.sm_itoa(v, NUMBER);
        self.mem.w8(DS, COLOUR, 0xA5);
        hooks.centred(self, NUMBER, x1.wrapping_add(x0) >> 1, y0.wrapping_add(1));
    }

    /// 2542:0848. The detail bar: its track (CBh, 5Eh, 64h wide, 3 high)
    /// in colour 70h, then (D37 - 6) x 5 of it in A5h.
    fn opt_detail_bar(&mut self, hooks: &mut dyn OptionsHooks) {
        self.mem.w8(DS, COLOUR, 0x70);
        hooks.fill(self, 0xCB, 0x5E, 0x64, 3);
        self.mem.w8(DS, COLOUR, 0xA5);
        let cx = self.mem.r16(DS, DETAIL).wrapping_sub(6).wrapping_mul(5);
        hooks.fill(self, 0xCB, 0x5E, cx, 3);
    }
}
