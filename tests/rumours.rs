//! Differential tests of the tavern's rumours (src/tavern.rs
//! `tavern_rumours`, 1194:E3F7) against vectors from the original code
//! (harness/gen_rumours.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::shops::ShopHooks;
use arena::tavern::TavernHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The menu's answer, and the log harness/gen_rumours.py's stubs keep.
struct Script {
    answer: u16,
    log: Vec<u32>,
}

impl ShopHooks for Script {
    fn menu(&mut self, _m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([8, si as u32, bx as u32, di as u32]);
        self.answer
    }
}

impl TavernHooks for Script {
    fn text(&mut self, m: &mut Machine, _files: &mut Files, mode: u16) -> Option<bool> {
        self.log.extend([
            16,
            m.mem.r16(DS, 0x137A) as u32,
            mode as u32,
            0x1044,
            m.mem.r8(DS, 0x137E) as u32,
            m.mem.r8(DS, 0x101C) as u32,
            m.mem.r16(DS, 0xB038) as u32,
        ]);
        Some(true)
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.log.extend([10, si as u32, m.mem.r8(DS, 0x9288) as u32]);
    }
}

/// Mirrors harness/gen_rumours.py `setup`, draw for draw; returns the
/// menu's answer.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = s.word();
    m.mem.w16(DS, 0x0151, v);
    for i in 0..4u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x05CC + i, b);
    }
    let v = s.word();
    m.mem.w16(DS, 0xC3D0, v);
    for i in 0..0x34u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x0F3E + i, b);
    }
    let mut q = s.word();
    if s.byte() < 0x40 {
        q &= !0xC0;
    }
    m.mem.w16(DS, 0x0F3E + 4, q);
    for i in 0..4u16 {
        let mut v = s.word();
        if s.byte() < 0x40 {
            v = v & !0xC0 | 0x80;
        }
        m.mem.w16(DS, 0x0E6E + 0x34 * i + 4, v);
    }
    let v = s.word();
    m.mem.w16(DS, 0x1036, v);
    for at in [0x9288u16, 0x137E, 0x101C] {
        let b = s.byte();
        m.mem.w8(DS, at, b);
    }
    [0xFFFF, 0, 1, 0, 1][(s.byte() % 5) as usize]
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Some(dir) = std::env::var_os("ARENA_EXE").map(|p| std::path::PathBuf::from(p).parent().unwrap().to_path_buf()) else {
        return;
    };
    let mut files = Files::new(&dir).expect("game directory");
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/rumours.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "rumours", "{line}");
        let mut m = base.clone();
        let answer = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let mut hooks = Script { answer, log: Vec::new() };
        let got = m.tavern_rumours(&mut files, &mut hooks).map(|_| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv(hooks.log.iter().flat_map(|v| v.to_le_bytes())))
        });
        match got {
            Some(g) => assert_eq!(f[2], g, "{line}"),
            None => assert_eq!(f[2], "stuck", "{line}"),
        }
        n += 1;
    }
    assert!(n > 2500, "{n}");
}
