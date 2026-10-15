//! The automap screen's squares, as the original puts them in its page
//! before the screen is shown (0000:2F20..2F47, in its redraw 2F10).
//!
//! 60 x 42 squares of 3 x 3 pixels from page offset 3219h, row by row,
//! the square under column k of a row the level's (left + 3Bh - k): the
//! map is drawn with x falling to the right (2F6D; the columns counted
//! down to `left` by a signed compare, so a left near 7FFFh ends a row
//! after one square, and a left of 8000h never ends: `automap_centre`'s
//! clamp keeps play from it). A square's colour comes from 303C (in the
//! wilderness, C398 bit 4 with C39A bit 200h, from 3161): the walls plane
//! (segment D806), the floors plane (CCD0), and how well it was seen (the
//! automap's bytes at GS = [9030], the EMS frame, one a square). The
//! player's square is an arrow by the heading (23F1:0219). Outside the
//! wilderness a second pass (A867 1) draws each square again, blending
//! its pixels with its neighbours' (23F1:02D1, 01E1) unless it is 4Ah.
//!
//! The screen's set-up (2CFE..2D17, `automap_screen_open`) keeps the
//! stairs' and the exit's wall numbers (C932, C933) for the squares (A866,
//! A865).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

pub mod var {
    /// The second pass (2F6D run again: blended).
    pub const PASS: u16 = 0xA867;
    /// 303C's: the square's wall word was positive; its high byte.
    pub const POSITIVE: u16 = 0xA862;
    pub const WALL_HIGH: u16 = 0xA863;
    /// The exit's and the stairs' wall numbers, kept at the screen's
    /// set-up (C933, C932).
    pub const EXIT: u16 = 0xA865;
    pub const STAIRS: u16 = 0xA866;
    /// The page the squares go in (its segment), and the seen bytes'.
    pub const PAGE: u16 = 0xA900;
    pub const SEEN: u16 = 0x9030;
    /// The note flags (0000:31FE's), the screen's copy of BB0A.
    pub const NOTE_FLAGS: u16 = 0xA888;
    pub const A889: u16 = 0xA889;
}
use var::*;

const WIDTH: u16 = 0xC357;
const DEPTH: u16 = 0xC359;
const PLAYER_X: u16 = 0xCCA4;
const PLAYER_Y: u16 = 0xCCA6;
const WALLS: u16 = 0xD806;
const FLOORS: u16 = 0xCCD0;
const SOLID: u16 = 0xCCCE;
const TRIGGERS: u16 = 0xC917;
const HEADING: u16 = 0xC3DA;

/// A colour shaded by how well its square was seen (`dh`, 15h less the
/// seen byte), None (the carry) where it leaves its block of 64 (a signed
/// compare, 30E8..30FB).
fn shade(c: u8, dh: u8) -> Option<u8> {
    let a = c.wrapping_sub(dh);
    ((a as i8) >= ((c & 0xC0) as i8)).then_some(a)
}

/// 23F1:01E1: the pixel at `*di` from the square's colour `dh` and the
/// neighbour's `dl`: their mean where the neighbour is 30h..7Fh (signed),
/// not the player's 96h, F4h or more less F4h not negative (a signed
/// test) and of the same block of 32 (01F0's `cmp dh, 80h; jge` is
/// always taken); else the square's own. `di` moves on; the carry: not
/// blended.
fn blend(p: &mut [u8], di: &mut u16, dh: u8, dl: u8) -> bool {
    let near = (dl as i8) >= 0x30 && dl != 0x96 && (dl.wrapping_sub(0xF4) as i8) >= 0;
    let (v, carry) = if near && dh >> 5 == dl >> 5 { (dl.wrapping_add(dh) >> 1, false) } else { (dh, true) };
    p[*di as usize] = v;
    *di = di.wrapping_add(1);
    carry
}

impl Machine {
    /// 0000:2CFE..2D17 and 2D1F..2D2E: the automap screen opened: the
    /// stairs' and the exit's numbers kept, the note being edited
    /// forgotten (A888 bit 2), the map written (23F1:0145), BB0A kept, and
    /// the view centred on the player (`automap_view_open`). (0000:A69E,
    /// the screen saved, is the front end's.)
    pub fn automap_screen_open(&mut self, files: &mut Files) {
        let v = self.mem.r8(DS, 0xC932);
        self.mem.w8(DS, STAIRS, v);
        let v = self.mem.r8(DS, 0xC933);
        self.mem.w8(DS, EXIT, v);
        let f = self.mem.r8(DS, NOTE_FLAGS) & !2;
        self.mem.w8(DS, NOTE_FLAGS, f);
        self.automap_write(files);
        let v = self.mem.r8(DS, 0xBB0A);
        self.mem.w8(DS, A889, v);
        self.automap_view_open();
    }

    /// 0000:2F20..2F47: the squares put in `page` (the segment at A900,
    /// 64K), the seen bytes read by `seen` (GS:offset, GS = [9030]).
    pub fn automap_squares(&mut self, seen: &dyn Fn(u16) -> u8, page: &mut [u8]) {
        assert!(page.len() >= 0x10000);
        self.mem.w8(DS, PASS, 0);
        self.automap_pass(seen, page);
        if !self.automap_wild() {
            let v = self.mem.r8(DS, PASS).wrapping_add(1);
            self.mem.w8(DS, PASS, v);
            self.automap_pass(seen, page);
        }
    }

    fn automap_wild(&self) -> bool {
        self.mem.r16(DS, 0xC398) & 4 != 0 && self.mem.r16(DS, 0xC39A) & 0x200 != 0
    }

    /// 0000:2F6D: one pass over the 42 rows.
    fn automap_pass(&mut self, seen: &dyn Fn(u16) -> u8, page: &mut [u8]) {
        let left = self.mem.r16(DS, crate::automap::var::MAP_LEFT);
        let mut row = self.mem.r16(DS, crate::automap::var::MAP_TOP);
        let mut start = 0x3219u16;
        for _ in 0..0x2A {
            let mut col = left.wrapping_add(0x3B);
            let mut di = start;
            loop {
                self.automap_square(seen, page, col, row, di);
                di = di.wrapping_add(3);
                col = col.wrapping_sub(1);
                if (col as i16) < (left as i16) {
                    break;
                }
            }
            start = start.wrapping_add(0x3C0);
            row = row.wrapping_add(1);
        }
    }

    /// 0000:2F9E: the square (`col`, `row`) at `di`.
    fn automap_square(&mut self, seen: &dyn Fn(u16) -> u8, page: &mut [u8], col: u16, row: u16, di: u16) {
        let wild = self.automap_wild();
        let c = if wild { self.automap_wild_colour(col, row) } else { self.automap_colour(seen, col, row) };
        let Some(c) = c else { return };
        if c == 0x96 {
            let dir = (self.mem.r16(DS, HEADING).wrapping_add(0x20) & 0x1FF) >> 6;
            return arrow(page, di, dir as u8);
        }
        if wild || c == 0x4A || self.mem.r8(DS, PASS) == 0 {
            for y in 0..3u16 {
                for x in 0..3u16 {
                    page[di.wrapping_add(y * 0x140 + x) as usize] = c;
                }
            }
        } else {
            smooth(page, di, c);
        }
    }

    /// The square's word offset in the planes (bx after 3066..306A: the
    /// row's low byte, then the column doubled in 8 bits).
    fn plane_at(col: u16, row: u16) -> u16 {
        (row << 8) | (col as u8).wrapping_mul(2) as u16
    }

    /// 0000:303C: a square's colour in a city, a dungeon or an interior;
    /// None where nothing is drawn (the carry).
    fn automap_colour(&mut self, seen: &dyn Fn(u16) -> u8, col: u16, row: u16) -> Option<u8> {
        if (col as i16) < 0 || (row as i16) < 0 || col >= self.mem.r16(DS, WIDTH) || row >= self.mem.r16(DS, DEPTH) {
            return None;
        }
        if col == self.mem.r16(DS, PLAYER_X) && row == self.mem.r16(DS, PLAYER_Y) {
            return Some(0x96);
        }
        let bx = Self::plane_at(col, row);
        let cl = seen(bx >> 1);
        if cl == 0 {
            return None;
        }
        let dh = 0x15u8.wrapping_sub(cl);
        let walls = self.mem.r16(DS, WALLS);
        let v = self.mem.r16(walls, bx);
        let positive = (v as i16) > 0;
        self.mem.w8(DS, POSITIVE, positive as u8);
        let [lo, hi] = v.to_le_bytes();
        if (v as i16) >= 0 && hi != lo {
            return shade(0xF4, dh);
        }
        if (v as i16) < 0 {
            self.mem.w8(DS, WALL_HIGH, hi);
            match v >> 12 {
                8 => {}
                9 if hi & 1 != 0 => {
                    self.mem.w8(DS, WALL_HIGH, hi & 1);
                    return None;
                }
                9 => {
                    self.mem.w8(DS, WALL_HIGH, 0);
                    return Some(0x3Fu8.wrapping_sub(cl >> 1));
                }
                0xA => return Some(0x3Fu8.wrapping_sub(cl >> 1)),
                0xB => return Some(0x4A),
                _ => return None,
            }
        }
        // 30D3: the floors plane.
        let f = self.mem.r16(self.mem.r16(DS, FLOORS), bx);
        let solid = self.mem.r16(DS, SOLID);
        if f < solid {
            if !positive {
                return None;
            }
            // 3825: the exit, the stairs and the trigger walls.
            let al = (v & 0x3F) as u8;
            if al == self.mem.r8(DS, EXIT) {
                return Some(0x97);
            }
            if al == self.mem.r8(DS, STAIRS) {
                return Some(0x95);
            }
            if (0..0x10).any(|i| self.mem.r8(DS, TRIGGERS + i) == al) {
                return Some(0x4A);
            }
            return Some(0x3Fu8.wrapping_sub(cl >> 1));
        }
        if f == solid {
            return shade(if positive { 0xF4 } else { 0xD4 }, dh);
        }
        shade(if f == solid.wrapping_add(0x100) { 0xB4 } else { 0x94 }, dh)
    }

    /// 0000:3161: a square's colour in the wilderness (all of it shown).
    /// Its floor is compared with the seen bytes' segment (9030) and 100h.
    fn automap_wild_colour(&self, col: u16, row: u16) -> Option<u8> {
        if (col as i16) < 0 || (row as i16) < 0 || col >= self.mem.r16(DS, WIDTH) || row >= self.mem.r16(DS, DEPTH) {
            return None;
        }
        if col == self.mem.r16(DS, PLAYER_X) && row == self.mem.r16(DS, PLAYER_Y) {
            return Some(0x96);
        }
        let bx = Self::plane_at(col, row);
        let v = self.mem.r16(self.mem.r16(DS, WALLS), bx);
        if (v as i16) > 0 {
            let al = (v & 0x3F) as u8;
            let water = [0xC918, 0xC91C, 0xC91F, 0xC920].iter().any(|&a| self.mem.r8(DS, a) == al);
            return Some(if water { 0x94 } else { 0x30 });
        }
        if (v as i16) < 0 && v >> 12 == 8 && (v & 0x3F) as u8 == self.mem.r8(DS, 0xC943) {
            return Some(0x94);
        }
        let f = self.mem.r16(self.mem.r16(DS, FLOORS), bx);
        if f == self.mem.r16(DS, SEEN).wrapping_add(0x100) {
            return Some(0x30);
        }
        (f >= self.mem.r16(DS, SOLID)).then_some(0xB4)
    }
}

/// 23F1:0219: the player's arrow, the heading's eighth `dir`, colour 96h,
/// at the square whose top left is `di`.
fn arrow(p: &mut [u8], di: u16, dir: u8) {
    let at: &[u16] = match dir {
        0 => &[0x140, 0x281, 0x142],
        1 => &[0, 0x140, 0x280, 0x281, 0x282],
        2 => &[1, 0x140, 0x281],
        3 => &[1, 2, 0, 0x140, 0x280],
        4 => &[1, 0x140, 0x142],
        5 => &[0, 1, 2, 0x282, 0x142],
        6 => &[1, 0x142, 0x281],
        _ => &[2, 0x142, 0x280, 0x281, 0x282],
    };
    for &o in at {
        p[di.wrapping_add(o) as usize] = 0x96;
    }
}

/// 23F1:02D1: a square's 3 x 3 pixels blended with its neighbours', in
/// the original's order (a read may see a pixel just written).
fn smooth(p: &mut [u8], top_left: u16, al: u8) {
    let step = |p: &mut [u8], di: &mut u16, off: u16| {
        let dl = p[di.wrapping_add(off) as usize];
        blend(p, di, al, dl)
    };
    const UP: u16 = 0u16.wrapping_sub(0x27F);
    let mut di = top_left;
    if step(p, &mut di, 0x13D) {
        di = di.wrapping_sub(1);
        step(p, &mut di, UP);
    }
    step(p, &mut di, UP);
    if step(p, &mut di, 0x144) {
        di = di.wrapping_sub(1);
        step(p, &mut di, 0x501);
    }
    di = top_left.wrapping_add(0x140);
    step(p, &mut di, 0x13D);
    p[di as usize] = al;
    di = di.wrapping_add(1);
    step(p, &mut di, 0x144);
    di = top_left.wrapping_add(0x280);
    if step(p, &mut di, 0x13D) {
        di = di.wrapping_sub(1);
        step(p, &mut di, 0x501);
    }
    step(p, &mut di, 0x501);
    if step(p, &mut di, 0x144) {
        di = di.wrapping_sub(1);
        step(p, &mut di, 0x501);
    }
}
