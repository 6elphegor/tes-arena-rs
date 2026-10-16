//! A main quest's dungeon shown on arriving from the travel map
//! (292B:0362, called by the map screen's end, 26F7:00CE, with AX the
//! province): its picture with the dungeon's words under it, until the
//! button at its foot is clicked.
//!
//! - The text colour FFh (9288); the province's picture's index from the
//!   bytes at 7C6C (a province past them reads on into the names), its
//!   name the index'th of the NULs' list at 7C74 (1194:559F:
//!   COLOSSUS.IMG .. MIRKWOOD.IMG).
//! - The screen faded to black (1194:55B2: the palette buffer, 300h
//!   bytes at 929E, cleared first), the font at 102E selected (0000:9AE6)
//!   and the bank at 1030 mapped (5C86); the picture loaded to [902E]
//!   (0000:7428) and drawn with its colours (5828, the front end's).
//! - DUNGEON.TXT (named at 7CEE) loaded to 929E. The index'th entry is
//!   the dungeon's: past that many '#'s and the two bytes after the last,
//!   to the next '#', a NUL put three bytes before it (over the CR). One
//!   count (4E20h) is shared by every search, as its REPNE SCASB's CX.
//! - Its lines (each ended by a CR, made a NUL, the LF skipped) drawn
//!   centred on x A0h from row 85h, 9 apart (0000:5364), the screen shown
//!   (9466) and faded in over 12 steps (5E3E) to the colours at
//!   [9030]:(the picture's length + 0Ch): the picture's own when [902E]
//!   is 0.
//! - A902 bit 0 cleared, then clicks (1194:564D) waited for until one is
//!   in (110h..13Ah, B7h..C3h); the screen faded to black again (55B2).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// What the screen leaves to the front end: its drawing and its clicks.
pub trait DungeonPicHooks {
    /// 1194:55B2: the screen faded to black (the buffer it fades to, 929E,
    /// already cleared) and cleared (55D9).
    fn fade(&mut self, _m: &mut Machine) {}
    /// 0000:5828 on [902E]: the picture just loaded there drawn with its
    /// colours.
    fn picture(&mut self, _m: &mut Machine) {}
    /// 0000:5364: the line at DS:`si` centred on x `ax` at row `bx`.
    fn line(&mut self, _m: &mut Machine, _si: u16, _ax: u16, _bx: u16) {}
    /// 0000:9466 then 5E3E: the screen shown and faded in over 12 steps
    /// to the colours at `seg`:`off`.
    fn shown(&mut self, _m: &mut Machine, _seg: u16, _off: u16) {}
    /// 1194:564D: a click waited for, the mouse's place left at 8F92 and
    /// 8F94. None: none comes (the front end left the screen), taken as a
    /// click on the button.
    fn click(&mut self, _m: &mut Machine) -> Option<()> {
        None
    }
}

/// `DungeonPicHooks` that draw nothing and click the button at once.
pub struct NoDungeonPicHooks;

impl DungeonPicHooks for NoDungeonPicHooks {}

/// The pictures' indices by province, their names, DUNGEON.TXT's name,
/// and where the text is loaded.
const INDEX: u16 = 0x7C6C;
const NAMES: u16 = 0x7C74;
const TEXT_NAME: u16 = 0x7CEE;
const TEXT: u16 = 0x929E;

impl Machine {
    /// 292B:0362 for province `ax` (`DungeonPicHooks` the drawing). A
    /// file that doesn't load is gone on from, as the original does; None
    /// only where its error hook would be called.
    pub fn dungeon_picture(&mut self, files: &mut Files, hooks: &mut dyn DungeonPicHooks, ax: u16) -> Option<()> {
        self.mem.w8(DS, 0x9288, 0xFF);
        let n = self.mem.r8(DS, INDEX.wrapping_add(ax));
        // 1194:5585: past `n` NULs, each search at most BB8h bytes.
        let mut si = NAMES;
        for _ in 0..n {
            for _ in 0..0xBB8 {
                let c = self.mem.r8(DS, si);
                si = si.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        }
        self.dungeon_pic_fade(hooks);
        self.select_font(0x102E);
        self.ems_map(self.mem.r16(DS, 0x1030));
        let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        self.file_load_or_carry(files, si, seg, off)?;
        hooks.picture(self);
        self.file_load_or_carry(files, TEXT_NAME, DS, TEXT)?;
        let mut cx: u16 = 0x4E20;
        let mut di = TEXT;
        // REPNE SCASB for '#': stops past it, or with CX run out.
        let mut scan = |m: &mut Machine, di: &mut u16| {
            while cx != 0 {
                cx -= 1;
                let c = m.mem.r8(DS, *di);
                *di = di.wrapping_add(1);
                if c == b'#' {
                    break;
                }
            }
        };
        if n != 0 {
            for _ in 0..n {
                scan(self, &mut di);
            }
            di = di.wrapping_add(2);
        }
        let mut si = di;
        scan(self, &mut di);
        self.mem.w8(DS, di.wrapping_sub(3), 0);
        let mut bx: u16 = 0x85;
        loop {
            let start = si;
            loop {
                match self.mem.r8(DS, si) {
                    0x0D => break,
                    0 => {
                        hooks.line(self, start, 0xA0, bx);
                        return self.dungeon_pic_end(hooks);
                    }
                    _ => si = si.wrapping_add(1),
                }
            }
            self.mem.w8(DS, si, 0);
            si = si.wrapping_add(2);
            hooks.line(self, start, 0xA0, bx);
            bx = bx.wrapping_add(9);
        }
    }

    fn dungeon_pic_end(&mut self, hooks: &mut dyn DungeonPicHooks) -> Option<()> {
        let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        let len = self.mem.r16(seg, off.wrapping_add(0x0A));
        hooks.shown(self, seg, len.wrapping_add(0x0C));
        let f = self.mem.r16(DS, 0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        loop {
            let clicked = hooks.click(self);
            let f = self.mem.r16(DS, 0x05C8) & !2;
            self.mem.w16(DS, 0x05C8, f);
            if clicked.is_none() {
                break;
            }
            let (x, y) = (self.mem.r16(DS, 0x8F92), self.mem.r16(DS, 0x8F94));
            if (0x110..=0x13A).contains(&x) && (0xB7..=0xC3).contains(&y) {
                break;
            }
        }
        self.dungeon_pic_fade(hooks);
        Some(())
    }

    /// 1194:55B2: the palette buffer cleared, the fade the front end's.
    fn dungeon_pic_fade(&mut self, hooks: &mut dyn DungeonPicHooks) {
        for i in 0..0x300u16 {
            self.mem.w8(DS, TEXT + i, 0);
        }
        hooks.fade(self);
    }
}
