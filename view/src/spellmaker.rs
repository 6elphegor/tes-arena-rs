//! The spellmaker's screen (src/spellmaker.rs runs it): the original's
//! picture (SPELLMKR.IMG, in its own colours) with the spell's parts
//! written where 1194:68D7 writes them, and an effect's form (formN.img)
//! over it with its controls' values where 1194:6410 writes them. The
//! game's files are read at run time.
//!
//! The view drives it from a script (`--spellmaker`): clicks by name
//! ("name", "effects", "target", "element", "new", "buy", "exit" on the
//! screen; "up:I", "down:I" (control I's arrows) and "done" on a form),
//! "pick:N" answers the next list or menu, "type:TEXT" the name typed;
//! "show" on a form keeps it as the picture shown (and clicks done);
//! "gold:N" sets the player's gold first, a debugging aid.
//! When the script runs out the screen's "exit" (or the form's done box)
//! is clicked. The picture shown is the last screen or form reached.

use crate::archive::Archive;
use crate::hud::{Font, Screen};
use crate::interface::image;
use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::spellmaker::SpellmakerHooks;
use std::collections::VecDeque;

/// A box: x0, y0, x1, y1.
type Area = (u16, u16, u16, u16);

/// The regions' boxes on the screen (DGROUP 5711).
const REGIONS: [(&str, Area); 7] = [
    ("name", (0x45, 0x31, 0xC8, 0x39)),
    ("effects", (0x12, 0x48, 0x34, 0x4F)),
    ("target", (0x12, 0x3C, 0x31, 0x45)),
    ("element", (0xCB, 0x31, 0xF2, 0x38)),
    ("new", (0x0A, 0xBC, 0x35, 0xC5)),
    ("buy", (0xCA, 0xBC, 0xF3, 0xC5)),
    ("exit", (0x11F, 0xBC, 0x131, 0xC5)),
];

/// The script's answers, and what was last shown.
#[derive(Default)]
pub struct Maker {
    clicks: VecDeque<String>,
    picks: VecDeque<u16>,
    names: VecDeque<String>,
    /// The machine as the screen was last drawn, and as a form's last
    /// click found it (with the form's record).
    pub screen: Option<Machine>,
    pub form: Option<(Machine, u16)>,
    /// "show" kept the form's picture as the one shown.
    frozen: bool,
    /// The message boxes' texts, in order.
    pub messages: Vec<String>,
    /// "gold:N": the player's gold set first (a debugging aid).
    pub gold: Option<u32>,
}

impl Maker {
    pub fn new(script: &str) -> Maker {
        let mut m = Maker::default();
        for t in script.split(',').filter(|t| !t.is_empty()) {
            if let Some(n) = t.strip_prefix("pick:") {
                m.picks.push_back(n.parse().expect("--spellmaker pick:N"));
            } else if let Some(n) = t.strip_prefix("gold:") {
                m.gold = Some(n.parse().expect("--spellmaker gold:N"));
            } else if let Some(n) = t.strip_prefix("type:") {
                m.names.push_back(n.to_string());
            } else {
                m.clicks.push_back(t.to_string());
            }
        }
        m
    }
}

impl SpellmakerHooks for Maker {
    fn click(&mut self, m: &mut Machine, form: Option<u16>) -> (u16, u16) {
        let t = self.clicks.pop_front();
        match form {
            None => {
                if !self.frozen {
                    self.form = None;
                }
                let r = t.as_deref().and_then(|t| REGIONS.iter().find(|(n, _)| *n == t)).map_or(REGIONS[6].1, |r| r.1);
                ((r.0 + r.2) / 2, (r.1 + r.3) / 2)
            }
            Some(f) => {
                if !self.frozen {
                    self.form = Some((m.clone(), f));
                }
                if t.as_deref() == Some("show") {
                    self.frozen = true;
                }
                let rd = |o: u16| m.mem.r16(DATA, f.wrapping_add(o));
                let (fx, fy) = (m.mem.r16(DS, 0xA880), m.mem.r16(DS, 0xA882));
                let arrow = t.as_deref().and_then(|t| {
                    let (dir, i) = t.split_once(':')?;
                    let i: u16 = i.parse().ok()?;
                    let c = f.wrapping_add(0x12 + 0x12 * (i % rd(4).max(1)));
                    let (x, y) = (m.mem.r16(DATA, c), m.mem.r16(DATA, c + 2));
                    Some((fx.wrapping_add(x).wrapping_add(4), fy.wrapping_add(y).wrapping_add(if dir == "up" { 2 } else { 9 })))
                });
                arrow.unwrap_or((fx.wrapping_add(rd(0xA)).wrapping_add(2), fy.wrapping_add(rd(0xC)).wrapping_add(2)))
            }
        }
    }
    fn list(&mut self, _m: &mut Machine, _si: u16, cx: u16) -> u16 {
        self.picks.pop_front().map_or(0xFFFF, |v| v % cx.max(1))
    }
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        self.picks.pop_front().unwrap_or(0xFFFF)
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.messages.push(String::from_utf8_lossy(m.mem.cstr(DS, si)).replace('\r', " ").trim().to_string());
    }
    fn type_name(&mut self, m: &mut Machine, si: u16) {
        let t = self.names.pop_front().unwrap_or_default();
        let b: Vec<u8> = t.bytes().take(0x1F).chain([0]).collect();
        for (i, &c) in b.iter().enumerate() {
            m.mem.w8(DS, si.wrapping_add(i as u16), c);
        }
    }
    fn screen(&mut self, m: &mut Machine) {
        self.screen = Some(m.clone());
    }
}

/// An image's 6-bit palette (768 bytes after its pixels) made 8-bit.
fn palette(d: &[u8], at: usize) -> Option<[[u8; 4]; 256]> {
    let p = d.get(at..at + 768)?;
    let mut out = [[0u8; 4]; 256];
    for (i, c) in out.iter_mut().enumerate() {
        let v = |k: usize| p[3 * i + k] << 2 | p[3 * i + k] >> 4;
        *c = [v(0), v(1), v(2), 255];
    }
    Some(out)
}

fn text(m: &Machine, seg: u16, off: u16) -> String {
    String::from_utf8_lossy(m.mem.cstr(seg, off)).into_owned()
}

/// The string `n` of the list at DS:`si` (NUL-separated).
fn nth(m: &Machine, mut si: u16, n: u16) -> String {
    for _ in 0..n {
        si = si.wrapping_add(m.mem.cstr(DS, si).len() as u16 + 1);
    }
    text(m, DS, si)
}

/// Draws the last screen (and the form over it, when one was up last).
pub fn draw(screen: &mut Screen, font: &Font, archive: &Archive, files: &Files, maker: &Maker) {
    let Some(base) = maker.screen.as_ref().or(maker.form.as_ref().map(|f| &f.0)) else { return };
    let Some(d) = archive.read("SPELLMKR.IMG") else { return };
    let Some((img, used)) = image(&d) else { return };
    if let Some(p) = palette(&d, used) {
        screen.set_palette(p);
    }
    screen.blit(0, 0, img.w, img.h, &img.px, 200);
    let m = match &maker.form {
        Some((m, _)) => m,
        None => base,
    };
    let c = 0xA0u8;
    let mut mm = m.clone();
    let (cost, per) = mm.made_spell_cost();
    screen.text_plain(font, 0x45, 0x2F, &text(m, DS, 0x581A), c);
    screen.text_plain(font, 0xF4, 0x30, &nth(m, 0x566F, m.mem.r8(DS, 0x580C) as u16), c);
    screen.text_plain(font, 0xF0, 0x17, &m.mem.r32(DS, 0x05C2).to_string(), c);
    screen.text_plain(font, 0x2D, 0x17, &text(m, DS, 0x01AD), c);
    screen.text_plain(font, 0x2E, 0x21, &m.mem.r16(DS, 0xA898).to_string(), c);
    screen.text_plain(font, 0xFA, 0x21, &cost.wrapping_add(cost).to_string(), c);
    screen.text_plain(font, 0x104, 0x3B, &per.to_string(), c);
    screen.text_plain(font, 0x32, 0x3B, &nth(m, 0x5691, mm.mem.r8(DS, 0x580A) as u16), c);
    let mut fs = files.clone();
    if let Some(effects) = mm.made_spell_effects(&mut fs) {
        let mut y = 0x52;
        for (name, lines) in effects {
            screen.text_plain(font, 0x12, y, &String::from_utf8_lossy(&name), c);
            y += 0xA;
            for l in lines {
                screen.text_plain(font, 0x12, y, &String::from_utf8_lossy(&l), c);
                y += 0xA;
            }
        }
    }
    let Some((m, f)) = &maker.form else { return };
    let name = text(m, DS, m.mem.r16(DATA, f.wrapping_add(2)));
    let Some(d) = archive.read(&name.to_ascii_uppercase()) else { return };
    let Some((img, _)) = image(&d) else { return };
    let (fx, fy) = (m.mem.r16(DS, 0xA880) as i16 as i32, m.mem.r16(DS, 0xA882) as i16 as i32);
    screen.blit(fx, fy, img.w, img.h, &img.px, 200);
    screen.text_plain(font, fx + (img.w as i32 - font.width(&text(m, DS, 0x9492)) as i32) / 2, fy + 6, &text(m, DS, 0x9492), 0xFF);
    let slot = m.mem.r16(DS, 0x585B);
    let n = m.mem.r16(DATA, f.wrapping_add(4));
    for k in 0..n {
        let s = f.wrapping_add(0x12 + 0x12 * k);
        let r = |o: u16| m.mem.r16(DATA, s.wrapping_add(o));
        if (r(8) as i16) < 0 {
            continue;
        }
        let field = 0x57E6u16.wrapping_add(slot).wrapping_add(slot).wrapping_add(r(0xC));
        // 1194:6410: the number ends at the field's x + 0Ch (+ 1Dh for
        // ranges past 100), one row below its y.
        let dx = if r(0xA) as i16 > 0x64 { 0x1D } else { 0xC };
        let v = m.mem.r16(DS, field).to_string();
        screen.text_plain(font, fx + r(4) as i32 + dx - font.width(&v) as i32, fy + r(6) as i32 + 1, &v, 0xFF);
    }
    // The cost as the class pays it (1194:636B), at the form's +6, +8.
    let shown = text(m, DS, 0x929E);
    screen.text_plain(font, fx + m.mem.r16(DATA, f.wrapping_add(6)) as i32, fy + m.mem.r16(DATA, f.wrapping_add(8)) as i32, &shown, 0xFF);
}
