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
//! Not drawn: the colours the original remaps an item's picture through
//! (by its +10, 1194:1FFB, 20BF) and a shield's by race (1194:2020), and
//! the face over the doll (1194:2087).

use crate::interface::Image;
use arena::machine::Machine;
use arena::mem::DS;

/// The player's record and the item slots (40 of 13h bytes).
const PLAYER: u16 = 0x01A4;
const SLOTS: u16 = arena::inventory::PLAYER_SLOTS;
const SLOT: u16 = arena::inventory::SLOT_SIZE;
/// The magic items' doll types.
const MAGIC_TYPES: u16 = 0x36DD;

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
    let piece = |si: u16, out: &mut Vec<Image>| {
        if let Some(img) = frame(si).and_then(|n| cif.get(n)) {
            if img.w > 0 {
                out.push(img.clone());
            }
        }
    };
    match worn.iter().find(|&&si| cuirass(si)) {
        Some(&si) => piece(si, &mut out),
        None => out.extend(crate::panels::load_image(archive, m, &format!("{sex}{}SHIRT.IMG", if robe { 'R' } else { 'S' }))),
    }
    for &si in worn.iter().filter(|&&si| !cuirass(si)) {
        piece(si, &mut out);
    }
    out
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
