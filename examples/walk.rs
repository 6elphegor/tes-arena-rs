//! Walks through a level with the ported movement code and draws the
//! path over the map.
//!
//! Usage: walk GAME_DIR LEVEL.MIF STEPS OUT.ppm
//!
//! The level is loaded with the ported loader and the walker starts at
//! the level's start position, at full height. Each step it moves
//! forward (`move_step`); when blocked it turns by a random number of
//! quarters, and now and then it turns anyway, so it wanders. The
//! level's INF is loaded too (stairs, triggers, textures); door opening
//! isn't ported yet, so doors stay shut. The walk stops at stairs,
//! after the level change (the map drawn is then the new level).

use arena::bsa::Files;
use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const SCALE: usize = 4;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: walk GAME_DIR LEVEL.MIF STEPS OUT.ppm");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let steps: usize = args[3].parse().expect("steps");
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(&dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buf, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x9030, buf);
    // EMS on (32 banks through the buffer segment), and the INF buffer,
    // as tests/info.rs sets them up.
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    let (seg, off) = m.heap_alloc_bytes(0x3E80).expect("heap");
    m.mem.w16(DS, 0xBA54, off);
    m.mem.w16(DS, 0xBA56, seg);
    m.mem.w16(DS, 0xCB8A, buf);
    m.mem.w16(DS, 0xCB88, 0x8000);
    for (i, b) in format!("{}\0", args[2]).bytes().enumerate() {
        m.mem.w8(DS, 0xF000 + i as u16, b);
    }
    m.level_load(&mut files, 0xF000).expect("level");
    // The level file's name, for level changes on stairs.
    for (i, b) in format!("{}\0", args[2]).bytes().enumerate() {
        m.mem.w8(DS, 0xC37F + i as u16, b);
    }
    let info = m.mem.r16(DS, 0xCB84);
    if info != 0 {
        m.info_load(&mut files, info).expect("INF");
        println!("INF {}: stairs up {:#x}, down {:#x}", String::from_utf8_lossy(m.mem.cstr(DS, 0xC2B5)),
            m.mem.r8(DS, 0xC932), m.mem.r8(DS, 0xC933));
    }
    m.mem.w16(DS, 0xC3D2, 0x3C);
    m.rng.seed = 1994;

    let width = (m.mem.r16(DS, 0xC357) as usize).min(128);
    let depth = (m.mem.r16(DS, 0xC359) as usize).min(128);
    let mut heading: u16 = 0;
    let mut path = vec![(m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4))];
    for _ in 0..steps {
        m.mem.w16(DS, 0xC3DA, heading);
        let before = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
        if m.rng.next() % 24 == 0 {
            heading = (heading.wrapping_add((m.rng.next() % 4) * 0x80)) & 0x1FF;
            m.mem.w16(DS, 0xC3DA, heading);
        }
        let level = m.mem.r8(DS, 0xBB0A);
        if m.move_step(&mut files, 0x30, heading).is_none() {
            println!("stopped: a hook or a failed level change");
            break;
        }
        if m.mem.r8(DS, 0xBB0A) != level {
            println!("stairs: level {level} -> {}", m.mem.r8(DS, 0xBB0A));
            break;
        }
        let after = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
        if after == before {
            heading = (heading.wrapping_add(0x80 * (1 + m.rng.next() % 3))) & 0x1FF;
        }
        path.push(after);
    }

    let walls = m.mem.r16(DS, 0xD806);
    let floor = m.mem.r16(DS, 0xCCD0);
    let mut px = vec![0u8; width * SCALE * depth * SCALE * 3];
    for y in 0..depth * SCALE {
        for x in 0..width * SCALE {
            let off = ((y / SCALE) * 0x100 + (x / SCALE) * 2) as u16;
            let wall = m.mem.r16(walls, off);
            let fl = m.mem.r16(floor, off).wrapping_sub(buf);
            let rgb = if wall != 0 { [70, 70, 90] } else if fl >> 8 != 0 { [150, 160, 170] } else { [110, 110, 120] };
            px[(y * width * SCALE + x) * 3..][..3].copy_from_slice(&rgb);
        }
    }
    // The path: map units are 128 a square, the map's x and y as drawn.
    for (i, &(x, y)) in path.iter().enumerate() {
        let (px_x, px_y) = (x as usize * SCALE / 128, y as usize * SCALE / 128);
        if px_x < width * SCALE && px_y < depth * SCALE {
            let t = (i * 255 / path.len().max(1)) as u8;
            px[(px_y * width * SCALE + px_x) * 3..][..3].copy_from_slice(&[255, t, 0]);
        }
    }
    let mut ppm = format!("P6\n{} {}\n255\n", width * SCALE, depth * SCALE).into_bytes();
    ppm.extend_from_slice(&px);
    std::fs::write(&args[4], ppm).expect("write");
    let moved = path.windows(2).filter(|w| w[0] != w[1]).count();
    let squares: std::collections::HashSet<(u16, u16)> = path.iter().map(|&(x, y)| (x >> 7, y >> 7)).collect();
    println!("squares visited: {}", squares.len());
    println!("{} steps, {moved} moved; start {:?}, end {:?} -> {}", path.len() - 1, path[0], path.last().unwrap(), args[4]);
}
