//! The game's animations (.FLC, Autodesk Animator Pro: all 320 x 200, 8
//! bits), as the original plays them (0000:6546): frame by frame at the
//! file's speed, in the file's own colours, the ring frame looping back
//! to the second.
//!
//! The chunks the game's files use: 4 (256-colour palette), 15 (the first
//! frame, run-length), 7 (later frames, word deltas, with 0000:63B4's
//! quirks), 18 (a thumbnail, skipped). 11, 12, 13 and 16 (the older FLI's)
//! are read too. A .CEL (the quiz's figures) is the same with a smaller
//! frame, placed by its CEL_DATA chunk.
//!
//! Where the original plays them:
//! - INTRO.FLC at the main menu's start (0000:125C, through 2294:0895),
//!   before TITLE.IMG and the menu (MENU.IMG, 0000:10FA);
//! - VISION.FLC behind the vision's text after creation (0000:113E..115D:
//!   1194:D459 with text 578h) and at the end of a game (0000:0D18:
//!   text 57Ah; JAGAR.FLC with 57Bh while F77 is set, 0000:0D08);
//! - SCROLL.FLC behind the opening's story (0000:12E3, 134E), CHAOSVSN.FLC
//!   in a dream (1194:D3D5), MORPH.FLC and JAGAR.FLC at the Emperor's
//!   audience (0000:41BC, 41C4), and the main quest's ending (1194:D936..:
//!   JAGAR, WALK, AFLC2, NUJAGDTH, STAFF, KING, NUKING).

use crate::archive::Archive;
use crate::hud::{self, Font, Screen, Span};

pub const W: usize = 320;
pub const H: usize = 200;

/// An animation being played.
pub struct Flc {
    data: Vec<u8>,
    frames: u16,
    /// Milliseconds a frame.
    pub speed: u32,
    /// The file offsets of the first frame and of the second (where the
    /// ring frame loops back to).
    first: usize,
    second: usize,
    at: usize,
    /// Frames decoded (1 after the first).
    pub shown: u32,
    /// Whether it has been through once (its last frame shown).
    pub looped: bool,
    pub px: Vec<u8>,
    pub palette: [[u8; 4]; 256],
    /// The frame's size: the file's (a .CEL's is smaller than the
    /// screen; the game's .FLCs are all 320 x 200).
    pub w: usize,
    pub h: usize,
}

fn le16(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([*d.get(at).unwrap_or(&0), *d.get(at + 1).unwrap_or(&0)])
}

fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([*d.get(at).unwrap_or(&0), *d.get(at + 1).unwrap_or(&0), *d.get(at + 2).unwrap_or(&0), *d.get(at + 3).unwrap_or(&0)])
}

impl Flc {
    /// The animation named `name` (a loose file or GLOBAL.BSA's), its
    /// first frame decoded.
    pub fn open(archive: &Archive, name: &str) -> Option<Flc> {
        let data = archive.read(name)?;
        let magic = le16(&data, 4);
        if magic != 0xAF12 && magic != 0xAF11 {
            return None;
        }
        let frames = le16(&data, 6);
        let (speed, first, second) = if magic == 0xAF12 {
            let f1 = le32(&data, 80) as usize;
            let f2 = le32(&data, 84) as usize;
            (le32(&data, 16).max(1), if f1 == 0 { 128 } else { f1 }, f2)
        } else {
            // FLI: the speed in 1/70 s, frames from 128.
            ((le16(&data, 16) as u32 * 1000 / 70).max(1), 128, 0)
        };
        let (w, h) = match (le16(&data, 8) as usize, le16(&data, 10) as usize) {
            (w @ 1..=W, h @ 1..=H) if magic == 0xAF12 => (w, h),
            _ => (W, H),
        };
        let mut f = Flc { data, frames, speed, first, second, at: first, shown: 0, looped: false, px: vec![0; w * h], palette: [[0, 0, 0, 0xFF]; 256], w, h };
        f.step();
        if f.second == 0 {
            f.second = f.at;
        }
        Some(f)
    }

    /// The next frame decoded; after the last, the ring frame (a delta
    /// back to the first) and on from the second.
    pub fn step(&mut self) {
        if self.shown as usize > self.frames as usize {
            // Past the ring frame: on from the second frame.
            self.at = self.second;
            self.shown = 1;
            self.looped = true;
        }
        let at = self.at;
        let size = le32(&self.data, at) as usize;
        if size < 16 || at + size > self.data.len() {
            self.at = self.second.max(self.first);
            self.shown = 1;
            self.looped = true;
            return;
        }
        if le16(&self.data, at + 4) == 0xF1FA {
            let data = std::mem::take(&mut self.data);
            let n = le16(&data, at + 6);
            let mut c = at + 16;
            for _ in 0..n {
                let cs = le32(&data, c) as usize;
                let kind = le16(&data, c + 4);
                if cs < 6 || c + cs > at + size {
                    break;
                }
                self.chunk(kind, &data[c + 6..c + cs]);
                c += cs;
            }
            self.data = data;
        }
        self.at = at + size;
        self.shown += 1;
        if self.shown as usize == self.frames as usize {
            self.looped = true;
        }
    }

    fn chunk(&mut self, kind: u16, d: &[u8]) {
        match kind {
            // COLOR_256 (8 bits) and COLOR_64 (6 bits).
            4 | 11 => {
                let shift = if kind == 11 { 2 } else { 0 };
                let packets = le16(d, 0);
                let (mut i, mut p) = (0usize, 2usize);
                for _ in 0..packets {
                    i += *d.get(p).unwrap_or(&0) as usize;
                    let mut n = *d.get(p + 1).unwrap_or(&0) as usize;
                    if n == 0 {
                        n = 256;
                    }
                    p += 2;
                    for _ in 0..n {
                        if i < 256 && p + 2 < d.len() {
                            let v = |k: usize| {
                                let x = d[p + k];
                                if shift == 2 { (x & 0x3F) << 2 | (x & 0x3F) >> 4 } else { x }
                            };
                            self.palette[i] = [v(0), v(1), v(2), 0xFF];
                        }
                        i += 1;
                        p += 3;
                    }
                }
            }
            // BLACK.
            13 => self.px.fill(0),
            // BYTE_RUN: each line its count byte, then runs.
            15 => {
                let mut p = 0;
                let (w, h) = (self.w, self.h);
                for y in 0..h {
                    p += 1;
                    let mut x = 0;
                    while x < w && p < d.len() {
                        let n = d[p] as i8;
                        p += 1;
                        if n >= 0 {
                            let v = *d.get(p).unwrap_or(&0);
                            p += 1;
                            for _ in 0..n {
                                if x < w {
                                    self.px[y * w + x] = v;
                                }
                                x += 1;
                            }
                        } else {
                            for _ in 0..(-(n as i32)) {
                                if x < w {
                                    self.px[y * w + x] = *d.get(p).unwrap_or(&0);
                                }
                                x += 1;
                                p += 1;
                            }
                        }
                    }
                }
            }
            // FLI_COPY: the frame as it is.
            16 => {
                let n = d.len().min(self.w * self.h);
                self.px[..n].copy_from_slice(&d[..n]);
            }
            // DELTA_FLI (LC): byte runs on a range of lines.
            12 => {
                let mut y = le16(d, 0) as usize;
                let lines = le16(d, 2);
                let mut p = 4;
                for _ in 0..lines {
                    let packets = *d.get(p).unwrap_or(&0);
                    p += 1;
                    let mut x = 0usize;
                    for _ in 0..packets {
                        x += *d.get(p).unwrap_or(&0) as usize;
                        let n = *d.get(p + 1).unwrap_or(&0) as i8;
                        p += 2;
                        if n >= 0 {
                            for _ in 0..n {
                                self.put(x, y, *d.get(p).unwrap_or(&0));
                                x += 1;
                                p += 1;
                            }
                        } else {
                            let v = *d.get(p).unwrap_or(&0);
                            p += 1;
                            for _ in 0..(-(n as i32)) {
                                self.put(x, y, v);
                                x += 1;
                            }
                        }
                    }
                    y += 1;
                }
            }
            // DELTA_FLC (SS2): word runs, lines skipped by negative words;
            // as 0000:63B4 reads it: the "last byte" word (8000h) puts its
            // byte before the line's start (DEC DI, STOSB: the line
            // before's last), and a count of 0 packets is passed over
            // for the next word, the line not done.
            7 => {
                let lines = le16(d, 0);
                let mut p = 2;
                let mut y = 0usize;
                for _ in 0..lines {
                    let packets;
                    loop {
                        let w = le16(d, p);
                        p += 2;
                        if p > d.len() {
                            return;
                        }
                        match w >> 14 {
                            3 => y += (w as i16).unsigned_abs() as usize,
                            2 => {
                                if let Some(i) = (y * self.w).checked_sub(1) {
                                    self.put_at(i, w as u8);
                                }
                            }
                            _ if w == 0 => {}
                            _ => {
                                packets = w;
                                break;
                            }
                        }
                    }
                    let mut x = 0usize;
                    for _ in 0..packets {
                        x += *d.get(p).unwrap_or(&0) as usize;
                        let n = *d.get(p + 1).unwrap_or(&0) as i8;
                        p += 2;
                        if n >= 0 {
                            for _ in 0..n {
                                self.put(x, y, *d.get(p).unwrap_or(&0));
                                self.put(x + 1, y, *d.get(p + 1).unwrap_or(&0));
                                x += 2;
                                p += 2;
                            }
                        } else {
                            let (a, b) = (*d.get(p).unwrap_or(&0), *d.get(p + 1).unwrap_or(&0));
                            p += 2;
                            let n = 2 * (-(n as i32)) as usize;
                            self.fill_77ce(y * self.w + x, n, a, b);
                            x += n;
                        }
                    }
                    y += 1;
                }
            }
            _ => {}
        }
    }

    /// 0000:77CE: `n` bytes from buffer offset `i` filled with the word
    /// `a`, `b` (AX) as a byte fill widened: at an odd offset first AL,
    /// then dwords of AX:AX (a b a b), then what is left (0..3 bytes)
    /// AL alone. (A word repeated once at an even offset is a, a.) The
    /// buffer is taken to start at an even address.
    fn fill_77ce(&mut self, mut i: usize, mut n: usize, a: u8, b: u8) {
        if n == 0 {
            return;
        }
        if i & 1 != 0 {
            self.put_at(i, a);
            i += 1;
            n -= 1;
        }
        for k in 0..(n >> 2) * 4 {
            self.put_at(i + k, if k & 1 == 0 { a } else { b });
        }
        let done = (n >> 2) * 4;
        for k in done..n {
            self.put_at(i + k, a);
        }
    }

    fn put_at(&mut self, i: usize, v: u8) {
        if let Some(p) = self.px.get_mut(i) {
            *p = v;
        }
    }

    fn put(&mut self, x: usize, y: usize, v: u8) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = v;
        }
    }

    /// The frame drawn on the layer in its colours, the rows above
    /// `rows`; the rest of the layer in its colour 0. (Colour 0 is a
    /// colour here: the layer is filled with it first, as `blit` leaves
    /// 0 see-through.)
    pub fn draw(&self, screen: &mut Screen, rows: usize) {
        screen.set_palette(self.palette);
        screen.fill(0, 0, W as i32, H as i32, 0, 0xFF);
        let rows = rows.min(H);
        screen.blit(0, 0, W, rows, &self.px[..W * rows], H as i32);
    }
}

/// An animation shown over the game, with (for a vision) its text
/// three lines at a time.
pub struct Movie {
    pub flc: Flc,
    /// The text's lines, and the first shown (1194:D459 through
    /// 2294:076C: three lines at once, centred at x A0h from row AAh, ten
    /// apart, the picture above row A7h; a key the next three, and the
    /// last key ends it).
    pub lines: Vec<Vec<Span>>,
    pub top: usize,
    /// Milliseconds into the current frame.
    elapsed: u32,
    /// Over: a key past the text, or (no text) a key or the end.
    pub over: bool,
    /// A still picture with its text all at once from row 32h, over the
    /// whole picture (the ending's house, 1194:DA38..DAC2); a key ends it.
    pub full: bool,
}

impl Flc {
    /// A picture of one frame, 320 x 200 indices in `palette`, that never
    /// steps.
    pub fn still(px: Vec<u8>, palette: [[u8; 4]; 256]) -> Flc {
        Flc { data: Vec::new(), frames: 1, speed: u32::MAX, first: 0, second: 0, at: 0, shown: 1, looped: false, px, palette, w: W, h: H }
    }
}

/// The vision's lines a page (2294:07B8: at most 3).
pub const VISION_LINES: usize = 3;

impl Movie {
    /// `name` played; with `lines`, as a vision with that text.
    pub fn new(archive: &Archive, name: &str, lines: Vec<Vec<Span>>) -> Option<Movie> {
        let flc = Flc::open(archive, name)?;
        Some(Movie { flc, lines, top: 0, elapsed: 0, over: false, full: false })
    }

    /// A still picture shown with all its text (`full`).
    pub fn still(flc: Flc, lines: Vec<Vec<Span>>) -> Movie {
        Movie { flc, lines, top: 0, elapsed: 0, over: false, full: true }
    }

    /// Time passing (milliseconds): frames stepped at the file's speed. An
    /// animation without text is over when it has been through once.
    pub fn advance(&mut self, ms: u32) {
        self.elapsed += ms;
        while self.elapsed >= self.flc.speed && !self.over {
            self.elapsed -= self.flc.speed;
            self.flc.step();
            if self.lines.is_empty() && self.flc.looped {
                self.over = true;
            }
        }
    }

    /// A key or click: the next three lines, or the end.
    pub fn key(&mut self) {
        if !self.full && self.top + VISION_LINES < self.lines.len() {
            self.top += VISION_LINES;
        } else {
            self.over = true;
        }
    }

    /// The frame, and the text's lines now in `font`.
    pub fn draw(&self, screen: &mut Screen, font: &Font) {
        if self.full {
            // 1194:DA91..DAC2: every line centred at x A0h from row 32h,
            // ten apart, over the whole picture.
            self.flc.draw(screen, H);
            for (i, l) in self.lines.iter().enumerate() {
                let w = font.width(&hud::plain(l)) as i32;
                let mut x = 0xA0 - w / 2;
                let y = 0x32 + 10 * i as i32;
                for s in l {
                    screen.text_plain(font, x, y, &s.text, s.colour);
                    x += font.width(&s.text) as i32;
                }
            }
            return;
        }
        if self.lines.is_empty() {
            self.flc.draw(screen, H);
            return;
        }
        self.flc.draw(screen, 0xA7);
        for (i, l) in self.lines.iter().skip(self.top).take(VISION_LINES).enumerate() {
            let w = font.width(&hud::plain(l)) as i32;
            let mut x = 0xA0 - w / 2;
            let y = 0xAA + 10 * i as i32;
            for s in l {
                screen.text_plain(font, x, y, &s.text, s.colour);
                x += font.width(&s.text) as i32;
            }
        }
    }
}
