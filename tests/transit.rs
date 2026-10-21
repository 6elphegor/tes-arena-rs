//! Differential tests of moving between areas (src/transit.rs: the outer
//! loop 0000:0ED5/0CAD, the areas' runs, 0000:0AA9 with 2294:025B, the
//! way out 4537/44BD, the place 45B2, the entry 4632) against vectors
//! from the original code (harness/gen_transit.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::transit::{AreaHooks, Run};
use common::blow::{put, Stream};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's stubs (see its STUBS).
struct Hooks;

impl AreaHooks for Hooks {
    fn sync(&mut self, m: &mut Machine) {
        count(m, 0xFFE0);
    }
    fn ending(&mut self, m: &mut Machine, ax: u16, si: u16, di: u16) {
        m.mem.w16(DS, 0xFFB0, ax);
        m.mem.w16(DS, 0xFFB2, si);
        m.mem.w16(DS, 0xFFB4, di);
        count(m, 0xFFE2);
    }
    fn normal_lgt(&mut self, m: &mut Machine) {
        count(m, 0xFFE4);
    }
    fn location(&mut self, m: &mut Machine, id: u16) {
        m.mem.w16(DS, 0xFFB6, id);
        count(m, 0xFFE6);
    }
    fn weather(&mut self, m: &mut Machine) {
        count(m, 0xFFE8);
    }
    fn build_city(&mut self, m: &mut Machine) {
        count(m, 0xFFEA);
    }
    fn wild_setup(&mut self, m: &mut Machine) {
        count(m, 0xFFEC);
    }
    fn wild_done(&mut self, m: &mut Machine) {
        count(m, 0xFFEE);
    }
    fn build_dungeon(&mut self, m: &mut Machine, wild: bool) {
        count(m, 0xFFF0);
        m.mem.w16(DS, 0xFFB8, wild as u16);
        m.mem.w16(DS, 0xFFBA, 1);
        count(m, 0xFFF2);
    }
    fn townsfolk_graphics(&mut self, m: &mut Machine) {
        count(m, 0xFFF6);
    }
    fn city_screen(&mut self, m: &mut Machine) {
        count(m, 0xFFF8);
    }
    fn automap(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        count(m, 0xFFFA);
    }
    fn map_keep(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        count(m, 0xFFFC);
        count(m, 0xFFFE);
    }
    fn ask_leave(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) -> bool {
        count(m, 0xFFA0);
        m.mem.r8(DS, 0xFFDE) & 1 != 0
    }
    fn map_colours(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        count(m, 0xFFAA);
    }
    fn map_given_back(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        count(m, 0xFFA2);
        count(m, 0xFFA4);
        Some(())
    }
    fn leave_screen(&mut self, m: &mut Machine) {
        count(m, 0xFFA6);
        count(m, 0xFF78);
    }
    fn leave_interior(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) {
        count(m, 0xFFAE);
        count(m, 0xFFAC);
    }
    fn leave_dungeon(&mut self, m: &mut Machine) {
        count(m, 0xFFAC);
    }
    fn spawn(&mut self, m: &mut Machine, n: u16) {
        m.mem.w16(DS, 0xFF92, n);
        count(m, 0xFF90);
    }
    fn sky(&mut self, m: &mut Machine) {
        count(m, 0xFF9A);
    }
    fn hook_2974(&mut self, m: &mut Machine) {
        count(m, 0xFF94);
    }
    fn wild_empty(&mut self, m: &mut Machine) -> bool {
        count(m, 0xFF96);
        count(m, 0xFF98);
        m.mem.r8(DS, 0xFFDE) & 2 != 0
    }
    fn mouse_wait(&mut self, m: &mut Machine) {
        count(m, 0xFF80);
    }
    fn border(&mut self, m: &mut Machine) {
        count(m, 0xFF8E);
    }
    fn restore_level(&mut self, m: &mut Machine) {
        count(m, 0xFF82);
        count(m, 0xFF84);
    }
    fn slots_usable(&mut self, m: &mut Machine) -> u16 {
        count(m, 0xFF86);
        m.mem.r16(DS, 0xFFC0)
    }
    fn hands(&mut self, m: &mut Machine) {
        count(m, 0xFF88);
    }
    fn hook_2631(&mut self, m: &mut Machine) {
        count(m, 0xFF8A);
    }
    fn ems_free(&mut self, m: &mut Machine) {
        count(m, 0xFF8C);
    }
    fn world_screen(&mut self, m: &mut Machine) {
        for at in [0xFF72, 0xFF74, 0xFF76, 0xFF78] {
            count(m, at);
        }
    }
}

const NAMES: [u16; 4] = [0x1351, 0x1385, 0x1319, 0x0A36];

/// Mirrors harness/gen_transit.py `setup`; returns the kind and sub.
fn setup(m: &mut Machine, seed: u32) -> (u8, u8) {
    let mut st = Stream(seed);
    let s = &mut st;
    let kind = s.byte() % 4;
    let sub = s.byte() % 5;
    let area = |s: &mut Stream| {
        if s.byte() < 0xE0 {
            [0, 1, 2, 3, 4, 4, 0, 1][(s.byte() % 8) as usize]
        } else {
            s.byte()
        }
    };
    let v = area(s);
    m.mem.w8(DS, 0x4BD0, v);
    let v = area(s);
    m.mem.w8(DS, 0x05E6, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    let mut v = s.word() & 0x7FFF;
    if s.byte() < 0x10 {
        v |= 0x8000;
    }
    m.mem.w16(DS, 0x05CA, v);
    let mut v = s.word() & 0xFEFF;
    if s.byte() < 0x10 {
        v |= 0x100;
    }
    m.mem.w16(DS, 0x05B1, v);
    let v = s.byte();
    m.mem.w8(DS, 0x05E4, v);
    let v = s.word();
    m.mem.w16(DS, 0xC398, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0x40 { 0 } else { s.word() };
    m.mem.w16(DS, 0x0FCE, v);
    let v = s.word();
    m.mem.w16(DS, 0x0FD0, v);
    let v = if s.byte() < 0x50 { 0 } else { NAMES[(s.byte() % 4) as usize] };
    m.mem.w16(DS, 0x0D58, v);
    let k = s.byte();
    let v = if k < 0x50 {
        0
    } else if k < 0x80 {
        9
    } else {
        s.byte()
    };
    m.mem.w8(DS, 0x0F77, v);
    let v = if s.byte() < 0x80 { 8 } else { s.byte() % 10 };
    m.mem.w8(DS, 0x0A46, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0A4C, v);
    let b: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, DS, 0x0A56, &b);
    for at in [0x0608, 0x060A] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = if s.byte() < 0x30 { 0xC9 } else { s.byte() };
    m.mem.w8(DS, 0x01A9, v);
    let v = if s.byte() < 0x30 { 6 } else { s.byte() % 8 };
    m.mem.w8(DS, 0x01A8, v);
    let k = s.byte();
    let v = if k < 0x40 { [0xFF, 0x100, 0x300, 0x301][(s.byte() % 4) as usize] } else { s.word() % 0x400 };
    m.mem.w16(DS, 0xCCFE, v);
    for at in [0x0D37, 0xC3D0, 0xC3D4] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    for at in [0x0D39, 0xBB0A] {
        let v = s.byte();
        m.mem.w8(DS, at, v);
    }
    let v = if s.byte() < 0xC0 { [0, 1, 2, 0x81, 0x82][(s.byte() % 5) as usize] } else { s.byte() };
    m.mem.w8(DS, 0xBB09, v);
    let v = [0, 4, 8][(s.byte() % 3) as usize];
    m.mem.w16(DS, 0x0DA8, v);
    let k = s.byte();
    let v = [0x4537, 0x44BD, s.word()][(k % 3) as usize];
    m.mem.w16(DS, 0xCD4E, v);
    m.mem.w16(DS, 0xCD50, 0);
    let v = s.byte();
    m.mem.w8(DS, 0xFFDE, v);
    let v = if s.byte() < 0x80 { 8 } else { s.word() };
    m.mem.w16(DS, 0xFFC0, v);
    for at in [0x1A81, 0xC994] {
        let v = s.byte();
        m.mem.w8(DS, at, v);
    }
    put(m, DS, 0x1351, b"temple3.mif\0");
    (kind, sub)
}

/// One case as the generator runs it; None for "none".
fn case(m: &mut Machine, kind: u8, sub: u8) -> Option<()> {
    let h = &mut Hooks;
    match kind {
        0 => {
            let (id, si) = match m.area_next(h) {
                Run::Quit | Run::End => (6, None),
                Run::City => (0, Some(0x1385)),
                Run::Wild => (1, Some(0x1319)),
                Run::Dungeon => (3, Some(0x1385)),
                Run::Interior(n) => (4, Some(n)),
                Run::Start => (2, Some(0x0A36)),
            };
            m.mem.w16(DS, 0xFFD0, id);
            if let Some(si) = si {
                m.mem.w16(DS, 0xFFD2, si);
            }
        }
        1 => {
            let run = match sub {
                0 => Run::Interior(match m.mem.r16(DS, 0x0D58) {
                    0 => 0x1351,
                    n => n,
                }),
                1 => Run::Start,
                2 => Run::Dungeon,
                3 => Run::City,
                _ => Run::Wild,
            };
            // The world gets SI as the caller set it (the generator: the
            // interior's name for every run).
            let si = match m.mem.r16(DS, 0x0D58) {
                0 => 0x1351,
                n => n,
            };
            m.area_begin(run, h);
            for (from, to) in [(0xCD32, 0xFFC4), (0xCD4E, 0xFFC6)] {
                let v = m.mem.r16(DS, from);
                m.mem.w16(DS, to, v);
            }
            m.mem.w16(DS, 0xFFBC, si);
            count(m, 0xFFF4);
            m.area_end(run, h);
        }
        2 => {
            m.world_begin(NAMES[(sub % 4) as usize], h);
            count(m, 0xFF70);
            m.world_end(h);
        }
        _ if sub < 2 => m.area_exit(&mut arena::bsa::Files::default(), h)?,
        _ if sub < 4 => m.area_place(h),
        _ => m.area_entry(h),
    }
    Some(())
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xAFF9, 0);
    let mut n = 0;
    for line in include_str!("vectors/transit.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "transit", "{line}");
        let mut m = base.clone();
        let (kind, sub) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let r = case(&mut m, kind, sub);
        if f[2] == "none" {
            assert!(r.is_none(), "{line}");
            continue;
        }
        assert!(r.is_some(), "{line}");
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line} (kind {kind} sub {sub})");
        n += 1;
    }
    assert!(n > 3500);
}

/// The end of a new game's start dungeon (`new_game_next`, 0000:0C98..0CA9
/// with 0CAD) against harness/gen_newgamenext.py's runs, set up as the
/// transit cases are.
#[test]
fn new_game_next_matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xAFF9, 0);
    let mut n = 0;
    for line in include_str!("vectors/newgamenext.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "newgamenext", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let (id, si) = match m.new_game_next(&mut Hooks) {
            Run::Quit | Run::End => (6, None),
            Run::City => (0, Some(0x1385)),
            Run::Wild => (1, Some(0x1319)),
            Run::Dungeon => (3, Some(0x1385)),
            Run::Interior(n) => (4, Some(n)),
            Run::Start => (2, Some(0x0A36)),
        };
        m.mem.w16(DS, 0xFFD0, id);
        if let Some(si) = si {
            m.mem.w16(DS, 0xFFD2, si);
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 1900);
}
