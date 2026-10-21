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
use arena::mapscreen::{MapHooks, MapRegion, MapStart};
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

/// The location records' lists (19h bytes each).
const CITIES: u16 = 0x1445;
const TOWNS: u16 = 0x150D;
const VILLAGES: u16 = 0x15D5;
const DUNGEONS: u16 = 0x1765;
const RECORD: u16 = 0x19;
const NEAREST_REC: u16 = 0xA8BA;
/// What `travel_nearest` writes: the nearest's index, record, distance,
/// and the known-only switch.
const SCAN: [u16; 4] = [0xA8BC, NEAREST_REC, 0xA8B6, 0x1294];

/// A point of the world map whose click opens province `p`'s map, the
/// world map's window up in `m`: inside its region (DATA:6D90) with its
/// mask's bit set and in no later region's (3159:019A keeps the last).
pub fn province_point(m: &Machine, p: u8) -> (i32, i32) {
    let r = REGIONS[p as usize];
    let mut m = m.clone();
    (r.1..=r.1 + r.3)
        .flat_map(|y| (r.0..=r.0 + r.2).map(move |x| (x + r.2 / 2 - r.0, y)))
        .map(|(dx, y)| (r.0 + dx % (r.2 + 1), y))
        .find(|&(x, y)| m.travel_world_region(x as u16, y as u16) == Some(p as u16))
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
    /// Travel while unwell (05B1 bits 0..2): 1194:48DB's question, the
    /// menu 1194:020E with the text at 6B1D, its keys at 4B60 ("YN") and
    /// entries at 4B63 (Yes, No): its title and entries.
    Question(Lines, Lines),
}

/// 1194:48DB's question (DGROUP 6B1D) and its menu's entries' table (4B63:
/// Yes, No).
const QUESTION: u16 = 0x6B1D;
const QUESTION_ENTRIES: u16 = 0x4B63;

/// 26F7:0AD5's field: 1194:5C30 with the prompt at 6AD1, 14h characters
/// into 9C62 (cleared before, 77C8, 1F4h bytes), in ARENAFNT.DAT
/// ([102C], the map's) and the map's colour EEh (26F7:0091's [9288],
/// over the 60h 2542:03CF set before it).
const SEARCH_LOOK: crate::field::Look = crate::field::Look { font: "ARENAFNT.DAT", colour: 0xEE, put_font: true };
/// The search's prompt (26F7:0ACF's SI).
const SEARCH_PROMPT: u16 = 0x6AD1;

pub struct Travel {
    start: MapStart,
    /// The left button down as the screen began (26F7:0011's read: a
    /// walk by the mouse held into the way out), not yet let go (`hold`,
    /// `let_go`).
    held: bool,
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
    /// The search's list's loop as it starts, on a copy of the game
    /// (`arena::listwindow::ListCapture`), and the reads given it so far:
    /// what the loop makes of them is what the list shows, its end the
    /// pick.
    list_live: Option<Box<arena::listwindow::ListCapture>>,
    list_reads: Vec<arena::listwindow::ListRead>,
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
    /// What the screen shows before the days and after them, a retrace
    /// each (`journey_from`): 26F7:081E's 55B2 (the screen as the click
    /// left it to black in 8, cleared), 56C8 with BX 24h (the cleared
    /// screen to the world map's colours in 12, 5E50), and 08F1's 55B2 (the
    /// last day to black in 8).
    journey_lead: Vec<(Vec<u8>, crate::film::Dac)>,
    journey_tail: Vec<(Vec<u8>, crate::film::Dac)>,
    /// A journey was made (its days played): the screen ends black.
    pub journeyed: bool,
    /// The game's page as the journey left it (the world map's picture,
    /// 26F7:07CD's 56C8), under its frames; where the page is the
    /// heap's block.
    journey_page: Option<Vec<u8>>,
    /// The time the screen has been up (ms): its passes (A8C4's blink).
    up_ms: u32,
    /// The boxes' tiles while it is up: PARCH.CIF's 20 x 20, which 26F7:
    /// 0050 loads (0000:756C, the name at 3AE0) and selects ([1038],
    /// 9AF8) for 965A.
    parch: Option<hud::Tiles>,
    /// Where the cursor last was over the map (the label's place).
    label_at: std::cell::Cell<Option<(i32, i32)>>,
    /// Where the screen was last clicked (8F92, 8F94 for its passes).
    at: (u16, u16),
    /// The label kept where it was: the search's screen (5E1D's copy,
    /// taken as its name is asked) put back after the list (0000:710E),
    /// until the map's next click.
    label_held: bool,
    /// The page the screen's first box opened over (`page`, the journey's
    /// box at 1194:59C8 keeping what it covers of it).
    pub boxed_over: Option<Vec<u8>>,
    /// The game's page as the first province's map opened left it (its
    /// picture and its places' icons, 26F7:01CD), where the page is the
    /// heap's block.
    pub province_page: Option<Vec<u8>>,
    /// The map's ticks the loop's idle passes have run to (`idle`).
    idle_ticks: u32,
    /// The page a box shown from the map opened over (what 1194:59C8
    /// keeps of it), the screen while it waits: the rest of its pass (or
    /// the pass after its handler) comes once it is clicked away, the
    /// loop's next pass putting this by.
    box_page: Option<Vec<u8>>,
    /// The screen once the unwell traveller's question was answered (its
    /// menu gone, `click`): where the journey's fades start.
    answered_over: Option<Vec<u8>>,
}

/// A game tick (1/18.2 s) in ms: a pass of the screen's loop, a day's
/// frame of the journey. The view's own pace: the original's window loop
/// (3159:000C) and its days (1194:30F1) neither wait for the retrace nor
/// read the timer, so they run as fast as the machine does (areaplay's
/// `journey` lines).
const TICK_MS: u32 = 55;

/// Screens as shown, each its pixels and 6-bit colours.
#[cfg(test)]
pub type Shown = Vec<(Vec<u8>, Vec<u8>)>;

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
    typed: Vec<EditKey>,
    pick: u16,
    listed: Option<Vec<String>>,
    failed: bool,
    /// The journey's frames drawn (1194:30F1, one a day).
    frames: Vec<u8>,
    /// The unwell traveller's question's answer (1194:48DB's AX: 0 Yes,
    /// 1 No, FFFFh none), once asked; `asked` whether it was.
    answer: Option<u16>,
    asked: bool,
    /// The game's page as the question's menu left it (1194:020E's end:
    /// its boxes put back, 933C), where it is the heap's block.
    menu_page: Option<Vec<u8>>,
    /// The province whose main quest's dungeon was reached (292B:0362's
    /// AX, `MapHooks::quest_dungeon`): its picture to show.
    quest: Option<u16>,
    /// The page as the screen's pass left it (`Travel::page`), which a
    /// box keeps what it covers of (1194:59C8).
    screen: Option<Vec<u8>>,
    /// The game's page as the journey's first day found it (the world
    /// map's picture, 26F7:07CD's 56C8 with BX 24h), where the page is
    /// the heap's block.
    journey_page: Option<Vec<u8>>,
    /// The search's list: whether its loop's start is to be kept
    /// (`Machine::list_capture`), and the reads its loop runs over
    /// (`Machine::list_reads`).
    capture: bool,
    reads: Option<Vec<arena::listwindow::ListRead>>,
    /// Run from the window's loop's pass (`Travel::pass`).
    in_pass: bool,
}

impl arena::clock::ClockHooks for Hooks<'_> {}

impl TravelHooks for Hooks<'_> {
    /// 1194:59C8: the box shown, and what it leaves in the game's state
    /// (`Machine::message_shown`: what it covers of the page kept in bank
    /// 18h, put away after the player's click).
    fn message_box(&mut self, m: &mut Machine, seg: u16, si: u16) {
        let lines = hud::lines(m.mem.cstr(seg, si), 0x0F);
        for l in &lines {
            println!("{}", hud::plain(l));
        }
        self.boxes.push(lines);
        // From the loop's pass (26F7:0625): over the page as the pass has
        // it so far (its label drawn, 0676, before the pick that shows the
        // box), where it is the game's own block. (The search's, from its
        // button's handler, opens over the screen its list's put-back
        // left, 1194:5E1D: the front end's.)
        if self.in_pass && m.page_is_block() {
            self.screen = Some(m.mem.segment(m.mem.r16(DS, 0xA900))[..64000].to_vec());
        }
        if m.message_shown((seg, si), self.screen.as_deref()).is_none() {
            eprintln!("the message box gave up: {}", arena::giveup::describe());
        }
    }
    /// 1194:48DB: the question (unwell: "Considering your condition...")
    /// asked as a menu (`Asking::Question`); not yet answered, staying.
    /// Answered, its menu (48E9: 020E with the keys at 4B60, the entries
    /// at 4B63) is the game's round the answer (`Machine::menu_boxes`,
    /// `menu_closed`: its boxes drawn on the page and put back), and the
    /// page it leaves kept for the journey's first fade.
    fn question(&mut self, m: &mut Machine, si: u16) -> u16 {
        self.asked = true;
        let Some(answer) = self.answer else { return 0xFFFF };
        match m.menu_boxes(si, QUESTION_ENTRIES, None) {
            Some(boxes) => m.menu_closed(boxes.as_ref()),
            None => eprintln!("the question's menu gave up: {}", arena::giveup::describe()),
        }
        if m.page_is_block() {
            self.menu_page = Some(m.mem.segment(m.mem.r16(DS, 0xA900))[..64000].to_vec());
        }
        answer
    }
    fn location(&mut self, m: &mut Machine, files: &mut Files, id: u16) {
        let ax = if m.mem.r8(DS, 0x0A46) == 8 { 0x100 } else { id };
        if m.city_location(files, DS, 0x0A22, ax).is_none() {
            self.failed = true;
        }
    }
    fn animation_frame(&mut self, m: &mut Machine, frame: u8) {
        if self.frames.is_empty() && m.page_is_block() {
            self.journey_page = Some(m.mem.segment(m.mem.r16(DS, 0xA900))[..64000].to_vec());
        }
        self.frames.push(frame);
    }
    fn dungeon_location(&mut self, m: &mut Machine, files: &mut Files, id: u16, _bx: u16) {
        if m.dungeon_location_built(files, id as u8).is_none() {
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
    /// 26F7:0AD5's 5C30: the prompt (6AD1) in its box as the game's state
    /// has it round the field (`Machine::named_shown`, `named_closed`;
    /// BX, an offset on the original's stack, 0: the field writes it
    /// over), the name's typing given to the game's editor
    /// (`field::replay`).
    fn type_name(&mut self, m: &mut Machine) {
        if m.named_shown((DS, SEARCH_PROMPT), 0, None).is_none() {
            eprintln!("the search's box gave up: {}", arena::giveup::describe());
            return;
        }
        if crate::field::replay(m, &arena::lineedit::fields::search(), &SEARCH_LOOK, &self.typed) != Some(true) {
            eprintln!("the search's name gave up: {}", arena::giveup::describe());
        }
        m.named_closed();
    }
    /// The names listed for the window, and the game's state round the
    /// pick (`Machine::list_window`, kind 8: the screen kept in the load
    /// bank, the panel, the window's record; its loop over `reads`) where
    /// the page is the game's own block.
    fn list(&mut self, m: &mut Machine, files: &mut arena::bsa::Files, si: u16, cx: u16, di: u16) -> u16 {
        let mut at = si;
        let mut names = Vec::new();
        for _ in 0..cx {
            let s = m.mem.cstr(DS, at);
            at = at.wrapping_add(s.len() as u16 + 1);
            names.push(String::from_utf8_lossy(s).into_owned());
        }
        self.listed = Some(names);
        if !m.page_is_block() {
            return self.pick;
        }
        m.list_capture = self.capture;
        m.list_reads = self.reads.take();
        m.list_window(files, 8, (DS, si), cx, di, self.pick).unwrap_or_else(|| {
            eprintln!("the search's list gave up: {}", arena::giveup::describe());
            self.pick
        })
    }
}

impl Travel {
    /// The map button pressed (2542:03DC): refused, its box; else the
    /// world kept (2542:0400, `Machine::map_keep`, `dac` the colours on
    /// the screen) and the screen's start (26F7:0000) on the world map.
    pub fn open(engine: &mut Engine, archive: &crate::archive::Archive, dac: Option<&[[u8; 3]; 256]>) -> Result<Travel, Lines> {
        let mut boxes = Vec::new();
        let m = &mut engine.m;
        {
            let mut h = Hooks { boxes: &mut boxes, typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None, answer: None, asked: false, menu_page: None, screen: None, journey_page: None, capture: false, reads: None, in_pass: false };
            // 2542:03BF: from here at all (038F), the colour 60h (03CF),
            // then the button's own refusals (03DC).
            if !m.map_from_here(&mut h) {
                return Err(boxes.pop().unwrap_or_default());
            }
            m.mem.w8(DS, 0x9288, 0x60);
            let mut h = Hooks { boxes: &mut boxes, typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None, answer: None, asked: false, menu_page: None, screen: None, journey_page: None, capture: false, reads: None, in_pass: false };
            if !m.map_button(&mut h) {
                return Err(boxes.pop().unwrap_or_default());
            }
        }
        engine.m.map_keep(&mut engine.files, dac);
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
        let mut h = Hooks { boxes: &mut boxes, typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None, answer: None, asked: false, menu_page: None, screen: None, journey_page: None, capture: false, reads: None, in_pass: false };
        // The window's colours read from black (1194:55B2's fade before).
        let start = m.travel_map_open(&mut engine.files, &mut h, Some(&[[0; 3]; 256])).unwrap_or_else(|| {
            eprintln!("the travel map's start gave up: {}", arena::giveup::describe());
            MapStart { id: m.mem.r8(DS, 0x0A4C), kind: m.mem.r8(DS, 0x0A47), province: m.mem.r8(DS, 0x0A46) }
        });
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
            held: false,
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
            list_live: None,
            list_reads: Vec::new(),
            boxes,
            done: None,
            mapout: archive.read("MAPOUT.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            parch: hud::Tiles::load(archive, "PARCH.CIF", 20, 20),
            label_at: std::cell::Cell::new(None),
            label_held: false,
            at: (0, 0),
            mapblink: archive.read("MAPBLINK.CIF").map(|d| cif(m, &d)).unwrap_or_default(),
            horse: dfa(archive, "HORSE.DFA"),
            journey: Vec::new(),
            journey_ms: 0,
            journey_lead: Vec::new(),
            journey_tail: Vec::new(),
            journeyed: false,
            journey_page: None,
            up_ms: 0,
            boxed_over: None,
            province_page: None,
            idle_ticks: 0,
            box_page: None,
            answered_over: None,
        }
    }

    /// The left button down as the screen begins: the map's read
    /// (26F7:0011) finds it, and the world map's pass (3159:00F3, AFF9 as
    /// it is, not a press) takes it as a click begun.
    pub fn hold(&mut self) {
        self.held = true;
    }

    /// Whether the button the screen began with is still down.
    pub fn held(&self) -> bool {
        self.held
    }

    /// The button the screen began with forgotten: let go before the
    /// map's read.
    pub fn forget_held(&mut self) {
        self.held = false;
    }

    /// The button the screen began with let go at (x, y): on the world
    /// map, a pass with it down over a region waits for its release
    /// (3159:0432, the region found again at each read, 019A) and then
    /// runs the region's handler, as a click there (a province's map
    /// opened, or the Exit); over none, nothing.
    pub fn let_go(&mut self, engine: &mut Engine, archive: &crate::archive::Archive, x: i32, y: i32) {
        if !std::mem::take(&mut self.held) || self.shown.is_some() || self.asking.is_some() || self.done.is_some() {
            return;
        }
        let (cx, cy) = (x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
        if engine.m.clone().travel_world_region(cx, cy).is_some() {
            self.click(engine, archive, x, y, false);
        }
    }

    /// The boxes shown since last asked.
    pub fn take_boxes(&mut self) -> Vec<Lines> {
        std::mem::take(&mut self.boxes)
    }

    fn hooks<'a>(boxes: &'a mut Vec<Lines>) -> Hooks<'a> {
        Hooks { boxes, typed: Vec::new(), pick: 0xFFFF, listed: None, failed: false, frames: Vec::new(), quest: None, answer: None, asked: false, menu_page: None, screen: None, journey_page: None, capture: false, reads: None, in_pass: false }
    }

    /// 26F7:0176: province `p`'s map (its data loaded and its window
    /// opened, `Machine::travel_province_open`).
    fn show_province(&mut self, engine: &mut Engine, archive: &crate::archive::Archive, p: u8) {
        let mut h = Self::hooks(&mut self.boxes);
        if engine.m.travel_province_open(&mut engine.files, &mut h, p, None).is_none() {
            eprintln!("the province's map gave up: {}", arena::giveup::describe());
        }
        if self.province_page.is_none() {
            self.province_page = engine.m.page_is_block().then(|| engine.m.mem.segment(engine.m.mem.r16(DS, 0xA900))[..64000].to_vec());
        }
        self.province = PICTURES.get(p as usize).and_then(|n| crate::create::image_with_palette(&engine.m, archive, n));
        self.shown = Some(p);
        // The province's loop's first pass (3159:0078..0085: 019A, then
        // 26F7:0625), before any input: bank 4 mapped, the page put back.
        self.pass(engine, false);
    }

    /// 26F7:0218: the province's window ended (its Exit, or a journey
    /// made), the world map's window back (`Machine::
    /// travel_province_close`); true where the world's loop ends too (a
    /// journey made), after its pass (3159:0085's 019A, the cursor where
    /// it was).
    fn province_close(&mut self, engine: &mut Engine) -> bool {
        self.shown = None;
        match engine.m.travel_province_close(&mut engine.files, None) {
            Some(ended) => {
                if ended {
                    engine.m.travel_world_region(self.at.0, self.at.1);
                }
                ended
            }
            None => {
                eprintln!("the province's map's end gave up: {}", arena::giveup::describe());
                true
            }
        }
    }

    /// The page as a pass of the province's map leaves it, the cursor at
    /// `at` (`draw` with nothing asked): what a box opened from its
    /// handlers covers.
    fn page(&self, engine: &Engine, (x, y): (u16, u16)) -> Option<Vec<u8>> {
        let font = self.menu_font()?;
        let mut screen = Screen::new([[0, 0, 0, 0xFF]; 256]);
        let mut m = engine.m.clone();
        let held = self.label_at.get();
        self.draw(&mut screen, font, &mut m, x as i32, y as i32);
        self.label_at.set(held);
        Some(screen.indices().iter().map(|&c| c as u8).collect())
    }

    /// A pass of the province's window's loop (3159:0078..0085: 019A,
    /// then its routine 26F7:0625, `Machine::travel_click`) with the
    /// cursor where it was last clicked and the left button down
    /// (`down`) or not: the nearest place, and with the button a pick
    /// and its journey's box.
    fn pass(&mut self, engine: &mut Engine, down: bool) {
        use arena::controls::var::{BUTTONS, MOUSE_X, MOUSE_Y};
        let screen = self.page(engine, self.at);
        let m = &mut engine.m;
        m.mem.w16(DS, MOUSE_X, self.at.0);
        m.mem.w16(DS, MOUSE_Y, self.at.1);
        m.window_regions();
        let b = m.mem.r8(DS, BUTTONS);
        m.mem.w8(DS, BUTTONS, down as u8);
        let shown = self.boxes.len();
        let mut h = Self::hooks(&mut self.boxes);
        h.screen = screen;
        h.in_pass = true;
        if m.travel_click(&mut engine.files, &mut h).is_none() {
            eprintln!("the map's pass gave up: {}", arena::giveup::describe());
        }
        let screen = h.screen.take();
        if self.boxes.len() > shown {
            if self.boxed_over.is_none() {
                self.boxed_over = screen.clone();
            }
            self.box_page = screen;
        } else {
            self.box_page = None;
        }
        m.mem.w8(DS, BUTTONS, b);
    }

    /// Whether the province's window's loop is running its passes: its
    /// map up, nothing asked over it, no journey or picture, not ended.
    fn looping(&self) -> bool {
        self.shown.is_some() && self.asking.is_none() && self.journey.is_empty() && self.picture.is_none() && self.done.is_none()
    }

    /// The cursor moved to (x, y) over the screen, no button: the loop's
    /// next pass reads it there (3159:00B4's 8FC4) and 26F7:0625 runs on
    /// it wherever it is, over the buttons too (`idle`): the place
    /// nearest it, its label on the page. A click before that pass is
    /// taken first, as the original's read finds the button down with
    /// the mouse (its handler before that read's pass, 007F).
    pub fn moved(&mut self, x: i32, y: i32) {
        let at = (x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
        if self.looping() {
            self.at = at;
        }
    }

    /// Its box clicked away: the page the box opened over put by (the
    /// screen the page again, as the rest of its pass left it).
    pub fn box_read(&mut self) {
        self.box_page = None;
    }

    /// The loop's passes go on between inputs (the label redrawn where
    /// the cursor moved, the destination's marker three passes in four):
    /// a pass for each of the map's ticks since the last (`advance`'s
    /// pace).
    pub fn idle(&mut self, engine: &mut Engine) {
        let ticks = self.up_ms / TICK_MS;
        if !self.looping() {
            self.idle_ticks = ticks;
            return;
        }
        if ticks != self.idle_ticks {
            self.idle_ticks = ticks;
            self.pass(engine, false);
        }
    }

    /// The screen's end: the province's window ended first where it is
    /// up, then the world's (`Machine::travel_map_close`); the carry kept
    /// (`done`).
    fn end(&mut self, engine: &mut Engine) {
        if self.shown.is_some() {
            self.province_close(engine);
        }
        let mut h = Self::hooks(&mut self.boxes);
        let carry = engine.m.travel_map_close(&mut h, self.start);
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

    /// Whether a main quest's dungeon's picture is up: kept from the
    /// screen's end (26F7:00CE's 292B:0362), shown once the journey before
    /// it has played (1194:30F1's days and their fades come first, inside
    /// Travel).
    pub fn picture_up(&self) -> bool {
        self.picture.is_some() && !self.journeying()
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
            if self.picture_up() {
                self.picture_click(engine, x, y);
            }
            return;
        }
        if self.done.is_some() {
            return;
        }
        if let Some(Asking::Question(title, entries)) = &self.asking {
            // The right button leaves the menu (3159:10D6), as Esc.
            let pick = match (&self.parch, self.menu_font()) {
                _ if right => Some(0xFFFF),
                (Some(t), Some(f)) => hud::custom::menu_entry(&hud::custom::menu_rects(t, f, title, entries).1, y),
                _ => None,
            };
            if let Some(i) = pick {
                self.asking = None;
                // The menu's reads left the mouse at its click (3159:0F2A),
                // where the loop's passes after find it.
                self.at = (x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
                // 1194:020E's end: its boxes' backgrounds put back and the
                // page dissolved onto the screen (4FF7), where the journey's
                // first fade then starts: the game's page as the menu left
                // it where it is the heap's block (`Hooks::question`), else
                // the map as the menu found it.
                self.answered_over = self.page(engine, self.at);
                self.travel_answered(engine, Some(i as u16));
            }
            return;
        }
        if let Some(Asking::List(menu, typed)) = &mut self.asking {
            // The right button leaves the list, none picked (its record's
            // 4000h, 3159:096D): the search ends there (26F7:0B00, 03A7).
            // The arrows, the track and the lines as 3159:0834 takes them
            // (`hud::list_click`): a line clicked is marked, the marked
            // line clicked picked (5734).
            let pick = match self.list_live.as_ref() {
                // The click as the loop reads it, down and up: the loop's
                // mark and first line shown, and its end the pick.
                Some(live) => {
                    let (cx, cy) = (x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
                    let b = if right { 2 } else { 1 };
                    self.list_reads.push(arena::listwindow::ListRead { buttons: b, x: cx, y: cy, keys: 0, codes: [0; 2] });
                    self.list_reads.push(arena::listwindow::ListRead { buttons: 0, x: cx, y: cy, keys: 0, codes: [0; 2] });
                    let (mark, top, end) = live.live(&self.list_reads);
                    menu.selected = if mark == 0xFFFF { usize::MAX } else { mark as usize };
                    menu.top = Some(top as usize);
                    end
                }
                None if right => Some(0xFFFF),
                None => hud::list_click(menu, x, y).map(|i| i.min(menu.entries.len().saturating_sub(1)) as u16),
            };
            if let Some(pick) = pick {
                let typed = typed.clone();
                self.asking = None;
                self.search(engine, typed, Some(pick));
            }
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
        let before = self.at;
        self.at = (x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
        // 3159:019A: the region under the cursor (its mask's pixel), and
        // 00F3: the right button as Esc (the world map's Exit's key, 1Bh;
        // a province's regions have none), the left a region's handler.
        let Some(_) = self.shown else {
            let region = engine.m.travel_world_region(self.at.0, self.at.1);
            match (right, region) {
                (true, _) | (false, Some(9)) => self.end(engine),
                (false, Some(p)) => self.show_province(engine, archive, p as u8),
                (false, None) => {}
            }
            return;
        };
        if right {
            // Nothing has the key; the pass after, the button let go.
            self.pass(engine, false);
            return;
        }
        match engine.m.travel_province_region(self.at.0, self.at.1) {
            MapRegion::Exit => self.exit(engine),
            MapRegion::Travel => self.travel(engine, before),
            MapRegion::Search => self.ask_name(engine),
            // 26F7:0625 with the button down (AFF9).
            MapRegion::Map => self.pass(engine, true),
        }
    }

    /// The font the question's menu is drawn in (ARENAFNT.DAT, the map's
    /// [B036]).
    fn menu_font(&self) -> Option<&Font> {
        self.fonts.iter().find(|(g, _)| *g == hud::GameFont::Arena).map(|(_, f)| f)
    }

    /// The province whose map is shown; None: the world map.
    pub fn shown(&self) -> Option<u8> {
        self.shown
    }

    /// `shown` while the screen is still up (not ended).
    pub fn shown_now(&self) -> Option<Option<u8>> {
        self.done.is_none().then_some(self.shown)
    }

    /// Exit (3159:0006): the province's map, after its pass, to the world
    /// map; the world map ends the screen.
    fn exit(&mut self, engine: &mut Engine) {
        if self.shown.is_some() {
            engine.m.window_close();
            self.pass(engine, false);
            self.province_close(engine);
        } else {
            self.end(engine);
        }
    }

    /// Travel (26F7:07CD); once a journey is made the screen ends (0236).
    /// Unwell, it asks first (1194:48DB, run on a copy to see): the
    /// question's menu is shown, and its answer travels or stays
    /// (`travel_answered`).
    /// Travel clicked, the loop's last pass having found the cursor at
    /// `before`: the button's handler runs before the pass that read finds
    /// (007F), so a box it shows covers the page as that pass left it.
    fn travel(&mut self, engine: &mut Engine, before: (u16, u16)) {
        let mut copy = engine.m.clone();
        let mut scratch = Vec::new();
        let mut h = Self::hooks(&mut scratch);
        let _ = copy.travel_go(&mut engine.files.clone(), &mut h);
        if h.asked {
            let m = &engine.m;
            let title = hud::lines(m.mem.cstr(DS, QUESTION), 0x0F);
            let mut entries = Vec::new();
            for i in 0..2u16 {
                let at = m.mem.r16(DS, QUESTION_ENTRIES + 2 * i);
                entries.extend(hud::lines(m.mem.cstr(DS, at), 0x0F));
            }
            self.asking = Some(Asking::Question(title, entries));
            return;
        }
        let screen = self.page(engine, before);
        let shown = self.boxes.len();
        self.travel_answered(engine, None);
        // "Select a destination" (DATA:5024): over the page the box
        // opened on till it is clicked away (the handler's pass after).
        if self.boxes.len() > shown && self.journey.is_empty() && self.done.is_none() {
            self.box_page = screen;
        }
    }

    /// The journey made, the question's answer `answer` (0 Yes, 1 No,
    /// FFFFh none) given if asked.
    fn travel_answered(&mut self, engine: &mut Engine, answer: Option<u16>) {
        let mut h = Self::hooks(&mut self.boxes);
        h.answer = answer;
        let ok = engine.m.travel_go(&mut engine.files, &mut h);
        let failed = h.failed;
        if let Some(page) = h.menu_page.take() {
            self.answered_over = Some(page);
        }
        self.journey = std::mem::take(&mut h.frames);
        self.journey_ms = 0;
        self.journey_lead.clear();
        self.journey_tail.clear();
        self.journeyed |= !self.journey.is_empty();
        self.journey_page = h.journey_page.take();
        if ok.is_none() || failed {
            eprintln!("the journey gave up: {}", arena::giveup::describe());
        }
        // The handler's pass after it (the button let go, 3159:0432).
        self.pass(engine, false);
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
            if self.journey_ms >= self.journey_lead_ms() + self.journey.len() as u32 * TICK_MS + self.journey_tail.len() as u32 * crate::film::RETRACE_MS {
                self.journey.clear();
                self.journey_lead.clear();
                self.journey_tail.clear();
            }
        }
    }

    /// The journey's frames left unplayed (the scripted shots).
    pub fn skip_journey(&mut self) {
        self.journey.clear();
        self.journey_lead.clear();
        self.journey_tail.clear();
    }

    /// The fades about a journey just made (`journey_lead`, `journey_tail`),
    /// from `before`, the screen as the Travel click found it: none where
    /// there is no journey, or the world map's picture or the page isn't
    /// the view's.
    pub fn journey_from(&mut self, before: &Screen) {
        use crate::film::Film;
        let (Some(page), Some((_, world)), Some(&last)) = (&self.journey_page, &self.world, self.journey.last()) else { return };
        if !self.journey_lead.is_empty() {
            return;
        }
        let black = [[0, 0, 0, 0xFF]; 256];
        let from = self.answered_over.take().unwrap_or_else(|| before.indices().iter().map(|&c| c as u8).collect());
        let mut film = Film::new(from, *before.palette());
        film.fade(&black, 8);
        film.vga.fill(0);
        film.fade(world, 12);
        self.journey_lead = film.frames;
        let mut screen = Screen::new(*world);
        screen.underlay(page);
        if let Some(f) = self.horse.get(last as usize) {
            screen.blit(f.x, f.y, f.w, f.h, &f.px, 199);
        }
        let mut film = Film::new(screen.indices().iter().map(|&c| c as u8).collect(), *world);
        film.fade(&black, 8);
        self.journey_tail = film.frames;
    }

    /// The lead's time (ms).
    fn journey_lead_ms(&self) -> u32 {
        self.journey_lead.len() as u32 * crate::film::RETRACE_MS
    }

    /// The lead's or the tail's frame for the time played, if in either.
    fn journey_fade(&self) -> Option<&(Vec<u8>, crate::film::Dac)> {
        let lead = self.journey_lead_ms();
        if self.journey_ms < lead {
            return self.journey_lead.get((self.journey_ms / crate::film::RETRACE_MS) as usize);
        }
        let days = lead + self.journey.len() as u32 * TICK_MS;
        if self.journey_ms >= days {
            return self.journey_tail.get(((self.journey_ms - days) / crate::film::RETRACE_MS) as usize);
        }
        None
    }

    /// The journey as the screen shows it, a retrace each before the days
    /// and after them, the days between (`Travel::journey_seen`): its
    /// pixels and 6-bit colours.
    #[cfg(test)]
    pub fn journey_fades(&self) -> (Shown, Shown) {
        let six = |f: &(Vec<u8>, crate::film::Dac)| (f.0.clone(), f.1.iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect());
        (self.journey_lead.iter().map(six).collect(), self.journey_tail.iter().map(six).collect())
    }

    /// The journey's frames playing.
    pub fn journeying(&self) -> bool {
        !self.journey.is_empty()
    }

    /// Each day of the journey as the screen shows it (its pixels and its
    /// 6-bit colours), the frames played or not.
    #[cfg(test)]
    pub fn journey_seen(&mut self, font: &Font, m: &mut Machine) -> Vec<(Vec<u8>, Vec<u8>)> {
        let ms = self.journey_ms;
        let mut seen = Vec::new();
        for day in 0..self.journey.len() as u32 {
            self.journey_ms = self.journey_lead_ms() + day * TICK_MS;
            let mut screen = Screen::new([[0, 0, 0, 0xFF]; 256]);
            self.draw(&mut screen, font, m, 0, 0);
            let px = screen.indices().iter().map(|&c| c as u8).collect();
            let dac = screen.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
            seen.push((px, dac));
        }
        self.journey_ms = ms;
        seen
    }

    /// The journey's frame for the time played (HORSE.DFA's).
    fn journey_frame(&self) -> Option<&Image> {
        let day = self.journey_ms.checked_sub(self.journey_lead_ms())? / TICK_MS;
        self.journey.get(day as usize).and_then(|&f| self.horse.get(f as usize))
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
    /// (FFFFh: none, the list left), None: not yet asked. Run on a copy
    /// first: if the list is asked and not yet answered, it is shown
    /// instead.
    fn search(&mut self, engine: &mut Engine, typed: Vec<EditKey>, pick: Option<u16>) {
        let mut copy = engine.m.clone();
        let mut scratch = Vec::new();
        let mut h = Self::hooks(&mut scratch);
        h.typed = typed.clone();
        h.pick = 0xFFFF;
        h.capture = pick.is_none();
        let _ = copy.travel_search(&mut engine.files.clone(), &mut h);
        if pick.is_none() {
            self.list_live = copy.list_captured.take();
            self.list_reads.clear();
        }
        if let (Some(names), None) = (h.listed.take(), pick) {
            let entries = names.iter().map(|n| hud::lines(n.as_bytes(), 0x0F).into_iter().flatten().collect()).collect();
            self.asking = Some(Asking::List(hud::list_menu(&copy, Vec::new(), entries), typed));
            return;
        }
        let screen = self.page(engine, self.at);
        let shown = self.boxes.len();
        let mut h = Self::hooks(&mut self.boxes);
        h.typed = typed;
        h.pick = pick.unwrap_or(0xFFFF);
        h.screen = screen;
        h.reads = self.list_live.take().map(|_| std::mem::take(&mut self.list_reads));
        if engine.m.travel_search(&mut engine.files, &mut h).is_none() {
            eprintln!("the search gave up: {}", arena::giveup::describe());
        }
        let boxed = h.screen.take();
        let boxed = boxed.filter(|_| self.boxes.len() > shown);
        // The handler's pass after it (once its box, if any, is clicked
        // away: the screen till then the page the box opened over).
        self.pass(engine, false);
        if boxed.is_some() {
            self.box_page = boxed;
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

    /// Whether the search's list of places is up.
    #[cfg(test)]
    pub fn listing(&self) -> bool {
        matches!(self.asking, Some(Asking::List(..)))
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
                self.search(engine, typed, None);
            }
        }
    }

    /// A key: on the world map Esc as Exit (its region's key, 1Bh,
    /// 3159:0171); a province's map's regions and buttons have none, so
    /// no key does anything there. A list's keys as 3159:06A0 takes them
    /// (`hud::list_key`; Enter picks the line marked, Esc leaves).
    pub fn key(&mut self, engine: &mut Engine, code: winit::keyboard::KeyCode, _text: Option<&str>, at: (i32, i32)) {
        use winit::keyboard::KeyCode;
        if self.done.is_some() {
            return;
        }
        match &mut self.asking {
            Some(Asking::Name(_)) => {}
            Some(Asking::List(menu, typed)) if self.list_live.is_some() => {
                // The key as the loop reads it, held and let go (the cursor
                // where it is): the loop's mark and first line shown, and
                // its end the pick.
                use arena::listwindow::list_keys::*;
                let bit = match code {
                    KeyCode::ArrowUp => UP.0,
                    KeyCode::ArrowDown => DOWN.0,
                    KeyCode::PageUp => PGUP.0,
                    KeyCode::PageDown => PGDN.0,
                    KeyCode::Home => HOME.0,
                    KeyCode::End => END.0,
                    KeyCode::Enter | KeyCode::NumpadEnter => ENTER.0,
                    KeyCode::Escape => ESC.0,
                    _ => return,
                };
                let (cx, cy) = (at.0.clamp(0, 319) as u16, at.1.clamp(0, 199) as u16);
                self.list_reads.push(arena::listwindow::ListRead { buttons: 0, x: cx, y: cy, keys: bit, codes: [0; 2] });
                self.list_reads.push(arena::listwindow::ListRead { buttons: 0, x: cx, y: cy, keys: 0, codes: [0; 2] });
                let Some(live) = self.list_live.as_ref() else { return };
                let (mark, top, end) = live.live(&self.list_reads);
                menu.selected = if mark == 0xFFFF { usize::MAX } else { mark as usize };
                menu.top = Some(top as usize);
                if let Some(pick) = end {
                    let typed = typed.clone();
                    self.asking = None;
                    self.search(engine, typed, Some(pick));
                }
            }
            Some(Asking::List(menu, typed)) => {
                let key = match code {
                    KeyCode::ArrowUp => hud::ListKey::Up,
                    KeyCode::ArrowDown => hud::ListKey::Down,
                    KeyCode::PageUp => hud::ListKey::PageUp,
                    KeyCode::PageDown => hud::ListKey::PageDown,
                    KeyCode::Home => hud::ListKey::Home,
                    KeyCode::End => hud::ListKey::End,
                    // Enter (ACD1) picks the line marked; Esc leaves, none
                    // picked, and the search ends there (26F7:0B00).
                    KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Escape => {
                        let pick = if code == KeyCode::Escape { 0xFFFF } else { menu.selected.min(menu.entries.len().saturating_sub(1)) as u16 };
                        let typed = typed.clone();
                        self.asking = None;
                        self.search(engine, typed, Some(pick));
                        return;
                    }
                    _ => return,
                };
                hud::list_key(menu, key);
            }
            // Its keys (4B60: Y, N); Esc none.
            Some(Asking::Question(..)) => {
                let answer = match code {
                    KeyCode::KeyY => 0,
                    KeyCode::KeyN => 1,
                    KeyCode::Escape => 0xFFFF,
                    _ => return,
                };
                self.asking = None;
                self.travel_answered(engine, Some(answer));
            }
            None => {
                if code == KeyCode::Escape && self.shown.is_none() {
                    // The key's region (B7C6 9, 3159:0171), its handler
                    // (0006: the loop ended), then the pass's region under
                    // the cursor (019A) once more before the loop's test.
                    self.at = (at.0.clamp(0, 319) as u16, at.1.clamp(0, 199) as u16);
                    engine.m.mem.w16(DS, 0xB7C6, 9);
                    engine.m.travel_world_region(self.at.0, self.at.1);
                    self.end(engine);
                }
            }
        }
    }

    /// A message box over the map (1194:59C8: 965A with margin 5, centred
    /// on the screen) in its tiles (`parch`) and its text in EEh, the
    /// colour 26F7:0091 sets ([9288]).
    /// Where the game's page isn't the heap's block: the buttons, and the
    /// icons centred on their places (26F7:0268) as the view draws them.
    fn icons_drawn(&self, screen: &mut Screen, m: &Machine, p: u8) {
        let blit = |screen: &mut Screen, i: &Image, x: i32, y: i32| screen.blit(x, y, i.w, i.h, &i.px, 199);
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
    }

    pub fn draw_box(&self, screen: &mut Screen, font: &Font, lines: &[Vec<hud::Span>]) {
        if let Some(t) = &self.parch {
            hud::custom::draw_box(screen, t, font, lines, 5, 0xEE);
        }
    }

    /// The screen, the cursor at (x, y): its palette set.
    pub fn draw(&self, screen: &mut Screen, font: &Font, m: &mut Machine, x: i32, y: i32) {
        let blit = |screen: &mut Screen, i: &Image, x: i32, y: i32| screen.blit(x, y, i.w, i.h, &i.px, 199);
        if let Some(p) = self.picture.as_ref().filter(|_| self.picture_up()) {
            p.draw(screen, &self.fonts);
            return;
        }
        // The fades before the days and after them (`journey_from`).
        if let Some((vga, dac)) = self.journey_fade().filter(|_| !self.journey.is_empty()) {
            screen.set_palette(*dac);
            screen.underlay(vga);
            return;
        }
        // 1194:30F1: the day's frame of the journey over the world map's
        // picture (26F7:07CD's 56C8 with BX 24h: its colours), as the
        // game's page held it after the journey (`journey_page`).
        if let (Some(f), Some(page)) = (self.journey_frame(), &self.journey_page) {
            if let Some((_, pal)) = &self.world {
                screen.set_palette(*pal);
            }
            screen.underlay(page);
            screen.blit(f.x, f.y, f.w, f.h, &f.px, 199);
            return;
        }
        // The game's page where it is the heap's block: the window's
        // picture (3159:057E), the current province outlined (26F7:013E),
        // or the province's picture and its places' icons as each pass
        // copies them back from bank 4 (26F7:062E).
        let page = m.page_is_block().then(|| m.mem.segment(m.mem.r16(DS, 0xA900))[..64000].to_vec());
        let Some(p) = self.shown else {
            if let Some((img, pal)) = &self.world {
                screen.set_palette(*pal);
                if let Some(page) = &page {
                    screen.underlay(page);
                } else {
                    screen.fill(0, 0, 320, 200, 0, 0xFF);
                    blit(screen, img, img.x, img.y);
                }
            }
            if page.is_none() {
                if let Some(f) = self.outprov.get(m.mem.r8(DS, 0x0A46) as usize) {
                    blit(screen, f, f.x, f.y);
                }
            }
            return;
        };
        // Each pass of the province's loop (3159:0078..0097, `pass`) puts
        // bank 4's copy back on the page and draws on it the current
        // place's marker, the label and the destination's (26F7:0625):
        // the page as the last pass left it is the screen.
        if let Some(page) = &page {
            if let Some((_, pal)) = &self.province {
                screen.set_palette(*pal);
            }
            screen.underlay(self.box_page.as_ref().unwrap_or(page));
            self.asked_drawn(screen, font, m);
            return;
        }
        if let Some((img, pal)) = &self.province {
            screen.set_palette(*pal);
            screen.fill(0, 0, 320, 200, 0, 0xFF);
            blit(screen, img, img.x, img.y);
        }
        self.icons_drawn(screen, m, p);
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
        // The search's state is put back after (where the page isn't the
        // game's, the label the view's own): the pass runs wherever the
        // cursor is, over the buttons too.
        if !self.label_held && self.asking.is_none() {
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
        self.asked_drawn(screen, font, m);
    }

    /// What the screen asks, over the map as its last pass left it; and
    /// the journey's day where the game's page isn't there to show.
    fn asked_drawn(&self, screen: &mut Screen, font: &Font, m: &mut Machine) {
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
            // 1194:020E's boxes in the map's tiles and colour.
            Some(Asking::Question(title, entries)) => {
                if let (Some(t), Some(f)) = (&self.parch, self.menu_font()) {
                    hud::custom::draw_menu(screen, t, f, title, entries, 0xEE);
                }
            }
            None => {}
        }
        // 1194:30F1: the day's frame of the journey (over the province's
        // map where the game's page isn't there to show).
        if let Some(f) = self.journey_frame() {
            screen.blit(f.x, f.y, f.w, f.h, &f.px, 199);
        }
    }
}

/// After an input to the travel map (or a frame): its boxes to `boxes`;
/// ended and the journey's frames played, it goes, its carry returned:
/// opened by the map `button`, the area left on the carry and the world
/// given back (`Machine::map_button_after`, 2542:0411); else
/// (`Travel::forced`) the caller's.
pub fn after(travel: &mut Option<Travel>, engine: &mut Engine, boxes: &mut Vec<Lines>, button: bool) -> Option<bool> {
    let t = travel.as_mut()?;
    boxes.extend(t.take_boxes());
    let carry = t.done.filter(|_| !t.journeying() && t.picture.is_none())?;
    if button && engine.m.map_button_after(&mut engine.files, carry).is_none() {
        eprintln!("the world given back gave up: {}", arena::giveup::describe());
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
