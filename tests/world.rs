//! Differential tests of the world loop's frame (src/engine.rs
//! `Engine::frame`, 0000:A2AC..A453): its order and its own state, its
//! big steps stood in for by a log, against vectors from the original
//! code (harness/gen_world.py).
mod common;

use arena::bsa::Files;
use arena::engine::{Engine, EngineHooks, Input, Step};
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const HOOK_SEG: u16 = 0x5000;
const LOG: u16 = 0xFE00;
const COUNT: u16 = 0xFDFE;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_world.py `setup` (a conditional's test byte comes
/// first, a tuple's items before its index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    for at in [0xC3D0, 0xC3D4, 0xBA62, 0xBA64, 0xBA66] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = if s.byte() < 0x40 { s.word() } else { 0x3C };
    m.mem.w16(DS, 0xC3D2, v);
    let v = if s.byte() < 0xE0 { s.word() & 0x1FF } else { s.word() };
    m.mem.w16(DS, 0xC3DA, v);
    let v = if s.byte() < 0xE0 { (s.byte() % 8) as u16 } else { s.word() };
    m.mem.w16(DS, 0xC394, v);
    let w = s.word() & !0x80;
    let v = w | if s.byte() < 0x20 { 0x80 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0xA0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0xCB5C, v);
    let v = if s.byte() < 0x80 { 1 } else { s.byte() };
    m.mem.w8(DS, 0xCB5B, v);
    let any = s.byte();
    let v = [0, 0x81, 0x82, any][s.byte() as usize % 4];
    m.mem.w8(DS, 0xBB09, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC8F0, v);
    let v = if s.byte() < 0x80 { 0 } else { s.word() };
    m.mem.w16(DS, 0xCB84, v);
    for base in [0xBA58u16, 0xA914, 0xC39C] {
        for i in 0..4 {
            let v = s.word();
            m.mem.w16(DS, base + 2 * i, v);
        }
    }
    for at in [0xCB86, 0x0151, 0xBA60, 0xCCBC, 0xBAC8, 0xCD7A, 0xCB5E, 0xC3DE, 0xC3E0] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0xBA4B, v);
    for _ in 0..6 {
        let k = s.byte() as u16;
        let v = s.word();
        m.mem.w16(DS, 0xD330 + 2 * (k & 0x7F), v);
        let k = s.byte() as u16;
        let v = s.word();
        m.mem.w16(DS, 0xD440 + 6 * (k & 0x7F), v);
    }
    for (at, off) in [(0xC3A4, 0x000u16), (0xCD16, 0x040), (0xCD66, 0x080), (0xCD1A, 0x0C0), (0xC3A8, 0x100)] {
        let v = if s.byte() < 0xC0 { (HOOK_SEG as u32) << 16 | off as u32 } else { 0 };
        m.mem.w32(DS, at, v);
    }
    m.mem.w16(DS, COUNT, 0);
}

/// The steps stood in for, logged as the original's stubs log them.
struct Log;

impl Log {
    fn log(m: &mut Machine, n: u8, v: u16) {
        let i = m.mem.r16(DS, COUNT);
        m.mem.w8(DS, LOG.wrapping_add(i), n);
        m.mem.w16(DS, LOG.wrapping_add(i).wrapping_add(1), v);
        m.mem.w16(DS, COUNT, i.wrapping_add(3));
    }
}

impl arena::hooks::StairsHooks for Log {}
impl arena::controls::ControlHooks for Log {}
impl arena::frame::FrameHooks for Log {}
impl arena::clock::ClockHooks for Log {}
impl arena::weapon::WeaponHooks for Log {}
impl EngineHooks for Log {
    fn step(&mut self, m: &mut Machine, step: Step) -> bool {
        let x = m.mem.r16(DS, 0xC3D0);
        let (n, v) = match step {
            Step::Actors => (1, x),
            Step::WalkBob => (2, x),
            Step::LightsNear => (3, x),
            Step::Textures(si) => (4, si),
            Step::MapBank(bx) => (5, bx),
            Step::MovingLights => (6, x),
            Step::DrawSetup(a) => (7, a),
            Step::View => (8, m.mem.r16(DS, 0xC394)),
            Step::DrawLists => (9, x),
            Step::DrawFlats => (10, x),
            Step::Snow => (11, x),
            Step::Rain => (12, x),
            Step::Physics => (13, x),
            Step::TempWalls => (14, x),
            Step::InfoLoad => (15, x),
            Step::Present => (16, x),
            Step::HookCD16 => (17, x),
            Step::HookCD66 => (18, x),
            Step::Sky => (19, x),
            Step::FrameBody => (20, x),
        };
        Log::log(m, n, v);
        // The physics' stand-in moves the eye's x on one, the frame body's
        // the window's bottom, as the generator's do.
        let moves = match step {
            Step::Physics => Some(0xC3D0),
            Step::FrameBody => Some(0xC3A2),
            _ => None,
        };
        if let Some(at) = moves {
            let v = m.mem.r16(DS, at).wrapping_add(1);
            m.mem.w16(DS, at, v);
        }
        true
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(files) = Files::new(&common::game_dir()) else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/world.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "world", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let mut e = Engine::new(m, files.clone());
        let got = e.frame(Input::Classic, &mut Log).map(|_| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| e.m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, e.m.mem.r8(DS, o)]);
            format!("{:x}", fnv(changed))
        });
        match got {
            Some(g) => assert_eq!(f[2], g, "{line}"),
            None => assert_eq!(f[2], "stuck", "{line}"),
        }
        n += 1;
    }
    assert!(n > 2500, "{n} compared");
}
