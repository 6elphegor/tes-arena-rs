//! Differential tests of the main quest's person in a town
//! (src/sidequest.rs `quest_person_talk`, 1194:D593) against vectors from
//! the original code (harness/gen_questperson.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The generator's stubs: each call logged, the answers the case's.
struct Log {
    log: Vec<u16>,
    pick: u16,
    temple: Option<Vec<u8>>,
}

impl QuestHooks for Log {
    fn text_at(&mut self, m: &mut Machine, _files: &mut Files, mode: u16, _di: u16) -> Option<u16> {
        self.log.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16, mode]);
        Some(0)
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([2, si]);
    }
    fn substitute(&mut self, m: &mut Machine, _files: &mut Files, di: u16) -> Option<()> {
        self.log.extend([3, di]);
        if let Some(t) = &self.temple {
            for (i, &b) in t.iter().chain(&[0]).enumerate() {
                m.mem.w8(DS, di.wrapping_add(i as u16), b);
            }
        }
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
    fn menu(&mut self, _m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([7, si, bx, di]);
        self.pick
    }
}

/// The `n`-th NUL-ended string from DS:`at` (1194:5585).
fn nth_string(m: &Machine, at: u16, n: u16) -> Vec<u8> {
    let at = m.skip_strings(DS, at, n);
    m.mem.cstr(DS, at).to_vec()
}

/// Mirrors harness/gen_questperson.py `setup` (a conditional's test byte
/// comes first, `and` short-circuits, a list's items before its index);
/// returns the menu's pick and the temple's name.
fn setup(m: &mut Machine, seed: u32) -> (u16, Option<Vec<u8>>) {
    let mut st = Stream(seed);
    let s = &mut st;
    let stage = s.byte() % 9;
    m.mem.w8(DS, 0x0F77, stage);
    let flags = [s.byte() & 1, s.byte() & 1, s.byte(), s.byte() & 1, s.byte() & 1, s.byte() & 1, s.byte(), s.byte()];
    for (i, &b) in flags.iter().enumerate() {
        m.mem.w8(DS, 0x0F78 + i as u16, b);
    }
    let v = if s.byte() < 0xC0 { m.mem.r8(DS, 0x80EC + stage as u16) } else { s.byte() % 9 };
    m.mem.w8(DS, 0x0A46, v);
    let town = nth_string(m, 0x80F4, stage as u16);
    let name: Vec<u8> = if s.byte() < 0xC0 {
        town.iter()
            .map(|&c| {
                if (0x41..=0x5A).contains(&(c & 0xDF)) && s.byte() < 0x40 { c ^ 0x20 } else { c }
            })
            .collect()
    } else {
        let n = s.byte() % 8;
        (0..n).map(|_| 0x41 + s.byte() % 26).collect()
    };
    for (i, &b) in name.iter().chain(&[0]).enumerate() {
        m.mem.w8(DS, 0x0A22 + i as u16, b);
    }
    let v = if s.byte() < 0x80 { 4 } else { s.byte() };
    m.mem.w8(DS, 0x05E6, v);
    let w = s.word() & !4;
    let w = w | if s.byte() < 0x30 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, w);
    let other = s.byte();
    let v = [1u8, 5, 7, 4, other][s.byte() as usize % 5];
    m.mem.w8(DS, 0x0D48, v);
    let v = if s.byte() < 0x10 { s.byte() } else { s.byte() % 20 };
    m.mem.w8(DS, 0x01AA, v);
    for i in 0..0x4CCu16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1429 + i, b);
    }
    let v = s.byte() % 100;
    m.mem.w8(DS, 0x1A6C, v);
    let lo = s.word() as u32;
    let v = lo | (s.word() as u32) << 16;
    m.mem.w32(DS, 0x91F6, v);
    let mut text = Vec::new();
    for _ in 0..s.byte() % 120 {
        let k = s.byte();
        text.push(if k < 0x18 {
            0x0D
        } else if k < 0x30 {
            0x0A
        } else {
            0x20 + s.byte() % 0x5F
        });
    }
    for (i, &b) in text.iter().chain(&[0]).enumerate() {
        m.mem.w8(DS, 0x9622 + i as u16, b);
    }
    let pick = [0u16, 0, 1, 0xFFFF][s.byte() as usize % 4];
    let temple = if s.byte() < 0xA0 { Some(nth_string(m, 0x822B, stage as u16)) } else { None };
    (pick, temple)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    // The hooks read no files: any directory will do.
    let files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    let base = Machine::new(&img);
    let (mut n, mut stuck, mut met) = (0, 0, 0);
    for line in include_str!("vectors/questperson.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "questperson", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (pick, temple) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let mut hooks = Log { log: Vec::new(), pick, temple };
        let Some(()) = m.quest_person_talk(&mut fs, &mut hooks) else {
            assert_eq!(f[2], "stuck", "{line}");
            stuck += 1;
            continue;
        };
        m.store_rng();
        // 26F7:0C0A's record of its caller's CX and SI, left out.
        for at in [0x1296u16, 0x12A0] {
            let v = before.mem.r16(DS, at);
            m.mem.w16(DS, at, v);
        }
        let ds = DS as usize * 16;
        let changed = m.mem.diff(&before.mem, ds..ds + 0x10000).into_iter().flat_map(|(i, v)| {
            let o = (i - ds) as u16;
            [o as u8, (o >> 8) as u8, v]
        });
        met += hooks.log.contains(&7) as u32;
        let lg = hooks.log.iter().flat_map(|v| v.to_le_bytes());
        assert_eq!(f[2], format!("{:x} {:x}", fnv(changed), fnv(lg)), "{line}");
        n += 1;
    }
    eprintln!("questperson: {n} compared, {met} asked, {stuck} stuck");
    assert!(n > 3500, "{n}");
}
