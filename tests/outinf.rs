//! Differential tests of the outdoor INF choice (src/engine.rs
//! `outdoor_inf`, 0000:0896) against vectors from the original code
//! (harness/gen_outinf.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_outinf.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = s.word() & !4;
    let v = v | if s.byte() & 1 != 0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & !0x200;
    let v = v | if s.byte() & 1 != 0 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0xE0 { [0, 4, 8][s.byte() as usize % 3] } else { s.word() };
    m.mem.w16(DS, 0x0DA8, v);
    let v = if s.byte() < 0xC0 { [0, 1, 2, 0x81, 0x82][s.byte() as usize % 5] } else { s.byte() };
    m.mem.w8(DS, 0xBB09, v);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/outinf.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "outinf", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        if m.outdoor_inf() {
            let c = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
            m.mem.w16(DS, 0xFFE0, c);
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 1500);
}
