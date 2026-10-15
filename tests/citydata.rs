//! Differential tests of CITYDATA's writes (src/city.rs
//! `city_save_province`, `city_save_entry`; src/quest.rs
//! `quest_dungeon_entry` with its default hooks, the whole of 26F7:0C0A)
//! against vectors from the original code (harness/gen_citydata.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const PROVINCE: usize = 0x4CC;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn name(slot: u8) -> Vec<u8> {
    let mut n = b"CITYDATA.".to_vec();
    n.extend([0x30 + slot / 10, 0x30 + slot % 10]);
    n
}

/// A case: the operation, AX, BX, CX and the slot.
struct Case {
    op: u8,
    ax: u16,
    bx: u16,
    cx: u16,
    slot: u8,
}

/// Mirrors harness/gen_citydata.py `setup` (a conditional's test byte
/// comes first).
fn setup(m: &mut Machine, files: &mut Files, seed: u32) -> Case {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() & 1;
    let slot = s.byte() % 10;
    let province = (s.byte() % 9) as u16;
    let here = s.byte() % 9;
    let mode = (s.byte() % 6) as u16;
    let index = (s.byte() % 0x10) as u16;
    let mut data = Vec::new();
    for _ in 0..9 {
        let mut block: Vec<u8> = (0..PROVINCE).map(|_| s.byte()).collect();
        for e in 0..0x10 {
            let at = 0x33C + e * 0x19;
            if at + 0x19 > PROVINCE {
                break;
            }
            let f = s.byte() & 0xFD;
            block[at + 0x18] = f | if s.byte() < 0x40 { 2 } else { 0 };
            let n = if s.byte() < 0xC0 { s.byte() % 12 } else { 0 };
            for i in 0..0x13 {
                block[at + i] = if i < n as usize { 0x41 + s.byte() % 26 } else { 0 };
            }
        }
        data.extend(block);
    }
    m.mem.w8(DS, 0x1A6C, slot);
    m.mem.w8(DS, 0x0A46, here);
    for i in 0..PROVINCE as u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1429 + i, b);
    }
    for i in 0..4 {
        let b = s.byte();
        m.mem.w8(DS, 0x91F6 + i, b);
    }
    m.mem.w16(DS, 0x91F0, 0);
    let n = 1 + s.byte() % 12;
    for i in 0..n as u16 {
        let b = 0x61 + s.byte() % 26;
        m.mem.w8(DS, 0x929E + i, b);
    }
    m.mem.w8(DS, 0x929E + n as u16, 0);
    files.put(&name(slot), &data);
    Case { op, ax: province, bx: mode, cx: index, slot }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/citydata.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "citydata", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let mut fs = Files::default();
        let c = setup(&mut m, &mut fs, seed);
        m.load_rng();
        let before = m.clone();
        let r = if c.op == 0 {
            m.quest_dungeon_entry(&mut fs, &mut arena::quest::NoQuestHooks, c.ax, c.bx, c.cx, 0x929E)
        } else {
            m.city_save_province(&mut fs, c.ax).map(|_| 0)
        };
        let Some(r) = r else {
            assert!(f[2] == "fault" || f[2] == "hang", "{line}: the port gave None");
            n += 1;
            continue;
        };
        m.store_rng();
        if std::env::var("CD_DUMP").ok().as_deref() == Some(f[1]) {
            let d: Vec<String> = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .map(|o| format!("{o:04x}:{:02x}>{:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o)))
                .collect();
            eprintln!("{}", d.join(" "));
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let file = fs.data(&name(c.slot)).map(|d| d.to_vec()).unwrap_or_default();
        let got = format!("{:x} {:x} {:x}", fnv(r.to_le_bytes()), fnv(changed), fnv(file));
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} cases");
}
