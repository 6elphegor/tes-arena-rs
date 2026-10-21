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

/// Fixed (kept bug #64): a player with HP below 0 counts as low after a
/// blow (0D39 bit 80h); the original's unsigned compare doesn't. (The
/// player starts at -5 HP; blows that set the bit otherwise, as a hit
/// does, are skipped.)
#[test]
fn fixed_negative_hp_counts_as_low() {
    use arena::fixes::{Fixes, LOW_HP_SIGNED};
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0x91F0, 0);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let blow = |seed: u32, fixes: Fixes| {
        let mut m = base.clone();
        m.fixes = fixes;
        setup(&mut m, seed, 0, &classes);
        m.mem.w16(DS, 0x1400, 0);
        m.mem.w16(DS, 0x1402, PLAYER);
        m.load_rng();
        m.mem.w8(DS, 0x0D39, 0);
        m.mem.w16(DS, PLAYER + 0x59, (-5i16) as u16);
        m.attack_player(0)?;
        Some((m.mem.r16(DS, PLAYER + 0x59) as i16, m.mem.r8(DS, 0x0D39) & 0x80 != 0))
    };
    let mut seen = 0;
    for seed in 1..400u32 {
        let Some((hp, low)) = blow(seed, Fixes::NONE) else { continue };
        if hp >= 0 || low {
            continue;
        }
        seen += 1;
        assert_eq!(blow(seed, Fixes::only(LOW_HP_SIGNED)), Some((hp, true)), "seed {seed}");
    }
    assert!(seen > 4, "{seen} blows left the HP below 0");
}

/// Fixed (kept bug #63): the bare-handed search walks the record's own
/// slots for DGROUP's count of its items (left by 1194:7510); the
/// original reads a count in the record's segment and walks from FFFF,
/// so what it finds there is chance: here a worn item of base type 3,
/// category 1 and armour kind 1 that makes the class 8.
#[test]
fn fixed_gauntlet_search_walks_the_records_slots() {
    use arena::fixes::{Fixes, GAUNTLET_COUNT};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(GAUNTLET_COUNT)] {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.mem.w8(CSEG, 0x129D, 1);
        m.mem.w8(CSEG, 0xFFFF, 3);
        m.mem.w8(CSEG, 0x000D, 0x80 | 0x40 | 0x10 | 0x04);
        // The record has no items: bare-handed, fists (7 + 2).
        let want = if fixes == Fixes::NONE { 8 } else { 9 };
        assert_eq!(m.weapon_class(CSEG, RSIZE), want, "{fixes:?}");
    }
}
