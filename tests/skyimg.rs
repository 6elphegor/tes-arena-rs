//! Differential tests of the sky's images loaded on entering an area
//! outdoors (src/skyobj.rs `sky_images`, 0000:E5C9, with the star field
//! 0000:E77A) against vectors from the original code
//! (harness/gen_skyimg.py). Needs the game directory (ESarena/, next to
//! ACD.EXE); the vectors hold only seeds and digests.
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

/// Mirrors harness/gen_skyimg.py `setup` (a conditional's test byte
/// first).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let flags = s.word() & !4;
    let flags = flags | if s.byte() < 0xE0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, flags);
    let k = s.byte();
    let bank = if k < 0xC0 {
        0x0E
    } else if k < 0xF0 {
        s.byte() % 0x20
    } else {
        0x80 | s.byte()
    };
    m.mem.w8(DS, 0xC8F3, bank);
    let dword = |s: &mut Stream| s.word() as u32 | (s.word() as u32) << 16;
    let v = dword(s);
    m.mem.w32(DS, 0xCB6E, v);
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    for i in 0..3u16 {
        let b = s.byte() & 1;
        m.mem.w8(DS, 0xD89F + i, b);
    }
    m.load_rng();
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
    for line in include_str!("vectors/skyimg.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "skyimg", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        match m.sky_images(&mut fs) {
            Some(()) => {
                m.store_rng();
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
    eprintln!("skyimg: {n} compared, {stuck} stuck in both");
    assert!(n > 800, "{n} compared");
}
