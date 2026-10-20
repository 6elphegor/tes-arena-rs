//! The game's typed fields (0000:9EE9, `arena::lineedit`) in the window.
//!
//! A field asked inside a turn stops it (`Need::Text`), as every
//! question does, with the field (where, how many characters, the text
//! it starts from) and the game as it was asked. The window then runs
//! the game's own editor live on a copy of that game (`Live`): each key
//! pressed and let go goes through the key interrupt's port
//! (`Machine::key_event`), so the characters are the game's keyboard
//! tables' (Shift, the arrows, Home, End, Del as the game reads them),
//! and each of the editor's passes (`Machine::edit_input`, `edit_key`)
//! takes what is waiting. The passes' inputs are kept; when the field
//! ends (Enter, the right button, Esc where the field ends on it) they
//! are the question's answer (`Answer::Keys`), and the turn run again
//! gives them to the editor on the game itself (`replay`), at the same
//! point, so the game's memory is as the original's editor leaves it.
//!
//! The editor measures with the font it is given (B034): the view puts
//! the field's font file in memory for it (`FONT_SEG`, put back after a
//! replay) where the game has none there.
//!
//! The window's passes: a key's characters are taken at once; a held
//! arrow moves the cursor once a pass when no key waits, the original
//! waiting five retraces after each (6BE2), so here once every sixth
//! frame. The cursor blinks as A0D9 makes it, toggled on every eighth
//! pass (once a frame here), put out by each redraw.

use std::collections::HashMap;
use std::sync::OnceLock;

use arena::lineedit::{EditKey, Field};
use arena::machine::Machine;
use arena::mem::DS;

use crate::hud::{Font, Screen};

/// Where the view puts a field's font for the editor: below the heap
/// (A000h on), above what the game's loaded data uses.
const FONT_SEG: u16 = 0x9A00;

/// The fonts' files, read once (`init`).
static FILES: OnceLock<HashMap<&'static str, Vec<u8>>> = OnceLock::new();
/// The fonts for drawing.
static DRAW: OnceLock<HashMap<&'static str, Font>> = OnceLock::new();

/// The font files the fields use.
pub const FONTS: [&str; 4] = ["ARENAFNT.DAT", "FONT_A.DAT", "TEENYFNT.DAT", "FONT_S.DAT"];

/// The fonts read from the game's files.
pub fn init(archive: &crate::archive::Archive) {
    let _ = FILES.set(FONTS.iter().filter_map(|&n| Some((n, archive.read(n)?))).collect());
    let _ = DRAW.set(FONTS.iter().filter_map(|&n| Some((n, Font::load(archive, n)?))).collect());
}

fn font_file(name: &str) -> Option<&'static [u8]> {
    FILES.get()?.get(name).map(|v| v.as_slice())
}

fn draw_font(name: &str) -> Option<&'static Font> {
    DRAW.get()?.get(name)
}

/// How a field's text and cursor are drawn: its font (the file of the
/// font the caller made current), its colour (9288 as the caller set
/// it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub font: &'static str,
    pub colour: u8,
    /// Whether the editor measures with this font (the view puts it in
    /// memory for it); false where the game's own font is current (the
    /// automap's FONT_S.DAT, which the view loads).
    pub put_font: bool,
}

/// The field's font put where the editor reads it (B034), and what was
/// there, to put back.
fn put_font(m: &mut Machine, look: &Look) -> Option<(u32, Vec<u8>)> {
    if !look.put_font {
        return None;
    }
    let f = font_file(look.font)?;
    let n = f.len().min(0xFFFF);
    let kept = (m.mem.r32(DS, arena::lineedit::var::FONT), (0..n as u16).map(|i| m.mem.r8(FONT_SEG, i)).collect());
    for (i, &b) in f.iter().take(n).enumerate() {
        m.mem.w8(FONT_SEG, i as u16, b);
    }
    m.mem.w32(DS, arena::lineedit::var::FONT, (FONT_SEG as u32) << 16);
    Some(kept)
}

fn take_font(m: &mut Machine, kept: Option<(u32, Vec<u8>)>) {
    if let Some((ptr, bytes)) = kept {
        for (i, &b) in bytes.iter().enumerate() {
            m.mem.w8(FONT_SEG, i as u16, b);
        }
        m.mem.w32(DS, arena::lineedit::var::FONT, ptr);
    }
}

/// The editor on the game itself with the passes' inputs `keys` (a
/// question's answer): Some(true) once it ended, Some(false) where the
/// keys ran out (left open), None where the original faults or hangs.
pub fn replay(m: &mut Machine, field: &Field, look: &Look, keys: &[EditKey]) -> Option<bool> {
    // The characters as the key interrupt left them in the ring and the
    // editor's passes read them (`Machine::ring_typed`).
    for &k in keys {
        if let EditKey::Key(c) = k {
            m.ring_typed(c);
        }
    }
    let kept = put_font(m, look);
    let r = m.line_edit(field, keys);
    take_font(m, kept);
    r
}

/// A script's text as a field's inputs: Esc (the field emptied, where it
/// doesn't end on it), its characters, Enter.
pub fn keys_of(text: &str, esc_ends: bool) -> Vec<EditKey> {
    let mut k = Vec::new();
    if !esc_ends {
        k.push(EditKey::Key(0x1B));
    }
    k.extend(text.bytes().map(EditKey::Key));
    k.push(EditKey::Key(0x0D));
    k
}

/// 1194:5C34's box: the format at 4FBC ("%s\r", then a line of spaces,
/// the field's room) with the prompt at `seg`:`off`.
pub fn prompt_5c30(m: &Machine, seg: u16, off: u16) -> String {
    let fmt = String::from_utf8_lossy(m.mem.cstr(DS, 0x4FBC)).into_owned();
    fmt.replacen("%s", &String::from_utf8_lossy(m.mem.cstr(seg, off)), 1)
}

/// A field being typed in the window.
#[derive(Clone)]
pub struct Live {
    m: Box<Machine>,
    pub field: Field,
    pub look: Look,
    keys: Vec<EditKey>,
    ended: bool,
    /// Whether the cursor is drawn (A0D9's XOR toggled since the last
    /// redraw), and where.
    shown: bool,
    at: Option<(u16, u16, u32, u32)>,
    /// Frames to wait before a held arrow moves the cursor again.
    wait: u32,
}

impl std::fmt::Debug for Live {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Live({:?}, {:?})", String::from_utf8_lossy(&self.text()), self.keys)
    }
}

impl Live {
    /// The field opened on a copy of `m` (the game as it asked). None
    /// where the editor would fault or hang at the start.
    pub fn open(m: &Machine, field: Field, look: Look) -> Option<Live> {
        let mut m = Box::new(m.clone());
        put_font(&mut m, &look);
        m.edit_open(&field)?;
        Some(Live { m, field, look, keys: Vec::new(), ended: false, shown: false, at: None, wait: 0 })
    }

    /// The passes' inputs so far.
    pub fn keys(&self) -> &[EditKey] {
        &self.keys
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    /// The text as it stands.
    pub fn text(&self) -> Vec<u8> {
        let (seg, off) = (self.field.seg, self.field.off);
        let mut t = Vec::new();
        for i in 0..0x400u16 {
            let b = self.m.mem.r8(seg, off.wrapping_add(i));
            if b == 0 {
                break;
            }
            t.push(b);
        }
        t
    }

    fn pass(&mut self, k: EditKey) {
        self.keys.push(k);
        self.shown = false;
        match self.m.edit_key(k) {
            Some(false) => {}
            _ => self.ended = true,
        }
    }

    /// A key (its scan code) pressed or let go: the key interrupt, then a
    /// pass for each character waiting.
    pub fn key(&mut self, scan: u8, down: bool) {
        if self.ended {
            return;
        }
        if self.m.key_event(if down { scan } else { scan | 0x80 }).is_none() {
            return;
        }
        while !self.ended && self.m.mem.r16(DS, arena::lineedit::var::COUNT) != 0 {
            match self.m.edit_input() {
                Some(k) => self.pass(k),
                None => break,
            }
        }
    }

    /// A key pressed and let go.
    pub fn tap(&mut self, scan: u8) {
        self.key(scan, true);
        self.key(scan, false);
    }

    /// The character `c` typed (a script's): the key its keyboard's table
    /// gives it for (the plain table's, else the shifted one's with the
    /// left Shift held); a character no key gives, nothing.
    pub fn type_char(&mut self, c: u8) {
        use arena::lineedit::var::{LAYOUT, PLAIN, SHIFTED};
        let layout = match self.m.mem.r8(DS, LAYOUT) {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        let find = |m: &Machine, t: u16| (1..0x80u8).find(|&s| m.mem.r8(DS, t + s as u16) == c);
        if let Some(s) = find(&self.m, PLAIN[layout]) {
            self.tap(s);
        } else if let Some(s) = find(&self.m, SHIFTED[layout]) {
            self.key(0x2A, true);
            self.tap(s);
            self.key(0x2A, false);
        }
    }

    /// The right button pressed (AFF9 bit 2): the field ends.
    pub fn button(&mut self) {
        if !self.ended {
            self.pass(EditKey::Button);
        }
    }

    /// A frame: a held arrow's pass, and the cursor's blink.
    pub fn frame(&mut self) {
        if self.ended {
            return;
        }
        if self.wait > 0 {
            self.wait -= 1;
        } else if let Some(k @ EditKey::Key(1 | 0xFF)) = self.m.edit_input() {
            self.pass(k);
            self.wait = 5;
        }
        if let Some(at) = self.m.edit_cursor() {
            self.shown = !self.shown;
            self.at = Some(at);
        }
    }

    /// The text at the field's place in its font and colour, and the
    /// cursor where shown (XORed in the text's colour). A shot, never
    /// counting frames, shows the cursor at its place.
    pub fn draw(&self, screen: &mut Screen, still: bool) {
        if draw_font(self.look.font).is_none() {
            return;
        }
        self.draw_text(screen);
        let at = if still { self.cursor_place() } else if self.shown { self.at } else { None };
        if let Some((cx, cy, w, h)) = at {
            screen.xor(cx as i16 as i32, cy as i16 as i32, w.min(320) as i32, h.min(200) as i32, self.look.colour);
        }
    }

    /// The game's page on the editor's copy (its own block): the prompt's
    /// box and the text as the last pass's redraw (A0AE) left them, what
    /// the original shows while it waits for a key. None where the page
    /// isn't the game's.
    pub fn page(&self) -> Option<Vec<u8>> {
        let seg = self.m.mem.r16(DS, 0xA900);
        self.m.page_is_block().then(|| self.m.mem.segment(seg)[..0xFA00].to_vec())
    }

    /// The text alone at the field's place in its font and colour.
    pub fn draw_text(&self, screen: &mut Screen) {
        let Some(font) = draw_font(self.look.font) else { return };
        let s = String::from_utf8_lossy(&self.text()).into_owned();
        let (x, y) = (self.field.x as i16 as i32, self.field.y as i16 as i32);
        screen.text_plain(font, x, y, &s, self.look.colour);
    }

    /// Where the cursor is drawn now (A0D9's place, its count left as it
    /// is).
    fn cursor_place(&self) -> Option<(u16, u16, u32, u32)> {
        let mut m = self.m.clone();
        m.mem.w16(DS, arena::lineedit::var::BLINK, 0);
        m.edit_cursor()
    }
}
