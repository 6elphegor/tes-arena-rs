//! Loads a saved game with the ported code and writes two images: the
//! screenshot stored in the save, and a map of the level the character
//! is in.
//!
//! Usage: save_view GAME_DIR SAVE_DIR SLOT SCREEN.ppm MAP.ppm

use arena::bsa::Files;
use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const SCREEN: u16 = 0x6000;
const SCALE: usize = 4;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        eprintln!("usage: save_view GAME_DIR SAVE_DIR SLOT SCREEN.ppm MAP.ppm");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let slot: u8 = args[3].parse().expect("slot");
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(&dir).expect("game directory");
    files.add_dir(&PathBuf::from(&args[2])).expect("save directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buffer, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x9030, buffer);
    m.mem.w16(DS, 0xA8FE, 0);
    m.mem.w16(DS, 0xA900, SCREEN);
    m.save_set_slot(slot);
    m.save_load_level(&mut files).expect("SAVEGAME for that slot");

    // The screenshot: 320 x 200 palette indices; the palette is VGA's
    // 6-bit red, green, blue.
    let mut ppm = b"P6\n320 200\n255\n".to_vec();
    for i in 0..0xFA00u16 {
        let c = m.mem.r8(SCREEN, i) as u16;
        for k in 0..3 {
            let v = m.mem.r8(DS, 0x929E + c * 3 + k) & 0x3F;
            ppm.push(v << 2 | v >> 4);
        }
    }
    std::fs::write(&args[4], ppm).expect("write screenshot");

    // The level's floor and walls, as in examples/city_map.rs.
    let width = (m.mem.r16(DS, 0xC357) as usize).min(128);
    let depth = (m.mem.r16(DS, 0xC359) as usize).min(128);
    let (floor, walls) = (m.mem.r16(DS, 0xCCD0), m.mem.r16(DS, 0xD806));
    let mut ppm = format!("P6\n{} {}\n255\n", width * SCALE, depth * SCALE).into_bytes();
    for y in 0..depth * SCALE {
        for x in 0..width * SCALE {
            let off = ((y / SCALE) * 0x100 + (x / SCALE) * 2) as u16;
            let wall = m.mem.r16(walls, off);
            let rgb = if wall != 0 {
                shade(wall >> 8, 60, 150)
            } else {
                shade(m.mem.r16(floor, off).wrapping_sub(buffer) >> 8, 170, 250)
            };
            ppm.extend_from_slice(&rgb);
        }
    }
    std::fs::write(&args[5], ppm).expect("write map");
    println!("screenshot -> {}, {width} x {depth} map -> {}", args[4], args[5]);
}

/// A colour for a type number, with channels between lo and hi.
fn shade(t: u16, lo: u8, hi: u8) -> [u8; 3] {
    let h = (t as u32).wrapping_mul(2_654_435_761);
    let span = (hi - lo) as u32;
    [0, 8, 16].map(|s| lo + ((h >> s) % (span + 1)) as u8)
}
