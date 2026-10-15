//! The 3D mode's level hooks' set-up, shared by tests/hooks.rs and
//! tests/inside.rs (mirrors harness/gen_hooks.py `setup_level`).
#![allow(dead_code)]

use super::blow::Stream;
use arena::hooks::StairsHooks;
use arena::machine::Machine;
use arena::mem::DS;

pub const WSEG: u16 = 0x6800;
pub const FSEG: u16 = 0x7000;
pub const CSEG: u16 = 0x7800;
pub fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}
pub fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}
/// The generator's stubs: calls counted at FFE0..FFE6.
pub struct Counted {
    pub level: bool,
}
impl StairsHooks for Counted {
    fn light_table(&mut self, m: &mut Machine) {
        count(m, 0xFFE0);
        count(m, 0xFFE2);
        count(m, 0xFFE4);
    }
    fn automap_keep(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        count(m, 0xFFE6);
    }
    fn automap_load(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        count(m, 0xFFE0);
    }
    fn quest_level(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        count(m, 0xFFE2);
        Some(())
    }
    fn level_piles(&mut self, m: &mut Machine) -> Option<()> {
        count(m, 0xFFE4);
        Some(())
    }
    fn level_screen(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        assert!(self.level);
        count(m, 0xFFE6);
    }
}
pub fn w16(m: &mut Machine, seg: u16, off: u16, v: u16) {
    m.mem.w16(seg, off, v);
}
/// Mirrors `flats`.
pub fn flats(m: &mut Machine, s: &mut Stream, n: u8) {
    for _ in 0..n {
        let r = (s.byte() % 0x40) as u16;
        let c = (s.byte() % 0x40) as u16;
        let hi = (s.byte() & 0x0F) as u16;
        let lo = (s.byte() % 0x20) as u16;
        w16(m, WSEG, r << 8 | c << 1, 0x8000 | hi << 8 | lo);
    }
}
pub fn setup_level(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    w16(m, DS, 0x1042, CSEG);
    let recs: Vec<u16> = (0..10).map(|i| 0x40 * (1 + i)).collect();
    for i in 0..8u16 {
        let si = 0x0C57 + i * 0x1C;
        let v = s.word() & 0xBFFF;
        let v = v | if s.byte() < 0x60 { 0x4000 } else { 0 };
        w16(m, DS, si + 0xF, v);
        let r = recs[(s.byte() % 10) as usize];
        w16(m, DS, si + 0x12, r);
    }
    for &r in &recs {
        let v = s.word();
        w16(m, CSEG, r + 0x40D, v);
    }
    for i in 0..16u16 {
        let e = 0x0660 + i * 0x3C;
        if s.byte() < 0x80 {
            let r = recs[(s.byte() % 10) as usize];
            w16(m, DS, e + 0x28, r);
            w16(m, DS, e + 0x2A, CSEG);
        } else {
            let a = s.word();
            w16(m, DS, e + 0x28, a);
            let b = s.word();
            w16(m, DS, e + 0x2A, b);
        }
        for k in 0..8 {
            let v = s.byte();
            m.mem.w8(DS, e + k, v);
        }
        let v = s.byte();
        m.mem.w8(DS, e + 0x3A, v);
    }
    for i in 0..12u16 {
        let v = s.byte();
        m.mem.w8(DS, 0x7DBC + i * 0x1C + 0x11, v);
    }
    for i in 0..8u16 {
        let v = s.word();
        w16(m, DS, 0x7F86 + i * 0x16 + 0xA, v);
    }
    for at in [0x7DB8, 0x0D5A, 0x7A46] {
        let v = s.word();
        w16(m, DS, at, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x4C91, v);
    let k = s.byte() % 4;
    let v = if k == 3 { s.byte() } else { [3, 3, 4][k as usize] };
    m.mem.w8(DS, 0x05E6, v);
    let levels = 1 + s.byte() % 6;
    m.mem.w8(DS, 0xC355, levels);
    let v = if s.byte() < 0xA0 { levels - 1 } else { s.byte() % 8 };
    m.mem.w8(DS, 0xBB0A, v);
    let place = s.byte() % 9;
    let sub = s.byte();
    m.mem.w8(DS, 0x0A46, place);
    m.mem.w8(DS, 0x0A4C, sub);
    for (p, l) in [(0x0FE3, 0x0FE2), (0x0FE5, 0x0FE4)] {
        if s.byte() < 0x60 {
            m.mem.w8(DS, p, place);
            m.mem.w8(DS, l, sub & 0x1F);
        } else {
            let a = s.byte() % 9;
            m.mem.w8(DS, p, a);
            let b = s.byte() & 0x1F;
            m.mem.w8(DS, l, b);
        }
    }
    let n = 1 + s.byte() % 8;
    m.mem.w8(DS, 0xC765, n);
    for i in 0..n as u16 {
        let r = (s.byte() % 0x40) as u16;
        let c = (s.byte() % 0x40) as u16;
        w16(m, DS, 0xC6E5 + 2 * i, r << 8 | c);
    }
    let v = s.byte();
    m.mem.w8(DS, 0xC93B, v);
    let n = s.byte() % 24;
    flats(m, s, n);
    for _ in 0..s.byte() % 16 {
        let r = (s.byte() % 0x40) as u16;
        let c = (s.byte() % 0x40) as u16;
        let v = s.word() & 0xFF1F;
        w16(m, FSEG, r << 8 | c << 1, v);
    }
    let v = s.byte() % 0x20;
    m.mem.w8(DS, 0xC943, v);
    for i in 0..8 {
        let v = s.byte() % 0x20;
        m.mem.w8(DS, 0xC987 + i, v);
    }
    let v = if s.byte() < 0x80 { 0 } else { 1 };
    m.mem.w8(DS, 0x191D, v);
    let v = if s.byte() < 0xA0 { 3 } else { s.byte() };
    m.mem.w8(DS, 0x0A47, v);
    let c941 = s.byte() % 0x20;
    m.mem.w8(DS, 0xC941, c941);
    for (at, small) in [(0xC357, 0), (0xC359, 1)] {
        let k = s.byte() % 4;
        let v = if k == 3 { (s.byte() % 0x80) as u16 } else { [0x40, 0x20, small][k as usize] };
        w16(m, DS, at, v);
    }
    let f77 = s.byte() % 8;
    m.mem.w8(DS, 0x0F77, f77);
    let v = if s.byte() < 0x80 { place } else { s.byte() % 9 };
    m.mem.w8(DS, 0x80EC + f77 as u16, v);
    let v = if s.byte() < 0x80 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x0F7D, v);
    for i in 0..0x40 {
        let v = s.byte() % 0x20;
        m.mem.w8(DS, 0xC954 + i, v);
    }
    let k = s.byte() % 4;
    let v = if k == 3 { s.word() } else { [0x3C, 0, 0xFFF0][k as usize] };
    w16(m, DS, 0xC3D2, v);
    let r = (1 + s.byte() % 0x3E) as u16;
    let c = (1 + s.byte() % 0x3E) as u16;
    let sq = r << 8 | c << 1;
    w16(m, DS, 0xCB90, sq);
    if s.byte() < 0xA0 {
        let v = (s.byte() | 1) as u16;
        let hi = if s.byte() < 0xC0 { v } else { s.byte() as u16 };
        w16(m, WSEG, sq, hi << 8 | v);
        for off in [0x100u16, 0xFF00, 2, 0xFFFE] {
            if s.byte() < 0x80 {
                w16(m, WSEG, sq.wrapping_add(off), 0x0101);
            }
        }
    }
    for at in [0xBA62, 0xBA66] {
        let v = s.word();
        w16(m, DS, at, v);
    }
    if s.byte() < 0x90 {
        let r = (s.byte() % 0x40) as u16;
        let c = (s.byte() % 0x40) as u16;
        w16(m, WSEG, r << 8 | c << 1, 0x8000 | c941 as u16);
    }
    w16(m, WSEG, 0x00FE, 0x0202);
    for _ in 0..8 {
        let r = (s.byte() % 0x40) as u16;
        w16(m, WSEG, r << 8 | 0xFE, 0x0303);
    }
}
pub fn changed(m: &Machine, before: &Machine) -> u64 {
    fnv((0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]))
}
