//! Differential tests of the sky's objects (src/skyobj.rs: `sky_objects`
//! 2294:08B9, `sky_draw` 2294:0A5D, `lightning` 2294:05A6) against
//! vectors from the original code (harness/gen_skyobj.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::skyobj::SkyHooks;
use common::blow::Stream;

const FRAME: u16 = 0x6000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The generator's stubs, logging as its code hooks do.
struct Hooks {
    log: Vec<u8>,
    loads: u16,
    mask: u8,
    fog: bool,
}

impl Hooks {
    fn log(&mut self, ws: &[u16]) {
        for w in ws {
            self.log.extend(w.to_le_bytes());
        }
    }
}

impl SkyHooks for Hooks {
    fn load(&mut self, m: &mut Machine, name: u16) -> Option<(u16, u16)> {
        self.log(&[1, name]);
        for i in 0..16 {
            let c = m.mem.r8(DS, name + i);
            if c == 0 {
                break;
            }
            self.log.push(c);
        }
        let k = self.loads;
        self.loads += 1;
        if k < 8 && self.mask >> k & 1 != 0 {
            return None;
        }
        Some((FRAME, 0x100 * (k + 1)))
    }
    fn fog(&mut self, _m: &mut Machine) -> bool {
        self.fog
    }
    fn draw_image(&mut self, m: &mut Machine, seg: u16, off: u16) {
        let h = |f: u16| m.mem.r16(seg, off.wrapping_add(f));
        let e = [2, seg, off, h(0), h(2), h(4), h(6), h(8), m.mem.r16(DS, 0xA91A)];
        self.log(&e);
    }
    fn draw_dfa(&mut self, _m: &mut Machine, seg: u16, off: u16, frame: u16, x: u16, y: u16) {
        self.log(&[3, seg, off, frame, x, y]);
    }
    fn fill(&mut self, m: &mut Machine, x0: u16, y0: u16, x1: u16, y1: u16) {
        let c = m.mem.r8(DS, 0x9288) as u16;
        self.log(&[4, x0, y0, x1, y1, c]);
    }
    fn bolt(&mut self, m: &mut Machine, seg: u16, off: u16, frame: u16) {
        let e = [5, seg, off, frame, m.mem.r16(seg, off + 6), m.mem.r16(seg, off + 8)];
        self.log(&e);
    }
    fn sound(&mut self, _m: &mut Machine, sound: u16, at: u16) {
        self.log(&[6, sound, at]);
    }
}

/// Mirrors harness/gen_skyobj.py `setup` (a conditional's test byte
/// first); returns (op, mask).
fn setup(m: &mut Machine, seed: u32) -> (u8, u8) {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 3;
    let mask = if s.byte() < 0x60 { s.byte() } else { 0 };
    let v = s.word() & !4;
    let v = if s.byte() < 0xE0 { v | 4 } else { v };
    m.mem.w16(DS, 0xC398, v);
    for at in [0xC39A, 0x05C8] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = s.byte() % 7;
    m.mem.w8(DS, 0x0A50, v);
    for at in [0x0A56, 0x0A58] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = (s.byte() % 0x20) as u16;
    m.mem.w16(DS, 0xCCA8, v);
    let k = s.byte() % 6;
    let v = if k == 5 { s.byte() } else { [0, 0, 1, 0x81, 2][k as usize] };
    m.mem.w8(DS, 0xBB09, v);
    let v = s.word() & 0x3FF;
    m.mem.w16(DS, 0xCCFE, v);
    let v = if s.byte() < 0xA0 { 3 } else { s.byte() % 10 };
    m.mem.w8(DS, 0x0A46, v);
    if s.byte() < 0xA0 {
        for (at, base) in [(0x0A48, 0x84u16), (0x0A4A, 0x34)] {
            let a = s.byte() as u16;
            let b = s.byte() as u16 / 2;
            m.mem.w16(DS, at, base.wrapping_sub(0xC0).wrapping_add(a).wrapping_add(b));
        }
    } else {
        for at in [0x0A48, 0x0A4A] {
            let v = s.word();
            m.mem.w16(DS, at, v);
        }
    }
    let v = if s.byte() < 0x80 { 0x14 } else { s.byte() % 0x24 };
    m.mem.w8(DS, 0xC8F8, v);
    for i in 0..0x20 {
        let v = s.word();
        m.mem.w16(DS, 0x8FAA + 2 * i, v);
    }
    for (count, list, max) in [(0x42B9u16, 0x42BAu16, 6u8), (0x438C, 0x4396, 8)] {
        let n = (s.byte() % max) as u16;
        m.mem.w8(DS, count, n as u8);
        for i in 0..n {
            let v = 0x100 * (1 + (s.byte() % 16) as u16);
            m.mem.w16(DS, list + 6 * i, v);
            for f in [2, 4] {
                let v = s.word();
                m.mem.w16(DS, list + 6 * i + f, v);
            }
        }
    }
    let k = s.byte() % 4;
    let v = if k < 3 { [0, 2, 3][k as usize] } else { s.byte() };
    m.mem.w8(DS, 0x7BD2, v);
    for at in [0x7BCA, 0x7BCC] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = 0x100 * (1 + (s.byte() % 16) as u16);
    m.mem.w16(DS, 0x7BCE, v);
    m.mem.w16(DS, 0x7BD0, FRAME);
    let k = (s.byte() % 5) as u16;
    m.mem.w16(DS, 0x43C6, k.wrapping_sub(1));
    let v = if s.byte() < 0x20 { 0 } else { 0x100 * (1 + (s.byte() % 16) as u16) };
    m.mem.w16(DS, 0x43C8, v);
    for at in [0x43C2, 0x43C4] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    for i in 0..6 {
        let v = 0x100 * (1 + (s.byte() % 16) as u16);
        m.mem.w16(DS, 0xD8AE + 4 * i, v);
        m.mem.w16(DS, 0xD8B0 + 4 * i, FRAME);
    }
    let v = s.byte() % 0x20;
    m.mem.w8(DS, 0xC8F3, v);
    let v = s.word();
    m.mem.w16(DS, 0xCB90, v);
    let v = s.word() & 0x1FF;
    m.mem.w16(DS, 0xC3DA, v);
    for (at, d) in [(0xCCF4, 0xA0), (0xC3D6, 0xA0), (0xC3D8, 0x64)] {
        let v = if s.byte() < 0xC0 { d } else { s.word() };
        m.mem.w16(DS, at, v);
    }
    for at in [0xA914, 0xA916, 0xA918, 0xA91A, 0xCCBC, 0x91F6, 0x91F8] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let b = s.byte();
    let v = b & if s.byte() < 0x40 { 1 } else { 0 };
    m.mem.w8(DS, 0xFFDE, v);
    for i in 0..16u16 {
        let h = 0x100 * (i + 1);
        let v = if s.byte() < 0x40 { s.word() } else { 1 + (s.byte() % 8) as u16 };
        m.mem.w16(FRAME, h, v);
        for f in [2, 8, 0xA] {
            let v = s.word();
            m.mem.w16(FRAME, h + f, v);
        }
        for f in [4, 6] {
            let v = 1 + (s.byte() % 0x80) as u16;
            m.mem.w16(FRAME, h + f, v);
        }
    }
    // The aimed cases: a depth on the edge, the near volcano on the
    // clip's edges, the roll for lightning at 7D0h.
    let mut op = op;
    match s.byte() % 4 {
        1 => {
            op = 1 + s.byte() % 2;
            m.mem.w16(DS, 0xC3DA, 0);
            let points = (0..6).map(|i| 0x42BC + 6 * i).chain((0..8).map(|i| 0x4398 + 6 * i)).chain([0x7BCA, 0x43C2]);
            for at in points {
                m.mem.w16(DS, at, 0);
                let y = 100 + (s.byte() % 3) as u16;
                m.mem.w16(DS, at + 2, y);
            }
        }
        2 => {
            op = 1;
            m.mem.w16(DS, 0xC398, 4);
            m.mem.w8(DS, 0xFFDE, 0);
            m.mem.w16(DS, 0xC3DA, 0);
            m.mem.w8(DS, 0x7BD2, 2);
            m.mem.w16(DS, 0x7BCA, 0);
            m.mem.w16(DS, 0x7BCC, 0x200);
            m.mem.w16(DS, 0x7BCE, 0x100);
            m.mem.w16(DS, 0xCCF4, 0xA0);
            m.mem.w16(DS, 0xC3D6, 0xA0);
            let v = 1 + (s.byte() % 8) as u16;
            m.mem.w16(FRAME, 0x100, v);
            m.mem.w16(FRAME, 0x106, 0x40);
            if s.byte() & 1 != 0 {
                let v = 0x7F + (s.byte() % 3) as u16;
                m.mem.w16(DS, 0xA918, v);
            } else {
                let v = 0xBF + (s.byte() % 3) as u16;
                m.mem.w16(DS, 0xA914, v);
            }
        }
        3 => {
            op = 2;
            m.mem.w16(DS, 0xC398, 4);
            m.mem.w8(DS, 0xBB09, 1);
            m.mem.w16(DS, 0xCCFE, 0x50);
            m.mem.w16(DS, 0x05C8, 1);
            m.mem.w16(DS, 0x43C6, 0xFFFF);
            let r = 0x7CF + (s.byte() % 3) as u32;
            let lo = s.byte() as u32;
            m.mem.w32(DS, 0x91F6, (r << 16 | lo).wrapping_mul(0x3F15_4FA5));
        }
        _ => {}
    }
    (op, mask)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x9030, FRAME);
    let mut n = 0;
    for line in include_str!("vectors/skyobj.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "skyobj", "{line}");
        let mut m = base.clone();
        for o in 0..=0xFFFFu16 {
            m.mem.w8(FRAME, o, 0);
        }
        let (op, mask) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        m.load_rng();
        let before = m.clone();
        let mut h = Hooks { log: Vec::new(), loads: 0, mask, fog: m.mem.r8(DS, 0xFFDE) & 1 != 0 };
        let ok = match op {
            0 => {
                m.sky_objects(&mut h);
                Some(())
            }
            1 => m.sky_draw(&mut h),
            _ => m.lightning(&mut h),
        };
        m.store_rng();
        if ok.is_none() {
            assert!(f[3] == "fault" || f[3] == "hang", "{line}");
            n += 1;
            continue;
        }
        let changed: Vec<u8> = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)])
            .collect();
        if std::env::var("DEBUG").ok().as_deref() == Some(f[1]) {
            let ds: Vec<_> = (0..=0xFFFFu16).filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o)).map(|o| (o, before.mem.r8(DS, o), m.mem.r8(DS, o))).collect();
            eprintln!("ds {ds:x?}");
            eprintln!("log {}", h.log.iter().map(|b| format!("{b:02x}")).collect::<String>());
            let fr: Vec<_> = (0..0x1100u16).filter(|&o| m.mem.r8(FRAME, o) != 0).map(|o| (o, m.mem.r8(FRAME, o))).take(400).collect();
            eprintln!("frame {fr:x?}");
        }
        let frame = (0..0x1100u16).map(|o| m.mem.r8(FRAME, o));
        assert_eq!(f[3], format!("{:x}", fnv(changed.into_iter().chain(frame).chain(h.log))), "{line}");
        n += 1;
    }
    assert!(n > 3500);
}
