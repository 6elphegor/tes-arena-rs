//! Differential tests of the wilderness grid (src/wild.rs) against
//! vectors from the original code (harness/gen_wild.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use std::path::PathBuf;
use arena::mem::DS;

const BUF: u16 = 0x7000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x9030, BUF);
    let dir: PathBuf = std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into());
    let files = Files::new(&dir).ok();
    // The block cases' machine: GLOBAL.BSA open, the game's heap, and the
    // buffers allocated as the harness does.
    let mut dbase = Machine::new(&img);
    dbase.heap_init(0xA000, 0xF000).unwrap();
    let mut dfiles = files.clone();
    if let Some(fs) = dfiles.as_mut() {
        assert!(dbase.bsa_open(fs));
    }
    for (var, paras) in [(0x9030u16, 0x1000u16), (0xCCD0, 0x800), (0xD806, 0x800), (0xD748, 0x800)] {
        let (seg, _) = dbase.heap_alloc(paras).unwrap();
        dbase.mem.w16(DS, var, seg);
    }
    let (seg, off) = dbase.heap_alloc_bytes(0x8000).unwrap();
    dbase.mem.w16(DS, 0xBA54, off);
    dbase.mem.w16(DS, 0xBA56, seg);
    let mut n = 0;
    for line in include_str!("vectors/wild.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f[0] == "block" {
            let Some(mut fs) = dfiles.clone() else { continue };
            let (slot, block) = f[1].split_once(' ').unwrap();
            let mut m = dbase.clone();
            let before = m.clone();
            let got = m
                .wild_load_block(&mut fs, u8::from_str_radix(slot, 16).unwrap(), u8::from_str_radix(block, 16).unwrap())
                .map(|()| {
                    let mut parts: Vec<u64> = [0xCCD0u16, 0xD806, 0xD748]
                        .iter()
                        .map(|&v| {
                            let s = m.mem.r16(DS, v);
                            fnv((0..0x8000u16).map(|o| m.mem.r8(s, o)))
                        })
                        .collect();
                    let buf = m.mem.r16(DS, 0x9030);
                    parts.push(fnv((0..=0xFFFFu16).map(|o| m.mem.r8(buf, o))));
                    let changed = (0..=0xFFFFu16)
                        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                    parts.push(fnv(changed));
                    parts.iter().map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" ")
                });
            assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
            n += 1;
            continue;
        }
        let (seed, template) = f[1].split_once(' ').unwrap();
        let mut m = base.clone();
        m.mem.w32(DS, 0xA22, u32::from_str_radix(seed, 16).unwrap());
        for (i, c) in format!("{template}\0").bytes().enumerate() {
            m.mem.w8(DS, 0xA36 + i as u16, c);
        }
        m.load_rng();
        let before = m.clone();
        let got = m.wild_generate().map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv((0..=0xFFFFu16).map(|o| m.mem.r8(BUF, o))), fnv(changed))
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("{n} grids");
}
