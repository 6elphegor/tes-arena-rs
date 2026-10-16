//! Differential tests of looking at a square (src/look.rs `look`,
//! 2826:0C15) against vectors from the original code (harness/gen_look.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::talk::TalkHooks;
use arena::text::TextHooks;
use common::blow::{put, Stream};

const MSEG: u16 = 0x7000;
const PSEG: u16 = 0x8000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The stubs' log (harness/gen_look.py's).
#[derive(Default)]
struct Log(Vec<u16>);

impl TextHooks for Log {}

impl TalkHooks for Log {
    fn way(&mut self, _m: &mut Machine, _files: &mut Files, si: u16, x: u16, y: u16) {
        self.0.extend([5, si, x, y]);
    }
    fn expand(&mut self, m: &mut Machine, di: u16) -> Option<u16> {
        self.0.extend([8, di]);
        let mut di = di;
        while m.mem.r8(DS, di) != 0 {
            di = di.wrapping_add(1);
        }
        Some(di)
    }
}

/// harness/gen_look.py's `setup`: the case's state; the square looked at.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut s = Stream(seed);
    let wild = if s.byte() < 0x60 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, wild);
    let town = if s.byte() < 0xD0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, town);
    let (mut col, row) = ((s.byte() & 0x7F) as u16, (s.byte() & 0x7F) as u16);
    if s.byte() < 0x10 {
        col = s.byte() as u16;
    }
    let ax = row << 8 | col;
    let sq = row << 8 | (col << 1) & 0xFF;
    let picked = if s.byte() < 0xE0 { sq } else { s.word() };
    m.mem.w16(DS, 0xCB92, picked);
    let triggers: Vec<u8> = (0..16).map(|_| s.byte() % 0x40).collect();
    for (i, &t) in triggers.iter().enumerate() {
        m.mem.w8(DS, 0xC917 + i as u16, t);
    }
    for i in 0..16 {
        let v = [0xFF, 2, 5, 0, 1, 3, 4][s.byte() as usize % 7];
        m.mem.w8(DS, 0x67FE + i, v);
    }
    let v = match s.byte() % 4 {
        1 => {
            let t = if s.byte() < 0xE0 { triggers[s.byte() as usize % 16] } else { s.byte() % 0x40 };
            let v = (t | (s.byte() & 0xC0)) as u16;
            v << 8 | v
        }
        2 => s.word(),
        3 => 0x8000 | s.word(),
        _ => 0,
    };
    m.mem.w16(MSEG, sq, v);
    for (i, (ptr, cnt)) in [(0x1AB5u16, 0x1387u16), (0x1004, 0x1389), (0x1008, 0x138B)].into_iter().enumerate() {
        let n = 1 + s.byte() as u16 % 5;
        m.mem.w16(DS, cnt, n);
        m.mem.w16(DS, ptr, 0x100 * i as u16);
        m.mem.w16(DS, ptr + 2, PSEG);
        for e in 0..n {
            let (c, r) = if s.byte() < 0x50 { (col as u8, row as u8) } else { (s.byte() & 0x7F, s.byte() & 0x7F) };
            let kind = s.byte() % 8;
            let suffix = s.byte() % 6;
            put(m, PSEG, 0x100 * i as u16 + 4 * e, &[c, r, kind, suffix]);
        }
    }
    let v = s.byte() % 9;
    m.mem.w8(DS, 0x0A46, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0A4C, v);
    let v = s.word();
    m.mem.w16(DS, 0xCB5E, v);
    ax
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, MSEG);
    let Ok(mut files) = Files::new(&common::game_dir()) else { return };
    let only = std::env::var("LOOK_SEED").ok();
    let (mut n, mut named) = (0, 0);
    for line in include_str!("vectors/look.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "look", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let ax = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let mut log = Log::default();
        let got = m.look(&mut files, &mut log, ax).map(|()| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv(log.0.iter().flat_map(|v| v.to_le_bytes())))
        });
        if only.is_some() {
            eprintln!("log {:x?}", log.0);
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            Some(d) => {
                assert_eq!(f[2], d, "{line}");
                if m.mem.r16(DS, 0x061A) == arena::look::NAME {
                    named += 1;
                }
                n += 1;
            }
            None => assert_eq!(f[2], "stuck", "{line}: the port gave up: {}", arena::giveup::describe()),
        }
    }
    eprintln!("look: {n} cases, {named} named");
    assert!(only.is_some() || named > 200, "too few named: {named}");
}
