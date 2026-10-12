//! Builds a dungeon with the ported code and writes its levels as an
//! image, side by side.
//!
//! Usage: dungeon_map GAME_DIR PROVINCE INDEX OUT.ppm
//!
//! GAME_DIR is the game directory (with ACD.EXE, GLOBAL.BSA and
//! CITYDATA.65). PROVINCE is 0..7; INDEX is the location within the
//! province's list (dungeons are 32..47). The dungeon is entered as a
//! randomly generated one, from its seed, even where the game has a fixed
//! map for it. Everything is read from the game files at run time.
//!
//! Walls in greys and colours by type, open floor in light tints, stairs
//! up in white and down in black.

use arena::bsa::Files;
use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const PROVINCE_DATA: u16 = 0x1429;
const LOCATIONS: u16 = 0x1445;
const PROVINCE_BYTES: usize = 0x4CC;
const SCALE: usize = 3;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: dungeon_map GAME_DIR PROVINCE INDEX OUT.ppm");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let province: u16 = args[2].parse().expect("province");
    let index: u16 = args[3].parse().expect("index");
    let out = &args[4];

    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(&dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buf, _) = m.heap_alloc(0x1000).expect("buffer");
    m.mem.w16(DS, 0x9030, buf);
    let (bs, bo) = m.heap_alloc_bytes(0x3E80).expect("chunk block");
    m.mem.w16(DS, 0xBA54, bo);
    m.mem.w16(DS, 0xBA56, bs);

    // The province's location list, and the entry as the travel screen
    // leaves it for 1194:3B0C: name, position and province.
    let data = std::fs::read(dir.join("CITYDATA.65")).expect("CITYDATA.65");
    let p = province as usize;
    for (i, &b) in data[p * PROVINCE_BYTES..(p + 1) * PROVINCE_BYTES].iter().enumerate() {
        m.mem.w8(DS, PROVINCE_DATA + i as u16, b);
    }
    let entry = LOCATIONS + index * 25;
    for i in 0..20 {
        let c = m.mem.r8(DS, entry + i);
        m.mem.w8(DS, 0x1929 + i, c);
    }
    m.mem.w16(DS, 0x193E, m.mem.r16(DS, entry + 0x14));
    m.mem.w16(DS, 0x1940, m.mem.r16(DS, entry + 0x16));
    m.mem.w8(DS, 0x47E1, province as u8);
    m.dungeon_build_location(index as u8, 0).expect("location record");
    let name = String::from_utf8_lossy(m.mem.cstr(DS, 0x0A22)).into_owned();
    let file = String::from_utf8_lossy(m.mem.cstr(DS, 0x0A36)).into_owned();

    m.dungeon_enter_load(&mut files).expect("enter");
    let levels = m.mem.r16(DS, 0xC355) as usize;
    let width = m.mem.r16(DS, 0xC357) as usize;
    let depth = m.mem.r16(DS, 0xC359) as usize;
    let (w, h) = (width.min(128), depth.min(128));
    let gap = 4;
    let total_w = levels * (w * SCALE + gap);
    let mut px = vec![40u8; total_w * h * SCALE * 3];
    for level in 0..levels {
        m.dungeon_build_level(&mut files, level as u8).expect("level");
        let floor = m.mem.r16(DS, 0xCCD0);
        let map = m.mem.r16(DS, 0xD806);
        for y in 0..h * SCALE {
            for x in 0..w * SCALE {
                let off = ((y / SCALE) * 0x100 + (x / SCALE) * 2) as u16;
                let wall = m.mem.r16(map, off);
                let rgb = match wall as u8 {
                    0x1D => [255, 255, 255],
                    0x1C => [0, 0, 0],
                    _ if wall != 0 => shade(wall >> 8, 60, 150),
                    _ => shade(m.mem.r16(floor, off).wrapping_sub(buf) >> 8, 170, 250),
                };
                let i = (y * total_w + level * (w * SCALE + gap) + x) * 3;
                px[i..i + 3].copy_from_slice(&rgb);
            }
        }
    }
    let mut ppm = format!("P6\n{} {}\n255\n", total_w, h * SCALE).into_bytes();
    ppm.extend_from_slice(&px);
    std::fs::write(out, ppm).expect("write image");
    println!("{name} ({file}): {levels} levels of {width} x {depth} squares -> {out}");
}

/// A colour for a type number, with channels between lo and hi.
fn shade(t: u16, lo: u8, hi: u8) -> [u8; 3] {
    let h = (t as u32).wrapping_mul(2_654_435_761);
    let span = (hi - lo) as u32;
    [0, 8, 16].map(|s| lo + ((h >> s) % (span + 1)) as u8)
}
