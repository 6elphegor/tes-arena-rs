//! A riddle's answer (292B:05A2), asked by the level's text while the
//! riddle waits (0000:0550, 7C5E): the answer typed under the text
//! (`fields::riddle`, 0000:9EE5), then looked for among the riddle's
//! answers (each after a `:` in the riddle's text, to a CR or NUL; the
//! last ends at a `` ` `` or the text's NUL), letters compared as capitals
//! (0000:9C2C). The text shown next is the riddle's for a right answer
//! (7C66) or a wrong one (7C64), and the level's triggers (C624, C6E4 of
//! them, 3 bytes each: the square, the trigger's number) with the right
//! answer's number (7C68) or the wrong one's (7C6A) open their walls (bit
//! 200h, as a door opening) with the sound of the wall's kind; a right
//! answer also clears the square's entry ([3D44] FFFFh), unless the level
//! has no triggers.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The answer typed (1Ah characters at most).
    pub const TYPED: u16 = 0x1044;
    /// An answer of the riddle's, copied out to be compared.
    pub const ANSWER: u16 = 0x1064;
    /// The riddle's answers in the level's text (the `:` before the
    /// first; in the EMS frame, [9030]).
    pub const ANSWERS: u16 = 0x7C62;
    /// The texts shown after a wrong answer and a right one.
    pub const WRONG_TEXT: u16 = 0x7C64;
    pub const RIGHT_TEXT: u16 = 0x7C66;
    /// The triggers' numbers a right answer and a wrong one open.
    pub const RIGHT_TRIGGER: u16 = 0x7C68;
    pub const WRONG_TRIGGER: u16 = 0x7C6A;
    /// The level's triggers and their count.
    pub const TRIGGERS: u16 = 0xC624;
    pub const TRIGGER_COUNT: u16 = 0xC6E4;
    /// The sounds of the walls' kinds (the wall's low byte's top two
    /// bits).
    pub const WALL_SOUNDS: u16 = 0xC3C0;
    /// The row above which the riddle's text ends (the field 0Ah under).
    pub const TEXT_Y: u16 = 0xADB8;
}
use var::*;

/// The walls' map (its segment's word).
const WALLS: u16 = 0xD806;
/// The EMS frame's segment.
const FRAME: u16 = 0x9030;

impl Machine {
    /// 292B:05A2 after its field: the answer typed at TYPED looked for
    /// among the riddle's (`riddle` module). The level's bank mapped
    /// (C8FE, 0000:5C86) for its text. None where a sound's volume faults
    /// (`sound_at`).
    pub fn riddle_answered(&mut self) -> Option<()> {
        let bank = self.mem.r8(DS, 0xC8FE) as u16;
        self.ems_map(bank);
        let seg = self.mem.r16(DS, FRAME);
        let mut di = self.mem.r16(DS, ANSWERS);
        let right = loop {
            // 05DD: past the `:`, the answer copied out to its CR or NUL.
            di = di.wrapping_add(1);
            let start = di;
            let mut si = ANSWER;
            loop {
                let c = self.mem.r8(seg, di);
                di = di.wrapping_add(1);
                if c == 0x0D || c == 0 {
                    self.mem.w8(DS, si, 0);
                    break;
                }
                self.mem.w8(DS, si, c);
                si = si.wrapping_add(1);
            }
            if text_equal(self, ANSWER, TYPED) {
                break true;
            }
            // 060D: on from the answer's start to the next `:`, or the
            // end (a backquote or NUL).
            di = start;
            loop {
                let c = self.mem.r8(seg, di);
                if c == 0x60 || c == 0 {
                    break;
                }
                if c == b':' {
                    break;
                }
                di = di.wrapping_add(1);
            }
            if self.mem.r8(seg, di) != b':' {
                break false;
            }
        };
        let (text, trigger) = if right { (RIGHT_TEXT, RIGHT_TRIGGER) } else { (WRONG_TEXT, WRONG_TRIGGER) };
        let v = self.mem.r16(DS, text);
        self.mem.w16(DS, crate::squares::var::SHOWN, v);
        let n = self.mem.r8(DS, TRIGGER_COUNT) as u16;
        if n == 0 {
            return Some(());
        }
        let number = self.mem.r8(DS, trigger);
        let walls = self.mem.r16(DS, WALLS);
        for k in 0..n {
            let at = TRIGGERS + k * 3;
            if self.mem.r8(DS, at + 2) != number {
                continue;
            }
            // The square (row << 8 | column) to the wall's offset: the low
            // byte doubled in 8 bits.
            let sq = self.mem.r16(DS, at);
            let off = (sq & 0xFF00) | ((sq as u8).wrapping_add(sq as u8) as u16);
            let w = self.mem.r16(walls, off) | 0x200;
            self.mem.w16(walls, off, w);
            let sound = self.mem.r8(DS, WALL_SOUNDS + ((w as u8 >> 6) & 3) as u16) as u16;
            // CX the loop's count as 23BA:002A finds it.
            self.sound_at(sound, n - k)?;
            self.mem.w16(DS, at, 0xFFFF);
        }
        if right {
            let si = self.mem.r16(DS, crate::squares::var::ENTRY);
            self.mem.w16(DS, si, 0xFFFF);
        }
        Some(())
    }
}

/// 0000:9C2C: the texts at DS:`a` and DS:`b` equal, letters a..z compared
/// as A..Z.
fn text_equal(m: &Machine, mut a: u16, mut b: u16) -> bool {
    let up = |c: u8| if c.is_ascii_lowercase() { c - 0x20 } else { c };
    loop {
        let (x, y) = (up(m.mem.r8(DS, a)), up(m.mem.r8(DS, b)));
        if x != y {
            return false;
        }
        if x == 0 {
            return true;
        }
        a = a.wrapping_add(1);
        b = b.wrapping_add(1);
    }
}
