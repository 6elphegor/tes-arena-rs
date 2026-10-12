//! Movement and collision (segment 0000, F478..FD00).
//!
//! Positions are in map units, 128 per square; the map planes hold one
//! word per square, rows of 0x100 bytes. To test a position, the square
//! it is in and its eight neighbours are turned into a 16 x 16 mask of
//! 8-unit cells (`move_mask`): each neighbour's wall (or, low down, floor)
//! word is classified (`move_classify`) and, if it blocks, a 16 x 16
//! shape is stamped into the mask at the neighbour's offset (11 cells
//! away, or 13 in creature mode). The cell under the position decides.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The cell mask (16 x 16 bytes) and the fill the next stamp uses.
    pub const MASK: u16 = 0x929E;
    pub const FILL: u16 = 0xE3A0;
    /// Set while moving a creature rather than the player.
    pub const CREATURE_MODE: u16 = 0xE3A1;
    /// Player position: x, height, y (words), heading (0..511).
    pub const X: u16 = 0xC3D0;
    pub const HEIGHT: u16 = 0xC3D2;
    pub const Y: u16 = 0xC3D4;
    pub const HEADING: u16 = 0xC3DA;
    /// Floor words below this are solid when low down.
    pub const FLOOR_SOLID: u16 = 0xCCCE;
    /// Ceiling height, and the highest step that can be climbed.
    pub const CEILING: u16 = 0xD80C;
    pub const STEP: u16 = 0xE8C0;
    /// Map offset of the last trigger (type B) square stamped.
    pub const TRIGGER_SQUARE: u16 = 0xE8C2;
    /// Map flags (C398) and level flags (C39A).
    pub const MAP_FLAGS: u16 = 0xC398;
    pub const LEVEL_FLAGS: u16 = 0xC39A;
    /// Door table base for type 8 walls (bytes, 5 per door).
    pub const DOOR_BASE: u16 = 0xC38E;
    pub const DOORS: u16 = 0xCDA1;
    /// Hooks called on a trigger square (far pointers).
    pub const TRIGGER_HOOK: u16 = 0xCD62;
    pub const OPEN_HOOK: u16 = 0xCD46;
}
use var::*;

/// Neighbour tables: (x cells, y cells, map offset) for the 3 x 3 squares,
/// for the player and for creatures.
const NEIGHBOURS: u16 = 0xE7B8;
const NEIGHBOURS_CREATURE: u16 = 0xE7EE;
/// Shapes (16 x 16 bytes): solid, a door's, and two halves.
const SOLID: u16 = 0xE3A2;
const DOOR_SHAPE: u16 = 0xE4A2;
const HALF_A: u16 = 0xE5A2;
const HALF_B: u16 = 0xE6A2;
/// Wall heights: bottoms by the low 3 bits of the high byte (three sets
/// of 8 by map flags), tops by its high 4 bits (two sets of 16).
const BOTTOMS: u16 = 0xCB94;
const TOPS: u16 = 0xCBC4;
/// Low down means under this height.
const LOW: i16 = 0x3C;

impl Machine {
    /// 0000:F697. The mask value of the cell under the position at
    /// DS:`pos` (x, then y): 0 free, otherwise what blocks it. Walls are
    /// taken from the wall plane, or for the player low down from the
    /// floor plane. A trigger square (0xB0) met by a creature on a level
    /// with flag 2 calls the trigger hook, and if it lets it, marks the
    /// square (0x200) and calls the open hook. None where a hook is set
    /// (not ported).
    pub fn move_cell(&mut self, pos: u16) -> Option<u8> {
        let x = self.mem.r16(DS, pos);
        let y = self.mem.r16(DS, pos.wrapping_add(2));
        let di = ((y >> 7) << 8).wrapping_add((x >> 7) << 1);
        let creature = self.mem.r8(DS, CREATURE_MODE) != 0;
        let plane = if !creature && (self.mem.r16(DS, HEIGHT) as i16) < LOW {
            crate::mif::var::FLOOR_SEG
        } else {
            crate::mif::var::MAP1_SEG
        };
        let seg = self.mem.r16(DS, plane);
        self.move_mask(seg, di);
        let cell = (((y & 0x7F) >> 3) << 4) + ((x & 0x7F) >> 3);
        let al = self.mem.r8(DS, MASK + cell);
        if al == 0xB0 && creature && self.mem.r16(DS, LEVEL_FLAGS) & 2 != 0 {
            if self.mem.r16(DS, TRIGGER_HOOK + 2) != 0 || self.mem.r16(DS, OPEN_HOOK + 2) != 0 {
                return None;
            }
            let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
            let sq = self.mem.r16(DS, TRIGGER_SQUARE);
            let w = self.mem.r16(walls, sq) | 0x200;
            self.mem.w16(walls, sq, w);
            return Some(0);
        }
        Some(al)
    }

    /// 0000:FB3A. Builds the cell mask for the square at map offset `di`
    /// of the plane at `seg` and its eight neighbours.
    pub fn move_mask(&mut self, seg: u16, di: u16) {
        for i in 0..0x100 {
            self.mem.w8(DS, MASK + i, 0);
        }
        let table = if self.mem.r8(DS, CREATURE_MODE) != 0 { NEIGHBOURS_CREATURE } else { NEIGHBOURS };
        for k in 0..9u16 {
            let e = table + k * 6;
            let ax = self.mem.r16(DS, e);
            let bx = self.mem.r16(DS, e + 2);
            let at = di.wrapping_add(self.mem.r16(DS, e + 4));
            let w = self.mem.r16(seg, at);
            // CL is the loop counter (9 down to 1) unless set.
            if let Some((shape, fill)) = self.move_classify(w, at, (9 - k) as u8) {
                self.move_stamp(ax, bx, shape, fill);
            }
        }
    }

    /// 0000:FB81. Whether wall word `w` (at map offset `at`) blocks, and
    /// with which shape and fill. `cl` is the fill when none is chosen.
    fn move_classify(&mut self, w: u16, at: u16, cl: u8) -> Option<(u16, u8)> {
        let creature = self.mem.r8(DS, CREATURE_MODE) != 0;
        let height = self.mem.r16(DS, HEIGHT);
        if !creature && (height as i16) < LOW {
            // A floor word: solid below FLOOR_SOLID.
            return (w < self.mem.r16(DS, FLOOR_SOLID)).then_some((SOLID, cl));
        }
        if w == 0 {
            return None;
        }
        let (lo, hi) = (w as u8, (w >> 8) as u8);
        if w & 0x8000 == 0 {
            let flags = self.mem.r16(DS, LEVEL_FLAGS);
            if hi == lo || (flags & 0x400 != 0 && creature) {
                return Some((SOLID, lo));
            }
            let (top, bottom) = self.move_wall_height(w);
            if (bottom as i16) > (height as i16) {
                return None;
            }
            // These exits fill with AL, by now the height being compared.
            let mut ax = top;
            if flags & 0x200 == 0 {
                ax = ax.wrapping_add(0x3C);
                if ax >= self.mem.r16(DS, CEILING) {
                    return Some((SOLID, ax as u8));
                }
            }
            let rise = ax.wrapping_sub(height);
            if (rise as i16) > self.mem.r16(DS, STEP) as i16 {
                return Some((SOLID, rise as u8));
            }
            return None;
        }
        match hi >> 4 {
            8 => {
                if w & 0x200 != 0 {
                    return None;
                }
                let fill = (0x80 | lo).wrapping_add(1);
                let door = (lo as u16).wrapping_add(self.mem.r16(DS, DOOR_BASE));
                if self.mem.r8(DS, DOORS.wrapping_add(door.wrapping_mul(5))) & 1 == 0 {
                    return None;
                }
                Some((DOOR_SHAPE, fill))
            }
            0xD => Some((if w & 0x100 == 0 { HALF_A } else { HALF_B }, 0x80 | lo)),
            0xA => (w & 0x100 != 0).then_some((SOLID, lo)),
            9 => (w & 0x100 == 0).then_some((SOLID, lo)),
            0xB => {
                self.mem.w16(DS, TRIGGER_SQUARE, at);
                (w & 0xF00 == 0).then_some((SOLID, 0xB0))
            }
            _ => None,
        }
    }

    /// 0000:FCB9. A wall's top and bottom heights from its high byte.
    pub fn move_wall_height(&self, w: u16) -> (u16, u16) {
        let ah = (w >> 8) as u8;
        let mut bx = ((ah & 7) as u16) * 2;
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
            bx += 0x10;
            if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
                bx += 0x10;
            }
        }
        let bottom = self.mem.r16(DS, BOTTOMS + bx);
        let mut bx = ((ah >> 3) & 0xF) as u16 * 2;
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            bx += 0x20;
        }
        (self.mem.r16(DS, TOPS + bx).wrapping_add(bottom), bottom)
    }

    /// 0000:FC5B. Stamps `shape` into the mask, shifted by (`ax`, `bx`)
    /// cells: its set bytes become `fill`; what falls outside the mask's
    /// rows is dropped, and columns are clipped.
    fn move_stamp(&mut self, ax: u16, bx: u16, shape: u16, fill: u8) {
        self.mem.w8(DS, FILL, fill);
        let mut row = bx << 4;
        let mut src = shape;
        for _ in 0..16 {
            if (row as i16) >= 0 && (row as i16) < 0x100 {
                let mut di = MASK.wrapping_add(row);
                let mut si = src;
                let n = if ax == 0 {
                    16
                } else if (ax as i16) > 0 {
                    di = di.wrapping_add(ax);
                    0x10u16.wrapping_sub(ax)
                } else {
                    si = si.wrapping_add(ax.wrapping_neg());
                    0x10u16.wrapping_add(ax)
                };
                // `loop` with CX = 0 runs 65536 times.
                for _ in 0..if n == 0 { 0x10000u32 } else { n as u32 } {
                    if self.mem.r8(DS, si) != 0 {
                        self.mem.w8(DS, di, fill);
                    }
                    si = si.wrapping_add(1);
                    di = di.wrapping_add(1);
                }
            }
            src = src.wrapping_add(16);
            row = row.wrapping_add(16);
        }
    }
}
