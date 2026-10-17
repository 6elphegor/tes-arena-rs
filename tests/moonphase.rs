//! Differential tests of a moon's phase made (src/celestial.rs
//! `moon_phase_load`, 0000:E6CB) against vectors from the original code
//! (harness/gen_moonphase.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FRAME: u16 = 0x8800;
const BUF: u16 = 0x7000;
const HEAP: (u16, u16) = (0xA000, 0xF000);

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn seg_bytes(m: &Machine, seg: u16) -> Vec<u8> {
    (0..=0xFFFFu16).map(|o| m.mem.r8(seg, o)).collect()
}

fn changes(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        if a != b {
            out.extend([i as u8, (i >> 8) as u8, *b]);
        }
    }
    out
}

/// SI, CX, DI and the frame's fill. Mirrors gen_moonphase.py's `setup`.
fn setup(seed: u32) -> (u16, u16, u16, u8) {
    let mut s = Stream(seed);
    let si = if s.byte() & 1 == 0 { 0xD871 } else { 0xD87C };
    let k = s.byte();
    let cx = if k < 0xE0 { (s.byte() % 0x20) as u16 } else { s.byte() as u16 };
    let k = s.byte();
    let di = if k < 0xE0 { s.word() % 0xF000 } else { s.word() };
    let fill = s.byte();
    (si, cx, di, fill)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: the game directory not found");
        return;
    };
    let mut files = files;
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    base.mem.w16(DS, 0xBA54, 0);
    base.mem.w16(DS, 0xBA56, BUF);
    let mut n = 0;
    for line in include_str!("vectors/moonphase.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let (si, cx, di, fill) = setup(seed);
        let mut m = base.clone();
        for o in 0..=0xFFFFu16 {
            m.mem.w8(FRAME, o, fill);
        }
        let mut fs = files.clone();
        let ds = seg_bytes(&m, DS);
        let got = match m.moon_phase_load(&mut fs, si, cx, di) {
            Some(()) => format!("{:x} {:x}", fnv(seg_bytes(&m, FRAME)), fnv(changes(&ds, &seg_bytes(&m, DS)))),
            None => format!("gave up: {:?}", arena::giveup::take()),
        };
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 0);
}
