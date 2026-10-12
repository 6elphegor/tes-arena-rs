//! Differential tests of item names (inventory::item_name) against
//! vectors from the original code (harness/gen_items.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/items.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seg, off) = (hex(p[0]) as u16, hex(p[1]) as u16);
        let mut m = base.clone();
        for i in 0..19 {
            m.mem.w8(seg, off.wrapping_add(i as u16), hex(&p[2][2 * i..2 * i + 2]) as u8);
        }
        let before = m.clone();
        let got = m.item_name(seg, off).map(|si| {
            let name = (0..64u16).map(|i| m.mem.r8(DS, si.wrapping_add(i))).take_while(|&c| c != 0);
            let buf = (0..96u16).map(|i| m.mem.r8(seg, 0x1395 + i));
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{si:x} {:x} {:x} {:x}", fnv(name), fnv(buf), fnv(changed))
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("{n} items");
}
