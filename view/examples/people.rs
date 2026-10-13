//! Draws a person's attack frames with the weapon over them, placed as
//! `world::sprites` places it (0000:1B7F's offsets), to a PNG: each frame
//! on a grey cell, scaled 3x.
//!
//! Usage: people GAME_DIR ATTACK.CFA WEAPON.CFA OUT.png
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let ar = archive::Archive::open(dir);
    let pal = world::palette(dir);
    let (ad, wd) = (ar.read(&a[2]).expect("attack"), ar.read(&a[3]).expect("weapon"));
    let (att, wpn) = (flats::cfa(&ad).expect("attack cfa"), flats::cfa(&wd).expect("weapon cfa"));
    let hd = |d: &[u8], o: usize| i16::from_le_bytes([d[o], d[o + 1]]) as i32;
    let z = (hd(&wd, 8) - hd(&ad, 8)) + (hd(&wd, 2) - hd(&ad, 2)) + 0xE;
    let shift = (hd(&wd, 6) - hd(&ad, 6)) + 4;
    println!("weapon lifted {} and shifted {shift}", -z);
    let (cw, ch) = (200usize, 220usize);
    let n = att.frames.len();
    let mut img = vec![[90u8, 90, 90]; cw * n * ch];
    for k in 0..n {
        let mut put = |f: &flats::Flat, frame: usize, left: i32, bottom: i32| {
            let src = &f.frames[frame % f.frames.len()];
            for y in 0..f.h {
                for x in 0..f.w {
                    let c = src[y * f.w + x];
                    let (px, py) = (left + x as i32, bottom - f.h as i32 + y as i32);
                    if c != 0 && (0..cw as i32).contains(&px) && (0..ch as i32).contains(&py) {
                        let p = pal[c as usize];
                        img[py as usize * cw * n + k * cw + px as usize] = [p[0], p[1], p[2]];
                    }
                }
            }
        };
        let (cx, base) = (cw as i32 / 2, ch as i32 - 10);
        put(&att, k, cx - att.w as i32 / 2, base);
        put(&wpn, k, cx + shift - wpn.w as i32 / 2, base + z);
    }
    let (w, h) = (cw * n * 3, ch * 3);
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        for x in 0..w {
            out.extend_from_slice(&img[(y / 3) * cw * n + x / 3]);
        }
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&a[4]).unwrap()), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&out).unwrap();
}
