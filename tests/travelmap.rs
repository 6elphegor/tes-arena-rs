//! Differential tests of the travel map (src/travelmap.rs: 26F7:0B7A,
//! 03C4, 07CD) against vectors from the original code
//! (harness/gen_travelmap.py).
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

/// The stubs' log, as harness/gen_travelmap.py keeps it.
struct Log {
    log: Vec<u32>,
    question: u16,
    days: u16,
    stored: u16,
}

impl Log {
    fn put(&mut self, v: &[u32]) {
        self.log.extend_from_slice(v);
    }
}

impl ClockHooks for Log {}

impl TravelHooks for Log {
    fn message_box(&mut self, _m: &mut Machine, seg: u16, si: u16) {
        self.put(&[1, si as u32, (seg != DS) as u32]);
    }
    fn question(&mut self, _m: &mut Machine, si: u16) -> u16 {
        self.put(&[2, si as u32]);
        self.question
    }
    fn screen(&mut self, _m: &mut Machine) {
        self.put(&[3]);
    }
    fn bank(&mut self, _m: &mut Machine, bank: u16) {
        self.put(&[4, bank as u32]);
    }
    fn resource(&mut self, _m: &mut Machine, _files: &mut Files, si: u16, bx: u16) -> Option<()> {
        self.put(&[5, si as u32, bx as u32]);
        Some(())
    }
    fn days(&mut self, m: &mut Machine, _files: &mut Files) -> Option<u16> {
        m.mem.w16(DS, 0x1927, self.stored);
        self.put(&[6]);
        Some(self.days)
    }
    fn show_bank(&mut self, _m: &mut Machine, bx: u16) {
        self.put(&[7, bx as u32]);
    }
    fn animation(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        self.put(&[8]);
        Some(())
    }
    fn animation_frame(&mut self, _m: &mut Machine, frame: u8) {
        self.put(&[9, frame as u32]);
    }
    fn advance(&mut self, _m: &mut Machine, _files: &mut Files, minutes: u32) -> Option<bool> {
        self.put(&[10, minutes]);
        Some(true)
    }
    fn province(&mut self, _m: &mut Machine, _files: &mut Files, p: u8) -> Option<()> {
        self.put(&[11, p as u32]);
        Some(())
    }
    fn location(&mut self, _m: &mut Machine, _files: &mut Files, id: u16) {
        self.put(&[12, id as u32]);
    }
    fn dungeon_location(&mut self, _m: &mut Machine, _files: &mut Files, id: u16, bx: u16) {
        self.put(&[13, id as u32, bx as u32]);
    }
    fn arrival_log(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<bool> {
        self.put(&[14]);
        Some(true)
    }
    fn close(&mut self, _m: &mut Machine) {
        self.put(&[15]);
    }
}

/// Mirrors harness/gen_travelmap.py `setup`, draw for draw (a
/// conditional's test byte first, a tuple's items before its index).
/// Returns the op and the answers (question, days).
fn setup(m: &mut Machine, seed: u32) -> (u8, u16, u16, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 3;
    let w8 = |m: &mut Machine, at: u16, v: u8| m.mem.w8(DS, at, v);
    let w16 = |m: &mut Machine, at: u16, v: u16| m.mem.w16(DS, at, v);
    let mut xy = Vec::new();
    for i in 0..48u16 {
        let rec = 0x1445 + 0x19 * i;
        let len = 1 + s.byte() as u16 % 12;
        for k in 0..len {
            let c = 0x41 + s.byte() % 26;
            w8(m, rec + k, c);
        }
        w8(m, rec + len, 0);
        let x = s.word() % 0x140;
        let y = s.word() % 0xC8;
        xy.push((x, y));
        w16(m, rec + 0x14, x);
        w16(m, rec + 0x16, y);
        let v = s.byte();
        w8(m, rec + 0x18, v);
    }
    let shown = if s.byte() < 0xE0 { s.byte() % 9 } else { s.byte() };
    w8(m, 0x47E1, shown);
    let v = s.word() % 0x140;
    w16(m, 0x8F92, v);
    let v = s.word() % 0xC8;
    w16(m, 0x8F94, v);
    let k = s.byte();
    let pick = if k < 0x80 {
        s.byte() as u16 % 0x20
    } else if k < 0xF0 {
        0x20 + s.byte() as u16 % 0x10
    } else {
        s.word()
    };
    w16(m, 0xA8AE, pick);
    let near = s.byte() as usize % 48;
    w16(m, 0xA8BA, 0x1445 + 0x19 * near as u16);
    let v = s.byte() % 9;
    w8(m, 0x0A46, v);
    let v = s.byte() % 4;
    w8(m, 0x0A47, v);
    let v = s.byte();
    w8(m, 0x0A4C, v);
    let v = s.byte() % 9;
    w8(m, 0x0A22 + 0x24, v);
    for i in 0..4 {
        let v = s.byte();
        w8(m, 0x0A22 + 0x26 + i, v);
    }
    let v = s.word();
    w16(m, 0xC39A, v);
    let v = s.word() % 0x140;
    w16(m, 0x143D, v);
    let v = s.word() % 0xC8;
    w16(m, 0x143F, v);
    let v = s.word() % 0x140;
    w16(m, 0x1441, v);
    let v = s.word() % 0xC8;
    w16(m, 0x1443, v);
    for i in 0..0x14 {
        let v = s.byte();
        w8(m, 0x1929 + i, v);
    }
    for at in [0x193D, 0x1942, 0x1943, 0x1944] {
        let v = s.byte();
        w8(m, at, v);
    }
    let v = s.byte() % 4;
    w8(m, 0x1943, v);
    for at in [0x193E, 0x1940, 0x1945] {
        let v = s.word();
        w16(m, at, v);
    }
    let v = s.byte() as u16 % 9;
    w16(m, 0x12A8, v);
    let v = if s.byte() < 0x20 { 0 } else { 1 };
    w8(m, 0x6C28, v);
    let v = if s.byte() < 0xE0 { 0 } else { 1 };
    w8(m, 0x6C41, v);
    let v = if s.byte() < 0x40 { s.word() } else { 0 };
    w16(m, 0x05B1, v);
    for at in [0x05C8, 0x0FC8, 0x12A2] {
        let v = s.word();
        w16(m, at, v);
    }
    let v = if s.byte() < 0x80 { 0x0E } else { s.byte() };
    w8(m, 0x01A9, v);
    let v = s.byte();
    w8(m, 0x01AA, v);
    for at in [0x929E, 0x1032] {
        let v = s.word();
        w16(m, at, v);
    }
    let v = s.byte();
    w8(m, 0x40A0, v);
    for i in 0..4 {
        let v = s.byte();
        w8(m, 0x91F6 + i, v);
    }
    w16(m, 0x91F0, 0);
    let question = if s.byte() < 0xC0 { 0 } else { s.word() };
    let k = s.byte();
    let mut days = if k < 0x40 {
        [0, 1, 2, 20, 21, 0x7CF, 0x7D0, 0x7D1, 0xFFFF][s.byte() as usize % 9]
    } else {
        s.word() % 0x40
    };
    let mut stored = days;
    let k = s.byte();
    if k < 0x30 {
        w8(m, 0x0A4C, ((shown as u16) << 5).wrapping_add(pick) as u8);
    } else if k < 0x60 {
        w8(m, 0x0A47, 3);
        w8(m, 0x0A22 + 0x24, shown);
        let (x, y) = xy[near];
        w16(m, 0x0A22 + 0x26, x);
        w16(m, 0x0A22 + 0x28, y);
    } else if k < 0x80 {
        w8(m, 0x01A9, 0x0E);
        w8(m, 0x01AA, 42);
        days = 43;
        stored = s.byte() as u16 % 21;
    }
    let v = 1 + s.byte() % 0xFF;
    w8(m, 0x929E + 0x2FF, v);
    (op, question, days, stored)
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
    for line in include_str!("vectors/travelmap.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "travelmap", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (op, question, days, stored) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        m.load_rng();
        let before = m.clone();
        let mut hooks = Log { log: Vec::new(), question, days, stored };
        let head = match op {
            0 => {
                let (x, y) = (m.mem.r16(DS, 0x8F92), m.mem.r16(DS, 0x8F94));
                Some(format!("{:x}", m.travel_nearest(x, y)))
            }
            1 => {
                let ax = m.mem.r16(DS, 0xA8AE);
                m.travel_pick(&mut fs, &mut hooks, ax).map(|a| format!("{a:x}"))
            }
            _ => m.travel_go(&mut fs, &mut hooks).map(|_| "-".to_string()),
        };
        let got = match head {
            Some(head) => {
                m.store_rng();
                if std::env::var("DUMP").ok().as_deref() == Some(f[1]) {
                    eprintln!("{:?}", hooks.log);
                    for o in 0..=0xFFFFu16 {
                        if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                            eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                        }
                    }
                }
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                let lg = hooks.log.iter().flat_map(|v| v.to_le_bytes());
                format!("{head} {:x} {:x}", fnv(changed), fnv(lg))
            }
            None => "stuck".to_string(),
        };
        assert_eq!(f[3], got, "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} compared");
}
