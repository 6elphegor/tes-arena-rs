//! Differential test of shop stock and potions against vectors produced
//! by running the original code (harness/gen_shop.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;

/// Image-relative segment the harness puts the stock lists in.
const LIST: u16 = 0x5000;
const REGIONS: [(usize, usize); 2] = [(0x3276 * 16, 0x3BA0 * 16 + 0x10000), (0x50000, 0x60000)];

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

/// Runs one routine; returns the registers it defines as results, or
/// None where the port reports the original's fault or hang.
fn run(m: &mut Machine, name: &str, regs: &HashMap<String, u32>) -> Option<HashMap<&'static str, u16>> {
    let reg = |r: &str| regs.get(r).copied().unwrap_or(0) as u16;
    let mut out = HashMap::new();
    match name {
        "class_can_use" => drop(out.insert("cf", m.shop_class_can_use(reg("ax"), reg("si"))? as u16)),
        "key_is_duplicate" => drop(out.insert("cf", m.shop_key_is_duplicate()? as u16)),
        "sort" => m.shop_sort(reg("bx"), reg("dx") as u8),
        "potion" => m.shop_potion()?,
        "roll_level" => drop(out.insert("ax", m.shop_roll_level()?)),
        "price_factors" => m.shop_price_factors(reg("ax"))?,
        "magic_weapon" => m.shop_magic_weapon()?,
        "magic_armor" => m.shop_magic_armor()?,
        "generate_stock" => m.shop_generate_stock()?,
        "open" => m.shop_open()?,
        "magic_item" => m.shop_magic_item(reg("ax") as u8, reg("bx") as u8)?,
        _ => panic!("unknown routine {name}"),
    }
    Some(out)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/shop.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, rets, diff) = (f[0], named(f[2]), f[3], f.get(4).copied().unwrap_or(""));

        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<&str> = w.split(':').collect();
            // "Loff" is in the list segment; "off:2:s:seg" is a segment
            // value the emulator relocated.
            let (seg, off) = match p[0].strip_prefix('L') {
                Some(o) => (LIST, hex(o)),
                None => (DS, hex(p[0])),
            };
            let (size, v) = (hex(p[1]), hex(p[p.len() - 1]));
            for i in 0..size {
                m.mem.w8(seg, (off + i) as u16, (v >> (8 * i)) as u8);
            }
        }
        m.load_rng();
        let before = m.mem.clone();
        let got = run(&mut m, name, &regs);

        if rets == "fault" || rets == "hang" {
            assert!(got.is_none(), "{name}: original {rets}s, port returned {got:?}\n{line}");
        } else {
            let got = got.unwrap_or_else(|| panic!("{name}: port faulted, original did not\n{line}"));
            for (key, want) in named(rets) {
                let v = got[key.as_str()];
                assert_eq!(v as u32, want, "{name}: {key}\n{line}");
            }
            m.store_rng();
            let want: Vec<(usize, u8)> = diff
                .split_whitespace()
                .map(|p| p.split_once(':').unwrap())
                .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
                .collect();
            let have: Vec<(usize, u8)> =
                REGIONS.iter().flat_map(|&(lo, hi)| m.mem.diff(&before, lo..hi)).collect();
            assert_eq!(have, want, "{name}: memory differs\n{line}");
        }
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 11);
}
