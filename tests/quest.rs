//! Differential tests of the quests' clock (src/quest.rs `quests_hourly`,
//! 1194:98E8, and `quests_daily`, 1194:99F8) against vectors from the
//! original code (harness/gen_quest.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const DAY: i64 = 0x5A0;
const OFFSETS: [i64; 9] = [-DAY - 1, -DAY, -DAY + 1, -1, 0, 1, DAY - 1, DAY, DAY + 1];

/// Mirrors harness/gen_quest.py `setup` (a conditional's test byte comes
/// first, a list's items before its index); returns the op.
fn setup(m: &mut Machine, seed: u32) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() & 1;
    let hi = s.byte() as u32;
    let t = hi << 16 | s.word() as u32;
    m.mem.w32(DS, 0x05CC, t);
    let mut items = [0u8; 9];
    for it in items.iter_mut() {
        *it = if s.byte() < 0x60 { 0xFF } else { s.byte() % 0x50 };
    }
    for (i, &b) in items.iter().enumerate() {
        m.mem.w8(DS, 0x0FD4 + i as u16, b);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x0FDD, v);
    let mut record = |m: &mut Machine, s: &mut Stream, at: u16, size: u16| {
        for i in 0..size {
            let b = s.byte();
            m.mem.w8(DS, at + i, b);
        }
        let flags = s.word();
        let kind = [1u16, 1, 1, 0, 2, 3][s.byte() as usize % 6];
        let stage = s.byte() as u16 % 8;
        m.mem.w16(DS, at + 4, flags & 0xF13F | kind << 6 | stage << 9);
        let k = s.byte() % 11;
        let d = if k < 9 {
            t as i64 + OFFSETS[k as usize]
        } else if k == 9 {
            t as i64 + (s.word() as i64 - 0x8000) * 16
        } else {
            s.byte() as i64
        };
        m.mem.w32(DS, at + 0xC, d as u32);
        let item = if s.byte() < 0x80 { items[s.byte() as usize % 9] } else { s.byte() };
        m.mem.w8(DS, at + 0x1C, item);
    };
    for i in 0..4 {
        record(m, s, 0x0E6E + i * 0x34, 0x34);
    }
    for i in 0..4 {
        record(m, s, 0x0DCA + i * 0x29, 0x29);
    }
    for i in 0..8u16 {
        let first = if s.byte() < 0x40 { 0xFF } else { s.byte() };
        m.mem.w8(DS, 0x0A7E + i * 6, first);
        for k in 1..6 {
            let b = s.byte();
            m.mem.w8(DS, 0x0A7E + i * 6 + k, b);
        }
    }
    let v = if s.byte() < 0x80 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x1013, v);
    let v = s.byte();
    m.mem.w8(DS, 0x8A56, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0D39, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    for i in 0..0x4CCu16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1429 + i, b);
    }
    op
}

/// Logs what the generator's stubs log.
#[derive(Default)]
struct Log(Vec<u16>);

impl QuestHooks for Log {
    fn text(&mut self, m: &mut Machine) {
        self.0.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16]);
    }
    fn province_load(&mut self, _m: &mut Machine, province: u8) {
        self.0.extend([2, province as u16]);
    }
    fn province_save(&mut self, _m: &mut Machine, province: u16) {
        self.0.extend([3, province]);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/quest.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "quest", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let mut log = Log::default();
        if op == 0 {
            m.quests_hourly(&mut log);
        } else {
            m.quests_daily(&mut log);
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let lg = log.0.iter().flat_map(|v| v.to_le_bytes());
        assert_eq!(f[3], format!("{:x} {:x}", fnv(changed), fnv(lg)), "{line}");
        n += 1;
    }
    assert!(n > 3500, "{n}");
}
