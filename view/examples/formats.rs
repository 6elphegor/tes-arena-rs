//! Counts the flats each level's decoders miss, by kind and IMG flags.
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
use arena::mem::DS;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    for level in &a[2..] {
        let e = std::panic::catch_unwind(|| world::load(dir, level));
        let Ok(e) = e else { println!("{level}: load failed"); continue };
        let m = &e.m;
        let fl = world::flats(m);
        let mut miss = std::collections::BTreeMap::new();
        for (f, d) in fl.iter().enumerate() {
            let off = m.mem.r16(DS, arena::info::var::FLAT_DATA + f as u16 * 4);
            if d.is_none() && off != 0xFFFF {
                let bank = m.mem.r16(DS, arena::info::var::FLAT_DATA + f as u16 * 4 + 2);
                let data = m.ems_bank(bank);
                let kind = m.mem.r8(DS, arena::info::var::FLAT_KINDS + f as u16);
                let flags = data[off as usize + 8] as u16 | (data[off as usize + 9] as u16) << 8;
                *miss.entry((kind, if kind == 0 { flags } else { 0 })).or_insert(0) += 1;
            }
        }
        println!("{level}: {} flats, {} decoded, missed {miss:x?}", fl.len(), fl.iter().flatten().count());
    }
}
