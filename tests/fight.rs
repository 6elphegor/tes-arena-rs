//! Differential tests of a fight starting (src/encounter.rs
//! `fight_prepare`, 25CA:0000, and `place_creatures`, 25CA:006A) against
//! vectors from the original code (harness/gen_fight.py).
mod common;

use arena::bsa::Files;
use arena::encounter::FightHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7000;
const CSEG: u16 = 0x7800;
const RSEG: u16 = 0x6000;
const FRAME: u16 = 0x8800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's hooks: calls counted at FFE0..FFE4.
struct Hooks;

impl FightHooks for Hooks {
    fn fight_graphics(&mut self, m: &mut Machine, _files: &mut Files, people: bool) -> Option<()> {
        count(m, if people { 0xFFE2 } else { 0xFFE0 });
        Some(())
    }
    fn fight_music(&mut self, m: &mut Machine, _music: u8) {
        count(m, 0xFFE4);
    }
}

/// Mirrors harness/gen_fight.py `setup`; returns (al, count).
fn setup(m: &mut Machine, seed: u32) -> (u8, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let px = 0x1000 + s.word() % 0x2000;
    let py = 0x1000 + s.word() % 0x2000;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    m.mem.w16(DS, 0xCB90, ((py >> 7) << 8) + ((px >> 7) << 1));
    for i in 0..8u16 {
        let si = 0x0C57 + 0x1C * i;
        let b: Vec<u8> = (0..0x1C).map(|_| s.byte()).collect();
        put(m, DS, si, &b);
        let w = s.word() & 0xBEFF;
        let v = w | if s.byte() < 0xA0 { 0x4000 } else { 0 };
        m.mem.w16(DS, si + 0xF, v);
        let rec = 0x41E * i;
        let v = s.byte();
        m.mem.w8(CSEG, rec + 0x2D, v);
        let v = s.word() & 0x1000;
        m.mem.w16(CSEG, rec + 0x40D, v);
    }
    for i in 0..23u16 {
        let k = s.byte();
        let sq = if k < 0x40 {
            let r = ((py >> 7) + (s.byte() % 9) as u16).wrapping_sub(4);
            let c = ((px >> 7) + (s.byte() % 9) as u16).wrapping_sub(4);
            (r << 8).wrapping_add(c << 1)
        } else {
            0xFFFF
        };
        m.mem.w16(DS, 0x1951 + 2 * i, sq);
    }
    for dy in -8i32..9 {
        for dx in -8i32..9 {
            let sq = ((((py >> 7) as i32 + dy) << 8) + (((px >> 7) as i32 + dx) << 1)) as u16;
            let k = s.byte();
            let v = if k < 0xB0 {
                0
            } else if k < 0xC8 {
                0x8000 | s.byte() as u16
            } else {
                s.word() & 0x7FFF
            };
            m.mem.w16(WSEG, sq, v);
            let k = s.byte();
            let v = if k < 0xE0 { FRAME } else { FRAME + 0xC00 + s.byte() as u16 };
            m.mem.w16(FSEG, sq, v);
        }
    }
    let v = if s.byte() < 0x80 { 0 } else { (s.byte() % 10) as u16 };
    m.mem.w16(DS, 0x0FF9, v);
    let v = s.word();
    m.mem.w16(DS, 0x0DBE, v);
    let v = s.word();
    m.mem.w16(DS, 0x0FF3, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x0F74, v);
    let v = s.byte();
    m.mem.w8(DS, 0x100D, v);
    let v = if s.byte() < 0x80 { 4 } else { s.byte() % 6 };
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC943, v);
    let b: Vec<u8> = (0..8).map(|_| s.byte()).collect();
    put(m, DS, 0xC987, &b);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
    let k = s.byte();
    let al = if k < 0xC0 { s.byte() } else { 0 };
    let n = if s.byte() < 0xF0 { 1 + (s.byte() % 6) as u16 } else { (s.byte() % 12) as u16 };
    (al, n)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    for (o, v) in [(0xD806u16, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0xCCC2, RSEG), (0x9030, FRAME), (0xCCCE, FRAME + 0xC00), (0x91F0, 0), (0x05C8, 0)] {
        base.mem.w16(DS, o, v);
    }
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/fight.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "fight", "{line}");
        let mut m = base.clone();
        let (al, n) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m
            .fight_prepare(&mut files, &mut Hooks, al)
            .and_then(|()| m.place_creatures(n))
            .map(|()| {
                m.store_rng();
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                format!(
                    "{:x} {:x} {:x}",
                    fnv(changed),
                    fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))),
                    fnv((0..0x1000u16).map(|o| m.mem.r8(RSEG, o)))
                )
            });
        if std::env::var("FIGHT_DUMP").is_ok_and(|v| v == f[1]) {
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
    assert!(counts.get("ran").copied().unwrap_or(0) > 2500);
}
