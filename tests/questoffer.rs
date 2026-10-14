//! Differential tests of the main quest's next step offered
//! (src/quest.rs `quest_offer`, 1194:CD9A) against vectors from the
//! original code (harness/gen_questoffer.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const TSEG: u16 = 0x7000;

/// Mirrors harness/gen_questoffer.py `setup` (a conditional's test byte
/// comes first, `and` short-circuits, a list's items before its index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let pieces = s.byte() < 0x30;
    let empty = s.byte() < 0x20;
    for i in 0..40u16 {
        let at = 0x0212 + i * 0x13;
        let t = s.byte();
        let t = if pieces && s.byte() < 0x20 { t | 0x80 } else { t & 0x7F };
        m.mem.w8(DS, at, t);
        let f = s.byte();
        m.mem.w8(DS, at + 0x0E, if empty { f & !0x40 } else { f });
    }
    let c = s.byte() % 18;
    let v = c | s.byte() & 0xE0;
    m.mem.w8(DS, 0x01A9, v);
    for i in 0..0x13u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x13DA + i, b);
    }
    let mut block = [0u8; 0x13];
    for b in block.iter_mut() {
        *b = s.byte();
    }
    block[9] = if s.byte() < 0x80 { [0x00, 0x04, 0x0C][s.byte() as usize % 3] } else { s.byte() };
    let k = s.byte() % 4;
    let one = 0xFFFFu16 ^ 1 << (s.byte() % 16);
    let any = s.word();
    let done = [0xFFFF, one, any, 0][k as usize];
    block[10..12].copy_from_slice(&done.to_le_bytes());
    for (i, &b) in block.iter().enumerate() {
        m.mem.w8(DS, 0x0FDD + i as u16, b);
    }
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0A46, v);
    let hi = s.byte() as u32;
    let t = hi << 16 | s.word() as u32;
    m.mem.w32(DS, 0x05CC, t);
    let v = if s.byte() < 0x08 { 0 } else { 1 + (s.byte() % 32) as u16 };
    m.mem.w16(DS, 0x1387, v);
    m.mem.w16(DS, 0x1AB5, 0);
    m.mem.w16(DS, 0x1AB7, TSEG);
    let lo = s.word() as u32;
    let seed = lo | (s.word() as u32) << 16;
    m.mem.w32(DS, 0x91F6, seed);
    for i in 0..0x80u16 {
        let b = s.byte();
        m.mem.w8(TSEG, i, b);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let (mut n, mut offered, mut stuck) = (0, 0, 0);
    for line in include_str!("vectors/questoffer.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "questoffer", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let Some(cf) = m.quest_offer() else {
            assert!(f[2] == "hang" || f[2] == "fault", "{line}");
            stuck += 1;
            continue;
        };
        m.store_rng();
        offered += !cf as u32;
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{} {:x}", cf as u8, fnv(changed)), "{line}");
        n += 1;
    }
    eprintln!("questoffer: {n} compared, {offered} offered, {stuck} hang or fault");
    assert!(n > 3500, "{n}");
}
