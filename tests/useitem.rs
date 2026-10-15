//! Differential tests of using items (src/use_item.rs: `equip_toggle`
//! 1194:7605, `equip_click` 1194:1AFB, `item_cast` 29CF:07C3, the charms
//! 2294:1051 and 0FFA, `use_item` 24FA:07CD, `drop_item` 1194:18F6 with
//! `drop_to_floor` 2433:09E1) against vectors from the original code
//! (harness/gen_useitem.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::use_item::UseHooks;
use common::blow::Stream;

const FRAME: u16 = 0x6000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

fn put(m: &mut Machine, seg: u16, off: u16, b: &[u8]) {
    for (i, &v) in b.iter().enumerate() {
        m.mem.w8(seg, off.wrapping_add(i as u16), v);
    }
}

/// The generator's stubs: casts logged (spell, level byte) at FF00 +
/// [FFE0], breakages (SI) at FF40 + [FFE2], message boxes counted at
/// FFE8, the click's refusals at FFE4 and FFE6; the list's pick.
struct Log {
    pick: u16,
}

impl UseHooks for Log {
    fn cast(&mut self, m: &mut Machine, _files: &mut Files, spell: u8) -> Option<()> {
        let i = m.mem.r16(DS, 0xFFE0);
        m.mem.w8(DS, 0xFF00u16.wrapping_add(i), spell);
        let lv = m.mem.r8(DS, 0x01AA);
        m.mem.w8(DS, 0xFF01u16.wrapping_add(i), lv);
        m.mem.w16(DS, 0xFFE0, i.wrapping_add(2));
        Some(())
    }
    fn broken(&mut self, m: &mut Machine, si: u16) -> Option<()> {
        let i = m.mem.r16(DS, 0xFFE2);
        m.mem.w16(DS, 0xFF40u16.wrapping_add(i), si);
        m.mem.w16(DS, 0xFFE2, i.wrapping_add(2));
        Some(())
    }
    fn pick(&mut self, _m: &mut Machine, _count: u16) -> u16 {
        self.pick
    }
    fn message_box(&mut self, m: &mut Machine, _si: u16) {
        count(m, 0xFFE8);
    }
    fn cant_use(&mut self, m: &mut Machine, _si: u16, _cat: u16) {
        count(m, 0xFFE4);
    }
    fn clash(&mut self, m: &mut Machine, _si: u16, _base: u16) {
        count(m, 0xFFE6);
    }
}

fn day_of(minutes: u32) -> u16 {
    ((minutes % 0x7E900) / 0x5A0) as u16
}

/// Mirrors harness/gen_useitem.py `setup` (a conditional's test byte
/// comes first; `or` stops at a true left side); returns (slot, extra).
fn setup(m: &mut Machine, seed: u32, op: u8) -> (u16, u8) {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| s.word() as u32 | (s.word() as u32) << 16;
    let attrs: Vec<u8> = (0..8).map(|_| s.byte()).collect();
    put(m, DS, 0x01CD, &attrs);
    let v = s.byte();
    m.mem.w8(DS, 0x01A9, v);
    let v = s.byte() % 30;
    m.mem.w8(DS, 0x01AA, v);
    let v = s.word() % 400;
    m.mem.w16(DS, 0x020A, v);
    let v = s.word() % 400;
    m.mem.w16(DS, 0x020C, v);
    let occupied = s.byte() % 41;
    let mut spells = Vec::new();
    for i in 0..40u16 {
        let mut b = [0u8; 19];
        b[0] = if s.byte() < 0x50 { 0x80 + s.byte() % 0x12 } else { s.byte() % 0x12 };
        let cat = s.byte() % 4;
        let mut f = s.byte() & 0x33 | cat << 2;
        if i < occupied as u16 || s.byte() < 0x10 {
            f |= 0x40;
        }
        if s.byte() < 0x60 {
            f |= 0x80;
        }
        b[0xE] = f;
        b[1..3].copy_from_slice(&s.word().to_le_bytes());
        b[3] = s.byte() % 4;
        b[4] = s.byte();
        b[5] = s.byte();
        b[6..8].copy_from_slice(&(s.word() % 0x300).to_le_bytes());
        b[8..10].copy_from_slice(&s.word().to_le_bytes());
        let price = if s.byte() < 0x40 { 0 } else { dword(s) };
        b[0xA..0xE].copy_from_slice(&price.to_le_bytes());
        b[0xF] = if s.byte() < 0x30 { 0xFF } else { s.byte() };
        b[0x10] = s.byte();
        let v = s.byte();
        b[0x11] = if s.byte() < 0x40 { [0xFF, 0xC1, v][v as usize % 3] } else { v };
        b[0x12] = if s.byte() < 0x40 { 0xFF } else { s.byte() % 8 };
        put(m, DS, 0x0212 + 0x13 * i, &b);
        spells.push(b[0x11]);
    }
    let v = s.byte() % 41;
    m.mem.w8(DS, 0x129D, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CA, v);
    let minutes = dword(s);
    m.mem.w32(DS, 0x05CC, minutes);
    let v = dword(s);
    m.mem.w32(DS, 0x05C2, v);
    for at in [0x0F84, 0x0DC0, 0x0DC2, 0x0DC4, 0x0DC8, 0x1036, 0x102C] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let t: Vec<u8> = (0..0x12).map(|_| s.byte() % 0x18).collect();
    put(m, DS, 0x7F81, &t);
    if op != 1 {
        for sp in spells {
            let base = if sp & 0x80 != 0 { 0x2A80 } else { 0 };
            let w = s.word();
            let v = w >> (s.byte() % 12);
            m.mem.w16(DATA, base + (sp & 0x7F) as u16 * 0x55 + 0x32, v);
        }
    }
    let v = s.byte() % 40;
    m.mem.w8(DS, 0x1296, v);
    let v = s.byte();
    m.mem.w8(DS, 0x1297, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x129A, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 };
    m.mem.w8(DS, 0x1299, v);
    let v = if s.byte() < 0xE0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x1270, v);
    let v = s.word() % 12;
    m.mem.w16(DS, 0x126C, v);
    let v = s.byte() % 41;
    m.mem.w8(DS, 0x050A, v);
    for at in [0xC3D0, 0xC3D4, 0xC39A, 0xC398] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = s.byte() % 6;
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() % 12;
    m.mem.w8(DS, 0x0D48, v);
    let v = s.byte();
    m.mem.w8(DS, 0xBB0A, v);
    let v = s.byte() % 40;
    m.mem.w8(DS, 0x0FF2, v);
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w8(DS, 0x8FA6, 0);
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    let slot = (s.byte() % 40) as u16;
    let extra = s.byte();
    if op == 3 && extra & 1 != 0 {
        m.mem.w16(DS, 0x0212 + 0x13 * slot + 1, day_of(minutes));
    }
    // Aimed cases, written over the rest.
    let (aim, a1, a2) = (s.byte() % 4, s.byte(), s.byte());
    let si = 0x0212 + 0x13 * slot;
    let other = 0x0212 + 0x13 * if slot == 0 { 1 } else { 0 };
    let item = |m: &mut Machine, at: u16, ty: u8, flags: u8| {
        m.mem.w8(DS, at, ty);
        m.mem.w8(DS, at + 0xE, flags);
    };
    let enchant = |m: &mut Machine, attr: u8, p: u16| {
        m.mem.w8(DS, si + 0xF, 0);
        m.mem.w8(DS, si + 0x11, 0);
        m.mem.w8(DS, si + 0x12, attr);
        m.mem.w8(DS, 0x01CD + attr as u16, attr_for(p));
    };
    match (op, aim) {
        (0, 1) => {
            item(m, si, a1 % 0x12, 0x41 | ((a1 >> 6) & 1) << 2);
            enchant(m, a2 % 8, 0x59 + ((a2 >> 3) % 3) as u16);
        }
        (0, 2) => {
            let t = a1 % 5;
            let gain = m.mem.r8(DS, 0x4AFE + t as u16) as i32;
            let p = if gain == 0xFF { (a2 % 0x64) as i32 } else { (0x63 + ((a2 >> 3) % 3) as i32 - gain).max(0) };
            item(m, si, t, 0x41 | 0x08 | 0x10);
            enchant(m, a2 % 8, p as u16);
        }
        (0, 3) => {
            item(m, si, a1 % 0x12, 0x41 | 0x04);
            m.mem.w8(DS, si + 3, 2);
            item(m, other, 0x8D + a2 % 3, 0xC0 | 0x04);
            m.mem.w8(DS, 0x129D, 40);
        }
        (1, 1) => {
            let t = a1 % 4;
            m.mem.w16(DS, 0x1270, 0);
            item(m, si, t, 0x41 | 0x08 | (a2 % 4) << 4);
            item(m, other, t, 0xC1 | 0x08 | ((a2 >> 2) % 4) << 4);
        }
        (2, 1) => {
            let t = a1 % 4;
            item(m, si, t, 0x41 | 0x08 | (1 + a2 % 3) << 4);
            m.mem.w8(DS, 0x7F81 + t as u16, 0);
        }
        (2, 2) | (2, 3) => {
            let cost = 15 * (a2 % 50) as u16 + (a1 % 15) as u16;
            let ty = if aim == 3 { 0x80 + a1 % 0x0D } else { a1 % 0x12 };
            let share = if aim == 3 { cost >> 1 } else { cost / 15 };
            item(m, si, ty, 0x40 | ((a1 >> 7) & 1) << 2);
            m.mem.w8(DS, si + 0x11, a1);
            m.mem.w16(DS, si + 6, share.wrapping_add(((a2 >> 6) % 3) as u16).wrapping_sub(1));
            let base = if a1 & 0x80 != 0 { 0x2A80 } else { 0 };
            m.mem.w16(DATA, base + (a1 & 0x7F) as u16 * 0x55 + 0x32, cost);
        }
        (3, 1) => m.mem.w16(DS, si + 6, (a1 % 0x70) as u16),
        (5, 1) | (5, 2) => {
            let (c, lines) = (a1 % 41, a2 % 12);
            let ax = c as i32 + lines as i32 - 7;
            let delta = [-1, 0, 1, 5, 6, 7][((a1 >> 5) % 6) as usize];
            m.mem.w8(DS, 0x129A, 0);
            m.mem.w8(DS, 0x1299, 0);
            m.mem.w8(DS, 0x050A, c);
            m.mem.w16(DS, 0x126C, lines as u16);
            m.mem.w8(DS, 0x1297, (ax + delta) as u8);
        }
        (5, 3) => {
            m.mem.w16(DS, 0xC39A, 0);
            m.mem.w16(DS, 0xC398, 0);
            m.mem.w8(DS, 0x05E6, 4);
            m.mem.w8(DS, 0x0D48, [8, 0x0B, 1, 3, 2, 5][(a1 % 6) as usize]);
        }
        _ => {}
    }
    (slot, extra)
}

/// The smallest attribute byte that is `p` percent (1194:5B07).
fn attr_for(p: u16) -> u8 {
    (0..=255u8).find(|&v| arena::scale::to100(v as i16) as u16 == p).unwrap_or(0)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut files = Files::new(std::path::Path::new(".")).expect("files");
    let mut n = 0;
    for line in include_str!("vectors/useitem.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "useitem", "{line}");
        let op: u8 = f[1].parse().unwrap();
        let mut m = base.clone();
        let (slot, extra) = setup(&mut m, u32::from_str_radix(f[2], 16).unwrap(), op);
        let pick = if extra >= 0xF0 { 0xFFFF } else { (extra % 12) as u16 };
        m.mem.w16(DS, 0xFFDC, pick);
        m.load_rng();
        let before = m.clone();
        let si = 0x0212 + 0x13 * slot;
        let mut hooks = Log { pick };
        let ok = match op {
            0 => {
                m.equip_toggle(si);
                Some(())
            }
            1 => m.equip_click(&mut hooks, slot).map(|_| ()),
            2 => m.item_cast(&mut files, &mut hooks, si),
            3 if extra & 2 != 0 => {
                m.open_charm(&mut hooks, si);
                Some(())
            }
            3 => m.gold_charm(&mut hooks, si),
            4 => m.use_item(&mut files, &mut hooks),
            _ => {
                m.drop_item();
                Some(())
            }
        };
        let Some(()) = ok else {
            assert!(f[3] == "fault" || f[3] == "hang", "{line}: the port gave up");
            n += 1;
            continue;
        };
        m.store_rng();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let frame = (0..=0xFFFFu16).filter(|&o| m.mem.r8(FRAME, o) != 0).flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(FRAME, o)]);
        assert_eq!(f[3], format!("{:x}", fnv(changed.chain(frame))), "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n}");
}

/// Mirrors harness/gen_dropbtn.py `button` after `setup` with op 5: the
/// question's answer.
fn button(m: &mut Machine, seed: u32) -> u8 {
    let mut s = Stream(seed ^ 0x5F13);
    let mut flags = s.word() & !0x20;
    if s.byte() < 0x30 {
        flags |= 0x20;
    }
    m.mem.w16(DS, 0x05E0, flags);
    let lost = if s.byte() < 0x60 { s.byte() } else { 0 };
    m.mem.w8(DS, 0x12D0, lost);
    let v = s.word();
    m.mem.w16(DS, 0x1026, v);
    let v = s.word();
    m.mem.w16(DS, 0x102C, v);
    let answer = s.byte() & 1;
    let _width = s.byte();
    let own = (s.byte() < 0xD0) as u8;
    m.mem.w8(DS, 0x129A, own);
    if s.byte() < 0x30 {
        m.mem.w8(DS, 0x1296, 0xFF);
    }
    answer
}

/// Mirrors harness/gen_dropbtn.py `place` (the planes at 7000h and
/// 8000h).
fn place(m: &mut Machine, seed: u32) {
    let mut s = Stream(seed);
    let mut sq = s.word() & 0xFFFE;
    if sq == 0xFFFE {
        sq = 0;
    }
    m.mem.w16(DS, 0xCB90, sq);
    m.mem.w16(DS, 0xD806, 0x7000);
    m.mem.w16(DS, 0xCCD0, 0x8000);
    let b = if s.byte() < 0x40 { s.byte() } else { 0 };
    m.mem.w8(0x7000, sq, b);
    let fl = s.word();
    m.mem.w16(0x8000, sq, fl);
    let mut sub = s.word();
    if s.byte() < 0x80 {
        sub = fl.wrapping_sub(0xC00).wrapping_sub((s.byte() % 3) as u16).wrapping_add(1);
    }
    m.mem.w16(DS, 0x9030, sub);
    let mut squares: Vec<u16> = (0..0x17).map(|_| s.word()).collect();
    if s.byte() < 0x40 {
        squares[(s.byte() % 0x17) as usize] = sq;
    }
    for (i, v) in squares.into_iter().enumerate() {
        m.mem.w16(DS, 0x1951 + 2 * i as u16, v);
    }
    let v = s.word();
    m.mem.w16(DS, 0x05E0, v);
    for at in [0xC39A, 0xC398] {
        let v = if s.byte() < 0x40 { s.word() } else { s.word() & !0x214 };
        m.mem.w16(DS, at, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x12D0, v);
}

/// The drop button (1194:17D8, `drop_button`) and the place's check
/// before the sheet (1194:5F13, `drop_place`) against
/// harness/gen_dropbtn.py's vectors; the questions logged by their SI
/// (DROP_TEXT).
#[test]
fn drop_button_matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/dropbtn.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "dropbtn", "{line}");
        let op: u8 = f[1].parse().unwrap();
        let seed = u32::from_str_radix(f[2], 16).unwrap();
        let mut m = base.clone();
        let mut asked: Vec<u16> = Vec::new();
        let before;
        if op == 0 {
            setup(&mut m, seed, 5);
            let answer = button(&mut m, seed);
            m.load_rng();
            before = m.clone();
            let r = m.drop_button(&mut |_m| {
                asked.push(arena::use_item::var::DROP_TEXT);
                answer == 0
            });
            if r.is_none() {
                assert!(f[3] == "fault" || f[3] == "hang", "{line}: the port gave up");
                n += 1;
                continue;
            }
            m.store_rng();
        } else {
            place(&mut m, seed);
            before = m.clone();
            m.drop_place();
        }
        let r: Vec<&str> = f[3].split(' ').collect();
        assert_eq!(r.len(), 2, "{line}: the original gave up, the port didn't");
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let frame = (0..=0xFFFFu16).filter(|&o| m.mem.r8(FRAME, o) != 0).flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(FRAME, o)]);
        assert_eq!(r[0], format!("{:x}", fnv(changed.chain(frame))), "{line}: DGROUP");
        let log: Vec<u8> = asked.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(r[1], format!("{:x}", fnv(log)), "{line}: questions");
        n += 1;
    }
    assert!(n > 3500, "{n}");
}
