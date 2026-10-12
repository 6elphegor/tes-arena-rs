//! Differential test of the character rules against vectors produced by
//! running the original code (harness/gen_rules.py).
mod common;

use arena::dice::dice;
use arena::machine::Machine;
use arena::mem::DS;
use arena::scale::{attrs_to100, attrs_to256, to100, to100_u, to256};
use std::collections::HashMap;

const LO: usize = 0x3276 * 16;
const HI: usize = 0x3BA0 * 16 + 0x10000;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

/// "name:hex" pairs, e.g. registers.
fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

fn pairs(s: &str) -> Vec<Vec<u32>> {
    s.split_whitespace().map(|p| p.split(':').map(hex).collect()).collect()
}

/// Runs one routine; None means the port reports the original's fault.
fn run(m: &mut Machine, name: &str, regs: &HashMap<String, u32>) -> Option<HashMap<&'static str, u16>> {
    let reg = |r: &str| regs[r] as u16;
    let mut out = HashMap::new();
    match name {
        "to256" => drop(out.insert("ax", to256(reg("ax") as i16) as u16)),
        "to100" => drop(out.insert("ax", to100(reg("ax") as i16) as u16)),
        "to100_u" => drop(out.insert("ax", to100_u(reg("ax")))),
        "dice" => drop(out.insert("ax", dice(&mut m.rng, reg("ax"))?)),
        "roll_attributes" => m.roll_attributes(reg("di"))?,
        "attrs_to100" => attrs_to100(&mut m.mem, reg("si"), reg("di"), reg("cx")),
        "attrs_to256" => attrs_to256(&mut m.mem, reg("si"), reg("di"), reg("cx")),
        "spell_points" => {
            let (max, cur) = m.spell_points(reg("si"), reg("ax"));
            out.insert("bx", max);
            out.insert("ax", cur);
        }
        "attribute_modifiers" => m.attribute_modifiers(reg("si")),
        "player_derived_stats" => m.player_derived_stats(),
        "roll_max_hp" => m.roll_max_hp(reg("si"))?,
        "roll_hit_die" => drop(out.insert("ax", m.roll_hit_die(reg("si"))?)),
        "roll_starting_gold" => m.roll_starting_gold()?,
        "create_player_stats" => m.create_player_stats()?,
        _ => panic!("unknown routine {name}"),
    }
    Some(out)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/rules.txt").lines() {
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
            // Only registers the routine returns; the rest are scratch.
            for (key, want) in named(rets) {
                if let Some(&v) = got.get(key.as_str()) {
                    assert_eq!(v as u32, want, "{name}: {key}\n{line}");
                }
            }
            m.store_rng();
            let want: Vec<(usize, u8)> = pairs(diff).iter().map(|p| (p[0] as usize, p[1] as u8)).collect();
            let have = m.mem.diff(&before, LO..HI);
            assert_eq!(have, want, "{name}: memory differs\n{line}");
        }
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 14);
}
