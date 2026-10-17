//! Following the game from area to area. When a frame leaves the area
//! (`Frame::Left`: a building's door clicked, an interior's way out), the
//! area's run ends and the outer loop picks the next (src/transit.rs);
//! the view loads its level as the original's run does, and then makes
//! its assets again.
//!
//! Cities, interiors and the wilderness are followed: a city's gate
//! writes the city as the wilderness's blocks 1..4 (217B:027A, in
//! memory), the wilderness is set up around it (217B:0084) and its
//! blocks follow the player (217B:0265). A dungeon is made from its
//! location's seed (2294:0ED4, 0000:28BC), its levels as they are
//! entered (0000:2957, the INF's hook).

use crate::light;
use crate::world;
use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mem::DS;
use arena::transit::{AreaHooks, Run};
use std::path::Path;

/// The view's side of the area changes.
pub struct Areas<'a> {
    pub dir: &'a Path,
    /// The game's files, while the outer loop runs (the world's entry
    /// uses the engine's).
    pub files: Option<&'a mut Files>,
    /// Something the original does here failed in the port.
    pub failed: bool,
    /// The game ended (the player died, or the main quest is over): its
    /// text's number (1194:D459's AX).
    pub ending: Option<u16>,
}

/// The location record the view's cities use (as `world::city_load`).
const RECORD: u16 = 0x0A22;

impl AreaHooks for Areas<'_> {
    /// 1194:D459, the ending: its animation (SI, DI name it) isn't
    /// shown; its text (AX) is put on the message line by `next`.
    fn ending(&mut self, _m: &mut Machine, ax: u16, _si: u16, _di: u16) {
        self.ending = Some(ax);
    }

    /// 292B:095E: the main quest dungeon's level's locks and triggers
    /// kept in STATES.nn on the way out, as the stairs keep them (`Kept`).
    fn automap(&mut self, m: &mut Machine, files: &mut Files) {
        m.states_keep(files);
    }

    /// 0000:068A: the sky's state forgotten (`Machine::sky_reset`), then
    /// 06B8's 2294:0428: the day's colours for the weather loaded.
    fn hands(&mut self, m: &mut Machine) {
        m.sky_reset();
        if let (Some(name), Some(files)) = (m.sky_colours(), self.files.as_deref_mut()) {
            if m.sky_colours_load(files, name).is_none() {
                eprintln!("the sky's colours did not load (2294:0428)");
                self.failed = true;
            }
        }
    }

    /// 0000:464A..467C: the sky's objects made (`Machine::area_sky`), their
    /// images loaded as the game loads them (`GameSkyLoad`); where it
    /// answers false the palette named at 1304 read to 929E (0000:7530).
    fn sky(&mut self, m: &mut Machine) {
        let Some(files) = self.files.as_deref_mut() else { return };
        light::SKY_NAMES.with(|n| n.borrow_mut().clear());
        if !m.area_sky(&mut light::GameSkyLoad { files }) && m.colours_load(files, 0x1304, 0x929E).is_none() {
            eprintln!("the palette did not load (0000:7530)");
        }
    }

    fn normal_lgt(&mut self, m: &mut Machine) {
        world::lgt(m, self.dir, arena::weather::NORMAL_LGT);
    }

    /// 1194:3AE8: the record built by 3B84 in full (`city_location`):
    /// the province's data loaded (3C0D), the climate (record +2Eh, A50)
    /// read from the terrain map (3D91). The Imperial City's record
    /// (province 8) stays the Imperial City.
    fn location(&mut self, m: &mut Machine, id: u16) {
        let id = if m.mem.r8(DS, RECORD + 0x24) == 8 { 0x100 } else { id };
        let Some(files) = self.files.as_deref_mut() else {
            self.failed = true;
            return;
        };
        if m.city_location(files, DS, RECORD, id).is_none() {
            eprintln!("the location record for location {id:#x} could not be built (1194:3AE8)");
            self.failed = true;
        }
    }

    fn weather(&mut self, m: &mut Machine) {
        m.weather_set(&mut world::Lgt(self.dir));
    }

    /// The city built; its build leaves the buffer pointer (BA54) at the
    /// frame, which entering the area gives a buffer again (0000:A4C5).
    fn build_city(&mut self, m: &mut Machine) {
        let (off, seg) = (m.mem.r16(DS, 0xBA54), m.mem.r16(DS, 0xBA56));
        let Some(files) = self.files.as_deref_mut() else { return };
        if m.mif_build_city(files).is_none() {
            eprintln!("the city could not be built (1194:0292)");
            self.failed = true;
        }
        m.mem.w16(DS, 0xBA54, off);
        m.mem.w16(DS, 0xBA56, seg);
    }

    /// 0000:468B: the interface set up (`Machine::interface_setup`); indoors,
    /// the level entered (`Machine::area_entered_inside`:
    /// its lights, a main quest dungeon's locks and triggers (STATES.nn),
    /// the main quest's piece and a side quest's item or person placed,
    /// its piles of treasure, the palace's automap kept and loaded again,
    /// `Kept`); outdoors, a town's
    /// buildings listed and its quests made (`Machine::city_entered`,
    /// 0000:46B2).
    fn entered(&mut self, m: &mut Machine) {
        m.entered_flats();
        // 0000:46A2: the interface set up where it changed (4DD6), its
        // files read here, before what follows the entry.
        if let Some(files) = self.files.as_deref_mut() {
            if m.interface_setup(files, &mut arena::frame::NoFrameHooks).is_none() {
                eprintln!("the interface's set-up gave up (0000:4DD6): {}", arena::giveup::describe());
                self.failed = true;
            }
        }
        // 46AF: outdoors, 0390's light for the time of day, its loads made
        // in it (`LightNow`); the light handed to the assets made next.
        let mut light = light::LightState::new(self.dir, m);
        if m.mem.r16(DS, 0xC398) & 4 != 0 {
            if let Some(files) = self.files.as_deref_mut() {
                m.outdoor_light(&mut light::LightNow { light: &mut light, files });
            }
        }
        light::ENTRY_LIGHT.with(|l| *l.borrow_mut() = Some(light));
        if m.mem.r16(DS, 0xC398) & 4 == 0 {
            let Some(files) = self.files.as_deref_mut() else {
                m.flat_lights();
                m.mem.w16(DS, 0x7A46, 0);
                return;
            };
            if m.area_entered_inside(files, &mut Kept).is_none() {
                eprintln!("entering the interior met something the original hangs or faults on (0000:4767)");
                self.failed = true;
            }
        } else if let Some(files) = self.files.as_deref_mut() {
            if m.city_entered(files, &mut arena::quest::NoQuestHooks).is_none() {
                eprintln!("the town's quests gave up: {}", arena::giveup::describe());
            }
        }
    }

    /// 217B:0084, the weather's light tables from the game directory.
    fn wild_setup(&mut self, m: &mut Machine) {
        if m.wild_setup(&mut Wild(self.dir)).is_none() {
            self.failed = true;
        }
    }

    /// 2294:0ED4 then 0000:28BC (AX 1, BX `wild`, CX the level count):
    /// the dungeon made from the seed the outer loop set (91F6), with
    /// blank.mif for its planes.
    fn build_dungeon(&mut self, m: &mut Machine, wild: bool) {
        let Some(files) = self.files.as_deref_mut() else {
            self.failed = true;
            return;
        };
        m.load_rng();
        let levels = m.dungeon_level_count();
        if m.dungeon_create(files, 1, wild as u16, levels).is_none() {
            self.failed = true;
        }
        m.store_rng();
    }

    /// 0000:2974, a made dungeon entered: its first level built and the
    /// player at its start.
    fn hook_2974(&mut self, m: &mut Machine) {
        let Some(files) = self.files.as_deref_mut() else {
            self.failed = true;
            return;
        };
        if m.dungeon_first_level(files).is_none() {
            self.failed = true;
        }
    }

    fn wild_done(&mut self, m: &mut Machine) {
        m.wild_done();
    }

    /// 0000:45FC..4611: the chunks follow the player (217B:0184, which
    /// loads the first blocks), then whether the quarter the player is in
    /// has no people (217B:0058).
    fn wild_empty(&mut self, m: &mut Machine) -> bool {
        let Some(files) = self.files.as_deref_mut() else {
            self.failed = true;
            return true;
        };
        if m.wild_shift(files, &mut arena::wild::PortWild).is_none() {
            self.failed = true;
        }
        !m.wild_quarter_ok()
    }

    /// 0000:3503, entering the world: the area's automap loaded.
    fn border(&mut self, m: &mut Machine) {
        if let Some(files) = self.files.as_deref_mut() {
            m.automap_load(files);
        }
    }

    /// After the world loop: the area's automap written back (23F1:013C);
    /// the screens' fades aren't the view's.
    fn world_screen(&mut self, m: &mut Machine) {
        if let Some(files) = self.files.as_deref_mut() {
            m.automap_keep(files);
        }
    }

    /// 0000:17F9 (`Machine::townsfolk_load`).
    fn townsfolk_graphics(&mut self, m: &mut Machine) {
        if let Some(files) = self.files.as_deref_mut() {
            if m.townsfolk_load(files).is_none() {
                self.failed = true;
            }
        }
    }

    fn spawn(&mut self, m: &mut Machine, n: u16) {
        if m.spawn_actors(n, 0xA).is_none() {
            eprintln!("the townspeople gave up: {}", arena::giveup::describe());
        }
    }
}

/// A load's fights: the view draws people and creatures from its sprite
/// sheet, made again after the load.
impl arena::encounter::FightHooks for Areas<'_> {}

impl arena::light::LightHooks for Areas<'_> {
    /// 2294:0B8A's load: NORMAL.LGT, then the table made (0000:AAA2).
    fn light_table(&mut self, m: &mut Machine) {
        world::lgt(m, self.dir, arena::weather::NORMAL_LGT);
        world::light_table(m);
    }
}

impl arena::load::LoadHooks for Areas<'_> {
    /// 0000:AAA2: the light table made (the load freed it).
    fn renderer(&mut self, m: &mut Machine) {
        world::light_table(m);
    }
    /// 0000:E5C9: outdoors, the sky's images into the sky's bank
    /// (`Machine::sky_images`).
    fn sky_graphics(&mut self, m: &mut Machine, files: &mut Files) {
        if m.sky_images(files).is_none() {
            eprintln!("the sky's images did not load (0000:E5C9)");
            self.failed = true;
        }
    }
    /// 25FD:06A1: the slot's files copied to the working ones, in the
    /// engine's files and in the copy these hooks read (`files`, taken
    /// before the load): the area's entry (`entered`: a town's
    /// buildings, from CITYDATA) runs inside the load and reads the copy.
    fn slot_files(&mut self, m: &mut Machine, files: &mut Files) {
        // The copy's on a scratch machine: the game's copies once (their
        // paths' turns, handles and EMS banks are the game's state).
        if let Some(copy) = self.files.as_deref_mut() {
            m.clone().slot_files_in(copy);
        }
        m.slot_files_in(files);
    }
}

/// The run of the area the game is in, by 5E6 (an interior's level named
/// at LEVEL_NAME).
pub fn run_of(m: &Machine) -> Run {
    match m.mem.r8(DS, arena::trigger::var::AREA) {
        0 => Run::City,
        1 => Run::Wild,
        2 => Run::Start,
        3 => Run::Dungeon,
        _ => Run::Interior(arena::transit::var::LEVEL_NAME),
    }
}

/// A saved game loaded in play from slot `slot` in `save` (the game's
/// own load, `Machine::game_load`, with the view's hooks), into a copy of
/// the engine kept only when it all loads. The old light table is freed
/// first (the light table's reset clears CCC2 before its hook runs); the
/// screenshot isn't read (the view has no screen buffer); the INF's
/// sounds hook is cleared after, as the view has no sound. Returns the
/// loaded area's run, or what went wrong.
pub fn load_game(engine: &mut Engine, dir: &Path, save: &Path, slot: u8) -> Result<Run, String> {
    load_game_as(engine, dir, save, slot, false)
}

/// A saved game loaded from the title screen's Load (0000:119D sets 5C8
/// bit 4 for it): the same, but the game's title-screen way
/// (`Machine::title_load`: the world loop entered afresh, the rebuild run
/// as the 3D mode's hook).
pub fn load_game_title(engine: &mut Engine, dir: &Path, save: &Path, slot: u8) -> Result<Run, String> {
    load_game_as(engine, dir, save, slot, true)
}

fn load_game_as(engine: &mut Engine, dir: &Path, save: &Path, slot: u8, title: bool) -> Result<Run, String> {
    NEW_GAME.with(|n| n.set(false));
    let mut e = Engine::new(engine.m.clone(), engine.files.clone());
    if title {
        let v = e.m.mem.r16(DS, 0x05C8) | 4;
        e.m.mem.w16(DS, 0x05C8, v);
    }
    e.files.add_dir(save).map_err(|err| format!("{}: {err}", save.display()))?;
    let old = e.m.mem.r16(DS, 0xCCC2);
    e.m.heap_free(old);
    e.m.mem.w16(DS, 0xCCC2, 0);
    let screen = e.m.mem.r8(DS, arena::save::var::NO_SCREEN);
    e.m.mem.w8(DS, arena::save::var::NO_SCREEN, 1);
    let mut copy = e.files.clone();
    let h = &mut Areas { dir, files: Some(&mut copy), failed: false, ending: None };
    let r = e.m.game_load(&mut e.files, slot, h);
    let failed = h.failed;
    e.m.mem.w8(DS, arena::save::var::NO_SCREEN, screen);
    match r {
        Some(arena::load::Loaded::Done) if !failed => {}
        // 1194:59C8: the game's own words for an empty slot (5F0F).
        Some(arena::load::Loaded::Missing) => return Err(String::from_utf8_lossy(e.m.mem.cstr(DS, arena::load::var::NO_SAVE)).into_owned()),
        _ => return Err(format!("Slot {slot} gave up while loading: {}.", arena::giveup::describe())),
    }
    // The load menu's way back (1194:5E87): PAL.COL read into 929E
    // (0000:7534), the screen's colours set from it.
    if title {
        e.m.colours_load(&mut e.files, 0x1304, 0x929E);
    }
    let run = run_of(&e.m);
    // When this area ends: a title load goes back to after the area's run
    // (no run's end); a load in play to the end of the run whose world
    // loop it loaded in.
    let tail = if title {
        Tail::None
    } else {
        match TAIL.with(|t| t.get()) {
            Tail::Current => Tail::Run(run_of(&engine.m)),
            other => other,
        }
    };
    TAIL.with(|t| t.set(tail));
    *engine = e;
    Ok(run)
}

/// The wilderness's hooks as the view runs them: the ports, the weather's
/// light tables from the game directory.
struct Wild<'a>(&'a Path);

impl arena::wild::WildHooks for Wild<'_> {
    fn weather(&mut self, m: &mut Machine) {
        m.weather_set(&mut world::Lgt(self.0));
    }
}

thread_local! {
    /// The run going is a new game's start dungeon, run straight from its
    /// start (0000:0C95): it ends through 0000:0C98 (`new_game_next`), not
    /// the outer loop's switch.
    static NEW_GAME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Whose run's end follows the world loop going (`Tail`).
    static TAIL: std::cell::Cell<Tail> = const { std::cell::Cell::new(Tail::Current) };
}

/// What follows the world loop going when its area ends: the run that
/// entered it (its end, `area_end`: indoors the level hooks cleared,
/// 0000:0971/09D8, a city's 2294:11F2 at 0AA3).
#[derive(Clone, Copy)]
enum Tail {
    /// The run of the area it's in (entered by the outer loop).
    Current,
    /// None: a title-screen load enters the world loop afresh, to go back
    /// to the outer loop's code after the area's run (25FD:0539, 0ED5).
    None,
    /// A load in play loads inside the world loop going: when it ends, it
    /// goes back into the run that entered it, the area's before the load.
    Run(Run),
}

/// A new game's start dungeon is the run going (`newgame::play`).
pub fn new_game_started() {
    NEW_GAME.with(|n| n.set(true));
}

/// The area the view starts in, as the outer loop would have entered it:
/// a city (5E6 0) or an interior (4). `town`: the city's entry run (its
/// buildings listed, its quests made, its song started); not where a save
/// is loaded straight after, as the title's Load enters no town first (and
/// would otherwise find the town's song marked playing, 430F).
pub fn start(engine: &mut Engine, city: bool, town: bool) -> Run {
    NEW_GAME.with(|n| n.set(false));
    TAIL.with(|t| t.set(Tail::Current));
    let area = if city { 0 } else { 4 };
    // The working slot as the original's main picks it (1194:3A51: 40h,
    // with no "STA=") and the game's own *.65 files (CITYDATA.65,
    // SPELLSG.65, ...) copied to its names (25FD:0219), in the engine's
    // files only: the game's folder is never written. Saves copy them to
    // the slot's names, loads back.
    engine.m.working_slot_main();
    engine.m.working_files_fresh(&mut engine.files);
    engine.m.mem.w8(DS, arena::trigger::var::AREA, area);
    engine.m.mem.w8(DS, arena::transit::var::ENTERED, area);
    // The area's automap (0000:3503), as entering the world loads it.
    engine.m.automap_load(&mut engine.files);
    // A town entered (0000:468B's 46B2, as `Areas::entered` runs it for
    // the areas after): its buildings listed (1008 the equipment stores,
    // ...) and its quests made. A store's greeting names it from that
    // list (%en).
    if city && town && engine.m.city_entered(&mut engine.files, &mut arena::quest::NoQuestHooks).is_none() {
        eprintln!("the town's quests gave up: {}", arena::giveup::describe());
    }
    // A level begun as an interior: its level hooks (0000:0B15, as an
    // interior's run sets them: the way out, the INF's hook, CD52 the
    // level's text, 0000:0628).
    if !city {
        let exit = if engine.m.mem.r16(DS, 0x05C8) & 0x80 == 0 { 0x4537 } else { 0x44BD };
        engine.m.level_hooks(exit);
    }
    if city { Run::City } else { Run::Interior(0xC37F) }
}

/// The area `run` was left: its run ends, the outer loop picks the next
/// and it is entered. Returns the new run, or None where the game ends
/// The level's states the stairs and an interior's entry keep: a main
/// quest dungeon's locks and triggers in STATES.nn (292B:095E, 09AC) and
/// the automap kept and loaded (0000:368E: 23F1:013C, 0000:3503).
pub struct Kept;

impl arena::hooks::StairsHooks for Kept {
    fn automap_keep(&mut self, m: &mut Machine, files: &mut Files) {
        m.states_keep(files);
    }
    fn automap_load(&mut self, m: &mut Machine, files: &mut Files) {
        m.states_load(files);
    }
    fn level_screen(&mut self, m: &mut Machine, files: &mut Files) {
        m.automap_keep(files);
        m.automap_load(files);
    }
}

/// there or the port can't follow (reported).
pub fn next(engine: &mut Engine, dir: &Path, run: Run) -> Option<Run> {
    let mut files = std::mem::take(&mut engine.files);
    let r = {
        let h = &mut Areas { dir, files: Some(&mut files), failed: false, ending: None };
        let m = &mut engine.m;
        m.world_leave(h);
        m.world_end(h);
        match TAIL.with(|t| t.replace(Tail::Current)) {
            Tail::Current => m.area_end(run, h),
            Tail::Run(r) => m.area_end(r, h),
            Tail::None => {}
        }
        // A new game's start dungeon ends at 0000:0C98 (into the town
        // of the player's province); every other run goes through 0ED5.
        let next = if run == Run::Start && NEW_GAME.with(|n| n.replace(false)) { m.new_game_next(h) } else { m.area_next(h) };
        match next {
            Run::City | Run::Interior(_) | Run::Start | Run::Wild | Run::Dungeon => {
                let name = m.area_begin(next, h);
                m.world_begin(name, h);
                (!h.failed).then_some(next)
            }
            Run::End => {
                // The game is over: the ending's text (TEXT 137A, through
                // 1194:A374) on the message line, shown while its
                // countdown runs (061A, 061E). Written where the original's
                // 1194:D459 writes it, a heap block of BB8h bytes (0:9CD2):
                // the farewell is longer than any DGROUP buffer (one at
                // 19DE ran over the slot bytes 1A6C and 1A80).
                if let (Some(ax), Some(files)) = (h.ending, h.files.as_deref_mut()) {
                    if let Some((seg, off)) = m.heap_alloc_bytes(0xBB8) {
                        m.mem.w16(DS, arena::text::var::TEXT, ax);
                        if m.text(files, &mut arena::text::NoTextHooks, 2, seg, off).is_some() {
                            let text = String::from_utf8_lossy(m.mem.cstr(seg, off)).replace('\r', "\n");
                            println!("{text}");
                            m.mem.w16(DS, 0x061A, off);
                            m.mem.w16(DS, 0x061C, seg);
                            m.mem.w16(DS, 0x061E, 0x7FFF);
                        }
                    }
                }
                eprintln!("the game has ended");
                None
            }
            other => {
                eprintln!("the next area isn't followed yet: {other:?}");
                None
            }
        }
    };
    // The world's entry reads the engine's files; the place hook's chunk
    // shift (the wilderness's first blocks) reads a copy of them (cheap:
    // their contents are shared).
    let mut copy = files.clone();
    engine.files = files;
    let next = r?;
    let h = &mut Areas { dir, files: Some(&mut copy), failed: false, ending: None };
    if engine.world_enter(h).is_none() || h.failed {
        // Kept noted (peek, not describe's take), so the caller can tell
        // the original's own fault from the port's.
        eprintln!("entering the area gave up: {}", arena::giveup::peek().map_or_else(|| "no reason noted".to_string(), |g| g.to_string()));
        return None;
    }
    eprintln!("entered {next:?} ({})", area_name(&engine.m));
    Some(next)
}


/// The area's name for the logs: its level file, or for a city (built
/// from its blocks, LEVEL_NAME "#": 0000:A26F loads no file) the place's
/// name the status box shows (0A22).
pub fn area_name(m: &arena::machine::Machine) -> String {
    let level = m.mem.cstr(DS, arena::transit::var::LEVEL_NAME);
    if level.first() == Some(&b'#') {
        format!("{} (#)", String::from_utf8_lossy(m.mem.cstr(DS, arena::status::var::PLACE_NAME)))
    } else {
        String::from_utf8_lossy(level).into_owned()
    }
}
