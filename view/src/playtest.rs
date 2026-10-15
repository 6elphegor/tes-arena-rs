//! A headless playtest (`--playtest SEED FRAMES [SETUP]`): the game as the
//! command line starts it, SETUP's steps (`at:COL:ROW:HEADING`, `click`,
//! `wait:N`, comma-separated), then FRAMES turns of a seeded random
//! policy: walking, turning, strafing, clicking in the centre, swinging,
//! camping, a fight summoned now and then, a jump to a random free
//! square, the player healed below a third, the game's questions
//! answered at random, and with `--save-dir` a save and a load back. Each turn runs as the scripted shots run theirs
//! (`shot_frame`: the view's hooks, its dialogs left as Esc leaves them,
//! the areas followed).
//!
//! It reports, one line each: a turn that ends in None (the port gives
//! up: something not ported, or where the original faults), a panic, a
//! turn slower than a second, and odd states (the player off the map,
//! its height out of range). The run stops at the first None or panic,
//! the state then being the port's guess. The last line sums it up:
//! `PLAYTEST seed .. frames .. result ..`. A None that a port routine
//! marks as the original's own fault (`arena::giveup::original`, or
//! `Machine::original_fault`) prints `ORIGFAULT frame ..: why` instead;
//! with `--save-dir` and the targeted acts on, the run then reloads its
//! save and goes on. One marked as not ported or as an error prints
//! `NOTPORTED` or `ERROR` with its why and stops; an unmarked one `NONE`.
//!
//! The targeted acts (`PT_TOURS` percent of turns) also use the game's
//! save and load menus (a slot clicked, a name typed, Enter; then the
//! same slot loaded back and compared), load that slot from a fresh
//! title screen's Load and play on, walk up to the Imperial City's
//! palace doors (trigger entries 11..13) and click, and after each load
//! run the CITYDATA check (242A:000E), reporting a failure as odd.
//!
//! The targeted acts also travel (the map opened, a random place or
//! region clicked, Travel), open the bar's screens (automap, sheet,
//! logbook, status, camp) and write automap notes. Every turn checks
//! the heap's chain, the EMS bank mapped, the text nesting (6E54,
//! 3276:7307) back at rest, the dice's seed moving, and DGROUP's file
//! names intact.
//!
//! Two Nones are the original's own faults, not the port's:
//! - `%en` (2DA2:05DC) outside an equipment store (D48 not 6) pushes DS
//!   and pops it only on the store's path, so its `pop dx; ret` (066C)
//!   returns to the text's offset. Townspeople's answers reach it
//!   (#0271 "work for the owner of %en", #0289, #0303, ...): seed 105
//!   at CITY:1:2 hour 10 stops at frame 8793 on #0271.
//! - A note clicked on the automap can overflow a divide in the hit test
//!   (0000:31FE / 32FE); the view prints "the note met a fault of the
//!   original's" and goes on.

use crate::{Assets, Shot, shot_frame};
use arena::controls::Intent;
use arena::engine::Engine;
use arena::mem::DS;
use arena::movement::var::{HEADING, HEIGHT, X, Y};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::Instant;

/// xorshift64*: the policy's dice.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Where the player is, for the reports.
fn place(engine: &Engine, run: &arena::transit::Run) -> String {
    let r = |at| engine.m.mem.r16(DS, at);
    format!(
        "{run:?} {} at ({:#x}, {:#x}) square ({}, {}) heading {:#x} height {} slot {}",
        String::from_utf8_lossy(engine.m.mem.cstr(DS, arena::transit::var::LEVEL_NAME)),
        r(X),
        r(Y),
        r(X) >> 7,
        r(Y) >> 7,
        r(HEADING),
        r(HEIGHT) as i16,
        engine.m.mem.r8(DS, arena::save::var::SLOT)
    )
}

/// What a save and a load must keep: the level's name, the position and
/// the heading (the run's label may differ: an interior's name pointer).
fn kept(engine: &Engine) -> (Vec<u8>, u16, u16, u16, u16) {
    let r = |at| engine.m.mem.r16(DS, at);
    (engine.m.mem.cstr(DS, arena::transit::var::LEVEL_NAME).to_vec(), r(X), r(Y), r(HEIGHT), r(HEADING))
}

/// The CITYDATA check (242A:000E) after a load: a failure reported.
fn citydata(engine: &mut Engine, n: usize, run: &arena::transit::Run, issues: &std::cell::Cell<usize>) {
    let mut bad = false;
    if engine.m.citydata_check(&mut engine.files, &mut |_, _| bad = true).is_none() {
        println!("NONE frame {n} (citydata check); {}", place(engine, run));
        issues.set(issues.get() + 1);
    } else if bad {
        println!("ODD frame {n}: the CITYDATA check failed after a load; {}", place(engine, run));
        issues.set(issues.get() + 1);
    }
}

/// The odd states looked for after each turn, and round 3's invariants:
/// the heap's chain whole (`heap_check`), the EMS bank mapped one there
/// is (EMS_MAPPED a page times 4, below EMS_PAGES), the text system's
/// nesting back where it rests between turns (LEVEL, SUB_LEVEL: a text
/// given up mid-way once left them raised and the next ran over DGROUP),
/// and the game's dice moving (the seed not the same for 2000 turns).
struct Watch {
    text_level: u8,
    sub_level: u8,
    seed: u32,
    same: usize,
}

impl Watch {
    fn new(engine: &Engine) -> Watch {
        Watch {
            text_level: engine.m.mem.r8(DS, arena::text::var::LEVEL),
            sub_level: engine.m.mem.r8(arena::mem::DATA, arena::text::var::SUB_LEVEL),
            seed: engine.m.rng.seed,
            same: 0,
        }
    }

    fn check(&mut self, engine: &Engine) -> Option<String> {
        let m = &engine.m;
        let r = |at| m.mem.r16(DS, at);
        let (w, d) = (r(0xC357), r(0xC359));
        let (col, row) = (r(X) >> 7, r(Y) >> 7);
        if w != 0 && d != 0 && (col >= w || row >= d) {
            return Some(format!("off the map: square ({col}, {row}) of {w} x {d}"));
        }
        let h = r(HEIGHT) as i16;
        if !(-200..=2000).contains(&h) {
            return Some(format!("height {h}"));
        }
        // DGROUP's constant file names stay as the image has them: the text
        // system's POINTER1.DAT (197F) and TEMPLATE.DAT (6E57).
        for (at, name) in [(0x197Fu16, &b"pointer1.dat"[..]), (0x6E57, b"template.dat")] {
            if m.mem.cstr(DS, at) != name {
                return Some(format!("DGROUP {at:04X} overwritten: {:?}", String::from_utf8_lossy(m.mem.cstr(DS, at))));
            }
        }
        if !m.heap_check() {
            return Some("the heap's chain broken (heap_check)".into());
        }
        use arena::level::var::{EMS_MAPPED, EMS_ON, EMS_PAGES};
        if m.mem.r8(DS, EMS_ON) != 0 {
            let mapped = r(EMS_MAPPED);
            if mapped & 3 != 0 || mapped >> 2 >= r(EMS_PAGES) {
                return Some(format!("EMS mapped {mapped:#x} of {} pages", r(EMS_PAGES)));
            }
        }
        let (tl, sl) = (m.mem.r8(DS, arena::text::var::LEVEL), m.mem.r8(arena::mem::DATA, arena::text::var::SUB_LEVEL));
        if (tl, sl) != (self.text_level, self.sub_level) {
            return Some(format!("text nesting left at {tl}, substitutions at {sl} (rest {}, {})", self.text_level, self.sub_level));
        }
        if m.rng.seed == self.seed {
            self.same += 1;
            if self.same >= 2000 {
                return Some(format!("the dice stuck: seed {:#x} for {} turns", self.seed, self.same));
            }
        } else {
            self.seed = m.rng.seed;
            self.same = 0;
        }
        None
    }
}

/// What a turn of the policy does.
#[derive(Clone, Copy, Debug)]
enum Act {
    Walk(i16, i16, i16),
    Click,
    Swing,
    Camp,
    Fight,
    SaveLoad,
    Jump,
    /// Round 2: stand facing a trigger wall (a building's door, a gate, a
    /// way out) and click it.
    Door,
    /// Stand facing a stairs square and walk into it.
    Stairs,
    /// Stand before a person or creature and click it (talk, loot).
    Talk,
    /// A shop of building kind 4..7 entered as the talk enters it.
    Shop(u8),
    /// A town's errand offered by its giver (the tavern's talk).
    Quest,
    /// A spell of the book cast, then aimed by clicks.
    Cast,
    /// Experience gained (the next minute checks the level).
    Xp,
    /// Items given, equipped, dropped and used.
    Items,
    /// The healing aid off, a fight summoned at 1 health: death, then the
    /// last save loaded back.
    Die,
    /// Round 3: the travel map opened (as Shift+M does), now and then the
    /// world map and another province, a place clicked and Travel: a
    /// journey when the click named one, the screen left otherwise.
    Travel,
    /// The save menu (a slot clicked, a name typed) and the load menu in
    /// play (`saveui`), the state compared as the save and load's.
    Menus,
    /// A fresh program's title Load (`load_game_title`) of the last save
    /// the menus made, and play going on from it.
    TitleLoad,
    /// A palace door clicked (the Imperial City's triggers 11..13).
    Palace,
    /// A button of the bar clicked (the automap, the status box, the
    /// logbook, the sheet, camping...), the automap's buttons pressed.
    Screens,
    /// The automap opened and a note begun where a click falls, its text
    /// typed from the answers.
    Note,
}

/// The squares of the walls plane whose word `pred` takes, each with an
/// open neighbour (its word 0) and the heading from there towards it.
fn facing(engine: &Engine, pred: impl Fn(u16) -> bool) -> Vec<(u16, u16, u16)> {
    let m = &engine.m;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    let word = |c: u16, r: u16| m.mem.r16(walls, r << 8 | c << 1);
    let mut out = Vec::new();
    for r in 0..d {
        for c in 0..w {
            if !pred(word(c, r)) {
                continue;
            }
            // From the north (row-1) facing +y (0), the south facing -y
            // (100h), the west facing +x (80h), the east facing -x (180h).
            for (dc, dr, h) in [(0i32, -1i32, 0u16), (0, 1, 0x100), (-1, 0, 0x80), (1, 0, 0x180)] {
                let (nc, nr) = (c as i32 + dc, r as i32 + dr);
                if nc >= 0 && nr >= 0 && (nc as u16) < w && (nr as u16) < d && word(nc as u16, nr as u16) == 0 {
                    out.push((nc as u16, nr as u16, h));
                    break;
                }
            }
        }
    }
    out
}

/// The player put on a square's centre facing `heading`.
fn stand(engine: &mut Engine, (c, r, h): (u16, u16, u16)) {
    engine.m.mem.w16(DS, X, c << 7 | 0x40);
    engine.m.mem.w16(DS, Y, r << 7 | 0x40);
    engine.m.mem.w16(DS, HEADING, h);
}

pub fn run(dir: &Path, mut engine: Engine, mut run: arena::transit::Run, seed: u64, frames: usize, setup: &str, saves: Option<(&Path, u8)>) {
    let run = &mut run;
    let assets = Assets::new(dir, &mut engine);
    let audio = crate::audio::Audio::new(assets.archive.clone(), false, false);
    let music = crate::music::Music::new(assets.archive.clone(), false, false);
    let st = &mut Shot {
        audio,
        music,
        movies: Vec::new(),
        assets,
        right: false,
        drag: (0, 0),
        clicked: false,
        camp: false,
        shop: None,
        answers: Default::default(),
        message: None,
        frames: 0,
        boxes: Vec::new(),
        need: None,
        maker_screen: None,
        status: None,
        panel: None,
        map: false,
        over: false,
        ui: None,
        travel: None,
        cursor: (160, 100),
        note: None,
    };
    let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    // Round 2's targeted acts (PT_TOURS=N: N% of the acts, 30 when not a
    // number), with a save to come back to after a death.
    let tours_pc: u64 = std::env::var("PT_TOURS").ok().map(|v| v.parse().unwrap_or(30)).unwrap_or(0);
    let tours = tours_pc > 0;
    let mut no_heal = 0usize;
    if let (true, Some((sdir, slot))) = (tours, saves) {
        crate::save_to(&mut engine, sdir, slot, st.assets.palette);
    }
    // A sub-step that gives up (None) is reported and the run goes on.
    let report_none = |what: &str, n: usize, engine: &Engine, run: &arena::transit::Run| match arena::giveup::peek() {
        Some(g) => println!("{} frame {n} ({what}): {}; {}", g.kind.marker(), g.why, place(engine, run)),
        None => println!("NONE frame {n} ({what}): {}", place(engine, run)),
    };
    // An original fault met in a turn: the game reloaded from the save, as
    // after a death (the original would have crashed there).
    let fault = std::cell::Cell::new(false);
    let issues = std::cell::Cell::new(0usize);
    let mut last_odd = None;
    let mut watch: Option<Watch> = None;
    // One turn, caught: Some(true) on, Some(false) stopped (reported).
    let mut turn = |engine: &mut Engine, st: &mut Shot, run: &mut arena::transit::Run, intent: Intent, what: &str, n: usize| -> bool {
        let t = Instant::now();
        // PT_TRACE: each turn named on stderr before it runs, to find a
        // hang's turn.
        if std::env::var_os("PT_TRACE").is_some() {
            eprintln!("TRACE frame {n} ({what}) {}", place(engine, run));
        }
        engine.m.gave_up = None;
        arena::giveup::take();
        let r = catch_unwind(AssertUnwindSafe(|| shot_frame(dir, engine, run, st, &intent)));
        let ms = t.elapsed().as_millis();
        if ms > 1000 {
            println!("SLOW frame {n} ({what}): {ms} ms, {}", place(engine, run));
        }
        match r {
            Ok(Some(())) => {}
            Ok(None) => {
                let gave_up = arena::giveup::peek();
                if let Some(g) = gave_up.filter(|g| g.kind == arena::giveup::Kind::Original) {
                    println!("ORIGFAULT frame {n} ({what}): {}; {}", g.why, place(engine, run));
                    if tours && saves.is_some() {
                        fault.set(true);
                        return true;
                    }
                    return false;
                }
                match gave_up {
                    Some(g) => println!("{} frame {n} ({what}): {}; {}", g.kind.marker(), g.why, place(engine, run)),
                    None => println!("NONE frame {n} ({what}): {}", place(engine, run)),
                }
                return false;
            }
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                println!("PANIC frame {n} ({what}): {msg}; {}", place(engine, run));
                return false;
            }
        }
        let w = watch.get_or_insert_with(|| Watch::new(engine));
        if let Some(o) = w.check(engine) {
            if last_odd.as_ref() != Some(&o) {
                println!("ODD frame {n} ({what}): {o}; {}", place(engine, run));
                issues.set(issues.get() + 1);
            }
            last_odd = Some(o);
        } else {
            last_odd = None;
        }
        true
    };
    let mut n = 0usize;
    let mut alive = true;
    // The setup's steps.
    for step in setup.split(',').filter(|s| !s.is_empty()) {
        if !alive {
            break;
        }
        if let Some(at) = step.strip_prefix("at:") {
            let v: Vec<u16> = at.split(':').map(|x| x.parse().expect("at:COL:ROW:HEADING")).collect();
            engine.m.mem.w16(DS, X, v[0] << 7 | 0x40);
            engine.m.mem.w16(DS, Y, v[1] << 7 | 0x40);
            engine.m.mem.w16(DS, HEADING, v[2]);
            n += 1;
            alive = turn(&mut engine, st, run, Intent::default(), step, n);
        } else if step == "click" {
            st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
            n += 1;
            alive = turn(&mut engine, st, run, Intent::default(), step, n);
        } else if let Some(k) = step.strip_prefix("wait:") {
            for _ in 0..k.parse::<usize>().expect("wait:N") {
                n += 1;
                alive = alive && turn(&mut engine, st, run, Intent::default(), step, n);
            }
        } else {
            panic!("--playtest setup: {step}?");
        }
    }
    println!("start: {}", place(&engine, run));
    let mut act = Act::Walk(256, 0, 0);
    // The slot the menus last saved, for a title's Load.
    let mut last_menu_slot: Option<u8> = None;
    let mut left = 0;
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    while alive && n < frames {
        if st.over || fault.replace(false) {
            // Dead (or the game won, or an original fault met): with a save,
            // loaded back.
            match (tours, saves) {
                // As the window does after the game's end (`game_over`): the
                // main menu's Load on a fresh engine, the start dungeon loaded.
                (true, Some((sdir, slot))) => {
                    engine = crate::world::load(dir, "START.MIF");
                    let loaded = crate::load_from(&mut engine, dir, sdir, slot);
                    match loaded {
                        Ok((_, r)) => {
                            *run = r;
                            st.assets = Assets::new(dir, &mut engine);
                            st.over = false;
                            st.ui = None;
                            no_heal = 0;
                            *counts.entry("reloaded").or_default() += 1;
                        }
                        Err(e) => {
                            println!("SAVELOAD frame {n}: reload after the end failed: {e}");
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
        if left == 0 {
            left = 5 + dice.below(40) as usize;
            act = if tours && dice.below(100) < tours_pc {
                match dice.below(25) {
                    22 => Act::Menus,
                    23 => Act::TitleLoad,
                    24 => Act::Palace,
                    0..=3 => Act::Door,
                    4..=5 => Act::Stairs,
                    6..=8 => Act::Talk,
                    9 | 17 => Act::Shop(4 + dice.below(4) as u8),
                    10 => Act::Quest,
                    11..=12 => Act::Cast,
                    13 => Act::Xp,
                    14..=15 => Act::Items,
                    18..=19 => Act::Travel,
                    20 => Act::Screens,
                    21 => Act::Note,
                    _ => Act::Die,
                }
            } else {
                match dice.below(100) {
                0..=54 => Act::Walk(256, 0, 0),
                55..=69 => Act::Walk(0, 0, (dice.below(9) as i16 - 4) * 32),
                70..=77 => Act::Walk(if dice.below(2) == 0 { 256 } else { -256 }, if dice.below(2) == 0 { 256 } else { -256 }, 0),
                78..=89 => Act::Click,
                90..=95 => Act::Swing,
                96 => Act::Camp,
                97 => Act::Fight,
                98 => Act::Jump,
                _ => Act::SaveLoad,
                }
            };
            if let Act::Door | Act::Stairs | Act::Talk | Act::Shop(_) | Act::Quest | Act::Cast | Act::Xp | Act::Items | Act::Die | Act::Travel | Act::Screens | Act::Note | Act::Menus | Act::TitleLoad | Act::Palace = act {
                left = 12;
            }
        }
        left -= 1;
        n += 1;
        let name = match act {
            Act::Walk(..) => "walk",
            Act::Click => "click",
            Act::Swing => "swing",
            Act::Camp => "camp",
            Act::Fight => "fight",
            Act::SaveLoad => "save/load",
            Act::Jump => "jump",
            Act::Door => "door",
            Act::Stairs => "stairs",
            Act::Talk => "talk",
            Act::Shop(_) => "shop",
            Act::Quest => "quest",
            Act::Cast => "cast",
            Act::Xp => "xp",
            Act::Items => "items",
            Act::Die => "die",
            Act::Travel => "travel",
            Act::Screens => "screens",
            Act::Note => "note",
            Act::Menus => "menus",
            Act::TitleLoad => "title load",
            Act::Palace => "palace",
        };
        // A test aid, as --do hp is: the player healed when below a third,
        // so a run goes on past its fights.
        let (hp, max) = (engine.m.mem.r16(DS, 0x01FD), engine.m.mem.r16(DS, 0x01FF));
        no_heal = no_heal.saturating_sub(1);
        if hp < max / 3 && no_heal == 0 {
            engine.m.mem.w16(DS, 0x01FD, max);
            *counts.entry("healed").or_default() += 1;
        }
        // And rested when fatigue (201h) is below a quarter of its most,
        // (strength + endurance) x 64: once exhausted, an hour's rest
        // (2901:0014, 016C) makes up 420 for a heal mod of 0 where the hour
        // takes 660, so the player collapses again and again, the
        // original until its stack runs into its data, the port until its
        // bound of 200 (None). A player rests before that.
        let attr = |i: u16| engine.m.mem.r8(DS, arena::character::PLAYER + arena::character::field::ATTRS + i) as u16;
        let most = (attr(0) + attr(5)) * 64;
        if engine.m.mem.r16(DS, 0x0201) < most / 4 {
            engine.m.mem.w16(DS, 0x0201, most);
            *counts.entry("rested").or_default() += 1;
        }
        *counts.entry(name).or_default() += 1;
        // The game's questions answered at random (talk menus, shops,
        // haggles, rests): menu entries 0..5, now and then a number typed.
        while st.answers.len() < 3 {
            let a = if dice.below(7) == 0 {
                crate::dialog::Answer::Text(dice.below(120).to_string())
            } else {
                crate::dialog::Answer::Pick(dice.below(6) as u16)
            };
            st.answers.push_back(a);
        }
        let mut intent = Intent::default();
        match act {
            Act::Walk(f, s, look) => {
                intent.forward = f;
                intent.strafe = s;
                intent.look = look;
            }
            Act::Click => {
                if left % 4 == 0 {
                    st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
                }
            }
            Act::Swing => {
                if left % 6 == 0 {
                    crate::interface::ready(&mut engine.m);
                }
                st.right = true;
                st.drag = ((dice.below(200) as i16) - 100, 100);
            }
            Act::Camp => {
                if left == 0 || left % 20 == 0 {
                    st.camp = true;
                }
            }
            Act::Fight => {
                if left == 0 {
                    crate::world::summon(&mut engine);
                }
            }
            Act::Jump => {
                // To a random free square of the map (its walls' word 0), a
                // test aid spreading a run over the map, as --at is.
                if left == 0 {
                    let walls = engine.m.mem.r16(DS, arena::mif::var::MAP1_SEG);
                    let (w, d) = (engine.m.mem.r16(DS, 0xC357).min(128) as u64, engine.m.mem.r16(DS, 0xC359).min(128) as u64);
                    for _ in 0..200 {
                        let (c, r) = (dice.below(w.max(1)) as u16, dice.below(d.max(1)) as u16);
                        if engine.m.mem.r16(walls, r << 8 | c << 1) == 0 {
                            engine.m.mem.w16(DS, X, c << 7 | 0x40);
                            engine.m.mem.w16(DS, Y, r << 7 | 0x40);
                            break;
                        }
                    }
                }
            }
            Act::Door => {
                // A trigger wall (0000:43B1's test: bit 15 or both bytes
                // equal, its low six bits in the trigger table at C917).
                if left == 11 {
                    let trig: Vec<u8> = (0..0x10).map(|i| engine.m.mem.r8(DS, arena::movement::var::TRIGGERS + i)).collect();
                    let doors = facing(&engine, |w| w != 0 && ((w as i16) < 0 || (w >> 8) as u8 == w as u8) && trig.contains(&(w as u8 & 0x3F)));
                    if !doors.is_empty() {
                        let d = doors[dice.below(doors.len() as u64) as usize];
                        stand(&mut engine, d);
                        *counts.entry("door found").or_default() += 1;
                    }
                } else if left == 9 || left == 5 {
                    st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
                }
            }
            Act::Stairs => {
                if left == 11 {
                    let marks = [engine.m.mem.r8(DS, arena::movement::var::STAIRS_UP), engine.m.mem.r8(DS, arena::movement::var::STAIRS_DOWN)];
                    let stairs = facing(&engine, |w| w != 0 && marks.contains(&(w as u8)));
                    if !stairs.is_empty() {
                        let s = stairs[dice.below(stairs.len() as u64) as usize];
                        stand(&mut engine, s);
                        *counts.entry("stairs found").or_default() += 1;
                    }
                } else {
                    intent.forward = 256;
                }
            }
            Act::Talk => {
                // An active slot's actor (0AB3, 23 of 1Ch; +0Fh bit 4000h
                // inactive), the player a square from it facing it.
                if left == 11 {
                    let active: Vec<u16> = (0..23u16).map(|i| 0x0AB3 + i * 0x1C).filter(|&si| engine.m.mem.r16(DS, si + 0xF) & 0x4000 == 0).collect();
                    // Only actors on the map (a slot's x and y can hold
                    // markers such as FFFFh).
                    let (w, d) = (engine.m.mem.r16(DS, 0xC357) << 7, engine.m.mem.r16(DS, 0xC359) << 7);
                    let active: Vec<u16> = active.into_iter().filter(|&si| engine.m.mem.r16(DS, si) < w && engine.m.mem.r16(DS, si + 4) < d).collect();
                    if !active.is_empty() {
                        let si = active[dice.below(active.len() as u64) as usize];
                        let (ax, ay) = (engine.m.mem.r16(DS, si) as f32, engine.m.mem.r16(DS, si + 4) as f32);
                        let h = dice.below(512) as f32;
                        let a = h / 512.0 * std::f32::consts::TAU;
                        engine.m.mem.w16(DS, X, (ax - a.sin() * 150.0).clamp(64.0, w as f32 - 64.0) as u16);
                        engine.m.mem.w16(DS, Y, (ay - a.cos() * 150.0).clamp(64.0, d as f32 - 64.0) as u16);
                        engine.m.mem.w16(DS, HEADING, h as u16);
                        *counts.entry("actor found").or_default() += 1;
                    }
                } else if left == 9 || left == 6 {
                    st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
                }
            }
            Act::Shop(k) => {
                if left == 11 {
                    st.shop = Some(k);
                }
            }
            Act::Quest => {
                use arena::sidequest::var::{TALK, TALKER};
                let town = engine.m.mem.r16(DS, 0xC398) & 4 != 0 && engine.m.mem.r16(DS, arena::movement::var::LEVEL_FLAGS) & 0x200 == 0;
                if left == 11 && town {
                    let offered = |m: &arena::machine::Machine| (0..4u16).map(|i| arena::quest::var::OTHERS + i * 0x29).find(|&si| m.quest_kind(si) == 2);
                    if offered(&engine.m).is_none() {
                        engine.m.mem.w16(DS, arena::sidequest::var::PLACE_SEEN, 0xFFFF);
                        if engine.m.city_entered(&mut engine.files, &mut arena::quest::NoQuestHooks).is_none() {
                            report_none("quest: city_entered", n, &engine, run);
                        }
                    }
                    if let Some(si) = offered(&engine.m) {
                        let g = engine.m.mem.r16(DS, si + 0x10);
                        engine.m.mem.w8(DS, TALK, 4);
                        engine.m.mem.w16(DS, TALKER, g & 0xFF00 | ((g as u8 as u16) << 1) & 0xFF);
                        st.shop = Some(4);
                        *counts.entry("errand offered").or_default() += 1;
                    }
                }
            }
            Act::Cast => {
                if left == 11 || left == 1 {
                    if engine.m.mem.r8(DS, crate::panels::BOOK_COUNT) == 0 {
                        for i in 0..12u8 {
                            crate::panels::learn(&mut engine.m, i);
                        }
                    }
                    engine.m.mem.w16(DS, 0x020A, 600);
                    engine.m.mem.w16(DS, 0x020C, 600);
                    let book = crate::panels::spells(&engine.m);
                    if !book.is_empty() {
                        let idx = book[dice.below(book.len() as u64) as usize].index;
                        let mut said = crate::panels::Said::default();
                        if engine.m.cast_spell(&mut engine.files, &mut said, idx).is_none() {
                            report_none(&format!("cast spell {idx}"), n, &engine, run);
                        }
                        *counts.entry("spells cast").or_default() += 1;
                    }
                } else if left % 3 == 0 {
                    st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
                }
            }
            Act::Xp => {
                if left == 11 {
                    let at = arena::character::PLAYER + 0x409;
                    let v = engine.m.mem.r32(DS, at).wrapping_add(500 + dice.below(20000) as u32);
                    engine.m.mem.w32(DS, at, v);
                    let f = engine.m.mem.r16(DS, 0x05CA) | 0x100;
                    engine.m.mem.w16(DS, 0x05CA, f);
                }
            }
            Act::Items => {
                if left == 11 && crate::panels::item_lines(&engine.m).len() < 20 {
                    crate::panels::give(&mut engine.m, 1 + dice.below(4) as u16);
                }
                if left % 3 == 0 {
                    let lines = crate::panels::item_lines(&engine.m);
                    let mut said = crate::panels::Said::default();
                    if !lines.is_empty() {
                        let slot = lines[dice.below(lines.len() as u64) as usize].slot;
                        match dice.below(3) {
                            0 => {
                                if engine.m.equip_click(&mut said, slot).is_none() {
                                    report_none(&format!("equip slot {slot}"), n, &engine, run);
                                }
                            }
                            1 => {
                                engine.m.mem.w8(DS, arena::use_item::var::SELECTED, slot as u8);
                                engine.m.drop_item();
                            }
                            _ => {
                                said.pick = Some(dice.below(4) as u16);
                                if engine.m.use_item(&mut engine.files, &mut said).is_none() {
                                    report_none("use item", n, &engine, run);
                                }
                            }
                        }
                    }
                }
            }
            Act::Die => {
                if left == 11 {
                    no_heal = 400;
                    engine.m.mem.w16(DS, 0x01FD, 1);
                    crate::world::summon(&mut engine);
                }
            }
            Act::Travel => {
                if left == 11 {
                    use winit::keyboard::KeyCode;
                    let archive = st.assets.archive.clone();
                    let here = engine.m.mem.cstr(DS, 0x0A22).to_vec();
                    match crate::travel::Travel::open(&mut engine, &archive) {
                        Ok(mut t) => {
                            *counts.entry("travel opened").or_default() += 1;
                            if dice.below(3) == 0 {
                                // The world map, and a province's region.
                                t.key(&mut engine, KeyCode::Escape, None);
                                let (x, y) = (dice.below(320) as i32, dice.below(200) as i32);
                                t.click(&mut engine, &archive, x, y, false);
                            }
                            let (x, y) = (10 + dice.below(300) as i32, 5 + dice.below(155) as i32);
                            t.click(&mut engine, &archive, x, y, false);
                            t.click(&mut engine, &archive, 62, 180, false);
                            let mut open = Some(t);
                            let mut boxes = Vec::new();
                            crate::travel::after(&mut open, &mut engine, &mut boxes, true);
                            // No journey: the screen left as Esc leaves it.
                            for _ in 0..4 {
                                if let Some(t) = open.as_mut() {
                                    t.key(&mut engine, KeyCode::Escape, None);
                                }
                                crate::travel::after(&mut open, &mut engine, &mut boxes, true);
                            }
                            if open.is_some() {
                                println!("ODD frame {n} (travel): the travel map didn't end; {}", place(&engine, run));
                                issues.set(issues.get() + 1);
                            }
                            if !boxes.is_empty() {
                                st.boxes = boxes;
                            }
                            if engine.m.mem.cstr(DS, 0x0A22) != here {
                                *counts.entry("journeys").or_default() += 1;
                            }
                        }
                        Err(_) => *counts.entry("travel refused").or_default() += 1,
                    }
                }
            }
            Act::Screens => {
                if left == 11 {
                    let archive = st.assets.archive.clone();
                    crate::sheets::pics(&archive, &engine.m);
                    let (x, y) = crate::sheets::bar_centre(dice.below(9) as usize);
                    match crate::sheets::bar_click(&mut engine.m, x, y, dice.below(4) == 0) {
                        Some(crate::sheets::BarAction::Automap) => {
                            crate::automap::open(&mut engine, &archive);
                            for _ in 0..1 + dice.below(4) {
                                engine.m.automap_view_button(dice.below(5) as u16);
                                engine.m.automap_origin();
                            }
                            *counts.entry("automap").or_default() += 1;
                        }
                        Some(crate::sheets::BarAction::Status(lines)) => {
                            st.boxes = vec![lines];
                            *counts.entry("status").or_default() += 1;
                        }
                        Some(crate::sheets::BarAction::Log) => {
                            let _ = crate::sheets::open_log(&engine.m, &engine.files);
                            *counts.entry("logbook").or_default() += 1;
                        }
                        Some(crate::sheets::BarAction::Camp) => st.camp = true,
                        _ => {}
                    }
                }
            }
            Act::Note => {
                if left == 11 && crate::automap::notes_here(&engine.m) {
                    let archive = st.assets.archive.clone();
                    crate::sheets::pics(&archive, &engine.m);
                    crate::automap::open(&mut engine, &archive);
                    st.note = Some((dice.below(320) as u16, dice.below(150) as u16));
                    *counts.entry("notes").or_default() += 1;
                }
            }
            Act::Menus => {
                if let (11, Some((sdir, own))) = (left, saves) {
                    use winit::keyboard::KeyCode;
                    // One of the menu's ten slots, not the run's own.
                    let mut slot = dice.below(10) as u8;
                    if slot == own {
                        slot = (slot + 1) % 10;
                    }
                    let row = |s: u8| 4 + s as i32 * 14 + 3;
                    let archive = st.assets.archive.clone();
                    let was = kept(&engine);
                    let (mut ui, mut e) = crate::saveui::SaveUi::open(crate::saveui::Kind::Save, &engine, sdir, &archive);
                    if let crate::saveui::Ended::Open = e {
                        e = ui.click(160, row(slot), false);
                    }
                    if let crate::saveui::Ended::Open = e {
                        for c in format!("pt{n}").chars() {
                            ui.key(KeyCode::KeyA, Some(&c.to_string()));
                        }
                        e = ui.key(KeyCode::Enter, None);
                    }
                    match e {
                        crate::saveui::Ended::Save(e2, s) => {
                            engine = e2;
                            let msg = crate::save_to(&mut engine, sdir, s, st.assets.palette);
                            if msg.contains("failed") {
                                println!("ODD frame {n} (menus): {msg}; {}", place(&engine, run));
                                issues.set(issues.get() + 1);
                            }
                            *counts.entry("menu saves").or_default() += 1;
                            let (mut ui, mut e) = crate::saveui::SaveUi::open(crate::saveui::Kind::Load, &engine, sdir, &archive);
                            if let crate::saveui::Ended::Open = e {
                                e = ui.click(160, row(s), false);
                            }
                            match e {
                                crate::saveui::Ended::Load(mut e3, s3) => match crate::areas::load_game(&mut e3, dir, sdir, s3) {
                                    Ok(r) => {
                                        engine = e3;
                                        *run = r;
                                        st.assets = Assets::new(dir, &mut engine);
                                        *counts.entry("menu loads").or_default() += 1;
                                        if was != kept(&engine) {
                                            println!("SAVELOAD frame {n} (menus): slot {s3}; before {:?}; after {}", String::from_utf8_lossy(&was.0), place(&engine, run));
                                            issues.set(issues.get() + 1);
                                        }
                                        citydata(&mut engine, n, run, &issues);
                                        last_menu_slot = Some(s3);
                                    }
                                    Err(err) => {
                                        println!("SAVELOAD frame {n} (menus): load failed: {err}; {}", place(&engine, run));
                                        issues.set(issues.get() + 1);
                                    }
                                },
                                crate::saveui::Ended::Left(m) => {
                                    println!("ODD frame {n} (menus): the load menu left ({m:?}); {}", place(&engine, run));
                                    issues.set(issues.get() + 1);
                                }
                                _ => {
                                    println!("ODD frame {n} (menus): the load menu didn't take slot {s}; {}", place(&engine, run));
                                    issues.set(issues.get() + 1);
                                }
                            }
                        }
                        // A refusal (in a shop, say) is the game's.
                        crate::saveui::Ended::Left(_) => *counts.entry("menu refused").or_default() += 1,
                        _ => {
                            println!("ODD frame {n} (menus): the save menu didn't end; {}", place(&engine, run));
                            issues.set(issues.get() + 1);
                        }
                    }
                }
            }
            Act::TitleLoad => {
                if let (11, Some((sdir, _)), Some(slot)) = (left, saves, last_menu_slot) {
                    let archive = st.assets.archive.clone();
                    let fresh = crate::world::load(dir, "START.MIF");
                    let (mut ui, mut e) = crate::saveui::SaveUi::open(crate::saveui::Kind::TitleLoad, &fresh, sdir, &archive);
                    if let crate::saveui::Ended::Open = e {
                        e = ui.click(160, 4 + slot as i32 * 14 + 3, false);
                    }
                    match e {
                        crate::saveui::Ended::Load(mut e2, s) => match crate::areas::load_game_title(&mut e2, dir, sdir, s) {
                            Ok(r) => {
                                engine = e2;
                                *run = r;
                                st.assets = Assets::new(dir, &mut engine);
                                st.over = false;
                                st.ui = None;
                                *counts.entry("title loads").or_default() += 1;
                                citydata(&mut engine, n, run, &issues);
                            }
                            Err(err) => {
                                println!("SAVELOAD frame {n} (title load): {err}; {}", place(&engine, run));
                                issues.set(issues.get() + 1);
                            }
                        },
                        _ => {
                            println!("ODD frame {n} (title load): the menu didn't load slot {slot}; {}", place(&engine, run));
                            issues.set(issues.get() + 1);
                        }
                    }
                }
            }
            Act::Palace => {
                // In the Imperial City, the doors whose trigger is one of the
                // palace's (11..13 of the table at C917, 0000:4192).
                if left == 11 {
                    let trig: Vec<u8> = (0..0x10).map(|i| engine.m.mem.r8(DS, arena::movement::var::TRIGGERS + i)).collect();
                    let palace: Vec<u8> = trig.iter().enumerate().filter(|(i, _)| (11..=13).contains(i)).map(|(_, &t)| t).filter(|&t| t != 0).collect();
                    let city = engine.m.mem.cstr(DS, 0x0A22).eq_ignore_ascii_case(b"Imperial City");
                    let doors = if city { facing(&engine, |w| w != 0 && ((w as i16) < 0 || (w >> 8) as u8 == w as u8) && palace.contains(&(w as u8 & 0x3F))) } else { Vec::new() };
                    if !doors.is_empty() {
                        let d = doors[dice.below(doors.len() as u64) as usize];
                        stand(&mut engine, d);
                        *counts.entry("palace door").or_default() += 1;
                    }
                } else if left == 9 || left == 5 {
                    st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
                }
            }
            Act::SaveLoad => {
                if let (0, Some((sdir, slot))) = (left, saves) {
                    let pre = place(&engine, run);
                    let msg = crate::save_to(&mut engine, sdir, slot, st.assets.palette);
                    let before = place(&engine, run);
                    let was = kept(&engine);
                    if std::env::var_os("PT_VERBOSE").is_some() {
                        println!("save frame {n}: fighting {} resting {} pre {pre}", engine.m.mem.r8(DS, arena::encounter::var::FIGHTING), engine.m.mem.r8(DS, arena::rest::var::RESTING));
                    }
                    match crate::load_from(&mut engine, dir, sdir, slot) {
                        Ok((_, r)) => {
                            *run = r;
                            st.assets = Assets::new(dir, &mut engine);
                            let after = place(&engine, run);
                            if was != kept(&engine) {
                                println!("SAVELOAD frame {n}: {msg}; before {before}; after {after}");
                                issues.set(issues.get() + 1);
                            }
                        }
                        Err(e) => {
                            println!("SAVELOAD frame {n}: {msg}; load failed: {e}; {before}");
                            issues.set(issues.get() + 1);
                        }
                    }
                }
            }
        }
        alive = turn(&mut engine, st, run, intent, name, n);
    }
    let result = if st.over {
        "over (the game ended)".to_string()
    } else if alive {
        "ok".to_string()
    } else {
        "stopped".to_string()
    };
    println!("end: {}", place(&engine, run));
    println!("PLAYTEST seed {seed} frames {n} result {result} odd {} acts {counts:?}", issues.get());
}
