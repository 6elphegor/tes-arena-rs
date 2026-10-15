//! Differential tests of the automap screen's view (src/automap.rs:
//! 0000:37ED..3815, 3796, 377D, 2D1F..2D2E) against vectors from the
//! original code (harness/gen_mapview.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_mapview.py `setup` (a conditional's test byte
/// comes first, a tuple's items before its index); returns the op.
fn setup(m: &mut Machine, seed: u32) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 8;
    let w = if s.byte() < 0xE0 { 0x3C + (s.byte() % 0x45) as u16 } else { (s.byte() % 0x3C) as u16 };
    let d = if s.byte() < 0xE0 { 0x2A + (s.byte() % 0x57) as u16 } else { (s.byte() % 0x2A) as u16 };
    let xs = [
        s.word(),
        (0xF00 + (s.byte() % 3) as i32 - 1) as u16,
        (((w as i32) << 7) - 0xE80 + (s.byte() % 3) as i32 - 1) as u16,
        (s.word() as u32 % ((w as u32) << 7 | 1)) as u16,
    ];
    let ys = [
        s.word(),
        (0xA7F + (s.byte() % 3) as i32 - 1) as u16,
        (((d as i32) << 7) - 0xA00 + (s.byte() % 3) as i32 - 1) as u16,
        (s.word() as u32 % ((d as u32) << 7 | 1)) as u16,
    ];
    let x = xs[s.byte() as usize % 4];
    let y = ys[s.byte() as usize % 4];
    let (px, py) = (s.word(), s.word());
    for (at, v) in [(0xC357, w), (0xC359, d), (0x126C, x), (0x126E, y), (0xC3D0, px), (0xC3D4, py)] {
        m.mem.w16(DS, at, v);
    }
    op
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/mapview.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "mapview", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        match op {
            0..=4 => m.automap_view_button(op as u16),
            5 => m.automap_centre(),
            6 => m.automap_origin(),
            _ => m.automap_view_open(),
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[3], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n}");
}
