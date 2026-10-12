//! Differential tests of movement and collision (src/movement.rs)
//! against vectors from the original code (harness/gen_move.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;

const WALLS: u16 = 0x7000;
const FLOOR: u16 = 0x7800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn lcg(seed: u32, n: usize) -> impl Iterator<Item = u32> {
    let mut x = seed;
    (0..n).map(move |_| {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        x
    })
}

/// Mirrors harness/gen_move.py `plane_words`.
fn plane_words(seed: u32) -> Vec<u16> {
    lcg(seed, 0x4000)
        .map(|x| {
            let r = x >> 8;
            let t = r % 10;
            let v = ((r >> 4) & 0xFFFF) as u16;
            match t {
                0..=2 => 0,
                3 => (v & 0x7F) * 0x101,
                4 => v & 0x7FFF,
                _ => ((0x8 + (t - 5) % 6) as u16) << 12 | (v & 0x0FFF),
            }
        })
        .collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WALLS);
    base.mem.w16(DS, 0xCCD0, FLOOR);
    let mut n = 0;
    for line in include_str!("vectors/movement.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<&str> = f[1].split(' ').collect();
        let mut m = base.clone();
        let seed = hex(p[2]);
        for (i, w) in plane_words(seed).into_iter().enumerate() {
            m.mem.w16(WALLS, (i * 2) as u16, w);
        }
        for (i, w) in plane_words(seed ^ 0x5A5A_5A5A).into_iter().enumerate() {
            m.mem.w16(FLOOR, (i * 2) as u16, w);
        }
        for (i, x) in lcg(seed ^ 0x3333_3333, 0x200).enumerate() {
            m.mem.w8(DS, 0xCDA1 + i as u16, (x >> 16) as u8);
        }
        let step = f[0] == "step";
        let kvs = if step { p[5] } else { p[3] };
        for kv in kvs.split(',') {
            let (k, v) = kv.split_once('=').unwrap();
            let k = hex(k) as u16;
            if k == 0xC917 {
                for i in 0..16 {
                    m.mem.w8(DS, k + i as u16, hex(&v[2 * i..2 * i + 2]) as u8);
                }
            } else if matches!(k, 0xE3A1 | 0xC932 | 0xC933) {
                m.mem.w8(DS, k, hex(v) as u8);
            } else {
                m.mem.w16(DS, k, hex(v) as u16);
            }
        }
        if step {
            m.mem.w16(DS, 0xC3D0, hex(p[0]) as u16);
            m.mem.w16(DS, 0xC3D4, hex(p[1]) as u16);
            let before = m.clone();
            // No files: the cases never reach a level change.
            let mut files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
            let got = m.move_step(&mut files, hex(p[3]) as u16, hex(p[4]) as u16).map(|_| {
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                format!("{:x} {:x} {:x}", m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4), fnv(changed))
            });
            assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
            n += 1;
            continue;
        }
        m.mem.w16(DS, 0xF000, hex(p[0]) as u16);
        m.mem.w16(DS, 0xF002, hex(p[1]) as u16);
        let got = m.move_cell(0xF000).map(|al| {
            let mask = (0..0x100u16).map(|i| m.mem.r8(DS, 0x929E + i));
            let trig = [m.mem.r8(DS, 0xE8C2), m.mem.r8(DS, 0xE8C3)];
            let walls = (0..0x8000u16).map(|i| m.mem.r8(WALLS, i));
            format!("{al:x} {:x} {:x} {:x} {:x}", fnv(mask), m.mem.r8(DS, 0xE3A0), fnv(trig), fnv(walls))
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("{n} cells");
}
