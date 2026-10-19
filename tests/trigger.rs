//! Differential tests of a wall's trigger (src/trigger.rs
//! `wall_trigger`, 0000:43B1, with the building entry 0000:4054)
//! against vectors from the original code (harness/gen_trigger.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::trigger::TriggerHooks;
use common::blow::{put, Stream};

const WSEG: u16 = 0x6800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's stubs: calls counted at FFE0..FFFC.
struct Hooks;

impl TriggerHooks for Hooks {
    fn message_box(&mut self, m: &mut Machine, _text: u16) {
        count(m, 0xFFE0);
    }
    fn sound(&mut self, m: &mut Machine, _sound: u16, _at: u16) {
        count(m, 0xFFE2);
    }
    fn tavern_room(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFE4);
        Some(())
    }
    fn palace_audience(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFE6);
        Some(())
    }
    fn artifact_audience(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFE8);
        Some(())
    }
    fn sounds_off(&mut self, m: &mut Machine) {
        count(m, 0xFFEA);
    }
    fn blackout(&mut self, m: &mut Machine) {
        count(m, 0xFFEC);
    }
    fn automap_write(&mut self, m: &mut Machine, _files: &mut Files) {
        count(m, 0xFFEE);
    }
    fn enter_dungeon(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFF0);
        Some(())
    }
    fn leave_city(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFF2);
        Some(())
    }
    fn enter_city(&mut self, m: &mut Machine) {
        count(m, 0xFFF4);
        count(m, 0xFFF6);
    }
    fn leave_interior(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        count(m, 0xFFF8);
        count(m, 0xFFFA);
        Some(())
    }
    fn guards(&mut self, m: &mut Machine) {
        count(m, 0xFFFC);
    }
}

/// Mirrors harness/gen_trigger.py `setup` (a conditional's test is drawn
/// before its branch).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let col = 4 + (s.byte() % 24) as u16;
    let row = 4 + (s.byte() % 24) as u16;
    let px = col << 7 | (s.byte() & 0x7F) as u16;
    let py = row << 7 | (s.byte() & 0x7F) as u16;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let k = s.byte();
    let sq = if k < 0x10 {
        0xFFFF
    } else if k < 0x18 {
        s.word()
    } else {
        let r = (row + (s.byte() % 3) as u16).wrapping_sub(1);
        let c = (col + (s.byte() % 3) as u16).wrapping_sub(1);
        r << 8 | c << 1
    };
    m.mem.w16(DS, 0xCB92, sq);
    let mut trig = [0u8; 16];
    for t in trig.iter_mut() {
        *t = if s.byte() < 0xC0 { s.byte() % 0x40 } else { s.byte() };
    }
    let i = (s.byte() % 16) as usize;
    let t = (s.byte() % 0x40) as u16;
    trig[i] = t as u8;
    put(m, DS, 0xC917, &trig);
    let k = s.byte();
    let wall = if k < 0x80 {
        t << 8 | t
    } else if k < 0xA0 {
        let hi = s.byte() as u16;
        0x8000 | hi << 8 | t | (s.byte() & 0xC0) as u16
    } else if k < 0xB0 {
        (t << 8 | t) ^ 0x40
    } else if k < 0xC0 {
        t << 8 | s.byte() as u16
    } else {
        s.word()
    };
    if sq != 0xFFFF {
        m.mem.w16(WSEG, sq, wall);
    }
    let outdoors = if s.byte() < 0xC0 { 4 } else { 0 };
    let v = s.word() & 0xFF7B | outdoors;
    m.mem.w16(DS, 0xC398, v);
    let mut v = s.word() & 0xFDEF;
    if s.byte() < 0x60 {
        v |= 0x200;
    }
    if s.byte() < 0x10 {
        v |= 0x10;
    }
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    let v = s.byte();
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0D48, v);
    let v = if s.byte() < 0x80 { 8 } else { s.byte() % 10 };
    m.mem.w8(DS, 0x0A46, v);
    let v = s.byte() % 3;
    m.mem.w8(DS, 0x0A47, v);
    let v = s.word();
    m.mem.w16(DS, 0x0A53, v);
    let k = s.byte();
    let v = if k < 0x40 {
        8
    } else if k < 0x80 {
        9
    } else {
        s.byte() % 12
    };
    m.mem.w8(DS, 0x0F77, v);
    let mut v = s.word() & 0xF7FE;
    if s.byte() < 0x30 {
        v |= 0x800;
    }
    if s.byte() < 0x80 {
        v |= 1;
    }
    m.mem.w16(DS, 0x05CA, v);
    let k = s.byte();
    let v = if k < 0x80 {
        0
    } else if k < 0xA0 {
        0x7D0
    } else {
        (s.byte() % 0x60) as u16
    };
    m.mem.w16(DS, 0x0DC4, v);
    let v = s.byte() % 0x20;
    m.mem.w8(DS, 0x01AA, v);
    let v = s.word() % 0x400;
    m.mem.w16(DS, 0xCCFE, v);
    let lo = s.word();
    let hi = s.word() % 4;
    m.mem.w16(DS, 0x05CC, lo);
    m.mem.w16(DS, 0x05CE, hi);
    for o in [0x2A, 0x2C] {
        let v = s.byte();
        m.mem.w8(DS, 0x01A4 + o, v);
    }
    let v = s.byte() % 0x12;
    m.mem.w8(DS, 0x01A4 + 5, v);
    let v = s.byte() % 30;
    m.mem.w8(DS, 0x01A4 + 6, v);
    for i in 0..0x20 {
        let v = if s.byte() < 0x20 {
            sq
        } else if s.byte() < 0xC0 {
            0xFFFF
        } else {
            s.word()
        };
        m.mem.w16(DS, 0x0F88 + 2 * i, v);
    }
    let v = (s.byte() % 3) as u16;
    m.mem.w16(DS, 0x061E, v);
    let v = if s.byte() < 0x80 { 0x7BFA } else { s.word() };
    m.mem.w16(DS, 0x061A, v);
    for o in [0x1036, 0x102C, 0xCB90] {
        let v = s.word();
        m.mem.w16(DS, o, v);
    }
    for k in 0..0x40 {
        let v = s.byte();
        m.mem.w8(DS, 0x8FAA + k, v);
    }
    for k in 0..4 {
        let v = s.byte();
        m.mem.w8(DS, 0x91F6 + k, v);
    }
    let v = s.word();
    m.mem.w16(DS, 0x0FF7, v);
    let v = s.byte();
    m.mem.w8(DS, 0x6636, v);
    // (EAX's high word is drawn last: the distance doesn't read it.)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x91F0, 0);
    let mut files = Files::default();
    let mut n = 0;
    for line in include_str!("vectors/trigger.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "trigger", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let r = m.wall_trigger(&mut files, &mut Hooks);
        if f[2] == "fault" || f[2] == "hang" {
            assert!(r.is_none(), "{line}");
            continue;
        }
        assert!(r.is_some(), "{line}");
        m.store_rng();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        assert_eq!(f[2], format!("{:x}", fnv(changed)), "{line}");
        n += 1;
    }
    assert!(n > 3500);
}
