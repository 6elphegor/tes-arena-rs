//! Decodes the level's flats and writes them side by side to a PNG, to
//! check the decoders by eye: arena-view's `flats` example GAME LEVEL OUT.
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let e = world::load(dir, &a[2]);
    let pal = world::palette(dir);
    let fl = world::flats(&e.m);
    let all: Vec<&Vec<u8>> = fl.iter().flatten().flat_map(|f| f.frames.iter()).collect();
    let dims: Vec<(usize, usize)> = fl.iter().flatten().flat_map(|f| f.frames.iter().map(|_| (f.w, f.h))).collect();
    let (w, h) = (dims.iter().map(|d| d.0 + 2).sum::<usize>(), dims.iter().map(|d| d.1).max().unwrap_or(1));
    let mut img = vec![40u8; w * h * 3];
    let mut x0 = 0;
    for (f, &(fw, fh)) in all.iter().zip(&dims) {
        for y in 0..fh {
            for x in 0..fw {
                let i = f[y * fw + x];
                if i != 0 {
                    let o = (y * w + x0 + x) * 3;
                    img[o..o + 3].copy_from_slice(&pal[i as usize][..3]);
                }
            }
        }
        x0 += fw + 2;
    }
    for (n, f) in fl.iter().enumerate() {
        if let Some(f) = f { println!("{n:2x}: {} x {}, {} frames", f.w, f.h, f.frames.len()); }
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&a[3]).unwrap()), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&img).unwrap();
}
