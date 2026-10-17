//! Differential tests of the sound loader (23BA:0217, `sound_load`)
//! against vectors from the original code (harness/gen_soundload.py):
//! the loader called on its own with random sound files, bank settings,
//! EMS room and driver buffers, and the INF loader with sound on, its
//! hook loading the level's sounds, and a fight's sound (23BA:01C7,
//! `fight_sound`). Needs the game directory (ESarena/,
//! next to ACD.EXE); the vectors hold only names, settings and digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;

const FRAME: u16 = 0x8800;
const BUF_SEG: u16 = 0x7000;
const NAME: u16 = 0x0A36;
const INFO: u16 = 0xE2BE;
const LIST: u16 = 0x929E;
const BLOCK: u16 = 0x3E80;
/// The harness's heap (harness/dos.py).
const HEAP: (u16, u16) = (0xA000, 0xF000);

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

fn bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| hex(&s[2 * i..2 * i + 2]) as u8).collect()
}

fn put(m: &mut Machine, off: u16, b: &[u8]) {
    for (i, &c) in b.iter().enumerate() {
        m.mem.w8(DS, off.wrapping_add(i as u16), c);
    }
}

fn digests(m: &Machine, before: &Machine) -> String {
    let changed = (0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
    // The harness's banks (harness/dos.py `ems_bank`): the frame for the
    // bank mapped, EMS on (8FA6) or not.
    let mapped = m.mem.r16(DS, 0x8FA8) >> 2;
    let frame = m.mem.r16(DS, 0x9030);
    let banks = (0..0x20).flat_map(|b| if b == mapped { m.mem.segment(frame).to_vec() } else { m.ems_bank(b) });
    let (seg, off) = (m.mem.r16(DS, 0xBA56), m.mem.r16(DS, 0xBA54));
    let block = (0..BLOCK).map(|i| m.mem.r8(seg, off.wrapping_add(i)));
    format!("{:x} {:x} {:x}", fnv(changed), fnv(banks), fnv(block))
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
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (seg, off) = base.heap_alloc_bytes(BLOCK).unwrap();
    base.mem.w16(DS, 0xBA54, off);
    base.mem.w16(DS, 0xBA56, seg);
    base.mem.w16(DS, 0xCB8A, FRAME);
    base.mem.w16(DS, 0xEB8B, BUF_SEG);
    base.mem.w16(DS, 0xEB91, 1);
    let vectors = include_str!("vectors/soundload.txt");
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let mut loaded = 0;
    for line in vectors.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<&str> = f[1].split(' ').collect();
        let mut m = base.clone();
        let mut fs = files.clone();
        let got = match f[0] {
            "load" => {
                m.mem.w16(DS, 0x8F64, hex(p[0]) as u16);
                put(&mut m, 0xC8FC, &bytes(p[1]));
                m.mem.w8(DS, 0x8FA6, hex(p[2]) as u8);
                m.mem.w16(DS, 0xEB89, hex(p[3]) as u16);
                let kept = bytes(p[4]);
                let fill = hex(p[5]) as u16;
                for i in 0..0x200u16 {
                    m.mem.w8(DS, 0x44F0 + i, i.wrapping_mul(fill).wrapping_add(fill >> 3) as u8);
                }
                put(&mut m, 0x45F0, &kept[..8]);
                put(&mut m, 0x1044, &kept[8..]);
                put(&mut m, 0x8FAA, &bytes(p[6]).chunks(2).flat_map(|w| [w[1], w[0]]).collect::<Vec<_>>());
                let mut list = Vec::new();
                if p[7] != "-" {
                    for n in p[7].split(',') {
                        list.extend_from_slice(n.as_bytes());
                        list.push(0);
                    }
                }
                list.push(0);
                put(&mut m, LIST, &list);
                let before = m.clone();
                m.sound_load(&mut fs, LIST).map(|()| {
                    loaded += (0..64u16).filter(|i| m.mem.r16(DS, 0x44F0 + 8 * i + 6) != 0 && *i != 32).count();
                    digests(&m, &before)
                })
            }
            "fight" => {
                m.mem.w8(DS, 0x8FA6, hex(p[1]) as u8);
                let before = m.clone();
                m.fight_sound(&mut fs, hex(p[0]) as u16).map(|()| digests(&m, &before))
            }
            "inf" => {
                put(&mut m, NAME, format!("{}\0", p[0]).as_bytes());
                m.mem.w8(DS, 0xBB0B, 0xFF);
                m.mem.w16(DS, 0xC39A, 0x400);
                m.mem.w16(DS, 0xCB84, 0);
                m.mem.w16(DS, 0xCB88, 0x8000);
                m.mem.w8(DS, 0xC8EF, 0x80);
                m.level_load(&mut fs, NAME).expect(line);
                assert_eq!(m.mem.r16(DS, 0xCB84), INFO, "{line}");
                m.mem.w16(DS, 0x8F64, 1);
                put(&mut m, 0xC8FC, &bytes(p[1]));
                m.mem.w32(DS, 0xCD4A, 0x23BA_0217);
                let before = m.clone();
                m.info_load(&mut fs, INFO).map(|()| digests(&m, &before))
            }
            _ => panic!("unknown vector {line}"),
        };
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(f[0]).or_default() += 1;
    }
    eprintln!("{counts:?}, {loaded} sounds loaded");
    assert_eq!(counts.len(), 3);
    assert!(loaded > 1000);
}

/// With no sound banks (C8FD 0) the original's LOOP fills DGROUP with
/// FFFFh: the port gives up.
#[test]
fn no_banks_gives_up() {
    let Some(img) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else { return };
    let mut m = Machine::new(&img);
    m.mem.w16(DS, 0x8F64, 1);
    m.mem.w8(DS, 0xC8FD, 0);
    assert!(m.sound_load(&mut files, LIST).is_none());
}
