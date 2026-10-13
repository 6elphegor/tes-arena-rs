//! Differential tests of the player's controls (src/controls.rs
//! `player_input`, 2826:0002) against vectors from the original code
//! (harness/gen_input.py).
mod common;

use arena::bsa::Files;
use arena::controls::NoControlHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7000;
const FRAME: u16 = 0x8800;
const CURSEG: u16 = 0x7800;
const HANDLERS: [u16; 9] = [0x0750, 0x055A, 0x0768, 0x06A0, 0x077E, 0x06FA, 0x05FE, 0x05A9, 0x0651];
const KEYS: [u16; 8] = [0x1D, 0x24, 0x2A, 0x36, 0x48, 0x4B, 0x4D, 0x50];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// Mirrors harness/gen_input.py `setup`; returns the frame count.
fn setup(m: &mut Machine, seed: u32) -> u32 {
    let mut st = Stream(seed);
    let s = &mut st;
    let k = s.byte();
    let x = if k < 0xE0 { s.word() % 0x140 } else { s.word() };
    let k = s.byte();
    let y = if k < 0xC0 {
        (s.byte() % 0x93) as u16
    } else if k < 0xF0 {
        (s.byte() % 0xC8) as u16
    } else {
        s.word()
    };
    m.mem.w16(DS, 0x8F92, x);
    m.mem.w16(DS, 0x8F94, y);
    let v = s.byte() & 3;
    m.mem.w8(DS, 0xAFF9, v);
    let v = s.byte() & 3;
    m.mem.w8(DS, 0x7926, v);
    for scan in KEYS {
        let v = (s.byte() < 0x40) as u8;
        m.mem.w8(DS, 0xACB5 + scan, v);
    }
    let v = if s.byte() < 0xC0 { 0x93 } else { 0xC7 };
    m.mem.w16(DS, 0xA91A, v);
    let px = 0x200 + s.word() % 0x1C00;
    let py = 0x200 + s.word() % 0x1C00;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let v = s.word() & 0x1FF;
    m.mem.w16(DS, 0xC3DA, v);
    let k = s.byte();
    let v = if k < 0x80 {
        0x3C
    } else if k < 0xC0 {
        0x3C + (s.byte() % 0x40) as u16
    } else {
        s.word()
    };
    m.mem.w16(DS, 0xC3D2, v);
    let sq = ((py >> 7) << 8) + ((px >> 7) << 1);
    m.mem.w16(DS, 0xCB90, sq);
    let v = s.word();
    m.mem.w16(DS, 0xCB92, v);
    let v = if s.byte() < 0x40 { 7 } else { s.byte() % 8 };
    m.mem.w8(DS, 0x01A8, v);
    let v = if s.byte() < 0x20 { 0xC9 } else { s.byte() };
    m.mem.w8(DS, 0x01A9, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01CD, v);
    let v = s.word();
    m.mem.w16(DS, 0x020A, v);
    for dy in -2i16..3 {
        for dx in -2i16..3 {
            let at = sq.wrapping_add((dy * 0x100 + dx * 2) as u16);
            let k = s.byte();
            let wall = if k < 0xA0 {
                0
            } else if k < 0xE0 {
                (1 + (s.byte() % 0x3F) as u16) * 0x101
            } else {
                s.word() & 0x7FFF
            };
            m.mem.w16(WSEG, at, wall);
            let k = s.byte();
            let floor = if k < 0x40 { FRAME + 0xC00 } else { s.word() };
            m.mem.w16(FSEG, at, floor);
        }
    }
    let v = if s.byte() < 0xC0 { FRAME + 0xC00 } else { s.word() };
    m.mem.w16(DS, 0xCCCE, v);
    let v = if s.byte() < 0xC0 { 0x96 } else { s.word() & 0x1FF };
    m.mem.w16(DS, 0xD80C, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC942, v);
    let v = HANDLERS[(s.byte() % 9) as usize];
    m.mem.w16(DS, 0x7A54, v);
    let v = HANDLERS[(s.byte() % 9) as usize];
    m.mem.w16(DS, 0x7A56, v);
    let k = s.byte();
    let v = if k < 0x60 {
        0
    } else if k < 0xA0 {
        0x100
    } else {
        s.word() & 0x1FF
    };
    m.mem.w16(DS, 0x7A46, v);
    let v = if s.byte() < 0x80 { 0 } else { s.byte() as u16 };
    m.mem.w16(DS, 0xCCBA, v);
    let v = if s.byte() < 0xC0 { 0x28 } else { s.byte() as u16 };
    m.mem.w16(DS, 0xCCBE, v);
    let v = if s.byte() < 0xC0 { 0x10 } else { s.byte() as u16 };
    m.mem.w16(DS, 0xC392, v);
    let k = s.byte();
    let v = if k < 0xA0 {
        0
    } else if k < 0xF8 {
        1 + s.byte() % 10
    } else {
        s.byte()
    };
    m.mem.w8(DS, 0x05E3, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() % 8 };
    m.mem.w8(DS, 0xE39F, v);
    let v = s.word();
    m.mem.w16(DS, 0x7A6C, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x79A9, v);
    for o in [0x0DC0u16, 0x0DC2, 0x0DC4, 0x0DC8, 0x0620] {
        let k = s.byte();
        let v = if k < 0xC0 {
            0
        } else if k < 0xD0 {
            0x7D0
        } else {
            s.word()
        };
        m.mem.w16(DS, o, v);
    }
    let v = if s.byte() < 0x40 { 0x8000 } else { s.word() };
    m.mem.w16(DS, 0x0F84, v);
    let v = s.word();
    m.mem.w16(DS, 0x7DBA, v);
    let v = if s.byte() < 0x40 { 0xFFFE } else { s.word() };
    m.mem.w16(DS, 0x061E, v);
    let k = s.byte();
    let v = if k < 0xE0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x43CA, v);
    let v = s.word();
    m.mem.w16(DS, 0x43CC, v);
    let v = s.word() & 0xC293;
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word() & 0x0985;
    m.mem.w16(DS, 0xC398, v);
    let w = s.word();
    let v = w & if s.byte() < 0x20 { 0x40 } else { 0 };
    m.mem.w16(DS, 0x05B1, v);
    let v = s.word() & 0x2450;
    m.mem.w16(DS, 0x05E0, v);
    let v = s.word() & 0x9013;
    m.mem.w16(DS, 0x05CA, v);
    let v = s.word();
    m.mem.w16(DS, 0xCCBC, v);
    let b: Vec<u8> = (0..0x100).map(|_| s.byte()).collect();
    put(m, CURSEG, 0, &b);
    1 + (s.byte() % 6) as u32
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0xCCD0, FSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x1002, CURSEG);
    put(&mut base, DS, 0xC917, &[0xFF; 16]);
    put(&mut base, DS, 0xC932, &[0xFF; 2]);
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/input.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "input", "{line}");
        let mut m = base.clone();
        let frames = setup(&mut m, hex(f[1]) as u32);
        let before = m.clone();
        let mut got = Some(());
        for _ in 0..frames {
            got = got.and_then(|()| m.player_input(&mut files, &mut NoControlHooks));
        }
        let got = got.map(|()| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))))
        });
        if std::env::var("INPUT_DUMP").is_ok_and(|v| v == f[1]) {
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
        *counts.entry(if f[2] == "fault" { "fault" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 2500);
}
