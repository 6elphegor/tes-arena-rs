//! Differential tests of the player's strike (src/strike.rs) against
//! vectors from the original code (harness/gen_strike.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, setup, Stream, CSEG, RSIZE};

const WSEG: u16 = 0x6800;
const FRAME: u16 = 0x8800;
const ACTORS: u16 = 0x0AB3;
const SIZE: u16 = 0x1C;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// Mirrors harness/gen_strike.py `strike`; returns the actor slot.
fn strike(m: &mut Machine, s: &mut Stream, rec: u16) -> u16 {
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

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/strike.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert!(f[0] == "strike" || f[0] == "quest", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        let si = strike(&mut m, &mut s, k * RSIZE);
        m.mem.w16(DS, si + 0x12, k * RSIZE);
        if f[0] == "quest" {
            // Mirrors harness/gen_strike.py `quest`.
            let rec = k * RSIZE;
            let v = m.mem.r16(DS, 0x05CA) | 0x40;
            m.mem.w16(DS, 0x05CA, v);
            m.mem.w16(CSEG, rec + 0x59, 1);
            m.mem.w16(CSEG, rec + 0x5B, 40);
            m.mem.w16(DS, 0x0E6E + 4, 0x40 | (hex(p[2]) as u16) << 9);
            let (ty, cl) = (m.mem.r8(CSEG, rec + 4), m.mem.r8(CSEG, rec + 5));
            put(&mut m, DS, 0x0E6E + 0x2F, &[(cl & 0x1F) + 0x18, ty]);
        }
        m.load_rng();
        let before = m.clone();
        let got = m.strike_actor(si).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("STRIKE_DUMP").is_ok_and(|v| v == p[0]) {
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
    assert!(counts.get("strike").copied().unwrap_or(0) > 2000);
    assert!(counts.get("quest").copied().unwrap_or(0) > 500);
}
