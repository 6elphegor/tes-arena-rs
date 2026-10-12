//! Differential test of loot and item generation against vectors produced
//! by running the original code (harness/gen_loot.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;

const LO: usize = 0x3276 * 16;
const HI: usize = 0x3BA0 * 16 + 0x10000;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

/// Runs one routine; returns the registers it defines as results.
fn run(m: &mut Machine, name: &str, regs: &HashMap<String, u32>) -> HashMap<&'static str, u16> {
    let reg = |r: &str| regs.get(r).copied().unwrap_or(0) as u16;
    let mut out = HashMap::new();
    match name {
        "loot_weapon" => m.loot_weapon(reg("ax") as u8, reg("bx") as u8).expect("original returned"),
        "loot_armor" => m.loot_armor(reg("ax") as u8, reg("bx") as u8),
        "loot_magic_weapon" => m.loot_magic_weapon(reg("ax") as u8, reg("bx")).expect("original returned"),
        "loot_magic_armor" => m.loot_magic_armor(reg("ax") as u8, reg("bx")).expect("original returned"),
        "roll_material" => m.roll_material(),
        "roll_enchantment" => m.roll_enchantment(reg("cf") & 1 != 0).expect("original returned"),
        "set_condition" => m.set_condition(reg("ax")),
        "finish_weapon" => m.finish_weapon(),
        "finish_armor" => m.finish_armor(),
        "pick_armor" => m.pick_armor(reg("di"), reg("bx")),
        "d100" => drop(out.insert("ax", m.d100())),
        "roll_loot_flags" => m.roll_loot_flags(reg("si")),
        "roll_corpse_gold" => drop(m.roll_corpse_gold()),
        "roll_creature_gold" => m.roll_creature_gold(reg("si")),
        _ => panic!("unknown routine {name}"),
    }
    out
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/loot.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, rets, diff) = (f[0], named(f[2]), f[3], f.get(4).copied().unwrap_or(""));

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
        assert_ne!(rets, "fault", "no loot routine should fault\n{line}");
        let got = run(&mut m, name, &regs);
        for (key, want) in named(rets) {
            if let Some(&v) = got.get(key.as_str()) {
                assert_eq!(v as u32, want, "{name}: {key}\n{line}");
            }
        }
        m.store_rng();
        let want: Vec<(usize, u8)> = diff
            .split_whitespace()
            .map(|p| p.split_once(':').unwrap())
            .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
            .collect();
        let have = m.mem.diff(&before, LO..HI);
        assert_eq!(have, want, "{name}: memory differs\n{line}");
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 14);
}

/// Where the original loops forever the port returns None.
#[test]
fn reports_hangs() {
    use arena::loot::{LEVEL, TEMPLATE, template};
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    // The image's seed is 0, a fixed point of the LCG (the game seeds it
    // at startup); with it every reroll would repeat forever.
    m.rng = arena::random::ArenaRandom::new(0x1994_0325);
    m.mem.w8(DS, TEMPLATE + template::WEAPON, 1);
    // Weapon enchantments need item level 7 or more (min levels at 23E9).
    let lowest = (0..14).map(|e| m.mem.r8(DS, 0x23E9 + e)).min().unwrap();
    assert!(lowest > 0);
    m.mem.w8(DS, LEVEL, lowest - 1);
    assert_eq!(m.clone().roll_enchantment(false), None);
    m.mem.w8(DS, LEVEL, lowest);
    assert_eq!(m.clone().roll_enchantment(false), Some(()));
    // The forced type is retried unchanged; one above level 20 never fits.
    let w_min = |m: &Machine, ty: u16| m.mem.r8(DS, 0x22E6 + ty);
    if let Some(ty) = (0..18).find(|&ty| w_min(&m, ty) > 20) {
        assert_eq!(m.clone().loot_weapon(0, ty as u8), None);
    }
}
