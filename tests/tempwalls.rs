//! Differential tests of the changed squares of the wall spells
//! (src/click.rs `temp_wall_drawn`, 0000:D3F9, and `temp_walls_frame`,
//! 0000:D476) against vectors from the original code
//! (harness/gen_tempwalls.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x6800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_tempwalls.py `setup`; returns the squares drawn
/// each frame.
fn setup(m: &mut Machine, seed: u32) -> Vec<Vec<u16>> {
    let mut st = Stream(seed);
    let s = &mut st;
    let mut squares = vec![];
    for i in 0..0x20u16 {
        if s.byte() < 0x60 {
            m.mem.w16(DS, 0xC2C2 + 2 * i, 0xFFFF);
            continue;
        }
        let sq = s.word() & 0x7FFE;
        squares.push(sq);
        let k = s.byte();
        let hi = (s.byte() % 0x0F) as u16;
        let lo = if k < 0x70 {
            0
        } else if k < 0xE0 {
            1
        } else {
            s.byte() as u16
        };
        m.mem.w16(DS, 0xC2C2 + 2 * i, sq);
        m.mem.w16(DS, 0xC302 + 2 * i, hi << 8 | lo);
        let v = s.word();
        m.mem.w16(WSEG, sq, v);
    }
    let v = if s.byte() < 0x20 { 1 } else { 0 };
    m.mem.w8(DS, 0xD73C, v);
    let n = 1 + s.byte() % 20;
    (0..n)
        .map(|_| {
            let k = s.byte() % 6;
            (0..k)
                .map(|_| {
                    if !squares.is_empty() && s.byte() < 0xE0 {
                        squares[s.byte() as usize % squares.len()]
                    } else {
                        s.word() & 0x7FFE
                    }
                })
                .collect()
        })
        .collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    let mut n = 0;
    for line in include_str!("vectors/tempwalls.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "temp", "{line}");
        let mut m = base.clone();
        let frames = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        for drawn in frames {
            for sq in drawn {
                m.temp_wall_drawn(sq);
            }
            m.temp_walls_frame();
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let got = format!("{:x} {:x}", fnv(changed), fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))));
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 1500);
}
