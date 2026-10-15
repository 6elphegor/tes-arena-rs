//! Differential tests of the level-up screen (src/levelup.rs
//! `level_up_screen`, 1194:1034) and exhaustion (`exhaustion`, through
//! 2901:0002) against vectors from the original code
//! (harness/gen_levelup.py).
mod common;

use arena::bsa::Files;
use arena::clock::ClockHooks;
use arena::levelup::LevelUpHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;
use common::clock::{CSEG, TSEG};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn inc(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's stubs, as hooks.
struct Stubs;

impl LevelUpHooks for Stubs {
    fn sound(&mut self, m: &mut Machine, _si: u16) {
        inc(m, 0xFFE2);
    }
    fn keep_screen(&mut self, m: &mut Machine) {
        inc(m, 0xFFE4);
    }
    /// SPEND: nothing the first time, then all the points on the
    /// attribute FFDC names.
    fn spend(&mut self, m: &mut Machine) {
        inc(m, 0xFFE0);
        if m.mem.r8(DS, 0xFFE0) & 1 != 0 {
            return;
        }
        let bx = m.mem.r16(DS, 0xFFDC);
        let at = 0x01CDu16.wrapping_add(bx);
        let v = m.mem.r8(DS, at).wrapping_add(m.mem.r8(DS, 0x129C));
        m.mem.w8(DS, at, v);
        m.mem.w8(DS, 0x129C, 0);
    }
    fn message_box(&mut self, m: &mut Machine, _si: u16) {
        inc(m, 0xFFE8);
    }
}

impl ClockHooks for Stubs {
    fn encounter(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_daily(&mut self, _m: &mut Machine) -> Option<()> {
        Some(())
    }
    fn quests_hourly(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        Some(())
    }
    fn exhaustion_box(&mut self, m: &mut Machine, _si: u16) {
        inc(m, 0xFFE8);
    }
    /// A level gained in the hour: the screen with the same stubs.
    fn level_up_screen(&mut self, m: &mut Machine, levels: u16) -> Option<()> {
        arena::levelup::level_up_around(m, levels, self)
    }
}

/// Mirrors harness/gen_levelup.py `setup_screen` (a conditional's test
/// byte comes first; `a | b` draws a first); returns CX.
fn setup_screen(m: &mut Machine, s: &mut Stream) -> u16 {
    for i in 0..8 {
        let b = s.byte();
        m.mem.w8(DS, 0x01CD + i, b);
    }
    let k = s.byte();
    let base: Vec<u8> = if k < 0x10 {
        vec![0xFF; 8]
    } else if k < 0x20 {
        // One point of room: seven at 100%, one at 99%.
        let i = s.byte() % 8;
        (0..8).map(|j| if j == i { 0xFD } else { 0xFF }).collect()
    } else if k < 0x28 {
        // Half of them FFh (1194:11C6 tests both dwords).
        if s.byte() & 1 != 0 {
            let mut v = vec![0xFF; 4];
            v.extend((0..4).map(|_| s.byte()));
            v
        } else {
            let mut v: Vec<u8> = (0..4).map(|_| s.byte()).collect();
            v.extend([0xFF; 4]);
            v
        }
    } else if k < 0x60 {
        (0..8).map(|_| 0xE0 + s.byte() % 0x20).collect()
    } else {
        (0..8).map(|_| s.byte()).collect()
    };
    for (i, &b) in base.iter().enumerate() {
        m.mem.w8(DS, 0x01D5 + i as u16, b);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x01A9, v);
    let v = s.byte() % 30;
    m.mem.w8(DS, 0x01AA, v);
    for at in [0x01FD, 0x01FF, 0x0201, 0x020A, 0x020C] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = if s.byte() < 0x40 { 0x8E } else { s.byte() };
    m.mem.w8(DS, 0x0FDD, v);
    for at in [0x05E0, 0x05C8, 0x129C, 0x9686, 0x102C, 0x1036, 0x103A] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    for i in 0..40u16 {
        let mut b = [0u8; 19];
        b[0] = if s.byte() < 0x50 { 0x80 + s.byte() % 0x12 } else { s.byte() % 0x12 };
        let lo = s.byte() & 0x33;
        let mut f = lo | (s.byte() % 4) << 2;
        if s.byte() < 0xA0 {
            f |= 0x40;
        }
        if s.byte() < 0x80 {
            f |= 0x80;
        }
        b[0xE] = f;
        for j in [1, 2, 3, 4, 5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xD, 0xF, 0x10, 0x11] {
            b[j] = s.byte();
        }
        b[0x12] = if s.byte() < 0x40 { 0xFF } else { s.byte() };
        for (j, &v) in b.iter().enumerate() {
            m.mem.w8(DS, 0x0212 + i * 19 + j as u16, v);
        }
    }
    for i in 0..4 {
        let b = s.byte();
        m.mem.w8(DS, 0x91F6 + i, b);
    }
    let v = (s.byte() % 8) as u16;
    m.mem.w16(DS, 0xFFDC, v);
    let k = s.byte();
    if k < 4 {
        0
    } else {
        let lo = 1 + (s.byte() % 4) as u16;
        lo | if k < 0x10 { (s.byte() as u16) << 8 } else { 0 }
    }
}

/// Mirrors harness/gen_levelup.py `setup_tired`.
fn setup_tired(m: &mut Machine, s: &mut Stream) {
    let v = if s.byte() < 0xE0 { (s.byte() % 11) as u16 } else { s.word() };
    m.mem.w16(DS, 0x0201, v);
    m.mem.w16(DS, 0x05CA, 0);
    let v = if s.byte() < 0xE0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x1013, v);
    let k = s.byte();
    let v = if k < 0x50 { (s.byte() | 1) as u16 } else { 0 };
    m.mem.w16(DS, 0x0FFB, v);
    let v = if s.byte() < 0x80 { s.word() & 0x4210 } else { s.word() & 0x0200 };
    m.mem.w16(DS, 0xC39A, v);
    for at in [0x102C, 0x1036, 0x103A] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    m.mem.w16(DS, 0xD806, WSEG);
}

/// The walls plane's scratch segment (zero in the base machine).
const WSEG: u16 = 0x6800;

/// The exhaustion nests up to `EXHAUSTION_DEPTH` hours of the clock, each
/// in the one before: the cases run on a thread with room for that.
#[test]
fn matches_original() {
    std::thread::Builder::new().stack_size(1 << 29).spawn(run).unwrap().join().unwrap();
}

fn run() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0x91F0, 0);
    let auras: Vec<u8> = (0..0x30u16).filter(|&t| base.mem.r8(DS, 0x2DE6 + t) == 0xFE).map(|t| t as u8 + 1).collect();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/levelup.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "levelup", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let op: u8 = f[2].parse().unwrap();
        let mut m = base.clone();
        let mut s = Stream(seed);
        let cx = if op == 0 {
            setup_screen(&mut m, &mut s)
        } else {
            common::clock::setup(&mut m, seed ^ 0x5A5A_5A5A, &auras);
            setup_tired(&mut m, &mut s);
            0
        };
        m.load_rng();
        let before = m.clone();
        let got = if op == 0 {
            m.level_up_screen(cx, &mut Stubs)
        } else {
            m.fatigue_minute(&mut Files::default(), &mut Stubs)
        };
        let got = got.map(|()| {
            m.store_rng();
            if std::env::var("DUMP").is_ok_and(|d| d == f[1]) {
                for o in 0..=0xFFFFu16 {
                    if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                        eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                    }
                }
            }
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let mut r = format!("{:x}", fnv(changed));
            if op == 1 {
                r += &format!(
                    " {:x} {:x}",
                    fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                    fnv((0..0x400u16).map(|o| m.mem.r8(TSEG, o)))
                );
            }
            r
        });
        match got {
            Some(g) => {
                assert_eq!(f[3], g, "{line}");
                n += 1;
            }
            None => {
                assert_eq!(f[3], "stuck", "{line}");
                stuck += 1;
            }
        }
    }
    eprintln!("levelup: {n} compared, {stuck} stuck in both");
    assert!(n > 2000, "{n} compared");
}
