//! A level's trigger squares (0000:D3A0's table at C766, its count at
//! C866: column, row, text, sound), as the INF loader made them, and each
//! text's kind as `square_lookup` (0000:06BE) finds it.
//!
//! Usage: trigger_squares GAME_DIR LEVEL.MIF
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
    for level in &a[2..] {
        let dir = a[1].clone();
        let level = level.clone();
        let r = std::panic::catch_unwind(move || {
            let e = world::load(std::path::Path::new(&dir), &level);
            let n = e.m.mem.r8(DS, 0xC866) as u16;
            if n != 0 {
                println!("{level}: {n} trigger squares");
            }
            for i in 0..n {
                let at = 0xC766 + 4 * i;
                let b = |k: u16| e.m.mem.r8(DS, at + k);
                println!("  ({}, {}) text {:02x} sound {:02x}", b(0), b(1), b(2), b(3));
            }
        });
        let _ = r;
    }
}
