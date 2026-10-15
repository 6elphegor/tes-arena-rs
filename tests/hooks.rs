//! Differential tests of the 3D mode's hooks into segment 0000
//! (src/hooks.rs, and the walk's register flow in src/movement.rs)
//! against vectors from the original code (harness/gen_hooks.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;
use common::level::*;

fn lcg(seed: u32, n: usize) -> impl Iterator<Item = u32> {
    let mut x = seed;
    (0..n).map(move |_| {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        x
    })
}

/// As tests/movement.rs (harness/gen_move.py `plane_words`).
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

/// Mirrors harness/gen_hooks.py `setup_walk`; returns (speed, angle).
fn setup_walk(m: &mut Machine, seed: u32) -> (u16, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let x = if s.byte() < 0xE0 { 0x100 + s.word() % 0x7D00 } else { s.word() % 0x200 };
    let y = if s.byte() < 0xE0 { 0x100 + s.word() % 0x7D00 } else { s.word() % 0x200 };
    w16(m, DS, 0xC3D0, x);
    w16(m, DS, 0xC3D4, y);
    let k = s.byte() % 6;
    let h = if k == 5 { s.byte() as u16 } else { [0x3C, 0x3C, 0x20, 0x36, 0x50][k as usize] };
    w16(m, DS, 0xC3D2, h);
    let v = s.word() & 0x4702;
    w16(m, DS, 0xC39A, v);
    let v = s.word() & 0x0085;
    w16(m, DS, 0xC398, v);
    let k = s.byte() % 3;
    let v = if k == 2 { s.word() } else { [0, 0x100][k as usize] };
    w16(m, DS, 0xCCCE, v);
    let v = if s.byte() < 0x80 { 0x96 } else { s.word() & 0x1FF };
    w16(m, DS, 0xD80C, v);
    let v = if s.byte() < 0x80 { 0x30 } else { (s.byte() & 0x7F) as u16 };
    w16(m, DS, 0xE8C0, v);
    let v = (s.byte() & 0xF) as u16;
    w16(m, DS, 0xC38E, v);
    let v = s.word() & 0x1FF;
    w16(m, DS, 0xC3DA, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC932, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC933, v);
    w16(m, DS, 0xCB90, ((y >> 7) << 8).wrapping_add((x >> 7) << 1));
    for i in 0..16 {
        let v = s.byte();
        m.mem.w8(DS, 0xC917 + i, v);
    }
    for i in 0..8u16 {
        let k = s.byte() % 5;
        let v = match k {
            0 => 0,
            1 => {
                let (dx, dy) = (s.byte() % 3, s.byte() % 3);
                let row = ((y >> 7) + dy as u16).wrapping_sub(1) & 0xFF;
                let col = ((x >> 7) + dx as u16).wrapping_sub(1).wrapping_mul(2) & 0xFF;
                row << 8 | col
            }
            2 => {
                let r = (s.byte() % 4) as u16;
                (r << 8) | ((s.byte() % 2) as u16 * 2)
            }
            3 => {
                if s.byte() < 0x80 {
                    0
                } else {
                    s.word()
                }
            }
            _ => s.word(),
        };
        w16(m, DS, 0x196F + 2 * i, v);
    }
    let v = s.word();
    w16(m, DS, 0x8F94, v);
    let v = s.byte() as u16;
    w16(m, DS, 0x8F78, v);
    let v = if s.byte() < 0xC0 { 0xC7 } else { s.word() };
    w16(m, DS, 0x8F7A, v);
    let k = s.byte() % 5;
    let speed = if k == 4 { s.word() & 0x1FF } else { [0x10, 0x20, 0x40, 0x80][k as usize] };
    let angle = s.word() & 0x1FF;
    (speed, angle)
}

fn setup_stairs(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    for at in [0xC8E7, 0xC8E9, 0xC8EB, 0xCCC2, 0x3C8B] {
        let v = s.word();
        w16(m, DS, at, v);
    }
    for at in [0xC3C6, 0xC3C7, 0x0F81, 0x0F82, 0x3C82] {
        let v = s.byte();
        m.mem.w8(DS, at, v);
    }
}

fn setup_infdone(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let n = s.byte() % 40;
    flats(m, s, n);
    for _ in 0..s.byte() % 8 {
        let r = (s.byte() % 0x40) as u16;
        let c = (s.byte() % 0x40) as u16;
        let v = s.word();
        w16(m, WSEG, r << 8 | c << 1, v);
    }
    for i in 0..0x32 {
        let v = s.byte() % 0x20;
        m.mem.w8(DS, 0xC954 + i, v);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0xCCD0, FSEG);
    let mut files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut n = std::collections::BTreeMap::new();
    for line in include_str!("vectors/hooks.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "hooks", "{line}");
        let seed = u32::from_str_radix(f[2], 16).unwrap();
        let mut m = base.clone();
        let got = match f[1] {
            "walk" => {
                for (i, w) in plane_words(seed).into_iter().enumerate() {
                    m.mem.w16(WSEG, (i * 2) as u16, w);
                }
                for (i, w) in plane_words(seed ^ 0x5A5A_5A5A).into_iter().enumerate() {
                    m.mem.w16(FSEG, (i * 2) as u16, w);
                }
                for (i, x) in lcg(seed ^ 0x3333_3333, 0x200).enumerate() {
                    m.mem.w8(DS, 0xCDA1 + i as u16, (x >> 16) as u8);
                }
                m.mem.w32(DS, 0xCD6A, arena::hooks::value::WALK_TEST);
                let (speed, angle) = setup_walk(&mut m, seed);
                let before = m.clone();
                m.move_step(&mut files, &mut Counted { level: false }, speed, angle).map(|_| {
                    format!("{:x} {:x} {:x}", m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4), changed(&m, &before))
                })
            }
            op => {
                match op {
                    "stairs" => setup_stairs(&mut m, seed),
                    "level" => setup_level(&mut m, seed),
                    "infdone" => setup_infdone(&mut m, seed),
                    _ => panic!("{line}"),
                }
                let before = m.clone();
                match op {
                    "stairs" => m.stairs_hook(&mut files, &mut Counted { level: false }),
                    "level" => {
                        m.level_entered(&mut arena::bsa::Files::default(), &mut Counted { level: true });
                    }
                    _ => m.inf_done_hook(),
                }
                let segs: String = [WSEG, FSEG, CSEG]
                    .iter()
                    .map(|&sg| format!(" {:x}", fnv((0..=0xFFFFu16).map(|o| m.mem.r8(sg, o)))))
                    .collect();
                Some(format!("{:x}{segs}", changed(&m, &before)))
            }
        };
        assert_eq!(f[3], got.as_deref().unwrap_or("fault"), "{line}");
        *n.entry(f[1]).or_insert(0) += 1;
    }
    eprintln!("{n:?}");
    assert!(n.values().sum::<usize>() > 4000);
}
