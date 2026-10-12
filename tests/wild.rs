//! Differential tests of the wilderness grid (src/wild.rs) against
//! vectors from the original code (harness/gen_wild.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;

const BUF: u16 = 0x7000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x9030, BUF);
    let mut n = 0;
    for line in include_str!("vectors/wild.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let (seed, template) = f[1].split_once(' ').unwrap();
        let mut m = base.clone();
        m.mem.w32(DS, 0xA22, u32::from_str_radix(seed, 16).unwrap());
        for (i, c) in format!("{template}\0").bytes().enumerate() {
            m.mem.w8(DS, 0xA36 + i as u16, c);
        }
        m.load_rng();
        let before = m.clone();
        let got = m.wild_generate().map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv((0..=0xFFFFu16).map(|o| m.mem.r8(BUF, o))), fnv(changed))
        });
        assert_eq!(f[2], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("{n} grids");
}
