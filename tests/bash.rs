//! Differential tests of a swing hitting the wall ahead (src/weapon.rs
//! `bash`, 0000:3D43) against vectors from the original code
//! (harness/gen_bash.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::weapon::WeaponHooks;
use common::blow::{put, Stream};

const WSEG: u16 = 0x6800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The generator's hooks: the wall trigger counts its calls at FFF4, and
/// a building is closed when FFF8 bit 0 is set.
struct Hooks;

impl WeaponHooks for Hooks {
    fn wall_trigger(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        let v = m.mem.r16(DS, 0xFFF4).wrapping_add(1);
        m.mem.w16(DS, 0xFFF4, v);
        Some(())
    }
    fn building_closed(&mut self, m: &mut Machine, _kind: u8) -> Option<bool> {
        Some(m.mem.r8(DS, 0xFFF8) & 1 != 0)
    }
}

/// Mirrors harness/gen_bash.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let px = 0x200 + s.word() % 0x3C00;
    let py = 0x200 + s.word() % 0x3C00;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let v = s.word() & 0x1FF;
    m.mem.w16(DS, 0xC3DA, v);
    let sq = ((py >> 7) << 8) + ((px >> 7) << 1);
    m.mem.w16(DS, 0xCB90, sq);
    let triggers: Vec<u8> = (0..16).map(|_| s.byte() % 0x40).collect();
    put(m, DS, 0xC917, &triggers);
    let flat = s.byte();
    m.mem.w8(DS, 0xC93B, flat);
    let v = s.byte();
    m.mem.w8(DS, 0xC93C, v);
    let mut locks = vec![];
    for dy in [0xFF00u16, 0, 0x100] {
        for dx in [0xFFFEu16, 0, 2] {
            let at = sq.wrapping_add(dy).wrapping_add(dx);
            let k = s.byte();
            let wall = if k < 0x30 {
                0
            } else if k < 0x80 {
                let t = if s.byte() < 0xC0 { triggers[(s.byte() % 16) as usize] } else { s.byte() % 0x40 };
                let hi = if s.byte() < 0xE0 { 0 } else { s.word() & 0x7F00 };
                (t as u16 * 0x101) | hi
            } else if k < 0xB0 {
                let w = s.word() & 0x0F00;
                let lo = if s.byte() < 0xC0 { flat } else { s.byte() };
                0x8000 | w | lo as u16
            } else if k < 0xF0 {
                let w = 0xB000 | (s.word() & 0x0DFF);
                if s.byte() < 0xC0 {
                    locks.push((at, s.byte() % 0x20));
                }
                w
            } else {
                s.word()
            };
            m.mem.w16(WSEG, at, wall);
        }
    }
    m.mem.w8(DS, 0xC6E4, locks.len() as u8);
    for (i, &(at, level)) in locks.iter().enumerate() {
        let r = 0xC624 + 3 * i as u16;
        m.mem.w16(DS, r, (at & 0xFF00) | ((at & 0xFF) >> 1));
        m.mem.w8(DS, r + 2, level);
    }
    let v = if s.byte() < 0xA0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & 0x0200;
    m.mem.w16(DS, 0xC39A, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0xFFF8, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01CD, v);
    let v = s.byte() % 20;
    m.mem.w8(DS, 0x01A4 + 6, v);
    let v = (s.byte() % 16) as u16;
    m.mem.w16(DS, 0x01A4 + 0x39, v);
    let v = s.word() & 0x0820;
    m.mem.w16(DS, 0x05CA, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x0FFB, v);
    let v = s.word();
    m.mem.w16(DS, 0x0FF9, v);
    let v = if s.byte() < 0x80 { 4 } else { s.byte() % 6 };
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() % 12;
    m.mem.w8(DS, 0x0D48, v);
    for i in 0..23u16 {
        let v = s.word();
        m.mem.w16(DS, 0x0AB3 + 0x1C * i + 0xF, v);
        m.mem.w16(DS, 0x0AB3 + 0x1C * i + 0x12, 0x41E * (i % 8));
    }
    let b: Vec<u8> = (0..16 * 0x3C).map(|_| s.byte()).collect();
    put(m, DS, 0x0660, &b);
    for i in 0..16u16 {
        if s.byte() < 0x60 {
            let rec = 0x41E * (s.byte() % 8) as u16;
            m.mem.w16(DS, 0x0660 + 0x3C * i + 0x28, rec);
            m.mem.w16(DS, 0x0660 + 0x3C * i + 0x2A, 0x7800);
        }
    }
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x1042, 0x7800);
    base.mem.w16(DS, 0x91F0, 0);
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/bash.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "bash", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let got = m.bash(&mut files, &mut Hooks).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))))
        });
        if std::env::var("BASH_DUMP").is_ok_and(|v| v == f[1]) {
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
