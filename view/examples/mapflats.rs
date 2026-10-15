//! Lists a level's flats standing in the map (their square, number, flat
//! record and image size) and the active actor slots, to check the
//! sprites' placement: arena-view's `mapflats` example GAME LEVEL.
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
    let e = world::load(dir, &a[2]);
    let m = &e.m;
    let fl = world::flats(m);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let base = m.mem.r16(DS, arena::info::var::FLAT_BASE);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    println!("player ({:#x}, {:#x}), flat base {base:#x}, keeper flat {:#x}", m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4), m.mem.r8(DS, 0xC943));
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            if v >> 12 != 8 {
                continue;
            }
            let n = (v & 0xFF) + base;
            let rec: Vec<u8> = (0..5).map(|i| m.mem.r8(DS, arena::info::var::FLATS + n * 5 + i)).collect();
            let size = fl.get(n as usize).and_then(|f| f.as_ref()).map(|f| (f.w, f.h, f.frames.len()));
            println!("flat ({col},{row}) n {n:#x} rec {rec:02x?} image {size:?}");
        }
    }
    for slot in 0..23u16 {
        let si = 0x0AB3 + slot * 0x1C;
        let flags = m.mem.r16(DS, si + 0xF);
        if flags & 0x4000 != 0 {
            continue;
        }
        let r = |o: u16| m.mem.r16(DS, si + o);
        println!("slot {slot} ({:#x}, {:#x}, {:#x}) sprite {:#x} frame {} flags {flags:#x}", r(0), r(2), r(4), m.mem.r8(DS, si + 0xC), m.mem.r8(DS, si + 0xD));
    }
}
