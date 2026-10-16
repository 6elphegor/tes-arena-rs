//! How the renderer draws a door ajar. `Machine::door_drawn` (0000:CD66)
//! leaves the door's kind in CB38 (its low byte's bits 6-7) and how far it
//! is open in CCC4 (0 shut .. 7); then:
//!
//! - kinds 1..3 (0000:ABA4) draw the door's faces with a copy of its
//!   texture moved over (0000:BAF3): 1 slides sideways, 2 rises, 3 parts
//!   in the middle, the uncovered texels 0 (which the walls' column
//!   drawer leaves unpainted, 0000:BAC3);
//! - kind 0 (0000:D344) swings the first face drawn after the door was
//!   met (C398 bit 1000h, which that face clears) about its first corner,
//!   by (open + 1) * 16 of the 512 to a turn (0000:93BC).
//!
//! Textures are column-major: 64 columns of 64 bytes, as the banks hold
//! them.

use crate::machine::Machine;
use crate::mem::DS;

pub mod var {
    /// The door's kind (0..3) as the renderer met it.
    pub const KIND: u16 = 0xCB38;
    /// How far it is open, 0..7.
    pub const OPEN: u16 = 0xCCC4;
    /// The textures' segment.
    pub const TEXTURES: u16 = 0x9030;
    /// The far pointer the moved texture is written through.
    pub const MOVED: u16 = 0xBA54;
    /// The face being drawn: its first corner (x, y) and its other.
    pub const FACE: u16 = 0xD7AA;
    /// The sines (512 to a turn, 7FFFh the largest); cosines from B13C.
    pub const SINES: u16 = 0xB03C;
}

/// 0000:BAF3: the texture at `tex` in `seg` (indexed modulo its length,
/// as the original's 16-bit SI wraps in a 64K segment) moved over as a
/// door of `kind` `open` steps open draws it: the bytes the original
/// writes, in order. Kind 1 writes 30h zeros past the texture's end.
pub fn door_texture(seg: &[u8], tex: u16, kind: u8, open: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(0x1030);
    let mut si;
    let movsd = |out: &mut Vec<u8>, si: &mut u16, n: u16| {
        for _ in 0..n as u32 * 4 {
            out.push(seg[*si as usize % seg.len()]);
            *si = si.wrapping_add(1);
        }
    };
    let stosd = |out: &mut Vec<u8>, n: u16| out.extend(std::iter::repeat_n(0, n as usize * 4));
    match kind {
        1 => {
            si = open << 9;
            let cx = 0x1000u16.wrapping_sub(si);
            let dx = 0x1000u16.wrapping_sub(cx);
            si = si.wrapping_add(tex);
            movsd(&mut out, &mut si, cx >> 2);
            stosd(&mut out, dx.wrapping_add(0x30) >> 2);
        }
        2 => {
            let shift = open << 3;
            let cx = 0x40u16.wrapping_sub(shift) >> 2;
            let dx = 0x10u16.wrapping_sub(cx);
            let mut row = shift.wrapping_add(tex);
            for _ in 0..0x40 {
                si = row;
                movsd(&mut out, &mut si, cx);
                stosd(&mut out, dx);
                row = row.wrapping_add(0x40);
            }
        }
        _ => {
            let s = open << 8;
            let cx = 0x800u16.wrapping_sub(s);
            let dx = 0x800u16.wrapping_sub(cx).wrapping_mul(2);
            let cx = cx >> 2;
            si = s.wrapping_add(tex);
            movsd(&mut out, &mut si, cx);
            stosd(&mut out, dx >> 2);
            si = 0x800u16.wrapping_add(tex);
            movsd(&mut out, &mut si, cx);
        }
    }
    out
}

impl Machine {
    /// 0000:BAF3 on the machine: the texture at `tex` in [9030] moved
    /// over by CB38 and CCC4, written through [BA54].
    pub fn door_texture(&mut self, tex: u16) {
        let seg: Vec<u8> = {
            let s = self.mem.r16(DS, var::TEXTURES);
            (0..=0xFFFFu16).map(|o| self.mem.r8(s, o)).collect()
        };
        let kind = self.mem.r8(DS, var::KIND);
        let open = self.mem.r16(DS, var::OPEN);
        let out = door_texture(&seg, tex, kind, open);
        let (off, dseg) = (self.mem.r16(DS, var::MOVED), self.mem.r16(DS, var::MOVED + 2));
        for (i, &b) in out.iter().enumerate() {
            self.mem.w8(dseg, off.wrapping_add(i as u16), b);
        }
    }

    /// 0000:93BC: (x, y) turned by `angle` (512 to a turn): each product
    /// of the doubled values with the table's sine or cosine its high word.
    pub fn rotate(&self, angle: u16, x: u16, y: u16) -> (u16, u16) {
        let di = angle.wrapping_add(angle);
        let (x2, y2) = (x.wrapping_add(x) as i16 as i32, y.wrapping_add(y) as i16 as i32);
        let cos = self.mem.r16(DS, di.wrapping_add(var::SINES + 0x100)) as i16 as i32;
        let sin = self.mem.r16(DS, di.wrapping_add(var::SINES)) as i16 as i32;
        // neg cx; jno; dec cx: -8000h stays 8000h, then 7FFFh.
        let nsin = if sin == -0x8000 { 0x7FFF } else { -sin };
        let hi = |v: i32| (v >> 16) as u16;
        let nx = hi(x2 * cos).wrapping_add(hi(y2 * nsin));
        let ny = hi(x2 * sin).wrapping_add(hi(y2 * cos));
        (nx, ny)
    }

    /// 0000:D344: for a kind 0 door, the face's other corner (D7B0, D7B4)
    /// turned about its first (D7AA, D7AE) by (CCC4 + 1) * 16; the top
    /// edge's (D7B6, D7BA) the same.
    pub fn door_swing(&mut self) {
        if self.mem.r8(DS, var::KIND) != 0 {
            return;
        }
        let (hx, hy) = (self.mem.r16(DS, var::FACE), self.mem.r16(DS, var::FACE + 4));
        let (fx, fy) = (self.mem.r16(DS, var::FACE + 6), self.mem.r16(DS, var::FACE + 10));
        let (x, y) = swing_end(self, (hx, hy), (fx, fy), self.mem.r16(DS, var::OPEN));
        self.mem.w16(DS, var::FACE + 6, x);
        self.mem.w16(DS, var::FACE + 10, y);
        self.mem.w16(DS, var::FACE + 12, x);
        self.mem.w16(DS, var::FACE + 16, y);
    }
}

/// A swinging door's far corner `far` turned about `hinge` as 0000:D344
/// turns it for a door `open` steps open (map units).
pub fn swing_end(m: &Machine, hinge: (u16, u16), far: (u16, u16), open: u16) -> (u16, u16) {
    let angle = open.wrapping_add(1) << 4;
    let (x, y) = m.rotate(angle, far.0.wrapping_sub(hinge.0), far.1.wrapping_sub(hinge.1));
    (x.wrapping_add(hinge.0), y.wrapping_add(hinge.1))
}

/// The four faces of a square as the renderer draws them (0000:D2C5,
/// the table at D74A with its swaps by side): each face's first corner
/// (the hinge a swinging door turns about) and its other, as offsets in
/// the square (80h a side; x the column's way, y the row's). Side 0 is
/// the edge at the square's lowest row, 1 its highest column, 2 its
/// highest row, 3 its lowest column.
pub const FACES: [((u16, u16), (u16, u16)); 4] =
    [((0, 0), (0x80, 0)), ((0x80, 0), (0x80, 0x80)), ((0x80, 0x80), (0, 0x80)), ((0, 0x80), (0, 0))];
