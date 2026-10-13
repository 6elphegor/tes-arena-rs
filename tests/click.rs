//! Differential tests of a click in the view (src/click.rs `view_click`,
//! 2826:077E) against vectors from the original code
//! (harness/gen_click.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};
use common::controls::*;

const SSEG: u16 = 0x6000;

/// Mirrors harness/gen_click.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    common::controls::setup(m, s);
    let (px, py) = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
    let (col, row) = (px >> 7, py >> 7);
    let near = |s: &mut Stream| {
        let r = (row + (s.byte() % 5) as u16).wrapping_sub(2) & 0xFF;
        let c = (col + (s.byte() % 5) as u16).wrapping_sub(2) & 0xFF;
        (r, c)
    };
    let square = |(r, c): (u16, u16)| (r << 8).wrapping_add(c << 1);
    let mut doors = vec![];
    for _ in 0..s.byte() % 6 {
        let rc = near(s);
        doors.push(rc);
        let w = s.word() & 0x01FF;
        let e = if s.byte() < 0xC0 { 0 } else { s.word() & 0x0E00 };
        m.mem.w16(WSEG, square(rc), 0xB000 | w | e);
    }
    let pick_door = |s: &mut Stream, doors: &Vec<(u16, u16)>| {
        if !doors.is_empty() && s.byte() < 0xC0 {
            doors[s.byte() as usize % doors.len()]
        } else {
            near(s)
        }
    };
    let (r, c) = pick_door(s, &doors);
    let x0 = s.word() % 0x150;
    m.mem.w16(DS, 0xCCB2, x0.wrapping_sub(0x20));
    let v = x0 + s.byte() as u16;
    m.mem.w16(DS, 0xCCB4, v);
    m.mem.w16(DS, 0xCCB6, c);
    m.mem.w16(DS, 0xCCB8, r);
    let v = if s.byte() < 0xE0 { square((r, c)) } else { s.word() };
    m.mem.w16(DS, 0xCCC8, v);
    let k = s.byte();
    let picked = if k < 0x30 {
        0xFFFF
    } else if k < 0xC0 && !doors.is_empty() {
        square(doors[s.byte() as usize % doors.len()])
    } else if k < 0xF0 {
        square(near(s))
    } else {
        s.word()
    };
    m.mem.w16(DS, 0xCB92, picked);
    let v = if s.byte() < 0x40 { 0xFFFF } else { square(near(s)) };
    m.mem.w16(DS, 0xFFF0, v);
    let n = s.byte() % 8;
    m.mem.w8(DS, 0xC6E4, n);
    for i in 0..n as u16 {
        let (r, c) = pick_door(s, &doors);
        let k = s.byte();
        let level = if k < 0x80 {
            1 + s.byte() % 12
        } else if k < 0xF0 {
            s.byte() % 0x20
        } else {
            s.byte()
        };
        m.mem.w16(DS, 0xC624 + 3 * i, r << 8 | c);
        m.mem.w8(DS, 0xC624 + 3 * i + 2, level);
    }
    for i in 0..12 {
        let v = if s.byte() < 0x80 { 0xC5 } else { 1 + s.byte() % 12 };
        m.mem.w8(DS, 0x0652 + i, v);
    }
    for i in 0..0x20 {
        let v = if s.byte() < 0x10 { picked } else { 0xFFFF };
        m.mem.w16(DS, 0x0F88 + 2 * i, v);
    }
    for at in [0x01A4 + 0x2A, 0x01A4 + 0x2C] {
        let v = s.byte();
        m.mem.w8(DS, at, v);
    }
    let v = s.byte() % 0x12;
    m.mem.w8(DS, 0x01A4 + 5, v);
    let v = s.byte() % 30;
    m.mem.w8(DS, 0x01A4 + 6, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 };
    m.mem.w8(DS, 0x12BC, v);
    let v = s.byte();
    m.mem.w8(DS, 0xCB5B, v);
    for o in [0x0DC0u16, 0x0DC2, 0x0DC4, 0x0DC8, 0x0620] {
        m.mem.w16(DS, o, 0);
    }
    let pm = |s: &mut Stream| {
        if s.byte() < 0x80 {
            (1 + (s.byte() % 3) as u16).wrapping_neg()
        } else {
            1 + (s.byte() % 3) as u16
        }
    };
    match s.byte() % 7 {
        1 => {
            let v = pm(s);
            m.mem.w16(DS, 0x0DC0, v);
        }
        2 => {
            let v = pm(s);
            m.mem.w16(DS, 0x0DC2, v);
        }
        3 => {
            let v = if s.byte() < 0x60 { 0x7D0 } else { s.byte() as u16 };
            m.mem.w16(DS, 0x0DC4, v);
        }
        4 => {
            let v = 1 + (s.byte() % 0x20) as u16;
            m.mem.w16(DS, 0x0DC8, v);
        }
        5 => {
            m.mem.w16(DS, 0x0620, 0x1044);
            m.mem.w16(DS, 0x1044, 0);
            m.mem.w16(DS, 0x1046, SSEG);
            let b: Vec<u8> = (0..0x30).map(|_| s.byte()).collect();
            put(m, SSEG, 0, &b);
        }
        _ => {}
    }
    let v = s.byte() as u16;
    m.mem.w16(DS, 0x0DC6, v);
    for i in 0..0x17 {
        let v = if s.byte() < 0x20 { square(near(s)) } else { 0xFFFF };
        m.mem.w16(DS, 0x1951 + 2 * i, v);
    }
    for i in 0..0x20 {
        let v = if s.byte() < 0xC0 { 0xFFFF } else { s.word() };
        m.mem.w16(DS, 0xC2C2 + 2 * i, v);
    }
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = base(&img);
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/click.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "click", "{line}");
        let mut m = base.clone();
        setup(&mut m, hex(f[1]) as u32);
        m.load_rng();
        let before = m.clone();
        let got = m.view_click(&mut files, &mut Pick).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(FSEG, o)))
            )
        });
        if std::env::var("CLICK_DUMP").is_ok_and(|v| v == f[1]) {
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
