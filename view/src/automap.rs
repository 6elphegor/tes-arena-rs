//! The automap screen (M): the map the game keeps (src/automap.rs, its
//! EMS bank 11h) drawn over the view, one pixel a square, north up. The
//! original's own screen (0000:2F10, with its images and notes) isn't
//! what this is: seen squares are grey by how well they were seen (7h,
//! 0Eh, 15h), walls lighter than floors (the walls plane), the player red;
//! a level of 64 squares or less at two pixels a square.

use crate::hud;
use arena::machine::Machine;
use arena::mem::DS;

pub fn draw(screen: &mut hud::Screen, m: &Machine) {
    let cells = m.ems_bank(arena::automap::var::BANK);
    let off = m.mem.r16(DS, arena::automap::var::FRAME) as usize;
    let w = m.mem.r16(DS, 0xC357).min(128) as usize;
    let d = m.mem.r16(DS, 0xC359).min(128) as usize;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (x0, y0) = ((hud::W - 128) / 2, 8);
    let k = if w.max(d) <= 64 { 2 } else { 1 };
    screen.panel(x0 as i32 - 4, y0 as i32 - 4, 136, 136, 0x0F);
    let mut put = |x: usize, y: usize, c: [u8; 3]| {
        if x < hud::W && y < hud::H {
            screen.px[y * hud::W + x] = [c[0], c[1], c[2], 255];
        }
    };
    for row in 0..d {
        for col in 0..w {
            let v = cells[(off + row * 128 + col) & 0xFFFF];
            if v == 0 {
                continue;
            }
            let shade: u8 = match v {
                7 => 70,
                0xE => 120,
                _ => 170,
            };
            let word = m.mem.r16(walls, ((row << 8) | (col << 1)) as u16);
            // A square of flats (8xxxh) stands open.
            let wall = word != 0 && word >> 12 != 8;
            let c = if wall { [shade + 70, shade + 55, shade + 25] } else { [shade - 30, shade - 30, shade - 20] };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                if dx < k && dy < k {
                    put(x0 + col * k + dx, y0 + row * k + dy, c);
                }
            }
        }
    }
    let (px, py) = ((m.mem.r16(DS, 0xC3D0) >> 7) as usize, (m.mem.r16(DS, 0xC3D4) >> 7) as usize);
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        put(x0 + px * k + dx, y0 + py * k + dy, [230, 30, 30]);
    }
}
