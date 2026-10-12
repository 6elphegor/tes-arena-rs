//! Differential test of creature and NPC generation against vectors from
//! the original code (harness/gen_creature.py).
mod common;

use arena::machine::Machine;
use std::collections::HashMap;

const CSEG: usize = 0x4C00;
const TSEG: usize = 0x5000;
const SSEG: usize = 0x5100;
/// As in the generator; the effect message text (DGROUP 1405..1427) is
/// not ported, so it is not compared.
const REGIONS: [(usize, usize); 6] = [
    (0x3276 * 16, 0x3BA0 * 16 + 0x1405),
    (0x3BA0 * 16 + 0x1428, 0x3BA0 * 16 + 0x10000),
    (CSEG * 16, CSEG * 16 + 0x3100),
    (CSEG * 16 + 0x91EE, CSEG * 16 + 0x91FC),
    (TSEG * 16, TSEG * 16 + 0x200),
    (SSEG * 16, SSEG * 16 + 0x10),
];

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

/// Runs one routine: None for the original's fault or hang, else the
/// registers it defines as results.
fn run(m: &mut Machine, name: &str, regs: &HashMap<String, u32>) -> Option<HashMap<&'static str, u16>> {
    let reg = |r: &str| regs.get(r).copied().unwrap_or(0) as u16;
    let mut out = HashMap::new();
    match name {
        "npc_roll_level" => drop(out.insert("ax", m.npc_roll_level())),
        "npc_roll_class" => drop(out.insert("al", m.npc_roll_class() as u16)),
        "npc_roll_sex" => drop(out.insert("ax", m.npc_roll_sex() as u16)),
        "npc_face" => {
            let r = m.npc_face(reg("ax"))?;
            // AX is left holding race × 10 + roll.
            out.insert("ax", reg("ax").wrapping_mul(10).wrapping_add(r));
        }
        "npc_seed" => m.npc_seed(reg("es"), reg("di"), reg("ax"), reg("bx"))?,
        "expand_npc" => m.expand_npc(reg("ds"), reg("si"), reg("es"), reg("di"))?,
        "npc_xp_value" => m.npc_xp_value(reg("es"), reg("di")),
        "derive_record_stats" => m.derive_record_stats(reg("ds"), reg("si"))?,
        "create_monster" => m.create_monster(reg("ax"), reg("di"))?,
        "alloc_creature_slot" => {
            let (si, di, cf) = match m.alloc_creature_slot() {
                Ok((rec, entry)) => (rec, entry, 0),
                Err(si) => (si, reg("di"), 1),
            };
            out.insert("si", si);
            out.insert("di", di);
            out.insert("cf", cf);
        }
        "usable_creature_slots" => drop(out.insert("ax", m.usable_creature_slots())),
        "spawn_monsters" => m.spawn_monsters(reg("ax"), reg("cx"))?,
        "npc_level_boost" => m.npc_level_boost(reg("es"), reg("si")),
        "npc_sorcerer_spell_points" => m.npc_sorcerer_spell_points(reg("ds"), reg("si")),
        "creature_gold_at" => m.creature_gold_at(reg("ds"), reg("si"))?,
        // CF set means no loot flag was set.
        "loot_corpse" => drop(out.insert("cf", !m.loot_corpse(reg("ds"), reg("si"))? as u16)),
        _ => panic!("unknown routine {name}"),
    }
    Some(out)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/creature.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, rets, diff) = (f[0], named(f[2]), f[3], f.get(4).copied().unwrap_or(""));
        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<&str> = w.split(':').collect();
            // "seg:off:2:s:segval" is a segment value; the emulator
            // relocated it, Rust uses it as is.
            let (seg, off, size, v) = (hex(p[0]) as u16, hex(p[1]) as u16, hex(p[2]), hex(p[p.len() - 1]));
            for i in 0..size {
                m.mem.w8(seg, off.wrapping_add(i as u16), (v >> (8 * i)) as u8);
            }
        }
        m.load_rng();
        let before = m.mem.clone();
        let got = run(&mut m, name, &regs);
        if rets == "fault" || rets == "hang" {
            assert!(got.is_none(), "{name}: original {rets}s, port returned {got:?}\n{line}");
        } else {
            let got = got.unwrap_or_else(|| panic!("{name}: port gave None, original returned\n{line}"));
            for (key, want) in named(rets) {
                match key.as_str() {
                    "ax" if got.contains_key("al") => assert_eq!(got["al"] as u32, want & 0xFF, "{name}: al\n{line}"),
                    k => {
                        if let Some(&v) = got.get(k) {
                            assert_eq!(v as u32, want, "{name}: {k}\n{line}");
                        }
                    }
                }
            }
            m.store_rng();
            let want: Vec<(usize, u8)> = diff
                .split_whitespace()
                .map(|p| p.split_once(':').unwrap())
                .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
                .collect();
            let have: Vec<(usize, u8)> = REGIONS.iter().flat_map(|&(lo, hi)| m.mem.diff(&before, lo..hi)).collect();
            if have != want {
                let hs: std::collections::BTreeMap<_, _> = have.iter().copied().collect();
                let ws: std::collections::BTreeMap<_, _> = want.iter().copied().collect();
                let mut d: Vec<String> = Vec::new();
                for k in hs.keys().chain(ws.keys()).collect::<std::collections::BTreeSet<_>>() {
                    if hs.get(k) != ws.get(k) {
                        d.push(format!("{k:x}: port {:?} orig {:?}", hs.get(k), ws.get(k)));
                    }
                }
                panic!("{name}: memory differs ({} bytes)\n{}\n{line}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
            }
        }
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 16);
}
