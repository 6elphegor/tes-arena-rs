//! Differential tests of the store's list lines (src/store.rs
//! `store_lines`, 1194:F142 weapons and F204 armour) against vectors from
//! the original code (harness/gen_storelines.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FSEG: u16 = 0x7000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_storelines.py `setup`, draw for draw (a tuple's
/// index is one byte); returns the op.
fn setup(m: &mut Machine, seed: u32) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() & 1;
    let count = if s.byte() < 0x08 { 0 } else { 1 + s.byte() % 10 };
    let base: u16 = if op == 0 { 0 } else { 0x7D0 };
    for i in 0..count as u16 {
        let mut r = [0u8; 13];
        r[0] = s.byte();
        r[1] = if op == 0 { [0, 0, 1, 1, 2][(s.byte() % 5) as usize] } else { [0, 0, 1, 3][(s.byte() % 4) as usize] };
        r[2] = [0, 1, 1, 2][(s.byte() % 4) as usize];
        r[3] = s.byte() % 4;
        r[4] = if r[2] == 2 {
            s.byte() % 3
        } else if s.byte() < 0x40 {
            0xFF
        } else {
            s.byte() % 8
        };
        r[5] = if s.byte() < 0xF0 { s.byte() % if op == 0 { 0x14 } else { 0x0C } } else { s.byte() };
        r[6] = if s.byte() < 0x40 { 0xFF } else { s.byte() % 0x20 };
        let price = if s.byte() < 0xE0 {
            s.word() as u32
        } else {
            let w = s.word() as u32;
            w | (s.byte() as u32) << 16
        };
        r[7..11].copy_from_slice(&price.to_le_bytes());
        r[11] = s.byte();
        r[12] = s.byte();
        for (k, &b) in r.iter().enumerate() {
            m.mem.w8(FSEG, base + 13 * i + k as u16, b);
        }
    }
    m.mem.w8(DS, if op == 0 { 0x12C6 } else { 0x12CF }, count);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x13C9, v);
    for i in 0..4u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x05CC + i, b);
    }
    for (at, n) in [(0x13F3u16, 13u16), (0x1395, 16), (0x10A8, 8)] {
        for i in 0..n {
            let b = s.byte();
            m.mem.w8(DS, at + i, b);
        }
    }
    let v = s.word();
    m.mem.w16(DS, 0xA874, v);
    op
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x9030, FSEG);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/storelines.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "storelines", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let got = m.store_lines(op == 0).map(|_| {
            if std::env::var("DUMP").is_ok_and(|d| d == f[1]) {
                for (name, seg) in [("ds", DS), ("fr", FSEG)] {
                    for o in 0..=0xFFFFu16 {
                        if m.mem.r8(seg, o) != before.mem.r8(seg, o) {
                            eprintln!("{name} {o:04x} {:02x} {:02x}", before.mem.r8(seg, o), m.mem.r8(seg, o));
                        }
                    }
                }
            }
            let ds = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let ds = fnv(ds);
            let fr = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(FSEG, o) != before.mem.r8(FSEG, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(FSEG, o)]);
            format!("{:x} {:x}", ds, fnv(fr))
        });
        match got {
            Some(g) => {
                assert_eq!(f[3], g, "{line}");
                n += 1;
            }
            None => {
                assert_eq!(f[3], "stuck", "{line}");
                stuck += 1;
            }
        }
    }
    eprintln!("storelines: {n} compared, {stuck} stuck in both");
    assert!(n > 2000, "{n}");
}
