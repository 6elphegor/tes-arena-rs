//! Differential tests of a person's graphics list (src/outfit.rs
//! `person_outfit`, 2686:0423) against vectors from the original code
//! (harness/gen_outfit.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const CSEG: u16 = 0x6000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_outfit.py `setup`; returns the record.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let rec = (s.byte() % 8) as u16 * 0x41E;
    let v = if s.byte() < 0x20 { 0 } else { 1 + s.byte() % 0x40 };
    m.mem.w8(CSEG, rec + 9, v);
    // (The generator's tuples take all their bytes.)
    let k = (s.byte() % 5) as usize;
    let cls = [0x20 | s.byte() & 0x1F, 0x4C, 0x0F, s.byte(), s.byte() & 0x1F][k];
    m.mem.w8(CSEG, rec + 5, cls);
    let v = s.byte() & 1;
    m.mem.w8(CSEG, rec + 7, v);
    let v = s.word();
    m.mem.w16(CSEG, rec + 0x40D, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0D49, v);
    let n = s.byte() % 12;
    for i in 0..40u16 {
        let at = rec + 0x6E + 0x13 * i;
        if i >= n as u16 || s.byte() < 0x20 {
            let v = if s.byte() < 0x80 { s.byte() & 0x3F & !0x40 } else { 0 };
            m.mem.w8(CSEG, at + 0xE, v);
            continue;
        }
        let kind = s.byte() % 3;
        let (t, mut flags) = if s.byte() < 0x90 {
            // (Its bytes, then the index.)
            let choices = [0, 7 + s.byte() % 4, 1 + s.byte() % 6, s.byte()];
            let t = choices[(s.byte() % 4) as usize];
            (t, 0x40 | kind << 4)
        } else {
            (s.byte(), 0x40 | 1 << 2 | kind << 4)
        };
        if s.byte() < 0xA0 {
            flags |= 0x80;
        }
        m.mem.w8(CSEG, at, t);
        let h = if s.byte() < 0x10 { 0xFFFF } else { s.word() };
        m.mem.w16(CSEG, at + 6, h);
        m.mem.w8(CSEG, at + 0xE, flags);
    }
    for i in 0..5 {
        let v = s.byte();
        m.mem.w8(DS, 0x12A8 + i, v);
    }
    let v = s.byte() % 8;
    m.mem.w8(CSEG, 0x129D, v);
    for k in 0..4u16 {
        let at = 0xFFFFu16.wrapping_add(0x13 * k);
        let t = if s.byte() < 0x80 { 3 } else { s.byte() };
        m.mem.w8(CSEG, at, t);
        let f = 0xC4 | (s.byte() % 3) << 4;
        m.mem.w8(CSEG, at.wrapping_add(0xE), f);
    }
    rec
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    let mut n = 0;
    for line in include_str!("vectors/outfit.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "outfit", "{line}");
        let mut m = base.clone();
        let rec = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        assert_eq!(m.person_outfit(CSEG, rec), 0x1044);
        let mut names = Vec::new();
        let mut at = 0x1044u16;
        while m.mem.r8(DS, at) != 0 {
            let mut s = String::new();
            while m.mem.r8(DS, at) != 0 {
                s.push(m.mem.r8(DS, at) as char);
                at += 1;
            }
            at += 1;
            names.push(s);
        }
        let names = if names.is_empty() { "-".to_string() } else { names.join(",") };
        assert_eq!(f[2], names, "{line}");
        let changed = |seg: u16| {
            (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(seg, o) != before.mem.r8(seg, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(seg, o)])
                .collect::<Vec<_>>()
        };
        let all = [changed(DS), changed(CSEG)].concat();
        assert_eq!(f[3], format!("{:x}", fnv(all)), "{line}");
        n += 1;
    }
    assert!(n > 3500);
}
