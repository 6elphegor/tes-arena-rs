//! The 3D view drawn as the original's renderer draws it, into its 320 x
//! 200 page: a routine-by-routine port of the frame's drawing stages
//! (the world loop's BA4B 3..8, 0000:A322..A393).
//!
//! The call tree, as the frame runs it:
//!
//! - 0000:D4B4 (`draw_half`), twice: the floor's half (140h), then the
//!   ceiling's (FEC0h) unless outdoors or C398 bit 40h.
//!   - DEA8, DFA2: the half filled with its colour (9A72, a rectangle of
//!     [9288]); water or lava (CB5B) animate instead (not ported).
//!   - the floor's half only: the walk (C923, below) with C398 bit 20h
//!     clear, which draws the chasms' sides (D0A1) and the flats lying
//!     above the floor (CFBA), then B358 (the deferred list) and B4E8.
//!   - D5BB: the textured rows. DC67 makes each row's light, eight steps
//!     across (DD85: the darkest of the lights' tables at CCD2), then
//!     each row is drawn by D78C from the row table at CD82 (the floor)
//!     or CD7E (the ceiling): two pixels at a time, the texture (64 x 64,
//!     the map's floor word naming its segment) through the shade table
//!     at [9030]:bx.
//! - 0000:C923 (`walk`): the squares within C394 of the player, ring by
//!   ring from the farthest (C980), each (C9FF) building its faces (CB20
//!   with D27A: a wall's four corners from D74A by the side; CC63 picks
//!   the kind: walls, doors 0Bh, raised floors, kinds 8 and 9, 0Ah, 0Dh)
//!   and drawing them at once (C398 bit 20h set: AC13 turns them to the
//!   eye (93BC), clips them to the near plane (B049), projects them
//!   (ADD4, E032), lights them (B649) and draws them (B210 -> AB37:
//!   a column at a time, AE59 stepping the columns and B835 drawing one,
//!   two shade tables alternating by the pixel's parity; BBB6 for a face
//!   nearer than 1F4h, stepping by the texture's columns instead); the
//!   flats on the square (CFBA -> B2A7 -> D18E -> C0F1) and the actors
//!   on it (C032 -> C0CE -> C0F1) scaled by 78A8.
//! - CBFF (`poly`): a square's cap, a polygon of the TOP texture, through
//!   the generic polygon drawer (7C51: cut, clipped, projected, filled).
//! - B4F5, B358: the deferred lists (BA80 clear), not ported: the 3D mode
//!   draws at once.
//!
//! The port runs on a copy of the machine taken at the start of the
//! drawing stages (`draw`), so the game's state is the engine's own: what
//! the drawing changes (the doors and changed squares it steps, the
//! hooks it calls for clicks and creatures, its own variables) stays in
//! the copy. Self-modified code's variables (CS:AA9E, CS:D752..) are the
//! copy's bytes at segment 0, where the port keeps the image.

use crate::machine::Machine;
use crate::mem::DS;

mod poly;
mod sky;
pub use sky::image_page;

/// The rows' offsets on the page (A6A0, a word a row).
const ROWS: u16 = 0xA6A0;
/// The view's window: left, top, right, bottom (A914..A91B).
const LEFT: u16 = 0xA914;
const TOP: u16 = 0xA916;
const RIGHT: u16 = 0xA918;
const BOTTOM: u16 = 0xA91A;

fn s(v: u16) -> i32 {
    v as i16 as i32
}

/// |v| of a word, as `jns` / `neg` leave it.
fn abs16(v: u16) -> u16 {
    if (v as i16) < 0 {
        v.wrapping_neg()
    } else {
        v
    }
}

/// 16-bit `imul` then `idiv`: (a * b) / c, the quotient's low word.
/// 0000:93BC (`Soft::rot`) on the machine's sine and cosine tables.
fn rot(m: &Machine, di: u16, ax: u16, cx: u16) -> (u16, u16) {
    let di = di.wrapping_add(di);
    let (ax, cx) = (ax.wrapping_add(ax), cx.wrapping_add(cx));
    let c = s(m.mem.r16(DS, 0xB13Cu16.wrapping_add(di)));
    let sn = m.mem.r16(DS, 0xB03Cu16.wrapping_add(di));
    let nsin = if sn == 0x8000 { 0x7FFF } else { s(sn.wrapping_neg()) };
    let hi = |p: i32| (p >> 16) as u16;
    let x = hi(s(ax) * c).wrapping_add(hi(s(cx) * nsin));
    let y = hi(s(ax) * s(sn)).wrapping_add(hi(s(cx) * c));
    (x, y)
}

fn muldiv(a: u16, b: u16, c: u16) -> u16 {
    let c = s(c);
    if c == 0 {
        return 0;
    }
    ((s(a) * s(b)) / c) as u16
}

/// The drawing's state: the machine's copy and the page.
pub struct Soft {
    pub m: Machine,
    pub page: Vec<u8>,
    /// Paths met that aren't ported, by the original's address.
    pub gaps: Vec<&'static str>,
    /// The .LGT files the fog's steps load (`Lgts`).
    lgts: Lgts,
    /// The light table the copy made (`setup`, CCC2), made again in place
    /// where a fog's step makes it again.
    table: u16,
    /// The creatures' flats reached (CF96), for the hook CD56 the
    /// original calls there: the square's centre (x, y), the flat's
    /// number, its map word's segment and offset.
    pub creatures: Vec<Creature>,
    /// 78A8's local [BP-14h] (bits a pixel not 8) as the last row
    /// compiled by 7A2B left it on the stack: 9546 doesn't set it, and
    /// the rows' start reads it.
    odd: u8,
    /// The actors' and projectiles' records whose animation the drawing
    /// stepped (C23A), in order (`Walked::stepped`).
    stepped: Vec<u16>,
}

/// What the walk leaves the game (`draw`'s fourth): the objects it
/// stepped (0000:C23A, for `Machine::drawn_step` on the game's memory)
/// and DGROUP's words and bytes it left that the game reads after the
/// drawing, by offset: the square and the door's span under the cursor
/// (CB92, CCB2..CCB9, CCC8), the hit test's find (CB5E, the record at
/// 3D96 and the scratch flat's +12h, 0000:C4BB and the hook CD3A), the
/// changed squares' steps (C302.., CB59, CB5A, D73C).
#[derive(Clone, Debug, Default)]
pub struct Walked {
    pub stepped: Vec<u16>,
    pub left: Vec<(u16, u8)>,
}

/// A creature's flat the drawing reached (`Soft::creatures`).
pub type Creature = (u16, u16, u8, u16, u16);

/// A DGROUP byte the drawing wrote on its copy: offset, before, after.
pub type Wrote = (u16, u8, u8);

/// An EMS byte the drawing changed on its copy (0000:5652's frames
/// unpacked into their banks): the bank times 10000h plus the offset,
/// before, after.
pub type EmsWrote = (u32, u8, u8);

/// The EMS bytes `after` (the drawing's copy) holds otherwise than
/// `before` (the machine): of the banks either has mapped or holds apart.
fn ems_changes(before: &Machine, after: &Machine) -> Vec<EmsWrote> {
    let mapped = |m: &Machine| (m.mem.r16(DS, crate::level::var::EMS_MAPPED) >> 2) as usize;
    let n = before.ems.len().max(after.ems.len());
    let mut out = Vec::new();
    for b in 0..n {
        let same = match (before.ems.get(b), after.ems.get(b)) {
            (Some(x), Some(y)) => std::sync::Arc::ptr_eq(x, y),
            (None, None) => true,
            _ => false,
        };
        if same && b != mapped(before) && b != mapped(after) {
            continue;
        }
        let (x, y) = (before.ems_bank(b as u16), after.ems_bank(b as u16));
        for (i, (&p, &q)) in x.iter().zip(&y).enumerate() {
            if p != q {
                out.push(((b as u32) << 16 | i as u32, p, q));
            }
        }
    }
    out
}

/// NORMAL.LGT's and FOG.LGT's bytes, for the fog's steps the drawing's
/// sky hook may take (2294:0BEF's load, 0C36's and 0B8A's tables made
/// again), which the original does with its files in the middle of the
/// frame.
#[derive(Clone, Default)]
pub struct Lgts {
    pub normal: Option<std::rc::Rc<[u8]>>,
    pub fog: Option<std::rc::Rc<[u8]>>,
}

impl Lgts {
    pub fn of(files: &crate::bsa::Files) -> Self {
        Lgts { normal: files.data(b"NORMAL.LGT"), fog: files.data(b"FOG.LGT") }
    }
    /// The one named at DS:`name` (NORMAL.LGT 1A6Dh, FOG.LGT 1A78h).
    fn named(&self, name: u16) -> Option<&[u8]> {
        match name {
            crate::weather::NORMAL_LGT => self.normal.as_deref(),
            crate::weather::FOG_LGT => self.fog.as_deref(),
            _ => None,
        }
    }
}

/// The view drawn from the machine's state at the start of the frame's
/// drawing stages (before 0000:A322): the page, the pixels it drew
/// written over `under` (the last page), and the paths not ported. Of
/// what the drawing changes, the water's step (D813), which only it
/// keeps, is put back in `m`.
/// The third: DGROUP's offsets the drawing wrote on its copy (which the
/// original's, drawing on the machine itself, leaves there), each with its
/// byte before and after (the same where it wrote the machine's own); the
/// fourth
/// the creatures' flats it reached (`Soft::creatures`), in order; the
/// sixth the EMS bytes it changed on its copy (`EmsWrote`).
#[allow(clippy::type_complexity)]
pub fn draw(m: &mut Machine, under: &[u8], lgts: &Lgts) -> (Vec<u8>, Vec<&'static str>, Vec<Wrote>, Vec<Creature>, Walked, Vec<EmsWrote>) {
    if let Ok(p) = std::env::var("ARENA_RENDER_DUMP") {
        // The first frame's (a file there already is kept).
        if !std::path::Path::new(&p).exists() {
            let _ = std::fs::write(p, m.mem.segment(DS));
        }
    }
    let mut r = Soft { m: m.clone(), page: under.to_vec(), gaps: Vec::new(), lgts: lgts.clone(), table: 0, creatures: Vec::new(), odd: 0, stepped: Vec::new() };
    r.m.mem.track();
    r.page.resize(0x10000, 0);
    r.stages();
    r.page.truncate(64000);
    let wrote = r.m.mem.written_offsets().into_iter().map(|o| (o, m.mem.r8(DS, o), r.m.mem.r8(DS, o))).collect();
    m.mem.w8(DS, 0xD813, r.m.mem.r8(DS, 0xD813));
    if r.gaps.is_empty() {
        // The player's square's floor (CB5B: 0 dry, 1 water, 2 lava, as
        // the walk finds it, 0000:CDxx), which the game reads after: the
        // textures' copy put back (A483) and a click's wall.
        m.mem.w8(DS, 0xCB5B, r.m.mem.r8(DS, 0xCB5B));
    }
    let mut left = Vec::new();
    for (from, to) in [(0xCB92, 0xCB94), (0xCCB2, 0xCCBA), (0xCCC8, 0xCCCA), (0xCB5E, 0xCB60), (crate::talk::var::RECORD, crate::talk::var::RECORD + 0x1C), (0xD7F0, 0xD7F2), (0xC302, 0xC322), (0xCB59, 0xCB5B), (0xD73C, 0xD73D)] {
        for o in from..to {
            left.push((o, r.m.mem.r8(DS, o)));
        }
    }
    let walked = Walked { stepped: r.stepped, left };
    let ems = ems_changes(m, &r.m);
    (r.page, r.gaps, wrote, r.creatures, walked, ems)
}

/// 0000:CBFF alone (tests/capdraw.rs): the cap at height `ax` wound by
/// `cx` drawn on `m` into `page` (64 KB, the page's segment's bytes), as
/// the 3D mode draws it; the machine and the page after, and the paths
/// not ported it reached.
pub fn cap_alone(m: Machine, page: Vec<u8>, ax: u16, cx: u16) -> (Machine, Vec<u8>, Vec<&'static str>) {
    let mut r = Soft { m, page, gaps: Vec::new(), lgts: Lgts::default(), table: 0, creatures: Vec::new(), odd: 0, stepped: Vec::new() };
    r.page.resize(0x10000, 0);
    r.cap_draw(0, ax, cx);
    (r.m, r.page, r.gaps)
}

/// 0000:78A8 alone (tests/scaler.rs), its rows compiled by the routine
/// CS:789E names on `m`: the arguments as pushed (x, y, the size drawn,
/// the shade's offset and segment, the size, bits a pixel, the flags,
/// the source's offset and segment, the colours' offset and segment),
/// `odd` the stack's [BP-14h]; onto `page` (the page's segment, 64 KB).
/// None where it would read the code segment's colours (not ported).
pub fn scaler_alone(m: &Machine, page: &mut [u8], a: [u16; 14], odd: u8) -> Option<()> {
    let mut odd = odd;
    blit_rows(m, page, a[0], a[1], a[2], a[3], (a[5], a[4]), a[6], a[7], a[9], (a[11], a[10]), a[8] as u8, (a[13], a[12]), &mut odd).then_some(())
}

impl Soft {
    fn g16(&self, o: u16) -> u16 {
        self.m.mem.r16(DS, o)
    }
    fn s16(&mut self, o: u16, v: u16) {
        self.m.mem.w16(DS, o, v)
    }
    fn g8(&self, o: u16) -> u8 {
        self.m.mem.r8(DS, o)
    }
    fn s8(&mut self, o: u16, v: u8) {
        self.m.mem.w8(DS, o, v)
    }
    fn g32(&self, o: u16) -> u32 {
        self.m.mem.r32(DS, o)
    }
    fn s32(&mut self, o: u16, v: u32) {
        self.m.mem.w32(DS, o, v)
    }
    /// The code segment's words (self-modified variables).
    fn c16(&self, o: u16) -> u16 {
        self.m.mem.r16(0, o)
    }
    fn cs16(&mut self, o: u16, v: u16) {
        self.m.mem.w16(0, o, v)
    }
    fn c32(&self, o: u16) -> u32 {
        self.m.mem.r32(0, o)
    }
    fn cs32(&mut self, o: u16, v: u32) {
        self.m.mem.w32(0, o, v)
    }
    fn or16(&mut self, o: u16, v: u16) {
        let x = self.g16(o) | v;
        self.s16(o, x);
    }
    fn and16(&mut self, o: u16, v: u16) {
        let x = self.g16(o) & v;
        self.s16(o, x);
    }
    fn gap(&mut self, at: &'static str) {
        if !self.gaps.contains(&at) {
            self.gaps.push(at);
        }
    }
    fn px(&mut self, off: u16, v: u8) {
        self.page[off as usize] = v;
    }
    /// The page offset of row `y` (A6A0's word).
    fn row(&self, y: u16) -> u16 {
        self.g16(ROWS.wrapping_add(y.wrapping_add(y)))
    }

    /// The frame's drawing stages, A322..A393, as the 3D mode runs them
    /// (the hooks CD66 none, the sky's CD1A outdoors: not ported).
    fn stages(&mut self) {
        self.setup();
        self.s8(0xBA4B, 3);
        self.draw_half(0x140);
        if self.g8(0xCB5C) != 0 {
            // A33E: the sky's hook (its fog step fills the view).
            self.sky_hook();
        } else {
            self.s8(0xBA4B, 4);
            if self.g16(0xC398) & 0x44 == 0 {
                self.draw_half(0xFEC0);
            }
        }
        self.s8(0xBA4B, 6);
        let reach = self.g16(0xC394);
        self.s16(0xC394, reach << 1);
        if (self.g16(0xC3D2) as i16) >= 0 {
            self.s16(0xC394, (reach << 1) >> 1);
            self.or16(0xC398, 0x20);
        }
        self.s8(0xBA4B, 7);
        self.walk();
        if self.g16(0xBAC8) > 1 {
            self.gap("B4F5: the deferred list's overlaps");
        }
        self.s8(0xBA4B, 8);
        self.deferred();
        self.s16(0xC394, reach);
    }

    /// 0000:B358: the deferred list drawn (BAC8 entries); empty in the
    /// 3D mode, which draws at once (BA80).
    fn deferred(&mut self) {
        if self.g16(0xBAC8) != 0 {
            self.gap("B358: the deferred list");
            return;
        }
        self.s16(0xBB06, 0);
    }

    /// 0000:9A72: a rectangle of [9288] at (`x`, `y`), `w` wide, `h`
    /// high (77CC a row).
    fn fill_rect(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let mut di = self.row(y).wrapping_add(x);
        let al = self.g8(0x9288);
        let mut n = h;
        loop {
            let mut d = di;
            for _ in 0..w {
                self.px(d, al);
                d = d.wrapping_add(1);
            }
            di = di.wrapping_add(0x140);
            n = n.wrapping_sub(1);
            if n == 0 {
                break;
            }
        }
    }

    /// 0000:D4B4 with its argument: a half of the view (`half` 140h the
    /// floor's, FEC0h the ceiling's).
    fn draw_half(&mut self, half: u16) {
        let neg = (half as i16) < 0;
        if self.g16(0xC398) & 0x40 != 0 {
            let c = self.g8(0xC3C6);
            self.s8(0x9288, c);
            let (l, r, b, h) = (self.g16(LEFT), self.g16(RIGHT), self.g16(BOTTOM), self.g16(0xC3D8));
            self.fill_rect(l, h, r.wrapping_sub(l).wrapping_add(1), b.wrapping_sub(h).wrapping_add(1));
            self.half_fill();
        } else {
            if !neg {
                self.half_fill();
            }
            if self.g16(0xC398) & 4 != 0 {
                // D502: outdoors, the sky.
                self.sky();
            } else {
                if neg {
                    return self.half_ceiling(half);
                }
                self.half_under();
            }
        }
        // D531.
        if self.g16(0xC398) & 0x4000 != 0 {
            return self.half_top();
        }
        if neg {
            return self.half_ceiling(half);
        }
        let reach = self.g16(0xC394);
        self.s16(0xC394, reach << 1);
        let mut walk = true;
        if (self.g16(0xC3D2) as i16) < 0 {
            if self.g16(0xC398) & 0x40 == 0 {
                let v = self.g16(0xC394) >> 2;
                self.s16(0xC394, v);
                self.or16(0xC398, 0x20);
            } else {
                walk = false;
            }
        }
        if walk {
            self.walk();
            self.deferred();
        }
        self.s16(0xC394, reach);
        self.s16(0xBAC8, 0);
        self.s16(0xCD7A, 0);
        let bank = self.g8(0xC8F0) as u16;
        self.m.ems_map(bank);
        if self.g16(0xC398) & 0x40 != 0 || (self.g16(0xC3D2) as i16) > 0 {
            self.rows(half);
        }
    }

    /// D584: the ceiling's half.
    fn half_ceiling(&mut self, half: u16) {
        if self.g16(0xC39A) & 8 != 0 {
            return self.half_top();
        }
        let bank = self.g8(0xC8F0) as u16;
        self.m.ems_map(bank);
        if (self.g16(0xC3D2) as i16) > 0 {
            self.rows(half);
        }
    }

    /// D59C: the upper half in [C3C7].
    fn half_top(&mut self) {
        let c = self.g8(0xC3C7);
        self.s8(0x9288, c);
        let (l, r, t, h) = (self.g16(LEFT), self.g16(RIGHT), self.g16(TOP), self.g16(0xC3D8));
        self.fill_rect(l, t, r.wrapping_sub(l).wrapping_add(1), h.wrapping_sub(t).wrapping_add(1));
    }

    /// 0000:DEA8: the lower half in [C3C6] (CB5B 0), or the water's or
    /// lava's frames (not ported).
    fn half_fill(&mut self) {
        if self.g8(0xCB5B) != 0 {
            return self.water();
        }
        let c = self.g8(0xC3C6);
        self.s8(0x9288, c);
        let (l, r, b, h) = (self.g16(LEFT), self.g16(RIGHT), self.g16(BOTTOM), self.g16(0xC3D8));
        self.fill_rect(l, h, r.wrapping_sub(l).wrapping_add(1), b.wrapping_sub(h).wrapping_add(1));
    }

    /// 0000:DEAF: water or lava (C8F2 their first bank; FFh none): the
    /// frame counter D813 stepped (0..4), its half the bank after C8F2,
    /// its odd steps the second half of the bank (from 8000h), the rows
    /// from the horizon down copied from 300h before that.
    fn water(&mut self) {
        let first = self.g8(0xC8F2);
        if first & 0x80 != 0 {
            return;
        }
        let step = (self.g8(0xD813) as u16 + 1) % 5;
        self.s8(0xD813, step as u8);
        self.m.ems_map((step >> 1) + first as u16);
        let si: u16 = if step & 1 != 0 { 0x8000u16.wrapping_sub(0x300) } else { 0 };
        let rows = self.g16(BOTTOM).wrapping_sub(self.g16(0xC3D8));
        let n = (0x50u16 * (rows & 0xFF)) as u32 * 4;
        let mut di = self.row(self.g16(0xC3D8));
        let seg = self.g16(0x9030);
        let mut si = si;
        for _ in 0..n {
            let b = self.m.mem.r8(seg, si);
            self.px(di, b);
            di = di.wrapping_add(1);
            si = si.wrapping_add(1);
        }
    }

    /// 0000:DFA2: with C398 bit 4000h or the eye below the floor, a half
    /// in [C3C6].
    fn half_under(&mut self) {
        let (bx, dx) = if self.g16(0xC398) & 0x4000 != 0 {
            (self.g16(0xC3D8), self.g16(BOTTOM))
        } else if (self.g16(0xC3D2) as i16) < 0 {
            (self.g16(TOP), self.g16(0xC3D8))
        } else {
            return;
        };
        let c = self.g8(0xC3C6);
        self.s8(0x9288, c);
        let (l, r) = (self.g16(LEFT), self.g16(RIGHT));
        self.fill_rect(l, bx, r.wrapping_sub(l).wrapping_add(1), dx.wrapping_sub(bx).wrapping_add(1));
    }

    /// 0000:D5BB: the half's textured rows.
    fn rows(&mut self, half: u16) {
        let neg = (half as i16) < 0;
        let mut ax = 0u16;
        if self.g16(0xC290) != 0 || self.g16(0xC398) & 0x8004 == 0 {
            let di = self.g16(0xC3D8).wrapping_sub(8).wrapping_sub(self.g16(0xC290));
            let mut di = self.row(di);
            let n = 0x500u16.wrapping_add(self.g16(0xC292));
            let c = self.g8(0xC3C6);
            for _ in 0..n as u32 * 4 {
                self.px(di, c);
                di = di.wrapping_add(1);
            }
            ax = 8;
        }
        self.s16(0xD83C, ax);
        self.cs16(0xD77E, half);
        let seg = self.g16(0x9030);
        self.cs16(0xD780, seg);
        let h = self.g16(0xC3D8);
        let skip_light = neg && self.g16(0xC3D2) == 0x32;
        if !skip_light {
            let n = if neg { self.g16(TOP).wrapping_sub(h).wrapping_neg().wrapping_add(1) } else { self.g16(BOTTOM).wrapping_sub(h) };
            self.row_lights(n, neg);
        }
        let w = self.g16(RIGHT).wrapping_sub(self.g16(LEFT)).wrapping_add(1);
        self.cs16(0xD762, w >> 3);
        let mut di = h;
        let (n, table) = if neg {
            (self.g16(TOP).wrapping_sub(h).wrapping_neg().wrapping_add(1), 0xCD7E)
        } else {
            (self.g16(BOTTOM).wrapping_sub(h), 0xCD82)
        };
        let (mut si, es) = (self.g16(table), self.g16(table + 2));
        let cx = abs16(self.g16(0xD83C));
        let mut count = n.wrapping_sub(cx);
        self.s16(0xCD86, cx << 5);
        let v = self.g16(0xC3D0) >> 1;
        self.cs16(0xD778, v);
        let v = self.g16(0xC3D4) >> 1;
        self.cs16(0xD77C, v);
        self.cs16(0xD78A, 0);
        let ax = self.g16(0xD83C);
        di = if neg { di.wrapping_sub(ax) } else { di.wrapping_add(ax) };
        si = si.wrapping_add(ax << 3);
        di = self.row(di).wrapping_add(self.g16(LEFT));
        let fs = self.g16(0xCCD0);
        loop {
            let r = |o: u16| self.m.mem.r16(es, si.wrapping_add(o));
            let (ax, bx, cx, dx) = (r(0), r(2), r(4), r(6));
            let cx = cx.wrapping_sub(ax);
            let dy = dx.wrapping_sub(bx);
            let esi = (bx as u32) << 16;
            let x0 = (ax as u32) << 16;
            self.cs16(0xD752, w);
            let xstep = (((cx as u32) << 16) as i32 / w as i32) as u32;
            let ystep = (((dy as u32) << 16) as i32 / w as i32) as u32;
            self.floor_row(x0, esi, xstep, ystep, di, fs);
            let v = self.c16(0xD78A) ^ 0xC0;
            self.cs16(0xD78A, v);
            di = di.wrapping_add(self.c16(0xD77E));
            si = si.wrapping_add(8);
            count = count.wrapping_sub(1);
            if count == 0 {
                break;
            }
        }
    }

    /// 0000:DC67: the rows' lights, `n` rows from the row table at CD82
    /// (the ceiling's CD7E), into CD88:0: each row's light at its start
    /// and at eight steps across, as (start, step) pairs.
    fn row_lights(&mut self, n: u16, neg: bool) {
        if self.g16(0xC398) & 0xC000 != 0 {
            return;
        }
        let d = self.g16(RIGHT).wrapping_sub(self.g16(LEFT)) >> 4;
        self.s16(0xD82A, d);
        let table = if neg { 0xCD7E } else { 0xCD82 };
        let (mut si, gs) = (self.g16(table), self.g16(table + 2));
        self.s16(0xCD86, 0);
        let es = self.g16(0xCD88);
        let mut rows = n;
        loop {
            let r = |o: u16| self.m.mem.r16(gs, si.wrapping_add(o));
            let ax = r(0).wrapping_add(r(0));
            let bx = r(2).wrapping_add(r(2));
            let cx = r(4).wrapping_add(r(4)).wrapping_sub(ax);
            let dx = r(6).wrapping_add(r(6)).wrapping_sub(bx);
            let ecx = if (cx as i16) < 0 { cx as u32 | 0xFFFF_0000 } else { cx as u32 };
            let edx = if (dx as i16) < 0 { dx as u32 | 0xFFFF_0000 } else { dx as u32 };
            self.s32(0xD82C, ecx);
            self.s32(0xD834, 0);
            self.s32(0xD830, edx);
            self.s32(0xD838, 0);
            let (px, py) = (self.g16(0xC3D0).wrapping_add(ax), self.g16(0xC3D4).wrapping_add(bx));
            self.light_at(px, py);
            let l = self.g16(0xA836);
            self.s16(0xA838, l);
            let at = self.g16(0xCD86);
            self.m.mem.w16(es, at, l);
            self.s16(0xCD86, at.wrapping_add(2));
            for _ in 0..8 {
                let c = self.g32(0xD834).wrapping_add(self.g32(0xD82C));
                self.s32(0xD834, c);
                let c = (c >> 3) as u16;
                let e = self.g32(0xD838).wrapping_add(self.g32(0xD830));
                self.s32(0xD838, e);
                let e = (e >> 3) as u16;
                let px = self.g16(0xC3D0).wrapping_add(ax).wrapping_add(c);
                let py = self.g16(0xC3D4).wrapping_add(bx).wrapping_add(e);
                self.light_at(px, py);
                let l = self.g16(0xA836);
                let dl = l.wrapping_sub(self.g16(0xA838));
                let q = if self.g16(0xD82A) == 0 { 0 } else { (s(dl) / s(self.g16(0xD82A))) as u16 };
                let at = self.g16(0xCD86);
                self.m.mem.w16(es, at, q);
                self.m.mem.w16(es, at.wrapping_add(2), l);
                self.s16(0xA838, l);
                self.s16(0xCD86, at.wrapping_add(4));
            }
            let at = self.g16(0xCD86).wrapping_sub(2);
            self.s16(0xCD86, at);
            si = si.wrapping_add(8);
            rows = rows.wrapping_sub(1);
            if rows == 0 {
                break;
            }
        }
    }

    /// 0000:DD85: the light at (`x`, `y`) into A836: the darkest the
    /// lights' tables give (CCEC unlit), at most the table's last.
    fn light_at(&mut self, x: u16, y: u16) {
        let v = self.g16(0xCCEC);
        self.s16(0xA836, v);
        let n = self.g16(0xC3DC);
        self.light_list(x, y, 0xC3DE, n);
        let n = self.g16(0xBC8C);
        if n != 0 {
            self.light_list(x, y, 0xBBCC, n);
        }
    }

    /// 0000:DDAA: one list of lights (x, y, radius, six bytes each).
    fn light_list(&mut self, x: u16, y: u16, mut bx: u16, n: u16) {
        let mut cx = n;
        loop {
            let mut a = abs16(x.wrapping_sub(self.g16(bx)));
            let mut b = abs16(y.wrapping_sub(self.g16(bx + 2)));
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            let d = (a >> 2).wrapping_add(b);
            let mut r = self.g16(bx + 4);
            if d < r {
                r >>= 1;
                if d < r {
                    let v = self.g16(0xCCD2);
                    self.s16(0xA836, v);
                    break;
                }
                let d = d - r;
                let q = if r == 0 { 0xFFFF } else { ((d as u32 * 0xD) / r as u32) as u16 };
                if q <= 0xD {
                    let v = self.g16(0xCCD2u16.wrapping_add(q + q));
                    if self.g16(0xA836) > v {
                        self.s16(0xA836, v);
                    }
                }
            }
            bx = bx.wrapping_add(6);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
        let i = self.g16(0xC8E7);
        let v = self.g16(0xCCD2u16.wrapping_add(i.wrapping_add(i)));
        if self.g16(0xA836) > v {
            self.s16(0xA836, v);
        }
    }

    /// 0000:D7AD: `floor_row` in full daylight (C398 bit 8000h): across
    /// the view a pair of pixels at a time, each its square's floor
    /// texture (the map's word at fs, its high byte the segment) unless
    /// that is CCCE or more, unshaded.
    fn floor_row_day(&mut self, x: u32, y: u32, xstep: u32, ystep: u32, mut di: u16, fs: u16) {
        let thr = self.g16(0xCCCE);
        let mut pairs = self.g16(RIGHT).wrapping_sub(self.g16(LEFT)).wrapping_add(1) >> 1;
        let (xs2, ys2) = (xstep << 1, ystep << 1);
        let (mut px, mut py) = (x, y);
        loop {
            px = px.wrapping_add(xs2);
            py = py.wrapping_add(ys2);
            let (u, v) = ((px >> 16) as u16, (py >> 16) as u16);
            let bx = ((((v as i16) >> 6) as u16) << 8).wrapping_add((((u as i16) >> 6) as u16).wrapping_add(((u as i16) >> 6) as u16)) & 0x7FFF;
            let tex = self.m.mem.r16(fs, bx) & 0xFF00;
            if tex < thr {
                let al = self.m.mem.r8(tex, ((v & 0x3F) << 6).wrapping_add(u & 0x3F));
                self.px(di, al);
                self.px(di.wrapping_add(1), al);
            }
            di = di.wrapping_add(2);
            pairs = pairs.wrapping_sub(1);
            if pairs == 0 {
                break;
            }
        }
    }

    /// 0000:D78C: one row of the floor (`D77E` positive) or the ceiling,
    /// from (x, y) in 16.16 by the steps, at the page's `di`.
    fn floor_row(&mut self, x: u32, y: u32, xstep: u32, ystep: u32, mut di: u16, fs: u16) {
        let x = x.wrapping_add(self.c32(0xD776));
        let y = y.wrapping_add(self.c32(0xD77A));
        let neg = (self.c16(0xD77E) as i16) < 0;
        if !neg && self.g16(0xC398) & 0x8000 != 0 {
            return self.floor_row_day(x, y, xstep, ystep, di, fs);
        }
        let (xs2, ys2) = (xstep << 1, ystep << 1);
        let thr = self.g16(0xCCCE);
        let d762 = self.c32(0xD762);
        // D865..D878: the dark run's x step is EDX times D762 after DX
        // took CCCE (the original's bug): the step's high word, CCCE low.
        // CCCE is the EMS frame's segment plus C00h (A49F), so where the
        // ceiling has dark runs, its texels after them depend on where
        // the frame is (the harness's at F000h, the view's on its heap).
        let xseg = (((xstep & 0xFFFF_0000) | thr as u32) as i32 as i64 * d762 as i32 as i64) as u32;
        let yseg = (ystep as i32 as i64 * d762 as i32 as i64) as u32;
        self.cs32(0xD754, xseg);
        self.cs32(0xD758, yseg);
        let (mut edx, mut ebp) = (x, y);
        let pairs = (self.c16(0xD762) as u8) >> 1;
        let seg = self.c16(0xD762);
        let tables = self.g16(0x9030);
        if neg {
            if self.g16(0xC398) & 0x8000 != 0 {
                // D953: in full daylight, the ceiling's texels themselves,
                // a pair of pixels each, D762 at a time.
                let tex = self.g16(0xCB64);
                let mut left = self.c16(0xD752);
                loop {
                    let mut ah = pairs;
                    loop {
                        edx = edx.wrapping_add(xs2);
                        ebp = ebp.wrapping_add(ys2);
                        let (u, v) = ((edx >> 16) as u16, (ebp >> 16) as u16);
                        let al = self.m.mem.r8(tex, ((v & 0x3F) << 6).wrapping_add(u & 0x3F));
                        self.px(di, al);
                        self.px(di.wrapping_add(1), al);
                        di = di.wrapping_add(2);
                        ah = ah.wrapping_sub(1);
                        if ah == 0 {
                            break;
                        }
                    }
                    left = left.wrapping_sub(seg);
                    self.cs16(0xD752, left);
                    if left == 0 {
                        break;
                    }
                }
                return;
            }
            let (mut si, sseg) = (self.g16(0xCD86), self.g16(0xCD88));
            let mut left = self.c16(0xD752);
            let mut reload = true;
            let mut cx = 0;
            loop {
                if reload {
                    cx = self.g16(0xCCEC);
                }
                reload = true;
                let ax = self.m.mem.r16(sseg, si);
                let dark = ax == cx && (ax == self.m.mem.r16(sseg, si.wrapping_add(4)) || self.m.mem.r16(sseg, si.wrapping_add(2)) == 0);
                if dark {
                    si = si.wrapping_add(4);
                    for _ in 0..(seg >> 2) as u32 * 4 {
                        self.px(di, 0x70);
                        di = di.wrapping_add(1);
                    }
                    edx = edx.wrapping_add(self.c32(0xD754));
                    ebp = ebp.wrapping_add(self.c32(0xD758));
                    left = left.wrapping_sub(seg);
                    self.cs16(0xD752, left);
                    if left == 0 {
                        break;
                    }
                    // D9F7: on at D9BE, CX kept.
                    cx = self.g16(0xCCEC);
                    reload = false;
                    continue;
                }
                let mut bx = ax;
                let inc = self.m.mem.r16(sseg, si.wrapping_add(2));
                si = si.wrapping_add(4);
                self.s16(0xCD86, si);
                let tex = self.g16(0xCB64);
                let mut ah = pairs;
                loop {
                    edx = edx.wrapping_add(xs2);
                    ebp = ebp.wrapping_add(ys2);
                    let (u, v) = ((edx >> 16) as u16, (ebp >> 16) as u16);
                    let t = ((v & 0x3F) << 6).wrapping_add(u & 0x3F);
                    let al = self.m.mem.r8(tex, t);
                    let c = self.m.mem.r8(tables, (bx & 0xFF00) | al as u16);
                    self.px(di, c);
                    self.px(di.wrapping_add(1), c);
                    di = di.wrapping_add(2);
                    bx = bx.wrapping_add(inc);
                    ah = ah.wrapping_sub(1);
                    if ah == 0 {
                        break;
                    }
                }
                left = left.wrapping_sub(seg);
                self.cs16(0xD752, left);
                if left == 0 {
                    break;
                }
            }
            self.s16(0xCD86, si);
            return;
        }
        let mut left = self.c16(0xD752);
        loop {
            let (si, sseg) = (self.g16(0xCD86), self.g16(0xCD88));
            let mut bx = self.m.mem.r16(sseg, si);
            let inc = self.m.mem.r16(sseg, si.wrapping_add(2));
            self.s16(0xCD86, si.wrapping_add(4));
            let mut ah = pairs;
            loop {
                edx = edx.wrapping_add(xs2);
                ebp = ebp.wrapping_add(ys2);
                let (u, v) = ((edx >> 16) as u16, (ebp >> 16) as u16);
                let at = ((((v as i16) >> 6) as u16) << 8).wrapping_add((((u as i16) >> 6) as u16).wrapping_mul(2)) & 0x7FFF;
                let cx = self.m.mem.r16(fs, at) & 0xFF00;
                if cx < thr {
                    let t = ((v & 0x3F) << 6).wrapping_add(u & 0x3F);
                    let al = self.m.mem.r8(cx, t);
                    let c = self.m.mem.r8(tables, (bx & 0xFF00) | al as u16);
                    self.px(di, c);
                    self.px(di.wrapping_add(1), c);
                }
                di = di.wrapping_add(2);
                bx = bx.wrapping_add(inc);
                ah = ah.wrapping_sub(1);
                if ah == 0 {
                    break;
                }
            }
            left = left.wrapping_sub(seg);
            self.cs16(0xD752, left);
            if left == 0 {
                break;
            }
        }
    }
}

/// The face being built (D7AA: four corners, x, height, z), the screen's
/// record (D7C2) and the scratch flat (D7DE).
const QUAD: u16 = 0xD7AA;
const SCREEN: u16 = 0xD7C2;
const FLAT: u16 = 0xD7DE;
/// D27A and D22B: the face drawers CB20 is given.
const WALL: u16 = 0xD27A;
const TOPS: u16 = 0xD22B;

impl Soft {
    /// 0000:93BC: (`ax`, `cx`) turned by `di` (512 to a turn): the high
    /// words of the doubled values' products with the sine (B03C) and
    /// cosine (B13C), -sin taken as 7FFFh for 8000h.
    fn rot(&self, di: u16, ax: u16, cx: u16) -> (u16, u16) {
        rot(&self.m, di, ax, cx)
    }

    /// 0000:E032: (`ax`, `bx`) at depth `cx` onto the screen: each times
    /// CCF4 (CCF6) over the depth.
    fn project(&self, ax: u16, bx: u16, cx: u16) -> (u16, u16) {
        (muldiv(ax, self.g16(0xCCF4), cx), muldiv(bx, self.g16(0xCCF6), cx))
    }

    /// 0000:C923: the view's walk (C980) between its resets, the player's
    /// square kept (CB90, CB6C) and C398 bit 20h cleared after.
    fn walk(&mut self) {
        self.s16(0xCB92, 0xFFFF);
        self.s16(0xCCCA, 0xFFFF);
        self.s16(0xCD14, 0xF830);
        self.s16(0xCCB2, 0);
        self.s16(0xCCB4, 0);
        let col = self.g16(0xC3D0) >> 7;
        self.s16(0xA854, col);
        self.s16(0xCCA4, col);
        let row = self.g16(0xC3D4) >> 7;
        self.s16(0xA856, row);
        self.s16(0xCCA6, row);
        self.s16(0xCB90, (row & 0xFF) << 8 | (col.wrapping_add(col) & 0xFF));
        self.squares();
        let sq = (row << 8).wrapping_add(col << 1);
        let walls = self.g16(crate::mif::var::MAP1_SEG);
        let w = self.m.mem.r16(walls, sq);
        self.s16(0xCB6C, w);
        self.s16(0xCB90, sq);
        self.and16(0xC398, 0xFFDF);
    }

    /// 0000:C980: each ring from C394 squares in to 1, then the player's.
    fn squares(&mut self) {
        let reach = self.g16(0xC394);
        loop {
            self.s16(0xD746, 0);
            loop {
                let r = self.g16(0xC394);
                let mut ax = r.wrapping_neg().wrapping_add(self.g16(0xD746));
                let mut bx = r.wrapping_neg();
                self.square(ax, bx);
                ax = ax.wrapping_neg();
                self.square(ax, bx);
                bx = bx.wrapping_neg();
                if bx != 0 {
                    self.square(ax, bx);
                    ax = ax.wrapping_neg();
                    self.square(ax, bx);
                    bx = bx.wrapping_neg();
                } else {
                    ax = ax.wrapping_neg();
                    bx = bx.wrapping_neg();
                }
                if ax != bx {
                    std::mem::swap(&mut ax, &mut bx);
                    self.square(ax, bx);
                    ax = ax.wrapping_neg();
                    self.square(ax, bx);
                    bx = bx.wrapping_neg();
                    if bx != 0 {
                        self.square(ax, bx);
                        ax = ax.wrapping_neg();
                        self.square(ax, bx);
                    }
                }
                let d = self.g16(0xD746).wrapping_add(1);
                self.s16(0xD746, d);
                if self.g16(0xC394) < d {
                    break;
                }
            }
            let r = self.g16(0xC394).wrapping_sub(1);
            self.s16(0xC394, r);
            if r == 0 {
                break;
            }
        }
        self.square(0, 0);
        // C9E8: the trigger squares (D39B) are the game's: the engine's.
        self.s16(0xC394, reach);
    }

    /// 0000:C9FF: the square (`ax`, `bx`) from the player's.
    fn square(&mut self, ax: u16, bx: u16) {
        self.s8(0xD73C, 0);
        let col = ax.wrapping_add(self.g16(0xCCA4));
        if (col as i16) < 0 {
            return;
        }
        let row = bx.wrapping_add(self.g16(0xCCA6));
        if (row as i16) < 0 || col >= self.g16(0xC357) || row >= self.g16(0xC359) {
            return;
        }
        if col == self.g16(0xCD14) && row == self.g16(0xD744) {
            return;
        }
        self.s16(0xCD14, col);
        self.s16(0xD744, row);
        self.s16(0xA84A, col);
        self.s16(0xA84E, row);
        let es = self.g16(0xD806);
        let di = (row << 8).wrapping_add(col.wrapping_add(col));
        self.s16(0xCCCA, di);
        let v = self.g16(0xD80A);
        self.s16(0xD7AC, v);
        self.s16(0xD7B2, v);
        let v = v.wrapping_add(self.g16(0xD80C));
        self.s16(0xD7B8, v);
        self.s16(0xD7BE, v);
        self.faces(es, di, WALL);
        if self.g16(0xC398) & 4 != 0 {
            self.or16(0xC39A, 4);
            for (at, v) in [(0xD7AC, 0xFF39), (0xD7B2, 0xFF39), (0xD7B8, 0xFF9D), (0xD7BE, 0xFF9D)] {
                self.s16(at, v);
            }
            let es = self.g16(0xD748);
            self.faces(es, di, TOPS);
            self.and16(0xC39A, 0xFFFB);
        }
        let es = self.g16(0xCCD0);
        if self.m.mem.r8(es, di) != 0 {
            self.flat_square(es, di);
        }
        let bx = self.g16(0xCCCE);
        if self.m.mem.r16(es, di) >= bx {
            let bl = (bx >> 8) as u8;
            let hi = self.m.mem.r8(es, di.wrapping_add(1));
            // CABF..CADD: 0 the frame's own floor; outdoors 1; indoors 1
            // (water) one past it, else 2 (lava).
            let al = if bl == hi {
                0
            } else if self.g16(0xC398) & 4 == 0 && hi != bl.wrapping_add(1) {
                2
            } else {
                1
            };
            self.s8(0xCB5B, al);
        }
        if self.g16(0xC398) & 0x20 == 0 {
            for (at, v) in [(0xD7AC, 0xFFFE), (0xD7B2, 0xFFFE), (0xD7B8, 0x62), (0xD7BE, 0x62)] {
                self.s16(at, v);
            }
            self.chasm_sides(es, di);
        } else {
            let (c, r) = (self.g16(0xCD14), self.g16(0xD744));
            self.actors_on(c, r);
        }
    }

    /// 0000:CB20: the square's faces, with the face drawer `bp`.
    fn faces(&mut self, es: u16, di: u16, bp: u16) {
        if bp == TOPS && self.g16(0xC398) & 0x20 == 0 {
            self.s8(0xD743, 0);
            return;
        }
        self.s16(0xD810, 0x2710);
        if self.m.mem.r16(es, di) != 0 {
            let (mut dx, mut bx) = (2, 0x100);
            if self.g16(0xA84E) >= self.g16(0xCCA6) {
                bx = 0xFF00;
                dx = 0;
            }
            self.s8(0xD742, 0);
            self.s8(0xD743, 0);
            self.side(es, di, bp, bx, dx);
            if self.g8(0xD742) != 1 {
                let (mut dx, mut bx) = (1, 2);
                if self.g16(0xA84A) >= self.g16(0xCCA4) {
                    dx = 3;
                    bx = 0xFFFE;
                }
                self.s8(0xD743, 1);
                self.side(es, di, bp, bx, dx);
                if (self.g8(0xD742) as i8) > 1 && bp != TOPS {
                    let ax = self.g16(0xC3D2);
                    if (ax as i16) > (self.g16(0xD802) as i16) {
                        let v = self.g16(0xD802);
                        self.cap(di, v, 0);
                    } else if (ax as i16) < (self.g16(0xD800) as i16) {
                        let v = self.g16(0xD800).wrapping_add(2);
                        self.cap(di, v, 1);
                    }
                    self.s8(0xD743, 0);
                    return;
                }
            }
        }
        // CBCF.
        let bx = self.g16(0xD810);
        if (self.g16(0xC3D2) as i16) > (bx as i16) {
            let ok = if es != self.g16(0xD748) {
                let ax = self.m.mem.r16(es, di);
                (ax as i16) > 0 && ax as u8 == (ax >> 8) as u8
            } else {
                true
            };
            if ok {
                let v = self.g16(0xCD06);
                self.s16(0xD804, v);
                self.cap(di, bx, 0);
            }
        }
        self.s8(0xD743, 0);
    }

    /// 0000:CBFF: a face's top or bottom (a square of the TOP texture at
    /// height `ax`), drawn by B2E5 (`poly`).
    fn cap(&mut self, di: u16, ax: u16, cx: u16) {
        self.cap_draw(di, ax, cx);
    }

    /// 0000:CC63: the square's side toward the neighbour at `bx` (the
    /// face's side `dx`: 0 north .. 3), by the square's kind.
    fn side(&mut self, es: u16, di: u16, bp: u16, bx: u16, dx: u16) {
        let mut dx = dx;
        self.s16(0xCCF8, 0);
        self.s8(0xCB37, 0);
        let rd = |m: &Machine, o: u16| m.mem.r16(es, di.wrapping_add(o));
        let nb = di.wrapping_add(bx);
        let al: u16;
        if self.g16(0xC39A) & 4 != 0 {
            if rd(&self.m, 0) == 0 {
                return;
            }
            self.s8(0xD73F, 0);
            let pack = |v: u16| {
                let v = (v & 0x8080) >> 7;
                (v as u8) | ((v >> 8) as u8).wrapping_add((v >> 8) as u8)
            };
            let a = pack(rd(&self.m, 0));
            self.s8(0xD741, a);
            self.s8(0xD740, a);
            let n = self.m.mem.r16(es, nb);
            if n != 0 {
                let b = pack(n);
                self.s8(0xD73F, b.wrapping_add(1));
                if b >= self.g8(0xD741) {
                    return;
                }
                let d = self.g8(0xD741).wrapping_sub(b);
                self.s8(0xD740, d.wrapping_sub(1));
            }
            return self.side_face(es, di, bp, dx);
        }
        let n = self.m.mem.r16(es, nb);
        if n as u8 == (n >> 8) as u8 && ((n as i16) > 0 || n >> 12 == 0xD) {
            return;
        }
        let w = rd(&self.m, 0);
        if (w as i16) >= 0 {
            // CE61: a raised floor.
            if self.g16(0xC398) & 0x20 == 0 {
                return;
            }
            if w as u8 == (w >> 8) as u8 {
                return self.side_face(es, di, bp, dx);
            }
            let hi = self.m.mem.r16(es, di.wrapping_add(1));
            let mut b = (hi & 7) << 1;
            if self.g16(0xC398) & 4 != 0 {
                b += 0x10;
                if self.g16(0xC39A) & 0x200 != 0 {
                    b += 0x10;
                }
            }
            let v = self.g16(0xCC74u16.wrapping_add(b)) & 0x3F;
            self.s16(0xD80E, v);
            let ax = self.g16(0xCB94u16.wrapping_add(b));
            self.s16(0xD800, ax);
            let mut cx = ax.wrapping_neg();
            let mut b = (hi >> 3) & 0xF;
            let t = self.g8(0xCC94u16.wrapping_add(b)) as u16;
            self.cs16(0xAA9E, t);
            let v = t.wrapping_add(self.g16(0xD80E)).wrapping_neg().wrapping_add(0x40);
            self.cs16(0xAAA0, v);
            b += b;
            if self.g16(0xC39A) & 0x200 != 0 {
                b += 0x20;
            }
            let ax = self.g16(0xCBC4u16.wrapping_add(b)).wrapping_add(self.g16(0xD800));
            self.s16(0xD802, ax);
            let bx2 = ax.wrapping_neg();
            self.s16(0xD7AC, bx2);
            self.s16(0xD7B2, bx2);
            self.s16(0xD7B8, cx);
            self.s16(0xD7BE, cx);
            cx = cx.wrapping_sub(bx2);
            let _ = cx;
            let t = (w >> 4) & 0xF;
            let v = self.g8(0xCB39u16.wrapping_add(t)) as u16;
            self.s16(0xD804, v);
            self.s8(0xBA7F, 1);
            al = self.g8(0xCB49u16.wrapping_add(w & 0xF)) as u16;
            self.s8(0xD742, 2);
            return self.face(bp, al, dx);
        }
        if bp == TOPS {
            return;
        }
        let kind = w >> 12;
        match kind {
            9 => {
                if self.g16(0xC398) & 0x20 == 0 {
                    return;
                }
                self.s8(0xCB37, 9);
                self.s8(0xBA7F, 1);
                if self.g16(0xC39A) & 0x200 != 0 && rd(&self.m, 0) == self.m.mem.r16(es, nb) {
                    return;
                }
            }
            8 => {
                self.s8(0xD742, 1);
                if self.m.mem.r8(es, di.wrapping_add(1)) & 2 != 0 {
                    // CF96: the creature's flat: the hook CD56 (the game's,
                    // called on the machine after: `creatures`) with the
                    // square's centre and the flat's number.
                    let x = (self.g16(0xA84A) << 7).wrapping_add(0x40);
                    let y = (self.g16(0xA84E) << 7).wrapping_add(0x40);
                    let flat = self.m.mem.r8(es, di);
                    self.creatures.push((x, y, flat, es, di));
                    return;
                }
                return self.flat_square(es, di);
            }
            _ => {
                if self.g16(0xC398) & 0x20 == 0 {
                    return;
                }
                match kind {
                    0xA => {
                        self.s8(0xCB37, 0xA);
                        self.s8(0xD742, 1);
                        let v = (self.m.mem.r8(es, di) as u16 >> 6) & 3;
                        self.s16(0xCCF8, v);
                        self.s8(0xBA7F, 3);
                    }
                    0xB => {
                        self.s8(0xCB37, 0xB);
                        self.s8(0xBA7F, 1);
                        let k = self.m.mem.r8(es, di) >> 6;
                        self.s8(0xCB38, k);
                        self.s16(0xCCC4, 0);
                        let a = self.m.mem.r8(es, di.wrapping_add(1)) & 0xF;
                        if a != 0 {
                            self.door_step(es, di, a);
                        }
                    }
                    0xD => {
                        self.s8(0xCB37, 0xD);
                        self.s8(0xD742, 1);
                        let a = if self.m.mem.r8(es, di.wrapping_add(1)) & 1 != 0 { 2 } else { 0 };
                        let v = self.g16(0xCCF8);
                        self.s16(0xD808, v);
                        self.s16(0xCCF8, 0xFFFE);
                        dx = a;
                    }
                    _ => return,
                }
            }
        }
        self.side_face(es, di, bp, dx)
    }

    /// 0000:CD89..CE32: an opening or closing door stepped as the walk
    /// meets it (its sound hooks are the game's): CCC4 its new opening.
    fn door_step(&mut self, es: u16, di: u16, al: u8) {
        self.or16(0xC398, 0x1000);
        let col = (di >> 1) & 0x7F;
        let row = di >> 8;
        let near = abs16((self.g16(0xC3D0) >> 7).wrapping_sub(col)) <= 2 && abs16((self.g16(0xC3D4) >> 7).wrapping_sub(row)) <= 2;
        let (mut dl, cx): (u8, u16) = if near {
            (0, 0x8001)
        } else if al & 1 != 0 {
            (1, 0xFF)
        } else {
            (0, 1)
        };
        let mut a = (al >> 1).wrapping_add(cx as u8);
        if (a as i8) >= 7 {
            a = 7;
            if (cx >> 8) as u8 & 0x80 == 0 {
                dl = 1;
            }
        }
        if (a as i8) <= 0 {
            a = 0;
            dl = 0;
        }
        self.s16(0xCCC4, a as i8 as i16 as u16);
        let v = (a << 1) | dl | 0xB0;
        self.m.mem.w8(es, di.wrapping_add(1), v);
    }

    /// 0000:CF20..CF87: the face drawn by `bp` with the square's texture
    /// (its low six bits), the changed squares stepped (D3F9) and the
    /// face's heights the level's (D80A, D80C), less a kind 0Ah's step.
    fn side_face(&mut self, es: u16, di: u16, bp: u16, dx: u16) {
        let b = self.m.mem.r8(es, di);
        if b & 0xC0 != 0 {
            self.temp_wall(di);
        }
        let al = (b & 0x3F) as u16;
        if bp != TOPS {
            let v = self.g16(0xD80A);
            self.s16(0xD7AC, v);
            self.s16(0xD7B2, v);
            let v = v.wrapping_add(self.g16(0xD80C));
            self.s16(0xD7B8, v);
            self.s16(0xD7BE, v);
            if self.g8(0xBA7F) == 3 {
                self.s8(0xBA7F, 2);
                let mut ax = (((self.m.mem.r8(es, di.wrapping_add(1)) & 0xF) >> 1) << 3) as u16;
                if self.g16(0xC398) & 4 != 0 {
                    ax = (ax << 2).wrapping_sub(8);
                }
                for at in [0xD7AC, 0xD7B2, 0xD7B8, 0xD7BE] {
                    let v = self.g16(at).wrapping_sub(ax);
                    self.s16(at, v);
                }
            }
        }
        self.face(bp, al, dx);
    }

    /// CF83..CF95: the face drawer called, the texture's offsets put back.
    fn face(&mut self, bp: u16, al: u16, side: u16) {
        if bp == WALL {
            self.wall(al, side);
        } else {
            self.top(al, side);
        }
        self.cs16(0xAA9E, 0x40);
        self.cs16(0xAAA0, 0);
    }

    /// 0000:D3F9: a changed square met by the walk (C2C2's list, its
    /// state at +40h): stepped once a square (D73C).
    fn temp_wall(&mut self, di: u16) {
        let mut at = None;
        for i in 0..0x10u16 {
            if self.g16(0xC2C2 + 2 * i) == di {
                at = Some(0xC2C2 + 2 * i);
                break;
            }
        }
        if let Some(p) = at {
            let ax = self.g16(p + 0x40);
            let (al, mut ah) = (ax as u8, (ax >> 8) as u8);
            if al == 0 || al == 1 {
                let v = self.g8(0xCB59) | 1;
                self.s8(0xCB59, v);
                if self.g8(0xD73C) == 0 {
                    if al == 0 {
                        ah = ah.wrapping_add(1);
                        if ah > 0xD {
                            ah = 0xD;
                        }
                    } else {
                        ah = ah.wrapping_sub(1);
                        if (ah as i8) < 0 {
                            ah = 0;
                        }
                    }
                }
                self.s16(p + 0x40, (ah as u16) << 8 | al as u16);
                self.s8(0xCB5A, ah);
            }
        }
        self.s8(0xD73C, 1);
    }

    /// 0000:D27A: a wall's face on side `bx` with texture `al` (one
    /// more than its number), its corners from D74A by the side.
    fn wall(&mut self, al: u16, bx: u16) {
        let bp = bx;
        let mut bx = bx;
        if self.g16(0xCCF8) != 0xFFFE {
            if self.g8(0xBA7F) == 2 {
                self.s8(0xBA7F, 1);
                bx = self.g16(0xCCF8);
                self.s16(0xCCF8, 0xFFFF);
            } else {
                self.s16(0xCCF8, bx);
            }
        }
        self.s16(0xD7D2, (al.wrapping_sub(1)) & 0xFF);
        let mut si = bx << 3;
        if (self.g16(0xCCF8) as i16) < 0 {
            si = si.wrapping_add(0x20);
            if self.g16(0xCCF8) == 0xFFFE {
                let v = self.g16(0xD808);
                self.s16(0xCCF8, v);
                si = si.wrapping_add(0x20);
            }
        }
        let x = self.g16(0xA84A) << 7;
        let z = self.g16(0xA84E) << 7;
        let mut ax = x.wrapping_add(self.g16(0xD74Au16.wrapping_add(si)));
        let mut cx = x.wrapping_add(self.g16(0xD74Cu16.wrapping_add(si)));
        let mut bz = z.wrapping_add(self.g16(0xD74Eu16.wrapping_add(si)));
        let mut dz = z.wrapping_add(self.g16(0xD750u16.wrapping_add(si)));
        match bp {
            0 | 3 => {}
            1 => std::mem::swap(&mut dz, &mut bz),
            _ => std::mem::swap(&mut cx, &mut ax),
        }
        self.s16(0xD7AA, ax);
        self.s16(0xD7B0, cx);
        self.s16(0xD7B6, cx);
        self.s16(0xD7BC, ax);
        self.s16(0xD7AE, bz);
        self.s16(0xD7B4, dz);
        self.s16(0xD7BA, dz);
        self.s16(0xD7C0, bz);
        let v = self.g16(0xD7AC).wrapping_neg();
        self.s16(0xD810, v);
        if self.g16(0xC398) & 0x1000 != 0 {
            self.m.door_swing();
            self.and16(0xC398, 0xEFFF);
            self.s16(0xCCF8, 0xFFFF);
        }
        self.face_draw();
    }

    /// 0000:D22B: a square's top face: its heights from D73F and D740
    /// (the top at -63h x D73F - 63h, the bottom 63h x (D740 + 1) above
    /// it), drawn as a wall (D27A) with BB04 the texture's repeats.
    fn top(&mut self, al: u16, bx: u16) {
        let al = al & 0x7F;
        let cx = self.g8(0xD740) as u16 + 1;
        let dx = (0xFF9Du16.wrapping_mul(self.g8(0xD73F) as u16)).wrapping_sub(0x63);
        self.s16(0xD7B8, dx);
        self.s16(0xD7BE, dx);
        let dx = self.g16(0xD7B8).wrapping_sub(0x63u16.wrapping_mul(cx));
        self.s16(0xD7AC, dx);
        self.s16(0xD7B2, dx);
        self.s16(0xD810, dx.wrapping_neg());
        self.s16(0xBB04, cx);
        self.wall(al, bx);
        self.s16(0xBB04, 0);
    }

    /// 0000:AE0D: a face turned from the eye (CCF8 its side) is culled:
    /// true where it is.
    fn culled(&mut self, di: u16) -> bool {
        if (self.g16(0xCCF8) as i16) <= -1 || self.g8(0xCB37) == 0xD {
            return false;
        }
        let (mut ch, mut cl) = (0xC0u8, 0u8);
        let ax = self.g16(di).wrapping_add(self.g16(di + 6)) >> 1;
        self.s16(0xBAF6, ax);
        if (ax.wrapping_sub(self.g16(0xC3D0)) as i16) >= 0 {
            ch &= 0x40;
        }
        let bx = self.g16(di + 4).wrapping_add(self.g16(di + 0xA)) >> 1;
        self.s16(0xBAF8, bx);
        if (bx.wrapping_sub(self.g16(0xC3D4)) as i16) >= 0 {
            cl |= 0x80;
        }
        let dl = (self.g16(0xCCF8) << 6) as u8;
        ch == dl || cl == dl
    }

    /// 0000:AC13: the face at QUAD drawn through the screen's record.
    fn face_draw(&mut self) {
        let (di, si) = (QUAD, SCREEN);
        if self.culled(di) {
            self.s8(0xBA7F, 0);
            return;
        }
        let v = if self.g16(0xC398) & 4 != 0 { 2 } else { 8 };
        self.s16(0xBB02, v);
        for (a, b) in [(0x18, 0), (0x1A, 6), (0x14, 4), (0x16, 0xA)] {
            let v = self.g16(di + b);
            self.s16(si + a, v);
        }
        let eye = (self.g16(0xC3D0), self.g16(0xC3D2), self.g16(0xC3D4), self.g16(0xC3DA));
        let ax = self.g16(di).wrapping_sub(eye.0);
        let bx = self.g16(di + 2).wrapping_add(eye.1);
        let cx = self.g16(di + 4).wrapping_sub(eye.2);
        self.s16(di + 2, bx);
        self.s16(di + 8, bx);
        let (ax, cx) = self.rot(eye.3, ax, cx);
        let cx = cx.wrapping_sub(0x1E);
        for (o, v) in [(0, ax), (4, cx), (0x12, ax), (0x16, cx)] {
            self.s16(di + o, v);
        }
        let ax = self.g16(di + 6).wrapping_sub(eye.0);
        let bx = self.g16(di + 0xE).wrapping_add(eye.1);
        let cx = self.g16(di + 0xA).wrapping_sub(eye.2);
        self.s16(di + 0xE, bx);
        self.s16(di + 0x14, bx);
        let (ax, cx) = self.rot(eye.3, ax, cx);
        let cx = cx.wrapping_sub(0x1E);
        for (o, v) in [(6, ax), (0xA, cx), (0xC, ax), (0x10, cx)] {
            self.s16(di + o, v);
        }
        if !self.near_clip(di) {
            self.s8(0xBA7F, 0);
            return;
        }
        let Some((ax, bx)) = self.vertex(self.g16(di), self.g16(di + 2), self.g16(di + 4).wrapping_add(0x1E)) else {
            self.s8(0xBA7F, 0);
            return;
        };
        self.s16(si, ax);
        self.s16(si + 0xC, ax);
        self.s16(si + 2, bx);
        let Some((ax, bx)) = self.vertex(self.g16(di + 6), self.g16(di + 8), self.g16(di + 0xA).wrapping_add(0x1E)) else {
            self.s8(0xBA7F, 0);
            return;
        };
        self.s16(si + 4, ax);
        self.s16(si + 8, ax);
        self.s16(si + 6, bx);
        let y = muldiv(self.g16(di + 0xE), self.g16(0xCCF6), self.g16(di + 0x10).wrapping_add(0x1E)).wrapping_add(self.g16(0xC3D8));
        self.s16(si + 0xA, y);
        let y = muldiv(self.g16(di + 0x14), self.g16(0xCCF6), self.g16(di + 0x16).wrapping_add(0x1E)).wrapping_add(self.g16(0xC3D8));
        self.s16(si + 0xE, y);
        let top = self.g16(TOP) as i16;
        if (self.g16(si + 0xA) as i16) <= top && (self.g16(si + 0xE) as i16) <= top {
            self.s8(0xBA7F, 0);
            return;
        }
        let v = self.g16(di + 2);
        self.s16(0xBA9C, v);
        let v = self.g16(di + 0xE);
        self.s16(0xBA9E, v);
        let v = self.g16(0xBAFE);
        self.s16(0xBAAC, v);
        self.s16(0xBAAE, 0x40);
        let mut d = di;
        let mut bx: u16 = 6;
        let (a, b) = (self.g16(si) as i16, self.g16(si + 4) as i16);
        if a >= b {
            if a == b {
                self.s8(0xBA7F, 0);
                return;
            }
            d = di + 6;
            self.s16(0xBAAC, 0x1000);
            self.s16(0xBAAE, 0xFFC0);
            bx = 0xFFFA;
        }
        let v = self.g16(d);
        self.s16(0xBAC4, v);
        let v2 = self.g16(d.wrapping_add(bx)).wrapping_sub(v);
        self.s16(0xBAB8, v2);
        let v = self.g16(d + 4);
        self.s16(0xBAC6, v);
        let v2 = self.g16(d.wrapping_add(bx).wrapping_add(4)).wrapping_sub(v);
        self.s16(0xBABA, v2);
        let t = self.g16(si + 0x10);
        let bank = (t / 0xF).wrapping_add(self.g8(0xC8F0) as u16);
        self.s16(0xBAF2, bank);
        self.m.ems_map(bank);
        let off = (t % 0xF).wrapping_mul(0x1000);
        self.s16(si + 0x10, off);
        let mut dx = 0;
        if self.g16(0xC398) & 0x8000 == 0 {
            match self.light(si) {
                Some(v) => dx = v,
                None => {
                    self.s8(0xBA7F, 0);
                    return;
                }
            }
        }
        self.face_span(si, dx);
        self.s8(0xBA7F, 0);
    }

    /// 0000:ADD4: a corner onto the screen; None past the sides' reach.
    fn vertex(&self, ax: u16, bx: u16, cx: u16) -> Option<(u16, u16)> {
        let mut ax = ax;
        if (ax as i16) >= 0 {
            if ax >= 0xDAC {
                return None;
            }
            if cx == 0x1E && ax <= 0x1E {
                ax = 0x1E;
            }
        } else {
            if (ax as i16) < (0xF254u16 as i16) {
                return None;
            }
            if cx == 0x1E && (ax as i16) >= -0x1E {
                ax = 0xFFE2;
            }
        }
        let (ax, bx) = self.project(ax, bx, cx);
        Some((ax.wrapping_add(self.g16(0xC3D6)), bx.wrapping_add(self.g16(0xC3D8))))
    }

    /// 0000:E045: where the edge from (`ax`, `bx`) to (`cx`, `dx`) meets
    /// depth 2: (x, 2).
    fn cross(ax: u16, bx: u16, cx: u16, dx: u16) -> (u16, u16) {
        let a = cx.wrapping_sub(ax);
        let d = 2u16.wrapping_sub(bx);
        let p = s(a) * s(d);
        let mut q = dx.wrapping_sub(bx);
        if q == 0 {
            q = 0x14;
        }
        let x = (p / s(q)) as u16;
        (x.wrapping_add(ax), 2)
    }

    /// 0000:B145: |cx| at least 23h.
    fn wide(cx: u16) -> u16 {
        if (cx as i16) < 0 {
            if (cx as i16) < -0x23 {
                cx
            } else {
                0xFFDD
            }
        } else if (cx as i16) > 0x23 {
            cx
        } else {
            0x23
        }
    }

    /// 0000:B049: the turned face clipped to the near plane (depth 2),
    /// the texture's start (BAFE) and span (BAFC) kept; false where it is
    /// all behind.
    fn near_clip(&mut self, si: u16) -> bool {
        let g = |m: &Machine, o: u16| m.mem.r16(DS, si + o);
        if (g(&self.m, 4) as i16) >= 2 && (g(&self.m, 0xA) as i16) >= 2 {
            self.s16(0xBAFE, 0);
            self.s16(0xBAFC, 0x1000);
            return true;
        }
        let span = |m: &Machine| {
            let a = abs16(g(m, 0).wrapping_sub(g(m, 6)));
            let b = abs16(g(m, 4).wrapping_sub(g(m, 0xA)));
            a.max(b)
        };
        let before = span(&self.m);
        let frac = |after: u16, before: u16| -> u16 {
            let n = ((after as i16 as i32) << 6) as u32;
            if before == 0 {
                return 0;
            }
            ((n / before as u32) as u16) << 6
        };
        if (g(&self.m, 0xA) as i16) > 2 {
            let (cx, dx) = Self::cross(g(&self.m, 6), g(&self.m, 0xA), g(&self.m, 0), g(&self.m, 4));
            let cx = Self::wide(cx);
            for (o, v) in [(0, cx), (4, dx), (0x12, cx), (0x16, dx)] {
                self.s16(si + o, v);
            }
            let ax = frac(span(&self.m), before);
            self.s16(0xBAFC, ax);
            self.s16(0xBAFE, 0x1000u16.wrapping_sub(ax));
            return true;
        }
        if (g(&self.m, 4) as i16) <= 2 {
            return false;
        }
        self.s16(0xBAFE, 0);
        let (cx, dx) = Self::cross(g(&self.m, 0), g(&self.m, 4), g(&self.m, 6), g(&self.m, 0xA));
        let cx = Self::wide(cx);
        for (o, v) in [(6, cx), (0xA, dx), (0xC, cx), (0x10, dx)] {
            self.s16(si + o, v);
        }
        let ax = frac(span(&self.m), before);
        let ax = ax.wrapping_neg().wrapping_add(0x1000);
        self.s16(0xBAFC, 0x1000u16.wrapping_sub(ax));
        true
    }
}

impl Soft {
    /// 0000:B649: the face's light at its two ends (B6EB at each), the
    /// flags of which lights it faces first (B6AB): A85E the first end's
    /// shade (8.8), the return the second's less the first's. None where
    /// B6EB carries. A changed square's own (CB59 bit 0) instead: its
    /// shade CB5A, the step 0.
    fn light(&mut self, si: u16) -> Option<u16> {
        if self.g8(0xCB59) & 1 != 0 {
            let v = self.g8(0xCB59) & 0xFE;
            self.s8(0xCB59, v);
            let v = (self.g8(0xCB5A) as u16) << 8;
            self.s16(0xA85E, v);
            return Some(0);
        }
        let v = (self.g16(0xCCF8) as u8) << 6;
        self.s8(0xBA7E, v);
        let v = self.g16(si + 0x18).wrapping_add(self.g16(si + 0x1A)) >> 1;
        self.s16(0xBAF6, v);
        let v = self.g16(si + 0x14).wrapping_add(self.g16(si + 0x16)) >> 1;
        self.s16(0xBAF8, v);
        self.lights_faced();
        let (a, c) = self.light_of(self.g16(si + 0x18), self.g16(si + 0x14));
        if c {
            return None;
        }
        let (b, c) = self.light_of(self.g16(si + 0x1A), self.g16(si + 0x16));
        if c {
            return None;
        }
        self.s16(0xA85E, a << 8);
        Some(b.wrapping_sub(a))
    }

    /// 0000:B6AB: for each light (C3DE's C3DC, then BBCC's BC8C), at
    /// C997.., whether the face hides it (B7A9).
    fn lights_faced(&mut self) {
        let mut di = 0xC997u16;
        for (list, n) in [(0xC3DEu16, self.g16(0xC3DC)), (0xBBCC, self.g16(0xBC8C))] {
            if list == 0xBBCC && n == 0 {
                break;
            }
            let mut si = list;
            let mut dx = n;
            loop {
                let c = self.behind(self.g16(si), self.g16(si + 2));
                self.s8(di, c as u8);
                di = di.wrapping_add(1);
                si = si.wrapping_add(6);
                dx = dx.wrapping_sub(1);
                if dx == 0 {
                    break;
                }
            }
        }
    }

    /// 0000:B7A9: whether (`ax`, `bx`) is on the face's far side.
    fn behind(&self, ax: u16, bx: u16) -> bool {
        if self.g16(0xCCF8) == 0xFFFF || self.g8(0xCB37) == 0xD {
            return false;
        }
        let (mut ch, mut cl) = (0xC0u8, 0u8);
        if (ax.wrapping_sub(self.g16(0xBAF6)) as i16) < 0 {
            ch &= 0x40;
        }
        if (bx.wrapping_sub(self.g16(0xBAF8)) as i16) < 0 {
            cl |= 0x80;
        }
        ch == self.g8(0xBA7E) || cl == self.g8(0xBA7E)
    }

    /// 0000:B6EB: the shade at (`ax`, `bx`) from the lights (B720 for
    /// each list), and its carry.
    fn light_of(&mut self, ax: u16, bx: u16) -> (u16, bool) {
        let n = self.g16(0xC3DC);
        self.s16(0xBAFA, 0xE);
        let a = self.light_list2(ax, bx, 0xC3DE, 0xC997, n);
        self.s16(0xA846, a.0);
        let mut carry = false;
        let n2 = self.g16(0xBC8C);
        if n2 != 0 {
            let b = self.light_list2(ax, bx, 0xBBCC, a.1, n2);
            carry = self.g16(0xA846) < b.0;
            if self.g16(0xA846) > b.0 {
                self.s16(0xA846, b.0);
            }
        }
        (self.g16(0xA846), carry)
    }

    /// 0000:B720: the least shade of `n` lights from `si`, BB02 more for
    /// one the face hides (its flag at `di`): (the shade, `di` after).
    fn light_list2(&mut self, ax: u16, bx: u16, mut si: u16, mut di: u16, n: u16) -> (u16, u16) {
        let mut cx = n;
        loop {
            let mut a = abs16(ax.wrapping_sub(self.g16(si)));
            let mut b = abs16(bx.wrapping_sub(self.g16(si + 2)));
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            let d = (a >> 2).wrapping_add(b);
            let mut r = self.falloff(d, self.g16(si + 4));
            if self.g8(di) != 0 {
                r = r.wrapping_add(self.g16(0xBB02));
                if r >= self.g16(0xC8E7) {
                    r = self.g16(0xC8E9);
                }
            }
            di = di.wrapping_add(1);
            if self.g16(0xBAFA) > r {
                self.s16(0xBAFA, r);
            }
            si = si.wrapping_add(6);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
        let mut ax = self.g16(0xBAFA);
        if ax >= self.g16(0xC8E7) {
            ax = self.g16(0xC8E9);
        }
        (ax, di)
    }

    /// 0000:B777: the shade at distance `ax` from a light of radius `cx`.
    fn falloff(&self, ax: u16, cx: u16) -> u16 {
        if ax > cx {
            return self.g16(0xC8EB);
        }
        let h = cx >> 1;
        if ax < h {
            return 0;
        }
        let ax = ax - h;
        let mut q = h / 0xD;
        if q == 0 {
            q = 1;
        }
        let v = (ax / q).wrapping_add(1);
        if v >= self.g16(0xC8E7) {
            return self.g16(0xC8EB);
        }
        v
    }

    /// 0000:B210: a face's span on the screen, kept for clicks (B1AD) and
    /// drawn (AB37: at once, BA80).
    fn face_span(&mut self, si: u16, dx: u16) {
        self.s16(0xA860, dx);
        let (mut cx, mut dx) = (self.g16(si), self.g16(si + 4));
        if (cx as i16) > (dx as i16) {
            std::mem::swap(&mut cx, &mut dx);
        }
        if (dx as i16) <= (self.g16(LEFT) as i16) || (cx as i16) >= (self.g16(RIGHT) as i16) {
            return;
        }
        self.clicked_span(cx, dx);
        if self.g8(0xBA80) == 0 {
            self.gap("B23F: a face to the deferred list");
            return;
        }
        let d = self.g16(0xA860);
        self.span(si, d);
    }

    /// 0000:B1AD: the face's columns kept for a door (CB37 0Bh: CCB2..),
    /// and the square under the cursor's column (8F92) kept (CB92).
    fn clicked_span(&mut self, cx: u16, dx: u16) {
        if self.g8(0xCB37) == 0xB {
            self.s16(0xCCB2, cx);
            self.s16(0xCCB4, dx);
            let v = self.g16(0xA84A);
            self.s16(0xCCB6, v);
            let v = self.g16(0xA84E);
            self.s16(0xCCB8, v);
            let v = self.g16(0xCCCA);
            self.s16(0xCCC8, v);
        } else {
            self.s8(0xCB38, 0xFF);
        }
        let m = self.g16(0x8F92) as i16;
        if (cx as i16) > m || (dx as i16) < m {
            return;
        }
        if self.g8(0xCB37) == 0xA {
            let seg = self.g16(0xD806);
            let at = self.g16(0xCCCA);
            if self.m.mem.r16(seg, at) & 0x100 == 0 {
                return;
            }
        }
        let v = self.g16(0xCCCA);
        self.s16(0xCB92, v);
    }

    /// 0000:AB37: the face at the screen's record `si` drawn, its light
    /// stepping by `dx` across.
    fn span(&mut self, si: u16, dx: u16) {
        if self.g8(0xBA80) != 0 && self.g8(0xBA82) != 0 {
            for o in [2, 6, 0xA, 0xE] {
                let v = self.g16(si + o).wrapping_sub(2);
                self.s16(si + o, v);
            }
        }
        let (mut ax, mut bx) = (self.g16(si), self.g16(si + 4));
        if (ax as i16) >= (bx as i16) {
            std::mem::swap(&mut ax, &mut bx);
        }
        if (bx as i16) >= (self.g16(LEFT) as i16) && (ax as i16) < (self.g16(RIGHT) as i16) && self.span_setup(si, dx) != 0 {
            self.s16(0xBAEC, 0);
            self.s16(0xBB00, 0x8AD0);
            let saved = self.g16(0x9030);
            self.s16(0xBAF4, 0xF000);
            if (self.g8(0xCB38) as i8) >= 1 && self.g16(0xCCC4) != 0 {
                let tex = self.g16(si + 0x10);
                self.m.door_texture(tex);
                let (dseg, sseg) = (self.g16(0xBA56), self.g16(0xCCC2));
                for o in 0..0x1000u16 {
                    let b = self.m.mem.r8(sseg, o);
                    self.m.mem.w8(dseg, 0x1000 + o, b);
                }
                self.s16(0xBAF4, 0x1000);
                self.s16(0x9030, dseg);
                self.s16(si + 0x10, 0);
            }
            if self.g16(0xBAC6) <= 0x1F4 {
                self.near_span(si);
            } else {
                while let Some((ax, bx, cx)) = self.next_column() {
                    let t = self.g16(si + 0x10).wrapping_add(self.g16(0xBAEA));
                    let dx = self.g16(0xBAE6);
                    if !self.column_drawn(ax, bx, cx, t, dx) {
                        break;
                    }
                }
            }
            self.s16(0x9030, saved);
        }
        self.s16(0xBB04, 0);
    }

    /// 0000:AFD6: the span's slopes (CD08.. the top edge's x and y, the
    /// bottom's y), its length in columns (BAEE, BAF0; C398 bit 1 whether
    /// it runs along x) and the light's step a column (A860).
    fn span_setup(&mut self, si: u16, dx: u16) -> u16 {
        let x0 = self.g16(si);
        self.s16(0xCD0A, self.g16(si + 4).wrapping_sub(x0));
        self.s16(0xCD08, x0);
        let y0 = self.g16(si + 2);
        self.s16(0xCD0E, self.g16(si + 6).wrapping_sub(y0));
        self.s16(0xCD0C, y0);
        let b0 = self.g16(si + 0xE);
        self.s16(0xCD12, self.g16(si + 0xA).wrapping_sub(b0));
        self.s16(0xCD10, b0);
        let mut ax = abs16(self.g16(0xCD0A));
        let mut bx = abs16(self.g16(0xCD0E));
        let mut cl = 2u8;
        if ax < bx {
            cl = 0;
            std::mem::swap(&mut ax, &mut bx);
        }
        self.s16(0xBAEE, ax);
        self.s16(0xBAF0, ax);
        let f = (self.g16(0xC398) & 0xFFFD) | cl as u16;
        self.s16(0xC398, f);
        if ax != 0 {
            self.s16(0xBA98, dx);
            let q = (s(dx) << 8) / s(ax);
            self.s16(0xA860, q as u16);
        }
        ax
    }

    /// 0000:AE59: the span's next column on the screen: (x, top, bottom),
    /// its texture's column (BAEA) and shades (BAE6, BAE8) set; None past
    /// its end. Columns off the screen, dark ones (shade 0Eh and on) and
    /// those of shade 0Dh in an opaque face (filled, B7DC) are passed.
    fn next_column(&mut self) -> Option<(u16, u16, u16)> {
        loop {
            let n = self.g16(0xBAEC);
            if n > self.g16(0xBAF0) {
                return None;
            }
            let ee = self.g16(0xBAEE);
            let q = if ee == 0 { 0 } else { ((n << 6) / ee) as u32 };
            let ax = ((q * (self.g16(0xBAFC) >> 6) as u32) as u16 & 0xFC0).wrapping_add(self.g16(0xBAFE));
            let ax = if ax >= 0x1000 { 0xFC0 } else { ax };
            self.s16(0xBAEA, ax);
            let t = if ee == 0 { 0 } else { ((((n as i16 as i32) << 12) as u32) / ee as u32) as u16 };
            let along = self.g16(0xC398) & 2 != 0;
            let shr12 = |p: i32| ((p as u32) >> 12) as u16;
            let x = if !along {
                shr12(s(t) * s(self.g16(0xCD0A)))
            } else if (self.g16(0xCD0A) as i16) < 0 {
                n.wrapping_neg()
            } else {
                n
            };
            let x = x.wrapping_add(self.g16(0xCD08));
            if x == self.g16(0xBB00) {
                self.s16(0xBAEC, n.wrapping_add(1));
                continue;
            }
            self.s16(0xBB00, x);
            if (x as i16) < (self.g16(LEFT) as i16) || (x as i16) > (self.g16(RIGHT) as i16) {
                let v = self.g16(0xA85E).wrapping_add(self.g16(0xA860));
                self.s16(0xA85E, v);
                self.s16(0xBAEC, n.wrapping_add(1));
                continue;
            }
            self.s16(0xA836, x);
            let y = if along {
                shr12(s(t) * s(self.g16(0xCD0E)))
            } else if (self.g16(0xCD0E) as i16) < 0 {
                n.wrapping_neg()
            } else {
                n
            };
            let y = y.wrapping_add(self.g16(0xCD0C));
            self.s16(0xA838, y);
            let b = shr12(s(t) * s(self.g16(0xCD12))).wrapping_add(self.g16(0xCD10));
            self.s16(0xA83C, b);
            if self.g16(0xC398) & 0x8000 == 0 {
                let v = self.g16(0xA85E).wrapping_add(self.g16(0xA860));
                self.s16(0xA85E, v);
                let sh = v >> 8;
                if sh >= 0xE {
                    self.s16(0xBAEC, n.wrapping_add(1));
                    continue;
                }
                if sh >= 0xD && self.g8(0xBA7F) == 0 {
                    self.fill_column(x, y, b);
                    self.s16(0xBAEC, n.wrapping_add(1));
                    continue;
                }
                let v = (sh << 8).wrapping_add(self.g16(0xBAF4));
                self.s16(0xBAE6, v);
                self.s16(0xBAE8, v);
            }
            self.s16(0xBAEC, n.wrapping_add(1));
            return Some((x, y, b));
        }
    }

    /// 0000:B7DC: the column `x` from `bx` to `cx` in [C3C6] (the code at
    /// [CD8E] a row a store, cut short at the rows' count).
    fn fill_column(&mut self, x: u16, bx: u16, cx: u16) {
        let (t, b) = (self.g16(TOP) as i16, self.g16(BOTTOM) as i16);
        if (cx as i16) <= t || (bx as i16) >= b {
            return;
        }
        let bx = if (bx as i16) < t { t as u16 } else { bx };
        let cx = if (cx as i16) > b { b as u16 } else { cx };
        let n = cx.wrapping_sub(bx).wrapping_add(1) as u8;
        let mut di = self.row(bx).wrapping_add(x);
        let c = self.g8(0xC3C6);
        for _ in 0..n {
            self.px(di, c);
            di = di.wrapping_add(0x140);
        }
    }

    /// The column drawer AB78 sets at CS:B1AB: B835 (`column`), or with
    /// BB04 a square's top's height (`top`) B96E (`column_top`).
    fn column_drawn(&mut self, x: u16, bx: u16, cx: u16, si: u16, dx: u16) -> bool {
        if self.g16(0xBB04) != 0 {
            self.column_top(x, bx, cx, si, dx)
        } else {
            self.column(x, bx, cx, si, dx)
        }
    }

    /// 0000:B96E: `column` for a square's top face, BB04 heights of the
    /// texture's CS:AA9E texels over the column (wrapping within the
    /// texture's column: the texel's row masked by AA9E - 1; the 0s-left-
    /// out shaded drawer adds the mask instead, as the original does),
    /// from the texture's column `si` in [9030] made a segment of its
    /// own (si >> 4 paragraphs on).
    fn column_top(&mut self, x: u16, bx: u16, cx: u16, si: u16, dx: u16) -> bool {
        let mut shade = dx;
        let bp = cx.wrapping_sub(bx);
        if bp == 0 {
            return false;
        }
        let h = self.c16(0xAA9E).wrapping_mul(self.g16(0xBB04));
        let step = ((h as u32) << 16) / bp as u32;
        let mut esi = 0u32;
        let mut bx = bx;
        if (bx as i16) < (self.g16(TOP) as i16) {
            let k = self.g16(TOP).wrapping_sub(bx) as u32;
            esi = step.wrapping_mul(k);
            bx = self.g16(TOP);
        }
        let mut cx = cx;
        if (cx as i16) > (self.g16(BOTTOM) as i16) {
            cx = self.g16(BOTTOM);
        }
        let n = cx.wrapping_sub(bx);
        if (n as i16) <= 0 {
            return true;
        }
        if (x ^ bx) & 1 != 0 {
            let other = self.g16(0xBAE8);
            self.s16(0xBAE8, shade);
            shade = other;
        }
        let mut di = x.wrapping_add(self.row(bx));
        let mask = self.c16(0xAA9E).wrapping_sub(1);
        let see = self.g8(0xBA7F) != 0;
        let seg = (si >> 4).wrapping_add(self.g16(0x9030));
        if self.g16(0xC398) & 0x8000 != 0 {
            for _ in 0..n {
                let al = self.m.mem.r8(seg, (esi >> 16) as u16 & mask);
                if !see || al != 0 {
                    self.px(di, al);
                }
                esi = esi.wrapping_add(step);
                di = di.wrapping_add(0x140);
            }
            return true;
        }
        shade = shade.wrapping_sub(si);
        let v = self.g16(0xBAE6).wrapping_sub(si);
        self.s16(0xBAE6, v);
        let v = self.g16(0xBAE8).wrapping_sub(si);
        self.s16(0xBAE8, v);
        for _ in 0..n {
            let t = (esi >> 16) as u16;
            let al = self.m.mem.r8(seg, if see { t.wrapping_add(mask) } else { t & mask });
            if !see || al != 0 {
                let c = self.m.mem.r8(seg, shade.wrapping_add(al as u16));
                self.px(di, c);
            }
            let other = self.g16(0xBAE8);
            self.s16(0xBAE8, shade);
            shade = other;
            esi = esi.wrapping_add(step);
            di = di.wrapping_add(0x140);
        }
        true
    }

    /// 0000:B835: a column of the face at `x` from `bx` to `cx`, the
    /// texture's column at `si` in [9030] (CS:AA9E texels high, from
    /// CS:AAA0), shaded by the table at `dx`, which alternates with
    /// BAE8's a pixel (by the pixel's parity). False (a carry) for a
    /// column of no height, which ends the face.
    fn column(&mut self, x: u16, bx: u16, cx: u16, si: u16, dx: u16) -> bool {
        let mut shade = dx;
        let bp = cx.wrapping_sub(bx);
        if bp == 0 {
            return false;
        }
        let step = ((self.c16(0xAA9E) as u32) << 16) / bp as u32;
        let mut esi = (si as u32) << 16;
        let mut bx = bx;
        if (bx as i16) < (self.g16(TOP) as i16) {
            let k = self.g16(TOP).wrapping_sub(bx) as u32;
            esi = esi.wrapping_add(step.wrapping_mul(k));
            bx = self.g16(TOP);
        }
        let mut cx = cx;
        if (cx as i16) > (self.g16(BOTTOM) as i16) {
            cx = self.g16(BOTTOM);
        }
        let n = cx.wrapping_sub(bx);
        if (n as i16) <= 0 {
            return true;
        }
        if (x ^ bx) & 1 != 0 {
            let other = self.g16(0xBAE8);
            self.s16(0xBAE8, shade);
            shade = other;
        }
        let mut di = x.wrapping_add(self.row(bx));
        esi = esi.wrapping_add((self.c16(0xAAA0) as u32) << 16);
        let tex = self.g16(0x9030);
        let see = self.g8(0xBA7F) != 0;
        if self.g16(0xC398) & 0x8000 != 0 {
            // B8BA: in full daylight, the texels themselves.
            for _ in 0..n {
                let al = self.m.mem.r8(tex, (esi >> 16) as u16);
                if !see || al != 0 {
                    self.px(di, al);
                }
                esi = esi.wrapping_add(step);
                di = di.wrapping_add(0x140);
            }
            return true;
        }
        for _ in 0..n {
            let al = self.m.mem.r8(tex, (esi >> 16) as u16);
            if !see || al != 0 {
                let c = self.m.mem.r8(tex, shade.wrapping_add(al as u16));
                self.px(di, c);
            }
            let other = self.g16(0xBAE8);
            self.s16(0xBAE8, shade);
            shade = other;
            esi = esi.wrapping_add(step);
            di = di.wrapping_add(0x140);
        }
        true
    }

    /// 0000:BBB6: a face nearer than 1F4h, stepped by its texture's
    /// columns (BAFC/40h of them): each one's screen column found
    /// (the table at 92A0, a word a screen column), then the screen's
    /// columns drawn from it, the top and bottom stepping straight.
    fn near_span(&mut self, si: u16) {
        self.s16(0xBA84, 0);
        self.s16(0x929E, 0);
        for i in 0..0xC7u16 {
            self.s32(0x92A0 + 4 * i, 0xFFFF_FFFF);
        }
        self.s8(0xBA81, 1);
        self.s16(0xBAB4, 0);
        self.s16(0xBAB0, 0);
        let cx = self.g16(0xBAFC) >> 6;
        if cx == 0 {
            return;
        }
        let v = self.g16(si);
        self.s16(0xBAA8, v);
        let w12 = |v: u16| ((v as i16 as i32) << 12) as u32;
        self.s32(0xBA90, w12(self.g16(0xBAC4)));
        let q = |v: u16| (((v as i16 as i32) << 12) / s(cx)) as i16 as i32 as u32;
        self.s32(0xBA88, q(self.g16(0xBAB8)));
        self.s32(0xBA8C, q(self.g16(0xBABA)));
        self.s32(0xBA94, w12(self.g16(0xBAC6).wrapping_add(0x1E)));
        let mut di = self.g16(0xBAAC);
        let bp0 = self.g16(si + 0x10);
        self.s16(0xBA9A, bp0);
        let fill_back = |r: &mut Soft, mut bx: u16, di: u16| {
            r.s16(0x92A0u16.wrapping_add(bx), di);
            while r.g16(0x929Eu16.wrapping_add(bx)) == 0xFFFF {
                bx = bx.wrapping_sub(2);
                r.s16(0x92A0u16.wrapping_add(bx), di);
            }
        };
        let mut bp = cx.wrapping_add(1);
        let mut past = false;
        loop {
            let ax = (self.g32(0xBA90) as i32 >> 12) as u16;
            let ecx = (self.g32(0xBA94) as i32 >> 12) as u16;
            let x = muldiv(ax, self.g16(0xCCF4), ecx).wrapping_add(self.g16(0xC3D6));
            if (x as i16) >= (self.g16(LEFT) as i16) {
                if (x as i16) > (self.g16(RIGHT) as i16) {
                    past = true;
                    break;
                }
                if self.g8(0xBA81) != 0 {
                    if (self.g16(0xBAA8) as i16) < (self.g16(LEFT) as i16) {
                        let v = self.g16(LEFT);
                        self.s16(0xBAA8, v);
                    } else {
                        self.s16(0xBAA8, x);
                    }
                    self.s16(0xBAAA, ecx);
                    self.s8(0xBA81, 0);
                }
                let d = x.wrapping_sub(self.g16(0xBAA8));
                if (d as i16) >= 0 {
                    self.s16(0xBAB0, d);
                    fill_back(self, d.wrapping_add(d), di);
                }
            }
            let v = self.g32(0xBA90).wrapping_add(self.g32(0xBA88));
            self.s32(0xBA90, v);
            let v = self.g32(0xBA94).wrapping_add(self.g32(0xBA8C));
            self.s32(0xBA94, v);
            di = di.wrapping_add(self.g16(0xBAAE));
            let v = self.g16(0xBAB4).wrapping_add(1);
            self.s16(0xBAB4, v);
            bp = bp.wrapping_sub(1);
            if bp == 0 {
                break;
            }
        }
        if past {
            let ax = self.g16(RIGHT).wrapping_sub(self.g16(0xBAA8));
            if (ax as i16) > 0 {
                let ax = ax.wrapping_add(2);
                self.s16(0xBAB0, ax);
                fill_back(self, ax.wrapping_add(ax), di);
            }
        } else {
            if (self.g16(0xBAA8) as i16) < (self.g16(LEFT) as i16) {
                let v = self.g16(LEFT);
                self.s16(0xBAA8, v);
            }
            let ax = self.g16(si + 4).wrapping_sub(self.g16(0xBAA8));
            if (ax as i16) > 0 {
                self.s16(0xBAB0, ax);
                di = di.wrapping_sub(self.g16(0xBAAE));
                fill_back(self, ax.wrapping_add(ax), di);
            }
        }
        // BD3B.
        if (self.g16(0xBAB0) as i16) <= 0 {
            return;
        }
        let h = self.g16(0xC3D8);
        let (a, b) = self.project(self.g16(0xBA9C), self.g16(0xBA9E), self.g16(0xBAAA));
        let (top0, bot0) = (a.wrapping_add(h), b.wrapping_add(h));
        self.s16(0xA836, top0);
        self.s16(0xA838, bot0);
        let (a, b) = self.project(self.g16(0xBA9C), self.g16(0xBA9E), (self.g32(0xBA94) >> 12) as u16);
        let (da, db) = (a.wrapping_add(h).wrapping_sub(top0), b.wrapping_add(h).wrapping_sub(bot0));
        let mut cx = self.g16(0xBAB0);
        let div = if cx <= 1 { 2 } else { cx };
        self.s16(0xBAA0, ((s(da) << 6) / s(div)) as u16);
        self.s16(0xBAA4, 0);
        self.s16(0xBAA2, ((s(db) << 6) / s(div)) as u16);
        self.s16(0xBAA6, 0);
        if self.g16(0xC398) & 0x8000 == 0 {
            let q = (s(self.g16(0xBA98)) << 8) / s(cx.wrapping_add(2));
            self.s16(0xA860, q as u16);
        }
        let over = cx.wrapping_add(self.g16(0xBAA8)).wrapping_sub(self.g16(RIGHT));
        if (over as i16) > 0 {
            cx = cx.wrapping_sub(over).wrapping_add(1);
        }
        self.s16(0xBAB4, 0);
        self.s16(0xBAEC, 0);
        let clamp = |v: u16| if v >= 0xD00 { 0xD00 } else { v };
        let base = self.g16(0xBAF4);
        self.s16(0xBAE6, clamp(self.g16(0xA85E) & 0xFF00).wrapping_add(base));
        self.s16(0xBAE8, clamp(self.g16(0xA85E).wrapping_add(self.g16(0xBA86)) & 0xFF00).wrapping_add(base));
        let mut x = self.g16(0xBAA8);
        let mut tab = 0x92A0u16;
        loop {
            let d = self.g16(0xBAA4).wrapping_add(self.g16(0xBAA0));
            self.s16(0xBAA4, d);
            let top = self.g16(0xA836).wrapping_add(((d as i16) >> 6) as u16);
            let d = self.g16(0xBAA6).wrapping_add(self.g16(0xBAA2));
            self.s16(0xBAA6, d);
            let bot = self.g16(0xA838).wrapping_add(((d as i16) >> 6) as u16);
            let mut t = self.g16(tab);
            if t >= 0x1000 {
                t = 0xFC0;
            }
            let t = t.wrapping_add(self.g16(0xBA9A));
            let dx = self.g16(0xBAE6);
            self.column_drawn(x, top, bot, t, dx);
            if self.g16(0xC398) & 0x8000 == 0 {
                let v = self.g16(0xA85E).wrapping_add(self.g16(0xA860));
                self.s16(0xA85E, v);
                let sh = (v >> 8).min(0xD);
                self.s16(0xBAE6, (sh << 8).wrapping_add(self.g16(0xBAF4)));
                let v2 = clamp(v.wrapping_add(self.g16(0xBA86)) & 0xFF00).wrapping_add(self.g16(0xBAF4));
                self.s16(0xBAE8, v2);
            }
            x = x.wrapping_add(1);
            tab = tab.wrapping_add(2);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
    }
}

impl Soft {
    /// 0000:D0A1: the floor pass's chasm sides: toward the player's
    /// column and row, a side where the neighbour's floor is a chasm
    /// (its word at least CCCE), its texture by the chasm's kind (C92F).
    fn chasm_sides(&mut self, es: u16, di: u16) {
        let cx = self.g16(0xCCCE);
        if self.m.mem.r16(es, di) >= cx {
            // D165: from below the floor, the chasm's floor (a square
            // whose wall word is positive, both bytes alike) as a cap at
            // height 0 in the texture CD06.
            if (self.g16(0xC3D2) as i16) < 0 {
                let w = self.m.mem.r16(self.g16(0xD806), di);
                if (w as i16) > 0 && w as u8 == (w >> 8) as u8 {
                    let t = self.g16(0xCD06);
                    self.s16(0xD804, t);
                    self.cap(di, 0, 1);
                }
            }
            return;
        }
        let (mut dx, mut bx) = (1u16, 2u16);
        if self.g16(0xA84A) >= self.g16(0xCCA4) {
            dx = 3;
            bx = 0xFFFE;
        }
        let kind = |r: &Soft, bx: u16| {
            let hi = r.m.mem.r8(es, di.wrapping_add(bx).wrapping_add(1));
            let i = hi.wrapping_sub(r.g8(0x9031)).wrapping_sub(0xC);
            r.g8(0xC92Fu16.wrapping_add(i as u16)) as u16
        };
        let lit = |r: &mut Soft| {
            if r.g16(0xC398) & 4 == 0 || r.g16(0xC39A) & 0x200 != 0 {
                r.s8(0xBA7F, 1);
            }
        };
        if self.m.mem.r16(es, di.wrapping_add(bx)) >= cx {
            let al = kind(self, bx);
            lit(self);
            self.wall(al, dx);
            for (at, v) in [(0xD7AC, 0xFFFE), (0xD7B2, 0xFFFE), (0xD7B8, 0x62), (0xD7BE, 0x62)] {
                self.s16(at, v);
            }
        }
        let (mut dx, mut bx) = (2u16, 0x100u16);
        if self.g16(0xA84E) >= self.g16(0xCCA6) {
            dx = 0;
            bx = 0xFF00;
        }
        let cx = self.g16(0xCCCE);
        if self.m.mem.r16(es, di.wrapping_add(bx)) >= cx {
            let al = kind(self, bx);
            lit(self);
            self.s8(0xBA82, 1);
            self.wall(al, dx);
            self.s8(0xBA82, 0);
        }
    }

    /// 0000:CFBA: a square's flat (the wall plane's low byte its number
    /// from C38E): those above the floor (a positive first byte of its
    /// record at CD9E) in the floor's pass, the others in the walls'.
    fn flat_square(&mut self, es: u16, di: u16) {
        self.s8(0xCB37, 8);
        let walls = self.g16(0xD806);
        let n = self.m.mem.r8(if es == walls { es } else { walls }, di) as u16;
        let rec = n.wrapping_add(self.g16(0xC38E)).wrapping_mul(5);
        let above = (self.g8(0xCD9Eu16.wrapping_add(rec)) as i8) > 0;
        if above == (self.g16(0xC398) & 0x20 != 0) {
            return;
        }
        let ax = (self.g16(0xA84A) << 7).wrapping_add(0x40).wrapping_sub(self.g16(0xC3D0));
        let cx = (self.g16(0xA84E) << 7).wrapping_add(0x40).wrapping_sub(self.g16(0xC3D4));
        let (ax, cx) = self.rot(self.g16(0xC3DA), ax, cx);
        if (cx as i16) <= 0x14 {
            return;
        }
        if abs16(ax) >= 0xBB8 && cx < 0x1F4 {
            return;
        }
        let mut bp = 0u16;
        let mut al = self.m.mem.r8(es, di);
        if es != walls {
            let w = self.m.mem.r16(walls, di);
            if (w as i16) > 0 {
                bp = if w as u8 != (w >> 8) as u8 { self.raised(w) } else { self.g16(0xD80C) };
                al = al.wrapping_sub(1);
            }
        }
        let (col, row) = (self.g16(0xA84A), self.g16(0xA84E));
        self.flat_at(col, (al as u16) << 8 | (row & 0xFF), bp.wrapping_neg());
    }

    /// 0000:FCB9: a raised floor's height from its word.
    fn raised(&self, w: u16) -> u16 {
        let ah = w >> 8;
        let mut bx = (ah & 7) << 1;
        if self.g16(0xC398) & 4 != 0 {
            bx += 0x10;
            if self.g16(0xC39A) & 0x200 != 0 {
                bx += 0x10;
            }
        }
        let cx = self.g16(0xCB94u16.wrapping_add(bx));
        let mut bx = ((ah >> 3) & 0xF) << 1;
        if self.g16(0xC39A) & 0x200 != 0 {
            bx += 0x20;
        }
        self.g16(0xCBC4u16.wrapping_add(bx)).wrapping_add(cx)
    }

    /// 0000:D18E: the square's flat through the scratch record (FLAT):
    /// at the square's middle, `bp` up, its number bh plus C38E, its
    /// frame by the frame count where it animates (CA97).
    fn flat_at(&mut self, col: u16, bx: u16, bp: u16) {
        let (ch, cl) = ((bx >> 8) as u8, bx as u8);
        self.s16(FLAT + 0xF, 0);
        self.s16(FLAT + 0x16, 0);
        self.s16(FLAT + 2, bp);
        self.s8(FLAT + 0xD, 0xFF);
        let idx = (ch as u16).wrapping_add(self.g16(0xC38E));
        if idx >= self.g16(0xD432) {
            return;
        }
        let rec = idx.wrapping_mul(5);
        let mut up = self.g8(0xCD9Eu16.wrapping_add(rec)) as i8 as i16 as u16;
        if self.g8(0xCDA1u16.wrapping_add(rec)) & 0x20 != 0 {
            up = up.wrapping_add(self.g16(0xD80C).wrapping_neg());
        }
        let v = self.g16(FLAT + 2).wrapping_add(up);
        self.s16(FLAT + 2, v);
        let ch = ch.wrapping_add(self.g8(0xC38E));
        self.s8(FLAT + 0xC, ch);
        let anim = self.g8(0xCA97u16.wrapping_add(ch as u16));
        if anim != 0 {
            self.s8(FLAT + 0xE, anim);
            let n = anim & 0x7F;
            if n == 0 {
                self.gap("D1FD: a flat's frames 0");
                return;
            }
            let f = self.g8(0xCCBC) % n;
            self.s8(FLAT + 0xD, f);
        }
        self.s16(FLAT, (col << 7).wrapping_add(0x40));
        self.s16(FLAT + 4, ((cl as u16) << 7).wrapping_add(0x40));
        let b = self.g16(BOTTOM).wrapping_sub(1);
        self.s16(BOTTOM, b);
        self.flat_draw(FLAT);
        let b = self.g16(BOTTOM).wrapping_add(1);
        self.s16(BOTTOM, b);
    }

    /// 0000:C032: the actors on the square (their list at D330 by the
    /// column, a node's +2 the row and +4 the actor).
    fn actors_on(&mut self, col: u16, row: u16) {
        let mut bx = self.g16(0xD330u16.wrapping_add(col.wrapping_add(col)));
        if bx == 0 || self.g16(bx + 4) == 0 {
            return;
        }
        while bx != 0 {
            if self.g16(bx + 2) == row {
                let a = self.g16(bx + 4);
                self.flat_draw(a);
            }
            bx = self.g16(bx);
        }
    }

    /// 0000:C0F1: a flat's record (`bx`: x, height, z, .., +0Ch its
    /// number, +0Dh its frame, +0Eh its frames, +0Fh its flags) drawn.
    fn flat_draw(&mut self, bx: u16) {
        self.s8(0xCD97, 0);
        let v = self.g16(bx + 0x16);
        self.s16(0xD43A, v);
        self.s16(0xCD02, bx);
        let or97 = |r: &mut Soft, v: u8| {
            let x = r.g8(0xCD97) | v;
            r.s8(0xCD97, x);
        };
        if self.g16(bx + 0xF) & 2 != 0 {
            or97(self, 4);
        }
        let dl = self.g8(bx + 0xC);
        let mut dx = dl as u16;
        let mut packed = false;
        if dl & 0xC0 == 0xC0 {
            if self.g16(bx + 0xF) & 4 == 0 {
                self.s8(bx + 0xE, 0xFF);
            }
            if self.g16(bx + 0xF) & 0x20 != 0 {
                or97(self, 0x20);
                self.light_rows();
            }
            or97(self, 1);
            let mut ax = self.g8(bx + 0x11) as u16;
            let mut di = 0u16;
            let k = (dl & 0xF) as u16;
            if self.g16(bx + 0xF) & 0x8000 != 0 {
                if k == 5 {
                    di = 0x8000;
                    ax = (ax >> 1).wrapping_sub(1);
                } else {
                    ax = (ax >> 1).wrapping_add(k);
                }
            } else {
                ax = ax.wrapping_add(k);
                di = (ax & 1) << 15;
                ax >>= 1;
            }
            self.s16(0xD0B4, di);
            self.s16(0xD0B6, ax);
            dx = 0xFFFF;
            packed = true;
        } else {
            let f = self.g8(0xCDA1u16.wrapping_add(dx.wrapping_mul(5)));
            if f & 2 != 0 {
                or97(self, 8);
            }
            if f & 8 != 0 || self.g16(bx + 0xF) & 0x200 != 0 {
                or97(self, 2);
            }
            if f & 4 != 0 {
                or97(self, 0x10);
            }
            if f & 0x40 != 0 {
                self.s16(bx + 0x16, 0x200);
            }
            if self.g16(bx + 0xF) & 0x20 != 0 || f & 0x10 != 0 {
                or97(self, 0x20);
                self.light_rows();
            }
            if self.g8(bx + 0xD) & 0x80 == 0 {
                if self.g16(bx + 0xF) & 4 != 0 {
                    or97(self, 1);
                }
                packed = true;
            }
        }
        let (x, y, z) = (self.g16(bx), self.g16(bx + 2), self.g16(bx + 4));
        if packed {
            if self.g16(bx + 0xF) & 0x20 != 0 {
                or97(self, 0x20);
                self.light_rows();
            }
            let bp = self.g8(bx + 0xD) as u16;
            self.s16(0xCD9C, bx);
            let v = self.g16(bx + 0x14);
            self.s16(0xD436, v);
            let v = self.g16(bx + 0x16);
            self.s16(0xD438, v);
            self.s8(0xCD96, 1);
            if (self.g8(bx + 0xE) as i8) < 0 {
                self.sprite_framed(x, y, z, dx, bp);
            } else {
                self.sprite_packed(x, y, z, dx, bp);
            }
            self.plain_rows();
            let v = self.g16(0xD43A);
            self.s16(bx + 0x16, v);
            // C23A..C26C: its animation stepped (`Machine::drawn_step`),
            // which the game's memory takes too (`Walked::stepped`).
            self.m.drawn_step(bx);
            if bx != crate::talk::var::FLAT_DRAWN {
                self.stepped.push(bx);
            }
        } else {
            self.s16(0xCD9C, bx);
            let v = self.g16(bx + 0x16);
            self.s16(0xD438, v);
            self.s8(0xCD96, 0);
            self.sprite_raw(x, y, z, dx);
            self.plain_rows();
        }
    }

    /// 0000:C4BB, before a flat is scaled (C485, C710, C8C8): with a
    /// click left (C39A bit 80h, or 1000h), the flat at (A836, A838),
    /// A844 x A84C, under the click's point (CB60, CB62; its x alone with
    /// C39A bit 1000h) becomes CB5E (its record, CD9C) and the hook CD3A
    /// is called with AL CD96 (the 3D mode's 2294:047B,
    /// `Machine::clicked_flat`). A map flat drawn through the scratch
    /// record (CD02 D7DEh) is tried only if its kind's flags (CDA1 + 5n)
    /// have bit 0 or its byte at CD9F + 5n is set, unless C39A bit 400h.
    fn hit_test(&mut self) {
        let lf = self.g16(0xC39A);
        if lf & 0x1080 == 0 {
            return;
        }
        if self.g16(0xCD02) == crate::talk::var::FLAT_DRAWN && lf & 0x400 == 0 {
            let n = self.g8(0xD7EA) as u16 * 5;
            if self.g8(0xCDA1u16.wrapping_add(n)) & 1 == 0 && self.g8(0xCD9Fu16.wrapping_add(n)) == 0 {
                return;
            }
        }
        let (x, y, w, h) = (s(self.g16(0xA836)), s(self.g16(0xA838)), self.g16(0xA844), self.g16(0xA84C));
        let cx = s(self.g16(0xCB60));
        if x >= cx || cx >= s(self.g16(0xA836).wrapping_add(w)) {
            return;
        }
        if lf & 0x1000 == 0 {
            let cy = s(self.g16(0xCB62));
            if y >= cy || cy >= s(self.g16(0xA838).wrapping_add(h)) {
                return;
            }
        }
        let v = self.g16(0xCD9C);
        self.s16(crate::talk::var::CLICKED, v);
        match self.g32(0xCD3A) {
            0 => {}
            0x2294_047B => self.m.clicked_flat(),
            _ => self.gap("C52A: the hook CD3A"),
        }
    }

    /// 0000:953E: 78A8's rows compiled by 9546 (`blit`: through the
    /// light table, the screen's pixel under each), CS:789E.
    fn light_rows(&mut self) {
        self.cs16(GENERATOR, LIGHT_ROWS);
    }

    /// 0000:78A0: 78A8's rows compiled by 7A2B again.
    fn plain_rows(&mut self) {
        self.cs16(GENERATOR, PLAIN_ROWS);
    }

    /// The flat's place turned and projected (C2F0..C33C, C769..C793):
    /// its middle's foot on the screen and its depth; None behind the
    /// eye or past the window's right.
    fn place(&mut self, x: u16, y: u16, z: u16, right: bool) -> Option<u16> {
        let ax = x.wrapping_sub(self.g16(0xC3D0));
        let bx = y.wrapping_add(self.g16(0xC3D2));
        let cx = z.wrapping_sub(self.g16(0xC3D4));
        let (ax, cx) = self.rot(self.g16(0xC3DA), ax, cx);
        if (cx as i16) <= 0x14 {
            return None;
        }
        let (ax, bx) = self.project(ax, bx, cx);
        let ax = ax.wrapping_add(self.g16(0xC3D6));
        let bx = bx.wrapping_add(self.g16(0xC3D8));
        if right && (ax as i16) >= (self.g16(RIGHT) as i16) {
            return None;
        }
        self.s16(0xA836, ax);
        self.s16(0xA838, bx);
        let mut cx = cx;
        if self.g8(0xCD97) & 0x10 != 0 {
            cx >>= 2;
        }
        if self.g8(0xCD97) & 1 != 0 {
            let h = cx >> 1;
            cx = cx.wrapping_add(cx).wrapping_sub(h);
        }
        Some(cx)
    }

    /// The flat's light (C3E3..C421, C832..C870): the shade table's
    /// offset (the light times 100h) in CCC2's segment, or none (0, 0).
    fn flat_light(&mut self) -> (u16, u16) {
        if self.g8(0xCD97) & 2 != 0 || self.g16(0xC398) & 0x8000 != 0 {
            return (0, 0);
        }
        let n = self.g16(0xC3DC);
        for i in 0..n {
            self.s8(0xC997u16.wrapping_add(i), 0);
        }
        let (l, _) = self.light_of(self.g16(0xA83C), self.g16(0xA83E));
        (l.wrapping_mul(0x100), self.g16(0xCCC2))
    }

    /// 0000:C2CE: a packed flat (its frames at [D0B8 + 4n] in the bank
    /// [D0BA + 4n]): frame `bp` unpacked into [BA54] (5656) and scaled.
    fn sprite_packed(&mut self, x: u16, y: u16, z: u16, n: u16, bp: u16) {
        self.s16(0xA83C, x);
        self.s16(0xA83E, z);
        self.s16(0xA83A, bp);
        let si4 = n.wrapping_mul(4);
        let bank = self.g16(0xD0BAu16.wrapping_add(si4));
        self.m.ems_map(bank);
        let si = self.g16(0xD0B8u16.wrapping_add(si4));
        let es = self.g16(0x9030);
        let Some(cx) = self.place(x, y, z, true) else { return };
        let (w, h) = (self.m.mem.r16(es, si + 6), self.m.mem.r16(es, si + 8));
        let (ax, bx) = self.project(w, h, cx);
        if (ax as i16) <= 0 || (bx as i16) <= 0 {
            return;
        }
        let x0 = self.g16(0xA836).wrapping_sub(ax >> 1);
        self.s16(0xA836, x0);
        let y0 = self.g16(0xA838).wrapping_sub(bx);
        self.s16(0xA838, y0);
        if (x0.wrapping_add(ax) as i16) <= (self.g16(LEFT) as i16)
            || (y0 as i16) >= (self.g16(BOTTOM) as i16)
            || (y0.wrapping_add(bx) as i16) <= (self.g16(TOP) as i16)
        {
            return;
        }
        self.s16(0xA844, ax);
        self.s16(0xA84C, bx);
        let (doff, dseg) = (self.g16(0xBA54), self.g16(0xBA56));
        self.unpack(es, si, bp, dseg, doff);
        let (shoff, shseg) = self.flat_light();
        self.s16(0xCD9A, h);
        self.s16(0xCD98, w);
        let flags = 1 | if self.g8(0xCD97) & 4 != 0 { 2 } else { 0 };
        self.hit_test();
        self.blit(x0, y0, ax, bx, (shseg, shoff), w, h, flags, (dseg, doff), 8, (0, 0));
        if self.g8(0xCD97) & 8 != 0 {
            self.reflect();
        }
    }

    /// 0000:C746: a flat as it is stored (+4 its width, +6 its height,
    /// its pixels from +0Ch), scaled by D438/100h where that is set.
    fn sprite_raw(&mut self, x: u16, y: u16, z: u16, n: u16) {
        self.s16(0xA83C, x);
        self.s16(0xA83E, z);
        let si4 = n.wrapping_mul(4);
        let bank = self.g16(0xD0BAu16.wrapping_add(si4));
        self.m.ems_map(bank);
        let si = self.g16(0xD0B8u16.wrapping_add(si4));
        let es = self.g16(0x9030);
        let Some(cx) = self.place(x, y, z, false) else { return };
        let (w, h) = (self.m.mem.r16(es, si + 4), self.m.mem.r16(es, si + 6));
        let (mut a, mut b) = (w, h);
        let sc = self.g16(0xD438);
        if sc != 0 {
            a = ((a as u32 * sc as u32) >> 8) as u16;
            b = ((b as u32 * sc as u32) >> 8) as u16;
            self.s16(0xD438, 0);
        }
        let (ax, bx) = self.project(a, b, cx);
        if (ax as i16) <= 0 || (bx as i16) <= 0 {
            return;
        }
        let x0 = self.g16(0xA836).wrapping_sub(ax >> 1);
        self.s16(0xA836, x0);
        let y0 = self.g16(0xA838).wrapping_sub(bx);
        self.s16(0xA838, y0);
        if (x0.wrapping_add(ax) as i16) <= (self.g16(LEFT) as i16)
            || (x0.wrapping_sub(ax) as i16) >= (self.g16(RIGHT) as i16)
            || (y0 as i16) >= (self.g16(BOTTOM) as i16)
            || (y0.wrapping_add(bx) as i16) <= (self.g16(TOP) as i16)
        {
            return;
        }
        self.s16(0xA844, ax);
        self.s16(0xA84C, bx);
        let (shoff, shseg) = if self.g16(0xC398) & 0x8000 == 0 && self.g8(0xCD97) & 2 == 0 { self.flat_light() } else { (0, 0) };
        // C877: the hook CD5A (292B:0028), a person's colours.
        let (shseg, shoff) = self.person_colours(shseg, shoff);
        self.s16(0xCD9A, h);
        self.s16(0xCD98, w);
        self.hit_test();
        self.blit(x0, y0, ax, bx, (shseg, shoff), w, h, 1, (es, si.wrapping_add(0xC)), 8, (0, 0));
        if self.g8(0xCD97) & 8 != 0 {
            self.reflect();
        }
    }

    /// 0000:C539: a flat drawn by its own frames (a person, the record's
    /// frames FFh): the image at [D0B8 + 4n] in the bank [D0BA + 4n] (n
    /// FFFFh: D0B4 and D0B6, which C0F1 set) has its width (+0), height
    /// (+2), row bytes (+4), bits a pixel (+0Ah), the frames' offsets
    /// (+0Ch) and its colours (+4Ch). Its height scaled by D438 and, for
    /// flag 1, D434; placed, its frame `bp` unpacked into [BA54] (5B65),
    /// lit, the hook CD5A (a person's colours) and drawn (78A8) through
    /// its colours.
    fn sprite_framed(&mut self, x: u16, y: u16, z: u16, n: u16, bp: u16) {
        self.s16(0xA83C, x);
        self.s16(0xA83E, z);
        self.s16(0xA83A, bp);
        let si4 = n.wrapping_mul(4);
        let bank = self.g16(0xD0BAu16.wrapping_add(si4));
        self.m.ems_map(bank);
        let si = self.g16(0xD0B8u16.wrapping_add(si4));
        let es = self.g16(0x9030);
        let scale = |v: u16, k: u16| (((v as u32) * (k as u32)) >> 8) as u16;
        let mut h = self.m.mem.r16(es, si.wrapping_add(2));
        if self.g16(0xD438) != 0 {
            h = scale(h, self.g16(0xD438));
        }
        if self.g8(0xCD97) & 1 != 0 {
            h = scale(h, self.g16(0xD434));
        }
        let ax = x.wrapping_sub(self.g16(0xC3D0));
        let bx = y.wrapping_sub(h).wrapping_add(self.g16(0xC3D2));
        let cx = z.wrapping_sub(self.g16(0xC3D4));
        let (ax, mut cx) = self.rot(self.g16(0xC3DA), ax, cx);
        if (cx as i16) <= 0x14 {
            return;
        }
        let ax = ax.wrapping_add(self.g16(0xD436));
        self.s16(0xD436, 0);
        let (ax, bx) = self.project(ax, bx, cx);
        let x0 = ax.wrapping_add(self.g16(0xC3D6));
        let y0 = bx.wrapping_add(self.g16(0xC3D8));
        if self.g8(0xCD97) & 0x10 != 0 {
            cx >>= 2;
        }
        let mut w = self.m.mem.r16(es, si);
        if self.g16(0xD438) != 0 {
            w = scale(w, self.g16(0xD438));
        }
        let (ax, bx) = self.project(w, h, cx);
        if (ax as i16) <= 0 || (bx as i16) <= 0 {
            return;
        }
        let x0 = x0.wrapping_sub(ax >> 1);
        self.s16(0xA836, x0);
        self.s16(0xA838, y0);
        // C5F3..C621 (the right edge against the left one less the width).
        if (x0.wrapping_add(ax) as i16) <= (self.g16(LEFT) as i16)
            || (x0.wrapping_sub(ax) as i16) >= (self.g16(RIGHT) as i16)
            || (y0 as i16) >= (self.g16(BOTTOM) as i16)
            || (y0.wrapping_add(bx) as i16) <= (self.g16(TOP) as i16)
        {
            return;
        }
        self.s16(0xA844, ax);
        self.s16(0xA84C, bx);
        let a83a = self.g16(0xA83A);
        let stride = self.m.mem.r16(es, si.wrapping_add(4));
        let rows = self.m.mem.r16(es, si.wrapping_add(2));
        let src = si.wrapping_add(self.m.mem.r16(es, si.wrapping_add(a83a.wrapping_add(a83a)).wrapping_add(0xC)));
        let (doff, dseg) = (self.g16(0xBA54), self.g16(0xBA56));
        self.runs_unpack(es, src, dseg, doff, stride, rows);
        // C651..C6A9: the light.
        let mut shade = (0u16, 0u16);
        if self.g16(0xC398) & 0x8000 == 0 && self.g8(0xCD97) & 2 == 0 {
            shade = (self.g16(0xCCC2).wrapping_add(0x10), 0);
            if self.g8(0xCD97) & 0x20 == 0 {
                let n = self.g16(0xC3DC);
                for i in 0..n {
                    self.s8(0xC997u16.wrapping_add(i), 0);
                }
                let (l, _) = self.light_of(self.g16(0xA83C), self.g16(0xA83E));
                shade = (self.g16(0xCCC2), l.wrapping_mul(0x100));
            }
        }
        let shade = self.person_colours(shade.0, shade.1);
        let flags = 1 | if self.g8(0xCD97) & 4 != 0 { 2 } else { 0 };
        let bpp = self.m.mem.r8(es, si.wrapping_add(0xA));
        let (fh, fw) = (self.m.mem.r16(es, si.wrapping_add(2)), self.m.mem.r16(es, si));
        self.s16(0xCD9A, fh);
        self.s16(0xCD98, fw);
        // C710: C4BB, the click's test, is the game's (the copy's).
        self.hit_test();
        self.blit(x0, y0, ax, bx, shade, fw, fh, flags, (dseg, doff), bpp, (es, si.wrapping_add(0x4C)));
        if self.m.hook_set(0xCD3E) {
            self.gap("C71C: the hook CD3E after a flat");
        }
        if self.g8(0xCD97) & 8 != 0 {
            self.reflect();
        }
    }

    /// 0000:5B65: `rows` x `stride` bytes run-length unpacked from
    /// `seg`:`si` to `dseg`:`doff`: a byte under 80h, that many plus one
    /// bytes copied; else its low bits plus one of the next byte; on
    /// until the count comes to 0.
    fn runs_unpack(&mut self, seg: u16, mut si: u16, dseg: u16, mut di: u16, stride: u16, rows: u16) {
        let mut bx = rows.wrapping_mul(stride);
        loop {
            let al = self.m.mem.r8(seg, si);
            si = si.wrapping_add(1);
            let n = ((al & 0x7F) as u16) + 1;
            bx = bx.wrapping_sub(n);
            if al & 0x80 == 0 {
                for _ in 0..n {
                    let b = self.m.mem.r8(seg, si);
                    self.m.mem.w8(dseg, di, b);
                    si = si.wrapping_add(1);
                    di = di.wrapping_add(1);
                }
            } else {
                let b = self.m.mem.r8(seg, si);
                si = si.wrapping_add(1);
                for _ in 0..n {
                    self.m.mem.w8(dseg, di, b);
                    di = di.wrapping_add(1);
                }
            }
            if bx == 0 {
                break;
            }
        }
    }

    /// The hook CD5A (292B:0028): for a person (the record CD9C's number
    /// C0h.. under the creatures) the shade `seg`:`off` made a table of
    /// their own (292B:0074) at 4A30h: the shade's row twice (or the
    /// colours as they are), then for each set bit of the person's record
    /// word (+12h, its top bit cleared), rolled in from the top, ten
    /// colours from 7D10's start for the bit put where 7D10's entry for
    /// the low four bits names (80h left for low bits 0Bh), and for the
    /// climate (0A46: not 0, 2, 5, 6) ten more (292B:0189). The shade is
    /// then that table.
    fn person_colours(&mut self, seg: u16, off: u16) -> (u16, u16) {
        let rec = self.g16(0xCD9C);
        if self.g8(rec.wrapping_add(0xC)) & 0xC0 != 0xC0 || rec >= 0x0C57 {
            return (seg, off);
        }
        const T: u16 = 0x4A30;
        let mut dx = self.g16(rec.wrapping_add(0x12)) & 0x7FFF;
        for k in 0..0x200u16 {
            let b = if seg != 0 { self.m.mem.r8(seg, off.wrapping_add(k & 0xFF)) } else { k as u8 };
            self.m.mem.w8(T, k, b);
        }
        let mut bx = 0x7D10u16;
        for _ in 0..0x10 {
            dx = dx.rotate_left(1);
            if dx & 1 != 0 {
                let di = self.g8(bx) as u16;
                if !(di == 0x80 && dx & 0xF == 0xB) {
                    let si = (self.g8(0x7D10 + (dx & 0xF)) as u16).wrapping_add(0x100);
                    for k in 0..0xA {
                        let b = self.m.mem.r8(T, si.wrapping_add(k));
                        self.m.mem.w8(T, di.wrapping_add(k), b);
                    }
                }
            }
            bx = bx.wrapping_add(1);
        }
        let al = self.g8(0x0A46);
        if !matches!(al, 0 | 2 | 5 | 6) {
            let mut b: u8 = match al {
                7 => 0x74,
                3 => 0x34,
                4 => 0xC0,
                _ => 0x94,
            };
            for k in 0..0xA {
                let di = self.g8(0x7D20 + k) as u16;
                let v = self.m.mem.r8(T, 0x100 | b as u16);
                self.m.mem.w8(T, di, v);
                b = b.wrapping_add(1);
            }
        }
        (T, 0)
    }

    /// 0000:E37F, after a flat with CD97 bit 8 is drawn, over its
    /// rectangle (A836, A838; A844 wide, A84C high), cut to the view's
    /// left, right and bottom (not its top): each pixel of colour 1Eh
    /// takes the one mirrored about the horizon (C3D8: the row table's
    /// word for the row less the horizon, doubled and negated, as the
    /// mirror's offset, read wherever the table's index falls), then each
    /// of 67h the one a row above that; the mirror moves up two rows
    /// with each row down. The page is read and written as both of its
    /// passes leave it (`repne scasb` from each match on).
    fn reflect(&mut self) {
        let (mut ax, bx, mut cx, mut dx) = (self.g16(0xA836), self.g16(0xA838), self.g16(0xA844), self.g16(0xA84C));
        let left = self.g16(LEFT);
        if (ax as i16) < (left as i16) {
            cx = cx.wrapping_add(ax.wrapping_sub(left));
            if (cx as i16) <= 0 {
                return;
            }
            ax = left;
        }
        let bp = ax.wrapping_add(cx).wrapping_sub(self.g16(RIGHT));
        if (bp as i16) > 0 {
            cx = cx.wrapping_sub(bp);
            if (cx as i16) <= 0 {
                return;
            }
        }
        let bp = bx.wrapping_add(dx).wrapping_sub(self.g16(BOTTOM));
        if (bp as i16) > 0 {
            dx = dx.wrapping_sub(bp);
            if (dx as i16) <= 0 {
                return;
            }
        }
        let mut di = self.row(bx).wrapping_add(ax);
        let mut mirror = self.row(bx.wrapping_sub(self.g16(0xC3D8))).wrapping_add(self.row(bx.wrapping_sub(self.g16(0xC3D8)))).wrapping_neg();
        let (w, mut rows) = (cx, dx);
        loop {
            for (colour, up) in [(0x1Eu8, 0u16), (0x67, 0x140)] {
                for k in 0..w {
                    let at = di.wrapping_add(k);
                    if self.page[at as usize] == colour {
                        let v = self.page[at.wrapping_add(mirror).wrapping_sub(up) as usize];
                        self.page[at as usize] = v;
                    }
                }
            }
            di = di.wrapping_add(0x140);
            mirror = mirror.wrapping_sub(0x280);
            rows = rows.wrapping_sub(1);
            if rows == 0 {
                break;
            }
        }
    }

    /// 0000:5656 (`Machine::frames_unpack`).
    fn unpack(&mut self, seg: u16, si: u16, frame: u16, dseg: u16, doff: u16) {
        self.m.frames_unpack(seg, si, frame, dseg, doff);
    }

    /// 0000:78A8 (`blit_rows`) onto the page.
    #[allow(clippy::too_many_arguments)]
    fn blit(&mut self, x: u16, y: u16, dw: u16, dh: u16, shade: (u16, u16), w: u16, h: u16, flags: u16, src: (u16, u16), bpp: u8, map: (u16, u16)) {
        if !blit_rows(&self.m, &mut self.page, x, y, dw, dh, shade, w, h, flags, src, bpp, map, &mut self.odd) {
            self.gap("9546: a light flat through 781E's colours");
        }
    }
}

/// CS:789E, the routine 78A8 has compile each row: 7A2B (`blit`) or,
/// after 953E, 9546 (`blit_light`).
const GENERATOR: u16 = 0x789E;
const PLAIN_ROWS: u16 = 0x7A2B;
const LIGHT_ROWS: u16 = 0x9546;

/// 0000:78A8 by the routine CS:789E names (`blit` or `blit_light`),
/// `odd` its local [BP-14h] as the stack holds it (7A2B sets it, 9546
/// reads it). False where 9546 would read the code segment through the
/// colours' table at CS:781E (not ported).
#[allow(clippy::too_many_arguments)]
pub(super) fn blit_rows(m: &Machine, page: &mut [u8], x: u16, y: u16, dw: u16, dh: u16, shade: (u16, u16), w: u16, h: u16, flags: u16, src: (u16, u16), bpp: u8, map: (u16, u16), odd: &mut u8) -> bool {
    let light = m.mem.r16(0, GENERATOR) == LIGHT_ROWS;
    if light {
        return blit_light(m, page, x, y, dw, dh, shade, w, h, flags, src, bpp, map, *odd);
    }
    if clip(m, x, y, dw, dh, w, h).is_some() {
        *odd = (bpp != 8) as u8;
    }
    blit(m, page, x, y, dw, dh, shade, w, h, flags, src, bpp, map);
    true
}

/// 78A8's steps and clipping (78B2..7955): the place, size and the
/// scale's start (x, y, dw, dh, fx, fy) and steps (xs, ys); None where it
/// returns before compiling a row.
fn clip(m: &Machine, x: u16, y: u16, dw: u16, dh: u16, w: u16, h: u16) -> Option<[u16; 8]> {
    if dw == 0 {
        return None;
    }
    let xs = ((w & 0xFF) << 8) / dw;
    if xs <= 4 || dh == 0 {
        return None;
    }
    let ys = ((h & 0xFF) << 8) / dh;
    if ys <= 4 {
        return None;
    }
    let (mut x, mut y, mut dw, mut dh) = (x, y, dw, dh);
    let (mut fx, mut fy) = (0u16, 0u16);
    let d = y.wrapping_sub(m.mem.r16(DS, TOP));
    if (d as i16) < 0 {
        dh = dh.wrapping_add(d);
        if (dh as i16) <= 0 {
            return None;
        }
        y = y.wrapping_sub(d);
        fy = d.wrapping_mul(ys).wrapping_neg();
    }
    let c = y.wrapping_add(dh).wrapping_sub(1).wrapping_sub(m.mem.r16(DS, BOTTOM));
    if (c as i16) > 0 {
        dh = dh.wrapping_sub(c);
        if (dh as i16) <= 0 {
            return None;
        }
    }
    let d = x.wrapping_sub(m.mem.r16(DS, LEFT));
    if (d as i16) < 0 {
        dw = dw.wrapping_add(d);
        if (dw as i16) <= 0 {
            return None;
        }
        x = x.wrapping_sub(d);
        fx = d.wrapping_mul(xs).wrapping_neg();
    }
    let c = x.wrapping_add(dw).wrapping_sub(1).wrapping_sub(m.mem.r16(DS, RIGHT));
    if (c as i16) > 0 {
        dw = dw.wrapping_sub(c);
        if (dw as i16) <= 0 {
            return None;
        }
    }
    Some([x, y, dw, dh, fx, fy, xs, ys])
}

/// 0000:78A8 with its rows compiled by 9546 (953E): each source byte
/// (8 bits whatever `bpp`, the rows `bpp` x `w` bits apart) and the
/// screen's pixel under it pick the pixel from the table at the shade's
/// segment, less 10h where the 0s are left out (9546 takes it from the
/// argument), at byte x 100h + the screen's (the shade's offset unused).
/// Under 8 bits a pixel with colours, the shade is the colours' where
/// it has none; else the table made at CS:781E, which isn't ported
/// (false). The rows start past the scale's whole part unless `odd`
/// (78A8's [BP-14h], 9546 not setting it).
#[allow(clippy::too_many_arguments)]
fn blit_light(m: &Machine, page: &mut [u8], x: u16, y: u16, dw: u16, dh: u16, shade: (u16, u16), w: u16, h: u16, flags: u16, src: (u16, u16), bpp: u8, map: (u16, u16), odd: u8) -> bool {
    let stride = ((bpp as u16 & 0xFF) * (w & 0xFF) + 7) >> 3 & 0xFF;
    let Some([x, y, dw, dh, fx, fy, xs, ys]) = clip(m, x, y, dw, dh, w, h) else { return true };
    let mut seg = shade.0;
    if bpp != 8 && map.0 != 0 {
        if shade.0 != 0 {
            return false;
        }
        seg = map.0;
    }
    let see = flags & 1 != 0;
    let mirror = flags & 2 != 0;
    if see {
        seg = seg.wrapping_sub(0x10);
    }
    let mut start = src.1;
    if odd == 0 {
        let hi = fx >> 8;
        start = start.wrapping_add(if mirror { hi.wrapping_neg().wrapping_add(w).wrapping_sub(1) } else { hi });
    }
    let mut dest = m.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
    let mut fy = fy;
    let mut rows = dh as u8;
    loop {
        let mut di = dest;
        let mut si = start.wrapping_add(((fy >> 8) as u8 as u16).wrapping_mul(stride));
        let mut bx = fx;
        let mut n = dw;
        loop {
            let t = m.mem.r8(src.0, si);
            if !(see && t == 0) {
                let under = page[di as usize];
                page[di as usize] = m.mem.r8(seg, (t as u16) << 8 | under as u16);
            }
            n = n.wrapping_sub(1);
            if n == 0 {
                break;
            }
            di = di.wrapping_add(1);
            let old = (bx >> 8) as u8;
            bx = bx.wrapping_add(xs);
            let delta = ((bx >> 8) as u8).wrapping_sub(old);
            let delta = if mirror { delta.wrapping_neg() } else { delta };
            si = si.wrapping_add(delta as i8 as i16 as u16);
        }
        dest = dest.wrapping_add(0x140);
        fy = fy.wrapping_add(ys);
        rows = rows.wrapping_sub(1);
        if rows == 0 {
            break;
        }
    }
    true
}

/// 0000:78A8: `w` x `h` pixels of `bpp` bits from `src` scaled to
/// `dw` x `dh` at (`x`, `y`), clipped to the window: as the code it
/// compiles at 3BA0:929E (7A2B) draws each row (`flags` bit 0 the 0s
/// left out, bit 1 mirrored; through the shade table where its
/// segment isn't 0). Under 8 bits a pixel, a pixel is read from the
/// row's big-endian word at its bit, and goes through the colours
/// `map` (where its segment isn't 0): as they are unshaded, else
/// through the shade (the table made at CS:781E).
#[allow(clippy::too_many_arguments)]
pub(super) fn blit(m: &Machine, page: &mut [u8], x: u16, y: u16, dw: u16, dh: u16, shade: (u16, u16), w: u16, h: u16, flags: u16, src: (u16, u16), bpp: u8, map: (u16, u16)) {
    if bpp != 8 {
        return blit_bits(m, page, x, y, dw, dh, shade, w, h, flags, src, bpp, map);
    }
    let stride = ((8 * (w & 0xFF)) + 7) >> 3;
    let xs = ((w & 0xFF) << 8) / dw.max(1);
    if dw == 0 || xs <= 4 {
        return;
    }
    let ys = ((h & 0xFF) << 8) / dh.max(1);
    if dh == 0 || ys <= 4 {
        return;
    }
    let (mut x, mut y, mut dw, mut dh) = (x, y, dw, dh);
    let (mut fx, mut fy) = (0u16, 0u16);
    let d = y.wrapping_sub(m.mem.r16(DS, TOP));
    if (d as i16) < 0 {
        dh = dh.wrapping_add(d);
        if (dh as i16) <= 0 {
            return;
        }
        y = y.wrapping_sub(d);
        fy = d.wrapping_mul(ys).wrapping_neg();
    }
    let c = y.wrapping_add(dh).wrapping_sub(1).wrapping_sub(m.mem.r16(DS, BOTTOM));
    if (c as i16) > 0 {
        dh = dh.wrapping_sub(c);
        if (dh as i16) <= 0 {
            return;
        }
    }
    let d = x.wrapping_sub(m.mem.r16(DS, LEFT));
    if (d as i16) < 0 {
        dw = dw.wrapping_add(d);
        if (dw as i16) <= 0 {
            return;
        }
        x = x.wrapping_sub(d);
        fx = d.wrapping_mul(xs).wrapping_neg();
    }
    let c = x.wrapping_add(dw).wrapping_sub(1).wrapping_sub(m.mem.r16(DS, RIGHT));
    if (c as i16) > 0 {
        dw = dw.wrapping_sub(c);
        if (dw as i16) <= 0 {
            return;
        }
    }
    let mirror = flags & 2 != 0;
    let see = flags & 1 != 0;
    let mut dest = m.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
    let hi = fx >> 8;
    let start = src.1.wrapping_add(if mirror { hi.wrapping_neg().wrapping_add(w).wrapping_sub(1) } else { hi });
    let mut rows = dh as u8;
    loop {
        let mut di = dest;
        let mut si = start.wrapping_add(((fy >> 8) as u8 as u16).wrapping_mul(stride & 0xFF));
        let mut bx = fx;
        let mut n = dw;
        loop {
            let al = m.mem.r8(src.0, si);
            if !(see && al == 0) {
                let c = if shade.0 != 0 { m.mem.r8(shade.0, shade.1.wrapping_add(al as u16)) } else { al };
                page[di as usize] = c;
            }
            n = n.wrapping_sub(1);
            if n == 0 {
                break;
            }
            di = di.wrapping_add(1);
            let old = (bx >> 8) as u8;
            bx = bx.wrapping_add(xs);
            let delta = ((bx >> 8) as u8).wrapping_sub(old);
            let delta = if mirror { delta.wrapping_neg() } else { delta };
            si = si.wrapping_add(delta as i8 as i16 as u16);
        }
        dest = dest.wrapping_add(0x140);
        fy = fy.wrapping_add(ys);
        rows = rows.wrapping_sub(1);
        if rows == 0 {
            break;
        }
    }
}

/// 0000:78A8 under 8 bits a pixel (`blit`): the compiled row reads
/// each pixel's bits from its column (the scale's whole part, from
/// the right when mirrored) in the row (the step's whole part times
/// the row's bytes, `bpp` x `w` bits rounded up).
#[allow(clippy::too_many_arguments)]
pub(super) fn blit_bits(m: &Machine, page: &mut [u8], x: u16, y: u16, dw: u16, dh: u16, shade: (u16, u16), w: u16, h: u16, flags: u16, src: (u16, u16), bpp: u8, map: (u16, u16)) {
    let stride = ((bpp as u16 * (w & 0xFF)) + 7) >> 3 & 0xFF;
    if dw == 0 {
        return;
    }
    let xs = ((w & 0xFF) << 8) / dw;
    if xs <= 4 || dh == 0 {
        return;
    }
    let ys = ((h & 0xFF) << 8) / dh;
    if ys <= 4 {
        return;
    }
    let (mut x, mut y, mut dw, mut dh) = (x, y, dw, dh);
    let (mut fx, mut fy) = (0u16, 0u16);
    let d = y.wrapping_sub(m.mem.r16(DS, TOP));
    if (d as i16) < 0 {
        dh = dh.wrapping_add(d);
        if (dh as i16) <= 0 {
            return;
        }
        y = y.wrapping_sub(d);
        fy = d.wrapping_mul(ys).wrapping_neg();
    }
    let c = y.wrapping_add(dh).wrapping_sub(1).wrapping_sub(m.mem.r16(DS, BOTTOM));
    if (c as i16) > 0 {
        dh = dh.wrapping_sub(c);
        if (dh as i16) <= 0 {
            return;
        }
    }
    let d = x.wrapping_sub(m.mem.r16(DS, LEFT));
    if (d as i16) < 0 {
        dw = dw.wrapping_add(d);
        if (dw as i16) <= 0 {
            return;
        }
        x = x.wrapping_sub(d);
        fx = d.wrapping_mul(xs).wrapping_neg();
    }
    let c = x.wrapping_add(dw).wrapping_sub(1).wrapping_sub(m.mem.r16(DS, RIGHT));
    if (c as i16) > 0 {
        dw = dw.wrapping_sub(c);
        if (dw as i16) <= 0 {
            return;
        }
    }
    // 795E..79A9: the colours, through the shade where it has one.
    let mut lut: Option<[u8; 256]> = None;
    if map.0 != 0 {
        let mut t = [0u8; 256];
        for (k, e) in t.iter_mut().enumerate().take(1 << bpp) {
            let c = m.mem.r8(map.0, map.1.wrapping_add(k as u16));
            *e = if shade.0 != 0 { m.mem.r8(shade.0, shade.1.wrapping_add(c as u16)) } else { c };
        }
        lut = Some(t);
    } else if shade.0 != 0 {
        let mut t = [0u8; 256];
        for (k, e) in t.iter_mut().enumerate() {
            *e = m.mem.r8(shade.0, shade.1.wrapping_add(k as u16));
        }
        lut = Some(t);
    }
    let mirror = flags & 2 != 0;
    let see = flags & 1 != 0;
    let mask = 0xFFu8 >> (8 - bpp);
    let mut dest = m.mem.r16(DS, ROWS.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
    let mut rows = dh as u8;
    loop {
        let base = src.1.wrapping_add(((fy >> 8) as u8 as u16).wrapping_mul(stride));
        let mut bx = fx;
        let mut di = dest;
        for _ in 0..dw {
            let mut col = (bx >> 8) as u8;
            if mirror {
                col = (w as u8).wrapping_sub(col).wrapping_sub(1);
            }
            let bit = col as u16 * bpp as u16;
            let at = base.wrapping_add(bit >> 3);
            let word = (m.mem.r8(src.0, at) as u16) << 8 | m.mem.r8(src.0, at.wrapping_add(1)) as u16;
            let sh = (16 - bpp as u16).wrapping_sub(bit & 7) & 0xF;
            let v = (word >> sh) as u8 & mask;
            if !(see && v == 0) {
                let c = match &lut {
                    Some(t) => t[v as usize],
                    None => v,
                };
                page[di as usize] = c;
            }
            di = di.wrapping_add(1);
            bx = bx.wrapping_add(xs);
        }
        dest = dest.wrapping_add(0x140);
        fy = fy.wrapping_add(ys);
        rows = rows.wrapping_sub(1);
        if rows == 0 {
            break;
        }
    }
}

/// 0000:AAA2 past its allocation, the table at `seg` (CCC2): 3AAC:0240's
/// 13 rows (the .LGT loaded there), then two of 16 zeros and the fill
/// colour (C3C6); CCD2's 15 rows in the EMS frame (F010h..); and in each
/// texture bank (C8F0, C8F1 of them, each mapped: the last is left so)
/// its shading rows at F000h (0000:B174): the table's 13 rows, then three
/// of the fill colour.
pub fn shade_tables(m: &mut Machine, seg: u16) {
    for i in 0..0xD00u16 {
        let b = m.mem.r8(0x3AAC, 0x240 + i);
        m.mem.w8(seg, i, b);
    }
    let fog = m.mem.r8(DS, 0xC3C6);
    let mut di = 0xD00u16;
    for _ in 0..2 {
        for _ in 0..0x10 {
            m.mem.w8(seg, di, 0);
            di += 1;
        }
        for _ in 0..0xF0 {
            m.mem.w8(seg, di, fog);
            di += 1;
        }
    }
    for i in 0..0xFu16 {
        m.mem.w16(DS, 0xCCD2 + 2 * i, 0xF010 + i * 0x100);
    }
    let first = m.mem.r8(DS, 0xC8F0) as u16;
    let n = m.mem.r8(DS, 0xC8F1) as u16;
    for b in first..first + n {
        m.ems_map(b);
        let frame = m.mem.r16(DS, 0x9030);
        let mut si = 0u16;
        for row in 0..16u16 {
            let d = 0xF000 + row * 0x100;
            for i in 0..0x100u16 {
                let v = if d < 0xFD00 {
                    let v = m.mem.r8(seg, si);
                    si += 1;
                    v
                } else {
                    fog
                };
                m.mem.w8(frame, d + i, v);
            }
        }
    }
}

/// The renderer's buffers the view's start doesn't make, in paragraphs:
/// the shade tables (CCC2), the rows' lights (CD86), the floor's and
/// ceiling's row tables (CD82, CD7E). The copy takes them from its own
/// heap each frame (the copy is dropped after), so they never sit where
/// the EMS frame does: at fixed segments F800h..FA80h they lay inside a
/// frame at F000h (the harness's), and the dungeon's textures read them.
const SHADES_PARAS: u16 = 0x100;
const LIGHTS_PARAS: u16 = 0x100;
const ROWS_PARAS: u16 = 0x40;

impl Soft {
    /// What the original's start and the 3D mode's entry set up for the
    /// renderer (21EE:0030's BA80, A561's buffers, AAA2's shade tables
    /// copied into each texture bank (B174), DA8E's row tables), made in
    /// the copy as they would be.
    fn setup(&mut self) {
        self.s8(0xBA80, 1);
        // AAA2.
        let Some((seg, _)) = self.m.heap_alloc(SHADES_PARAS + LIGHTS_PARAS + 2 * ROWS_PARAS) else {
            self.gap("AAA2: no heap for the renderer's buffers");
            return;
        };
        let (lights, floor, ceiling) = (seg + SHADES_PARAS, seg + SHADES_PARAS + LIGHTS_PARAS, seg + SHADES_PARAS + LIGHTS_PARAS + ROWS_PARAS);
        let machines = self.g16(0xCCC2);
        self.s16(0xCCC2, seg);
        self.table = seg;
        shade_tables(&mut self.m, seg);
        let first = self.g8(0xC8F0) as u16;
        let fog = self.g8(0xC3C6);
        self.m.ems_map(first);
        // The fog's table (2294:0C36, once its bit 80h of CB5C or F81 is
        // set) is AAA2's with its last two rows the fill colour.
        if (self.g8(0xCB5C) | self.g8(0x0F81)) & 0x80 != 0 {
            for i in 0..0x200u16 {
                self.m.mem.w8(seg, 0xD00 + i, fog);
            }
        }
        // The rows E00h and F00h as the machine's table has them: a
        // remapped creature's colours (25CA:013A, `place_creatures`), which
        // its light rows (953E) draw.
        if machines != 0 {
            for i in 0xE00..0x1000u16 {
                let b = self.m.mem.r8(machines, i);
                self.m.mem.w8(seg, i, b);
            }
        }
        // A561.
        self.s16(0xCD86, 0);
        self.s16(0xCD88, lights);
        self.s16(0xCD82, 0);
        self.s16(0xCD84, floor);
        self.s16(0xCD7E, 0);
        self.s16(0xCD80, ceiling);
        self.row_tables();
    }

    /// 0000:DA8E in the copy (`Machine::row_tables`).
    fn row_tables(&mut self) {
        self.m.row_tables();
    }
}

impl Machine {
    /// 0000:DA8E: the floor's row table at CD82 (each row below the
    /// horizon: the row's two ends on the floor, turned by the heading),
    /// then DB78 the ceiling's at CD7E. Each keeps its distance's
    /// product at A836, A838 (DAC3, DBCA).
    pub fn row_tables(&mut self) {
        let g = |m: &Machine, o: u16| m.mem.r16(DS, o);
        if g(self, 0xC398) & 0x4000 != 0 {
            return;
        }
        let mut h = g(self, 0xC3D2);
        if g(self, 0xC398) & 0x40 != 0 {
            h = h.wrapping_add(0x64);
        }
        if (h as i16) <= 0 {
            return;
        }
        let bp = g(self, 0xC3A2).wrapping_sub(g(self, 0xC3D8)).wrapping_add(2);
        let num = h as u32 * g(self, 0xCCF6) as u32;
        self.mem.w16(DS, 0xA836, num as u16);
        self.mem.w16(DS, 0xA838, (num >> 16) as u16);
        self.row_table(g(self, 0xCD82), g(self, 0xCD84), num, bp);
        // DB78.
        if g(self, 0xC39A) & 8 != 0 {
            return;
        }
        let (fo, fs, co, cs) = (g(self, 0xCD82), g(self, 0xCD84), g(self, 0xCD7E), g(self, 0xCD80));
        if g(self, 0xC3D2) == g(self, 0xD80C) >> 1 {
            for i in 0..0x320u16 {
                let b = self.mem.r8(fs, fo.wrapping_add(i));
                self.mem.w8(cs, co.wrapping_add(i), b);
            }
            return;
        }
        let mut bp = g(self, 0xC3A2).wrapping_sub(g(self, 0xC3D8)).wrapping_add(3);
        if bp > 0x64 {
            bp = 0x66;
        }
        let num = g(self, 0xD80C).wrapping_sub(g(self, 0xC3D2)) as u32 * g(self, 0xCCF6) as u32;
        self.mem.w16(DS, 0xA836, num as u16);
        self.mem.w16(DS, 0xA838, (num >> 16) as u16);
        // The divisions after take its low word alone.
        self.row_table(co, cs, num & 0xFFFF, bp);
    }

    /// DACA..DB3C: rows 2.. `bp` - 1 of a row table at `seg`:`si`, the
    /// distance `num` over the row.
    fn row_table(&mut self, mut si: u16, seg: u16, num: u32, bp: u16) {
        let mut cx = 2u16;
        let turn = self.mem.r16(DS, 0xC3DA).wrapping_neg().wrapping_add(0x1FF);
        while cx != bp {
            let d = (num / cx as u32) as u16;
            let x = muldiv(d, self.mem.r16(DS, LEFT).wrapping_sub(self.mem.r16(DS, 0xC3D6)), self.mem.r16(DS, 0xCCF4));
            let (a0, c0) = rot(self, turn, x, d);
            let (a1, c1) = rot(self, turn, x.wrapping_neg(), d);
            let sar = |v: u16| ((v as i16) >> 1) as u16;
            self.mem.w16(seg, si, sar(a0));
            self.mem.w16(seg, si.wrapping_add(2), sar(c0));
            self.mem.w16(seg, si.wrapping_add(4), sar(a1));
            self.mem.w16(seg, si.wrapping_add(6), sar(c1));
            si = si.wrapping_add(8);
            cx = cx.wrapping_add(1);
        }
    }
}

impl Machine {
    /// 0000:5656: frame `frame` of the packed frames at `seg`:`si` (the
    /// count first; from +0Ah the first frame, RLE (567A), then each
    /// next one's changes (56A8)) into `dseg`:`doff`.
    pub fn frames_unpack(&mut self, seg: u16, si: u16, frame: u16, dseg: u16, doff: u16) {
        if self.mem.r16(seg, si) <= frame {
            return;
        }
        let mut p = si.wrapping_add(0xA);
        let r8 = |m: &Machine, p: u16| m.mem.r8(seg, p);
        let r16 = |m: &Machine, p: u16| m.mem.r16(seg, p);
        // 567A.
        {
            let mut dx = r16(self, p);
            let mut q = p.wrapping_add(2);
            let mut di = doff;
            loop {
                let al = r8(self, q);
                q = q.wrapping_add(1);
                if al < 0x80 {
                    let n = al as u16 + 1;
                    for _ in 0..n {
                        let b = r8(self, q);
                        self.mem.w8(dseg, di, b);
                        q = q.wrapping_add(1);
                        di = di.wrapping_add(1);
                    }
                    dx = dx.wrapping_sub(1).wrapping_sub(n);
                } else {
                    let n = (al & 0x7F) as u16 + 1;
                    let v = r8(self, q);
                    q = q.wrapping_add(1);
                    for _ in 0..n {
                        self.mem.w8(dseg, di, v);
                        di = di.wrapping_add(1);
                    }
                    dx = dx.wrapping_sub(2);
                }
                if dx == 0 {
                    break;
                }
            }
        }
        if frame == 0 {
            return;
        }
        for _ in 0..frame {
            p = p.wrapping_add(r16(self, p)).wrapping_add(2);
        }
        // 56A8.
        let mut q = p.wrapping_add(2);
        let n = r16(self, q);
        q = q.wrapping_add(2);
        for _ in 0..n {
            let off = r16(self, q);
            let len = r16(self, q.wrapping_add(2));
            q = q.wrapping_add(4);
            for k in 0..len {
                let b = r8(self, q);
                self.mem.w8(dseg, doff.wrapping_add(off).wrapping_add(k), b);
                q = q.wrapping_add(1);
            }
        }
    }
}
