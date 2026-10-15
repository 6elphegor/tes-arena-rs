//! Differential tests of the automap screen's squares (src/mapdraw.rs:
//! 0000:2F20..2F47) against vectors from the original code
//! (harness/gen_mapdraw.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WALLS: u16 = 0x6000;
const FLOORS: u16 = 0x7000;
const SEEN: u16 = 0x8000;
const PAGE: u16 = 0xA000;
const TILE: usize = 997;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn lcg(x: &mut u32) -> u16 {
    *x = x.wrapping_mul(1103515245).wrapping_add(12345);
    (*x >> 16) as u16
}

/// Mirrors harness/gen_mapdraw.py `setup` (a tuple's items before its
/// index); returns the page.
fn setup(m: &mut Machine, seed: u32) -> Vec<u8> {
    let mut st = Stream(seed);
    let s = &mut st;
    let mode = s.byte() % 4;
    let (mut c398, mut c39a) = (s.word(), s.word());
    match mode {
        0 => {
            c398 |= 4;
            c39a |= 0x200;
        }
        1 => c398 &= !4,
        2 => {
            c398 |= 4;
            c39a &= !0x200;
        }
        _ => {}
    }
    let w = 1 + (s.byte() % 0x90) as u16;
    let d = 1 + (s.byte() % 0x40) as u16;
    let lefts = [s.word(), 0x7FE0 + (s.byte() % 0x40) as u16, ((s.byte() % 0x60) as i32 - 0x30) as u16, ((s.byte() % 0xA0) as i32 - 0x30) as u16];
    let left = lefts[s.byte() as usize % 4];
    let tops = [s.word(), ((s.byte() % 0x50) as i32 - 0x20) as u16, ((s.byte() % 0x50) as i32 - 0x20) as u16];
    let top = tops[s.byte() as usize % 3];
    let px = left.wrapping_add((s.byte() % 0x3C) as u16);
    let py = top.wrapping_add((s.byte() % 0x2A) as u16);
    let heading = s.word();
    let (exit, stairs) = (s.byte() % 8, s.byte() % 8);
    let triggers: Vec<u8> = (0..16).map(|_| s.byte() % 12).collect();
    let c943 = s.byte() % 8;
    let solid = 0x100 + (s.byte() % 4) as u16 * 0x100;
    let mut x = (s.word() as u32) << 16 | s.word() as u32;
    for (at, v) in [
        (0xC398, c398),
        (0xC39A, c39a),
        (0xC357, w),
        (0xC359, d),
        (0xA880, left),
        (0xA882, top),
        (0xCCA4, px),
        (0xCCA6, py),
        (0xC3DA, heading),
        (0xCCCE, solid),
        (0xD806, WALLS),
        (0xCCD0, FLOORS),
        (0x9030, SEEN),
        (0xA900, PAGE),
    ] {
        m.mem.w16(DS, at, v);
    }
    m.mem.w8(DS, 0xA865, exit);
    m.mem.w8(DS, 0xA866, stairs);
    for (i, &t) in triggers.iter().enumerate() {
        m.mem.w8(DS, 0xC917 + i as u16, t);
    }
    m.mem.w8(DS, 0xC943, c943);
    for r in 0..d {
        for c in 0..w {
            let i = (r << 8) | ((c * 2) & 0xFF);
            let v = lcg(&mut x);
            let u = lcg(&mut x);
            let wall = match v & 7 {
                0 => 0,
                1 => {
                    let b = u & 7 | (u >> 8) & 0x40;
                    b | b << 8
                }
                2 => u & 0x7F07,
                3 | 4 => 0x8000 | ((v >> 3) & 7) << 12 | (u & 0x0F07),
                5 => u & 0x0107,
                6 => u,
                _ => 0x8000 | (u & 0x0F07),
            };
            let f = lcg(&mut x);
            let floor = [solid.wrapping_sub(1), solid, solid + 0x100, SEEN + 0x100, solid + 0x200, f][f as usize % 6];
            let g = lcg(&mut x);
            let sn = [0, 7, 0xE, 0x15, (g >> 8) as u8][g as usize % 5];
            m.mem.w16(WALLS, i, wall);
            m.mem.w16(FLOORS, i, floor);
            m.mem.w8(SEEN, i >> 1, sn);
        }
    }
    let tile: Vec<u8> = (0..TILE)
        .map(|_| {
            let v = lcg(&mut x);
            if v & 1 != 0 {
                (v >> 8) as u8
            } else {
                0x30 + ((v >> 8) % 0x50) as u8
            }
        })
        .collect();
    tile.iter().copied().cycle().take(0x10000).collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let (mut n, mut faults) = (0, 0);
    for line in include_str!("vectors/mapdraw.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "mapdraw", "{line}");
        // A left of 8000h hangs the original (and the port): skipped.
        if f[2] == "fault" || f[2] == "hang" {
            faults += 1;
            continue;
        }
        let mut m = base.clone();
        let mut page = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let seen: Vec<u8> = (0..=0xFFFFu16).map(|o| m.mem.r8(SEEN, o)).collect();
        m.automap_squares(&|o| seen[o as usize], &mut page);
        let changed = (0..=0xFFFFu16).filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o)).flat_map(|o| {
            let mut v = o.to_le_bytes().to_vec();
            v.push(m.mem.r8(DS, o));
            v
        });
        let got = format!("{:x} {:x}", fnv(changed), fnv(page.iter().copied()));
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    eprintln!("{n} compared, {faults} faulted");
    assert!(n > 2900, "{n} compared");
}
