//! Differential tests of a creature's flat coming alive (src/encounter.rs
//! `flat_creature`, 25CA:0271) against vectors from the original code
//! (harness/gen_flatseen.py).
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
const TSEG: u16 = 0x8000;

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

/// Mirrors harness/gen_flatseen.py `setup`: (x, y, cl, di).
fn setup(m: &mut Machine, seed: u32) -> (u16, u16, u8, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let px = 0x1000 + s.word() % 0x2000;
    let py = 0x1000 + s.word() % 0x2000;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    m.mem.w16(DS, 0xCB90, ((py >> 7) << 8) + ((px >> 7) << 1));
    let v = s.byte() % 6;
    m.mem.w8(DS, 0x05E6, v);
    let v = s.word() & 0x0640;
    m.mem.w16(DS, 0x05CA, v);
    let v = if s.byte() < 0x40 { 0x1000 } else { 0 };
    m.mem.w16(DS, 0x05C8, v);
    let v = if s.byte() < 0x20 { 1 } else { 0 };
    m.mem.w16(DS, 0x0FF7, v);
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
    for dy in -8i32..9 {
        for dx in -8i32..9 {
            let at = ((((py >> 7) as i32 + dy) << 8) + (((px >> 7) as i32 + dx) << 1)) as u16;
            let v = if s.byte() < 0xC0 { 0 } else { s.word() & 0x7FFF };
            m.mem.w16(WSEG, at, v);
        }
    }
    let v = s.byte() % 0x30;
    m.mem.w8(CSEG, 4, v);
    let v = s.byte() % 8;
    m.mem.w8(CSEG, 5, v);
    let v = s.word() & 0x1000;
    m.mem.w16(CSEG, 0x40D, v);
    let mut flats: Vec<u8> = (0..0x17).map(|_| 1 + s.byte() % 0x7F).collect();
    flats.resize(0x40, 0);
    put(m, DS, 0xC954, &flats);
    let col = (px >> 7) + (s.byte() % 15) as u16 - 7;
    let row = (py >> 7) + (s.byte() % 15) as u16 - 7;
    let (x, y) = (col << 7 | 0x40, row << 7 | 0x40);
    let di = (row << 8).wrapping_add(col << 1);
    let number = if s.byte() < 0xA0 { flats[(s.byte() % 0x17) as usize] } else { s.byte() | 0x80 };
    m.mem.w16(WSEG, di, 0x8200 | number as u16);
    let cl = if s.byte() < 0x20 { s.byte() | 0x80 } else { number };
    let n = s.byte() % 6;
    m.mem.w8(DS, 0xC866, n);
    for i in 0..n as u16 {
        let (c, r) = if s.byte() < 0x80 { (col as u8, row as u8) } else { (s.byte() % 0x40, s.byte() % 0x40) };
        let a = if s.byte() < 0x20 { 0xFF } else { s.byte() % 3 };
        let b = if s.byte() < 0x20 { 0xFF } else { s.byte() };
        put(m, DS, 0xC766 + 4 * i, &[c, r, a, b]);
    }
    for i in 0..8u16 {
        let v = if s.byte() < 0x80 { 0xFFFF } else { s.word() };
        m.mem.w16(DS, 0x0622 + 6 * i, v);
    }
    let toff = s.word() & 0x0FFF;
    m.mem.w16(DS, 0xCB88, toff);
    let mut t = Vec::new();
    for _ in 0..3 {
        if s.byte() < 0xA0 {
            let v = if s.byte() < 0x40 { 0x7B } else { s.byte() };
            t.extend(format!("+{v}\rITEM").bytes());
        } else {
            t.extend(b"TEXT\rL");
        }
        t.push(0);
    }
    put(m, TSEG, toff, &t);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
    (x, y, cl, di)
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
        (0xCB8A, TSEG),
    ] {
        base.mem.w16(DS, o, v);
    }
    base.mem.w8(DS, 0x8FA6, 0);
    base.heap_init(0xA000, 0xF000).unwrap();
    let dir = common::game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    // GLOBAL.BSA's handle, 5, taken as the generator's stand-in has it.
    files.bsa_handle_taken();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/flatseen.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "flatseen", "{line}");
        let mut m = base.clone();
        let (x, y, cl, di) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m.flat_creature(&mut files, &mut Hooks, x, y, cl, WSEG, di).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let heap = (0xA000u16..0xF000).flat_map(|seg| (0..16u16).map(move |o| (seg, o)));
            format!(
                "{:x} {:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..=0xFFFFu16).map(|o| m.mem.r8(WSEG, o))),
                fnv(heap.map(|(seg, o)| m.mem.r8(seg, o)))
            )
        });
        if std::env::var("FLATSEEN_DUMP").is_ok_and(|v| v == f[1]) {
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
        let alive = m.mem.r16(DS, 0x05C8) & 0x1000 != 0 && before.mem.r16(DS, 0x05C8) & 0x1000 == 0;
        *counts.entry(if f[2] == "fault" { "fault" } else if alive { "alive" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("alive").copied().unwrap_or(0) > 300);
}
