//! Differential tests of a place's buildings listed (src/cityinfo.rs
//! `city_info`, 1194:3EBF) against vectors from the original code
//! (harness/gen_cityinfo.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x7C00;
const FSEG: u16 = 0x5C00;
const LSEG: u16 = 0x6C00;
const HEAP: u16 = 0xA000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn changes(before: &Machine, after: &Machine, seg: u16) -> u64 {
    fnv((0..=0xFFFFu16)
        .filter(|&o| after.mem.r8(seg, o) != before.mem.r8(seg, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, after.mem.r8(seg, o)]))
}

/// Mirrors harness/gen_cityinfo.py `setup`; returns AX.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let k = s.byte() % 8;
    let p0 = s.byte() % 8;
    let p = if s.byte() < 0x10 { 8 } else { p0 };
    let cur = ((p as u16) << 5 | (s.byte() % 32) as u16) as u8;
    let d = s.byte();
    let dim = if d < 0xF0 { d % 6 + 1 } else { d % 10 };
    let (x, y) = (s.word(), s.word());
    let (lo, hi) = (s.word(), s.word());
    let toggle = s.byte() & 1;
    m.mem.w8(DS, 0x0A46, p);
    m.mem.w8(DS, 0x0A4C, cur);
    m.mem.w8(DS, 0x0A43, dim);
    m.mem.w16(DS, 0x0A48, x);
    m.mem.w16(DS, 0x0A4A, y);
    m.mem.w16(DS, 0x91F6, lo);
    m.mem.w16(DS, 0x91F8, hi);
    m.mem.w16(DS, 0x91F0, 0);
    m.mem.w8(DS, 0x1AEA, toggle);
    m.mem.w8(DS, 0x1A6C, 65);
    for i in 0..81u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x486B + i, if b < 0x20 { 1 } else { b });
    }
    let ax = if k < 4 {
        (s.byte() as u16) << 8 | cur as u16
    } else if k < 7 {
        0x100 | ((p as u16) << 5 | (s.byte() % 32) as u16) & 0xFF
    } else {
        let b = s.byte();
        if b < 0x40 { 0x100 } else { 0x100 | b as u16 }
    };
    let kinds: Vec<u8> = (0..5).map(|_| 0x40 + s.byte() % 12).collect();
    for (at, v) in [0xC918u16, 0xC917, 0xC91A, 0xC919, 0xC922].into_iter().zip(&kinds) {
        m.mem.w8(DS, at, *v);
    }
    let fw = s.word();
    for i in 0..0x4000u16 {
        m.mem.w16(WSEG, 2 * i, fw.wrapping_mul(2 * i + 1));
    }
    for (j, &kind) in kinds.iter().enumerate() {
        let mut n = s.byte() % 6;
        let extra = s.byte();
        if j == 2 && extra < 0x40 {
            n = 24 + extra % 6;
        }
        for _ in 0..n {
            let sq = s.word() & 0x3FFF;
            m.mem.w16(WSEG, 2 * sq, kind as u16 * 0x101);
        }
    }
    let ff = s.word();
    for i in 0..=0xFFFFu16 {
        m.mem.w8(FSEG, i, (ff as u32 + 7 * i as u32) as u8);
    }
    for i in 0..=0xFFFFu16 {
        m.mem.w8(LSEG, i, 0);
    }
    for (at, off) in [(0x1AB5u16, 0u16), (0x1008, 0x100), (0x1004, 0x180)] {
        m.mem.w16(DS, at, off);
        m.mem.w16(DS, at + 2, LSEG);
    }
    m.mem.w16(DS, 0xD806, WSEG);
    m.mem.w16(DS, 0x9030, FSEG);
    m.load_rng();
    ax
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = common::game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut base = Machine::new(&img);
    base.heap_init(0xA000, 0xF000).unwrap();
    assert!(base.bsa_open(&mut files));
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/cityinfo.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "cityinfo", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let ax = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        if std::env::var("CI_TRACE").is_ok() {
            eprintln!("case {} ax {ax:04x}", f[1]);
        }
        let before = m.clone();
        let got = m.city_info(&mut fs, ax).map(|_| {
            m.store_rng();
            if std::env::var("CI_DUMP").ok().as_deref() == Some(f[1]) {
                for o in 0..=0xFFFFu16 {
                    if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                        eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                    }
                }
            }
            let fl: Vec<u8> = fs
                .written_names()
                .into_iter()
                .flat_map(|name| {
                    let d = fs.data(&name).unwrap();
                    let mut v = name.clone();
                    v.push(0);
                    v.extend((d.len() as u32).to_le_bytes());
                    v.extend(d.iter());
                    v
                })
                .collect();
            [DS, LSEG, FSEG, HEAP].map(|s| format!("{:x}", changes(&before, &m, s))).join(" ") + &format!(" {:x}", fnv(fl))
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => {
                assert_eq!(f[2], "stuck", "{line}");
                stuck += 1;
            }
        }
    }
    eprintln!("cityinfo: {n} compared, {stuck} stuck in both");
    assert!(n > 1000, "{n} compared");
}
