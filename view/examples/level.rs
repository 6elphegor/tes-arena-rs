//! Prints what the view reads of a level after loading it: the textures,
//! flats, ceiling, light, time, the squares around the player, and a
//! city's townspeople.
//!
//! Usage: level GAME_DIR LEVEL.MIF
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
    let mut e = world::load(std::path::Path::new(&a[1]), &a[2]);
    world::city_people(&mut e);
    let m = &e.m;
    let v = e.view();
    let tex = world::textures(m);
    let fl = world::flats(m);
    println!("level lights {} (C8F0 {:#x} {:#x})", m.mem.r16(DS, arena::light::var::LEVEL_COUNT), m.mem.r8(DS, 0xC8F0), m.mem.r8(DS, 0xC8F1));
    let sh = world::shading(m);
    let same = |r: usize| (0..256).filter(|&i| sh[r * 256 + i] as usize == i).count();
    println!("shading rows identity count: {:?}", (0..16).map(same).collect::<Vec<_>>());
    println!("textures {} (non-empty {}), flats {} (decoded {})", tex.len(), tex.iter().filter(|t| t.iter().any(|&b| b != 0)).count(), fl.len(), fl.iter().flatten().count());
    println!("ceiling {:#x} map {}x{} light {:#x} time {:#x} level flags C39A {:#x} C398 {:#x}", m.mem.r16(DS, arena::movement::var::CEILING), m.mem.r16(DS, 0xC357), m.mem.r16(DS, 0xC359), v.light, v.time_of_day, m.mem.r16(DS, 0xC39A), m.mem.r16(DS, 0xC398));
    let (col, row) = (v.x >> 7, v.y >> 7);
    println!("player ({col}, {row}) heading {:#x} height {}", v.heading, v.height);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let floors = m.mem.r16(DS, arena::mif::var::FLOOR_SEG);
    let frame = m.mem.r16(DS, 0x9030);
    for r in row.saturating_sub(3)..=row + 3 {
        let line: Vec<String> = (col.saturating_sub(3)..=col + 3)
            .map(|c| {
                let sq = r << 8 | c << 1;
                format!("{:04x}/{:02x}", m.mem.r16(walls, sq), m.mem.r16(floors, sq).wrapping_sub(frame) >> 8)
            })
            .collect();
        println!("  {}", line.join(" "));
    }
    let n8 = (0..m.mem.r16(DS, 0xC359).min(128)).flat_map(|r| (0..m.mem.r16(DS, 0xC357).min(128)).map(move |c| (r, c))).filter(|&(r, c)| m.mem.r16(walls, r << 8 | c << 1) >> 12 == 8).count();
    println!("flat squares {n8}");
    let slots: Vec<u8> = (0..15u16).filter(|i| m.mem.r16(DS, 0x0AB3 + i * 0x1C + 0xF) & 0x4000 == 0).map(|i| m.mem.r8(DS, 0x0AB3 + i * 0x1C + 0x11)).collect();
    println!("townspeople {slots:x?}, files {:?}", world::townsfolk(m));
    let name: String = (0..13).map(|i| m.mem.r8(DS, arena::info::var::NAME + i)).take_while(|&c| c != 0).map(|c| c as char).collect();
    let used: Vec<u16> = (0..0x40).filter(|&i| m.mem.r8(DS, arena::info::var::USED + i) != 0).collect();
    let (bo, bs) = (m.mem.r16(DS, 0xBA54), m.mem.r16(DS, 0xBA56));
    let text: String = (0..80).map(|i| m.mem.r8(bs, bo + i)).map(|c| if c < 32 { '|' } else { c as char }).collect();
    println!("buffer {bs:04x}:{bo:04x} size {:#x}: {text}", m.mem.r16(DS, arena::bsa::var::OPEN_SIZE));
    println!("INF {name:?} (CB84 {:#x}), textures used {used:x?}, texture banks {:#x} {:#x}, flat number {}", m.mem.r16(DS, 0xCB84), m.mem.r8(DS, 0xC8F0), m.mem.r8(DS, 0xC8F1), m.mem.r16(DS, arena::info::var::FLAT_NUMBER));
}
