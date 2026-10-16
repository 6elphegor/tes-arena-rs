//! Character creation in the window (`--create`), as the original's
//! creation (1194:2498..25B7) presents it, its rules the engine's
//! (src/newgame.rs) and its pictures, fonts and texts the game's own, read
//! from the game's files at run time:
//!
//! - how the class is chosen (1194:25B9): STARTGAM.MNU in its own colours,
//!   a menu in FONT_A (the keys G and S at 3F6A, the entries at 3F6D);
//! - the questions (222C:0000): the instructions (3276:3547), then ten
//!   questions from QUESTION.TXT on the scroll (SCROLL01.DFA at 0, 120),
//!   their lines at x 14h from row 136 (six at a time, 222C:03E9) in
//!   TEENYFNT, answered with A, B or C (the region keys at 3276:5F60; the
//!   scroll's top and bottom move it), and the class they give offered
//!   (3276:3651, a yes/no menu); refused, the list;
//! - the class list (1194:2678): eighteen names in the order of 2AEE, a
//!   list of kind 6 (5734: POPUP2.IMG, its title in FONT_C, the lines in
//!   FONT_A);
//! - the name typed (2711, 3276:35F8) and the gender (2791: M, F);
//! - the province on TAMRIEL.MNU (27C2: NOEXIT.IMG over its exit, the
//!   question 3276:362A, the eight regions at
//!   3276:5B80), each click rolling the portrait, then accepted (369F);
//! - what the player is told (2902: 36EC, the province's text 581h + the
//!   province from TEMPLATE.DAT after "Know ye this also:", 3742, 3780);
//! - the attributes on CHARSTAT.IMG in CHARSHT.COL's colours (2A69): the
//!   texts where 1194:0DE1 writes them, the bonus points at (5Ch, 71h)
//!   in BONUS.IMG, UPDOWN.IMG's arrows on the line picked, the regions at
//!   3276:5312 (the eight lines, the arrows, the button at the bottom
//!   left), then "Save stats" or "Reroll stats" (2CD0), refused while
//!   points are left;
//! - the face (2BC5): FACES1r.CIF or FACESF1r.CIF (r the race), clicked
//!   through (the regions at 3276:5E90), then Done;
//! - the vision (1194:D459 with text 578h): VISION.FLC playing (`flc`)
//!   with its text three lines at a time, then the game.
//!
//! Before it, as the original starts (0000:10C9): INTRO.FLC (0000:125C),
//! TITLE.IMG, then the main menu (MENU.IMG, 0000:10FA: its regions at
//! 3276:6300, keys L, S and E: Load, Start a new game, Exit). The opening
//! story on SCROLL.FLC (0000:1323) isn't played. At a game's end
//! (`Creation::ending`) the farewell's vision (text 57Ah on VISION.FLC, or
//! 57Bh on JAGAR.FLC while F77 is set: 0000:0D01..0D3E), then the main
//! menu again. On a level gained (`Creation::level_up`) the attribute
//! screen alone, its clicks kept for the turn to replay (1194:122C).
//!
//! The boxes (0000:965A from 1194:59CC and 5C34), the menus (1194:020E:
//! 3159:0F2A's boxes, the keys' letters in the entries' own colours) and
//! the list are drawn as the original draws them, in PARCH.CIF's 20 x 20
//! tiles until the attributes and MARBLE2.CIF's after; the screens are
//! checked against the original's (`createtest`). Each answer plays its
//! kind's figure (WARRIOR.CEL, ROGUE.CEL, MAGE.CEL, 222C:011A); the next
//! question's screen covers it, and the class offered after the last is
//! drawn over what it left. The questions' scroll rolls a row at a time
//! with its rollers turning (222C:040B, 045D). Between screens the
//! original's dissolves, fades and colours set play frame by frame
//! (`film::Film`). Left out: the map's scroll unrolling (TOP.DFA, 1194:5B58:
//! it ends off the screen).

use crate::archive::Archive;
use crate::dialog::Click;
use crate::film::{Dac, Film, FULL, RETRACE_MS};
use crate::flc::Movie;
use crate::hud::{self, Font, Screen, Span, H, W};
use crate::interface::Image;
use arena::engine::Engine;
use arena::format::Arg;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;

/// The player's record, and the buffer the texts are made in.
const PLAYER: u16 = 0x01A4;
const BUF: u16 = 0x1044;
/// The record's fields read here.
const NAME: u16 = PLAYER + 9;
const RACE: u16 = PLAYER + 4;
const CLASS: u16 = PLAYER + 5;
const SEX: u16 = PLAYER + 7;
/// The class list's order, the classes' names, the races' (singular and
/// plural) and the provinces'.
const LIST_ORDER: u16 = 0x2AEE;
const CLASS_NAMES: u16 = 0x2A62;
const RACES: u16 = 0x2B49;
const PROVINCES: u16 = 0x2F06;
/// The region tables (3276), records of 62h bytes: x, y, width, height.
const MAP_REGIONS: u16 = 0x5B80;
const STAT_REGIONS: u16 = 0x5312;
const FACE_REGIONS: u16 = 0x5E90;
const REGION: u16 = 0x62;
/// The text colour (9288) the screens set: EEh, then FEh from the
/// attributes on.
const COLOUR_A: u8 = 0xEE;
const COLOUR_B: u8 = 0xFE;

/// A key or a click (on the 320 x 200 screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Char(u8),
    Enter,
    Esc,
    Back,
    Up,
    Down,
    Click(i32, i32),
    /// Time passing, in milliseconds (a movie's frames).
    Wait(u32),
    /// A key (its scan code) pressed or let go, while the name is typed
    /// (the game's editor takes both).
    Key(u8, bool),
}

/// `--create`'s steps: a character (`G`), `enter`, `esc`, `back`, `up`,
/// `down`, `type:TEXT` (its characters), `click:X:Y`, `wait:MS` (time for
/// a movie), comma-separated.
pub fn parse_script(s: &str) -> Vec<Input> {
    let mut out = Vec::new();
    for t in s.split(',').filter(|t| !t.is_empty()) {
        match t {
            "enter" => out.push(Input::Enter),
            "esc" => out.push(Input::Esc),
            "back" => out.push(Input::Back),
            "up" => out.push(Input::Up),
            "down" => out.push(Input::Down),
            _ => {
                if let Some(text) = t.strip_prefix("type:") {
                    out.extend(text.bytes().map(Input::Char));
                } else if let Some(ms) = t.strip_prefix("wait:") {
                    out.push(Input::Wait(ms.parse().expect("wait:MS")));
                } else if let Some(xy) = t.strip_prefix("click:") {
                    let (x, y) = xy.split_once(':').expect("--create click:X:Y");
                    out.push(Input::Click(x.parse().expect("click:X:Y"), y.parse().expect("click:X:Y")));
                } else if t.len() == 1 {
                    out.push(Input::Char(t.as_bytes()[0]));
                } else {
                    panic!("--create: {t}?");
                }
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FontId {
    A,
    C,
    Arena,
}

/// What the screen behind shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Back {
    Start,
    Map,
    Stats,
    Black,
}

/// What is up on the screen (`Creation::up`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Up {
    Box,
    Menu,
    List,
    Prompt,
    Quiz,
    Other,
}

/// A text box waiting to be read, the screen it came up on.
#[derive(Clone, Debug)]
struct Boxed {
    lines: Vec<Vec<Span>>,
    font: FontId,
    back: Back,
}

/// A menu asked (1194:020E) or a list (5734): its title, entries and keys;
/// the entry highlighted and, for a list, its first row shown.
#[derive(Clone, Debug)]
struct Ask {
    title: Vec<Vec<Span>>,
    entries: Vec<Vec<Span>>,
    keys: Vec<u8>,
    sel: usize,
    top: usize,
    /// The rows shown (0: all).
    rows: usize,
    font: FontId,
    /// Whether the arrows have moved the pick (the view then marks it;
    /// the original picks by key or click and marks nothing).
    moved: bool,
}

#[derive(Clone, Debug)]
enum Stage {
    Method(Ask),
    Quiz { asked: u8, roll: Roll },
    ClassYes(Ask, u8),
    List(Ask),
    /// The name asked (1194:2711): the prompt, the game's editor on the
    /// name (`field::Live`).
    Name(Vec<Vec<Span>>, Box<crate::field::Live>),
    Gender(Ask),
    Map,
    ProvinceYes(Ask, u8),
    Stats,
    /// The name typed over on the attribute page (1194:1528), the game's
    /// editor on it.
    SheetName(Box<crate::field::Live>),
    Choose(Ask),
    Face,
    /// The movies (`Creation::movie`): the intro, the vision after
    /// creation, the farewell at a game's end.
    Intro,
    Vision,
    Ending,
    /// The opening story after the menu's Start (0000:1323): SCROLL.FLC
    /// (the movie), then INTRO01..09.IMG (`pic` 1..9, its picture and
    /// the colours the movie left) for 350 screen refreshes each (5 s at
    /// 70 Hz, 0000:13BF); a key ends it all (ACB6). Then `next`, the
    /// class's first question, made at the Start.
    Story { pic: u8, ms: u32, img: Option<Box<(Image, [[u8; 4]; 256])>>, next: Box<Stage> },
    Title,
    MainMenu,
    Done,
}

/// How creation (or the screen up) ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The character made: the game to start.
    Play,
    /// The main menu's Load and Exit.
    Load,
    Exit,
    /// A level's points spent, by these clicks.
    Spent(Vec<Click>),
}

/// The main menu's regions (3276:6300, three: Load, Start, Exit; their
/// keys at +59h).
const MENU_REGIONS: u16 = 0x6300;

/// The game's pictures for creation, decoded once.
struct Pics {
    start: Option<(Image, [[u8; 4]; 256])>,
    map: Option<(Image, [[u8; 4]; 256])>,
    noexit: Option<Image>,
    /// The scroll's 16 rollers' frames (SCROLL01.DFA's then SCROLL02's:
    /// 222C:03B1 by 129C).
    rollers: Vec<Image>,
    charstat: Option<Image>,
    bonus: Option<Image>,
    updown: Vec<u8>,
    /// The boxes' tiles: PARCH.CIF's 20 x 20 (1194:2520, [1038]) until
    /// the attributes, MARBLE2.CIF's 3 x 3 ([103A], 2562) from them.
    parch: Option<hud::Tiles>,
    /// The shirt drawn over each face (1194:2486: 3E42 as the doll's
    /// chest left it, [M|F][S|R]SHIRT.IMG, 1194:1D30).
    shirt: Option<Image>,
    marble2: Option<hud::Tiles>,
    charsheet: [[u8; 4]; 256],
    game: [[u8; 4]; 256],
    faces: Vec<Image>,
    /// The face shown and the shirt over it, as the loop at 1194:2C04
    /// last decoded them (`show_face`).
    face_now: Option<Image>,
    shirt_now: Option<Image>,
    /// The shirt's file (1194:2486 loads it again for each face).
    shirt_file: Vec<u8>,
    /// The last answer's kind's figure as its animation (222C:011A..012B)
    /// left it: the last frame, where the file centres it.
    cel: Option<Image>,
}

struct Fonts {
    a: Font,
    c: Font,
    arena: Font,
    teeny: Font,
}

impl Fonts {
    fn get(&self, id: FontId) -> &Font {
        match id {
            FontId::A => &self.a,
            FontId::C => &self.c,
            FontId::Arena => &self.arena,
        }
    }
}

/// Character creation under way.
pub struct Creation {
    stage: Stage,
    boxes: VecDeque<Boxed>,
    archive: Rc<Archive>,
    pics: Pics,
    fonts: Fonts,
    name: String,
    /// The text colour now (9288).
    colour: u8,
    /// The movie playing (Intro, Vision, Ending).
    movie: Option<Movie>,
    /// TITLE.IMG and MENU.IMG, in their own colours.
    title: Option<(Image, [[u8; 4]; 256])>,
    menu_pic: Option<(Image, [[u8; 4]; 256])>,
    /// On a level gained: the clicks so far.
    level_up: Option<Vec<Click>>,
    /// The attribute page as last drawn (palette indices), which the
    /// faces are drawn over.
    /// The faces' page (the attributes' last) and the head's corner
    /// 2C96 puts back under each face (0DAh, 0, 4Ah x 37h).
    face_back: Option<(Vec<u16>, Vec<u16>)>,
    /// The last question's lines and its roll, under the class offered.
    quiz_shown: Option<Roll>,
    outcome: Option<Outcome>,
    /// The frames of the screen's change playing (`Film`), the first now.
    film: VecDeque<(Vec<u8>, Dac)>,
    /// Milliseconds into the film's frame.
    film_ms: u32,
}

/// The creation's `NewGameHooks`: its boxes kept (the attributes' note).
struct Hooks<'a> {
    boxes: &'a mut VecDeque<Boxed>,
}

impl Creation {
    /// The song the original plays at this stage (`arena::music::var`):
    /// percntro from the intro through the title and the menu (0000:1246),
    /// evlintro with the opening story (0000:1323), sheet through the
    /// creation's screens (1194:249B), vision with the vision and the
    /// farewell (0000:113E, 0D08/0D18).
    pub fn song(&self) -> Option<u16> {
        use arena::music::var::*;
        Some(match self.stage {
            Stage::Intro | Stage::Title | Stage::MainMenu => PERCNTRO,
            Stage::Story { .. } => EVLINTRO,
            Stage::Vision | Stage::Ending => VISION,
            Stage::Done => return None,
            _ => SHEET,
        })
    }
}

impl arena::newgame::NewGameHooks for Hooks<'_> {
    fn message_box(&mut self, m: &mut Machine, seg: u16, si: u16) {
        self.boxes.push_back(Boxed { lines: hud::lines(m.mem.cstr(seg, si), COLOUR_B), font: FontId::Arena, back: Back::Stats });
    }
}

/// An image with a 12-byte header and its palette after the data
/// (6-bit; flag 100h), as the .MNU screens are.
pub(crate) fn mnu(archive: &Archive, name: &str) -> Option<(Image, [[u8; 4]; 256])> {
    let d = archive.read(name)?;
    let (img, used) = crate::interface::image(&d)?;
    let p = d.get(used..used + 0x300)?;
    let mut pal = [[0u8; 4]; 256];
    for (i, c) in pal.iter_mut().enumerate() {
        let v = |k: usize| {
            let x = p[3 * i + k] & 0x3F;
            x << 2 | x >> 4
        };
        *c = [v(0), v(1), v(2), 0xFF];
    }
    Some((img, pal))
}

/// A .DFA's frames, each at its header's place.
fn dfa_frames(archive: &Archive, name: &str) -> Vec<Image> {
    let Some(d) = archive.read(name) else { return Vec::new() };
    let le = |at: usize| u16::from_le_bytes([*d.get(at).unwrap_or(&0), *d.get(at + 1).unwrap_or(&0)]) as i16 as i32;
    let Some(f) = crate::flats::decode(1, &d) else { return Vec::new() };
    f.frames.into_iter().map(|px| Image { x: le(2), y: le(4), w: f.w, h: f.h, px }).collect()
}

/// The questions' scroll as 222C keeps it: the text's strings (the
/// buffer at 40F0: a LF, the question less its last byte, two LFs; each
/// LF a string's end, 02D4), the bitmap they're drawn into (40F2: 8 lines
/// of 8 rows, a line from x 14h in EEh, 04E5), the top line (1298), the
/// rows rolled past it (129A / 140h), the rollers' frame (129C) and the
/// last top line the scroll goes to (1296: the strings' count, at most 8).
#[derive(Clone, Debug)]
struct Roll {
    strings: Vec<String>,
    bitmap: Vec<u8>,
    line: usize,
    off: usize,
    frame: usize,
    last: usize,
    /// The rollers as drawn so far: each frame over the last, its 0s
    /// see-through (5526), what earlier ones drew left where it has 0.
    rollers: Vec<u8>,
}

/// The bitmap's rows (0..64; 0x4600 bytes are shifted, 0x3C00 shown).
const ROLL_ROWS: usize = 64;

/// The bank 2DAF loads the faces into (5C86 with BX 17h).
const FACES_BANK: u16 = 0x17;

/// `f` with EMS bank `bank` mapped into the frame (DS:9030; 5C86), the
/// bank before mapped back after.
fn with_bank<T>(m: &mut Machine, bank: u16, f: impl FnOnce(&mut Machine, u16) -> T) -> T {
    let before = m.mem.r16(DS, 0x8FA8) >> 2;
    m.ems_map(bank);
    let frame = m.mem.r16(DS, 0x9030);
    let out = f(m, frame);
    m.ems_map(before);
    out
}

/// 0000:5828's decoding of the image record at `seg`:`off` (a 12-byte
/// header, then its data): coded ones (flags & 0Dh) into w x h bytes from
/// the heap (51EC), LZHUF (8) through 3065:0D2D, LZSS (4) as 7608; the
/// bytes then freed (6C36). None for others or where the heap fails.
fn decode_5828(m: &mut Machine, seg: u16, off: u16) -> Option<Image> {
    let r = |i: u16| m.mem.r16(seg, off.wrapping_add(i));
    let (x, y, w, h, flags, size) = (r(0), r(2), r(4), r(6), r(8), r(10));
    if flags & 0xD == 0 || w == 0 || h == 0 {
        return None;
    }
    let bytes = w.checked_mul(h)?;
    let (dseg, doff) = m.heap_alloc_bytes(bytes)?;
    if flags & 8 != 0 {
        m.mif_decompress(seg, off.wrapping_add(12), dseg, doff, size);
    } else if flags & 4 != 0 {
        let rec: Vec<u8> = (0..12u16.wrapping_add(size)).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
        for (i, &b) in crate::interface::image(&rec).map(|(img, _)| img.px).unwrap_or_default().iter().enumerate() {
            m.mem.w8(dseg, doff.wrapping_add(i as u16), b);
        }
    }
    let px = (0..bytes).map(|i| m.mem.r8(dseg, doff.wrapping_add(i))).collect();
    m.heap_free(dseg);
    Some(Image { x: x as i16 as i32, y: y as i16 as i32, w: w as usize, h: h as usize, px })
}

/// A .CIF's images (12-byte headers), each decoded as `interface::image`
/// does or, coded with type 8, through the port's LZHUF.
fn cif(m: &Machine, d: &[u8]) -> Vec<Image> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 12 <= d.len() {
        let size = u16::from_le_bytes([d[at + 10], d[at + 11]]) as usize;
        let one = &d[at..(at + 12 + size).min(d.len())];
        let img = crate::interface::image(one).map(|(i, _)| i).or_else(|| crate::panels::image_type8(m, one));
        match img {
            Some(i) => out.push(i),
            None => break,
        }
        at += 12 + size;
    }
    out
}

impl Creation {
    /// Creation started (1194:24A6..2512: the state cleared) at its first
    /// question, how the class is chosen.
    pub fn new(engine: &mut Engine, archive: Rc<Archive>, dir: &Path, game: [[u8; 4]; 256]) -> Creation {
        let m = &mut engine.m;
        m.newgame_reset();
        let font = |n: &str| Font::load(&archive, n).unwrap_or_else(|| panic!("{n}"));
        let fonts = Fonts { a: font("FONT_A.DAT"), c: font("FONT_C.DAT"), arena: font("ARENAFNT.DAT"), teeny: font("TEENYFNT.DAT") };
        let pics = Pics {
            start: mnu(&archive, "STARTGAM.MNU"),
            map: mnu(&archive, "TAMRIEL.MNU"),
            noexit: crate::panels::load_image(&archive, m, "NOEXIT.IMG"),
            rollers: dfa_frames(&archive, "SCROLL01.DFA").into_iter().chain(dfa_frames(&archive, "SCROLL02.DFA")).collect(),
            charstat: crate::panels::load_image(&archive, m, "CHARSTAT.IMG"),
            bonus: crate::panels::load_image(&archive, m, "BONUS.IMG"),
            updown: archive.read("UPDOWN.IMG").unwrap_or_default(),
            parch: hud::Tiles::load(&archive, "PARCH.CIF", 20, 20),
            shirt: None,
            marble2: hud::Tiles::load(&archive, "MARBLE2.CIF", 3, 3),
            charsheet: crate::world::palette_file(dir, "CHARSHT.COL"),
            game,
            faces: Vec::new(),
            face_now: None,
            shirt_now: None,
            shirt_file: Vec::new(),
            cel: None,
        };
        let title = image_with_palette(m, &archive, "TITLE.IMG");
        let menu_pic = image_with_palette(m, &archive, "MENU.IMG");
        let mut c = Creation { stage: Stage::Done, boxes: VecDeque::new(), archive, pics, fonts, name: String::new(), colour: COLOUR_A, movie: None, title, menu_pic, level_up: None, face_back: None, quiz_shown: None, outcome: None, film: VecDeque::new(), film_ms: 0 };
        // The program's start (0000:10C9): the intro (0000:125C), the
        // title, the menu.
        c.movie = Movie::new(&c.archive, "INTRO.FLC", Vec::new());
        c.stage = if c.movie.is_some() { Stage::Intro } else { c.after_intro() };
        c
    }

    /// A game over (5B1 bit 100h): the farewell's vision (0000:0CE9..0D3E:
    /// text 57Bh on JAGAR.FLC in colour F5h while F77 is set, else 57Ah on
    /// VISION.FLC in 2Fh; none when F77 is 9), then the main menu. The
    /// vision (1194:D459) first fades the screen as the game left it
    /// (`shown`, in the game's colours; none: a cleared one) to black in
    /// 8 steps (55B6) and clears it (55D9), then plays the movie.
    pub fn ending(engine: &mut Engine, archive: Rc<Archive>, dir: &Path, game: [[u8; 4]; 256], shown: Option<&[u8]>) -> Creation {
        // The text made from the game as it ended (the player's name in
        // it), before creation's set-up clears the state.
        let f77 = engine.m.mem.r8(DS, 0x0F77);
        let (text, flc, colour) = if f77 != 0 { (0x57B, "JAGAR.FLC", 0xF5) } else { (0x57A, "VISION.FLC", 0x2F) };
        let lines = if f77 != 9 { vision_text(engine, text, colour) } else { Vec::new() };
        let mut c = Creation::new(engine, archive, dir, game);
        c.movie = None;
        c.stage = Stage::MainMenu;
        if f77 != 9 {
            c.movie = Movie::new(&c.archive, flc, lines);
            if c.movie.is_some() {
                c.stage = Stage::Ending;
                let black = [[0, 0, 0, 0xFF]; 256];
                let mut film = Film::new(shown.map_or_else(|| vec![0; W * H], <[u8]>::to_vec), game);
                film.fade(&black, 8);
                film.whole(&[0; W * H]);
                film.wait();
                c.film = film.frames.into();
            }
        }
        c
    }

    /// The main menu, as the program's outer loop comes back to it after
    /// the options screen's New (0000:109B with 5CA bit 8000h): no intro.
    pub fn main_menu(engine: &mut Engine, archive: Rc<Archive>, dir: &Path, game: [[u8; 4]; 256]) -> Creation {
        let mut c = Creation::new(engine, archive, dir, game);
        c.movie = None;
        c.stage = Stage::MainMenu;
        c
    }

    /// A level gained (1194:1034): the attribute screen (1194:122C) over
    /// the state `m` (the attributes in percent), its clicks kept.
    pub fn level_up(m: &mut Machine, archive: Rc<Archive>, dir: &Path, game: [[u8; 4]; 256]) -> Creation {
        let font = |n: &str| Font::load(&archive, n).unwrap_or_else(|| panic!("{n}"));
        let fonts = Fonts { a: font("FONT_A.DAT"), c: font("FONT_C.DAT"), arena: font("ARENAFNT.DAT"), teeny: font("TEENYFNT.DAT") };
        let pics = Pics {
            start: None,
            map: None,
            noexit: None,
            rollers: Vec::new(),
            charstat: crate::panels::load_image(&archive, m, "CHARSTAT.IMG"),
            bonus: crate::panels::load_image(&archive, m, "BONUS.IMG"),
            updown: archive.read("UPDOWN.IMG").unwrap_or_default(),
            parch: hud::Tiles::load(&archive, "PARCH.CIF", 20, 20),
            shirt: None,
            marble2: hud::Tiles::load(&archive, "MARBLE2.CIF", 3, 3),
            charsheet: crate::world::palette_file(dir, "CHARSHT.COL"),
            game,
            faces: Vec::new(),
            face_now: None,
            shirt_now: None,
            shirt_file: Vec::new(),
            cel: None,
        };
        Creation { stage: Stage::Stats, boxes: VecDeque::new(), archive, pics, fonts, name: String::new(), colour: COLOUR_B, movie: None, title: None, menu_pic: None, level_up: Some(Vec::new()), face_back: None, quiz_shown: None, outcome: None, film: VecDeque::new(), film_ms: 0 }
    }

    /// Back on the main menu (a load that failed).
    pub fn back_to_menu(&mut self) {
        self.outcome = None;
        self.stage = Stage::MainMenu;
    }

    /// After the intro: the title (a key or click), else the menu.
    fn after_intro(&self) -> Stage {
        if self.title.is_some() { Stage::Title } else { Stage::MainMenu }
    }

    /// How it ended, once it has: the game to play, Load or Exit picked,
    /// or a level's points spent.
    pub fn outcome(&self) -> Option<Outcome> {
        if !self.boxes.is_empty() {
            return None;
        }
        match (&self.outcome, &self.stage) {
            (Some(o), _) => Some(o.clone()),
            (None, Stage::Done) => Some(Outcome::Play),
            _ => None,
        }
    }

    /// Time passing (milliseconds) for the movie playing: its frames; at
    /// its end (no text: once through) the next screen.
    pub fn advance(&mut self, ms: u32) {
        // The change's frames, one a retrace (70 a second).
        self.film_ms += ms;
        while !self.film.is_empty() && self.film_ms >= RETRACE_MS {
            self.film_ms -= RETRACE_MS;
            self.film.pop_front();
        }
        if self.film.is_empty() {
            self.film_ms = 0;
        }
        // The movie after the change (the vision's fade, 1194:D4E1).
        if let (Some(mv), true) = (&mut self.movie, self.film.is_empty()) {
            mv.advance(ms);
        }
        self.movie_over();
        self.story(ms);
    }

    /// The opening story's pictures: each for 5 s, in the colours the
    /// movie left, then the next stage.
    fn story(&mut self, ms: u32) {
        let Stage::Story { pic, ms: shown, img, .. } = &mut self.stage else { return };
        if *pic == 0 {
            return;
        }
        *shown += ms;
        if *shown < 5000 {
            return;
        }
        *shown = 0;
        *pic += 1;
        if *pic > 9 {
            if let Stage::Story { next, .. } = std::mem::replace(&mut self.stage, Stage::Done) {
                self.stage = *next;
            }
            return;
        }
        let pal = img.as_ref().map(|b| b.1).unwrap_or([[0; 4]; 256]);
        *img = intro_pic(&self.archive, *pic, pal).map(Box::new);
    }

    /// The next screen when the movie is over.
    fn movie_over(&mut self) {
        if !self.movie.as_ref().is_some_and(|m| m.over) {
            return;
        }
        if let Stage::Story { pic: 0, .. } = self.stage {
            // The movie over: its colours stay for the pictures.
            let pal = self.movie.as_ref().map(|m| m.flc.palette).unwrap_or([[0; 4]; 256]);
            self.movie = None;
            if let Stage::Story { pic, img, .. } = &mut self.stage {
                *pic = 1;
                *img = intro_pic(&self.archive, 1, pal).map(Box::new);
            }
            return;
        }
        let next = match self.stage {
            Stage::Intro => self.after_intro(),
            Stage::Ending => Stage::MainMenu,
            Stage::Vision => Stage::Done,
            _ => return,
        };
        self.movie = None;
        self.stage = next;
    }

    /// Whether creation is over (the game to be started).
    pub fn done(&self) -> bool {
        matches!(self.stage, Stage::Done) && self.boxes.is_empty() && self.outcome.is_none()
    }

    /// The text at `seg`:`fmt` made with `args` into the buffer (as the
    /// original's sprintf), as lines in the colour now.
    fn say(&self, m: &mut Machine, fmt: (u16, u16), args: &[Arg]) -> Vec<Vec<Span>> {
        m.sprintf((DS, BUF), fmt, args);
        hud::lines(m.mem.cstr(DS, BUF), self.colour)
    }

    /// A menu made the original's way: its title at `fmt` (made into the
    /// buffer), its keys and entries' table in DGROUP.
    fn menu(&self, m: &mut Machine, fmt: (u16, u16), args: &[Arg], keys: u16, table: u16, font: FontId) -> Ask {
        let title = self.say(m, fmt, args);
        let raw = crate::dialog::menu_at(m, BUF, keys, table);
        let entries = raw.entries.iter().map(|l| l.iter().map(|s| Span { colour: if s.colour == 0x0F { self.colour } else { s.colour }, text: s.text.clone(), font: Default::default(), code: None }).collect()).collect();
        Ask { title, entries, keys: raw.keys, sel: 0, top: 0, rows: 0, font, moved: false }
    }

    fn method(&self, m: &mut Machine) -> Ask {
        self.menu(m, (DATA, 0x3520), &[], 0x3F6A, 0x3F6D, FontId::A)
    }

    fn yes_no(&self, m: &mut Machine, fmt: u16, args: &[Arg]) -> Ask {
        self.menu(m, (DATA, fmt), args, 0x4B60, 0x4B63, FontId::A)
    }

    /// The name of class byte `class` (its low 5 bits).
    fn class_of(m: &Machine, class: u8) -> (u16, u16) {
        (DS, m.skip_strings(DS, CLASS_NAMES, (class & 0x1F) as u16))
    }

    /// One of the strings one after another at DGROUP `at`.
    fn nth(m: &Machine, at: u16, n: u8) -> (u16, u16) {
        (DS, m.skip_strings(DS, at, n as u16))
    }

    fn push_box(&mut self, lines: Vec<Vec<Span>>, font: FontId, back: Back) {
        self.boxes.push_back(Boxed { lines, font, back });
    }

    /// A key or click, and the screen's change played as the original
    /// shows it (`Film`):
    /// - a menu answered (1194:020E): it was drawn on the screen alone,
    ///   and dissolves out to the page (4FF7 at 0269, the whole screen);
    /// - a list answered (5734): put back at once from what it was drawn
    ///   over;
    /// - a box shown (59CC at 5A1A, 5C34 at 5C9F) dissolves in; taken
    ///   away (5A41, 5CBD), the page as it was under it dissolves in;
    /// - a question answered: its kind's figure played over the screen
    ///   (222C:011A), the figures lit, the next question dissolved in on
    ///   the scroll's rows (222C:02A5..02B0: 0, 77h, 140h x 50h);
    /// - a new background (the map, the attributes): the colours faded
    ///   out (1194:55B6), the screen cleared (55D9), the picture's colours
    ///   set (0000:57DD).
    pub fn input(&mut self, engine: &mut Engine, inp: Input) {
        if let Input::Wait(ms) = inp {
            return self.advance(ms);
        }
        self.settle();
        let was = self.up();
        let was_back = self.back();
        let (before, before_dac) = self.shot(&engine.m, true);
        let (bare_before, _) = self.shot(&engine.m, false);
        let quiz = matches!(self.stage, Stage::Quiz { .. });
        let answer = matches!(inp, Input::Char(c) if (b'A'..=b'C').contains(&c.to_ascii_uppercase()));
        self.input_inner(engine, inp);
        if self.movie.is_some() {
            return;
        }
        let now = self.up();
        let (after, after_dac) = self.shot(&engine.m, true);
        let m = &mut engine.m;
        let mut film = Film::new(before.clone(), before_dac);
        match was {
            Up::Quiz if quiz && answer => {
                // The figure played over the screen, the figures lit.
                if let Some(i) = &self.pics.cel {
                    let mut s = Screen::new(before_dac);
                    let px: Vec<u8> = before.clone();
                    s.blit(0, 0, W, H, &px, H as i32);
                    s.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                    film.whole(&s.indices().iter().map(|&v| v as u8).collect::<Vec<u8>>());
                }
                film.dac = after_dac;
                film.wait();
                if now == Up::Quiz {
                    film.dissolve(m, &after, (0, 0x77, 0x140, 0x50));
                }
            }
            Up::Menu => {
                film.dissolve(m, &bare_before, FULL);
                match now {
                    Up::Box => self.box_in(&mut film, m, was_back, &after, after_dac),
                    Up::List => {
                        film.wait();
                        film.colours(&after_dac);
                    }
                    _ => {}
                }
            }
            Up::List => {
                let (under, _) = self.shot(m, false);
                film.whole(&under);
                if now != Up::List {
                    film.dissolve(m, &after, FULL);
                }
            }
            Up::Box => {
                // Under it: the page now, or, when a box over a new
                // background follows, the old background.
                let shown = std::mem::take(&mut self.boxes);
                let (mut under, _) = self.shot(m, true);
                self.boxes = shown;
                if matches!(now, Up::Box | Up::Quiz) {
                    under = bare_before.clone();
                }
                film.dissolve(m, &under, FULL);
                match now {
                    Up::Box => self.box_in(&mut film, m, was_back, &after, after_dac),
                    Up::Quiz => {
                        film.wait();
                        film.dissolve(m, &after, (0, 0x77, 0x140, 0x50));
                    }
                    _ => {}
                }
            }
            Up::Prompt if now != Up::Prompt => {
                let (under, _) = self.shot(m, false);
                film.dissolve(m, &under, FULL);
            }
            _ => {}
        }
        self.film = film.frames.into();
        let _ = after;
    }

    /// A box shown (dissolved in), over a new background if the back
    /// changed: the old faded out, the screen cleared, the new picture's
    /// colours set, the picture shown.
    fn box_in(&mut self, film: &mut Film, m: &mut Machine, was_back: Back, after: &[u8], after_dac: Dac) {
        let back = self.back();
        if back != was_back {
            film.wait();
            film.fade(&[[0, 0, 0, 0xFF]; 256], 8);
            film.vga.fill(0);
            let shown = std::mem::take(&mut self.boxes);
            let (under, _) = self.shot(m, true);
            self.boxes = shown;
            if back == Back::Stats {
                // The race's picture first (2B58), then the
                // page whole; the colours set after.
                let mut s = Screen::new(after_dac);
                s.fill(0, 0, W as i32, H as i32, 0, 0xFF);
                if let Some(i) = crate::doll::backdrop(&self.archive, m) {
                    s.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                }
                film.whole(&s.indices().iter().map(|&v| v as u8).collect::<Vec<u8>>());
                film.wait();
                film.whole(&under);
                film.colours(&after_dac);
            } else {
                film.colours(&after_dac);
                film.wait();
                film.whole(&under);
            }
        }
        film.dissolve(m, after, FULL);
    }

    /// What is up on the screen, as the routines put it there and take it
    /// away.
    fn up(&self) -> Up {
        if !self.boxes.is_empty() {
            return Up::Box;
        }
        match self.stage {
            Stage::Method(_) | Stage::Gender(_) | Stage::ProvinceYes(..) | Stage::Choose(_) | Stage::ClassYes(..) => Up::Menu,
            Stage::List(_) => Up::List,
            Stage::Name(..) => Up::Prompt,
            Stage::Quiz { .. } => Up::Quiz,
            _ => Up::Other,
        }
    }

    /// The background the screen is drawn on now.
    fn back(&self) -> Back {
        match (&self.boxes.front(), &self.stage) {
            (Some(b), _) => b.back,
            (None, Stage::Method(_) | Stage::Quiz { .. } | Stage::ClassYes(..) | Stage::List(_) | Stage::Name(..) | Stage::Gender(_)) => Back::Start,
            (None, Stage::Map | Stage::ProvinceYes(..)) => Back::Map,
            (None, Stage::Stats | Stage::SheetName(_) | Stage::Choose(_) | Stage::Face) => Back::Stats,
            (None, _) => Back::Black,
        }
    }

    /// The screen as `draw_page` draws it (with the menu, list or prompt
    /// up, or without: the page under them), its indices and colours.
    fn shot(&self, m: &Machine, ui: bool) -> (Vec<u8>, Dac) {
        let mut s = Screen::new(self.pics.game);
        self.draw_page_with(&mut s, m, ui);
        (s.indices().iter().map(|&i| i as u8).collect(), *s.palette())
    }

    /// The change playing passed: the screen as it settles.
    pub fn settle(&mut self) {
        self.film.clear();
        self.film_ms = 0;
    }

    /// Every frame the change will show, in turn: the screen's indices
    /// and the colours (6-bit), at each wait for the retrace.
    #[cfg(test)]
    pub fn film_frames(&self) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.film.iter().map(|(px, dac)| (px.clone(), dac.iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect())).collect()
    }

    fn input_inner(&mut self, engine: &mut Engine, inp: Input) {
        if !self.boxes.is_empty() {
            if matches!(inp, Input::Enter | Input::Esc | Input::Char(b' ') | Input::Click(..)) {
                self.boxes.pop_front();
            }
            return;
        }
        if let Stage::Story { .. } = self.stage {
            // A key ends the whole story (0000:1314, 137F: ACB6).
            if !matches!(inp, Input::Up | Input::Down) {
                self.movie = None;
                if let Stage::Story { next, .. } = std::mem::replace(&mut self.stage, Stage::Done) {
                    self.stage = *next;
                }
            }
            return;
        }
        if matches!(self.stage, Stage::Intro | Stage::Vision | Stage::Ending) {
            // A key the next lines (or, the intro, its end).
            if let Some(mv) = &mut self.movie {
                if !matches!(inp, Input::Up | Input::Down) {
                    if matches!(self.stage, Stage::Intro) {
                        mv.over = true
                    } else {
                        mv.key()
                    }
                }
            }
            return self.movie_over();
        }
        let stage = std::mem::replace(&mut self.stage, Stage::Done);
        self.stage = match stage {
            Stage::Title => match inp {
                Input::Up | Input::Down => Stage::Title,
                _ => Stage::MainMenu,
            },
            Stage::MainMenu => {
                let key = |m: &Machine, i: u16| m.mem.r8(DATA, MENU_REGIONS + i * REGION + 0x59);
                let pick = match inp {
                    Input::Char(c) => (0..3u16).find(|&r| key(&engine.m, r) == c.to_ascii_uppercase()),
                    Input::Click(x, y) => self.region(&engine.m, MENU_REGIONS, 3, x, y),
                    _ => None,
                };
                match pick {
                    Some(0) => {
                        self.outcome = Some(Outcome::Load);
                        Stage::MainMenu
                    }
                    Some(1) => {
                        // 0000:1131: the opening story (0000:1323), then
                        // creation (1194:2498), which starts from a cleared
                        // state (1194:24A6).
                        engine.m.newgame_reset();
                        self.colour = COLOUR_A;
                        let next = Box::new(Stage::Method(self.method(&mut engine.m)));
                        self.movie = crate::flc::Movie::new(&self.archive, "SCROLL.FLC", Vec::new());
                        Stage::Story { pic: 0, ms: 0, img: None, next }
                    }
                    Some(_) => {
                        self.outcome = Some(Outcome::Exit);
                        Stage::MainMenu
                    }
                    None => Stage::MainMenu,
                }
            }
            Stage::Method(mut ask) => match self.ask_input(&mut ask, inp) {
                None => Stage::Method(ask),
                Some(0) => {
                    // 1194:260A: the instructions (in FONT_A), then the
                    // questions.
                    let l = hud::lines(engine.m.mem.cstr(DATA, 0x3547), self.colour);
                    self.push_box(l, FontId::A, Back::Start);
                    if engine.m.quiz_begin(&mut engine.files).is_none() {
                        eprintln!("QUESTION.TXT could not be loaded");
                        self.list(engine)
                    } else {
                        self.next_question(engine, 0)
                    }
                }
                // S, or Esc (FFFFh, not 0): the list.
                Some(_) => self.list(engine),
            },
            Stage::Quiz { asked, mut roll } => match inp {
                Input::Char(c) if (b'A'..=b'C').contains(&c.to_ascii_uppercase()) => {
                    let a = c.to_ascii_uppercase() - b'A';
                    engine.m.quiz_answer(a);
                    // 222C:010B..012B: the answer's kind counted, its
                    // figure played (2294:0895). The next question's
                    // screen covers it; the class offered after the last
                    // is drawn over it.
                    let kind = engine.m.mem.r8(DS, arena::newgame::var::KINDS + a.min(2) as u16);
                    self.pics.cel = self.class_cel(&engine.m, kind);
                    if asked + 1 < 10 {
                        self.next_question(engine, asked + 1)
                    } else {
                        self.quiz_shown = Some(roll);
                        self.quiz_done(engine)
                    }
                }
                // The top roller rolls on (222C:0641), the bottom back
                // (0612): a row each.
                Input::Down | Input::Click(_, 119..=136) => {
                    self.roll_on(&mut roll);
                    Stage::Quiz { asked, roll }
                }
                Input::Up | Input::Click(_, 183..=199) => {
                    self.roll_back(&mut roll);
                    Stage::Quiz { asked, roll }
                }
                _ => Stage::Quiz { asked, roll },
            },
            Stage::ClassYes(mut ask, class) => match self.ask_input(&mut ask, inp) {
                None => Stage::ClassYes(ask, class),
                Some(0) => self.class_chosen(engine, class),
                Some(_) => self.list(engine),
            },
            Stage::List(mut ask) => match self.ask_input(&mut ask, inp) {
                Some(i) if (i as usize) < 18 => {
                    let class = engine.m.mem.r8(DS, LIST_ORDER + i);
                    self.class_chosen(engine, class)
                }
                _ => Stage::List(ask),
            },
            Stage::Name(prompt, mut live) => {
                // The keys to the game's editor (a script's characters and
                // keys as the keys that make them).
                match inp {
                    Input::Key(scan, down) => live.key(scan, down),
                    Input::Char(c) => live.type_char(c),
                    Input::Back => live.tap(0x0E),
                    Input::Enter => live.tap(0x1C),
                    Input::Esc => live.tap(0x01),
                    _ => {}
                }
                if !live.ended() {
                    return self.stage = Stage::Name(prompt, live);
                }
                // 1194:2789: nothing typed (01AD 0) asks again.
                let text = live.text();
                if text.is_empty() {
                    Stage::Name(prompt, self.name_field(engine))
                } else {
                    self.name = String::from_utf8_lossy(&text).into_owned();
                    let ask = self.menu(&mut engine.m, (DATA, 0x3614), &[], 0x3F85, 0x3F88, FontId::A);
                    Stage::Gender(ask)
                }
            }
            Stage::Gender(mut ask) => match self.ask_input(&mut ask, inp) {
                Some(s) if s < 2 => {
                    let name = self.name.clone();
                    engine.m.name_and_sex(name.as_bytes(), s as u8);
                    self.map_screen(engine)
                }
                _ => Stage::Gender(ask),
            },
            Stage::Map => match inp {
                Input::Click(x, y) => match self.region(&engine.m, MAP_REGIONS, 8, x, y) {
                    Some(p) => self.province_clicked(engine, p as u8),
                    None => Stage::Map,
                },
                _ => Stage::Map,
            },
            Stage::ProvinceYes(mut ask, p) => match self.ask_input(&mut ask, inp) {
                None => Stage::ProvinceYes(ask, p),
                Some(0) => self.province_accepted(engine, p),
                // 1194:28DD: no, the question and the map again.
                Some(_) => self.map_screen(engine),
            },
            Stage::Stats => self.stats_input(engine, inp),
            Stage::SheetName(mut live) => {
                match inp {
                    Input::Key(scan, down) => live.key(scan, down),
                    Input::Char(c) => live.type_char(c),
                    Input::Back => live.tap(0x0E),
                    Input::Enter => live.tap(0x1C),
                    Input::Esc => live.tap(0x01),
                    _ => {}
                }
                if !live.ended() {
                    return self.stage = Stage::SheetName(live);
                }
                // Into 01AD, the page drawn again (1564..1578).
                let mut name = [0u8; 0x15];
                for (n, &b) in name.iter_mut().zip(live.text().iter().take(0x14)) {
                    *n = b;
                }
                for (i, &b) in name.iter().enumerate() {
                    engine.m.mem.w8(DS, 0x01AD + i as u16, b);
                }
                if let Some(v) = &mut self.level_up {
                    v.push(Click::Name(name));
                }
                Stage::Stats
            }
            Stage::Choose(mut ask) => match self.ask_input(&mut ask, inp) {
                None => Stage::Choose(ask),
                Some(0) => {
                    if engine.m.mem.r8(DS, arena::newgame::var::POINTS) != 0 {
                        // 1194:2D1C: in ARENAFNT.
                        let l = hud::lines(engine.m.mem.cstr(DS, 0x3F2B), self.colour);
                        self.push_box(l, FontId::Arena, Back::Stats);
                        Stage::Stats
                    } else {
                        // The page as the attributes leave it, for the faces.
                        let mut page = Screen::new(self.pics.charsheet);
                        page.fill(0, 0, W as i32, H as i32, 0, 0xFF);
                        self.stats_page(&mut page, &engine.m, true, true);
                        let back = page.indices().to_vec();
                        // The head's corner as 2A9D kept it before the doll
                        // was drawn (the race's background alone, 161A),
                        // put back under each face (2C96).
                        let mut bare = Screen::new(self.pics.charsheet);
                        bare.fill(0, 0, W as i32, H as i32, 0, 0xFF);
                        let m = &engine.m;
                        let bg = if m.mem.r8(DS, arena::character::PLAYER + 7) != 0 { "CHRBKF" } else { "CHARBK" };
                        let bg = format!("{bg}{:02}.IMG", m.mem.r8(DS, arena::character::PLAYER + 4));
                        if let Some(i) = crate::panels::load_image(&self.archive, m, &bg) {
                            bare.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                        }
                        let corner = (0..0x37).flat_map(|y| (0xDA..0xDA + 0x4A).map(move |x| y * W + x)).map(|i| bare.indices()[i]).collect();
                        self.face_back = Some((back, corner));
                        engine.m.creation_finish();
                        self.face_screen(engine)
                    }
                }
                // Reroll, or Esc (FFFFh, not 0).
                Some(_) => self.roll(engine),
            },
            Stage::Face => self.face_input(engine, inp),
            s @ (Stage::Intro | Stage::Vision | Stage::Ending | Stage::Story { .. }) => s,
            Stage::Done => Stage::Done,
        };
    }

    /// 1194:2678: the class list, its names in the order of 2AEE, six rows,
    /// in FONT_C, under "Choose thy class...".
    fn list(&mut self, engine: &mut Engine) -> Stage {
        let m = &engine.m;
        let entries = (0..18u16)
            .map(|i| {
                let c = m.mem.r8(DS, LIST_ORDER + i);
                let (s, o) = Self::class_of(m, c);
                // In the list's own colours (5734's BL, FDh; the title 01h).
                vec![Span { colour: 0x0F, text: String::from_utf8_lossy(m.mem.cstr(s, o)).into_owned(), font: Default::default(), code: None }]
            })
            .collect();
        let title = hud::lines(m.mem.cstr(DS, 0x3F56), 0x0F);
        Stage::List(Ask { title, entries, keys: Vec::new(), sel: 0, top: 0, rows: 6, font: FontId::C, moved: false })
    }

    /// The next question (number `asked` from 0), its text's lines.
    fn next_question(&mut self, engine: &mut Engine, asked: u8) -> Stage {
        let Some((seg, di)) = engine.m.quiz_question() else {
            eprintln!("a question could not be found");
            return self.list(engine);
        };
        let text = engine.m.mem.cstr(seg, di).to_vec();
        Stage::Quiz { asked, roll: self.roll_new(&text) }
    }

    /// The scroll for question `text` (222C:02D4, 034C): the strings, the
    /// first `last` drawn from the bitmap's top.
    fn roll_new(&self, text: &[u8]) -> Roll {
        let mut buf = vec![b'\n'];
        buf.extend_from_slice(&text[..text.len().saturating_sub(1)]);
        buf.extend_from_slice(b"\n\n");
        let count = buf.iter().filter(|&&b| b == b'\n').count();
        let strings = buf.split(|&b| b == b'\n').map(|l| String::from_utf8_lossy(l).trim_end_matches('\r').to_string()).collect();
        let mut roll = Roll { strings, bitmap: vec![0; W * ROLL_ROWS], line: 0, off: 0, frame: 0, last: count.min(8), rollers: vec![0; W * H] };
        self.roll_frame(&mut roll);
        let last = roll.last;
        self.roll_draw(&mut roll, 0, 0, last);
        roll
    }

    /// 0000:9E06 into the bitmap: `n` strings from `first`, a line every
    /// 8 rows from row `y` (past the bitmap: not kept).
    fn roll_draw(&self, roll: &mut Roll, first: usize, y: usize, n: usize) {
        let mut s = Screen::new(self.pics.game);
        s.fill(0, 0, W as i32, H as i32, 0, 0xFF);
        s.blit(0, 0, W, ROLL_ROWS, &roll.bitmap, H as i32);
        for j in 0..n {
            let row = y + 8 * j;
            if row >= ROLL_ROWS {
                break;
            }
            // Past the strings the original reads on into the heap.
            if let Some(t) = roll.strings.get(first + j) {
                s.text_plain(&self.fonts.teeny, 0x14, row as i32, t, COLOUR_A);
            }
        }
        for (b, &i) in roll.bitmap.iter_mut().zip(s.indices()) {
            *b = i as u8;
        }
    }

    /// The rollers' frame now drawn over those before (03B1).
    fn roll_frame(&self, roll: &mut Roll) {
        if let Some(i) = self.pics.rollers.get(roll.frame) {
            for r in 0..i.h {
                for c in 0..i.w {
                    let (x, y) = (i.x + c as i32, i.y + r as i32);
                    let v = i.px[r * i.w + c];
                    if v != 0 && (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
                        roll.rollers[y as usize * W + x as usize] = v;
                    }
                }
            }
        }
    }

    /// 222C:045D: a row on; past the bitmap's first line, the lines
    /// shifted up (04BC) and the next drawn into the last (04E5 at 38h),
    /// unless the top is now the last.
    fn roll_on(&self, roll: &mut Roll) {
        if roll.line > roll.last {
            return;
        }
        if roll.off != 7 {
            roll.off += 1;
            roll.frame = (roll.frame + 1) & 15;
            return self.roll_frame(roll);
        }
        roll.off = 0;
        roll.bitmap.copy_within(8 * W..ROLL_ROWS * W, 0);
        roll.line += 1;
        if roll.line == roll.last {
            return;
        }
        roll.frame = (roll.frame + 1) & 15;
        self.roll_frame(roll);
        roll.bitmap[56 * W..].fill(0);
        let first = roll.line + 7;
        self.roll_draw(roll, first, 56, 0x140);
    }

    /// 222C:040B: a row back; at the bitmap's top, the lines shifted down
    /// and the one before drawn into the first (04E5 at 0, the strings
    /// after it over the rest).
    fn roll_back(&self, roll: &mut Roll) {
        if roll.off != 0 {
            roll.off -= 1;
            roll.frame = (roll.frame + 15) & 15;
            return self.roll_frame(roll);
        }
        if roll.line == 0 {
            return;
        }
        roll.off = 7;
        roll.frame = (roll.frame + 15) & 15;
        self.roll_frame(roll);
        roll.bitmap.copy_within(0..56 * W, 8 * W);
        roll.line -= 1;
        roll.bitmap[..8 * W].fill(0);
        let first = roll.line;
        self.roll_draw(roll, first, 0, 0x140);
    }

    /// The figure of answers of `kind` (222C:011A: the name at
    /// [4142 + 2 kind]: WARRIOR.CEL, ROGUE.CEL, MAGE.CEL) as 0000:6546
    /// leaves it: every frame played, the last kept; placed by the file's
    /// CEL_DATA (0000:6102..612D: an F100 chunk first whose first part is
    /// of type 3: its centre less half the size).
    fn class_cel(&self, m: &Machine, kind: u8) -> Option<Image> {
        let at = m.mem.r16(DS, 0x4142 + 2 * kind as u16);
        let name = String::from_utf8_lossy(m.mem.cstr(DS, at)).to_ascii_uppercase();
        let d = self.archive.read(&name)?;
        let le = |i: usize| u16::from_le_bytes([*d.get(i).unwrap_or(&0), *d.get(i + 1).unwrap_or(&0)]);
        let mut f = crate::flc::Flc::open(&self.archive, &name)?;
        // Each frame drawn over the page with its 0s see-through (A902
        // bit 400h): what a frame drew stays where later ones have 0.
        let mut px = f.px.clone();
        while (f.shown as usize) < le(6) as usize {
            f.step();
            for (p, &v) in px.iter_mut().zip(&f.px) {
                if v != 0 {
                    *p = v;
                }
            }
        }
        let (mut x, mut y) = (0i32, 0i32);
        if le(132) == 0xF100 && le(138) == 3 {
            x = le(142).wrapping_sub((f.w >> 1) as u16) as i16 as i32;
            y = le(144).wrapping_sub((f.h >> 1) as u16) as i16 as i32;
        }
        Some(Image { x, y, w: f.w, h: f.h, px })
    }

    /// The ten answered: the class they give, offered (1194:261F..264E;
    /// the class byte kept in the record as the original keeps it).
    fn quiz_done(&mut self, engine: &mut Engine) -> Stage {
        let Some(class) = engine.m.quiz_end(&mut engine.files) else {
            eprintln!("CLASSES.DAT could not be loaded");
            return self.list(engine);
        };
        engine.m.mem.w8(DS, CLASS, class);
        let (s, o) = Self::class_of(&engine.m, class);
        let ask = self.yes_no(&mut engine.m, 0x3651, &[Arg::Str(s, o)]);
        Stage::ClassYes(ask, class)
    }

    /// The class chosen (1194:2668..2674), then the name asked (2711:
    /// 3276:35F8 with the class's name, in FONT_A).
    fn class_chosen(&mut self, engine: &mut Engine, class: u8) -> Stage {
        if engine.m.class_chosen(class).is_none() {
            eprintln!("the class's weapon could not be made");
        }
        let (cs, co) = Self::class_of(&engine.m, class);
        let m = &mut engine.m;
        m.sprintf((DS, BUF), (DATA, 0x35F8), &[Arg::Str(cs, co)]);
        // 2731..2776: the text's last character (its CR) made room, two
        // for each letter the class's name is short of ten, less four,
        // then the CR again; then 5C34's box text (4FBC: the field's
        // line of room under it).
        let mut text = m.mem.cstr(DS, BUF).to_vec();
        let short = 10 - m.mem.cstr(cs, co).len() as i32;
        let pad = if short == 0 { 0 } else { (2 * short - 4).max(0) as usize };
        if pad != 0 {
            text.pop();
            text.extend(std::iter::repeat_n(b' ', pad));
            text.push(b'\r');
        }
        for (i, &b) in text.iter().chain(&[0]).enumerate() {
            m.mem.w8(DS, BUF + i as u16, b);
        }
        let prompt = hud::lines(crate::field::prompt_5c30(m, DS, BUF).as_bytes(), self.colour);
        Stage::Name(prompt, self.name_field(engine))
    }

    /// 1194:2786: 5C34 with AX 3Ch, CX 12h, DI 01AD: the name's field
    /// (`lineedit::fields::creation_name`), the game's editor opened on a
    /// copy of the game, in FONT_A.DAT ([1026], 271D) and the screen's
    /// colour.
    fn name_field(&self, engine: &Engine) -> Box<crate::field::Live> {
        let look = crate::field::Look { font: "FONT_A.DAT", colour: self.colour, put_font: true };
        let field = arena::lineedit::fields::creation_name();
        Box::new(crate::field::Live::open(&engine.m, field, look).expect("the name's field opens"))
    }

    /// 1194:1528: the sheet's name, 9EE5 with AX 0Bh, BX 7, CX 0, DX 14h,
    /// DI A0h, BP 0Ch, SI 01AD, in the page's font and colour (0DE1's).
    fn sheet_name_field(&self, m: &Machine) -> Box<crate::field::Live> {
        let look = crate::field::Look { font: "ARENAFNT.DAT", colour: COLOUR_B, put_font: true };
        let field = arena::lineedit::Field { x: 0x0B, y: 7, click: 0, max: 0x14, seg: DS, off: 0x01AD, w: 0xA0, h: 0x0C };
        Box::new(crate::field::Live::open(m, field, look).expect("the name's field opens"))
    }

    /// Whether the name is being typed (the keys, pressed and let go,
    /// go to it as `Input::Key`).
    pub fn typing(&self) -> bool {
        self.boxes.is_empty() && matches!(self.stage, Stage::Name(..) | Stage::SheetName(..))
    }

    /// A frame of the window: the name's field's (`Live::frame`).
    pub fn field_frame(&mut self) {
        if let Stage::Name(_, live) | Stage::SheetName(live) = &mut self.stage {
            live.frame();
        }
    }

    /// The map and its question (1194:27C2..2868).
    fn map_screen(&mut self, engine: &mut Engine) -> Stage {
        let m = &mut engine.m;
        let (cs, co) = m.class_name(DS, PLAYER);
        let l = self.say(m, (DATA, 0x362A), &[Arg::Str(DS, NAME), Arg::Str(cs, co)]);
        self.push_box(l, FontId::A, Back::Map);
        Stage::Map
    }

    /// A province clicked (1194:28F6, 289B..28D8): its race, the portrait
    /// rolled, and the question.
    fn province_clicked(&mut self, engine: &mut Engine, p: u8) -> Stage {
        if engine.m.province_clicked(p).is_none() {
            eprintln!("the portrait could not be rolled");
        }
        let (ps, po) = Self::nth(&engine.m, PROVINCES, p);
        let (rs, ro) = Self::nth(&engine.m, RACES, p);
        let ask = self.yes_no(&mut engine.m, 0x369F, &[Arg::Str(ps, po), Arg::Str(rs, ro)]);
        Stage::ProvinceYes(ask, p)
    }

    /// The province accepted (28E1), what the player is told (2902), then
    /// the attributes (2A69).
    fn province_accepted(&mut self, engine: &mut Engine, p: u8) -> Stage {
        if engine.m.province_accepted(&mut engine.files).is_none() {
            eprintln!("the place could not be built");
        }
        let m = &mut engine.m;
        let (cs, co) = m.class_name(DS, PLAYER);
        let (ps, po) = Self::nth(m, PROVINCES, p);
        let (rs, ro) = Self::nth(m, RACES, p);
        m.sprintf((DS, BUF), (DATA, 0x36EC), &[Arg::Str(cs, co), Arg::Str(DS, NAME), Arg::Str(ps, po), Arg::Str(rs, ro)]);
        let mut first = m.mem.cstr(DS, BUF).to_vec();
        // 1194:2945..2970: "his home" becomes "her home" for a woman.
        if m.mem.r8(DS, SEX) != 0 {
            if let Some(i) = first.windows(8).position(|w| w == b"his home") {
                first[i..i + 3].copy_from_slice(b"her");
            }
        }
        let colour = self.colour;
        self.push_box(hud::lines(&first, colour), FontId::Arena, Back::Map);
        // 1194:29DE: the province's text (581h + the province) after
        // "Know ye this also:", wrapped at 28h.
        m.mem.w16(DS, 0x137A, 0x581 + p as u16);
        m.mem.w8(DS, 0x137E, 0x28);
        m.mem.w8(DS, 0x101C, 1);
        let mut known = b"Know ye this also:\r".to_vec();
        match m.text(&mut engine.files, &mut arena::text::NoTextHooks, 2, DS, BUF) {
            Some(_) => {
                // 1194:2A4E: each line break the wrap made (a LF) made a
                // CR (49DF), so 965A centres every line.
                known.extend(engine.m.mem.cstr(DS, BUF).iter().map(|&c| if c == 0x0A { 0x0D } else { c }));
                // 2A51..2A5F: the text ended by a CR (its last line
                // centred too).
                known.push(b'\r');
            }
            None => eprintln!("the province's text could not be made"),
        }
        self.push_box(hud::lines(&known, colour), FontId::Arena, Back::Map);
        let m = &mut engine.m;
        let class = m.mem.r8(DS, CLASS) & 0x1F;
        let adj = (DATA, m.skip_strings(DATA, 0x38D4, class as u16));
        let (cs, co) = m.class_name(DS, PLAYER);
        let l = self.say(m, (DATA, 0x3742), &[Arg::Str(adj.0, adj.1), Arg::Str(cs, co)]);
        self.push_box(l, FontId::Arena, Back::Map);
        let l = hud::lines(engine.m.mem.cstr(DATA, 0x3780), colour);
        self.push_box(l, FontId::Arena, Back::Map);
        self.roll(engine)
    }

    /// The attributes rolled (1194:2AC7; the first time its note).
    fn roll(&mut self, engine: &mut Engine) -> Stage {
        self.colour = COLOUR_B;
        if engine.m.creation_roll(&mut Hooks { boxes: &mut self.boxes }).is_none() {
            eprintln!("the attributes could not be rolled");
        }
        Stage::Stats
    }

    /// A region of `n` from DATA:`table` under (x, y).
    fn region(&self, m: &Machine, table: u16, n: u16, x: i32, y: i32) -> Option<u16> {
        (0..n).find(|&i| {
            let r = |k: u16| m.mem.r16(DATA, table + i * REGION + 2 * k) as i16 as i32;
            let (rx, ry, rw, rh) = (r(0), r(1), r(2), r(3));
            rw > 0 && rh > 0 && (rx..rx + rw).contains(&x) && (ry..ry + rh).contains(&y)
        })
    }

    /// The attribute screen's regions (3276:5312): 0 the button at the
    /// bottom left (Esc), 1..8 the attributes' lines (their keys at +59h),
    /// 9 the arrows (where the line picked moved them).
    fn stats_input(&mut self, engine: &mut Engine, inp: Input) -> Stage {
        let m = &mut engine.m;
        let key = |m: &Machine, i: u16| m.mem.r8(DATA, STAT_REGIONS + i * REGION + 0x59);
        let done = |c: &mut Self, m: &mut Machine| {
            if let Some(clicks) = &c.level_up {
                // 1194:122C's end: refused (3E17) while points are left.
                if m.mem.r8(DS, arena::newgame::var::POINTS) != 0 {
                    let l = hud::lines(m.mem.cstr(DS, 0x3E17), COLOUR_B);
                    c.push_box(l, FontId::Arena, Back::Stats);
                } else {
                    c.outcome = Some(Outcome::Spent(clicks.clone()));
                }
                return Stage::Stats;
            }
            // 1194:2CD0: "Which dost thou choose?", in FONT_A.
            Stage::Choose(c.menu(m, (DATA, 0x3A43), &[], 0x3F9C, 0x3F9F, FontId::A))
        };
        let record = |c: &mut Self, k: Click| {
            if let Some(v) = &mut c.level_up {
                v.push(k);
            }
        };
        match inp {
            Input::Esc => done(self, m),
            Input::Char(c) => {
                let c = c.to_ascii_uppercase();
                if let Some(r) = (1..=8u16).find(|&r| key(m, r) == c) {
                    m.creation_select(r);
                    record(self, Click::Select(r));
                }
                Stage::Stats
            }
            // The arrows' halves (a convenience: the original's are clicked).
            Input::Up | Input::Down => {
                let mid = m.mem.r8(DS, arena::newgame::var::ARROWS) as u16;
                let y = if inp == Input::Up { mid } else { mid + 1 };
                m.creation_arrow(y);
                record(self, Click::Arrow(y));
                Stage::Stats
            }
            Input::Click(x, y) => match self.region(m, STAT_REGIONS, 10, x, y) {
                Some(0) => done(self, m),
                Some(r @ 1..=8) => {
                    m.creation_select(r);
                    record(self, Click::Select(r));
                    Stage::Stats
                }
                Some(9) => {
                    m.creation_arrow(y as u16);
                    record(self, Click::Arrow(y as u16));
                    Stage::Stats
                }
                // The name (3276:56E6: 0Bh, 7, 64h x 0Ah): typed over.
                None if (0x0B..0x0B + 0x64).contains(&x) && (7..7 + 0x0A).contains(&y) => Stage::SheetName(self.sheet_name_field(m)),
                _ => Stage::Stats,
            },
            _ => Stage::Stats,
        }
    }

    /// 1194:2BC5: the note, then the faces of the player's race and sex
    /// (FACES1r.CIF, FACESF1r.CIF while creating), from the first.
    fn face_screen(&mut self, engine: &mut Engine) -> Stage {
        let m = &mut engine.m;
        let l = hud::lines(m.mem.cstr(DATA, 0x3824), COLOUR_B);
        self.push_box(l, FontId::Arena, Back::Stats);
        let name = format!("{}1{}.CIF", if m.mem.r8(DS, SEX) != 0 { "FACESF" } else { "FACES" }, m.mem.r8(DS, RACE));
        self.pics.faces = self.archive.read(&name).map(|d| cif(m, &d)).unwrap_or_default();
        let robe = if m.mem.r8(DS, CLASS) & 0x20 != 0 { 'R' } else { 'S' };
        let shirt = format!("{}{robe}SHIRT.IMG", if m.mem.r8(DS, SEX) != 0 { 'F' } else { 'M' });
        self.pics.shirt = crate::panels::load_image(&self.archive, m, &shirt);
        self.pics.shirt_file = self.archive.read(&shirt).unwrap_or_default();
        m.mem.w16(DS, arena::newgame::var::FACE_PICKED, 0);
        // 2DAF: the faces' file into bank 17h's first bytes.
        if let Some(d) = self.archive.read(&name) {
            with_bank(m, FACES_BANK, |m, frame| {
                for (i, &b) in d.iter().enumerate().take(0x10000) {
                    m.mem.w8(frame, i as u16, b);
                }
            });
        }
        self.show_face(m);
        Stage::Face
    }

    /// One pass of the faces' loop (1194:2C04..2C1F): the face picked
    /// drawn from bank 17h by 5828, then the shirt (2486: its file loaded
    /// again into bank [1030], 56C8, and drawn). 5828 decodes a coded
    /// image into w x h bytes from the heap (51EC; the faces' coding,
    /// 3065:0D2D, takes its tables from the heap after them) and frees
    /// them (6C36), so the decoder's ring holds what the decodes before
    /// left there: a face after the first can take pixels from them.
    fn show_face(&mut self, m: &mut Machine) {
        let k = m.mem.r8(DS, arena::newgame::var::FACE_PICKED) as usize;
        self.pics.face_now = with_bank(m, FACES_BANK, |m, frame| {
            let mut at = 0u16;
            for _ in 0..k {
                at = at.wrapping_add(m.mem.r16(frame, at.wrapping_add(10))).wrapping_add(12);
            }
            decode_5828(m, frame, at)
        })
        .or_else(|| self.pics.faces.get(k).cloned());
        let bank = m.mem.r16(DS, 0x1030);
        let file = std::mem::take(&mut self.pics.shirt_file);
        self.pics.shirt_now = with_bank(m, bank, |m, frame| {
            for (i, &b) in file.iter().enumerate().take(0x10000) {
                m.mem.w8(frame, i as u16, b);
            }
            decode_5828(m, frame, 0)
        })
        .or_else(|| self.pics.shirt.clone());
        self.pics.shirt_file = file;
    }

    /// The faces' regions (3276:5E90): 0 the face (the next one, after the
    /// tenth the first), 1 Done (Esc).
    fn face_input(&mut self, engine: &mut Engine, inp: Input) -> Stage {
        let m = &mut engine.m;
        let next = |m: &mut Machine| {
            let f = m.mem.r8(DS, arena::newgame::var::FACE_PICKED);
            m.mem.w8(DS, arena::newgame::var::FACE_PICKED, if f == 9 { 0 } else { f + 1 });
        };
        let region = match inp {
            Input::Click(x, y) => self.region(m, FACE_REGIONS, 2, x, y),
            Input::Esc => Some(1),
            Input::Char(b' ') | Input::Down | Input::Up => Some(0),
            _ => None,
        };
        match region {
            Some(0) => {
                next(m);
                self.show_face(m);
                Stage::Face
            }
            Some(1) => self.finish(engine),
            _ => Stage::Face,
        }
    }

    /// The face kept (2C72), creation's end (257B..25B7), then the vision
    /// (0000:113E..115D: text 578h through 1194:D459).
    fn finish(&mut self, engine: &mut Engine) -> Stage {
        let m = &mut engine.m;
        m.face_chosen();
        m.creation_end(&mut arena::newgame::NoNewGameHooks);
        self.colour = 0x2F;
        let Some((seg, off)) = m.heap_alloc_bytes(0xBB8) else {
            return Stage::Done;
        };
        m.mem.w16(DS, 0x137A, 0x578);
        m.mem.w8(DS, 0x137E, 0);
        m.mem.w8(DS, 0x101C, 0);
        let text = match m.text(&mut engine.files, &mut arena::text::NoTextHooks, 2, seg, off) {
            Some(_) => engine.m.mem.cstr(seg, off).to_vec(),
            None => {
                eprintln!("the vision's text could not be made");
                Vec::new()
            }
        };
        engine.m.heap_free(seg);
        let lines = hud::lines(&text, 0x2F);
        if lines.iter().all(|l| l.is_empty()) {
            return Stage::Done;
        }
        self.movie = Movie::new(&self.archive, "VISION.FLC", lines);
        if self.movie.is_some() { Stage::Vision } else { Stage::Done }
    }

    /// A menu's or list's answer to `inp`: an entry, FFFFh for Esc, or
    /// None while it waits.
    fn ask_input(&self, ask: &mut Ask, inp: Input) -> Option<u16> {
        let n = ask.entries.len();
        let rows = if ask.rows == 0 { n } else { ask.rows.min(n) };
        match inp {
            Input::Up => {
                ask.sel = ask.sel.saturating_sub(1);
                ask.moved = true;
            }
            Input::Down => {
                ask.sel = (ask.sel + 1).min(n.saturating_sub(1));
                ask.moved = true;
            }
            Input::Enter => return Some(ask.sel as u16),
            Input::Esc => return Some(0xFFFF),
            Input::Char(c) => {
                let c = c.to_ascii_uppercase();
                if let Some(i) = ask.keys.iter().position(|&k| k.to_ascii_uppercase() == c) {
                    return Some(i as u16);
                }
            }
            Input::Click(x, y) => {
                let f = self.fonts.get(ask.font);
                if ask.rows != 0 {
                    return hud::custom::list_pick(&self.fonts.a, &self.list_menu(ask), x, y).map(|i| i as u16);
                }
                let tiles = self.tiles(Back::Start)?;
                let (_, rects) = hud::custom::menu_rects(tiles, f, &ask.title, &ask.entries);
                return rects.iter().position(|r| (r.0..r.0 + r.2).contains(&x) && (r.1..r.1 + r.3).contains(&y)).map(|i| i as u16);
            }
            Input::Back | Input::Wait(_) | Input::Key(..) => {}
        }
        if ask.sel < ask.top {
            ask.top = ask.sel;
        } else if ask.sel >= ask.top + rows {
            ask.top = ask.sel + 1 - rows;
        }
        None
    }

    /// The tiles boxes are drawn with on the screen `back`.
    fn tiles(&self, back: Back) -> Option<&hud::Tiles> {
        match back {
            Back::Stats => self.pics.marble2.as_ref(),
            _ => self.pics.parch.as_ref(),
        }
    }

    /// The list `ask` as 1194:5734 shows the classes' (2678: kind 6,
    /// POPUP2.IMG, its title drawn by 5734).
    fn list_menu(&self, ask: &Ask) -> hud::Menu {
        hud::Menu { title: ask.title.clone(), entries: ask.entries.clone(), keys: Vec::new(), selected: ask.sel, list: true, kind: 6, font: None }
    }

    /// The layer drawn: the screen behind, the stage, a box waiting.
    pub fn draw(&self, screen: &mut Screen, m: &Machine) {
        self.draw_page(screen, m);
        if let Some((px, dac)) = self.film.front() {
            screen.set_palette(*dac);
            screen.fill(0, 0, W as i32, H as i32, 0, 0xFF);
            screen.blit(0, 0, W, H, px, H as i32);
        }
    }

    /// The screen as it settles (the page 9466 shows).
    pub fn draw_page(&self, screen: &mut Screen, m: &Machine) {
        self.draw_page_with(screen, m, true)
    }

    /// The screen, with its box, menu, list or prompt (`ui`) or without
    /// them (the page they're put over).
    fn draw_page_with(&self, screen: &mut Screen, m: &Machine, ui: bool) {
        let back = self.back();
        if let (Some(mv), None) = (&self.movie, self.boxes.front()) {
            return mv.draw(screen, &self.fonts.arena);
        }
        if self.boxes.is_empty() {
            let pic = match &self.stage {
                Stage::Title => self.title.as_ref(),
                Stage::MainMenu => self.menu_pic.as_ref(),
                Stage::Story { img, .. } => img.as_deref(),
                _ => None,
            };
            if let Some((i, pal)) = pic {
                screen.set_palette(*pal);
                screen.fill(0, 0, W as i32, H as i32, 0, 0xFF);
                screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                return;
            }
        }
        self.draw_back(screen, m, back);
        if let Some(b) = self.boxes.front() {
            if ui {
                self.draw_box(screen, &b.lines, b.font, back, 5);
            }
            return;
        }
        match &self.stage {
            Stage::ClassYes(a, _) => {
                // Over the questions as the last answer left them.
                if let Some(roll) = &self.quiz_shown {
                    self.draw_quiz(screen, m, roll);
                }
                if let Some(i) = &self.pics.cel {
                    screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                }
                if ui {
                    self.draw_ask(screen, a, back);
                }
            }
            Stage::Method(a) | Stage::List(a) | Stage::Gender(a) | Stage::ProvinceYes(a, _) | Stage::Choose(a) if ui => self.draw_ask(screen, a, back),
            Stage::Quiz { roll, .. } => self.draw_quiz(screen, m, roll),
            Stage::SheetName(live) if ui => {
                // 1194:1531: the name's strip in FBh, the field over it.
                screen.fill(0x0B, 7, 0x96, 9, 0xFB, 0xFF);
                live.draw(screen, false);
            }
            Stage::Name(prompt, live) if ui => {
                // The prompt's box with its line of room (5C34's "%s\r"
                // and spaces), the field over it where 5C34 puts it.
                self.draw_box(screen, prompt, FontId::A, back, 10);
                live.draw(screen, false);
            }
            _ => {}
        }
    }

    /// The questions' scroll (its rollers' frame, 222C:03B1) with the
    /// text's bitmap from the rows rolled past its top line, 48 rows into
    /// rows 136..183, its 0s see-through (03E9), over the figures as lit.
    fn draw_quiz(&self, screen: &mut Screen, m: &Machine, roll: &Roll) {
        self.quiz_figures(screen, m);
        screen.blit(0, 0, W, H, &roll.rollers, H as i32);
        let at = roll.off * W;
        screen.blit(0, 136, W, 48, &roll.bitmap[at..at + 48 * W], H as i32);
    }

    /// The figures behind the questions lit as the answers go (222C:0149:
    /// each kind's colour, 40E4, the blue of its count, 40DA).
    fn quiz_figures(&self, screen: &mut Screen, m: &Machine) {
        use arena::newgame::var::COUNTS;
        for kind in 0..3u16 {
            let n = m.mem.r8(DS, COUNTS + kind);
            if n == 0 {
                continue;
            }
            let blue = m.mem.r8(DS, 0x40DA + (n - 1) as u16) & 0x3F;
            screen.set_colour(m.mem.r8(DS, 0x40E4 + kind), [0, 0, blue << 2 | blue >> 4, 0xFF]);
        }
    }

    /// The screen behind: its picture, set in its colours.
    fn draw_back(&self, screen: &mut Screen, m: &Machine, back: Back) {
        let blit = |screen: &mut Screen, i: &Image| screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
        let pal = match back {
            Back::Start => self.pics.start.as_ref().map_or(self.pics.game, |p| p.1),
            Back::Map => self.pics.map.as_ref().map_or(self.pics.game, |p| p.1),
            Back::Stats => self.pics.charsheet,
            Back::Black => self.pics.game,
        };
        screen.set_palette(pal);
        screen.fill(0, 0, W as i32, H as i32, 0, 0xFF);
        match back {
            Back::Start => {
                if let Some((i, _)) = &self.pics.start {
                    blit(screen, i);
                }
            }
            Back::Map => {
                if let Some((i, _)) = &self.pics.map {
                    blit(screen, i);
                }
                for i in [&self.pics.noexit].into_iter().flatten() {
                    blit(screen, i);
                }
            }
            Back::Stats => self.draw_stats(screen, m),
            Back::Black => {}
        }
    }

    /// The attribute screen (1194:0DE1's texts, in ARENAFNT, colour FEh).
    fn draw_stats(&self, screen: &mut Screen, m: &Machine) {
        if let (Stage::Face, Some((back, corner))) = (&self.stage, &self.face_back) {
            // The faces' screen (2BC5) draws on the page the attributes
            // left (their last values, the points and the arrows: 2D28's
            // changes aren't drawn), each face over the doll's head.
            let px: Vec<u8> = back.iter().map(|&i| i as u8).collect();
            screen.blit(0, 0, W, H, &px, H as i32);
            if self.boxes.is_empty() {
                // The loop's pass (2C04): the corner put back, the face.
                // (599C copies it whole: its 0s too.)
                let px: Vec<u8> = corner.iter().map(|&i| i as u8).collect();
                screen.fill(0xDA, 0, 0x4A, 0x37, 0, 0xFF);
                screen.blit(0xDA, 0, 0x4A, 0x37, &px, H as i32);
                if let Some(i) = &self.pics.face_now {
                    screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                }
                // Then the shirt again, over the face's neck (2486).
                if let Some(i) = &self.pics.shirt_now {
                    screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
                }
            }
            return;
        }
        let creating = m.mem.r16(DS, 0x05C8) & 0x10 != 0 || self.level_up.is_some();
        self.stats_page(screen, m, matches!(self.stage, Stage::Stats | Stage::SheetName(_) | Stage::Choose(_)), creating);
    }

    /// The attribute page as 0DC7 draws it (from 2B58 and 122F's redraw
    /// 12AE): the doll (16AA: the body and what is worn, 1194:23E5),
    /// CHARSTAT.IMG over it, 0DE1's texts (the derived numbers, 0F08,
    /// among them; while `creating`, 5C8 bit 10h, the attributes as they
    /// are, else on the 100 scale, 1194:5B07) and, with `points`, the
    /// bonus points and the arrows.
    fn stats_page(&self, screen: &mut Screen, m: &Machine, points: bool, creating: bool) {
        let f = &self.fonts.arena;
        let c = COLOUR_B;
        for i in crate::doll::images(&self.archive, m) {
            screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
        }
        if let Some(i) = &self.pics.charstat {
            screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
        }
        crate::sheets::sheet_texts(screen, f, m, creating);
        if points {
            if let Some(i) = &self.pics.bonus {
                screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
            }
            screen.text_plain(f, 0x5C, 0x71, &m.mem.r8(DS, arena::newgame::var::POINTS).to_string(), c);
            // The arrows where the line picked put them (region 9's y).
            let ay = m.mem.r16(DATA, STAT_REGIONS + 9 * REGION + 2) as i32;
            if self.pics.updown.len() >= 128 {
                screen.blit(38, ay, 8, 16, &self.pics.updown[..128], H as i32);
            }
        }
    }

    /// A box with `lines` as 965A draws it with margin `margin` (5 for
    /// 1194:59CC's, 0Ah for 5C34's prompt), centred, in the tiles of
    /// the screen `back`.
    fn draw_box(&self, screen: &mut Screen, lines: &[Vec<Span>], font: FontId, back: Back, margin: i32) {
        if let Some(t) = self.tiles(back) {
            hud::custom::draw_box(screen, t, self.fonts.get(font), lines, margin, self.colour);
        }
    }

    /// A menu (1194:020E: its title's box and a box for each entry, as
    /// 3159:0F2A lays them out) or a list (5734); the entry the arrows
    /// moved to marked (a view's addition).
    fn draw_ask(&self, screen: &mut Screen, ask: &Ask, back: Back) {
        let f = self.fonts.get(ask.font);
        if ask.rows != 0 {
            // The title in FONT_C, the lines in FONT_A (1194:5914).
            return hud::custom::draw_list(screen, &self.fonts.a, f, &self.list_menu(ask));
        }
        let Some(t) = self.tiles(back) else { return };
        hud::custom::draw_menu(screen, t, f, &ask.title, &ask.entries, self.colour);
        if ask.moved {
            let (_, rects) = hud::custom::menu_rects(t, f, &ask.title, &ask.entries);
            if let Some(&(x, y, w, h)) = rects.get(ask.sel) {
                for (fx, fy, fw, fh) in [(x + 2, y + 2, w - 4, 1), (x + 2, y + h - 3, w - 4, 1), (x + 2, y + 2, 1, h - 4), (x + w - 3, y + 2, 1, h - 4)] {
                    screen.fill(fx, fy, fw, fh, self.colour, 0xFF);
                }
            }
        }
    }

    /// The layer as a picture (2 x, RGBA), for `--shot`.
    pub fn save_png(screen: &Screen, out: &str) {
        let (w, h) = (W * 2, H * 2);
        let mut data = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let p = screen.px[(y / 2) * W + x / 2];
                data.extend_from_slice(&[p[0], p[1], p[2], 0xFF]);
            }
        }
        let file = std::fs::File::create(out).expect("create");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().expect("png").write_image_data(&data).expect("png");
    }
}

/// INTROnn.IMG (`n` 1..9; the digit at 4DD7 counted up, 0000:137B),
/// shown in the colours `pal`.
fn intro_pic(archive: &Archive, n: u8, pal: [[u8; 4]; 256]) -> Option<(Image, [[u8; 4]; 256])> {
    let d = archive.read(&format!("INTRO0{n}.IMG"))?;
    crate::interface::image(&d).map(|(i, _)| (i, pal))
}

/// An image (12-byte header; LZSS or the type-8 coding) with its 6-bit
/// palette after the data (flag 100h), as TITLE.IMG and MENU.IMG are.
pub(crate) fn image_with_palette(m: &Machine, archive: &Archive, name: &str) -> Option<(Image, [[u8; 4]; 256])> {
    let d = archive.read(name)?;
    let size = u16::from_le_bytes([*d.get(10)?, *d.get(11)?]) as usize;
    let img = crate::interface::image(&d).map(|(i, _)| i).or_else(|| crate::panels::image_type8(m, &d))?;
    let p = d.get(12 + size..12 + size + 0x300)?;
    let mut pal = [[0u8; 4]; 256];
    for (i, c) in pal.iter_mut().enumerate() {
        let v = |k: usize| {
            let x = p[3 * i + k] & 0x3F;
            x << 2 | x >> 4
        };
        *c = [v(0), v(1), v(2), 0xFF];
    }
    Some((img, pal))
}

/// Text number `n` made (1194:A374 into a buffer, as 1194:D459 makes the
/// vision's) as lines in colour `colour`.
pub(crate) fn vision_text(engine: &mut Engine, n: u16, colour: u8) -> Vec<Vec<Span>> {
    let m = &mut engine.m;
    let Some((seg, off)) = m.heap_alloc_bytes(0xBB8) else {
        return Vec::new();
    };
    m.mem.w16(DS, 0x137A, n);
    m.mem.w8(DS, 0x137E, 0);
    m.mem.w8(DS, 0x101C, 0);
    let text = match m.text(&mut engine.files, &mut arena::text::NoTextHooks, 2, seg, off) {
        Some(_) => engine.m.mem.cstr(seg, off).to_vec(),
        None => Vec::new(),
    };
    engine.m.heap_free(seg);
    hud::lines(&text, colour)
}

/// The ending's text over the house (1194:DA64..DA75: text `n`, 579h,
/// made with 101C FFh into 929E), its lines split at the line feeds as
/// DA9E does, in colour `colour` (5Ch).
pub(crate) fn house_text(engine: &mut Engine, n: u16, colour: u8) -> Vec<Vec<Span>> {
    let m = &mut engine.m;
    let Some((seg, off)) = m.heap_alloc_bytes(0xBB8) else {
        return Vec::new();
    };
    m.mem.w16(DS, 0x137A, n);
    m.mem.w8(DS, 0x137E, 0);
    m.mem.w8(DS, 0x101C, 0xFF);
    let text = match m.text(&mut engine.files, &mut arena::text::NoTextHooks, 2, seg, off) {
        Some(_) => engine.m.mem.cstr(seg, off).to_vec(),
        None => Vec::new(),
    };
    engine.m.mem.w8(DS, 0x101C, 0);
    engine.m.heap_free(seg);
    text.split(|&b| b == 0x0A).flat_map(|l| hud::lines(l, colour)).collect()
}
