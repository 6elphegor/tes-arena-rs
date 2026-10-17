//! Differential tests of the guards arriving (src/encounter.rs
//! `guards_arrive`, 1194:33C0) against vectors from the original code
//! (harness/gen_guards.py).
mod common;

use arena::bsa::Files;
use arena::encounter::FightHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7000;
const CSEG: u16 = 0x7800;
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
        count(m, if people { 0xFFE0 } else { 0xFFE2 });
        Some(())
    }
    fn fight_music(&mut self, m: &mut Machine, _files: &mut Files, _music: u8) -> Option<()> {
        count(m, 0xFFE4);
        Some(())
    }
}

/// Mirrors harness/gen_guards.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let px = 0x1000 + s.word() % 0x2000;
    let py = 0x1000 + s.word() % 0x2000;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    m.mem.w16(DS, 0xCB90, ((py >> 7) << 8) + ((px >> 7) << 1));
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0A46, v);
    let v = s.byte() % 9;
    m.mem.w8(DS, 0x0A47, v);
    let v = s.byte() % 4;
    m.mem.w8(DS, 0xC936, v);
    for i in 0..23u16 {
        let si = 0x0AB3 + 0x1C * i;
        let v = 0xC0 + s.byte() % 6;
        m.mem.w8(DS, si + 0xC, v);
        let w = s.word() & 0x3FFF;
        let v = w | if i >= 15 && s.byte() < 0xC0 { 0x4000 } else { 0 };
        m.mem.w16(DS, si + 0xF, v);
    }
    for dy in -8i32..9 {
        for dx in -8i32..9 {
            let sq = ((((py >> 7) as i32 + dy) << 8) + (((px >> 7) as i32 + dx) << 1)) as u16;
            let v = if s.byte() < 0xC0 { 0 } else { s.word() & 0x7FFF };
            m.mem.w16(WSEG, sq, v);
        }
    }
    m.mem.w16(DS, 0x0FF9, 0);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    for (o, v) in [
        (0xD806u16, WSEG),
        (0xCCD0, FSEG),
        (0x1042, CSEG),
        (0x9030, FRAME),
        (0xCCCE, FRAME + 0xC00),
        (0x91F0, 0),
        (0x05C8, 0),
        (0xC38E, 0xC0),
        (0xC357, 0x80),
        (0xC359, 0x80),
    ] {
        base.mem.w16(DS, o, v);
    }
    base.heap_init(0xA000, 0xF000).unwrap();
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena"));
    let Ok(mut files) = Files::new(&dir) else { return };
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/guards.txt").lines().take(std::env::var("GUARDS_N").map_or(usize::MAX, |v| v.parse().unwrap())) {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "guards", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m.guards_arrive(&mut files, &mut Hooks).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let heap = (0xA000u16..0xF000).flat_map(|seg| (0..16u16).map(move |o| (seg, o)));
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv(heap.map(|(seg, o)| m.mem.r8(seg, o)))
            )
        });
        if std::env::var("GUARDS_DUMP").is_ok_and(|v| v == f[1]) {
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
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(if f[2] == "fault" { "fault" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 1500);
}
