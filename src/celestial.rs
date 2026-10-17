//! The sky's bodies (segment 0000): the stars, the sun and the two moons,
//! drawn outdoors in clear weather before the sky's objects (0000:E41A,
//! from the drawing's set-up at D50D).
//!
//! A body is a point in the sky's sphere (x, y, z) turned by the time of
//! day (CCFE / 2, about the x axis: 0000:93F0), by the place's latitude
//! (-(CB66 + E052), about the y axis: 9388) and by the player's heading
//! (C3DA, 93BC), each a product's high word against the doubled sine
//! and cosine tables (B03C, B13C); then, where more than 3E8h ahead, put
//! on the screen at (x * CCF4 / z, y * CCF6 / z) from the centre and the
//! horizon (0000:E51B, E032).
//!
//! The stars (0000:E6E9) are the 28h of the field `skyobj::star_field`
//! made: clusters of single pixels (hook CD22) and lone star images (hook
//! CD26), each drawn only strictly inside the view's clip above the
//! horizon (0000:E75E; the hooks themselves draw only over a dark sky).
//! The sun by day (0000:E43C), the moons always (0000:E46D): their places
//! (0000:E557: the first at (0, E0C0h, 0), the second at (0, D120h, BB8h),
//! turned by the day of the month and lifted by their own latitudes 1Eh
//! and 2Dh), their phase pictures made once a day (0000:E6CB: the frame
//! of MOON01.DFA / MOON02.DFA for the day, the second fourteen days on),
//! then each drawn where it is seen (0000:E4D2).

use crate::machine::Machine;
use crate::mem::DS;

/// What the sky's bodies leave to the front end.
pub trait CelestialHooks {
    /// 0000:E6CB: the moon file named at DS:`name` loaded and its frame
    /// `frame` decoded into the EMS frame (9030) at `dest`.
    fn moon_phase(&mut self, _m: &mut Machine, _name: u16, _frame: u16, _dest: u16) {}
    /// 0000:E4D2's blit (0000:5A6A): the image the far pointer at DS:`si`
    /// points to, `w` x `h`, its top left at (`x`, `y`), its rows cut at
    /// the horizon.
    fn draw(&mut self, _m: &mut Machine, _si: u16, _x: u16, _y: u16, _w: u16, _h: u16) {}
    /// The hook CD22 (2294:03A1): a star's pixel of colour `colour` at
    /// (`x`, `y`), where the sky there is dark.
    fn star_pixel(&mut self, _m: &mut Machine, _x: u16, _y: u16, _colour: u16) {}
    /// The hook CD26 (2294:03D7): the star image at `image` (an offset in
    /// the EMS frame) at (`x`, `y`), where the sky there is dark.
    fn star_image(&mut self, _m: &mut Machine, _x: u16, _y: u16, _image: u16) {}
}

/// `CelestialHooks` that do nothing.
pub struct NoCelestialHooks;

impl CelestialHooks for NoCelestialHooks {}

/// DGROUP variables.
pub mod var {
    /// The moons' places on the screen (x FFFFh: not seen), sizes, and
    /// their pictures' far pointers; the day their pictures were made for.
    pub const MOON1_AT: u16 = 0xE042;
    pub const MOON2_AT: u16 = 0xE046;
    pub const MOON1_SIZE: u16 = 0xE04A;
    pub const MOON2_SIZE: u16 = 0xE04E;
    pub const MOON1: u16 = 0xCCAA;
    pub const MOON2: u16 = 0xCCAE;
    pub const PHASE_DAY: u16 = 0xCB36;
    /// The sun's picture (a far pointer to its header).
    pub const SUN: u16 = 0xCD92;
    /// The latitude added to the place's (CB66) for the moons.
    pub const LATITUDE: u16 = 0xE052;
    pub const PLACE_LATITUDE: u16 = 0xCB66;
    /// The star field (28h of 10 bytes) and the star images' offsets.
    pub const STAR_FIELD: u16 = 0xD8C6;
    pub const STAR_IMAGES: u16 = 0xE032;
    /// The hooks drawing a star's pixel and a star's image.
    pub const STAR_PIXEL_HOOK: u16 = 0xCD22;
    pub const STAR_IMAGE_HOOK: u16 = 0xCD26;
}
use var::*;

const SIN: u16 = 0xB03C;
const COS: u16 = 0xB13C;
const MOON1_NAME: u16 = 0xD871;
const MOON2_NAME: u16 = 0xD87C;
const CENTRE: u16 = 0xC3D6;
const HORIZON: u16 = 0xC3D8;
const CLIP_LEFT: u16 = 0xA914;
const CLIP_TOP: u16 = 0xA916;
const CLIP_RIGHT: u16 = 0xA918;
const CLIP_BOTTOM: u16 = 0xA91A;
const CLIP_HEIGHT: u16 = 0xA91C;

/// The high word of a signed 16-bit product.
fn hi(a: u16, b: u16) -> u16 {
    ((a as i16 as i32 * b as i16 as i32) >> 16) as u16
}

/// `neg` then `jno`/`dec`: 8000h stays 7FFFh.
fn negs(v: u16) -> u16 {
    if v == 0x8000 { 0x7FFF } else { v.wrapping_neg() }
}

impl Machine {
    fn trig(&self, di: u16) -> (u16, u16) {
        let o = di.wrapping_add(di);
        (self.mem.r16(DS, SIN.wrapping_add(o)), self.mem.r16(DS, COS.wrapping_add(o)))
    }

    /// 0000:93F0. (`ax`, `bx`) turned by `di`: AX = 2b sin + 2a cos,
    /// BX = CX = 2a (-sin) + 2b cos (high words).
    fn turn_93f0(&self, ax: u16, bx: u16, di: u16) -> (u16, u16) {
        let (sin, cos) = self.trig(di);
        let (a, b) = (ax.wrapping_add(ax), bx.wrapping_add(bx));
        let x = hi(b, sin).wrapping_add(hi(a, cos));
        let y = hi(a, negs(sin)).wrapping_add(hi(b, cos));
        (x, y)
    }

    /// 0000:9388. (`bx`, `cx`) turned by `di`; AX is left the low word of
    /// the last product (2c cos).
    fn turn_9388(&self, bx: u16, cx: u16, di: u16) -> (u16, u16, u16) {
        let (sin, cos) = self.trig(di);
        let (b, c) = (bx.wrapping_add(bx), cx.wrapping_add(cx));
        let y = hi(c, sin).wrapping_add(hi(b, cos));
        let z = hi(b, negs(sin)).wrapping_add(hi(c, cos));
        let ax = (c as i16 as i32).wrapping_mul(cos as i16 as i32) as u16;
        (ax, y, z)
    }

    /// 0000:93BC. (`ax`, `cx`) turned by `di`: AX = 2c (-sin) + 2a cos,
    /// CX = 2a sin + 2c cos.
    pub(crate) fn turn_93bc(&self, ax: u16, cx: u16, di: u16) -> (u16, u16) {
        let (sin, cos) = self.trig(di);
        let (a, c) = (ax.wrapping_add(ax), cx.wrapping_add(cx));
        let x = hi(c, negs(sin)).wrapping_add(hi(a, cos));
        let z = hi(a, sin).wrapping_add(hi(c, cos));
        (x, z)
    }

    /// 0000:E51B. The sphere's point (`ax`, `bx`, `cx`) turned by the time
    /// of day, the latitude and the heading, and put on the screen (from
    /// the centre and the horizon) where more than 3E8h ahead: (AX, BX,
    /// seen). Unseen, AX and BX are the turned point's. None where the
    /// projection's division overflows (the original faults).
    pub fn sky_point(&self, ax: u16, bx: u16, cx: u16) -> Option<(u16, u16, bool)> {
        let di = (self.mem.r16(DS, 0xCCFE) >> 1) & 0x1FF;
        let (ax, bx) = self.turn_93f0(ax, bx, di);
        let lat = self.mem.r16(DS, PLACE_LATITUDE).wrapping_add(self.mem.r16(DS, LATITUDE));
        let (_, bx, cx) = self.turn_9388(bx, cx, lat.wrapping_neg() & 0x1FF);
        let (ax, cx) = self.turn_93bc(ax, cx, self.mem.r16(DS, 0xC3DA) & 0x1FF);
        if (cx as i16) <= 0x3E8 {
            return Some((ax, bx, false));
        }
        let div = |v: u16, k: u16| -> Option<u16> {
            let q = (v as i16 as i32 * k as i16 as i32) / cx as i16 as i32;
            i16::try_from(q).ok().map(|q| q as u16)
        };
        Some((div(ax, self.mem.r16(DS, 0xCCF4))?, div(bx, self.mem.r16(DS, 0xCCF6))?, true))
    }

    /// 0000:E557. The moons' places on the screen (MOON1_AT, MOON2_AT; x
    /// FFFFh where not seen), each turned by the day of the month and
    /// lifted by its latitude (1Eh, 2Dh; LATITUDE put back after).
    pub fn moon_places(&mut self) -> Option<()> {
        let saved = self.mem.r16(DS, LATITUDE);
        let day = (self.mem.r16(DS, 0xCCA8) & 0x1F) << 4;
        for (bx, cx, lat, at) in [(0xE0C0u16, 0u16, 0x1Eu16, MOON1_AT), (0xD120, 0xBB8, 0x2D, MOON2_AT)] {
            let (ax, bx, cx) = self.turn_9388(bx, cx, day);
            self.mem.w16(DS, LATITUDE, lat);
            let (x, y, seen) = self.sky_point(ax, bx, cx)?;
            self.mem.w16(DS, at, 0xFFFF);
            if seen {
                self.mem.w16(DS, at, x.wrapping_add(self.mem.r16(DS, CENTRE)));
                self.mem.w16(DS, at + 2, y.wrapping_add(self.mem.r16(DS, HORIZON)));
            }
        }
        self.mem.w16(DS, LATITUDE, saved);
        Some(())
    }

    /// 0000:E4D2. The image whose far pointer is at DS:`si`, `w` x `h`, at
    /// (`x`, `y`): drawn (`CelestialHooks::draw`, the clip's bottom the
    /// horizon meanwhile) unless wholly right of the clip, left of it,
    /// above it, or not above the horizon.
    fn sky_body(&mut self, hooks: &mut dyn CelestialHooks, si: u16, x: u16, y: u16, w: u16, h: u16) {
        let r = |m: &Machine, at| m.mem.r16(DS, at) as i16;
        let (right, bottom) = (x.wrapping_add(w) as i16, y.wrapping_add(h) as i16);
        if x as i16 >= r(self, CLIP_RIGHT) || right <= r(self, CLIP_LEFT) || bottom <= r(self, CLIP_TOP) || y as i16 >= r(self, HORIZON) {
            return;
        }
        let saved = (self.mem.r16(DS, CLIP_BOTTOM), self.mem.r16(DS, CLIP_HEIGHT));
        let horizon = self.mem.r16(DS, HORIZON);
        self.mem.w16(DS, CLIP_BOTTOM, horizon);
        self.mem.w16(DS, CLIP_HEIGHT, horizon.wrapping_sub(self.mem.r16(DS, CLIP_TOP)).wrapping_add(1));
        hooks.draw(self, si, x, y, w, h);
        self.mem.w16(DS, CLIP_BOTTOM, saved.0);
        self.mem.w16(DS, CLIP_HEIGHT, saved.1);
    }

    /// 0000:E75E. Strictly inside the clip and above the horizon.
    fn in_sky(&self, x: u16, y: u16) -> bool {
        let r = |at| self.mem.r16(DS, at) as i16;
        let (x, y) = (x as i16, y as i16);
        x > r(CLIP_LEFT) && x < r(CLIP_RIGHT) && y > r(CLIP_TOP) && y < r(HORIZON)
    }

    /// 0000:E6E9. The star field drawn, the clip's bottom the horizon
    /// meanwhile: each star's place (`sky_point`; seen or not, the
    /// original doesn't look), then a cluster's pixels (hook CD22, colour
    /// +4) at their offsets, or a lone star's image (hook CD26, from
    /// STAR_IMAGES), where `in_sky`.
    fn stars(&mut self, hooks: &mut dyn CelestialHooks) -> Option<()> {
        let saved = self.mem.r16(DS, CLIP_BOTTOM);
        let horizon = self.mem.r16(DS, HORIZON);
        self.mem.w16(DS, CLIP_BOTTOM, horizon);
        let (pixels, images) = (self.hook_set(STAR_PIXEL_HOOK), self.hook_set(STAR_IMAGE_HOOK));
        for i in 0..0x28u16 {
            let si = STAR_FIELD + 10 * i;
            let (ax, bx, _) = self.sky_point(self.mem.r16(DS, si), self.mem.r16(DS, si + 2), self.mem.r16(DS, si + 4))?;
            let ax = ax.wrapping_add(self.mem.r16(DS, CENTRE));
            let bx = bx.wrapping_add(self.mem.r16(DS, HORIZON));
            let n = self.mem.r16(DS, si + 6);
            if n != 0 {
                let mut p = self.mem.r16(DS, si + 8);
                for _ in 0..n {
                    let x = ax.wrapping_add(self.mem.r16(DS, p));
                    let y = bx.wrapping_add(self.mem.r16(DS, p.wrapping_add(2)));
                    if self.in_sky(x, y) && pixels {
                        let c = self.mem.r16(DS, p.wrapping_add(4));
                        hooks.star_pixel(self, x, y, c);
                    }
                    p = p.wrapping_add(6);
                }
            } else if self.in_sky(ax, bx) && images {
                let k = self.mem.r16(DS, si + 8);
                let image = self.mem.r16(DS, k.wrapping_add(k).wrapping_add(STAR_IMAGES));
                hooks.star_image(self, ax, bx, image);
            }
        }
        self.mem.w16(DS, CLIP_BOTTOM, saved);
        Some(())
    }

    /// 0000:E6CB: the moon file named at DS:`name` read to [BA54]
    /// (0000:742C) and its frame `frame` unpacked (5656) into the EMS
    /// frame ([9030]) at `dest`. None where the file doesn't load.
    pub fn moon_phase_load(&mut self, files: &mut crate::bsa::Files, name: u16, frame: u16, dest: u16) -> Option<()> {
        let (off, seg) = (self.mem.r16(DS, 0xBA54), self.mem.r16(DS, 0xBA56));
        self.file_load(files, name, seg, off)?;
        let es = self.mem.r16(DS, 0x9030);
        self.frames_unpack(seg, off, frame, es, dest);
        Some(())
    }

    /// 0000:E470..E49F, the moons' phases made for a new day (CCA8 not
    /// PHASE_DAY), with the files (E6CB, `moon_phase_load`): for a front
    /// end that draws the sky with `celestial` on a copy of the machine
    /// (the copy's loads have no files), run on the machine itself first
    /// where the frame's E41A would run (outdoors, out of fog, BB09
    /// clear), in the bank E41A maps (C8F3), the bank mapped before put
    /// back. The second moon's frame goes to the EMS frame at CCAEh, the
    /// offset E499 puts in DI, not the far pointer there: the original's.
    pub fn moon_phases(&mut self, files: &mut crate::bsa::Files) -> Option<()> {
        let flags = (self.mem.r16(DS, 0xC398), self.mem.r16(DS, 0xC39A));
        if flags.0 & 4 == 0 || flags.1 & 0x20 != 0 || self.mem.r8(DS, 0xBB09) != 0 {
            return Some(());
        }
        let day = self.mem.r8(DS, 0xCCA8);
        if day == self.mem.r8(DS, PHASE_DAY) {
            return Some(());
        }
        let mapped = self.mem.r16(DS, crate::level::var::EMS_MAPPED) >> 2;
        self.ems_map(self.mem.r8(DS, 0xC8F3) as u16);
        self.mem.w8(DS, PHASE_DAY, day);
        let dest = self.mem.r16(DS, MOON1);
        let r = self.moon_phase_load(files, MOON1_NAME, day as u16, dest).and_then(|()| self.moon_phase_load(files, MOON2_NAME, (day as u16 + 0xE) & 0x1F, MOON2));
        self.ems_map(mapped);
        r
    }

    /// 0000:E41A. The sky's bodies, in clear weather (BB09 0): the sky's
    /// bank mapped; the stars; by day (time 100h..2FFh) the sun at the
    /// sphere's (0, 3A98h, 0), its header's size (+4, +6), its picture
    /// past the header; then the moons (0000:E46D): their places, their
    /// pictures made again when the day of the month changed (the second
    /// decoded at the frame's offset CCAEh itself, not through its
    /// pointer: kept), each drawn where seen. None where a projection's
    /// division overflows (the original faults).
    pub fn celestial(&mut self, hooks: &mut dyn CelestialHooks) -> Option<()> {
        if self.mem.r8(DS, 0xBB09) != 0 {
            return Some(());
        }
        self.ems_map(self.mem.r8(DS, 0xC8F3) as u16);
        self.stars(hooks)?;
        let t = self.mem.r16(DS, 0xCCFE);
        if (0x100..0x300).contains(&t) {
            let (x, y, seen) = self.sky_point(0, 0x3A98, 0)?;
            if seen {
                let x = x.wrapping_add(self.mem.r16(DS, CENTRE));
                let y = y.wrapping_add(self.mem.r16(DS, HORIZON));
                let (off, seg) = (self.mem.r16(DS, SUN), self.mem.r16(DS, SUN + 2));
                let (w, h) = (self.mem.r16(seg, off.wrapping_add(4)), self.mem.r16(seg, off.wrapping_add(6)));
                self.mem.w16(DS, SUN, off.wrapping_add(0xC));
                self.sky_body(hooks, SUN, x, y, w, h);
                let o = self.mem.r16(DS, SUN).wrapping_sub(0xC);
                self.mem.w16(DS, SUN, o);
            }
        }
        self.moon_places()?;
        let day = self.mem.r8(DS, 0xCCA8);
        if day != self.mem.r8(DS, PHASE_DAY) {
            self.mem.w8(DS, PHASE_DAY, day);
            let dest = self.mem.r16(DS, MOON1);
            hooks.moon_phase(self, MOON1_NAME, day as u16, dest);
            hooks.moon_phase(self, MOON2_NAME, (day as u16 + 0xE) & 0x1F, MOON2);
        }
        for (at, size, si) in [(MOON1_AT, MOON1_SIZE, MOON1), (MOON2_AT, MOON2_SIZE, MOON2)] {
            let x = self.mem.r16(DS, at);
            if (x as i16) < 0 {
                continue;
            }
            let (y, w, h) = (self.mem.r16(DS, at + 2), self.mem.r16(DS, size), self.mem.r16(DS, size + 2));
            self.sky_body(hooks, si, x, y, w, h);
        }
        Some(())
    }
}
