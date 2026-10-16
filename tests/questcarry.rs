//! Differential tests of the frame's hold on a quest's message
//! (src/sidequest.rs `quest_carry`, 0000:4D00) and the main quest's check
//! on entering a temple or a guild (`main_quest_check`, 1194:D961)
//! against vectors from the original code (harness/gen_questcarry.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

/// harness/gen_questcarry.py's `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut s = Stream(seed);
    for k in 0..4u16 {
        let kind = [0u16, 1, 2, 3, 1][s.byte() as usize % 5];
        let stage = match s.byte() % 3 {
            0 => 4,
            1 => 6,
            _ => s.byte() as u16 % 8,
        };
        let hi = s.byte() as u16;
        let lo = s.byte() as u16;
        m.mem.w16(DS, 0x0E6E + 0x34 * k + 4, kind << 6 | stage << 9 | (hi << 8 | lo) & 0xF13F);
    }
    let v = s.byte() % 0x60;
    m.mem.w8(DS, 0x5E38, v);
    for i in 0..0x40 {
        let v = s.byte() % 0x60;
        m.mem.w8(DS, 0xC954 + i, v);
    }
}

/// harness/gen_questcarry.py's `check_setup`.
fn check_setup(m: &mut Machine, seed: u32) {
    let mut s = Stream(seed);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x0F76, v);
    let v = s.byte() % 10;
    m.mem.w8(DS, 0x0F77, v);
    let v = s.byte() % 9;
    m.mem.w8(DS, 0x0A46, v);
    for i in 0..10 {
        let v = [0u8, 5, 7, 1][s.byte() as usize % 4];
        m.mem.w8(DS, 0x8223 + i, v);
        let v = s.byte() % 9;
        m.mem.w8(DS, 0x80EC + i, v);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut held = 0;
    let mut ran = 0;
    for line in include_str!("vectors/questcarry.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f[0] == "questcheck" {
            let mut m = base.clone();
            check_setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
            // The talk (D593, a stub there) runs exactly when it carries.
            let due = m.main_quest_due() as u8;
            assert_eq!(f[2], format!("{due} {due}"), "{line}");
            ran += due as usize;
            continue;
        }
        assert_eq!(f[0], "questcarry", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let got = m.quest_carry();
        assert_eq!(f[2], if got { "1" } else { "0" }, "{line}");
        held += got as usize;
    }
    assert!(held > 100, "too few held: {held}");
    assert!(ran > 100, "too few checks met: {ran}");
}
