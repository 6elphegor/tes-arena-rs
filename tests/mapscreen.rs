//! Differential tests of the travel map's screen (src/mapscreen.rs:
//! 2542:038F, 03DC, 26F7:0000's start and end, 0483, 0625, 03A4) against
//! vectors from the original code (harness/gen_mapscreen.py).
mod common;

use arena::bsa::Files;
use arena::clock::ClockHooks;
use arena::machine::Machine;
use arena::mapscreen::{MapHooks, MapStart};
use arena::mem::DS;
use arena::travelmap::TravelHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// What a case's stubs answer.
struct Answers {
    carry: bool,
    days: (u16, u16, u32),
    pick: u16,
    typed: Option<Vec<u8>>,
    known: u16,
    list: u16,
}

/// The stubs' log, as harness/gen_mapscreen.py keeps it.
struct Log {
    log: Vec<u32>,
    a: Answers,
    typed: Vec<u8>,
    inner: bool,
}

impl ClockHooks for Log {}

impl TravelHooks for Log {
    fn message_box(&mut self, m: &mut Machine, seg: u16, si: u16) {
        let text = m.mem.cstr(seg, si).to_vec();
        self.log.extend([1, si as u32, fnv(text) as u32]);
    }
    fn screen(&mut self, _m: &mut Machine) {
        self.log.push(3);
    }
    fn days(&mut self, m: &mut Machine, _files: &mut Files) -> Option<u16> {
        let (d, km, mins) = self.a.days;
        m.mem.w16(DS, 0x1927, d);
        m.mem.w16(DS, 0x3FF9, km);
        m.mem.w32(DS, 0x3FF2, mins);
        self.log.push(5);
        Some(d)
    }
    fn show_bank(&mut self, _m: &mut Machine, bx: u16) {
        self.log.extend([6, bx as u32]);
    }
    // (The label's 66F2 and 9E06, the markers' 9D84 and 5828 are stubbed
    // in gen_mapscreen.py.)
    fn label(&mut self, _m: &mut Machine, _above: u16) {}
    fn marker(&mut self, _m: &mut Machine, _marker: arena::travelmap::Marker) {}
    fn province(&mut self, _m: &mut Machine, _files: &mut Files, p: u8) -> Option<()> {
        self.log.extend([7, p as u32]);
        Some(())
    }
    fn bank(&mut self, _m: &mut Machine, bank: u16) {
        self.log.extend([8, bank as u32]);
    }
    fn pick_stub(&mut self, _m: &mut Machine, ax: u16) -> Option<u16> {
        if !self.inner {
            return None;
        }
        self.log.extend([9, ax as u32]);
        Some(self.a.pick)
    }
    fn journey_stub(&mut self, _m: &mut Machine) -> bool {
        if self.inner {
            self.log.push(10);
        }
        self.inner
    }
}

impl MapHooks for Log {
    fn quest_dungeon(&mut self, _m: &mut Machine, ax: u16) {
        self.log.extend([4, ax as u32]);
    }
    fn type_name(&mut self, m: &mut Machine) {
        self.log.push(11);
        for (i, &c) in self.typed.iter().chain(&[0]).enumerate() {
            m.mem.w8(DS, 0x9C62 + i as u16, c);
        }
    }
    fn list(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files, si: u16, cx: u16, _di: u16) -> u16 {
        self.log.extend([12, si as u32, cx as u32]);
        self.a.list
    }
}

/// Mirrors harness/gen_mapscreen.py `setup` (a conditional's test byte
/// comes first); returns the op and the stubs' answers.
fn setup(m: &mut Machine, seed: u32) -> (u8, Answers) {
    let mut st = Stream(seed);
    let s = &mut st;
    let letters = |s: &mut Stream, n: u8| -> Vec<u8> {
        (0..n).map(|i| if i == 0 { 0x41 + s.byte() % 26 } else { 0x61 + s.byte() % 26 }).collect()
    };
    let op = s.byte() % 7;
    let w16 = |m: &mut Machine, at: u16, v: u16| m.mem.w16(DS, at, v);
    let w8 = |m: &mut Machine, at: u16, v: u8| m.mem.w8(DS, at, v);
    let ws = |m: &mut Machine, at: u16, v: &[u8]| {
        for (i, &c) in v.iter().chain(&[0]).enumerate() {
            m.mem.w8(DS, at + i as u16, c);
        }
    };
    let v = s.word();
    w16(m, 0xC39A, v);
    let v = s.word();
    w16(m, 0xC398, v);
    let v = s.word();
    w16(m, 0x05C8, v);
    let f = s.byte();
    let v = if f < 0xA0 { 0 } else if f < 0xC0 { 1 } else { s.word() };
    w16(m, 0x0FFB, v);
    let v = s.word();
    w16(m, 0x0FCE, v);
    let v = s.byte();
    w8(m, 0xC994, v);
    let v = s.byte() % 9;
    w8(m, 0x0A46, v);
    let v = if s.byte() < 0x60 { 3 } else { s.byte() % 3 };
    w8(m, 0x0A47, v);
    let v = if s.byte() < 0x60 { 0x20 + s.byte() % 4 } else { s.byte() };
    w8(m, 0x0A4C, v);
    let v = s.word() % 320;
    w16(m, 0x0A48, v);
    let v = s.word() % 200;
    w16(m, 0x0A4A, v);
    let v = s.byte() % 9;
    w8(m, 0x47E1, v);
    let n = 1 + s.byte() % 15;
    let v = letters(s, n);
    ws(m, 0x1929, &v);
    let n = 1 + s.byte() % 12;
    let v = letters(s, n);
    ws(m, 0x1429, &v);
    let v = s.byte() % 9;
    w8(m, 0x193D, v);
    let v = s.byte() % 4;
    w8(m, 0x1943, v);
    let v = if s.byte() < 0x40 { 8 } else { (s.byte() % 9) as u16 };
    w16(m, 0x12A8, v);
    let v = s.byte() & 1;
    w8(m, 0x6C28, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 };
    w8(m, 0x6C41, v);
    let v = (s.byte() % 0x30) as u16;
    w16(m, 0xA8AE, v);
    let v = (s.byte() % 0x30) as u16;
    w16(m, 0xA8BC, v);
    let v = s.byte();
    w8(m, 0xA8C4, v);
    let v = (s.byte() % 60) as u16;
    w16(m, 0x1927, v);
    let v = s.word();
    w16(m, 0x3FF9, v);
    let v = (s.word() as u32) << 4;
    m.mem.w32(DS, 0x3FF2, v);
    let v = (s.byte() & 1) as u16;
    w16(m, 0x12A2, v);
    let v = if s.byte() < 0x60 { 2 } else { s.byte() % 4 };
    w8(m, 0x05E6, v);
    let hi = s.word() as u32;
    let lo = s.byte() as u32;
    m.mem.w32(DS, 0x05CC, hi << 8 | lo);
    let v = s.word() % 320;
    w16(m, 0x8F92, v);
    let v = s.word() % 200;
    w16(m, 0x8F94, v);
    let v = if s.byte() < 0x80 { 0 } else { 1 };
    w8(m, 0xAFF9, v);
    let v = s.byte() as u16;
    w16(m, 0x1032, v);
    let v = s.byte();
    w8(m, 0x6636, v);
    let v = if s.byte() < 0x80 { 0 } else { s.byte() };
    w8(m, 0x1A81, v);
    for i in 0..0x200u16 {
        w8(m, 0x9C62 + i, (i.wrapping_mul(37).wrapping_add(11) as u8) | 1);
    }
    for i in 0..32u16 {
        let rec = 0x1445 + i * 0x19;
        let n = 1 + s.byte() % 10;
        let v = letters(s, n);
        ws(m, rec, &v);
        let x = s.word() % 320;
        let y = s.word() % 200;
        w16(m, rec + 0x14, x);
        w16(m, rec + 0x16, y);
        let v = s.byte();
        w8(m, rec + 0x18, v);
    }
    let carry = s.byte() & 1 != 0;
    let d = (s.byte() % 50) as u16;
    let km = s.word();
    let mins = (s.word() as u32) << 4;
    let pick = if s.byte() < 0x40 { 0xFFFF } else { (s.byte() % 0x30) as u16 };
    let from = s.byte();
    let typed = if from < 0x50 {
        None
    } else if from < 0xA0 {
        Some(Vec::new())
    } else {
        let n = 1 + s.byte() % 8;
        Some(letters(s, n))
    };
    let known = (s.byte() % 32) as u16;
    let f = s.byte();
    let list = if f < 0x30 { 0xFFFF } else if f < 0x40 { 0x8000 | s.byte() as u16 } else { (s.byte() % 32) as u16 };
    (op, Answers { carry, days: (d, km, mins), pick, typed, known, list })
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
    for line in include_str!("vectors/mapscreen.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "mapscreen", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (op, a) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        m.load_rng();
        let typed = match &a.typed {
            Some(t) => t.clone(),
            None => m.mem.cstr(DS, 0x1445 + a.known * 0x19).to_ascii_lowercase(),
        };
        let before = m.clone();
        let mut kept = MapStart { id: m.mem.r8(DS, 0x0A4C), kind: m.mem.r8(DS, 0x0A47), province: m.mem.r8(DS, 0x0A46) };
        if a.carry {
            let k = a.known;
            if k & 1 == 0 {
                kept.id = k as u8;
            }
            if k & 2 == 0 {
                kept.kind = (a.pick & 3) as u8;
            }
            if k & 4 == 0 {
                kept.province = (a.list % 9) as u8;
            }
        }
        let carry = a.carry;
        let mut hooks = Log { log: Vec::new(), a, typed, inner: op == 4 || op == 5 };
        let mut extra = String::new();
        let ok = match op {
            0 => {
                if m.map_button(&mut hooks) {
                    hooks.log.push(2);
                    m.map_button_done(carry);
                }
                Some(())
            }
            1 => {
                m.travel_map_begin(&mut hooks);
                Some(())
            }
            2 => {
                let c = m.travel_map_end(&mut hooks, kept);
                extra = format!(" {}", c as u8);
                Some(())
            }
            3 => m.travel_journey(&mut fs, &mut hooks),
            4 => m.travel_click(&mut fs, &mut hooks),
            5 => m.travel_search(&mut fs, &mut hooks),
            _ => {
                let c = !m.map_from_here(&mut hooks);
                extra = format!(" {}", c as u8);
                Some(())
            }
        };
        if std::env::var("DUMP").ok().as_deref() == Some(f[1]) {
            for o in (0..=0xFFFFu16).filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o)) {
                eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
            }
        }
        let got = match ok {
            Some(()) => {
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                let log = hooks.log.iter().flat_map(|v| v.to_le_bytes());
                format!("{:x} {:x}{extra}", fnv(changed), fnv(log))
            }
            None => "stuck".to_string(),
        };
        assert_eq!(f[3], got, "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} compared");
}
