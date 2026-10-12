//! Differential test of levels and experience against vectors produced by
//! running the original code (harness/gen_progression.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;

const LO: usize = 0x3276 * 16;
const HI: usize = 0x3BA0 * 16 + 0x10000;
/// The harness loads the image at this segment; register values that hold
/// segments are relocated by it.
const LOAD_SEG: u16 = 0x1000;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

fn pairs(s: &str) -> Vec<Vec<u32>> {
    s.split_whitespace().map(|p| p.split(':').map(hex).collect()).collect()
}

/// Runs one routine; None means the port reports the original's fault.
fn run(m: &mut Machine, name: &str, regs: &HashMap<String, u32>) -> Option<HashMap<&'static str, u32>> {
    let reg = |r: &str| regs[r];
    let mut out = HashMap::new();
    let cf = |ok: bool| ok as u32;
    match name {
        "check_level_up" => match m.check_level_up() {
            Some(ax) => {
                out.insert("ax", ax as u32);
                out.insert("cf", 1);
            }
            None => drop(out.insert("cf", 0)),
        },
        "level_up" => drop(m.level_up()?),
        "level_tier_stats" => m.level_tier_stats(reg("ds") as u16 - LOAD_SEG, reg("si") as u16),
        "player_class_is" => drop(out.insert("cf", cf(m.player_class_is(reg("ax") as u8)))),
        "class_name" => {
            let (seg, off) = m.class_name(reg("ds") as u16 - LOAD_SEG, reg("si") as u16);
            out.insert("seg", seg as u32);
            out.insert("si", off as u32);
        }
        "award_xp" => m.award_xp(reg("eax")),
        "award_quest_xp" => m.award_quest_xp(reg("cx") as u16),
        "start_staff_piece" => drop(out.insert("cf", cf(!m.start_staff_piece()))),
        "complete_staff_piece" => drop(out.insert("cf", cf(!m.complete_staff_piece()))),
        "roll_level_hit_points" => m.roll_level_hit_points(reg("si") as u16)?,
        _ => panic!("unknown routine {name}"),
    }
    Some(out)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/progression.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, writes, regs, rets, diff) = (f[0], pairs(f[1]), named(f[2]), f[3], f.get(4).copied().unwrap_or(""));

        let mut m = base.clone();
        for w in &writes {
            for i in 0..w[1] {
                m.mem.w8(DS, (w[0] + i) as u16, (w[2] >> (8 * i)) as u8);
            }
        }
        m.load_rng();
        let before = m.mem.clone();
        let got = run(&mut m, name, &regs);

        if rets == "fault" {
            assert!(got.is_none(), "{name}: original faults, port returned {got:?}\n{line}");
        } else {
            let got = got.unwrap_or_else(|| panic!("{name}: port faulted, original did not\n{line}"));
            // Only registers the routine returns; AX is meaningless when
            // check_level_up reports no level (CF clear).
            for (key, want) in named(rets) {
                if let Some(&v) = got.get(key.as_str()) {
                    assert_eq!(v, want, "{name}: {key}\n{line}");
                }
            }
            m.store_rng();
            let want: Vec<(usize, u8)> = pairs(diff).iter().map(|p| (p[0] as usize, p[1] as u8)).collect();
            assert_eq!(m.mem.diff(&before, LO..HI), want, "{name}: memory differs\n{line}");
        }
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 10);
}
