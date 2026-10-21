//! Differential tests of the sky's bodies (src/celestial.rs `celestial`,
//! 0000:E41A) against vectors from the original code
//! (harness/gen_celestial.py).
mod common;

use arena::celestial::CelestialHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FRAME: u16 = 0x6000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The generator's stubs, logging as its code hooks do.
#[derive(Default)]
struct Hooks {
    log: Vec<u8>,
}

impl Hooks {
    fn log(&mut self, ws: &[u16]) {
        for w in ws {
            self.log.extend(w.to_le_bytes());
        }
    }
}

impl CelestialHooks for Hooks {
    fn moon_phase(&mut self, m: &mut Machine, name: u16, frame: u16, dest: u16) {
        self.log(&[1]);
        for i in 0..16 {
            let c = m.mem.r8(DS, name + i);
            if c == 0 {
                break;
            }
            self.log.push(c);
        }
        self.log(&[2, frame, m.mem.r16(DS, 0x9030), dest]);
    }
    fn draw(&mut self, m: &mut Machine, si: u16, x: u16, y: u16, w: u16, h: u16) {
        let (off, seg) = (m.mem.r16(DS, si), m.mem.r16(DS, si + 2));
        self.log(&[3, seg, off, x, y, w, h, m.mem.r16(DS, 0xA91A), m.mem.r16(DS, 0xA91C)]);
    }
    fn star_pixel(&mut self, m: &mut Machine, x: u16, y: u16, colour: u16) {
        self.log(&[4, x, y, colour, m.mem.r16(DS, 0xA91A)]);
    }
    fn star_image(&mut self, m: &mut Machine, x: u16, y: u16, image: u16) {
        self.log(&[5, x, y, image, m.mem.r16(DS, 0xA91A)]);
    }
}

/// Python's `//` (floor) by 4.
fn floor4(v: i32) -> u16 {
    v.div_euclid(4) as u16
}

/// Mirrors harness/gen_celestial.py `setup` (a conditional's test byte
/// first, `and` short-circuiting); returns its `edge`.
fn setup(m: &mut Machine, seed: u32) -> Option<(u8, u16)> {
    let mut st = Stream(seed);
    let s = &mut st;
    let w16 = |m: &mut Machine, at: u16, v: u16| m.mem.w16(DS, at, v);
    let aimed = s.byte() % 3 == 0;
    let v = if s.byte() < 0xD0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0xBB09, v);
    let v = if aimed { 0 } else { s.word() & 0x3FF };
    w16(m, 0xCCFE, v);
    if aimed && s.byte() & 1 != 0 {
        let v = 0x100 + s.byte() as u16;
        w16(m, 0xCCFE, v);
    }
    let v = if aimed { 0 } else { s.word() };
    w16(m, 0xCB66, v);
    let v = if aimed { 0 } else { s.word() & 0x1FF };
    w16(m, 0xE052, v);
    let v = if aimed { 0 } else { s.word() & 0x1FF };
    w16(m, 0xC3DA, v);
    for (at, d) in [(0xCCF4, 0xA0), (0xCCF6, 0xA0), (0xC3D6, 0xA0), (0xC3D8, 0x49)] {
        let v = if s.byte() < 0xC0 { d } else { s.word() };
        w16(m, at, v);
    }
    let clip = if s.byte() < 0xC0 { [0, 0, 0x13F, 0x92] } else { [s.word(), s.word(), s.word(), s.word()] };
    for (at, v) in [0xA914, 0xA916, 0xA918, 0xA91A].into_iter().zip(clip) {
        w16(m, at, v);
    }
    let v = s.word();
    w16(m, 0xA91C, v);
    let day = (s.byte() % 0x20) as u16;
    let hi = if s.byte() < 0x40 { (s.byte() as u16) << 8 } else { 0 };
    w16(m, 0xCCA8, day | hi);
    let v = if s.byte() < 0x80 { day as u8 } else { s.byte() };
    m.mem.w8(DS, 0xCB36, v);
    for at in [0xE042, 0xE044, 0xE046, 0xE048] {
        let v = s.word();
        w16(m, at, v);
    }
    for at in [0xE04A, 0xE04C, 0xE04E, 0xE050] {
        let v = 1 + (s.byte() % 0x40) as u16;
        w16(m, at, v);
    }
    let v = 0x100 * (1 + (s.byte() % 16) as u16);
    w16(m, 0xCCAA, v);
    w16(m, 0xCCAC, FRAME);
    let v = 0x100 * (1 + (s.byte() % 16) as u16);
    w16(m, 0xCCAE, v);
    w16(m, 0xCCB0, FRAME);
    let sun = 0x100 * (1 + (s.byte() % 16) as u16);
    w16(m, 0xCD92, sun);
    w16(m, 0xCD94, FRAME);
    let v = 1 + (s.byte() % 0x40) as u16;
    m.mem.w16(FRAME, sun + 4, v);
    let v = 1 + (s.byte() % 0x40) as u16;
    m.mem.w16(FRAME, sun + 6, v);
    let mut p = 0xDA56u16;
    for i in 0..0x28u16 {
        let si = 0xD8C6 + 10 * i;
        let xyz = if aimed {
            let x = ((s.byte() as i32 - 0x80) * 8) as u16;
            let y = ((s.byte() as i32 - 0x80) * 8) as u16;
            let z = 0x300 + s.byte() as u16 * 0x20;
            [x, y, z]
        } else {
            [s.word(), s.word(), s.word()]
        };
        for (k, v) in xyz.into_iter().enumerate() {
            w16(m, si + 2 * k as u16, v);
        }
        if s.byte() & 3 == 0 {
            w16(m, si + 6, 0);
            let v = if s.byte() < 0xE0 { (s.byte() % 8) as u16 } else { s.word() };
            w16(m, si + 8, v);
        } else {
            let n = 1 + (s.byte() % 5) as u16;
            w16(m, si + 6, n);
            w16(m, si + 8, p);
            for _ in 0..n {
                let x = floor4(s.byte() as i32 - 0x80);
                w16(m, p, x);
                let y = floor4(s.byte() as i32 - 0x80);
                w16(m, p + 2, y);
                let c = 0x40 + (s.byte() % 10) as u16;
                w16(m, p + 4, c);
                p += 6;
            }
        }
    }
    for i in 0..8 {
        let v = s.word();
        w16(m, 0xE032 + 2 * i, v);
    }
    for (at, off) in [(0xCD22u16, 0x03A1u16), (0xCD26, 0x03D7)] {
        let v = if s.byte() < 0xC0 { 0x2294_0000 | off as u32 } else { 0 };
        m.mem.w32(DS, at, v);
    }
    let mut edge = None;
    match s.byte() % 4 {
        1 => {
            m.mem.w8(DS, 0xBB09, 0);
            let v = 0x2FF + (s.byte() % 3) as u16;
            w16(m, 0xCCFE, v);
        }
        2 => {
            m.mem.w8(DS, 0xBB09, 0);
            for at in [0xCCFE, 0xCB66, 0xE052, 0xC3DA] {
                w16(m, at, 0);
            }
            for (at, v) in [(0xCCF4, 0xA0), (0xCCF6, 0xA0), (0xC3D6, 0xA0), (0xC3D8, 0x49)] {
                w16(m, at, v);
            }
            for (at, v) in [(0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0x92)] {
                w16(m, at, v);
            }
            w16(m, 0xD8C6, 0);
            w16(m, 0xD8C8, 0xFFE0);
            let z = 0x3E9 + (s.byte() % 3) as u16;
            w16(m, 0xD8CA, z);
            w16(m, 0xD8CC, 0);
            w16(m, 0xD8CE, 0);
        }
        3 => {
            m.mem.w8(DS, 0xBB09, 0);
            for at in [0xCCFE, 0xCB66, 0xE052, 0xC3DA] {
                w16(m, at, 0);
            }
            let day = 4 + (s.byte() % 4) as u16;
            w16(m, 0xCCA8, day);
            w16(m, 0xA914, 0);
            w16(m, 0xA916, 0);
            let kind = s.byte() & 1;
            let d = (s.byte() % 3) as u16;
            edge = Some((kind, d));
        }
        _ => {}
    }
    edge
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x9030, FRAME);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/celestial.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "celestial", "{line}");
        let mut m = base.clone();
        for o in 0..=0xFFFFu16 {
            m.mem.w8(FRAME, o, 0);
        }
        if let Some((kind, d)) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap()) {
            // The first moon's place as 0000:E557 makes it, the clip's right
            // edge or the centre then put about it.
            let mut probe = m.clone();
            let x = probe.moon_places().map_or(0xFFFF, |_| probe.mem.r16(DS, 0xE042));
            if x != 0xFFFF {
                if kind == 0 {
                    m.mem.w16(DS, 0xA918, x.wrapping_add(d).wrapping_sub(1));
                } else {
                    let c = m.mem.r16(DS, 0xC3D6).wrapping_sub(x).wrapping_add(d).wrapping_sub(1);
                    m.mem.w16(DS, 0xC3D6, c);
                }
            }
        }
        let before = m.clone();
        let mut h = Hooks::default();
        let got = m.celestial(&mut h).map(|_| {
            let changed: Vec<u8> = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)])
                .collect();
            format!("{:x}", fnv(changed.into_iter().chain(h.log.iter().copied())))
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => {
                assert!(f[2] == "fault" || f[2] == "hang", "{line}");
                stuck += 1;
            }
        }
    }
    eprintln!("celestial: {n} compared, {stuck} fault in both");
    assert!(n > 2000, "{n} compared");
}

/// The destinations the moons' phases are decoded to.
#[derive(Default)]
struct Phases(Vec<(u16, u16)>);

impl CelestialHooks for Phases {
    fn moon_phase(&mut self, _m: &mut Machine, name: u16, _frame: u16, dest: u16) {
        self.0.push((name, dest));
    }
}

/// Fixed (kept bug #47): the second moon's phase is decoded through its
/// far pointer (CCAE), where it is drawn from; the original decodes it
/// at the frame's offset CCAEh itself.
#[test]
fn fixed_second_moon_decoded_through_its_pointer() {
    use arena::fixes::{Fixes, MOON2_PHASE};
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x9030, FRAME);
    let mut seen = 0;
    for seed in 1..400u32 {
        for fixes in [Fixes::NONE, Fixes::only(MOON2_PHASE)] {
            let mut m = base.clone();
            m.fixes = fixes;
            if setup(&mut m, seed).is_none() {
                break;
            }
            let pointer = m.mem.r16(DS, 0xCCAE);
            let mut h = Phases::default();
            if m.celestial(&mut h).is_none() || h.0.len() != 2 {
                break;
            }
            let want = if fixes == Fixes::NONE { 0xCCAE } else { pointer };
            assert_eq!(h.0[1], (0xD87C, want), "seed {seed} {fixes:?}");
            if pointer != 0xCCAE {
                seen += 1;
            }
        }
    }
    assert!(seen > 10, "{seen} phases decoded");
}
