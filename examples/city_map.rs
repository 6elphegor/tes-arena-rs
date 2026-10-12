//! Builds a city with the ported code and writes its map as an image.
//!
//! Usage: city_map GAME_DIR PROVINCE INDEX OUT.ppm
//!
//! GAME_DIR is the game directory (with ACD.EXE, GLOBAL.BSA and
//! CITYDATA.65). PROVINCE is 0..7, or 8 for the Imperial City; INDEX is
//! the location within the province (0..7 city-states, 8..15 towns,
//! 16..31 villages). Everything is read from the game files at run time.
//!
//! The image has one pixel per map square, scaled up: walls (MAP1) in
//! greys and colours by wall type, open floor (FLOR) in light tints by
//! floor type.

use arena::bsa::Files;
use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

/// The current location record and where the game loads a province's
/// block of CITYDATA (0x4CC bytes per province).
const RECORD: u16 = 0x0A22;
const PROVINCE_DATA: u16 = 0x1429;
const PROVINCE_BYTES: usize = 0x4CC;
const SCALE: usize = 4;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: city_map GAME_DIR PROVINCE INDEX OUT.ppm");
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
    // The heap where startup would put it (any free memory will do), and
    // a 64 KB block for the decompression buffer, as the game allocates.
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buf, _) = m.heap_alloc(0x1000).expect("buffer");
    m.mem.w16(DS, 0x9030, buf);
    m.mem.w32(DS, 0x91F6, 0x1234_5678);
    m.load_rng();

    // The province's location list, as 1194:2E18 loads it.
    let p = province.min(8) as usize;
    let data = std::fs::read(dir.join("CITYDATA.65")).expect("CITYDATA.65");
    for (i, &b) in data[p * PROVINCE_BYTES..(p + 1) * PROVINCE_BYTES].iter().enumerate() {
        m.mem.w8(DS, PROVINCE_DATA + i as u16, b);
    }
    let id = if province == 8 { 0x100 } else { province * 32 + index };
    m.city_build_location(DS, RECORD, id, 0).expect("location record");
    let name = String::from_utf8_lossy(m.mem.cstr(DS, RECORD)).into_owned();
    let template = String::from_utf8_lossy(m.mem.cstr(DS, RECORD + 0x14)).into_owned();

    m.mif_build_city(&mut files).expect("city build");

    let width = m.mem.r16(DS, 0xC357) as usize;
    let depth = m.mem.r16(DS, 0xC359) as usize;
    let floor = m.mem.r16(DS, 0xCCD0);
    let walls = m.mem.r16(DS, 0xD806);
    let (w, h) = (width.min(128), depth.min(128));
    let mut px = Vec::with_capacity(w * h * SCALE * SCALE * 3);
    for y in 0..h * SCALE {
        for x in 0..w * SCALE {
            let off = ((y / SCALE) * 0x100 + (x / SCALE) * 2) as u16;
            let wall = m.mem.r16(walls, off);
            let rgb = if wall != 0 {
                shade(wall >> 8, 60, 150)
            } else {
                shade(m.mem.r16(floor, off).wrapping_sub(buf) >> 8, 170, 250)
            };
            px.extend_from_slice(&rgb);
        }
    }
    let mut file = format!("P6\n{} {}\n255\n", w * SCALE, h * SCALE).into_bytes();
    file.extend_from_slice(&px);
    std::fs::write(out, file).expect("write image");
    println!("{name} (location {id:#x}, template {template}): {width} x {depth} squares -> {out}");
}

/// A colour for a type number, with channels between lo and hi.
fn shade(t: u16, lo: u8, hi: u8) -> [u8; 3] {
    let h = (t as u32).wrapping_mul(2_654_435_761);
    let span = (hi - lo) as u32;
    [0, 8, 16].map(|s| lo + ((h >> s) % (span + 1)) as u8)
}
