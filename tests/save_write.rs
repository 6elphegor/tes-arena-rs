//! Differential tests of saving a game (src/save_write.rs) against
//! vectors from the original code (harness/gen_savew.py). They need the
//! saved game the vectors were made from: set ARENA_SAVE to its folder,
//! or they are skipped. The vectors hold only digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::save_write::SaveHooks;
use common::blow::Stream;
use std::path::PathBuf;

const CREATURES: u16 = 0x7000;
const BUFFER: u16 = 0x7400;
const EXTRA: (u16, u16) = (0x8800, 0x0010);
const SCREEN: u16 = 0x6000;
const PLANES: [u16; 3] = [0xD806, 0xCCD0, 0xD748];

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_savew.py `setup`; returns whether EMS goes on and
/// the slot.
fn setup(m: &mut Machine, seed: u32) -> (bool, u8) {
    let mut st = Stream(seed);
    let s = &mut st;
    let ems = s.byte() & 1 != 0;
    let slot = s.byte() % 10;
    m.mem.w8(DS, 0x1A6C, slot);
    let t = [0, 0xFF, 1][s.byte() as usize % 3];
    m.mem.w8(DS, 0x1AEA, t);
    for (base, len, count) in [(0x01A4u16, 0x0E4Fu16, 16u8), (0xBB08, 0x0DE7, 16), (0x9686, 0x01E0, 8)] {
        for _ in 0..s.byte() % count {
            let at = base + s.word() % len;
            let b = s.byte();
            m.mem.w8(DS, at, b);
        }
    }
    if s.byte() < 0x80 {
        for i in 0..6 {
            let b = s.byte();
            m.mem.w8(DS, 0xBA62 + i, b);
        }
    }
    for _ in 0..s.byte() % 8 {
        let plane = s.byte() % 3;
        let at = s.word() & 0x7FFF;
        let b = s.byte();
        let seg = m.mem.r16(DS, PLANES[plane as usize]);
        m.mem.w8(seg, at, b);
    }
    (ems, slot)
}

/// The files as gen_savew.py's DOS layer records them.
#[derive(Default)]
struct Disk {
    written: Vec<String>,
    deleted: Vec<String>,
}

fn name(m: &Machine, path: u16) -> String {
    let p = String::from_utf8_lossy(m.mem.cstr(DS, path)).replace(':', "\\");
    p.rsplit('\\').next().unwrap().to_uppercase()
}

impl SaveHooks for Disk {
    fn delete(&mut self, m: &Machine, path: u16) {
        self.deleted.push(name(m, path));
    }
    fn write(&mut self, m: &Machine, path: u16, data: &[u8]) {
        self.written.push(format!("{} {:x}", name(m, path), fnv(data.iter().copied())));
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
    // The saved game, loaded as gen_savew.py loads it (slot 1).
    let mut base = Machine::new(&img);
    base.heap_init(0xA000, 0xF000).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x1042, CREATURES);
    base.mem.w16(DS, 0x9030, BUFFER);
    base.mem.w16(DS, 0x1020, EXTRA.1);
    base.mem.w16(DS, 0x1022, EXTRA.0);
    base.save_load_engine(&mut files, 1).expect("SAVEENGN.01");
    base.mem.w16(DS, 0xA8FE, 0);
    base.mem.w16(DS, 0xA900, SCREEN);
    for var in PLANES {
        let (sg, _) = base.heap_alloc_bytes(0x8000).unwrap();
        base.mem.w16(DS, var, sg);
    }
    base.save_load_level(&mut files).expect("SAVEGAME.01");
    let mut n = 0;
    for line in include_str!("vectors/save_write.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "savew", "{line}");
        let mut m = base.clone();
        let (ems, _slot) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        if ems {
            m.mem.w8(DS, 0x8FA6, 1);
            m.mem.w16(DS, 0x9032, 0x20);
            m.mem.w16(DS, 0x8FA8, 0);
        }
        let before = m.clone();
        let mut disk = Disk::default();
        m.save_game(&mut disk);
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let mut parts = disk.written.clone();
        parts.push(format!("del {}", disk.deleted.join(" ")));
        parts.push(format!("{:x}", fnv(changed)));
        assert_eq!(f[2], parts.join(" / "), "{line}");
        n += 1;
    }
    assert!(n >= 50, "{n} compared");
}
