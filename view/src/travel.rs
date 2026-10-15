//! The travel map (26F7:0000), as the original shows it: opened by the
//! map button's right click or Shift+M (2542:0193, 0380 with no left
//! release: 03BF): refused indoors ("You cannot travel from here.",
//! `Machine::map_from_here`), from a boat or with monsters near
//! (`Machine::map_button`).
//!
//! - It starts on the current province's map (26F7:0176): its picture
//!   (HIGHROCK.IMG .. IMPERIAL.IMG by the province, its own palette),
//!   MAPBTNS.MNU's buttons over it (search at 34, travel at 53, exit at
//!   72, all at row 167: the window's regions at DATA:7164), the
//!   locations' icons centred on them (26F7:0268: the cities, but the
//!   Imperial Province's one; the known dungeons, the staff's piece's
//!   from STAFDUNG.CIF; the towns and villages), and the name of the
//!   location nearest the cursor (`Machine::travel_nearest`, the
//!   handler's every pass).
//! - A click on it picks the nearest location and shows the journey
//!   (`Machine::travel_click`: "the trip to X will take N days", a box);
//!   Search asks for a name, and if none starts it, lists the province's
//!   places (`Machine::travel_search`); Travel makes the journey
//!   (`Machine::travel_go`: the days pass through the game's clock, the
//!   destination's record built, the arrival's text written to
//!   ENTERCTY.$$$) and ends the screen; Exit (Esc) goes to the world map.
//! - The world map (TAMRIEL.MNU, the current province outlined from
//!   OUTPROV.CIF, 26F7:013E): a province's region (DATA:6D90) opens its
//!   map; Exit or Esc ends the screen.
//!
//! The screen's end (`Machine::travel_map_end`) gives the carry: set, the
//! area is left (`Machine::map_button_done`) and the window's next turn
//! goes on to the next area (5E6) as any leaving does; the town's welcome
//! then shows the arrival's text (0000:4ACD).
//!
//! - The markers (26F7:071F, from 0625): the current place's frame of
//!   MAPOUT.CIF (by its kind) when its province is shown, before the
//!   label; the destination's frame of MAPBLINK.CIF after it, when one is
//!   chosen and its province shown, three passes in four (A8C4 & 3). A
//!   dungeon picked as 20h (mod 40h) is marked by colour instead: a 22 x
//!   17 rectangle from (x - 8, y - 10) remapped (0000:9D84) with DCh made
//!   C2h (the current place) or DFh.
//! - The journey (1194:2F69/30F1): HORSE.DFA (named at 4096) drawn over
//!   the screen at its own place, frame by frame, a frame each day
//!   (0..0Ah round), the screen ending after the last.
//!
//! Approximate: the original's passes and days go as fast as its loop
//! runs; the view counts a pass, and plays a day's frame, each game tick
//! (1/18.2 s). The scripted shots skip the journey's frames.

use crate::hud::{self, Font, Screen, Span};
use crate::interface::Image;
use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mapscreen::{MapHooks, MapStart};
use arena::mem::DS;
use arena::travelmap::TravelHooks;

type Palette = [[u8; 4]; 256];
type Lines = Vec<Vec<Span>>;

/// The provinces' pictures, by province (26F7's names at 6BB6).
const PICTURES: [&str; 9] =
    ["HIGHROCK.IMG", "HAMERFEL.IMG", "SKYRIM.IMG", "MOROWIND.IMG", "SUMERSET.IMG", "VALNWOOD.IMG", "ELSWEYR.IMG", "BLAKMRSH.IMG", "IMPERIAL.IMG"];

/// The world map's regions (DATA:6D90: x, y, width, height), the
/// provinces' in order; the tenth is the exit.
const REGIONS: [(i32, i32, i32, i32); 10] = [
    (37, 32, 86, 57),
    (47, 53, 90, 62),
    (113, 29, 88, 53),
    (190, 31, 102, 93),
    (31, 131, 65, 52),
    (100, 118, 61, 55),
    (144, 119, 50, 57),
    (204, 116, 67, 67),
    (103, 72, 131, 84),
    (279, 188, 37, 11),
];

/// A province map's buttons (DATA:7164's regions: x, y, width, height).
const SEARCH: (i32, i32, i32, i32) = (34, 167, 18, 28);
const TRAVEL: (i32, i32, i32, i32) = (53, 167, 18, 28);
const EXIT: (i32, i32, i32, i32) = (72, 167, 18, 29);

/// The location records' lists (19h bytes each).
const CITIES: u16 = 0x1445;
const TOWNS: u16 = 0x150D;
const VILLAGES: u16 = 0x15D5;
const DUNGEONS: u16 = 0x1765;
const RECORD: u16 = 0x19;
const SHOWN: u16 = 0x47E1;
const NEAREST_REC: u16 = 0xA8BA;
/// What `travel_nearest` writes: the nearest's index, record, distance,
/// and the known-only switch.
const SCAN: [u16; 4] = [0xA8BC, NEAREST_REC, 0xA8B6, 0x1294];

fn inside(r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= r.0 && x <= r.0 + r.2 && y >= r.1 && y <= r.1 + r.3
}

/// What the screen asks between clicks.
enum Asking {
    /// A name typed for the search.
    Name(String),
    /// The province's places listed (the name typed matched none): the
    /// names, the name typed.
    List(Vec<String>, String),
}

pub struct Travel {
    start: MapStart,
    /// The province's map shown; None: the world map.
    shown: Option<u8>,
    world: Option<(Image, Palette)>,
    province: Option<(Image, Palette)>,
    buttons: Option<Image>,
    /// City, town, village and dungeon (raw pictures, 26F7:0268's sizes).
    icons: [Option<Image>; 4],
    outprov: Vec<Image>,
    stafdung: Vec<Image>,
    asking: Option<Asking>,
    /// Boxes shown since asked for (`take_boxes`).
    boxes: Vec<Lines>,
    /// Ended: the carry (the area left).
    pub done: Option<bool>,
    mapout: Vec<Image>,
    mapblink: Vec<Image>,
    /// HORSE.DFA's frames, at its header's place.
    horse: Vec<Image>,
    /// The journey's frames still to play (one a day), and the time since
    /// they began (ms).
    journey: Vec<u8>,
    journey_ms: u32,
    /// The time the screen has been up (ms): its passes (A8C4's blink).
    up_ms: u32,
}

/// A game tick (1/18.2 s) in ms: a pass of the screen's loop, a day's
/// frame of the journey.
const TICK_MS: u32 = 55;

/// A .CIF's frames, each by its coding (type 8 through the port's
/// decompressor).
fn cif(m: &Machine, d: &[u8]) -> Vec<Image> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 12 <= d.len() {
        let size = u16::from_le_bytes([d[at + 10], d[at + 11]]) as usize;
        let f = &d[at..];
        out.push(crate::panels::image_type8(m, f).or_else(|| crate::interface::image(f).map(|(i, _)| i)).unwrap_or(Image { x: 0, y: 0, w: 0, h: 0, px: Vec::new() }));
        at += 12 + size;
    }
    out
}

/// A .DFA's frames, each at its header's place.
fn dfa(archive: &crate::archive::Archive, name: &str) -> Vec<Image> {
    let Some(d) = archive.read(name) else { return Vec::new() };
    let le = |at: usize| u16::from_le_bytes([*d.get(at).unwrap_or(&0), *d.get(at + 1).unwrap_or(&0)]) as i16 as i32;
    let Some(f) = crate::flats::decode(1, &d) else { return Vec::new() };
    f.frames.into_iter().map(|px| Image { x: le(2), y: le(4), w: f.w, h: f.h, px }).collect()
}

/// The hooks the screen's routines run with: boxes kept, the destination's
/// record built (1194:3AE8 in full, `Machine::city_location`; a
/// dungeon's, 3B0C, with its climate), the search's name and list pick.
struct Hooks<'a> {
    boxes: &'a mut Vec<Lines>,
    files: Files,
    typed: Vec<u8>,
    pick: u16,
    listed: Option<Vec<String>>,
    failed: bool,
    /// The journey's frames drawn (1194:30F1, one a day).
    frames: Vec<u8>,
}

impl arena::clock::ClockHooks for Hooks<'_> {}

impl TravelHooks for Hooks<'_> {
    fn message_box(&mut self, m: &mut Machine, seg: u16, si: u16) {
        let lines = hud::lines(m.mem.cstr(seg, si), 0x0F);
        for l in &lines {
            println!("{}", hud::plain(l));
        }
        self.boxes.push(lines);
    }
    /// The question (unwell: "Considering your condition...") shown; the
    /// journey goes ahead.
    fn question(&mut self, m: &mut Machine, si: u16) -> u16 {
        self.message_box(m, DS, si);
        0
    }
    fn location(&mut self, m: &mut Machine, id: u16) {
        let ax = if m.mem.r8(DS, 0x0A46) == 8 { 0x100 } else { id };
        if m.city_location(&mut self.files, DS, 0x0A22, ax).is_none() {
            self.failed = true;
        }
    }
    fn animation_frame(&mut self, _m: &mut Machine, frame: u8) {
        self.frames.push(frame);
    }
    fn dungeon_location(&mut self, m: &mut Machine, id: u16, _bx: u16) {
        let (x, y) = (m.mem.r16(DS, 0x193E), m.mem.r16(DS, 0x1940));
        let p = m.mem.r8(DS, SHOWN);
        let ok = m.city_climate(&mut self.files, x, y, p).and_then(|c| m.dungeon_build_location(id as u8, c));
        if ok.is_none() {
            self.failed = true;
        }
    }
}

impl MapHooks for Hooks<'_> {
    fn type_name(&mut self, m: &mut Machine) {
        for (i, &c) in self.typed.iter().take(0x1F3).chain(&[0]).enumerate() {
            m.mem.w8(DS, 0x9C62 + i as u16, c);
        }
    }
    fn list(&mut self, m: &mut Machine, si: u16, cx: u16) -> u16 {
        let mut at = si;
        let mut names = Vec::new();
        for _ in 0..cx {
            let s = m.mem.cstr(DS, at);
            at = at.wrapping_add(s.len() as u16 + 1);
            names.push(String::from_utf8_lossy(s).into_owned());
        }
        self.listed = Some(names);
        self.pick
    }
}

impl Travel {
    /// The map button pressed (2542:03DC): refused, its box; else the
    /// screen's start (26F7:0000) on the current province's map.
    pub fn open(engine: &mut Engine, archive: &crate::archive::Archive) -> Result<Travel, Lines> {
        let mut boxes = Vec::new();
        let m = &mut engine.m;
        {
            let mut h = Hooks { boxes: &mut boxes, files: engine.files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new() };
            // 2542:03BF: from here at all (038F), the colour 60h (03CF),
            // then the button's own refusals (03DC).
            if !m.map_from_here(&mut h) {
                return Err(boxes.pop().unwrap_or_default());
            }
            m.mem.w8(DS, 0x9288, 0x60);
            let mut h = Hooks { boxes: &mut boxes, files: engine.files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new() };
            if !m.map_button(&mut h) {
                return Err(boxes.pop().unwrap_or_default());
            }
        }
        Ok(Travel::begin(engine, archive, boxes))
    }

    /// The screen shown on the way out of a dungeon (0000:4570, 1A81
    /// set): no refusals, its end's carry the answer (`after`'s, with
    /// the area left by the caller).
    pub fn forced(engine: &mut Engine, archive: &crate::archive::Archive) -> Travel {
        Travel::begin(engine, archive, Vec::new())
    }

    /// 26F7:0000's start, on the current province's map.
    fn begin(engine: &mut Engine, archive: &crate::archive::Archive, mut boxes: Vec<Lines>) -> Travel {
        let m = &mut engine.m;
        let mut h = Hooks { boxes: &mut boxes, files: engine.files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new() };
        let start = m.travel_map_begin(&mut h);
        let raw = |name: &str, w: usize, h: usize| archive.read(name).map(|d| Image { x: 0, y: 0, w, h, px: d });
        let mut t = Travel {
            start,
            shown: None,
            world: crate::create::mnu(archive, "TAMRIEL.MNU"),
            province: None,
            buttons: crate::create::mnu(archive, "MAPBTNS.MNU").map(|(i, _)| i),
            icons: [raw("CITY.IMG", 16, 11), raw("TOWN.IMG", 9, 10), raw("VILLAGE.IMG", 8, 8), raw("DUNGEON.IMG", 14, 8)],
            outprov: archive.read("OUTPROV.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            stafdung: archive.read("STAFDUNG.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            asking: None,
            boxes,
            done: None,
            mapout: archive.read("MAPOUT.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            mapblink: archive.read("MAPBLINK.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            horse: dfa(archive, "HORSE.DFA"),
            journey: Vec::new(),
            journey_ms: 0,
            up_ms: 0,
        };
        let p = m.mem.r8(DS, 0x0A46);
        t.show_province(engine, archive, p);
        t
    }

    /// The boxes shown since last asked.
    pub fn take_boxes(&mut self) -> Vec<Lines> {
        std::mem::take(&mut self.boxes)
    }

    fn hooks<'a>(boxes: &'a mut Vec<Lines>, files: &Files) -> Hooks<'a> {
        Hooks { boxes, files: files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new() }
    }

    /// 26F7:0176: province `p`'s map (its data loaded, `travel_province`).
    fn show_province(&mut self, engine: &mut Engine, archive: &crate::archive::Archive, p: u8) {
        let mut h = Self::hooks(&mut self.boxes, &engine.files);
        if engine.m.travel_map_province(&mut engine.files, &mut h, p).is_none() {
            eprintln!("the province's data didn't load");
        }
        self.province = PICTURES.get(p as usize).and_then(|n| crate::create::image_with_palette(&engine.m, archive, n));
        self.shown = Some(p);
    }

    /// The screen's end: the carry kept (`done`).
    fn end(&mut self, engine: &mut Engine) {
        let mut h = Self::hooks(&mut self.boxes, &engine.files);
        let carry = engine.m.travel_map_end(&mut h, self.start);
        self.done = Some(carry);
    }

    /// A click at (x, y) of the screen (a button down: `right` the right).
    pub fn click(&mut self, engine: &mut Engine, archive: &crate::archive::Archive, x: i32, y: i32, right: bool) {
        if self.done.is_some() {
            return;
        }
        if let Some(Asking::List(names, typed)) = &self.asking {
            let (names, typed) = (names.clone(), typed.clone());
            self.asking = None;
            let pick = list_at(&names, x, y).map_or(0xFFFF, |i| i as u16);
            if !right {
                self.search(engine, typed, pick);
            }
            return;
        }
        if self.asking.is_some() {
            return;
        }
        let Some(_) = self.shown else {
            match REGIONS.iter().position(|&r| inside(r, x, y)) {
                Some(9) => self.end(engine),
                Some(p) => self.show_province(engine, archive, p as u8),
                None => {}
            }
            return;
        };
        if inside(EXIT, x, y) {
            self.exit(engine);
        } else if inside(TRAVEL, x, y) {
            self.travel(engine);
        } else if inside(SEARCH, x, y) {
            self.asking = Some(Asking::Name(String::new()));
        } else {
            // 26F7:0625 with the button down (AFF9).
            use arena::controls::var::{BUTTONS, MOUSE_X, MOUSE_Y};
            let m = &mut engine.m;
            m.mem.w16(DS, MOUSE_X, x.clamp(0, 319) as u16);
            m.mem.w16(DS, MOUSE_Y, y.clamp(0, 199) as u16);
            let b = m.mem.r8(DS, BUTTONS);
            m.mem.w8(DS, BUTTONS, if right { 2 } else { 1 });
            let mut h = Self::hooks(&mut self.boxes, &engine.files);
            if m.travel_click(&mut engine.files, &mut h).is_none() {
                eprintln!("the click gave up: {}", arena::giveup::describe());
            }
            m.mem.w8(DS, BUTTONS, b);
        }
    }

    /// Exit: the province's map to the world map; the world map ends the
    /// screen.
    fn exit(&mut self, engine: &mut Engine) {
        if self.shown.is_some() {
            self.shown = None;
        } else {
            self.end(engine);
        }
    }

    /// Travel (26F7:07CD); once a journey is made the screen ends (0236).
    fn travel(&mut self, engine: &mut Engine) {
        let mut h = Self::hooks(&mut self.boxes, &engine.files);
        let ok = engine.m.travel_go(&mut engine.files, &mut h);
        let failed = h.failed;
        self.journey = std::mem::take(&mut h.frames);
        self.journey_ms = 0;
        if ok.is_none() || failed {
            eprintln!("the journey gave up: {}", arena::giveup::describe());
        }
        if engine.m.mem.r16(DS, 0x12A2) != 0 {
            self.end(engine);
        }
    }

    /// `ms` of time on: the passes counted, the journey's frames played.
    pub fn advance(&mut self, ms: u32) {
        self.up_ms = self.up_ms.wrapping_add(ms);
        if !self.journey.is_empty() {
            self.journey_ms += ms;
            if (self.journey_ms / TICK_MS) as usize >= self.journey.len() {
                self.journey.clear();
            }
        }
    }

    /// The journey's frames left unplayed (the scripted shots).
    pub fn skip_journey(&mut self) {
        self.journey.clear();
    }

    /// The journey's frames playing.
    pub fn journeying(&self) -> bool {
        !self.journey.is_empty()
    }

    /// 26F7:071F: the marker of kind `kind` (frame `kind` of `pics`) centred
    /// on (x, y), or for a dungeon picked as 20h the colour remapped.
    #[allow(clippy::too_many_arguments)]
    fn marker(&self, screen: &mut Screen, pics: &[Image], kind: u16, pick: u16, here: bool, x: i32, y: i32) {
        if kind == 3 && pick % 0x40 == 0x20 {
            let mut table = [0u8; 256];
            for (i, t) in table.iter_mut().enumerate() {
                *t = i as u8;
            }
            table[0xDC] = if here { 0xC2 } else { 0xDF };
            screen.remap(x - 8, y - 10, 0x16, 0x11, &table);
        } else if let Some(i) = pics.get(kind as usize) {
            screen.blit(x - (i.w / 2) as i32, y - (i.h / 2) as i32, i.w, i.h, &i.px, 199);
        }
    }

    /// The search (26F7:03A4) with the name `typed`; its list's pick `pick`
    /// (FFFFh: none). Run on a copy first: if the list is asked and not
    /// yet answered, it is shown instead.
    fn search(&mut self, engine: &mut Engine, typed: String, pick: u16) {
        let mut copy = engine.m.clone();
        let mut scratch = Vec::new();
        let mut h = Self::hooks(&mut scratch, &engine.files);
        h.typed = typed.clone().into_bytes();
        h.pick = 0xFFFF;
        let _ = copy.travel_search(&mut engine.files.clone(), &mut h);
        if let (Some(names), 0xFFFF) = (h.listed.take(), pick) {
            self.asking = Some(Asking::List(names, typed));
            return;
        }
        let mut h = Self::hooks(&mut self.boxes, &engine.files);
        h.typed = typed.into_bytes();
        h.pick = pick;
        if engine.m.travel_search(&mut engine.files, &mut h).is_none() {
            eprintln!("the search gave up: {}", arena::giveup::describe());
        }
    }

    /// A key: Esc as Exit (a list or the name: left); the name typed;
    /// Enter searches.
    pub fn key(&mut self, engine: &mut Engine, code: winit::keyboard::KeyCode, text: Option<&str>) {
        use winit::keyboard::KeyCode;
        if self.done.is_some() {
            return;
        }
        match &mut self.asking {
            Some(Asking::Name(s)) => match code {
                KeyCode::Escape => self.asking = None,
                KeyCode::Backspace => {
                    s.pop();
                }
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let typed = std::mem::take(s);
                    self.asking = None;
                    self.search(engine, typed, 0xFFFF);
                }
                _ => {
                    if let Some(c) = text.and_then(|t| t.chars().next()).filter(|c| c.is_ascii_graphic() || *c == ' ') {
                        if s.len() < 20 {
                            s.push(c);
                        }
                    }
                }
            },
            Some(Asking::List(..)) => {
                if code == KeyCode::Escape {
                    self.asking = None;
                }
            }
            None => match code {
                KeyCode::Escape => self.exit(engine),
                KeyCode::KeyT | KeyCode::Enter if self.shown.is_some() => self.travel(engine),
                KeyCode::KeyS if self.shown.is_some() => self.asking = Some(Asking::Name(String::new())),
                _ => {}
            },
        }
    }

    /// The screen, the cursor at (x, y): its palette set.
    pub fn draw(&self, screen: &mut Screen, font: &Font, m: &mut Machine, x: i32, y: i32) {
        let blit = |screen: &mut Screen, i: &Image, x: i32, y: i32| screen.blit(x, y, i.w, i.h, &i.px, 199);
        let Some(p) = self.shown else {
            if let Some((img, pal)) = &self.world {
                screen.set_palette(*pal);
                screen.fill(0, 0, 320, 200, 0, 0xFF);
                blit(screen, img, img.x, img.y);
            }
            if let Some(f) = self.outprov.get(m.mem.r8(DS, 0x0A46) as usize) {
                blit(screen, f, f.x, f.y);
            }
            return;
        };
        if let Some((img, pal)) = &self.province {
            screen.set_palette(*pal);
            screen.fill(0, 0, 320, 200, 0, 0xFF);
            blit(screen, img, img.x, img.y);
        }
        if let Some(b) = &self.buttons {
            blit(screen, b, b.x, b.y);
        }
        // 26F7:0268: the icons centred on their places.
        let icon = |screen: &mut Screen, k: usize, rec: u16| {
            if let Some(i) = &self.icons[k] {
                let (x, y) = (m.mem.r16(DS, rec + 0x14) as i32, m.mem.r16(DS, rec + 0x16) as i32);
                blit(screen, i, x - (i.w / 2) as i32, y - (i.h / 2) as i32);
            }
        };
        let cities = if p == 8 { 1 } else { 8 };
        for i in 0..cities {
            icon(screen, 0, CITIES + i * RECORD);
        }
        for i in 1..16 {
            let rec = DUNGEONS + i * RECORD;
            if m.mem.r8(DS, rec + 0x18) & 2 != 0 {
                icon(screen, 3, rec);
            }
        }
        if m.mem.r8(DS, DUNGEONS + 0x18) & 2 != 0 {
            if let Some(f) = self.stafdung.get(p as usize) {
                blit(screen, f, f.x, f.y);
            }
        }
        if p != 8 {
            for i in 0..8 {
                icon(screen, 1, TOWNS + i * RECORD);
            }
            for i in 0..16 {
                icon(screen, 2, VILLAGES + i * RECORD);
            }
        }
        match &self.asking {
            Some(Asking::Name(s)) => {
                // 26F7:0ABC: 1194:5C30 with the prompt at 6AD1.
                let prompt = String::from_utf8_lossy(m.mem.cstr(DS, 0x6AD1)).into_owned();
                hud::typed_prompt(screen, &prompt, s);
                return;
            }
            Some(Asking::List(names, _)) => {
                screen.panel(20, 10, 280, 150, 0xDF);
                for (i, n) in names.iter().enumerate() {
                    let (cx, cy) = list_cell(i);
                    screen.text(font, cx, cy, n, 0xDF);
                }
                return;
            }
            None => {}
        }
        // 26F7:064A: the current place's marker.
        let r8 = |a: u16| m.mem.r8(DS, a);
        let r16 = |a: u16| m.mem.r16(DS, a) as i32;
        if r8(0x0A46) == p {
            self.marker(screen, &self.mapout, r8(0x0A47) as u16, r8(0x0A4C) as u16, true, r16(0x0A48), r16(0x0A4A));
        }
        // 26F7:06F0: the destination's, three passes in four.
        let pass = (self.up_ms / TICK_MS).wrapping_add(1);
        let dest = (r8(0x6C28) != 0 && pass & 3 != 0 && r8(0x193D) == p).then(|| {
            let pick = m.mem.r16(DS, 0xA8AE);
            (r8(0x1943) as u16, pick, pick == r8(0x0A4C) as u16 && p == r8(0x0A46), r16(0x193E), r16(0x1940))
        });
        // 26F7:0625's label: the location nearest the cursor named
        // (shadow CEh, text DFh), on the map above the buttons.
        // The search's state is put back after: the label is the window's,
        // drawn whenever, and the game's state follows the clicks only.
        if y < 165 {
            let kept: Vec<u16> = SCAN.iter().map(|&a| m.mem.r16(DS, a)).collect();
            m.travel_nearest(x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
            let rec = if p == 8 { CITIES } else { m.mem.r16(DS, NEAREST_REC) };
            for (&a, &v) in SCAN.iter().zip(&kept) {
                m.mem.w16(DS, a, v);
            }
            let name = String::from_utf8_lossy(m.mem.cstr(DS, rec)).into_owned();
            let (lx, ly) = (m.mem.r16(DS, rec + 0x14) as i32, m.mem.r16(DS, rec + 0x16) as i32);
            let w = font.width(&name) as i32;
            let tx = (lx - w / 2).clamp(2, 318 - w);
            let ty = (ly - 16).max(2);
            screen.text_plain(font, tx + 1, ty + 1, &name, 0xCE);
            screen.text_plain(font, tx, ty, &name, 0xDF);
        }
        if let Some((kind, pick, here, x, y)) = dest {
            self.marker(screen, &self.mapblink, kind, pick, here, x, y);
        }
        // 1194:30F1: the day's frame of the journey.
        if let Some(f) = self.journey.get((self.journey_ms / TICK_MS) as usize).and_then(|&f| self.horse.get(f as usize)) {
            screen.blit(f.x, f.y, f.w, f.h, &f.px, 199);
        }
    }
}

/// The list's cell of entry `i` (two columns of 16).
fn list_cell(i: usize) -> (i32, i32) {
    (28 + (i / 16) as i32 * 136, 14 + (i % 16) as i32 * 9)
}

fn list_at(names: &[String], x: i32, y: i32) -> Option<usize> {
    (0..names.len()).find(|&i| {
        let (cx, cy) = list_cell(i);
        x >= cx && x < cx + 130 && y >= cy && y < cy + 9
    })
}

/// After an input to the travel map (or a frame): its boxes to `boxes`;
/// ended and the journey's frames played, it goes, its carry returned:
/// opened by the map `button`, the area left on the carry
/// (`Machine::map_button_done`, 2542:0411); else (`Travel::forced`) the
/// caller's.
pub fn after(travel: &mut Option<Travel>, engine: &mut Engine, boxes: &mut Vec<Lines>, button: bool) -> Option<bool> {
    let t = travel.as_mut()?;
    boxes.extend(t.take_boxes());
    let carry = t.done.filter(|_| !t.journeying())?;
    if button {
        engine.m.map_button_done(carry);
    }
    *travel = None;
    Some(carry)
}
