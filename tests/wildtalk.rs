//! Differential tests of two clicks of the wild (src/talk.rs): the boat
//! boarded (`board_boat`, 2294:0DD0) and the way told (`wild_way`,
//! 217B:0682) against vectors from the original code
//! (harness/gen_wildtalk.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::talk::TalkHooks;
use arena::text::TextHooks;
use common::blow::{put, Stream};

const MSEG: u16 = 0x6000;
const FSEG: u16 = 0x7000;
const RECORD: u16 = 0x3D96;
const LISTS: [&[u8]; 3] = [&[10, 17, 49, 51, 55, 57, 64, 65, 66, 67, 68, 69], &[9, 16, 25, 30, 32, 37, 52, 58, 59, 60], &[8, 18, 22, 27, 30, 34, 36, 40, 54, 61, 70]];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The stubs' log (harness/gen_wildtalk.py's).
#[derive(Default)]
struct Log(Vec<u16>);

impl TextHooks for Log {
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.0.extend([1, si]);
    }
}

impl TalkHooks for Log {
    fn text(&mut self, m: &mut Machine, _files: &mut Files, mode: u16) -> Option<bool> {
        let r = |o: u16| m.mem.r16(DS, o);
        self.0.extend([2, r(0x137A), mode, r(0xC3D0), r(0xC3D4), r(0x8A52), r(0x8A54)]);
        Some(true)
    }
}

/// harness/gen_wildtalk.py's `boat_setup`.
fn boat_setup(m: &mut Machine, seed: u32) {
    let mut s = Stream(seed);
    let (x, y) = (s.word() & 0x3FFF, s.word() & 0x3FFF);
    m.mem.w16(DS, RECORD, x);
    m.mem.w16(DS, RECORD + 4, y);
    let v = s.word();
    m.mem.w16(MSEG, (y >> 7) << 8 | ((x >> 7) * 2) & 0xFF, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CA, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC8F4, v);
    let v = 0x1000 * (s.byte() as u16 % 8);
    m.mem.w16(DS, 0x902E, v);
    m.mem.w16(DS, 0x9030, FSEG);
}

/// harness/gen_wildtalk.py's `way_setup`: the kind.
fn way_setup(m: &mut Machine, seed: u32) -> u16 {
    let mut s = Stream(seed);
    let kind = s.byte() as usize % 3;
    let walls: Vec<u8> = (0..3).map(|_| s.byte() % 0x40).collect();
    for (off, &v) in [0xC91Au16, 0xC91B, 0xC943].iter().zip(&walls) {
        m.mem.w8(DS, *off, v);
    }
    let v = s.word();
    m.mem.w16(DS, 0xC3D0, v);
    let v = s.word();
    m.mem.w16(DS, 0xC3D4, v);
    let v = s.byte() as u16 % 0x40;
    m.mem.w16(DS, 0x0608, v);
    let v = s.byte() as u16 % 0x40;
    m.mem.w16(DS, 0x060A, v);
    m.mem.w16(DS, 0x9030, FSEG);
    let none = s.byte() < 0x40;
    let mut blocks = Vec::with_capacity(0x1000);
    for _ in 0..0x1000 {
        if !none && s.byte() < 4 {
            let l = LISTS[kind];
            blocks.push(l[s.byte() as usize % l.len()]);
        } else {
            blocks.push(s.byte() % 8);
        }
    }
    put(m, FSEG, 0xF000, &blocks);
    if s.byte() < 0x80 {
        let wall = if kind == 0 { 0x8000 | walls[2] as u16 } else { walls[kind - 1] as u16 * 0x101 };
        let at = 2 * (s.word() % 0x4000);
        m.mem.w16(MSEG, at, wall);
    }
    let v = s.word();
    m.mem.w16(DS, 0x8A52, v);
    let v = s.word();
    m.mem.w16(DS, 0x8A54, v);
    kind as u16
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, MSEG);
    base.mem.w8(DS, 0x8FA6, 0);
    let Ok(mut files) = Files::new(&common::game_dir()) else { return };
    assert!(base.bsa_open(&mut files));
    let only = std::env::var("WILD_SEED").ok();
    let mut n = [0; 2];
    for line in include_str!("vectors/wildtalk.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut log = Log::default();
        let got = match f[0] {
            "boat" => {
                boat_setup(&mut m, seed);
                let before = m.clone();
                m.board_boat(&mut files, RECORD).map(|()| before)
            }
            "wildway" => {
                let kind = way_setup(&mut m, seed);
                let before = m.clone();
                m.wild_way(&mut files, &mut log, kind).map(|()| before)
            }
            k => panic!("{k}"),
        };
        if let (Some(_), Some(b)) = (&only, &got) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != b.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", b.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        let got = got.map(|before| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x} {:x}",
                fnv(changed),
                fnv((0..=0xFFFFu16).map(|o| m.mem.r8(FSEG, o))),
                fnv((0..=0xFFFFu16).map(|o| m.mem.r8(MSEG, o))),
                fnv(log.0.iter().flat_map(|v| v.to_le_bytes()))
            )
        });
        if only.is_some() {
            eprintln!("log {:x?}", log.0);
        }
        match got {
            Some(d) => {
                assert_eq!(f[2], d, "{line}");
                n[(f[0] == "wildway") as usize] += 1;
            }
            None => assert_eq!(f[2], "stuck", "{line}: the port gave up: {}", arena::giveup::describe()),
        }
    }
    assert!(only.is_some() || n.iter().all(|&k| k > 1000), "too few: {n:?}");
}
