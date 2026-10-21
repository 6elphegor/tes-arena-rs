//! Differential tests of the piles of treasure made (src/piles.rs
//! `piles`, 2433:0000) against vectors from the original code
//! (harness/gen_piles.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x7C00;
const FRAME: u16 = 0xA000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn put(m: &mut Machine, at: u16, data: &[u8]) {
    for (i, &b) in data.iter().enumerate() {
        m.mem.w8(DS, at.wrapping_add(i as u16), b);
    }
}

/// Mirrors harness/gen_piles.py `setup`, draw for draw. Returns bank 6
/// (or the frame's bytes) and whether EMS is on.
fn setup(m: &mut Machine, seed: u32) -> (Vec<u8>, bool) {
    let mut st = Stream(seed);
    let s = &mut st;
    let making = s.byte() < 0xC0;
    m.mem.w8(DS, 0x4C91, if making { 0xFF } else { 0 });
    let ems = s.byte() < 0xC0;
    let count = match s.byte() % 8 {
        0 => 0,
        1 => 0xC8,
        2 => s.byte() % 0xC8,
        _ => s.byte() % 6,
    };
    m.mem.w8(DS, 0x0FF2, count);
    let v = [3, 4, 4, 2][s.byte() as usize % 4];
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() % 0xC;
    m.mem.w8(DS, 0x0D48, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    let spots = match s.byte() % 8 {
        0 => s.byte(),
        1 => s.byte() % 3,
        _ => 2 + s.byte() % 20,
    };
    m.mem.w8(DS, 0xC765, spots);
    let mut v = s.word() & !0x200;
    if s.byte() < 0x20 {
        v |= 0x200;
    }
    m.mem.w16(DS, 0xC39A, v);
    let mut v = s.word() & !4;
    if s.byte() < 0x50 {
        v |= 4;
    }
    m.mem.w16(DS, 0xC398, v);
    let level = if s.byte() < 0xE0 { s.byte() % 8 } else { s.byte() };
    m.mem.w8(DS, 0xBB0A, level);
    let climate = if s.byte() < 0xE0 { s.byte() % 3 } else { s.byte() };
    m.mem.w8(DS, 0x0A47, climate);
    let v = s.word();
    m.mem.w16(DS, 0x12C0, v);
    let v = if s.byte() < 0xC0 { (s.byte() % 8) as u16 } else { s.word() };
    m.mem.w16(DS, 0x12C4, v);
    let seed: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, 0x91F6, &seed);
    m.mem.w16(DS, 0x91F0, 0);
    for i in 0..4u16 {
        let v = if s.byte() < 0xC0 { 0 } else { s.byte() };
        m.mem.w8(DS, 0x4C8D + i, v);
    }
    let rec: Vec<u8> = (0..0x1C).map(|_| s.byte()).collect();
    put(m, 0x4BF6, &rec);
    let v = if s.byte() < 0xE0 { s.byte() % 8 } else { s.byte() };
    m.mem.w8(DS, 0x4D76, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CC, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CE, v);
    let mut spot = Vec::new();
    for _ in 0..130 {
        let k = s.byte();
        spot.push(if k < 0x20 {
            0xFF
        } else if k < 0x50 {
            0x80 + s.byte() % 0x7F
        } else {
            s.byte() % 0x40
        });
        spot.push(s.byte() % 0x40);
    }
    put(m, 0xC6E5, &spot);
    let flats: Vec<u8> = (0..8).map(|_| s.byte()).collect();
    put(m, 0xC934, &flats);
    let taken: Vec<u8> = (0..0x40).map(|_| s.byte()).collect();
    put(m, 0x1395, &taken);
    let mut bank = vec![0u8; 0x10000];
    for i in 0..count as usize {
        let at = 0xA000 + 0x1C * i;
        for j in 0..0x1C {
            bank[at + j] = s.byte();
        }
        bank[at + 4] = if s.byte() < 0x80 { level } else { s.byte() % 8 };
    }
    bank[0x4D76] = s.byte() % 8;
    m.load_rng();
    (bank, ems)
}

fn seg_bytes(m: &Machine, seg: u16) -> Vec<u8> {
    (0..=0xFFFFu16).map(|o| m.mem.r8(seg, o)).collect()
}

fn changes(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        if a != b {
            out.extend([i as u8, (i >> 8) as u8, *b]);
        }
    }
    out
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    // EMS on (harness/dos.py `enable_ems`): 20h banks, bank 0 mapped
    // into the frame; the driver's page maps' far pointers as there.
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    for b in 0..0x20u16 {
        base.mem.w16(DS, 0x9036 + 4 * b, b * 2);
        base.mem.w16(DS, 0x9038 + 4 * b, 0x0600);
    }
    let only = std::env::var("PILES_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/piles.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "piles", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let (bank, ems) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        if ems {
            m.ems.resize(7, std::sync::Arc::new(vec![0; 0x10000]));
            m.ems[6] = std::sync::Arc::new(bank);
        } else {
            m.mem.w8(DS, 0x8FA6, 0);
            for (i, &b) in bank.iter().enumerate() {
                m.mem.w8(FRAME, i as u16, b);
            }
        }
        let before = (seg_bytes(&m, DS), seg_bytes(&m, WSEG));
        let got = m.piles().map(|()| {
            m.store_rng();
            let after = seg_bytes(&m, DS);
            if only.is_some() {
                for (o, (a, b)) in before.0.iter().zip(&after).enumerate() {
                    if a != b {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
            }
            let d = [
                fnv(changes(&before.0, &after)),
                fnv(m.ems_bank(6)),
                fnv(seg_bytes(&m, FRAME)),
                fnv(changes(&before.1, &seg_bytes(&m, WSEG))),
            ];
            d.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(" ")
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => {
                assert_eq!(f[2], "stuck", "{line}: the port gave None");
                stuck += 1;
            }
        }
    }
    eprintln!("piles: {n} compared, {stuck} stuck in both");
    if only.is_none() {
        assert!(n > 3000, "{n} compared");
        assert!(stuck < 400, "{stuck} stuck");
    }
}

/// A machine with EMS off (the piles' table at FRAME:A000), no piles,
/// level 2, climate `climate`, inside a town's building of kind 0 (5E6
/// 4, D48 0: kind of place 0) unless `wild` (the wild's level flag: 5),
/// one round for the clicked square (MAKING 0), the dice seeded `seed`.
fn pile_machine(img: &arena::exe::Image, fixes: arena::fixes::Fixes, seed: u32, climate: u8, wild: bool) -> Machine {
    let mut m = Machine::new(img);
    m.fixes = fixes;
    m.mem.w16(DS, 0xD806, WSEG);
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w8(DS, 0x8FA6, 0);
    m.mem.w8(DS, 0x4C91, 0);
    m.mem.w8(DS, 0x0FF2, 0);
    m.mem.w8(DS, 0x05E6, 4);
    m.mem.w8(DS, 0x0D48, 0);
    m.mem.w16(DS, 0xC39A, if wild { 0x200 } else { 0 });
    m.mem.w16(DS, 0xC398, if wild { 4 } else { 0 });
    m.mem.w8(DS, 0xBB0A, 2);
    m.mem.w8(DS, 0x0A47, climate);
    m.mem.w16(DS, 0x12C0, 0x0810);
    put(&mut m, 0x91F6, &seed.to_le_bytes());
    m.load_rng();
    m
}

/// The piles in the table: (kind of place, gold, the item's type).
fn piles_made(m: &Machine) -> Vec<(u8, u16, u8)> {
    (0..m.mem.r8(DS, 0x0FF2) as u16)
        .map(|i| {
            let at = 0xA000 + 0x1C * i;
            (m.mem.r8(FRAME, at + 1), m.mem.r16(FRAME, at + 5), m.mem.r8(FRAME, at + 9))
        })
        .collect()
}

/// Fixed (kept bug #5): a pile made in the wild (kind of place 5) is
/// rolled by the outdoors' tables (kind 3's): gold always, the others by
/// their chances, the pile still of kind 5. The original reads the
/// tables past their end: gold and items never, and a weapon or magic
/// pile (1% each) calls into a string (None).
#[test]
fn fixed_wild_piles_read_the_outdoors_tables() {
    use arena::fixes::{Fixes, WILD_PILES};
    let Some(img) = common::image() else { return };
    let (mut faulted, mut made) = (0, 0);
    for seed in 0..600u32 {
        let seed = seed.wrapping_mul(0x9E37_79B9);
        let mut m = pile_machine(&img, Fixes::NONE, seed, 0, true);
        arena::giveup::take();
        match m.piles() {
            None => {
                let why = arena::giveup::take().expect("a give-up").why;
                assert!(why.contains("string"), "{why}");
                faulted += 1;
            }
            Some(()) => assert!(piles_made(&m).iter().all(|p| p.1 == 0xFFFF), "no gold in the original's wild"),
        }
        let mut m = pile_machine(&img, Fixes::only(WILD_PILES), seed, 0, true);
        if m.piles().is_none() {
            let why = arena::giveup::take().expect("a give-up").why;
            assert!(!why.contains("string"), "{why}");
            continue;
        }
        let p = piles_made(&m);
        assert!(p.iter().all(|p| p.0 == 5), "{p:?}");
        assert!(p.iter().any(|p| p.1 != 0xFFFF), "outdoors, gold always: {p:?}");
        made += 1;
    }
    assert!(faulted > 0, "no case met the original's fault");
    assert!(made > 500, "{made} made");
    // A town's building is as before.
    for seed in 0..50u32 {
        let mut a = pile_machine(&img, Fixes::NONE, seed, 0, false);
        let mut b = pile_machine(&img, Fixes::only(WILD_PILES), seed, 0, false);
        assert_eq!(a.piles().map(|()| piles_made(&a)), b.piles().map(|()| piles_made(&b)));
    }
}

/// Fixed (kept bug #43), the gold pile in climate 2: its amount (2..10)
/// halved keeps the pile when odd, where the original's `shr` leaves the
/// low bit in the carry that drops it.
#[test]
fn fixed_an_odd_halved_amount_keeps_its_pile() {
    use arena::fixes::{Fixes, PILE_QUIRKS};
    let Some(img) = common::image() else { return };
    let gold_only = |m: &mut Machine| put(m, 0x4D5A, &[100, 0, 0, 0]);
    let mut dropped = 0;
    for seed in 0..100u32 {
        let mut m = pile_machine(&img, Fixes::NONE, seed, 2, false);
        gold_only(&mut m);
        m.piles().expect("piles");
        let faithful = piles_made(&m);
        let mut m = pile_machine(&img, Fixes::only(PILE_QUIRKS), seed, 2, false);
        gold_only(&mut m);
        m.piles().expect("piles");
        let fixed = piles_made(&m);
        assert_eq!(fixed.len(), 1, "{fixed:?}");
        assert!((1..=5).contains(&fixed[0].1), "{fixed:?}");
        if faithful.is_empty() {
            dropped += 1;
        } else {
            assert_eq!(faithful, fixed);
        }
    }
    assert!(dropped > 20, "{dropped} dropped by the original");
}

/// Fixed (kept bug #43), the magic weapon or armour pile: its type isn't
/// forced; the original forces the level routine's BX (kind of place 0's
/// routine 03D5 leaves 5, the span of its roll), so every one is type 5.
#[test]
fn fixed_magic_piles_types_are_not_forced() {
    use arena::fixes::{Fixes, PILE_QUIRKS};
    use std::collections::BTreeSet;
    let Some(img) = common::image() else { return };
    let magic_only = |m: &mut Machine| put(m, 0x4D5A, &[0, 0, 0, 100]);
    let (mut faithful, mut fixed) = (BTreeSet::new(), BTreeSet::new());
    for seed in 0..100u32 {
        for (fixes, types) in [(Fixes::NONE, &mut faithful), (Fixes::only(PILE_QUIRKS), &mut fixed)] {
            let mut m = pile_machine(&img, fixes, seed, 0, false);
            magic_only(&mut m);
            if m.piles().is_some() {
                types.extend(piles_made(&m).iter().map(|p| p.2));
            }
        }
    }
    assert_eq!(faithful, BTreeSet::from([5]), "the original's forced type");
    assert!(fixed.len() > 3, "{fixed:?}");
}

/// Fixed (kept bug #43), the level's limit of piles (2433:0101) is read
/// from DGROUP's LIMIT, which 02C3 has just set (half the level's spots),
/// not through the frame's segment.
#[test]
fn fixed_a_full_level_gets_no_more_piles() {
    use arena::fixes::{Fixes, PILE_QUIRKS};
    let Some(img) = common::image() else { return };
    let run = |fixes: Fixes| {
        let mut m = pile_machine(&img, fixes, 7, 0, false);
        // Entering a dungeon's level (kind of place 3) with four spots
        // (a limit of 2) and two of its piles made already; the frame's
        // byte at the limit's offset 9.
        m.mem.w8(DS, 0x4C91, 0xFF);
        m.mem.w8(DS, 0x05E6, 3);
        m.mem.w8(DS, 0xC765, 4);
        put(&mut m, 0xC6E5, &[2, 3, 4, 5, 6, 7, 8, 9]);
        m.mem.w8(DS, 0x0FF2, 2);
        m.mem.w8(FRAME, 0xA000 + 4, 2);
        m.mem.w8(FRAME, 0xA000 + 0x1C + 4, 2);
        m.mem.w8(FRAME, 0x4D76, 9);
        m.piles().expect("piles");
        m.mem.r8(DS, 0x0FF2)
    };
    assert!(run(Fixes::NONE) > 2, "the original reads 9");
    assert_eq!(run(Fixes::only(PILE_QUIRKS)), 2, "full at 2");
}
