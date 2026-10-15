//! Differential tests of the arrival's text (src/travelmap.rs
//! `travel_arrival`, 26F7:08F7) against vectors from the original code
//! (harness/gen_arrival.py).
mod common;

use arena::bsa::Files;
use arena::clock::ClockHooks;
use arena::machine::Machine;
use arena::mem::DS;
use arena::travelmap::TravelHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The stubs, as harness/gen_arrival.py has them.
struct Log {
    log: Vec<u32>,
    text: Vec<u8>,
}

impl ClockHooks for Log {}

impl TravelHooks for Log {
    fn arrival_text(&mut self, m: &mut Machine, _files: &mut Files, di: u16) -> Option<()> {
        self.log.extend([1, m.mem.r16(DS, 0x137A) as u32]);
        for (i, &c) in self.text.iter().chain(&[0]).enumerate() {
            m.mem.w8(DS, di.wrapping_add(i as u16), c);
        }
        Some(())
    }
    fn arrival_delete(&mut self, _m: &mut Machine, _files: &mut Files, si: u16) {
        self.log.extend([2, si as u32]);
    }
    fn arrival_write(&mut self, m: &mut Machine, _files: &mut Files, si: u16, di: u16, cx: u16) {
        let h = fnv((0..cx).map(|i| m.mem.r8(DS, di.wrapping_add(i))));
        self.log.extend([3, si as u32, cx as u32, h as u32]);
    }
}

/// Mirrors harness/gen_arrival.py `setup` (a conditional's test byte
/// comes first); returns the stubbed text.
fn setup(m: &mut Machine, seed: u32) -> Vec<u8> {
    let mut st = Stream(seed);
    let s = &mut st;
    for i in 0..0x400u16 {
        let b = s.byte() | 1;
        m.mem.w8(DS, 0x10A8 + i, b);
    }
    for i in 0..0x80u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x9C62 + i, b);
    }
    let kind = if s.byte() < 0x20 { 3 } else { s.byte() % 3 };
    m.mem.w8(DS, 0x1943, kind);
    let v = s.byte();
    m.mem.w8(DS, 0x0A4C, v);
    let v = if s.byte() < 0xE0 { s.byte() % 9 } else { s.byte() };
    m.mem.w8(DS, 0x0A46, v);
    let v = if s.byte() < 0x40 { 8 } else { s.byte() % 9 };
    m.mem.w8(DS, 0x47E1, v);
    let letters = |s: &mut Stream, n: u8| -> Vec<u8> { (0..n).map(|_| 0x61 + s.byte() % 26).collect() };
    for (at, len) in [(0x1929u16, 20u8), (0x1429, 13)] {
        let n = s.byte() % len;
        let mut v = letters(s, n);
        v.push(0);
        for (i, &c) in v.iter().enumerate() {
            m.mem.w8(DS, at + i as u16, c);
        }
    }
    let hi = s.word() as u32;
    let lo = s.byte() as u32;
    m.mem.w32(DS, 0x05CC, hi << 8 | lo);
    let v = if s.byte() < 0x40 { s.word() } else { (s.byte() % 60) as u16 };
    m.mem.w16(DS, 0x1927, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    let v = s.byte();
    m.mem.w8(DS, 0x101C, v);
    let v = s.byte();
    m.mem.w8(DS, 0x137E, v);
    let n = if s.byte() < 0xE0 { s.byte() % 40 } else { 40 + s.byte() % 40 };
    let mut text = Vec::new();
    for _ in 0..n {
        let k = if s.byte() < 0xF0 { 1 + s.byte() % 12 } else { 30 + s.byte() % 40 };
        text.extend(letters(s, k));
        let sep = s.byte();
        text.push(if sep < 0x20 { b'\r' } else if sep < 0x30 { b'\n' } else { b' ' });
    }
    text
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: game files not found");
        return;
    };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/arrival.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "arrival", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let text = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let mut hooks = Log { log: Vec::new(), text };
        let got = match m.travel_arrival(&mut fs, &mut hooks) {
            Some(()) => {
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                let log = hooks.log.iter().flat_map(|v| v.to_le_bytes());
                format!("{:x} {:x}", fnv(changed), fnv(log))
            }
            None => "stuck".to_string(),
        };
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} compared");
}
