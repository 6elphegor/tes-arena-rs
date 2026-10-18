//! Differential tests of a quest's item picked up (src/quest.rs
//! `quest_item_taken`, 1194:9F04) and the main quest's chest
//! (`quest_chest`, 2294:0F33) against vectors from the original code
//! (harness/gen_mainquest.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const FSEG: u16 = 0x6000;
const RECORD: u16 = 0x3D96;

/// Mirrors harness/gen_mainquest.py `setup` (a conditional's test byte
/// comes first, `or` short-circuits, a list's items before its index);
/// returns the op.
fn setup(m: &mut Machine, seed: u32, step_8e: u8) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() & 1;
    let v = if s.byte() < 0xD8 { 3 } else { s.byte() };
    m.mem.w8(DS, 0x05E6, v);
    let province = s.byte() % 8;
    let place = s.byte();
    m.mem.w8(DS, 0x0A46, province);
    m.mem.w8(DS, 0x0A4C, place);
    let local = place & 0x1F;
    let col = if s.byte() < 0xC0 { s.byte() as u16 % 64 } else { s.byte() as u16 };
    let row = s.byte() as u16 % 64;
    let x = col << 7 | (s.byte() & 0x7F) as u16;
    m.mem.w16(DS, RECORD, x);
    let y = row << 7 | (s.byte() & 0x7F) as u16;
    m.mem.w16(DS, RECORD + 4, y);
    let square = row << 8 | ((col * 2) & 0xFF);
    let mut block = [0u8; 0x13];
    for b in block.iter_mut() {
        *b = s.byte();
    }
    block[0] = if s.byte() < 0x50 { step_8e } else { s.byte() % 16 };
    match s.byte() % 3 {
        0 => {
            block[5] = local;
            block[6] = province;
        }
        1 => {
            block[7] = local;
            block[8] = province;
            block[6] = (province + 1 + s.byte() % 7) % 8;
        }
        _ => {}
    }
    let t = if s.byte() < 0xC0 { square } else { s.word() };
    block[0x11..0x13].copy_from_slice(&t.to_le_bytes());
    for (i, &b) in block.iter().enumerate() {
        m.mem.w8(DS, 0x0FDD + i as u16, b);
    }
    for i in 0..9u16 {
        let v = if s.byte() < 0x60 { 0xFF } else { s.byte() % 0x50 };
        m.mem.w8(DS, 0x0FD4 + i, v);
    }
    for i in 0..4u16 {
        let at = 0x0E6E + i * 0x34;
        for k in 0..0x34 {
            let b = s.byte();
            m.mem.w8(DS, at + k, b);
        }
        let kind = if s.byte() < 0xC0 { 1 } else { (s.byte() % 4) as u16 };
        let stage = if s.byte() < 0x80 { 5 } else { (s.byte() % 8) as u16 };
        let w = s.word();
        m.mem.w16(DS, at + 4, w & 0xF13F | kind << 6 | stage << 9);
        let v = if s.byte() < 0x80 { local } else { s.byte() };
        m.mem.w8(DS, at + 0x14, v);
        let v = if s.byte() < 0x80 { 0 } else { s.byte() };
        m.mem.w8(DS, at + 0x28, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x01AA, v);
    let full = s.byte() < 0x50;
    for i in 0..40u16 {
        let at = 0x0212 + i * 0x13 + 0x0E;
        let f = s.byte();
        let f = if full || s.byte() < 0xA0 { f | 0x40 } else { f & !0x40 };
        m.mem.w8(DS, at, f);
    }
    let seed = u32::from_le_bytes([s.byte(), s.byte(), s.byte(), s.byte()]);
    m.mem.w32(DS, 0x91F6, seed);
    for i in 0..0x4CCu16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1429 + i, b);
    }
    for i in 0..0x13u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x13DA + i, b);
    }
    op
}

/// Logs what the generator's stubs log.
#[derive(Default)]
struct Log(Vec<u16>);

impl QuestHooks for Log {
    fn text(&mut self, m: &mut Machine) {
        self.0.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16, 2]);
    }
    fn text_shown(&mut self, m: &mut Machine) {
        self.0.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16, 0]);
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.0.extend([2, si]);
    }
    fn expand(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files, di: u16) {
        self.0.extend([3, di]);
    }
    fn province_load(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files, province: u8) {
        self.0.extend([4, province as u16]);
    }
    fn entry_save(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files, si: u16) {
        self.0.extend([5, si, 0x19, m.mem.r16(DS, 0x129E)]);
    }
    fn log_entry(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files, si: u16) -> Option<()> {
        self.0.extend([6, si]);
        Some(())
    }
    fn finale(&mut self, _m: &mut Machine) {
        self.0.extend([7]);
    }
    fn inventory_full(&mut self, _m: &mut Machine, ax: u16) {
        self.0.extend([8, ax]);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, FSEG);
    let step_8e = (0..16u16).find(|&i| base.mem.r8(DS, 0x78C3 + i) == 0x8E).unwrap_or(16) as u8;
    let (mut n, mut chests) = (0, 0);
    // Next dungeon, the end, a piece given, inventory full, items taken.
    let mut paths = [0u32; 5];
    for line in include_str!("vectors/mainquest.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "mainquest", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap(), step_8e);
        assert_eq!(f[2], op.to_string(), "{line}");
        m.load_rng();
        let before = m.clone();
        let mut log = Log::default();
        let cf = if op == 0 { m.quest_item_taken(&mut arena::bsa::Files::default(), &mut log) } else { m.quest_chest(&mut arena::bsa::Files::default(), &mut log, RECORD) };
        let Some(cf) = cf else {
            assert!(f[3] == "hang" || f[3] == "fault", "{line}");
            continue;
        };
        m.store_rng();
        chests += (op == 1 && cf) as u32;
        for (i, t) in [0x1F4u16, 0x1FA, 0x1F5].into_iter().enumerate() {
            paths[i] += log.0.windows(2).any(|w| w == [1, t]) as u32;
        }
        paths[3] += log.0.contains(&8) as u32;
        paths[4] += (op == 0 && !cf) as u32;
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let floor = (0..=0xFFFFu16).map(|o| m.mem.r8(FSEG, o));
        let lg = log.0.iter().flat_map(|v| v.to_le_bytes());
        let got = format!("{} {:x} {:x} {:x}", cf as u8, fnv(changed), fnv(floor), fnv(lg));
        assert_eq!(f[3], got, "{line}");
        n += 1;
    }
    eprintln!("mainquest: {n} compared, {chests} chests; next dungeon, end, piece, full, item: {paths:?}");
    assert!(n > 3500, "{n}");
    assert!(paths.iter().all(|&p| p > 20), "{paths:?}");
}
