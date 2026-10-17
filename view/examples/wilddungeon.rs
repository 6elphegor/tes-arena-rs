//! Out of a city's gate into the wilderness, into one of the wilderness's
//! dungeons and back out, as the view follows them (`areas::next`).
//!
//! A dungeon's entrance in the wilderness is a flat, the dungeon's keeper
//! (C943; in WILD010, WILD017 and WILD049, for instance): clicked, the
//! talk (1194:DB6C) takes the player in (0000:4EFD, 08EC). The window of
//! blocks is moved over the wilderness (217B:0265, the player stepped a
//! block at a time) until one is in it; the player stands beside it and
//! clicks it, the dungeon is made (2294:0ED4, 0000:28BC) and entered, and
//! its first level's way up walked into leads back out to the wilderness
//! where the player was (C39A bit 2000h).
//!
//! Usage: wilddungeon GAME_DIR CITY:P:I
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
#[path = "../src/areas.rs"]
mod areas;
#[path = "../src/light.rs"]
mod light;
use arena::controls::Intent;
use arena::engine::{Engine, Frame, Input};
use arena::machine::Machine;
use arena::mem::DS;
use arena::movement::var::{HEADING, LEVEL, STAIRS_UP, TRIGGERS, X, Y};
use arena::transit::Run;
use std::path::Path;

/// The view's hooks for what this needs: the gate out (217B:027A), the
/// wilderness's blocks following the player, the keeper's click, the
/// stairs' light table.
struct Front;

impl arena::trigger::TriggerHooks for Front {
    fn message_box(&mut self, m: &mut Machine, text: u16) {
        println!("    message: {:?}", String::from_utf8_lossy(m.mem.cstr(DS, text)));
    }
    fn leave_city(&mut self, m: &mut Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_leave_city(files, &mut arena::wild::PortWild);
        Some(())
    }
    fn enter_dungeon(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_done();
        Some(())
    }
}
impl arena::text::TextHooks for Front {}
impl arena::talk::TalkHooks for Front {
    fn dungeon_entrance(&mut self, m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_dungeon_entry();
        Some(())
    }
}
impl arena::hooks::StairsHooks for Front {
    fn light_table(&mut self, m: &mut Machine) {
        world::light_table(m);
    }
}
impl arena::controls::ControlHooks for Front {
    fn wall_trigger(&mut self, m: &mut Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wall_trigger(files, self)
    }
    fn wild_recentre(&mut self, m: &mut Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_shift(files, &mut arena::wild::PortWild)
    }
}
impl arena::weapon::WeaponHooks for Front {}
impl arena::clock::ClockHooks for Front {}
impl arena::frame::FrameHooks for Front {}
impl arena::engine::EngineHooks for Front {}

/// The open square beside (`col`, `row`) and the heading from there to it
/// (0 towards +y, 80h +x, 100h -y, 180h -x).
fn beside(m: &Machine, col: u16, row: u16) -> Option<(u16, u16, u16)> {
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    [(0i32, -1i32, 0u16), (0, 1, 0x100), (-1, 0, 0x80), (1, 0, 0x180)].into_iter().find_map(|(dc, dr, h)| {
        let (c, r) = (col as i32 + dc, row as i32 + dr);
        let open = c >= 0 && r >= 0 && c < 128 && r < 128 && m.mem.r16(walls, ((r as u16) << 8) | (c as u16) << 1) == 0;
        open.then_some((c as u16, r as u16, h))
    })
}

/// Squares (column, row) of the map whose wall word satisfies `f`.
fn squares(m: &Machine, f: impl Fn(u16) -> bool) -> Vec<(u16, u16)> {
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    (0..d).flat_map(|r| (0..w).map(move |c| (c, r))).filter(|&(c, r)| f(m.mem.r16(walls, r << 8 | c << 1))).collect()
}

fn stand(m: &mut Machine, (c, r, h): (u16, u16, u16)) {
    m.mem.w16(DS, X, c << 7 | 0x40);
    m.mem.w16(DS, Y, r << 7 | 0x40);
    m.mem.w16(DS, HEADING, h);
    // The real position too (BA62), which each frame's physics start
    // from (0000:A3AA).
    for i in 0..3u16 {
        let w = m.mem.r16(DS, X + 2 * i);
        m.mem.w16(DS, 0xBA62 + 2 * i, w);
    }
}

fn place(e: &Engine, run: Run) {
    let m = &e.m;
    println!(
        "  {run:?}: {} at ({:#x}, {:#x}) heading {:#x}, level {} of {}, map {}x{}, origin ({}, {}), C39A {:04x}",
        String::from_utf8_lossy(m.mem.cstr(DS, arena::transit::var::LEVEL_NAME)),
        m.mem.r16(DS, X),
        m.mem.r16(DS, Y),
        m.mem.r16(DS, HEADING),
        m.mem.r8(DS, LEVEL),
        m.mem.r8(DS, 0xC355),
        m.mem.r16(DS, 0xC357),
        m.mem.r16(DS, 0xC359),
        m.mem.r16(DS, arena::wild::var::ORIGIN_X),
        m.mem.r16(DS, arena::wild::var::ORIGIN_Y),
        m.mem.r16(DS, 0xC39A)
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = Path::new(&a[1]);
    let mut e = world::load(dir, &a[2]);
    let mut run = areas::start(&mut e, true, true);

    // A city's gates are its triggers 7 and 8 (898B).
    let trig: Vec<u8> = (0..16).map(|i| e.m.mem.r8(DS, TRIGGERS + i)).collect();
    let gate = squares(&e.m, |v| {
        v != 0 && ((v as i16) < 0 || (v >> 8) as u8 == v as u8) && matches!(trig.iter().position(|&t| t == v as u8 & 0x3F), Some(7 | 8))
    })
    .into_iter()
    .find_map(|(c, r)| beside(&e.m, c, r).map(|b| (c, r, b)))
    .expect("a gate");
    stand(&mut e.m, gate.2);
    e.m.mem.w16(DS, arena::controls::var::PICKED, gate.1 << 8 | gate.0 << 1);
    e.m.wall_trigger(&mut e.files, &mut Front).expect("the gate");
    println!("the gate at ({}, {})", gate.0, gate.1);
    run = areas::next(&mut e, dir, run).expect("the wilderness");
    place(&e, run);
    if std::env::var("TRACE").is_ok() {
        let m = &e.m;
        let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
        let (pc, pr) = (m.mem.r16(DS, X) >> 7, m.mem.r16(DS, Y) >> 7);
        println!("    window {:02x?}, the player's square ({pc}, {pr}):", (0..4).map(|i| m.mem.r8(DS, arena::wild::var::WINDOW + i)).collect::<Vec<_>>());
        for r in pr - 2..=pr + 2 {
            let row: Vec<String> = (pc - 2..=pc + 2).map(|c| format!("{:04x}", m.mem.r16(walls, r << 8 | c << 1))).collect();
            println!("    row {r}: {}", row.join(" "));
        }
        // For harness/check_move.py: DGROUP and the wall and floor
        // planes as they are now, and the port's single move (0000:F8DD,
        // speed 40h) each way from here.
        if let Ok(d) = std::env::var("DUMP") {
            let seg_bytes = |s: u16| (0..=0xFFFFu16).map(|o| m.mem.r8(s, o)).collect::<Vec<u8>>();
            std::fs::write(format!("{d}/ds.bin"), seg_bytes(DS)).unwrap();
            for (name, var) in [("walls", arena::mif::var::MAP1_SEG), ("floor", arena::mif::var::FLOOR_SEG)] {
                let s = m.mem.r16(DS, var);
                std::fs::write(format!("{d}/{name}_{s:04x}.bin"), seg_bytes(s)).unwrap();
            }
            for h in [0u16, 0x80, 0x100, 0x180] {
                let mut t = e.m.clone();
                let r = t.move_step(&mut e.files.clone(), &mut Front, 0x40, h);
                println!("    port move heading {h:#x}: {r:?} to ({:#x}, {:#x})", t.mem.r16(DS, X), t.mem.r16(DS, Y));
            }
        }
        // Each way from where the player arrived, a few frames' walk.
        for h in [0u16, 0x80, 0x100, 0x180] {
            let mut t = Engine::new(e.m.clone(), e.files.clone());
            t.m.mem.w16(DS, HEADING, h);
            let walk = Intent { forward: 256, ..Default::default() };
            for _ in 0..8 {
                let k = t.frame_ticks();
                t.tick(k);
                if t.frame(Input::Steer(&walk), &mut Front).is_none() {
                    println!("    heading {h:#x}: code not ported");
                    break;
                }
            }
            println!("    heading {h:#x}: to ({:#x}, {:#x}), height {:#x}", t.m.mem.r16(DS, X), t.m.mem.r16(DS, Y), t.m.mem.r16(DS, 0xC3D2));
        }
    }

    // The window moved a block at a time, out in a spiral, until a
    // keeper is in it.
    let keeper = e.m.mem.r8(DS, arena::talk::var::KEEPER_FLAT);
    let is_keeper = |v: u16| v >> 12 == 8 && v as u8 == keeper;
    let mut found = squares(&e.m, is_keeper);
    let mut steps = 0;
    let (mut dx, mut dy, mut leg) = (1i32, 0i32, 1);
    'search: while found.is_empty() && steps < 400 {
        for _ in 0..2 {
            for _ in 0..leg {
                for (at, d) in [(X, dx), (Y, dy)] {
                    let v = (e.m.mem.r16(DS, at) as i32 + d * 0x1000) as u16;
                    e.m.mem.w16(DS, at, v);
                }
                e.m.wild_shift(&mut e.files, &mut arena::wild::PortWild).expect("the shift");
                steps += 1;
                found = squares(&e.m, is_keeper);
                if !found.is_empty() {
                    break 'search;
                }
            }
            (dx, dy) = (-dy, dx);
        }
        leg += 1;
    }
    let Some(&(col, row)) = found.iter().find(|&&(c, r)| beside(&e.m, c, r).is_some()) else {
        println!("no keeper found in {steps} steps");
        return;
    };
    println!(
        "a dungeon's keeper (flat {keeper:#x}) at ({col}, {row}), window origin ({}, {}) after {steps} steps",
        e.m.mem.r16(DS, arena::wild::var::ORIGIN_X),
        e.m.mem.r16(DS, arena::wild::var::ORIGIN_Y)
    );
    // Clicked as the view's drawing finds it: the flat's scratch record
    // (0000:D18E), made ready (2294:047B), then the click (1194:DB6C).
    let b = beside(&e.m, col, row).unwrap();
    stand(&mut e.m, b);
    let base = e.m.mem.r16(DS, arena::info::var::FLAT_BASE);
    world::flat_drawn(&mut e.m, col, row, keeper as u16 + base);
    e.m.mem.w16(DS, arena::talk::var::CLICKED, arena::talk::var::FLAT_DRAWN);
    e.m.clicked_flat();
    let r = e.m.actor_click(&mut e.files, &mut Front);
    println!(
        "  clicked: {r:?}, area {} left {}",
        e.m.mem.r8(DS, arena::trigger::var::AREA),
        e.m.mem.r16(DS, 0xC398) & 0x80 != 0
    );
    let saved = [arena::wild::var::SAVED_POS, arena::wild::var::SAVED_POS + 2].map(|at| e.m.mem.r16(DS, at));
    run = areas::next(&mut e, dir, run).expect("the dungeon");
    place(&e, run);

    // The way up on the first level leads out (the exit hook CD4E, 44BD).
    let up = e.m.mem.r8(DS, STAIRS_UP);
    let Some((c, r, b)) = squares(&e.m, |v| v != 0 && v as u8 == up).into_iter().find_map(|(c, r)| beside(&e.m, c, r).map(|b| (c, r, b))) else {
        println!("  no way up found");
        return;
    };
    println!("  the way up at ({c}, {r})");
    stand(&mut e.m, b);
    if std::env::var("TRACE").is_ok() {
        let m = &e.m;
        let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
        println!("    C932 {:02x} C933 {:02x}, standing at ({}, {}) heading {:#x}", m.mem.r8(DS, 0xC932), m.mem.r8(DS, 0xC933), b.0, b.1, b.2);
        for rr in r.saturating_sub(2)..r + 3 {
            let row: Vec<String> = (c.saturating_sub(2)..c + 3).map(|cc| format!("{:04x}", m.mem.r16(walls, rr << 8 | cc << 1))).collect();
            println!("    row {rr}: {}", row.join(" "));
        }
    }
    let walk = Intent { forward: 256, ..Default::default() };
    for k in 0..40 {
        let t = e.frame_ticks();
        e.tick(t);
        match e.frame(Input::Steer(&walk), &mut Front) {
            None => {
                println!("  frame {k} met code not ported");
                return;
            }
            Some(Frame::Left) => {
                println!("  frame {k}: the dungeon left");
                run = areas::next(&mut e, dir, run).expect("back out");
                place(&e, run);
                println!("  saved place ({:#x}, {:#x})", saved[0], saved[1]);
                return;
            }
            Some(Frame::Running) => {
                if std::env::var("TRACE").is_ok() {
                    let m = &e.m;
                    println!("    frame {k}: ({:#x}, {:#x}) heading {:#x} level {} trigger {:04x}", m.mem.r16(DS, X), m.mem.r16(DS, Y), m.mem.r16(DS, HEADING), m.mem.r8(DS, LEVEL), m.mem.r16(DS, 0xCB6A));
                }
            }
        }
    }
    println!("  still in the dungeon at ({:#x}, {:#x})", e.m.mem.r16(DS, X), e.m.mem.r16(DS, Y));
}
