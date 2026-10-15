//! Differential tests of a talk's quests (src/sidequest.rs `quests_talk`,
//! 1194:8F1E) against vectors from the original code
//! (harness/gen_questtalk.py).
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
    answer: u16,
    pick: u16,
}

impl Log {
    /// The text's call: its number and flag, AX, and what its
    /// substitutions would read (the player's square, the RNG's seed).
    fn text_log(&mut self, m: &Machine, ax: u16) {
        let seed = m.rng.seed;
        self.log.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16, ax]);
        self.log.extend([m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4), seed as u16, (seed >> 16) as u16]);
    }
}

impl QuestHooks for Log {
    fn text(&mut self, m: &mut Machine) {
        self.text_log(m, 2);
    }
    fn text_shown(&mut self, m: &mut Machine) {
        self.text_log(m, 0);
    }
    fn text_at(&mut self, m: &mut Machine, _files: &mut Files, mode: u16, _di: u16) -> Option<u16> {
        self.text_log(m, mode);
        Some(self.answer)
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([2, si]);
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
    fn menu(&mut self, _m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([7, si, bx, di]);
        self.pick
    }
    fn travel(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        self.log.push(9);
        Some(())
    }
}

/// Mirrors harness/gen_questtalk.py `setup` (a conditional's test byte
/// comes first, a list's items before its index); returns the answers.
fn setup(m: &mut Machine, seed: u32) -> (u16, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    let w = s.word() & !0x4020;
    let w = w | if s.byte() < 0x10 { 0x4000 } else { 0 };
    let w = w | if s.byte() < 0x40 { 0x20 } else { 0 };
    m.mem.w16(DS, 0x05C8, w);
    let v = if s.byte() < 0x60 { 4 } else { s.byte() };
    m.mem.w8(DS, 0x137F, v);
    let v = if s.byte() < 0xA0 { 4 } else { s.byte() };
    m.mem.w8(DS, 0x05E6, v);
    let building = if s.byte() < 0x60 { 1 } else { s.byte() };
    m.mem.w8(DS, 0x0D48, building);
    let here = s.byte();
    m.mem.w8(DS, 0x0A4C, here);
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0A46, v);
    for at in [0x0A48u16, 0x0A4A, 0xC3D0, 0xC3D4] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let talker = s.word();
    m.mem.w16(DS, 0x0FD2, talker);
    let who = talker & 0xFF00 | (talker & 0xFF) >> 1;
    for (base, size) in [(0x0DCAu16, 0x29u16), (0x0E6E, 0x34)] {
        for i in 0..4 {
            let mut rec = vec![0u8; size as usize];
            for b in rec.iter_mut() {
                *b = s.byte();
            }
            let kind = [0u16, 1, 1, 2, 2, 3][s.byte() as usize % 6];
            let stage = (s.byte() % 8) as u16;
            let k = s.word() & !0x0EC0 | kind << 6 | stage << 9;
            rec[4..6].copy_from_slice(&k.to_le_bytes());
            if s.byte() < 0xA0 {
                rec[0x24] = here;
            }
            let person = if building == 1 && s.byte() < 0x80 { 0xAAAA } else { who };
            if s.byte() < 0xA0 {
                rec[0x10..0x12].copy_from_slice(&person.to_le_bytes());
            }
            if s.byte() < 0xA0 {
                rec[0x14..0x16].copy_from_slice(&person.to_le_bytes());
            }
            for (j, &b) in rec.iter().enumerate() {
                m.mem.w8(DS, base + i * size + j as u16, b);
            }
        }
    }
    let v = if s.byte() < 0x10 { s.byte() } else { s.byte() % 40 };
    m.mem.w8(DS, 0x01AA, v);
    let v = dword(s);
    m.mem.w32(DS, 0x05C2, v);
    let v = dword(s);
    m.mem.w32(DS, 0x05AD, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CA, v);
    let v = s.byte();
    m.mem.w8(DS, 0x8A56, v);
    let v = s.word();
    m.mem.w16(DS, 0x0AB0, v);
    for i in 0..9u16 {
        let v = if s.byte() < 0x80 { 0xFF } else { s.byte() };
        m.mem.w8(DS, 0x0FD4 + i, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x0D39, v);
    for i in 0..16 * 0x19u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1445 + i, b);
    }
    for i in 0..0x4CCu16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1429 + i, b);
    }
    let v = s.byte() % 100;
    m.mem.w8(DS, 0x1A6C, v);
    let v = (s.byte() % 40) as u16;
    m.mem.w16(DS, 0x1927, v);
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    let answer = [0u16, 0, 1, 2][s.byte() as usize % 4];
    let pick = [0u16, 0, 1, 0xFFFF][s.byte() as usize % 4];
    (answer, pick)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    // The hooks read no files: any directory will do.
    let files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    let base = Machine::new(&img);
    let (mut n, mut stuck, mut logged) = (0, 0, 0);
    for line in include_str!("vectors/questtalk.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "questtalk", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (answer, pick) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let mut hooks = Log { log: Vec::new(), answer, pick };
        let Some(()) = m.quests_talk(&mut fs, &mut hooks) else {
            assert_eq!(f[2], "stuck", "{line}");
            stuck += 1;
            continue;
        };
        m.store_rng();
        let ds = DS as usize * 16;
        let changed = m.mem.diff(&before.mem, ds..ds + 0x10000).into_iter().flat_map(|(i, v)| {
            let o = (i - ds) as u16;
            [o as u8, (o >> 8) as u8, v]
        });
        logged += !hooks.log.is_empty() as u32;
        let lg = hooks.log.iter().flat_map(|v| v.to_le_bytes());
        assert_eq!(f[2], format!("{:x} {:x}", fnv(changed), fnv(lg)), "{line}");
        n += 1;
    }
    eprintln!("questtalk: {n} compared, {logged} called a hook, {stuck} stuck");
    assert!(n > 3500, "{n}");
}
