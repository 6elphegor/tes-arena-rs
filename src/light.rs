//! Light: how dark a point of the view is drawn (segment 0000).
//!
//! The view is shaded by light sources, not by distance. A point's row of
//! the shading table (0 bright .. 0Dh) is the brightest any light gives
//! it (`light_row`, 0000:B6EB): within half a light's radius 0, then a
//! row more every (radius / 2) / 13 units, beyond the radius the darkest
//! (C8EB). Distances are the game's usual approximation, the larger of
//! the two axes plus a quarter of the smaller. A light on the far side of
//! the wall being drawn (a flag byte per light at C997, set by the
//! drawing, 0000:B6AB) lights it 8 rows less (BB02).
//!
//! The lights are:
//! - the player's (C3DE: x, y, radius; the radius C3E2 is the frame's,
//!   larger under a Light spell), first of the list at C3DE (count C3DC);
//! - the level's lights near the player (`lights_near`, 0000:E12E): of
//!   the level's list (BC90, count BC8E: from the map's LITE chunk, or
//!   from its flats whose record gives a light, `flat_lights`, 0000:0810)
//!   those within C396 of the player, their radius 5/8;
//! - the moving ones (the projectiles' and spells', BBCC, count BC8C).
//!
//! Outdoors in full daylight (C398 bit 8000h) the drawing skips the
//! lights: everything is bright.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The lights near the player, the player's first (6 bytes each: x,
    /// y, radius), and their count; the flag byte of each (lit from
    /// behind the wall drawn).
    pub const NEAR: u16 = 0xC3DE;
    pub const NEAR_COUNT: u16 = 0xC3DC;
    pub const BEHIND: u16 = 0xC997;
    /// The level's lights (6 bytes each) and their count.
    pub const LEVEL: u16 = 0xBC90;
    pub const LEVEL_COUNT: u16 = 0xBC8E;
    /// The moving lights (packed from their slots at BB0C, 0000:E106)
    /// and their count.
    pub const MOVING: u16 = 0xBBCC;
    pub const MOVING_COUNT: u16 = 0xBC8C;
    /// How near a level light must be to count, and the player's place
    /// the near list was made for ((x >> 9) + (y >> 9); FFFFh none).
    pub const REACH: u16 = 0xC396;
    pub const NEAR_KEY: u16 = 0xC8ED;
    /// The rows: at or past LIMIT a point is given DARK; beyond a light's
    /// radius it is FAR. The rows a light behind the wall loses.
    pub const LIMIT: u16 = 0xC8E7;
    pub const DARK: u16 = 0xC8E9;
    pub const FAR: u16 = 0xC8EB;
    pub const BEHIND_ROWS: u16 = 0xBB02;
    /// The darkest row so far (word), and the result.
    pub const DARKEST: u16 = 0xBAFA;
    pub const ROW: u16 = 0xA846;
}
use var::*;

/// The flat records (5 bytes; +4 the light, its radius in 256 units)
/// and the level's first flat.
const FLAT_LIGHT: u16 = 0xCDA2;
const FLAT_BASE: u16 = 0xC38E;

/// `loop` with CX = 0 runs 65536 times.
fn count(n: u16) -> u32 {
    if n == 0 { 0x10000 } else { n as u32 }
}

/// The game's distance from (`ax`, `bx`) to the point at `si`: the larger
/// axis plus a quarter of the smaller (each `sub` then `neg` if signed
/// negative).
fn reach(m: &Machine, ax: u16, bx: u16, si: u16) -> u16 {
    let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
    let a = abs(ax.wrapping_sub(m.mem.r16(DS, si)));
    let b = abs(bx.wrapping_sub(m.mem.r16(DS, si.wrapping_add(2))));
    let (lo, hi) = if a > b { (b, a) } else { (a, b) };
    (lo >> 2).wrapping_add(hi)
}

impl Machine {
    /// 0000:0810. The level's lights from its flats: for each square, a
    /// flat in the floor plane (its low byte, less 1) or a wall of flats
    /// (high byte 80h; its low byte), whose record (from FLAT_BASE) has a
    /// light byte, gives a light at the square's centre, the byte times
    /// 256 its radius. The near list is to be made again (NEAR_KEY
    /// FFFFh); 7A46 is cleared.
    pub fn flat_lights(&mut self) {
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let floors = self.mem.r16(DS, crate::mif::var::FLOOR_SEG);
        let mut di = LEVEL;
        self.mem.w16(DS, LEVEL_COUNT, 0);
        let mut dx = 0x40u16;
        let mut row = 0u16;
        for _ in 0..count(self.mem.r16(DS, 0xC359)) {
            let mut si = row;
            let mut ax = 0x40u16;
            for _ in 0..count(self.mem.r16(DS, 0xC357)) {
                let fb = self.mem.r8(floors, si);
                let flat = if fb != 0 {
                    Some((fb as u16).wrapping_sub(1))
                } else if self.mem.r8(walls, si.wrapping_add(1)) == 0x80 {
                    Some(self.mem.r8(walls, si) as u16)
                } else {
                    None
                };
                if let Some(bx) = flat {
                    let bx = bx.wrapping_add(self.mem.r16(DS, FLAT_BASE));
                    let r = (self.mem.r8(DS, bx.wrapping_mul(5).wrapping_add(FLAT_LIGHT)) as u16) << 8;
                    if r != 0 {
                        self.mem.w16(DS, di, ax);
                        self.mem.w16(DS, di.wrapping_add(2), dx);
                        self.mem.w16(DS, di.wrapping_add(4), r);
                        di = di.wrapping_add(6);
                        let n = self.mem.r16(DS, LEVEL_COUNT).wrapping_add(1);
                        self.mem.w16(DS, LEVEL_COUNT, n);
                    }
                }
                si = si.wrapping_add(2);
                ax = ax.wrapping_add(0x80);
            }
            row = row.wrapping_add(0x100);
            dx = dx.wrapping_add(0x80);
        }
        self.mem.w16(DS, NEAR_KEY, 0xFFFF);
        self.mem.w16(DS, 0x7A46, 0);
    }

    /// 0000:E12E. Unless in full daylight (C398 bit 8000h), when the
    /// player has moved to another place ((x >> 9) + (y >> 9), so not
    /// every move counts), the near list is made again: the player's
    /// light, then each level light nearer than REACH, its radius 5/8
    /// (less a quarter, less an eighth).
    pub fn lights_near(&mut self) {
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 0x8000 != 0 {
            return;
        }
        let (x, y) = (self.mem.r16(DS, 0xC3D0), self.mem.r16(DS, 0xC3D4));
        let key = (x >> 9).wrapping_add(y >> 9);
        if self.mem.r16(DS, NEAR_KEY) == key {
            return;
        }
        self.mem.w16(DS, NEAR_KEY, key);
        self.mem.w16(DS, NEAR_COUNT, 1);
        let n = self.mem.r16(DS, LEVEL_COUNT);
        if n == 0 {
            return;
        }
        let mut si = LEVEL;
        let mut di = NEAR.wrapping_add(6);
        for _ in 0..n {
            let d = reach(self, x, y, si);
            if d < self.mem.r16(DS, REACH) {
                for k in 0..2 {
                    let v = self.mem.r16(DS, si.wrapping_add(2 * k));
                    self.mem.w16(DS, di.wrapping_add(2 * k), v);
                }
                let r = self.mem.r16(DS, si.wrapping_add(4));
                let q = r >> 2;
                let v = r.wrapping_sub(q).wrapping_sub(q >> 1);
                self.mem.w16(DS, di.wrapping_add(4), v);
                di = di.wrapping_add(6);
                let c = self.mem.r16(DS, NEAR_COUNT).wrapping_add(1);
                self.mem.w16(DS, NEAR_COUNT, c);
            }
            si = si.wrapping_add(6);
        }
    }

    /// 0000:E106. The moving lights' slots (32 of 6 bytes at BB0C, free
    /// with x 8000h) packed into their list at MOVING, in order. (Their
    /// count, MOVING_COUNT, is kept as lights come and go.)
    pub fn moving_lights(&mut self) {
        let mut di = MOVING;
        for i in 0..0x20u16 {
            let si = 0xBB0C + 6 * i;
            if self.mem.r16(DS, si) == 0x8000 {
                continue;
            }
            for k in 0..3 {
                let v = self.mem.r16(DS, si + 2 * k);
                self.mem.w16(DS, di + 2 * k, v);
            }
            di += 6;
        }
    }

    /// 0000:B777. The row a light of radius `r` gives at distance `d`, and
    /// whether it is beyond the light (CF).
    fn light_falloff(&self, d: u16, r: u16) -> (u16, bool) {
        let far = (self.mem.r16(DS, FAR), true);
        if d > r {
            return far;
        }
        let half = r >> 1;
        if d < half {
            return (0, false);
        }
        let step = (half / 13).max(1);
        let row = ((d - half) / step).wrapping_add(1);
        if row >= self.mem.r16(DS, LIMIT) {
            return far;
        }
        (row, false)
    }

    /// 0000:B720. The brightest row the `n` lights from `si` give (x, y),
    /// into DARKEST (kept from before), their flags from `di` on; returns
    /// it (DARK at or past LIMIT) and the next flag's place.
    fn light_list(&mut self, x: u16, y: u16, mut si: u16, mut di: u16, n: u16) -> (u16, u16) {
        for _ in 0..count(n) {
            let d = reach(self, x, y, si);
            let (mut row, _) = self.light_falloff(d, self.mem.r16(DS, si.wrapping_add(4)));
            if self.mem.r8(DS, di) != 0 {
                row = row.wrapping_add(self.mem.r16(DS, BEHIND_ROWS));
                if row >= self.mem.r16(DS, LIMIT) {
                    row = self.mem.r16(DS, DARK);
                }
            }
            di = di.wrapping_add(1);
            if self.mem.r16(DS, DARKEST) > row {
                self.mem.w16(DS, DARKEST, row);
            }
            si = si.wrapping_add(6);
        }
        let mut row = self.mem.r16(DS, DARKEST);
        if row >= self.mem.r16(DS, LIMIT) {
            row = self.mem.r16(DS, DARK);
        }
        (row, di)
    }

    /// 0000:B6EB. The row of the shading table the point (`x`, `y`) is
    /// drawn with: the brightest the near lights (from C3DE, flags from
    /// C997) and then the moving lights (BBCC) give it; the result is
    /// also at ROW. Returns the original's CF too: set when the moving
    /// lights give a darker row than the near ones (the darkest so far
    /// isn't reset between the lists, so this is when the near lists'
    /// row was past LIMIT and became DARK).
    pub fn light_row(&mut self, x: u16, y: u16) -> (u16, bool) {
        self.mem.w16(DS, DARKEST, 0xE);
        let n = self.mem.r16(DS, NEAR_COUNT);
        let (row, di) = self.light_list(x, y, NEAR, BEHIND, n);
        self.mem.w16(DS, ROW, row);
        let n = self.mem.r16(DS, MOVING_COUNT);
        if n == 0 {
            return (row, false);
        }
        let (moving, _) = self.light_list(x, y, MOVING, di, n);
        let near = self.mem.r16(DS, ROW);
        if near > moving {
            self.mem.w16(DS, ROW, moving);
            return (moving, false);
        }
        (near, near < moving)
    }
}
