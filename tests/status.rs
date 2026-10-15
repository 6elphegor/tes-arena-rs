//! Differential tests of the interface bar's status (src/status.rs:
//! 0000:1486, 1194:4E17, 2542:04B1, 2542:0279) against vectors from the
//! original code (harness/gen_status.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_status.py `setup` (a tuple's items before its
/// index); returns the op.
fn setup(m: &mut Machine, seed: u32) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 4;
    let k = s.byte() % 4;
    let clock = if k == 0 {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    } else {
        let d = (s.word() % 2000) as u32;
        let h = (s.byte() % 24) as u32;
        let m = [0u32, 0, 1, 9, 10, 59][s.byte() as usize % 6];
        d * 1440 + h * 60 + m
    };
    m.mem.w32(DS, 0x05CC, clock);
    let full = s.byte() % 41;
    for i in 0..40u16 {
        let mut rec: Vec<u8> = (0..0x13).map(|_| s.byte()).collect();
        let mut f = rec[0xE] & !0x40;
        if (i as u8) < full && s.byte() < 0xF0 {
            f |= 0x40;
        }
        rec[0xE] = f;
        if s.byte() < 0x40 {
            rec[1] = s.byte();
            rec[2] = 0;
        }
        for (j, b) in rec.into_iter().enumerate() {
            m.mem.w8(DS, 0x0212 + 0x13 * i + j as u16, b);
        }
    }
    let v = s.word();
    m.mem.w16(DS, 0x01DF, v);
    let n = s.byte() % 16;
    for j in 0..n as u16 {
        let c = 0x41 + s.byte() % 26;
        m.mem.w8(DS, 0x0A22 + j, c);
    }
    m.mem.w8(DS, 0x0A22 + n as u16, 0);
    let c = s.byte();
    m.mem.w8(DS, 0x9288, c);
    op
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/status.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "status", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let ax = match op {
            0 => {
                m.time_text(0x1144);
                0
            }
            1 => m.carried_weight(),
            2 => {
                m.status_text();
                0
            }
            _ => {
                m.pilfer_arm();
                0
            }
        };
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(format!("{ax:x} | {:x}", fnv(changed)), format!("{} | {}", f[3], f[4]), "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n}");
}
