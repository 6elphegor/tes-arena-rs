//! Differential tests of the doors stepping as the original walks the
//! squares in view (src/engine.rs `view_walk_doors`, after `view_begin`)
//! against vectors from the original's view call, drawing included
//! (harness/gen_walk.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x6800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_walk.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let (cols, rows) = (8 + s.byte() as u16 % 121, 8 + s.byte() as u16 % 121);
    m.mem.w16(DS, 0xC357, cols);
    m.mem.w16(DS, 0xC359, rows);
    let x = (s.byte() as u16 % (cols + 4)) << 7 | (s.byte() & 0x7F) as u16;
    let y = (s.byte() as u16 % (rows + 4)) << 7 | (s.byte() & 0x7F) as u16;
    m.mem.w16(DS, 0xC3D0, x);
    m.mem.w16(DS, 0xC3D4, y);
    let h = s.word();
    let h = if s.byte() & 3 != 0 { h & 0x7F } else { h };
    m.mem.w16(DS, 0xC3D2, h);
    let reach = 1 + s.byte() as i32 % 8;
    m.mem.w16(DS, 0xC394, reach as u16);
    let v = s.word() & !0x24;
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & !0x204;
    let v = v | if s.byte() % 8 == 0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    m.mem.w16(DS, 0xCD46, 0);
    m.mem.w16(DS, 0xCD48, 0);
    let (col, row) = ((x >> 7) as i32, (y >> 7) as i32);
    for dr in -reach - 1..reach + 2 {
        for dc in -reach - 1..reach + 2 {
            let k = s.byte() % 10;
            let v = if k < 4 {
                0
            } else if k < 6 {
                let t = 1 + (s.byte() % 0x3F) as u16;
                t << 8 | t
            } else if k < 7 {
                let t = s.byte() as u16;
                0xD000 | (t & 0xF) << 8 | t
            } else {
                0xB000 | s.word() & 0x0FFF
            };
            m.mem.w16(WSEG, (((row + dr) << 8) + ((col + dc) << 1)) as u16, v);
        }
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    let mut n = 0;
    for line in include_str!("vectors/walk.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "walk", "{line}");
        if f[2] == "fault" {
            continue;
        }
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.view_begin();
        assert!(m.view_walk_doors().is_empty(), "{line}");
        let plane = (0..=0xFFFFu16).map(|o| m.mem.r8(WSEG, o));
        assert_eq!(f[2], format!("{:x}", fnv(plane)), "{line}");
        n += 1;
    }
    assert!(n > 1200);
}
