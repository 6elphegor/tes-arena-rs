//! Differential test of combat against vectors produced by running the
//! original code with inventory routines stubbed (harness/gen_combat.py).
mod common;

use arena::combat::{Gear, Ptr};
use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;

const MSEG: u16 = 0x5000;
const REGIONS: [(usize, usize); 2] =
    [(0x3276 * 16, 0x3BA0 * 16 + 0x10000), (MSEG as usize * 16, MSEG as usize * 16 + 0x1000)];

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

fn gear(s: &str) -> Gear {
    let v: Vec<u16> = s.split(':').map(|x| hex(x) as u16).collect();
    let opt = |x: u16| (x != 0xFFFF).then_some(x);
    Gear { armor: v[0] as i16, weapon: opt(v[1]), gauntlets: opt(v[2]), weapon_bonus: v[3], missile: v[4] != 0, missile_cx: 0 }
}

/// Runs one routine; returns (ax, cf) where the routine defines them, or
/// None where the port reports the original's fault.
fn run(m: &mut Machine, name: &str, r: &HashMap<String, u32>, g: &Gear) -> Option<(Option<u16>, Option<bool>)> {
    let reg = |k: &str| r[k] as u16;
    let p = |s: &str, o: &str| Ptr::new(reg(s), reg(o));
    Some(match name {
        "percent" => (Some(m.percent()), None),
        "hit_location" => (Some(m.hit_location()?), None),
        "hit_base" => (Some(m.hit_base(p("fs", "si"), p("es", "di"))), None),
        "hit_chance" => (Some(m.hit_chance(p("fs", "si"), p("es", "di"), g)), None),
        "roll_to_hit" => {
            let (hit, roll) = m.roll_to_hit(p("fs", "si"), p("es", "di"), reg("ax"), g)?;
            (Some(roll), Some(hit))
        }
        "critical_multiplier" => {
            // The harness's 78DE stub leaves CX as the caller set it: the level.
            let att = p("ds", "si");
            let g = Gear { missile_cx: m.mem.r8(att.seg, att.off + 6) as u16, ..*g };
            (Some(m.critical_multiplier(att, &g)), None)
        }
        "weapon_damage" => (Some(m.weapon_damage(reg("ds"), reg("es"), reg("ax"), g)?.0), None),
        "weapon_wear" => (Some(m.weapon_wear(p("ds", "si"), reg("es"), reg("ax"))), None),
        "creature_damage_taken" => (Some(m.creature_damage_taken(reg("ax"))), None),
        "creature_damage" => (Some(m.creature_damage(p("ds", "si"))?), None),
        _ => panic!("unknown routine {name}"),
    })
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/combat.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, g, rets) = (f[0], named(f[2]), gear(f[3]), f[4]);
        let diff = f.get(5).copied().unwrap_or("");

        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<&str> = w.split(':').collect();
            let seg = if p[0] == "m" { MSEG } else { DS };
            let (off, size, v) = (hex(p[1]), hex(p[2]), hex(p[3]));
            for i in 0..size {
                m.mem.w8(seg, (off + i) as u16, (v >> (8 * i)) as u8);
            }
        }
        m.load_rng();
        let before = m.mem.clone();
        let got = run(&mut m, name, &regs, &g);

        if rets.starts_with("fault") {
            assert!(got.is_none(), "{name}: original faults, port returned {got:?}\n{line}");
        } else {
            let (ax, cf) = got.unwrap_or_else(|| panic!("{name}: port faulted, original did not\n{line}"));
            let want = named(rets);
            if let Some(ax) = ax {
                assert_eq!(ax as u32, want["ax"], "{name}: ax\n{line}");
            }
            if let Some(cf) = cf {
                assert_eq!(cf as u32, want["cf"], "{name}: cf\n{line}");
            }
            m.store_rng();
            let want: Vec<(usize, u8)> = diff
                .split_whitespace()
                .map(|p| p.split_once(':').unwrap())
                .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
                .collect();
            let have: Vec<(usize, u8)> = REGIONS.iter().flat_map(|&(lo, hi)| m.mem.diff(&before, lo..hi)).collect();
            assert_eq!(have, want, "{name}: memory differs\n{line}");
        }
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 10);
}

/// Fixed (kept bug #19): an Archer's critical chance with a missile
/// weapon is 3 x level; the original uses what the missile-table search
/// left in CX (here 0: never a critical).
#[test]
fn fixed_archer_crits_grow_with_level() {
    use arena::fixes::{ARCHER_CRIT, Fixes};
    let Some(img) = common::image() else { return };
    let crits = |fixes: Fixes, level: u8| {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.mem.w8(MSEG, 5, 0x4D);
        m.mem.w8(MSEG, 6, level);
        m.rng.seed = 777;
        let g = Gear { missile: true, missile_cx: 0, ..Gear::default() };
        (0..1000).filter(|_| m.critical_multiplier(Ptr::new(MSEG, 0), &g) == 3).count()
    };
    assert_eq!(crits(Fixes::NONE, 20), 0);
    let n = crits(Fixes::only(ARCHER_CRIT), 20);
    assert!((520..680).contains(&n), "level 20: {n} of 1000");
    let n = crits(Fixes::only(ARCHER_CRIT), 5);
    assert!((100..200).contains(&n), "level 5: {n} of 1000");
}
