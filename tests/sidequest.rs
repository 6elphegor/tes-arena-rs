//! Differential tests of the side quests made on entering a place
//! (src/sidequest.rs `quests_place`, 1194:9358) against vectors from the
//! original code (harness/gen_sidequest.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const TSEG: u16 = 0x7000;
const HEAP: (u16, u16) = (0xA000, 0xF000);
const EXCLUDED: [u8; 7] = [0x00, 0x01, 0x02, 0x08, 0x0F, 0x12, 0x13];

/// The generator's stubs: 3EC3 and 2E18 count their calls and add AL to
/// a byte, 2E69 counts; the timer is the case's.
struct Stubs {
    ticks: u32,
}

impl QuestHooks for Stubs {
    fn city_info(&mut self, m: &mut Machine, _files: &mut Files, ax: u16) -> Option<()> {
        let v = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
        m.mem.w16(DS, 0xFFE0, v);
        let b = m.mem.r8(DS, 0xFFE6).wrapping_add(ax as u8);
        m.mem.w8(DS, 0xFFE6, b);
        Some(())
    }
    fn province_load(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files, province: u8) {
        let v = m.mem.r16(DS, 0xFFE2).wrapping_add(1);
        m.mem.w16(DS, 0xFFE2, v);
        let b = m.mem.r8(DS, 0xFFE7).wrapping_add(province);
        m.mem.w8(DS, 0xFFE7, b);
    }
    fn travel(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        let v = m.mem.r16(DS, 0xFFE4).wrapping_add(1);
        m.mem.w16(DS, 0xFFE4, v);
        Some(())
    }
    fn bios_ticks(&mut self, _m: &mut Machine) -> u32 {
        self.ticks
    }
}

/// Mirrors harness/gen_sidequest.py `setup` (a conditional's test byte
/// comes first, a list's items before its index); returns the timer.
fn setup(m: &mut Machine, seed: u32) -> u32 {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    let place = s.word();
    m.mem.w16(DS, 0xCCA8, place);
    let v = if s.byte() < 0x20 { place } else { s.word() };
    m.mem.w16(DS, 0x0FC8, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0A4C, v);
    let v = if s.byte() < 0x20 { 8 } else { s.byte() % 8 };
    m.mem.w8(DS, 0x0A46, v);
    let v = if s.byte() < 0x10 { s.byte() } else { s.byte() % 40 };
    m.mem.w8(DS, 0x01AA, v);
    let v = s.byte() % 4;
    m.mem.w8(DS, 0x0A47, v);
    let v = dword(s);
    m.mem.w32(DS, 0x0A52, v);
    let t = if s.byte() < 0x10 {
        let hi = s.word() as u32;
        hi << 16 | s.word() as u32
    } else {
        let hi = s.byte() as u32;
        hi << 16 | s.word() as u32
    };
    m.mem.w32(DS, 0x05CC, t);
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    let mut quests = Vec::new();
    for (base, size) in [(0x0DCAu16, 0x29u16), (0x0E6E, 0x34)] {
        for i in 0..4 {
            let mut rec = vec![0u8; size as usize];
            for b in rec.iter_mut() {
                *b = s.byte();
            }
            let kind = [0u16, 0, 1, 2, 3][s.byte() as usize % 5];
            let k = s.word() & !0xC0 | kind << 6;
            rec[4..6].copy_from_slice(&k.to_le_bytes());
            for (j, &b) in rec.iter().enumerate() {
                m.mem.w8(DS, base + i * size + j as u16, b);
            }
            if size == 0x34 {
                quests.push(((k >> 9 & 7) as u8, rec[0x25]));
            }
        }
    }
    for i in 0..8u16 {
        let (mut stage, mut place) = (s.byte() % 8, s.byte());
        if s.byte() < 0x80 {
            (stage, place) = quests[s.byte() as usize % 4];
        }
        let k = s.byte();
        let v = if k < 0x40 {
            t
        } else if k < 0xC0 {
            t.wrapping_add((s.word() as u32).wrapping_sub(0x8000))
        } else {
            dword(s)
        };
        let e = 0x0A7E + i * 6;
        m.mem.w8(DS, e, stage);
        m.mem.w8(DS, e + 1, place);
        m.mem.w32(DS, e + 2, v);
    }
    for at in [0x1387u16, 0x1389, 0x138B] {
        let v = if s.byte() < 0x10 { 0 } else { 1 + (s.byte() % 12) as u16 };
        m.mem.w16(DS, at, v);
    }
    for (at, off) in [(0x1AB5u16, 0u16), (0x1004, 0x100), (0x1008, 0x180)] {
        m.mem.w16(DS, at, off);
        m.mem.w16(DS, at + 2, TSEG);
    }
    let v = s.word();
    m.mem.w16(DS, 0x138D, v);
    let v = s.word();
    m.mem.w16(DS, 0x138F, v);
    for i in 0..16 * 0x19u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1445 + i, b);
    }
    let v = (s.byte() % 40) as u16;
    m.mem.w16(DS, 0x1927, v);
    let mut lists = [0u8; 0x200];
    for b in lists.iter_mut() {
        *b = s.byte();
    }
    for i in 0..0x80 {
        if s.byte() < 0x60 {
            lists[i * 4 + 2] = EXCLUDED[s.byte() as usize % 7];
        }
    }
    for (i, &b) in lists.iter().enumerate() {
        m.mem.w8(TSEG, i as u16, b);
    }
    let ticks = if s.byte() < 0xF0 { dword(s) } else { 0 };
    let mut block = [0u8; 0x300];
    for b in block.iter_mut() {
        *b = s.byte();
    }
    for i in 0..0xC0 {
        if s.byte() < 0x60 {
            block[i * 4 + 2] = EXCLUDED[s.byte() as usize % 7];
        }
    }
    for (i, &b) in block.iter().enumerate() {
        m.mem.w8(HEAP.0 + 1, i as u16, b);
    }
    ticks
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    // The stubs read no files: any directory will do.
    let files = Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    // The heap's bytes before its set-up, which the vectors' digests of
    // its changes start from.
    let zero = Machine::new(&img).mem;
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    let (mut n, mut stuck, mut made) = (0, 0, 0);
    for line in include_str!("vectors/sidequest.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "sidequest", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let ticks = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let Some(()) = m.quests_place(&mut fs, &mut Stubs { ticks }) else {
            assert_eq!(f[2], "stuck", "{line}");
            stuck += 1;
            continue;
        };
        m.store_rng();
        if std::env::var("SQ_DUMP").is_ok_and(|d| d == f[1]) {
            for o in (0..=0xFFFFu16).filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o)) {
                println!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
            }
        }
        let ds = DS as usize * 16;
        let changed: Vec<u8> = m.mem.diff(&before.mem, ds..ds + 0x10000).into_iter().flat_map(|(i, v)| {
            let o = (i - ds) as u16;
            [o as u8, (o >> 8) as u8, v]
        }).collect();
        made += !changed.is_empty() as u32;
        let heap = m.mem.diff(&zero, HEAP.0 as usize * 16..HEAP.1 as usize * 16).into_iter().flat_map(|(i, v)| {
            let mut b = (i as u32).to_le_bytes().to_vec();
            b.push(v);
            b
        });
        if std::env::var("SQ_DUMP").is_ok() {
            continue;
        }
        assert_eq!(f[2], format!("{:x} {:x}", fnv(changed), fnv(heap)), "{line}");
        n += 1;
    }
    eprintln!("sidequest: {n} compared, {made} changed something, {stuck} stuck");
    assert!(n > 3000, "{n}");
}
