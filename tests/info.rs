//! Differential tests of the level .INF loader (0000:EC03) against
//! vectors from the original code (harness/gen_info.py): each case loads
//! a level with EMS on, then its INF; stairs cases then take a step into
//! a stair square, changing level (0000:EBB5) and loading the new INF;
//! ceiling cases add a ceiling plane, which no level with an INF has. Needs the game directory (ESarena/,
//! next to ACD.EXE); the vectors hold only names, settings and digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const FRAME: u16 = 0x8800;
const SHADES: u16 = 0x7000;
const NAME: u16 = 0x0A36;
const INFO: u16 = 0xE2BE;
const BLOCK: u16 = 0x3E80;
/// The ceiling plane of the ceiling cases.
const CEILING: u16 = 0x8000;
/// The harness's heap (harness/dos.py).
const HEAP: (u16, u16) = (0xA000, 0xF000);

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

fn bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| hex(&s[2 * i..2 * i + 2]) as u8).collect()
}

fn put(m: &mut Machine, off: u16, b: &[u8]) {
    for (i, &c) in b.iter().enumerate() {
        m.mem.w8(DS, off.wrapping_add(i as u16), c);
    }
}

fn digests(m: &Machine, before: &Machine) -> String {
    let changed = (0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
    let banks = (0..0x20).flat_map(|b| m.ems_bank(b));
    let (seg, off) = (m.mem.r16(DS, 0xBA56), m.mem.r16(DS, 0xBA54));
    let block = (0..BLOCK).map(|i| m.mem.r8(seg, off.wrapping_add(i)));
    format!("{:x} {:x} {:x}", fnv(changed), fnv(banks), fnv(block))
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
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (seg, off) = base.heap_alloc_bytes(BLOCK).unwrap();
    base.mem.w16(DS, 0xBA54, off);
    base.mem.w16(DS, 0xBA56, seg);
    base.mem.w16(DS, 0xCB8A, FRAME);
    for i in 0..0x1000u16 {
        base.mem.w8(SHADES, i, (i.wrapping_mul(7).wrapping_add(i >> 8)) as u8);
    }
    let vectors = include_str!("vectors/info.txt");
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let mut changes = 0;
    for line in vectors.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<&str> = f[1].split(' ').collect();
        let mut m = base.clone();
        let mut fs = files.clone();
        let got = match f[0] {
            "info" => {
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                m.mem.w8(DS, 0xBB0B, hex(p[1]) as u8);
                m.mem.w16(DS, 0xC39A, hex(p[2]) as u16);
                m.mem.w16(DS, 0xCB84, 0);
                m.mem.w16(DS, 0xCB88, 0x8000);
                m.mem.w8(DS, 0xC8EF, hex(p[3]) as u8);
                m.mem.w16(DS, 0xCCC2, hex(p[4]) as u16);
                m.mem.w8(DS, 0xC3C6, hex(p[5]) as u8);
                m.mem.w16(DS, 0xC38E, hex(p[6]) as u16);
                put(&mut m, 0xCB49, &bytes(p[7]));
                put(&mut m, 0xCB39, &bytes(p[8]));
                m.level_load(&mut fs, NAME).expect(line);
                assert_eq!(m.mem.r16(DS, 0xCB84), INFO, "{line}");
                if p[9] != "-" {
                    put(&mut m, INFO, format!("{}\0", p[9]).as_bytes());
                }
                let before = m.clone();
                m.info_load(&mut fs, INFO).map(|()| digests(&m, &before))
            }
            "stairs" => {
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                m.mem.w8(DS, 0xBB0B, 0xFF);
                m.mem.w16(DS, 0xC39A, 0x400);
                m.mem.w16(DS, 0xCB84, 0);
                m.mem.w16(DS, 0xCB88, 0x8000);
                m.mem.w8(DS, 0xC8EF, 0x80);
                m.level_load(&mut fs, NAME).expect(line);
                m.info_load(&mut fs, INFO).expect(line);
                let (x, y) = (hex(p[1]) as u16, hex(p[2]) as u16);
                let heading = hex(p[3]) as u16;
                m.mem.w16(DS, 0xC3D0, x);
                m.mem.w16(DS, 0xC3D4, y);
                m.mem.w16(DS, 0xC3D2, 0x3C);
                m.mem.w16(DS, 0xC3DA, heading);
                m.mem.w16(DS, 0xCB90, (y >> 7 << 8) + (x >> 7 << 1));
                m.mem.w8(DS, 0xE3A1, 0);
                put(&mut m, 0xC37F, format!("{}\0", p[0]).as_bytes());
                let before = m.clone();
                let level = m.mem.r8(DS, 0xBB0A);
                let got = m.move_step(&mut fs, hex(p[4]) as u16, heading);
                changes += (m.mem.r8(DS, 0xBB0A) != level) as usize;
                got.map(|_| {
                    let planes = [0xCCD0u16, 0xD806, 0xD748].map(|v| {
                        let s = m.mem.r16(DS, v);
                        if s == 0 { 0 } else { fnv((0..0x8000u16).map(|o| m.mem.r8(s, o))) }
                    });
                    format!(
                        "{} {:x} {:x} {:x} {:x} {:x}",
                        digests(&m, &before),
                        m.mem.r16(DS, 0xC3D0),
                        m.mem.r16(DS, 0xC3D4),
                        planes[0],
                        planes[1],
                        planes[2]
                    )
                })
            }
            "ceiling" => {
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                m.mem.w8(DS, 0xBB0B, 0xFF);
                m.mem.w16(DS, 0xC39A, hex(p[2]) as u16);
                m.mem.w16(DS, 0xCB84, 0);
                m.mem.w16(DS, 0xCB88, 0x8000);
                m.mem.w8(DS, 0xC8EF, 0x80);
                m.level_load(&mut fs, NAME).expect(line);
                for i in 0..0x4000u16 {
                    m.mem.w16(CEILING, 2 * i, i.wrapping_mul(0x9E37) ^ (i >> 5));
                }
                m.mem.w16(DS, 0xD748, CEILING);
                let f = m.mem.r16(DS, 0xC398) | 4;
                m.mem.w16(DS, 0xC398, f);
                if p[1] != "-" {
                    put(&mut m, INFO, format!("{}\0", p[1]).as_bytes());
                }
                let before = m.clone();
                m.info_load(&mut fs, INFO).map(|()| digests(&m, &before))
            }
            _ => panic!("unknown vector {line}"),
        };
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(f[0]).or_default() += 1;
    }
    eprintln!("{counts:?}, {changes} level changes");
    assert_eq!(counts.len(), 3);
    assert!(changes > 10);
}
