//! Differential tests of random encounters (src/encounter.rs
//! `random_encounter`, 1194:887A) against vectors from the original code
//! (harness/gen_encounter.py).
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
    fn jagar(&mut self, m: &mut Machine, text: u16) {
        m.mem.w16(DS, 0xFFEC, text);
        count(m, 0xFFE6);
    }
    fn view_saved(&mut self, m: &mut Machine) {
        count(m, 0xFFE8);
    }
    fn view_restored(&mut self, m: &mut Machine) {
        count(m, 0xFFEA);
    }
    fn fight_music(&mut self, m: &mut Machine, _files: &mut Files, _music: u8) -> Option<()> {
        count(m, 0xFFE4);
        Some(())
    }
}

/// Mirrors harness/gen_encounter.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let px = 0x1000 + s.word() % 0x2000;
    let py = 0x1000 + s.word() % 0x2000;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let sq = ((py >> 7) << 8) + ((px >> 7) << 1);
    m.mem.w16(DS, 0xCB90, sq);
    let v = (s.byte() % 24) as u16;
    m.mem.w16(DS, 0x05D6, v);
    let k = s.byte();
    let day = if k < 0x40 {
        if s.byte() < 0x80 { 0xF3 } else { 0x11B }
    } else {
        (s.word() % 360) as u32
    };
    let year = (s.byte() % 30) as u32;
    let minute = (s.word() % 1440) as u32;
    m.mem.w32(DS, 0x05CC, year * 518_400 + day * 1440 + minute);
    let v = s.byte() % 6;
    m.mem.w8(DS, 0x05E6, v);
    let v = if s.byte() < 0x30 { 1 } else { 0 };
    m.mem.w8(DS, 0x1013, v);
    let v = s.byte() % 12;
    m.mem.w8(DS, 0x0D48, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0A50, v);
    let v = if s.byte() < 0x10 {
        0x200
    } else if s.byte() < 0x40 {
        0x40
    } else {
        0
    };
    m.mem.w16(DS, 0x05CA, v);
    let v = if s.byte() < 0x10 { 1 } else { 0 };
    m.mem.w16(DS, 0x0FF7, v);
    let v = if s.byte() < 0x10 { 1 } else { 0 };
    m.mem.w16(DS, 0x0FFB, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 + (s.byte() % 3) as u16 };
    m.mem.w16(DS, 0x0FF9, v);
    let v = s.byte() % 30;
    m.mem.w8(DS, 0x01AA, v);
    for i in 0..8u16 {
        let si = 0x0C57 + 0x1C * i;
        let w = s.word() & 0x3FFF;
        let v = w | if s.byte() < 0xC0 { 0x4000 } else { 0 };
        m.mem.w16(DS, si + 0xF, v);
    }
    for q in 0..4u16 {
        let e = 0x0E6E + 0x34 * q;
        let k = s.byte();
        let stage = if k < 0x30 {
            6
        } else if k < 0x60 {
            4
        } else {
            (s.byte() % 8) as u16
        };
        m.mem.w16(DS, e + 4, 0x40 | stage << 9);
        for f in [0x2Fu16, 0x30] {
            let v = if s.byte() < 0x80 { 0x18 + s.byte() % 8 } else { s.byte() % 0x18 };
            m.mem.w8(DS, e + f, v);
        }
    }
    for dy in -8i32..9 {
        for dx in -8i32..9 {
            let at = ((((py >> 7) as i32 + dy) << 8) + (((px >> 7) as i32 + dx) << 1)) as u16;
            let v = if s.byte() < 0xC0 { 0 } else { s.word() & 0x7FFF };
            m.mem.w16(WSEG, at, v);
        }
    }
    let v = if s.byte() < 0xC0 { 0 } else { 0x0102 * (1 + (s.byte() % 0x40) as u16) };
    m.mem.w16(WSEG, sq, v);
    let v = s.byte() % 0x30;
    m.mem.w8(CSEG, 4, v);
    let v = s.byte() % 8;
    m.mem.w8(CSEG, 5, v);
    let v = s.word() & 0x1000;
    m.mem.w16(CSEG, 0x40D, v);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
    if s.byte() < 0x40 {
        m.mem.w16(DS, 0x05C8, 0x8000);
        let v = if s.byte() < 0x40 { 0 } else { s.byte() % 9 };
        m.mem.w8(DS, 0x0F77, v);
        let v = s.word();
        m.mem.w16(DS, 0xA902, v);
    }
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
    let dir = common::game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    // GLOBAL.BSA's handle, 5, taken as the generator's stand-in has it.
    files.bsa_handle_taken();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/encounter.txt").lines().take(std::env::var("ENCOUNTER_N").map_or(usize::MAX, |v| v.parse().unwrap())) {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "encounter", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m.random_encounter(&mut files, &mut Hooks).map(|()| {
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
        if std::env::var("ENCOUNTER_DUMP").is_ok_and(|v| v == f[1]) {
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
    assert!(counts.get("ran").copied().unwrap_or(0) > 2500);
}
