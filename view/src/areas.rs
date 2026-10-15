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

    fn normal_lgt(&mut self, m: &mut Machine) {
        world::lgt(m, self.dir, arena::weather::NORMAL_LGT);
    }

    /// 1194:3AE8 as the view's cities need it: the Imperial City's
    /// record (province 8) stays the Imperial City, and the terrain
    /// (record +2Eh, A50), which the original reads from the map file
    /// (1194:3D91), stays the one the view was started with.
    fn location(&mut self, m: &mut Machine, id: u16) {
        let id = if m.mem.r8(DS, RECORD + 0x24) == 8 { 0x100 } else { id };
        let terrain = m.mem.r8(DS, arena::weather::var::TERRAIN);
        if m.city_build_location(DS, RECORD, id, terrain).is_none() {
            self.failed = true;
        }
        m.mem.w8(DS, arena::weather::var::TERRAIN, terrain);
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
            self.failed = true;
        }
        m.mem.w16(DS, 0xBA54, off);
        m.mem.w16(DS, 0xBA56, seg);
    }

    /// 0000:468B's part the view needs: indoors, the level's lights
    /// from its flats (0000:478F; outdoors the night makes them).
    fn entered(&mut self, m: &mut Machine) {
        if m.mem.r16(DS, 0xC398) & 4 == 0 {
            m.flat_lights();
            m.mem.w16(DS, 0x7A46, 0);
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

    fn spawn(&mut self, m: &mut Machine, n: u16) {
        if m.spawn_actors(n, 0xA).is_none() {
            eprintln!("the townspeople met code not ported");
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
    let mut e = Engine { m: engine.m.clone(), files: engine.files.clone() };
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
        Some(arena::load::Loaded::Missing) => return Err(format!("Slot {slot} is empty in {}.", save.display())),
        _ => return Err(format!("Slot {slot} met code not ported while loading.")),
    }
    e.m.mem.w32(DS, 0xCD4A, 0);
    let run = run_of(&e.m);
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

/// The area the view starts in, as the outer loop would have entered it:
/// a city (5E6 0) or an interior (4).
pub fn start(engine: &mut Engine, city: bool) -> Run {
    let area = if city { 0 } else { 4 };
    // The working slot (1194:3AC5): the view plays from the game's own
    // *.65 files (CITYDATA.65, SPELLSG.65, ...), where the original first
    // copies them to *.00. Saves copy them to the slot's names, loads back.
    engine.m.working_slot_set(65);
    engine.m.mem.w8(DS, arena::trigger::var::AREA, area);
    engine.m.mem.w8(DS, arena::transit::var::ENTERED, area);
    // The area's automap (0000:3503), as entering the world loads it.
    engine.m.automap_load(&mut engine.files);
    if city { Run::City } else { Run::Interior(0xC37F) }
}

/// The area `run` was left: its run ends, the outer loop picks the next
/// and it is entered. Returns the new run, or None where the game ends
/// there or the port can't follow (reported).
pub fn next(engine: &mut Engine, dir: &Path, run: Run) -> Option<Run> {
    let mut files = std::mem::take(&mut engine.files);
    let r = {
        let h = &mut Areas { dir, files: Some(&mut files), failed: false, ending: None };
        let m = &mut engine.m;
        m.world_leave(h);
        m.world_end(h);
        m.area_end(run, h);
        let next = m.area_next(h);
        match next {
            Run::City | Run::Interior(_) | Run::Start | Run::Wild | Run::Dungeon => {
                let name = m.area_begin(next, h);
                m.world_begin(name, h);
                // The view has no sound: the INF's sounds (the hook CD4A,
                // 23BA:0217, which the 3D mode set) aren't loaded.
                m.mem.w32(DS, 0xCD4A, 0);
                (!h.failed).then_some(next)
            }
            Run::End => {
                // The game is over: the ending's text (TEXT 137A, through
                // 1194:A374 into a buffer) on the message line, shown
                // while its countdown runs (061A, 061E).
                if let (Some(ax), Some(files)) = (h.ending, h.files.as_deref_mut()) {
                    const BUF: u16 = 0x19DE;
                    m.mem.w16(DS, arena::text::var::TEXT, ax);
                    if m.text(files, &mut arena::text::NoTextHooks, 2, DS, BUF).is_some() {
                        let text = String::from_utf8_lossy(m.mem.cstr(DS, BUF)).replace('\r', "\n");
                        println!("{text}");
                        m.mem.w16(DS, 0x061A, BUF);
                        m.mem.w16(DS, 0x061C, DS);
                        m.mem.w16(DS, 0x061E, 0x7FFF);
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
        eprintln!("entering the area met code not ported");
        return None;
    }
    let name = String::from_utf8_lossy(engine.m.mem.cstr(DS, arena::transit::var::LEVEL_NAME)).into_owned();
    eprintln!("entered {next:?} ({name})");
    Some(next)
}

