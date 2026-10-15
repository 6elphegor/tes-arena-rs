//! Differential test of inventory and equipment against vectors produced
//! by running the original code (harness/gen_inventory.py).
mod common;

use arena::machine::Machine;
use std::collections::HashMap;

const LO: usize = 0x3276 * 16;
const HI: usize = 0x3BA0 * 16 + 0x10000;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

/// Mirrors `fill_slots` in harness/gen_inventory.py.
fn fill_slots(seed: u32) -> Vec<u8> {
    let mut x = seed | 1;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    let mut out = vec![0u8; 40 * 19];
    for s in 0..40 {
        if next() % 5 == 0 {
            continue;
        }
        let mut b: Vec<u8> = (0..19).map(|_| next() as u8).collect();
        let kind = next() % 8;
        let (ty, cat) = match kind {
            0..=2 => ((next() % 18) as u8, 1),
            3..=4 => ((next() % 11) as u8, 0),
            5 => ((0x80 | (next() % 0x10)) as u8, (next() % 4) as u8),
            6 => (b[0], (next() % 4) as u8),
            _ => ((next() % 4) as u8, 2),
        };
        let mut flags = b[14] & 0xF3 | cat << 2;
        flags = if next() % 16 != 0 { flags | 0x40 } else { flags & 0xBF };
        if next() % 3 == 0 {
            b[6] = 0;
            b[7] = 0;
        }
        b[0] = ty;
        b[14] = flags;
        out[s * 19..(s + 1) * 19].copy_from_slice(&b);
    }
    out
}

fn poke(m: &mut Machine, lin: u32, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        let a = lin + i as u32;
        m.mem.w8((a >> 4) as u16, (a & 15) as u16, b);
    }
}

/// Runs one routine; returns its result registers (cf/sf as 0/1).
fn run(m: &mut Machine, name: &str, r: &HashMap<String, u32>) -> HashMap<&'static str, u16> {
    let reg = |k: &str| r.get(k).copied().unwrap_or(0) as u16;
    let ds = r.get("ds").map_or(0x3BA0, |&v| v as u16);
    let mut out = HashMap::new();
    let mut put = |k: &'static str, v: u16| {
        out.insert(k, v);
    };
    match name {
        "count_items" => {
            let (n, cf) = m.count_items(ds, reg("si"));
            put("ax", n);
            put("cf", cf as u16);
        }
        "base_type" => {
            let (t, cf) = m.base_type(reg("ax") as u8);
            put("ax", t);
            put("cf", cf as u16);
        }
        "find_item" => {
            let (f, si, cx) = m.find_item(ds, reg("si"), reg("cx"), reg("ax"), reg("bx"));
            put("cf", f as u16);
            put("si", si);
            put("cx", cx);
        }
        "find_equipped" => {
            let (f, si) = m.find_equipped(ds, reg("si"), reg("cx"), reg("ax"), reg("bx"), reg("cfin") != 0);
            put("cf", f as u16);
            put("si", si);
        }
        "find_player_equipped" => {
            let (f, si) = m.find_player_equipped(ds, reg("si"), reg("ax"), reg("bx"));
            put("cf", f as u16);
            put("si", si);
        }
        "equipped_shield" => {
            let s = m.equipped_shield(ds, reg("si"));
            put("cf", s.is_some() as u16);
            put("si", s.unwrap_or(reg("si")));
        }
        "shield_at" => {
            let (covers, si) = m.shield_at(ds, reg("si"), reg("ax"));
            put("sf", covers as u16);
            put("si", si);
        }
        "armor_at" => put("ax", m.armor_at(ds, reg("si"), reg("ax"))),
        "equipped_weapon" => put("si", m.equipped_weapon(ds, reg("si"))),
        "has_missile_weapon" => put("cf", m.has_missile_weapon(ds, reg("si")) as u16),
        "weapon_bonus" => put("ax", m.weapon_bonus(ds, reg("si"))),
        "nth_usable_item" => put("si", m.nth_usable_item(ds, reg("si"), reg("ax"))),
        "free_slot" => {
            let (cf, si, bx) = match m.free_slot(ds, reg("si")) {
                Ok((si, i)) => (false, si, i),
                Err(si) => (true, si, 40),
            };
            put("cf", cf as u16);
            put("si", si);
            put("bx", bx);
        }
        "class_wears_no_armor" => put("cf", m.class_wears_no_armor(reg("ax")) as u16),
        "class_can_use" => put("cf", m.class_can_use(ds, reg("si"), reg("ax")).expect("terminates") as u16),
        "item_has_effect" => put("cf", m.item_has_effect(ds, reg("si")) as u16),
        "condition_name" => put("si", m.condition_name(ds, reg("si"))),
        "add_item" => {
            let seg = m.mem.r16(0x3BA0, 0x4EFE);
            let rec = m.mem.r16(0x3BA0, 0x4F00);
            m.add_item(ds, reg("si"), seg, rec);
        }
        "remove_item" => m.remove_item(ds, reg("si"), reg("ax")),
        "player_armor_class" => m.player_armor_class(),
        "memmove" => {
            let (si, di) = m.memmove(ds, reg("si"), reg("es"), reg("di"), reg("cx"));
            put("si", si);
            put("di", di);
        }
        // The original returns CF clear on a clash.
        "accessory_clash" => put("cf", !m.accessory_clash(reg("si")) as u16),
        "sort_inventory" => m.sort_inventory(),
        "hit_chance" => {
            use arena::combat::{Gear, Ptr};
            let (att, def) = (Ptr::new(reg("fs"), reg("si")), Ptr::new(reg("es"), reg("di")));
            let armor = m.defender_armor(def);
            let gear = Gear { armor, weapon: None, gauntlets: None, weapon_bonus: 0, missile: false, missile_cx: 0 };
            put("ax", m.hit_chance(att, def, &gear));
        }
        _ => panic!("unknown routine {name}"),
    }
    out
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/inventory.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, rets, diff) = (f[0], named(f[2]), f[3], f.get(4).copied().unwrap_or(""));

        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<&str> = w.split(':').collect();
            match p[0] {
                "inv" => poke(&mut m, hex(p[1]), &fill_slots(hex(p[2]))),
                "blob" => {
                    let b: Vec<u8> = (0..p[2].len()).step_by(2).map(|i| hex(&p[2][i..i + 2]) as u8).collect();
                    poke(&mut m, hex(p[1]), &b);
                }
                // A segment value: image-relative here, relocated in the emulator.
                "seg" => poke(&mut m, hex(p[1]), &(hex(p[2]) as u16).to_le_bytes()),
                _ => {
                    let (lin, size, v) = (hex(p[0]), hex(p[1]), hex(p[2]));
                    poke(&mut m, lin, &v.to_le_bytes()[..size as usize]);
                }
            }
        }
        let before = m.mem.clone();
        let got = run(&mut m, name, &regs);
        for (k, want) in named(rets) {
            assert_eq!(got.get(k.as_str()).map(|&v| v as u32), Some(want), "{name}: {k}\n{line}");
        }
        let want: Vec<(usize, u8)> = diff
            .split_whitespace()
            .map(|p| p.split_once(':').unwrap())
            .map(|(a, b)| (hex(a) as usize, hex(b) as u8))
            .collect();
        assert_eq!(m.mem.diff(&before, LO..HI), want, "{name}: memory differs\n{line}");
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 24);
}

/// The scan in `class_can_use` stops at an FFh or a match; with neither
/// anywhere in segment 3276 the original never returns.
#[test]
fn class_list_without_terminator_is_none() {
    use arena::mem::DATA;
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    for off in 0..=0xFFFFu16 {
        m.mem.w8(DATA, off, 0x00);
    }
    // A weapon of base type 1, class 0, in a segment past 3276's reach
    // (3276 overlaps DGROUP, so an item there would be scanned too).
    let (seg, it) = (0x4AF7, 0x6E);
    m.mem.w8(seg, it, 1);
    m.mem.w16(seg, it + 6, 10);
    m.mem.w8(seg, it + 0x0E, 0x40 | 1 << 2);
    m.mem.w16(DATA, 0x3C25, 0x1000);
    assert_eq!(m.class_can_use(seg, it, 0), None);
    m.mem.w8(DATA, 0x1234, 0xFF);
    assert_eq!(m.class_can_use(seg, it, 0), Some(false));
    m.mem.w8(DATA, 0x1100, 1);
    assert_eq!(m.class_can_use(seg, it, 0), Some(true));
}
