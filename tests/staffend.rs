//! Differential tests of the staff made whole (src/staffend.rs
//! `staff_end`, 1194:D986) against vectors from the original code
//! (harness/gen_staffend.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::staffend::EndHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The screens, stubbed in the generator, counted as its stubs count
/// their calls (in the order of its list), with the lines' SI and rows
/// summed.
#[derive(Default)]
struct Screens {
    n: [u16; 13],
    sums: [u16; 2],
}

impl EndHooks for Screens {
    fn begin(&mut self, m: &mut Machine, _files: &mut Files) {
        self.n[0] += 1;
        // As the generator's stub of DAF0.
        m.mem.w16(DS, 0xA91A, 0x1234);
        m.mem.w16(DS, 0x0D56, 0x5678);
    }
    fn movie(&mut self, _m: &mut Machine, _files: &mut Files, _si: u16) {
        self.n[1] += 1;
    }
    fn speech(&mut self, _m: &mut Machine, _files: &mut Files, _ax: u16, _si: u16, _di: u16) {
        self.n[2] += 1;
    }
    fn fade(&mut self, _m: &mut Machine) {
        self.n[3] += 1;
    }
    fn house(&mut self, _m: &mut Machine, _files: &mut Files) {
        // 9426, 9466, 5C86, 7428 (5828 and 9B4E aren't counted).
        for i in 4..8 {
            self.n[i] += 1;
        }
    }
    fn text(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        self.n[8] += 1;
        Some(())
    }
    fn line(&mut self, _m: &mut Machine, si: u16, y: u16) {
        self.n[9] += 1;
        self.sums[0] = self.sums[0].wrapping_add(si);
        self.sums[1] = self.sums[1].wrapping_add(y);
    }
    fn shown(&mut self, _m: &mut Machine) {
        self.n[5] += 1;
        self.n[10] += 1;
    }
    fn end(&mut self, _m: &mut Machine, _files: &mut Files) {
        self.n[11] += 1;
    }
    fn clear(&mut self, _m: &mut Machine) {
        self.n[12] += 1;
    }
}

/// Mirrors harness/gen_staffend.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    for at in [0x0D56u16, 0xA91A, 0xA902, 0x102C, 0x137A] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    for at in [0x9288u16, 0x803C, 0x101C, 0x137E] {
        let v = s.byte();
        m.mem.w8(DS, at, v);
    }
    let n = s.byte() % 120;
    let mut text = Vec::new();
    for _ in 0..n {
        let b = s.byte();
        text.push(if b < 0x30 { 0x0A } else { 0x20 + b % 0x5F });
    }
    if s.byte() >= 0x20 {
        text.push(0);
    }
    common::blow::put(m, DS, 0x929E, &text);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/staffend.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "staffend", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let mut screens = Screens::default();
        m.staff_end(&mut files, &mut screens).expect("staff_end");
        let counts: Vec<String> = screens.n.iter().chain(screens.sums.iter()).map(|c| format!("{c:x}")).collect();
        assert_eq!(f[2], counts.join("."), "{line}");
        let ds = DS as usize * 16;
        let changed = m.mem.diff(&before.mem, ds..ds + 0x10000).into_iter().flat_map(|(i, v)| {
            let o = (i - ds) as u16;
            [o as u8, (o >> 8) as u8, v]
        });
        assert_eq!(f[3], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n >= 1000, "{n}");
}
