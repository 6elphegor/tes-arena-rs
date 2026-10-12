//! Differential tests of the calendar and log entries (src/log.rs)
//! against vectors from the original code (harness/gen_log.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn changed(m: &Machine, before: &Machine) -> u64 {
    fnv((0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]))
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/log.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<&str> = f[1].split(' ').collect();
        let mut m = base.clone();
        let got = match f[0] {
            "date" => {
                let dest = hex(p[1]) as u16;
                let before = m.clone();
                m.log_date(hex(p[0]), dest);
                format!("{:x} {:x}", fnv(m.mem.cstr(DS, dest).to_vec()), changed(&m, &before))
            }
            "entry" => {
                m.mem.w32(DS, 0x5CC, hex(p[0]));
                let text = p.get(1).copied().unwrap_or("");
                for i in 0..text.len() / 2 {
                    m.mem.w8(DS, 0xA800 + i as u16, hex(&text[2 * i..2 * i + 2]) as u8);
                }
                m.mem.w8(DS, 0xA800 + (text.len() / 2) as u16, 0);
                let before = m.clone();
                let n = m.log_entry(0xA800);
                format!("{n:x} {:x}", changed(&m, &before))
            }
            _ => panic!("unknown vector {line}"),
        };
        assert_eq!(f[2], got, "{line}");
        *counts.entry(f[0]).or_default() += 1;
    }
    eprintln!("{counts:?}");
}
