//! The typed field (0000:9EE9, far 9EE5 and the C-called 9EC4) and the
//! keyboard under it (the key interrupt 0000:718A, its buffer read 713C
//! and flush 7112).
//!
//! The field edits a text in place where its caller puts it: AX, BX the
//! screen's x and y, DI, BP its width and height (the screen under it
//! kept in a block of DI * BP bytes from the heap's top, 9CD6, put back
//! at the end and freed, 6C36), DX the most characters it takes, DS:SI
//! the text, CX the pixels from the field's left where the player
//! clicked (the cursor meant to go on the character there, A1B9, which
//! counts the first character's widths instead; 0 at the start). Then,
//! a pass at a time (A172), it takes a key:
//!
//! | Key | Does |
//! |---|---|
//! | the right button | ends (the text as it is) |
//! | Enter (0Dh) | ends |
//! | Esc (1Bh) | with A902 bit 1000h (the save menu's), clears the bit and ends; else empties the text and goes on |
//! | Backspace (08) | the character before the cursor taken out |
//! | Del (7Fh) | the character at the cursor taken out |
//! | 0Eh (Home) / 0Fh (End) | the cursor to the start / the end |
//! | Right / Left held (AD02 / AD00, when no key is waiting) | the cursor a character on / back, and the blink restarted |
//! | 20h and up | typed at the cursor, unless the text is full or the cursor at the most |
//! | the rest (and 83h..8Eh) | nothing |
//!
//! A character typed inside the text moves the rest up a byte and is
//! put at the cursor, but nothing writes the byte after the new end: the
//! text's old NUL is moved up into the text and whatever byte followed it
//! is the text's next (kept). Backspace and Del copy the rest of the text
//! down to its NUL (A16B), wherever that is.
//!
//! The cursor (A0D9, on every eighth pass) is the character under it XOR
//! ed with the text's colour (9288): as wide as that character (a space's
//! width plus one at the end, or past 7Fh), as high as the font less a
//! row, at the text's width up to it. `Machine::edit_cursor` gives its
//! place; drawing is the front end's, as are the redraws (A0AE: the
//! screen put back, the text drawn by 9E0A) and the waits (the arrows'
//! five retraces, 6BE2; Enter let go, ACD1).
//!
//! The key interrupt keeps each key's state (ACB5 + scan: FFh down, 0 up)
//! and, for a key pressed, puts its character (the tables at A944, with
//! Shift held A9C4; the second and third keyboards' at AA44/AAC4 and
//! AB44/ABC4 by AC44) in a ring of 100 bytes (AC4C..ACAF; written at
//! AC48, read at AC4A, ACB0 counting). Its bugs are kept: a full ring
//! (AC46) still takes the key's character at the write point but doesn't
//! move on, and while it is full a key pressed sets the state of the last
//! key taken (ACB4), not its own; a key typed with the left Shift down
//! marks the right Shift down too (cleared with the left's release); a
//! byte F0h makes the next a release.

use crate::machine::Machine;
use crate::mem::DS;

pub mod var {
    /// The field's x, y.
    pub const X: u16 = 0xA836;
    pub const Y: u16 = 0xA838;
    /// The text (far pointer: offset, then segment).
    pub const TEXT: u16 = 0xA83A;
    pub const TEXT_SEG: u16 = 0xA83C;
    /// The cursor (characters from the start).
    pub const CURSOR: u16 = 0xA83E;
    /// The text's length.
    pub const LEN: u16 = 0xA840;
    /// The most characters taken.
    pub const MAX: u16 = 0xA842;
    /// The field's width and height.
    pub const W: u16 = 0xA844;
    pub const H: u16 = 0xA846;
    /// The passes counted for the cursor's blink.
    pub const BLINK: u16 = 0xA84C;
    /// The cursor's width when last drawn.
    pub const CURSOR_W: u16 = 0xA854;
    /// The screen kept under the field (far pointer: 0, the block's
    /// segment).
    pub const KEPT: u16 = 0xB7EE;
    /// Bit 1000h: Esc ends the field (and is cleared).
    pub const FLAGS: u16 = 0xA902;
    /// The font (far pointer): its height, then each character's width
    /// from the space's.
    pub const FONT: u16 = 0xB034;
    /// The rows' offsets on the screen.
    pub const ROWS: u16 = 0xA6A0;
    /// The screen's segment.
    pub const SCREEN: u16 = 0xA900;
    /// The text's colour.
    pub const COLOUR: u16 = 0x9288;
    /// The mouse's buttons (bit 2 the right).
    pub const BUTTONS: u16 = 0xAFF9;

    /// The keyboard: each key's state by scan code.
    pub const KEYS: u16 = 0xACB5;
    /// The ring of characters typed, its write and read points, its
    /// count, and whether it is full.
    pub const RING: u16 = 0xAC4C;
    pub const RING_END: u16 = 0xACB0;
    pub const WRITE: u16 = 0xAC48;
    pub const READ: u16 = 0xAC4A;
    pub const COUNT: u16 = 0xACB0;
    pub const FULL: u16 = 0xAC46;
    /// A release prefix (F0h) seen.
    pub const PREFIX: u16 = 0xACB2;
    /// The last key pressed.
    pub const LAST: u16 = 0xACB4;
    /// F12 pressed (58h).
    pub const F12: u16 = 0xADB4;
    /// Which keyboard's tables (0, 1, 2).
    pub const LAYOUT: u16 = 0xAC44;
    /// The keyboards' character tables, plain and shifted.
    pub const PLAIN: [u16; 3] = [0xA944, 0xAA44, 0xAB44];
    pub const SHIFTED: [u16; 3] = [0xA9C4, 0xAAC4, 0xABC4];
}

/// Where and what a field edits (0000:9EE9's registers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub x: u16,
    pub y: u16,
    /// The pixels from the field's left of the click (CX).
    pub click: u16,
    /// The most characters (DX).
    pub max: u16,
    /// The text: segment and offset (DS:SI).
    pub seg: u16,
    pub off: u16,
    /// The field's size (DI, BP).
    pub w: u16,
    pub h: u16,
}

/// A pass's input (A172): a key's byte, or the right button (CF).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    Key(u8),
    Button,
}

impl Machine {
    fn font_byte(&self, i: u16) -> u8 {
        let off = self.mem.r16(DS, var::FONT);
        let seg = self.mem.r16(DS, var::FONT + 2);
        self.mem.r8(seg, off.wrapping_add(i))
    }

    fn edit_text(&self) -> (u16, u16) {
        (self.mem.r16(DS, var::TEXT_SEG), self.mem.r16(DS, var::TEXT))
    }

    /// 0000:A1B9: the cursor for a click `cx` pixels in, on DGROUP:`si`
    /// (whatever the text's segment). Meant to count the characters
    /// before the click, it never moves on from the first: it counts how
    /// many of the first character's widths fit (past the text's end, as
    /// may be). A first space ends it at 0: AH is still DGROUP's
    /// segment's high byte, so its width is thousands (taken as more
    /// than any click: DGROUP sits at 3BA0h past where DOS loaded the
    /// game, and the callers' clicks are within the screen's 140h). A
    /// first character of width 0 never ends (None).
    fn edit_click(&self, mut cx: u16, si: u16) -> Option<u16> {
        let mut bx = 0u16;
        let mut ah_clear = false;
        if cx as i16 <= 0 {
            return Some(0);
        }
        let b = self.mem.r8(DS, si);
        if b == 0 {
            return Some(0);
        }
        loop {
            let w = if b == 0x20 {
                if !ah_clear {
                    return Some(bx);
                }
                self.font_byte(1).wrapping_add(1)
            } else {
                ah_clear = true;
                self.font_byte(b.wrapping_sub(0x20) as u16)
            };
            if w == 0 {
                return crate::giveup::original("0000:A1B9: a click on a character of width 0: the original counts for ever");
            }
            cx = cx.wrapping_sub(w as u16);
            if cx as i16 <= 0 {
                return Some(bx);
            }
            bx = bx.wrapping_add(1);
        }
    }

    /// 0000:A1F6: the width of the text's first `cx` characters.
    fn edit_width(&self, cx: u16) -> u16 {
        let (seg, mut si) = self.edit_text();
        let mut bx = 0u16;
        for _ in 0..cx {
            let b = self.mem.r8(seg, si);
            let w = if b == 0x20 { self.font_byte(1).wrapping_add(1) } else { self.font_byte(b.wrapping_sub(0x20) as u16) };
            bx = bx.wrapping_add(w as u16);
            si = si.wrapping_add(1);
        }
        bx
    }

    /// 0000:9EE9 to its first key: the field's registers kept, the cursor
    /// put at the click (A1B9) and the text's length taken (9CB0), the
    /// keys flushed (7112), and the screen's block taken from the heap
    /// (9CD6; the screen kept in it is the front end's). None where the
    /// heap's chain is broken.
    pub fn edit_open(&mut self, f: &Field) -> Option<()> {
        self.mem.w16(DS, var::X, f.x);
        self.mem.w16(DS, var::Y, f.y);
        self.mem.w16(DS, var::MAX, f.max);
        self.mem.w16(DS, var::TEXT, f.off);
        self.mem.w16(DS, var::TEXT_SEG, f.seg);
        self.mem.w16(DS, var::W, f.w);
        self.mem.w16(DS, var::H, f.h);
        let cursor = self.edit_click(f.click, f.off)?;
        self.mem.w16(DS, var::CURSOR, cursor);
        let mut len = 0u16;
        while self.mem.r8(f.seg, f.off.wrapping_add(len)) != 0 {
            len = len.wrapping_add(1);
            if len == 0xFFFF {
                return crate::giveup::original("0000:9CB0: a text with no NUL in its segment");
            }
        }
        self.mem.w16(DS, var::LEN, len);
        self.key_flush();
        let seg = self.heap_alloc_high(f.w.wrapping_mul(f.h))?;
        self.mem.w16(DS, var::KEPT, 0);
        self.mem.w16(DS, var::KEPT + 2, seg);
        Some(())
    }

    /// 0000:A16B: DS:`si` copied down to DS:`di` through its NUL. None
    /// where the segment holds no NUL (the original copies for ever).
    fn edit_copy(&mut self, seg: u16, mut si: u16, mut di: u16) -> Option<()> {
        for _ in 0..0x10000u32 {
            let b = self.mem.r8(seg, si);
            self.mem.w8(seg, di, b);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if b == 0 {
                return Some(());
            }
        }
        crate::giveup::original("0000:A16B: a text with no NUL in its segment, copied for ever")
    }

    /// One pass of 0000:9EE9's loop with A172's input: whether the field
    /// ended (Enter, the right button, Esc in the save menu).
    pub fn edit_key(&mut self, k: EditKey) -> Option<bool> {
        let al = match k {
            EditKey::Button => return Some(true),
            EditKey::Key(al) => al,
        };
        let cursor = self.mem.r16(DS, var::CURSOR);
        let len = self.mem.r16(DS, var::LEN);
        let (seg, off) = self.edit_text();
        match al {
            0x0D => return Some(true),
            0x1B => {
                let flags = self.mem.r16(DS, var::FLAGS);
                if flags & 0x1000 != 0 {
                    self.mem.w16(DS, var::FLAGS, flags & !0x1000);
                    return Some(true);
                }
                self.mem.w16(DS, var::LEN, 0);
                self.mem.w16(DS, var::CURSOR, 0);
                self.mem.w8(seg, off, 0);
            }
            0x08 => {
                if cursor != 0 {
                    self.mem.w16(DS, var::LEN, len.wrapping_sub(1));
                    let si = off.wrapping_add(cursor);
                    self.edit_copy(seg, si, si.wrapping_sub(1))?;
                    self.mem.w16(DS, var::CURSOR, cursor.wrapping_sub(1));
                }
            }
            0x7F => {
                if len != cursor {
                    self.mem.w16(DS, var::LEN, len.wrapping_sub(1));
                    let si = off.wrapping_add(cursor);
                    self.edit_copy(seg, si.wrapping_add(1), si)?;
                }
            }
            0x0E => self.mem.w16(DS, var::CURSOR, 0),
            0x0F => self.mem.w16(DS, var::CURSOR, len),
            0x01 => {
                if cursor != len {
                    self.mem.w16(DS, var::BLINK, 0);
                    self.mem.w16(DS, var::CURSOR, cursor.wrapping_add(1));
                }
            }
            0xFF => {
                if cursor != 0 {
                    self.mem.w16(DS, var::BLINK, 0);
                    self.mem.w16(DS, var::CURSOR, cursor.wrapping_sub(1));
                }
            }
            0x20.. => {
                let max = self.mem.r16(DS, var::MAX);
                if max == len || cursor == max {
                    return Some(false);
                }
                if cursor != len {
                    // A070: the rest moved up a byte, from its end down.
                    let top = len.wrapping_sub(1);
                    let n = top.wrapping_sub(cursor).wrapping_add(1);
                    let (mut si, mut di) = (off.wrapping_add(top), off.wrapping_add(top).wrapping_add(1));
                    for _ in 0..n {
                        let b = self.mem.r8(seg, si);
                        self.mem.w8(seg, di, b);
                        si = si.wrapping_sub(1);
                        di = di.wrapping_sub(1);
                    }
                }
                // A155: put at the cursor; at the end, a NUL after it.
                let at = off.wrapping_add(cursor);
                self.mem.w8(seg, at, al);
                self.mem.w16(DS, var::CURSOR, cursor.wrapping_add(1));
                self.mem.w16(DS, var::LEN, len.wrapping_add(1));
                if cursor == len {
                    self.mem.w8(seg, at.wrapping_add(1), 0);
                }
            }
            _ => {}
        }
        Some(false)
    }

    /// 0000:A096 on: the keys flushed (7112) and the screen's block freed
    /// (6C36; putting the screen back, A0AE, is the front end's). The
    /// text is DS:`TEXT` as the field left it.
    pub fn edit_close(&mut self) {
        self.key_flush();
        let seg = self.mem.r16(DS, var::KEPT + 2);
        self.heap_free(seg);
    }

    /// The whole field (0000:9EE9) with the passes' inputs `keys`, in
    /// turn: Some(true) when it ended (and closed), Some(false) where the
    /// keys ran out first (left open), None where the original faults or
    /// hangs.
    pub fn line_edit(&mut self, f: &Field, keys: &[EditKey]) -> Option<bool> {
        self.edit_open(f)?;
        for &k in keys {
            if self.edit_key(k)? {
                self.edit_close();
                return Some(true);
            }
        }
        Some(false)
    }

    /// 0000:A172's input for a pass, from the machine's state: the right
    /// button (AFF9 bit 2) ends; else the next key in the ring (713C;
    /// 83h..8Eh read as 0); else Right (AD02) or Left (AD00) held. None
    /// where nothing is pressed (the original waits on).
    pub fn edit_input(&mut self) -> Option<EditKey> {
        if self.mem.r8(DS, var::BUTTONS) & 2 != 0 {
            return Some(EditKey::Button);
        }
        let mut al = self.key_read();
        if al == 0 {
            if self.mem.r8(DS, var::KEYS + 0x4D) != 0 {
                al = 1;
            } else if self.mem.r8(DS, var::KEYS + 0x4B) != 0 {
                al = 0xFF;
            } else {
                return None;
            }
        }
        if (0x83..=0x8E).contains(&al) {
            al = 0;
        }
        Some(EditKey::Key(al))
    }

    /// 0000:A0D9, a pass's cursor: on every eighth pass (BLINK & 7 = 0)
    /// the cursor's place, x, y, then how many bytes across and rows down
    /// it XORs (each count as LOOP takes it: 0 is 65536), its CURSOR_W
    /// kept; the count goes on either way. Its first byte is at
    /// [ROWS + 2y] + x on the screen's segment ([SCREEN]).
    pub fn edit_cursor(&mut self) -> Option<(u16, u16, u32, u32)> {
        let blink = self.mem.r16(DS, var::BLINK);
        self.mem.w16(DS, var::BLINK, blink.wrapping_add(1));
        if blink & 7 != 0 {
            return None;
        }
        let cursor = self.mem.r16(DS, var::CURSOR);
        let (seg, off) = self.edit_text();
        let bl = self.mem.r8(seg, off.wrapping_add(cursor));
        let w = if (bl as i8) > 0x20 { self.font_byte(bl.wrapping_sub(0x20) as u16) } else { self.font_byte(1).wrapping_add(1) };
        self.mem.w16(DS, var::CURSOR_W, w as u16);
        let height = self.font_byte(0) as u16;
        let x = self.mem.r16(DS, var::X).wrapping_add(self.edit_width(cursor));
        let y = self.mem.r16(DS, var::Y);
        let count = |c: u16| if c == 0 { 0x10000 } else { c as u32 };
        Some((x, y, count(w as u16), count(height.wrapping_sub(1))))
    }

    /// 0000:A0D9's XOR, onto segment `seg` (the screen's, [SCREEN]): the
    /// cursor `edit_cursor` placed, in the text's colour.
    pub fn edit_cursor_xor(&mut self, seg: u16, at: (u16, u16, u32, u32)) {
        let (x, y, w, h) = at;
        let colour = self.mem.r8(DS, var::COLOUR);
        let mut bx = self.mem.r16(DS, var::ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
        for _ in 0..h {
            let mut p = bx;
            for _ in 0..w {
                let v = self.mem.r8(seg, p) ^ colour;
                self.mem.w8(seg, p, v);
                p = p.wrapping_add(1);
            }
            bx = bx.wrapping_add(0x140);
        }
    }

    /// 0000:7112: the keys typed and not yet read forgotten.
    pub fn key_flush(&mut self) {
        self.mem.w16(DS, var::FULL, 0);
        self.mem.w16(DS, var::COUNT, 0);
        self.mem.w16(DS, var::READ, var::RING);
        self.mem.w16(DS, var::WRITE, var::RING);
    }

    /// 0000:713C: the next character typed, or 0.
    pub fn key_read(&mut self) -> u8 {
        let n = self.mem.r16(DS, var::COUNT);
        if n == 0 {
            return 0;
        }
        let mut bx = self.mem.r16(DS, var::READ);
        let al = self.mem.r8(DS, bx);
        bx = bx.wrapping_add(1);
        if bx == var::RING_END {
            bx = var::RING;
        }
        self.mem.w16(DS, var::READ, bx);
        self.mem.w16(DS, var::COUNT, n.wrapping_sub(1));
        self.mem.w16(DS, var::FULL, 0);
        al
    }

    /// The key interrupt (0000:718A) for the byte `code` from the
    /// keyboard (port 60h): a key's state, its character in the ring.
    /// None where Ctrl, Alt and Del are all down after it (the original
    /// reboots, 72AA).
    pub fn key_event(&mut self, code: u8) -> Option<()> {
        let mut al = code;
        'state: {
            if self.mem.r16(DS, var::PREFIX) != 0 {
                al |= 0x80;
            } else if al == 0xF0 {
                self.mem.w16(DS, var::PREFIX, 0xFFFF);
                break 'state;
            }
            self.mem.w16(DS, var::PREFIX, 0);
            let (scan, down) = if al & 0x80 != 0 {
                (al & 0x7F, 0u8)
            } else if self.mem.r16(DS, var::FULL) != 0 {
                (self.mem.r8(DS, var::LAST), 0xFF)
            } else {
                self.mem.w8(DS, var::LAST, al);
                if al == 0x58 {
                    self.mem.w8(DS, var::F12, 0xFF);
                }
                let layout = self.mem.r8(DS, var::LAYOUT);
                let pick = |t: [u16; 3]| match layout {
                    1 => t[1],
                    2 => t[2],
                    _ => t[0],
                };
                let mut table = pick(var::SHIFTED);
                let shifted = if self.mem.r16(DS, var::FLAGS) & 0x20 != 0 {
                    false
                } else if self.mem.r8(DS, var::KEYS + 0x2A) != 0 {
                    self.mem.w8(DS, var::KEYS + 0x36, 0xFF);
                    true
                } else {
                    self.mem.r8(DS, var::KEYS + 0x36) != 0
                };
                if !shifted {
                    table = pick(var::PLAIN);
                }
                let c = self.mem.r8(DS, table.wrapping_add(al as u16));
                if c != 0 {
                    let mut bx = self.mem.r16(DS, var::WRITE);
                    if bx == var::RING_END {
                        bx = var::RING;
                    }
                    self.mem.w8(DS, bx, c);
                    bx = bx.wrapping_add(1);
                    if bx == self.mem.r16(DS, var::READ) {
                        self.mem.w16(DS, var::FULL, 0xFFFF);
                    } else {
                        self.mem.w16(DS, var::WRITE, bx);
                        let n = self.mem.r16(DS, var::COUNT);
                        self.mem.w16(DS, var::COUNT, n.wrapping_add(1));
                    }
                }
                (self.mem.r8(DS, var::LAST), 0xFF)
            };
            // L7279: the make's state is ACB4's (the key just taken, or
            // with the ring full the last one), a full byte as the index.
            let at = var::KEYS.wrapping_add(scan as u16);
            self.mem.w8(DS, at, down);
            if scan == 0x2A {
                self.mem.w8(DS, at + 0xC, down);
            }
        }
        if self.mem.r8(DS, var::KEYS + 0x1D) != 0 && self.mem.r8(DS, var::KEYS + 0x38) != 0 && self.mem.r8(DS, var::KEYS + 0x53) != 0 {
            return crate::giveup::original("0000:72AA: Ctrl, Alt and Del down: the original reboots");
        }
        Some(())
    }
}

/// The fields the game's callers open (their registers to 0000:9EE5),
/// for a front end that hands the field's keys to the editor where the
/// caller would call it.
pub mod fields {
    use super::Field;
    use crate::machine::Machine;
    use crate::mem::DS;

    /// The mouse's x (8F92): the clicks measured from it.
    const MOUSE_X: u16 = 0x8F92;

    /// 1194:4800: a prompt's field at (`x`, `y`), `max` characters into
    /// DS:`text` (cleared before), 5 * max + 5 pixels wide, 0Ch high,
    /// from its start (CX 0).
    pub fn prompt(x: u16, y: u16, max: u16, text: u16) -> Field {
        Field { x, y, click: 0, max, seg: DS, off: text, w: max.wrapping_mul(5).wrapping_add(5), h: 0x0C }
    }

    /// 1194:46BF: the rest's hours, 3 characters at FCh, 44h into 4980.
    pub fn rest() -> Field {
        prompt(0xFC, 0x44, 3, crate::rest::var::TYPED)
    }

    /// 1194:4F5A (4E88 with 13D3 2): the field under the dialog's
    /// `lines` lines (its CX, A872): x 22h, y 93h + 8 * the lines' count
    /// (its low byte, MUL BL), 1Eh by 0Ah, `max` characters (the caller's
    /// AX) into DS:`text`.
    pub fn dialog_at(lines: u16, max: u16, text: u16) -> Field {
        let y = ((lines as u8 as u16) * 8).wrapping_add(0x93);
        Field { x: 0x22, y, click: 0, max, seg: DS, off: text, w: 0x1E, h: 0x0A }
    }

    /// `dialog_at` with the lines 1194:4E54 counted last (0FF5).
    pub fn dialog(m: &Machine, max: u16, text: u16) -> Field {
        dialog_at(m.mem.r16(DS, crate::shops::var::LINES), max, text)
    }

    /// The characters 4E88's callers with 13D3 2 take (their AX), by
    /// the text they type into: the tavern's days (1194:BDC4, 12C6) 2,
    /// the temple's gold (C5CA, 75F1) 4, the store's repair (CB69, CBFC,
    /// 76DE) 4, the guild's potions (B133, 1044) 2, the haggle's offer
    /// (810D, 6599) 6; else 4.
    pub fn dialog_max(text: u16) -> u16 {
        match text {
            0x12C6 | 0x1044 => 2,
            0x6599 => 6,
            _ => 4,
        }
    }

    /// 1194:5C34: a prompt's field `max` characters into DS:`text` at x
    /// `x` (FFFFh: centred, A0h less half its width), y 64h, 15 * max + 5
    /// pixels wide, 0Ch high, from its start.
    pub fn named(x: u16, max: u16, text: u16) -> Field {
        let w = max.wrapping_mul(0x0F);
        let x = if x == 0xFFFF { 0xA0u16.wrapping_sub(w >> 1) } else { x };
        Field { x, y: 0x64, click: 0, max, seg: DS, off: text, w: w.wrapping_add(5), h: 0x0C }
    }

    /// 26F7:0AD5: the travel map's search, 14h characters at x 55h into
    /// 9C62.
    pub fn search() -> Field {
        named(0x55, 0x14, 0x9C62)
    }

    /// 1194:2786: creation's name, 12h characters at x 3Ch into 01AD.
    pub fn creation_name() -> Field {
        named(0x3C, 0x12, 0x01AD)
    }

    /// 1194:661B: the spellmaker's name at (45h, 30h), 83h by 9, 20h
    /// characters into DS:`text` (581A), from where the mouse is.
    pub fn spell_name(m: &Machine, text: u16) -> Field {
        let click = m.mem.r16(DS, MOUSE_X).wrapping_sub(0x45);
        Field { x: 0x45, y: 0x30, click, max: 0x20, seg: DS, off: text, w: 0x83, h: 9 }
    }

    /// 0000:3276..32A5: an automap note at (`x`, `y`) into 1044, the
    /// click `click` pixels in, `max` characters (the screen's room, at
    /// most 3Bh), as wide as the screen's room (140h less the mouse's x),
    /// 6 high.
    pub fn note(m: &Machine, x: u16, y: u16, click: u16, max: u16) -> Field {
        let w = 0x140u16.wrapping_sub(m.mem.r16(DS, MOUSE_X));
        Field { x, y, click, max, seg: DS, off: crate::mapnotes::var::TEXT, w, h: 6 }
    }

    /// 25FD:0098: the save menu's name, 2Fh characters into 1044 at the
    /// row's box (127A + 0Ah, 127C), 135h by 9.
    pub fn save_name(m: &Machine) -> Field {
        let x = m.mem.r16(DS, crate::savemenu::var::BOX_X).wrapping_add(0x0A);
        let y = m.mem.r16(DS, crate::savemenu::var::BOX_Y);
        Field { x, y, click: 0, max: 0x2F, seg: DS, off: crate::savemenu::var::TYPED, w: 0x135, h: 9 }
    }
}
