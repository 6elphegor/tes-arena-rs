//! Differential tests of the outdoor light level (src/sky.rs
//! `sky_light`, 0000:0390 with its callees stubbed) against vectors from
//! the original code (harness/gen_light.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

/// Mirrors harness/gen_light.py `setup` (a tuple's items are all drawn).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = s.word() & !4;
    let v = v | if s.byte() < 0xE0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0F81, v);
    let k = (s.byte() % 4) as usize;
    let t = [0xF0 + (s.byte() % 0x30) as u16, 0x2E0 + (s.byte() % 0x30) as u16, s.word() & 0x3FF, s.word()][k];
    m.mem.w16(DS, 0xCCFE, t);
    let v = s.word();
    m.mem.w16(DS, 0x3C87, v);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/light.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "light", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], format!("{:x}", m.sky_light()), "{line}");
        n += 1;
    }
    assert!(n > 2500);
}
