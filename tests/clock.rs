//! Differential tests of the game clock (src/clock.rs, 0000:142D)
//! against vectors from the original code (harness/gen_clock.py).
mod common;

use arena::clock::NoClockHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::clock::{setup, CSEG, TSEG};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0x91F0, 0);
    let auras: Vec<u8> = (0..0x30u16).filter(|&t| base.mem.r8(DS, 0x2DE6 + t) == 0xFE).map(|t| t as u8 + 1).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/clock.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "clock", "{line}");
        let seed = hex(f[1]) as u32;
        let mut m = base.clone();
        let minutes = setup(&mut m, seed, &auras);
        m.load_rng();
        if std::env::var("CLOCK_STEPS").is_ok_and(|v| v == f[1]) {
            let mut t = m.clone();
            for i in 0..minutes {
                let r = t.advance_minutes(&mut arena::bsa::Files::default(), 1, &mut NoClockHooks);
                t.store_rng();
                eprintln!("minute {i} {:?} seed {:08x} t {:x}", r, t.mem.r32(DS, 0x91F6), t.mem.r32(DS, 0x05CC));
            }
        }
        let before = m.clone();
        let got = m.advance_minutes(&mut arena::bsa::Files::default(), minutes, &mut NoClockHooks).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x400u16).map(|o| m.mem.r8(TSEG, o)))
            )
        });
        if std::env::var("CLOCK_DUMP").is_ok_and(|v| v == f[1]) {
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
        *counts.entry(if f[2] == "fault" { "fault" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 900);
}
