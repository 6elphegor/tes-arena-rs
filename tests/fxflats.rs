//! Differential tests of the effects' flats loaded at the program's start
//! (src/info.rs `effect_flats`, 21EE:0134) against vectors from the
//! original code (harness/gen_fxflats.py). Needs the game directory
//! (ESarena/, next to ACD.EXE); the vectors hold only seeds and digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FRAME: u16 = 0x8800;
/// The harness's heap (harness/dos.py).
const HEAP: (u16, u16) = (0xA000, 0xF000);

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_fxflats.py `setup` (a conditional's test byte
/// first; a tuple's items, all drawn, before its index is used).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let first = if s.byte() < 0xC0 { 0x0F } else { s.byte() % 0x1E };
    let n = if s.byte() < 0xC0 { 2 } else { 1 + s.byte() % 3 };
    m.mem.w8(DS, 0xC8FA, first);
    m.mem.w8(DS, 0xC8FB, n);
    for i in 0..0x20u16 {
        let k = s.byte() % 8;
        let w = s.word();
        let v = [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0, w, 0x8000][k as usize];
        m.mem.w16(DS, 0x8FAA + 2 * i, v);
    }
    let v = s.word() % 0x40;
    m.mem.w16(DS, 0xD432, v);
    for i in 0..0xA0u16 {
        let b = s.byte();
        m.mem.w8(DS, 0xCA97 + i, b);
    }
    let v = 0xCA97 + s.word() % 0x60;
    m.mem.w16(DS, 0xCCC0, v);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = common::game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/fxflats.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "fxflats", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        match m.effect_flats(&mut fs) {
            Some(()) => {
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                let banks = (0..0x20).flat_map(|b| m.ems_bank(b));
                assert_eq!(f[2], format!("{:x} {:x}", fnv(changed), fnv(banks)), "{line}");
                n += 1;
            }
            None => {
                assert!(f[2] == "fault" || f[2] == "hang", "{line}: the port gave up");
                stuck += 1;
            }
        }
    }
    eprintln!("fxflats: {n} compared, {stuck} stuck in both");
    assert!(n > 150, "{n} compared");
}
