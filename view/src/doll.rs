//! The paper doll of the equipment screen (1194:23B1's pictures): the
//! game's own images, loaded at run time, in the order the original draws
//! them, each at its header's place on the 320 x 200 screen.
//!
//! - The page with the body (1194:161A): CHARBK00.IMG for a man,
//!   CHRBKF00.IMG for a woman (the player's +7), the race (+4) written
//!   over the digits before the dot (0000:7024).
//! - The head (1194:15DC, 2D60): the player's face (+6Bh) from
//!   FACES1r.CIF, FACESF1r.CIF for a woman, r the race (1194:2DAF: '1'
//!   with the sheet up, 12BA), at the frame's place.
//! - The pants (1194:2422: [M|F]PANTS.IMG by the sex, 1194:1D3B).
//! - The chest (1194:2187): the cuirass worn, else the shirt,
//!   [M|F][S|R]SHIRT.IMG (R for a class with bit 20h of +5, 1194:1D30).
//! - Every other item worn (slot +E bit 80h; 1194:242D, 2451 leaving out
//!   the cuirass): a frame of [0|1]EQUIP.CIF by the sex (1194:2463), by
//!   its kind ((+E & 0Ch) >> 2, 1194:748E): 0, armour, its type plus 12h,
//!   1Dh or 24h by its material ((+E & 30h) >> 4, 1194:21A8); 1 its type;
//!   2 none (1194:1F8D). A magic item's type (bit 80h) is the table's at
//!   DGROUP 36DD (1194:7596).
//!
//! An item with +E bit 0 is drawn through its material's colours
//! (1194:1F90 with 1FFB, 20BF: its signed +10 picks a start colour at
//! DGROUP 3EA0; the picture's colours EEh..FAh become that start and the
//! twelve after it, or for material 2 EEh..F4h the start's seven and
//! F5h..F9h 80h..84h). A helmet worn over some races' heads (1194:2020):
//! races 1, 3 and 4 have 30 colours of a square around the head remapped
//! (DATA 5750's indices to the race's list at DATA 576E; the square
//! F4h,13h 13h x 17h, a woman's ECh,12h Fh x Fh, through 0000:9D84); race 7
//! has the lizard's head MLIZn.IMG drawn, n the helmet's material.
//!
//! The doll is composed here in palette indices, so the square's remap
//! applies to what is drawn under it, as on the original's screen.

use crate::interface::Image;
use arena::machine::Machine;
use arena::mem::DS;

/// The player's record and the item slots (40 of 13h bytes).
const PLAYER: u16 = 0x01A4;
const SLOTS: u16 = arena::inventory::PLAYER_SLOTS;
const SLOT: u16 = arena::inventory::SLOT_SIZE;
/// The magic items' doll types.
const MAGIC_TYPES: u16 = 0x36DD;
/// The materials' first colours (by an item's signed +10).
const MATERIAL_BASE: u16 = 0x3EA0;
/// DATA: the 30 colours a helmet's head remaps, and each race's list of
/// their new colours (a word per race; 0 none).
const HEAD_COLOURS: u16 = 0x5750;
const RACE_HEADS: u16 = 0x576E;

/// The doll's images, bottom first.
pub fn images(archive: &crate::archive::Archive, m: &Machine) -> Vec<Image> {
    let woman = m.mem.r8(DS, PLAYER + 7) != 0;
    let race = m.mem.r8(DS, PLAYER + 4);
    let robe = m.mem.r8(DS, PLAYER + 5) & 0x20 != 0;
    let sex = if woman { 'F' } else { 'M' };
    let mut out = Vec::new();
    let page = if woman { format!("CHRBKF{race:02}.IMG") } else { format!("CHARBK{race:02}.IMG") };
    out.extend(crate::panels::load_image(archive, m, &page));
    let faces = if woman { format!("FACESF1{race}.CIF") } else { format!("FACES1{race}.CIF") };
    let face = m.mem.r8(DS, PLAYER + 0x6B) as usize;
    if let Some(img) = archive.read(&faces).and_then(|d| cif_frame(m, &d, face)) {
        out.push(img);
    }
    out.extend(crate::panels::load_image(archive, m, &format!("{sex}PANTS.IMG")));
    let cif = archive.read(if woman { "1EQUIP.CIF" } else { "0EQUIP.CIF" }).map(|d| crate::interface::cif(&d)).unwrap_or_default();
    let worn: Vec<u16> = (0..40).map(|i| SLOTS + i * SLOT).filter(|&si| m.mem.r8(DS, si + 0xE) & 0xC0 == 0xC0).collect();
    let kind = |si: u16| (m.mem.r8(DS, si + 0xE) & 0xC) >> 2;
    let cuirass = |si: u16| m.mem.r8(DS, si) == 2 && kind(si) == 0;
    let frame = |si: u16| -> Option<usize> {
        let mut t = m.mem.r8(DS, si) as i32;
        if t & 0x80 != 0 {
            t = m.mem.r8(DS, MAGIC_TYPES + (t & 0x7F) as u16) as i8 as i32;
        }
        let n = match kind(si) {
            0 => t + [0x12, 0x1D, 0x24, 0x24][((m.mem.r8(DS, si + 0xE) & 0x30) >> 4) as usize],
            1 => t,
            _ => return None,
        };
        usize::try_from(n).ok()
    };
    let mut doll = Doll::new();
    for img in &out {
        doll.blit(img, None);
    }
    let piece = |si: u16, doll: &mut Doll| {
        if let Some(img) = frame(si).and_then(|n| cif.get(n)) {
            if img.w > 0 {
                doll.blit(img, material(m, si).as_ref());
            }
        }
        // 1194:1FE6: a helmet (the slot's type 5, armour) over the head.
        if m.mem.r8(DS, si) == 5 && kind(si) == 0 {
            helmet_head(archive, m, si, doll);
        }
    };
    match worn.iter().find(|&&si| cuirass(si)) {
        Some(&si) => piece(si, &mut doll),
        None => {
            if let Some(img) = crate::panels::load_image(archive, m, &format!("{sex}{}SHIRT.IMG", if robe { 'R' } else { 'S' })) {
                doll.blit(&img, None);
            }
        }
    }
    for &si in worn.iter().filter(|&&si| !cuirass(si)) {
        piece(si, &mut doll);
    }
    vec![doll.image()]
}

/// The doll composed on a 320 x 200 screen of palette indices (0: not
/// drawn).
struct Doll {
    px: Vec<u8>,
}

impl Doll {
    fn new() -> Doll {
        Doll { px: vec![0; 320 * 200] }
    }

    /// `img` at its place, colour 0 see-through, through `map` if any.
    fn blit(&mut self, img: &Image, map: Option<&[u8; 256]>) {
        for row in 0..img.h {
            for col in 0..img.w {
                let c = img.px[row * img.w + col];
                if c == 0 {
                    continue;
                }
                let (x, y) = (img.x + col as i32, img.y + row as i32);
                if (0..320).contains(&x) && (0..200).contains(&y) {
                    self.px[y as usize * 320 + x as usize] = map.map_or(c, |t| t[c as usize]);
                }
            }
        }
    }

    /// 0000:9D84: the square at (`x`, `y`), `w` x `h`, through `map`.
    fn remap(&mut self, x: usize, y: usize, w: usize, h: usize, map: &[u8; 256]) {
        for row in y..(y + h).min(200) {
            for col in x..(x + w).min(320) {
                let at = row * 320 + col;
                self.px[at] = map[self.px[at] as usize];
            }
        }
    }

    fn image(self) -> Image {
        Image { x: 0, y: 0, w: 320, h: 200, px: self.px }
    }
}

/// The colours an item's picture is drawn through (1194:1F90, 20BF): by
/// its material when +E bit 0 is set (its signed +10; -1 none).
fn material(m: &Machine, si: u16) -> Option<[u8; 256]> {
    if m.mem.r8(DS, si + 0xE) & 1 == 0 {
        return None;
    }
    let k = m.mem.r8(DS, si + 0x10) as i8 as i16 as u16;
    if k == 0xFFFF {
        return None;
    }
    let base = m.mem.r8(DS, k.wrapping_add(MATERIAL_BASE));
    let mut t: [u8; 256] = std::array::from_fn(|i| i as u8);
    let n = if k == 2 { 7 } else { 13 };
    for i in 0..n {
        t[0xEE + i] = base.wrapping_add(i as u8);
    }
    if k == 2 {
        for i in 0..5 {
            t[0xEE + 7 + i] = 0x80 + i as u8;
        }
    }
    Some(t)
}

/// 1194:2020: the head under a helmet by the player's race (+4).
fn helmet_head(archive: &crate::archive::Archive, m: &Machine, si: u16, doll: &mut Doll) {
    let race = m.mem.r8(DS, PLAYER + 4) as u16;
    match race {
        1 | 3 | 4 => {
            let list = m.mem.r16(arena::mem::DATA, RACE_HEADS + 2 * race);
            let mut t: [u8; 256] = std::array::from_fn(|i| i as u8);
            for i in 0..30u16 {
                let k = m.mem.r8(arena::mem::DATA, HEAD_COLOURS + i);
                t[k as usize] = m.mem.r8(arena::mem::DATA, list.wrapping_add(i));
            }
            let (x, y, w, h) = if m.mem.r8(DS, PLAYER + 7) != 0 { (0xEC, 0x12, 0xF, 0xF) } else { (0xF4, 0x13, 0x13, 0x17) };
            doll.remap(x, y, w, h, &t);
        }
        7 => {
            let name = format!("MLIZ{}.IMG", (m.mem.r8(DS, si + 0xE) & 0x30) >> 4);
            if let Some(img) = crate::panels::load_image(archive, m, &name) {
                doll.blit(&img, None);
            }
        }
        _ => {}
    }
}

/// Frame `k` of a .CIF (its images one after another, each 12 bytes of
/// header then +0Ah bytes, 0000:3C94), decoded by its coding: type 8
/// (LZHUF, the heads) through the port's decompressor.
fn cif_frame(m: &Machine, d: &[u8], k: usize) -> Option<Image> {
    let mut at = 0usize;
    for _ in 0..k {
        let size = u16::from_le_bytes([*d.get(at + 10)?, *d.get(at + 11)?]) as usize;
        at += 12 + size;
    }
    let frame = d.get(at..)?;
    crate::panels::image_type8(m, frame).or_else(|| crate::interface::image(frame).map(|(img, _)| img))
}
