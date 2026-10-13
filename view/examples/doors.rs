//! Lists the doors (wall kind 0Bh) of a level nearest the player's start,
//! with their state, the way to face them (heading units) and their lock.
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
use arena::mem::DS;
use arena::movement::var::{X, Y};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let e = world::load(std::path::Path::new(&a[1]), &a[2]);
    let m = &e.m;
    let (x, y) = (m.mem.r16(DS, X) as f32, m.mem.r16(DS, Y) as f32);
    println!("player at ({x}, {y}), square ({}, {})", x as u16 >> 7, y as u16 >> 7);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    let mut doors = Vec::new();
    for row in 0..d {
        for col in 0..w {
            let sq = row << 8 | col << 1;
            let v = m.mem.r16(walls, sq);
            if v >> 12 == 0xB {
                let (dx, dy) = (col as f32 * 128.0 + 64.0 - x, row as f32 * 128.0 + 64.0 - y);
                doors.push(((dx * dx + dy * dy).sqrt(), col, row, v, dx.atan2(dy)));
            }
        }
    }
    doors.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = m.mem.r8(DS, arena::movement::var::LOCK_COUNT) as u16;
    for (dist, col, row, v, angle) in doors.into_iter().take(10) {
        let key = row << 8 | col;
        let lock = (0..n).map(|i| arena::movement::var::LOCKS + 3 * i).find(|&r| m.mem.r16(DS, r) == key);
        let heading = (angle / std::f32::consts::TAU * 512.0).rem_euclid(512.0) as u16;
        println!("door ({col}, {row}) {v:04x} {:.1} squares, heading {heading}, lock {:?}", dist / 128.0, lock.map(|r| m.mem.r8(DS, r + 2)));
        for r in row.saturating_sub(1)..=row + 1 {
            let line: Vec<String> = (col.saturating_sub(1)..=col + 1).map(|c| format!("{:04x}", m.mem.r16(walls, r << 8 | c << 1))).collect();
            println!("    {}", line.join(" "));
        }
    }
}
