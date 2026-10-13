//! Differential tests of a building's hours (src/weapon.rs
//! `building_closed`, 1194:4FB8) against vectors from the original code
//! (harness/gen_hours.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/hours.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<u32> = f[1].split(' ').map(|v| u32::from_str_radix(v, 16).unwrap()).collect();
        m.mem.w32(DS, 0x05CC, p[1]);
        m.mem.w16(DS, 0xCCFE, p[2] as u16);
        let closed = m.building_closed(p[0] as u8);
        let got = format!("{:x} {:x}", closed as u8, m.mem.r8(DS, 0x137F));
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert_eq!(n, 2000);
}
