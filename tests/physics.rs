//! Differential tests of the player's physics frame (src/movement.rs
//! `physics_frame`, 0000:F57C) against vectors from the original code
//! (harness/gen_physics.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::movement::NoPhysicsHooks;
use common::blow::Stream;

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7000;
const FRAME: u16 = 0x8800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// Mirrors harness/gen_physics.py `setup`; returns the frame count.
fn setup(m: &mut Machine, seed: u32) -> u32 {
    let mut st = Stream(seed);
    let s = &mut st;
    let (px, py) = (s.word() & 0x1FFF, s.word() & 0x1FFF);
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let k = s.byte();
    let height = if k < 0x60 {
        0x3C
    } else if k < 0xA0 {
        0x3C + s.byte() as u16
    } else if k < 0xE0 {
        (s.byte() as u16).wrapping_sub(0x80)
    } else {
        s.word()
    };
    m.mem.w16(DS, 0xC3D2, height);
    let v = if s.byte() < 0x80 { 4 } else { (s.byte() % 0x40) as u16 };
    m.mem.w16(DS, 0xE8BE, v);
    let sq = ((py >> 7) << 8) + ((px >> 7) << 1);
    let k = s.byte();
    let wall = if k < 0x50 {
        0
    } else if k < 0x80 {
        let v = s.byte() as u16;
        v << 8 | v
    } else if k < 0xE0 {
        s.word() & 0x7FFF
    } else {
        s.word()
    };
    m.mem.w16(WSEG, sq, wall);
    let k = s.byte();
    let floor = if k < 0x40 {
        FRAME + 0xC00
    } else if k < 0xA0 {
        FRAME + 0xC00 + s.word() % 0x800
    } else {
        s.word()
    };
    m.mem.w16(FSEG, sq, floor);
    let v = if s.byte() < 0xC0 { FRAME + 0xC00 } else { s.word() };
    m.mem.w16(DS, 0xCCCE, v);
    let v = s.word() & 0xC211;
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word() & 0x2204;
    m.mem.w16(DS, 0xC398, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() % 4 };
    m.mem.w8(DS, 0xE39F, v);
    for at in [0xE8B6u16, 0xCCBC, 0xC3D8, 0xD80C] {
        let v = s.word();
        m.mem.w16(DS, at, if at == 0xE8B6 { v & 0xF } else { v });
    }
    let v = if s.byte() < 0xC0 { 0 } else { s.word() };
    m.mem.w16(DS, 0xCB6A, v);
    let v = s.word();
    m.mem.w16(DS, 0xCB68, v);
    1 + (s.byte() % 24) as u32
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0xCCD0, FSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    // The input and transition hooks are set (their targets are hooks).
    for hook in [0xC3B4u16, 0xC3AC] {
        base.mem.w16(DS, hook, 0x002A);
        base.mem.w16(DS, hook + 2, 0x23BA);
    }
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/physics.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "physics", "{line}");
        let mut m = base.clone();
        let frames = setup(&mut m, hex(f[1]) as u32);
        let before = m.clone();
        let mut got = Some(());
        for _ in 0..frames {
            got = got.and_then(|()| m.physics_frame(&mut files, &mut NoPhysicsHooks));
        }
        let got = got.map(|()| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x}", fnv(changed))
        });
        if std::env::var("PHYSICS_DUMP").is_ok_and(|v| v == f[1]) {
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
    assert!(counts.get("ran").copied().unwrap_or(0) > 2000);
}
