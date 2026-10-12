//! Differential tests of saved-game loading (src/save.rs) against vectors
//! from the original code (harness/gen_save.py). The engine cases need
//! the saved game the vectors were made from: set ARENA_SAVE to its
//! folder, or they are skipped. The vectors hold only digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const CREATURES: u16 = 0x7000;
const BUFFER: u16 = 0x7400;
const EXTRA: (u16, u16) = (0x8800, 0x0010);
const LOAD: u16 = 0x7800;

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

fn lcg_bytes(n: u32, seed: u32) -> Vec<u8> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            (x >> 16) as u8
        })
        .collect()
}

fn changed(m: &Machine, before: &Machine) -> u64 {
    fnv((0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]))
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let save = std::env::var("ARENA_SAVE").ok().map(PathBuf::from);
    if let Some(s) = &save {
        files.add_dir(s).expect("ARENA_SAVE");
    }
    let mut base = Machine::new(&img);
    assert!(base.bsa_open(&mut files));
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/save.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<u32> = f[1].split(' ').map(|x| u32::from_str_radix(x, 16).unwrap_or(0)).collect();
        let mut m = base.clone();
        let got = match f[0] {
            "decrypt" => {
                for (i, b) in lcg_bytes(p[0], p[1]).into_iter().enumerate() {
                    m.mem.w8(DS, 0xA000 + i as u16, b);
                }
                m.save_decrypt(DS, 0xA000, p[0] as u16);
                format!("{:x}", fnv((0..p[0] as u16).map(|i| m.mem.r8(DS, 0xA000 + i))))
            }
            "path" => {
                m.mem.w8(DS, 0x1AEA, p[0] as u8);
                let before = m.clone();
                let si = m.save_path(p[1] as u16);
                format!("{:x} {si:x}", changed(&m, &before))
            }
            "engine" => {
                if save.is_none() {
                    eprintln!("skipping engine case: ARENA_SAVE not set");
                    continue;
                }
                m.mem.w16(DS, 0x1042, CREATURES);
                m.mem.w16(DS, 0x9030, BUFFER);
                m.mem.w16(DS, 0x1020, EXTRA.1);
                m.mem.w16(DS, 0x1022, EXTRA.0);
                let before = m.clone();
                let mut fs = files.clone();
                match m.save_load_engine(&mut fs, p[0] as u8) {
                    Some(()) => {
                        let blocks = [
                            fnv((0..0x20F0u16).map(|i| m.mem.r8(CREATURES, i))),
                            fnv((0..0x15E0u16).map(|i| m.mem.r8(BUFFER, 0xA000 + i))),
                            fnv((0..0x120u16).map(|i| m.mem.r8(EXTRA.0, EXTRA.1 + i))),
                        ];
                        format!("{:x} {:x} {:x} {:x}", changed(&m, &before), blocks[0], blocks[1], blocks[2])
                    }
                    None => "fault".into(),
                }
            }
            "load" => {
                let (name, off) = f[1].split_once(' ').unwrap();
                let off = hex(off) as u16;
                for (i, c) in format!("{name}\0").bytes().enumerate() {
                    m.mem.w8(DS, 0xF000 + i as u16, c);
                }
                let before = m.clone();
                let mut fs = files.clone();
                match m.file_load(&mut fs, 0xF000, LOAD, off) {
                    Some((_, ax)) => format!(
                        "{ax:x} {:x} {:x}",
                        changed(&m, &before),
                        fnv((0..=0xFFFFu16).map(|i| m.mem.r8(LOAD, i)))
                    ),
                    None => "fault".into(),
                }
            }
            "spells" => {
                if save.is_none() {
                    eprintln!("skipping spells case: ARENA_SAVE not set");
                    continue;
                }
                m.mem.w8(DS, 0x1A6C, p[0] as u8);
                let before = m.clone();
                let mut fs = files.clone();
                match m.spells_load(&mut fs) {
                    Some(()) => format!(
                        "{:x} {:x}",
                        changed(&m, &before),
                        fnv((0..0x3520u16).map(|i| m.mem.r8(0x3276, i)))
                    ),
                    None => "fault".into(),
                }
            }
            _ => panic!("unknown vector {line}"),
        };
        assert_eq!(f[2], got, "{line}");
        *counts.entry(f[0]).or_default() += 1;
    }
    eprintln!("{counts:?}");
}
