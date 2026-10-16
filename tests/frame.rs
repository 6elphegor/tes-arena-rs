//! Differential tests of the world loop's frame body (src/frame.rs
//! `frame_body`, 0000:481E..4CE7) against vectors from the original code
//! (harness/gen_frame.py).
mod common;

use arena::bsa::Files;
use arena::clock::ClockHooks;
use arena::frame::FrameHooks;
use arena::machine::Machine;
use arena::mem::DS;
use arena::weapon::WeaponHooks;
use common::blow::{put, Stream};

const WSEG: u16 = 0x6800;
const CSEG: u16 = 0x7800;
const TSEG: u16 = 0x5C00;
const HSEG: u16 = 0x6000;
const KEYS: [u16; 11] = [0x0D, 0x0C, 0x3E, 0x3C, 0x3D, 0x31, 0x38, 0x3F, 0x30, 0x10, 0x2D];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's hooks: calls counted at FFE0..FFEE, the quest's
/// carry from FFDE.
struct Hooks;

impl WeaponHooks for Hooks {
    fn wall_trigger(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFF4);
        Some(())
    }
    fn building_closed(&mut self, m: &mut Machine, _kind: u8) -> Option<bool> {
        Some(m.mem.r8(DS, 0xFFF8) & 1 != 0)
    }
    fn fire_bow(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFF6);
        Some(())
    }
}

impl ClockHooks for Hooks {
    // The generator stubs the encounters and the quests (gen_clock.py's
    // stubs), the level-up screen and the exhaustion.
    fn exhausted(&mut self, _m: &mut Machine) -> Option<bool> {
        Some(false)
    }
    fn level_up_screen(&mut self, _m: &mut Machine, _levels: u16) -> Option<()> {
        Some(())
    }
    fn encounter(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_daily(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_hourly(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        Some(())
    }
    fn encounter_minute(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
}

impl FrameHooks for Hooks {
    /// The generator's 0000:3491 is a RET.
    fn automap_mark(&mut self, _m: &mut Machine) {}
    /// The generator's 2294:04F5 is a RETF.
    fn area_music(&mut self, _m: &mut Machine) {}
    fn message_box(&mut self, m: &mut Machine, _text: u16) {
        count(m, 0xFFE8);
    }
    fn message_box_number(&mut self, m: &mut Machine, _files: &mut Files) {
        count(m, 0xFFE0);
    }
    fn guards_arrive(&mut self, m: &mut Machine, _files: &mut Files, hook: u32) -> Option<()> {
        match hook {
            0x3F0C => count(m, 0xFFEA),
            0x3F3B => count(m, 0xFFEC),
            _ => return None,
        }
        Some(())
    }
    fn actor_click(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFE4);
        Some(())
    }
    fn townsfolk_graphics(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFEE);
        Some(())
    }
    fn menu_keys(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFE2);
        Some(())
    }
    fn quest_carry(&mut self, m: &mut Machine) -> bool {
        m.mem.r8(DS, 0xFFDE) & 1 != 0
    }
    fn key_d327(&mut self, m: &mut Machine) {
        count(m, 0xFFE6);
    }
    fn level_text_draw(&mut self, m: &mut Machine) {
        count(m, 0xFFD0);
    }
    fn riddle(&mut self, m: &mut Machine) {
        count(m, 0xFFD2);
    }
}

/// Mirrors harness/gen_frame.py `setup`.
fn setup(m: &mut Machine, seed: u32, holidays: &[u16]) {
    let mut st = Stream(seed);
    let s = &mut st;
    for scan in KEYS {
        let v = (s.byte() < 0x30) as u8;
        m.mem.w8(DS, 0xACB5 + scan, v);
        let v = (s.byte() < 0x40) as u8;
        m.mem.w8(DS, 0x9208 + scan, v);
    }
    let v = s.byte() % 11;
    m.mem.w8(DS, 0x37E9, v);
    let v = s.byte() & 3;
    m.mem.w8(DS, 0xAFF9, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 };
    m.mem.w8(DS, 0x05E3, v);
    for (at, mask) in [(0x7A46u16, 0x1FFu16), (0x05CA, 0x721E), (0x05E0, 0x2554), (0xC39A, 0x4211), (0x8848, 0x0010)] {
        let v = s.word() & mask;
        m.mem.w16(DS, at, v);
    }
    let k = s.byte();
    let v = if k < 0x20 { 0xFF } else { s.byte() % 8 };
    m.mem.w8(DS, 0xC942, v);
    let v = s.word() & 0x0184;
    m.mem.w16(DS, 0x05BE, v);
    m.mem.w16(DS, 0x1020, 0);
    m.mem.w16(DS, 0x1022, TSEG);
    let v = (s.byte() % 8) as u16;
    m.mem.w16(TSEG, 0x22, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01A9, v);
    let v = if s.byte() < 0xE0 { s.byte() % 10 } else { 0xFF };
    m.mem.w8(DS, 0x0D49, v);
    let v = s.byte() % 10;
    m.mem.w8(DS, 0x1012, v);
    let v = if s.byte() < 0xE0 { 0xFF } else { s.byte() % 8 };
    m.mem.w8(DS, 0x7F80, v);
    let v = if s.byte() < 0x80 { 0x20 } else { (s.byte() % 0x24) as u16 };
    m.mem.w16(DS, 0x1947, v);
    let v = if s.byte() < 0x80 { 0 } else { s.byte() as u16 };
    m.mem.w16(DS, 0x061E, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01D1, v);
    let v = s.byte();
    m.mem.w8(DS, 0x05BD, v);
    let v = if s.byte() < 0xA0 { 0 } else { (s.byte() % 9) as u16 };
    m.mem.w16(DS, 0x0FFB, v);
    let v = s.byte() % 4;
    m.mem.w8(DS, 0xC936, v);
    m.mem.w16(DS, 0xC38E, 0xC0);
    for i in 0..23u16 {
        let si = 0x0AB3 + 0x1C * i;
        let v = if s.byte() < 0x20 { 0xC5 } else { 0xC0 + s.byte() % 5 };
        m.mem.w8(DS, si + 0xC, v);
        let v = s.word() & 0x7FFF;
        m.mem.w16(DS, si + 0xF, v);
        m.mem.w16(DS, si + 0x12, 0x41E * (i % 8));
    }
    for k in 0..8u16 {
        let v = s.byte();
        m.mem.w8(CSEG, 0x41E * k + 0x2D, v);
    }
    let k = s.byte();
    let v = if k < 0x80 {
        0
    } else if k < 0xC0 {
        1
    } else {
        s.byte() % 4
    };
    m.mem.w8(DS, 0x680F, v);
    let v = if s.byte() < 0x80 { 0x3F0C } else { 0x3F3B };
    m.mem.w16(DS, 0x67EA, v);
    m.mem.w16(DS, 0x67EC, 0);
    let v = if s.byte() < 0x18 { 0x40 } else { 0 };
    m.mem.w16(DS, 0x05B1, v);
    let v = (s.byte() % 8) as u16;
    m.mem.w16(DS, 0xCCBC, v);
    let day = if s.byte() < 0x60 { holidays[s.byte() as usize % holidays.len()] as u32 } else { (s.word() % 360) as u32 };
    let year = (s.byte() % 30) as u32;
    let minute = (s.word() % 1440) as u32;
    m.mem.w32(DS, 0x05CC, year * 518_400 + day * 1440 + minute);
    let v = s.byte() & 7;
    m.mem.w8(DS, 0x05E4, v);
    let v = if s.byte() < 0x80 { 2 } else { s.byte() % 4 };
    m.mem.w8(DS, 0xCB5B, v);
    let v = if s.byte() < 0x80 { 0 } else { (s.byte() % 8) as u16 };
    m.mem.w16(DS, 0x05C0, v);
    let v = (s.byte() % 12) as u16;
    m.mem.w16(DS, 0x01FD, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 };
    m.mem.w16(DS, 0x0FF7, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 };
    m.mem.w16(DS, 0x0FF9, v);
    let v = if s.byte() < 0xE0 { 0 } else { 1 };
    m.mem.w8(DS, 0x1013, v);
    let v = s.word() & 4;
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() % 0x400;
    m.mem.w16(DS, 0xCCFE, v);
    let b: Vec<u8> = (0..4).map(|_| s.byte() % 8).collect();
    put(m, DS, 0x0604, &b);
    let v = s.byte() % 4;
    m.mem.w8(DS, 0x392D, v);
    let b: Vec<u8> = (0..4).map(|_| s.byte() % 8).collect();
    put(m, DS, 0x392E, &b);
    let v = s.word() & 0x0100;
    m.mem.w16(DS, 0x05C8, v);
    let v = if s.byte() < 0x20 { 1 } else { 0 };
    m.mem.w8(DS, 0x191F, v);
    let t = s.word();
    m.mem.w16(DS, 0x0151, t);
    let d = if s.byte() < 0x80 { 0x5A } else { (s.byte() % 0x5A) as u16 };
    m.mem.w16(DS, 0x884A, t.wrapping_sub(d));
    let v = if s.byte() < 0x80 { 0 } else { 0x1044 };
    m.mem.w16(DS, 0x194B, v);
    for i in 0..0x10u16 {
        let v = ((s.byte() % 5) as u16).wrapping_sub(2);
        m.mem.w16(DS, 0x1044 + 2 * i, v);
    }
    let v = (s.byte() % 16) as u16;
    m.mem.w16(DS, 0x1949, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0xFFDE, v);
    for at in [0xC3D0u16, 0xC3D4] {
        let v = 0x1000 + s.word() % 0x2000;
        m.mem.w16(DS, at, v);
    }
    let cb90 = s.word() & 0x7EFE;
    m.mem.w16(DS, 0xCB90, cb90);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x91F6, &b);
    for _ in 0..s.byte() % 24 {
        let k = s.byte();
        let at = s.word() & 0x7FFE;
        let w = s.word() & 0x0F00;
        let t = if k < 0xC0 { 0x8000 } else { 0x9000 };
        m.mem.w16(WSEG, at, t | w | (k % 10) as u16);
    }
    // A level's text shown (0000:0550).
    let v = if s.byte() < 0x60 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x3C82, v);
    let v = if s.byte() < 0x80 { cb90 } else { s.word() & 0x7EFE };
    m.mem.w16(DS, 0x0DBC, v);
    let txt = if s.byte() < 0xC0 { s.word() } else { 0 };
    m.mem.w16(DS, 0x3D42, txt);
    let v = if s.byte() < 0x60 { txt } else { s.word() };
    m.mem.w16(DS, 0x7C5E, v);
    let v = s.word();
    m.mem.w16(DS, 0x102C, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC8FE, v);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    for (at, v) in [(0x902Eu16, 0u16), (0x9030, HSEG), (0xBA54, 0), (0xBA56, 0x6400), (0xD806, WSEG), (0x1042, CSEG), (0x91F0, 0), (0xCCD0, 0x7000), (0xC357, 0x80), (0xC359, 0x80), (0xCCCE, HSEG + 0xC00)] {
        base.mem.w16(DS, at, v);
    }
    let mut holidays = vec![];
    let mut at = 0x33ADu16;
    while base.mem.r16(DS, at) <= 0x158 {
        holidays.push(base.mem.r16(DS, at));
        at += 2;
    }
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/frame.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "frame", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap(), &holidays);
        m.load_rng();
        let before = m.clone();
        let got = m.frame_body(&mut files, &mut Hooks).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o)))
            )
        });
        if std::env::var("FRAME_DUMP").is_ok_and(|v| v == f[1]) {
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
