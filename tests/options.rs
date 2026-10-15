//! Differential tests of the options screen Esc brings up in play
//! (src/options.rs `options_screen`, 2542:000A) against vectors from the
//! original code (harness/gen_options.py). The hooks play the generator's
//! script and log what its stubs log.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::options::{OptionsEnd, OptionsHooks};
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const BOXES: [(u16, u16, u16, u16); 10] = [
    (0, 0x76, 0x40, 0x92),
    (0x41, 0x76, 0x80, 0x92),
    (0x81, 0x76, 0xC0, 0x92),
    (0xC1, 0x76, 0x100, 0x92),
    (0x101, 0x76, 0x13F, 0x92),
    (0x2D, 0x4F, 0x3E, 0x57),
    (0x77, 0x4F, 0x87, 0x57),
    (0x2D, 0x68, 0x3E, 0x6F),
    (0x77, 0x68, 0x87, 0x6F),
    (0xCB, 0x5C, 0x12E, 0x63),
];
const PICKS: [usize; 22] = [0, 1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 9];

struct Script {
    clicks: Vec<(u16, u16, bool)>,
    answers: Vec<u16>,
    cyber: bool,
    log: Vec<Vec<u16>>,
}

/// Mirrors harness/gen_options.py `setup` (a conditional's test byte comes
/// first; a tuple's elements before its index).
fn setup(m: &mut Machine, seed: u32) -> Script {
    let mut st = Stream(seed);
    let s = &mut st;
    let volume = |s: &mut Stream| {
        let k = s.byte() % 6;
        let (a, b) = (s.byte(), s.byte());
        // Both of the tuple's draws are taken whatever the index.
        [0, 1, 0x63, 0x64, a, b % 0x65][k as usize]
    };
    let v = volume(s);
    m.mem.w8(DS, 0x0D3B, v);
    let v = volume(s);
    m.mem.w8(DS, 0x0D3A, v);
    let v = if s.byte() < 0x20 { s.word() } else { 6 + (s.byte() % 0x14) as u16 };
    m.mem.w16(DS, 0x0D37, v);
    for at in [0x102C, 0x1026, 0x1036, 0x1030, 0xEB93, 0xEB00, 0x05C8, 0x05CA, 0xA902, 0xC398, 0x0FCE] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let mut clicks = Vec::new();
    for _ in 0..1 + s.byte() % 8 {
        let (ax, bx) = match s.byte() % 8 {
            0 => (0xFFFF, 0xFFFF),
            1 => (s.word() % 0x140, (s.byte() % 0xC8) as u16),
            _ => {
                let (x0, y0, x1, y1) = BOXES[PICKS[s.byte() as usize % PICKS.len()]];
                let r = x0 + s.byte() as u16 % (x1 - x0 + 1);
                let ax = [x0, x1, r][s.byte() as usize % 3];
                let r = y0 + s.byte() as u16 % (y1 - y0 + 1);
                let bx = [y0, y1, r][s.byte() as usize % 3];
                (ax, bx)
            }
        };
        clicks.push((ax, bx, s.byte() < 0x08));
    }
    let mut answers: Vec<u16> = (0..3).map(|_| [0, 1, 0xFFFF][s.byte() as usize % 3]).collect();
    clicks.reverse();
    answers.reverse();
    Script { clicks, answers, cyber: false, log: Vec::new() }
}

impl OptionsHooks for Script {
    fn font(&mut self, _m: &mut Machine, ax: u16, text: bool) {
        self.log.push(vec![if text { 3 } else { 2 }, ax]);
    }
    fn automap_write(&mut self, _m: &mut Machine, _files: &mut Files) {
        self.log.push(vec![15, 0]);
    }
    fn map_bank(&mut self, _m: &mut Machine, bank: u16) {
        self.log.push(vec![4, bank]);
    }
    fn picture(&mut self, _m: &mut Machine, _files: &mut Files, name: u16) {
        self.log.push(vec![6, name]);
    }
    fn fill(&mut self, _m: &mut Machine, ax: u16, bx: u16, cx: u16, dx: u16) {
        self.log.push(vec![7, ax, bx, cx, dx]);
    }
    fn centred(&mut self, _m: &mut Machine, si: u16, ax: u16, bx: u16) {
        self.log.push(vec![8, si, ax, bx]);
    }
    fn hide(&mut self, _m: &mut Machine) {
        self.log.push(vec![17, 0]);
    }
    fn unhide(&mut self, _m: &mut Machine) {
        self.log.push(vec![18, 0]);
    }
    fn show(&mut self, _m: &mut Machine) {
        self.log.push(vec![9, 0]);
    }
    fn click(&mut self, m: &mut Machine) -> Option<(u16, u16)> {
        let (ax, bx, cyber) = self.clicks.pop().unwrap_or((0x120, 0x80, false));
        self.cyber = cyber;
        self.log.push(vec![10, ax, bx]);
        m.mem.w16(DS, 0x8F92, ax);
        m.mem.w16(DS, 0x8F94, bx);
        Some((ax, bx))
    }
    fn cyber(&mut self, _m: &mut Machine) -> bool {
        self.cyber
    }
    fn save_menu(&mut self, _m: &mut Machine, _files: &mut Files) {
        self.log.push(vec![20, 0]);
    }
    fn load_menu(&mut self, _m: &mut Machine, _files: &mut Files) {
        self.log.push(vec![21, 0]);
    }
    fn confirm(&mut self, _m: &mut Machine) -> u16 {
        let v = self.answers.pop().unwrap_or(1);
        self.log.push(vec![22, v]);
        v
    }
    fn delete_matching(&mut self, _m: &mut Machine, _files: &mut Files, si: u16) {
        self.log.push(vec![13, si]);
    }
    fn music_volume(&mut self, m: &mut Machine, volume: u16) {
        self.log.push(vec![23, m.mem.r16(DS, 0xEB93), m.mem.r16(DS, 0xEB00), volume, 0]);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/options.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "options", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let mut script = setup(&mut m, seed);
        let before = m.clone();
        let mut fs = Files::default();
        let end = match m.options_screen(&mut fs, &mut script) {
            None => "none",
            Some(OptionsEnd::Resume) => "resume",
            Some(OptionsEnd::NewGame) => "new",
            Some(OptionsEnd::Load) => "load",
            Some(OptionsEnd::Save) => "save",
            Some(OptionsEnd::Quit) => "quit",
        };
        let r: Vec<&str> = f[2].split(' ').collect();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let log: Vec<u8> = script.log.iter().flatten().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(r[0], end, "{line}");
        assert_eq!(r[1], format!("{:x}", fnv(changed)), "{line}: DGROUP");
        assert_eq!(r[2], format!("{:x}", fnv(log)), "{line}: calls");
        n += 1;
    }
    assert!(n > 2500, "{n} cases");
}
