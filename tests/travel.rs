//! Differential tests of travel time (src/travel.rs) against vectors from
//! the original code (harness/gen_travel.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir: PathBuf = std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into());
    let Ok(mut files) = Files::new(&dir) else { return };
    let mut base = Machine::new(&img);
    base.heap_init(0xA000, 0xF000).unwrap();
    assert!(base.bsa_open(&mut files));
    let (buf, _) = base.heap_alloc(0x1000).unwrap();
    base.mem.w16(DS, 0x9030, buf);
    let mut n = 0;
    for line in include_str!("vectors/travel.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<&str> = f[1].split(' ').collect();
        let mut m = base.clone();
        if f[0] == "weather" {
            m.mem.w32(DS, 0x5CC, hex(p[0]));
            m.mem.w32(DS, 0x91F6, hex(p[1]));
            m.mem.w16(DS, 0x91F0, 0);
            m.load_rng();
            let before = m.clone();
            m.weather_roll().unwrap();
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
            n += 1;
            continue;
        }
        let mut fs = files.clone();
        m.mem.w8(DS, 0x1A6C, p[0].parse().unwrap());
        m.mem.w8(DS, 0x193D, hex(p[1]) as u8);
        m.mem.w16(DS, 0x193E, hex(p[2]) as u16);
        m.mem.w16(DS, 0x1940, hex(p[3]) as u16);
        m.mem.w8(DS, 0xA46, hex(p[4]) as u8);
        m.mem.w16(DS, 0xA48, hex(p[5]) as u16);
        m.mem.w16(DS, 0xA4A, hex(p[6]) as u16);
        m.mem.w32(DS, 0x5CC, hex(p[7]));
        for i in 0..36 {
            m.mem.w8(DS, 0xA5A + i as u16, hex(&p[8][2 * i..2 * i + 2]) as u8);
        }
        let before = m.clone();
        let got = m.travel_days(&mut fs).map(|days| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{days:x} {:x}", fnv(changed))
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("{n} journeys");
}
