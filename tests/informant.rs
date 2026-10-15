//! Differential tests of the main quest's informant (src/sidequest.rs
//! `quest_informant`, 1194:CED7) against vectors from the original code
//! (harness/gen_informant.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const FSEG: u16 = 0x6000;

/// The generator's stubs: each call logged, the answers the case's.
struct Log {
    log: Vec<u16>,
    cf: bool,
    answer: u16,
}

impl QuestHooks for Log {
    fn text_at(&mut self, m: &mut Machine, _files: &mut Files, mode: u16, _di: u16) -> Option<u16> {
        self.log.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16, mode]);
        Some(0)
    }
    fn substitute(&mut self, _m: &mut Machine, _files: &mut Files, di: u16) -> Option<()> {
        self.log.extend([3, di]);
        Some(())
    }
    fn province_load(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files, province: u8) {
        self.log.extend([4, province as u16]);
    }
    fn entry_save(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files, si: u16) {
        self.log.extend([5, si, 0x19, m.mem.r16(DS, 0x129E)]);
    }
    fn log_entry(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([6, si]);
    }
    fn informant_haggle(&mut self, _m: &mut Machine, _files: &mut Files, price: u32, per: u16, mode: u8) -> Option<bool> {
        self.log.extend([10, price as u16, (price >> 16) as u16, per, mode as u16]);
        Some(self.cf)
    }
    fn frame_load(&mut self, _m: &mut Machine, _files: &mut Files, name: u16) -> Option<()> {
        self.log.extend([11, name]);
        Some(())
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([12, si]);
    }
    fn dialog(&mut self, _m: &mut Machine, kind: u8) -> u16 {
        self.log.extend([13, kind as u16]);
        self.answer
    }
    fn notice(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([14, si]);
    }
}

/// Mirrors harness/gen_informant.py `setup` (a conditional's test byte
/// comes first, a list's items before its index); returns the haggle's
/// carry and the dialog's AX.
fn setup(m: &mut Machine, seed: u32) -> (bool, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    let w = s.word() & !0x4000;
    let w = w | if s.byte() < 0x10 { 0x4000 } else { 0 };
    m.mem.w16(DS, 0x05C8, w);
    let talker = s.word();
    m.mem.w16(DS, 0x0FD2, talker);
    let who = talker & 0xFF00 | (talker & 0xFF) >> 1;
    let v = s.byte() % 20;
    m.mem.w8(DS, 0x0FDD, v);
    let v = if s.byte() < 0xC0 { who } else { s.word() };
    m.mem.w16(DS, 0x0FDE, v);
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0FE3, v);
    let mut f = s.byte() & 0xF1;
    f |= if s.byte() < 0xD0 { 2 } else { 0 };
    f |= if s.byte() < 0x30 { 4 } else { 0 };
    f |= if s.byte() < 0x80 { 1 } else { 0 };
    f |= if s.byte() < 0x80 { 8 } else { 0 };
    m.mem.w8(DS, 0x0FE6, f);
    let met = dword(s);
    let days = (s.byte() % 10) as u32;
    m.mem.w32(DS, 0x0FE9, met);
    m.mem.w8(DS, 0x0FED, days as u8);
    let fee = s.word() % 2000;
    m.mem.w16(DS, 0x0FEE, fee);
    let passed = (days + (s.byte() % 5) as u32 + 0x100 - 2) % 0x100;
    let t = met.wrapping_add(passed * 0x5A0).wrapping_add((s.word() % 0x5A0) as u32);
    m.mem.w32(DS, 0x05CC, t);
    let k = s.byte();
    let gold = if k < 0x30 {
        fee as u32
    } else if k < 0xA0 {
        fee as u32 + (s.word() % 500) as u32
    } else {
        (s.word() % (fee + 1)) as u32
    };
    m.mem.w32(DS, 0x05C2, gold);
    let v = s.byte();
    m.mem.w8(DS, 0x01D3, v);
    for i in 0..0x4CCu16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1429 + i, b);
    }
    let v = s.byte() % 100;
    m.mem.w8(DS, 0x1A6C, v);
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    m.mem.w16(DS, 0x9030, FSEG);
    if s.byte() < 0x80 {
        for i in 0..0x2000u16 {
            let k = s.byte();
            let b = if k < 0x04 {
                0
            } else if k < 0x30 {
                0x20
            } else {
                s.byte()
            };
            m.mem.w8(FSEG, i, b);
        }
    } else {
        let mut at = 0u16;
        while at < 0x1F00 {
            let n = [0x3Au16, 0x3B, 0x3C, 0x3D, 0x3E, 0x77, 0x78, 0x79][s.byte() as usize % 8];
            let mut text: Vec<u8> = (0..n).map(|_| 0x41 + s.byte() % 26).collect();
            let sp = s.byte() as usize % n as usize;
            text[sp] = 0x20;
            for (i, &b) in text.iter().enumerate() {
                m.mem.w8(FSEG, at + i as u16, b);
            }
            at += n + 1;
        }
    }
    let v = dword(s);
    m.mem.w32(FSEG, 0x91F6, v);
    let cf = s.byte() & 1 != 0;
    let answer = [0u16, 0, 1, 2, 3][s.byte() as usize % 5];
    (cf, answer)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    // The hooks read no files: any directory will do.
    let files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    let base = Machine::new(&img);
    let (mut n, mut stuck, mut told) = (0, 0, 0);
    for line in include_str!("vectors/informant.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "informant", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (cf, answer) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let mut hooks = Log { log: Vec::new(), cf, answer };
        let Some(carry) = m.quest_informant(&mut fs, &mut hooks) else {
            assert_eq!(f[2], "stuck", "{line}");
            stuck += 1;
            continue;
        };
        m.store_rng();
        if std::env::var("DUMP").is_ok_and(|d| d == f[1]) {
            for o in (0..=0xFFFFu16).filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o)) {
                println!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
            }
            continue;
        }
        let ds = DS as usize * 16;
        let changed = m.mem.diff(&before.mem, ds..ds + 0x10000).into_iter().flat_map(|(i, v)| {
            let o = (i - ds) as u16;
            [o as u8, (o >> 8) as u8, v]
        });
        told += hooks.log.contains(&6) as u32;
        let lg = hooks.log.iter().flat_map(|v| v.to_le_bytes());
        let frame = fnv((0..=0xFFFFu16).map(|o| m.mem.r8(FSEG, o)));
        assert_eq!(f[2], format!("{} {:x} {:x} {:x}", carry as u8, fnv(changed), fnv(lg), frame), "{line}");
        n += 1;
    }
    eprintln!("informant: {n} compared, {told} told the next dungeon, {stuck} stuck");
    assert!(n > 3500, "{n}");
}
