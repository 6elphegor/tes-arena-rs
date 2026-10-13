//! Differential tests of two pieces of entering an area (src/frame.rs
//! `ems_reset`, 0000:5D74, and `weather_drops`, 0000:E322) against
//! vectors from the original code (harness/gen_area.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_area.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = (s.byte() < 0xC0) as u8;
    m.mem.w8(DS, 0x8FA6, v);
    let n = s.byte() % 0x40;
    m.mem.w16(DS, 0x9032, n as u16);
    let v = if s.byte() < 0xF0 { s.byte() % (n + 1) } else { n };
    m.mem.w16(DS, 0x9034, v as u16);
    let b: Vec<u8> = (0..0x80).map(|_| s.byte()).collect();
    put(m, DS, 0x8FAA, &b);
    let v = if s.byte() < 0xA0 { 0x82 } else { s.byte() };
    m.mem.w8(DS, 0xBB09, v);
    let (x0, y0) = ((s.byte() % 0x40) as u16, (s.byte() % 0x40) as u16);
    m.mem.w16(DS, 0xA914, x0);
    m.mem.w16(DS, 0xA916, y0);
    let v = if s.byte() < 0xF0 { x0 + 1 + s.byte() as u16 } else { x0 };
    m.mem.w16(DS, 0xA918, v);
    let v = if s.byte() < 0xF0 { y0 + 0x15 + s.byte() as u16 } else { y0 + 0x14 };
    m.mem.w16(DS, 0xA91A, v);
    let b: Vec<u8> = (0..0x40).map(|_| s.byte()).collect();
    put(m, DS, 0xE05E, &b);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x91F0, 0);
    let mut n = 0;
    for line in include_str!("vectors/area.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "area", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        m.ems_reset();
        let got = m.weather_drops().map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x}", fnv(changed))
        });
        if std::env::var("AREA_DUMP").is_ok_and(|v| v == f[1]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        n += 1;
    }
    assert!(n > 1500);
}
