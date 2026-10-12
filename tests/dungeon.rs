//! Differential test of dungeon generation against vectors produced by
//! running the original code (harness/gen_dungeon.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;

const LO: usize = 0x3276 * 16;
const HI: usize = 0x3BA0 * 16 + 0x10000;
const MAPSEG: u16 = 0x5000;
const HANDLE: u16 = 7;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

/// Bytes of the map segment that differ from a cleared level (0x01 below
/// 8000, 0x00 above), as the generator reports them.
fn map_marks(m: &Machine) -> String {
    (0..=0xFFFFu16)
        .filter_map(|i| {
            let b = m.mem.r8(MAPSEG, i);
            (b != if i < 0x8000 { 1 } else { 0 }).then(|| format!("{i:x}:{b:x}"))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/dungeon.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, result) = (f[0], named(f[2]), f[3]);
        let (diff, marks) = (f.get(4).copied().unwrap_or(""), f.get(5).copied().unwrap_or(""));

        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<&str> = w.split(':').collect();
            // "off:2:s:seg" is a segment value; the emulator relocated it.
            let (off, size, v) = (hex(p[0]), hex(p[1]), hex(p[p.len() - 1]));
            for i in 0..size {
                m.mem.w8(DS, (off + i) as u16, (v >> (8 * i)) as u8);
            }
        }
        m.load_rng();
        let before = m.mem.clone();
        let reg = |r: &str| regs[r] as u16;

        let mut chunks = None;
        let got: Option<String> = match name {
            "build_location" => m.dungeon_build_location(reg("ax") as u8, reg("climate") as u8).map(|_| "ok".into()),
            "level_count" => Some(format!("cx:{:x}", m.dungeon_level_count())),
            "setup" => m.dungeon_setup(reg("ax"), reg("bx"), reg("cx")).map(|_| "ok".into()),
            "enter" => m.dungeon_enter().map(|_| "ok".into()),
            "level" => m.dungeon_level(reg("cx") as u8, HANDLE).map(|c| {
                chunks = Some(c);
                "ok".into()
            }),
            "start_position" => {
                m.dungeon_start_position();
                Some("ok".into())
            }
            "quest_provinces" => m.dungeon_pick_quest_provinces().map(|_| "ok".into()),
            "quest_target" => {
                m.dungeon_set_quest_target();
                Some("ok".into())
            }
            "pick_entry" => m
                .dungeon_pick_entry(reg("ax"), reg("bx"), reg("cx"), reg("si"))
                .map(|p| format!("ax:{:x} bx:{:x} cx:{:x} si:{:x}", p.index, p.x, p.y, reg("si"))),
            _ => panic!("unknown routine {name}"),
        };

        if result == "fault" || result == "hang" {
            assert!(got.is_none(), "{name}: original {result}s, port returned {got:?}\n{line}");
        } else {
            let got = got.unwrap_or_else(|| panic!("{name}: port failed, original did not\n{line}"));
            assert_eq!(got, result, "{name}: result\n{line}");
            m.store_rng();
            let want: Vec<(usize, u8)> = diff
                .split_whitespace()
                .map(|p| p.split_once(':').unwrap())
                .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
                .collect();
            assert_eq!(m.mem.diff(&before, LO..HI), want, "{name}: memory differs\n{line}");
            if name == "level" {
                assert_eq!(map_marks(&m), marks, "{name}: map differs\n{line}");
            } else {
                assert_eq!(m.mem.diff(&before, MAPSEG as usize * 16..MAPSEG as usize * 16 + 0x10000), vec![]);
            }
            if let Some(c) = chunks {
                let have: Vec<String> = c.iter().map(|c| format!("{:x}={:x}", c.offset, c.block)).collect();
                let want: Vec<&str> = f.get(6).map(|s| s.split_whitespace().collect()).unwrap_or_default();
                assert_eq!(have, want, "{name}: chunks\n{line}");
            }
        }
        *count.entry(format!("{name}:{result}")).or_default() += 1;
    }
    eprintln!("{count:?}");
    for n in [
        "build_location",
        "level_count",
        "setup",
        "enter",
        "level",
        "start_position",
        "quest_provinces",
        "quest_target",
        "pick_entry",
    ] {
        assert!(count.keys().any(|k| k.starts_with(n)), "no cases for {n}");
    }
}

/// Seeds that the LCG never leaves (it only multiplies) make the reroll
/// loops spin forever in the original; the port reports None.
#[test]
fn degenerate_seeds_hang() {
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    m.rng.seed = 0;
    assert_eq!(m.clone().dungeon_setup(1, 1, 2), None);
    m.mem.w8(DS, 0x0FDD, 6); // quest step 6: province 0, which seed 0 always rolls
    assert_eq!(m.clone().dungeon_pick_quest_provinces(), None);
    m.rng.seed = 0x1994_0325;
    assert!(m.clone().dungeon_setup(1, 1, 2).is_some());
}
