//! The player's interface over the view, from the game's own graphics
//! (read from its files at run time), placed as the original places
//! them:
//!
//! - the bottom bar P1.IMG (no header: 320 x 53 from row 147, as the
//!   interface's set-up 0000:4DD6 loads it to screen offset B7C0h);
//! - the condition picture STATUS.CIF (0000:4E81: image 0, or 4 when
//!   2686:0270 says so, 1 or 2 by the record's +40Dh flags, 3 with health
//!   at most a tenth of its maximum; at its header's place, 14, 166), the
//!   face over it (1194:2D5C: FACES0r.CIF, FACESF0r.CIF for a woman, r the
//!   race, image +6Bh, put at 14, 166) and the name's first word (1194:4BC8:
//!   colour F0h at 17, 154);
//! - the three bars (2901:00A8, 00E4): 4 x 26 from row 168 at x 57
//!   (health 1FD / 1FF, colour 1Ah), 67 (fatigue 201 / (strength +
//!   endurance) x 64, 9Fh) and 77 (spell points 20A / 20C, 3Eh), filled
//!   from the bottom by value x 26 / max (8.8 fixed point), black above;
//! - the compass (0000:4D7C, while 5C8 bit 2000h and not 1013): seven rows
//!   of 32 from SLIDER.IMG (289 x 7, no header) at ((heading + E0h) & 1FFh)
//!   / 2, put at (144, 7), and COMPASS.IMG's frame over it at its header's
//!   place;
//! - the weapon (0000:3B8E, called by the weapon's frame 38CE): image
//!   [1947] of the class's .CIF (the names at 674E by the shown class
//!   [1012]; the bare hands, classes 7..9, from frame 1Eh on 14h less), at
//!   its header's place, cut below row 93h; the bow (class 6) the first
//!   frame of ARROWHLF.CFA at its offsets while not reloading; the casting
//!   hands (29CF:0A28) frame [7F80] of the element's .CFA as the game
//!   loaded it into EMS bank [C8F4], at its offsets.
//!
//! The images' coding (0000:582C): flags bit 1 is run-length coding by
//! rows (0000:5B9A, as a DFA's), bit 2 LZSS (0000:7608: a 4 KB ring from
//! FEEh, filled with spaces before it, a flag byte for eight items, set a
//! literal, clear two bytes: position low 8 bits and high 4, length + 3);
//! the size at +0Ah is the coded data's. Colour 0 is see-through.
//!
//! What's drawn when follows the weapon's frame's paths: nothing while
//! casting (then the hands), with [620], or in water but levitating
//! (C39A bit 10h without 4000h); sheathed (5E0 bit 10h clear) only while
//! the frame is still 1Eh or more. The original draws before the frame
//! counts on, the view after: a frame later.

use crate::archive::Archive;
use crate::hud::Screen;
use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;
use std::rc::Rc;

/// An image with its place on the 320 x 200 screen (its header's x, y).
#[derive(Clone, Debug)]
pub struct Image {
    pub x: i32,
    pub y: i32,
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

fn le16(d: &[u8], at: usize) -> usize {
    *d.get(at).unwrap_or(&0) as usize | (*d.get(at + 1).unwrap_or(&0) as usize) << 8
}

/// One image at the start of `d` (a 12-byte header: x, y, width, height,
/// flags, the coded size; then the data), and its length in `d`.
pub fn image(d: &[u8]) -> Option<(Image, usize)> {
    let (x, y, w, h, flags, size) = (le16(d, 0), le16(d, 2), le16(d, 4), le16(d, 6), le16(d, 8), le16(d, 10));
    if w == 0 || h == 0 || w > 320 || h > 200 {
        return None;
    }
    let data = d.get(12..)?;
    let px = if flags & 2 != 0 {
        rle(data, w * h)?
    } else if flags & 4 != 0 {
        lzss(data.get(..size)?, w * h)
    } else if flags & 0xFF == 0 {
        data.get(..w * h)?.to_vec()
    } else {
        return None;
    };
    let used = if flags & 6 != 0 { 12 + size } else { 12 + (w * h).max(size) };
    Some((Image { x: x as i16 as i32, y: y as i16 as i32, w, h, px }, used))
}

/// A .CIF: its images one after another (0000:3C94 skips them by their
/// sizes, +0Ah, and 12).
pub fn cif(d: &[u8]) -> Vec<Image> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 12 <= d.len() {
        let size = le16(d, at + 10);
        match image(&d[at..]) {
            Some((img, _)) => out.push(img),
            None => out.push(Image { x: 0, y: 0, w: 0, h: 0, px: Vec::new() }),
        }
        at += 12 + size;
    }
    out
}

/// 0000:5B9A's coding: a byte with bit 7 set repeats the next (low 7 bits
/// + 1 times), otherwise that many + 1 bytes follow.
fn rle(src: &[u8], len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        let b = *src.get(i)?;
        i += 1;
        let n = (b & 0x7F) as usize + 1;
        if b & 0x80 != 0 {
            let v = *src.get(i)?;
            i += 1;
            out.extend(std::iter::repeat_n(v, n));
        } else {
            out.extend_from_slice(src.get(i..i + n)?);
            i += n;
        }
    }
    out.truncate(len);
    Some(out)
}

/// 0000:7608's LZSS, `src` the coded bytes; the output cut or padded to
/// `len`. (The ring's last 12h bytes start as whatever its buffer held:
/// zeros here.)
fn lzss(src: &[u8], len: usize) -> Vec<u8> {
    let mut ring = [0u8; 0x1000];
    ring[..0xFEE].fill(0x20);
    let mut r = 0xFEEusize;
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    let mut flags = 0u16;
    loop {
        flags >>= 1;
        if flags & 0x100 == 0 {
            let Some(&b) = src.get(i) else { break };
            i += 1;
            flags = b as u16 | 0xFF00;
        }
        if flags & 1 != 0 {
            let Some(&b) = src.get(i) else { break };
            i += 1;
            out.push(b);
            ring[r] = b;
            r = (r + 1) & 0xFFF;
        } else {
            let (Some(&lo), Some(&hi)) = (src.get(i), src.get(i + 1)) else { break };
            i += 2;
            let pos = lo as usize | ((hi as usize & 0xF0) << 4);
            for k in 0..(hi as usize & 0xF) + 3 {
                let b = ring[(pos + k) & 0xFFF];
                out.push(b);
                ring[r] = b;
                r = (r + 1) & 0xFFF;
            }
        }
    }
    out.resize(len, 0);
    out
}

/// A string of the list at DS:`at`, the `n`th (1194:559F).
fn listed(m: &Machine, at: u16, n: u16) -> String {
    let off = m.skip_strings(DS, at, n);
    String::from_utf8_lossy(m.mem.cstr(DS, off)).to_ascii_uppercase()
}

/// The interface's graphics, loaded as they're first needed.
pub struct Interface {
    archive: Rc<Archive>,
    bar: Option<Vec<u8>>,
    slider: Option<Vec<u8>>,
    compass: Option<Image>,
    status: Vec<Image>,
    cifs: HashMap<String, Vec<Image>>,
    cfas: HashMap<String, Option<crate::flats::Flat>>,
}

impl Interface {
    pub fn new(archive: Rc<Archive>) -> Interface {
        let bar = archive.read("P1.IMG").filter(|d| d.len() >= 320 * 53).map(|d| d[..320 * 53].to_vec());
        let slider = archive.read("SLIDER.IMG").filter(|d| d.len() >= 289 * 7);
        let compass = archive.read("COMPASS.IMG").and_then(|d| image(&d).map(|(i, _)| i));
        let status = archive.read("STATUS.CIF").map(|d| cif(&d)).unwrap_or_default();
        Interface { archive, bar, slider, compass, status, cifs: HashMap::new(), cfas: HashMap::new() }
    }

    fn cif(&mut self, name: &str) -> &[Image] {
        let archive = &self.archive;
        self.cifs.entry(name.to_string()).or_insert_with(|| archive.read(name).map(|d| cif(&d)).unwrap_or_default())
    }

    fn cfa(&mut self, name: &str) -> Option<&crate::flats::Flat> {
        let archive = &self.archive;
        self.cfas.entry(name.to_string()).or_insert_with(|| archive.read(name).and_then(|d| crate::flats::cfa(&d))).as_ref()
    }

    /// The interface for the machine's state, drawn on `screen` (under the
    /// texts), the name in `font`.
    pub fn draw(&mut self, screen: &mut Screen, font: &crate::hud::Font, m: &Machine) {
        use arena::weapon::var::{CAST_FRAME, CLASS, FRAME, RELOAD, SHOWN};
        let level = m.mem.r16(DS, arena::movement::var::LEVEL_FLAGS);
        let casting = (m.mem.r8(DS, CAST_FRAME) as i8) >= 0;
        // The hands: casting, the bow, or the weapon (3B8E).
        if casting {
            self.cast_hands(screen, m);
        } else if m.mem.r16(DS, 0x0620) == 0 && (level & 0x10 == 0 || level & 0x4000 != 0) && m.mem.r16(DS, 0x05CA) & 0x10 == 0 && (m.mem.r8(DS, CLASS) as i8) >= 0 {
            let shown = m.mem.r8(DS, SHOWN);
            let mut frame = m.mem.r16(DS, FRAME);
            let out = m.mem.r16(DS, 0x05E0) & 0x10 != 0;
            if shown == 6 {
                if out && m.mem.r8(DS, RELOAD) == 0 {
                    if let Some(f) = self.cfa("ARROWHLF.CFA").cloned() {
                        let (x, y) = cfa_place(&self.archive, "ARROWHLF.CFA");
                        screen.blit(x, y, f.w, f.h, &f.frames[0], 0x93);
                    }
                }
            } else if (shown as i8) >= 0 && (out || frame >= 0x1E) {
                if (7..=9).contains(&shown) && frame >= 0x1E {
                    frame -= 0x14;
                }
                let name = listed(m, 0x674E, shown as u16);
                if let Some(img) = self.cif(&name).get(frame as usize) {
                    screen.blit(img.x, img.y, img.w, img.h, &img.px, 0x93);
                }
            }
        }
        // The bar from row 147.
        if let Some(bar) = &self.bar {
            screen.blit(0, 147, 320, 53, bar, 199);
        }
        // The portrait: the condition, then the face (4E81, 2D5C).
        let p = arena::character::PLAYER;
        let r16 = |o: u16| m.mem.r16(DS, p + o);
        let fx = r16(0x40D);
        let mut cond = 0;
        if fx & 1 != 0 {
            cond = 1;
        }
        if fx & 2 != 0 || fx & 0x20 != 0 {
            cond = 2;
        }
        if r16(0x59) <= r16(0x5B) / 10 {
            cond = 3;
        }
        if let Some(img) = self.status.get(cond) {
            screen.blit(img.x, img.y, img.w, img.h, &img.px, 199);
        }
        let race = m.mem.r8(DS, p + 4);
        let female = m.mem.r8(DS, p + 7) != 0;
        let name = format!("{}0{}.CIF", if female { "FACESF" } else { "FACES" }, race);
        let face = m.mem.r8(DS, p + 0x6B) as usize;
        if let Some(img) = self.cif(&name).get(face).cloned() {
            screen.blit(14, 166, img.w, img.h, &img.px, 199);
        }
        // The name's first word (1194:4BC8).
        let full = String::from_utf8_lossy(m.mem.cstr(DS, p + 9)).into_owned();
        let first = full.split(' ').next().unwrap_or("");
        screen.text_plain(font, 17, 154, first, 0xF0);
        // The bars (2901:00A8).
        let fatigue_max = (m.mem.r8(DS, 0x01CD) as u16 + m.mem.r8(DS, 0x01D2) as u16) << 6;
        for (x, v, max, c) in [(0x39, m.mem.r16(DS, 0x01FD), m.mem.r16(DS, 0x01FF), 0x1A), (0x43, m.mem.r16(DS, 0x0201), fatigue_max, 0x9F), (0x4D, m.mem.r16(DS, 0x020A), m.mem.r16(DS, 0x020C), 0x3E)] {
            let full = bar_rows(v, max);
            screen.fill(x, 168, 4, 26 - full, 0, 0xFF);
            screen.fill(x, 168 + 26 - full, 4, full, c, 0xFF);
        }
        // The compass (4D7C).
        if m.mem.r16(DS, 0x05C8) & 0x2000 != 0 && m.mem.r8(DS, 0x1013) == 0 {
            if let Some(s) = &self.slider {
                let at = ((m.mem.r16(DS, arena::movement::var::HEADING).wrapping_add(0xE0) & 0x1FF) >> 1) as usize;
                for row in 0..7 {
                    let from = at + row * 0x121;
                    if let Some(px) = s.get(from..from + 32) {
                        screen.blit(144, 7 + row as i32, 32, 1, px, 199);
                    }
                }
            }
            if let Some(img) = &self.compass {
                screen.blit(img.x, img.y, img.w, img.h, &img.px, 199);
            }
        }
    }

    /// 29CF:0A28's drawing: frame [7F80] of the casting hands' .CFA, as
    /// loaded at offset [902E] of EMS bank [C8F4], at its offsets.
    fn cast_hands(&mut self, screen: &mut Screen, m: &Machine) {
        let bank = m.ems_bank(m.mem.r8(DS, 0xC8F4) as u16);
        let at = m.mem.r16(DS, 0x902E) as usize;
        let Some(d) = bank.get(at..) else { return };
        let Some(f) = crate::flats::cfa(d) else { return };
        let k = m.mem.r8(DS, arena::weapon::var::CAST_FRAME) as usize;
        if let Some(px) = f.frames.get(k) {
            screen.blit(le16(d, 6) as i32, le16(d, 8) as i32, f.w, f.h, px, 0x93);
        }
    }
}

/// The game's start (21EE:01DC): the compass shown (5C8 bit 2000h).
pub fn start(m: &mut Machine) {
    let f = m.mem.r16(DS, 0x05C8) | 0x2000;
    m.mem.w16(DS, 0x05C8, f);
}

/// The interface's set-up for the 3D view (0000:4DD6, 4F1B): the view's
/// window the 320 x 147 above the bar (the record at 883E: 0, 0, 13Fh,
/// 92h, as C39C and the clip A914..A91A hold it), the eye line row 3Dh
/// (C3D8), the centre column from it (as `world::load` makes it).
pub fn enter(m: &mut Machine) {
    for (i, v) in [0u16, 0, 0x13F, 0x92].into_iter().enumerate() {
        m.mem.w16(DS, 0xC39C + 2 * i as u16, v);
        m.mem.w16(DS, 0xA914 + 2 * i as u16, v);
    }
    m.mem.w16(DS, 0xC3D8, 0x3D);
    let centre = (m.mem.r16(DS, 0xA918) + m.mem.r16(DS, 0xA914)) / 2 + m.mem.r16(DS, 0xCB86);
    m.mem.w16(DS, 0xC3D6, centre);
}

/// The renderer's window for the machine's: the clip's rows (A91A + 1)
/// of 200, the eye line (C3D8 + 1, moving as the player bobs in water)
/// of those.
pub fn window(m: &Machine) -> crate::render::Window {
    let rows = (m.mem.r16(DS, 0xA91A) as i16 as f32 + 1.0).clamp(1.0, 200.0);
    let horizon = (m.mem.r16(DS, 0xC3D8) as i16 as f32 + 1.0) / rows;
    crate::render::Window { rows: rows / 200.0, horizon: horizon.clamp(0.0, 1.0) }
}

/// The weapon readied or put away (the menu keys' 'A', 2542:0137..0144;
/// `Machine::ready_weapon`).
pub fn ready(m: &mut Machine) {
    m.ready_weapon();
}

/// The original's scan code (set 1, the byte at DS:ACB5 + scan it keeps
/// per key) for a key of the window's, for the keys the game reads.
pub fn scan_code(code: winit::keyboard::KeyCode) -> Option<u8> {
    use winit::keyboard::KeyCode as K;
    Some(match code {
        K::Escape => 0x01,
        K::Digit1 => 0x02,
        K::Digit2 => 0x03,
        K::Digit3 => 0x04,
        K::Digit4 => 0x05,
        K::Digit5 => 0x06,
        K::Digit6 => 0x07,
        K::Digit7 => 0x08,
        K::Digit8 => 0x09,
        K::Digit9 => 0x0A,
        K::Digit0 => 0x0B,
        K::Minus => 0x0C,
        K::Equal => 0x0D,
        K::Backspace => 0x0E,
        K::Tab => 0x0F,
        K::KeyQ => 0x10,
        K::KeyW => 0x11,
        K::KeyE => 0x12,
        K::KeyR => 0x13,
        K::KeyT => 0x14,
        K::KeyY => 0x15,
        K::KeyU => 0x16,
        K::KeyI => 0x17,
        K::KeyO => 0x18,
        K::KeyP => 0x19,
        K::Enter | K::NumpadEnter => 0x1C,
        K::ControlLeft | K::ControlRight => 0x1D,
        K::KeyA => 0x1E,
        K::KeyS => 0x1F,
        K::KeyD => 0x20,
        K::KeyF => 0x21,
        K::KeyG => 0x22,
        K::KeyH => 0x23,
        K::KeyJ => 0x24,
        K::KeyK => 0x25,
        K::KeyL => 0x26,
        K::ShiftLeft => 0x2A,
        K::KeyZ => 0x2C,
        K::KeyX => 0x2D,
        K::KeyC => 0x2E,
        K::KeyV => 0x2F,
        K::KeyB => 0x30,
        K::KeyN => 0x31,
        K::KeyM => 0x32,
        K::ShiftRight => 0x36,
        K::AltLeft | K::AltRight => 0x38,
        K::Space => 0x39,
        K::F1 => 0x3B,
        K::F2 => 0x3C,
        K::F3 => 0x3D,
        K::F4 => 0x3E,
        K::F5 => 0x3F,
        K::F6 => 0x40,
        K::F7 => 0x41,
        K::F8 => 0x42,
        K::F9 => 0x43,
        K::F10 => 0x44,
        K::ArrowUp => 0x48,
        K::ArrowLeft => 0x4B,
        K::ArrowRight => 0x4D,
        K::ArrowDown => 0x50,
        // The rest a typed field reads (through the key interrupt's
        // tables, A944).
        K::BracketLeft => 0x1A,
        K::BracketRight => 0x1B,
        K::Semicolon => 0x27,
        K::Quote => 0x28,
        K::Backquote => 0x29,
        K::Backslash => 0x2B,
        K::Comma => 0x33,
        K::Period => 0x34,
        K::Slash => 0x35,
        K::NumpadMultiply => 0x37,
        K::CapsLock => 0x3A,
        K::Home => 0x47,
        K::PageUp => 0x49,
        K::NumpadSubtract => 0x4A,
        K::NumpadAdd => 0x4E,
        K::End => 0x4F,
        K::PageDown => 0x51,
        K::Insert => 0x52,
        K::Delete => 0x53,
        K::F11 => 0x57,
        K::F12 => 0x58,
        _ => return None,
    })
}

/// Whether `scan` is one of the menu keys 2542:0100 asks about.
pub fn is_menu_key(scan: u8) -> bool {
    use arena::menukeys::key;
    [key::ESC, key::A, key::C, key::P, key::S, key::U, key::M, key::F1, key::F8, key::L, key::R].contains(&(scan as u16))
}

/// The compass shown or hidden (the menu keys' F8, 2542:01C5).
pub fn toggle_compass(m: &mut Machine) {
    let f = m.mem.r16(DS, 0x05C8) ^ 0x2000;
    m.mem.w16(DS, 0x05C8, f);
}

/// A CFA's place: its offsets (+6, +8).
fn cfa_place(archive: &Archive, name: &str) -> (i32, i32) {
    archive.read(name).map_or((0, 0), |d| (le16(&d, 6) as i32, le16(&d, 8) as i32))
}

/// 2901:00E4: the rows filled of 26, value (not below 0, not above max)
/// x 256 / max x 26 / 256; none for a max of 0.
pub fn bar_rows(v: u16, max: u16) -> i32 {
    if max == 0 {
        return 0;
    }
    let v = if (v as i16) < 0 { 0 } else { v.min(max) } as u32;
    ((((v << 8) / max as u32) * 26) >> 8) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_fill_as_the_original() {
        assert_eq!(bar_rows(50, 50), 26);
        assert_eq!(bar_rows(0, 50), 0);
        assert_eq!(bar_rows(25, 50), 13);
        assert_eq!(bar_rows(0xFFFF, 50), 0);
        assert_eq!(bar_rows(10, 0), 0);
        assert_eq!(bar_rows(99, 50), 26);
    }

    #[test]
    fn lzss_literals_and_a_copy() {
        // Flags 0000_0111: three literals, then a copy of 3 from the ring's
        // first written byte (FEEh).
        let src = [0x07, b'a', b'b', b'c', 0xEE, 0xF0];
        assert_eq!(lzss(&src, 6), b"abcabc");
    }
}
