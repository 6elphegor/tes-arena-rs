//! Differential tests of the floor under the cursor (src/click.rs,
//! `Machine::floor_pick`: 2826:0B52 through 0000:DB43) against vectors
//! from the original code (harness/gen_floorpick.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WORDS: [u16; 13] = [0x8F92, 0x8F94, 0xC3D8, 0xA91A, 0xC3D2, 0xCCF6, 0xC3D6, 0xCCF4, 0xC3DA, 0xC3D0, 0xC3D4, 0xC357, 0xC359];

/// Mirrors harness/gen_floorpick.py `setup`.
fn setup(seed: u32) -> Vec<u16> {
    let mut s = Stream(seed);
    if s.byte() % 8 == 0 {
        return WORDS.iter().map(|_| s.word()).collect();
    }
    let horizon = 0x40 + (s.byte() % 0x30) as u16;
    let cols = 0x20 + (s.byte() % 0x60) as u16;
    let rows = 0x20 + (s.byte() % 0x60) as u16;
    let mut v = vec![(s.byte() as u16) % 0x140];
    v.push(horizon - 8 + (s.byte() % 0x50) as u16);
    v.push(horizon);
    v.push(0x92 - horizon + (s.byte() % 0x20) as u16);
    v.push(if s.byte() & 1 != 0 { 0x32 } else { (s.byte() as u16) << 1 });
    v.push(0x80 + (s.byte() % 0x80) as u16);
    v.push(0xA0);
    v.push(0x80 + (s.byte() % 0x80) as u16);
    v.push(s.word() & 0x1FF);
    let mut x = ((s.byte() as u16) % cols) << 7;
    x |= (s.byte() & 0x7F) as u16;
    let mut y = ((s.byte() as u16) % rows) << 7;
    y |= (s.byte() & 0x7F) as u16;
    v.extend([x, y, cols, rows]);
    v
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let (mut hit, mut n) = (0, 0);
    for line in include_str!("vectors/floorpick.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        if f[2] == "hang" {
            continue;
        }
        let mut m = base.clone();
        for (&at, w) in WORDS.iter().zip(setup(seed)) {
            m.mem.w16(DS, at, w);
        }
        let got = match m.floor_pick() {
            Some(Some(si)) => format!("{si:x}"),
            Some(None) => "ffff".to_string(),
            None => "fault".to_string(),
        };
        assert_eq!(got, f[2], "{line}");
        if f[2] != "ffff" && f[2] != "fault" {
            hit += 1;
        }
        n += 1;
    }
    assert!(n == 3000 && hit > 1000, "{n} cases, {hit} squares");
}
