//! Differential tests of a city entered (src/cityinfo.rs `city_entered`,
//! 0000:46B2..4766) against vectors from the original code
//! (harness/gen_entered.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

const LSEG: u16 = 0x6C00;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn changes(before: &Machine, after: &Machine, seg: u16) -> u64 {
    fnv((0..=0xFFFFu16)
        .filter(|&o| after.mem.r8(seg, o) != before.mem.r8(seg, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, after.mem.r8(seg, o)]))
}

/// The generator's stubs: 3EBF counts at FFE0 and adds AL to FFE6, the
/// music counts at FFE2.
struct Stubs;

impl QuestHooks for Stubs {
    fn city_info(&mut self, m: &mut Machine, _files: &mut Files, ax: u16) -> Option<()> {
        let v = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
        m.mem.w16(DS, 0xFFE0, v);
        let b = m.mem.r8(DS, 0xFFE6).wrapping_add(ax as u8);
        m.mem.w8(DS, 0xFFE6, b);
        Some(())
    }
    fn music(&mut self, m: &mut Machine, _files: &mut Files) {
        let v = m.mem.r16(DS, 0xFFE2).wrapping_add(1);
        m.mem.w16(DS, 0xFFE2, v);
    }
}

/// Mirrors harness/gen_entered.py `setup` (a conditional's test byte
/// first; a tuple's items before its index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = if s.byte() < 0x20 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0x40 { 0x4000 } else { 0 };
    m.mem.w16(DS, 0x05CA, v);
    let place = s.byte();
    m.mem.w8(DS, 0x0A4C, place);
    let seen = s.word();
    m.mem.w16(DS, 0xCCA8, seen);
    m.mem.w16(DS, 0x0FC8, seen);
    for at in [0x1387u16, 0x138B, 0x1389] {
        let b = s.byte();
        m.mem.w16(DS, at, if b < 0x10 { 0 } else { (b % 7) as u16 });
    }
    let v = s.word();
    m.mem.w16(DS, 0x138D, v);
    let fill = s.word();
    for i in 0..=0xFFFFu16 {
        m.mem.w8(LSEG, i, (fill as u32 + 3 * i as u32) as u8);
    }
    let mut pairs: [(u16, Vec<u16>); 3] = [(0, Vec::new()), (1, Vec::new()), (3, Vec::new())];
    for (sort, base) in [(0usize, 0u16), (1, 0x100), (2, 0x180)] {
        for k in 0..8u16 {
            let (sq, pair) = (s.word(), s.word());
            m.mem.w16(LSEG, base + 4 * k, sq);
            m.mem.w16(LSEG, base + 4 * k + 2, pair);
            pairs[sort].1.push(pair);
        }
    }
    for (at, off) in [(0x1AB5u16, 0u16), (0x1008, 0x100), (0x1004, 0x180)] {
        m.mem.w16(DS, at, off);
        m.mem.w16(DS, at + 2, LSEG);
    }
    for q in 0..4u16 {
        let si = 0x0E6E + q * 0x34;
        let b = s.byte();
        let kind = (if b < 0x90 { 1 } else { s.byte() % 4 }) as u16;
        let sort = [0u16, 1, 2, 3, 6, 4, 0, 1, 3][s.byte() as usize % 9];
        let low = s.word() & 0x0F3F;
        m.mem.w16(DS, si + 4, sort << 12 | low | kind << 6);
        let b = s.byte();
        let p = if b < 0xC0 { place } else { s.byte() };
        m.mem.w8(DS, si + 0x24, p);
        let list = pairs.iter().find(|(k, _)| *k == sort).map(|(_, l)| l.clone()).unwrap_or_default();
        let b = s.byte();
        let pair = if !list.is_empty() && b < 0x90 { list[b as usize % 8] } else { s.word() };
        m.mem.w16(DS, si + 0x16, pair);
        let g = s.word();
        m.mem.w16(DS, si + 0x14, g);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/entered.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "entered", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m.city_entered(&mut Files::default(), &mut Stubs).map(|()| {
            m.store_rng();
            [DS, LSEG].map(|s| format!("{:x}", changes(&before, &m, s))).join(" ")
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("stuck"), "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n}");
}
