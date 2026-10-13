//! Differential tests of the player's controls (src/controls.rs
//! `player_input`, 2826:0002) against vectors from the original code
//! (harness/gen_input.py).
mod common;

use arena::bsa::Files;
use arena::mem::DS;
use common::blow::Stream;
use common::controls::*;

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = base(&img);
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/input.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "input", "{line}");
        let mut m = base.clone();
        let frames = setup(&mut m, &mut Stream(hex(f[1]) as u32));
        m.load_rng();
        let before = m.clone();
        let mut got = Some(());
        for _ in 0..frames {
            got = got.and_then(|()| m.player_input(&mut files, &mut Pick));
        }
        let got = got.map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x} {:x}", fnv(changed), fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o))))
        });
        if std::env::var("INPUT_DUMP").is_ok_and(|v| v == f[1]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(if f[2] == "fault" { "fault" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 2500);
}
