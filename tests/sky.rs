//! Differential tests of the sky's colours (src/sky.rs `sky_colours`,
//! 2294:0428, and `sky_step`, 2294:0341) against vectors from the
//! original code (harness/gen_sky.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// Mirrors harness/gen_sky.py `setup` (a conditional's test byte comes
/// first, a tuple's items before its index); returns the op.
fn setup(m: &mut Machine, seed: u32) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() & 1;
    let v = s.word() & !4;
    let v = v | if s.byte() & 1 != 0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = if s.byte() < 0x80 { 0 } else { s.byte() };
    m.mem.w8(DS, 0xBB09, v);
    let v = if s.byte() < 0xE0 { s.word() & 0x3FF } else { s.word() };
    m.mem.w16(DS, 0xCCFE, v);
    let v = if s.byte() < 0xC0 { [0x42A2, 0x42AD, 0xFFFF][s.byte() as usize % 3] } else { s.word() };
    m.mem.w16(DS, 0x3C85, v);
    let v = if s.byte() < 0xC0 { [0x428B, 0x4296, 0xFFFF][s.byte() as usize % 3] } else { s.word() };
    m.mem.w16(DS, 0x3C89, v);
    let v = if s.byte() < 0xC0 {
        let w = (s.word() & 0xFF) * 3;
        [w, 0, 0xFFFF][s.byte() as usize % 3]
    } else {
        s.word()
    };
    m.mem.w16(DS, 0x3C8B, v);
    op
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/sky.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "sky", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        if op == 0 {
            if m.sky_colours().is_some() {
                count(&mut m, 0xFFE0);
            }
        } else if m.sky_step().dither.is_some() {
            count(&mut m, 0xFFE4);
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[3], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 2500);
}
