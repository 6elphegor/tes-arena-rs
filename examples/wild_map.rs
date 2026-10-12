//! Generates the wilderness around a location with the ported code and
//! writes its 64 x 64 block grid as an image (one colour per block
//! number, the location's own blocks 1..4 in white).
//!
//! Usage: wild_map GAME_DIR PROVINCE INDEX OUT.ppm   (as for city_map)

use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use arena::wild::{GRID, GRID_SIZE};
use std::path::PathBuf;

const SCALE: usize = 6;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: wild_map GAME_DIR PROVINCE INDEX OUT.ppm");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let province: u16 = args[2].parse().expect("province");
    let index: u16 = args[3].parse().expect("index");
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    m.mem.w16(DS, 0x9030, 0x7000);
    let data = std::fs::read(dir.join("CITYDATA.65")).expect("CITYDATA.65");
    let p = province.min(8) as usize;
    for (i, &b) in data[p * 0x4CC..(p + 1) * 0x4CC].iter().enumerate() {
        m.mem.w8(DS, 0x1429 + i as u16, b);
    }
    let id = if province == 8 { 0x100 } else { province * 32 + index };
    m.city_build_location(DS, 0x0A22, id, 0).expect("location record");
    m.wild_generate().expect("wilderness");
    let name = String::from_utf8_lossy(m.mem.cstr(DS, 0x0A22)).into_owned();

    let n = GRID_SIZE as usize;
    let mut ppm = format!("P6\n{} {}\n255\n", n * SCALE, n * SCALE).into_bytes();
    for y in 0..n * SCALE {
        for x in 0..n * SCALE {
            let b = m.mem.r8(0x7000, GRID + ((y / SCALE) * n + x / SCALE) as u16);
            let rgb = if (1..=4).contains(&b) {
                [255, 255, 255]
            } else {
                let h = (b as u32).wrapping_mul(2_654_435_761);
                [0, 8, 16].map(|s| 40 + ((h >> s) % 180) as u8)
            };
            ppm.extend_from_slice(&rgb);
        }
    }
    std::fs::write(&args[4], ppm).expect("write");
    println!("wilderness of {name} -> {}", args[4]);
}
