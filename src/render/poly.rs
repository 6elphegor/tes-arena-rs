//! A square's cap (its top or bottom, a square of the TOP texture), as the
//! 3D mode draws it at once (BA80 set): 0000:CBFF sets the square's four
//! corners at A872 and calls B2E5, which maps the texture's bank and
//! calls FF26: the corners turned to the eye (FF74) into E8C4 with their
//! texture's corners (E8EE), then the generic polygon drawer.
//!
//! The polygon drawer (0000:7C51, a far routine):
//!
//! - 7C51: a quad whose depths span twice or more (or reach behind the
//!   eye) is cut into four eight-sided polygons through its edges'
//!   middles (AF8F, AF99, AFA3, AFAD) and its centre (AFB7), at 929E;
//! - 85AE: each vertex scaled into clip space (x by CCF4 over the view's
//!   half width, y by CCF6 over its half height, z plus 1Eh), its outcode
//!   (8950); all out on one side, nothing; any out, clipped (8698) plane
//!   by plane (8755 with 8839, 887B, 88C0, 8907), up to four passes, with
//!   INT 0 pointed at 87D4 meanwhile: a division that overflows gives
//!   7FFFh with the sign of the dividend's over the divisor's;
//! - then projected onto the screen and filled (8AF2, through the far
//!   pointer CS:7C3A): from the topmost vertex down, two edges walked
//!   (89CE forward, 8A60 back through the list) in 16.16 steps, each row
//!   a span stepping the texture's u (8.8, its integer the column) and v
//!   (its 10 high bits the row's offset) with `xlat`, through the colour
//!   table at GS:[AFEE] where AFEE isn't 0.
//!
//! The filler and the edges keep their steps in the code (the immediates
//! of 8BCE.., 8BF6..: self-modified), here the copy's bytes at segment 0
//! as the rest of the renderer keeps CS's variables. The stack 85AE moves
//! to DS:974E (its caller's SS:SP at AF8B, AF8D) isn't modelled, nor INT
//! 0's vector, saved and put back around the clipping.

use super::{s, Soft, ROWS};

/// The polygon's corners as FF74 leaves them: 4 of x, y, z, u, v.
const CORNERS: u16 = 0xE8C4;
/// The texture's corners (u 8.8, v 10.6), a dword each.
const TEXTURE_CORNERS: u16 = 0xE8EE;
/// Where 7C51 builds an eight-sided part of a quad cut in four.
const PART: u16 = 0x929E;
/// 85AE's two lists of clip-space vertices (x, y, z, outcode, u, v).
const LIST_A: u16 = 0xADC4;
const LIST_B: u16 = 0xAE9C;

/// The four clipping planes in the order 8698 tries them: the outcode's
/// bit and its routine.
const PLANES: [(u8, u16); 4] = [(4, 0x8839), (8, 0x887B), (2, 0x88C0), (1, 0x8907)];

/// 16-bit signed division as IDIV does it, None where it faults.
fn idiv16(n: i32, d: u16) -> Option<u16> {
    let d = s(d);
    if d == 0 {
        return None;
    }
    let q = n / d;
    (q == q as i16 as i32).then_some(q as u16)
}

/// 32-bit signed division as IDIV does it (EDX:EAX the dividend's sign
/// extension), None where it faults.
fn idiv32(n: i32, d: i32) -> Option<u32> {
    if d == 0 || (n == i32::MIN && d == -1) {
        return None;
    }
    Some((n / d) as u32)
}

/// `imul cx; shl ax, 1; rcl dx, 1`: DX of twice the product.
fn scaled(diff: u16, t: u16) -> u16 {
    (((s(diff) * s(t)) as u32) >> 15) as u16
}

impl Soft {
    /// 0000:CBFF: a cap at height `ax` (its corners' y -`ax`) on the
    /// square A84A, A84E, wound by `cx`, in the texture [D804].
    pub(super) fn cap_draw(&mut self, di: u16, ax: u16, cx: u16) {
        let y = ax.wrapping_neg();
        for o in [0xA874, 0xA87A, 0xA880, 0xA886] {
            self.s16(o, y);
        }
        let x = self.g16(0xA84A) << 7;
        let z = self.g16(0xA84E) << 7;
        self.s16(0xA872, x);
        self.s16(0xA884, x);
        self.s16(0xA878, x.wrapping_add(0x80));
        self.s16(0xA87E, x.wrapping_add(0x80));
        let (near, far) = (z, z.wrapping_add(0x80));
        let (a, b) = if cx != 0 { (near, far) } else { (far, near) };
        self.s16(0xA876, a);
        self.s16(0xA87C, a);
        self.s16(0xA882, b);
        self.s16(0xA888, b);
        let tex = self.g16(0xD804);
        self.polygon_textured(0xA872, tex, di);
    }

    /// 0000:B2E5: the polygon at `si` in texture `ax` (1-based: its bank
    /// C8F0 plus (ax - 1) / 15, its segment the frame's plus 100h each).
    /// With BA80 clear the original files it in the deferred list (B312):
    /// not ported, the 3D mode draws at once.
    fn polygon_textured(&mut self, si: u16, ax: u16, _dx: u16) {
        if self.g8(0xBA80) == 0 {
            self.gap("B312: a polygon deferred");
            return;
        }
        let ax = ax.wrapping_sub(1);
        let (q, r) = (ax / 0xF, ax % 0xF);
        let bank = (q & 0xFF00) | (q as u8).wrapping_add(self.g8(0xC8F0)) as u16;
        self.m.ems_map(bank);
        let seg = (r << 8).wrapping_add(self.g16(0x9030));
        let v = self.g16(0xA91A).wrapping_add(1);
        self.s16(0xA91A, v);
        self.polygon_world(si, seg);
        let v = self.g16(0xA91A).wrapping_sub(1);
        self.s16(0xA91A, v);
    }

    /// 0000:FF26: the four world corners at `si` (x, y, z) turned to the
    /// eye (FF74) and drawn in the texture at `seg`, unless all lie
    /// within E8EC of the eye.
    fn polygon_world(&mut self, si: u16, seg: u16) {
        self.s16(0xE8FE, seg);
        // FF74.
        self.s16(0xA836, 1);
        let (mut si, mut di, mut bx) = (si, CORNERS, TEXTURE_CORNERS);
        for _ in 0..4 {
            let ax = self.g16(si).wrapping_sub(self.g16(0xC3D0));
            let cx = self.g16(si.wrapping_add(4)).wrapping_sub(self.g16(0xC3D4));
            let (ax, cx) = self.rot(self.g16(0xC3DA), ax, cx);
            self.s16(di, ax);
            let y = self.g16(si.wrapping_add(2)).wrapping_add(self.g16(0xC3D2));
            self.s16(di.wrapping_add(2), y);
            let cx = cx.wrapping_sub(0x1E);
            self.s16(di.wrapping_add(4), cx);
            if (cx.wrapping_sub(self.g16(0xE8EC)) as i16) >= 0 {
                self.s16(0xA836, 0);
            }
            let (u, v) = (self.g16(bx), self.g16(bx.wrapping_add(2)));
            self.s16(di.wrapping_add(6), u);
            self.s16(di.wrapping_add(8), v);
            bx = bx.wrapping_add(4);
            di = di.wrapping_add(10);
            si = si.wrapping_add(6);
        }
        if self.g16(0xA836) != 0 {
            return;
        }
        let half = |a: u16, b: u16| ((a.wrapping_sub(b).wrapping_add(1)) >> 1).wrapping_sub(1);
        let w = half(self.g16(0xA918), self.g16(0xA914));
        let h = half(self.g16(0xA91A), self.g16(0xA916));
        self.s16(0xAF83, w);
        self.s16(0xAF85, h);
        for (to, from) in [(0xAF87, 0xC3D6), (0xAF89, 0xC3D8), (0xAF7F, 0xCCF4), (0xAF81, 0xCCF6)] {
            let v = self.g16(from);
            self.s16(to, v);
        }
        self.polygon(CORNERS, 4);
    }

    /// 0000:7C51: the polygon of `cx` vertices (x, y, z, u, v) at `si`,
    /// a quad cut in four where its depths span twice or more.
    fn polygon(&mut self, si: u16, cx: u16) {
        self.cs16(0x7C3A, 0x8AF2);
        self.cs16(0x7C3C, 0);
        if cx != 4 {
            self.polygon_clipped(si, cx);
            return;
        }
        let v = |m: &Self, k: u16, w: u16| m.g16(si.wrapping_add(10 * k + 2 * w));
        let (mut most, mut least) = (0x8001u16, 0x7FFFu16);
        for k in 0..4 {
            let z = v(self, k, 2).wrapping_add(0x1E);
            if s(most) <= s(z) {
                most = z;
            }
            if s(least) >= s(z) {
                least = z;
            }
        }
        if s(most) < 0 {
            return;
        }
        if s(least) > 0 && s(most / least) < 2 {
            self.polygon_clipped(si, 4);
            return;
        }
        // The middles: of the edges (AF8F 0-1, AF99 1-2, AFA3 2-3, AFAD
        // 0-3), then the centre (AFB7) of 0-1's and 2-3's.
        let mid = |a: u16, b: u16| ((s(a) + s(b)) >> 1) as u16;
        for (to, a, b) in [(0xAF8Fu16, 0, 1), (0xAF99, 1, 2), (0xAFA3, 2, 3), (0xAFAD, 0, 3)] {
            for w in 0..5 {
                let m = mid(v(self, a, w), v(self, b, w));
                self.s16(to + 2 * w, m);
            }
        }
        for w in 0..5 {
            let m = mid(self.g16(0xAF8F + 2 * w), self.g16(0xAFA3 + 2 * w));
            self.s16(0xAFB7 + 2 * w, m);
        }
        // The four parts, each its corner, the middles and the centre
        // with the points halfway between (7EAD, 806A, ..).
        let (v0, v1, v2, v3) = (si, si.wrapping_add(0xA), si.wrapping_add(0x14), si.wrapping_add(0x1E));
        let (m01, m12, m23, m30, c) = (0xAF8F, 0xAF99, 0xAFA3, 0xAFAD, 0xAFB7);
        for ring in [[v0, m01, c, m30], [m01, v1, m12, c], [m30, c, m23, v3], [c, m12, v2, m23]] {
            let mut di = PART;
            for k in 0..4 {
                let (a, b) = (ring[k], ring[(k + 1) % 4]);
                for w in 0..5 {
                    let x = self.g16(a + 2 * w);
                    self.s16(di + 2 * w, x);
                }
                for w in 0..5 {
                    let x = mid(self.g16(a + 2 * w), self.g16(b + 2 * w));
                    self.s16(di + 0xA + 2 * w, x);
                }
                di += 0x14;
            }
            self.polygon_clipped(PART, 8);
        }
    }

    /// 0000:85AE: the polygon of `cx` vertices at `si` into clip space,
    /// clipped, projected and filled.
    fn polygon_clipped(&mut self, mut si: u16, cx: u16) {
        self.s16(0xADC2, cx);
        self.s16(0xADC0, si);
        self.s8(0xAF76, 0);
        self.s8(0xAF77, 0xFF);
        self.s16(0xAF74, LIST_A);
        self.s16(0xAF78, LIST_A);
        self.s16(0xAF7A, LIST_B);
        for _ in 0..if cx == 0 { 0x10000 } else { cx as u32 } {
            let (x, y) = (self.g16(si), self.g16(si.wrapping_add(2)));
            let (Some(x), Some(y)) = (idiv16(s(x) * s(self.g16(0xAF7F)), self.g16(0xAF83)), idiv16(s(y) * s(self.g16(0xAF81)), self.g16(0xAF85))) else {
                self.gap("85E6: a vertex's division overflows (the original's fault)");
                return;
            };
            let z = self.g16(si.wrapping_add(4)).wrapping_add(0x1E);
            let code = outcode(x, y, z);
            let o = self.g16(0xAF74);
            self.s8(o.wrapping_add(6), code);
            self.code_seen(code);
            self.s16(o, x);
            self.s16(o.wrapping_add(2), y);
            self.s16(o.wrapping_add(4), z);
            let (u, v) = (self.g16(si.wrapping_add(6)), self.g16(si.wrapping_add(8)));
            self.s16(o.wrapping_add(8), u);
            self.s16(o.wrapping_add(0xA), v);
            self.s16(0xAF74, o.wrapping_add(0xC));
            si = si.wrapping_add(10);
        }
        if self.g8(0xAF77) != 0 {
            return;
        }
        if self.g8(0xAF76) != 0 && !self.clip() {
            return;
        }
        // 863D: onto the screen, x, y, u, v at AF7A.
        let (mut si, mut di, end) = (self.g16(0xAF78), self.g16(0xAF7A), self.g16(0xAF74));
        let mut n = 0u16;
        loop {
            n = n.wrapping_add(1);
            let z = self.g16(si.wrapping_add(4));
            let x = idiv16(s(self.g16(si)) * s(self.g16(0xAF83)), z);
            let y = idiv16(s(self.g16(si.wrapping_add(2))) * s(self.g16(0xAF85)), z);
            let (Some(x), Some(y)) = (x, y) else {
                self.gap("8655: a projection's division overflows (the original's fault)");
                return;
            };
            let x = x.wrapping_add(self.g16(0xAF87));
            let y = y.wrapping_add(self.g16(0xAF89));
            self.s16(di, x);
            self.s16(di.wrapping_add(2), y);
            let (u, v) = (self.g16(si.wrapping_add(8)), self.g16(si.wrapping_add(0xA)));
            self.s16(di.wrapping_add(4), u);
            self.s16(di.wrapping_add(6), v);
            di = di.wrapping_add(8);
            si = si.wrapping_add(0xC);
            if si == end {
                break;
            }
        }
        let si = self.g16(0xAF7A);
        let tex = self.g16(0xE8FE);
        self.fill(si, n, tex);
    }

    /// AF76 ORs a vertex's outcode in, AF77 ANDs it.
    fn code_seen(&mut self, code: u8) {
        let v = self.g8(0xAF76) | code;
        self.s8(0xAF76, v);
        let v = self.g8(0xAF77) & code;
        self.s8(0xAF77, v);
    }

    /// 0000:8698: the list clipped by the planes its vertices are out of,
    /// up to four passes; false where nothing is left (or still out).
    fn clip(&mut self) -> bool {
        self.s8(0xAF7C, 4);
        loop {
            for (bit, at) in PLANES {
                if self.g8(0xAF76) & bit != 0 {
                    self.s16(0xAF7D, at);
                    self.clip_plane(bit);
                    if self.g8(0xAF77) != 0 {
                        return false;
                    }
                }
            }
            if self.g8(0xAF76) == 0 {
                return true;
            }
            let n = self.g8(0xAF7C).wrapping_sub(1);
            self.s8(0xAF7C, n);
            if n == 0 {
                return false;
            }
        }
    }

    /// 0000:8755: the list at AF78..AF74 clipped by the plane of outcode
    /// bit `bl` into AF7A, the lists then swapped.
    fn clip_plane(&mut self, bl: u8) {
        self.s8(0xAF77, 0xFF);
        self.s8(0xAF76, 0);
        let start = self.g16(0xAF78);
        let end = self.g16(0xAF74);
        // The first two vertices after the last (a dword at a time).
        for k in 0..6u16 {
            let d = self.g32(start.wrapping_add(4 * k));
            self.s32(end.wrapping_add(4 * k), d);
        }
        self.s16(0xAF74, end.wrapping_add(0xC));
        let mut di = self.g16(0xAF7A);
        let mut si = start;
        'next: loop {
            si = si.wrapping_add(0xC);
            let end = self.g16(0xAF74);
            if si == end {
                break;
            }
            loop {
                let code = self.g8(si.wrapping_add(6));
                if bl & code != 0 {
                    if self.g8(si.wrapping_sub(6)) & bl == 0 {
                        di = self.clip_edge(bl, si, si.wrapping_sub(0xC), di);
                    }
                    if self.g8(si.wrapping_add(0x12)) & bl == 0 {
                        di = self.clip_edge(bl, si, si.wrapping_add(0xC), di);
                    }
                    continue 'next;
                }
                for k in 0..3u16 {
                    let d = self.g32(si.wrapping_add(4 * k));
                    self.s32(di.wrapping_add(4 * k), d);
                }
                si = si.wrapping_add(0xC);
                di = di.wrapping_add(0xC);
                self.code_seen(code);
                if si == end {
                    break 'next;
                }
            }
        }
        self.s16(0xAF74, di);
        let (a, b) = (self.g16(0xAF78), self.g16(0xAF7A));
        self.s16(0xAF78, b);
        self.s16(0xAF7A, a);
    }

    /// The plane's routine (8839, 887B, 88C0, 8907): where the edge from
    /// `bp` (in) to `si` (out) meets the plane of bit `bl`, a vertex at
    /// `di`, its outcode (8950); `di` past it.
    fn clip_edge(&mut self, bl: u8, si: u16, bp: u16, di: u16) -> u16 {
        let w = |m: &Self, p: u16, k: u16| m.g16(p.wrapping_add(k));
        let (xs, ys, zs) = (w(self, si, 0), w(self, si, 2), w(self, si, 4));
        let (xb, yb, zb) = (w(self, bp, 0), w(self, bp, 2), w(self, bp, 4));
        let (cx, dx) = match bl {
            4 => (ys.wrapping_sub(yb).wrapping_sub(zs).wrapping_add(zb), zb.wrapping_sub(yb)),
            8 => (yb.wrapping_add(zb).wrapping_sub(zs).wrapping_sub(ys), zb.wrapping_add(yb)),
            2 => (zs.wrapping_sub(xs).wrapping_add(xb).wrapping_sub(zb), xb.wrapping_sub(zb)),
            _ => (xb.wrapping_add(zb).wrapping_sub(zs).wrapping_sub(xs), zb.wrapping_add(xb)),
        };
        // xor ax, ax; sar dx, 1; rcr ax, 1; idiv cx: DX << 15 over CX,
        // INT 0 (87D4) giving 7FFFh signed by DX ^ CX where it overflows.
        let t = idiv16(s(dx) << 15, cx).unwrap_or_else(|| {
            self.cs16(0x87D2, bp);
            let hi = ((s(dx) << 15) >> 16) as u16;
            let m = if (hi ^ cx) & 0x8000 != 0 { 0xFFFF } else { 0 };
            (0x7FFFu16 ^ m).wrapping_sub(m)
        });
        let lerp = |a: u16, b: u16| scaled(a.wrapping_sub(b), t).wrapping_add(b);
        let x = lerp(xs, xb);
        let y = lerp(ys, yb);
        let (x, y, z) = match bl {
            4 => (x, y, y),
            8 => (x, y, y.wrapping_neg()),
            2 => (x, y, x),
            _ => (x, y, x.wrapping_neg()),
        };
        self.s16(di, x);
        self.s16(di.wrapping_add(2), y);
        self.s16(di.wrapping_add(4), z);
        // 87F1: u, v.
        let u = lerp(w(self, si, 8), w(self, bp, 8));
        let v = lerp(w(self, si, 0xA), w(self, bp, 0xA));
        self.s16(di.wrapping_add(8), u);
        self.s16(di.wrapping_add(0xA), v);
        let code = outcode(x, y, z);
        self.s8(di.wrapping_add(6), code);
        self.code_seen(code);
        di.wrapping_add(0xC)
    }

    /// 0000:8AF2: the screen polygon of `cx` vertices (x, y, u, v) at
    /// `si` filled from the texture at segment `tex`.
    fn fill(&mut self, si: u16, cx: u16, tex: u16) {
        let remap = self.g16(0xAFEE);
        self.cs16(0x89CC, if remap != 0 { 0x8C52 } else { 0x8B80 });
        self.s16(0xAFCC, cx);
        self.s16(0xAFCA, si);
        let (mut dx, mut bx, mut p) = (0x500u16, si, si);
        for _ in 0..if cx == 0 { 0x10000 } else { cx as u32 } {
            let ax = self.g16(p.wrapping_add(2));
            if s(dx) > s(ax) {
                dx = ax;
                bx = p;
            }
            p = p.wrapping_add(8);
        }
        self.s16(0xAFD2, bx);
        self.s16(0xAFD0, bx);
        self.s16(0xAFCE, p);
        let n = self.g16(0xAFCC).wrapping_add(1);
        self.s16(0xAFCC, n);
        self.s16(0xAFD4, dx);
        if !self.edge_back() || !self.edge_on() {
            return;
        }
        // The plain span's immediates, or the remapped one's.
        let (su, sv, eu, ev, ex, sx, ustep, vstep) = if remap != 0 {
            (0x8CD8, 0x8CE1, 0x8CEA, 0x8CF3, 0x8CFC, 0x8D05, 0x8CAD, 0x8CA9)
        } else {
            (0x8BFB, 0x8C04, 0x8C0D, 0x8C16, 0x8C1F, 0x8C28, 0x8BD4, 0x8BD0)
        };
        while s(self.g16(0xAFCC)) > 0 {
            let y = self.g16(0xAFD4);
            let mut rows = self.g16(0xAFE8).wrapping_sub(y);
            let d = self.g16(0xAFD6).wrapping_sub(y);
            if s(rows) > s(d) {
                rows = d;
            }
            self.s16(0xAFEA, rows);
            self.s16(0xAFD4, y.wrapping_add(rows));
            let mut di = self.g16(ROWS.wrapping_add(y.wrapping_add(y)));
            loop {
                // 8B80 / 8C52: a row.
                let mut bp = self.g16(0xAFE6);
                let mut du = self.g16(0xAFE2);
                let x0 = self.g16(0xAFC8);
                di = di.wrapping_add(x0);
                let cx = self.g16(0xAFC4).wrapping_sub(x0);
                if s(cx) >= 0 {
                    if cx != 0 {
                        // CWD; IDIV CX: never overflows with CX > 0.
                        let u = (s(self.g16(0xAFDA).wrapping_sub(du)) / s(cx)) as u16;
                        let v = (s(self.g16(0xAFDE).wrapping_sub(bp)) / s(cx)) as u16;
                        self.cs16(ustep, u);
                        self.cs16(ustep + 0x10 + 4 * (remap != 0) as u16, u);
                        self.cs16(vstep, v);
                        self.cs16(vstep + 0x10 + 4 * (remap != 0) as u16, v);
                    }
                    let (us, vs) = (self.c16(ustep), self.c16(vstep));
                    for _ in 0..cx as u32 + 1 {
                        let bx = bp & 0xFFC0;
                        let mut al = self.m.mem.r8(tex, bx.wrapping_add(du >> 8));
                        if remap != 0 {
                            al = self.m.mem.r8(remap, al as u16);
                        }
                        self.px(di, al);
                        di = di.wrapping_add(1);
                        bp = bp.wrapping_add(vs);
                        du = du.wrapping_add(us);
                    }
                    di = di.wrapping_sub(self.g16(0xAFC4)).wrapping_add(0x13F);
                }
                // 8BF6: the edges stepped.
                for (at, imm) in [(0xAFE0, su), (0xAFE4, sv), (0xAFD8, eu), (0xAFDC, ev), (0xAFC2, ex), (0xAFC6, sx)] {
                    let v = self.g32(at).wrapping_add(self.c32(imm));
                    self.s32(at, v);
                }
                let n = self.g16(0xAFEA).wrapping_sub(1);
                self.s16(0xAFEA, n);
                if n == 0 {
                    break;
                }
            }
            // 8C34: the edge that ended walked on.
            let y = self.g16(0xAFD4);
            if self.g16(0xAFE8) == y && !self.edge_back() {
                return;
            }
            let y = self.g16(0xAFD4);
            if self.g16(0xAFD6) == y && !self.edge_on() {
                return;
            }
        }
    }

    /// 0000:89CE: the edge walked forward (AFD0) to the next vertex below
    /// the row: its end (AFD6), its x, u, v (AFC4, AFDA, AFDE) and their
    /// steps. False where a division faults (the original's end).
    fn edge_on(&mut self) -> bool {
        for at in [0xAFD8, 0xAFC2, 0xAFDC] {
            self.s16(at, 0);
        }
        let mut si = self.g16(0xAFD0);
        loop {
            let bx = si;
            si = si.wrapping_add(8);
            if si == self.g16(0xAFCE) {
                si = self.g16(0xAFCA);
            }
            let n = self.g16(0xAFCC).wrapping_sub(1);
            self.s16(0xAFCC, n);
            if s(n) <= 0 {
                return true;
            }
            let end = self.g16(si.wrapping_add(2));
            if s(end) <= s(self.g16(0xAFD4)) {
                continue;
            }
            self.s16(0xAFD6, end);
            let dy = s(end.wrapping_sub(self.g16(0xAFD4)).wrapping_neg());
            for (k, to, imm) in [(0u16, 0xAFC4, [0x8C1F, 0x8CFC]), (4, 0xAFDA, [0x8C0D, 0x8CEA]), (6, 0xAFDE, [0x8C16, 0x8CF3])] {
                let a = self.g16(bx.wrapping_add(k));
                self.s16(to, a);
                let diff = a.wrapping_sub(self.g16(si.wrapping_add(k)));
                let Some(step) = idiv32(((diff as u32) << 16) as i32, dy) else {
                    self.gap("8A16: an edge's division overflows (the original's fault)");
                    return false;
                };
                for at in imm {
                    self.cs32(at, step);
                }
            }
            self.s16(0xAFD0, si);
            return true;
        }
    }

    /// 0000:8A60: the edge walked back (AFD2) to the previous vertex below
    /// the row: its end (AFE8), its x, u, v (AFC8, AFE2, AFE6) and their
    /// steps. False where a division faults.
    fn edge_back(&mut self) -> bool {
        for at in [0xAFE0, 0xAFC6, 0xAFE4] {
            self.s16(at, 0);
        }
        let mut si = self.g16(0xAFD2);
        loop {
            let bx = si;
            if si == self.g16(0xAFCA) {
                si = self.g16(0xAFCE);
            }
            si = si.wrapping_sub(8);
            let n = self.g16(0xAFCC).wrapping_sub(1);
            self.s16(0xAFCC, n);
            if s(n) <= 0 {
                return true;
            }
            let end = self.g16(si.wrapping_add(2));
            if s(end) <= s(self.g16(0xAFD4)) {
                continue;
            }
            self.s16(0xAFE8, end);
            let dy = s(end.wrapping_sub(self.g16(0xAFD4)).wrapping_neg());
            for (k, to, imm) in [(0u16, 0xAFC8, [0x8C28, 0x8D05]), (4, 0xAFE2, [0x8BFB, 0x8CD8]), (6, 0xAFE6, [0x8C04, 0x8CE1])] {
                let a = self.g16(bx.wrapping_add(k));
                self.s16(to, a);
                let diff = a.wrapping_sub(self.g16(si.wrapping_add(k)));
                let Some(step) = idiv32(((diff as u32) << 16) as i32, dy) else {
                    self.gap("8AA8: an edge's division overflows (the original's fault)");
                    return false;
                };
                for at in imm {
                    self.cs32(at, step);
                }
            }
            self.s16(0xAFD2, si);
            return true;
        }
    }
}

/// 0000:8950: a clip-space vertex's outcode: 1 left of -z, 2 right of z,
/// 4 above z, 8 below -z; 80h with them where z <= 0 (as the original's
/// branches set them).
fn outcode(x: u16, y: u16, z: u16) -> u8 {
    let (x, y, zs) = (s(x), s(y), s(z));
    let nz = s(z.wrapping_neg());
    if zs > 0 {
        let side = |a: u8, b: u8, c: u8| if y > zs { a } else if y < nz { b } else { c };
        return if x < nz {
            side(5, 9, 1)
        } else if x > zs {
            side(6, 0xA, 2)
        } else {
            side(4, 8, 0)
        };
    }
    if x >= nz {
        return if y > zs { if y < nz { 0x8E } else { 0x86 } } else { 0x8A };
    }
    if x > zs {
        if y > zs { if y < nz { 0x8F } else { 0x87 } } else { 0x8B }
    } else if y > zs {
        if y < nz { 0x8D } else { 0x85 }
    } else {
        0x89
    }
}
