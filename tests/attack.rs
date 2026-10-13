//! Differential tests of blows (src/attack.rs) against vectors from the
//! original code (harness/gen_attack.py): each case fills the player and
//! a combatant from a seed and runs one blow.
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{setup, CSEG, PLAYER, RSIZE};

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
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/attack.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert!(f[0] == "creature" || f[0] == "special", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let mut m = base.clone();
        setup(&mut m, seed, k, &classes);
        if f[0] == "special" {
            // Mirrors harness/gen_attack.py `special`.
            m.mem.w8(CSEG, k * RSIZE + 4, hex(p[2]) as u8);
            m.mem.w16(CSEG, k * RSIZE + 0x40D, 0x1000);
            m.mem.w8(DS, PLAYER + 0x62, 0xFF);
            m.mem.w16(DS, 0x05BE, 0);
        }
        m.mem.w16(DS, 0x1400, k * RSIZE);
        m.mem.w16(DS, 0x1402, PLAYER);
        m.load_rng();
        let before = m.clone();
        let got = m.attack_player(0).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))))
        });
        if std::env::var("ATTACK_DUMP").is_ok_and(|v| v == p[0]) {
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
    assert!(counts.get("creature").copied().unwrap_or(0) > 2000);
    assert!(counts.get("special").copied().unwrap_or(0) > 500);
}
