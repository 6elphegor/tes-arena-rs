//! Differential tests of the Emperor's audience (src/sidequest.rs
//! `emperor_audience`, 0000:41A4..4247) against vectors from the original
//! code (harness/gen_audience.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The screens, stubbed in the generator, do nothing; their parts are
/// counted to see that each runs once.
struct Screens(Vec<u8>);

impl QuestHooks for Screens {
    fn audience(&mut self, m: &mut Machine, _files: &mut Files, part: u8) -> Option<()> {
        self.0.push(part);
        if part == 0 {
            // As the generator's stub of DAF0.
            m.mem.w16(DS, 0xA91A, 0x1234);
            m.mem.w16(DS, 0x0D56, 0x5678);
        }
        Some(())
    }
}

/// Mirrors harness/gen_audience.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    for at in [0x0D56u16, 0xA91A, 0xA902, 0x05E0, 0x05C8, 0xC398, 0xC39A, 0xC3D0, 0xC3D4] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x05E6, v);
    for i in 0..12u16 {
        let v = s.byte();
        m.mem.w8(DS, 0x0652 + i, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x803C, v);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/audience.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "audience", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let mut screens = Screens(Vec::new());
        m.emperor_audience(&mut files, &mut screens).expect("audience");
        assert_eq!(screens.0, [0, 1, 2, 3]);
        let ds = DS as usize * 16;
        let changed = m.mem.diff(&before.mem, ds..ds + 0x10000).into_iter().flat_map(|(i, v)| {
            let o = (i - ds) as u16;
            [o as u8, (o >> 8) as u8, v]
        });
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n >= 1000, "{n}");
}
