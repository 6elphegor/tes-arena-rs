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

/// Fixed (kept bug #39): with two effects fortifying the player (kind 4,
/// strength and then agility), the status lists both; the original's
/// pointer for the comparison is clobbered by the first line, so only
/// the first effect's attribute is listed.
#[test]
fn fixed_every_fortified_attribute_is_listed() {
    use arena::fixes::{Fixes, STATUS_FORTIFIED};
    let Some(img) = common::image() else { return };
    let lines = |fixes: Fixes| {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        for i in 0..0x11u16 {
            m.mem.w8(DS, 0x0660 + i * 0x3C + 0x3B, 0xFF);
        }
        for (i, attr) in [(0u16, 0u16), (1, 3)] {
            let slot = 0x0660 + i * 0x3C;
            m.mem.w16(DS, slot + 0x28, 0x01A4);
            m.mem.w16(DS, slot + 0x2A, DS);
            m.mem.w8(DS, slot + 0x3A, 4);
            m.mem.w8(DS, slot + 0x3B, 0);
            m.mem.w8(DS, slot + 6 + attr, 10);
        }
        m.mem.w16(DS, 0x01A4 + 0x59, 100);
        m.mem.w16(DS, 0x01A4 + 0x5B, 100);
        m.mem.w16(DS, 0x05B1, 0x10);
        m.status_conditions(0x1044);
        let text = String::from_utf8_lossy(m.mem.cstr(DS, 0x1044)).into_owned();
        text.split('\r').filter(|l| l.contains("fortified")).map(String::from).collect::<Vec<_>>()
    };
    let faithful = lines(Fixes::NONE);
    assert_eq!(faithful.len(), 1, "{faithful:?}");
    let fixed = lines(Fixes::only(STATUS_FORTIFIED));
    assert_eq!(fixed.len(), 2, "{fixed:?}");
    assert_eq!(fixed[0], faithful[0]);
    assert_ne!(fixed[1], fixed[0]);
}
