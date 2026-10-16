//! Differential tests of an item's description (src/use_item.rs
//! `item_info`, 1194:7CBA) against vectors from the original code
//! (harness/gen_iteminfo.py): each item put in the player's first slot,
//! 12B0 3 and 12B2 0, the changed DGROUP and DATA bytes compared by
//! digest; where the original reaches what isn't ported (a special item's
//! box, an identified stack's text) or faults, the port gives up.
mod common;

use arena::machine::Machine;
use arena::mem::{DATA, DS};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for c in b {
        h = (h ^ c as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn seg_bytes(m: &Machine, seg: u16) -> Vec<u8> {
    (0..=0xFFFFu16).map(|o| m.mem.r8(seg, o)).collect()
}

fn changes(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        if a != b {
            out.extend([i as u8, (i >> 8) as u8, *b]);
        }
    }
    out
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/iteminfo.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let item: Vec<u8> = (0..f[1].len()).step_by(2).map(|i| u8::from_str_radix(&f[1][i..i + 2], 16).unwrap()).collect();
        let mut m = base.clone();
        for (i, &b) in item.iter().enumerate() {
            m.mem.w8(DS, 0x0212 + i as u16, b);
        }
        m.mem.w16(DS, 0x12B0, 3);
        m.mem.w16(DS, 0x12B2, 0);
        let (ds, data) = (seg_bytes(&m, DS), seg_bytes(&m, DATA));
        match m.item_info(0x0212) {
            Some(()) => {
                let got = format!("{:x} {:x}", fnv(changes(&ds, &seg_bytes(&m, DS))), fnv(changes(&data, &seg_bytes(&m, DATA))));
                assert_eq!(f[2], got, "{line}");
                n += 1;
            }
            None => {
                assert!(matches!(f[2], "special" | "stack" | "fault" | "hang"), "{line}: the port gave up");
                stuck += 1;
            }
        }
    }
    assert!(n > 0);
    eprintln!("{n} matched, {stuck} given up as the original");
}
