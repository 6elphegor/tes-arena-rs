//! Differential tests of whole dungeon levels (dungeon_create and
//! dungeon_build_level, with their file I/O) against vectors from the
//! original code (harness/gen_dungeon_map.py). Needs the game directory
//! (ESarena/, next to ACD.EXE); the vectors hold only settings and
//! digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

/// The harness's heap (harness/dos.py) and chunk block size.
const HEAP: (u16, u16) = (0xA000, 0xF000);
const CHUNK_BLOCK: u16 = 0x3E80;

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

/// Changed DGROUP bytes (offset, value), the floor and map planes, the
/// buffer and the chunk block.
fn digests(m: &Machine, before: &Machine) -> String {
    let changed = (0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
    let mut parts = vec![fnv(changed)];
    for (var, n) in [(0xCCD0, 0x8000u32), (0xD806, 0x8000), (0x9030, 0x10000), (0xBA56, CHUNK_BLOCK as u32)] {
        let s = m.mem.r16(DS, var);
        parts.push(if s == 0 { 0 } else { fnv((0..n).map(|o| m.mem.r8(s, o as u16))) });
    }
    parts.iter().map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" ")
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    let mut n = 0;
    for line in include_str!("vectors/dungeon_map.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<u32> = f[1].split(' ').map(hex).collect();
        let mut m = base.clone();
        let mut fs = files.clone();
        let (buf, _) = m.heap_alloc(0x1000).unwrap();
        m.mem.w16(DS, 0x9030, buf);
        let (bs, bo) = m.heap_alloc_bytes(CHUNK_BLOCK).unwrap();
        m.mem.w16(DS, 0xBA54, bo);
        m.mem.w16(DS, 0xBA56, bs);
        m.mem.w32(DS, 0x91F6, p[4]);
        m.mem.w16(DS, 0x91F0, 0);
        m.mem.w16(DS, 0xC398, p[5] as u16);
        m.mem.w16(DS, 0xC39A, p[6] as u16);
        m.load_rng();
        let before = m.clone();
        let got = m
            .dungeon_create(&mut fs, p[0] as u16, p[1] as u16, p[2] as u16)
            .and_then(|()| m.dungeon_build_level(&mut fs, p[3] as u8))
            .map(|()| {
                m.store_rng();
                digests(&m, &before)
            });
        match got {
            // The original ran out of the harness's instruction budget,
            // which proves nothing; the port only has to return.
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        n += 1;
    }
    eprintln!("{n} dungeon levels");
}
