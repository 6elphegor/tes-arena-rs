//! Differential tests of the CITYDATA check (src/city.rs
//! `citydata_check`, 242A:000E) against vectors from the original code
//! (harness/gen_citycheck.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const PROVINCE: usize = 0x4CC;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn name(s: &mut Stream, n: u8) -> Vec<u8> {
    (0..n).map(|_| {
        let c = s.byte();
        if c < 0x20 { 0x20 } else { 0x41 + c % 26 }
    }).collect()
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

/// Mirrors harness/gen_citycheck.py `setup` (a conditional's test byte
/// comes first).
fn setup(m: &mut Machine, files: &mut Files, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let slot = s.byte() % 10;
    let here = s.byte() % 9;
    let mut blocks = vec![vec![0u8; PROVINCE]; 9];
    for b in blocks.iter_mut() {
        let k = s.byte() % 16;
        let n = name(s, k);
        b[..n.len()].copy_from_slice(&n);
        for loc in 0..0x20 {
            let at = 0x1C + loc * 0x19;
            let k = s.byte() % 16;
            let n = name(s, k);
            b[at..at + n.len()].copy_from_slice(&n);
            let x = s.word() % 0x141;
            let y = s.word() % 0xC9;
            put16(b, at + 0x14, x);
            put16(b, at + 0x16, y);
        }
    }
    if s.byte() >= 0x80 {
        let (p, kind, loc) = (s.byte() as usize % 8, s.byte() % 5, s.byte() as usize % 0x20);
        let b = &mut blocks[p];
        let at = 0x1C + loc * 0x19;
        match kind {
            0 => {
                let v = 0x140 + (s.byte() % 2) as u16;
                put16(b, at + 0x14, v);
            }
            1 => {
                let v = 0xC8 + (s.byte() % 2) as u16;
                put16(b, at + 0x16, v);
            }
            2 => b[at] = [0x40, 0x7B, 0x1F, 0x5B, 0x7A, 0x41][s.byte() as usize % 6],
            3 => {
                let n = 0x13 + s.byte() % 2;
                for i in 0..0x14u8 {
                    b[at + i as usize] = if i < n { 0x41 + i } else { 0 };
                }
                put16(b, at + 0x14, 0);
            }
            _ => b[0] = 0x40,
        }
    }
    m.mem.w8(DS, 0x1A6C, slot);
    m.mem.w8(DS, 0x0A46, here);
    let mut n = b"CITYDATA.".to_vec();
    n.extend([0x30 + slot / 10, 0x30 + slot % 10]);
    files.put(&n, &blocks.concat());
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/citycheck.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "citycheck", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let mut fs = Files::default();
        setup(&mut m, &mut fs, seed);
        let before = m.clone();
        let mut message = |m: &mut Machine, si: u16| {
            let c = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
            m.mem.w16(DS, 0xFFE0, c);
            m.mem.w16(DS, 0xFFE2, si);
        };
        let Some(_) = m.citydata_check(&mut fs, &mut message) else {
            assert!(f[2] == "fault" || f[2] == "hang", "{line}: the port gave None");
            n += 1;
            continue;
        };
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} cases");
}
