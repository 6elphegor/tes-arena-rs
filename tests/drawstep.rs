//! Differential tests of an object's animation step as it is drawn
//! (src/projectile.rs `drawn_step`, 0000:C23A..C26C) against vectors from
//! the original code (harness/gen_drawstep.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const SLOT: u16 = 0x7DBC;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_drawstep.py `setup` (a conditional's test byte
/// first; a tuple's items, all drawn, before its index is used).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let mut rec: Vec<u8> = (0..0x1C).map(|_| s.byte()).collect();
    let mut flags = s.word() & !0x2104;
    if s.byte() < 0x40 {
        flags |= [0x4, 0x2000, 0x2004][s.byte() as usize % 3];
    }
    if s.byte() < 0x80 {
        flags |= 0x100;
    }
    rec[0xF..0x11].copy_from_slice(&flags.to_le_bytes());
    let count = s.byte() % 0x10 | if s.byte() & 1 != 0 { 0x80 } else { 0 };
    rec[0xE] = count;
    rec[0xD] = if s.byte() < 0xC0 { ((count & 0x7F) as i32 - 1 + (s.byte() % 3) as i32) as u8 } else { s.byte() };
    let k = s.byte() % 4;
    let some = 0xBB0C + 6 * (s.byte() as u16 % 32);
    let any = s.word();
    let light = [0, some, any, 0xBB0C][k as usize];
    rec[6..8].copy_from_slice(&light.to_le_bytes());
    for (i, &b) in rec.iter().enumerate() {
        m.mem.w8(DS, SLOT + i as u16, b);
    }
    let n = (s.byte() % 0x22) as u16;
    m.mem.w16(DS, 0xBC8C, n);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/drawstep.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "drawstep", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        m.drawn_step(SLOT);
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 2500);
}
