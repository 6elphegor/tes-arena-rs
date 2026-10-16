//! A level's trigger squares (the MIF's TRIG list: C766, four bytes an
//! entry, count C866): a square's text and sound.
//!
//! At the end of its walk over the squares (0000:C9E8) the drawing runs
//! `trigger_squares` (0000:D39B) when C398 bit 20h is set, which it sets
//! while the eye isn't below the floor (0000:A36F). On the player's square
//! an entry's text (its third byte) goes to the hook CD52 and its sound
//! (fourth) to the sound hook CD46, each once while the player stays on it.
//! The 3D mode's CD52 is `level_text` (0000:0628), which finds the text
//! by the square (0000:06BE, `square_lookup`) in the level's INF texts
//! and shows it: the text kept (3D42), its lines counted (3D40), shown
//! for 28h ticks (3C82); a text shown once (`~`) has its entry's square
//! put out (FFFFh).
//!
//! `square_lookup` is also what clicking a level's item (a `+` text)
//! asks, through 292B:0942 (`TalkHooks::square_text`).

use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::SOUND_HOOK;

/// DGROUP variables.
pub mod var {
    /// The trigger list and its count.
    pub const TRIGGERS: u16 = 0xC766;
    pub const COUNT: u16 = 0xC866;
    /// The text and the sound last started on the trigger square (FFh
    /// none).
    pub const LAST_TEXT: u16 = 0xD73D;
    pub const LAST_SOUND: u16 = 0xD73E;
    /// The square's text hook.
    pub const TEXT_HOOK: u16 = 0xCD52;
    /// The INF's texts (a far pointer) and their EMS bank.
    pub const TEXTS: u16 = 0xCB88;
    pub const TEXT_BANK: u16 = 0xC8FE;
    /// What `square_lookup` leaves: the text (7C60), a riddle's text and
    /// its numbers, answers and their texts.
    pub const TEXT_AT: u16 = 0x7C60;
    pub const RIDDLE_AT: u16 = 0x7C5E;
    pub const ANSWERS_AT: u16 = 0x7C62;
    /// The texts after a wrong answer and after the right one (the `C
    /// block's: 292B:05A2 shows it on a match).
    pub const WRONG_AT: u16 = 0x7C64;
    pub const RIGHT_AT: u16 = 0x7C66;
    pub const RIDDLE_A: u16 = 0x7C68;
    pub const RIDDLE_B: u16 = 0x7C6A;
    /// `level_text`: the entry, the text shown, its height in pixels, its
    /// time and square.
    pub const ENTRY: u16 = 0x3D44;
    pub const SHOWN: u16 = 0x3D42;
    pub const SHOWN_HEIGHT: u16 = 0x3D40;
    pub const SHOWN_TIME: u16 = 0x3C82;
    pub const SHOWN_SQUARE: u16 = 0x0DBC;
}
use var::*;

/// The renderer's column and row of the player (CCA4, CCA6) and square.
const COL: u16 = 0xCCA4;
const ROW: u16 = 0xCCA6;
const SQUARE: u16 = 0xCB90;
/// The buffer segment (the EMS frame).
const FRAME: u16 = 0x9030;

/// What 0000:06BE returns: the carry (not found), AX (the kind: 1 no
/// text, 2 a sound only, 3 a `+` number, 4 a `^` riddle, 5 a `~` text
/// shown once, 6 a text), BX and SI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub not_found: bool,
    pub ax: u16,
    pub bx: u16,
    pub si: u16,
}

/// The longest a scan for a byte may run before it is the original's
/// endless loop (all of a segment without it).
const SCAN: u32 = 0x10000;

impl Machine {
    /// 0000:D39B..D3F8. The trigger squares at the end of the drawing's
    /// walk (BX the row, CCA6, as it calls). The CD52 hook runs here when
    /// it is `level_text` (None for another set hook); Some(Some((sound,
    /// square))) for the sound hook's call, when it is set.
    pub fn trigger_squares(&mut self) -> Option<Option<(u16, u16)>> {
        let n = self.mem.r8(DS, COUNT) as u16;
        if n == 0 {
            return Some(None);
        }
        let key = (self.mem.r8(DS, ROW) as u16) << 8 | self.mem.r8(DS, COL) as u16;
        let mut si = TRIGGERS;
        for _ in 0..n {
            if self.mem.r16(DS, si) != key {
                si = si.wrapping_add(4);
                continue;
            }
            let a = self.mem.r8(DS, si.wrapping_add(2));
            if a != 0xFF && self.mem.r8(DS, LAST_TEXT) != a {
                self.mem.w8(DS, LAST_TEXT, a);
                if self.hook_set(TEXT_HOOK) {
                    if self.mem.r32(DS, TEXT_HOOK) != 0x0628 {
                        return crate::giveup::not_ported("0000:D39B: the level's text hook set to a routine not ported");
                    }
                    let bx = self.mem.r16(DS, ROW);
                    self.level_text(bx, TEXT_HOOK)?;
                }
            }
            let b = self.mem.r8(DS, si.wrapping_add(3));
            if b == 0xFF || self.mem.r8(DS, LAST_SOUND) == b {
                return Some(None);
            }
            self.mem.w8(DS, LAST_SOUND, b);
            if !self.hook_set(SOUND_HOOK) {
                return Some(None);
            }
            let w = self.mem.r16(DS, si);
            let cx = (w & 0xFF00) | ((w as u8).wrapping_add(w as u8)) as u16;
            return Some(Some((b as u16, cx)));
        }
        self.mem.w8(DS, LAST_TEXT, 0xFF);
        self.mem.w8(DS, LAST_SOUND, 0xFF);
        Some(None)
    }

    /// 0000:0628 (the hook CD52 the 3D mode sets): the level's text on the
    /// player's square shown, BX and SI as the caller left them. Through
    /// 292B:0956 (the square's column halved) `square_lookup`, whose carry
    /// isn't looked at; a `+` text of number 7Bh counts as shown once.
    /// None where the lookup's scan never ends.
    pub fn level_text(&mut self, bx: u16, si: u16) -> Option<()> {
        let sq = self.mem.r16(DS, SQUARE);
        let key = (sq & 0xFF00) | ((sq as u8) >> 1) as u16;
        let l = self.square_lookup(key, bx, si)?;
        self.mem.w16(DS, ENTRY, l.si);
        let mut ax = l.ax;
        if ax == 3 {
            if l.bx != 0x7B {
                return Some(());
            }
            ax = 5;
        }
        if ax < 4 {
            return Some(());
        }
        self.mem.w8(DS, SHOWN_TIME, 0x28);
        let at = self.mem.r16(DS, TEXT_AT);
        self.mem.w16(DS, SHOWN, at);
        if ax == 5 {
            self.mem.w16(DS, l.si, 0xFFFF);
        }
        let seg = self.mem.r16(DS, FRAME);
        let mut height = 0xAu16;
        let mut p = at;
        for _ in 0..SCAN {
            let c = self.mem.r8(seg, p);
            p = p.wrapping_add(1);
            if c == 0 {
                self.mem.w16(DS, SHOWN_HEIGHT, height);
                self.mem.w16(DS, SHOWN_SQUARE, sq);
                return Some(());
            }
            if c == 0x0D {
                height = height.wrapping_add(0xA);
                if self.mem.r8(seg, p) == b':' {
                    self.mem.w16(DS, SHOWN_HEIGHT, height);
                    self.mem.w16(DS, SHOWN_SQUARE, sq);
                    return Some(());
                }
            }
        }
        None
    }

    /// 0000:06BE. The trigger at square `key` (row high, column low) and
    /// its text in the INF's texts (the far pointer at TEXTS, its bank
    /// mapped): `not_found` when no entry has the square (AX and BX as
    /// given, SI past the list; SI as given for an empty list); otherwise
    /// the kind in AX with SI the entry, BX the `+` number, a riddle's
    /// second number's leftover (0000:52BA's), or the bank. The texts are
    /// NUL-ended records (one starting with a backtick continues the one
    /// before); a riddle's answers are cut off with a NUL. None where a
    /// scan never ends.
    pub fn square_lookup(&mut self, key: u16, bx: u16, si: u16) -> Option<Lookup> {
        let n = self.mem.r8(DS, COUNT) as u16;
        if n == 0 {
            return Some(Lookup { not_found: true, ax: key, bx, si });
        }
        let mut e = TRIGGERS;
        let mut found = false;
        for _ in 0..n {
            if self.mem.r16(DS, e) == key {
                found = true;
                break;
            }
            e = e.wrapping_add(4);
        }
        if !found {
            return Some(Lookup { not_found: true, ax: key, bx, si: e });
        }
        let w = self.mem.r16(DS, e.wrapping_add(2));
        if w == 0xFFFF {
            return Some(Lookup { not_found: false, ax: 1, bx, si: e });
        }
        if w as u8 == 0xFF {
            return Some(Lookup { not_found: false, ax: 2, bx, si: e });
        }
        let mut cx = (w & 0xFF) as u32;
        let bx = self.mem.r8(DS, TEXT_BANK) as u16;
        self.ems_map(bx);
        let mut s = self.mem.r16(DS, TEXTS);
        let seg = self.mem.r16(DS, TEXTS + 2);
        let at = |m: &Machine, o: u16| m.mem.r8(seg, o);
        if cx != 0 {
            let mut steps = 0u32;
            loop {
                while at(self, s) != 0 {
                    s = s.wrapping_add(1);
                    steps += 1;
                    if steps > SCAN {
                        return crate::giveup::original("0000:06BE: the scan never ends: the original reads on for ever");
                    }
                }
                s = s.wrapping_add(1);
                if at(self, s) == b'`' {
                    continue;
                }
                cx -= 1;
                if cx == 0 {
                    break;
                }
            }
        }
        let done = |ax: u16, bx: u16| Some(Lookup { not_found: false, ax, bx, si: e });
        match at(self, s) {
            b'+' => {
                let (v, s2, _) = atoi(self, seg, s.wrapping_add(1), bx);
                if v == 0x7B {
                    let mut p = s2.wrapping_sub(1);
                    let mut steps = 0u32;
                    while at(self, p) != 0x0D {
                        p = p.wrapping_add(1);
                        steps += 1;
                        if steps > SCAN {
                            return crate::giveup::original("0000:06BE: the scan never ends: the original reads on for ever");
                        }
                    }
                    self.mem.w16(DS, TEXT_AT, p.wrapping_add(1));
                }
                done(3, v)
            }
            b'^' => {
                let (v1, mut p, b1) = atoi(self, seg, s.wrapping_add(1), bx);
                self.mem.w16(DS, RIDDLE_A, v1);
                let mut steps = 0u32;
                while at(self, p) <= 0x20 {
                    p = p.wrapping_add(1);
                    steps += 1;
                    if steps > SCAN {
                        return crate::giveup::original("0000:06BE: the scan never ends: the original reads on for ever");
                    }
                }
                let (v2, p2, b2) = atoi(self, seg, p, b1);
                self.mem.w16(DS, RIDDLE_B, v2);
                p = p2;
                while at(self, p.wrapping_sub(1)) != 0x0D {
                    p = p.wrapping_add(1);
                    steps += 1;
                    if steps > SCAN {
                        return crate::giveup::original("0000:06BE: the scan never ends: the original reads on for ever");
                    }
                }
                self.mem.w16(DS, RIDDLE_AT, p);
                self.mem.w16(DS, TEXT_AT, p);
                // repne scasb for ':' (CX 2710h), then back one.
                let mut di = p;
                for _ in 0..0x2710 {
                    let c = at(self, di);
                    di = di.wrapping_add(1);
                    if c == b':' {
                        break;
                    }
                }
                di = di.wrapping_sub(1);
                self.mem.w16(DS, ANSWERS_AT, di);
                loop {
                    let c = at(self, di);
                    if c == 0 {
                        break;
                    }
                    if c != b'`' {
                        di = di.wrapping_add(1);
                        steps += 1;
                        if steps > SCAN {
                            return crate::giveup::original("0000:06BE: the scan never ends: the original reads on for ever");
                        }
                        continue;
                    }
                    self.mem.w8(seg, di.wrapping_sub(1), 0);
                    let to = if at(self, di.wrapping_add(1)) == b'C' { RIGHT_AT } else { WRONG_AT };
                    while at(self, di) != 0x0D {
                        di = di.wrapping_add(1);
                        steps += 1;
                        if steps > SCAN {
                            return crate::giveup::original("0000:06BE: the scan never ends: the original reads on for ever");
                        }
                    }
                    di = di.wrapping_add(1);
                    self.mem.w16(DS, to, di);
                }
                done(4, b2)
            }
            b'~' => {
                self.mem.w16(DS, TEXT_AT, s.wrapping_add(1));
                done(5, bx)
            }
            _ => {
                self.mem.w16(DS, TEXT_AT, s);
                done(6, bx)
            }
        }
    }
}

/// 0000:52BA on the buffer: the number at `s` (a sign, then digits),
/// with SI one past the character that ended it and BX twice the value
/// before its last digit (unchanged without digits).
fn atoi(m: &Machine, seg: u16, mut s: u16, mut bx: u16) -> (u16, u16, u16) {
    let mut neg = false;
    match m.mem.r8(seg, s) {
        b'-' => {
            s = s.wrapping_add(1);
            neg = true;
        }
        b'+' => s = s.wrapping_add(1),
        _ => {}
    }
    let mut dx = 0u16;
    loop {
        let c = m.mem.r8(seg, s);
        s = s.wrapping_add(1);
        if c == 0 || !c.is_ascii_digit() {
            break;
        }
        let d2 = dx.wrapping_add(dx);
        bx = d2;
        dx = d2.wrapping_mul(5).wrapping_add((c - b'0') as u16);
    }
    (if neg { dx.wrapping_neg() } else { dx }, s, bx)
}
