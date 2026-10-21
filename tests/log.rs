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

/// Fixed (kept bug #33): a logbook of 2000 bytes or more has its oldest
/// entry dropped before the new one is added; the original writes the
/// trimmed log to "C:\ARENA_CD\C:\ARENA_CD\log.nn", which DOS refuses,
/// so the log only grows.
#[test]
fn fixed_a_full_logbook_drops_its_oldest_entry() {
    use arena::fixes::{Fixes, LOG_TRIM};
    let Some(img) = common::image() else { return };
    let mut old = Vec::new();
    for i in 0..60 {
        old.extend(format!("&Day {i}\r\nSomething happened today, number {i}. *").bytes());
    }
    assert!(old.len() >= 2000);
    let after = |fixes: Fixes| {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.mem.w16(DS, 0x9030, 0x7000);
        m.mem.w8(DS, 0x1A6C, 1);
        for (i, &b) in b"A new entry\0".iter().enumerate() {
            m.mem.w8(DS, 0xA800 + i as u16, b);
        }
        let mut files = arena::bsa::Files::default();
        files.put(b"LOG.01", &old);
        m.log_add(&mut files, 0xA800).expect("added");
        files.written(b"LOG.01").expect("the log").to_vec()
    };
    let grown = after(Fixes::NONE);
    assert_eq!(grown[..old.len()], old[..], "the original's log kept whole");
    let trimmed = after(Fixes::only(LOG_TRIM));
    let first = old.iter().position(|&c| c == b'*').unwrap() + 1;
    assert_eq!(trimmed[..old.len() - first], old[first..], "the oldest entry dropped");
    assert_eq!(trimmed[old.len() - first..], grown[old.len()..], "the new entry after");
}
