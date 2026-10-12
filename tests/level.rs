//! Differential tests of the .MIF level loader (0000:E87C) and the whole
//! city (1194:0292) against vectors from the original code
//! (harness/gen_level.py). Needs the game directory (ESarena/, next to
//! ACD.EXE); the vectors hold only names, settings and digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const PLANE_VARS: [u16; 4] = [0xCCD0, 0xD806, 0xD748, 0x9030];
const BUFFER_SEG: u16 = 0x8800;
/// The harness's heap (harness/dos.py).
const HEAP: (u16, u16) = (0xA000, 0xF000);
const NAME: u16 = 0x0A36;

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

fn put(m: &mut Machine, off: u16, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        m.mem.w8(DS, off.wrapping_add(i as u16), b);
    }
}

/// Changed DGROUP bytes (offset, value), then the planes (32 KB blocks)
/// and the buffer segment.
fn digests(m: &Machine, before: &Machine) -> String {
    let changed = (0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
    let mut out = format!("{:x}", fnv(changed));
    for (v, n) in PLANE_VARS.into_iter().zip([0x8000u32, 0x8000, 0x8000, 0x10000]) {
        let s = m.mem.r16(DS, v);
        let d = if s == 0 { 0 } else { fnv((0..n).map(|o| m.mem.r8(s, o as u16))) };
        out += &format!(" {d:x}");
    }
    out
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
    base.mem.w16(DS, 0x9030, BUFFER_SEG);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    let vectors = include_str!("vectors/level.txt");
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in vectors.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let mut m = base.clone();
        let mut fs = files.clone();
        let p: Vec<&str> = f[1].split(' ').collect();
        let got = match f[0] {
            "level" => {
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                m.mem.w8(DS, 0xBB0B, hex(p[1]) as u8);
                m.mem.w16(DS, 0xC39A, hex(p[2]) as u16);
                m.mem.w16(DS, 0xCB84, hex(p[3]) as u16);
                m.mem.w16(DS, 0xD748, hex(p[4]) as u16);
                m.mem.w16(DS, 0xCB88, hex(p[5]) as u16);
                m.mem.w16(DS, 0xC398, hex(p[6]) as u16);
                let before = m.clone();
                m.level_load(&mut fs, NAME).map(|()| digests(&m, &before))
            }
            "city" => {
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                let plan: Vec<u8> = (0..p[7].len() / 2).map(|i| hex(&p[7][2 * i..2 * i + 2]) as u8).collect();
                put(&mut m, 0x486B, &plan);
                m.mem.w16(DS, 0x0A48, hex(p[1]) as u16);
                m.mem.w16(DS, 0x0A4A, hex(p[2]) as u16);
                m.mem.w8(DS, 0x0A43, hex(p[3]) as u8);
                m.mem.w8(DS, 0x0A46, hex(p[4]) as u8);
                m.mem.w16(DS, 0x0A44, hex(p[5]) as u16);
                m.mem.w32(DS, 0x91F6, hex(p[6]) as u32);
                m.mem.w16(DS, 0x91F0, 0);
                m.load_rng();
                let before = m.clone();
                m.mif_build_city(&mut fs).map(|()| {
                    m.store_rng();
                    digests(&m, &before)
                })
            }
            "reload" => {
                let size = hex(p[2]) as u16;
                m.mem.w16(DS, 0xCB8E, size);
                let (buf, _) = m.heap_alloc(0x1000).unwrap();
                m.mem.w16(DS, 0x9030, buf);
                let (seg, off) = m.heap_alloc_bytes(size).unwrap();
                m.mem.w16(DS, 0xBA54, off);
                m.mem.w16(DS, 0xBA56, seg);
                m.mem.w8(DS, 0xBA16, 1);
                m.mem.w8(DS, 0xBB0B, hex(p[1]) as u8);
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                let before = m.clone();
                m.level_load(&mut fs, NAME).map(|()| digests(&m, &before))
            }
            _ => panic!("unknown vector {line}"),
        };
        // "hang": the original ran out of the harness's instruction
        // budget, which proves nothing either way; the port only has to
        // return.
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(f[0]).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert_eq!(counts.len(), 3);
}
