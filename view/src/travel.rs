//! The travel map (26F7:0000), as the original shows it: opened by the
//! map button's right click or Shift+M (2542:0193, 0380 with no left
//! release: 03BF): refused indoors ("You cannot travel from here.",
//! `Machine::map_from_here`), from a boat or with monsters near
//! (`Machine::map_button`).
//!
//! - It starts on the world map (the window at DATA:6C56, below); a
//!   province's region opens the province's map (26F7:0176): its picture
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
use arena::lineedit::EditKey;
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

/// A point of the world map whose click opens province `p`'s map: inside
/// its region (DATA:6D90) and no earlier one's, as the regions are tried
/// in order.
pub fn province_point(p: u8) -> (i32, i32) {
    let r = REGIONS[p as usize];
    (r.1..=r.1 + r.3)
        .flat_map(|y| (r.0..=r.0 + r.2).map(move |x| (x + r.2 / 2 - r.0, y)))
        .map(|(dx, y)| (r.0 + dx % (r.2 + 1), y))
        .find(|&(x, y)| REGIONS.iter().position(|&q| inside(q, x, y)) == Some(p as usize))
        .expect("a point of the province's own")
}

/// What the screen asks between clicks.
enum Asking {
    /// A name typed for the search (the game's editor, `field::Live`).
    Name(Box<crate::field::Live>),
    /// The province's places listed (the name typed matched none): the
    /// list as 1194:5734 shows it (kind 8, `list_kind`: POPUP8.IMG, its
    /// lines in EEh), the name's typing.
    List(hud::Menu, Vec<EditKey>),
}

/// 26F7:0AD5's field: 1194:5C30 with the prompt at 6AD1, 14h characters
/// into 9C62 (cleared before, 77C8, 1F4h bytes), in ARENAFNT.DAT
/// ([102C], the map's) and the map's colour EEh (26F7:0091's [9288],
/// over the 60h 2542:03CF set before it).
const SEARCH_LOOK: crate::field::Look = crate::field::Look { font: "ARENAFNT.DAT", colour: 0xEE, put_font: true };

pub struct Travel {
    start: MapStart,
    /// A main quest's dungeon reached: its picture up until its button is
    /// clicked (292B:0362).
    picture: Option<Picture>,
    /// The fonts the picture's lines may be in (B036 as 0000:5364 draws
    /// them).
    fonts: Vec<(hud::GameFont, Font)>,
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
    /// The boxes' tiles while it is up: PARCH.CIF's 20 x 20, which 26F7:
    /// 0050 loads (0000:756C, the name at 3AE0) and selects ([1038],
    /// 9AF8) for 965A.
    parch: Option<hud::Tiles>,
    /// Where the cursor last was over the map (the label's place).
    label_at: std::cell::Cell<Option<(i32, i32)>>,
    /// The label kept where it was: the search's screen (5E1D's copy,
    /// taken as its name is asked) put back after the list (0000:710E),
    /// until the map's next click.
    label_held: bool,
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
    typed: Vec<EditKey>,
    pick: u16,
    listed: Option<Vec<String>>,
    failed: bool,
    /// The journey's frames drawn (1194:30F1, one a day).
    frames: Vec<u8>,
    /// The province whose main quest's dungeon was reached (292B:0362's
    /// AX, `MapHooks::quest_dungeon`): its picture to show.
    quest: Option<u16>,
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
    /// Kept: the picture is shown after the screen's end (`Picture`), the
    /// game's state the same either way (292B:0362 reads and writes none
    /// of what 26F7:00CE goes on to).
    fn quest_dungeon(&mut self, _m: &mut Machine, ax: u16) {
        self.quest = Some(ax);
    }
    /// The name's typing given to the game's editor (`field::replay`).
    fn type_name(&mut self, m: &mut Machine) {
        if crate::field::replay(m, &arena::lineedit::fields::search(), &SEARCH_LOOK, &self.typed) != Some(true) {
            eprintln!("the search's name gave up: {}", arena::giveup::describe());
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
    /// screen's start (26F7:0000) on the world map.
    pub fn open(engine: &mut Engine, archive: &crate::archive::Archive) -> Result<Travel, Lines> {
        let mut boxes = Vec::new();
        let m = &mut engine.m;
        {
            let mut h = Hooks { boxes: &mut boxes, files: engine.files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None };
            // 2542:03BF: from here at all (038F), the colour 60h (03CF),
            // then the button's own refusals (03DC).
            if !m.map_from_here(&mut h) {
                return Err(boxes.pop().unwrap_or_default());
            }
            m.mem.w8(DS, 0x9288, 0x60);
            let mut h = Hooks { boxes: &mut boxes, files: engine.files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None };
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

    /// 26F7:0000's start, on the world map.
    fn begin(engine: &mut Engine, archive: &crate::archive::Archive, mut boxes: Vec<Lines>) -> Travel {
        let m = &mut engine.m;
        let mut h = Hooks { boxes: &mut boxes, files: engine.files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None };
        let start = m.travel_map_begin(&mut h);
        let raw = |name: &str, w: usize, h: usize| archive.read(name).map(|d| Image { x: 0, y: 0, w, h, px: d });
        // The window it runs (3159:000C on DATA:6C56) is the world map's:
        // TAMRIEL.MNU, its provinces' regions at 6D90, the current one
        // outlined (26F7:013E). A province's map is a region's click away.
        let fonts = [(hud::GameFont::A, "FONT_A.DAT"), (hud::GameFont::B, "FONT_B.DAT"), (hud::GameFont::Arena, "ARENAFNT.DAT"), (hud::GameFont::Teeny, "TEENYFNT.DAT")]
            .into_iter()
            .filter_map(|(g, n)| Font::load(archive, n).map(|f| (g, f)))
            .collect();
        Travel {
            start,
            picture: None,
            fonts,
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
            parch: hud::Tiles::load(archive, "PARCH.CIF", 20, 20),
            label_at: std::cell::Cell::new(None),
            label_held: false,
            mapblink: archive.read("MAPBLINK.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            horse: dfa(archive, "HORSE.DFA"),
            journey: Vec::new(),
            journey_ms: 0,
            up_ms: 0,
        }
    }

    /// The boxes shown since last asked.
    pub fn take_boxes(&mut self) -> Vec<Lines> {
        std::mem::take(&mut self.boxes)
    }

    fn hooks<'a>(boxes: &'a mut Vec<Lines>, files: &Files) -> Hooks<'a> {
        Hooks { boxes, files: files.clone(), typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None }
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
        let quest = h.quest;
        self.done = Some(carry);
        self.picture = quest.map(|ax| Picture::drawn(engine, ax));
    }

    /// The picture of province `ax`'s main quest dungeon put up as the
    /// screen's end puts it up (the tests': no journey made).
    #[cfg(test)]
    pub fn show_picture(&mut self, engine: &Engine, ax: u16) {
        self.picture = Some(Picture::drawn(engine, ax));
    }

    /// Whether a main quest's dungeon's picture is up.
    pub fn picture_up(&self) -> bool {
        self.picture.is_some()
    }

    /// A click on the picture: in its button (110h..13Ah, B7h..C3h, as
    /// 292B:042B tests the mouse), 292B:0362 run on the game with it, the
    /// picture gone; elsewhere nothing.
    fn picture_click(&mut self, engine: &mut Engine, x: i32, y: i32) {
        let Some(p) = &self.picture else { return };
        if !((0x110..=0x13A).contains(&x) && (0xB7..=0xC3).contains(&y)) {
            return;
        }
        let mut h = PictureHooks { click: Some((x as u16, y as u16)), ..Default::default() };
        if engine.m.dungeon_picture(&mut engine.files, &mut h, p.ax).is_none() {
            eprintln!("the dungeon's picture gave up: {}", arena::giveup::describe());
        }
        self.picture = None;
    }

    /// A click at (x, y) of the screen (a button down: `right` the right).
    pub fn click(&mut self, engine: &mut Engine, archive: &crate::archive::Archive, x: i32, y: i32, right: bool) {
        if self.picture.is_some() {
            self.picture_click(engine, x, y);
            return;
        }
        if self.done.is_some() {
            return;
        }
        if let Some(Asking::List(menu, typed)) = &mut self.asking {
            // The right button leaves the list (none picked); a line
            // clicked is marked, the marked line clicked picked (5734).
            if right {
                let typed = typed.clone();
                self.asking = None;
                self.search(engine, typed, 0xFFFF);
                return;
            }
            let Some(i) = hud::custom::game_list_pick(menu, x, y) else { return };
            if i != menu.selected {
                menu.selected = i;
                return;
            }
            let typed = typed.clone();
            self.asking = None;
            self.search(engine, typed, i as u16);
            return;
        }
        // The name's field: the right button ends it (A172).
        if let Some(Asking::Name(live)) = &mut self.asking {
            if right {
                live.button();
                self.name_ended(engine);
            }
            return;
        }
        self.label_held = false;
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
            self.ask_name(engine);
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

    /// The province whose map is shown; None: the world map.
    pub fn shown(&self) -> Option<u8> {
        self.shown
    }

    /// `shown` while the screen is still up (not ended).
    pub fn shown_now(&self) -> Option<Option<u8>> {
        self.done.is_none().then_some(self.shown)
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
        let before = self.up_ms / TICK_MS;
        self.up_ms = self.up_ms.wrapping_add(ms);
        // The editor's pass (its blink, a held arrow) each of the map's
        // passes, a tick each as the label's and the markers' (a call
        // with no time, the scripted shots', one pass).
        if let Some(Asking::Name(live)) = &mut self.asking {
            for _ in 0..(self.up_ms / TICK_MS).wrapping_sub(before).max((ms == 0) as u32) {
                live.frame();
            }
        }
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
    fn search(&mut self, engine: &mut Engine, typed: Vec<EditKey>, pick: u16) {
        let mut copy = engine.m.clone();
        let mut scratch = Vec::new();
        let mut h = Self::hooks(&mut scratch, &engine.files);
        h.typed = typed.clone();
        h.pick = 0xFFFF;
        let _ = copy.travel_search(&mut engine.files.clone(), &mut h);
        if let (Some(names), 0xFFFF) = (h.listed.take(), pick) {
            let entries = names.iter().map(|n| hud::lines(n.as_bytes(), 0x0F).into_iter().flatten().collect()).collect();
            self.asking = Some(Asking::List(hud::list_menu(&copy, Vec::new(), entries), typed));
            return;
        }
        let mut h = Self::hooks(&mut self.boxes, &engine.files);
        h.typed = typed;
        h.pick = pick;
        if engine.m.travel_search(&mut engine.files, &mut h).is_none() {
            eprintln!("the search gave up: {}", arena::giveup::describe());
        }
    }

    /// The search's name asked: the game's editor opened on a copy of the
    /// game, its buffer cleared as 26F7:0AC4 clears it.
    fn ask_name(&mut self, engine: &Engine) {
        let mut m = engine.m.clone();
        for i in 0..0x1F4 {
            m.mem.w8(DS, 0x9C62 + i, 0);
        }
        // What 5C34's box (0000:965A with CX 7) leaves at A84C, the
        // editor's blink count after: CX, 80h once drawn (970F), 40h as
        // its last pass over the text (98A0, from the text's start) finds
        // the first line ended by a CR.
        m.mem.w16(DS, arena::lineedit::var::BLINK, 0x07 | 0x80 | 0x40);
        self.asking = crate::field::Live::open(&m, arena::lineedit::fields::search(), SEARCH_LOOK).map(|l| Asking::Name(Box::new(l)));
        self.label_held = true;
    }

    /// Whether the search's name is being typed (the keys, pressed and
    /// let go, are `field_key`'s).
    pub fn typing(&self) -> bool {
        matches!(self.asking, Some(Asking::Name(_)))
    }

    /// A key pressed or let go while the name is typed: to the game's
    /// editor; when the field ends, the search with what was typed.
    pub fn field_key(&mut self, engine: &mut Engine, scan: u8, down: bool) {
        if let Some(Asking::Name(live)) = &mut self.asking {
            live.key(scan, down);
        }
        self.name_ended(engine);
    }

    fn name_ended(&mut self, engine: &mut Engine) {
        if let Some(Asking::Name(live)) = &self.asking {
            if live.ended() {
                let typed = live.keys().to_vec();
                self.asking = None;
                self.search(engine, typed, 0xFFFF);
            }
        }
    }

    /// A key: Esc as Exit (a list: left); S the search, T or Enter
    /// travels.
    pub fn key(&mut self, engine: &mut Engine, code: winit::keyboard::KeyCode, _text: Option<&str>) {
        use winit::keyboard::KeyCode;
        if self.done.is_some() {
            return;
        }
        match &mut self.asking {
            Some(Asking::Name(_)) => {}
            Some(Asking::List(..)) => {
                if code == KeyCode::Escape {
                    self.asking = None;
                }
            }
            None => match code {
                KeyCode::Escape => self.exit(engine),
                KeyCode::KeyT | KeyCode::Enter if self.shown.is_some() => self.travel(engine),
                KeyCode::KeyS if self.shown.is_some() => self.ask_name(engine),
                _ => {}
            },
        }
    }

    /// A message box over the map (1194:59C8: 965A with margin 5, centred
    /// on the screen) in its tiles (`parch`) and its text in EEh, the
    /// colour 26F7:0091 sets ([9288]).
    pub fn draw_box(&self, screen: &mut Screen, font: &Font, lines: &[Vec<hud::Span>]) {
        if let Some(t) = &self.parch {
            hud::custom::draw_box(screen, t, font, lines, 5, 0xEE);
        }
    }

    /// The screen, the cursor at (x, y): its palette set.
    pub fn draw(&self, screen: &mut Screen, font: &Font, m: &mut Machine, x: i32, y: i32) {
        let blit = |screen: &mut Screen, i: &Image, x: i32, y: i32| screen.blit(x, y, i.w, i.h, &i.px, 199);
        if let Some(p) = &self.picture {
            p.draw(screen, &self.fonts);
            return;
        }
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
        // 26F7:0625's label: the location nearest the cursor named, on the
        // map above the buttons. Its row the place's less the kind's
        // height (DATA:6AA0 by 0B7A's BX: a town, city, village or
        // dungeon), 2 if that is above the screen; centred on the place
        // (less half its width), 2 if that is left of the screen, and
        // moved in from the right (13Eh less the width, its lowest bit
        // dropped) where it would reach 140h; drawn one pixel right in CEh,
        // then in DFh (9E06).
        // The search's state is put back after: the label is the window's,
        // drawn whenever, and the game's state follows the clicks only.
        // Over the buttons the pass isn't run (their regions' handlers
        // are): the label stays where the cursor last was over the map.
        if y < 165 && !self.label_held && self.asking.is_none() {
            self.label_at.set(Some((x, y)));
        }
        if let Some((x, y)) = self.label_at.get() {
            let kept: Vec<u16> = SCAN.iter().map(|&a| m.mem.r16(DS, a)).collect();
            let kind = m.travel_nearest(x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
            let rec = if p == 8 { CITIES } else { m.mem.r16(DS, NEAREST_REC) };
            for (&a, &v) in SCAN.iter().zip(&kept) {
                m.mem.w16(DS, a, v);
            }
            let name = String::from_utf8_lossy(m.mem.cstr(DS, rec)).into_owned();
            let (lx, ly) = (m.mem.r16(DS, rec + 0x14), m.mem.r16(DS, rec + 0x16));
            let ty = ly.wrapping_sub(m.mem.r8(DS, 0x6AA0 + kind) as u16);
            let ty = if (ty as i16) < 0 { 2 } else { ty };
            let half = font.width(&name) as u16 >> 1;
            let tx = lx.wrapping_sub(half);
            let tx = if (tx as i16) < 0 {
                2
            } else if tx.wrapping_add(half << 1) >= 0x140 {
                0x13E - (half << 1)
            } else {
                tx
            };
            let (tx, ty) = (tx as i16 as i32, ty as i16 as i32);
            screen.text_plain(font, tx + 1, ty, &name, 0xCE);
            screen.text_plain(font, tx, ty, &name, 0xDF);
        }
        if let Some((kind, pick, here, x, y)) = dest {
            self.marker(screen, &self.mapblink, kind, pick, here, x, y);
        }
        // What the screen asks, over the map as its last pass left it.
        match &self.asking {
            Some(Asking::Name(live)) => {
                // 26F7:0ABC: 1194:5C30 with the prompt at 6AD1, its box
                // (965A with margin 0Ah, in the map's tiles and colour, as
                // `draw_box`), then the field.
                if let Some(t) = &self.parch {
                    let prompt = crate::field::prompt_5c30(m, DS, 0x6AD1);
                    hud::custom::draw_box(screen, t, font, &hud::lines(prompt.as_bytes(), 0x0F), 0x0A, 0xEE);
                }
                live.draw(screen, false);
            }
            Some(Asking::List(menu, _)) => hud::custom::draw_game_list(screen, menu),
            None => {}
        }
        // 1194:30F1: the day's frame of the journey.
        if let Some(f) = self.journey.get((self.journey_ms / TICK_MS) as usize).and_then(|&f| self.horse.get(f as usize)) {
            screen.blit(f.x, f.y, f.w, f.h, &f.px, 199);
        }
    }
}

/// After an input to the travel map (or a frame): its boxes to `boxes`;
/// ended and the journey's frames played, it goes, its carry returned:
/// opened by the map `button`, the area left on the carry
/// (`Machine::map_button_done`, 2542:0411); else (`Travel::forced`) the
/// caller's.
pub fn after(travel: &mut Option<Travel>, engine: &mut Engine, boxes: &mut Vec<Lines>, button: bool) -> Option<bool> {
    let t = travel.as_mut()?;
    boxes.extend(t.take_boxes());
    let carry = t.done.filter(|_| !t.journeying() && t.picture.is_none())?;
    if button {
        engine.m.map_button_done(carry);
    }
    *travel = None;
    Some(carry)
}

/// A main quest's dungeon's picture as 292B:0362 draws it (`Machine::
/// dungeon_picture` run on a copy of the game, its button never clicked):
/// the picture at its own place with the colours it fades in to, the
/// dungeon's lines centred on x A0h (0000:5364) in the font and colour
/// selected.
struct Picture {
    ax: u16,
    img: Option<Image>,
    palette: Palette,
    lines: Vec<(String, i32, i32, hud::GameFont, u8)>,
}

/// The picture's drawing collected, and its button's click.
#[derive(Default)]
struct PictureHooks {
    img: Option<Image>,
    palette: Option<Palette>,
    lines: Vec<(String, i32, i32, hud::GameFont, u8)>,
    click: Option<(u16, u16)>,
}

impl arena::dungeonpic::DungeonPicHooks for PictureHooks {
    /// 0000:5828: the file 7428 left at [9030]:[902E] (its size at A90C).
    fn picture(&mut self, m: &mut Machine) {
        let (off, seg, n) = (m.mem.r16(DS, 0x902E), m.mem.r16(DS, 0x9030), m.mem.r16(DS, 0xA90C));
        let d: Vec<u8> = (0..n).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
        self.img = crate::interface::image(&d).map(|(i, _)| i).or_else(|| crate::panels::image_type8(m, &d));
    }
    fn line(&mut self, m: &mut Machine, si: u16, ax: u16, bx: u16) {
        let text = String::from_utf8_lossy(m.mem.cstr(DS, si)).into_owned();
        self.lines.push((text, ax as i32, bx as i32, hud::GameFont::selected(m), m.mem.r8(DS, 0x9288)));
    }
    /// 5E3E's colours (6 bits each, 300h bytes at `seg`:`off`).
    fn shown(&mut self, m: &mut Machine, seg: u16, off: u16) {
        let mut pal = [[0u8; 4]; 256];
        for (i, c) in pal.iter_mut().enumerate() {
            let v = |k: u16| {
                let x = m.mem.r8(seg, off.wrapping_add(3 * i as u16 + k)) & 0x3F;
                x << 2 | x >> 4
            };
            *c = [v(0), v(1), v(2), 0xFF];
        }
        self.palette = Some(pal);
    }
    fn click(&mut self, m: &mut Machine) -> Option<()> {
        let (x, y) = self.click.take()?;
        m.mem.w16(DS, 0x8F92, x);
        m.mem.w16(DS, 0x8F94, y);
        Some(())
    }
}

impl Picture {
    fn drawn(engine: &Engine, ax: u16) -> Picture {
        let mut m = engine.m.clone();
        let mut h = PictureHooks::default();
        if m.dungeon_picture(&mut engine.files.clone(), &mut h, ax).is_none() {
            eprintln!("the dungeon's picture gave up: {}", arena::giveup::describe());
        }
        Picture { ax, img: h.img, palette: h.palette.unwrap_or([[0, 0, 0, 0xFF]; 256]), lines: h.lines }
    }

    fn draw(&self, screen: &mut Screen, fonts: &[(hud::GameFont, Font)]) {
        screen.set_palette(self.palette);
        screen.fill(0, 0, 320, 200, 0, 0xFF);
        if let Some(i) = &self.img {
            screen.blit(i.x, i.y, i.w, i.h, &i.px, 199);
        }
        for (text, x, y, g, colour) in &self.lines {
            if let Some((_, f)) = fonts.iter().find(|(k, _)| k == g) {
                screen.text_plain(f, x - (f.width(text) as i32 >> 1), *y, text, *colour);
            }
        }
    }
}
