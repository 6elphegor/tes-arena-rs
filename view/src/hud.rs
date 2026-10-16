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
    /// The font the game had selected when it drew the text (B036), for
    /// the boxes: a box is drawn in its first span's.
    pub font: GameFont,
    /// The colour byte of the `\t` code before the span: 0000:98A0 keeps
    /// it in the line it measures, and 66F6 counts it as a glyph.
    pub code: Option<u8>,
}

impl Span {
    /// 66F6's width of the colour byte before the span: a space's ('!'
    /// plus one) for 20h, none below it or from 80h, else the glyph's.
    fn code_width(&self, font: &Font) -> i32 {
        match self.code {
            Some(c @ 0x20..=0x7E) => font.glyph(c).0 as i32,
            _ => 0,
        }
    }
}

/// A line's width as 0000:98A0 measures it in a box (66F6): its text and
/// the colour bytes of its `\t` codes.
fn measured(font: &Font, line: &[Span]) -> i32 {
    line.iter().map(|s| font.width(&s.text) as i32 + s.code_width(font)).sum()
}

/// The fonts the program's start loads and the routines select by
/// copying their word to B036 (`world::fonts_load`): FONT_A.DAT ([1026]),
/// FONT_B.DAT ([1028]), ARENAFNT.DAT ([102C]), TEENYFNT.DAT ([102E]).
/// ARENAFNT where B036 holds none of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameFont {
    A,
    B,
    #[default]
    Arena,
    Teeny,
}

impl GameFont {
    /// The font B036 selects.
    pub fn selected(m: &arena::machine::Machine) -> GameFont {
        let r = |at| m.mem.r16(arena::mem::DS, at);
        let b036 = r(0xB036);
        [(0x1026, GameFont::A), (0x1028, GameFont::B), (0x102C, GameFont::Arena), (0x102E, GameFont::Teeny)]
            .into_iter()
            .find(|&(at, _)| r(at) != 0 && r(at) == b036)
            .map_or(GameFont::Arena, |(_, f)| f)
    }
}

/// A message drawn in place rather than in a box (1194:4D31's: 0000:9E06
/// at (x, y), one line, no shadow, the bytes below 20h skipped): a box's
/// lines whose first is a marker span (its colour code 0, which no text
/// can hold) with the place, the second the text. Read like a box.
pub fn placed(x: i32, y: i32, text: &[u8], colour: u8, font: GameFont) -> Vec<Vec<Span>> {
    placed_lines(x, y, 0, &[text], colour, font)
}

/// `placed` with several lines, each `step` rows under the one before
/// (1194:4E88's dialog lines: at 20h, 91h on, 9 apart).
pub fn placed_lines(x: i32, y: i32, step: i32, lines: &[&[u8]], colour: u8, font: GameFont) -> Vec<Vec<Span>> {
    let mut out = vec![vec![Span { colour, text: format!("{x},{y},{step}"), font, code: Some(0) }]];
    for l in lines {
        let text: String = l.iter().filter(|&&c| c >= 0x20).map(|&c| c as char).collect();
        out.push(vec![Span { colour, text, font, code: None }]);
    }
    out
}

/// The place, the rows between lines and the lines of a box made by
/// `placed` or `placed_lines`.
pub fn placed_of(lines: &[Vec<Span>]) -> Option<(i32, i32, i32, &[Vec<Span>])> {
    let [m] = lines.first()?.as_slice() else { return None };
    if m.code != Some(0) {
        return None;
    }
    let mut f = m.text.split(',').map(|v| v.parse::<i32>());
    let (x, y, step) = (f.next()?.ok()?, f.next()?.ok()?, f.next()?.ok()?);
    Some((x, y, step, &lines[1..]))
}

/// Draws a box made by `placed`: each line's text with no shadow.
fn draw_placed(screen: &mut Screen, st: &Style, (x, y, step, lines): (i32, i32, i32, &[Vec<Span>])) {
    for (i, l) in lines.iter().enumerate() {
        if let Some(t) = l.first() {
            screen.text_plain(st.font(t.font), x, y + step * i as i32, &t.text, t.colour);
        }
    }
}

/// `Menu::kind` of a question 1194:4E88 asks with a picture's buttons
/// (13D3 1: a shop's YESNO.IMG, the haggle's NEGOTBUT.IMG, shown at 59h,
/// 7Fh on the shop's screen): its title the dialog's lines, drawn in the
/// strip (`placed_lines`: 20h, 91h on, 9 apart, TEENYFNT, C0h, three to a
/// page), its entries the buttons, which the picture shows: picked by a
/// click on them (1194:6001: x 59h..E1h, y 7Fh..8Bh; the button by x, to
/// 88h, B5h, E2h).
pub const DIALOG_BUTTONS: u16 = 0xFFFE;

/// A `DIALOG_BUTTONS` question: `lines` (the text 4E54 split, at most
/// three shown) and the buttons' names.
pub fn dialog_buttons(lines: &[&[u8]], buttons: &[&str]) -> Menu {
    let shown = &lines[..lines.len().min(3)];
    Menu {
        title: placed_lines(0x20, 0x91, 9, shown, 0xC0, GameFont::Teeny),
        entries: buttons.iter().map(|b| vec![Span { colour: 0x0F, text: b.to_string(), font: Default::default(), code: None }]).collect(),
        keys: buttons.iter().map(|b| b.as_bytes()[0]).collect(),
        selected: 0,
        list: false,
        kind: DIALOG_BUTTONS,
        font: None,
    }
}

/// The button of a `DIALOG_BUTTONS` question at (x, y) (1194:6001).
fn dialog_button_at(menu: &Menu, x: i32, y: i32) -> Option<usize> {
    if !(0x59..=0xE1).contains(&x) || !(0x7F..=0x8B).contains(&y) {
        return None;
    }
    let i = [0x88, 0xB5, 0xE2].iter().position(|&e| x <= e)?;
    (i < menu.entries.len()).then_some(i)
}

/// `lines` with every span in `font`.
pub fn in_font(mut lines: Vec<Vec<Span>>, font: GameFont) -> Vec<Vec<Span>> {
    for s in lines.iter_mut().flatten() {
        s.font = font;
    }
    lines
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
    // The colour bytes of the codes since the last span (`Span::code`).
    let mut codes: Vec<u8> = Vec::new();
    let flush = |out: &mut Vec<Vec<Span>>, cur: &mut String, colour: u8, codes: &mut Vec<u8>| {
        if !cur.is_empty() {
            let code = codes.pop();
            // Codes one after another: each but the last as an empty span
            // of its own, for its width.
            for &c in codes.iter() {
                out.last_mut().unwrap().push(Span { colour: c, text: String::new(), font: Default::default(), code: Some(c) });
            }
            codes.clear();
            out.last_mut().unwrap().push(Span { colour, text: std::mem::take(cur), font: Default::default(), code });
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\t' if bytes.get(i + 1..i + 4).is_some_and(|d| d.iter().all(u8::is_ascii_digit)) => {
                flush(&mut out, &mut cur, colour, &mut codes);
                let col = String::from_utf8_lossy(&bytes[i..i + 4]).into_owned();
                out.last_mut().unwrap().push(Span { colour, text: col, font: Default::default(), code: None });
                i += 3;
            }
            b'\t' => {
                flush(&mut out, &mut cur, colour, &mut codes);
                if let Some(&c) = bytes.get(i + 1) {
                    colour = c;
                    codes.push(c);
                }
                i += 1;
            }
            b'\r' | b'\n' => {
                flush(&mut out, &mut cur, colour, &mut codes);
                // A line its break ended: an empty span at its end (0000:965A
                // centres such a line in its box).
                for &c in codes.iter().rev().skip(1).rev() {
                    out.last_mut().unwrap().push(Span { colour: c, text: String::new(), font: Default::default(), code: Some(c) });
                }
                let code = codes.pop();
                codes.clear();
                out.last_mut().unwrap().push(Span { colour, text: String::new(), font: Default::default(), code });
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
    flush(&mut out, &mut cur, colour, &mut codes);
    while out.len() > 1 && out.last().is_some_and(|l| l.iter().all(|s| s.text.is_empty())) {
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

/// The layer: RGBA, (0, 0, 0, 0) where nothing is drawn, and the palette
/// colour each drawn pixel has (`NONE` where nothing is), for recolouring
/// through a table as the original's 0000:9D84 recolours the screen.
pub struct Screen {
    pub px: Vec<[u8; 4]>,
    idx: Vec<u16>,
    palette: [[u8; 4]; 256],
}

/// No palette colour: nothing drawn there.
const NONE: u16 = 0x100;

impl Screen {
    pub fn new(palette: [[u8; 4]; 256]) -> Screen {
        Screen { px: vec![[0; 4]; W * H], idx: vec![NONE; W * H], palette }
    }

    pub fn clear(&mut self) {
        self.px.fill([0; 4]);
        self.idx.fill(NONE);
    }

    /// Each pixel's palette colour, row by row; 100h where nothing is
    /// drawn (for the screen check).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn indices(&self) -> &[u16] {
        &self.idx
    }

    /// The colours drawn with (for the screen check).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn palette(&self) -> &[[u8; 4]; 256] {
        &self.palette
    }

    /// The colours from now on (the character sheet's screens have their
    /// own, CHARSHT.COL).
    pub fn set_palette(&mut self, palette: [[u8; 4]; 256]) {
        self.palette = palette;
    }

    /// Palette entry `c` changed, the pixels drawn in it with it (the
    /// DAC's colour set over what is on the screen).
    pub fn set_colour(&mut self, c: u8, rgb: [u8; 4]) {
        self.palette[c as usize] = rgb;
        for (p, &i) in self.px.iter_mut().zip(&self.idx) {
            if i == c as u16 {
                *p = [rgb[0], rgb[1], rgb[2], p[3]];
            }
        }
    }

    /// A rectangle in palette colour `c`, `alpha` opaque.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8, alpha: u8) {
        let p = self.palette[c as usize];
        for yy in y.max(0)..(y + h).min(H as i32) {
            for xx in x.max(0)..(x + w).min(W as i32) {
                self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], alpha];
                self.idx[yy as usize * W + xx as usize] = c as u16;
            }
        }
    }

    /// A rectangle's colours XORed with `c` (0000:A0D9's cursor: the
    /// block drawn and undrawn by the same XOR).
    pub fn xor(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        for yy in y.max(0)..(y + h).min(H as i32) {
            for xx in x.max(0)..(x + w).min(W as i32) {
                let at = yy as usize * W + xx as usize;
                let n = if self.idx[at] == NONE { c } else { self.idx[at] as u8 ^ c };
                let p = self.palette[n as usize];
                self.px[at] = [p[0], p[1], p[2], 0xFF];
                self.idx[at] = n as u16;
            }
        }
    }

    /// The pixels drawn in a rectangle recoloured through `table` (0000:9D84:
    /// each pixel's colour replaced by the table's entry for it).
    pub fn remap(&mut self, x: i32, y: i32, w: i32, h: i32, table: &[u8; 256]) {
        for yy in y.max(0)..(y + h).min(H as i32) {
            for xx in x.max(0)..(x + w).min(W as i32) {
                let at = yy as usize * W + xx as usize;
                let c = self.idx[at];
                if c == NONE || table[c as usize] as u16 == c {
                    continue;
                }
                let n = table[c as usize];
                let p = self.palette[n as usize];
                let a = self.px[at][3];
                self.px[at] = [p[0], p[1], p[2], a];
                self.idx[at] = n as u16;
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
                    self.idx[yy as usize * W + xx as usize] = v as u16;
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
                            self.idx[yy as usize * W + xx as usize] = c as u16;
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
    /// A typed field asked: what its caller shows around it (the field
    /// itself is drawn over the layer, `field::Live::draw`).
    pub typing: Option<crate::dialog::Prompt>,
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
    /// A list's kind (5734's AX, `Machine::list_kind`): its panel, record
    /// and colour.
    pub kind: u16,
    /// The font the game had selected when it asked (B036), which the
    /// menu's or the list's lines are drawn in; None (the view's own
    /// questions): FONT_A, a list TEENYFNT.
    pub font: Option<GameFont>,
}

/// What drawing `text` through 0000:98xx (the boxes', the menus' lines)
/// leaves in the game's state: each `\t` and the byte after it the
/// colour (9288, 996C), each `0Ch` and a number n the font of the table
/// at 91AC's n-th word (B036, 9987).
pub fn text_codes(m: &mut arena::machine::Machine, text: &[u8]) {
    use arena::mem::DS;
    let mut i = 0;
    while i + 1 < text.len() {
        match text[i] {
            9 => {
                m.mem.w8(DS, 0x9288, text[i + 1]);
                i += 2;
            }
            0x0C => {
                let n = text[i + 1].wrapping_sub(1) as u16;
                let f = m.mem.r16(DS, 0x91AC_u16.wrapping_add(n.wrapping_mul(2)));
                m.mem.w16(DS, 0xB036, f);
                i += 2;
            }
            _ => i += 1,
        }
    }
}

/// A list 1194:5734 asks (its kind `Machine::list_kind`): its title and
/// lines kept in the font selected as it is asked (B036), which 3159:06A0
/// draws them in.
pub fn list_menu(m: &arena::machine::Machine, title: Vec<Vec<Span>>, entries: Vec<Vec<Span>>) -> Menu {
    let font = GameFont::selected(m);
    Menu { title: in_font(title, font), entries: in_font(entries, font), keys: Vec::new(), selected: 0, list: true, kind: m.list_kind, font: Some(font) }
}

/// The font a list is drawn in: its lines' (`list_menu`); the bar's lists
/// (2542:06F8) in TEENYFNT.
fn list_font<'a>(st: &'a Style, menu: &Menu) -> &'a Font {
    match menu.font {
        Some(f) if menu.kind != BAR_LIST => st.font(f),
        _ => &st.list_font,
    }
}

/// The font a menu is drawn in (`Menu::font`; FONT_A by default).
fn menu_font<'a>(st: &'a Style, menu: &Menu) -> &'a Font {
    menu.font.map_or(&st.menu_font, |f| st.font(f))
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
    pub fn load(archive: &crate::archive::Archive, name: &str, w: usize, h: usize) -> Option<Tiles> {
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
    /// The lists' kinds 0..11 as 1194:5734 sets them up (any other kind
    /// is kind 1's).
    kinds: Vec<ListKind>,
    /// The list in the bar's corner (`BAR_LIST`).
    bar: ListKind,
    /// FONT_B.DAT ([1028]), for a box drawn in it.
    font_b: Option<Font>,
}

impl Style {
    /// The font a box's text selected (`Span::font`).
    fn font(&self, f: GameFont) -> &Font {
        match f {
            GameFont::A => &self.menu_font,
            GameFont::B => self.font_b.as_ref().unwrap_or(&self.box_font),
            GameFont::Arena => &self.box_font,
            GameFont::Teeny => &self.list_font,
        }
    }

    /// The font a box with `lines` is drawn in: its first span's.
    fn box_font_of(&self, lines: &[Vec<Span>]) -> &Font {
        self.font(lines.iter().flatten().next().map_or(GameFont::Arena, |s| s.font))
    }
}

/// `Menu::kind` of the list 2542:06F8 shows in the bar's bottom right
/// corner (the spellbook's, 29CF:060B with 8036 9; the items to use,
/// 2542:066A) rather than through 1194:5734: the record at DGROUP 5AF4,
/// no panel (the bar under it stays), the lines in FDh.
pub const BAR_LIST: u16 = 0xFFFF;

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
    /// The lines' colour (BL, the record's +28h).
    colour: u8,
    /// A title 5734 draws itself (kinds 6 and 10: 0000:5364 centred on
    /// x A0h at this row, in this colour), from the caller's DI.
    title: Option<(i32, u8)>,
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Centre,
    Right,
}

/// The records 1194:5734 gives 3159:06A0 (DGROUP 5006, 505B, 50B0, 5105,
/// 515A): the text area (+18h: x, y, w, h), the scroll track (+10h) and
/// the flags (+20h).
type ListRecord = ((i32, i32, i32, i32), (i32, i32, i32, i32), u16);
const RECORD_5006: ListRecord = ((0x54, 0x21, 0x96, 0x3C), (0x44, 0x1E, 6, 0x3C), 0x4000);
const RECORD_505B: ListRecord = ((0x54, 0x2D, 0x96, 0x34), (0x44, 0x1E, 6, 0x3C), 0x4000);
const RECORD_50B0: ListRecord = ((0x1C, 0x1B, 0x109, 0x4C), (0x0D, 0x14, 6, 0x52), 0x6000);
const RECORD_5105: ListRecord = ((0x48, 0x13, 0xBA, 0x52), (0x37, 0x11, 9, 0x54), 0x4000);
const RECORD_515A: ListRecord = ((0x54, 0x2D, 0x96, 0x42), (0x42, 0x1E, 6, 0x50), 0);
const RECORD_5AF4: ListRecord = ((0xDE, 0x99, 0x54, 0x2A), (0xD1, 0xA1, 7, 0x1A), 0x4000);

/// The table at BA0E (DGROUP B80E) the list's picked line is recoloured
/// through (3159:0BE4 with 0000:9D84): the identity but for the pairs at
/// 3992 that 3159:0EE2 makes it from (21EE's set-up): FDh to FCh, FEh to
/// FFh, 65h to FDh, EEh to 01h.
fn picked_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        *v = i as u8;
    }
    for w in [0xFCFDu16, 0xFFFE, 0xFD65, 0x01EE] {
        t[(w & 0xFF) as usize] = (w >> 8) as u8;
    }
    t
}

static STYLE: std::sync::OnceLock<Style> = std::sync::OnceLock::new();

/// Loads the game's fonts, box tiles and list panel for the texts drawn
/// from then on (without them the layer's own plain boxes are drawn).
pub fn init_style(archive: &crate::archive::Archive, m: &arena::machine::Machine) {
    crate::field::init(archive);
    let (Some(menu_font), Some(box_font), Some(list_font), Some(tiles)) =
        (Font::load(archive, "FONT_A.DAT"), Font::load(archive, "ARENAFNT.DAT"), Font::load(archive, "TEENYFNT.DAT"), Tiles::load(archive, "MARBLE.CIF", 3, 3))
    else {
        return;
    };
    // 1194:5734 by AX: the panel (its name's offset), the record and BL
    // (the lines' colour); kinds 3 and 5 set the record's 4000h (it has
    // it), kinds 6 and 10 draw the caller's title.
    let kind = |name: &str, rec: ListRecord, colour: u8, title: Option<(i32, u8)>| {
        let (area, track, flags) = rec;
        let last = if flags & 0x2000 != 0 { Align::Right } else if flags & 0x1000 != 0 { Align::Centre } else { Align::Left };
        ListKind { panel: (!name.is_empty()).then(|| crate::panels::load_image(archive, m, name)).flatten(), area, track, last, colour, title }
    };
    let kinds = vec![
        kind("POPUP.IMG", RECORD_5006, 0xFD, None),
        kind("POPUP3.IMG", RECORD_50B0, 0xFD, None),
        kind("POPUP4.IMG", RECORD_50B0, 0xFD, None),
        kind("POPUP.IMG", RECORD_5006, 0xD5, None),
        kind("POPUP5.IMG", RECORD_5006, 0xFD, None),
        kind("LOADSAVE.IMG", RECORD_5006, 0xFD, None),
        kind("POPUP2.IMG", RECORD_515A, 0xFD, Some((0x20, 0x01))),
        kind("POPUP7.IMG", RECORD_50B0, 0xFD, None),
        kind("POPUP8.IMG", RECORD_5006, 0xEE, None),
        kind("POPUP11.IMG", RECORD_5105, 0x65, None),
        kind("NEWPOP.IMG", RECORD_505B, 0xFD, Some((0x1F, 0x60))),
        kind("NEWPOP.IMG", RECORD_5006, 0xFD, None),
    ];
    let bar = kind("", RECORD_5AF4, 0xFD, None);
    let font_b = Font::load(archive, "FONT_B.DAT");
    let _ = STYLE.set(Style { menu_font, box_font, list_font, tiles, kinds, bar, font_b });
}

/// The kind a list is shown as: 1194:5734's AX (`Menu::kind`, from the
/// port's `list_kind`); any kind past 11 is kind 1's, as 5734's default.
fn list_kind<'a>(st: &'a Style, menu: &Menu) -> &'a ListKind {
    if menu.kind == BAR_LIST {
        return &st.bar;
    }
    st.kinds.get(menu.kind as usize).unwrap_or(&st.kinds[1])
}

/// The margin of the message boxes and the menus' boxes (DX 5 at
/// 1194:59FE and 0247).
const MARGIN: i32 = 5;

/// A box's size as 0000:965A sizes it (98A0, 9798): the widest line
/// (0000:66F6) and the lines (the font's height plus one each) with twice
/// the margin round them, then rounded up to whole tiles, half the extra
/// moving the text. (width, height, the text's x and y in the box.)
fn box_size(st: &Style, font: &Font, lines: &[Vec<Span>]) -> (i32, i32, i32, i32) {
    box_size_margin(st, font, lines, MARGIN)
}

/// `box_size` with 965A's margin (DX) `margin`.
fn box_size_margin(st: &Style, font: &Font, lines: &[Vec<Span>], margin: i32) -> (i32, i32, i32, i32) {
    box_size_in(&st.tiles, font, lines, margin)
}

/// `box_size_margin` with the tiles `tiles`.
fn box_size_in(tiles: &Tiles, font: &Font, lines: &[Vec<Span>], margin: i32) -> (i32, i32, i32, i32) {
    let (tw, th) = (tiles.w as i32, tiles.h as i32);
    let text_w = lines.iter().map(|l| measured(font, l)).max().unwrap_or(0);
    let (mut w, mut h) = (text_w + 2 * margin, lines.len() as i32 * (font.h as i32 + 1) + 2 * margin);
    let rw = (tw - w % tw) % tw;
    let rh = (th - h % th) % th;
    // The text's x (A84E) is the margin plus the box's left edge centred
    // on the width before it is rounded up to whole tiles (96F1), then
    // half the rounding (9798); the box's edge is centred on the rounded
    // width (97EE). Its y (A850) has half the rounding, up, then the
    // box's top added after (9817).
    let tx = (w + rw) / 2 - w / 2 + margin + rw / 2;
    w += rw;
    h += rh;
    (w, h, tx, margin + (rh + 1) / 2)
}

/// The box 965A draws for the text `text` with CX 7 (the town's welcome,
/// 0000:4B21): sized as `box_size` in the boxes' font, centred in the
/// clip (A914..A91A, flags 2 and 4). (x, y, width, height); None before
/// the game's style is loaded.
pub fn centred_box(m: &arena::machine::Machine, text: &[u8]) -> Option<(u16, u16, u16, u16)> {
    let st = STYLE.get()?;
    let (w, h, _, _) = box_size(st, &st.box_font, &lines(text, 0));
    let (w, h) = (w as u16, h as u16);
    let r = |at| m.mem.r16(arena::mem::DS, at);
    let x = (r(0xA918).wrapping_add(r(0xA914)) >> 1).wrapping_sub(w >> 1);
    let y = (r(0xA91A).wrapping_add(r(0xA916)) >> 1).wrapping_sub(h >> 1);
    Some((x, y, w, h))
}

/// A box of `w` x `h` at (x, y) with `lines` in it, as 965A draws them:
/// a line ended by a CR (`lines` leaves an empty span at its end) centred
/// on the box, a last one without from the text's x.
#[allow(clippy::too_many_arguments)]
fn draw_box(screen: &mut Screen, st: &Style, font: &Font, rect: (i32, i32, i32, i32), lines: &[Vec<Span>], colour: u8) {
    draw_box_in(screen, &st.tiles, font, rect, lines, MARGIN, colour);
}

/// `draw_box` with the tiles `tiles` and 965A's margin `margin`.
fn draw_box_in(screen: &mut Screen, tiles: &Tiles, font: &Font, (x, y, w, h): (i32, i32, i32, i32), lines: &[Vec<Span>], margin: i32, colour: u8) {
    screen.tiles(tiles, x, y, w / tiles.w as i32, h / tiles.h as i32);
    let (_, _, tx, ty) = box_size_in(tiles, font, lines, margin);
    for (i, l) in lines.iter().enumerate() {
        let lw = measured(font, l);
        let ended = i + 1 < lines.len() || l.last().is_some_and(|s| s.text.is_empty());
        let lx = if ended { x + w / 2 - lw / 2 } else { x + tx };
        screen.spans_game(font, lx, y + ty + i as i32 * (font.h as i32 + 1), l, false, colour);
    }
}

/// Boxes, menus and lists drawn with other tiles and fonts than play's
/// (character creation's: PARCH.CIF's 20 x 20 tiles, 1194:2520, and
/// the font each question selects), as 965A, 3159:0F2A and 1194:5734
/// draw them on the whole screen.
pub mod custom {
    use super::*;

    const SCREEN: (i32, i32, i32, i32) = (0, 0, W as i32 - 1, H as i32 - 1);

    /// A box for `lines` as 965A sizes it with margin `margin`, centred
    /// on the screen (CX 4007h, 7): (x, y, w, h).
    pub fn box_rect(tiles: &Tiles, font: &Font, lines: &[Vec<Span>], margin: i32) -> (i32, i32, i32, i32) {
        let (w, h, _, _) = box_size_in(tiles, font, lines, margin);
        ((SCREEN.0 + SCREEN.2) / 2 - w / 2, (SCREEN.1 + SCREEN.3) / 2 - h / 2, w, h)
    }

    /// That box drawn, its text in `colour` where a span has none.
    pub fn draw_box(screen: &mut Screen, tiles: &Tiles, font: &Font, lines: &[Vec<Span>], margin: i32, colour: u8) {
        let r = box_rect(tiles, font, lines, margin);
        draw_box_in(screen, tiles, font, r, lines, margin, colour);
    }

    /// A menu's boxes as 0F2A lays them out (1194:020E: margin 5, CX
    /// 4006h): the title's and the entries'.
    #[allow(clippy::type_complexity)]
    pub fn menu_rects(tiles: &Tiles, font: &Font, title: &[Vec<Span>], entries: &[Vec<Span>]) -> (Option<(i32, i32, i32, i32)>, Vec<(i32, i32, i32, i32)>) {
        menu_boxes_in(tiles, font, title, entries, SCREEN, None)
    }

    /// That menu drawn.
    pub fn draw_menu(screen: &mut Screen, tiles: &Tiles, font: &Font, title: &[Vec<Span>], entries: &[Vec<Span>], colour: u8) {
        let (t, es) = menu_rects(tiles, font, title, entries);
        if let Some(r) = t {
            draw_box_in(screen, tiles, font, r, title, MARGIN, colour);
        }
        for (r, e) in es.iter().zip(entries) {
            draw_box_in(screen, tiles, font, *r, std::slice::from_ref(e), MARGIN, colour);
        }
    }

    /// A list (5734 of `menu.kind`), its lines in the font `font`, the
    /// title it draws in `title_font`; nothing before the game's style
    /// is loaded (`init_style`).
    pub fn draw_list(screen: &mut Screen, font: &Font, title_font: &Font, menu: &Menu) {
        if let Some(st) = STYLE.get() {
            draw_list_in(screen, st, menu, font, title_font);
        }
    }

    /// The list's line under (x, y) (the area's lines shown).
    pub fn list_pick(font: &Font, menu: &Menu, x: i32, y: i32) -> Option<usize> {
        let st = STYLE.get()?;
        let (top, rows) = list_rows_in(st, menu, font);
        let a = list_kind(st, menu).area;
        if !(x >= a.0 && x < a.0 + a.2 && y >= a.1 && y < a.1 + a.3) {
            return None;
        }
        let i = top + ((y - a.1 - 1).max(0) / font.h as i32) as usize;
        (i < menu.entries.len() && i < top + rows).then_some(i)
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
    if placed_of(lines).is_some() {
        return None;
    }
    let (w, h, _, _) = box_size(st, st.box_font_of(lines), lines);
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
    let font = menu_font(st, menu);
    let title = menu_title(st, menu);
    let t = if title.is_empty() { 0 } else { box_size(st, font, &title).1 };
    t + menu.entries.iter().map(|e| box_size(st, font, std::slice::from_ref(e)).1).sum::<i32>()
}

/// A menu's boxes as 3159:0F2A lays them out: one for the title (all its
/// lines) and one for each entry, all as wide as the widest (its flag 8:
/// FS), each as high as its own lines, centred across the clip (965A's
/// flag 2). The first's top is y 64h less half their heights as 0000:9A08
/// measures them, before 965A rounds each up to whole tiles (0F6F..0FAC);
/// each next box goes under the one drawn, by its rounded height (1087).
/// Below a message box shown with it. The title's box (None without one)
/// and the entries'.
#[allow(clippy::type_complexity)]
fn menu_boxes(st: &Style, menu: &Menu, hud: &Hud) -> (Option<(i32, i32, i32, i32)>, Vec<(i32, i32, i32, i32)>) {
    let below = boxed_rect(st, hud).map(|b| b.1 + b.3 + 2);
    menu_boxes_in(&st.tiles, menu_font(st, menu), &menu_title(st, menu), &menu.entries, clip(hud), below)
}

/// `menu_boxes` with the tiles `tiles` and the font `font`, the title's
/// lines `title`, in the clip `c`, from y `below` at least.
#[allow(clippy::type_complexity)]
fn menu_boxes_in(tiles: &Tiles, font: &Font, title_lines: &[Vec<Span>], entries: &[Vec<Span>], c: (i32, i32, i32, i32), below: Option<i32>) -> (Option<(i32, i32, i32, i32)>, Vec<(i32, i32, i32, i32)>) {
    let title = (!title_lines.is_empty()).then(|| box_size_in(tiles, font, title_lines, MARGIN));
    let sizes: Vec<_> = entries.iter().map(|e| box_size_in(tiles, font, std::slice::from_ref(e), MARGIN)).collect();
    let w = sizes.iter().chain(title.iter()).map(|b| b.0).max().unwrap_or(0);
    let unrounded = |lines: &[Vec<Span>]| lines.len() as i32 * (font.h as i32 + 1) + 2 * MARGIN;
    let title_h = if title_lines.is_empty() { 0 } else { unrounded(title_lines) };
    let total = title_h + entries.iter().map(|e| unrounded(std::slice::from_ref(e))).sum::<i32>();
    let mut y = 0x64 - total / 2;
    if let Some(b) = below {
        y = y.max(b);
    }
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
    let font = menu_font(st, menu);
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
                out.push(vec![Span { colour, text: std::mem::take(&mut cur), font: Default::default(), code: None }]);
                cur = word.to_string();
            } else {
                cur = next;
            }
        }
        if !cur.is_empty() {
            out.push(vec![Span { colour, text: cur, font: Default::default(), code: None }]);
        }
    }
    out
}

/// A list's first line shown and how many fit (3159:0778: the area's
/// height over the font's, B7F4).
fn list_rows(st: &Style, menu: &Menu) -> (usize, usize) {
    list_rows_in(st, menu, list_font(st, menu))
}

/// `list_rows` in the font `font`.
fn list_rows_in(st: &Style, menu: &Menu, font: &Font) -> (usize, usize) {
    let rows = (list_kind(st, menu).area.3 / font.h as i32).max(1) as usize;
    let top = (menu.selected + 1).saturating_sub(rows).min(menu.entries.len().saturating_sub(rows));
    (top, rows)
}

/// The entry under (x, y) of the 320 x 200 screen, laid out as drawn.
pub fn menu_pick(font: &Font, hud: &Hud, x: i32, y: i32) -> Option<usize> {
    let menu = hud.menu.as_ref()?;
    if menu.kind == DIALOG_BUTTONS {
        return dialog_button_at(menu, x, y);
    }
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
        let i = top + ((y - area.1 - 1).max(0) / list_font(st, menu).h as i32) as usize;
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
        let lines = vec![vec![Span { colour: 0x0F, text: s.clone(), font: Default::default(), code: None }]];
        let (w, h, _, _) = box_size(st, &st.box_font, &lines);
        draw_box(screen, st, &st.box_font, ((c.0 + c.2) / 2 - w / 2, c.3 - h - 2, w, h), &lines, 0x0F);
    }
    if let Some(p) = hud.boxed.as_deref().and_then(placed_of) {
        draw_placed(screen, st, p);
    }
    if let (Some(lines), Some(r)) = (&hud.boxed, boxed_rect(st, hud)) {
        draw_box(screen, st, st.box_font_of(lines), r, lines, 0x60);
    }
    if let Some(menu) = &hud.menu {
        if menu.kind == DIALOG_BUTTONS {
            if let Some(p) = placed_of(&menu.title) {
                draw_placed(screen, st, p);
            }
        } else if menu.list {
            draw_list(screen, st, menu);
        } else {
            let (title, entries) = menu_boxes(st, menu, hud);
            if let Some(t) = title {
                draw_box(screen, st, menu_font(st, menu), t, &menu_title(st, menu), 0x60);
            }
            for (i, (r, e)) in entries.iter().zip(&menu.entries).enumerate() {
                draw_box(screen, st, menu_font(st, menu), *r, std::slice::from_ref(e), 0x60);
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
    match &hud.typing {
        Some(crate::dialog::Prompt::Boxed(prompt)) => draw_typing(screen, st, c, prompt),
        Some(crate::dialog::Prompt::Lines(lines)) => draw_dialog_lines(screen, st, lines),
        _ => {}
    }
}

/// A typed field's prompt box as the original draws it: the prompt in a
/// box by 965A with CX 7 (centred in the clip) and a margin of 0Ah. Two
/// callers: 1194:4800 (the rest's hours), whose prompt ends in spaces,
/// the field's room on its line; 1194:5C30 (the travel map's search,
/// creation's name), whose prompt is formatted "%s\r" and 17 spaces
/// (4FBC), the field's line under it. The field itself (0000:9EE9) is
/// where the caller puts it, drawn over the box (`field::Live::draw`).
pub fn typed_prompt(screen: &mut Screen, prompt: &str) {
    if let Some(st) = STYLE.get() {
        draw_typing(screen, st, (0, 0, W as i32 - 1, H as i32 - 1), prompt);
    }
}

/// `typed_prompt` centred in the clip `c`.
fn draw_typing(screen: &mut Screen, st: &Style, c: (i32, i32, i32, i32), prompt: &str) {
    let font = &st.box_font;
    let text = lines(prompt.as_bytes(), 0x0F);
    let last = text.len().saturating_sub(1);
    let (w, h, tx, ty) = box_size_margin(st, font, &text, 10);
    let (x, y) = ((c.0 + c.2) / 2 - w / 2, (c.1 + c.3) / 2 - h / 2);
    screen.tiles(&st.tiles, x, y, w / st.tiles.w as i32, h / st.tiles.h as i32);
    let lh = font.h as i32 + 1;
    for (i, l) in text.iter().enumerate() {
        let lw = font.width(&plain(l)) as i32;
        let lx = if i < last { x + w / 2 - lw / 2 } else { x + tx };
        screen.spans_game(font, lx, y + ty + i as i32 * lh, l, false, 0x0F);
    }
}

/// 1194:4E88's lines before its typed field: each from (20h, 91h), 9
/// apart, in TEENYFNT.DAT ([102E]) and colour C0h (4EAD..4EBF).
fn draw_dialog_lines(screen: &mut Screen, st: &Style, lines: &[Vec<Span>]) {
    for (i, l) in lines.iter().enumerate() {
        screen.text_raw(&st.list_font, 0x20, 0x91 + 9 * i as i32, &plain(l), 0xC0);
    }
}

/// The window's frames counted, for what blinks on them (the travel
/// map's markers keep their own passes).
static BLINK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn blink_frame() {
    BLINK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
                Align::Centre => area.0 + area.2 / 2 - w / 2,
                Align::Right => area.0 + area.2 - w,
            }
        };
        screen.spans_game(font, x, y, &spans, false, kind.colour);
    }
}

/// A list as 1194:5734 shows it: its kind's panel where its header puts
/// it; the lines in the text area from its top plus one, a font's height
/// apart, in the kind's colour (the record's +28h), from the area's x
/// plus one and a line's parts at their columns (2F26:2E12); then the
/// line picked recoloured through the table at BA0E (3159:0BE4); the
/// scroll track's thumb in F5h with its edges F9h and F2h (3159:0CFF), as
/// high as the part shown, at least 3. Kinds 6 and 10 draw the caller's
/// title where 5734 does; other kinds have none of their own (the view's
/// callers may give one: above the area, centred).
fn draw_list(screen: &mut Screen, st: &Style, menu: &Menu) {
    let font = list_font(st, menu);
    draw_list_in(screen, st, menu, font, font);
}

/// `draw_list` with its lines in the font `font` and the title 5734
/// draws in `title_font` (the one selected before it: kind 6 then
/// selects FONT_A for the lines, 1194:5914).
fn draw_list_in(screen: &mut Screen, st: &Style, menu: &Menu, font: &Font, title_font: &Font) {
    let kind = list_kind(st, menu);
    if let Some(p) = &kind.panel {
        screen.blit(p.x, p.y, p.w, p.h, &p.px, H as i32);
    }
    let area = kind.area;
    let fh = font.h as i32;
    let (top, rows) = list_rows_in(st, menu, font);
    for (i, l) in menu.title.iter().enumerate() {
        let w = title_font.width(&plain(l)) as i32;
        let (y, c) = match kind.title {
            Some((y, c)) => (y + i as i32 * (title_font.h as i32 + 1), c),
            None => (area.1 - fh - 4 + i as i32 * (fh + 1), 0x60),
        };
        screen.spans_game(title_font, 0xA0 - w / 2, y, l, false, c);
    }
    for (k, e) in menu.entries.iter().enumerate().skip(top).take(rows) {
        let y = area.1 + 1 + (k - top) as i32 * fh;
        list_line(screen, font, kind, y, e);
    }
    // 3159:0BE4, after the lines: the picked line's row of the area
    // recoloured through BA0E's table.
    if (top..top + rows).contains(&menu.selected) {
        let y = area.1 + 1 + (menu.selected - top) as i32 * fh;
        screen.remap(area.0, y, area.2, fh, &picked_table());
    }
    // The thumb (3159:0C93 sizes it: the whole track while every line
    // fits, else its share, at least 3; 0CFF draws it): filled in F5h,
    // then by 0000:7334's lines the left edge and the bottom in F9h, the
    // right edge and the top in F2h.
    let n = menu.entries.len();
    let (tx, ty, tw, th) = kind.track;
    let h = if n > rows { (th * rows as i32 / n as i32).max(3) } else { th };
    let y = if n > rows { ty + (th - h) * top as i32 / (n - rows) as i32 } else { ty };
    screen.fill(tx, y, tw, h, 0xF5, 0xFF);
    screen.fill(tx, y, 1, h, 0xF9, 0xFF);
    screen.fill(tx, y + h - 1, tw, 1, 0xF9, 0xFF);
    screen.fill(tx + tw - 1, y, 1, h - 1, 0xF2, 0xFF);
    screen.fill(tx, y, tw, 1, 0xF2, 0xFF);
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
    if let Some((x, y, step, lines)) = hud.boxed.as_deref().and_then(placed_of) {
        for (i, l) in lines.iter().enumerate() {
            screen.text_plain(font, x, y + step * i as i32, &plain(l), 0xC0);
        }
    } else if let Some(lines) = &hud.boxed {
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
    if let Some(crate::dialog::Prompt::Boxed(prompt)) = &hud.typing {
        let w = (font.width(prompt) as i32 + 16).clamp(80, W as i32 - 8);
        let x = (W as i32 - w) / 2;
        let y = H as i32 / 2 - lh;
        screen.panel(x, y, w, lh + 10, 0x0F);
        screen.text(font, x + 8, y + 5, prompt, 0x0F);
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
        assert_eq!(l[0], vec![Span { colour: 0xC0, text: " CAMP".into(), font: Default::default(), code: Some(0xC0) }, Span { colour: 0xC0, text: String::new(), font: Default::default(), code: None }]);
        assert_eq!(plain(&l[1]), "Camp");
        assert_eq!(l[1][1].colour, 0xD4);
    }

    /// 98A0 keeps a code's colour byte in the line 66F6 measures: '`'
    /// (60h) counts as its glyph, C0h and a byte under 20h as nothing.
    #[test]
    fn a_codes_colour_byte_is_measured() {
        let l = lines(b"\t`AB\t\xc0C\r", 0x0F);
        assert_eq!(l[0].iter().map(|s| s.code).collect::<Vec<_>>(), vec![Some(0x60), Some(0xC0), None]);
        let font = Font { h: 1, widths: std::array::from_fn(|i| i as u8 % 7 + 1), rows: Vec::new() };
        let glyphs = font.width("ABC") as i32;
        assert_eq!(measured(&font, &l[0]), glyphs + font.glyph(b'`').0 as i32);
    }
}
