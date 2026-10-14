//! Differential tests of the light routines (src/light.rs: `flat_lights`
//! 0000:0810, `lights_near` 0000:E12E, `light_row` 0000:B6EB and
//! `moving_lights` 0000:E106) against vectors from the original code (harness/gen_lights.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_lights.py `setup` (a conditional's test byte is
/// drawn first); returns (op, x, y).
fn setup(m: &mut Machine, seed: u32) -> (u8, u16, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 4;
    let cols = 1 + (s.byte() % 20) as u16;
    let rows = 1 + (s.byte() % 20) as u16;
    m.mem.w16(DS, 0xC357, cols);
    m.mem.w16(DS, 0xC359, rows);
    for r in 0..rows {
        for c in 0..cols {
            let k = s.byte() % 8;
            let sq = r << 8 | c << 1;
            match k {
                0 => {
                    let v = 0x8000 | (s.byte() % 0x20) as u16;
                    m.mem.w16(WSEG, sq, v);
                }
                1 => {
                    let hi = s.byte() as u16;
                    let v = hi << 8 | (1 + s.byte() % 0x20) as u16;
                    m.mem.w16(FSEG, sq, v);
                }
                2 => {
                    let v = s.word();
                    m.mem.w16(WSEG, sq, v);
                }
                _ => {}
            }
        }
    }
    let base = (s.byte() % 0x10) as u16;
    m.mem.w16(DS, 0xC38E, base);
    for f in 0..0x40u16 {
        let v = if s.byte() < 0xA0 { 0 } else { 1 + s.byte() % 8 };
        m.mem.w8(DS, 0xCDA2 + 5 * f, v);
    }
    let x = (s.word() as u32 % (cols as u32 * 128)) as u16;
    let y = (s.word() as u32 % (rows as u32 * 128)) as u16;
    m.mem.w16(DS, 0xC3D0, x);
    m.mem.w16(DS, 0xC3D4, y);
    let v = 0x100 + s.word() % 0x800;
    m.mem.w16(DS, 0xC396, v);
    let v = if s.byte() < 0x40 { (x >> 9) + (y >> 9) } else { s.word() };
    m.mem.w16(DS, 0xC8ED, v);
    let n = (s.byte() % 12) as u16;
    m.mem.w16(DS, 0xBC8E, n);
    for i in 0..n {
        let at = 0xBC90 + 6 * i;
        let v = (s.word() as u32 % (cols as u32 * 128 + 256)) as u16;
        m.mem.w16(DS, at, v);
        let v = (s.word() as u32 % (rows as u32 * 128 + 256)) as u16;
        m.mem.w16(DS, at + 2, v);
        let v = ((1 + s.byte() % 8) as u16) << 8;
        m.mem.w16(DS, at + 4, v);
    }
    let cnt = 1 + (s.byte() % 6) as u16;
    m.mem.w16(DS, 0xC3DC, cnt);
    for i in 0..cnt {
        let at = 0xC3DE + 6 * i;
        let v = if i == 0 { (x as i32 + s.byte() as i32 - 0x80) as u16 } else { (s.word() as u32 % (cols as u32 * 128)) as u16 };
        m.mem.w16(DS, at, v);
        let v = if i == 0 { (y as i32 + s.byte() as i32 - 0x80) as u16 } else { (s.word() as u32 % (rows as u32 * 128)) as u16 };
        m.mem.w16(DS, at + 2, v);
        let v = if s.byte() < 0x80 { 0x140 } else { s.word() % 0xC00 };
        m.mem.w16(DS, at + 4, v);
    }
    let (lx, ly, lr) = (m.mem.r16(DS, 0xC3DE), m.mem.r16(DS, 0xC3E0), m.mem.r16(DS, 0xC3E2));
    for i in 0..16 {
        let v = if s.byte() < 0xC0 { 0 } else { 1 };
        m.mem.w8(DS, 0xC997 + i, v);
    }
    let k = (s.byte() % 5) as u16;
    m.mem.w16(DS, 0xBC8C, k);
    for i in 0..k {
        let at = 0xBBCC + 6 * i;
        let v = (x as i32 + s.byte() as i32 * 2 - 0x100) as u16;
        m.mem.w16(DS, at, v);
        let v = (y as i32 + s.byte() as i32 * 2 - 0x100) as u16;
        m.mem.w16(DS, at + 2, v);
        let v = s.word() % 0x600;
        m.mem.w16(DS, at + 4, v);
    }
    let lim = if s.byte() < 0x80 {
        [0xE, 0xD, 0xE]
    } else {
        [(s.byte() % 16) as u16, (s.byte() % 16) as u16, (s.byte() % 16) as u16]
    };
    for (at, v) in [0xC8E7u16, 0xC8E9, 0xC8EB].into_iter().zip(lim) {
        m.mem.w16(DS, at, v);
    }
    let v = if s.byte() & 1 != 0 { 8 } else { 2 };
    m.mem.w16(DS, 0xBB02, v);
    let v = s.word() & 0x7FFF;
    let v = v | if s.byte() < 0x30 { 0x8000 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let px = (x as i32 + s.byte() as i32 * 4 - 0x200) as u16;
    let py = (y as i32 + s.byte() as i32 * 4 - 0x200) as u16;
    let (px, py) = match s.byte() % 4 {
        1 => (lx.wrapping_add(lr >> 1), ly),
        2 => (lx, ly.wrapping_add(lr)),
        3 => (x.wrapping_add(s.word() % 0x1000), y.wrapping_add(s.word() % 0x1000)),
        _ => (px, py),
    };
    if s.byte() < 0x60 {
        m.mem.w16(DS, 0xBBCC, px);
        m.mem.w16(DS, 0xBBCE, py);
    }
    for i in 0..0x20u16 {
        let at = 0xBB0C + 6 * i;
        let v = if s.byte() < 0x80 { 0x8000 } else { s.word() };
        m.mem.w16(DS, at, v);
        let v = s.word();
        m.mem.w16(DS, at + 2, v);
        let v = s.word();
        m.mem.w16(DS, at + 4, v);
    }
    (op, px, py)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0xCCD0, FSEG);
    let mut n = 0;
    for line in include_str!("vectors/lights.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "lights", "{line}");
        let mut m = base.clone();
        let (op, x, y) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let mut extra = String::new();
        match op {
            0 => m.flat_lights(),
            1 => m.lights_near(),
            3 => m.moving_lights(),
            _ => {
                let (row, cf) = m.light_row(x, y);
                extra = format!(" {row:x} {}", cf as u8);
            }
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[3], format!("{:x}{extra}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 3500);
}
