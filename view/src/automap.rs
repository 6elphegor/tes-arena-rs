//! The automap screen as the original draws it (0000:2CFE, the map's
//! button with the left button, 2542:045A; the drawing 0000:2F10).
//!
//! AUTOMAP.IMG in its own colours, then the place's name in FONT_A
//! centred on (79h, 17h) in 18h over a shadow in 74h at (7Bh, 19h) (not
//! in a dungeon, 5E6 3). Then 60 x 42 squares of 3 x 3 pixels from (25,
//! 40): the square under column k of a row is (left + 3Bh - k): the map
//! is drawn with x falling to the right. Each square's colour (303C; in
//! the wilderness 3161) comes from the walls and floors planes and from
//! how well it was seen (the automap's bytes, EMS bank 11h: 0, 7, 0Eh,
//! 15h), the player's square an arrow by the heading (23F1:0219). In a
//! city, a dungeon or an interior a second pass (23F1:02D1) blends each
//! square's pixels with its neighbours'. The notes (0000:32FE) aren't
//! drawn.
//!
//! The screen's centre is the game's (src/automap.rs: 126C/126E, made by
//! `automap_view_open` and the buttons).

use crate::hud;
use arena::machine::Machine;
use arena::mem::DS;

const LEFT: u16 = arena::automap::var::MAP_LEFT;
const TOP: u16 = arena::automap::var::MAP_TOP;

/// The five buttons (66CA: x0, y0, x1, y1) and the exit (2E30).
pub fn button_at(x: i32, y: i32) -> Option<u16> {
    const B: [(i32, i32, i32, i32); 5] = [(263, 24, 281, 38), (262, 58, 280, 72), (282, 42, 300, 56), (242, 41, 260, 55), (260, 41, 282, 56)];
    B.iter().position(|&(x0, y0, x1, y1)| x >= x0 && x <= x1 && y >= y0 && y <= y1).map(|i| i as u16)
}

/// Whether (x, y) is on the exit (2E30: F0h..120h, A4h..BBh).
pub fn exit_at(x: i32, y: i32) -> bool {
    (0xF0..=0x120).contains(&x) && (0xA4..=0xBB).contains(&y)
}

/// 0000:3046..3160 (303C after its bounds): a square's colour in a city,
/// a dungeon or an interior; None where nothing is drawn (the carry).
fn colour(m: &Machine, seen: &[u8], seen_at: usize, col: u16, row: u16) -> Option<u8> {
    let (w, d) = (m.mem.r16(DS, 0xC357), m.mem.r16(DS, 0xC359));
    if (col as i16) < 0 || (row as i16) < 0 || col >= w || row >= d {
        return None;
    }
    if col == m.mem.r16(DS, 0xCCA4) && row == m.mem.r16(DS, 0xCCA6) {
        return Some(0x96);
    }
    let bx = (row << 8) | ((col as u8).wrapping_mul(2) as u16);
    let cl = seen[(seen_at + (bx >> 1) as usize) & 0xFFFF];
    if cl == 0 {
        return None;
    }
    let dh = 0x15u8.wrapping_sub(cl);
    // A colour shaded by how well the square was seen, while it stays
    // in its block of 64 (signed compare, as 30E8..30FB).
    let shade = |c: u8| {
        let top = c & 0xC0;
        let a = c.wrapping_sub(dh);
        ((a as i8) >= (top as i8)).then_some(a)
    };
    let walls = m.mem.r16(DS, 0xD806);
    let v = m.mem.r16(walls, bx);
    let positive = (v as i16) > 0;
    let [lo, hi] = v.to_le_bytes();
    if (v as i16) >= 0 && hi != lo {
        return shade(0xF4);
    }
    if (v as i16) < 0 {
        match v >> 12 {
            8 => {}
            9 if hi & 1 != 0 => return None,
            9 | 0xA => return Some(0x3F - (cl >> 1)),
            0xB => return Some(0x4A),
            _ => return None,
        }
    }
    // 30D3: the floor plane.
    let floors = m.mem.r16(DS, 0xCCD0);
    let f = m.mem.r16(floors, bx);
    let solid = m.mem.r16(DS, 0xCCCE);
    if f < solid {
        if !positive {
            return None;
        }
        // 3825: stairs and the trigger walls.
        let al = (v & 0x3F) as u8;
        if al == m.mem.r8(DS, 0xC933) {
            return Some(0x97);
        }
        if al == m.mem.r8(DS, 0xC932) {
            return Some(0x95);
        }
        if (0..0x10).any(|i| m.mem.r8(DS, 0xC917 + i) == al) {
            return Some(0x4A);
        }
        return Some(0x3F - (cl >> 1));
    }
    if f == solid {
        return shade(if positive { 0xF4 } else { 0xD4 });
    }
    shade(if f == solid.wrapping_add(0x100) { 0xB4 } else { 0x94 })
}

/// 0000:3161: a square's colour in the wilderness (all of it shown).
fn wild_colour(m: &Machine, col: u16, row: u16) -> Option<u8> {
    let (w, d) = (m.mem.r16(DS, 0xC357), m.mem.r16(DS, 0xC359));
    if (col as i16) < 0 || (row as i16) < 0 || col >= w || row >= d {
        return None;
    }
    if col == m.mem.r16(DS, 0xCCA4) && row == m.mem.r16(DS, 0xCCA6) {
        return Some(0x96);
    }
    let bx = (row << 8) | ((col as u8).wrapping_mul(2) as u16);
    let walls = m.mem.r16(DS, 0xD806);
    let v = m.mem.r16(walls, bx);
    if (v as i16) > 0 {
        let al = (v & 0x3F) as u8;
        let water = [0xC918, 0xC91C, 0xC91F, 0xC920].iter().any(|&a| m.mem.r8(DS, a) == al);
        return Some(if water { 0x94 } else { 0x30 });
    }
    if (v as i16) < 0 && v >> 12 == 8 && (v & 0x3F) as u8 == m.mem.r8(DS, 0xC943) {
        return Some(0x94);
    }
    let floors = m.mem.r16(DS, 0xCCD0);
    let f = m.mem.r16(floors, bx);
    if f == m.mem.r16(DS, 0x9030).wrapping_add(0x100) {
        return Some(0x30);
    }
    (f >= m.mem.r16(DS, 0xCCCE)).then_some(0xB4)
}

/// 23F1:0219: the player's arrow, the heading's eighth `dir`, colour 96h,
/// at the square whose top left is `di`.
fn arrow(p: &mut [u8], di: usize, dir: u16) {
    let at: &[usize] = match dir {
        0 => &[0x140, 0x281, 0x142],
        1 => &[0, 0x140, 0x280, 0x281, 0x282],
        2 => &[1, 0x140, 0x281],
        3 => &[1, 2, 0, 0x140, 0x280],
        4 => &[1, 0x140, 0x142],
        5 => &[0, 1, 2, 0x282, 0x142],
        6 => &[1, 0x142, 0x281],
        _ => &[2, 0x142, 0x280, 0x281, 0x282],
    };
    for &o in at {
        if let Some(px) = p.get_mut(di + o) {
            *px = 0x96;
        }
    }
}

/// 23F1:01E1: the pixel at `*di` from the square's colour `dh` and the
/// neighbour `dl`: their mean where the neighbour is 30h..73h, not the
/// player's and of the same colour block (the top three bits), else the
/// square's own; `di` moves on. Returns the carry (not blended).
fn blend(p: &mut [u8], di: &mut usize, dh: u8, dl: u8) -> bool {
    let near = (dl as i8) >= 0x30 && dl != 0x96 && (dl.wrapping_sub(0xF4) as i8) >= 0;
    let (v, carry) = if near && (dh >> 5) == (dl >> 5) { (dl.wrapping_add(dh) >> 1, false) } else { (dh, true) };
    if let Some(px) = p.get_mut(*di) {
        *px = v;
    }
    *di += 1;
    carry
}

/// 23F1:02D1: a square's 3 x 3 pixels blended with its neighbours',
/// in the original's order (each read may see a pixel just written).
fn smooth(p: &mut [u8], top_left: usize, al: u8) {
    let dh = al;
    // One 23F1:01E1 with the neighbour at `di` + `off`.
    let step = |p: &mut [u8], di: &mut usize, off: isize| {
        let dl = *p.get((*di as isize + off) as usize).unwrap_or(&0);
        blend(p, di, dh, dl)
    };
    let mut di = top_left;
    // First row.
    if step(p, &mut di, 0x13D) {
        di -= 1;
        step(p, &mut di, -0x27F);
    }
    step(p, &mut di, -0x27F);
    if step(p, &mut di, 0x144) {
        di -= 1;
        step(p, &mut di, 0x501);
    }
    // Second row: the middle pixel is the square's own.
    di = top_left + 0x140;
    step(p, &mut di, 0x13D);
    if let Some(px) = p.get_mut(di) {
        *px = al;
    }
    di += 1;
    step(p, &mut di, 0x144);
    // Third row.
    di = top_left + 0x280;
    if step(p, &mut di, 0x13D) {
        di -= 1;
        step(p, &mut di, 0x501);
    }
    step(p, &mut di, 0x501);
    if step(p, &mut di, 0x144) {
        di -= 1;
        step(p, &mut di, 0x501);
    }
}

/// The automap screen over the layer (call `automap_view_open` when it
/// opens; the buttons change its centre).
pub fn draw(screen: &mut hud::Screen, m: &Machine) {
    let Some(pics) = crate::sheets::loaded() else { return };
    let mut page = vec![0u8; 320 * 200];
    let mut pal = [[0u8; 4]; 256];
    if let Some((img, p)) = &pics.automap {
        pal = *p;
        for r in 0..img.h.min(200) {
            for c in 0..img.w.min(320) {
                page[r * 320 + c] = img.px[r * img.w + c];
            }
        }
    }
    let outdoors = m.mem.r16(DS, 0xC398) & 4 != 0;
    let wild = outdoors && m.mem.r16(DS, 0xC39A) & 0x200 != 0;
    let seen = m.ems_bank(arena::automap::var::BANK);
    let seen_at = m.mem.r16(DS, arena::automap::var::FRAME) as usize;
    let (left, top) = (m.mem.r16(DS, LEFT), m.mem.r16(DS, TOP));
    let dir = (m.mem.r16(DS, 0xC3DA).wrapping_add(0x20) & 0x1FF) >> 6;
    let passes = if wild { 1 } else { 2 };
    for pass in 0..passes {
        for r in 0..42u16 {
            for k in 0..60u16 {
                let col = left.wrapping_add(0x3B).wrapping_sub(k);
                let row = top.wrapping_add(r);
                let di = 0x3219 + r as usize * 0x3C0 + 3 * k as usize;
                let c = if wild { wild_colour(m, col, row) } else { colour(m, &seen, seen_at, col, row) };
                let Some(c) = c else { continue };
                if c == 0x96 {
                    arrow(&mut page, di, dir);
                } else if pass == 0 || c == 0x4A || wild {
                    for y in 0..3 {
                        for x in 0..3 {
                            page[di + y * 320 + x] = c;
                        }
                    }
                } else {
                    smooth(&mut page, di, c);
                }
            }
        }
    }
    screen.set_palette(pal);
    screen.fill(0, 0, hud::W as i32, hud::H as i32, 0, 0xFF);
    // (Colour 0 is see-through to `blit`: the fill under it is colour 0.)
    screen.blit(0, 0, 320, 200, &page, 199);
    if m.mem.r8(DS, 0x05E6) != 3 {
        if let Some(f) = &pics.font_a {
            let name = String::from_utf8_lossy(m.mem.cstr(DS, 0x0A22)).into_owned();
            let half = f.width(&name) as i32 / 2;
            screen.text_plain(f, 0x7B - half, 0x19, &name, 0x74);
            screen.text_plain(f, 0x79 - half, 0x17, &name, 0x18);
        }
    }
}
