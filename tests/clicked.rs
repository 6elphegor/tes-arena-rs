//! Differential tests of the hit test's result made ready (src/talk.rs
//! `clicked_flat`, 2294:047B) against vectors from the original code
//! (harness/gen_clicked.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_clicked.py `setup` (a tuple's items before its
/// index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let k = s.byte() % 6;
    let slot = 0x0AB3 + 0x1C * (s.byte() as u16 % 23);
    let edge = [0x0AB2, 0x0AB3, 0x0D36, 0x0D37][s.byte() as usize % 4];
    let any = s.word();
    let v = [0xD7DE, slot, edge, 0x3D96, 0, any][k as usize];
    m.mem.w16(DS, 0xCB5E, v);
    for i in 0..0x1Cu16 {
        let b = s.byte();
        m.mem.w8(DS, 0xD7DE + i, b);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/clicked.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "clicked", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        m.clicked_flat();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 1500);
}
