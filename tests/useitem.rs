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
    (slot, extra)
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
