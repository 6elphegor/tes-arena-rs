//! The seeded setup shared by the blow tests (harness/gen_attack.py).
#![allow(dead_code)]

use arena::machine::Machine;
use arena::mem::DS;

pub const CSEG: u16 = 0x7800;
pub const PLAYER: u16 = 0x01A4;
pub const RSIZE: u16 = 0x500;

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

