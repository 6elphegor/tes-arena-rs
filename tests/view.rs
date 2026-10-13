//! Differential tests of the state the original sets as it starts
//! drawing the view (src/engine.rs `view_begin`, 0000:C923 with its walk
//! over the squares stubbed) against vectors from the original code
//! (harness/gen_view.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x6800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_view.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let (x, y) = (s.word(), s.word());
    m.mem.w16(DS, 0xC3D0, x);
    m.mem.w16(DS, 0xC3D4, y);
    let v = s.word();
    m.mem.w16(DS, 0xC398, v);
    for at in [0xCB92, 0xCCCA, 0xCD14, 0xCCB2, 0xCCB4, 0xA854, 0xA856, 0xCCA4, 0xCCA6, 0xCB90, 0xCB6C] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let sq = ((y >> 7) << 8).wrapping_add((x >> 7) << 1);
    for d in [0u16, 2, 0x100, 0xFFFE] {
        let v = s.word();
        m.mem.w16(WSEG, sq.wrapping_add(d), v);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    let mut n = 0;
    for line in include_str!("vectors/view.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "view", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        m.view_begin();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 1500);
}
