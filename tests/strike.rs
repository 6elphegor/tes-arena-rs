//! Differential tests of the player's strike (src/strike.rs) against
//! vectors from the original code (harness/gen_strike.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, setup, strike, CSEG, FRAME, RSIZE, WSEG};

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
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/strike.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert!(f[0] == "strike" || f[0] == "quest", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        let si = strike(&mut m, &mut s, k * RSIZE);
        m.mem.w16(DS, si + 0x12, k * RSIZE);
        if f[0] == "quest" {
            // Mirrors harness/gen_strike.py `quest`.
            let rec = k * RSIZE;
            let v = m.mem.r16(DS, 0x05CA) | 0x40;
            m.mem.w16(DS, 0x05CA, v);
            m.mem.w16(CSEG, rec + 0x59, 1);
            m.mem.w16(CSEG, rec + 0x5B, 40);
            m.mem.w16(DS, 0x0E6E + 4, 0x40 | (hex(p[2]) as u16) << 9);
            let (ty, cl) = (m.mem.r8(CSEG, rec + 4), m.mem.r8(CSEG, rec + 5));
            put(&mut m, DS, 0x0E6E + 0x2F, &[(cl & 0x1F) + 0x18, ty]);
        }
        m.load_rng();
        let before = m.clone();
        let got = m.strike_actor(si, &mut arena::strike::NoKillHooks).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("STRIKE_DUMP").is_ok_and(|v| v == p[0]) {
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
        *counts.entry(if f[2] == "fault" { "fault" } else { f[0] }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("strike").copied().unwrap_or(0) > 2000);
    assert!(counts.get("quest").copied().unwrap_or(0) > 500);
}
