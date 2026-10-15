//! Differential tests of the status box's conditions (src/status.rs
//! `status_conditions`, 2686:005E) against vectors from the original code
//! (harness/gen_conditions.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const PLAYER: u16 = 0x01A4;
const SLOTS: u16 = 0x0660;

/// Mirrors harness/gen_conditions.py `setup` (a conditional's test byte
/// comes first, a tuple's items before its index); returns DI.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let mut flags = 0u16;
    for bit in 0..16 {
        if s.byte() < 0x30 {
            flags |= 1 << bit;
        }
    }
    m.mem.w16(DS, PLAYER + 0x40D, flags);
    for at in [0x41A, 0x41C] {
        let v = if s.byte() & 1 != 0 { s.word() } else { 0 };
        m.mem.w16(DS, PLAYER + at, v);
    }
    let mx = s.word();
    let k = s.byte() % 3;
    let hp = if k != 0 { s.word() } else { (mx / 10).wrapping_add(s.byte() as u16 % 3).wrapping_sub(1) };
    m.mem.w16(DS, PLAYER + 0x59, hp);
    m.mem.w16(DS, PLAYER + 0x5B, mx);
    let v = s.word() & !0x10;
    let v = v | if s.byte() & 1 != 0 { 0x10 } else { 0 };
    m.mem.w16(DS, 0x05B1, v);
    let b = s.byte();
    m.mem.w8(DS, 0x0D39, b);
    let v = s.word();
    m.mem.w16(DS, 0x12B2, v);
    let no_seg = s.byte() < 0x40;
    for i in 0..0x11u16 {
        let mut rec: Vec<u8> = (0..0x3C).map(|_| s.byte()).collect();
        if s.byte() < 0x40 {
            rec[0x3B] = 0xFF;
        } else if rec[0x3B] == 0xFF {
            rec[0x3B] = 0;
        }
        if s.byte() & 1 != 0 {
            rec[0x28..0x2A].copy_from_slice(&PLAYER.to_le_bytes());
            if s.byte() >= 0x40 && !no_seg {
                rec[0x2A..0x2C].copy_from_slice(&DS.to_le_bytes());
            }
        }
        let kind = s.byte();
        rec[0x3A] = [4, 0x0A + kind % 15, kind][s.byte() as usize % 3];
        for j in 0..8 {
            if s.byte() < 0xA0 {
                rec[6 + j] = 0;
            }
        }
        for (j, b) in rec.into_iter().enumerate() {
            m.mem.w8(DS, SLOTS + 0x3C * i + j as u16, b);
        }
    }
    let n = s.byte() % 8;
    for j in 0..n as u16 {
        let c = 0x61 + s.byte() % 26;
        m.mem.w8(DS, 0x941E + j, c);
    }
    m.mem.w8(DS, 0x941E + n as u16, 0);
    0x941E + n as u16
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/conditions.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "conditions", "{line}");
        let mut m = base.clone();
        let di = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        m.status_conditions(di);
        if std::env::var("COND_DUMP").is_ok_and(|d| d == f[1]) {
            eprintln!("{:?}", String::from_utf8_lossy(m.mem.cstr(DS, 0x941E)));
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) && !(0x941E..0x9700).contains(&o) {
                    eprintln!("{o:#x} {} {}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} compared");
}
