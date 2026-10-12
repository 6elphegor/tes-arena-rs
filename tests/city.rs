//! Differential test of the city generator against vectors produced by
//! running the original code (harness/gen_city.py).
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

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/city.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, result) = (f[0], named(f[2]), f[3]);
        let diff = f.get(4).copied().unwrap_or("");

        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<u32> = w.split(':').map(hex).collect();
            for i in 0..p[1] {
                m.mem.w8(DS, (p[0] + i) as u16, (p[2] >> (8 * i)) as u8);
            }
        }
        m.load_rng();
        let before = m.mem.clone();

        // Some(result string) or None for a fault or hang.
        let mut blocks = None;
        let got: Option<String> = match name {
            "place_block" => m.city_place_block(regs["ax"] as u8).map(|_| "ok".into()),
            "city_plan" => m.city_plan().map(|_| "ok".into()),
            "block_name" => m.city_block_name(regs["si"] as u8).map(|r| match r {
                Some(off) => format!("load:{off:x}"),
                None => "none".into(),
            }),
            "generate_city" => m.generate_city().map(|b| {
                blocks = Some(b);
                "ok".into()
            }),
            "local_to_global" => m
                .city_local_to_global(regs["ax"] as u16, regs["bx"] as u16)
                .map(|(x, y)| format!("ax:{x:x} bx:{y:x}")),
            "quadrant" => {
                let q = m.city_quadrant(regs["ax"] as u16, regs["bx"] as u16, regs["cx"] as u8);
                Some(format!("ax:{q:x}"))
            }
            "map_y" => m.city_map_y(regs["ax"] as u16, regs["bx"] as u16).map(|(x, y)| format!("ax:{x:x} bx:{y:x}")),
            "template" => {
                let t = m.city_template(regs["ax"] as u8);
                Some(format!("ax:{:x} si:{:x} dx:{:x} bx:{:x} cx:{:x}", t.starts, t.format, t.flag, t.kind, t.count))
            }
            "reserve_blocks" => {
                m.city_reserve_blocks(regs["ax"] as u16);
                Some("ok".into())
            }
            "plan_for_record" => m.city_plan_for_record(DS, regs["di"] as u16).map(|_| "ok".into()),
            "build_location" => m
                .city_build_location(DS, regs["di"] as u16, regs["ax"] as u16, regs["climate"] as u8)
                .map(|_| "ok".into()),
            _ => panic!("unknown routine {name}"),
        };

        if result == "fault" || result == "hang" {
            assert!(got.is_none(), "{name}: original {result}s, port returned {got:?}\n{line}");
        } else {
            let got = got.unwrap_or_else(|| panic!("{name}: port failed, original did not\n{line}"));
            // place_block/city_plan report "ok" where the original returned.
            let want = if result == "ret" { "ok" } else { result };
            assert_eq!(got, want, "{name}: result\n{line}");
            m.store_rng();
            let want: Vec<(usize, u8)> = diff
                .split_whitespace()
                .map(|p| p.split_once(':').unwrap())
                .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
                .collect();
            assert_eq!(m.mem.diff(&before, LO..HI), want, "{name}: memory differs\n{line}");
            if let Some(b) = blocks {
                let have: Vec<String> =
                    b.iter().map(|(p, n)| format!("{p:x}={}", String::from_utf8_lossy(n))).collect();
                let want: Vec<&str> = f.get(5).map(|s| s.split_whitespace().collect()).unwrap_or_default();
                assert_eq!(have, want, "{name}: blocks\n{line}");
            }
        }
        *count.entry(format!("{name}:{result}")).or_default() += 1;
    }
    eprintln!("{count:?}");
    for n in [
        "place_block",
        "city_plan",
        "block_name",
        "generate_city",
        "local_to_global",
        "quadrant",
        "map_y",
        "template",
        "reserve_blocks",
        "build_location",
        "plan_for_record",
    ] {
        assert!(count.keys().any(|k| k.starts_with(n)), "no cases for {n}");
    }
}
