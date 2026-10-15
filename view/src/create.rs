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
//! - the class list (1194:2678): eighteen names in the order of 2AEE, six
//!   rows (FONT_C);
//! - the name typed (2711, 3276:35F8) and the gender (2791: M, F);
//! - the province on TAMRIEL.MNU (27C2: NOEXIT.IMG over its exit, the
//!   first frame of TOP.DFA, the question 3276:362A, the eight regions at
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
//! - the vision's text (1194:D459 with text 578h; its picture, VISION.FLC,
//!   isn't played here), then the game.
//!
//! Approximate: the boxes and menus are PARCH.CIF's nine tiles around the
//! text, centred (the original's sizes and the keys' highlights aren't
//! traced); the paper doll right of the attributes and the derived
//! numbers (1194:0F08) aren't drawn; the scroll doesn't roll; the fades
//! and the music are left out.

use crate::archive::Archive;
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
const RACE_NAMES: u16 = 0x2B94;
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
}

/// `--create`'s steps: a character (`G`), `enter`, `esc`, `back`, `up`,
/// `down`, `type:TEXT` (its characters), `click:X:Y`, comma-separated.
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
}

#[derive(Clone, Debug)]
enum Stage {
    Method(Ask),
    Quiz { asked: u8, lines: Vec<String>, top: usize },
    ClassYes(Ask, u8),
    List(Ask),
    Name(Vec<Vec<Span>>, String),
    Gender(Ask),
    Map,
    ProvinceYes(Ask, u8),
    Stats,
    Choose(Ask),
    Face,
    Vision(Vec<Vec<Span>>, usize),
    Done,
}

/// The game's pictures for creation, decoded once.
struct Pics {
    start: Option<(Image, [[u8; 4]; 256])>,
    map: Option<(Image, [[u8; 4]; 256])>,
    noexit: Option<Image>,
    top: Option<Image>,
    scroll: Option<Image>,
    charstat: Option<Image>,
    bonus: Option<Image>,
    updown: Vec<u8>,
    parch: Vec<Vec<u8>>,
    charsheet: [[u8; 4]; 256],
    game: [[u8; 4]; 256],
    faces: Vec<Image>,
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
}

/// The creation's `NewGameHooks`: its boxes kept (the attributes' note).
struct Hooks<'a> {
    boxes: &'a mut VecDeque<Boxed>,
}

impl arena::newgame::NewGameHooks for Hooks<'_> {
    fn message_box(&mut self, m: &mut Machine, seg: u16, si: u16) {
        self.boxes.push_back(Boxed { lines: hud::lines(m.mem.cstr(seg, si), COLOUR_B), font: FontId::Arena, back: Back::Stats });
    }
}

/// An image with a 12-byte header and its palette after the data
/// (6-bit; flag 100h), as the .MNU screens are.
fn mnu(archive: &Archive, name: &str) -> Option<(Image, [[u8; 4]; 256])> {
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

/// A .DFA's first frame, at its header's place.
fn dfa_first(archive: &Archive, name: &str) -> Option<Image> {
    let d = archive.read(name)?;
    let le = |at: usize| u16::from_le_bytes([*d.get(at).unwrap_or(&0), *d.get(at + 1).unwrap_or(&0)]) as i16 as i32;
    let f = crate::flats::decode(1, &d)?;
    Some(Image { x: le(2), y: le(4), w: f.w, h: f.h, px: f.frames.into_iter().next()? })
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
            top: dfa_first(&archive, "TOP.DFA"),
            scroll: dfa_first(&archive, "SCROLL01.DFA"),
            charstat: crate::panels::load_image(&archive, m, "CHARSTAT.IMG"),
            bonus: crate::panels::load_image(&archive, m, "BONUS.IMG"),
            updown: archive.read("UPDOWN.IMG").unwrap_or_default(),
            parch: archive.read("PARCH.CIF").map(|d| d.chunks(400).map(<[u8]>::to_vec).collect()).unwrap_or_default(),
            charsheet: crate::world::palette_file(dir, "CHARSHT.COL"),
            game,
            faces: Vec::new(),
        };
        let mut c = Creation { stage: Stage::Done, boxes: VecDeque::new(), archive, pics, fonts, name: String::new(), colour: COLOUR_A };
        c.stage = Stage::Method(c.method(m));
        c
    }

    /// Whether creation is over (the game to be started).
    pub fn done(&self) -> bool {
        matches!(self.stage, Stage::Done) && self.boxes.is_empty()
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
        let entries = raw.entries.iter().map(|l| l.iter().map(|s| Span { colour: if s.colour == 0x0F { self.colour } else { s.colour }, text: s.text.clone() }).collect()).collect();
        Ask { title, entries, keys: raw.keys, sel: 0, top: 0, rows: 0, font }
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

    /// A key or click.
    pub fn input(&mut self, engine: &mut Engine, inp: Input) {
        if !self.boxes.is_empty() {
            if matches!(inp, Input::Enter | Input::Esc | Input::Char(b' ') | Input::Click(..)) {
                self.boxes.pop_front();
            }
            return;
        }
        let stage = std::mem::replace(&mut self.stage, Stage::Done);
        self.stage = match stage {
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
            Stage::Quiz { asked, lines, mut top } => match inp {
                Input::Char(c) if (b'A'..=b'C').contains(&c.to_ascii_uppercase()) => {
                    engine.m.quiz_answer(c.to_ascii_uppercase() - b'A');
                    if asked + 1 < 10 {
                        self.next_question(engine, asked + 1)
                    } else {
                        self.quiz_done(engine)
                    }
                }
                Input::Up | Input::Click(_, 119..=136) => {
                    top = top.saturating_sub(1);
                    Stage::Quiz { asked, lines, top }
                }
                Input::Down | Input::Click(_, 183..=199) => {
                    if top + 6 < lines.len() {
                        top += 1;
                    }
                    Stage::Quiz { asked, lines, top }
                }
                _ => Stage::Quiz { asked, lines, top },
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
            Stage::Name(prompt, mut typed) => match inp {
                Input::Char(c) if (0x20..0x7F).contains(&c) && typed.len() < 0x12 => {
                    typed.push(c as char);
                    Stage::Name(prompt, typed)
                }
                Input::Back => {
                    typed.pop();
                    Stage::Name(prompt, typed)
                }
                // 1194:2789: an empty name asks again.
                Input::Enter if !typed.trim().is_empty() => {
                    self.name = typed;
                    let ask = self.menu(&mut engine.m, (DATA, 0x3614), &[], 0x3F85, 0x3F88, FontId::A);
                    Stage::Gender(ask)
                }
                _ => Stage::Name(prompt, typed),
            },
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
            Stage::Choose(mut ask) => match self.ask_input(&mut ask, inp) {
                None => Stage::Choose(ask),
                Some(0) => {
                    if engine.m.mem.r8(DS, arena::newgame::var::POINTS) != 0 {
                        // 1194:2D1C: in ARENAFNT.
                        let l = hud::lines(engine.m.mem.cstr(DS, 0x3F2B), self.colour);
                        self.push_box(l, FontId::Arena, Back::Stats);
                        Stage::Stats
                    } else {
                        engine.m.creation_finish();
                        self.face_screen(engine)
                    }
                }
                // Reroll, or Esc (FFFFh, not 0).
                Some(_) => self.roll(engine),
            },
            Stage::Face => self.face_input(engine, inp),
            Stage::Vision(lines, page) => match inp {
                Input::Enter | Input::Esc | Input::Char(b' ') | Input::Click(..) => {
                    if (page + 1) * VISION_ROWS < lines.len() {
                        Stage::Vision(lines, page + 1)
                    } else {
                        Stage::Done
                    }
                }
                _ => Stage::Vision(lines, page),
            },
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
                vec![Span { colour: self.colour, text: String::from_utf8_lossy(m.mem.cstr(s, o)).into_owned() }]
            })
            .collect();
        let title = hud::lines(m.mem.cstr(DS, 0x3F56), self.colour);
        Stage::List(Ask { title, entries, keys: Vec::new(), sel: 0, top: 0, rows: 6, font: FontId::C })
    }

    /// The next question (number `asked` from 0), its text's lines.
    fn next_question(&mut self, engine: &mut Engine, asked: u8) -> Stage {
        let Some((seg, di)) = engine.m.quiz_question() else {
            eprintln!("a question could not be found");
            return self.list(engine);
        };
        let text = engine.m.mem.cstr(seg, di).to_vec();
        let lines = text.split(|&b| b == b'\n').map(|l| String::from_utf8_lossy(l).trim_end_matches('\r').to_string()).collect();
        Stage::Quiz { asked, lines, top: 0 }
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
        let prompt = self.say(&mut engine.m, (DATA, 0x35F8), &[Arg::Str(cs, co)]);
        Stage::Name(prompt, String::new())
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
        if engine.m.province_accepted().is_none() {
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
            Some(_) => known.extend_from_slice(engine.m.mem.cstr(DS, BUF)),
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
        let done = |c: &Self, m: &mut Machine| {
            // 1194:2CD0: "Which dost thou choose?", in FONT_A.
            Stage::Choose(c.menu(m, (DATA, 0x3A43), &[], 0x3F9C, 0x3F9F, FontId::A))
        };
        match inp {
            Input::Esc => done(self, m),
            Input::Char(c) => {
                let c = c.to_ascii_uppercase();
                if let Some(r) = (1..=8u16).find(|&r| key(m, r) == c) {
                    m.creation_select(r);
                }
                Stage::Stats
            }
            // The arrows' halves (a convenience: the original's are clicked).
            Input::Up | Input::Down => {
                let mid = m.mem.r8(DS, arena::newgame::var::ARROWS) as u16;
                m.creation_arrow(if inp == Input::Up { mid } else { mid + 1 });
                Stage::Stats
            }
            Input::Click(x, y) => match self.region(m, STAT_REGIONS, 10, x, y) {
                Some(0) => done(self, m),
                Some(r @ 1..=8) => {
                    m.creation_select(r);
                    Stage::Stats
                }
                Some(9) => {
                    m.creation_arrow(y as u16);
                    Stage::Stats
                }
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
        m.mem.w16(DS, arena::newgame::var::FACE_PICKED, 0);
        Stage::Face
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
        Stage::Vision(lines, 0)
    }

    /// A menu's or list's answer to `inp`: an entry, FFFFh for Esc, or
    /// None while it waits.
    fn ask_input(&self, ask: &mut Ask, inp: Input) -> Option<u16> {
        let n = ask.entries.len();
        let rows = if ask.rows == 0 { n } else { ask.rows.min(n) };
        match inp {
            Input::Up => ask.sel = ask.sel.saturating_sub(1),
            Input::Down => ask.sel = (ask.sel + 1).min(n.saturating_sub(1)),
            Input::Enter => return Some(ask.sel as u16),
            Input::Esc => return Some(0xFFFF),
            Input::Char(c) => {
                let c = c.to_ascii_uppercase();
                if let Some(i) = ask.keys.iter().position(|&k| k.to_ascii_uppercase() == c) {
                    return Some(i as u16);
                }
            }
            Input::Click(x, y) => {
                let l = self.layout(ask);
                for row in 0..rows {
                    let ey = l.entries_y + row as i32 * l.lh;
                    if (l.x..l.x + l.w).contains(&x) && (ey..ey + l.lh).contains(&y) {
                        return Some((ask.top + row) as u16);
                    }
                }
            }
            Input::Back => {}
        }
        if ask.sel < ask.top {
            ask.top = ask.sel;
        } else if ask.sel >= ask.top + rows {
            ask.top = ask.sel + 1 - rows;
        }
        None
    }

    /// Where a menu's box and entries go.
    fn layout(&self, ask: &Ask) -> Layout {
        let f = self.fonts.get(ask.font);
        let n = ask.entries.len();
        let rows = if ask.rows == 0 { n } else { ask.rows.min(n) };
        let lines: Vec<&Vec<Span>> = ask.title.iter().chain(ask.entries.iter()).collect();
        let tw = lines.iter().map(|l| f.width(&hud::plain(l)) as i32).max().unwrap_or(0);
        let lh = f.h as i32 + 2;
        let th = (ask.title.len() + rows) as i32 * lh;
        let b = boxed(tw, th + if ask.title.is_empty() { 0 } else { 4 });
        Layout { b, x: b.x + MARGIN, w: b.w - 2 * MARGIN, lh, title_y: b.y + (b.h - th) / 2, entries_y: b.y + (b.h - th) / 2 + ask.title.len() as i32 * lh + 4 }
    }

    /// The layer drawn: the screen behind, the stage, a box waiting.
    pub fn draw(&self, screen: &mut Screen, m: &Machine) {
        let back = match (&self.boxes.front(), &self.stage) {
            (Some(b), _) => b.back,
            (None, Stage::Method(_) | Stage::Quiz { .. } | Stage::ClassYes(..) | Stage::List(_) | Stage::Name(..) | Stage::Gender(_)) => Back::Start,
            (None, Stage::Map | Stage::ProvinceYes(..)) => Back::Map,
            (None, Stage::Stats | Stage::Choose(_) | Stage::Face) => Back::Stats,
            (None, Stage::Vision(..) | Stage::Done) => Back::Black,
        };
        self.draw_back(screen, m, back);
        if let Some(b) = self.boxes.front() {
            self.draw_box(screen, &b.lines, b.font, None);
            return;
        }
        match &self.stage {
            Stage::Method(a) | Stage::ClassYes(a, _) | Stage::List(a) | Stage::Gender(a) | Stage::ProvinceYes(a, _) | Stage::Choose(a) => self.draw_ask(screen, a),
            Stage::Quiz { lines, top, .. } => {
                if let Some(s) = &self.pics.scroll {
                    screen.blit(s.x, s.y, s.w, s.h, &s.px, H as i32);
                }
                for (i, l) in lines.iter().skip(*top).take(6).enumerate() {
                    screen.text_plain(&self.fonts.teeny, 0x14, 136 + 8 * i as i32, l, COLOUR_A);
                }
            }
            Stage::Name(prompt, typed) => {
                let mut lines = prompt.clone();
                lines.push(vec![Span { colour: self.colour, text: format!("{typed}_") }]);
                self.draw_box(screen, &lines, FontId::A, None);
            }
            Stage::Vision(lines, page) => {
                let f = &self.fonts.arena;
                let lh = f.h as i32 + 2;
                let shown: Vec<_> = lines.iter().skip(page * VISION_ROWS).take(VISION_ROWS).collect();
                let y0 = H as i32 - 16 - shown.len() as i32 * lh;
                for (i, l) in shown.iter().enumerate() {
                    let w = f.width(&hud::plain(l)) as i32;
                    self.spans(screen, f, (W as i32 - w) / 2, y0 + i as i32 * lh, l);
                }
            }
            _ => {}
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
                for i in [&self.pics.noexit, &self.pics.top].into_iter().flatten() {
                    blit(screen, i);
                }
            }
            Back::Stats => self.draw_stats(screen, m),
            Back::Black => {}
        }
    }

    /// The attribute screen (1194:0DE1's texts, in ARENAFNT, colour FEh).
    fn draw_stats(&self, screen: &mut Screen, m: &Machine) {
        let f = &self.fonts.arena;
        let c = COLOUR_B;
        if let Some(i) = &self.pics.charstat {
            screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
        }
        let s = |screen: &mut Screen, x: i32, y: i32, t: &str| screen.text_plain(f, x, y, t, c);
        let cstr = |seg: u16, off: u16| String::from_utf8_lossy(m.mem.cstr(seg, off)).into_owned();
        s(screen, 0xB, 7, &cstr(DS, NAME));
        let (rs, ro) = Self::nth(m, RACE_NAMES, m.mem.r8(DS, RACE));
        s(screen, 0xB, 0x10, &cstr(rs, ro));
        let (cs, co) = m.class_name(DS, PLAYER);
        s(screen, 0xB, 0x19, &cstr(cs, co));
        // While creating (5C8 bit 10h) the attributes as they are, else on
        // the 100 scale (1194:5B07). On the faces' screen the original
        // doesn't draw them again: they stay as they were before they were
        // kept on the 256 scale (2D28), which the 100 scale gives back.
        let creating = m.mem.r16(DS, 0x05C8) & 0x10 != 0 && !matches!(self.stage, Stage::Face);
        let scale = |v: u16| {
            let p = v as u32 * 100;
            (p / 256 + (p % 256 >= 0x80) as u32) as u16
        };
        for i in 0..8u16 {
            let v = m.mem.r8(DS, PLAYER + 0x29 + i) as u16;
            s(screen, 0x1A, 0x34 + 8 * i as i32, &(if creating { v } else { scale(v) }).to_string());
        }
        let w = |o: u16| m.mem.r16(DS, PLAYER + o);
        let max_hp = w(0x5B);
        let hp = if creating { max_hp } else { w(0x59) };
        s(screen, 0x2D, 0x7F, &format!("{hp}/{max_hp}"));
        let a = |o: u16| m.mem.r8(DS, PLAYER + o) as u16;
        let fat_max = if creating { a(0x29) + a(0x2E) } else { scale(a(0x29) + a(0x2E)) };
        s(screen, 0x2D, 0x87, &format!("{}/{fat_max}", scale(w(0x5D) >> 6)));
        s(screen, 0x56, 0x3C, &format!("{}/{}", w(0x66), w(0x68)));
        s(screen, 0x2D, 0x90, &m.mem.r32(DS, 0x05C2).to_string());
        s(screen, 0x2D, 0x9E, &m.mem.r32(DS, PLAYER + 0x409).to_string());
        s(screen, 0x2D, 0xA7, &(m.mem.r8(DS, PLAYER + 6) as u16 + 1).to_string());
        if matches!(self.stage, Stage::Stats | Stage::Choose(_)) {
            if let Some(i) = &self.pics.bonus {
                screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
            }
            s(screen, 0x5C, 0x71, &m.mem.r8(DS, arena::newgame::var::POINTS).to_string());
            // The arrows where the line picked put them (region 9's y).
            let ay = m.mem.r16(DATA, STAT_REGIONS + 9 * REGION + 2) as i32;
            if self.pics.updown.len() >= 128 {
                screen.blit(38, ay, 8, 16, &self.pics.updown[..128], H as i32);
            }
        }
        if matches!(self.stage, Stage::Face) && self.boxes.is_empty() {
            let k = m.mem.r8(DS, arena::newgame::var::FACE_PICKED) as usize;
            if let Some(i) = self.pics.faces.get(k) {
                screen.blit(i.x, i.y, i.w, i.h, &i.px, H as i32);
            }
        }
    }

    fn spans(&self, screen: &mut Screen, f: &Font, mut x: i32, y: i32, line: &[Span]) {
        for s in line {
            screen.text_plain(f, x, y, &s.text, s.colour);
            x += f.width(&s.text) as i32;
        }
    }

    /// The parchment (PARCH.CIF's nine 20 x 20 tiles: corners, edges and
    /// middle) over `b`.
    fn parchment(&self, screen: &mut Screen, b: Rect) {
        if self.pics.parch.len() < 9 {
            screen.panel(b.x, b.y, b.w, b.h, 0x0F);
            return;
        }
        let (cols, rows) = (b.w / TILE, b.h / TILE);
        for r in 0..rows {
            for c in 0..cols {
                let tr = if r == 0 { 0 } else if r == rows - 1 { 2 } else { 1 };
                let tc = if c == 0 { 0 } else if c == cols - 1 { 2 } else { 1 };
                let t = &self.pics.parch[tr * 3 + tc];
                screen.blit(b.x + c * TILE, b.y + r * TILE, TILE as usize, TILE as usize, t, H as i32);
            }
        }
    }

    /// A box with `lines` centred, on the parchment.
    fn draw_box(&self, screen: &mut Screen, lines: &[Vec<Span>], font: FontId, _sel: Option<usize>) {
        let f = self.fonts.get(font);
        let lh = f.h as i32 + 2;
        let tw = lines.iter().map(|l| f.width(&hud::plain(l)) as i32).max().unwrap_or(0);
        let th = lines.len() as i32 * lh;
        let b = boxed(tw, th);
        self.parchment(screen, b);
        let y0 = b.y + (b.h - th) / 2;
        for (i, l) in lines.iter().enumerate() {
            let w = f.width(&hud::plain(l)) as i32;
            self.spans(screen, f, b.x + (b.w - w) / 2, y0 + i as i32 * lh, l);
        }
    }

    /// A menu or list: its title, its entries (the one highlighted marked
    /// by a darker band), on the parchment.
    fn draw_ask(&self, screen: &mut Screen, ask: &Ask) {
        let f = self.fonts.get(ask.font);
        let l = self.layout(ask);
        self.parchment(screen, l.b);
        for (i, t) in ask.title.iter().enumerate() {
            let w = f.width(&hud::plain(t)) as i32;
            self.spans(screen, f, l.b.x + (l.b.w - w) / 2, l.title_y + i as i32 * l.lh, t);
        }
        let rows = if ask.rows == 0 { ask.entries.len() } else { ask.rows.min(ask.entries.len()) };
        let band = self.pics.parch.get(1).and_then(|t| t.get(20 * 2 + 10).copied()).unwrap_or(0x0F);
        for row in 0..rows {
            let k = ask.top + row;
            let Some(e) = ask.entries.get(k) else { break };
            let y = l.entries_y + row as i32 * l.lh;
            if k == ask.sel {
                screen.fill(l.x - 2, y - 1, l.w + 4, l.lh, band, 0xFF);
            }
            let w = f.width(&hud::plain(e)) as i32;
            self.spans(screen, f, l.b.x + (l.b.w - w) / 2, y, e);
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

/// The vision's lines a page.
const VISION_ROWS: usize = 6;
const TILE: i32 = 20;
const MARGIN: i32 = 12;

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct Layout {
    b: Rect,
    x: i32,
    w: i32,
    lh: i32,
    title_y: i32,
    entries_y: i32,
}

/// The box for text `tw` x `th`: whole tiles, a margin around, centred.
fn boxed(tw: i32, th: i32) -> Rect {
    let cols = ((tw + 2 * MARGIN + TILE - 1) / TILE).clamp(2, W as i32 / TILE);
    let rows = ((th + 2 * MARGIN - 4 + TILE - 1) / TILE).clamp(2, H as i32 / TILE);
    let (w, h) = (cols * TILE, rows * TILE);
    Rect { x: (W as i32 - w) / 2, y: (H as i32 - h) / 2, w, h }
}
