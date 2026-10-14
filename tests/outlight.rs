//! Differential tests of the outdoor frame's light (src/light.rs
//! `outdoor_light`, 0000:0390, with the night and day switches) against
//! vectors from the original code (harness/gen_outlight.py).
mod common;

use arena::light::LightHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's stubs: calls counted at FFE0..FFF0.
struct Hooks;

impl LightHooks for Hooks {
    fn light_table(&mut self, m: &mut Machine) {
        count(m, 0xFFE0);
        count(m, 0xFFE2);
        count(m, 0xFFE4);
    }
    fn relight(&mut self, m: &mut Machine, _level: u16) {
        count(m, 0xFFE8);
        count(m, 0xFFEA);
    }
    fn restore_textures(&mut self, m: &mut Machine) {
        count(m, 0xFFE6);
    }
    fn sky_objects(&mut self, m: &mut Machine) {
        count(m, 0xFFEE);
    }
    fn palette(&mut self, m: &mut Machine, _index: u8, _at: u16) {
        count(m, 0xFFEC);
    }
    fn sky(&mut self, m: &mut Machine, step: arena::sky::SkyStep) {
        if step.palette {
            count(m, 0xFFF0);
        }
        if step.dither.is_some() {
            count(m, 0xFFE2);
        }
    }
}

/// Mirrors harness/gen_outlight.py `setup` (a tuple's items are all
/// drawn before its index; a conditional's test first).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = s.word() & 0x7FFB;
    let v = v | if s.byte() < 0xE0 { 4 } else { 0 };
    let v = v | if s.byte() & 1 != 0 { 0x8000 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & !0x200;
    let v = v | if s.byte() < 0x40 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x0F81, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0F82, v);
    let k = (s.byte() % 5) as usize;
    let t = [
        0xF8 + (s.byte() % 0x20) as u16,
        0x2E8 + (s.byte() % 0x20) as u16,
        s.byte() as u16,
        0x300 + s.byte() as u16,
        0x110 + s.word() % 0x1E0,
    ][k];
    m.mem.w16(DS, 0xCCFE, t);
    let choices = [0xA00, 0xFFFF, ((s.byte() % 11) as u16) << 8, s.word()];
    let v = choices[(s.byte() % 4) as usize];
    m.mem.w16(DS, 0x3C87, v);
    for at in [0xC8E7, 0xC8E9, 0xC8EB] {
        let v = (s.byte() % 16) as u16;
        m.mem.w16(DS, at, v);
    }
    for at in [0x3C85, 0x3C89, 0x3C8B, 0xC8ED] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let (cols, rows) = (1 + (s.byte() % 12) as u16, 1 + (s.byte() % 12) as u16);
    m.mem.w16(DS, 0xC357, cols);
    m.mem.w16(DS, 0xC359, rows);
    for r in 0..rows {
        for c in 0..cols {
            let sq = r << 8 | c << 1;
            match s.byte() % 8 {
                k @ 0..=1 => m.mem.w16(WSEG, sq, 0x801D + k as u16),
                2 => {
                    let v = 0x8000 | (s.byte() % 0x20) as u16;
                    m.mem.w16(WSEG, sq, v);
                }
                3 => {
                    let hi = s.byte() as u16;
                    let v = hi << 8 | (1 + s.byte() % 0x20) as u16;
                    m.mem.w16(FSEG, sq, v);
                }
                4 => {
                    let v = s.word();
                    m.mem.w16(WSEG, sq, v);
                }
                _ => {}
            }
        }
    }
    let v = (s.byte() % 0x10) as u16;
    m.mem.w16(DS, 0xC38E, v);
    for f in 0..0x40u16 {
        let v = if s.byte() < 0xA0 { 0 } else { 1 + s.byte() % 8 };
        m.mem.w8(DS, 0xCDA2 + 5 * f, v);
    }
    let n = (s.byte() % 8) as u16;
    m.mem.w16(DS, 0xBC8E, n);
    for i in 0..n * 3 {
        let v = s.word();
        m.mem.w16(DS, 0xBC90 + 2 * i, v);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0xCCD0, FSEG);
    base.mem.w16(DS, 0x43C0, DS);
    let mut n = 0;
    for line in include_str!("vectors/outlight.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "outlight", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        m.outdoor_light(&mut Hooks);
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let plane = (0..0x1000u16).map(|o| m.mem.r8(WSEG, o));
        assert_eq!(f[2], format!("{:x}", fnv(changed.chain(plane))), "{line}");
        n += 1;
    }
    assert!(n > 3500);
}
