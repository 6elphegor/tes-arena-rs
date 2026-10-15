//! The heads-up layer: the game's texts drawn over the view in the game's
//! own font, on a 320 x 200 screen as the original's (scaled to the
//! window), composed on the CPU and blended over the world.
//!
//! A text is the game's: `\t` and a byte set the colour (a palette
//! index), `\r` ends a line. The font is ARENAFNT.DAT, read from the
//! game's files at run time: its first byte is the height, the next 94
//! the widths of characters 21h..7Eh, then each character's rows, a
//! little-endian word each, the leftmost pixel in the top bit.

pub const W: usize = 320;
pub const H: usize = 200;

/// A font of the game's.
pub struct Font {
    pub h: usize,
    widths: [u8; 94],
    rows: Vec<u16>,
}

impl Font {
    /// The font file `name` from the game's files.
    pub fn load(archive: &crate::archive::Archive, name: &str) -> Option<Font> {
        let d = archive.read(name)?;
        let h = *d.first()? as usize;
        if h == 0 || d.len() < 95 + 94 * h * 2 {
            return None;
        }
        let mut widths = [0u8; 94];
        widths.copy_from_slice(&d[1..95]);
        let rows = d[95..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Some(Font { h, widths, rows })
    }

    /// The width a space takes: the first glyph's ('!') plus one, as the
    /// original's text routines advance (0000:9EA6, the width 0000:66F6).
    fn space(&self) -> usize {
        self.widths[0] as usize + 1
    }

    /// The width of `c` and its glyph's index (None: a space or a
    /// character the font hasn't).
    fn glyph(&self, c: u8) -> (usize, Option<usize>) {
        match c {
            0x21..=0x7E => {
                let k = (c - 0x21) as usize;
                (self.widths[k] as usize, Some(k))
            }
            _ => (self.space(), None),
        }
    }

    /// The width of a line of plain text.
    pub fn width(&self, s: &str) -> usize {
        s.bytes().map(|c| self.glyph(c).0).sum()
    }
}

/// A run of text in one colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub colour: u8,
    pub text: String,
}

/// The game's text at DS:`at` split into lines of coloured spans: `\t`
/// and a byte set the colour, `\t` and three digits a column (the lines
/// of the shops' lists: kept as a span `\tNNN` of its own, which a list
/// draws at x NNN and anything else as two spaces), `\r` (or `\n`) ends
/// a line.
pub fn lines(bytes: &[u8], colour: u8) -> Vec<Vec<Span>> {
    let mut out = vec![Vec::new()];
    let mut colour = colour;
    let mut cur = String::new();
    let flush = |out: &mut Vec<Vec<Span>>, cur: &mut String, colour: u8| {
        if !cur.is_empty() {
            out.last_mut().unwrap().push(Span { colour, text: std::mem::take(cur) });
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\t' if bytes.get(i + 1..i + 4).is_some_and(|d| d.iter().all(u8::is_ascii_digit)) => {
                flush(&mut out, &mut cur, colour);
                let col = String::from_utf8_lossy(&bytes[i..i + 4]).into_owned();
                out.last_mut().unwrap().push(Span { colour, text: col });
                i += 3;
            }
            b'\t' => {
                flush(&mut out, &mut cur, colour);
                if let Some(&c) = bytes.get(i + 1) {
                    colour = c;
                }
                i += 1;
            }
            b'\r' | b'\n' => {
                flush(&mut out, &mut cur, colour);
                out.push(Vec::new());
                // \r\n is one break.
                if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            c => cur.push(c as char),
        }
        i += 1;
    }
    flush(&mut out, &mut cur, colour);
    while out.len() > 1 && out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

/// The plain text of a line (a column two spaces, after the first span).
pub fn plain(line: &[Span]) -> String {
    let mut out = String::new();
    for s in line {
        match column(s) {
            Some(_) if out.is_empty() => {}
            Some(_) => out.push_str("  "),
            None => out.push_str(&s.text),
        }
    }
    out
}

/// A span's list column (`\tNNN`), if it is one.
pub fn column(s: &Span) -> Option<i32> {
    let t = s.text.strip_prefix('\t')?;
    (t.len() == 3 && t.bytes().all(|c| c.is_ascii_digit())).then(|| t.parse().unwrap())
}

/// The layer: RGBA, (0, 0, 0, 0) where nothing is drawn.
pub struct Screen {
    pub px: Vec<[u8; 4]>,
    palette: [[u8; 4]; 256],
}

impl Screen {
    pub fn new(palette: [[u8; 4]; 256]) -> Screen {
        Screen { px: vec![[0; 4]; W * H], palette }
    }

    pub fn clear(&mut self) {
        self.px.fill([0; 4]);
    }

    /// The colours from now on (the character sheet's screens have their
    /// own, CHARSHT.COL).
    pub fn set_palette(&mut self, palette: [[u8; 4]; 256]) {
        self.palette = palette;
    }

    /// A rectangle in palette colour `c`, `alpha` opaque.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8, alpha: u8) {
        let p = self.palette[c as usize];
        for yy in y.max(0)..(y + h).min(H as i32) {
            for xx in x.max(0)..(x + w).min(W as i32) {
                self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], alpha];
            }
        }
    }

    /// The pixels drawn in a rectangle darkened (the list's picked line,
    /// which the original recolours through a table, 3159:0BE4).
    pub fn darken(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for yy in y.max(0)..(y + h).min(H as i32) {
            for xx in x.max(0)..(x + w).min(W as i32) {
                let p = &mut self.px[yy as usize * W + xx as usize];
                for c in &mut p[..3] {
                    *c = (*c as u16 * 3 / 5) as u8;
                }
            }
        }
    }

    /// Palette pixels (`w` x `h`, row-major) with their top left at
    /// (x, y); colour 0 is see-through, rows below `bottom` are cut.
    pub fn blit(&mut self, x: i32, y: i32, w: usize, h: usize, px: &[u8], bottom: i32) {
        for r in 0..h as i32 {
            let yy = y + r;
            if yy < 0 || yy > bottom || yy >= H as i32 {
                continue;
            }
            for c in 0..w as i32 {
                let xx = x + c;
                if !(0..W as i32).contains(&xx) {
                    continue;
                }
                let v = *px.get((r * w as i32 + c) as usize).unwrap_or(&0);
                if v != 0 {
                    let p = self.palette[v as usize];
                    self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], 0xFF];
                }
            }
        }
    }

    /// Text at (x, y) in colour `c`, without a shadow.
    pub fn text_plain(&mut self, font: &Font, x: i32, y: i32, s: &str, c: u8) {
        self.text_raw(font, x, y, s, c);
    }

    /// A box: a dark fill with a one-pixel frame in colour `frame`.
    pub fn panel(&mut self, x: i32, y: i32, w: i32, h: i32, frame: u8) {
        self.fill(x, y, w, h, 0, 0xE0);
        self.fill(x, y, w, 1, frame, 0xFF);
        self.fill(x, y + h - 1, w, 1, frame, 0xFF);
        self.fill(x, y, 1, h, frame, 0xFF);
        self.fill(x + w - 1, y, 1, h, frame, 0xFF);
    }

    /// Plain text at (x, y), its top left, in colour `c`, with a shadow
    /// one pixel down and right in colour 0.
    pub fn text(&mut self, font: &Font, x: i32, y: i32, s: &str, c: u8) {
        self.text_raw(font, x + 1, y + 1, s, 0);
        self.text_raw(font, x, y, s, c);
    }

    fn text_raw(&mut self, font: &Font, mut x: i32, y: i32, s: &str, c: u8) {
        let p = self.palette[c as usize];
        for ch in s.bytes() {
            let (w, g) = font.glyph(ch);
            if let Some(k) = g {
                for r in 0..font.h {
                    let bits = font.rows.get(k * font.h + r).copied().unwrap_or(0);
                    for i in 0..16 {
                        if bits & (0x8000 >> i) == 0 {
                            continue;
                        }
                        let (xx, yy) = (x + i, y + r as i32);
                        if (0..W as i32).contains(&xx) && (0..H as i32).contains(&yy) {
                            self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], 0xFF];
                        }
                    }
                }
            }
            x += w as i32;
        }
    }

    /// A line of spans at (x, y) (a column two spaces).
    pub fn spans(&mut self, font: &Font, mut x: i32, y: i32, line: &[Span]) {
        for (i, s) in line.iter().enumerate() {
            if column(s).is_some() {
                if i > 0 {
                    x += font.width("  ") as i32;
                }
                continue;
            }
            self.text(font, x, y, &s.text, s.colour);
            x += font.width(&s.text) as i32;
        }
    }

    /// A line of spans at (x, y) as the original's routines draw it: no
    /// shadow (0000:9E0A); a column (`\tNNN`) moves x to NNN + 1 (2F26:2E12
    /// draws each part at its x plus one) when `columns`, else is two
    /// spaces; colour `default` replaces the dialog's default 0Fh.
    fn spans_game(&mut self, font: &Font, mut x: i32, y: i32, line: &[Span], columns: bool, default: u8) {
        let start = x;
        for (i, s) in line.iter().enumerate() {
            if let Some(c) = column(s) {
                if columns {
                    // `\t000`: a part without a column, back at the start.
                    x = if c == 0 { start } else { c + 1 };
                } else if i > 0 {
                    x += font.width("  ") as i32;
                }
                continue;
            }
            let c = if s.colour == 0x0F { default } else { s.colour };
            self.text_raw(font, x, y, &s.text, c);
            x += font.width(&s.text) as i32;
        }
    }

    /// A box of the game's tiles (0000:9727): the nine tiles, corners once,
    /// edges and the middle repeated, `cols` x `rows` of them from (x, y);
    /// colour 0 see-through (9A34).
    fn tiles(&mut self, t: &Tiles, x: i32, y: i32, cols: i32, rows: i32) {
        for r in 0..rows {
            let tr = if r == 0 { 0 } else if r == rows - 1 { 2 } else { 1 };
            for c in 0..cols {
                let tc = if c == 0 { 0 } else if c == cols - 1 { 2 } else { 1 };
                let k = (tr * 3 + tc) as usize;
                let px = &t.px[k * t.w * t.h..(k + 1) * t.w * t.h];
                self.blit(x + c * t.w as i32, y + r * t.h as i32, t.w, t.h, px, H as i32);
            }
        }
    }
}

/// What the layer shows this frame.
#[derive(Clone, Debug, Default)]
pub struct Hud {
    /// The game's message line (061A), while it counts down.
    pub message: Option<Vec<Vec<Span>>>,
    /// A message box waiting to be read, its lines.
    pub boxed: Option<Vec<Vec<Span>>>,
    /// A menu waiting for a pick: its title's lines, its entries, the
    /// entry highlighted.
    pub menu: Option<Menu>,
    /// Hours to type: the prompt and what has been typed.
    pub typing: Option<(String, String)>,
    /// A status line at the bottom (the rest's hours).
    pub status: Option<String>,
    /// The level's text shown (`level_text`): each line with its row.
    pub level: Vec<(i32, Vec<Span>)>,
    /// The window the game centres its boxes in (A914, A916, A918, A91A:
    /// left, top, right, bottom); all zero: the whole screen.
    pub clip: (i32, i32, i32, i32),
}

/// The level's text while it is shown (0000:0550, through 05B5): the text
/// at [3D42] in the EMS frame with the level's bank (C8FE) mapped, while
/// its time (3C82) lasts. Its lines end at a CR; a line beginning with `:`
/// ends it (the riddle's answers follow); `%` and the three bytes after it
/// become the name at 01AD, read in the frame's segment as the original
/// reads it (DS is still the frame there). The first line's row is the
/// view's bottom (C3A2) less the text's height (3D40), each next 0Ah
/// lower, in colour F0h, centred at x A0h.
pub fn level_text(m: &arena::machine::Machine) -> Vec<(i32, Vec<Span>)> {
    use arena::mem::DS;
    use arena::squares::var::{SHOWN, SHOWN_HEIGHT, SHOWN_TIME};
    if m.mem.r8(DS, SHOWN_TIME) == 0 {
        return Vec::new();
    }
    let mut c = m.clone();
    let bank = c.mem.r8(DS, 0xC8FE) as u16;
    c.ems_map(bank);
    let seg = c.mem.r16(DS, 0x9030);
    let mut si = c.mem.r16(DS, SHOWN);
    let mut y = c.mem.r16(DS, 0xC3A2).wrapping_sub(c.mem.r16(DS, SHOWN_HEIGHT)) as i16 as i32;
    let mut out = Vec::new();
    let mut line = Vec::new();
    for _ in 0..0x1000 {
        let ch = c.mem.r8(seg, si);
        si = si.wrapping_add(1);
        match ch {
            b':' if matches!(c.mem.r8(seg, si.wrapping_sub(2)), 0 | 0x0D) => return out,
            b'%' => {
                si = si.wrapping_add(3);
                line.extend_from_slice(c.mem.cstr(seg, 0x01AD));
            }
            0 => {
                out.extend(lines(&line, 0xF0).into_iter().map(|l| (y, l)));
                return out;
            }
            0x0D => {
                out.extend(lines(&line, 0xF0).into_iter().map(|l| (y, l)));
                line.clear();
                y += 0x0A;
            }
            _ => line.push(ch),
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub title: Vec<Vec<Span>>,
    pub entries: Vec<Vec<Span>>,
    pub keys: Vec<u8>,
    pub selected: usize,
    /// A list (1194:5734: a panel with a scrolling text area), not a menu
    /// (1194:020E: a column of boxes).
    pub list: bool,
}

/// Where a menu's entries are drawn: (x, y of the first, line height,
/// width), for the mouse to pick them.
pub fn menu_layout(font: &Font, menu: &Menu) -> (i32, i32, i32, i32) {
    let lh = font.h as i32 + 2;
    let w = menu
        .title
        .iter()
        .chain(menu.entries.iter())
        .map(|l| font.width(&plain(l)) as i32)
        .max()
        .unwrap_or(0)
        + 16;
    let w = w.clamp(80, W as i32 - 8);
    let h = (menu.title.len() + menu.entries.len()) as i32 * lh + 10;
    let x = (W as i32 - w) / 2;
    let y = ((H as i32 - h) / 2).max(4);
    (x + 8, y + 5 + menu.title.len() as i32 * lh, lh, w - 16)
}

/// A set of the game's box tiles (0000:756C's records, as 21EE:009A loads
/// them): nine `w` x `h` tiles one after another.
pub struct Tiles {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Tiles {
    fn load(archive: &crate::archive::Archive, name: &str, w: usize, h: usize) -> Option<Tiles> {
        let px = archive.read(name)?;
        (px.len() >= 9 * w * h).then_some(Tiles { w, h, px })
    }
}

/// What the game's texts are drawn with (`init_style`).
pub struct Style {
    /// FONT_A.DAT: the talk's and the shops' (1194:DBF1, B978, C280, F4D8
    /// select it with MARBLE.CIF), for menus and lists here.
    menu_font: Font,
    /// ARENAFNT.DAT: the message boxes' (1194:42EE and most callers).
    box_font: Font,
    /// TEENYFNT.DAT: the item lists' (the store's, 1194:F7DA and F857
    /// before 5734; the guild's, A9F0; the loot window's). The tavern's
    /// (BB8C) is ARENAFNT; all lists have this one here.
    list_font: Font,
    /// MARBLE.CIF, 3 x 3 tiles ([1036], 21EE:00BC).
    tiles: Tiles,
    /// The lists' panels: the store's (kinds 1 and 2) and the others'
    /// (kind 11: the tavern, the guild, the loot window).
    wide: ListKind,
    armour: ListKind,
    narrow: ListKind,
}

/// A kind of list as 1194:5734 sets it up: its panel (where its header
/// puts it) and its record's text area and scroll track (x, y, w, h).
struct ListKind {
    panel: Option<crate::interface::Image>,
    area: (i32, i32, i32, i32),
    track: (i32, i32, i32, i32),
    /// How a line's last part is drawn: the record's flags (+20h) pick
    /// the line's routine (3159:0718, the table at B7FE: 0000:9E06 at the
    /// area's x plus one, 5364 centred on its middle with 1000h, 9370
    /// ending at its right edge with 2000h).
    last: Align,
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

static STYLE: std::sync::OnceLock<Style> = std::sync::OnceLock::new();

/// Loads the game's fonts, box tiles and list panel for the texts drawn
/// from then on (without them the layer's own plain boxes are drawn).
pub fn init_style(archive: &crate::archive::Archive, m: &arena::machine::Machine) {
    let (Some(menu_font), Some(box_font), Some(list_font), Some(tiles)) =
        (Font::load(archive, "FONT_A.DAT"), Font::load(archive, "ARENAFNT.DAT"), Font::load(archive, "TEENYFNT.DAT"), Tiles::load(archive, "MARBLE.CIF", 3, 3))
    else {
        return;
    };
    // Kind 1 (the store's weapons, 1194:F7E9; its armour, kind 2, has
    // POPUP4.IMG on the same record): POPUP3.IMG, the record at 50B0: the
    // text from 1Ch, 1Bh, 109h x 4Ch, the track 0Dh, 14h, 6 x 52h. Kind 11
    // (1194:A9F9, BB9E, the loot window's): NEWPOP.IMG, the record at
    // 5006: the text from 54h, 21h, 96h x 3Ch, the track 44h, 1Eh, 6 x 3Ch.
    let (area, track) = ((0x1C, 0x1B, 0x109, 0x4C), (0x0D, 0x14, 6, 0x52));
    // The record at 50B0 has flags 6000h: its lines end at the right.
    let wide = ListKind { panel: crate::panels::load_image(archive, m, "POPUP3.IMG"), area, track, last: Align::Right };
    let armour = ListKind { panel: crate::panels::load_image(archive, m, "POPUP4.IMG"), area, track, last: Align::Right };
    // 5006: 4000h, its lines from the left.
    let narrow = ListKind { panel: crate::panels::load_image(archive, m, "NEWPOP.IMG"), area: (0x54, 0x21, 0x96, 0x3C), track: (0x44, 0x1E, 6, 0x3C), last: Align::Left };
    let _ = STYLE.set(Style { menu_font, box_font, list_font, tiles, wide, armour, narrow });
}

/// The kind a list is shown as: the hooks don't pass 1194:5734's AX, so
/// the store's lines (by their first column: the weapons' format at
/// 86BD from x 31, kind 1; armour's at 86DD from x 29, kind 2) choose
/// those, any other list is kind 11's (the tavern's, BB9E; the guild's,
/// A9F9; the loot window's).
fn list_kind<'a>(st: &'a Style, menu: &Menu) -> &'a ListKind {
    match menu.entries.first().and_then(|e| e.iter().find_map(column)) {
        Some(29) => &st.armour,
        Some(31) => &st.wide,
        _ => &st.narrow,
    }
}

/// The margin of the message boxes and the menus' boxes (DX 5 at
/// 1194:59FE and 0247).
const MARGIN: i32 = 5;

/// A box's size as 0000:965A sizes it (98A0, 9798): the widest line
/// (0000:66F6) and the lines (the font's height plus one each) with twice
/// the margin round them, then rounded up to whole tiles, half the extra
/// moving the text. (width, height, the text's x and y in the box.)
fn box_size(st: &Style, font: &Font, lines: &[Vec<Span>]) -> (i32, i32, i32, i32) {
    let (tw, th) = (st.tiles.w as i32, st.tiles.h as i32);
    let text_w = lines.iter().map(|l| font.width(&plain(l)) as i32).max().unwrap_or(0);
    let (mut w, mut h) = (text_w + 2 * MARGIN, lines.len() as i32 * (font.h as i32 + 1) + 2 * MARGIN);
    let rw = (tw - w % tw) % tw;
    let rh = (th - h % th) % th;
    w += rw;
    h += rh;
    (w, h, MARGIN + rw / 2, MARGIN + (rh + 1) / 2)
}

/// A box of `w` x `h` at (x, y) with `lines` in it, as 965A draws them:
/// a line ended by a CR (here all but the last) centred on the box, the
/// last from the text's x.
#[allow(clippy::too_many_arguments)]
fn draw_box(screen: &mut Screen, st: &Style, font: &Font, (x, y, w, h): (i32, i32, i32, i32), lines: &[Vec<Span>], colour: u8) {
    screen.tiles(&st.tiles, x, y, w / st.tiles.w as i32, h / st.tiles.h as i32);
    let (_, _, tx, ty) = box_size(st, font, lines);
    for (i, l) in lines.iter().enumerate() {
        let lw = font.width(&plain(l)) as i32;
        let lx = if i + 1 < lines.len() { x + w / 2 - lw / 2 } else { x + tx };
        screen.spans_game(font, lx, y + ty + i as i32 * (font.h as i32 + 1), l, false, colour);
    }
}

/// The window boxes are centred in; the whole screen when unset.
fn clip(hud: &Hud) -> (i32, i32, i32, i32) {
    if hud.clip.2 > hud.clip.0 && hud.clip.3 > hud.clip.1 { hud.clip } else { (0, 0, W as i32 - 1, H as i32 - 1) }
}

/// Where a message box goes (965A with CX 4007h: centred in the clip),
/// or at the clip's top when a menu is up with it (the view shows both).
fn boxed_rect(st: &Style, hud: &Hud) -> Option<(i32, i32, i32, i32)> {
    let lines = hud.boxed.as_ref()?;
    let (w, h, _, _) = box_size(st, &st.box_font, lines);
    let c = clip(hud);
    let x = (c.0 + c.2) / 2 - w / 2;
    let y = match &hud.menu {
        // Both shown (a shot's last box with the question left; the
        // window shows a box before its menu): the box only when the
        // menu fits below it, the menu otherwise.
        Some(menu) if !menu.list => {
            if c.1 + 4 + h + 2 + menu_height(st, menu) > H as i32 {
                return None;
            }
            c.1 + 4
        }
        Some(_) => c.1 + 4,
        None => (c.1 + c.3) / 2 - h / 2,
    };
    Some((x, y, w, h))
}

/// A menu's boxes' height together (`menu_boxes`).
fn menu_height(st: &Style, menu: &Menu) -> i32 {
    let font = &st.menu_font;
    let title = menu_title(st, menu);
    let t = if title.is_empty() { 0 } else { box_size(st, font, &title).1 };
    t + menu.entries.iter().map(|e| box_size(st, font, std::slice::from_ref(e)).1).sum::<i32>()
}

/// A menu's boxes as 3159:0F2A lays them out: one for the title (all its
/// lines) and one for each entry, all as wide as the widest (its flag 8:
/// FS), each as high as its own lines, stacked down from y 64h less half
/// their height, centred across the clip (965A's flag 2). Below a
/// message box shown with it. The title's box (None without one) and the
/// entries'.
#[allow(clippy::type_complexity)]
fn menu_boxes(st: &Style, menu: &Menu, hud: &Hud) -> (Option<(i32, i32, i32, i32)>, Vec<(i32, i32, i32, i32)>) {
    let font = &st.menu_font;
    let title_lines = menu_title(st, menu);
    let title = (!title_lines.is_empty()).then(|| box_size(st, font, &title_lines));
    let sizes: Vec<_> = menu.entries.iter().map(|e| box_size(st, font, std::slice::from_ref(e))).collect();
    let w = sizes.iter().chain(title.iter()).map(|b| b.0).max().unwrap_or(0);
    let total = title.map_or(0, |t| t.1) + sizes.iter().map(|b| b.1).sum::<i32>();
    let mut y = 0x64 - total / 2;
    if let Some(b) = boxed_rect(st, hud) {
        y = y.max(b.1 + b.3 + 2);
    }
    let c = clip(hud);
    let x = (c.0 + c.2) / 2 - w / 2;
    let mut next = |h: i32| {
        let r = (x, y, w, h);
        y += h;
        r
    };
    let title = title.map(|t| next(t.1));
    let entries = sizes.iter().map(|b| next(b.1)).collect();
    (title, entries)
}

/// A menu's title lines as shown: when a line is wider than the screen
/// allows (0F2A gives up on a box wider than 140h; the game's own texts
/// come broken to fit, the view's were broken for ARENAFNT), the lines
/// as one text broken again at spaces, in the first span's colour.
fn menu_title(st: &Style, menu: &Menu) -> Vec<Vec<Span>> {
    let font = &st.menu_font;
    let max = W as i32 - 8 - 2 * MARGIN - 2 * st.tiles.w as i32;
    if menu.title.iter().all(|l| font.width(&plain(l)) as i32 <= max) {
        return menu.title.clone();
    }
    let colour = menu.title.iter().flatten().next().map_or(0x0F, |s| s.colour);
    let text = menu.title.iter().map(|l| plain(l)).collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    {
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let next = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if font.width(&next) as i32 > max && !cur.is_empty() {
                out.push(vec![Span { colour, text: std::mem::take(&mut cur) }]);
                cur = word.to_string();
            } else {
                cur = next;
            }
        }
        if !cur.is_empty() {
            out.push(vec![Span { colour, text: cur }]);
        }
    }
    out
}

/// A list's first line shown and how many fit (3159:0778: the area's
/// height over the font's, B7F4).
fn list_rows(st: &Style, menu: &Menu) -> (usize, usize) {
    let rows = (list_kind(st, menu).area.3 / st.list_font.h as i32).max(1) as usize;
    let top = (menu.selected + 1).saturating_sub(rows).min(menu.entries.len().saturating_sub(rows));
    (top, rows)
}

/// The entry under (x, y) of the 320 x 200 screen, laid out as drawn.
pub fn menu_pick(font: &Font, hud: &Hud, x: i32, y: i32) -> Option<usize> {
    let menu = hud.menu.as_ref()?;
    let inside = |r: (i32, i32, i32, i32)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
    let Some(st) = STYLE.get() else {
        let (ex, ey, lh, ew) = menu_layout(font, menu);
        let i = ((y - ey) / lh.max(1)) as usize;
        return (x >= ex - 3 && x < ex + ew + 3 && y >= ey && i < menu.entries.len()).then_some(i);
    };
    if menu.list {
        let (top, rows) = list_rows(st, menu);
        let area = list_kind(st, menu).area;
        if !inside(area) {
            return None;
        }
        let i = top + ((y - area.1 - 1).max(0) / st.list_font.h as i32) as usize;
        return (i < menu.entries.len() && i < top + rows).then_some(i);
    }
    menu_boxes(st, menu, hud).1.iter().position(|&r| inside(r))
}

/// `hud`'s texts in the game's style: its boxes (0000:965A), menus
/// (3159:0F2A), lists (3159:06A0 with 2F26:2E12) and prompts.
fn compose_game(screen: &mut Screen, font: &Font, st: &Style, hud: &Hud) {
    let lh = font.h as i32 + 2;
    for (y, l) in &hud.level {
        let w = font.width(&plain(l)) as i32;
        screen.spans(font, 0xA0 - w / 2, *y, l);
    }
    if let Some(lines) = &hud.message {
        let mut y = 4;
        for l in lines {
            let w = font.width(&plain(l)) as i32;
            screen.spans(font, (W as i32 - w) / 2, y, l);
            y += lh;
        }
    }
    let c = clip(hud);
    if let Some(s) = &hud.status {
        let lines = vec![vec![Span { colour: 0x0F, text: s.clone() }]];
        let (w, h, _, _) = box_size(st, &st.box_font, &lines);
        draw_box(screen, st, &st.box_font, ((c.0 + c.2) / 2 - w / 2, c.3 - h - 2, w, h), &lines, 0x0F);
    }
    if let (Some(lines), Some(r)) = (&hud.boxed, boxed_rect(st, hud)) {
        draw_box(screen, st, &st.box_font, r, lines, 0x60);
    }
    if let Some(menu) = &hud.menu {
        if menu.list {
            draw_list(screen, st, menu);
        } else {
            let (title, entries) = menu_boxes(st, menu, hud);
            if let Some(t) = title {
                draw_box(screen, st, &st.menu_font, t, &menu_title(st, menu), 0x60);
            }
            for (i, (r, e)) in entries.iter().zip(&menu.entries).enumerate() {
                draw_box(screen, st, &st.menu_font, *r, std::slice::from_ref(e), 0x60);
                // The original has no highlight (picks are clicks or the
                // keys' letters); the view marks the entry the arrows are on.
                if i == menu.selected {
                    let (x, y, w, h) = *r;
                    for (fx, fy, fw, fh) in [(x + 2, y + 2, w - 4, 1), (x + 2, y + h - 3, w - 4, 1), (x + 2, y + 2, 1, h - 4), (x + w - 3, y + 2, 1, h - 4)] {
                        screen.fill(fx, fy, fw, fh, 0x60, 0xFF);
                    }
                }
            }
        }
    }
    if let Some((prompt, typed)) = &hud.typing {
        let lines = vec![vec![Span { colour: 0x0F, text: prompt.clone() }], vec![Span { colour: 0x0F, text: format!("{typed}_") }]];
        let (w, h, _, _) = box_size(st, &st.box_font, &lines);
        draw_box(screen, st, &st.box_font, ((c.0 + c.2) / 2 - w / 2, (c.1 + c.3) / 2 - h / 2, w, h), &lines, 0x0F);
    }
}

/// A list's line as 2F26:2DF6 draws it: each part but the last (ended by
/// a `\n`) by 2E12 at its column plus one (`\tNNN`; the area's x plus one
/// without), in FDh (the record's +28h) unless it sets its own colour;
/// the last part by the line's routine (`ListKind::last`), its own column
/// unread (2E12 hands back the x it was given).
fn list_line(screen: &mut Screen, font: &Font, kind: &ListKind, y: i32, line: &[Span]) {
    let area = kind.area;
    // The parts: each with its column (None: none).
    let mut parts: Vec<(Option<i32>, Vec<Span>)> = vec![(None, Vec::new())];
    for s in line {
        match column(s) {
            Some(c) => {
                if !parts.last().unwrap().1.is_empty() {
                    parts.push((None, Vec::new()));
                }
                parts.last_mut().unwrap().0 = (c != 0).then_some(c);
            }
            None => parts.last_mut().unwrap().1.push(s.clone()),
        }
    }
    let n = parts.len();
    for (i, (col, spans)) in parts.into_iter().enumerate() {
        let w = spans.iter().map(|s| font.width(&s.text) as i32).sum::<i32>();
        let x = if i + 1 < n {
            col.unwrap_or(area.0) + 1
        } else {
            match kind.last {
                Align::Left => area.0 + 1,
                Align::Right => area.0 + area.2 - w,
            }
        };
        screen.spans_game(font, x, y, &spans, false, 0xFD);
    }
}

/// A list as 1194:5734 shows it: its kind's panel where its header puts
/// it; the lines in the text area from its top plus one, a font's height
/// apart, in colour FDh (the record's +28h), from the area's x plus one
/// and a line's parts at their columns (2F26:2E12); the line picked
/// recoloured (3159:0BE4 maps it through the table at BA0E; darkened
/// here); the scroll track's thumb in F5h with its edges F9h and F2h
/// (3159:0CFF), as high as the part shown, at least 3. A title (the
/// lists have none of their own; the view's callers may give one) above
/// the area, centred.
fn draw_list(screen: &mut Screen, st: &Style, menu: &Menu) {
    let kind = list_kind(st, menu);
    if let Some(p) = &kind.panel {
        screen.blit(p.x, p.y, p.w, p.h, &p.px, H as i32);
    }
    let area = kind.area;
    let font = &st.list_font;
    let fh = font.h as i32;
    let (top, rows) = list_rows(st, menu);
    for (i, l) in menu.title.iter().enumerate() {
        let w = font.width(&plain(l)) as i32;
        screen.spans_game(font, 0xA0 - w / 2, area.1 - fh - 4 + i as i32 * (fh + 1), l, false, 0x60);
    }
    for (k, e) in menu.entries.iter().enumerate().skip(top).take(rows) {
        let y = area.1 + 1 + (k - top) as i32 * fh;
        if k == menu.selected {
            screen.darken(area.0, y, area.2, fh);
        }
        list_line(screen, font, kind, y, e);
    }
    let n = menu.entries.len();
    if n > rows {
        let (tx, ty, tw, th) = kind.track;
        let h = (th * rows as i32 / n as i32).max(3);
        let y = ty + (th - h) * top as i32 / (n - rows).max(1) as i32;
        screen.fill(tx, y, tw, h, 0xF5, 0xFF);
        screen.fill(tx, y, tw, 1, 0xF9, 0xFF);
        screen.fill(tx, y, 1, h, 0xF9, 0xFF);
        screen.fill(tx, y + h - 1, tw, 1, 0xF2, 0xFF);
        screen.fill(tx + tw - 1, y, 1, h, 0xF2, 0xFF);
    }
}

/// `hud`'s texts drawn over what the layer has (the interface, drawn
/// after the layer is cleared): in the game's style once `init_style`
/// has loaded it, else in plain boxes.
pub fn compose_over(screen: &mut Screen, font: &Font, hud: &Hud) {
    if let Some(st) = STYLE.get() {
        return compose_game(screen, font, st, hud);
    }
    let lh = font.h as i32 + 2;
    for (y, l) in &hud.level {
        let w = font.width(&plain(l)) as i32;
        screen.spans(font, 0xA0 - w / 2, *y, l);
    }
    if let Some(lines) = &hud.message {
        let mut y = 4;
        for l in lines {
            let w = font.width(&plain(l)) as i32;
            screen.spans(font, (W as i32 - w) / 2, y, l);
            y += lh;
        }
    }
    if let Some(s) = &hud.status {
        let w = font.width(s) as i32;
        screen.panel((W as i32 - w) / 2 - 6, H as i32 - lh - 8, w + 12, lh + 4, 0x0F);
        screen.text(font, (W as i32 - w) / 2, H as i32 - lh - 6, s, 0x0F);
    }
    let mut boxed_bottom = None;
    if let Some(lines) = &hud.boxed {
        let w = lines.iter().map(|l| font.width(&plain(l)) as i32).max().unwrap_or(0) + 16;
        let w = w.clamp(60, W as i32 - 8);
        let h = lines.len() as i32 * lh + 10;
        let x = (W as i32 - w) / 2;
        // Above a menu shown with it, else in the middle.
        let y = if hud.menu.is_some() { 8 } else { ((H as i32 - h) / 2).max(4) };
        screen.panel(x, y, w, h, 0x0F);
        for (i, l) in lines.iter().enumerate() {
            screen.spans(font, x + 8, y + 5 + i as i32 * lh, l);
        }
        boxed_bottom = Some(y + h);
    }
    if let Some(menu) = &hud.menu {
        let (ex, ey, lh, ew) = menu_layout(font, menu);
        let (mut x, mut y) = (ex - 8, ey - 5 - menu.title.len() as i32 * lh);
        if let Some(b) = boxed_bottom {
            y = y.max(b + 4);
        }
        let dy = y - (ey - 5 - menu.title.len() as i32 * lh);
        let h = (menu.title.len() + menu.entries.len()) as i32 * lh + 10;
        x = x.max(4);
        screen.panel(x, y, ew + 16, h, 0x0F);
        for (i, l) in menu.title.iter().enumerate() {
            screen.spans(font, ex, y + 5 + i as i32 * lh, l);
        }
        for (i, l) in menu.entries.iter().enumerate() {
            let ly = ey + dy + i as i32 * lh;
            if i == menu.selected {
                screen.fill(ex - 3, ly - 1, ew + 6, lh, 0x08, 0xC0);
            }
            screen.spans(font, ex, ly, l);
        }
    }
    if let Some((prompt, typed)) = &hud.typing {
        let line = format!("{prompt} {typed}_");
        let w = (font.width(&line) as i32 + 16).clamp(80, W as i32 - 8);
        let x = (W as i32 - w) / 2;
        let y = H as i32 / 2 - lh;
        screen.panel(x, y, w, lh + 10, 0x0F);
        screen.text(font, x + 8, y + 5, &line, 0x0F);
    }
}

/// The layer's pipeline: a 320 x 200 RGBA texture, nearest-sampled over
/// the whole target, alpha-blended.
pub struct Overlay {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    texture: wgpu::Texture,
}

const SHADER: &str = r#"
@group(0) @binding(0) var layer: texture_2d<f32>;
@group(0) @binding(1) var near: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return textureSample(layer, near, i.uv);
}
"#;

impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Overlay {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("hud"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hud"),
            size: wgpu::Extent3d { width: W as u32, height: H as u32, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let view = texture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("hud"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Overlay { pipeline, bind, texture }
    }

    /// Draws the layer `screen` over `target`.
    pub fn draw(&self, device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::TextureView, screen: &Screen) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&screen.px),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(W as u32 * 4), rows_per_image: Some(H as u32) },
            wgpu::Extent3d { width: W as u32, height: H as u32, depth_or_array_layers: 1 },
        );
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([enc.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_colours_and_breaks() {
        let l = lines(b"\t\xc0 CAMP\r\t\xc0C\t\xd4amp\r", 0x0F);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], vec![Span { colour: 0xC0, text: " CAMP".into() }]);
        assert_eq!(plain(&l[1]), "Camp");
        assert_eq!(l[1][1].colour, 0xD4);
    }
}
