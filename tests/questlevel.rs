//! Differential tests of a quest dungeon's last level reached
//! (src/quest.rs `quest_level`, 1194:9FAC) against vectors from the
//! original code (harness/gen_questlevel.py).
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

fn game_dir() -> std::path::PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| std::path::PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

/// Mirrors harness/gen_questlevel.py `setup` (a conditional's test byte
/// comes first, a list's items before its index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = if s.byte() < 0xE0 { s.word() & !0x4000 } else { s.word() };
    m.mem.w16(DS, 0x05C8, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CA, v);
    let v = s.word();
    m.mem.w16(DS, 0x05E0, v);
    let v = if s.byte() < 0xE0 { 3 } else { s.byte() };
    m.mem.w8(DS, 0x05E6, v);
    let level = s.byte() % 8;
    m.mem.w8(DS, 0xBB0A, level);
    let v = if s.byte() < 0xE0 { level as u16 + 1 } else { (s.byte() % 9) as u16 };
    m.mem.w16(DS, 0xC355, v);
    let province = s.byte() % 8;
    let place = s.byte();
    m.mem.w8(DS, 0x0A46, province);
    m.mem.w8(DS, 0x0A4C, place);
    let width = 8 + (s.byte() % 57) as u16;
    let depth = 8 + (s.byte() % 57) as u16;
    m.mem.w16(DS, 0xC357, width);
    m.mem.w16(DS, 0xC359, depth);
    let c = s.byte() % 64;
    let count = c + if s.byte() < 0x08 { 0 } else { 1 };
    m.mem.w8(DS, 0xC765, count);
    for i in 0..64u16 {
        let v = match s.byte() % 16 {
            0 => 0xFFFF,
            1 => s.byte() as u16 % width,
            2 => s.word(),
            3 => (1 + s.byte() as u16 % (depth - 1)) << 8 | width,
            _ => {
                let y = 1 + s.byte() as u16 % (depth - 1);
                let x = 1 + s.byte() as u16 % (width - 1);
                y << 8 | x
            }
        };
        m.mem.w16(DS, 0xC6E5 + 2 * i, v);
    }
    for i in 0..0x30u16 {
        let b = s.byte();
        m.mem.w8(DS, 0xC934 + i, b);
    }
    let mut floor = Vec::new();
    for _ in 0..s.byte() % 64 {
        let y = (s.byte() as u16 % depth) << 8;
        let x = (s.byte() as u16 % width) * 2;
        floor.push((y | x, s.word() | 1));
    }
    for i in 0..4u16 {
        let at = 0x0E6E + i * 0x34;
        for k in 0..0x34 {
            let b = s.byte();
            m.mem.w8(DS, at + k, b);
        }
        let kind = if s.byte() < 0xD0 { 1 } else { (s.byte() % 4) as u16 };
        let stage = if s.byte() < 0xD0 { [4u16, 5, 6][s.byte() as usize % 3] } else { (s.byte() % 8) as u16 };
        let ty = if s.byte() < 0xD0 { 7 } else { (s.byte() % 16) as u16 };
        let w = s.word() & 0x011F;
        let goal = if s.byte() < 0x20 { 0x20 } else { 0 };
        m.mem.w16(DS, at + 4, w | goal | kind << 6 | stage << 9 | ty << 12);
        let p = if s.byte() < 0xD0 { province } else { s.byte() % 8 };
        let v = p << 5 | s.byte() % 32;
        m.mem.w8(DS, at + 0x25, v);
        let v = if s.byte() < 0xD0 { place & 0x1F } else { s.byte() };
        m.mem.w8(DS, at + 0x14, v);
        let v = s.byte() % 4;
        m.mem.w8(DS, at + 0x28, v);
        let v = s.byte() % 16;
        m.mem.w8(DS, at + 0x2F, v);
        let v = s.byte() % 16;
        m.mem.w8(DS, at + 0x30, v);
    }
    let lo = s.word() as u32;
    let seed = lo | (s.word() as u32) << 16;
    m.mem.w32(DS, 0x91F6, seed);
    for (sq, v) in floor {
        m.mem.w16(FSEG, sq, v);
    }
}

/// Logs what the generator's name stub logs.
#[derive(Default)]
struct Log(Vec<u16>);

impl QuestHooks for Log {
    fn npc_name(&mut self, _m: &mut Machine, _files: &mut Files, ax: u16, seed: u32, di: u16) -> Option<()> {
        self.0.extend([ax, seed as u16, (seed >> 16) as u16, di]);
        Some(())
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(mut files) = Files::new(&game_dir()) else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, FSEG);
    let (mut n, mut stuck, mut named) = (0, 0, 0);
    for line in include_str!("vectors/questlevel.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "questlevel", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let mut log = Log::default();
        let Some(got) = m.quest_level(&mut files, &mut log) else {
            assert!(f[2] == "hang" || f[2] == "fault", "{line}");
            stuck += 1;
            continue;
        };
        m.store_rng();
        let (cf, ax, bx) = match got {
            Some((ax, bx)) => (1, ax, bx),
            None => (0, 0, 0),
        };
        named += cf;
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let floor = (0..=0xFFFFu16).map(|o| m.mem.r8(FSEG, o));
        let lg = log.0.iter().flat_map(|v| v.to_le_bytes());
        let got = format!("{cf} {ax:x} {bx:x} {:x} {:x} {:x}", fnv(changed), fnv(floor), fnv(lg));
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    eprintln!("questlevel: {n} compared, {named} named, {stuck} hang or fault");
    assert!(n > 3500, "{n}");
}
