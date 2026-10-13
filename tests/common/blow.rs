//! The seeded setup shared by the blow tests (harness/gen_attack.py).
#![allow(dead_code)]

use arena::machine::Machine;
use arena::mem::DS;

pub const CSEG: u16 = 0x7800;
pub const PLAYER: u16 = 0x01A4;
pub const RSIZE: u16 = 0x500;
/// The strike tests' wall plane, EMS frame and actor slots.
pub const WSEG: u16 = 0x6800;
pub const FRAME: u16 = 0x8800;
pub const ACTORS: u16 = 0x0AB3;
pub const SIZE: u16 = 0x1C;

/// The LCG byte stream of harness/gen_attack.py `stream`.
pub struct Stream(pub u32);

impl Stream {
    pub fn byte(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 16) as u8
    }
    pub fn word(&mut self) -> u16 {
        let lo = self.byte() as u16;
        lo | (self.byte() as u16) << 8
    }
}

pub fn put(m: &mut Machine, seg: u16, off: u16, b: &[u8]) {
    for (i, &c) in b.iter().enumerate() {
        m.mem.w8(seg, off.wrapping_add(i as u16), c);
    }
}

fn items(m: &mut Machine, s: &mut Stream, seg: u16, at: u16) {
    for i in 0..40 {
        let it = at + i * 19;
        if s.byte() < 0x40 {
            continue;
        }
        let mut b: Vec<u8> = (0..19).map(|_| s.byte()).collect();
        let kind = s.byte() & 3;
        let opts = [s.byte() % 11, s.byte() % 18, s.byte() & 3, s.byte()];
        let mut ty = opts[kind as usize];
        let cat = if kind < 3 { kind } else { s.byte() & 3 };
        if s.byte() < 0x20 {
            ty = 0x80 | ty & 0xF;
        }
        b[0] = ty;
        b[4] = s.byte() % 20;
        b[5] = b[4].wrapping_add(s.byte() % 12);
        let v = s.byte();
        let health = if v < 0x50 {
            (s.byte() & 7) as u16
        } else if v >= 0xF8 {
            0xFFFF
        } else {
            s.word() % 1500
        };
        b[6..8].copy_from_slice(&health.to_le_bytes());
        let eq = if s.byte() < 0x90 { 0x80 } else { 0 };
        b[14] = cat << 2 | 0x40 | eq | b[14] & 0x31;
        b[16] = if s.byte() < 0xD0 { s.byte() & 7 } else { 0xFF };
        b[18] = if s.byte() < 0xC0 { s.byte() & 7 } else { 0xFF };
        if s.byte() < 0x10 {
            b[17] = 0xC1;
        }
        put(m, seg, it, &b);
    }
}

fn record(m: &mut Machine, s: &mut Stream, seg: u16, rec: u16, creature: bool, classes: &[u8]) {
    let ty = if creature { s.byte() % 0x18 } else { s.byte() & 7 };
    put(m, seg, rec + 4, &[ty]);
    let class = if s.byte() < 0x30 {
        0x4D
    } else if s.byte() < 0xE0 {
        classes[s.byte() as usize % classes.len()]
    } else {
        s.byte()
    };
    put(m, seg, rec + 5, &[class]);
    let level = 1 + s.byte() % 30;
    put(m, seg, rec + 6, &[level]);
    let lo = s.byte() % 12;
    put(m, seg, rec + 9, &[lo]);
    let hi = 12 + s.byte() % 30;
    put(m, seg, rec + 0xA, &[hi]);
    let v = (s.byte() % 16) as i16 - 4;
    put(m, seg, rec + 0x39, &v.to_le_bytes());
    let v = s.byte() as i16 - 0x80;
    put(m, seg, rec + 0x47, &v.to_le_bytes());
    let v = s.byte() as i16 - 0x80;
    put(m, seg, rec + 0x49, &v.to_le_bytes());
    let v = (s.byte() % 64) as i16 - 32;
    put(m, seg, rec + 0x57, &v.to_le_bytes());
    let v = 1 + s.byte() as u16;
    put(m, seg, rec + 0x59, &v.to_le_bytes());
    let v = 1 + s.word() % 400;
    put(m, seg, rec + 0x5B, &v.to_le_bytes());
    let v = s.byte();
    put(m, seg, rec + 0x62, &[v]);
    let v = (if creature { 0x1000 } else { 0 }) | s.word() & 0x0090;
    put(m, seg, rec + 0x40D, &v.to_le_bytes());
    let v: u16 = if s.byte() < 0xA0 { 0 } else { s.byte() as u16 };
    put(m, seg, rec + 0x41C, &v.to_le_bytes());
}

/// Mirrors harness/gen_attack.py `setup`; returns the stream, which
/// `extra` continues.
pub fn setup(m: &mut Machine, seed: u32, k: u16, classes: &[u8]) -> Stream {
    let mut s = Stream(seed);
    record(m, &mut s, DS, PLAYER, false, classes);
    items(m, &mut s, DS, PLAYER + 0x6E);
    let creature = s.byte() < 0xA0;
    record(m, &mut s, CSEG, k * RSIZE, creature, classes);
    items(m, &mut s, CSEG, k * RSIZE + 0x6E);
    if k != 0 {
        let ty = s.byte() % 0x18;
        put(m, CSEG, 4, &[ty]);
        let f: u16 = if s.byte() < 0x80 { 0x1000 } else { 0 };
        put(m, CSEG, 0x40D, &f.to_le_bytes());
    }
    let b: Vec<u8> = (0..0x3C0).map(|_| s.byte()).collect();
    put(m, DS, 0x0660, &b);
    for i in 0..16 {
        let t = s.byte();
        let at = 0x0660 + i * 0x3C;
        if t < 0x80 {
            put(m, DS, at + 0x3B, &[0xFF]);
        } else if t < 0xC0 {
            m.mem.w16(DS, at + 0x28, PLAYER);
            m.mem.w16(DS, at + 0x2A, DS);
            let kind = s.byte() % 10;
            put(m, DS, at + 0x3A, &[kind]);
        }
    }
    let b: Vec<u8> = (0..4 + 12 * 0x1C).map(|_| s.byte()).collect();
    put(m, DS, 0x7DB8, &b);
    let b: Vec<u8> = (0..8 * 0x16).map(|_| s.byte()).collect();
    put(m, DS, 0x7F86, &b);
    let b: Vec<u8> = (0..2).map(|_| s.byte()).collect();
    put(m, DS, 0x0D5A, &b);
    for (o, n) in [(0x05BEu16, 2usize), (0x05C6, 2), (0x05CA, 2), (0x0D49, 1), (0x0D39, 1), (0x1949, 2), (0x91F6, 4)] {
        let mut v: Vec<u8> = (0..n).map(|_| s.byte()).collect();
        match o {
            0x05BE => v = vec![v[0] & 0x60, 0],
            0x05CA => v = vec![v[0] & 0x60, v[1] & 0x10],
            0x05C6 => v = vec![v[0] % 40, 0],
            0x0D49 => v = vec![v[0] % 8],
            0x1949 if v[0] < 0xC0 => v = vec![0, 0],
            _ => {}
        }
        put(m, DS, o, &v);
    }
    s
}

/// Mirrors harness/gen_strike.py `strike`; returns the actor slot.
pub fn strike(m: &mut Machine, s: &mut Stream, rec: u16) -> u16 {
    let j = 15 + (s.byte() % 8) as u16;
    let si = ACTORS + j * SIZE;
    let b: Vec<u8> = (0..SIZE).map(|_| s.byte()).collect();
    put(m, DS, si, &b);
    let (x, y) = (s.word() & 0x1FFF, s.word() & 0x1FFF);
    m.mem.w16(DS, si, x);
    m.mem.w16(DS, si + 2, 0);
    m.mem.w16(DS, si + 4, y);
    m.mem.w16(DS, si + 6, 0xC3D0);
    let v = s.word() & 0x1FF;
    m.mem.w16(DS, si + 0xA, v);
    let v = s.word() & 0x3FFF;
    m.mem.w16(DS, si + 0xF, v);
    let px = x.wrapping_add(s.byte() as u16).wrapping_sub(0x80);
    let py = y.wrapping_add(s.byte() as u16).wrapping_sub(0x80);
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let sq = (y >> 7) << 8 | (x >> 7) << 1;
    let t = s.byte();
    let wall = if t < 0x80 {
        0xB000 | s.word() & 0xFFF
    } else if t < 0xC0 {
        0
    } else {
        s.word()
    };
    m.mem.w16(WSEG, sq, wall);
    let north = if s.byte() < 0x80 { 0 } else { s.word() };
    m.mem.w16(WSEG, sq.wrapping_sub(0x100), north);
    for i in 0..0x30 {
        let o = s.byte() as u16;
        m.mem.w16(DS, 0xD0B8 + 4 * i, o);
        m.mem.w16(DS, 0xD0BA + 4 * i, 0);
    }
    let b: Vec<u8> = (0..0x110).map(|_| s.byte()).collect();
    put(m, FRAME, 0, &b);
    let b: Vec<u8> = (0..4 * 0x34).map(|_| s.byte()).collect();
    put(m, DS, 0x0E6E, &b);
    let t = s.byte();
    if t < 0x60 {
        let stage: u16 = if t < 0x30 { 6 } else { 4 };
        let v = 0x40 | stage << 9 | s.word() & 0xF13F;
        m.mem.w16(DS, 0x0E6E + 4, v);
        let c = 0x18 + s.byte() % 0x20;
        let ty = s.byte() % 0x18;
        put(m, DS, 0x0E6E + 0x2F, &[c, ty]);
    }
    let b: Vec<u8> = (0..9).map(|_| if s.byte() < 0xC0 { s.byte() % 10 } else { 0xFF }).collect();
    put(m, DS, 0x0FD4, &b);
    let v = s.word();
    m.mem.w16(DS, 0x102C, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CA, v & 0x0160);
    let v = s.word();
    m.mem.w16(DS, 0x653E, v);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x05C2, &b);
    if s.byte() < 0x40 {
        let v = s.word() & 0x1190;
        m.mem.w16(CSEG, rec + 0x40D, v);
    }
    if s.byte() < 0x30 {
        let v = if s.byte() & 1 != 0 { 0 } else { 0xFFF0 };
        m.mem.w16(CSEG, rec + 0x5B, v);
    }
    let t = s.byte();
    if t < 0x50 {
        let ty = match t {
            0..=0x1F => 0x17,
            0x20..=0x2F => 0x2A,
            _ if t & 1 != 0 => 0x0E,
            _ => 0x16,
        };
        m.mem.w8(CSEG, rec + 4, ty);
    }
    if s.byte() < 0x40 {
        let q = s.byte() % 0x18;
        m.mem.w16(DS, 0x0E6E + 0x34 + 4, 0x40 | 6 << 9);
        m.mem.w8(DS, 0x0E6E + 0x34 + 0x30, q);
        if s.byte() < 0x80 {
            m.mem.w8(CSEG, rec + 4, q);
        }
    }
    si
}
