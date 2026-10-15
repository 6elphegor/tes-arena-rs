//! The game clock's test set-up (harness/gen_clock.py `setup`), shared by
//! the clock's test and those running the clock inside other routines.
#![allow(dead_code)]

use arena::machine::Machine;
use arena::mem::DS;
use super::blow::{put, Stream};

pub const CSEG: u16 = 0x7800;
pub const TSEG: u16 = 0x5C00;
pub const PLAYER: u16 = 0x01A4;
pub const CSIZE: u16 = 0x41E;
pub const SLOTS: u16 = 0x0660;

pub fn raw(s: &mut Stream, n: u16) -> Vec<u8> {
    (0..n).map(|_| s.byte()).collect()
}

/// Mirrors harness/gen_clock.py `setup`; returns the minutes.
pub fn setup(m: &mut Machine, seed: u32, auras: &[u8]) -> u32 {
    let mut st = Stream(seed);
    let s = &mut st;
    let mut t = s.word() as u32 | (s.byte() as u32) << 16;
    let k = s.byte();
    if k < 0x40 {
        t = t - t % 1440 + 1440 - 1 - (s.byte() % 4) as u32;
    } else if k < 0x50 {
        t = t - t % 518_400 + 518_400 - 1 - (s.byte() % 4) as u32;
    }
    m.mem.w32(DS, 0x05CC, t);
    let b = raw(s, 10);
    put(m, DS, 0x05D0, &b);
    let v = s.byte();
    m.mem.w8(DS, 0x0A20, v);
    let b = raw(s, 0x6E);
    put(m, DS, PLAYER, &b);
    let c = s.byte();
    m.mem.w8(DS, PLAYER + 5, if c < 0x40 { 0x11 } else { c % 0x12 });
    let race = if s.byte() < 0x30 { 7 } else { s.byte() % 8 };
    m.mem.w8(DS, PLAYER + 4, race);
    let level = 1 + s.byte() % 20;
    m.mem.w8(DS, PLAYER + 6, level);
    let f = if s.byte() < 0xC0 { s.byte() as u16 * 4 } else { s.word() };
    m.mem.w16(DS, PLAYER + 0x5D, f);
    let xp = s.word() as u32 * s.byte() as u32;
    m.mem.w32(DS, PLAYER + 0x409, xp);
    let v = s.word() & 0x00D0;
    m.mem.w16(DS, PLAYER + 0x40D, v);
    let v = if s.byte() < 0x40 { s.word() } else { 0 };
    m.mem.w16(DS, PLAYER + 0x41A, v);
    for i in 0..40 {
        let it = 0x0212 + i * 19;
        if s.byte() < 0x40 {
            continue;
        }
        let mut b = raw(s, 19);
        b[14] = b[14] & 0xB3 | 0x40 | (s.byte() % 4) << 2;
        put(m, DS, it, &b);
    }
    for k in 0..8u16 {
        let rec = k * CSIZE;
        let b = raw(s, 0x6E);
        put(m, CSEG, rec, &b);
        let ty = if s.byte() < 0x80 { auras[s.byte() as usize % auras.len()] } else { 1 + s.byte() % 0x18 };
        m.mem.w8(CSEG, rec + 4, ty);
        let v = s.word() & 0x00D0 | if s.byte() < 0xE0 { 0x1000 } else { 0 };
        m.mem.w16(CSEG, rec + 0x40D, v);
        let si = 0x0C57 + k * 0x1C;
        let near = s.byte() < 0x60;
        let (x, y) = if near { (s.byte() as u16, s.byte() as u16) } else { (s.word(), s.word()) };
        let t2 = s.byte();
        let sprite = if t2 < 0x10 { 0xC5 } else { 0xC0 + t2 % 5 };
        m.mem.w16(DS, si, x);
        m.mem.w16(DS, si + 2, 0);
        m.mem.w16(DS, si + 4, y);
        m.mem.w8(DS, si + 0xC, sprite);
        let v = if s.byte() < 0x30 { s.word() & 0x7FFF } else { s.word() & 0x3FFF };
        m.mem.w16(DS, si + 0xF, v);
        m.mem.w16(DS, si + 0x12, rec);
    }
    m.mem.w16(DS, 0x1020, 0);
    m.mem.w16(DS, 0x1022, TSEG);
    let b = raw(s, 0x240);
    put(m, TSEG, 0, &b);
    let b = raw(s, 16 * 0x3C);
    put(m, DS, SLOTS, &b);
    for i in 0..16 {
        let at = SLOTS + i * 0x3C;
        if s.byte() < 0xA0 {
            m.mem.w8(DS, at + 0x3B, 0xFF);
        } else {
            let k = (s.byte() % 9) as u16;
            let (off, seg) = if k == 8 { (PLAYER, DS) } else { (k * CSIZE, CSEG) };
            m.mem.w16(DS, at + 0x28, off);
            m.mem.w16(DS, at + 0x2A, seg);
            let kind = s.byte() % 12;
            m.mem.w8(DS, at + 0x3A, kind);
        }
    }
    let b = raw(s, 5);
    put(m, DS, 0x1378, &b);
    let v = if s.byte() < 0x20 { s.word() & 0x0084 } else { s.word() & 0x0004 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & 0x4210;
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0x30 { s.word() & 0x0020 } else { 0 };
    m.mem.w16(DS, 0x05C8, v);
    let v = if s.byte() < 0x30 { s.byte() } else { 0 };
    m.mem.w8(DS, 0x1013, v);
    let v = s.byte() % 5;
    m.mem.w8(DS, 0x05E6, v);
    let v = if s.byte() < 0x80 { 0xFF } else { s.byte() };
    m.mem.w8(DS, 0x137F, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() as u16 };
    m.mem.w16(DS, 0x0FFB, v);
    let v = s.word() & 0x0100;
    m.mem.w16(DS, 0x05CA, v);
    let v = if s.byte() < 0x20 { s.byte() } else { 0 };
    m.mem.w8(DS, 0xCB5C, v);
    let b = raw(s, 4);
    put(m, DS, 0x91F6, &b);
    let b = raw(s, 2);
    put(m, DS, 0x05E0, &b);
    let b = raw(s, 2);
    put(m, DS, 0x05C6, &b);
    let k = s.byte();
    if k < 0x80 {
        1 + (s.byte() % 30) as u32
    } else if k < 0xF0 {
        1 + (s.word() % 400) as u32
    } else {
        0x10001 + (s.byte() % 2) as u32
    }
}

