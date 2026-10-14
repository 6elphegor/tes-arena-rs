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
//!
//! Outdoors each frame `outdoor_light` (0000:0390) follows the time of
//! day: by night, and through dawn and dusk, the daylight bit is clear,
//! the textures are darkened (a row of the light table, `sky_light`'s
//! level), the level's lights are made and a city's street lamps lit
//! (2294:028C); by day the lamps go out and the bit is set again
//! (0000:033D), unless in fog.

use crate::machine::Machine;
use crate::mem::DS;

/// What the day and night switch leaves to the front end.
pub trait LightHooks {
    /// 2294:0B8A's I/O: the light table (CCC2) freed, NORMAL.LGT loaded
    /// to 3AAC:0240 and the table made again (0000:AAA2). The rest of
    /// 0B8A is `light_table_reset`'s.
    fn light_table(&mut self, _m: &mut Machine) {}
    /// 0000:027A and 02F2: the sky's images (when listed) and the
    /// textures remapped through row `level >> 8` of the light table.
    /// 027A remakes the sky's objects first (2294:08B9) when there are
    /// any.
    fn relight(&mut self, _m: &mut Machine, _level: u16) {}
    /// 0000:DFDB (with BA17): the textures as loaded, put back.
    fn restore_textures(&mut self, _m: &mut Machine) {}
    /// 2294:08B9: the sky's objects made again.
    fn sky_objects(&mut self, _m: &mut Machine) {}
    /// 0000:9B5B: palette entry `index` set to the colour at DS:`at`
    /// (three 6-bit values).
    fn palette(&mut self, _m: &mut Machine, _index: u8, _at: u16) {}
    /// 2294:0341's loads and palette writes (`sky_step` decides).
    fn sky(&mut self, _m: &mut Machine, _step: crate::sky::SkyStep) {}
}

/// `LightHooks` that do nothing.
pub struct NoLightHooks;

impl LightHooks for NoLightHooks {}

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

    /// 0000:0390. Outdoors (C398 bit 4) the frame's light by the time of
    /// day (indoors only the row limits are reset: 0Eh, 0Dh, 0Eh):
    /// - first the sky's window (`sky_step`, 2294:0341);
    /// - night (before 100h, after 300h): no daylight bit, the row limits
    ///   reset, the night switch (`night_switch`); when the level isn't
    ///   0A00h already it becomes so, the lamps' colour (entry 71h) is
    ///   set, and the textures relit;
    /// - dawn (100h..10Fh) and dusk (2F1h..300h): no daylight bit, the
    ///   night switch, the level of the step (`sky_light`'s table); when
    ///   it changes, the lamps' colour from the table at 3D66 and the
    ///   textures relit, and the row limits follow the level's row r:
    ///   LIMIT r + max(0, r / 2 - 1), DARK one less (not below 0), FAR
    ///   DARK + 2;
    /// - day: level FFFFh; once (5C8 bit 200h) the day switch
    ///   (`day_switch`), the textures put back, then in fog (F81) the
    ///   lamps' colour a grey, otherwise the daylight bit and no colour.
    pub fn outdoor_light(&mut self, hooks: &mut dyn LightHooks) {
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 4 == 0 {
            self.reset_rows();
            return;
        }
        let step = self.sky_step();
        hooks.sky(self, step);
        let t = self.mem.r16(DS, crate::sky::var::TIME);
        if !(0x100..=0x300).contains(&t) {
            self.daylight(false);
            self.reset_rows();
            self.night_switch(hooks);
            if self.mem.r16(DS, crate::sky::var::LIGHT) == 0xA00 {
                return;
            }
            self.mem.w16(DS, crate::sky::var::LIGHT, 0xA00);
            hooks.relight(self, 0xA00);
            self.lamp_colour(hooks, [0x3F, 0x29, 0]);
            return;
        }
        if t < 0x110 || t > 0x2F0 {
            self.daylight(false);
            self.night_switch(hooks);
            let i = if t < 0x110 { t - 0x100 } else { 0x300 - t };
            let level = self.mem.r16(DS, crate::sky::var::LIGHT_STEPS + 2 * i);
            if level == self.mem.r16(DS, crate::sky::var::LIGHT) {
                return;
            }
            self.mem.w16(DS, crate::sky::var::LIGHT, level);
            hooks.palette(self, 0x71, 0x3D66 + 3 * i);
            hooks.relight(self, level);
            let r = level >> 8;
            let limit = r + ((r >> 1) as i16 - 1).max(0) as u16;
            self.mem.w16(DS, LIMIT, limit);
            let dark = (limit as i16 - 1).max(0) as u16;
            self.mem.w16(DS, DARK, dark);
            self.mem.w16(DS, FAR, dark + 2);
            return;
        }
        self.mem.w16(DS, crate::sky::var::LIGHT, 0xFFFF);
        let f = self.mem.r16(DS, 0x05C8);
        if f & 0x200 != 0 {
            return;
        }
        self.mem.w16(DS, 0x05C8, f | 0x200);
        self.day_switch(hooks);
        self.mem.w16(DS, 0xCD00, 0xFFFF);
        hooks.restore_textures(self);
        self.mem.w16(DS, crate::sky::var::LIGHT, 0xFFFF);
        if self.mem.r8(DS, 0x0F81) != 0 {
            self.lamp_colour(hooks, [0x22, 0x22, 0x25]);
            return;
        }
        self.daylight(true);
        self.lamp_colour(hooks, [0, 0, 0]);
    }

    fn daylight(&mut self, on: bool) {
        let f = self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 0x7FFF | if on { 0x8000 } else { 0 };
        self.mem.w16(DS, crate::movement::var::MAP_FLAGS, f);
    }

    /// 0000:048B: the row limits 0Eh, 0Dh, 0Eh.
    fn reset_rows(&mut self) {
        self.mem.w16(DS, LIMIT, 0xE);
        self.mem.w16(DS, DARK, 0xD);
        self.mem.w16(DS, FAR, 0xE);
    }

    /// Palette entry 71h (the lamps' glow) set to `rgb` through the
    /// buffer at 1044, as the original writes it.
    fn lamp_colour(&mut self, hooks: &mut dyn LightHooks, rgb: [u8; 3]) {
        for (i, c) in rgb.into_iter().enumerate() {
            self.mem.w8(DS, 0x1044 + i as u16, c);
        }
        hooks.palette(self, 0x71, 0x1044);
    }

    /// 2294:0B8A. The light table set for clear weather: the row limits
    /// reset, the fill colour 70h (C3C6, C3C7), the table loaded again
    /// (`LightHooks::light_table`) and its rows from 0D00h filled with it,
    /// fog off (F81, F82), the sky's window to be set again.
    pub fn light_table_reset(&mut self, hooks: &mut dyn LightHooks) {
        self.reset_rows();
        self.mem.w8(DS, 0xC3C6, 0x70);
        self.mem.w8(DS, 0xC3C7, 0x70);
        self.mem.w16(DS, 0xCCC2, 0);
        hooks.light_table(self);
        let seg = self.mem.r16(DS, 0xCCC2);
        for i in 0..0x200u16 {
            self.mem.w8(seg, 0xD00 + i, 0x70);
        }
        self.mem.w8(DS, 0x0F81, 0);
        self.mem.w8(DS, 0x0F82, 0);
        self.mem.w16(DS, crate::sky::var::WINDOW, 0xFFFF);
    }

    /// 2294:028C. Once a night (5C8 bit 400h): the light table reset,
    /// the day's mark (bit 200h) cleared, the level's lights made
    /// (`flat_lights`); in a city its street lamps lit (map flats 1Eh
    /// become 1Dh) and each lit lamp a light of radius 380h; the near
    /// list to be made again.
    pub fn night_switch(&mut self, hooks: &mut dyn LightHooks) {
        let f = self.mem.r16(DS, 0x05C8);
        if f & 0x400 != 0 {
            return;
        }
        self.light_table_reset(hooks);
        let mark = |m: &mut Machine| {
            let f = m.mem.r16(DS, 0x05C8) | 0x400;
            m.mem.w16(DS, 0x05C8, f & !0x200);
        };
        if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x200 != 0 {
            self.flat_lights();
            mark(self);
            return;
        }
        mark(self);
        self.flat_lights();
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let mut di = LEVEL.wrapping_add(self.mem.r16(DS, LEVEL_COUNT).wrapping_mul(6));
        let (mut bx, mut row) = (0x40u16, 0u16);
        for _ in 0..count(self.mem.r16(DS, 0xC359)) {
            let (mut si, mut ax) = (row, 0x40u16);
            for _ in 0..count(self.mem.r16(DS, 0xC357)) {
                if self.mem.r8(walls, si.wrapping_add(1)) == 0x80 {
                    if self.mem.r8(walls, si) == 0x1E {
                        self.mem.w8(walls, si, 0x1D);
                    }
                    if self.mem.r8(walls, si) == 0x1D {
                        self.mem.w16(DS, di, ax);
                        self.mem.w16(DS, di.wrapping_add(2), bx);
                        self.mem.w16(DS, di.wrapping_add(4), 0x380);
                        di = di.wrapping_add(6);
                        let n = self.mem.r16(DS, LEVEL_COUNT).wrapping_add(1);
                        self.mem.w16(DS, LEVEL_COUNT, n);
                    }
                }
                si = si.wrapping_add(2);
                ax = ax.wrapping_add(0x80);
            }
            row = row.wrapping_add(0x100);
            bx = bx.wrapping_add(0x80);
        }
        self.mem.w16(DS, NEAR_KEY, 0xFFFF);
    }

    /// 0000:033D. The day: night's mark (5C8 bit 400h) cleared, no level
    /// lights, a city's lamps out (map flats 1Dh become 1Eh), and the
    /// sky's objects made again (their image lists emptied).
    pub fn day_switch(&mut self, hooks: &mut dyn LightHooks) {
        let f = self.mem.r16(DS, 0x05C8) & !0x400;
        self.mem.w16(DS, 0x05C8, f);
        self.mem.w16(DS, LEVEL_COUNT, 0);
        if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x200 == 0 {
            let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
            let mut row = 0u16;
            for _ in 0..count(self.mem.r16(DS, 0xC359)) {
                let mut si = row;
                for _ in 0..count(self.mem.r16(DS, 0xC357)) {
                    if self.mem.r8(walls, si.wrapping_add(1)) == 0x80 && self.mem.r8(walls, si) == 0x1D {
                        self.mem.w8(walls, si, 0x1E);
                    }
                    si = si.wrapping_add(2);
                }
                row = row.wrapping_add(0x100);
            }
        }
        self.mem.w8(DS, 0x42B9, 0);
        self.mem.w8(DS, 0x438C, 0);
        hooks.sky_objects(self);
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
