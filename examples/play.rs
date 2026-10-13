//! Runs the engine (`engine::Engine::frame`) on a level from your game
//! files with modern controls: the player walks forward and turns when
//! blocked, and swings now and then. Prints where the player is every
//! so often, the game time, and why it stopped if it did.
//!
//! Usage: play GAME_DIR LEVEL.MIF FRAMES

use arena::bsa::Files;
use arena::clock::ClockHooks;
use arena::controls::{ControlHooks, Intent};
use arena::engine::{Engine, EngineHooks, Frame, Input};
use arena::exe;
use arena::frame::FrameHooks;
use arena::machine::Machine;
use arena::mem::DS;
use arena::weapon::WeaponHooks;
use std::path::PathBuf;

/// The front end's side: nothing drawn, nothing asked.
struct Play;

impl ControlHooks for Play {}
impl WeaponHooks for Play {}
impl ClockHooks for Play {}
impl FrameHooks for Play {}
impl EngineHooks for Play {}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: play GAME_DIR LEVEL.MIF FRAMES");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let frames: usize = args[3].parse().expect("frames");
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(&dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buf, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x9030, buf);
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    let (seg, off) = m.heap_alloc_bytes(0x3E80).expect("heap");
    m.mem.w16(DS, 0xBA54, off);
    m.mem.w16(DS, 0xBA56, seg);
    m.mem.w16(DS, 0xCB8A, buf);
    m.mem.w16(DS, 0xCB88, 0x8000);
    for (i, b) in format!("{}\0", args[2]).bytes().enumerate() {
        m.mem.w8(DS, 0xF000 + i as u16, b);
        m.mem.w8(DS, 0xC37F + i as u16, b);
    }
    m.level_load(&mut files, 0xF000).expect("level");
    let info = m.mem.r16(DS, 0xCB84);
    // The world loop's hooks: the input step is set (the port's controls
    // run in its place), transitions too.
    m.mem.w32(DS, 0xC3B4, 0x2826_0002);
    m.mem.w32(DS, 0xC3AC, 0x0000_4470);
    // A creature segment for the actors' records.
    let (cseg, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x1042, cseg);
    // A living player.
    m.mem.w16(DS, 0x01FD, 50);
    m.mem.w16(DS, 0x01FF, 50);
    m.mem.w16(DS, 0x0201, 200);
    for i in 0..8 {
        m.mem.w8(DS, 0x01A4 + arena::character::field::ATTRS + i, 50);
        m.mem.w8(DS, 0x01A4 + arena::character::field::BASE_ATTRS + i, 50);
    }
    m.rng.seed = 1994;
    m.store_rng();

    let mut engine = Engine::new(m, files);
    engine.enter_area(info).expect("entering the area");
    let mut hooks = Play;
    let mut turn = 0i16;
    let mut last = (0, 0);
    for f in 0..frames {
        let v = engine.view();
        let blocked = (v.x, v.y) == last;
        last = (v.x, v.y);
        if blocked && f > 0 {
            turn = 64;
        } else if turn > 0 {
            turn -= 16;
        }
        let intent = Intent { forward: 256, look: turn.min(16), ..Default::default() };
        // A swing every 40 frames: the weapon drawn, a drag to the right.
        if f % 40 == 20 {
            let flags = engine.m.mem.r16(DS, 0x05E0) | 0x10;
            engine.m.mem.w16(DS, 0x05E0, flags);
            engine.m.mem.w8(DS, 0xAFF9, 2);
            engine.m.mouse_motion = (0x60, 0);
        } else {
            engine.m.mem.w8(DS, 0xAFF9, 0);
        }
        let ticks = engine.frame_ticks();
        engine.tick(ticks);
        match engine.frame(Input::Steer(&intent), &mut hooks) {
            Some(Frame::Running) => {}
            Some(Frame::Left) => {
                println!("frame {f}: the area was left");
                break;
            }
            None => {
                println!("frame {f}: stopped (unported code or a fault)");
                break;
            }
        }
        if std::env::var("PLAY_DEBUG").is_ok() && f < 12 {
            let r = |a| engine.m.mem.r16(DS, a);
            println!("{f}: x {:#x} y {:#x} h {:#x} accel {:#x} speed {:#x} walk {:#x} c398 {:#x} c39a {:#x} 5b1 {:#x} jump {:#x} hp {:#x}",
                r(0xC3D0), r(0xC3D4), r(0xC3D2), r(0x7A46), r(0xCCBA), r(0xCCBE), r(0xC398), r(0xC39A), r(0x05B1), r(0x05E3) & 0xFF, r(0x01FD));
        }
        if f % 100 == 0 {
            let v = engine.view();
            println!(
                "frame {f}: at ({:#x}, {:#x}) heading {:#x} height {:#x}; minute {}, hand frame {:#x}",
                v.x,
                v.y,
                v.heading,
                v.height,
                engine.m.mem.r32(DS, 0x05CC),
                v.hand_frame
            );
        }
    }
    let v = engine.view();
    println!("end: at ({:#x}, {:#x}), minute {}", v.x, v.y, engine.m.mem.r32(DS, 0x05CC));
}
