//! Lists a level's stairs (map squares whose low byte is the INF's stair
//! markers, C932 up and C933 down: a crypt's are flats, 800Eh) nearest
//! the player's start, then for
//! the first few stands the player on the open square beside one, facing
//! it, and walks into it frame by frame: the level changes through the
//! game's stairs hook (CD6E, 0000:51B8) and its level hook (C3B8,
//! 0000:44D7) as the 3D mode sets them.
//!
//! Usage: stairs GAME_DIR LEVEL [N]
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
use arena::controls::Intent;
use arena::engine::Input;
use arena::machine::Machine;
use arena::mem::DS;
use arena::movement::var::{HEADING, LEVEL, STAIRS_DOWN, STAIRS_UP, X, Y};

/// Says which of the stairs' hooks ran.
struct Show;

impl arena::hooks::StairsHooks for Show {
    fn light_table(&mut self, m: &mut Machine) {
        println!("    the light table made again (2294:0B8A)");
        world::light_table(m);
    }
    fn automap_keep(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) {
        println!("    the automap kept (292B:095E)");
    }
    fn automap_load(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) {
        println!("    the new level's automap (292B:09AC)");
    }
    fn level_piles(&mut self, m: &mut Machine) -> Option<()> {
        println!("    the level's piles of treasure (2433:0000)");
        m.piles()
    }
}
impl arena::controls::ControlHooks for Show {}
impl arena::weapon::WeaponHooks for Show {}
impl arena::clock::ClockHooks for Show {}
impl arena::frame::FrameHooks for Show {}
impl arena::engine::EngineHooks for Show {}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(3).map(|s| s.parse().expect("N")).unwrap_or(4);
    let e = world::load(std::path::Path::new(&a[1]), &a[2]);
    let m = &e.m;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    let (px, py) = (m.mem.r16(DS, X) as i32 >> 7, m.mem.r16(DS, Y) as i32 >> 7);
    let marks = [m.mem.r8(DS, STAIRS_UP), m.mem.r8(DS, STAIRS_DOWN)];
    println!("level {} of {}, stairs up {:02x} down {:02x}", m.mem.r8(DS, LEVEL), m.mem.r8(DS, 0xC355), marks[0], marks[1]);
    let mut found = vec![];
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, row << 8 | col << 1);
            if v == 0 {
                continue;
            }
            let Some(i) = marks.iter().position(|&s| v as u8 == s) else { continue };
            // An open square beside it, and the heading from there to it
            // (0 towards +y, 80h +x, 100h -y, 180h -x).
            let side = [(0i32, -1i32, 0u16), (0, 1, 0x100), (-1, 0, 0x80), (1, 0, 0x180)].into_iter().find(|&(dc, dr, _)| {
                let (c, r) = (col as i32 + dc, row as i32 + dr);
                c >= 0 && r >= 0 && m.mem.r16(walls, ((r as u16) << 8) | (c as u16) << 1) == 0
            });
            let dist = (col as i32 - px).abs() + (row as i32 - py).abs();
            found.push((dist, col, row, i, side));
        }
    }
    found.sort();
    println!("{} stairs", found.len());
    for &(_, col, row, i, side) in found.iter().take(n) {
        println!("({col}, {row}) {}", ["up", "down"][i]);
        let Some((dc, dr, heading)) = side else { continue };
        let mut e2 = world::load(std::path::Path::new(&a[1]), &a[2]);
        let (c, r) = (col as i32 + dc, row as i32 + dr);
        e2.m.mem.w16(DS, X, (c as u16) << 7 | 0x40);
        e2.m.mem.w16(DS, Y, (r as u16) << 7 | 0x40);
        e2.m.mem.w16(DS, HEADING, heading);
        e2.world_loop_start();
        if std::env::var("NO_WALK_TEST").is_ok() {
            e2.m.mem.w32(DS, 0xCD6A, 0);
        }
        if std::env::var("TRACE").is_ok() {
            println!("    hooks CD6A {:08x} CD6E {:08x} C3B8 {:08x} square {:04x}", e2.m.mem.r32(DS, 0xCD6A), e2.m.mem.r32(DS, 0xCD6E), e2.m.mem.r32(DS, 0xC3B8), e2.m.mem.r16(DS, 0xCB90));
        }
        let level = e2.m.mem.r8(DS, LEVEL);
        let walk = Intent { forward: 256, ..Default::default() };
        let mut went = false;
        for k in 0..40 {
            let t = e2.frame_ticks();
            e2.tick(t);
            if e2.frame(Input::Steer(&walk), &mut Show).is_none() {
                println!("    frame {k} met code not ported");
                went = true;
                break;
            }
            if std::env::var("TRACE").is_ok() {
                println!("    frame {k}: ({:#x}, {:#x}) height {:#x} square {:04x}", e2.m.mem.r16(DS, X), e2.m.mem.r16(DS, Y), e2.m.mem.r16(DS, 0xC3D2), e2.m.mem.r16(DS, 0xCB90));
            }
            if e2.m.mem.r8(DS, LEVEL) != level || e2.m.mem.r16(DS, 0xC398) & 0x80 != 0 {
                let m = &e2.m;
                println!("    frame {k}: level {} at ({:#x}, {:#x}) heading {:#x}{}", m.mem.r8(DS, LEVEL), m.mem.r16(DS, X), m.mem.r16(DS, Y),
                    m.mem.r16(DS, HEADING), if m.mem.r16(DS, 0xC398) & 0x80 != 0 { ", the area left" } else { "" });
                went = true;
                break;
            }
        }
        if std::env::var("TRACE").is_ok() {
            let m = &mut e2.m;
            let x = m.mem.r16(DS, X);
            for y in (0x9C0u16..0xA80).step_by(8) {
                m.mem.w16(DS, 0xF000, x);
                m.mem.w16(DS, 0xF002, y);
                print!(" {y:x}:{:02x?}", m.move_cell(0xF000));
            }
            println!();
        }
        if !went {
            println!("    still on level {level} at ({:#x}, {:#x})", e2.m.mem.r16(DS, X), e2.m.mem.r16(DS, Y));
        }
    }
}
