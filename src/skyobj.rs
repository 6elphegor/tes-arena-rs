//! The sky's objects (segment 2294, with 292B): the distant mountains
//! and the clouds outdoors, a volcano seen from afar, and lightning.
//!
//! Entering an area outdoors (0000:464A) makes them (`sky_objects`,
//! 2294:08B9), and the night's relight (0000:027A) and the day switch
//! (0000:033D) make them again. They come from the location's seed
//! (A56), so an area always has the same ones: two to five mountains by
//! the location's climate (A50: mount001..011, desert1..4 or
//! temp01..10.img), each at a random heading on a circle of 20000 units,
//! and in clear weather seven clouds (cloud01..17.img; the seed moved on
//! by the day, CCA8), each also given a random height (its image's y,
//! 0..63). The images go to the sky's EMS bank (C8F8), which is emptied
//! first; the bank table is kept aside meanwhile so they land nowhere
//! else (0000:FEF4, FECE).
//!
//! Each frame the hook at CD1A (2294:0A5D, `sky_draw`) draws them over the
//! sky: turned to the player's heading and projected (292B:057F,
//! `sky_project`; behind the player, nothing), a mountain standing on the
//! horizon, a cloud at its height, after the volcano (292B:0201) and
//! before lightning (2294:05A6, `lightning`).

use crate::machine::Machine;
use crate::mem::DS;

/// What the sky's objects leave to the front end.
pub trait SkyHooks {
    /// 0000:7428 with DI FFFDh: the file named at DS:`name` loaded into
    /// EMS (0000:5C26 allocates; the bank is mapped). Its (segment,
    /// offset), the segment the frame's (9030); None where it can't be
    /// loaded (the original returns 0:0).
    fn load(&mut self, _m: &mut Machine, _name: u16) -> Option<(u16, u16)> {
        None
    }
    /// 2294:0B01: in fog (or with the clock stopped, CB5C) the view is
    /// filled with the fog's colour instead, and nothing is drawn (carry).
    /// Not ported here: it belongs to the weather's light.
    fn fog(&mut self, _m: &mut Machine) -> bool {
        false
    }
    /// 0000:5828: the image whose header is at seg:off drawn at the
    /// header's x and y (+0, +2), clipped to the view (A914..A91A).
    fn draw_image(&mut self, _m: &mut Machine, _seg: u16, _off: u16) {}
    /// 0000:5652 then 5A66: frame `frame` of the DFA at seg:off
    /// decoded to the buffer at BA54 and drawn there with its top left at
    /// (`x`, `y`).
    fn draw_dfa(&mut self, _m: &mut Machine, _seg: u16, _off: u16, _frame: u16, _x: u16, _y: u16) {}
    /// 0000:9A6E: the rectangle (x0, y0)..(x1, y1) filled with the
    /// colour at 9288.
    fn fill(&mut self, _m: &mut Machine, _x0: u16, _y0: u16, _x1: u16, _y1: u16) {}
    /// 0000:78A8: frame `frame` of the lightning bolt (a CFA whose header
    /// is at seg:off; +6, +8 its x and y) drawn.
    fn bolt(&mut self, _m: &mut Machine, _seg: u16, _off: u16, _frame: u16) {}
    /// 23BA:002A: sound `sound` played at square `at` (CB90's form).
    fn sound(&mut self, m: &mut Machine, sound: u16, at: u16) {
        let _ = m.sound_at(sound, at);
    }
}

/// `SkyHooks` that load nothing.
pub struct NoSkyHooks;

impl SkyHooks for NoSkyHooks {}

/// DGROUP variables.
pub mod var {
    /// The mountains (6 bytes each: the image's offset in the frame, x,
    /// y) and their count (a byte).
    pub const OBJECTS: u16 = 0x42BA;
    pub const OBJECT_COUNT: u16 = 0x42B9;
    /// The clouds, the same, and their count (a byte).
    pub const CLOUDS: u16 = 0x4396;
    pub const CLOUD_COUNT: u16 = 0x438C;
    /// The name being made: where its number goes, how many there are,
    /// the name.
    pub const DIGIT_AT: u16 = 0xA836;
    pub const VARIANTS: u16 = 0xA838;
    pub const NAME: u16 = 0xA83A;
    /// Set while the objects are made.
    pub const MAKING: u16 = 0x41C6;
    /// The sky's EMS bank, and where the bank table is kept meanwhile.
    pub const BANK: u16 = 0xC8F8;
    pub const BANK_SAVE: u16 = 0x929E;
    /// The location's climate and seed, the day for the clouds.
    pub const CLIMATE: u16 = 0x0A50;
    pub const SEED: u16 = 0x0A56;
    pub const DAY: u16 = 0xCCA8;
    /// The volcano: 0 none, 2 near (an animated DFA), 3 far (an IMG);
    /// its x and y on the circle; its image (far pointer).
    pub const VOLCANO: u16 = 0x7BD2;
    pub const VOLCANO_AT: u16 = 0x7BCA;
    pub const VOLCANO_IMAGE: u16 = 0x7BCE;
    /// The view's centre column and the horizon's row, the projection's
    /// scale, the view's clip (left, top, right, bottom).
    pub const CENTRE: u16 = 0xC3D6;
    pub const HORIZON: u16 = 0xC3D8;
    pub const SCALE: u16 = 0xCCF4;
    pub const CLIP_LEFT: u16 = 0xA914;
    pub const CLIP_RIGHT: u16 = 0xA918;
    pub const CLIP_BOTTOM: u16 = 0xA91A;
    /// Lightning: its frame (FFFFh none), its image's offset (in the
    /// frame), its x and y on the circle; the bolts' images (6 far
    /// pointers), the flash's colours per frame, the fill colour.
    pub const BOLT_FRAME: u16 = 0x43C6;
    pub const BOLT_IMAGE: u16 = 0x43C8;
    pub const BOLT_AT: u16 = 0x43C2;
    pub const BOLTS: u16 = 0xD8AE;
    pub const FLASH: u16 = 0x420C;
    pub const FILL: u16 = 0x9288;
}
use var::*;

/// The sky's images (0000:E5C9): the sun's, the moons', the bolts' and
/// the stars' names (the bolts' and stars' digits counted up), the stars'
/// images' offsets, the star field and its clusters, the sky's bank.
const SUN: u16 = 0xD869;
const MOON1: u16 = 0xD871;
const MOON2: u16 = 0xD87C;
const STAR_NAME: u16 = 0xD890;
const BOLT_NAME: u16 = 0xD8A2;
const STAR_IMAGES: u16 = 0xE032;
const STAR_FIELD: u16 = 0xD8C6;
const CLUSTERS: u16 = 0xDA56;
const SKY_BANK: u16 = 0xC8F3;
const OPEN_SIZE: u16 = 0xA90C;

const SIN: u16 = 0xB03C;
const COS: u16 = 0xB13C;
const BANK_TABLE: u16 = 0x8FAA;
const FRAME_SEG: u16 = 0x9030;

impl Machine {
    /// 0000:464A..466C, entering an area: no objects (OBJECT_COUNT 0);
    /// unless 5C8 bit 800h, outdoors (C398 bit 4) or in the wild (C39A
    /// bit 200h) the sky's objects are made. Returns false where the
    /// original loads the palette named at 1304 instead (left to the
    /// front end).
    pub fn area_sky(&mut self, hooks: &mut dyn SkyHooks) -> bool {
        self.mem.w8(DS, OBJECT_COUNT, 0);
        if self.mem.r16(DS, 0x05C8) & 0x800 != 0 {
            return false;
        }
        if self.mem.r16(DS, 0xC39A) & 0x200 == 0 && self.mem.r16(DS, 0xC398) & 4 == 0 {
            return false;
        }
        self.sky_objects(hooks);
        true
    }

    /// 2294:08B9. The sky's objects made, from the location's seed (the
    /// game's seed is put back after): the volcano (`volcano`), two to
    /// five mountains, and in clear weather (BB09 0) seven clouds with
    /// the seed moved on by the day. See the module's notes.
    pub fn sky_objects(&mut self, hooks: &mut dyn SkyHooks) {
        self.mem.w8(DS, MAKING, 1);
        let (at, n, name) = match self.mem.r8(DS, CLIMATE) {
            4 | 1 => (6, 0xB, 0x42DE),
            3 | 5 => (6, 4, 0x42EB),
            _ => (4, 0xA, 0x42F7),
        };
        self.mem.w16(DS, DIGIT_AT, at);
        self.mem.w16(DS, VARIANTS, n);
        self.mem.w16(DS, NAME, name);
        self.store_rng();
        let saved = self.mem.r32(DS, crate::random::SEED);
        let seed = self.mem.r32(DS, SEED);
        self.mem.w32(DS, crate::random::SEED, seed);
        self.load_rng();
        let bank = self.mem.r8(DS, BANK) as u16;
        self.mem.w16(DS, BANK_TABLE.wrapping_add(bank.wrapping_mul(2)), 0xFFFF);
        // 0000:FEF4: the table kept, then every bank but the sky's full.
        for i in 0..0x20u16 {
            let v = self.mem.r16(DS, BANK_TABLE + 2 * i);
            self.mem.w16(DS, BANK_SAVE + 2 * i, v);
        }
        self.bank_table_except(bank, |m, at| m.mem.w16(DS, at, 0));
        self.volcano(hooks);
        let n = (self.rng.next() & 3) + 2;
        self.sky_place(hooks, OBJECTS, false, n);
        self.mem.w8(DS, CLOUD_COUNT, 0);
        if self.mem.r8(DS, 0xBB09) == 0 {
            self.store_rng();
            let s = self.mem.r32(DS, crate::random::SEED).wrapping_add(self.mem.r16(DS, DAY) as u32);
            self.mem.w32(DS, crate::random::SEED, s);
            self.load_rng();
            self.mem.w16(DS, DIGIT_AT, 5);
            self.mem.w16(DS, VARIANTS, 0x11);
            self.mem.w16(DS, NAME, 0x4302);
            let objects = self.mem.r8(DS, OBJECT_COUNT);
            self.mem.w8(DS, OBJECT_COUNT, 0);
            self.sky_place(hooks, CLOUDS, true, 7);
            let clouds = self.mem.r8(DS, OBJECT_COUNT);
            self.mem.w8(DS, CLOUD_COUNT, clouds);
            self.mem.w8(DS, OBJECT_COUNT, objects);
        }
        // 0000:FECE: the table back but for the sky's bank.
        self.bank_table_except(bank, |m, at| {
            let v = m.mem.r16(DS, at.wrapping_sub(BANK_TABLE).wrapping_add(BANK_SAVE));
            m.mem.w16(DS, at, v);
        });
        self.mem.w32(DS, crate::random::SEED, saved);
        self.load_rng();
        self.mem.w8(DS, MAKING, 0);
    }

    /// The bank table's 32 entries but bank `bank`'s (0000:FEF4's and
    /// FECE's loop: AX doubled, the entries below it or from it + 2 on).
    fn bank_table_except(&mut self, bank: u16, mut f: impl FnMut(&mut Machine, u16)) {
        let lo = bank.wrapping_add(bank);
        let hi = lo.wrapping_add(2);
        for bx in (0..0x40u16).step_by(2) {
            if bx < lo || bx >= hi {
                f(self, BANK_TABLE + bx);
            }
        }
    }

    /// 2294:09BA. `n` tries at an object: a variant numbered from the
    /// seed (1..VARIANTS: 0 becomes the last), put in the name (two digits
    /// from 10 variants on), loaded; a loaded one gets (clouds) a height
    /// 0..63 as its image's y, then a heading 0..511 and its place on a
    /// circle of 20000 (each axis the high word of 10000 * the sine,
    /// doubled), entered at `di` and counted in OBJECT_COUNT. A file that
    /// doesn't load uses up its try.
    fn sky_place(&mut self, hooks: &mut dyn SkyHooks, mut di: u16, cloud: bool, n: u16) {
        for _ in 0..n {
            let count = self.mem.r16(DS, VARIANTS);
            let mut v = self.rng.next() % count;
            if v == 0 {
                v = count;
            }
            let (tens, units) = ((v / 10) as u8 + 0x30, (v % 10) as u8 + 0x30);
            let at = self.mem.r16(DS, NAME).wrapping_add(self.mem.r16(DS, DIGIT_AT));
            if count < 10 {
                self.mem.w8(DS, at, units);
            } else {
                self.mem.w8(DS, at, tens);
                self.mem.w8(DS, at.wrapping_add(1), units);
            }
            let name = self.mem.r16(DS, NAME);
            let loaded = hooks.load(self, name);
            let Some((_, off)) = loaded.filter(|&(s, o)| s | o != 0) else {
                continue;
            };
            if cloud {
                let y = self.rng.next() & 0x3F;
                let frame = self.mem.r16(DS, FRAME_SEG);
                self.mem.w16(frame, off.wrapping_add(2), y);
            }
            self.mem.w16(DS, di, off);
            let a = self.rng.next() & 0x1FF;
            let (x, y) = self.on_circle(a);
            self.mem.w16(DS, di.wrapping_add(2), x);
            self.mem.w16(DS, di.wrapping_add(4), y);
            let c = self.mem.r8(DS, OBJECT_COUNT).wrapping_add(1);
            self.mem.w8(DS, OBJECT_COUNT, c);
            di = di.wrapping_add(6);
        }
    }

    /// The point at `heading` on the sky's circle: the high words of
    /// 10000 * the sine and the cosine, doubled.
    fn on_circle(&self, heading: u16) -> (u16, u16) {
        let at = |t: u16| {
            let s = self.mem.r16(DS, t.wrapping_add(heading.wrapping_mul(2))) as i16 as i32;
            (((10000 * s) >> 16) as u16).wrapping_add(((10000 * s) >> 16) as u16)
        };
        (at(SIN), at(COS))
    }

    /// 292B:029A. The volcano, seen from province 3 outdoors: its place
    /// (84h, 34h) is near (the larger axis plus a quarter of the smaller
    /// under 96h; volcano2.dfa, VOLCANO 2) or far (volcano3.img, 3) from
    /// the location's (A48, A4A), loaded, and put on the sky's circle
    /// toward it (0000:1226 with its arguments as the original passes
    /// them: the x from the location to the volcano, the y the other way).
    pub fn volcano(&mut self, hooks: &mut dyn SkyHooks) {
        self.mem.w8(DS, VOLCANO, 0);
        if self.mem.r8(DS, 0x0A46) != 3 || self.mem.r16(DS, 0xC398) & 4 == 0 {
            return;
        }
        let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
        let (lx, ly) = (self.mem.r16(DS, 0x0A48), self.mem.r16(DS, 0x0A4A));
        let a = abs(0x84u16.wrapping_sub(lx));
        let b = abs(0x34u16.wrapping_sub(ly));
        let (lo, hi) = if a > b { (b, a) } else { (a, b) };
        let d = (lo >> 2).wrapping_add(hi);
        let n = if d < 0x96 { 1 } else { 2 };
        self.mem.w8(DS, VOLCANO, n as u8 + 1);
        let name = self.skip_strings(DS, 0x7BD3, n);
        let (seg, off) = hooks.load(self, name).unwrap_or((0, 0));
        self.mem.w16(DS, VOLCANO_IMAGE, off);
        self.mem.w16(DS, VOLCANO_IMAGE + 2, seg);
        let h = crate::travel::travel_heading(0x84, ly, lx, 0x34);
        let (x, y) = self.on_circle(h);
        self.mem.w16(DS, VOLCANO_AT, x);
        self.mem.w16(DS, VOLCANO_AT + 2, y);
    }

    /// 292B:057F. A point (`x`, `y`) of the sky's circle on the screen:
    /// turned by the player's heading (0000:93B8: each product's high
    /// word, the arguments doubled), nothing unless more than 64h ahead;
    /// then the column CENTRE + across * SCALE / ahead (0000:E02E), the
    /// row the horizon's. Some(None) where it is behind, None where the
    /// division overflows (the original faults).
    pub fn sky_project(&self, x: u16, y: u16) -> Option<Option<(u16, u16)>> {
        let h = self.mem.r16(DS, crate::movement::var::HEADING).wrapping_mul(2);
        let sin = self.mem.r16(DS, SIN.wrapping_add(h)) as i16 as i32;
        let cos = self.mem.r16(DS, COS.wrapping_add(h)) as i16 as i32;
        let (x, y) = (x.wrapping_mul(2) as i16 as i32, y.wrapping_mul(2) as i16 as i32);
        let hi = |p: i32| (p >> 16) as u16;
        // `neg` with no overflow; -8000h stays and is made 7FFFh.
        let nsin = if sin == -0x8000 { 0x7FFF } else { -sin };
        let across = hi(x * cos).wrapping_add(hi(y * nsin));
        let ahead = hi(x * sin).wrapping_add(hi(y * cos));
        if (ahead as i16) <= 0x64 {
            return Some(None);
        }
        let q = (across as i16 as i32 * self.mem.r16(DS, SCALE) as i16 as i32) / ahead as i16 as i32;
        if !(-0x8000..=0x7FFF).contains(&q) {
            return None;
        }
        Some(Some(((q as u16).wrapping_add(self.mem.r16(DS, CENTRE)), self.mem.r16(DS, HORIZON))))
    }

    /// 2294:0A5D, the hook at CD1A: the sky's objects drawn, unless in fog
    /// (`SkyHooks::fog`), outdoors (C398 bit 4): the sky's bank mapped,
    /// the volcano (`volcano_draw`), the clouds, the mountains, then
    /// lightning (`lightning`). Each object seen (`sky_project`) has its
    /// image's x set to centre it there (left so) and its flag 800h set;
    /// a mountain stands on the horizon (its y, put back after); the view's
    /// bottom is the horizon less 1 meanwhile. None where a projection
    /// or the volcano's frame divides wrongly.
    pub fn sky_draw(&mut self, hooks: &mut dyn SkyHooks) -> Option<()> {
        if hooks.fog(self) || self.mem.r16(DS, 0xC398) & 4 == 0 {
            return Some(());
        }
        let bank = self.mem.r8(DS, BANK) as u16;
        self.ems_map(bank);
        self.volcano_draw(hooks)?;
        for (list, count, cloud) in [(CLOUDS, CLOUD_COUNT, true), (OBJECTS, OBJECT_COUNT, false)] {
            let n = self.mem.r8(DS, count);
            if n == 0 {
                continue;
            }
            let bottom = self.mem.r16(DS, CLIP_BOTTOM);
            let v = self.mem.r16(DS, HORIZON).wrapping_sub(1);
            self.mem.w16(DS, CLIP_BOTTOM, v);
            for i in 0..n as u16 {
                let si = list + 6 * i;
                let at = self.sky_project(self.mem.r16(DS, si + 2), self.mem.r16(DS, si + 4));
                let Some(at) = at else {
                    self.mem.w16(DS, CLIP_BOTTOM, bottom);
                    return None;
                };
                let Some((ax, bx)) = at else { continue };
                let off = self.mem.r16(DS, si);
                let frame = self.mem.r16(DS, FRAME_SEG);
                let y = self.mem.r16(frame, off.wrapping_add(2));
                let bx = bx.wrapping_sub(self.mem.r16(frame, off.wrapping_add(6))).wrapping_add(1);
                let ax = ax.wrapping_sub(self.mem.r16(frame, off.wrapping_add(4)) >> 1);
                if !cloud {
                    self.mem.w16(frame, off.wrapping_add(2), bx);
                }
                self.mem.w16(frame, off, ax);
                let f = self.mem.r16(frame, off.wrapping_add(8)) | 0x800;
                self.mem.w16(frame, off.wrapping_add(8), f);
                hooks.draw_image(self, frame, off);
                self.mem.w16(frame, off.wrapping_add(2), y);
            }
            self.mem.w16(DS, CLIP_BOTTOM, bottom);
        }
        self.lightning(hooks)
    }

    /// 292B:0201. The volcano drawn where it is seen: far, its image
    /// standing on the horizon (one row lower than a mountain; its y put
    /// back after); near, the frame counter's (CCBC) frame of its DFA, centred,
    /// when it overlaps the view (A914..A918).
    pub fn volcano_draw(&mut self, hooks: &mut dyn SkyHooks) -> Option<()> {
        let mode = self.mem.r8(DS, VOLCANO);
        if mode == 0 {
            return Some(());
        }
        let at = self.sky_project(self.mem.r16(DS, VOLCANO_AT), self.mem.r16(DS, VOLCANO_AT + 2))?;
        let Some((ax, bx)) = at else { return Some(()) };
        let off = self.mem.r16(DS, VOLCANO_IMAGE);
        let seg = self.mem.r16(DS, VOLCANO_IMAGE + 2);
        if mode == 3 {
            let y = self.mem.r16(seg, off.wrapping_add(2));
            let bx = bx.wrapping_sub(self.mem.r16(seg, off.wrapping_add(6))).wrapping_add(2);
            let ax = ax.wrapping_sub(self.mem.r16(seg, off.wrapping_add(4)) >> 1);
            self.mem.w16(seg, off.wrapping_add(2), bx);
            self.mem.w16(seg, off, ax);
            hooks.draw_image(self, seg, off);
            self.mem.w16(seg, off.wrapping_add(2), y);
            return Some(());
        }
        let w = self.mem.r16(seg, off.wrapping_add(6));
        let bx = bx.wrapping_sub(self.mem.r16(seg, off.wrapping_add(8)));
        let ax = ax.wrapping_sub(w >> 1);
        if ax as i16 >= self.mem.r16(DS, CLIP_RIGHT) as i16 {
            return Some(());
        }
        if (ax.wrapping_add(w) as i16) <= self.mem.r16(DS, CLIP_LEFT) as i16 {
            return Some(());
        }
        let frames = self.mem.r16(seg, off);
        if frames == 0 {
            return None;
        }
        let frame = self.mem.r16(DS, 0xCCBC) % frames;
        hooks.draw_dfa(self, seg, off, frame, ax, bx.wrapping_add(1));
        Some(())
    }

    /// 2294:05A6. Lightning, outdoors in rain (BB09 low bits 1) at night
    /// (time of day outside 100h..300h) with 5C8 bit 1: while a bolt
    /// shows (BOLT_FRAME 0..2 and an image), the sky above the horizon
    /// flashes (FLASH's colour for the frame, `fill`), and the bolt, where
    /// it is seen, is drawn there standing on the horizon (its x left so;
    /// its y put back, by the original into DGROUP at the image's offset
    /// + 8, not into the image: kept); after three frames, thunder (sound
    /// 1Ch at the player's square, CB90). Otherwise one time in 32 or so
    /// (a roll up to 7D0h) a bolt starts: one of the six images, a heading
    /// on the circle.
    pub fn lightning(&mut self, hooks: &mut dyn SkyHooks) -> Option<()> {
        if self.mem.r16(DS, 0xC398) & 4 == 0 || self.mem.r8(DS, 0xBB09) & 0x7F != 1 {
            return Some(());
        }
        let t = self.mem.r16(DS, 0xCCFE);
        if (0x100..=0x300).contains(&t) || self.mem.r16(DS, 0x05C8) & 1 == 0 {
            return Some(());
        }
        let k = self.mem.r16(DS, BOLT_FRAME);
        let image = self.mem.r16(DS, BOLT_IMAGE);
        if (k as i16) < 0 || image == 0 {
            let r = self.rng.next();
            if r > 0x7D0 {
                return Some(());
            }
            let b = (r & 0xFF) % 6;
            let v = self.mem.r16(DS, BOLTS.wrapping_add(b * 4));
            self.mem.w16(DS, BOLT_IMAGE, v);
            self.mem.w16(DS, BOLT_FRAME, 0);
            let a = self.rng.next() & 0x1FF;
            let (x, y) = self.on_circle(a);
            self.mem.w16(DS, BOLT_AT, x);
            self.mem.w16(DS, BOLT_AT + 2, y);
            return Some(());
        }
        let c = self.mem.r8(DS, k.wrapping_add(FLASH));
        self.mem.w8(DS, FILL, c);
        let h = self.mem.r16(DS, HORIZON);
        hooks.fill(self, 0, 0, 0x140, h);
        let at = self.sky_project(self.mem.r16(DS, BOLT_AT), self.mem.r16(DS, BOLT_AT + 2))?;
        if let Some((ax, bx)) = at {
            let bank = self.mem.r8(DS, 0xC8F3) as u16;
            self.ems_map(bank);
            let fs = self.mem.r16(DS, FRAME_SEG);
            let si = image;
            let y = self.mem.r16(fs, si.wrapping_add(8));
            let bx = bx.wrapping_sub(self.mem.r16(fs, si.wrapping_add(2))).wrapping_add(1);
            let ax = ax.wrapping_sub(self.mem.r16(fs, si) >> 1);
            self.mem.w16(fs, si.wrapping_add(8), bx);
            self.mem.w16(fs, si.wrapping_add(6), ax);
            hooks.bolt(self, fs, si, k);
            self.mem.w16(DS, si.wrapping_add(8), y);
        }
        let k = self.mem.r16(DS, BOLT_FRAME).wrapping_add(1);
        self.mem.w16(DS, BOLT_FRAME, k);
        if k >= 3 {
            self.mem.w16(DS, BOLT_FRAME, 0xFFFF);
            let at = self.mem.r16(DS, 0xCB90);
            hooks.sound(self, 0x1C, at);
        }
        Some(())
    }

    /// 0000:E5C9. Outdoors (C398 bit 4), with a sky's bank (C8F3, not
    /// negative): its images loaded into that bank, mapped. CB36 FFh; the
    /// sun (SUN, sun.img) at frame:8000, its far pointer at CD92; the two
    /// moons' sizes from their headers (`moon_size`, 0000:E6AF: E04A/E04C,
    /// E04E/E050), the room for their frames after the sun (far pointers
    /// at CCAA, CCAE); the six lightning bolts (lglit01..06.cfa) after
    /// that, each one's offset in BOLTS (high word FFFFh); the eight stars
    /// (star1..8.img) after them, their offsets at STAR_IMAGES; then the
    /// star field (`star_field`, 0000:E77A). None where a file is missing
    /// (the original goes on with 0:0).
    pub fn sky_images(&mut self, files: &mut crate::bsa::Files) -> Option<()> {
        if self.mem.r16(DS, 0xC398) & 4 == 0 || self.mem.r8(DS, SKY_BANK) & 0x80 != 0 {
            return Some(());
        }
        self.ems_map(self.mem.r8(DS, SKY_BANK) as u16);
        self.mem.w8(DS, 0xCB36, 0xFF);
        let frame = self.mem.r16(DS, FRAME_SEG);
        self.file_load(files, SUN, frame, 0x8000)?;
        self.mem.w32(DS, 0xCD92, (frame as u32) << 16 | 0x8000);
        let bp = self.mem.r16(DS, OPEN_SIZE);
        let (w, h) = self.moon_size(files, MOON1)?;
        self.mem.w16(DS, 0xE04A, w);
        self.mem.w16(DS, 0xE04C, h);
        let first = w.wrapping_mul(h);
        self.mem.w32(DS, 0xCCAA, (frame as u32) << 16 | bp.wrapping_add(0x8000) as u32);
        let (w, h) = self.moon_size(files, MOON2)?;
        self.mem.w16(DS, 0xE04E, w);
        self.mem.w16(DS, 0xE050, h);
        let second = first.wrapping_add(bp).wrapping_add(0x8000);
        self.mem.w32(DS, 0xCCAE, (frame as u32) << 16 | second as u32);
        let mut di = second.wrapping_add(w.wrapping_mul(h));
        self.mem.w8(DS, BOLT_NAME + 6, b'1');
        for i in 0..6u16 {
            self.file_load(files, BOLT_NAME, frame, di)?;
            self.mem.w16(DS, BOLTS + 4 * i, di);
            self.mem.w16(DS, BOLTS + 4 * i + 2, 0xFFFF);
            di = di.wrapping_add(self.mem.r16(DS, OPEN_SIZE));
            let d = self.mem.r8(DS, BOLT_NAME + 6).wrapping_add(1);
            self.mem.w8(DS, BOLT_NAME + 6, d);
        }
        self.mem.w8(DS, STAR_NAME + 4, b'1');
        for i in 0..8u16 {
            self.file_load(files, STAR_NAME, frame, di)?;
            self.mem.w16(DS, STAR_IMAGES + 2 * i, di);
            di = di.wrapping_add(self.mem.r16(DS, OPEN_SIZE));
            let d = self.mem.r8(DS, STAR_NAME + 4).wrapping_add(1);
            self.mem.w8(DS, STAR_NAME + 4, d);
        }
        self.star_field()
    }

    /// 0000:E6AF. A moon's .DFA header (10 bytes) read to 929E: its width
    /// and height (+6, +8).
    fn moon_size(&mut self, files: &mut crate::bsa::Files, name: u16) -> Option<(u16, u16)> {
        self.file_open(files, name)?;
        self.file_read(files, DS, 0x929E, 0xA);
        self.file_close(files);
        Some((self.mem.r16(DS, 0x92A4), self.mem.r16(DS, 0x92A6)))
    }

    /// 0000:E77A. The night sky's stars from the area's seed (CB6E; the
    /// game's seed put back after): 28h of 10 bytes at STAR_FIELD, each a
    /// place (three of `star_coordinate`), then, one time in four, a
    /// single star of one of eight images (+6 0, +8 the image: 5..7 once
    /// each, marked at D89F..D8A1), else a cluster (+6 its count, 2..5;
    /// +8 its points, 6 bytes each from CLUSTERS: two offsets, rng >> 9
    /// signed, and a colour 40h + rng % 10). Where the clusters reach
    /// STAR_IMAGES it stops, through an INT 3 (0000:E83B, which DOS's
    /// handler passes over).
    fn star_field(&mut self) -> Option<()> {
        self.store_rng();
        let saved = self.mem.r32(DS, crate::random::SEED);
        let seed = self.mem.r32(DS, 0xCB6E);
        self.mem.w32(DS, crate::random::SEED, seed);
        self.load_rng();
        for i in 0..3u16 {
            self.mem.w8(DS, 0xD89F + i, 0);
        }
        let mut si = STAR_FIELD;
        let mut di = CLUSTERS;
        'stars: for _ in 0..0x28 {
            for k in 0..3u16 {
                let v = self.star_coordinate();
                self.mem.w16(DS, si + 2 * k, v);
            }
            if self.rng.next() & 3 == 0 {
                self.mem.w8(DS, si + 6, 0);
                let bx = loop {
                    let bx = self.rng.next() & 7;
                    if bx < 5 {
                        break bx;
                    }
                    if self.mem.r8(DS, 0xD89A + bx) == 0 {
                        self.mem.w8(DS, 0xD89A + bx, 1);
                        break bx;
                    }
                };
                self.mem.w16(DS, si + 8, bx);
            } else {
                if di >= STAR_IMAGES {
                    break 'stars;
                }
                let n = (self.rng.next() & 3) + 2;
                self.mem.w16(DS, si + 6, n);
                self.mem.w16(DS, si + 8, di);
                for _ in 0..n {
                    let x = ((self.rng.next() as i16) >> 9) as u16;
                    self.mem.w16(DS, di, x);
                    let y = ((self.rng.next() as i16) >> 9) as u16;
                    self.mem.w16(DS, di + 2, y);
                    let c = self.rng.next() % 10 + 0x40;
                    self.mem.w16(DS, di + 4, c);
                    di += 6;
                    if di >= STAR_IMAGES {
                        break 'stars;
                    }
                }
            }
            si += 10;
        }
        self.store_rng();
        self.mem.w32(DS, crate::random::SEED, saved);
        self.load_rng();
        Some(())
    }

    /// 0000:E83D with DL 2: a star's coordinate, 800h..17FFh, negative
    /// where bit 1 of the low byte is set.
    fn star_coordinate(&mut self) -> u16 {
        let v = (self.rng.next() & 0xFFF) + 0x800;
        if v & 2 != 0 { v.wrapping_neg() } else { v }
    }
}
