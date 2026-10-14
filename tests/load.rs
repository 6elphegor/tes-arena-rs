//! Differential tests of the rebuild after loading a game (src/load.rs
//! `save_rebuild`, 25FD:03F9..051E) against vectors from the original
//! code (harness/gen_load.py). Slot 1's spells and CITYDATA come from the
//! saved game the vectors were made from: set ARENA_SAVE to its folder,
//! or the test is skipped. The vectors hold only digests.
mod common;

use arena::bsa::Files;
use arena::encounter::FightHooks;
use arena::light::LightHooks;
use arena::load::LoadHooks;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::transit::AreaHooks;
use common::blow::Stream;
use std::path::PathBuf;

const CSEG: u16 = 0x7000;
const EXTRA: (u16, u16) = (0x8800, 0x0010);
const PLAYER: u16 = 0x01A4;
const SLOTS: u16 = 0x0660;
const SLOT_SIZE: u16 = 0x3C;

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The generator's far stubs: each counts its calls in a DGROUP word.
struct Count;

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

impl FightHooks for Count {
    fn fight_graphics(&mut self, m: &mut Machine, _files: &mut Files, people: bool) -> Option<()> {
        count(m, if people { 0xFFE0 } else { 0xFFE2 });
        Some(())
    }
    fn fight_music(&mut self, m: &mut Machine, _music: u8) {
        count(m, 0xFFE4);
    }
}
impl LightHooks for Count {}
impl AreaHooks for Count {
    fn location(&mut self, m: &mut Machine, _id: u16) {
        count(m, 0xFFEE);
    }
    fn border(&mut self, m: &mut Machine) {
        count(m, 0xFFF4);
    }
}
impl LoadHooks for Count {
    fn sky_graphics(&mut self, m: &mut Machine, _files: &mut Files) {
        count(m, 0xFFE6);
    }
    fn mode_3d(&mut self, m: &mut Machine) -> bool {
        count(m, 0xFFE8);
        true
    }
    fn renderer(&mut self, m: &mut Machine) {
        count(m, 0xFFEA);
    }
    fn copy_file(&mut self, m: &mut Machine, _files: &mut Files, _from: u16, _to: u16) {
        count(m, 0xFFEC);
    }
    fn level_automap(&mut self, m: &mut Machine, _files: &mut Files) {
        count(m, 0xFFF0);
    }
    fn interior_text(&mut self, m: &mut Machine) {
        count(m, 0xFFF2);
    }
}

/// Mirrors harness/gen_load.py `setup` (a conditional's test byte comes
/// first, a tuple's items before its index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = if s.byte() < 0xF0 { s.byte() % 5 } else { s.byte() };
    m.mem.w8(DS, 0x05E6, v);
    let v = s.word() & !4;
    let v = v | if s.byte() < 0x40 { 4 } else { 0 };
    m.mem.w16(DS, 0x05C8, v);
    let v = s.byte();
    m.mem.w8(DS, 0x05E4, v);
    let v = s.word();
    m.mem.w16(DS, 0x05E0, v);
    let v = s.word() & 0x7FFB;
    let v = v | if s.byte() < 0x80 { 4 } else { 0 };
    let v = v | if s.byte() < 0x40 { 0x8000 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & !0x200;
    let v = v | if s.byte() < 0x40 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0x80 { 0 } else { s.word() };
    m.mem.w16(DS, 0xCCC2, v);
    let v = s.byte();
    m.mem.w8(DS, 0xCB5C, v);
    let v = if s.byte() < 0xE0 { s.byte() % 9 } else { s.byte() };
    m.mem.w8(DS, 0x0A46, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0A4C, v);
    let v = [1, 65][(s.byte() & 1) as usize];
    m.mem.w8(DS, 0x1A6C, v);
    let v = s.byte();
    m.mem.w8(DS, 0x1A80, v);
    let v = s.word();
    m.mem.w16(DS, 0x0FF9, v);
    let v = s.byte();
    m.mem.w8(DS, 0x6636, v);
    for i in 0..0x41Eu16 {
        let b = s.byte();
        m.mem.w8(CSEG, i, b);
    }
    let v = if s.byte() < 0x20 { 0x2A } else { 1 + s.byte() % 0x29 };
    m.mem.w8(CSEG, 4, v);
    for k in 1..8u16 {
        let v = s.word();
        m.mem.w16(CSEG, 0x41E * k + 0x40D, v);
    }
    for i in 0..8u16 {
        let v = s.word() & 0x3FFF;
        let v = v | if s.byte() < 0xA0 { 0x4000 } else { 0 };
        m.mem.w16(DS, 0x0C57 + 0x1C * i + 0xF, v);
    }
    for i in 0..16u16 {
        let sl = SLOTS + SLOT_SIZE * i;
        let v = if s.byte() < 0x80 { 0xFF } else { s.byte() };
        m.mem.w8(DS, sl + 0x3B, v);
        let v = if s.byte() < 0x80 { PLAYER } else { s.word() };
        m.mem.w16(DS, sl + 0x28, v);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let Ok(save) = std::env::var("ARENA_SAVE") else {
        eprintln!("skipping: ARENA_SAVE not set");
        return;
    };
    files.add_dir(&PathBuf::from(save)).expect("ARENA_SAVE");
    let mut base = Machine::new(&img);
    // The harness's heap (harness/dos.py).
    base.heap_init(0xA000, 0xF000).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0x1020, EXTRA.1);
    base.mem.w16(DS, 0x1022, EXTRA.0);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/load.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "load", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m.save_rebuild(&mut fs, &mut Count).map(|_| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let recs = (0..0x41E * 8u16).map(|o| m.mem.r8(CSEG, o));
            let spells = (0..0x3520u16).map(|o| m.mem.r8(DATA, o));
            let extra = (0..0x80u16).map(|o| m.mem.r8(EXTRA.0, EXTRA.1 + o));
            format!("{:x} {:x} {:x} {:x}", fnv(changed), fnv(recs), fnv(spells), fnv(extra))
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => {
                assert!(f[2] == "hang" || f[2] == "fault", "{line}: the port gave up");
                stuck += 1;
            }
        }
    }
    eprintln!("load: {n} compared, {stuck} stuck in both");
    assert!(n > 2500, "{n} compared");
}
