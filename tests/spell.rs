//! Differential tests of a cast applied to its targets (src/spell.rs,
//! 29CF:0941) against vectors from the original code
//! (harness/gen_spell.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

const CSEG: u16 = 0x7800;
const SSEG: u16 = 0x6000;
const TSEG: u16 = 0x5C00;
const WSEG: u16 = 0x6800;
const FRAME: u16 = 0x8800;
const PLAYER: u16 = 0x01A4;
const CSIZE: u16 = 0x41E;
const CASTS: u16 = 0x7F86;
const SLOTS: u16 = 0x0660;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

fn raw(s: &mut Stream, n: u16) -> Vec<u8> {
    (0..n).map(|_| s.byte()).collect()
}

/// A far pointer: the player or one of the creatures.
fn target(s: &mut Stream) -> (u16, u16) {
    let k = (s.byte() % 9) as u16;
    if k == 8 { (PLAYER, DS) } else { (k * CSIZE, CSEG) }
}

fn person(m: &mut Machine, s: &mut Stream, seg: u16, off: u16, creature: bool) {
    let b = raw(s, 0x6E);
    put(m, seg, off, &b);
    let ty = if creature { s.byte() % 0x18 } else { s.byte() % 8 };
    m.mem.w8(seg, off + 4, ty);
    let c = s.byte();
    let class = match c {
        0..=0x1F => 0x23,
        0x20..=0x2F => 0x11,
        0x30..=0x3F => 0x0F,
        _ => c,
    };
    m.mem.w8(seg, off + 5, class);
    let level = s.byte() % 30;
    m.mem.w8(seg, off + 6, level);
    let f = s.word() & if creature { 0x00D0 } else { 0x40D0 } | if creature { 0x1000 } else { 0 };
    m.mem.w16(seg, off + 0x40D, f);
    let b = raw(s, 9);
    put(m, seg, off + 0x40F, &b);
    let v = if s.byte() < 0x80 { s.word() } else { 0 };
    m.mem.w16(seg, off + 0x41A, v);
    let v = if s.byte() < 0xA0 { 0 } else { s.byte() as u16 };
    m.mem.w16(seg, off + 0x41C, v);
}

/// Mirrors harness/gen_spell.py `setup`; returns the cast record.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut s = Stream(seed);
    let s = &mut s;
    person(m, s, DS, PLAYER, false);
    for k in 0..8 {
        person(m, s, CSEG, k * CSIZE, true);
        let si = 0x0C57 + k * 0x1C;
        let v = s.word() & 0x3FFF;
        m.mem.w16(DS, si + 0xF, v);
        m.mem.w16(DS, si + 0x12, k * CSIZE);
    }
    for i in 0..18 {
        let v = if s.byte() < 0xF0 { (s.byte() % 8) as u16 } else { 0 };
        m.mem.w16(SSEG, 2 * i, v);
    }
    let ty = (s.byte() % 6).wrapping_sub(1);
    let element = s.byte() % 6;
    put(m, SSEG, 0x24, &[ty, 0, element]);
    let v = s.word();
    m.mem.w16(SSEG, 0x27, v);
    let mut kinds = [0u8; 3];
    for k in kinds.iter_mut() {
        let t = s.byte();
        *k = match t {
            0..=0x17 => 0xFF,
            0x18..=0x1F => 0x19 + t % 8,
            _ => t % 0x19,
        };
    }
    let mut subs = [0u8; 3];
    for v in subs.iter_mut() {
        *v = s.byte() % 6;
    }
    for (k, v) in kinds.iter().zip(subs.iter_mut()) {
        if *k == 13 {
            *v %= 2;
        }
    }
    let params: Vec<u8> = (0..3).map(|_| s.byte() % 16).collect();
    put(m, SSEG, 0x29, &kinds);
    put(m, SSEG, 0x2C, &subs);
    put(m, SSEG, 0x2F, &params);
    let v = s.word();
    m.mem.w16(SSEG, 0x32, v);
    let cast = CASTS + (s.byte() % 8) as u16 * 0x16;
    let (off, seg) = target(s);
    let level = 1 + (s.byte() % 30) as u16;
    let flags = s.word();
    for (i, v) in [0, SSEG, off, seg, level, flags].into_iter().enumerate() {
        m.mem.w16(DS, cast + 2 * i as u16, v);
    }
    let n = 1 + (s.byte() % 3) as u16;
    m.mem.w16(DS, 0x0D5A, n);
    for i in 0..16 {
        let (off, seg) = target(s);
        m.mem.w16(DS, 0x0D5C + 4 * i, off);
        m.mem.w16(DS, 0x0D5E + 4 * i, seg);
    }
    m.mem.w16(DS, 0x1020, 0);
    m.mem.w16(DS, 0x1022, TSEG);
    let b = raw(s, 0x240);
    put(m, TSEG, 0, &b);
    let b = raw(s, 16 * 0x3C);
    put(m, DS, SLOTS, &b);
    for i in 0..16 {
        let at = SLOTS + i * 0x3C;
        if s.byte() < 0x80 {
            m.mem.w8(DS, at + 0x3B, 0xFF);
        } else {
            let (off, seg) = target(s);
            m.mem.w16(DS, at + 0x28, off);
            m.mem.w16(DS, at + 0x2A, seg);
            let k = s.byte() % 12;
            m.mem.w8(DS, at + 0x3A, k);
        }
    }
    let v = 1 + (s.byte() % 30) as u16;
    m.mem.w16(DS, 0x1292, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x05E6, v);
    let b = raw(s, 10);
    put(m, DS, 0x0DC0, &b);
    let v = if s.byte() < 0x80 { 0x3C } else { s.word() };
    m.mem.w16(DS, 0xC3D2, v);
    let sq = ((s.byte() % 64) as u16) << 8 | ((s.byte() % 64) as u16) << 1;
    m.mem.w16(DS, 0xCB90, sq);
    let v = if s.byte() < 0x80 { 0 } else { s.word() };
    m.mem.w16(WSEG, sq, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC942, v);
    let o = s.byte() as u16;
    m.mem.w16(DS, 0xD0B8 + 4 * 0x19, o);
    m.mem.w16(DS, 0xD0BA + 4 * 0x19, 0);
    let b = raw(s, 0x110);
    put(m, FRAME, 0, &b);
    let b = raw(s, 4);
    put(m, DS, 0x91F6, &b);
    cast
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/spell.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "cast", "{line}");
        let seed = hex(f[1]) as u32;
        let mut m = base.clone();
        let cast = setup(&mut m, seed);
        m.load_rng();
        let before = m.clone();
        let got = m.cast_apply(cast).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x400u16).map(|o| m.mem.r8(TSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("SPELL_DUMP").is_ok_and(|v| v == f[1]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
            for o in 0..0x8000u16 {
                if m.mem.r8(CSEG, o) != before.mem.r8(CSEG, o) {
                    eprintln!("c{o:04x} {:02x} {:02x}", before.mem.r8(CSEG, o), m.mem.r8(CSEG, o));
                }
            }
            for o in 0..0x400u16 {
                if m.mem.r8(TSEG, o) != before.mem.r8(TSEG, o) {
                    eprintln!("t{o:04x} {:02x} {:02x}", before.mem.r8(TSEG, o), m.mem.r8(TSEG, o));
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
    assert!(counts.get("ran").copied().unwrap_or(0) > 2000);
}
