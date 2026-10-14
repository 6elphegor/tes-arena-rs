//! Lists a level's trigger walls (C917: a city's buildings and gates, an
//! interior's ways out) nearest the player's start, then for the first
//! few stands the player on the open square beside one and clicks it
//! (`Machine::wall_trigger`, 0000:43B1): what the original would do.
//!
//! Usage: triggers GAME_DIR LEVEL [N]
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
use arena::machine::Machine;
use arena::mem::DS;
use arena::movement::var::{TRIGGERS, X, Y};

/// Prints the messages and the area changes the trigger asks for.
struct Show;

impl arena::trigger::TriggerHooks for Show {
    fn message_box(&mut self, m: &mut Machine, text: u16) {
        println!("    message: {:?}", String::from_utf8_lossy(m.mem.cstr(DS, text)));
    }
    fn leave_interior(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        println!("    leaves the interior");
        Some(())
    }
    fn leave_city(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        println!("    out to the wilderness");
        Some(())
    }
    fn guards(&mut self, _m: &mut Machine) {
        println!("    the guards are called");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(3).map(|s| s.parse().expect("N")).unwrap_or(8);
    let e = world::load(std::path::Path::new(&a[1]), &a[2]);
    let m = &e.m;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    let (px, py) = (m.mem.r16(DS, X) as i32 >> 7, m.mem.r16(DS, Y) as i32 >> 7);
    let trig: Vec<u8> = (0..16).map(|i| m.mem.r8(DS, TRIGGERS + i)).collect();
    println!("triggers {trig:02x?}");
    let mut found = vec![];
    for row in 0..d {
        for col in 0..w {
            let sq = row << 8 | col << 1;
            let v = m.mem.r16(walls, sq);
            if v == 0 || ((v as i16) >= 0 && (v >> 8) as u8 != v as u8) {
                continue;
            }
            let Some(i) = trig.iter().position(|&t| t == v as u8 & 0x3F) else { continue };
            // An open square beside it, to stand on.
            let side = [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)].into_iter().find(|&(dc, dr)| {
                let (c, r) = (col as i32 + dc, row as i32 + dr);
                c >= 0 && r >= 0 && m.mem.r16(walls, ((r as u16) << 8) | (c as u16) << 1) == 0
            });
            let dist = (col as i32 - px).abs() + (row as i32 - py).abs();
            found.push((dist, col, row, i, side));
        }
    }
    found.sort();
    println!("{} trigger walls", found.len());
    for &(_, col, row, i, side) in found.iter().take(n) {
        println!("({col}, {row}) trigger {i}");
        let Some((dc, dr)) = side else { continue };
        let mut e2 = world::load(std::path::Path::new(&a[1]), &a[2]);
        let m = &mut e2.m;
        let (c, r) = (col as i32 + dc, row as i32 + dr);
        m.mem.w16(DS, X, (c as u16) << 7 | 0x40);
        m.mem.w16(DS, Y, (r as u16) << 7 | 0x40);
        m.mem.w16(DS, arena::controls::var::PICKED, row << 8 | col << 1);
        m.mem.w16(DS, 0x0D58, 0);
        let r = m.wall_trigger(&mut e2.files, &mut Show);
        let left = m.mem.r16(DS, 0xC398) & 0x80 != 0;
        let file = m.mem.r16(DS, 0x0D58);
        let into = if file != 0 { String::from_utf8_lossy(m.mem.cstr(DS, file)).into_owned() } else { String::new() };
        println!("    -> {r:?} left {left} area {} {into}", m.mem.r8(DS, arena::trigger::var::AREA));
    }
}
