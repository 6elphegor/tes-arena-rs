//! The main quest walked through its first steps, by the game's own
//! entries, printing its state (the block at 0FDD) at each:
//!
//! 1. a town (as a new game leaves the start dungeon into it, the main
//!    quest's block as a new game has it: 0FDD and 0FE4 FFh, the rest 0)
//!    entered (`city_entered`, 0000:46B2: its lists, the taverns' at
//!    1AB5);
//! 2. its townspeople talked to, standing beside each (`actor_click`,
//!    1194:DB6C), the talk's Rumors and their first kind asked: a roll
//!    within (level + 1) x 2 offers the next step (1194:E4A3, CD9A), its
//!    text shown;
//! 3. the tavern the offer names (0FDE: its place word) entered and its
//!    keeper talked to (`tavern_shop`, 1194:B93C, whose talk runs the
//!    informant, CED7): the informant's deal (haggled, mode 5) tells the
//!    piece's dungeon (CFAF: 0FE2/0FE3), its CITYDATA entry made known
//!    (26F7:0C0A);
//! 4. that dungeon clicked on the travel map (26F7:0625, the nearest
//!    known one), travelled to (07CD) and entered by the outer loop (the
//!    view's `areas::next`; the hook CD42's indoor part, 0000:4767);
//! 5. a camp there until healed (2433:12D6): the dream (1194:D340, D3AF);
//! 6. the stairs down to the last level (0000:51B8, EBB5, 44D7, where
//!    2294:0E90 put the chest at 0FEE) and the chest clicked (292B:06DC,
//!    2294:0F33): the next dungeon named (%adn) and made known (D140);
//! 7. that dungeon travelled to, its last level's chest opened: the piece
//!    given (1194:D198), the block cleared but for the steps done (0FE7,
//!    whose bit the offer set).
//!
//! The level is reached the game's way: experience given as the view's
//! `xp` step gives it and the level-up (2686:02D8) run until the level
//! byte is LEVEL. With Firewatch (CITY:3:3) at level 9 the third talk
//! round offers Rajhin's ring (step 0Fh) and the informant at the
//! Haunted Wharf sells the way for 950 gold: the Fortress of Grumotr
//! (Black Marsh), then the Fortress of Yenovor (Skyrim).
//!
//! SAVE:DIR[:SLOT[:P:I]] starts from a saved game instead (a copy: the
//! view's loads only read it, but keep the original safe), first
//! dropping a piece held (1194:18F6; CD9A offers nothing while one is,
//! 1194:CDEA) and travelling to city P:I (default 0:0) for its streets;
//! after step 7 it travels to the Imperial City and uses the palace's
//! door (0000:4192): by the stage at F77, 8 the Emperor's audience, 9
//! the artifact's (text 5DDh).
//!
//! Usage: mainquest GAME_DIR [CITY:P:I | SAVE:DIR[:SLOT[:P:I]]] [LEVEL]
//! LEVEL is the player's level byte (01AA) for the rumours' roll.
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
#[path = "../src/areas.rs"]
mod areas;
#[path = "mainquest/staff.rs"]
mod staff;
use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mem::DS;
use arena::transit::Run;

/// The main quest's state.
fn state(m: &Machine) -> String {
    let block: Vec<String> = (0..0x13u16).map(|i| format!("{:02x}", m.mem.r8(DS, 0x0FDD + i))).collect();
    format!(
        "step {:02x}, block [{}], done {:04x}, flags {:02x}, gold {}",
        m.mem.r8(DS, 0x0FDD),
        block.join(" "),
        m.mem.r16(DS, 0x0FE7),
        m.mem.r8(DS, 0x0FE6),
        m.mem.r32(DS, 0x05C2),
    )
}

/// Answers the menus from a queue (then leaves them), prints the texts.
struct Talk {
    picks: Vec<u16>,
}

impl Talk {
    fn next(&mut self) -> u16 {
        if self.picks.is_empty() { 0xFFFF } else { self.picks.remove(0) }
    }
}

fn show(m: &Machine, at: u16) -> String {
    String::from_utf8_lossy(m.mem.cstr(DS, at)).replace('\r', " ")
}

impl arena::text::TextHooks for Talk {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn menu(&mut self, m: &mut Machine, si: u16, _bx: u16, _di: u16) -> u16 {
        let p = self.next();
        println!("    [menu] {} -> {}", show(m, si).trim(), p as i16);
        p
    }
}

impl arena::talk::TalkHooks for Talk {
    fn way(&mut self, m: &mut Machine, si: u16, x: u16, y: u16) {
        println!("    [the way to {} at ({x}, {y})]", show(m, si));
    }
}

impl arena::shops::ShopHooks for Talk {
    fn message(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn menu(&mut self, m: &mut Machine, si: u16, _bx: u16, _di: u16) -> u16 {
        let p = self.next();
        println!("    [menu] {} -> {}", show(m, si).trim(), p as i16);
        p
    }
}
impl arena::tavern::TavernHooks for Talk {
    /// 1194:CED7, the informant, his price accepted.
    fn quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        let r = m.quest_informant(files, &mut Quests);
        println!("    [informant] {}", match r {
            Some(true) => "not the informant here",
            Some(false) => "the informant talked to",
            None => "met something not ported",
        });
        r.map(|_| ())
    }
}

/// The haggle: the first price accepted, the shopkeeper's lines printed.
struct Accept;

impl arena::haggle::HaggleIo for Accept {
    fn menu(&mut self) -> Option<u16> {
        Some(0)
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn key(&mut self) -> Option<()> {
        Some(())
    }
    fn say(&mut self, m: &mut Machine, si: u16, _kind: u8) {
        println!("    [haggle] {}", show(m, si));
    }
}

/// The quests' hooks: the informant's haggle accepted.
struct Quests;

impl arena::quest::QuestHooks for Quests {
    fn informant_haggle(&mut self, m: &mut Machine, files: &mut Files, price: u32, per: u16, mode: u8) -> Option<bool> {
        let r = m.haggle(files, &mut Accept, price, per, 0x1E, 0x28, 0, mode, 0)?;
        println!("    [informant's haggle, mode {mode}, price {price}] {r:?}");
        Some(r == arena::haggle::Haggle::NoDeal)
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let spec = a.get(2).cloned().unwrap_or_else(|| "CITY:3:3".into());
    let level: u8 = a.get(3).map_or(9, |s| s.parse().expect("LEVEL"));
    if let Some(save) = spec.strip_prefix("STAFF:") {
        staff::walk(&dir, save, level);
        return;
    }
    let (mut engine, mut run) = match spec.strip_prefix("SAVE:") {
        Some(save) => match from_save(&dir, save) {
            Some(e) => e,
            None => return,
        },
        None => match new_game(&dir, &spec, level) {
            Some(e) => e,
            None => return,
        },
    };
    let m = &engine.m;
    println!("   taverns {}", m.mem.r16(DS, 0x1387));
    // 2. The townspeople's rumours until the next step is offered.
    let mut offered = false;
    'talk: for round in 0..40 {
        // New townspeople each round, as walking the streets meets them
        // (0000:1793; the rumours' roll is seeded per person, 0151).
        world::city_people(&mut engine);
        engine.m.actors_update(&mut engine.files);
        for slot in 0..15u16 {
            let si = 0x0AB3 + slot * 0x1C;
            if engine.m.mem.r16(DS, si + 0xF) & 0x4000 != 0 {
                continue;
            }
            // Stand beside them.
            let (x, y) = (engine.m.mem.r16(DS, si), engine.m.mem.r16(DS, si + 4));
            engine.m.mem.w16(DS, arena::movement::var::X, x.wrapping_add(0x40));
            engine.m.mem.w16(DS, arena::movement::var::Y, y);
            // Time passes between talks (the timer at 0151 seeds the
            // rumours' roll).
            engine.tick(37);
            engine.m.mem.w16(DS, arena::talk::var::CLICKED, si);
            let mut t = Talk { picks: vec![2, 0, 0xFFFF] };
            let before = engine.m.mem.r8(DS, 0x0FDD);
            if engine.m.actor_click(&mut engine.files, &mut t).is_none() {
                println!("   round {round} slot {slot}: the talk met something not ported");
                continue;
            }
            if engine.m.mem.r8(DS, 0x0FDD) != before {
                println!("2. offered by townsperson {slot} (round {round}): {}", state(&engine.m));
                offered = true;
                break 'talk;
            }
        }
    }
    if !offered {
        println!("2. no townsperson offered the next step");
        return;
    }
    walk_on(&mut engine, &dir, &mut run);
    if spec.starts_with("SAVE:") {
        palace(&mut engine, &dir, run);
    }
}

/// The palace's hooks: boxes and the audience's text printed.
struct Palace;

impl arena::trigger::TriggerHooks for Palace {
    fn message_box(&mut self, m: &mut Machine, text: u16) {
        println!("    [box] {}", show(m, text));
    }
    /// 1194:A36C (AX 0): the text TEXT (137A) shown.
    fn artifact_audience(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        let n = m.mem.r16(DS, 0x137A);
        println!("    [the artifact's audience, text {n:#x}]");
        let Some((seg, off)) = m.heap_alloc_bytes(0xBB8) else {
            println!("    no heap for its text");
            return None;
        };
        // 5DDh is TEMPLATE.DAT's last text, which no `#` ends: its read
        // (1194:A374) runs on past the end of the file into what the text
        // bank holds: the town's welcome box's background (`Welcome`).
        if m.text(files, &mut arena::text::NoTextHooks, 2, seg, off).is_none() {
            println!("    its text met something not ported");
            return Some(());
        }
        println!("    {}", String::from_utf8_lossy(m.mem.cstr(seg, off)).replace('\r', " "));
        Some(())
    }
}

/// The world's frames after arriving in a town, until its welcome
/// (0000:4ACD, the fourth frame on): ENTERCTY.$$$ printed, and what its
/// box covers kept in the load bank's frame as the original keeps it
/// (`Machine::box_background`), black here. The box as 965A sizes it
/// (98A0, 9798) in ARENAFNT.DAT with MARBLE.CIF's 3 x 3 tiles, centred
/// in the clip.
struct Welcome {
    widths: Vec<u8>,
    height: u16,
    shown: bool,
}

impl arena::hooks::StairsHooks for Welcome {}
impl arena::controls::ControlHooks for Welcome {}
impl arena::weapon::WeaponHooks for Welcome {}
impl arena::clock::ClockHooks for Welcome {}
impl arena::engine::EngineHooks for Welcome {}
impl arena::frame::FrameHooks for Welcome {
    fn town_welcome(&mut self, m: &mut Machine, files: &mut Files) {
        if m.file_load(files, 0x130C, DS, 0x1044).is_none() {
            return;
        }
        self.shown = true;
        let text = m.mem.cstr(DS, 0x1044).to_vec();
        println!("   [welcome] {}", String::from_utf8_lossy(&text).replace('\r', " / "));
        let glyph = |c: u8| match c {
            0x21..=0x7E => self.widths[(c - 0x21) as usize] as u16,
            _ => self.widths[0] as u16 + 1,
        };
        let lines: Vec<&[u8]> = text.split(|&c| c == b'\r').collect();
        let tw = lines.iter().map(|l| l.iter().map(|&c| glyph(c)).sum::<u16>()).max().unwrap_or(0);
        let (w, h) = (tw + 10, lines.len() as u16 * (self.height + 1) + 10);
        let (w, h) = (w + (3 - w % 3) % 3, h + (3 - h % 3) % 3);
        let r = |at| m.mem.r16(DS, at);
        let x = (r(0xA918).wrapping_add(r(0xA914)) >> 1).wrapping_sub(w >> 1);
        let y = (r(0xA91A).wrapping_add(r(0xA916)) >> 1).wrapping_sub(h >> 1);
        let frame = r(0x9030);
        m.box_background(frame, 0, (x, y, w, h), None);
    }
}

fn welcome(engine: &mut Engine, dir: &std::path::Path) {
    let font = archive::Archive::open(dir).read("ARENAFNT.DAT").unwrap_or_default();
    if font.len() < 95 {
        println!("   no ARENAFNT.DAT");
        return;
    }
    let mut h = Welcome { widths: font[1..95].to_vec(), height: font[0] as u16, shown: false };
    for _ in 0..8 {
        let t = engine.frame_ticks();
        engine.tick(t);
        if engine.frame(arena::engine::Input::Steer(&Default::default()), &mut h).is_none() {
            println!("   a frame met code not ported");
            return;
        }
        if h.shown {
            return;
        }
    }
    println!("   no welcome");
}

/// Step 8: the Imperial City (province 8) travelled to and its palace's door
/// used (0000:43B1: a wall whose trigger is the city table's 11..13,
/// 0000:4192): by the main quest's stage (F77) the audience (8), the
/// artifact's (9) or "no time".
fn palace(engine: &mut Engine, dir: &std::path::Path, run: Run) {
    let Some(_) = trip(engine, dir, run, 8, 0, "8") else { return };
    welcome(engine, dir);
    let m = &mut engine.m;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (rows, cols) = (m.mem.r16(DS, 0xC359), m.mem.r16(DS, 0xC357));
    let trigger = |m: &Machine, sq: u16| {
        let w = m.mem.r16(walls, sq);
        if (w as i16) >= 0 && (w >> 8) as u8 != w as u8 {
            return None;
        }
        (0..0x10u16).find(|&i| m.mem.r8(DS, arena::movement::var::TRIGGERS + i) == w as u8 & 0x3F)
    };
    let door = (0..rows).flat_map(|r| (0..cols).map(move |c| r << 8 | c << 1)).find(|&sq| matches!(trigger(m, sq), Some(11..=13)));
    let Some(sq) = door else {
        println!("   no palace door found");
        return;
    };
    m.mem.w16(DS, arena::controls::var::PICKED, sq);
    m.mem.w16(DS, arena::movement::var::X, ((sq & 0xFF) >> 1) << 7 | 0x40);
    m.mem.w16(DS, arena::movement::var::Y, (sq >> 8) << 7 | 0x40);
    println!("   the palace's door at {sq:04x} used, stage (F77) {}", m.mem.r8(DS, 0x0F77));
    if m.wall_trigger(&mut engine.files, &mut Palace).is_none() {
        println!("   the door met something not ported");
    }
}

/// A town as a new game leaves the start dungeon into it, the player
/// raised to `level` the game's way.
fn new_game(dir: &std::path::Path, spec: &str, level: u8) -> Option<(Engine, Run)> {
    let mut engine = world::load(dir, spec);
    // A frame of the actors: the townspeople take their views (C0h..C4h).
    engine.m.actors_update(&mut engine.files);
    let m = &mut engine.m;
    // The main quest's block as a new game has it, and the player's level.
    for i in 0..0x13u16 {
        m.mem.w8(DS, 0x0FDD + i, 0);
    }
    m.mem.w8(DS, 0x0FDD, 0xFF);
    m.mem.w8(DS, 0x0FE4, 0xFF);
    // The level reached the game's way: a new game's level byte 0, then
    // experience given as the view's `xp` step gives it (the player's
    // +409h, 5CA bit 100h) and the level-up run (2686:02D8), until the
    // level byte is LEVEL.
    m.mem.w8(DS, 0x01AA, 0);
    m.mem.w32(DS, arena::character::PLAYER + 0x409, 0);
    while m.mem.r8(DS, 0x01AA) < level {
        let at = arena::character::PLAYER + 0x409;
        let xp = m.mem.r32(DS, at).wrapping_add(250);
        m.mem.w32(DS, at, xp);
        let f = m.mem.r16(DS, 0x05CA) | 0x100;
        m.mem.w16(DS, 0x05CA, f);
        match m.level_up() {
            Some(Some(n)) => println!("   xp {xp}: {} level(s) up, the level byte {}", n as u8, m.mem.r8(DS, 0x01AA)),
            Some(None) => {}
            None => {
                println!("   the level-up met something not ported");
                return None;
            }
        }
    }
    // A weapon in the first slot, as a new game gives one (1194:26DC):
    // with no slot occupied the offer's search for a piece (1194:CDEA)
    // runs through all of DGROUP and finds one.
    let slot = arena::inventory::PLAYER_SLOTS;
    m.mem.w8(DS, slot, 1);
    let f = m.mem.r8(DS, slot + arena::item::FLAGS) | arena::item::OCCUPIED | 4;
    m.mem.w8(DS, slot + arena::item::FLAGS, f);
    m.mem.w32(DS, 0x05C2, 1000);
    println!("1. {spec}: {}", state(m));
    // The town entered as the outer loop enters it (`areas::start`: the
    // working slot 65, whose CITYDATA.65 the quests write to; 0000:46B2).
    let run = areas::start(&mut engine, true);
    Some((engine, run))
}

/// A saved game (DIR:SLOT, a scratch copy: the view's loads only read
/// it) loaded as the view loads one (`areas::load_game`, 1194:0D9F),
/// then a journey (the travel map) to city P:I (default 0:0) to talk in
/// its streets, waiting for the day if it is night.
fn from_save(dir: &std::path::Path, save: &str) -> Option<(Engine, Run)> {
    let v: Vec<&str> = save.split(':').collect();
    let (sdir, slot) = (std::path::Path::new(v[0]), v.get(1).map_or(1, |s| s.parse().expect("SLOT")));
    let (p, i): (u8, u8) = (v.get(2).map_or(0, |s| s.parse().expect("P")), v.get(3).map_or(0, |s| s.parse().expect("I")));
    let mut engine = world::load(dir, "START.MIF");
    // As the view starts before its --load: an interior begun (`areas::start`).
    areas::start(&mut engine, false);
    let mut run = match areas::load_game(&mut engine, dir, sdir, slot) {
        Ok(r) => r,
        Err(e) => {
            println!("{e}");
            return None;
        }
    };
    println!("1. {} slot {slot}: {run:?}, level byte {}: {}", sdir.display(), engine.m.mem.r8(DS, 0x01AA), state(&engine.m));
    // A piece held (an item's type with bit 80h, 1194:CDEA) refuses the
    // next offer until it is taken to the staff's dungeon.
    // A piece held is dropped as the inventory's Drop drops it (its line
    // selected, 1194:18F6), so the next step can be offered.
    let m = &mut engine.m;
    let held: Vec<u16> = (0..40u16)
        .filter(|k| {
            let s = arena::inventory::PLAYER_SLOTS + k * 0x13;
            m.mem.r8(DS, s + arena::item::FLAGS) & arena::item::OCCUPIED != 0 && m.mem.r8(DS, s) & 0x80 != 0
        })
        .collect();
    let names: Vec<String> = held.iter().map(|k| format!("{:02x}", m.mem.r8(DS, arena::inventory::PLAYER_SLOTS + k * 0x13))).collect();
    println!("   pieces held: [{}]; dream (F76) {}, its count (F77) {}", names.join(" "), m.mem.r8(DS, 0x0F76), m.mem.r8(DS, 0x0F77));
    for &k in held.iter().rev() {
        use arena::use_item::var::{OTHER_LIST, OWN_ITEMS, SELECTED};
        m.mem.w8(DS, OWN_ITEMS, 1);
        m.mem.w8(DS, OTHER_LIST, 0);
        m.mem.w8(DS, SELECTED, k as u8);
        println!("   the piece in slot {k} dropped: {}", m.drop_item());
    }
    run = trip(&mut engine, dir, run, p, i, "1")?;
    for _ in 0..24 {
        let t = engine.m.mem.r16(DS, 0xCCFE);
        if (0x100..0x2C0).contains(&t) {
            break;
        }
        engine.m.advance_minutes(&mut engine.files, 60, &mut arena::clock::NoClockHooks)?;
    }
    Some((engine, run))
}

/// The steps after the offer, with the engine in the town that offered.
fn walk_on(engine: &mut Engine, dir: &std::path::Path, run: &mut Run) {
    let dir = dir.to_path_buf();
    // 3. The informant: 0FDE names the tavern (its word, the talker's as
    // a tavern's keeper is kept at TALKER: kind 4).
    let m = &mut engine.m;
    let g = m.mem.r16(DS, 0x0FDE);
    println!("3. the informant at {g:04x}; the tavern's keeper talked to");
    m.mem.w8(DS, arena::sidequest::var::TALK, 4);
    // (TALKER keeps the place word with its low byte doubled; the
    // informant compares it halved, `sq_talker`.)
    m.mem.w16(DS, arena::sidequest::var::TALKER, g & 0xFF00 | ((g as u8 as u16) << 1) & 0xFF);
    let mut t = Talk { picks: vec![0, 0, 0, 0xFFFF] };
    let r = m.tavern_shop(&mut engine.files, &mut t);
    println!("   {}: {}", if r.is_some() { "done" } else { "met something not ported" }, state(&engine.m));
    if r.is_none() {
        return;
    }
    // 4. The dungeon the informant told of: known now in CITYDATA (the
    // entry 26F7:0C0A wrote back), travelled to and entered.
    use arena::quest::main_quest::{NEXT_PLACE, NEXT_PROVINCE, PLACE, PROVINCE};
    let (p, i) = (engine.m.mem.r8(DS, PROVINCE), engine.m.mem.r8(DS, PLACE));
    let Some(r) = trip(engine, &dir, *run, p, 0x20 + i, "4") else { return };
    *run = r;
    // 5. Rest (camping, as a dungeon allows): the dream (1194:D340, D3AF).
    rest(engine, "5");
    // 6. Down to the last level, the chest at FEE opened (2294:0F33): the
    // next dungeon revealed (FE4/FE5).
    if descend_and_open(engine, "6").is_none() {
        return;
    }
    // 7. The next dungeon: its chest holds the piece (1194:D198).
    let (p, i) = (engine.m.mem.r8(DS, NEXT_PROVINCE), engine.m.mem.r8(DS, NEXT_PLACE));
    if p == 0xFF {
        println!("7. no next dungeon");
        return;
    }
    let Some(r) = trip(engine, &dir, *run, p, 0x20 + i, "7") else { return };
    *run = r;
    let items = |m: &Machine| (0..40u16).filter(|k| m.mem.r8(DS, arena::inventory::PLAYER_SLOTS + k * 0x13 + arena::item::FLAGS) & arena::item::OCCUPIED != 0).count();
    let before = items(&engine.m);
    if descend_and_open(engine, "7").is_none() {
        return;
    }
    println!("   items {before} -> {}; pieces (F77) {}", items(&engine.m), engine.m.mem.r8(DS, 0x0F77));
}

/// The travel map's hooks, as the view's: the destination's record built
/// (1194:3AE8 in full; a dungeon's, 3B0C, with its climate), boxes printed.
struct Trip {
    files: Files,
}

impl arena::clock::ClockHooks for Trip {}

impl arena::travelmap::TravelHooks for Trip {
    fn message_box(&mut self, m: &mut Machine, seg: u16, si: u16) {
        println!("    [box] {}", String::from_utf8_lossy(m.mem.cstr(seg, si)).replace('\r', " "));
    }
    fn location(&mut self, m: &mut Machine, id: u16) {
        let ax = if m.mem.r8(DS, 0x0A46) == 8 { 0x100 } else { id };
        if m.city_location(&mut self.files, DS, 0x0A22, ax).is_none() {
            println!("    the location record could not be built");
        }
    }
    fn dungeon_location(&mut self, m: &mut Machine, id: u16, _bx: u16) {
        let (x, y) = (m.mem.r16(DS, 0x193E), m.mem.r16(DS, 0x1940));
        let p = m.mem.r8(DS, arena::travelmap::var::SHOWN);
        if m.city_climate(&mut self.files, x, y, p).and_then(|c| m.dungeon_build_location(id as u8, c)).is_none() {
            println!("    the dungeon's record could not be built");
        }
    }
}

impl arena::mapscreen::MapHooks for Trip {}

/// Province `p`'s location `i` (a place below 20h, else dungeon i - 20h)
/// looked up in CITYDATA (a dungeon's entry +18h bit 1: known), clicked
/// on the travel map (26F7:0625: the nearest to the cursor, of the
/// dungeons the known ones), travelled to (26F7:07CD) and entered by the
/// outer loop.
fn trip(engine: &mut Engine, dir: &std::path::Path, run: Run, p: u8, i: u8, n: &str) -> Option<Run> {
    use arena::travelmap::var::*;
    let m = &mut engine.m;
    let here = m.mem.r8(DS, HERE_PROVINCE);
    m.city_load_province(&mut engine.files, p)?;
    let e = arena::travelmap::CITIES + i as u16 * 0x19;
    let (name, x, y, f) = (String::from_utf8_lossy(m.mem.cstr(DS, e)).into_owned(), m.mem.r16(DS, e + 0x14), m.mem.r16(DS, e + 0x16), m.mem.r8(DS, e + 0x18));
    let known = if i < 0x20 { "" } else if f & 2 != 0 { ", known" } else { ", not known" };
    println!("{n}. location {i:#x} of province {p}: {name} at ({x}, {y}), flags {f:02x}{known}");
    m.city_load_province(&mut engine.files, here)?;
    let mut h = Trip { files: engine.files.clone() };
    let start = m.travel_map_begin(&mut h);
    m.travel_map_province(&mut engine.files, &mut h, p)?;
    m.mem.w16(DS, 0x8F92, x);
    m.mem.w16(DS, 0x8F94, y);
    m.mem.w8(DS, 0xAFF9, 1);
    m.travel_click(&mut engine.files, &mut h)?;
    m.mem.w8(DS, 0xAFF9, 0);
    println!("   picked {:#x} ({}), chosen {}, shown {}, nearest {:#x} at {}", m.mem.r16(DS, PICK), String::from_utf8_lossy(m.mem.cstr(DS, DEST_NAME)), m.mem.r8(DS, CHOSEN), m.mem.r8(DS, SHOWN), m.mem.r16(DS, NEAREST), m.mem.r16(DS, NEAREST_DIST));
    m.travel_go(&mut engine.files, &mut h)?;
    let carry = m.travel_map_end(&mut h, start);
    m.map_button_done(carry);
    println!("   travelled: here {} {:#x} (kind {}), 5E6 {}, the carry {carry}", m.mem.r8(DS, HERE_PROVINCE), m.mem.r8(DS, HERE_ID), m.mem.r8(DS, HERE_KIND), m.mem.r8(DS, 0x05E6));
    let r = areas::next(engine, dir, run);
    let m = &engine.m;
    println!("   entered {r:?}: level {} of {}, the piece's square (FEE) {:04x}: {}", m.mem.r8(DS, arena::movement::var::LEVEL), m.mem.r8(DS, arena::movement::var::LEVELS), m.mem.r16(DS, 0x0FEE), state(m));
    r
}

/// Resting hooks: the rest until healed picked, the dream's text shown.
struct Rest;

impl arena::clock::ClockHooks for Rest {}

impl arena::rest::RestHooks for Rest {
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        1
    }
    fn message(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn dream(&mut self, _m: &mut Machine, text: u16) {
        println!("    [dream] text {text:#x}");
    }
}

/// Camping (2433:12D6) until healed, wounded first so the rest is needed.
fn rest(engine: &mut Engine, n: &str) {
    let m = &mut engine.m;
    m.mem.w16(DS, 0x01FD, 1);
    let dream = |m: &Machine| format!("dream {} count {} shown {}", m.mem.r8(DS, 0x0F76), m.mem.r8(DS, 0x0F77), m.mem.r8(DS, 0x0F7A));
    println!("{n}. camping at level byte {}: {}", m.mem.r8(DS, 0x01AA), dream(m));
    if m.camp(&mut engine.files, &mut Rest).is_none() {
        println!("   the rest met something not ported");
        return;
    }
    for _ in 0..2000 {
        if m.mem.r8(DS, arena::rest::var::RESTING) == 0 {
            break;
        }
        if m.rest_frame(&mut engine.files, &mut Rest).is_none() {
            println!("   the rest met something not ported");
            return;
        }
    }
    println!("   rested: {}; {}", dream(m), state(m));
}

/// The stairs taken down to the last level as `staff::change_level`
/// takes them (the stairs' hook, the level reloaded, 0000:EBB5, its INF
/// loaded where one was named, the hook C3B8: 0000:44D7, where 2294:0E90
/// puts the chest), then the chest at the piece's square (FEE) clicked
/// (292B:06DC, whose chest runs 2294:0F33).
fn descend_and_open(engine: &mut Engine, n: &str) -> Option<()> {
    use arena::movement::var::{LEVEL, LEVELS, X, Y};
    while engine.m.mem.r8(DS, LEVEL) + 1 < engine.m.mem.r8(DS, LEVELS) {
        let l = engine.m.mem.r8(DS, LEVEL) + 1;
        if staff::change_level(engine, l).is_none() {
            println!("{n}. level {l}: the level's change gave up: {}", arena::giveup::describe());
            return None;
        }
    }
    let m = &mut engine.m;
    let sq = m.mem.r16(DS, 0x0FEE);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    println!("{n}. the last level ({}): the piece's square {sq:04x} holds {:04x} (the chest flat C93B {:02x})", m.mem.r8(DS, LEVEL), m.mem.r16(walls, sq), m.mem.r8(DS, 0xC93B));
    // The square's centre: the row in the high byte, the column doubled.
    let (x, y) = (((sq & 0xFF) >> 1) << 7 | 0x40, (sq >> 8) << 7 | 0x40);
    m.mem.w16(DS, X, x.wrapping_add(0x40));
    m.mem.w16(DS, Y, y);
    let rec = arena::talk::var::RECORD;
    m.mem.w16(DS, rec, x);
    m.mem.w16(DS, rec + 4, y);
    let base = m.mem.r16(DS, 0xC38E);
    let sprite = (m.mem.r8(DS, 0xC93B) as u16).wrapping_add(base) as u8;
    m.mem.w8(DS, rec + 0xC, sprite);
    let mut t = Talk { picks: vec![] };
    let r = m.object_click(&mut engine.files, &mut t, rec);
    println!("   the chest clicked: {r:?}; {}", state(&engine.m));
    r.map(|_| ())
}
