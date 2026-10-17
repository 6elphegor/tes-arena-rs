//! The sky outdoors, as the floor's half draws it (0000:D502..D51F):
//! unless in fog (C39A bit 20h), the sky's rows (0000:DF2F) and its
//! bodies (0000:E41A, `Machine::celestial`); then the hook CD1A
//! (2294:0A5D, `Machine::sky_draw`: the fog's step, the volcano, the
//! clouds and the mountains, lightning). With the level's fog (CB5C) the
//! frame's stage A33E calls the hook alone.
//!
//! The pictures they draw go through the original's drawing routines:
//! 0000:5828 (an image by its header), 5A6A and 59D8 (clipped to the
//! view, with and without its 0s), 5B08 and 59A4 (whole), 9A72 (a
//! rectangle of [9288]).

use super::{Soft, ROWS};
use crate::machine::Machine;
use crate::mem::DS;

const LEFT: u16 = super::LEFT;
const TOP: u16 = super::TOP;
const RIGHT: u16 = super::RIGHT;
const BOTTOM: u16 = super::BOTTOM;

/// What the sky's drawing reaches: the page and the paths not ported.
struct Out<'a> {
    page: &'a mut [u8],
    gaps: &'a mut Vec<&'static str>,
}

impl Out<'_> {
    fn gap(&mut self, at: &'static str) {
        if !self.gaps.contains(&at) {
            self.gaps.push(at);
        }
    }
}

fn r16(m: &Machine, o: u16) -> u16 {
    m.mem.r16(DS, o)
}

fn below(a: u16, b: u16) -> bool {
    (a as i16) < (b as i16)
}

/// The page offset of row `y` (A6A0's word).
fn row(m: &Machine, y: u16) -> u16 {
    r16(m, ROWS.wrapping_add(y.wrapping_add(y)))
}

/// 0000:5A6A (`transparent`, its 0s left out) or 59D8: `w` x `h` from
/// seg:off at (`x`, `y`), cut to the view (A914..A91A). The rows' loop
/// is the original's `loop`: a count of 0 or one gone negative would run
/// it on through memory, which is left as a gap.
#[allow(clippy::too_many_arguments)]
fn clipped(m: &Machine, out: &mut Out, x: u16, y: u16, w: u16, h: u16, seg: u16, off: u16, transparent: bool) {
    let (mut ax, mut bx, mut cx, mut si) = (x, y, w, off);
    let mut rows = h;
    let top = r16(m, TOP);
    if below(bx, top) {
        let d = top.wrapping_sub(bx);
        rows = rows.wrapping_sub(d);
        si = si.wrapping_add(w.wrapping_mul(d));
        bx = top;
    }
    let end = bx.wrapping_add(rows);
    let bottom = r16(m, BOTTOM);
    if (end as i16) > (bottom as i16) {
        rows = rows.wrapping_sub(end.wrapping_sub(bottom)).wrapping_add(1);
    }
    let mut bp = 0u16;
    let left = r16(m, LEFT);
    if below(ax, left) {
        let d = left.wrapping_sub(ax);
        si = si.wrapping_add(d);
        cx = cx.wrapping_sub(d);
        bp = bp.wrapping_add(d);
        ax = left;
    }
    let end = ax.wrapping_add(cx);
    let right = r16(m, RIGHT);
    if (end as i16) > (right as i16) {
        let d = end.wrapping_sub(right);
        cx = cx.wrapping_sub(d).wrapping_add(1);
        bp = bp.wrapping_add(d).wrapping_sub(1);
    }
    let mut di = row(m, bx).wrapping_add(ax);
    if transparent && (cx == 0 || rows == 0) {
        return;
    }
    if (cx as i16) < 0 || (rows as i16) <= 0 {
        out.gap("5A6A: a picture cut to nothing");
        return;
    }
    for _ in 0..rows {
        let mut d = di;
        for _ in 0..cx {
            let b = m.mem.r8(seg, si);
            si = si.wrapping_add(1);
            if b != 0 || !transparent {
                out.page[d as usize] = b;
            }
            d = d.wrapping_add(1);
        }
        si = si.wrapping_add(bp);
        di = di.wrapping_add(0x140);
    }
}

/// 0000:5B08 (`transparent`) or 59A4: `w` x `h` from seg:off at (`x`,
/// `y`), not cut.
#[allow(clippy::too_many_arguments)]
fn whole(m: &Machine, out: &mut Out, x: u16, y: u16, w: u16, h: u16, seg: u16, off: u16, transparent: bool) {
    if (h as i16) <= 0 || (transparent && w == 0) {
        out.gap("5B08: a picture of no rows");
        return;
    }
    let mut di = row(m, y).wrapping_add(x);
    let mut si = off;
    for _ in 0..h {
        let mut d = di;
        for _ in 0..w {
            let b = m.mem.r8(seg, si);
            si = si.wrapping_add(1);
            if b != 0 || !transparent {
                out.page[d as usize] = b;
            }
            d = d.wrapping_add(1);
        }
        di = di.wrapping_add(0x140);
    }
}

/// 0000:5828: the image whose 12-byte header is at seg:off (x, y, width,
/// height, flags), drawn at its place: whole where it lies within the
/// view (or 9207 says so), else cut to it; 0s left out with flags bit
/// 800h. Compressed images (flags 1, 2, 4, 8) and the capture to 8F9A
/// aren't ported.
fn image(m: &Machine, out: &mut Out, seg: u16, off: u16) {
    let flags = m.mem.r16(seg, off.wrapping_add(8));
    if m.mem.r32(DS, 0x8F9A) != 0 {
        return out.gap("5854: an image to the capture buffer");
    }
    if flags & 0xF != 0 {
        return out.gap("5871: a compressed image");
    }
    let x = m.mem.r16(seg, off);
    let y = m.mem.r16(seg, off.wrapping_add(2));
    let w = m.mem.r16(seg, off.wrapping_add(4));
    let h = m.mem.r16(seg, off.wrapping_add(6));
    let si = off.wrapping_add(0xC);
    let transparent = flags & 0x800 != 0;
    let gt = |a: u16, b: u16| (a as i16) > (b as i16);
    let (left, top, right, bottom) = (r16(m, LEFT), r16(m, TOP), r16(m, RIGHT), r16(m, BOTTOM));
    if m.mem.r8(DS, 0x9207) == 0 {
        if gt(x, right) || gt(y, bottom) || below(y.wrapping_add(h), top) || below(x.wrapping_add(w), left) {
            return;
        }
        let inside = !below(y, top) && !gt(y.wrapping_add(h), bottom) && !below(x, left) && !gt(x.wrapping_add(w), right);
        if !inside {
            return clipped(m, out, x, y, w, h, seg, si, transparent);
        }
    }
    whole(m, out, x, y, w, h, seg, si, transparent)
}

/// 0000:9A72: a rectangle `w` x `h` of [9288] at (`x`, `y`).
fn fill(m: &Machine, out: &mut Out, x: u16, y: u16, w: u16, h: u16) {
    if (h as i16) <= 0 {
        return out.gap("9A72: a rectangle of no rows");
    }
    let c = m.mem.r8(DS, 0x9288);
    let mut di = row(m, y).wrapping_add(x);
    for _ in 0..h {
        for k in 0..w {
            out.page[di.wrapping_add(k) as usize] = c;
        }
        di = di.wrapping_add(0x140);
    }
}

/// 2294:03A1 and 03D7's test: whether the pixel at (`x`, `y`) is of a
/// dark sky, its colour's place in the day's colours (43C0, from the
/// window 3C8B) under 10h in each of red, green and blue.
fn dark(m: &Machine, out: &Out, x: u16, y: u16) -> (bool, u16) {
    let at = row(m, y).wrapping_add(x);
    let c = out.page[at as usize] as u16;
    let si = (c * 3).wrapping_add(r16(m, crate::sky::var::WINDOW));
    let seg = r16(m, crate::sky::var::COLOURS_SEG);
    let dark = (0..3).all(|k| m.mem.r8(seg, si.wrapping_add(k)) < 0x10);
    (dark, at)
}

/// The sky's bodies' drawing (`CelestialHooks`).
struct Bodies<'a>(Out<'a>);

impl crate::celestial::CelestialHooks for Bodies<'_> {
    fn moon_phase(&mut self, _m: &mut Machine, _name: u16, _frame: u16, _dest: u16) {
        self.0.gap("0000:E6CB: a moon's phase made");
    }
    /// 0000:E4D2's blit: 5A6A from the far pointer at DS:`si`.
    fn draw(&mut self, m: &mut Machine, si: u16, x: u16, y: u16, w: u16, h: u16) {
        let (off, seg) = (r16(m, si), r16(m, si.wrapping_add(2)));
        clipped(m, &mut self.0, x, y, w, h, seg, off, true);
    }
    /// 2294:03A1.
    fn star_pixel(&mut self, m: &mut Machine, x: u16, y: u16, colour: u16) {
        let (dark, at) = dark(m, &self.0, x, y);
        if dark {
            self.0.page[at as usize] = colour as u8;
        }
    }
    /// 2294:03D7: 5A66 of the image at the EMS frame's `image` (its
    /// header's width and height at +4, +6, its pixels past 0Ch).
    fn star_image(&mut self, m: &mut Machine, x: u16, y: u16, image: u16) {
        if !dark(m, &self.0, x, y).0 {
            return;
        }
        let seg = r16(m, 0x9030);
        let (w, h) = (m.mem.r16(seg, image.wrapping_add(4)), m.mem.r16(seg, image.wrapping_add(6)));
        clipped(m, &mut self.0, x, y, w, h, seg, image.wrapping_add(0xC), true);
    }
}

/// The light's steps the fog's (2294:0B01) may take: none of them is
/// ported here; taking one is a gap.
struct Fogless<'a, 'b>(&'a mut Out<'b>);

impl crate::light::LightHooks for Fogless<'_, '_> {
    fn light_table(&mut self, _m: &mut Machine) {
        self.0.gap("2294:0B8A: the light table made again");
    }
    fn lgt(&mut self, _m: &mut Machine, _name: u16) {
        self.0.gap("2294:0BEF: the fog's light loaded");
    }
    fn fog_light_table(&mut self, _m: &mut Machine) {
        self.0.gap("2294:0C36: the fog's light table");
    }
    fn fog_fill(&mut self, _m: &mut Machine) {
        self.0.gap("2294:0B4D: the view filled with the fog");
    }
}

/// The sky's objects' drawing (`SkyHooks`).
struct Objects<'a>(Out<'a>);

impl crate::skyobj::SkyHooks for Objects<'_> {
    fn fog(&mut self, m: &mut Machine) -> bool {
        let fog = m.fog_step(&mut Fogless(&mut self.0));
        if fog {
            self.0.gap("2294:0B01: fog");
        }
        fog
    }
    fn draw_image(&mut self, m: &mut Machine, seg: u16, off: u16) {
        image(m, &mut self.0, seg, off);
    }
    fn draw_dfa(&mut self, _m: &mut Machine, _seg: u16, _off: u16, _frame: u16, _x: u16, _y: u16) {
        self.0.gap("292B:0201: the volcano's frames");
    }
    fn fill(&mut self, m: &mut Machine, x: u16, y: u16, w: u16, h: u16) {
        fill(m, &mut self.0, x, y, w, h);
    }
    fn bolt(&mut self, _m: &mut Machine, _seg: u16, _off: u16, _frame: u16) {
        self.0.gap("0000:78A8: lightning's bolt");
    }
}

impl Soft {
    /// 0000:D502..D513, outdoors: unless in fog (C39A bit 20h) the sky's
    /// rows and bodies, then the hook CD1A.
    pub(super) fn sky(&mut self) {
        if self.g16(0xC39A) & 0x20 == 0 {
            self.sky_rows();
            let Soft { m, page, gaps } = self;
            if m.celestial(&mut Bodies(Out { page, gaps })).is_none() {
                self.gap("0000:E51B: a sky's body's division overflows");
            }
        }
        self.sky_hook();
    }

    /// 0000:A8EF with SI CD1A: the sky's hook, where set (2294:0A5D).
    pub(super) fn sky_hook(&mut self) {
        if !self.m.hook_set(0xCD1A) {
            return;
        }
        if self.g32(0xCD1A) != 0x2294_0A5D {
            return self.gap("0000:A8EF: the sky's hook another routine");
        }
        let Soft { m, page, gaps } = self;
        if m.sky_draw(&mut Objects(Out { page, gaps })).is_none() {
            self.gap("2294:0A5D: a sky's object's division");
        }
    }

    /// 0000:DF2F: the sky's rows, from the view's top to the horizon,
    /// each the dither's row of 8 (at CD04, the last 100 of them ending at
    /// the horizon) repeated across the view (the routine at CD8A, cut
    /// short after the view's width in eights).
    fn sky_rows(&mut self) {
        if self.g16(crate::sky::var::DITHER_SEG) == 0 {
            return self.gap("2294:01F1: the sky's buffers not made");
        }
        let rows = self.g16(0xC3D8).wrapping_sub(self.g16(TOP));
        if (rows as i16) <= 0 {
            return self.gap("DF2F: a sky of no rows");
        }
        let mut si = 0x64u16.wrapping_sub(rows) << 3;
        let eights = (self.g16(RIGHT).wrapping_sub(self.g16(LEFT)).wrapping_add(7) >> 3) & 0xFF;
        let mut di = self.row(self.g16(TOP)).wrapping_add(self.g16(LEFT));
        let seg = self.g16(crate::sky::var::DITHER_SEG);
        for _ in 0..rows {
            let mut pattern = [0u8; 8];
            for (k, p) in pattern.iter_mut().enumerate() {
                *p = self.m.mem.r8(seg, si.wrapping_add(k as u16));
            }
            for g in 0..eights {
                for (k, &p) in pattern.iter().enumerate() {
                    self.px(di.wrapping_add(8 * g).wrapping_add(k as u16), p);
                }
            }
            di = di.wrapping_add(0x140);
            si = si.wrapping_add(8);
        }
    }
}
