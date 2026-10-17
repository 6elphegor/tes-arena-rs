//! Differential tests of a quest's ambush (src/sidequest.rs
//! `quest_ambush`, 1194:86F2) against vectors from the original code
//! (harness/gen_ambush.py).
mod common;

use arena::bsa::Files;
use arena::encounter::FightHooks;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

const WSEG: u16 = 0x6800;
const FSEG: u16 = 0x7000;
const CSEG: u16 = 0x7800;
const FRAME: u16 = 0x8800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's hooks: the fight's calls counted at FFE0..FFE4, the
/// text's logged.
struct Hooks(Vec<u16>);

impl FightHooks for Hooks {
    fn fight_graphics(&mut self, m: &mut Machine, _files: &mut Files, people: bool) -> Option<()> {
        count(m, if people { 0xFFE0 } else { 0xFFE2 });
        Some(())
    }
    fn fight_music(&mut self, m: &mut Machine, _files: &mut Files, _music: u8) -> Option<()> {
        count(m, 0xFFE4);
        Some(())
    }
}

impl QuestHooks for Hooks {
    fn text_at(&mut self, m: &mut Machine, _files: &mut Files, mode: u16, _di: u16) -> Option<u16> {
        self.0.extend([1, m.mem.r16(DS, 0x137A), m.mem.r8(DS, 0x137E) as u16, mode]);
        Some(0)
    }
}

/// The two hooks' halves, borrowed apart.
struct Fight<'a>(&'a mut Hooks);

impl FightHooks for Fight<'_> {
    fn fight_graphics(&mut self, m: &mut Machine, files: &mut Files, people: bool) -> Option<()> {
        self.0.fight_graphics(m, files, people)
    }
    fn fight_music(&mut self, m: &mut Machine, files: &mut Files, music: u8) -> Option<()> {
        self.0.fight_music(m, files, music)
    }
}

/// Mirrors harness/gen_ambush.py `setup` (a conditional's test byte
/// comes first, `and` short-circuits, a list's items before its index).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let px = 0x1000 + s.word() % 0x2000;
    let py = 0x1000 + s.word() % 0x2000;
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let sq = ((py >> 7) << 8) + ((px >> 7) << 1);
    m.mem.w16(DS, 0xCB90, sq);
    let v = (s.byte() % 24) as u16;
    m.mem.w16(DS, 0x05D6, v);
    let v = s.byte() % 6;
    m.mem.w8(DS, 0x05E6, v);
    let v = if s.byte() < 0x30 { 1 } else { 0 };
    m.mem.w8(DS, 0x1013, v);
    let v = s.byte() % 12;
    m.mem.w8(DS, 0x0D48, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    let v = if s.byte() < 0x08 {
        0x200
    } else if s.byte() < 0x40 {
        0x40
    } else {
        0
    };
    m.mem.w16(DS, 0x05CA, v);
    let v = if s.byte() < 0x08 { 1 } else { 0 };
    m.mem.w16(DS, 0x0FF7, v);
    let v = if s.byte() < 0x08 { 1 } else { 0 };
    m.mem.w16(DS, 0x0FFB, v);
    let v = if s.byte() < 0xC0 { 0 } else { 1 + (s.byte() % 3) as u16 };
    m.mem.w16(DS, 0x0FF9, v);
    let v = s.byte() % 30;
    m.mem.w8(DS, 0x01AA, v);
    let v = if s.byte() < 0xC0 { 1 } else { s.byte() };
    m.mem.w8(DS, 0x1404, v);
    for i in 0..8u16 {
        let si = 0x0C57 + 0x1C * i;
        let w = s.word() & 0x3FFF;
        let v = w | if s.byte() < 0xC0 { 0x4000 } else { 0 };
        m.mem.w16(DS, si + 0xF, v);
    }
    for q in 0..4u16 {
        let e = 0x0E6E + 0x34 * q;
        let kind = if q == 0 && s.byte() < 0xA0 { 1 } else { [0u16, 1, 1, 2, 3][s.byte() as usize % 5] };
        let stage = if s.byte() < 0x30 { 6 } else { (s.byte() % 8) as u16 };
        m.mem.w16(DS, e + 4, kind << 6 | stage << 9);
        let v = (s.byte() % 8) as u16;
        m.mem.w16(DS, e + 0x22, v);
        let v = if s.byte() < 0x80 { 0x18 + s.byte() % 8 } else { s.byte() % 0x18 };
        m.mem.w8(DS, e + 0x2F, v);
        let v = if s.byte() < 0x80 { 0x18 + s.byte() % 8 } else { s.byte() % 0x18 };
        m.mem.w8(DS, e + 0x30, v);
    }
    for dy in -8i32..=8 {
        for dx in -8i32..=8 {
            let at = ((((py >> 7) as i32 + dy) << 8) + (((px >> 7) as i32 + dx) << 1)) as u16;
            let v = if s.byte() < 0xC0 { 0 } else { s.word() & 0x7FFF };
            m.mem.w16(WSEG, at, v);
        }
    }
    let v = if s.byte() < 0xE0 { 0 } else { 0x0102 * (1 + (s.byte() % 0x40) as u16) };
    m.mem.w16(WSEG, sq, v);
    let v = s.byte() % 0x30;
    m.mem.w8(CSEG, 4, v);
    let v = s.byte() % 8;
    m.mem.w8(CSEG, 5, v);
    let v = s.word() & 0x1000;
    m.mem.w16(CSEG, 0x40D, v);
    let mut seed4 = [s.byte(), s.byte(), s.byte(), s.byte()];
    if s.byte() < 0x80 {
        let h = (s.byte() % 16) as u32 + 100 * (s.word() % 655) as u32;
        let next = h << 16 | s.word() as u32;
        seed4 = next.wrapping_mul(0x3F15_4FA5).to_le_bytes();
    }
    m.mem.w32(DS, 0x91F6, u32::from_le_bytes(seed4));
}

/// The game's directory: ARENA_EXE's, or ESarena/ next to this crate.
fn game_dir() -> std::path::PathBuf {
    std::env::var("ARENA_EXE")
        .ok()
        .and_then(|p| std::path::Path::new(&p).parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    for (o, v) in [
        (0xD806u16, WSEG),
        (0xCCD0, FSEG),
        (0x1042, CSEG),
        (0x9030, FRAME),
        (0xCCCE, FRAME + 0xC00),
        (0x91F0, 0),
        (0x05C8, 0),
        (0xC38E, 0xC0),
        (0xC357, 0x80),
        (0xC359, 0x80),
    ] {
        base.mem.w16(DS, o, v);
    }
    base.heap_init(0xA000, 0xF000).unwrap();
    let Ok(mut files) = Files::new(&game_dir()) else {
        eprintln!("skipping: no game directory");
        return;
    };
    let (mut n, mut stuck, mut ambushed) = (0, 0, 0);
    for line in include_str!("vectors/ambush.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "ambush", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let mut hooks = Hooks(Vec::new());
        let mut quest = Hooks(Vec::new());
        let got = m.quest_ambush(&mut files, &mut Fight(&mut hooks), &mut quest);
        let Some(()) = got else {
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
        let heap = (0xA000u16..0xF000).flat_map(|seg| (0..16u16).map(move |o| (seg, o)));
        ambushed += !quest.0.is_empty() as u32;
        let lg = quest.0.iter().flat_map(|v| v.to_le_bytes());
        let got = format!(
            "{:x} {:x} {:x} {:x}",
            fnv(changed),
            fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
            fnv(heap.map(|(seg, o)| m.mem.r8(seg, o))),
            fnv(lg)
        );
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    eprintln!("ambush: {n} compared, {ambushed} ambushes, {stuck} stuck");
    assert!(n > 3500, "{n}");
}
