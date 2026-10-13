//! Differential tests of blows (src/attack.rs) against vectors from the
//! original code (harness/gen_attack.py): each case fills the player and
//! a combatant from a seed and runs one blow.
mod common;

use arena::machine::Machine;
use arena::mem::DS;

const CSEG: u16 = 0x7800;
const PLAYER: u16 = 0x01A4;
const RSIZE: u16 = 0x500;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// The LCG byte stream of harness/gen_attack.py `stream`.
struct Stream(u32);

impl Stream {
    fn byte(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 16) as u8
    }
    fn word(&mut self) -> u16 {
        let lo = self.byte() as u16;
        lo | (self.byte() as u16) << 8
    }
}

fn put(m: &mut Machine, seg: u16, off: u16, b: &[u8]) {
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

/// Mirrors harness/gen_attack.py `setup`.
fn setup(m: &mut Machine, seed: u32, k: u16, classes: &[u8]) {
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
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0x91F0, 0);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/attack.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert!(f[0] == "creature" || f[0] == "special", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let mut m = base.clone();
        setup(&mut m, seed, k, &classes);
        if f[0] == "special" {
            // Mirrors harness/gen_attack.py `special`.
            m.mem.w8(CSEG, k * RSIZE + 4, hex(p[2]) as u8);
            m.mem.w16(CSEG, k * RSIZE + 0x40D, 0x1000);
            m.mem.w8(DS, PLAYER + 0x62, 0xFF);
            m.mem.w16(DS, 0x05BE, 0);
        }
        m.mem.w16(DS, 0x1400, k * RSIZE);
        m.mem.w16(DS, 0x1402, PLAYER);
        m.load_rng();
        let before = m.clone();
        let got = m.attack_player(0).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))))
        });
        if std::env::var("ATTACK_DUMP").is_ok_and(|v| v == p[0]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(if f[2] == "fault" { "fault" } else { f[0] }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("creature").copied().unwrap_or(0) > 2000);
    assert!(counts.get("special").copied().unwrap_or(0) > 500);
}
