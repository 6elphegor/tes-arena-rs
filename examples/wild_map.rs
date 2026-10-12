//! Generates the wilderness around a location with the ported code and
//! writes its 64 x 64 block grid as an image (one colour per block
//! number, the location's own blocks 1..4 in white).
//!
//! Usage: wild_map GAME_DIR PROVINCE INDEX GRID.ppm TERRAIN.ppm
//!
//! TERRAIN.ppm is the 128 x 128 map of the four blocks just north-west
//! of the location, loaded as the game loads them.

use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use arena::wild::{GRID, GRID_SIZE};
use std::path::PathBuf;

const SCALE: usize = 6;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        eprintln!("usage: wild_map GAME_DIR PROVINCE INDEX GRID.ppm TERRAIN.ppm");
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

    // Four blocks (rows 29..30, columns 29..30; the location is at 31..32)
    // loaded into a 128 x 128 map.
    let mut files = arena::bsa::Files::new(&dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let grid_seg = 0x7000;
    let (buf, _) = m.heap_alloc(0x1000).expect("heap");
    // The grid stays where it was generated; blocks unpack into `buf`.
    let blocks: Vec<u8> = [(29, 29), (29, 30), (30, 29), (30, 30)]
        .iter()
        .map(|&(r, c)| m.mem.r8(grid_seg, GRID + r * 64 + c))
        .collect();
    m.mem.w16(DS, 0x9030, buf);
    for var in [0xCCD0u16, 0xD806, 0xD748] {
        let (seg, _) = m.heap_alloc(0x800).expect("heap");
        m.mem.w16(DS, var, seg);
    }
    let (seg, off) = m.heap_alloc_bytes(0x8000).expect("heap");
    m.mem.w16(DS, 0xBA54, off);
    m.mem.w16(DS, 0xBA56, seg);
    for (slot, &b) in blocks.iter().enumerate() {
        m.wild_load_block(&mut files, slot as u8, b).expect("block");
    }
    let (floor, walls) = (m.mem.r16(DS, 0xCCD0), m.mem.r16(DS, 0xD806));
    let mut ppm = format!("P6\n{0} {0}\n255\n", 128 * 4).into_bytes();
    for y in 0..128 * 4 {
        for x in 0..128 * 4 {
            let off = ((y / 4) * 0x100 + (x / 4) * 2) as u16;
            let wall = m.mem.r16(walls, off);
            let t = if wall != 0 { wall >> 8 } else { m.mem.r16(floor, off).wrapping_sub(buf) >> 8 };
            let h = (t as u32).wrapping_mul(2_654_435_761);
            let (lo, span) = if wall != 0 { (60, 90) } else { (170, 80) };
            ppm.extend_from_slice(&[0, 8, 16].map(|s| lo + ((h >> s) % span) as u8));
        }
    }
    std::fs::write(&args[5], ppm).expect("write");
    println!("wilderness of {name} -> {}, blocks {blocks:?} -> {}", args[4], args[5]);
}
