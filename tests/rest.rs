//! Differential tests of resting (src/rest.rs: 2901:016C, 2433:12F5,
//! 1194:424E, 1194:4635) against vectors from the original code
//! (harness/gen_rest.py).
mod common;

use arena::bsa::Files;
use arena::clock::ClockHooks;
use arena::machine::Machine;
use arena::mem::DS;
use arena::rest::RestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const PLAYER: u16 = 0x01A4;
const CSEG: u16 = 0x7800;
const TEXTS: [&[u8]; 14] = [b"", b"0", b"1", b"2", b"3", b"12", b"999", b"5a", b"-3", b"00", b"ab", b"24", b"100", b"7"];
const CLASSES: [u8; 10] = [0x00, 0x0F, 0x20, 0x21, 0x23, 0x24, 0x25, 0x2A, 0x05, 0x37];

/// Mirrors harness/gen_rest.py `setup` (a conditional's test byte comes
/// first, a tuple's items before its index); returns the op and AX.
fn setup(m: &mut Machine, seed: u32) -> (u8, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let w16 = |m: &mut Machine, o: u16, v: u16| m.mem.w16(DS, o, v);
    let w8 = |m: &mut Machine, o: u16, v: u8| m.mem.w8(DS, o, v);
    let op = s.byte() % 4;
    for i in 0..0x6E {
        let b = s.byte();
        w8(m, PLAYER + i, b);
    }
    let c = CLASSES[s.byte() as usize % CLASSES.len()];
    w8(m, 0x01A9, c);
    let l = s.byte() % 4;
    w8(m, 0x01AA, l);
    let mut cur = [0u8; 8];
    for i in 0..8 {
        let base = s.byte();
        w8(m, 0x01D5 + i, base);
        let v = if s.byte() < 0xC0 { base.wrapping_sub(s.byte() % 40) } else { s.byte() };
        cur[i as usize] = v;
        w8(m, 0x01CD + i, v);
    }
    let max_hp = s.word() % 300 + 1;
    w16(m, 0x01FF, max_hp);
    let k = s.byte();
    let hp = if k < 0x50 {
        max_hp
    } else if k < 0xF0 {
        s.word() % (max_hp + 1)
    } else {
        s.word()
    };
    w16(m, 0x01FD, hp);
    let v = if s.byte() < 0xF0 { ((s.byte() % 12) as u16).wrapping_sub(4) } else { s.word() };
    w16(m, 0x01F7, v);
    let v = if s.byte() < 0xC0 { s.word() & 0x7FFF } else { 0x7F00 + s.byte() as u16 };
    w16(m, 0x0201, v);
    let max_sp = s.word() % 400;
    w16(m, 0x020C, max_sp);
    let v = if s.byte() < 0x60 { max_sp } else { s.word() % (max_sp + 1) };
    w16(m, 0x020A, v);
    let v = if op != 3 { s.byte() % 2 } else if s.byte() < 0xE0 { 1 } else { 0 };
    w8(m, 0x1013, v);
    let v = s.word() & 0x7FFF;
    w16(m, 0x1014, v);
    let v = if s.byte() < 0xE0 { (s.byte() % 5) as u16 } else { s.word() };
    w16(m, 0x1016, v);
    let v = s.byte() % 2;
    w8(m, 0x1018, v);
    let v = if s.byte() < 0xE8 { (s.byte() % 30) as u16 + 1 } else { (s.byte() % 3) as u16 };
    w16(m, 0x1019, v);
    let v = s.byte() % 3;
    w8(m, 0x49E6, v);
    let v = if s.byte() < 0xF0 { s.byte() % 2 } else { s.byte() };
    w8(m, 0x13C8, v);
    let v = s.word();
    w16(m, 0x0FF9, v);
    let v = if s.byte() < 0xD0 { 0 } else { s.word() };
    w16(m, 0x0FFB, v);
    let v = if s.byte() < 0xE0 { 0 } else { 1 };
    w8(m, 0xAFF9, v);
    let v = s.word() & !4;
    let v = v | if s.byte() < 0x30 { 4 } else { 0 };
    w16(m, 0xC398, v);
    let v = s.word() & !0x210;
    let v = v | if s.byte() < 0x20 { 0x10 } else { 0 };
    let v = v | if s.byte() < 0x30 { 0x200 } else { 0 };
    w16(m, 0xC39A, v);
    let v = if s.byte() < 0x90 { 4 } else { s.byte() % 6 };
    w8(m, 0x05E6, v);
    let v = [4, 4, 0xB, 8, 5, 7, 6][s.byte() as usize % 7];
    w8(m, 0x0D48, v);
    let v = s.byte() % 2;
    w8(m, 0x191D, v);
    let v = s.byte();
    w8(m, 0x0D39, v);
    let lo = s.word() as u32;
    let now = lo | (s.word() as u32) << 16;
    m.mem.w32(DS, 0x05CC, now);
    let end = now.wrapping_add((s.word() % 3000) as u32).wrapping_sub(300);
    m.mem.w32(DS, 0x12CA, end);
    let v = s.word();
    w16(m, 0x12C4, v);
    let v = s.byte();
    w8(m, 0x12BC, v);
    let v = s.byte();
    w8(m, 0x12BE, v);
    let v = if s.byte() < 0xA0 { 0 } else { 1 };
    w8(m, 0x0F76, v);
    let v = s.byte() % 10;
    w8(m, 0x0F77, v);
    let v = s.byte() % 2;
    w8(m, 0x0F7A, v);
    for i in 0..0x17u16 {
        let v = s.word();
        let v = v | if s.byte() < 0x80 { 0x4000 } else { 0 };
        w16(m, 0x0AB3 + i * 0x1C + 0xF, v);
    }
    for i in 0..8u16 {
        let wv = s.word();
        let p = [0xFFFF, 0, 1, 2, wv][s.byte() as usize % 5];
        w16(m, 0xFF00 + 2 * i, p);
    }
    for i in 0..4u16 {
        let t = TEXTS[s.byte() as usize % TEXTS.len()];
        for k in 0..4u16 {
            w8(m, 0xFF20 + 4 * i + k, t.get(k as usize).copied().unwrap_or(0));
        }
    }
    for i in 0..16u16 {
        w8(m, 0xFF30 + i, 0);
    }
    w16(m, 0xFFF8, 0xFF00);
    w16(m, 0xFFFA, 0xFF20);
    let v = s.byte() % 2;
    w8(m, 0xFFE8, v);
    let ax = if op == 0 {
        if s.byte() < 0xD0 { (s.byte() % 4 + 1) as u16 } else { s.word() }
    } else {
        0
    };
    if s.byte() < 0x40 {
        let lim = (cur[0] as u16 + cur[5] as u16) << 6;
        w16(m, 0x1014, lim);
        let d = (s.byte() % 3) as u16;
        w16(m, 0x0201, lim.wrapping_sub(0x3C).wrapping_add(d).wrapping_sub(1));
        w16(m, 0x01FD, max_hp);
        w16(m, 0x020A, max_sp);
    }
    for _ in 0..s.byte() % 4 {
        let slot = 15 + (s.byte() % 8) as u16;
        let rec = s.word();
        w16(m, 0x0AB3 + slot * 0x1C + 0x12, rec);
        let e = (s.byte() % 16) as u16;
        m.mem.w32(DS, 0x0660 + e * 0x3C + 0x28, (CSEG as u32) << 16 | rec as u32);
    }
    (op, ax)
}

/// The stubs of harness/gen_rest.py, as hooks.
struct Stubs;

fn inc(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

impl ClockHooks for Stubs {}
impl RestHooks for Stubs {
    fn menu(&mut self, m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        let p = m.mem.r16(DS, 0xFFF8);
        m.mem.w16(DS, 0xFFF8, p.wrapping_add(2));
        m.mem.r16(DS, p)
    }
    fn type_hours(&mut self, m: &mut Machine) {
        let p = m.mem.r16(DS, 0xFFFA);
        let v = m.mem.r32(DS, p);
        m.mem.w32(DS, 0x4980, v);
        m.mem.w16(DS, 0xFFFA, p.wrapping_add(4));
    }
    fn message(&mut self, m: &mut Machine, si: u16) {
        m.mem.w16(DS, 0xFFF6, si);
        inc(m, 0xFFF4);
    }
    fn hours_text(&mut self, m: &mut Machine) {
        inc(m, 0xFFE4);
    }
    fn dream(&mut self, m: &mut Machine, _files: &mut Files, text: u16, _si: u16, _di: u16) -> Option<()> {
        m.mem.w16(DS, 0xFFEE, text);
        inc(m, 0xFFEC);
        Some(())
    }
    // (gen_rest.py's stubs: A611, 55B2, A6B5, 2542:0446, 9A98 and 9AE6
    // return.)
    fn dream_keep(&mut self, _m: &mut Machine, _files: &mut Files) {}
    fn dream_black(&mut self, _m: &mut Machine) {}
    fn dream_back(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn dream_screen(&mut self, _m: &mut Machine) {}
    fn sky(&mut self, m: &mut Machine) {
        inc(m, 0xFFEA);
    }
    fn room_load(&mut self, m: &mut Machine, _files: &mut Files) -> bool {
        m.mem.r8(DS, 0xFFE8) & 1 != 0
    }
    fn room_file(&mut self, m: &mut Machine, _files: &mut Files) {
        inc(m, 0xFFE6);
    }
    fn advance(&mut self, m: &mut Machine, _files: &mut Files, minutes: u32) -> Option<bool> {
        let v = m.mem.r32(DS, 0xFFE0).wrapping_add(minutes);
        m.mem.w32(DS, 0xFFE0, v);
        Some(true)
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    let mut files = Files::default();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/rest.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "rest", "{line}");
        let mut m = base.clone();
        let (op, ax) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let mut carry = 0;
        let got = match op {
            0 => m.rest_heal(ax),
            1 => {
                carry = m.camp_check(&mut files, &mut Stubs) as u8;
                Some(())
            }
            2 => m.rest_start(&mut files, &mut Stubs),
            _ => m.rest_frame(&mut files, &mut Stubs),
        };
        match got {
            Some(()) => {
                let changed = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                let cseg = fnv((0..=0xFFFFu16).map(|o| m.mem.r8(CSEG, o)));
                let res = format!("{:x} {cseg:x}", fnv(changed));
                if op == 1 {
                    assert_eq!(f[3], format!("{carry} {res}"), "{line}");
                } else {
                    assert_eq!(f[3].split_once(' ').map(|x| x.1), Some(res.as_str()), "{line}");
                }
                n += 1;
            }
            None => {
                assert!(f[3] == "fault" || f[3] == "hang", "{line}: the port gave up");
                stuck += 1;
            }
        }
    }
    eprintln!("rest: {n} compared, {stuck} hang or fault in both");
    assert!(n > 3000, "{n} compared");
}
