//! Differential tests of NPC names (src/names.rs) against vectors from
//! the original code (harness/gen_names.py). Needs the game directory
//! (ESarena/, next to ACD.EXE); the vectors hold only inputs and digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const OUT_SEG: u16 = 0x7000;

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

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut base = Machine::new(&img);
    assert!(base.bsa_open(&mut files));
    let mut n = 0;
    for line in include_str!("vectors/names.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<u32> = f[1].split(' ').map(hex).collect();
        let mut m = base.clone();
        let mut fs = files.clone();
        if f[0] == "expand" {
            let (sseg, cseg, rec) = (0x5100u16, 0x4C00u16, p[7] as u16);
            m.mem.w32(sseg, 0, p[0]);
            for (i, v) in [p[1], p[2], p[3], p[4], p[5]].into_iter().enumerate() {
                m.mem.w8(sseg, 4 + i as u16, v as u8);
            }
            m.mem.w32(cseg, 0x91F6, p[6]);
            m.mem.w16(cseg, 0x91F0, 0);
            m.load_rng();
            let before = m.clone();
            let got = m.expand_npc_named(&mut fs, sseg, 0, cseg, rec).map(|()| {
                m.store_rng();
                let record = (0..0x41Eu16).map(|i| m.mem.r8(cseg, rec.wrapping_add(i)));
                let state = (0..0xEu16).map(|i| m.mem.r8(cseg, 0x91EE + i));
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                format!("{:x} {:x} {:x}", fnv(record), fnv(state), fnv(changed))
            });
            assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
            n += 1;
            continue;
        }
        let off = p[3] as u16;
        let before = m.clone();
        let got = m.npc_name(&mut fs, p[0] as u8, p[1] as u8, p[2], OUT_SEG, off).map(|_| {
            let name = (0..64u16).map(|i| m.mem.r8(OUT_SEG, off.wrapping_add(i)));
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(name), fnv(changed))
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("{n} names");
}
