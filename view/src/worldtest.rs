//! The view's play against the original's world frames
//! (harness/gen_worldframes.py, tests/vectors/worldframes.txt): from the
//! dungeon both sides make (`into_dungeon`), the same keys held, the
//! player's place frame by frame, and what the 2D drawing leaves on the
//! screen in play (the interface, the compass, the bars: the pixels the
//! original's frame didn't draw as the 3D view) against the view's layer.

use super::*;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/worldframes.txt");

/// The case's track: per frame, level, x, height, y, heading.
fn track(name: &str) -> Vec<[u16; 5]> {
    track_in(VECTORS, name)
}

/// `track` from the vectors at `path`.
fn track_in(path: &str, name: &str) -> Vec<[u16; 5]> {
    let text = std::fs::read_to_string(path).expect(path);
    text.lines()
        .map(|l| l.split(" | ").collect::<Vec<_>>())
        .filter(|f| f.len() == 3 && f[0] == "track" && f[1] == name)
        .map(|f| {
            let v: Vec<u16> = f[2].split(' ').map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
            [v[1], v[2], v[3], v[4], v[5]]
        })
        .collect()
}

/// The player's state as a track's (the physics' place, BA62..BA67).
fn state(a: &App) -> [u16; 5] {
    let m = &a.engine.m;
    [m.mem.r8(DS, arena::movement::var::LEVEL) as u16, m.mem.r16(DS, 0xBA62), m.mem.r16(DS, 0xBA64), m.mem.r16(DS, 0xBA66), m.mem.r16(DS, arena::movement::var::HEADING)]
}

/// The projectiles' slots (7DB8's count, 7DBC's twelve 1Ch-byte slots;
/// 7DBA, a segment, left out) and the eight active actors' entries
/// (0C57, 1Ch bytes each), as gen_worldframes.py's `objects_digest`.
fn objects(a: &App) -> u64 {
    let m = &a.engine.m;
    fnv((0x7DB8..0x7DBA).chain(0x7DBC..0x7DBC + 12 * 0x1C).chain(0x0C57..0x0C57 + 8 * 0x1C).map(|o| m.mem.r8(DS, o)))
}

/// A track entry for the frame just run, and its projectiles and actors
/// beside it (`OBJECTS`).
fn pushed(a: &App, seen: &mut Vec<[u16; 5]>) {
    seen.push(state(a));
    OBJECTS.with(|o| o.borrow_mut().push(objects(a)));
}

/// The player at the middle of a square, facing `heading`.
fn at_square(a: &mut App, row: u16, col: u16, heading: u16) {
    let m = &mut a.engine.m;
    for base in [arena::movement::var::X, 0xBA62] {
        m.mem.w16(DS, base, (col << 7) + 0x40);
        m.mem.w16(DS, base + 4, (row << 7) + 0x40);
    }
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

/// Facing `heading` at `hour` o'clock of the save's day, as
/// gen_worldframes.py's `at_hour` sets it.
fn at_hour(a: &mut App, hour: u16, heading: u16) {
    at_time(a, hour, 0, heading);
}

/// `at_hour` at `hour`:`minute`.
fn at_time(a: &mut App, hour: u16, minute: u16, heading: u16) {
    use arena::clock::var::{HOUR, MINUTE, MINUTES, SKY_TIME};
    let m = &mut a.engine.m;
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
    let t = m.mem.r32(DS, MINUTES);
    let of_day = hour as u32 * 60 + minute as u32;
    m.mem.w32(DS, MINUTES, t - t % 1440 + of_day);
    m.mem.w16(DS, HOUR, hour);
    m.mem.w16(DS, MINUTE, minute);
    // (In u32: the minutes shifted left by 10 are past a u16.)
    m.mem.w16(DS, SKY_TIME, ((of_day << 10) / 1440) as u16);
}

/// Facing `heading` at ten at night of the save's day (`at_hour`).
fn night(a: &mut App, heading: u16) {
    at_hour(a, 22, heading);
}

/// gen_worldframes.py's DISEASE_ID.
const DISEASE_ID: u16 = 10;

/// gen_worldframes.py's `midnight`: 23:59 facing north, seeded, the
/// player given disease DISEASE_ID (2EBD:0573) and its slot's count put at
/// 1, so the next minute tick passes midnight and pulses it.
fn midnight(a: &mut App) {
    use arena::effects::{slot, NSLOTS, SLOTS, SLOT_SIZE};
    at_time(a, 23, 59, 0);
    seeded(a);
    let m = &mut a.engine.m;
    m.apply_disease(DS, arena::character::PLAYER, DISEASE_ID).expect("the disease given");
    let s = (0..NSLOTS)
        .map(|i| SLOTS + i * SLOT_SIZE)
        .find(|&s| m.mem.r8(DS, s + slot::KIND) as u16 == DISEASE_ID && m.mem.r16(DS, s + slot::TARGET_OFF) == arena::character::PLAYER)
        .expect("the disease took no slot");
    m.mem.w16(DS, s, 1);
}

/// gen_worldframes.py's `dawning`: 5:59 facing north.
fn dawning(a: &mut App) {
    at_time(a, 5, 59, 0);
}

/// gen_worldframes.py's HOLIDAY_DAY (gen_screens.py's).
const HOLIDAY_DAY: u32 = 14;

/// gen_worldframes.py's `on_holiday`: noon on HOLIDAY_DAY of the save's
/// year, the frame counter (CCBC) past 3, seeded, facing north.
fn on_holiday(a: &mut App) {
    let m = &mut a.engine.m;
    let now = m.mem.r32(DS, arena::clock::var::MINUTES);
    m.mem.w32(DS, arena::clock::var::MINUTES, now / 518_400 * 518_400 + HOLIDAY_DAY * 1440 + 12 * 60);
    m.mem.w16(DS, 0xCCBC, 4);
    seeded(a);
    at_hour(a, 12, 0);
}

/// gen_worldframes.py's `store_closed`: ten at night beside the town's
/// first store's door (its entry list at 1008, the count at 138B), in the
/// first neighbouring square with no wall (empty, or only a flat), at its
/// edge toward the door and facing it; seeded.
fn store_closed(a: &mut App) {
    let m = &a.engine.m;
    assert_ne!(m.mem.r16(DS, 0x138B), 0, "no store in the town");
    let sq = m.mem.r16(m.mem.r16(DS, 0x100A), m.mem.r16(DS, 0x1008));
    let (row, col) = (sq >> 8, sq & 0xFF);
    let walls = m.mem.r16(DS, 0xD806);
    let (heading, r, c, dx, dz) = [
        (0u16, row - 1, col, 0u16, 0x30u16),
        (0x200, row + 1, col, 0, 0x30u16.wrapping_neg()),
        (0x100, row, col - 1, 0x30, 0),
        (0x300, row, col + 1, 0x30u16.wrapping_neg(), 0),
    ]
    .into_iter()
    .find(|&(_, r, c, _, _)| {
        let v = m.mem.r16(walls, (r << 8) + (c << 1));
        v == 0 || v & 0x8000 != 0
    })
    .expect("the store's door has no open square beside it");
    at_square(a, r, c, heading);
    let m = &mut a.engine.m;
    for base in [0xC3D0, 0xBA62] {
        let v = m.mem.r16(DS, base).wrapping_add(dx);
        m.mem.w16(DS, base, v);
        let v = m.mem.r16(DS, base + 4).wrapping_add(dz);
        m.mem.w16(DS, base + 4, v);
    }
    night(a, heading);
    seeded(a);
}

/// `night(heading)` in a thunderstorm with a bolt forced at the circle's
/// `angle`, as gen_worldframes.py's `storm` sets it: the weather rain
/// (BB09 1), 5C8 bit 1, the first bolt's image (D8AE) at its frame 0
/// (43C6), its place 2710h times the sine table's words (B03C, B13C),
/// the high word doubled.
fn storm(a: &mut App, heading: u16, angle: u16) {
    night(a, heading);
    let m = &mut a.engine.m;
    m.mem.w8(DS, 0xBB09, 1);
    let v = m.mem.r16(DS, 0x05C8);
    m.mem.w16(DS, 0x05C8, v | 1);
    let image = m.mem.r16(DS, 0xD8AE);
    m.mem.w16(DS, 0x43C8, image);
    m.mem.w16(DS, 0x43C6, 0);
    for (at, table) in [(0x43C2u16, 0xB03Cu16), (0x43C4, 0xB13C)] {
        let v = m.mem.r16(DS, table + 2 * angle) as i16 as i32;
        m.mem.w16(DS, at, ((((0x2710 * v) >> 16) * 2) & 0xFFFF) as u16);
    }
}

/// Facing `heading`, the location moved to province 3 at (84h + `dx`,
/// 34h + `dy`) and the sky's objects made again (the counts cleared
/// first, as 0000:027A clears them), as gen_worldframes.py's `volcanic`
/// sets it.
fn volcanic(a: &mut App, heading: u16, dx: u16, dy: u16) {
    use arena::skyobj::var::{CLOUD_COUNT, OBJECT_COUNT};
    let m = &mut a.engine.m;
    m.mem.w8(DS, 0x0A46, 3);
    m.mem.w16(DS, 0x0A48, 0x84 + dx);
    m.mem.w16(DS, 0x0A4A, 0x34 + dy);
    m.mem.w8(DS, OBJECT_COUNT, 0);
    m.mem.w8(DS, CLOUD_COUNT, 0);
    // (Made now with their files, as the generator calls 2294:08B9 in its
    // setup, not deferred to the frame's loads as `LightState` asks.)
    crate::sky_objects_made(&mut a.engine.m, &mut a.engine.files);
    a.engine.m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

/// Where a case is played: the dungeon `into_dungeon` makes, or the main
/// quest's Crypt of Hearts (`into_crypt`).
#[derive(Clone, Copy)]
enum Place {
    /// The guild the load leaves, no frame run (savetest's).
    Guild,
    Dungeon,
    Crypt,
    Street,
    /// The street with its zone's weather rain (`wet_street`), or snow.
    WetStreet,
    SnowStreet,
    Wild,
    /// The main quest dungeon 39699021.MIF, whose start sees a square's
    /// cap.
    Capped,
    /// The main quest dungeon 40516921.MIF, with fog.
    Fogged,
    /// The guild the load leaves, played on as the other places are
    /// (gen_worldframes.py's `guild`).
    Loaded,
    /// The town's first equipment store, in at its door from the street
    /// (gen_worldframes.py's `building`).
    Store,
    /// The town's first tavern, in as the store.
    Tavern,
    /// The town's first temple, in as the store.
    Temple,
    /// The town's palace, in at its first door (gen_worldframes.py's
    /// `palace`).
    Palace,
    /// The street with the guild left from the seed SEED
    /// (gen_worldframes.py's `people`): its townspeople placed the same
    /// on both sides.
    People,
}

/// From the guild to the street, then into the level `name` the way the
/// outer loop enters a named level (0000:0993): the next area 2, no
/// interior, its .MIF named at 0A36.
fn into_named(a: &mut App, name: &[u8]) {
    area_left(a);
    let m = &mut a.engine.m;
    m.mem.w8(DS, arena::trigger::var::AREA, 2);
    m.mem.w16(DS, arena::transit::var::INTERIOR, 0);
    for (i, &c) in name.iter().chain(b"\0").enumerate() {
        m.mem.w8(DS, 0x0A36 + i as u16, c);
    }
    area_left(a);
}

/// From the guild to the street, then out of the city's gate the way
/// its trigger leaves (0000:43A0): the city written as the wilderness's
/// blocks 1..4 (217B:027A, `wild_leave_city`), the next area 1.
fn into_wild(a: &mut App) {
    area_left(a);
    let e = &mut a.engine;
    e.m.wild_leave_city(&mut e.files, &mut arena::wild::PortWild);
    e.m.mem.w8(DS, arena::trigger::var::AREA, 1);
    area_left(a);
}

/// The random seed of gen_worldframes.py's game after its load from the
/// save (the harness's BIOS ticks at 21EE:0059, 0000:6FC2's seed, run on
/// through the title and the load).
const HARNESS_SEED: u32 = 0x19B7_6141;

thread_local! {
    /// Beside each track entry `pushed` keeps, the frame's projectiles and
    /// actors (`objects`: gen_worldframes.py's `objects_digest`).
    static OBJECTS: std::cell::RefCell<Vec<u64>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The state as `played`'s load left it, before the place's entry
    /// (for `the_state_measured_against_the_originals`).
    static BASE: std::cell::RefCell<Option<(Vec<u8>, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
    /// The heap's blocks as `played`'s load left them (segment, size,
    /// flags), for `the_load_measured_against_the_originals`.
    static BASE_HEAP: std::cell::RefCell<Vec<(u16, u16, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The files as `played`'s load left them (savetest's).
    static BASE_FILES: std::cell::RefCell<Option<Disk>> = const { std::cell::RefCell::new(None) };
    /// The case ends with its play's last frame (gen_worldframes.py's
    /// `ends`): no last frame run again after it.
    static ENDS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The level a stairs walk started on (gen_worldframes.py's
    /// `stairs_from`, for `level_moved`).
    static LEVEL_AT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

#[path = "savetest.rs"]
mod savetest;

#[path = "areaplaytest.rs"]
mod areaplaytest;

/// The heap's blocks from the first header (DGROUP 0157): segment, size,
/// flags.
fn heap_blocks(m: &arena::machine::Machine) -> Vec<(u16, u16, u8)> {
    let (mut off, mut seg) = (m.mem.r16(DS, 0x157), m.mem.r16(DS, 0x159));
    let mut out = Vec::new();
    while (seg | off) != 0 && out.len() < 500 {
        out.push((seg, m.mem.r16(seg, off.wrapping_add(8)), m.mem.r8(seg, off.wrapping_add(0xA))));
        (seg, off) = (m.mem.r16(seg, off.wrapping_add(2)), m.mem.r16(seg, off));
    }
    out
}

/// DGROUP and the EMS banks 0..1Fh.
fn grab(a: &App) -> (Vec<u8>, Vec<u8>) {
    // The random seed as the game's memory would hold it (the engine
    // keeps it in `rng` between its stores).
    // DGROUP with the drawing's writes on its copy as the original's,
    // drawing on the machine, has them (`Engine::page_state`).
    let mut e = arena::engine::Engine::new(a.engine.m.clone(), a.engine.files.clone());
    e.m.store_rng();
    e.page_wrote = a.engine.page_wrote.clone();
    e.page_ems_wrote = a.engine.page_ems_wrote.clone();
    (e.page_state(), (0..0x20).flat_map(|b| e.page_ems(b)).collect())
}

/// Into the place, `setup` run, then `play` driving frames and
/// collecting the track: the app as the case leaves it.
fn played(place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) -> Option<(App, Vec<[u16; 5]>)> {
    played_in(place, setup, play).map(|(a, seen, _)| (a, seen))
}

/// As `played`, and the save folder's copy the game was loaded from and
/// saves to (removed when dropped), and the driver.
fn played_in(place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) -> Option<(App, Vec<[u16; 5]>, Scratch)> {
    let dir = game_dir()?;
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return None;
    };
    // Loaded as `loaded` loads, but with no frame run in the guild: the
    // harness leaves it straight from the load, so what the world loop's
    // first frame would change is still the load's (the load's 5CA bit
    // 4000h, which 0000:48BE clears and which keeps 0000:4767 from making
    // the next level's lights; the fall's floor E8BC, still 0, which
    // drops the player a step on the street's door square).
    // With sound on (`SOUND_CASE`, gen_soundplay.py's cases): the driver
    // loaded at the program's start, its answers the original's, the
    // load's (`driver | load`) then the case's from its place's way in.
    let sound = SOUND_CASE.with(|c| c.borrow().clone());
    // With music on (`MUSIC_CASE`, gen_musicplay.py's cases): a music
    // driver loaded at the program's start, its answers the original's,
    // the driver's load's and the load's (`music | load`) then the case's.
    let music = MUSIC_CASE.with(|c| c.borrow().clone());
    let mut a = app(&dir, &Start { hour: None, load: Some((save.to_path_buf(), 1)), save_dir: Some(save.to_path_buf()), sound: sound.as_ref().map(|_| sound_replay("load")), music: music.as_ref().map(|_| music_replay("load")), ..Default::default() });
    // The random seed as the harness's game has it after its load: the
    // original seeds it from the BIOS ticks (21EE:0059 -> 0000:6FC2), so
    // a game run in the harness has the seed its clock gives.
    a.engine.m.rng.seed = HARNESS_SEED;
    a.engine.m.store_rng();
    let mut d = Driver::new(&mut a);
    clear(&mut d, &mut a);
    BASE.with(|b| *b.borrow_mut() = Some(grab(&a)));
    if let Some(name) = &sound {
        let load = a.engine.m.digital.replace(sound_replay(name)).map(|r| r.calls).unwrap_or_default();
        LOAD_CALLS.with(|c| *c.borrow_mut() = load);
    }
    if let Some(name) = &music {
        let load = a.engine.m.music.replace(music_replay(name)).map(|r| r.calls).unwrap_or_default();
        LOAD_CALLS.with(|c| *c.borrow_mut() = load);
    }
    BASE_HEAP.with(|h| *h.borrow_mut() = heap_blocks(&a.engine.m));
    BASE_FILES.with(|b| *b.borrow_mut() = Some(files_now(&a, &save)));
    match place {
        Place::Guild | Place::Loaded => {}
        // (The building's words on the way in, 191F, taken as read, as
        // gen_worldframes.py's `building` takes them.)
        Place::Store => into(&mut a, 0x1008),
        Place::Tavern => into(&mut a, 0x1AB5),
        Place::Temple => into(&mut a, 0x1004),
        Place::Palace => {
            into_palace(&mut a);
            a.engine.m.mem.w8(DS, 0x191F, 0);
        }
        Place::Dungeon => into_dungeon(&mut a),
        Place::Crypt => into_named(&mut a, b"41649375.MIF"),
        Place::Capped => into_named(&mut a, b"39699021.MIF"),
        Place::Fogged => into_named(&mut a, b"40516921.MIF"),
        Place::Street => area_left(&mut a),
        Place::People => {
            seeded(&mut a);
            area_left(&mut a)
        }
        Place::WetStreet | Place::SnowStreet => {
            // gen_worldframes.py's `weathered`: the zone's weather (A5A
            // + the zone at A51) rain (2) or snow (3) as the guild is left.
            let w = if matches!(place, Place::WetStreet) { 2 } else { 3 };
            let zone = a.engine.m.mem.r8(DS, arena::weather::var::ZONE) as u16;
            a.engine.m.mem.w8(DS, arena::weather::var::ZONE_WEATHER + zone, w);
            area_left(&mut a)
        }
        Place::Wild => into_wild(&mut a),
    }
    a.fade.clear();
    a.fade_old = None;
    setup(&mut a);
    OBJECTS.with(|o| o.borrow_mut().clear());
    let mut seen = vec![];
    pushed(&a, &mut seen);
    ENDS.with(|e| e.set(false));
    play(&mut a, &mut d, &mut seen);
    // The harness's last frame, run again with the page's writes watched
    // (gen_worldframes.py `play`): the page it leaves is the case's. (The
    // guild's case runs none, nor a game ended or back at the main menu.)
    if !matches!(place, Place::Guild) && a.engine.m.mem.r16(DS, 0x05B1) & 0x100 == 0 && a.creation.is_none() && !ENDS.with(|e| e.get()) {
        d.frame(&mut a);
    }
    Some((a, seen, save))
}

/// Into the place, `setup` run, then `play` driving frames and
/// collecting the track; both against the original's.
fn world_case(name: &str, place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) {
    let Some((mut a, seen)) = played(place, setup, play) else { return };
    let want = track(name);
    assert_eq!(seen, want, "{name}: the player's frames differ from the original's");
    check_fade_in("worldframes.txt", name, &mut a);
    let (idx, dac) = layer(&mut a);
    assert!(check_in(VECTORS, name, &idx, &dac), "{name}: differs from the original's");
}

fn frames(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, until: impl Fn(&App) -> bool) {
    for _ in 0..n {
        d.frame(a);
        pushed(a, seen);
        if until(a) {
            break;
        }
    }
}

/// The dungeon's first frames, nothing held.
#[test]
fn play_in_a_dungeon_as_the_original_draws_it() {
    world_case("playdungeon", Place::Dungeon, |_| {}, |a, d, seen| frames(a, d, seen, 3, |_| false));
}

/// The street outside the guild, its first frames, nothing held.
#[test]
fn play_in_the_street_as_the_original_draws_it() {
    let (place, setup, play) = case("playstreet").unwrap();
    world_case("playstreet", place, setup, play);
}

/// Inside the main quest's dungeon 39699021.MIF, its first frames,
/// nothing held.
#[test]
fn play_in_a_main_quest_dungeon_as_the_original_draws_it() {
    let (place, setup, play) = case("playcap").unwrap();
    world_case("playcap", place, setup, play);
}

/// Inside the main quest's dungeon 40516921.MIF, with fog, its first
/// frames, nothing held.
#[test]
fn play_in_fog_as_the_original_draws_it() {
    let (place, setup, play) = case("playfog").unwrap();
    world_case("playfog", place, setup, play);
}

/// Out of the city's gate into the wilderness, its first frames, nothing
/// held.
#[test]
fn play_in_the_wilderness_as_the_original_draws_it() {
    let (place, setup, play) = case("playwild").unwrap();
    world_case("playwild", place, setup, play);
}

/// The wilderness's top edge walked over: the window's origin a row above
/// the grid (217B:0434), its blocks loaded from the words before the grid
/// (217B:03C0), as the original goes on.
#[test]
fn over_the_wildernesss_edge_as_the_original_goes() {
    let (place, setup, play) = case("playwildedge").unwrap();
    world_case("playwildedge", place, setup, play);
}

/// The first level's down stairs walked into with the up arrow held from
/// the square south of them, facing north: the second level.
#[test]
fn down_a_dungeons_stairs_as_the_original_goes() {
    world_case(
        "playdown",
        Place::Dungeon,
        |a| at_square(a, 11, 10, 0x100),
        |a, d, seen| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, 2, |_| false);
        },
    );
}

/// The Crypt of Hearts entered, turned north and walked on into the square
/// in front of its start, whose trigger shows the INF's first text; let
/// go, a frame more: the text over the view.
#[test]
fn the_crypt_of_hearts_first_text_as_the_original_draws_it() {
    world_case(
        "playcrypt",
        Place::Crypt,
        |a| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100),
        |a, d, seen| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, 1, |_| false);
        },
    );
}

/// South of the crypt's door at row 14, column 4, facing it, the door
/// given a lock of level 5; clicked in the middle of the view two frames
/// on: not armed, the lock's words on the message line.
#[test]
fn a_locked_door_clicked_as_the_original_answers_it() {
    let (place, setup, play) = case("playlock").unwrap();
    world_case("playlock", place, setup, play);
}

/// The view's world drawn offscreen at 320 x 200 (as `shot` draws it),
/// read back: a pixel's colour, RGB.
fn rendered(a: &mut App) -> Option<Vec<[u8; 3]>> {
    let (w, h) = (hud::W as u32, hud::H as u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).ok()?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = a.assets.renderer(&device, &queue, format);
    let (sky, lights) = a.assets.drawn(&mut a.engine, w as f32 / h as f32);
    let cam = camera(&a.engine);
    let ticks = a.engine.m.bios_ticks as usize;
    let sprites = a.assets.sprites(&mut a.engine, cam, ticks, w as f32 / h as f32);
    r.set_sky(sky);
    r.set_lights(lights);
    r.set_window(interface::window(&a.engine.m));
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("world"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    r.draw(&device, &queue, &view, w, h, cam, &sprites);
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (w * h * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = buf.slice(..).get_mapped_range().to_vec();
    Some(data.chunks(4).map(|p| [p[0], p[1], p[2]]).collect())
}

/// The view's 3D world against the original's, measured: with
/// ARENA_SCREENS (gen_worldframes.py --dump's NAME.full, NAME.mask and
/// NAME.dac there) and a GPU, each case's 3D pixels (the mask's 0s) as
/// the view's renderer draws them against the original's page, by colour;
/// printed, with NAME-3d.png (the original's, the view's, and the pixels
/// that differ: grey a near colour, white a far one). A report, not a
/// test of exactness: the view's renderer is its own (polygons, not the
/// original's columns), and nothing here fails on what it measures.
#[test]
fn the_3d_view_measured_against_the_originals() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    type Play = Box<dyn FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)>;
    type Case = (&'static str, Place, Box<dyn FnOnce(&mut App)>, Play);
    let cases: Vec<Case> = vec![
        ("playdungeon", Place::Dungeon, Box::new(|_| {}), Box::new(|a, d, seen| frames(a, d, seen, 3, |_| false))),
        (
            "playdown",
            Place::Dungeon,
            Box::new(|a| at_square(a, 11, 10, 0x100)),
            Box::new(|a, d, seen| {
                a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
                frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0);
                a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
                frames(a, d, seen, 2, |_| false);
            }),
        ),
        (
            "playcrypt",
            Place::Crypt,
            Box::new(|a| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100)),
            Box::new(|a, d, seen| {
                a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
                frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0);
                a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
                frames(a, d, seen, 1, |_| false);
            }),
        ),
    ];
    for (name, place, setup, play) in cases {
        let (Ok(full), Ok(mask), Ok(odac)) = (std::fs::read(dump.join(format!("{name}.full"))), std::fs::read(dump.join(format!("{name}.mask"))), std::fs::read(dump.join(format!("{name}.dac")))) else {
            eprintln!("{name}: no dump");
            continue;
        };
        let Some((mut a, _)) = played(place, setup, play) else { return };
        let Some(px) = rendered(&mut a) else {
            eprintln!("no GPU adapter: nothing measured");
            return;
        };
        // The original's pixel in the view's colours for its index (the
        // view's palette is PAL.COL's 8 bits, the DAC's 6 its top ones),
        // where the DAC holds those colours.
        let pal = a.assets.dac();
        let same_dac = (0..256).all(|k| (0..3).all(|c| pal[k][c] >> 2 == odac[3 * k + c]));
        if !same_dac {
            eprintln!("{name}: the original's DAC isn't the view's palette; colours compared as the DAC's");
        }
        let orig = |i: usize| {
            let k = full[i] as usize;
            if same_dac {
                [pal[k][0], pal[k][1], pal[k][2]]
            } else {
                [odac[3 * k] << 2, odac[3 * k + 1] << 2, odac[3 * k + 2] << 2]
            }
        };
        let dist = |p: [u8; 3], q: [u8; 3]| p.iter().zip(&q).map(|(&x, &y)| (x as i32 - y as i32).abs()).max().unwrap_or(0);
        let (mut n, mut same, mut near) = (0usize, 0usize, 0usize);
        let mut bands = [(0usize, 0usize); 20];
        let mut out = vec![0u8; hud::W * 3 * hud::H * 3];
        for i in 0..hud::W * hud::H {
            let (x, y) = (i % hud::W, i / hud::W);
            let o = orig(i);
            let v = px[i];
            let d = dist(o, v);
            let three = mask[i] == 0;
            if three {
                n += 1;
                bands[y / 10].0 += 1;
                if d == 0 {
                    same += 1;
                    bands[y / 10].1 += 1;
                } else if d <= 16 {
                    near += 1;
                }
            }
            let diff = if !three { [0x40, 0, 0x40] } else if d == 0 { [0, 0, 0] } else if d <= 16 { [0x80, 0x80, 0x80] } else { [0xFF, 0xFF, 0xFF] };
            for (k, c) in [o, v, diff].into_iter().enumerate() {
                let at = (y * hud::W * 3 + k * hud::W + x) * 3;
                out[at..at + 3].copy_from_slice(&c);
            }
        }
        let pct = |k: usize| 100.0 * k as f64 / n.max(1) as f64;
        eprintln!("{name}: {n} 3D pixels, {:.1}% the same colour, {:.1}% near (within 16 a channel)", pct(same), pct(near));
        let rows: Vec<String> = bands.iter().enumerate().filter(|(_, b)| b.0 > 0).map(|(k, b)| format!("{}-{}: {:.0}%", k * 10, k * 10 + 9, 100.0 * b.1 as f64 / b.0 as f64)).collect();
        eprintln!("{name}: same by rows {}", rows.join(", "));
        // The view's picture moved by (dx, dy) against the original's: how
        // much more matches, for an offset in the drawing.
        let mut best = (0i32, 0i32, same);
        for dy in -3i32..=3 {
            for dx in -6i32..=6 {
                let k = (0..hud::W * hud::H)
                    .filter(|&i| {
                        let (x, y) = ((i % hud::W) as i32 + dx, (i / hud::W) as i32 + dy);
                        mask[i] == 0 && (0..hud::W as i32).contains(&x) && (0..hud::H as i32).contains(&y) && px[y as usize * hud::W + x as usize] == orig(i)
                    })
                    .count();
                if k > best.2 {
                    best = (dx, dy, k);
                }
            }
        }
        eprintln!("{name}: best with the view's moved by ({}, {}): {:.1}% the same", best.0, best.1, pct(best.2));
        let file = std::fs::File::create(dump.join(format!("{name}-3d.png"))).expect("create");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), hud::W as u32 * 3, hud::H as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().expect("png").write_image_data(&out).expect("png");
    }
}

type Play = Box<dyn FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)>;
type Case = (Place, Box<dyn FnOnce(&mut App)>, Play);

/// gen_worldframes.py's `ghost`: a ghost (type Ch, remapped: drawn
/// through the light table, 0000:953E) brought by a quest's encounter
/// (1194:89A7, one at level 1), put 100h north of the start and faced.
fn ghost(a: &mut App) {
    use arena::encounter::var::CREATURE_SLOTS;
    let e = &mut a.engine;
    e.m.quest_encounter(&mut e.files, &mut arena::encounter::NoFightHooks, 1, 0xC, 1).expect("a ghost brought");
    let (x, z) = (e.m.mem.r16(DS, 0xC3D0), e.m.mem.r16(DS, 0xC3D4));
    e.m.mem.w16(DS, CREATURE_SLOTS + 4, z.wrapping_add(0x100));
    e.m.mem.w16(DS, CREATURE_SLOTS, x);
    e.m.mem.w16(DS, arena::movement::var::HEADING, 0);
}

/// gen_worldframes.py's `brought`: a creature of type `ty` at `level`
/// brought by a quest's encounter (1194:89A7), put `ahead` north of the
/// start, the player facing it.
fn brought(a: &mut App, ty: u8, level: u8, ahead: u16) {
    use arena::encounter::var::CREATURE_SLOTS;
    let e = &mut a.engine;
    e.m.quest_encounter(&mut e.files, &mut arena::encounter::NoFightHooks, 1, ty, level).expect("a creature brought");
    let (x, z) = (e.m.mem.r16(DS, 0xC3D0), e.m.mem.r16(DS, 0xC3D4));
    e.m.mem.w16(DS, CREATURE_SLOTS + 4, z.wrapping_add(ahead));
    e.m.mem.w16(DS, CREATURE_SLOTS, x);
    e.m.mem.w16(DS, arena::movement::var::HEADING, 0);
}

/// gen_worldframes.py's `night_encounter`: a minute before ten at night,
/// the clock's minute due on the first frame (MINUTE_TICK 91 ticks back),
/// the seed one whose next roll is 0 (`ENCOUNTER_SEED`): the hour's
/// encounter check (0000:1622 -> 1194:887A) comes up and brings the
/// wilderness's night encounter as play does.
fn night_encounter(a: &mut App, heading: u16) {
    use arena::frame::var::{MINUTE_TICK, TICKS};
    at_time(a, 21, 59, heading);
    let m = &mut a.engine.m;
    let t = m.mem.r16(DS, TICKS).wrapping_sub(0x5B);
    m.mem.w16(DS, MINUTE_TICK, t);
    m.rng.seed = ENCOUNTER_SEED;
    m.store_rng();
}

/// gen_worldframes.py's `day_encounter`: `night_encounter` a minute before
/// noon, the hour's encounter roll come up by day.
fn day_encounter(a: &mut App, heading: u16) {
    use arena::frame::var::{MINUTE_TICK, TICKS};
    at_time(a, 11, 59, heading);
    let m = &mut a.engine.m;
    let t = m.mem.r16(DS, TICKS).wrapping_sub(0x5B);
    m.mem.w16(DS, MINUTE_TICK, t);
    m.rng.seed = ENCOUNTER_SEED;
    m.store_rng();
}

/// gen_worldframes.py's `before_door`: at the middle of the square south of
/// the door at `row`, `col` facing it (heading 0), or north of it facing
/// it (heading 100h) for a negative `row`; at `hour` if given.
fn before_door(a: &mut App, row: i16, col: u16, hour: Option<u16>) {
    if let Some(h) = hour {
        at_hour(a, h, 0);
    }
    if row < 0 {
        at_square(a, (-row) as u16 + 1, col, 0x100);
    } else {
        at_square(a, row as u16 - 1, col, 0);
    }
}

/// The harness's click at (`x`, `y`): down for the frame whose controls read
/// it, then up; the window's cursor 4 on (CB60), where the harness's lands.
fn door_clicked(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, x: f64, y: f64) {
    a.input(In::Cursor((x + 4.0) * 3.0 + 1.0, (y + 4.0) * 3.0 + 1.0));
    a.input(In::Button { button: MouseButton::Left, down: true });
    frames(a, d, seen, 1, |_| false);
    a.input(In::Button { button: MouseButton::Left, down: false });
}

/// gen_worldframes.py's ENCOUNTER_SEED: 99B5h (high word 0: a roll of 0)
/// stepped back once through the LCG (6D002Dh's inverse mod 2^32).
const ENCOUNTER_SEED: u32 = 0x56AA_ECA9;

/// The camp's menu's second entry picked: resting until healed.
struct UntilHealed;

impl arena::clock::ClockHooks for UntilHealed {}

impl arena::rest::RestHooks for UntilHealed {
    fn menu(&mut self, _m: &mut arena::machine::Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        1
    }
}

/// gen_worldframes.py's `resting`: at ten at night, the player's health
/// down to 1, a rest until healed begun (1194:424E).
fn resting(a: &mut App, heading: u16) {
    at_hour(a, 22, heading);
    let e = &mut a.engine;
    e.m.mem.w16(DS, arena::character::PLAYER + arena::character::field::HP, 1);
    e.m.rest_start(&mut e.files, &mut UntilHealed).expect("the rest begun");
    assert_ne!(e.m.mem.r8(DS, arena::rest::var::RESTING), 0, "not resting");
}

/// `n` frames, a box read with Space after the frame that shows it, as
/// gen_worldframes.py's script reads a box's wait (0000:8FC4); the
/// screen shown before each (`compose`, as the window shows it), for what
/// a box keeps of it.
/// The movies the frame played (the window shows them in place of its
/// frames) clicked through, a click and a few milliseconds each, no
/// game frame due meanwhile: the original plays them inside the frame
/// that started them.
fn movies_through(a: &mut App, d: &mut Driver) {
    for _ in 0..200 {
        if a.movies.is_empty() {
            return;
        }
        a.input(In::Button { button: MouseButton::Left, down: true });
        a.input(In::Button { button: MouseButton::Left, down: false });
        d.wait(a, 10);
    }
    panic!("the movies didn't end");
}

fn frames_reading(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize) {
    for _ in 0..n {
        a.compose();
        frames(a, d, seen, 1, |_| false);
        // A movie up (a dream's vision): its pages clicked through, as
        // gen_worldframes.py's `visioned` clicks them (no keys typed, so
        // none in the ring); then the box read with the one Space.
        while !a.movies.is_empty() {
            d.click(a, 160, 100, MouseButton::Left);
        }
        if a.waiting() {
            d.key(a, KeyCode::Space, Some(" "));
        }
    }
}

/// The mouse captured (a right click in the view: the window's first
/// click takes it, as the original's mouse is always the game's).
fn captured(a: &mut App) {
    a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
    a.input(In::Button { button: MouseButton::Right, down: true });
    a.input(In::Button { button: MouseButton::Right, down: false });
}

/// gen_worldframes.py's `SWINGING` for `n` frames: the right button held,
/// the mouse dragged down 60h mickeys before each frame.
fn swinging(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize) {
    a.input(In::Button { button: MouseButton::Right, down: true });
    for _ in 0..n {
        a.input(In::Motion(0.0, 96.0));
        frames(a, d, seen, 1, |_| false);
    }
    a.input(In::Button { button: MouseButton::Right, down: false });
}

/// `swinging`, but the right button let go once the surrender's box has
/// been shown (the first quest's +4 bit 20h, 2AA1:0800), as
/// gen_worldframes.py's playsurrender lets it go for the box's loop
/// (1194:59E5) and clicks it away; the frames after go on with none held.
fn swinging_to_surrender(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize) {
    a.input(In::Button { button: MouseButton::Right, down: true });
    let mut held = true;
    for _ in 0..n {
        a.input(In::Motion(0.0, 96.0));
        frames(a, d, seen, 1, |_| false);
        if held && a.engine.m.mem.r16(DS, 0x0E6E + 4) & 0x20 != 0 {
            a.input(In::Button { button: MouseButton::Right, down: false });
            held = false;
            d.click(a, 160, 100, MouseButton::Left);
        }
    }
    if held {
        a.input(In::Button { button: MouseButton::Right, down: false });
    }
}

/// The A key held for a frame (the weapon readied), let go: its press
/// runs the frame that takes it.
fn ready(a: &mut App, seen: &mut Vec<[u16; 5]>) {
    a.input(In::Key { code: KeyCode::KeyA, down: true, text: Some("a".into()) });
    pushed(a, seen);
    a.input(In::Key { code: KeyCode::KeyA, down: false, text: None });
}

/// The sheet opened by F1 inside the next frame (its screen saved to
/// SCREENSV.$$2), left by Esc, and the case ended there (`ENDS`), as
/// gen_worldframes.py's sheet cases.
fn sheet_ends(a: &mut App, d: &mut Driver) {
    a.input(In::Key { code: KeyCode::F1, down: true, text: None });
    d.key(a, KeyCode::Escape, None);
    faded(a);
    ENDS.with(|e| e.set(true));
}

/// gen_worldframes.py's `bowed`: the player's equipped weapon made a
/// short bow (type 10h), the weapon class (0D49) 6.
fn bowed(a: &mut App) {
    let m = &mut a.engine.m;
    let at = (0..40u16).map(|i| 0x0212 + 0x13 * i).find(|&at| {
        let f = m.mem.r8(DS, at + 0x0E);
        f & 0xC0 == 0xC0 && (f & 0x0C) >> 2 == 1
    });
    m.mem.w8(DS, at.expect("no weapon equipped"), 0x10);
    m.mem.w8(DS, 0x0D49, 6);
}

/// gen_worldframes.py's `boated`: a boat's record at FFE0h at the
/// player's place, boarded (2294:0DD0, `Machine::board_boat`).
fn boated(a: &mut App) {
    let m = &mut a.engine.m;
    let (x, y) = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
    m.mem.w16(DS, 0xFFE0, x);
    m.mem.w16(DS, 0xFFE4, y);
    a.engine.m.board_boat(&mut a.engine.files, 0xFFE0).expect("the boat boarded");
}

/// gen_worldframes.py's KEYS_HELD (0652): the first and the fifth key.
const KEYS_HELD: [u8; 12] = [1, 5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5, 0xC5];
/// gen_worldframes.py's EFFECT_WORDS (1405).
const EFFECT_WORDS: &[u8] = b"You feel better.\0";

/// gen_worldframes.py's `wild_edge`: the wilderness's window at the
/// grid's top row, the player just inside its second quarter, facing
/// north.
fn wild_edge(a: &mut App) {
    let m = &mut a.engine.m;
    m.mem.w16(DS, 0x060A, 0);
    for o in [0xC3D4, 0xBA66, 0x0602] {
        m.mem.w16(DS, o, 0x1008);
    }
    m.mem.w16(DS, arena::movement::var::HEADING, 0x100);
}

/// gen_worldframes.py's `picking`: `locked_door`'s door with a lock of
/// `level`, picking armed (5CA bit 0).
fn picking(a: &mut App, level: u8) {
    at_square(a, 13, 4, 0);
    let m = &mut a.engine.m;
    let n = m.mem.r8(DS, arena::movement::var::LOCK_COUNT);
    let rec = arena::movement::var::LOCKS + 3 * n as u16;
    m.mem.w16(DS, rec, 0x0E04);
    m.mem.w8(DS, rec + 2, level);
    m.mem.w8(DS, arena::movement::var::LOCK_COUNT, n + 1);
    let f = m.mem.r16(DS, 0x05CA) | 1;
    m.mem.w16(DS, 0x05CA, f);
}

/// gen_worldframes.py's `riddle_door`: gen_screens.py's `riddled` on the
/// player's square (the text at FF00h in the level's texts' bank), and a
/// trigger at square 0101h, number 1, its wall a shut door (B000h), the
/// square's entry F000h.
fn riddle_door(a: &mut App) {
    use arena::squares::var as sq;
    let m = &mut a.engine.m;
    let bank = m.mem.r8(DS, sq::TEXT_BANK) as u16;
    m.ems_map(bank);
    let seg = m.mem.r16(DS, 0x9030);
    for (i, &c) in b"What burns?\r:fire\r:flame\0`C\r".iter().enumerate() {
        m.mem.w8(seg, 0xFF00 + i as u16, c);
    }
    let here = m.mem.r16(DS, 0xCB90);
    for (at, v) in [(sq::SHOWN, 0xFF00), (sq::RIDDLE_AT, 0xFF00), (sq::ANSWERS_AT, 0xFF0C), (sq::WRONG_AT, 0xFF00), (sq::RIGHT_AT, 0), (sq::RIDDLE_A, 1), (sq::RIDDLE_B, 2), (sq::SHOWN_SQUARE, here)] {
        m.mem.w16(DS, at, v);
    }
    m.mem.w8(DS, sq::SHOWN_TIME, 0x28);
    m.mem.w8(DS, arena::riddle::var::TRIGGER_COUNT, 1);
    m.mem.w16(DS, arena::riddle::var::TRIGGERS, 0x0101);
    m.mem.w8(DS, arena::riddle::var::TRIGGERS + 2, 1);
    m.mem.w16(DS, sq::ENTRY, 0xF000);
    let walls = m.mem.r16(DS, 0xD806);
    m.mem.w16(walls, 0x0102, 0xB000);
}

/// gen_worldframes.py's `corpse_items`: the corpse (the actor at 0C57,
/// its record [+12h] in the segment [1042]) given no gold and the
/// player's first item in its first two slots; a slot of the player's
/// free.
fn corpse_items(a: &mut App) {
    let m = &mut a.engine.m;
    let seg = m.mem.r16(DS, 0x1042);
    let rec = m.mem.r16(DS, 0x0C57 + 0x12);
    m.mem.w16(seg, rec + 0x407, 0);
    for k in 0..2 * 0x13u16 {
        let b = m.mem.r8(DS, 0x0212 + k % 0x13);
        m.mem.w8(seg, rec + 0x6E + k, b);
    }
    room(a);
}

/// gen_worldframes.py's `surrendering`: a person of class 0 (type 18h)
/// brought as `brought` brings one, the first side quest (0E6E) a kind-1
/// quest at stage 4 naming that class (+2Fh 18h), 5CA bit 40h: fought
/// below half its HP it gives up and says so (2AA1:07A4, 0819).
fn surrendering(a: &mut App) {
    brought(a, 0x18, 1, 0x180);
    let m = &mut a.engine.m;
    let seg = m.mem.r16(DS, 0x1042);
    let rec = m.mem.r16(DS, 0x0C57 + 0x12);
    let f = m.mem.r16(seg, rec + 0x40D) & !0x1000;
    m.mem.w16(seg, rec + 0x40D, f);
    let c = m.mem.r8(seg, rec + 5) & 0xE0;
    m.mem.w8(seg, rec + 5, c);
    let q = 0x0E6E;
    let w = (m.mem.r16(DS, q + 4) & !0x0EC0) | 0x0840;
    m.mem.w16(DS, q + 4, w);
    m.mem.w8(DS, q + 0x2F, 0x18);
    let v = m.mem.r16(DS, 0x05CA) | 0x40;
    m.mem.w16(DS, 0x05CA, v);
}

/// gen_worldframes.py's `ruled`: the palace's ruler clicked (a map flat of
/// the palace's first kind copied to the clicked record, out of a
/// thing's reach, the seed SEED) and the click's turn run (1194:DB6C:
/// the palace's quests, 8F1E from DCE1), `script` answering it.
fn ruled(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, script: &[Act]) {
    let m = &mut a.engine.m;
    for at in [0x191F, 0x101C, 0x137E] {
        m.mem.w8(DS, at, 0);
    }
    let (x, y) = (m.mem.r16(DS, arena::movement::var::X), m.mem.r16(DS, arena::movement::var::Y));
    let rec = arena::talk::var::RECORD;
    m.mem.w16(DS, rec, x.wrapping_add(0x200));
    m.mem.w16(DS, rec + 4, y);
    let base = m.mem.r16(DS, 0xC38E) as u8;
    m.mem.w8(DS, rec + 0xC, base);
    m.mem.w16(DS, arena::talk::var::CLICKED, rec);
    seeded(a);
    a.compose();
    super::super::super::talk_turn(a);
    for step in script {
        assert!(a.pending.is_some() || !a.boxes.is_empty(), "the ruler's words ended early");
        act(a, d, step);
    }
    assert!(a.pending.is_none() && a.boxes.is_empty(), "the ruler still asking");
    pushed(a, seen);
}

/// `n` frames, a box one of them shows (a quest's words, 0000:4903)
/// answered by `script`'s next clicks, as the harness's step `script`
/// answers the box's wait inside that frame.
fn frames_answered(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, script: &[Act]) {
    let mut left = script.iter();
    for _ in 0..n {
        a.compose();
        d.frame(a);
        while a.pending.is_some() || !a.boxes.is_empty() {
            act(a, d, left.next().expect("a box the script doesn't answer"));
        }
        pushed(a, seen);
    }
}

/// gen_worldframes.py's PALACE_ACCEPT_SCRIPT (the ruler's question's Yes,
/// the offer's Yes, 136..157, its box after) and the palace's words after
/// the offer taken, which the harness's next frame shows (the step's
/// `script`) and the view's turn shows before it ends.
const PALACE_ACCEPT_TURN: [Act; 4] = [Act::Click(0xA0, 0x6E), Act::Click(0xA0, 0x90), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x60)];
/// gen_worldframes.py's QUEST_REACHED_SCRIPT: the quest's words, then the
/// tavern's menu's Exit.
const QUEST_REACHED_SCRIPT: [Act; 2] = [Act::Click(0xA0, 0x60), Act::Click(0xA0, 0xA3)];
/// gen_worldframes.py's QUEST_REWARD_SCRIPT: the reward's boxes.
const QUEST_REWARD_SCRIPT: [Act; 2] = [Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x60)];
/// The step's own clicks for a box the next frames show.
const FRAME_BOXES: [Act; 3] = [Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x60)];

/// gen_worldframes.py's `quest_under_way`: the save's palace quest (0E6E's
/// first) taken on and leading here (+24h the location, 0A4C): `reached`
/// false, its goal the town's first tavern's keeper (+14h's low word its
/// door square, 0FD2, its column halved); true, the goal reached and
/// back at its giver (+4 bit 20h); the seed SEED.
fn quest_under_way(a: &mut App, reached: bool) {
    let m = &mut a.engine.m;
    let q = 0x0E6E;
    let w = (m.mem.r16(DS, q + 4) & !0x00E0) | 0x0040 | if reached { 0x20 } else { 0 };
    m.mem.w16(DS, q + 4, w);
    let here = m.mem.r8(DS, 0x0A4C);
    m.mem.w8(DS, q + 0x24, here);
    if !reached {
        let t = m.mem.r16(DS, 0x0FD2);
        m.mem.w16(DS, q + 0x14, t & 0xFF00 | (t & 0xFF) >> 1);
    }
    seeded(a);
}

/// gen_worldframes.py's `pile_ahead`: at row 0Bh, column 2Ah facing the
/// square beyond (heading 0), its floor byte flat 7 (a pile of
/// treasure); a slot of the player's free, seeded.
fn pile_ahead(a: &mut App) {
    at_square(a, 0xB, 0x2A, 0);
    let m = &mut a.engine.m;
    let floor = m.mem.r16(DS, 0xCCD0);
    m.mem.w8(floor, 0x0C54, 7);
    room(a);
    seeded(a);
}

/// gen_worldframes.py's `chest_ahead`: facing north at row 0Bh, column 2Ah,
/// near the square's far edge, the level's locked chest (C93B) on the
/// walls plane two squares on, an Open spell waiting at its surest (0DC4
/// 7D0h), a slot free, seeded.
fn chest_ahead(a: &mut App) {
    at_square(a, 0xB, 0x2A, 0);
    let m = &mut a.engine.m;
    for base in [arena::movement::var::X, 0xBA62] {
        m.mem.w16(DS, base + 4, (0xB << 7) + 0x70);
    }
    let chest = m.mem.r8(DS, 0xC93B) as u16;
    let walls = m.mem.r16(DS, 0xD806);
    m.mem.w16(walls, 0x0D54, 0x8000 | chest);
    m.mem.w16(DS, 0x0DC4, 0x7D0);
    room(a);
    seeded(a);
}

/// gen_worldframes.py's `open_ahead(brought(1))`: `fought`, then the walls
/// plane cleared for three rows north of the player's square, a column
/// either side.
fn fought_in_the_open(a: &mut App) {
    fought(a);
    let m = &mut a.engine.m;
    let (col, row) = (m.mem.r16(DS, arena::movement::var::X) >> 7, m.mem.r16(DS, arena::movement::var::Y) >> 7);
    let walls = m.mem.r16(DS, 0xD806);
    for r in row + 1..row + 4 {
        for c in col - 1..col + 2 {
            m.mem.w16(walls, (r << 8) + (c << 1), 0);
        }
    }
}

/// gen_worldframes.py's `chest_picking`:`chest_ahead`'s chest with no Open
/// spell waiting and picking armed (5CA bit 0) instead.
fn chest_picking(a: &mut App) {
    chest_ahead(a);
    let m = &mut a.engine.m;
    m.mem.w16(DS, 0x0DC4, 0);
    let f = m.mem.r16(DS, 0x05CA);
    m.mem.w16(DS, 0x05CA, f | 1);
}

/// A step of gen_worldframes.py's with a click at the view's centre and
/// `script` for its waits, `n` frames in all: the button down for the
/// first frame, let go for the next; the frame that takes the click (the
/// loot window, 1194:DB6C) waits on the window's boxes and lists, which
/// `script` answers, and goes on.
fn clicked_with(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, script: &[Act]) {
    // The harness's mouse at (160, 100): the click 4 on (CB60), as
    // playghostclick's; the mouse let go first (after a fight's swings the
    // captured mouse clicks where they left it, not where the harness's
    // click puts it).
    a.release();
    a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
    a.input(In::Button { button: MouseButton::Left, down: true });
    frames(a, d, seen, 1, |_| false);
    a.input(In::Button { button: MouseButton::Left, down: false });
    let mut used = 1;
    loop {
        assert!(used < n, "the click asked nothing");
        // The screen as the window last showed it (the page a box or a
        // list covers: `Front::shown`), as a shown window has it.
        a.compose();
        d.frame(a);
        used += 1;
        if a.pending.is_some() || !a.boxes.is_empty() {
            break;
        }
        pushed(a, seen);
    }
    for step in script {
        assert!(a.pending.is_some() || !a.boxes.is_empty(), "the window ended early");
        act(a, d, step);
    }
    assert!(a.pending.is_none() && a.boxes.is_empty(), "the window still asking");
    pushed(a, seen);
    frames(a, d, seen, n - used, |_| false);
}

/// playfight's set-up: the creature brought, the mouse captured.
fn fought(a: &mut App) {
    brought(a, 1, 1, 0x180);
    captured(a);
}

/// playfight's frames, then its corpse clicked (gen_worldframes.py's
/// playloot: the gold's box clicked away), or (`items`, playlootitems)
/// the corpse given two items first (`corpse_items`), both taken.
fn looted(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, items: bool) {
    ready(a, seen);
    frames(a, d, seen, 30, |_| false);
    swinging(a, d, seen, 200);
    frames(a, d, seen, 20, |_| false);
    if items {
        corpse_items(a);
        frames(a, d, seen, 1, |_| false);
        clicked_with(a, d, seen, 10, &[LOOT_LINE, LOOT_LINE, Act::Click(0xA0, 0x60)]);
    } else {
        clicked_with(a, d, seen, 10, &[Act::Click(0xA0, 0x60)]);
    }
    frames(a, d, seen, 5, |_| false);
}

/// gen_worldframes.py's LOOT_LINE: the loot list's first line.
const LOOT_LINE: Act = Act::Click(0x60, 0x25);

/// The riddle's frame: shown and its answer asked (0000:0550, 292B:05A2),
/// `answer` typed and Enter, the frame going on; ten frames after.
fn riddle_played(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, answer: &str) {
    d.frame(a);
    assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Text(_))), "the riddle's answer asked");
    super::super::super::type_text(d, a, answer);
    d.key(a, KeyCode::Enter, None);
    assert!(a.pending.is_none(), "the riddle answered");
    pushed(a, seen);
    frames(a, d, seen, 10, |_| false);
}

/// gen_worldframes.py's `levitating`: spell flag 80h (05BE), its timer
/// 30 minutes.
fn levitating(a: &mut App) {
    let m = &mut a.engine.m;
    let f = m.mem.r16(DS, 0x05BE) | 0x80;
    m.mem.w16(DS, 0x05BE, f);
    let (off, seg) = (m.mem.r16(DS, 0x1020), m.mem.r16(DS, 0x1022));
    m.mem.w16(seg, off + 4 * 7, 30);
}

/// gen_worldframes.py's POTION: a potion of healing (spell FFh) in a
/// stack of one.
const POTION: [u8; 19] = [0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0x14, 0, 0, 0, 0x4C, 0xFF, 0xFF, 0xFF, 0];

/// gen_worldframes.py's `wounded_with_potion`: health 64h, POTION in the
/// inventory's 39th slot (the use list's fifth).
fn wounded_with_potion(a: &mut App) {
    use arena::character::{field, PLAYER};
    let m = &mut a.engine.m;
    m.mem.w16(DS, PLAYER + field::HP, 0x64);
    for (i, &b) in POTION.iter().enumerate() {
        m.mem.w8(DS, PLAYER + 0x6E + 0x13 * 38 + i as u16, b);
    }
}

/// gen_worldframes.py's `used`: the use list's `index`-th item used, as
/// the view's use button uses it (`panels::use_pick`).
fn used(a: &mut App, index: u16) {
    let e = &mut a.engine;
    crate::panels::use_pick(&mut e.m, &mut e.files, index, &mut crate::panels::Said { listless: true, ..Default::default() });
}

/// gen_worldframes.py's `audience(8)`: west of the palace's door at noon,
/// the town taken for the Imperial City (A46 8), the staff's line at the
/// Emperor's audience (F77 8).
fn audience8(a: &mut App) {
    at_hour(a, 12, 0);
    at_square(a, 9, 0x7C, 0x80);
    a.engine.m.mem.w8(DS, 0x0A46, 8);
    a.engine.m.mem.w8(DS, 0x0F77, 8);
}

/// A step's `do` between frames, as gen_worldframes.py's steps take it:
/// the harness reads the track at the top of each frame, and the next
/// step's first top comes after its `do`, so the last frame's line holds
/// what the `do` left.
fn done_between(a: &mut App, seen: &mut [[u16; 5]], put: impl FnOnce(&mut App)) {
    put(a);
    if let Some(l) = seen.last_mut() {
        *l = state(a);
    }
}

/// `playaudience8`'s door and audience into IMPPAL, then `stairs` of its
/// levels' down stairs walked into (gen_worldframes.py's `IMPPAL_DOWN`:
/// the square beside them, a frame, the up arrow held until the level
/// changes, two frames after).
fn imppal_down(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, stairs: usize) {
    frames(a, d, seen, 2, |_| false);
    let at = a.engine.m.mem.r8(DS, arena::trigger::var::AREA);
    door_clicked(a, d, seen, 160.0, 100.0);
    frames(a, d, seen, 3, |a| a.engine.m.mem.r8(DS, arena::trigger::var::AREA) != at);
    movies_through(a, d);
    frames(a, d, seen, 3, |_| false);
    for &(row, col, heading) in [(3, 3, 0x100), (20, 29, 0x80), (18, 47, 0x80)].iter().take(stairs) {
        done_between(a, seen, |a| at_square(a, row, col, heading));
        frames(a, d, seen, 1, |_| false);
        let level = a.engine.m.mem.r8(DS, arena::movement::var::LEVEL);
        LEVEL_AT.with(|l| l.set(level));
        a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
        frames(a, d, seen, 40, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != LEVEL_AT.with(|l| l.get()));
        a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
        frames(a, d, seen, 2, |_| false);
    }
}

/// A left click at (x, y) of the 320 x 200 screen whose release isn't
/// read: the button held as the case ends, as the harness's last click's
/// is (the screen it ends taken it at its press).
fn held_click(a: &mut App, x: i32, y: i32) {
    a.input(In::Cursor(x as f64 * 3.0 + 1.0, y as f64 * 3.0 + 1.0));
    a.input(In::Button { button: MouseButton::Left, down: true });
}

/// The window's fades played out at once: the original's screens fade
/// inside the frame that opens them, which the harness's timer doesn't
/// count; the window's queue would hold back the next frames instead.
fn faded(a: &mut App) {
    a.fade.clear();
    a.fade_old = None;
    // (A screen inside a frame ended: the frame's rest, as the tick
    // runs it once the fade is over.)
    a.screen_ended();
}

/// gen_worldframes.py's `shopping`: a slot free (`room`), each item worn
/// to half (`worn`), the seed gen_screens.py's SEED (`seeded`).
fn shopping(a: &mut App) {
    room(a);
    worn(a);
    seeded(a);
}

fn seeded(a: &mut App) {
    seeded_with(a, SEED);
}

fn seeded_with(a: &mut App, seed: u32) {
    a.engine.m.mem.w32(DS, arena::random::SEED, seed);
    a.engine.m.rng.seed = seed;
}

/// gen_worldframes.py's `stealing`: a slot free, the seed `seed`.
fn stealing(seed: u32) -> impl Fn(&mut App) {
    move |a: &mut App| {
        room(a);
        seeded_with(a, seed);
    }
}

/// gen_screens.py's STEAL_SEED and gen_worldframes.py's STEAL_QUIET: the
/// store's lockpicking roll succeeds; it fails and the guards' roll
/// doesn't call them.
const STEAL_SEED: u32 = 0x2F6E_1A36;
const STEAL_QUIET: u32 = 0x2F6E_1A48;

/// A step of gen_worldframes.py's STORE_SCRIPT or GUILD_SCRIPT: a click
/// at (x, y), or a key typed.
enum Act {
    Click(i32, i32),
    RClick(i32, i32),
    Key(KeyCode, &'static str),
}

/// `act` done as the window does it.
fn act(a: &mut App, d: &mut Driver, act: &Act) {
    match *act {
        Act::Click(x, y) => d.click(a, x, y, MouseButton::Left),
        Act::RClick(x, y) => d.click(a, x, y, MouseButton::Right),
        Act::Key(k, t) => d.key(a, k, (!t.is_empty()).then_some(t)),
    }
}

/// gen_worldframes.py's SPELLMAKER_SCRIPT: the guild's Spellmaker, the
/// spell named "zap", Damage Health added with its first field raised
/// once, bought, its box clicked away, the spellmaker's Exit, the guild's.
const SPELLMAKER_SCRIPT: [Act; 16] = [
    Act::Click(0xA0, 0x74), Act::Click(0x80, 0x34), Act::Key(KeyCode::KeyZ, "z"), Act::Key(KeyCode::KeyA, "a"), Act::Key(KeyCode::KeyP, "p"), Act::Key(KeyCode::Enter, ""),
    Act::Click(0x20, 0x4B), Act::Click(100, 74), Act::Click(100, 74), Act::Click(100, 38), Act::Click(142, 71), Act::Click(230, 143),
    Act::Click(0xD8, 0xC0), Act::Click(0xA0, 0x60), Act::Click(0x128, 0xC0), Act::Click(0xA0, 0xA3),
];

/// gen_worldframes.py's ACCEPT and MORE.
const ACCEPT: Act = Act::Click(110, 133);
const MORE: Act = Act::Click(297, 172);

/// gen_worldframes.py's STORE_SCRIPT: a weapon bought, an item sold, one
/// repaired, the store left.
const STORE_SCRIPT: [Act; 17] = [
    Act::Click(0xA0, 0x43), Act::Click(0xA0, 0x65), Act::Click(0x70, 0x26), Act::Click(0x70, 0x26), MORE, ACCEPT, ACCEPT,
    Act::Click(0xA0, 0x5B), Act::Click(0x80, 0x25), Act::Click(0xA0, 0x69), ACCEPT, ACCEPT,
    Act::Click(0xA0, 0x73), Act::Click(0x70, 0x26), ACCEPT, ACCEPT,
    Act::Click(0xA0, 0xA3),
];

/// gen_worldframes.py's OFFER: the haggle's second button.
const OFFER: Act = Act::Click(160, 133);

/// gen_worldframes.py's STORE_HAGGLE_SCRIPT: an armour bought for 14 (5
/// refused, 13 countered), the store left.
const STORE_HAGGLE_SCRIPT: [Act; 16] = [
    Act::Click(0xA0, 0x43), Act::Click(0xA0, 0x7D), Act::Click(0x70, 0x26), Act::Click(0x70, 0x26), MORE, OFFER,
    Act::Key(KeyCode::Digit5, "5"), Act::Key(KeyCode::Enter, ""), MORE,
    Act::Key(KeyCode::Digit1, "1"), Act::Key(KeyCode::Digit3, "3"), Act::Key(KeyCode::Enter, ""), MORE, ACCEPT, ACCEPT,
    Act::Click(0xA0, 0xA3),
];

/// gen_worldframes.py's STORE_SELL_HAGGLE_SCRIPT: an item sold for 16 (25
/// refused, 17 countered), the store left.
const STORE_SELL_HAGGLE_SCRIPT: [Act; 15] = [
    Act::Click(0xA0, 0x5B), Act::Click(0x80, 0x25), Act::Click(0xA0, 0x69), OFFER,
    Act::Key(KeyCode::Digit2, "2"), Act::Key(KeyCode::Digit5, "5"), Act::Key(KeyCode::Enter, ""), MORE,
    Act::Key(KeyCode::Digit1, "1"), Act::Key(KeyCode::Digit7, "7"), Act::Key(KeyCode::Enter, ""), MORE, ACCEPT, ACCEPT,
    Act::Click(0xA0, 0xA3),
];

/// gen_worldframes.py's STEAL_SCRIPT: Steal, its message clicked away,
/// Exit.
const STEAL_SCRIPT: [Act; 3] = [Act::Click(0xA0, 0x8B), Act::Click(160, 100), Act::Click(0xA0, 0xA3)];

/// gen_worldframes.py's STEAL_CAUGHT_SCRIPT: Steal and its message; the
/// guards called, the store left.
const STEAL_CAUGHT_SCRIPT: [Act; 2] = [Act::Click(0xA0, 0x8B), Act::Click(160, 100)];

/// gen_worldframes.py's `informant`: the main quest's informant in this
/// tavern, offered but not met, ten days and 100 gold; the seed SEED.
fn informant(a: &mut App) {
    let m = &mut a.engine.m;
    let w = m.mem.r16(DS, arena::sidequest::var::TALKER);
    m.mem.w8(DS, 0x0FDD, 0x0F);
    m.mem.w16(DS, 0x0FDE, w & 0xFF00 | (w & 0xFF) >> 1);
    let province = m.mem.r8(DS, 0x0A46);
    m.mem.w8(DS, 0x0FE3, province);
    m.mem.w8(DS, 0x0FE6, 2);
    let now = m.mem.r32(DS, 0x05CC);
    m.mem.w32(DS, 0x0FE9, now);
    m.mem.w8(DS, 0x0FED, 10);
    m.mem.w16(DS, 0x0FEE, 100);
    seeded(a);
}

/// gen_worldframes.py's INFORMANT_SCRIPT: the informant's words paged, his
/// price accepted, the rest paged to the tavern's menu, its Exit.
const INFORMANT_SCRIPT: [Act; 8] = [MORE, MORE, ACCEPT, MORE, MORE, MORE, MORE, Act::Click(0xA0, 0xA3)];

/// gen_worldframes.py's `informant_owed`: `informant`, his price agreed
/// before (0FE6 bit 0): paid for (1194:D019) rather than haggled with.
fn informant_owed(a: &mut App) {
    informant(a);
    a.engine.m.mem.w8(DS, 0x0FE6, 3);
}

/// gen_worldframes.py's INFORMANT_PAID_SCRIPT: his offer paged once, Yes
/// on YESNO.IMG's first button (1194:4E88 kind 1, 6001), his words after
/// paged, the tavern's menu's Exit.
const INFORMANT_PAID_SCRIPT: [Act; 7] = [MORE, ACCEPT, MORE, MORE, MORE, MORE, Act::Click(0xA0, 0xA3)];

/// gen_worldframes.py's GUILD_BAR_SCRIPT: as GUILD_SCRIPT, the potions'
/// list scrolled by its bar (71, 80) first, its top line then clicked
/// twice (marked, taken).
const GUILD_BAR_SCRIPT: [Act; 10] = [
    Act::Click(0xA0, 0x46), Act::Click(0xA0, 0x59), Act::Click(71, 80), Act::Click(0x70, 0x26), Act::Click(0x70, 0x26), Act::Key(KeyCode::Digit1, "1"), Act::Key(KeyCode::Enter, ""), ACCEPT, ACCEPT,
    Act::Click(0xA0, 0xA3),
];

/// gen_worldframes.py's GUILD_SCRIPT: a potion bought, the guild left.
const GUILD_SCRIPT: [Act; 8] = [
    Act::Click(0xA0, 0x46), Act::Click(0xA0, 0x59), Act::Click(0x70, 0x26), Act::Key(KeyCode::Digit1, "1"), Act::Key(KeyCode::Enter, ""), ACCEPT, ACCEPT,
    Act::Click(0xA0, 0xA3),
];

/// gen_worldframes.py's TEMPLE_HEAL_SCRIPT: Heal, its price accepted,
/// "thou art healed" clicked away, Exit.
const TEMPLE_HEAL_SCRIPT: [Act; 4] = [Act::Click(0xA0, 0x7F), ACCEPT, Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x97)];

/// gen_worldframes.py's TEMPLE_CURE_SCRIPT: Cure, the list's one line
/// (taken: paid and cured), Exit.
const TEMPLE_CURE_SCRIPT: [Act; 3] = [Act::Click(0xA0, 0x67), Act::Click(0x60, 0x32), Act::Click(0xA0, 0x97)];

/// gen_worldframes.py's TEMPLE_BLESS_SCRIPT: Bless, 100 typed, the
/// blessing clicked away, Exit.
const TEMPLE_BLESS_SCRIPT: [Act; 7] = [
    Act::Click(0xA0, 0x4F), Act::Key(KeyCode::Digit1, "1"), Act::Key(KeyCode::Digit0, "0"), Act::Key(KeyCode::Digit0, "0"), Act::Key(KeyCode::Enter, ""),
    Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x97),
];

/// gen_worldframes.py's TAVERN_DRINK_SCRIPT: Drinks, the first line, its
/// words clicked away, Exit.
const TAVERN_DRINK_SCRIPT: [Act; 4] = [Act::Click(0xA0, 0x43), Act::Click(0x70, 0x26), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0xA3)];

/// gen_worldframes.py's TAVERN_ROOM_SCRIPT: Get a Room, the Single, a day
/// typed, the price accepted and the deal confirmed, then Exit.
const TAVERN_ROOM_SCRIPT: [Act; 7] = [
    Act::Click(0xA0, 0x5B), Act::Click(0x80, 0x31), Act::Key(KeyCode::Digit1, "1"), Act::Key(KeyCode::Enter, ""),
    Act::Click(0x70, 0x85), Act::Click(0x70, 0x85), Act::Click(0xA0, 0x87),
];

/// gen_worldframes.py's `levelled`: a level's worth of experience for
/// the save's level 22 (LEVEL_XP), 5CA bit 100h set as a gain sets it.
fn levelled(a: &mut App) {
    let m = &mut a.engine.m;
    let xp = m.mem.r32(DS, 0x05AD).wrapping_add(9_750_000);
    m.mem.w32(DS, 0x05AD, xp);
    let f = m.mem.r16(DS, 0x05CA) | 0x100;
    m.mem.w16(DS, 0x05CA, f);
}

/// gen_worldframes.py's `minute_due`: the clock's last minute (884A) 90
/// ticks back, so the next frame's minute tick runs.
/// gen_worldframes.py's `guards_called`: the guards' countdown (680F) at
/// 1 and their hook at 67EA 0000:3F3B, as a crime seen leaves them.
fn guards_called(a: &mut App) {
    let m = &mut a.engine.m;
    m.mem.w8(DS, 0x680F, 1);
    m.mem.w16(DS, 0x67EA, 0x3F3B);
    m.mem.w16(DS, 0x67EC, 0);
}

/// gen_worldframes.py's `known`: spell number `spell` added to the end
/// of the player's book (+366h, +367h) and its line the spell picked
/// last (5CAE), which Shift+C casts.
fn known(a: &mut App, spell: u8) {
    let m = &mut a.engine.m;
    let n = m.mem.r8(DS, 0x01A4 + 0x366);
    m.mem.w8(DS, 0x01A4 + 0x367 + n as u16, spell);
    m.mem.w8(DS, 0x01A4 + 0x366, n + 1);
    m.mem.w8(DS, 0x5CAE, n);
}

/// gen_worldframes.py's spell numbers (3276:0000's records).
const LIGHT: u8 = 2;
const OPEN: u8 = 9;
const HEAL: u8 = 13;
const SILENCE: u8 = 31;
const FORTIFY: u8 = 5;
const RESIST_FIRE: u8 = 14;
const WYVERNS_STING: u8 = 20;
const CURE_POISON: u8 = 21;
const INVISIBLE: u8 = 24;
const MEDUSAS_GAZE: u8 = 42;
const MIRROR: u8 = 44;

/// gen_worldframes.py's `poisoned`: Wyvern's Sting's effects on the
/// player (2CDD:000C, the caster's level 10) from POISON_SEED, its slot
/// counted to its pulse.
fn poisoned(a: &mut App) {
    use arena::effects::{slot, NSLOTS, SLOTS, SLOT_SIZE};
    seeded_with(a, 0xF2A7_4DE4);
    let m = &mut a.engine.m;
    let (gs, spell) = m.spell_record(WYVERNS_STING);
    m.spell_effects(gs, spell, DS, arena::character::PLAYER, 10, &mut arena::strike::NoKillHooks).expect("the poison given");
    let s = (0..NSLOTS)
        .map(|i| SLOTS + i * SLOT_SIZE)
        .rev()
        .find(|&s| m.mem.r8(DS, s + slot::STATE) != 0xFF && m.mem.r16(DS, s + slot::TARGET_OFF) == arena::character::PLAYER)
        .expect("the poison took no slot");
    m.mem.w16(DS, s, 1);
}

/// gen_worldframes.py's spell cases: two frames, Shift+C (2542:056A: the
/// spell picked last cast without the list, the frame that took the key
/// counted), the keys let go and `n` frames, the first of them with the
/// target clicked at the view's centre when `click` (the harness's click
/// puts the mouse at 160, 100: the window's click is the cursor's point,
/// 4 down and right of it), then `after` frames.
fn quick_cast(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, click: bool, n: usize, after: usize) {
    frames(a, d, seen, 2, |_| false);
    a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
    a.input(In::Key { code: KeyCode::KeyC, down: true, text: Some("C".into()) });
    assert!(a.panel.is_none(), "no list with a Shift");
    pushed(a, seen);
    a.input(In::Key { code: KeyCode::KeyC, down: false, text: None });
    a.input(In::Key { code: KeyCode::ShiftLeft, down: false, text: None });
    if click {
        a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
        a.input(In::Button { button: MouseButton::Left, down: true });
        frames(a, d, seen, 1, |_| false);
        a.input(In::Button { button: MouseButton::Left, down: false });
        frames(a, d, seen, n - 1, |_| false);
    } else {
        frames(a, d, seen, n, |_| false);
    }
    frames(a, d, seen, after, |_| false);
}

/// gen_worldframes.py's `light_ending`: Light cast (29CF:071F), then the
/// timer of each spell flag it left set (05BE, the table at [1020]) put
/// at 1 minute.
fn light_ending(a: &mut App) {
    let e = &mut a.engine;
    e.m.cast_spell(&mut e.files, &mut arena::cast::NoCastHooks, LIGHT).expect("Light cast");
    assert_eq!(e.m.mem.r16(DS, arena::cast::var::PENDING), 0, "Light waiting for a target");
    let flags = e.m.mem.r16(DS, 0x05BE);
    assert_ne!(flags, 0, "Light set no flag");
    let (off, seg) = (e.m.mem.r16(DS, 0x1020), e.m.mem.r16(DS, 0x1022));
    for i in 0..16 {
        if flags & (1 << i) != 0 {
            e.m.mem.w16(seg, off + 4 * i, 1);
        }
    }
}

fn minute_due(a: &mut App) {
    let m = &mut a.engine.m;
    let t = m.mem.r16(DS, 0x0151).wrapping_sub(0x5A);
    m.mem.w16(DS, 0x884A, t);
}

/// The frame whose minute tick gains the level, gen_worldframes.py's
/// LEVEL_SCRIPT answering it: the notice clicked away, on the attribute
/// screen each line picked and its arrows' upper half clicked six times,
/// then the button; the turn's frame counted as the case's next.
fn levelled_frame(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>) {
    d.frame(a);
    assert!(!a.boxes.is_empty(), "the level's notice");
    d.click(a, 0xA0, 0x60, MouseButton::Left);
    assert!(a.creation.is_some(), "the attribute screen up for the level's points");
    for r in 0..8 {
        d.click(a, 10, 54 + 8 * r, MouseButton::Left);
        for _ in 0..6 {
            d.click(a, 40, 0x32 + 8 * r, MouseButton::Left);
        }
    }
    d.click(a, 10, 175, MouseButton::Left);
    assert!(a.creation.is_none() && a.pending.is_none() && a.boxes.is_empty(), "the level-up still asking");
    pushed(a, seen);
}

/// gen_worldframes.py's `temple_hurt`: the player at half health, seeded.
fn temple_hurt(a: &mut App) {
    let m = &mut a.engine.m;
    let max = m.mem.r16(DS, 0x01FF);
    m.mem.w16(DS, 0x01FD, max / 2);
    seeded(a);
}

/// gen_worldframes.py's `diseased`: the player's afflictions (05B1) a
/// disease only, seeded.
fn diseased(a: &mut App) {
    a.engine.m.mem.w8(DS, 0x05B1, 1);
    seeded(a);
}

/// gen_worldframes.py's `serviced`: the building's services (869F's
/// `kind`) run as its keeper's click runs them, between frames, `script`
/// answering them; the turn's frame after them counted as the case's next.
fn serviced(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, kind: u8, script: &[Act]) {
    // The screen as the window last showed it (the page the shop's screen
    // is drawn over: `Front::shown`), as a shown window has it.
    a.compose();
    super::super::super::shop_turn(a, kind);
    for (i, step) in script.iter().enumerate() {
        assert!(a.pending.is_some() || !a.boxes.is_empty(), "the services ended early, before step {i}");
        act(a, d, step);
    }
    assert!(a.pending.is_none() && a.boxes.is_empty(), "the services still asking");
    pushed(a, seen);
}

/// gen_worldframes.py's TALK_WHERE_SCRIPT: "Where is", its first line,
/// the sorted list's first line, the way's box clicked away, then the
/// menu's Goodbye.
const TALK_WHERE_SCRIPT: [Act; 5] = [Act::Click(0xA0, 0x65), Act::Click(100, 23), Act::Click(100, 23), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x95)];

/// gen_worldframes.py's TALK_KEY_SCRIPT: TALK_WHERE_SCRIPT with the talk's
/// menu answered by its key, H, and left by the right button.
const TALK_KEY_SCRIPT: [Act; 5] = [Act::Key(KeyCode::KeyH, "h"), Act::Click(100, 23), Act::Click(100, 23), Act::Click(0xA0, 0x60), Act::RClick(0xA0, 0x95)];

/// gen_worldframes.py's TALK_QUEST_SCRIPT: "Where is" with the quest's
/// place tenth, the down arrow (35h, 65h) scrolling it into the last
/// row, that row clicked twice (marked, taken: 1194:D593, the quest's
/// person's one box clicked away), then the menu's Goodbye.
const TALK_QUEST_SCRIPT: [Act; 6] = [Act::Click(0xA0, 0x65), Act::Click(58, 105), Act::Click(100, 95), Act::Click(100, 95), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x95)];

/// gen_worldframes.py's TALK_BAR_SCRIPT: "Where is" as TALK_QUEST_SCRIPT,
/// its lines scrolled by the bar (59, 80) and then the up arrow (58, 10),
/// a line clicked twice (marked, taken), its box clicked away, Goodbye:
/// the list's loop (3159:06A0) run by the port over the view's reads.
const TALK_BAR_SCRIPT: [Act; 7] = [Act::Click(0xA0, 0x65), Act::Click(59, 80), Act::Click(58, 10), Act::Click(100, 50), Act::Click(100, 50), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x95)];

/// gen_worldframes.py's TALK_PALACE_SCRIPT ("Where is", the palace,
/// the way's box, Goodbye), the list's fourth line taken by its number
/// (the original's two clicks on it: the first marks it).
const TALK_PALACE_SCRIPT: [Act; 4] = [Act::Click(0xA0, 0x65), Act::Key(KeyCode::Digit4, "4"), Act::Click(0xA0, 0x60), Act::Click(0xA0, 0x95)];

/// gen_worldframes.py's `dreaming`: the staff's line at stage 3 with its
/// dream seen (F76 set, F77 3, F78 and F7D clear), so "Where is" lists
/// the stage's place tenth (1194:E003).
fn dreaming(a: &mut App) {
    let m = &mut a.engine.m;
    m.mem.w8(DS, 0x0F76, 1);
    m.mem.w8(DS, 0x0F77, 3);
    m.mem.w8(DS, 0x0F78, 0);
    m.mem.w8(DS, 0x0F7D, 0);
}

/// gen_worldframes.py's `talked`: the first townsperson clicked with the
/// player on their square (gen_screens.py's `beside`: the timer 0151 at
/// TALK_TICKS), the seed SEED, and the click run between frames
/// (`talk_turn`), `script` answering it.
fn talked(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, script: &[Act]) {
    let m = &mut a.engine.m;
    let (x, y) = (m.mem.r16(DS, 0x0AB3), m.mem.r16(DS, 0x0AB7));
    m.mem.w16(DS, arena::movement::var::X, x);
    m.mem.w16(DS, arena::movement::var::Y, y);
    m.mem.w16(DS, arena::talk::var::CLICKED, 0x0AB3);
    m.mem.w16(DS, 0x0151, 1);
    seeded(a);
    a.compose();
    super::super::super::talk_turn(a);
    for step in script {
        assert!(a.pending.is_some() || !a.boxes.is_empty(), "the talk ended early");
        act(a, d, step);
    }
    assert!(a.pending.is_none() && a.boxes.is_empty(), "the talk still asking");
    pushed(a, seen);
}

/// The cases' set-ups and plays, by name (as the tests above play them).
fn case(name: &str) -> Option<Case> {
    let held = |n: usize, until: fn(&App) -> bool, after: usize| -> Play {
        Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, n, until);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, after, |_| false);
        })
    };
    Some(match name {
        "playdungeon" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playghost" => (Place::Dungeon, Box::new(ghost), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playghostclick" => (Place::Dungeon, Box::new(ghost), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            // The harness's click: the button seen down by its step's first
            // frame's controls, let go (the click, 2826:0D85) by the next's;
            // the window's click is taken by the input step of the frame
            // after the one it came before (`TurnIn::click`).
            frames(a, d, seen, 1, |_| false);
            // The harness's mouse at (160, 100): the click 4 on (CB60).
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 10, |a| a.engine.m.mem.r16(DS, 0x061E) != 0);
            frames(a, d, seen, 1, |_| false);
        })),
        "playdown" => (Place::Dungeon, Box::new(|a: &mut App| at_square(a, 11, 10, 0x100)), held(20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0, 2)),
        "playsky" => (Place::Street, Box::new(|a: &mut App| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playnight" => (Place::Street, Box::new(|a: &mut App| night(a, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playmidnight" => (Place::Street, Box::new(midnight), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            minute_due(a);
            frames(a, d, seen, 6, |_| false);
        })),
        "playdawn" => (Place::Street, Box::new(dawning), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            for _ in 0..3 {
                minute_due(a);
                frames(a, d, seen, 2, |_| false);
            }
        })),
        "playholiday" => (Place::Street, Box::new(on_holiday), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            // The first frame's holiday message (0000:49D9), clicked away
            // inside it; the turn's frame counted as the case's first.
            d.frame(a);
            assert!(!a.boxes.is_empty(), "the holiday's message");
            d.click(a, 0xA0, 0x60, MouseButton::Left);
            assert!(a.boxes.is_empty() && a.pending.is_none(), "the holiday's message still up");
            pushed(a, seen);
            frames(a, d, seen, 2, |_| false);
        })),
        "playstoreclosed" => (Place::Street, Box::new(store_closed), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The door clicked at the view's middle, 4 on as the harness's
            // click lands (CB60).
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 5, |_| false);
        })),
        "playbolt" => (Place::Street, Box::new(|a: &mut App| storm(a, 0, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playfog" => (Place::Fogged, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playfogfirst" => (Place::Fogged, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 0, |_| false))),
        "playcap" => (Place::Capped, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playrain" => (Place::WetStreet, Box::new(|a: &mut App| at_square(a, 89, 41, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playsnow" => (Place::SnowStreet, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playwildedge" => (Place::Wild, Box::new(wild_edge), held(12, |_| false, 1)),
        "playwild" => (Place::Wild, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playvolcano" => (Place::Wild, Box::new(|a: &mut App| volcanic(a, 0x100, 0, 0x20)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playvolcanofar" => (Place::Wild, Box::new(|a: &mut App| volcanic(a, 0x100, 0, 0xA0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playstreet" => (Place::Street, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playguards" => (Place::Street, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            guards_called(a);
            frames(a, d, seen, 4, |_| false);
        })),
        "playsurrender" => (Place::Dungeon, Box::new(|a: &mut App| {
            surrendering(a);
            captured(a);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            ready(a, seen);
            frames(a, d, seen, 30, |_| false);
            swinging_to_surrender(a, d, seen, 200);
            frames(a, d, seen, 20, |_| false);
        })),
        // gen_worldframes.py's `surrender_spell`: the quest target at 201
        // of 400 HP, Wizard's Fire cast at it as playcast casts; its box
        // clicked away once up.
        "playsurrenderspell" => (Place::Dungeon, Box::new(|a: &mut App| {
            surrendering(a);
            let m = &mut a.engine.m;
            let seg = m.mem.r16(DS, 0x1042);
            let rec = m.mem.r16(DS, 0x0C57 + 0x12);
            m.mem.w16(seg, rec + 0x5B, 400);
            m.mem.w16(seg, rec + 0x59, 201);
            let e = &mut a.engine;
            e.m.cast_spell(&mut e.files, &mut arena::cast::NoCastHooks, 6).expect("the spell cast");
            assert_ne!(e.m.mem.r16(DS, arena::cast::var::PENDING), 0, "the spell not waiting for its target");
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The harness's mouse at (160, 100), its click 4 past it
            // (CB60): the window's click there, the mouse 4 up and left of
            // it, which the aim reads (29CF:00D2).
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 9, |_| false);
            let mut up = false;
            for _ in 0..120 {
                frames(a, d, seen, 1, |_| false);
                if !up && a.engine.m.mem.r16(DS, 0x0E6E + 4) & 0x20 != 0 {
                    up = true;
                    d.click(a, 164, 104, MouseButton::Left);
                }
            }
        })),
        "playfight" => (Place::Dungeon, Box::new(|a: &mut App| {
            brought(a, 1, 1, 0x180);
            captured(a);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            ready(a, seen);
            frames(a, d, seen, 30, |_| false);
            swinging(a, d, seen, 200);
            frames(a, d, seen, 20, |_| false);
        })),
        "playcast" => (Place::Dungeon, Box::new(|a: &mut App| {
            // gen_worldframes.py's `casting`: the creature four squares
            // off, Wizard's Fire (the spellbook's seventh) cast at it.
            brought(a, 1, 1, 0x200);
            let e = &mut a.engine;
            e.m.cast_spell(&mut e.files, &mut arena::cast::NoCastHooks, 6).expect("the spell cast");
            assert_ne!(e.m.mem.r16(DS, arena::cast::var::PENDING), 0, "the spell not waiting for its target");
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // (The aim reads the mouse, 4 up and left of the window's
            // click: there at the harness's (160, 100).)
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 9, |_| false);
            frames(a, d, seen, 120, |_| false);
        })),
        "playcreaturecast" => (Place::Dungeon, Box::new(|a: &mut App| brought(a, 0xB, 1, 0x300)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 400, |_| false))),
        "playstairsfight" => (Place::Dungeon, Box::new(|a: &mut App| {
            // gen_worldframes.py's `stairs_fight`: south of the down
            // stairs facing them, a creature two squares behind.
            at_square(a, 11, 10, 0x100);
            brought(a, 1, 1, 0x100);
            a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 40, |_| false);
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, 24, |_| false);
        })),
        "playcamp" => (Place::Wild, Box::new(|a: &mut App| at_hour(a, 12, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // Camping (R, 2542:01E6): its menu answered C (camp for a
            // while), the hours typed (2, Enter: 1194:46BF); the rest's
            // frames count them down to "done", its box read with Space.
            // (R asks for the camp before the next frame, whose turn waits
            // on the menu and the hours; answered, that frame goes on.)
            a.compose();
            d.key(a, KeyCode::KeyR, Some("r"));
            assert!(a.pending.is_some(), "R asks how to rest");
            d.key(a, KeyCode::KeyC, Some("c"));
            d.key(a, KeyCode::Digit2, Some("2"));
            d.key(a, KeyCode::Enter, None);
            assert!(a.pending.is_none(), "the hours answered");
            pushed(a, seen);
            frames_reading(a, d, seen, 12);
        })),
        // `playcamp` with the staff's dream due (F76 0, F77 3, F7A 0): the
        // count's end makes it pending and shows it (1194:4606, D340, D3AF:
        // text 511h through D45D, its pages clicked through).
        "playdream" => (Place::Wild, Box::new(|a: &mut App| {
            at_hour(a, 12, 0x100);
            let m = &mut a.engine.m;
            m.mem.w8(DS, 0x0F76, 0);
            m.mem.w8(DS, 0x0F77, 3);
            m.mem.w8(DS, 0x0F7A, 0);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.compose();
            d.key(a, KeyCode::KeyR, Some("r"));
            assert!(a.pending.is_some(), "R asks how to rest");
            d.key(a, KeyCode::KeyC, Some("c"));
            d.key(a, KeyCode::Digit2, Some("2"));
            d.key(a, KeyCode::Enter, None);
            assert!(a.pending.is_none(), "the hours answered");
            pushed(a, seen);
            frames_reading(a, d, seen, 30);
        })),
        "playquickcast" => (Place::Dungeon, Box::new(|a: &mut App| {
            // gen_worldframes.py's `last_spell`: Wizard's Fire (the book's
            // seventh) the spell picked last.
            a.engine.m.mem.w8(DS, 0x5CAE, 6);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // Shift+C (2542:056A): the last spell again without the list,
            // its target clicked at the view's centre.
            a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
            a.input(In::Key { code: KeyCode::KeyC, down: true, text: Some("C".into()) });
            assert!(a.panel.is_none(), "no list with a Shift");
            pushed(a, seen);
            a.input(In::Key { code: KeyCode::KeyC, down: false, text: None });
            a.input(In::Key { code: KeyCode::ShiftLeft, down: false, text: None });
            // (The harness's click puts the mouse at 160, 100; the window's
            // click is the cursor's point, the mouse 4 up and left of it.)
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 9, |_| false);
            frames(a, d, seen, 40, |_| false);
        })),
        // gen_worldframes.py's spell cases (`known`, `quick_cast`).
        "playheal" => (Place::Dungeon, Box::new(|a: &mut App| {
            a.engine.m.mem.w16(DS, 0x01A4 + 0x59, 0x40);
            known(a, HEAL);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 20, 0))),
        "playlight" => (Place::Dungeon, Box::new(|a: &mut App| known(a, LIGHT)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 40, 0))),
        "playopen" => (Place::Crypt, Box::new(|a: &mut App| {
            // `locked_door`'s door, a lock of level 5.
            at_square(a, 13, 4, 0);
            let m = &mut a.engine.m;
            let n = m.mem.r8(DS, arena::movement::var::LOCK_COUNT);
            let rec = arena::movement::var::LOCKS + 3 * n as u16;
            m.mem.w16(DS, rec, 0x0E04);
            m.mem.w8(DS, rec + 2, 5);
            m.mem.w8(DS, arena::movement::var::LOCK_COUNT, n + 1);
            known(a, OPEN);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, true, 10, 20))),
        "playsilence" => (Place::Dungeon, Box::new(|a: &mut App| {
            brought(a, 1, 1, 0x100);
            known(a, SILENCE);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, true, 10, 20))),
        "playlightout" => (Place::Dungeon, Box::new(light_ending), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            minute_due(a);
            frames(a, d, seen, 10, |_| false);
        })),
        // gen_worldframes.py's spells-2 cases.
        "playresist" => (Place::Dungeon, Box::new(|a: &mut App| known(a, RESIST_FIRE)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 20, 0))),
        "playfortify" => (Place::Dungeon, Box::new(|a: &mut App| known(a, FORTIFY)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 20, 0))),
        "playinvisible" => (Place::Dungeon, Box::new(|a: &mut App| known(a, INVISIBLE)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 40, 0))),
        "playreflect" => (Place::Dungeon, Box::new(|a: &mut App| known(a, MIRROR)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 20, 0))),
        "playparalyse" => (Place::Dungeon, Box::new(|a: &mut App| {
            brought(a, 1, 1, 0x200);
            known(a, MEDUSAS_GAZE);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, true, 10, 40))),
        "playcurepoison" => (Place::Dungeon, Box::new(|a: &mut App| {
            poisoned(a);
            known(a, CURE_POISON);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| quick_cast(a, d, seen, false, 20, 0))),
        "playpoisontick" => (Place::Dungeon, Box::new(poisoned), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            for _ in 0..3 {
                minute_due(a);
                frames(a, d, seen, 4, |_| false);
            }
        })),
        "playrest" => (Place::Wild, Box::new(|a: &mut App| resting(a, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames_reading(a, d, seen, 300))),
        "playwildnight" => (Place::Wild, Box::new(|a: &mut App| night_encounter(a, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 400, |_| false))),
        // gen_worldframes.py's wild-life cases: the noon encounter fought;
        // doors clicked (`before_door`).
        // gen_worldframes.py's `jagar_encounter`: the first frame's hour
        // brings the main quest's encounter, Jagar's words first (1194:
        // D91C), clicked through; twenty frames after.
        "playjagar" => (Place::Wild, Box::new(|a: &mut App| {
            day_encounter(a, 0x100);
            let m = &mut a.engine.m;
            let f = m.mem.r16(DS, 0x05C8);
            m.mem.w16(DS, 0x05C8, f | 0x8000);
            m.mem.w8(DS, 0x0F77, 3);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 1, |_| false);
            movies_through(a, d);
            frames(a, d, seen, 20, |_| false);
        })),
        "playwildambush" => (Place::Wild, Box::new(|a: &mut App| {
            day_encounter(a, 0x100);
            captured(a);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            ready(a, seen);
            frames(a, d, seen, 120, |_| false);
            swinging(a, d, seen, 150);
            frames(a, d, seen, 20, |_| false);
        })),
        "playhouselocked" | "playwildhouse" => {
            let (place, row, col) = if name == "playhouselocked" { (Place::Street, 95, 47) } else { (Place::Wild, -85, 22) };
            (place, Box::new(move |a: &mut App| before_door(a, row, col, None)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames(a, d, seen, 2, |_| false);
                door_clicked(a, d, seen, 160.0, 100.0);
                frames(a, d, seen, 10, |a| a.engine.m.mem.r16(DS, 0x061E) != 0);
                frames(a, d, seen, 1, |_| false);
            }))
        }
        // The palace closed at night: its box clicked away at its middle,
        // the harness's mouse at (A0h, 60h) (the window's click 4 on): the
        // box's reads (1194:59C8's 5536 and 5E21) leave the mouse there,
        // where the next frame's controls take the door's click's release
        // (2826:0A4C: CB60).
        "playpalacenight" => (Place::Street, Box::new(|a: &mut App| {
            at_hour(a, 22, 0);
            at_square(a, 9, 0x7C, 0x80);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            clicked_with(a, d, seen, 3, &[Act::Click(0xA4, 0x64)]);
            frames(a, d, seen, 3, |_| false);
        })),
        // gen_worldframes.py's `audience`: the palace's door by day with the
        // town taken for the Imperial City (A46 8) at the staff's ninth
        // stage: the artifact's audience (0000:425F, text 5DDh), its box
        // clicked away with the harness's mouse at (160, 100) (the window's
        // click 4 on: the click left for the actors goes on from there).
        "playaudience9" => (Place::Street, Box::new(|a: &mut App| {
            at_hour(a, 12, 0);
            at_square(a, 9, 0x7C, 0x80);
            a.engine.m.mem.w8(DS, 0x0A46, 8);
            a.engine.m.mem.w8(DS, 0x0F77, 9);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            clicked_with(a, d, seen, 3, &[Act::Click(164, 104)]);
            frames(a, d, seen, 3, |_| false);
        })),
        // The Emperor's audience (F77 8): the palace's door clicked, the
        // audience's movies (0000:41A4) and the palace entered (the area
        // left); three frames in the palace.
        "playaudience8" => (Place::Street, Box::new(|a: &mut App| {
            at_hour(a, 12, 0);
            at_square(a, 9, 0x7C, 0x80);
            a.engine.m.mem.w8(DS, 0x0A46, 8);
            a.engine.m.mem.w8(DS, 0x0F77, 8);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            let at = a.engine.m.mem.r8(DS, arena::trigger::var::AREA);
            door_clicked(a, d, seen, 160.0, 100.0);
            frames(a, d, seen, 3, |a| a.engine.m.mem.r8(DS, arena::trigger::var::AREA) != at);
            movies_through(a, d);
            frames(a, d, seen, 3, |_| false);
        })),
        // gen_worldframes.py's `playimppal`: `playaudience8` into IMPPAL,
        // its stairs down to the fourth level (`IMPPAL_DOWN`), its staff
        // piece clicked (`imppal_piece`): the staff made whole, its scenes
        // clicked through, the area left (as `playlastpiece` ends).
        "playimppal" => (Place::Street, Box::new(audience8), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            imppal_down(a, d, seen, 3);
            done_between(a, seen, |a| {
                at_square(a, 23, 20, 0);
                for base in [0xC3D0u16, 0xBA62] {
                    a.engine.m.mem.w16(DS, base + 4, (23 << 7) + 0x70);
                }
            });
            frames(a, d, seen, 2, |_| false);
            let at = a.engine.m.mem.r8(DS, arena::trigger::var::AREA);
            a.release();
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            movies_through(a, d);
            frames(a, d, seen, 3, |a| a.engine.m.mem.r8(DS, arena::trigger::var::AREA) != at);
            assert_ne!(a.engine.m.mem.r8(DS, arena::trigger::var::AREA), at, "the area isn't left");
            seen.pop();
            ENDS.with(|e| e.set(true));
        })),
        // The house picked (picking armed, the seed SEED) and in (the area
        // left, `leaves`); three frames in the house.
        "playhousepick" => (Place::Street, Box::new(|a: &mut App| {
            before_door(a, 95, 47, None);
            let f = a.engine.m.mem.r16(DS, 0x05CA);
            a.engine.m.mem.w16(DS, 0x05CA, f | 1);
            seeded(a);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            let at = a.engine.m.mem.r8(DS, arena::trigger::var::AREA);
            door_clicked(a, d, seen, 160.0, 100.0);
            frames(a, d, seen, 10, |a| a.engine.m.mem.r8(DS, arena::trigger::var::AREA) != at);
            frames(a, d, seen, 3, |_| false);
        })),
        "playcrowd" => (Place::Street, Box::new(|a: &mut App| at_hour(a, 12, 0)), held(60, |_| false, 240)),
        "playcrypt" => (Place::Crypt, Box::new(|a: &mut App| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100)), held(20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0, 1)),
        "playpick" => (Place::Crypt, Box::new(|a: &mut App| picking(a, 1)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 19, |_| false);
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 40, |_| false);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, 10, |_| false);
        })),
        "playloot" => (Place::Dungeon, Box::new(fought), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| looted(a, d, seen, false))),
        "playlootitems" => (Place::Dungeon, Box::new(fought), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| looted(a, d, seen, true))),
        "playchest" => (Place::Dungeon, Box::new(chest_ahead), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The click that opens it (292B:0743), then the one that lists
            // its piles (292B:07FB).
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 3, |_| false);
            clicked_with(a, d, seen, 10, &[LOOT_LINE, LOOT_LINE, Act::RClick(0x60, 0x25)]);
            frames(a, d, seen, 5, |_| false);
        })),
        "playchestpick" => (Place::Dungeon, Box::new(chest_picking), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The click that tries the lock (292B:0743, 1194:53F9), 4 on
            // as the harness's lands (CB60).
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 7, |_| false);
        })),
        "playlootwild" => (Place::Wild, Box::new(fought), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| looted(a, d, seen, false))),
        "playlootstreet" => (Place::Street, Box::new(fought_in_the_open), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| looted(a, d, seen, false))),
        "playpile" => (Place::Dungeon, Box::new(pile_ahead), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            clicked_with(a, d, seen, 10, &[LOOT_LINE, LOOT_LINE, Act::RClick(0x60, 0x25)]);
            frames(a, d, seen, 5, |_| false);
        })),
        // gen_worldframes.py's `piece_ahead`: the pile's flat ahead made the
        // level's staff flat (C941 7), the staff's line at its third piece
        // and its text not told: clicked, the piece taken (2433:0328's AH
        // 0Fh), its text's box clicked away; five frames after.
        // `playpiece`'s pile as the staff's last piece (F77 8): the staff
        // made whole (1194:D986), its scenes clicked through, the area
        // left; the case ends before the next area's first frame.
        "playlastpiece" => (Place::Dungeon, Box::new(|a: &mut App| {
            at_square(a, 0xB, 0x2A, 0);
            let m = &mut a.engine.m;
            let floor = m.mem.r16(DS, 0xCCD0);
            m.mem.w8(floor, 0x0C54, 7);
            m.mem.w8(DS, 0xC941, 7);
            m.mem.w8(DS, 0x0F77, 8);
            m.mem.w8(DS, 0x0F7D, 0);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            let at = a.engine.m.mem.r8(DS, arena::trigger::var::AREA);
            a.release();
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            movies_through(a, d);
            frames(a, d, seen, 3, |a| a.engine.m.mem.r8(DS, arena::trigger::var::AREA) != at);
            assert_ne!(a.engine.m.mem.r8(DS, arena::trigger::var::AREA), at, "the area isn't left");
            // (The harness ends before the next area's first frame: the
            // frame that left has its top, not what it led into.)
            seen.pop();
            ENDS.with(|e| e.set(true));
        })),
        "playpiece" => (Place::Dungeon, Box::new(|a: &mut App| {
            at_square(a, 0xB, 0x2A, 0);
            let m = &mut a.engine.m;
            let floor = m.mem.r16(DS, 0xCCD0);
            m.mem.w8(floor, 0x0C54, 7);
            m.mem.w8(DS, 0xC941, 7);
            m.mem.w8(DS, 0x0F77, 3);
            m.mem.w8(DS, 0x0F7D, 0);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            clicked_with(a, d, seen, 10, &[Act::Click(0xA0, 0x60)]);
            frames(a, d, seen, 5, |_| false);
        })),
        "playriddleright" => (Place::Dungeon, Box::new(riddle_door), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| riddle_played(a, d, seen, "fire"))),
        "playriddlewrong" => (Place::Dungeon, Box::new(riddle_door), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| riddle_played(a, d, seen, "water"))),
        "playlock" => (Place::Crypt, Box::new(|a: &mut App| {
            at_square(a, 13, 4, 0);
            let m = &mut a.engine.m;
            let n = m.mem.r8(DS, arena::movement::var::LOCK_COUNT);
            let rec = arena::movement::var::LOCKS + 3 * n as u16;
            m.mem.w16(DS, rec, 0x0E04);
            m.mem.w8(DS, rec + 2, 5);
            m.mem.w8(DS, arena::movement::var::LOCK_COUNT, n + 1);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // Down for the frame whose controls read it, then up, as the
            // original's mouse reads give it (its controls act on the
            // next frame's).
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 10, |a| a.engine.m.mem.r16(DS, 0x061E) != 0);
            frames(a, d, seen, 1, |_| false);
        })),
        "playdiagonal" => (Place::Crypt, Box::new(|a: &mut App| at_square(a, 6, 3, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playdoormoving" => (Place::Crypt, Box::new(|a: &mut App| picking(a, 1)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 3, |_| false);
        })),
        "playswim" => (Place::Crypt, Box::new(|a: &mut App| at_square(a, 21, 7, 0)), held(30, |_| false, 60)),
        "playlevitate" => (Place::Crypt, Box::new(|a: &mut App| {
            at_square(a, 21, 7, 0);
            levitating(a);
        }), held(30, |_| false, 30)),
        "playscreens" => (Place::Dungeon, Box::new(|a: &mut App| brought(a, 1, 1, 0x180)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 20, |_| false);
            // The automap (M) opened and left by its exit (0000:2E30), the
            // character sheet (F1) with Esc, each in the frame after (the
            // original's frame body opens them: 2542:0100).
            // The harness's mouse at the view's centre, where the screen's loop
            // first reads it.
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyM, Some("m"));
            d.click(a, 0x100, 0xB0, MouseButton::Left);
            faded(a);
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
            d.key(a, KeyCode::F1, None);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            pushed(a, seen);
            frames(a, d, seen, 30, |_| false);
        })),
        "playautomap" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The harness's mouse at the view's centre, where the screen's loop
            // first reads it.
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyM, Some("m"));
            d.click(a, 0x100, 0xB0, MouseButton::Left);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playlog" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The logbook (L, 2542:02A3), two clicks off its buttons (the
            // next pages, 27E3:03A6) and Exit twice: the first ends the
            // pages' loop, the loop after them (0137) takes the second.
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyL, Some("l"));
            faded(a);
            d.click(a, 0xA0, 0x60, MouseButton::Left);
            d.click(a, 0xA0, 0x60, MouseButton::Left);
            d.click(a, 0x110, 0xBC, MouseButton::Left);
            assert!(matches!(a.panel, Some(crate::panels::Panel::Log(_))), "the first Exit ends only the pages' loop");
            d.click(a, 0x110, 0xBC, MouseButton::Left);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playlogprint" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The logbook (L), Print on its first page (27E3:0272: the
            // whole log to PRN, the pages' loop ended), Exit.
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyL, Some("l"));
            faded(a);
            d.click(a, 0x30, 0xBC, MouseButton::Left);
            assert!(matches!(a.panel, Some(crate::panels::Panel::Log(_))), "Print ends only the pages' loop");
            d.click(a, 0x110, 0xBC, MouseButton::Left);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playbarjump" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            frames(a, d, seen, 1, |_| false);
            // The crossed swords clicked with J held: the frame that takes
            // the click on the bar makes no jump (2826:00CB on 02E2's carry).
            a.input(In::Key { code: KeyCode::KeyJ, down: true, text: None });
            let (x, y) = crate::sheets::bar_centre(0);
            d.click(a, x, y, MouseButton::Left);
            pushed(a, seen);
            a.input(In::Key { code: KeyCode::KeyJ, down: false, text: None });
            frames(a, d, seen, 5, |_| false);
        })),
        "playlogbar" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            frames(a, d, seen, 1, |_| false);
            // The bar's scroll button (2542:06CE's table, 02A3): the
            // logbook, its SP at 12C6 a word deeper than the L key's; a
            // click off its buttons, Exit twice.
            d.click(a, 131, 186, MouseButton::Left);
            faded(a);
            assert!(matches!(a.panel, Some(crate::panels::Panel::Log(_))), "the scroll button opens the logbook");
            d.click(a, 0xA0, 0x60, MouseButton::Left);
            d.click(a, 0x110, 0xBC, MouseButton::Left);
            d.click(a, 0x110, 0xBC, MouseButton::Left);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 9, |_| false);
        })),
        "playuselist" => (Place::Dungeon, Box::new(wounded_with_potion), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The use list (U, 2542:034D), its fifth line (the potion)
            // clicked twice: marked, then picked (2542:06F8, 3159:06A0),
            // which keeps the screen as shown in bank [1032].
            a.compose();
            d.key(a, KeyCode::KeyU, Some("u"));
            assert!(matches!(&a.panel, Some(crate::panels::Panel::Use(n, _, _)) if n.len() > 4), "U opens the use list");
            d.click(a, 0xF0, 0xBD, MouseButton::Left);
            d.click(a, 0xF0, 0xBD, MouseButton::Left);
            assert!(a.panel.is_none(), "the potion picked");
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 60, |_| false);
        })),
        "playbook" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The spellbook (C, 2542:056A): its names sorted and listed in
            // the bar's corner (0000:0B9C, 2542:06F8), left by a right
            // click (5AF4's flag 4000h).
            a.compose();
            d.key(a, KeyCode::KeyC, Some("c"));
            assert!(matches!(a.panel, Some(crate::panels::Panel::Spellbook(_))), "C opens the spellbook");
            d.click(a, 0xF0, 0xA0, MouseButton::Right);
            assert!(a.panel.is_none(), "a right click leaves the list");
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        // The screens a menu key opens inside the frame, each case ending
        // with that frame (gen_worldframes.py's `ends`): the screen closed,
        // then the rest of the frame's body run after it.
        "playsheetend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::F1, down: true, text: None });
            d.key(a, KeyCode::Escape, None);
            faded(a);
            ENDS.with(|e| e.set(true));
        })),
        // playsheetend with the weapon readied first (as playfight's): the
        // hands drawn into the page that the sheet saves (SCREENSV.$$2).
        "playsheetready" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            ready(a, seen);
            frames(a, d, seen, 30, |_| false);
            a.input(In::Key { code: KeyCode::F1, down: true, text: None });
            d.key(a, KeyCode::Escape, None);
            faded(a);
            ENDS.with(|e| e.set(true));
        })),
        // gen_worldframes.py's sheet cases over what no other saves: the
        // bow readied, the casting hands, the boat's prow, keys held, the
        // crypt's text, an effect's words.
        "playbowsheet" => (Place::Dungeon, Box::new(bowed), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            ready(a, seen);
            frames(a, d, seen, 30, |_| false);
            sheet_ends(a, d);
        })),
        "playcastsheet" => (Place::Dungeon, Box::new(|a: &mut App| {
            brought(a, 1, 1, 0x200);
            let e = &mut a.engine;
            e.m.cast_spell(&mut e.files, &mut arena::cast::NoCastHooks, 6).expect("the spell cast");
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The harness's mouse at (160, 100), whose click (CB60) is 4
            // past it; the view's click is that point (`take_click`). The
            // aim reads the mouse (29CF:00D2), so it counts here.
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 2, |_| false);
            sheet_ends(a, d);
        })),
        "playboatsheet" => (Place::Street, Box::new(boated), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            sheet_ends(a, d);
        })),
        "playkeyssheet" => (Place::Dungeon, Box::new(|a: &mut App| {
            for (i, b) in KEYS_HELD.iter().enumerate() {
                a.engine.m.mem.w8(DS, 0x0652 + i as u16, *b);
            }
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            sheet_ends(a, d);
        })),
        "playtextsheet" => (Place::Crypt, Box::new(|a: &mut App| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            sheet_ends(a, d);
        })),
        "playeffectsheet" => (Place::Dungeon, Box::new(|a: &mut App| {
            for (i, b) in EFFECT_WORDS.iter().enumerate() {
                a.engine.m.mem.w8(DS, arena::effects::var::MESSAGE_TEXT + i as u16, *b);
            }
            a.engine.m.mem.w8(DS, arena::effects::var::MESSAGE, 0x0F);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            sheet_ends(a, d);
        })),
        "playlogend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Key { code: KeyCode::KeyL, down: true, text: Some("l".into()) });
            faded(a);
            d.click(a, 0xA0, 0x60, MouseButton::Left);
            d.click(a, 0x110, 0xBC, MouseButton::Left);
            held_click(a, 0x110, 0xBC);
            faded(a);
            ENDS.with(|e| e.set(true));
        })),
        "playautomapend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Key { code: KeyCode::KeyM, down: true, text: Some("m".into()) });
            held_click(a, 0x100, 0xB0);
            faded(a);
            ENDS.with(|e| e.set(true));
        })),
        "playbookend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.compose();
            a.input(In::Key { code: KeyCode::KeyC, down: true, text: Some("c".into()) });
            d.click(a, 0xF0, 0xA0, MouseButton::Right);
            assert!(a.panel.is_none(), "a right click leaves the list");
            ENDS.with(|e| e.set(true));
        })),
        // The screens whose ends can leave the world loop, inside the
        // frame too: the options screen left by its Resume, the travel
        // map by Esc, a journey (the area left at the frame's end, the
        // next area entered, before its first frame).
        "playoptionsend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::Escape, down: true, text: None });
            faded(a);
            assert!(a.options.is_some(), "Esc opens the options screen");
            held_click(a, 288, 132);
            faded(a);
            assert!(a.options.is_none(), "Resume leaves it");
            ENDS.with(|e| e.set(true));
        })),
        // The options screen's Load, Esc let go first (the load menu's
        // click takes a held Esc as Esc), slot 1: the load returns into
        // the frame the screen opened in, which runs on to its end.
        "playoptionsload" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::Escape, down: true, text: None });
            faded(a);
            assert!(a.options.is_some(), "Esc opens the options screen");
            a.input(In::Key { code: KeyCode::Escape, down: false, text: None });
            d.click(a, 96, 132, MouseButton::Left);
            assert!(a.saveui.as_ref().is_some_and(|u| u.kind == crate::saveui::Kind::Load), "Load: the load menu");
            d.click(a, 160, 21, MouseButton::Left);
            for _ in 0..50 {
                if a.inside.is_none() {
                    break;
                }
                faded(a);
                d.wait(a, 100);
            }
            assert!(a.saveui.is_none() && a.inside.is_none(), "loaded, the frame's rest run");
            ENDS.with(|e| e.set(true));
        })),
        "playmapend" => (Place::Street,Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
            a.input(In::Key { code: KeyCode::KeyM, down: true, text: Some("M".into()) });
            faded(a);
            assert!(a.travel.is_some(), "Shift+M opens the travel map");
            // (The harness's mouse at the screen's centre, which the
            // map's passes find a region under.)
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::Escape, None);
            for _ in 0..100 {
                if a.travel.is_none() {
                    break;
                }
                d.wait(a, 100);
            }
            faded(a);
            assert!(a.travel.is_none(), "Esc leaves the world map");
            ENDS.with(|e| e.set(true));
        })),
        // gen_worldframes.py's playsearchend: the search's list picked
        // from (Daggerfall, twice), the journey's box read, the province
        // left and the world map by Esc.
        "playsearchend" => (Place::Street, Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
            a.input(In::Key { code: KeyCode::KeyM, down: true, text: Some("M".into()) });
            faded(a);
            assert!(a.travel.is_some(), "Shift+M opens the travel map");
            for (x, y) in [(70, 45), (43, 175)] {
                d.click(a, x, y, MouseButton::Left);
            }
            for _ in 0..2 {
                a.travel.as_mut().expect("the travel map").advance(0);
            }
            crate::inputtest::type_text(d, a, "z");
            d.key(a, KeyCode::Enter, None);
            for (x, y) in [(110, 46), (110, 46), (110, 46), (80, 180)] {
                d.click(a, x, y, MouseButton::Left);
            }
            // (The harness's mouse still on the province's Exit, where
            // the world map's pass finds its region.)
            d.key(a, KeyCode::Escape, None);
            for _ in 0..100 {
                if a.travel.is_none() {
                    break;
                }
                d.wait(a, 100);
            }
            faded(a);
            assert!(a.travel.is_none(), "Esc leaves the world map");
            ENDS.with(|e| e.set(true));
        })),
        "playjourneyend" => (Place::Street, Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
            a.input(In::Key { code: KeyCode::KeyM, down: true, text: Some("M".into()) });
            faded(a);
            for (x, y) in [(70, 45), (200, 100), (200, 100), (62, 180)] {
                d.click(a, x, y, MouseButton::Left);
            }
            for _ in 0..100 {
                if a.travel.is_none() {
                    break;
                }
                d.wait(a, 100);
            }
            faded(a);
            assert!(a.travel.is_none(), "the journey ends the travel map");
            ENDS.with(|e| e.set(true));
        })),
        // gen_musicplay.py's own: `playjourneyend` on into three of the
        // town's frames (each asking after the town's song, 2294:0509).
        "playjourneyon" => (Place::Street, Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
            a.input(In::Key { code: KeyCode::KeyM, down: true, text: Some("M".into()) });
            faded(a);
            for (x, y) in [(70, 45), (200, 100), (200, 100), (62, 180)] {
                d.click(a, x, y, MouseButton::Left);
            }
            for _ in 0..100 {
                if a.travel.is_none() {
                    break;
                }
                d.wait(a, 100);
            }
            faded(a);
            assert!(a.travel.is_none(), "the journey ends the travel map");
            // (Shift and M still held, as the harness's step leaves them.)
            // The town's first frame's top, then its three frames.
            pushed(a, seen);
            frames(a, d, seen, 3, |_| false);
        })),
        // The first level's up stairs walked into: the way out's travel
        // map (0000:4537, 26F7:0000) inside the frame, Esc on it stays,
        // the world given back and the frame run on to its end.
        "playexitmapend" => (Place::Dungeon, Box::new(|a: &mut App| at_square(a, 11, 42, 0x100)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 30, |a| a.travel.is_some());
            assert!(a.travel.is_some(), "the up stairs show the travel map");
            // (The frame the map is in ends the case: its top isn't seen.)
            seen.pop();
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::Escape, None);
            for _ in 0..100 {
                if a.travel.is_none() {
                    break;
                }
                d.wait(a, 100);
            }
            faded(a);
            assert!(a.travel.is_none(), "Esc leaves the travel map");
            ENDS.with(|e| e.set(true));
        })),
        // The status box (S) inside its frame, a click closing it.
        "playstatusend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            a.input(In::Key { code: KeyCode::KeyS, down: true, text: Some("s".into()) });
            faded(a);
            d.click(a, 160, 100, MouseButton::Left);
            faded(a);
            ENDS.with(|e| e.set(true));
        })),
        "playnewend" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playnew, New clicked on the options screen Esc opens in
            // the frame: the rest of that frame ends the world loop.
            a.input(In::Key { code: KeyCode::Escape, down: true, text: None });
            faded(a);
            d.click(a, 32, 132, MouseButton::Left);
            faded(a);
            a.engine = a.title_engine.take().expect("the way back to the menu");
        })),
        "playsheet" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            d.key(a, KeyCode::F1, None);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequip" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The sheet (F1), its equipment page (N, 1194:0D67), the page's
            // exit (Esc, 3276:57D8's fifth record) and the sheet's (Esc).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipscroll" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the list moved on twice ('+', 1194:1A4A) and
            // back once ('-', 1A94) on the equipment page.
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.key(a, KeyCode::NumpadAdd, Some("+"));
            d.key(a, KeyCode::NumpadAdd, Some("+"));
            d.key(a, KeyCode::Minus, Some("-"));
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipwear" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the list's second line clicked (marked,
            // 1194:1C5E) and clicked again (its item taken off, 1AFB).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipdrop" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the list's third line marked (1194:1C5E) and
            // dropped: D (17D8), its question answered Y (18F6).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.click(a, 0x40, 0x4D, MouseButton::Left);
            d.key(a, KeyCode::KeyD, Some("d"));
            d.key(a, KeyCode::KeyY, Some("y"));
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipspells" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the Spells button's key (S, 1194:2365): the
            // spells' list (1D5C), one line on ('+'), the items' again (S).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.key(a, KeyCode::KeyS, Some("s"));
            d.key(a, KeyCode::NumpadAdd, Some("+"));
            d.key(a, KeyCode::KeyS, Some("s"));
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipdescribe" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the list's second line right-clicked (marked,
            // 1194:1C5E, and described, 1988) and the click the panel's
            // wait takes (5536).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.click(a, 0x40, 0x42, MouseButton::Right);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipstaff" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the list moved to its end ('+' 34 times), its
            // last line, the staff's quest line (F77), clicked (marked,
            // 1194:1C5E, counted again, 1F56), right-clicked (described,
            // 1988) and the click the panel's wait takes (5536), clicked
            // again (1B6F) and that wait's click.
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            for _ in 0..34 {
                d.key(a, KeyCode::NumpadAdd, Some("+"));
            }
            d.click(a, 0x40, 0x7A, MouseButton::Left);
            d.click(a, 0x40, 0x7A, MouseButton::Right);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.click(a, 0x40, 0x7A, MouseButton::Left);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipclash" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the list moved on 16 lines, its second line (a
            // ring of a kind worn already) clicked (marked) and clicked
            // again (1B78: "You can only equip one %s.", 223A) and the
            // click the panel's wait takes (5536).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            for _ in 0..16 {
                d.key(a, KeyCode::NumpadAdd, Some("+"));
            }
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipcantuse" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequipstaff's end of the list, its sixth line (the
            // save's last item, type 8Dh) clicked (marked) and clicked
            // again (1B6F, 1BE0: "This item can't be equipped.", 223A) and
            // the click the panel's wait takes (5536).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            for _ in 0..34 {
                d.key(a, KeyCode::NumpadAdd, Some("+"));
            }
            d.click(a, 0x40, 0x6F, MouseButton::Left);
            d.click(a, 0x40, 0x6F, MouseButton::Left);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipspellpage" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequip, the spells' list (S, 1194:2365), its second
            // line clicked (marked, 1C5E) and clicked again (its page,
            // 1B3C, 51D1), the page's Exit (the list drawn again).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.key(a, KeyCode::KeyS, Some("s"));
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.click(a, 0x40, 0x42, MouseButton::Left);
            faded(a);
            d.click(a, 0x120, 0xC0, MouseButton::Left);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipdropwhat" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequipdrop, the third line dropped (no line marked
            // after, 1194:1924), D again ("Drop what?", 18CA, its loop
            // 2325), the second line clicked (the loop left, the line
            // marked), its question answered Y.
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.click(a, 0x40, 0x4D, MouseButton::Left);
            d.key(a, KeyCode::KeyD, Some("d"));
            d.key(a, KeyCode::KeyY, Some("y"));
            d.key(a, KeyCode::KeyD, Some("d"));
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::KeyY, Some("y"));
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipdropin" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // As playequipdropwhat to "Drop what?"'s loop (2325), then '+'
            // (the list moved on in it), Y (no record's: the ring's only),
            // D (17D8 within: "Drop what?" and its loop again), the second
            // line clicked (the inner loop left, the line marked), its
            // question answered Y, and Esc (the outer loop left, no mark).
            d.key(a, KeyCode::F1, None);
            faded(a);
            d.key(a, KeyCode::KeyN, Some("n"));
            faded(a);
            d.click(a, 0x40, 0x4D, MouseButton::Left);
            d.key(a, KeyCode::KeyD, Some("d"));
            d.key(a, KeyCode::KeyY, Some("y"));
            d.key(a, KeyCode::KeyD, Some("d"));
            d.key(a, KeyCode::NumpadAdd, Some("+"));
            d.key(a, KeyCode::KeyY, Some("y"));
            d.key(a, KeyCode::KeyD, Some("d"));
            d.click(a, 0x40, 0x42, MouseButton::Left);
            d.key(a, KeyCode::KeyY, Some("y"));
            d.key(a, KeyCode::Escape, None);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            d.key(a, KeyCode::Escape, None);
            faded(a);
            // The frame that took the key (or the button) ends with its screen.
            pushed(a, seen);
            frames(a, d, seen, 10, |_| false);
        })),
        "playequipnoroom" => (
            Place::Dungeon,
            Box::new(|_: &mut App| {}),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames(a, d, seen, 2, |_| false);
                // gen_worldframes.py's `crowded_square`, before F1: the
                // player's square (CB90) given a byte in the plane at
                // [D806], which 1194:5F13 takes for no room.
                let (plane, sq) = (a.engine.m.mem.r16(DS, 0xD806), a.engine.m.mem.r16(DS, 0xCB90));
                a.engine.m.mem.w8(plane, sq, 1);
                // As playequip, D with no room (1194:17D8's 1962: the
                // refusal in the list's panel, 223A) and the click its
                // wait takes (5536).
                d.key(a, KeyCode::F1, None);
                faded(a);
                d.key(a, KeyCode::KeyN, Some("n"));
                faded(a);
                d.key(a, KeyCode::KeyD, Some("d"));
                d.click(a, 0x40, 0x42, MouseButton::Left);
                d.key(a, KeyCode::Escape, None);
                faded(a);
                d.key(a, KeyCode::Escape, None);
                faded(a);
                // The frame that took the key (or the button) ends with its screen.
                pushed(a, seen);
                frames(a, d, seen, 10, |_| false);
            }),
        ),
        "playuse" => (Place::Dungeon, Box::new(wounded_with_potion), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            used(a, 4);
            frames(a, d, seen, 5, |_| false);
            used(a, 3);
            frames(a, d, seen, 60, |_| false);
        })),
        "playtalkwhere" => (Place::People, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            talked(a, d, seen, &TALK_WHERE_SCRIPT);
            frames(a, d, seen, 1, |_| false);
        })),
        "playtalkkey" => (Place::People, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            talked(a, d, seen, &TALK_KEY_SCRIPT);
            frames(a, d, seen, 1, |_| false);
        })),
        "playtalkquest" => (Place::People, Box::new(dreaming), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            talked(a, d, seen, &TALK_WHERE_SCRIPT);
            frames(a, d, seen, 1, |_| false);
        })),
        "playtalkquestplace" => (Place::People, Box::new(dreaming), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            talked(a, d, seen, &TALK_QUEST_SCRIPT);
            frames(a, d, seen, 1, |_| false);
        })),
        "playtalkbar" => (Place::People, Box::new(dreaming), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            talked(a, d, seen, &TALK_BAR_SCRIPT);
            frames(a, d, seen, 1, |_| false);
        })),
        "playtalkpalace" => (Place::People, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            talked(a, d, seen, &TALK_PALACE_SCRIPT);
            frames(a, d, seen, 1, |_| false);
        })),
        "playstore" => (Place::Store, Box::new(shopping), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 6, &STORE_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playstorehaggle" => (Place::Store, Box::new(shopping), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 6, &STORE_HAGGLE_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playstoresellhaggle" => (Place::Store, Box::new(shopping), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 6, &STORE_SELL_HAGGLE_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playstorestolen" | "playstorefailed" | "playstorecaught" => {
            let seed = match name { "playstorestolen" => STEAL_SEED, "playstorefailed" => STEAL_QUIET, _ => SEED };
            (Place::Store, Box::new(stealing(seed)), Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames(a, d, seen, 2, |_| false);
                serviced(a, d, seen, 6, if seed == SEED { &STEAL_CAUGHT_SCRIPT[..] } else { &STEAL_SCRIPT[..] });
                frames(a, d, seen, 29, |_| false);
            }))
        }
        "playinformant" => (Place::Tavern, Box::new(informant), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 4, &INFORMANT_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playinformantpaid" => (Place::Tavern, Box::new(informant_owed), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 4, &INFORMANT_PAID_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playdeath" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // gen_worldframes.py's `killed`: health 0, 2686:02B4; the
            // frame that ends the world loop for it, and the view's way on
            // (`App::area_next`: the outer loop's death, 0000:0CE1..0D43).
            let m = &mut a.engine.m;
            m.mem.w16(DS, arena::character::PLAYER + arena::character::field::HP, 0);
            m.player_dies();
            d.frame(a);
        })),
        "playnew" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // gen_worldframes.py's `newgame`: the options screen's New
            // (2542:0627, `Machine::options_new`); the frame that ends the
            // world loop for it, the outer loop's way out and the program's
            // main back to its menu (`App::new_game_menu`, its copy of the
            // game kept for the menu's Load, `title_engine`), whose state
            // the case leaves.
            let e = &mut a.engine;
            e.m.options_new(&mut e.files, &mut arena::options::NoOptionsHooks);
            d.frame(a);
            a.engine = a.title_engine.take().expect("the way back to the menu");
        })),
        "playloadplay" => (Place::Street, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 1, |_| false);
            d.frame(a);
            // gen_worldframes.py's `loaded`: slot 1 loaded in play between
            // frames, as the load menu's pick loads it (`App::load_slot`
            // on the menu's copy of the machine, `game_loaded`), before the
            // next frame's top; play goes on in the game loaded.
            let dir = a.save_dir.clone().expect("the save folder");
            let mut e = arena::engine::Engine::new(a.engine.m.clone(), a.engine.files.clone());
            let run = a.load_slot(&mut e, &dir, 1, false).expect("slot 1 loads in play");
            a.game_loaded(e, run, 1);
            pushed(a, seen);
            frames(a, d, seen, 4, |_| false);
        })),
        "playfarewell" => (Place::Dungeon, Box::new(|a: &mut App| {
            // (And 9A6E filled with what the colours D45D reads there aren't.)
            a.engine.m.mem.w8(DS, 0x0F77, 3);
            for i in 0..0x300u16 {
                a.engine.m.mem.w8(DS, 0x9A6E + i, (i * 7 + 1) as u8 & 0x3F);
            }
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // `playdeath` at the main quest's third stage (gen_worldframes.py's
            // `farewell`, F77 3): the outer loop's death plays the farewell's
            // vision (1194:D459, `Machine::vision`), its pages clicked
            // through.
            let m = &mut a.engine.m;
            m.mem.w16(DS, arena::character::PLAYER + arena::character::field::HP, 0);
            m.player_dies();
            d.frame(a);
        })),
        "playfarewellsound" | "playfarewellsoundclick" => {
            let name = name.to_string();
            (Place::Dungeon, Box::new(|a: &mut App| {
                // `playfarewell`'s, and gen_worldframes.py's
                // `farewell_sound`: ARENA.BAT's sound driver named as its
                // command line leaves it.
                let m = &mut a.engine.m;
                m.mem.w8(DS, 0x0F77, 3);
                for i in 0..0x300u16 {
                    m.mem.w8(DS, 0x9A6E + i, (i * 7 + 1) as u8 & 0x3F);
                }
                for (i, &c) in b"SBPDIG.ADV\0".iter().enumerate() {
                    m.mem.w8(DS, 0x3827 + i as u16, c);
                }
                m.mem.w16(DS, 0x386A, 0x220);
                m.mem.w16(DS, 0x386C, 7);
                m.mem.w16(DS, 0x386E, 1);
            }), Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames(a, d, seen, 2, |_| false);
                // `playfarewell` with the sound driver loaded as the ending
                // begins (`areas::SPEECH`), its answers the original's
                // driver's (`SpeechReplay`, the case's `driver` line); no
                // click, or one.
                let replay = std::rc::Rc::new(std::cell::RefCell::new(SpeechReplay::of(&name)));
                SPEECH_REPLAY.with(|r| *r.borrow_mut() = Some(replay.clone()));
                let clicks = name == "playfarewellsoundclick";
                crate::areas::SPEECH.with(|s| *s.borrow_mut() = Some(crate::areas::Speech { driver: Box::new(Shared(replay)), clicks }));
                let m = &mut a.engine.m;
                m.mem.w16(DS, arena::character::PLAYER + arena::character::field::HP, 0);
                m.player_dies();
                d.frame(a);
                crate::areas::SPEECH.with(|s| s.borrow_mut().take());
            }))
        }
        "playtitle" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // `playdeath`, then gen_worldframes.py's `titled`: the program's
            // main on to its menu's wait (`Machine::title_menu`, 0000:109B..
            // 1114).
            let m = &mut a.engine.m;
            m.mem.w16(DS, arena::character::PLAYER + arena::character::field::HP, 0);
            m.player_dies();
            d.frame(a);
            let e = &mut a.engine;
            e.m.title_menu(&mut e.files, &mut arena::title::NoTitleHooks).expect("the way back to the menu");
        })),
        "playtitleload" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // `playtitle` as the app goes it (the world faded out, the
            // program's main back to its menu on the game's own state,
            // `App::title_engine`), then the menu's Load (`Outcome::Load`:
            // 117E's set-up on that state, the load menu) and slot 1's row:
            // the game loaded the title's way, up to its world loop's first
            // frame.
            let m = &mut a.engine.m;
            m.mem.w16(DS, arena::character::PLAYER + arena::character::field::HP, 0);
            m.player_dies();
            d.frame(a);
            d.wait(a, 2000);
            assert!(a.creation.is_some() && a.title_engine.is_some(), "the menu's screens up on the title's state");
            a.open_saveui(crate::saveui::Kind::TitleLoad);
            assert!(a.saveui.is_some(), "the title's load menu");
            d.click(a, 160, 21, MouseButton::Left);
            assert!(a.saveui.is_none() && a.title_engine.is_none() && a.creation.is_none(), "loaded");
            // (The case ends there: no frame after, as the harness's.)
            ENDS.with(|e| e.set(true));
        })),
        "playguild" => (Place::Loaded, Box::new(|a: &mut App| {
            room(a);
            seeded(a);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 7, &GUILD_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playguildbar" => (Place::Loaded, Box::new(|a: &mut App| {
            room(a);
            seeded(a);
        }), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 7, &GUILD_BAR_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playspellmaker" => (Place::Loaded, Box::new(seeded), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 7, &SPELLMAKER_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playlevelup" => (Place::Dungeon, Box::new(levelled), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            // The next frame's minute tick (gen_worldframes.py's
            // `minute_due`) brings up the level's notice and the attribute
            // screen inside it (1194:1034): LEVEL_SCRIPT's clicks.
            minute_due(a);
            levelled_frame(a, d, seen);
            frames(a, d, seen, 9, |_| false);
        })),
        "playtempleheal" => (Place::Temple, Box::new(temple_hurt), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 5, &TEMPLE_HEAL_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playtemplecure" => (Place::Temple, Box::new(diseased), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 5, &TEMPLE_CURE_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playtemplebless" => (Place::Temple, Box::new(seeded), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 5, &TEMPLE_BLESS_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playtaverndrink" => (Place::Tavern, Box::new(seeded), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 4, &TAVERN_DRINK_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        "playpalaceaccept" => (Place::Palace, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            ruled(a, d, seen, &PALACE_ACCEPT_TURN);
            frames_answered(a, d, seen, 29, &FRAME_BOXES);
        })),
        "playquestreached" => (Place::Tavern, Box::new(|a: &mut App| quest_under_way(a, false)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 4, &QUEST_REACHED_SCRIPT);
            frames_answered(a, d, seen, 29, &FRAME_BOXES);
        })),
        "playquestreward" => (Place::Palace, Box::new(|a: &mut App| quest_under_way(a, true)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            ruled(a, d, seen, &QUEST_REWARD_SCRIPT);
            frames_answered(a, d, seen, 29, &FRAME_BOXES);
        })),
        "playtavernroom" => (Place::Tavern, Box::new(seeded), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames(a, d, seen, 2, |_| false);
            serviced(a, d, seen, 4, &TAVERN_ROOM_SCRIPT);
            frames(a, d, seen, 29, |_| false);
        })),
        _ => return None,
    })
}

/// The engine's own drawing of the 3D view (`render::draw`, the page it
/// keeps) against the original's page, from gen_worldframes.py --dump's
/// NAME.full and NAME.mask in ARENA_SCREENS: each case's 3D pixels (the
/// mask's 0s) counted alike, the first that differ printed, and
/// NAME-soft.png (the original's, the engine's, white where they
/// differ).
#[test]
fn the_3d_view_drawn_as_the_original_draws_it() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    let only = std::env::var("ARENA_CASE").ok();
    let all = ["playdungeon", "playdown", "playcrypt", "playstreet", "playsky", "playnight", "playwild", "playcap", "playfog", "playghost"];
    // (ARENA_CASE: that case alone, gen_areaplay.py's too.)
    for name in only.as_deref().map_or(all.to_vec(), |o| vec![o]) {
        let (Ok(full), Ok(mask)) = (std::fs::read(dump.join(format!("{name}.full"))), std::fs::read(dump.join(format!("{name}.mask")))) else {
            eprintln!("{name}: no dump");
            continue;
        };
        let (place, setup, play) = case(name).or_else(|| areaplaytest::transit_case(name)).unwrap();
        let Some((a, _)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        }, play) else { return };
        let page = a.engine.page.clone().unwrap();
        let (mut n, mut same) = (0, 0);
        let mut shown = 0;
        let mut out = vec![0u8; 960 * 200 * 3];
        let pal = a.assets.dac();
        for i in 0..64000 {
            let three = mask[i] == 0;
            let ok = full[i] == page[i];
            if three {
                n += 1;
                if ok {
                    same += 1;
                } else if shown < 12 {
                    eprintln!("{name}: ({}, {}) original {:02x}, engine {:02x}", i % 320, i / 320, full[i], page[i]);
                    shown += 1;
                }
            }
            let (x, y) = (i % 320, i / 320);
            let diff = if !three { [0x40, 0, 0x40, 0] } else if ok { [0, 0, 0, 0] } else { [0xFF; 4] };
            for (k, c) in [pal[full[i] as usize], pal[page[i] as usize], diff].into_iter().enumerate() {
                let at = (y * 960 + k * 320 + x) * 3;
                out[at..at + 3].copy_from_slice(&c[..3]);
            }
        }
        let bad: Vec<String> = (0..200)
            .filter_map(|y| {
                let k = (0..320).filter(|&x| mask[y * 320 + x] == 0 && full[y * 320 + x] != page[y * 320 + x]).count();
                (k > 0).then(|| format!("{y}:{k}"))
            })
            .collect();
        eprintln!("{name}: rows differing {}", bad.join(" "));
        let _ = std::fs::write(dump.join(format!("{name}.soft")), &page);
        if let Ok(r) = std::env::var("ARENA_ROW") {
            let y: usize = r.parse().unwrap();
            let hx = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
            eprintln!("{name}: row {y} original {}", hx(&full[y * 320..y * 320 + 320]));
            eprintln!("{name}: row {y} engine   {}", hx(&page[y * 320..y * 320 + 320]));
        }
        eprintln!("{name}: {same} of {n} 3D pixels the original's ({:.2}%); not ported: {:?}", 100.0 * same as f64 / n.max(1) as f64, a.engine.page_gaps);
        let file = std::fs::File::create(dump.join(format!("{name}-soft.png"))).expect("create");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 960, 200);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().expect("png").write_image_data(&out).expect("png");
    }
}

/// The engine's drawing of the case's 3D view (`render::draw` each frame,
/// the level's lights as the original has them: see below) and the 3D
/// pixels of it (where the view's own layer draws nothing): their digest
/// against the original's (`view3d`), and the share of them alike where
/// ARENA_SCREENS has the original's page.
fn soft_view(name: &str) -> Option<(bool, Option<f64>)> {
    soft_view_from(name, 0)
}

/// `soft_view` from row `top` on (`view3d1`: from row 1, the rows above
/// left 0).
fn soft_view_from(name: &str, top: usize) -> Option<(bool, Option<f64>)> {
    let (place, setup, play) = case(name)?;
    let (mut a, _) = played(
        place,
        |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        },
        play,
    )?;
    let page = a.engine.page.take().unwrap();
    let (idx, _) = layer(&mut a);
    let three: Vec<u8> = page.iter().zip(&idx).enumerate().map(|(k, (&c, &i))| if i >= 0x100 && k >= top * 320 { c } else { 0 }).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let kind = if top == 0 { "view3d" } else { "view3d1" };
    let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == kind && f[1] == name).map(|f| u64::from_str_radix(f[2], 16).unwrap());
    let same = std::env::var("ARENA_SCREENS").ok().and_then(|d| {
        let full = std::fs::read(PathBuf::from(&d).join(format!("{name}.full"))).ok()?;
        let n = idx.iter().filter(|&&i| i >= 0x100).count();
        let k = (0..64000).filter(|&i| idx[i] >= 0x100 && full[i] == page[i]).count();
        Some(100.0 * k as f64 / n.max(1) as f64)
    });
    Some((want == Some(fnv(three.iter().copied())), same))
}

/// The dungeon's first frames' 3D view, the second level's after the
/// stairs, and two of the main quest's dungeons' (one with fog: the view
/// filled to the horizon with its colour, 2294:0B4D, the fog's light
/// table's last rows the fill colour, 0C36), every pixel as the
/// original's renderer draws them.
#[test]
fn the_dungeons_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playdungeon", "playdown", "playcap", "playfog", "playfogfirst", "playghost"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The street outside the guild by day (the sky's rows, the water's
/// frame under the floor, the floor and the walls unshaded, the trees,
/// the townspeople in their colours), turned to the open sky (a cloud,
/// a lamp post, people far off) and at ten at night (the afternoon's
/// dither, the textures relit by the night's row of the light table),
/// every pixel as the original's renderer draws them.
#[test]
fn the_streets_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playstreet", "playsky", "playnight", "playbolt", "playrain", "playsnow"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The wilderness out of the city's gate (its fields, the sky), every
/// pixel as the original's renderer draws them.
#[test]
fn the_wildernesss_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playwild", "playvolcano", "playvolcanofar"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The crypt (the ceiling's dark runs stepped by the EMS frame's segment,
/// CCCE: 0000:D865's EDX bug, the frame at F000h as the harness's), every
/// pixel as the original's renderer draws them from row 1 on. Its topmost
/// ceiling row reads one entry past the row table (DB78's), the next heap
/// block's header in the original, whose words are its heap's segments
/// (7A33h, 78FFh there): no other layout has them, so row 0 is left out
/// (`view3d1`).
#[test]
fn the_crypts_3d_view_drawn_as_the_original_draws_it() {
    let Some((ok, same)) = soft_view_from("playcrypt", 1) else { return };
    assert!(ok, "playcrypt: the 3D view from row 1 differs from the original's ({same:?}% alike)");
}


/// Byte ranges where `a` and `b` differ, runs closer than `gap` joined.
fn differing(a: &[u8], b: &[u8], gap: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in (0..a.len().min(b.len())).filter(|&i| a[i] != b[i]) {
        match out.last_mut() {
            Some(r) if i - r.1 <= gap => r.1 = i + 1,
            _ => out.push((i, i + 1)),
        }
    }
    out
}

/// The game's state as each case leaves it, the engine's against the
/// original's (gen_worldframes.py --dump's NAME.ds and NAME.ems in
/// ARENA_SCREENS): DGROUP and the EMS banks, the ranges that differ
/// printed. A report: what a frame stage the view skips leaves behind.
#[test]
fn the_state_measured_against_the_originals() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    // (ARENA_CASE: that case alone, whichever it is.)
    let only = std::env::var("ARENA_CASE").ok();
    let all = ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playdown", "playcrypt", "playfog", "playghostclick"];
    for name in only.as_deref().map_or(all.to_vec(), |o| vec![o]) {
        let (Ok(ds), Ok(ems)) = (std::fs::read(dump.join(format!("{name}.ds"))), std::fs::read(dump.join(format!("{name}.ems")))) else {
            eprintln!("{name}: no dump");
            continue;
        };
        // From the load (ARENA_FROM=load: the places' entries counted
        // too) or from the case's start.
        let from_load = std::env::var("ARENA_FROM").is_ok_and(|v| v == "load");
        let (f_ds, f_ems) = if from_load { ("base.ds".to_string(), "base.ems".to_string()) } else { (format!("{name}.ds0"), format!("{name}.ems0")) };
        let (Ok(ds0), Ok(ems0)) = (std::fs::read(dump.join(f_ds)), std::fs::read(dump.join(f_ems))) else {
            eprintln!("{name}: no dump of the start");
            continue;
        };
        // (Or gen_areaplay.py's: their dumps beside these.)
        let (place, setup, play) = case(name).or_else(|| areaplaytest::transit_case(name)).unwrap();
        let pre = std::cell::RefCell::new(None);
        let Some((a, _)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a);
            // (ARENA_HEAP: the heap's blocks as the case starts.)
            if std::env::var("ARENA_HEAP").is_ok() {
                eprintln!("heap {:?}", heap_blocks(&a.engine.m));
            }
            *pre.borrow_mut() = Some(grab(a));
        }, play) else { return };
        let (mine0, banks0) = if from_load { BASE.with(|b| b.borrow_mut().take()).unwrap() } else { pre.into_inner().unwrap() };
        let (mine, banks) = grab(&a);
        // (ARENA_MINE: the engine's DGROUP as the case leaves it, and as
        // the load left it, beside the original's dumps.)
        if std::env::var("ARENA_MINE").is_ok() {
            let _ = std::fs::write(dump.join(format!("{name}.mine")), &mine);
            let _ = std::fs::write(dump.join(format!("{name}.mine0")), &mine0);
            let _ = std::fs::write(dump.join(format!("{name}.mems")), &banks);
            let _ = std::fs::write(dump.join(format!("{name}.mems0")), &banks0);
        }
        // What each side's frames changed: a byte the original's changed
        // and the engine's didn't (or the other way), or both changed it
        // to different values where they started alike.
        let drawn: Vec<u8> = a.engine.page_wrote.iter().map(|w| u8::from(w.is_some())).collect();
        let report = |what: &str, o0: &[u8], o1: &[u8], m0: &[u8], m1: &[u8], gap: usize, unit: usize| {
            let flag: Vec<u8> = (0..o0.len()).map(|i| {
                // Where they end differently and one side's frames
                // changed it and the other's didn't, or both started alike.
                let (oc, mc) = (o0[i] != o1[i], m0[i] != m1[i]);
                u8::from(o1[i] != m1[i] && (oc || mc) && (oc != mc || o0[i] == m0[i]))
            }).collect();
            let zero = vec![0u8; flag.len()];
            let r = differing(&flag, &zero, gap);
            eprintln!("{name}: {what} {} ranges", r.len());
            let hx = |v: &[u8]| v.iter().take(12).map(|b| format!("{b:02x}")).collect::<String>();
            for &(s, e) in r.iter().take(80) {
                // The drawing's copy wrote there (DGROUP only).
                let tag = if unit == 0x10000 && what == "DGROUP" && (s..e).all(|i| flag[i] == 0 || drawn[i] != 0) { " (the drawing's)" } else { "" };
                eprintln!("  {:02x}:{:04x}..{:04x} orig {}->{} mine {}->{}{tag}", s / unit, s % unit, s % unit + (e - s), hx(&o0[s..e]), hx(&o1[s..e]), hx(&m0[s..e]), hx(&m1[s..e]));
            }
        };
        report("DGROUP", &ds0, &ds, &mine0, &mine, 4, 0x10000);
        if std::env::var("ARENA_STRICT").is_ok() {
            // The changes as sets: changed on one side only, or on both to
            // different values (the drawing's copy's writes left out).
            let flag: Vec<u8> = (0..0x10000).map(|i| {
                let (oc, mc) = (ds0[i] != ds[i], mine0[i] != mine[i]);
                u8::from(drawn[i] == 0 && (oc != mc || (oc && ds[i] != mine[i])))
            }).collect();
            for &(s, e) in differing(&flag, &vec![0u8; 0x10000], 4).iter().take(80) {
                let hx = |v: &[u8]| v.iter().take(12).map(|b| format!("{b:02x}")).collect::<String>();
                eprintln!("  strict {s:04x}..{e:04x} orig {}->{} mine {}->{}", hx(&ds0[s..e]), hx(&ds[s..e]), hx(&mine0[s..e]), hx(&mine[s..e]));
            }
        }
        report("EMS", &ems0, &ems, &banks0, &banks, 32, 0x10000);
    }
}

/// DGROUP as the harness holds it, from the engine's: the image's
/// segments 1000h higher (the harness relocates the image to 1000h; the
/// port runs it unrelocated): every relocated word of the image that lies
/// in DGROUP, and the words tests/vectors/loadsegs.txt names (segments of
/// the image the game stores as it runs, unless their far pointer is null,
/// 0:0, a hook the area left unset, which the harness's holds as 0:0
/// too, or a word marked `alone` is 0, another writer's); the offsets
/// tests/vectors/loadexclude.txt names zeroed (on both sides).
fn load_canonical(dir: &std::path::Path, d: &[u8]) -> Vec<u8> {
    load_canonical_kept(dir, d, &[])
}

/// `load_canonical` with the loadsegs.txt words at `kept` left as they
/// are: where the path compared never ran the writer the file names (a
/// new game's A83C is 56C8's last picture's height, no load's segment).
fn load_canonical_kept(dir: &std::path::Path, d: &[u8], kept: &[usize]) -> Vec<u8> {
    let image = arena::exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut d = d.to_vec();
    let base = DS as usize * 16;
    let mut slots: Vec<usize> = image.relocs.iter().map(|&(seg, off)| seg as usize * 16 + off as usize).filter(|&l| l >= base && l + 1 < base + 0x10000).map(|l| l - base).collect();
    let segs = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/loadsegs.txt");
    for line in std::fs::read_to_string(segs).expect(segs).lines() {
        let mut f = line.split('#').next().unwrap_or("").split_whitespace();
        if let Some(w) = f.next() {
            let at = usize::from_str_radix(w, 16).unwrap();
            let held = if f.next() == Some("alone") { &d[at..at + 2] } else { &d[at - 2..at + 2] };
            if held.iter().any(|&b| b != 0) && !kept.contains(&at) {
                slots.push(at);
            }
        }
    }
    for at in slots {
        let w = u16::from_le_bytes([d[at], d[at + 1]]).wrapping_add(0x1000);
        d[at..at + 2].copy_from_slice(&w.to_le_bytes());
    }
    let mut skip = vec![false; 0x10000];
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/loadexclude.txt"), &mut skip);
    for (b, &x) in d.iter_mut().zip(&skip) {
        if x {
            *b = 0;
        }
    }
    d
}

/// The game's state as the title's Load leaves it (`world::boot`, then
/// `areas::load_game_title` of the user's slot 1): DGROUP exactly the
/// original's (`loadstate`), the engine's image segments taken to the
/// harness's relocated ones and the offsets tests/vectors/loadexclude.txt
/// names left out (`load_canonical`), and the EMS banks whole (`loadems`).
#[test]
fn the_load_as_the_original_leaves_it() {
    let Some(_) = played(Place::Street, |_| {}, |_, _, _| {}) else { return };
    let (mine, banks) = BASE.with(|b| b.borrow_mut().take()).unwrap();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix("loadstate | ")).expect("a loadstate line");
    let got = fnv(load_canonical(&game_dir().unwrap(), &mine));
    assert_eq!(format!("{got:x}"), want, "the state after the load differs from the original's (the_load_measured_against_the_originals shows where)");
    let want = text.lines().find_map(|l| l.strip_prefix("loadems | ")).expect("a loadems line");
    assert_eq!(format!("{:x}", fnv(banks)), want, "the EMS banks after the load differ from the original's (the_load_measured_against_the_originals shows where)");
}

/// The game's state as the load leaves it, the engine's against the
/// original's (gen_worldframes.py --dump's base.ds and base.ems in
/// ARENA_SCREENS; the engine's written beside them as base.mine and
/// base.mems): the DGROUP ranges and EMS ranges that differ, printed.
#[test]
fn the_load_measured_against_the_originals() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    let (Ok(ds), Ok(ems)) = (std::fs::read(dump.join("base.ds")), std::fs::read(dump.join("base.ems"))) else {
        eprintln!("no base dump");
        return;
    };
    let Some(_) = played(Place::Street, |_| {}, |_, _, _| {}) else { return };
    let (mine, banks) = BASE.with(|b| b.borrow_mut().take()).unwrap();
    BASE_HEAP.with(|h| {
        for (seg, size, used) in h.borrow().iter() {
            eprintln!("block {seg:04x} size {size:04x} used {used}");
        }
    });
    let _ = std::fs::write(dump.join("base.mine"), &mine);
    let _ = std::fs::write(dump.join("base.mems"), &banks);
    measured(&ds, &ems, &mine, &banks);
}

/// The DGROUP and EMS ranges where the engine's state (`mine`, `banks`)
/// differs from the original's (`ds`, `ems`), printed: DGROUP in the
/// original's form (`load_canonical`) and raw.
fn measured(ds: &[u8], ems: &[u8], mine: &[u8], banks: &[u8]) {
    let hx = |v: &[u8]| v.iter().take(16).map(|b| format!("{b:02x}")).collect::<String>();
    // Against the original's in its form (`load_canonical`).
    let dir = game_dir().unwrap();
    let canon = load_canonical(&dir, mine);
    let mut skip = vec![false; 0x10000];
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/loadexclude.txt"), &mut skip);
    let orig: Vec<u8> = ds.iter().zip(&skip).map(|(&b, &x)| if x { 0 } else { b }).collect();
    let r = differing(&orig, &canon, 0);
    eprintln!("canonical DGROUP: {} bytes in {} ranges", (0..0x10000).filter(|&i| orig[i] != canon[i]).count(), r.len());
    for &(s, e) in &r {
        eprintln!("  c {s:04x}..{e:04x} orig {} mine {}", hx(&orig[s..e]), hx(&canon[s..e]));
    }
    let r = differing(ds, mine, 4);
    eprintln!("DGROUP: {} bytes in {} ranges", (0..0x10000).filter(|&i| ds[i] != mine[i]).count(), r.len());
    for &(s, e) in &r {
        eprintln!("  {s:04x}..{e:04x} orig {} mine {}", hx(&ds[s..e]), hx(&mine[s..e]));
    }
    let r = differing(ems, banks, 32);
    eprintln!("EMS: {} bytes in {} ranges", (0..ems.len()).filter(|&i| ems[i] != banks[i]).count(), r.len());
    for &(s, e) in r.iter().take(60) {
        eprintln!("  {:02x}:{:04x}..{:04x} orig {} mine {}", s >> 16, s & 0xFFFF, (s & 0xFFFF) + (e - s), hx(&ems[s..e]), hx(&banks[s..e]));
    }
}

/// The places tests/vectors/playload.txt loads in (gen_playload.py): the
/// guild as the title's Load leaves it, and the street.
const PLAY_LOADS: [(&str, bool); 2] = [("guild", false), ("street", true)];

/// Slot 1 loaded in play (`areas::load_game`: the load menu's 25FD:028D)
/// at the top of a frame in the guild as the title's Load leaves it, or
/// with `street` in the street (the guild left as `played` leaves it):
/// DGROUP and the EMS banks after (`grab`).
fn play_loaded(street: bool) -> Option<(Vec<u8>, Vec<u8>)> {
    let dir = game_dir()?;
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return None;
    };
    let mut a = app(&dir, &Start { hour: None, load: Some((save.to_path_buf(), 1)), save_dir: Some(save.to_path_buf()), ..Default::default() });
    a.engine.m.rng.seed = HARNESS_SEED;
    a.engine.m.store_rng();
    let mut d = Driver::new(&mut a);
    clear(&mut d, &mut a);
    if street {
        area_left(&mut a);
    }
    if let Err(e) = crate::areas::load_game(&mut a.engine, &dir, &save.to_path_buf(), 1) {
        panic!("the load in play failed: {e}");
    }
    Some(grab(&a))
}

/// A load in play leaves the game's state as the original's does: DGROUP
/// in the original's form (`load_canonical`) and the EMS banks whole,
/// against tests/vectors/playload.txt.
#[test]
fn the_play_loads_as_the_original_leaves_them() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/playload.txt");
    let text = std::fs::read_to_string(path).expect(path);
    for (name, street) in PLAY_LOADS {
        let Some((mine, banks)) = play_loaded(street) else { return };
        let want = text.lines().find_map(|l| l.strip_prefix(&format!("playload | {name} | "))).expect("a playload line");
        let got = format!("{:x} {:x}", fnv(load_canonical(&game_dir().unwrap(), &mine)), fnv(banks));
        assert_eq!(got, want, "{name}: the state after the load in play differs from the original's (the_play_loads_measured shows where)");
    }
}

/// `the_play_loads_as_the_original_leaves_them` measured against the
/// original's dumps (gen_playload.py --dump's NAME.ds and NAME.ems in
/// ARENA_SCREENS): the ranges that differ, printed.
#[test]
fn the_play_loads_measured() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    for (name, street) in PLAY_LOADS {
        let (Ok(ds), Ok(ems)) = (std::fs::read(dump.join(format!("{name}.ds"))), std::fs::read(dump.join(format!("{name}.ems")))) else {
            eprintln!("no {name} dump");
            continue;
        };
        let Some((mine, banks)) = play_loaded(street) else { return };
        eprintln!("== {name}");
        measured(&ds, &ems, &mine, &banks);
    }
}

/// tests/vectors/stateexclude.txt's DGROUP offsets (`state` lines leave
/// them out).
fn state_excluded() -> Vec<bool> {
    let mut out = vec![false; 0x10000];
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/stateexclude.txt"), &mut out);
    out
}

/// stateexclude.txt's and stateloadexclude.txt's DGROUP offsets
/// (`stateload` lines leave them out).
fn state_load_excluded() -> Vec<bool> {
    let mut out = state_excluded();
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/stateloadexclude.txt"), &mut out);
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/loadexclude.txt"), &mut out);
    out
}

/// The ranges a state exclusion file names, marked in `out`.
fn excluded_in(path: &str, out: &mut [bool]) {
    for line in std::fs::read_to_string(path).expect(path).lines() {
        let f: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        if f.len() >= 2 {
            let (s, e) = (usize::from_str_radix(f[0], 16).unwrap(), usize::from_str_radix(f[1], 16).unwrap());
            out[s..e].iter_mut().for_each(|x| *x = true);
        }
    }
}

/// The case's frames' changes to DGROUP against the original's (`state`
/// lines: the offsets it changed, the digest of their bytes after), the
/// drawing's writes on its copy counted as the original's drawing makes
/// them on the machine (`Engine::page_state`): the same offsets changed,
/// to the same bytes. Where a stage the view skips leaves the game's
/// state other than the original's, this fails.
fn state_case(name: &str) {
    state_compared(name, false);
}

/// `state_case` from the load (`stateload` lines): the place's way in
/// counted too, against the state `played`'s load left (`BASE`).
fn state_load_case(name: &str) {
    state_compared(name, true);
}

fn state_compared(name: &str, from_load: bool) {
    state_compared_in(VECTORS, name, from_load, case(name).unwrap());
}

/// The files the game wrote, by name.
fn files_written(f: &arena::bsa::Files) -> std::collections::BTreeMap<String, Vec<u8>> {
    f.written_names().into_iter().map(|n| (String::from_utf8_lossy(&n).into_owned(), f.written(&n).unwrap().to_vec())).collect()
}

/// gen_worldframes.py's `written` line from the files before and after:
/// those created or changed, each its size and digest, deleted ones `-`;
/// TEMPSAVE.$$$'s third plane counted as zeros where the floor plane's
/// segment `floor` (D748) is 0, or where the plane is 0000:0000's bytes
/// (`low`, written where D748 was 0 between the case's start and end), as
/// there.
fn written_line(pre: &std::collections::BTreeMap<String, Vec<u8>>, now: &std::collections::BTreeMap<String, Vec<u8>>, floor: u16, low: &[u8]) -> String {
    let names: std::collections::BTreeSet<&String> = pre.keys().chain(now.keys()).collect();
    names.into_iter().filter_map(|k| match (now.get(k), pre.get(k)) {
        (None, _) => Some(format!("{k}:-")),
        (Some(n), was) if was != Some(n) => {
            let mut d = n.clone();
            // (What changes there after the write aside: the BIOS's tick
            // count, 0040:006C, in the harness's; the movie player's chunk
            // header, 0000:64B7..64BC, in the port's image.)
            let masked = |b: &[u8]| -> Vec<u8> { b.iter().enumerate().map(|(i, &x)| if (0x46C..0x470).contains(&i) || (0x64B7..0x64BD).contains(&i) { 0 } else { x }).collect() };
            let from_low = d.len() > 0x164BD && low.get(..d.len() - 0x10000).is_some_and(|l| masked(l) == masked(&d[0x10000..]));
            if k == "TEMPSAVE.$$$" && (floor == 0 || from_low) && d.len() > 0x10000 {
                d[0x10000..].fill(0);
            }
            Some(format!("{k}:{:x}:{:x}", d.len(), fnv(d.iter().copied())))
        }
        _ => None,
    }).collect::<Vec<_>>().join(" ")
}

/// `state_compared` against the vectors at `path`, the case `c`.
fn state_compared_in(path: &str, name: &str, from_load: bool, c: Case) {
    let kind = if from_load { "stateload" } else { "state" };
    let text = std::fs::read_to_string(path).expect(path);
    let Some(f) = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 4 && f[0] == kind && f[1] == name) else {
        panic!("{name}: no {kind} line");
    };
    let mut want = vec![false; 0x10000];
    for run in f[2].split_whitespace() {
        let (s, n) = run.split_once('+').unwrap();
        let (s, n) = (usize::from_str_radix(s, 16).unwrap(), usize::from_str_radix(n, 16).unwrap());
        want[s..s + n].iter_mut().for_each(|x| *x = true);
    }
    let digest = u64::from_str_radix(f[3], 16).unwrap();
    let skip = if from_load { state_load_excluded() } else { state_excluded() };
    let (place, setup, play) = c;
    let pre = std::cell::RefCell::new(None);
    let pre_files = std::cell::RefCell::new(std::collections::BTreeMap::new());
    let Some((a, _)) = played(place, |a| {
        a.engine.page = Some(vec![0; 64000]);
        setup(a);
        *pre.borrow_mut() = Some(grab(a).0);
        *pre_files.borrow_mut() = files_written(&a.engine.files);
    }, play) else { return };
    // The files the frames wrote, as the original's (gen_worldframes.py's
    // `written` line; none where it has none).
    if path == VECTORS && !from_load {
        let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "written" && f[1] == name).map_or(String::new(), |f| f[2].to_string());
        let now = files_written(&a.engine.files);
        // (The floor plane's segment where TEMPSAVE.$$$ was written: 0
        // when it is 0 as the case begins or as it ends, as the generator
        // counts it; a case leaving the area ends with the next area's,
        // e.g. playlastpiece's scenes keeping the dungeon's world.)
        let start_floor = pre.borrow().as_ref().map_or(1, |d: &Vec<u8>| u16::from_le_bytes([d[0xD748], d[0xD749]]));
        let floor = if start_floor == 0 { 0 } else { a.engine.m.mem.r16(DS, 0xD748) };
        let got = written_line(&pre_files.into_inner(), &now, floor, a.engine.m.mem.segment(0));
        // (With ARENA_SCREENS holding gen_worldframes.py's dumps, NAME.file.F
        // the original's: the engine's beside them as NAME.port.F.)
        if got != want {
            if let Ok(d) = std::env::var("ARENA_SCREENS") {
                for (k, v) in &now {
                    let _ = std::fs::write(PathBuf::from(&d).join(format!("{name}.port.{k}")), v);
                }
            }
        }
        assert_eq!(got, want, "{name}: the files written differ from the original's");
    }
    let before = if from_load { BASE.with(|b| b.borrow_mut().take()).unwrap().0 } else { pre.into_inner().unwrap() };
    let after = grab(&a).0;
    // (The drawing's own setting up, the original's done before the case
    // starts, the engine's copy's at its first page: not counted.)
    let extra: Vec<usize> = (0..0x10000).filter(|&i| before[i] != after[i] && !skip[i] && !want[i] && a.engine.page_wrote[i].is_none()).collect();
    let got = fnv((0..0x10000).filter(|&i| want[i]).map(|i| after[i]));
    let hx = |v: &[usize]| v.iter().take(24).map(|i| format!("{i:04x}")).collect::<Vec<_>>().join(" ");
    // From the load the two start alike (`the_load_as_the_original_leaves_it`)
    // but for loadexclude.txt's offsets, which aren't counted.
    let extra = if from_load {
        let mut skip = vec![false; 0x10000];
        excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/loadexclude.txt"), &mut skip);
        extra.into_iter().filter(|&i| !skip[i]).collect()
    } else {
        extra
    };
    assert!(extra.is_empty(), "{name}: the frames changed what the original's don't: {}", hx(&extra));
    let unchanged: Vec<usize> = (0..0x10000).filter(|&i| want[i] && before[i] == after[i]).collect();
    // (With ARENA_SCREENS holding gen_worldframes.py's or gen_areaplay.py's
    // dumps: the offsets of the changes whose bytes end otherwise.)
    if got != digest {
        if let Some(orig) = std::env::var("ARENA_SCREENS").ok().and_then(|d| std::fs::read(PathBuf::from(d).join(format!("{name}.ds"))).ok()) {
            let off: Vec<usize> = (0..0x10000).filter(|&i| want[i] && orig[i] != after[i]).collect();
            eprintln!("{name}: {kind}: ending otherwise: {}", off.iter().map(|i| format!("{i:04x} {:02x}/{:02x}", orig[*i], after[*i])).collect::<Vec<_>>().join(", "));
        }
    }
    assert_eq!(got, digest, "{name}: the state differs from the original's (of its changes, unchanged here: {})", hx(&unchanged));
}

/// Each case's frames change the EMS banks 0..1Fh as the original's do
/// (`stateems` lines: the same bytes changed, to the same values).
fn state_ems_case(name: &str) {
    state_ems_in(VECTORS, name, case(name).unwrap());
}

/// `state_ems_case` against the vectors at `path`, the case `c`.
fn state_ems_in(path: &str, name: &str, c: Case) {
    let text = std::fs::read_to_string(path).expect(path);
    let Some(f) = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 4 && f[0] == "stateems" && f[1] == name) else {
        panic!("{name}: no stateems line");
    };
    let (count, offs) = f[2].split_once(' ').unwrap();
    let want = (usize::from_str_radix(count, 16).unwrap(), u64::from_str_radix(offs, 16).unwrap(), u64::from_str_radix(f[3], 16).unwrap());
    let (place, setup, play) = c;
    let pre = std::cell::RefCell::new(None);
    let Some((a, _)) = played(place, |a| {
        a.engine.page = Some(vec![0; 64000]);
        setup(a);
        *pre.borrow_mut() = Some(grab(a));
    }, play) else { return };
    let (ds0, before) = pre.into_inner().unwrap();
    let after = grab(&a).1;
    // (With ARENA_STATE_DUMP naming a directory: the banks here before and
    // after, and DGROUP before and after, to compare with
    // gen_worldframes.py's dumps.)
    if let Ok(d) = std::env::var("ARENA_STATE_DUMP") {
        let d = PathBuf::from(d);
        let _ = std::fs::write(d.join(format!("{name}.port.ds0")), &ds0);
        let _ = std::fs::write(d.join(format!("{name}.port.ems0")), &before);
        let _ = std::fs::write(d.join(format!("{name}.port.ems")), &after);
        let _ = std::fs::write(d.join(format!("{name}.port.ds")), grab(&a).0);
    }
    let changed: Vec<usize> = (0..after.len()).filter(|&i| before[i] != after[i]).collect();
    let got = (changed.len(), fnv(changed.iter().flat_map(|&i| (i as u32).to_le_bytes())), fnv(changed.iter().map(|&i| after[i])));
    // (With ARENA_SCREENS holding gen_worldframes.py's dumps: the runs of
    // bytes, bank and offset, that end otherwise or change only on one
    // side.)
    if got != want {
        let dumps = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from);
        if let Some((orig0, orig)) = dumps.and_then(|d| Some((std::fs::read(d.join(format!("{name}.ems0"))).ok()?, std::fs::read(d.join(format!("{name}.ems"))).ok()?))) {
            let odd: Vec<usize> = (0..after.len().min(orig.len())).filter(|&i| (orig0[i] != orig[i] || before[i] != after[i]) && orig[i] != after[i]).collect();
            let mut runs: Vec<(usize, usize)> = Vec::new();
            for &i in &odd {
                match runs.last_mut() {
                    Some((s, n)) if *s + *n == i => *n += 1,
                    _ => runs.push((i, 1)),
                }
            }
            eprintln!("{name}: stateems: {} bytes otherwise: {}", odd.len(), runs.iter().take(40).map(|&(s, n)| format!("{:02x}:{:04x}+{n:x}", s >> 16, s & 0xFFFF)).collect::<Vec<_>>().join(" "));
            for &(s, n) in runs.iter().take(4) {
                let hx = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
                eprintln!("  {:02x}:{:04x}: original {} -> {}, here {} -> {}", s >> 16, s & 0xFFFF, hx(&orig0[s..s + n]), hx(&orig[s..s + n]), hx(&before[s..s + n]), hx(&after[s..s + n]));
            }
        }
    }
    assert_eq!(got, want, "{name}: the EMS banks' changes differ from the original's ({} bytes changed here)", changed.len());
}

/// Each case's frames change the EMS banks as the original's do
/// (`state_ems_case`). Not the stairs' (playdown): three bytes the
/// original's frames change already hold their value as the engine's
/// case starts (no byte ends otherwise, the report finds).
#[test]
fn the_ems_banks_as_the_original_leaves_them() {
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playcrypt", "playfog", "playcap", "playsky"] {
        state_ems_case(name);
    }
}

/// From the load, the way into each case's place and its frames leave
/// the game's state as the original's do (`state_load_case`).
#[test]
fn the_state_from_the_load_as_the_original_leaves_it() {
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playdown", "playcrypt", "playfog", "playcap", "playsky"] {
        state_load_case(name);
    }
}

/// Each case's frames leave the game's state as the original's do
/// (`state_case`): the street by day, at night, in rain and snow, the
/// wilderness, a dungeon and its stairs, the crypt, fog.
#[test]
fn the_state_as_the_original_leaves_it() {
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playdown", "playcrypt", "playfog", "playcap", "playsky", "playghostclick", "playwildedge"] {
        state_case(name);
    }
}

/// The busy cases: tens to hundreds of frames of play each (a crowd at
/// noon, a fight swung through, a spell cast into a creature, a creature
/// casting, the wilderness's night encounter, the stairs taken mid-fight,
/// a rest ended by enemies, a lock picked and its door walked through,
/// water swum in and levitated over, a potion and a ring used, the
/// automap and the character sheet opened mid-fight, the sheet's
/// equipment page, a store's buying,
/// selling and repairing, a potion bought in the mages' guild, the main
/// quest's informant paid in a tavern, a townsperson asked the way).
// (One name a line, in order, so that cases added on different branches
// merge on their own.)
const BUSY: &[&str] = &[
    "playaudience8",
    "playaudience9",
    "playautomap",
    "playautomapend",
    "playbarjump",
    "playboatsheet",
    "playbook",
    "playbookend",
    "playbowsheet",
    "playcamp",
    "playcast",
    "playcastsheet",
    "playchest",
    "playchestpick",
    "playcreaturecast",
    "playcrowd",
    "playcurepoison",
    "playdawn",
    "playdream",
    "playeffectsheet",
    "playequip",
    "playequipcantuse",
    "playequipclash",
    "playequipdescribe",
    "playequipdrop",
    "playequipdropin",
    "playequipdropwhat",
    "playequipnoroom",
    "playequipscroll",
    "playequipspellpage",
    "playequipspells",
    "playequipstaff",
    "playequipwear",
    "playexitmapend",
    "playfight",
    "playfortify",
    "playguards",
    "playguild",
    "playguildbar",
    "playheal",
    "playholiday",
    "playhouselocked",
    "playhousepick",
    "playimppal",
    "playinformant",
    "playinformantpaid",
    "playinvisible",
    "playjagar",
    "playjourneyend",
    "playkeyssheet",
    "playlastpiece",
    "playlevelup",
    "playlevitate",
    "playlight",
    "playlightout",
    "playlog",
    "playlogbar",
    "playlogend",
    "playlogprint",
    "playloot",
    "playlootitems",
    "playlootstreet",
    "playlootwild",
    "playmapend",
    "playmidnight",
    "playopen",
    "playoptionsend",
    "playoptionsload",
    "playpalaceaccept",
    "playpalacenight",
    "playparalyse",
    "playpick",
    "playpiece",
    "playpile",
    "playpoisontick",
    "playquestreached",
    "playquestreward",
    "playquickcast",
    "playreflect",
    "playresist",
    "playrest",
    "playriddleright",
    "playriddlewrong",
    "playscreens",
    "playsearchend",
    "playsheet",
    "playsheetend",
    "playsheetready",
    "playsilence",
    "playspellmaker",
    "playstairsfight",
    "playstatusend",
    "playstore",
    "playstorecaught",
    "playstoreclosed",
    "playstorefailed",
    "playstorehaggle",
    "playstoresellhaggle",
    "playstorestolen",
    "playsurrender",
    "playsurrenderspell",
    "playswim",
    "playtalkbar",
    "playtalkkey",
    "playtalkpalace",
    "playtalkquest",
    "playtalkquestplace",
    "playtalkwhere",
    "playtaverndrink",
    "playtavernroom",
    "playtemplebless",
    "playtemplecure",
    "playtempleheal",
    "playtextsheet",
    "playuse",
    "playuselist",
    "playwildambush",
    "playwildhouse",
    "playwildnight",
];

/// The logbook's Print in playlogprint hands PRN what the original's
/// does: each of the log's lines with an LF CR, then the form feed
/// (27E3:0272, DOS's cooked write, `Files::printed`).
#[test]
fn the_logbook_prints_as_the_original() {
    let (place, setup, play) = case("playlogprint").unwrap();
    let Some((a, _)) = played(place, setup, play) else { return };
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "printed" && f[1] == "playlogprint").expect("the printed line")[2].to_string();
    let p = a.engine.files.printed();
    assert_eq!(format!("{:x} {:x}", p.len(), fnv(p.iter().copied())), want, "what PRN took differs from the original's");
}

/// Each busy case's player, frame by frame, as the original's.
#[test]
fn busy_play_goes_as_the_originals() {
    // (ARENA_CASE: that case alone.)
    let only = std::env::var("ARENA_CASE").ok();
    for name in BUSY.iter().copied().filter(|n| only.as_deref().is_none_or(|o| o == *n)) {
        let (place, setup, play) = case(name).unwrap();
        let Some((_, seen)) = played(place, setup, play) else { return };
        assert_eq!(seen, track(name), "{name}: the player's frames differ from the original's");
    }
}

/// Each case's drawings make their row tables from what the original's
/// did (the `rows` lines: the inputs as the last 0000:DA8E found them,
/// which the tables keep while the state changes on, `render::rows_read`
/// and `Machine::rows_note` at each of DA8E's callers), in every case of
/// worldframes.txt and areaplay.txt the view plays. (ARENA_CASE: that
/// case alone.)
#[test]
fn the_row_tables_made_as_the_original_makes_them() {
    let only = std::env::var("ARENA_CASE").ok();
    let mut bad = Vec::new();
    for path in [VECTORS, areaplaytest::AREAPLAY] {
        let text = std::fs::read_to_string(path).expect(path);
        for f in text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).filter(|f| f.len() == 3 && f[0] == "rows") {
            let name = f[1];
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let Some((place, setup, play)) = case(name).or_else(|| areaplaytest::transit_case(name)) else { continue };
            let Some((mut a, _)) = played(place, |a| {
                setup(a);
                a.engine.m.rows_drawn = Some(Vec::new());
            }, play) else { return };
            let drawn = a.engine.m.rows_drawn.take().unwrap();
            if only.is_some() {
                for (i, v) in drawn.iter().enumerate() {
                    eprintln!("{name} rows#{} {}", i + 1, v.iter().map(|w| format!("{w:x}")).collect::<Vec<_>>().join(" "));
                }
            }
            let digest = fnv(drawn.iter().flat_map(|v| v.iter().flat_map(|w| w.to_le_bytes())));
            let got = format!("{} {digest:x}", drawn.len());
            if got != f[2] {
                eprintln!("{name}: the drawings' row tables differ from the original's: {got} against {}", f[2]);
                bad.push(name.to_string());
            }
        }
    }
    assert!(bad.is_empty(), "the row tables differ from the original's in {bad:?}");
}

/// The busy cases whose state is compared, with the shorter ones whose
/// state only these checks cover: the volcano seen near and far (its
/// image loaded into the sky's bank, 292B:029A), the lightning's bolt
/// (2294:05A6: the y it reads from the image an earlier frame's drawing
/// left, `Engine::page_ems_prior`), the fog's first frame, the ghost and
/// the locked door clicked; the store's and the guild's
/// shopping among them (the shops' screens as the engine's: 1194:5496 and
/// 54B3, the list windows 1194:5734 and 3159:06A0, the strip 5F8C/5FC0,
/// the dialogs 4E88 and messages 4D31, the texts drawn into the page),
/// the informant's (the logbook's entry appended to LOG.nn, its oldest
/// dropped through bank 1Fh: `Machine::log_add`), and the character
/// sheet's in playscreens and playsheet (the world kept and given back
/// around it, 0000:A611 and A6B5; its files, banks 17h and [1030] and
/// numbers, 1194:0CF8: `Machine::sheet_open`, `sheet_close`) and its
/// equipment page's in playequip (1194:0D67 and 16BD: BACKMSG.$$$ and
/// banks 4, 5 and [1032], the list's and the quests' lines:
/// `Machine::sheet_equipment`, `sheet_equipment_close`), and its buttons'
/// in playequipscroll, playequipwear and playequipdrop (the arrows, the
/// list's click and the drop with its question: 1194:1A4A, 1A94, 1AC6,
/// 17D8: `sheet_equipment_scroll`, `sheet_equipment_click`,
/// `sheet_equipment_drop`), and the staff's quest line marked, described
/// and clicked again in playequipstaff (1F56 into the DI the page's loop
/// carries, 5536h after a description: `page_di`), and the spell's page
/// opened from the spells' list and left in playequipspellpage (51D1: its
/// screen's state, 68D7's, the effects' texts at 1044 and CHARSPEL.IMG in
/// bank [1032]: `Machine::sm_screen`), and "Drop what?" with no line
/// marked in playequipdropwhat (18CA: its loop, 2325, left by a line's
/// click that marks it, the drop over again: `drop_what_loop`), and the
/// logbook paged and left in playlog (2542:02A3, 27E3:0088: the log read
/// into bank [1030], its lines split there, each page's lines copied to
/// 1044 and drawn, the page kept in LOG.$$$: `Machine::log_open`,
/// `log_turn`, `log_close`), and camping for two hours in playcamp
/// (2542:01E6, the camp's menu, the hours typed and counted down to
/// "done": 1194:424E, 46BF, 456F), and the last spell cast again by
/// Shift+C in playquickcast (2542:056A without the list:
/// `Machine::spellbook_last`), and the use list's and the spellbook's in
/// playuselist and playbook (the book's names sorted by 0000:0B9C; 2542:
/// 06F8 keeps the page in bank [1032] as the frame's input step finds
/// it: the drawing's rows 0..146, before the compass and C3A2's clear,
/// over the interface), and a townsperson's "Where is" in playtalkwhere
/// and playtalkpalace (1194:DF7F's lists of kind 9 through
/// `list_window`, A872 the DI each caller leaves: E037's two short of
/// the talk menu's table, E0A7's the sorted names'; the page the list
/// keeps the screen shown, its picture left on it).
// (One name a line, in order, so that cases added on different branches
// merge on their own.)
const BUSY_STATE: &[&str] = &[
    "playaudience8",
    "playaudience9",
    "playautomapend",
    "playbarjump",
    "playboatsheet",
    "playbolt",
    "playbook",
    "playbookend",
    "playbowsheet",
    "playcamp",
    "playcast",
    "playcastsheet",
    "playchest",
    "playchestpick",
    "playcreaturecast",
    "playcrowd",
    "playcurepoison",
    "playdawn",
    "playdream",
    "playeffectsheet",
    "playequip",
    "playequipcantuse",
    "playequipclash",
    "playequipdescribe",
    "playequipdrop",
    "playequipdropin",
    "playequipdropwhat",
    "playequipnoroom",
    "playequipscroll",
    "playequipspellpage",
    "playequipspells",
    "playequipstaff",
    "playequipwear",
    "playexitmapend",
    "playfight",
    "playfogfirst",
    "playfortify",
    "playghost",
    "playguards",
    "playguild",
    "playguildbar",
    "playheal",
    "playholiday",
    "playhouselocked",
    "playhousepick",
    "playimppal",
    "playinformant",
    "playinformantpaid",
    "playinvisible",
    "playjagar",
    "playjourneyend",
    "playkeyssheet",
    "playlastpiece",
    "playlevelup",
    "playlevitate",
    "playlight",
    "playlightout",
    "playlock",
    "playlog",
    "playlogbar",
    "playlogend",
    "playlogprint",
    "playloot",
    "playlootitems",
    "playlootstreet",
    "playlootwild",
    "playmapend",
    "playmidnight",
    "playopen",
    "playoptionsend",
    "playoptionsload",
    "playpalaceaccept",
    "playpalacenight",
    "playparalyse",
    "playpick",
    "playpiece",
    "playpile",
    "playpoisontick",
    "playquestreached",
    "playquestreward",
    "playquickcast",
    "playreflect",
    "playresist",
    "playrest",
    "playriddleright",
    "playriddlewrong",
    "playscreens",
    "playsearchend",
    "playsheet",
    "playsheetend",
    "playsheetready",
    "playsilence",
    "playspellmaker",
    "playstairsfight",
    "playstatusend",
    "playstore",
    "playstorecaught",
    "playstoreclosed",
    "playstorefailed",
    "playstorehaggle",
    "playstoresellhaggle",
    "playstorestolen",
    "playsurrender",
    "playsurrenderspell",
    "playswim",
    "playtalkbar",
    "playtalkkey",
    "playtalkpalace",
    "playtalkquest",
    "playtalkquestplace",
    "playtalkwhere",
    "playtaverndrink",
    "playtavernroom",
    "playtemplebless",
    "playtemplecure",
    "playtempleheal",
    "playtextsheet",
    "playuse",
    "playuselist",
    "playvolcano",
    "playvolcanofar",
    "playwildambush",
    "playwildhouse",
    "playwildnight",
];

/// The cases whose doors, locks or chests change the walls plane, which
/// neither DGROUP nor the EMS banks hold (gen_worldframes.py's
/// WALLS_CASES).
const WALLS_CASES: [&str; 8] = ["playriddleright", "playriddlewrong", "playpick", "playdoormoving", "playlock", "playchest", "playchestpick", "playopen"];

/// Those cases leave the walls plane ([D806], the level's width by its
/// depth in words, C357, C359) as the original's do: the riddle's door
/// opened or not, the lock picked, the chest opened.
#[test]
fn the_walls_left_as_the_original() {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    for name in WALLS_CASES {
        let Some((place, setup, play)) = case(name) else { return };
        let Some((a, _)) = played(place, setup, play) else { return };
        let m = &a.engine.m;
        let (seg, w, d) = (m.mem.r16(DS, 0xD806), m.mem.r16(DS, 0xC357), m.mem.r16(DS, 0xC359));
        let n = w as usize * d as usize * 2;
        let got = format!("{:x} {:x}", n, fnv((0..n).map(|i| m.mem.r8(seg, i as u16))));
        let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "walls" && f[1] == name).unwrap_or_else(|| panic!("{name}: no walls line"))[2].to_string();
        assert_eq!(got, want, "{name}: the walls plane differs from the original's");
    }
}

/// The spell cases (gen_worldframes.py's SPELL_CASES): their effects
/// land outside DGROUP and the EMS banks, so the creature segment and
/// the timers are compared too.
const SPELL_CASES: [&str; 12] = [
    "playheal", "playlight", "playopen", "playsilence", "playlightout", "playresist", "playfortify", "playinvisible", "playreflect", "playparalyse",
    "playcurepoison", "playpoisontick",
];

/// The cases whose projectiles and actors are compared frame by frame
/// (gen_worldframes.py's OBJECT_CASES).
const OBJECT_CASES: &[&str] = &[
    "playcast",
    "playcreaturecast",
    "playfight",
    "playparalyse",
    "playquickcast",
    "playsurrenderspell",
];

/// The missile and fight cases' projectiles and actors, frame by frame,
/// as the original's (the `objects` lines: each track entry's
/// `objects_digest`), not only as the last frame leaves them: a fireball
/// a frame late can end where the original's ends. With the engine's
/// page, as the state comparison runs. (ARENA_CASE: that case alone.)
#[test]
fn missiles_and_actors_go_as_the_originals_frame_by_frame() {
    let only = std::env::var("ARENA_CASE").ok();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut bad = vec![];
    for name in OBJECT_CASES.iter().copied().filter(|n| only.as_deref().is_none_or(|o| o == *n)) {
        let (place, setup, play) = case(name).unwrap();
        let Some((_, seen)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        }, play) else { return };
        let got = OBJECTS.with(|o| o.borrow().clone());
        let want: Vec<u64> = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "objects" && f[1] == name)
            .unwrap_or_else(|| panic!("{name}: no objects line"))[2].split(' ').map(|x| u64::from_str_radix(x, 16).unwrap()).collect();
        assert_eq!(got.len(), seen.len(), "{name}: an objects entry per track entry");
        let differ: Vec<usize> = (0..got.len().max(want.len())).filter(|&k| got.get(k) != want.get(k)).collect();
        if !differ.is_empty() {
            bad.push(format!("{name}: frames {differ:?} of {} (the original's {})", got.len(), want.len()));
        }
    }
    assert!(bad.is_empty(), "the projectiles or actors differ from the original's: {bad:#?}");
}

/// The spell cases leave the eight active creatures' records (0C57's
/// entries: +12h the record in the segment at [1042], 41Eh bytes) and
/// the timed spell flags' table ([1020], 9 x 40h bytes) as the
/// original's do (gen_worldframes.py's `creatures_line`): the touch
/// spell's flag on its creature, the timers counting.
#[test]
fn the_spells_leave_creatures_and_timers_as_the_original() {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut bad = vec![];
    for name in SPELL_CASES {
        let Some((place, setup, play)) = case(name) else { return };
        // With the engine's page, as the state comparison runs (the
        // touch's target is what the ported renderer's hit test finds).
        let Some((a, _)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        }, play) else { return };
        let m = &a.engine.m;
        let seg = m.mem.r16(DS, 0x1042);
        let recs = (0..8u16).flat_map(|k| {
            let off = m.mem.r16(DS, 0x0C57 + 0x1C * k + 0x12);
            (0..0x41Eu16).map(move |i| m.mem.r8(seg, off.wrapping_add(i)))
        });
        let (toff, tseg) = (m.mem.r16(DS, 0x1020), m.mem.r16(DS, 0x1022));
        let table = (0..9 * 0x40u16).map(|i| m.mem.r8(tseg, toff.wrapping_add(i)));
        let got = format!("{:x} {:x}", fnv(recs), fnv(table));
        let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "creatures" && f[1] == name).unwrap_or_else(|| panic!("{name}: no creatures line"))[2].to_string();
        if got != want {
            bad.push(format!("{name}: {got} against the original's {want}"));
        }
    }
    assert!(bad.is_empty(), "the creatures (first digest) or the spell timers (second) differ from the original's: {bad:#?}");
}

/// Each busy case's frames leave DGROUP as the original's do, from the
/// case's start and from the load (`state_case`, `state_load_case`).
#[test]
fn busy_play_leaves_the_state_as_the_original() {
    for name in BUSY_STATE.iter().copied() {
        state_case(name);
        state_load_case(name);
    }
}

/// The case ARENA_STATE_CASE names, while working on it: its DGROUP from
/// the case's start and from the load, and its EMS banks (nothing
/// without it).
#[test]
fn one_case_leaves_the_state_as_the_original() {
    let Ok(name) = std::env::var("ARENA_STATE_CASE") else { return };
    // (All three run, each one's failure reported, before the test fails.)
    let failed: Vec<&str> = [("state", state_case as fn(&str)), ("stateload", state_load_case), ("stateems", state_ems_case)]
        .into_iter()
        .filter(|(_, check)| std::panic::catch_unwind(|| check(&name)).is_err())
        .map(|(kind, _)| kind)
        .collect();
    assert!(failed.is_empty(), "{name}: {} differ", failed.join(", "));
}

/// Each busy case's frames change the EMS banks as the original's do
/// (`state_ems_case`).
#[test]
fn busy_play_leaves_the_ems_banks_as_the_original() {
    let only = std::env::var("ARENA_CASE").ok();
    for name in BUSY_STATE.iter().copied().filter(|n| only.as_deref().is_none_or(|o| o == *n)) {
        state_ems_case(name);
    }
}

/// The player killed in the dungeon (`playdeath`): the frame that ends the
/// world loop, then the outer loop's death (0000:0CE1..0D43, the save's
/// F77 9: no vision) as the view takes it on (`App::area_next`): the
/// player's frames, DGROUP from the case's start and from the load, and
/// the EMS banks, as the original leaves them at the death's RET.
#[test]
fn a_death_leaves_the_state_as_the_original() {
    let (place, setup, play) = case("playdeath").unwrap();
    let Some((_, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track("playdeath"), "playdeath: the player's frames differ from the original's");
    state_case("playdeath");
    state_load_case("playdeath");
    state_ems_case("playdeath");
}

/// The player killed (`playdeath`), then the program's main back to its
/// menu (`playtitle`: 0000:109B..1114, `Machine::title_menu`: the intro's
/// movies and pictures read, MENU.IMG drawn): DGROUP from the case's
/// start and from the load, the EMS banks, and the page the menu waits
/// on, as the original leaves them at the menu's wait.
#[test]
fn a_death_goes_back_to_the_menu_as_the_original() {
    let (place, setup, play) = case("playtitle").unwrap();
    let Some((a, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track("playtitle"), "playtitle: the player's frames differ from the original's");
    let m = &a.engine.m;
    let (off, seg) = (m.mem.r16(DS, 0xA8FE), m.mem.r16(DS, 0xA900));
    let page: Vec<u8> = (0..64000u16).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix("screen | playtitle | ")).expect("a screen line");
    assert_eq!(format!("{:x}", fnv(page)), want.split(' ').next().unwrap(), "playtitle: the menu's page differs from the original's");
    state_case("playtitle");
    state_load_case("playtitle");
    state_ems_case("playtitle");
}

/// `playtitle`, then the menu's Load (`playtitleload`: its pick, 1194:5CF4;
/// 0000:117E's set-up on the title's state; the load menu, 25FD:0242;
/// slot 1): DGROUP from the case's start and from the load and the EMS
/// banks as the original leaves them where the loaded game's world loop is
/// about to run its first frame, and the interface the load drew into the
/// page (its rows from 146 on). The rest of the original's page is the
/// load menu's picture under its "Kindly waiteth" box, and the portrait,
/// the name and the bars (columns 14..80) are the HUD's: both the front
/// end's here (`saveui`, `hud`).
#[test]
fn the_menus_load_after_a_death_as_the_original() {
    title_load_case("playtitleload");
}

/// tests/vectors/titleplay.txt (gen_titleplay.py): games begun from the
/// program's own start.
const TITLEPLAY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/titleplay.txt");

/// The app as `--create` starts it (creation's main menu over the start
/// dungeon, the title's state the original's program start leaves at the
/// menu, `world::title`) with the save copy as its save folder, and its
/// driver; None without the game or the save.
fn started_at_menu() -> Option<(App, Driver, Scratch)> {
    started_at_menu_with(None)
}

/// `started_at_menu`, with `sound` the sound driver loaded where main's
/// command line loads it (`world::title_sound`), the original's answers
/// its calls' (`Machine::digital`) from there on.
fn started_at_menu_with(sound: Option<arena::digital::Replay>) -> Option<(App, Driver, Scratch)> {
    let dir = game_dir()?;
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return None;
    };
    let mut engine = world::load(&dir, "START.MIF");
    let archive = Rc::new(archive::Archive::open(&dir));
    let c = create::Creation::new(&mut engine, archive, &dir, world::palette(&dir));
    let mut a = make_app(dir.clone(), engine, arena::transit::Run::Start, Some(c), Default::default(), Some(save.to_path_buf()), 0, false);
    a.title_engine = match sound {
        Some(r) => world::title_sound(&dir, &mut arena::digital::Replayed).map(|mut e| {
            e.m.digital = Some(r);
            e
        }),
        None => world::title(&dir),
    };
    assert!(a.title_engine.is_some(), "the program's start to its menu");
    let d = Driver::new(&mut a);
    Some((a, d, save))
}

/// `name`'s line of TITLEPLAY against the app's game: DGROUP whole (the
/// harness's form, loadexclude.txt's offsets left out: `load_canonical`),
/// the EMS banks whole, and the interface's rows of the page (as
/// `title_load_case`'s). `kept`: loadsegs.txt's words the path never
/// gives a segment (`load_canonical_kept`).
fn titleplay_compared(name: &str, a: &App, kept: &[usize]) {
    let text = std::fs::read_to_string(TITLEPLAY).expect(TITLEPLAY);
    let want: Vec<&str> = text.lines().find_map(|l| l.strip_prefix(&format!("titleplay | {name} | "))).expect("a titleplay line").split(' ').collect();
    let (ds, ems) = grab(a);
    let m = &a.engine.m;
    let (off, seg) = (m.mem.r16(DS, 0xA8FE), m.mem.r16(DS, 0xA900));
    let page: Vec<u8> = (0..64000u16).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
    let rows: Vec<u8> = (146 * 320..64000).map(|i| if (14..=80).contains(&(i % 320)) { 0 } else { page[i] }).collect();
    let canon = load_canonical_kept(&a.dir, &ds, kept);
    if let Ok(d) = std::env::var("ARENA_STATE_DUMP") {
        let d = PathBuf::from(d);
        let _ = std::fs::write(d.join(format!("{name}.port.ds")), &canon);
        let _ = std::fs::write(d.join(format!("{name}.port.ems")), &ems);
    }
    assert_eq!(format!("{:x}", fnv(canon)), want[0], "{name}: DGROUP differs from the original's");
    assert_eq!(format!("{:x}", fnv(ems)), want[1], "{name}: the EMS banks differ from the original's");
    assert_eq!(format!("{:x}", fnv(rows)), want[2], "{name}: the interface differs from the original's");
}

/// The program started, its main on to the menu (`world::title`), the
/// menu's Load (`startload`: its pick, 117E's set-up, the load menu) and
/// slot 1's row: the game loaded the title's way, where its world loop is
/// about to run its first frame, as the original's.
#[test]
fn the_menus_load_at_the_start_as_the_original() {
    let Some((mut a, mut d, _save)) = started_at_menu() else { return };
    a.open_saveui(crate::saveui::Kind::TitleLoad);
    assert!(a.saveui.is_some(), "the title's load menu");
    d.click(&mut a, 160, 21, MouseButton::Left);
    assert!(a.saveui.is_none() && a.title_engine.is_none() && a.creation.is_none(), "loaded");
    titleplay_compared("startload", &a, &[]);
}

/// The program started, its main on to the menu (`world::title`), the
/// menu's Start, the opening story run its course, and one character made
/// (`creation`: gen_titleplay.py's CREATION, its inputs as the view's
/// screens take them: the class from the list, Acrobat; "ria"; female;
/// Morrowind; every bonus point on the first attribute; the second face),
/// its vision clicked through: the game set up and the start dungeon
/// entered, where its world loop is about to run its first frame, as the
/// original's: DGROUP (A83C left unshifted: no load writes a segment
/// there in a new game), all 32 EMS banks and the interface.
#[test]
fn a_new_game_from_the_menu_as_the_original() {
    new_game_from_the_menu("creation", LISTED, None);
}

/// The same with the class the questions give (gen_titleplay.py's
/// `creationquiz`: 222C:0000): G, the instructions' box taken, the ten
/// questions answered A, B, C in turn from A, the class offered taken
/// with Y, then the rest as `creation`.
#[test]
fn a_new_game_by_the_questions_as_the_original() {
    new_game_from_the_menu("creationquiz", QUIZ, None);
}

/// The questions with ARENA.BAT's sound driver loaded
/// (`creationquizsound`): BURST5.VOC readied for the driver (23BA:0183's
/// 85h), played at each answer (014A), stopped at the end (01A4), then
/// the vision's speech; every call into the driver as the original's.
#[test]
fn a_new_game_by_the_questions_with_sound_on_as_the_original() {
    new_game_from_the_menu("creationquizsound", QUIZ, Some(titleplay_replay("creationquizsound")));
}

/// The class from the list: S, Acrobat (its first line) clicked twice.
const LISTED: &str = "S,click:94:51,click:94:51";

/// The class by the questions (`a_new_game_by_the_questions_as_the_original`).
const QUIZ: &str = "G,enter,A,B,C,A,B,C,A,B,C,A,Y";

/// The same with ARENA.BAT's sound driver loaded as main's command line
/// loads it (gen_titleplay.py's `creationsound`): creation's vision's
/// speech played through it, no button pressed (its pages turned as its
/// parts end), every call into the driver as the original's, and the game
/// then as the original's.
#[test]
fn a_new_game_with_sound_on_as_the_original() {
    new_game_from_the_menu("creationsound", LISTED, Some(titleplay_replay("creationsound")));
}

/// TITLEPLAY's `driver` line for `name`, its runs spelt out.
fn titleplay_calls(name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(TITLEPLAY).expect(TITLEPLAY);
    let line = text.lines().find_map(|l| l.strip_prefix(&format!("driver | {name} | "))).unwrap_or_else(|| panic!("{name}: a driver line"));
    line.split(' ').filter(|w| !w.is_empty()).flat_map(|w| {
        let (c, n) = w.split_once('*').map_or((w, 1), |(c, n)| (c, n.parse().unwrap()));
        std::iter::repeat_n(c.to_string(), n)
    }).collect()
}

/// The original driver's answers to `name`'s status calls, in turn.
fn titleplay_replay(name: &str) -> arena::digital::Replay {
    let answers = titleplay_calls(name).iter().filter_map(|c| c.strip_prefix("7C=")).map(|v| u16::from_str_radix(v, 16).unwrap()).collect();
    arena::digital::Replay { answers, calls: Vec::new() }
}

/// The menu's New on the program's start's game, `name`'s character made
/// (its class by `class`, the rest as gen_titleplay.py's CREATION) and its
/// vision run, against TITLEPLAY's `name` (and with `sound` its calls into
/// the driver).
fn new_game_from_the_menu(name: &str, class: &str, sound: Option<arena::digital::Replay>) {
    let with_sound = sound.is_some();
    let Some((mut a, _d, _save)) = started_at_menu_with(sound) else { return };
    let stage = |a: &App| a.creation.as_ref().map(|c| c.phase());
    let wait_until = |a: &mut App, done: &dyn Fn(&App) -> bool, what: &str| {
        for _ in 0..20_000 {
            if done(a) {
                return;
            }
            a.creation_input(create::Input::Wait(50));
        }
        panic!("{what}: still {:?}", stage(a));
    };
    // The intro's movie skipped and the title left (the view's own
    // screens: the game's state is the program's start's, `title_engine`).
    for inp in create::parse_script("enter,esc") {
        a.creation_input(inp);
    }
    wait_until(&mut a, &|a| stage(a).is_some_and(|p| p.2 == 5 && p.0 == 0), "the main menu");
    a.creation_input(create::Input::Char(b'S'));
    assert!(a.creation_from_title, "creation on the program's start's game");
    // The opening story with no input: its movie and nine pictures.
    wait_until(&mut a, &|a| stage(a).is_some_and(|p| p.2 == 0 && p.0 == 0), "the opening story");
    let clicks = std::iter::repeat_n("click:41:50", 16).collect::<Vec<_>>().join(",");
    let script = format!("{class},type:ria,enter,F,enter,click:240:80,Y,enter,enter,enter,enter,enter,{clicks},click:10:175,S,enter,click:240:20,click:240:20,click:10:175");
    for (i, inp) in create::parse_script(&script).into_iter().enumerate() {
        assert!(a.creation.is_some(), "creation ended early at step {i} ({inp:?})");
        a.creation_input(inp);
    }
    // The vision's pages clicked through, until the game starts.
    for _ in 0..200 {
        if a.creation.is_none() {
            break;
        }
        a.creation_input(create::Input::Click(160, 100));
        a.creation_input(create::Input::Wait(50));
    }
    assert!(a.creation.is_none(), "the game not started: {:?}", stage(&a));
    // Each box creation showed read the game's page (`Creation::game_box`):
    // the page then the screen the window drew behind it (the frames the
    // createframes vectors check against the original's), pixel for pixel.
    let pages = create::BOX_PAGES.with(|b| std::mem::take(&mut *b.borrow_mut()));
    assert!(!pages.is_empty(), "{name}: no box read the game's page");
    for (back, differ) in &pages {
        assert_eq!(*differ, 0, "{name}: the page under a box on the {back:?} screen differs from the screen in {differ} pixels");
    }
    if with_sound {
        let got = a.engine.m.digital.as_ref().expect("the replay").calls.clone();
        let want = titleplay_calls(name);
        if let Some(i) = (0..want.len().max(got.len())).find(|&i| want.get(i) != got.get(i)) {
            let around = |v: &[String]| v[i.saturating_sub(3)..(i + 3).min(v.len())].join(" ");
            panic!("{name}: the calls into the sound driver differ from the original's at {i} of {}: {} against the original's {}", want.len(), around(&got), around(&want));
        }
    }
    titleplay_compared(name, &a, &[0xA83C]);
}

/// A case that ends where a game the title's Load loaded is about to run
/// its first frame: its track, DGROUP from the case's start and from the
/// load, the EMS banks, and the interface's rows of the page (see
/// `the_menus_load_after_a_death_as_the_original`).
fn title_load_case(name: &str) {
    let (place, setup, play) = case(name).unwrap();
    let Some((a, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track(name), "{name}: the player's frames differ from the original's");
    state_ems_case(name);
    state_case(name);
    state_load_case(name);
    let m = &a.engine.m;
    let (off, seg) = (m.mem.r16(DS, 0xA8FE), m.mem.r16(DS, 0xA900));
    let page: Vec<u8> = (0..64000u16).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
    let rows: Vec<u8> = (146 * 320..64000).map(|i| if (14..=80).contains(&(i % 320)) { 0 } else { page[i] }).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix(&format!("interface | {name} | "))).expect("an interface line");
    assert_eq!(format!("{:x}", fnv(rows)), want, "{name}: the interface the load drew differs from the original's");
}

/// The options screen's New (`playnew`: 2542:0627, then the frame that
/// ends the world loop, the outer loop's way out and the program's main
/// back to its menu, 0000:109B..1114): DGROUP from the case's start and
/// from the load, the EMS banks, and the page the menu waits on, as the
/// original leaves them at the menu's wait.
#[test]
fn new_goes_back_to_the_menu_as_the_original() {
    let (place, setup, play) = case("playnew").unwrap();
    let Some((a, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track("playnew"), "playnew: the player's frames differ from the original's");
    let m = &a.engine.m;
    let (off, seg) = (m.mem.r16(DS, 0xA8FE), m.mem.r16(DS, 0xA900));
    let page: Vec<u8> = (0..64000u16).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix("screen | playnew | ")).expect("a screen line");
    assert_eq!(format!("{:x}", fnv(page)), want.split(' ').next().unwrap(), "playnew: the menu's page differs from the original's");
    state_case("playnew");
    state_load_case("playnew");
    state_ems_case("playnew");
}

/// Slot 1 loaded in play between the street's frames (`playloadplay`,
/// the load menu's pick): the game loaded plays on, its 3D view drawn
/// over the page the street's left, every pixel, its frames, DGROUP from
/// the case's start and from the load, and the EMS banks, as the
/// original's (the engine the load makes keeps the drawing's page and
/// writes, `Engine::keep_drawing`).
#[test]
fn a_load_in_play_plays_on_as_the_original() {
    let (place, setup, play) = case("playloadplay").unwrap();
    let Some((_, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track("playloadplay"), "playloadplay: the player's frames differ from the original's");
    let Some((ok, same)) = soft_view("playloadplay") else { return };
    assert!(ok, "playloadplay: the 3D view after the load differs from the original's ({same:?}% alike)");
    state_case("playloadplay");
    state_load_case("playloadplay");
    state_ems_case("playloadplay");
}

/// The options screen's New clicked on the screen Esc opens inside the
/// frame (`playnewend`): the rest of that frame ends the world loop (no
/// frame more), then the way out and main back to the menu, as playnew.
#[test]
fn new_in_the_frame_goes_back_to_the_menu_as_the_original() {
    let (place, setup, play) = case("playnewend").unwrap();
    let Some((a, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track("playnewend"), "playnewend: the player's frames differ from the original's");
    let m = &a.engine.m;
    let (off, seg) = (m.mem.r16(DS, 0xA8FE), m.mem.r16(DS, 0xA900));
    let page: Vec<u8> = (0..64000u16).map(|i| m.mem.r8(seg, off.wrapping_add(i))).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix("screen | playnewend | ")).expect("a screen line");
    assert_eq!(format!("{:x}", fnv(page)), want.split(' ').next().unwrap(), "playnewend: the menu's page differs from the original's");
    state_case("playnewend");
    state_load_case("playnewend");
    state_ems_case("playnewend");
}

/// A death at the main quest's third stage (`playfarewell`: F77 3, so
/// the outer loop's death plays the farewell's vision, 1194:D459, with
/// the install's speech, its pages clicked through): DGROUP from the
/// case's start and from the load, and the EMS banks, as the original
/// leaves them at the death's RET (0000:0D43).
#[test]
fn a_death_plays_the_farewell_as_the_original() {
    let (place, setup, play) = case("playfarewell").unwrap();
    let Some((_, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track("playfarewell"), "playfarewell: the player's frames differ from the original's");
    state_case("playfarewell");
    state_load_case("playfarewell");
    state_ems_case("playfarewell");
}

/// The same with the sound driver loaded as the ending begins
/// (`playfarewellsound`: ARENA.BAT's SBPDIG.ADV, run in the harness on a
/// Sound Blaster Pro, harness/sbdig.py): the speech played through it, its
/// pages turned as its parts end, no click; then with one click, which
/// ends it, the part playing stopped (`playfarewellsoundclick`). The
/// calls the speech makes into the driver, and its state, as the
/// original's.
#[test]
fn a_death_plays_the_farewell_through_the_sound_driver_as_the_original() {
    for name in ["playfarewellsound", "playfarewellsoundclick"] {
        let (place, setup, play) = case(name).unwrap();
        let Some((_, seen)) = played(place, setup, play) else { return };
        let replay = SPEECH_REPLAY.with(|r| r.borrow_mut().take()).expect("the speech replayed");
        let r = replay.borrow();
        assert!(r.wrong.is_empty(), "{name}: the speech's calls into the driver differ from the original's: {:?}", r.wrong);
        assert!(r.calls.is_empty(), "{name}: the original made {} calls more, the first {:?}", r.calls.len(), r.calls.front());
        assert_eq!(seen, track(name), "{name}: the player's frames differ from the original's");
        state_case(name);
        state_load_case(name);
        state_ems_case(name);
    }
}

/// The original's sound driver's answers to the speech (a case's
/// `driver` line: gen_worldframes.py's, from SBPDIG.ADV run on a Sound
/// Blaster Pro, harness/sbdig.py), given back in turn, and each call the
/// speech makes checked against the original's: 7Bh the VOC file handed
/// (its block, the digest of the FFDCh bytes there), 7Dh, 7Ch (answered
/// as the original's driver answered), 7Eh. Its load answered as
/// SBPDIG.ADV answers it: the first handle (0), its descriptor at offset
/// 73h of its image (its function 64h returns CS:0073).
struct SpeechReplay {
    calls: std::collections::VecDeque<String>,
    image: u16,
    wrong: Vec<String>,
}

impl SpeechReplay {
    fn of(name: &str) -> SpeechReplay {
        let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
        let line = text.lines().find_map(|l| l.strip_prefix(&format!("driver | {name} | "))).unwrap_or_else(|| panic!("{name}: a driver line"));
        let mut calls = std::collections::VecDeque::new();
        for w in line.split(' ') {
            let (c, n) = w.split_once('*').map_or((w, 1), |(c, n)| (c, n.parse().unwrap()));
            for _ in 0..n {
                calls.push_back(c.to_string());
            }
        }
        SpeechReplay { calls, image: 0, wrong: Vec::new() }
    }

    /// The original's next call, `made` checked against it (a status
    /// asked, 7C, against its 7C=answer).
    fn next(&mut self, made: String) -> Option<String> {
        let want = self.calls.pop_front();
        let same = want.as_deref().is_some_and(|w| w == made || (made == "7C" && w.starts_with("7C=")));
        if !same && self.wrong.len() < 8 {
            self.wrong.push(format!("{made} where the original made {}", want.as_deref().unwrap_or("none")));
        }
        want
    }
}

/// A `SpeechReplay` the test keeps a hold of while the speech uses it.
struct Shared(std::rc::Rc<std::cell::RefCell<SpeechReplay>>);

impl arena::digital::DigitalDriver for Shared {
    fn register(&mut self, _m: &mut arena::machine::Machine, seg: u16, _off: u16) -> u16 {
        self.0.borrow_mut().image = seg;
        0
    }
    fn describe(&mut self, _m: &mut arena::machine::Machine, _handle: u16) -> (u16, u16) {
        (self.0.borrow().image, 0x73)
    }
    fn detect(&mut self, _m: &mut arena::machine::Machine, _h: u16, _io: u16, _irq: u16, _dma: u16, _drq: u16) -> u16 {
        1
    }
    fn init(&mut self, _m: &mut arena::machine::Machine, _h: u16, _io: u16, _irq: u16, _dma: u16, _drq: u16) {}
    fn play_voc(&mut self, m: &mut arena::machine::Machine, _h: u16, seg: u16, off: u16, block: u16) {
        let data = (0..0xFFDCu16).map(|i| m.mem.r8(seg, off.wrapping_add(i)));
        self.0.borrow_mut().next(format!("7B:{block:x}:{:x}", fnv(data)));
    }
    fn start(&mut self, _m: &mut arena::machine::Machine, _h: u16) {
        self.0.borrow_mut().next("7D".into());
    }
    fn status(&mut self, _m: &mut arena::machine::Machine, _h: u16) -> u16 {
        let want = self.0.borrow_mut().next("7C".into());
        want.and_then(|w| w.strip_prefix("7C=").map(|v| u16::from_str_radix(v, 16).unwrap())).unwrap_or(0)
    }
    fn stop(&mut self, _m: &mut arena::machine::Machine, _h: u16) {
        self.0.borrow_mut().next("7E".into());
    }
}

/// gen_soundplay.py's vectors: worldframes cases with the sound driver
/// loaded at the program's start.
const SOUND_VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/soundplay.txt");

/// The cases gen_soundplay.py runs with sound on: footsteps in water, a
/// door picked and walked through, a fight, a spell, a level gained (its
/// fanfare, FANFARE1.VOC through 23BA:012B), the guards called (HALT.VOC).
const SOUND_CASES: [&str; 6] = ["playswim", "playpick", "playfight", "playcast", "playlevelup", "playguards"];

/// A `driver` line of SOUND_VECTORS, its runs (`7C=2*3`) spelt out.
fn sound_calls(name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(SOUND_VECTORS).expect(SOUND_VECTORS);
    let line = text.lines().find_map(|l| l.strip_prefix(&format!("driver | {name} | "))).unwrap_or_else(|| panic!("{name}: a driver line"));
    let mut out = Vec::new();
    for w in line.split(' ').filter(|w| !w.is_empty()) {
        let (c, n) = w.split_once('*').map_or((w, 1), |(c, n)| (c, n.parse().unwrap()));
        out.extend(std::iter::repeat_n(c.to_string(), n));
    }
    out
}

/// The original driver's answers to `name`'s status calls (7Ch), in turn,
/// for the engine's replay (`Machine::digital`).
fn sound_replay(name: &str) -> arena::digital::Replay {
    let answers = sound_calls(name).iter().filter_map(|c| c.strip_prefix("7C=")).map(|v| u16::from_str_radix(v, 16).unwrap()).collect();
    arena::digital::Replay { answers, calls: Vec::new() }
}

/// The calls the engine made into the driver (`got`) against the
/// original's (`name`'s line): the first that differs, and around it.
fn sound_calls_check(name: &str, got: &[String]) {
    let want = sound_calls(name);
    if let Some(i) = (0..want.len().max(got.len())).find(|&i| want.get(i) != got.get(i)) {
        let around = |v: &[String]| v[i.saturating_sub(3)..(i + 3).min(v.len())].join(" ");
        panic!("{name}: the calls into the sound driver differ from the original's at {i} of {}: {} against the original's {}", want.len(), around(got), around(&want));
    }
}

/// With the sound driver loaded at the program's start (gen_soundplay.py:
/// `world::boot_sound`), the title's Load leaves the game's state as the
/// original's (`loadstate`, `loadems`: the level's sound files read into
/// their EMS banks, 23BA:0217), and makes the original's calls into the
/// driver (`driver | load`: the sound stopped, 23BA:000C; each file
/// formatted, 85h, its far pointer and handle read from the sound buffer
/// as the original's are).
#[test]
fn the_load_with_sound_on_as_the_original_leaves_it() {
    SOUND_CASE.with(|c| *c.borrow_mut() = Some(SOUND_CASES[0].into()));
    let r = played(Place::Guild, |_| {}, |_, _, _| {});
    SOUND_CASE.with(|c| c.borrow_mut().take());
    let Some(_) = r else { return };
    let (mine, banks) = BASE.with(|b| b.borrow_mut().take()).unwrap();
    sound_calls_check("load", &LOAD_CALLS.with(|c| c.take()));
    let text = std::fs::read_to_string(SOUND_VECTORS).expect(SOUND_VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix("loadstate | ")).expect("a loadstate line");
    assert_eq!(format!("{:x}", fnv(load_canonical(&game_dir().unwrap(), &mine))), want, "the state after the load with sound on differs from the original's");
    let want = text.lines().find_map(|l| l.strip_prefix("loadems | ")).expect("a loadems line");
    assert_eq!(format!("{:x}", fnv(banks)), want, "the EMS banks after the load with sound on differ from the original's");
}

/// gen_soundplay.py's cases, with sound on: the player's frames, the
/// calls into the sound driver (each sound, 23BA:002A: the driver asked,
/// the sample copied from its bank to the driver's buffer, played at its
/// volume and started), DGROUP from the case's start and from the load,
/// and the EMS banks, as the original's. (ARENA_CASE: that case alone.)
#[test]
fn the_sounds_play_as_the_original() {
    let only = std::env::var("ARENA_CASE").ok();
    for name in SOUND_CASES {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        SOUND_CASE.with(|c| *c.borrow_mut() = Some(name.into()));
        let (place, setup, play) = case(name).unwrap();
        // (On a page, as the state's comparison has it: the frames' drawing
        // runs, as the original's.)
        let Some((a, seen)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a);
        }, play) else {
            SOUND_CASE.with(|c| c.borrow_mut().take());
            return;
        };
        assert_eq!(seen, track_in(SOUND_VECTORS, name), "{name}: the player's frames differ from the original's");
        sound_calls_check(name, &a.engine.m.digital.as_ref().expect("the replay").calls);
        state_compared_in(SOUND_VECTORS, name, false, case(name).unwrap());
        state_compared_in(SOUND_VECTORS, name, true, case(name).unwrap());
        state_ems_in(SOUND_VECTORS, name, case(name).unwrap());
        SOUND_CASE.with(|c| c.borrow_mut().take());
    }
}

/// gen_musicplay.py's vectors: worldframes cases with a music driver
/// loaded at the program's start.
const MUSIC_VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/musicplay.txt");

/// The cases gen_musicplay.py runs with music on: a town's street by day
/// and at night, the wilderness, a dungeon, a tavern (each starting its
/// song, 2294:04F5's), and a journey to another town (the street's song
/// faded and stopped, the town's started and asked after each frame).
const MUSIC_CASES: [&str; 6] = ["playstreet", "playnight", "playwild", "playdungeon", "playtaverndrink", "playjourneyon"];

/// A `music` line of MUSIC_VECTORS, its runs (`DD6:0=1*4`) spelt out.
fn music_calls(name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(MUSIC_VECTORS).expect(MUSIC_VECTORS);
    let line = text.lines().find_map(|l| l.strip_prefix(&format!("music | {name} | "))).unwrap_or_else(|| panic!("{name}: a music line"));
    let mut out = Vec::new();
    for w in line.split(' ').filter(|w| !w.is_empty()) {
        let (c, n) = w.rsplit_once('*').map_or((w, 1), |(c, n)| (c, n.parse().unwrap()));
        out.extend(std::iter::repeat_n(c.to_string(), n));
    }
    out
}

/// The original driver's answers in `name`'s line, in turn, for the
/// engine's replay (`Machine::music`).
fn music_replay(name: &str) -> arena::musicdrv::Replay {
    arena::musicdrv::Replay::of(&music_calls(name).join(" "))
}

/// The calls the engine made into the music driver (`got`) against the
/// original's (`name`'s line): the first that differs, and around it.
fn music_calls_check(name: &str, got: &[String]) {
    let want = music_calls(name);
    if let Some(i) = (0..want.len().max(got.len())).find(|&i| want.get(i) != got.get(i)) {
        let around = |v: &[String]| v[i.saturating_sub(3)..(i + 3).min(v.len())].join(" ");
        panic!("{name}: the calls into the music driver differ from the original's at {i} of {}: {} against the original's {}", want.len(), around(got), around(&want));
    }
}

/// With a music driver loaded at the program's start (gen_musicplay.py:
/// `world::boot_music`), the driver's load and the title's Load make the
/// original's calls into it (`music | load`: the image registered,
/// described, its device found and set going, the state table's and the
/// timbre cache's sizes asked and the cache given; the load's interior's
/// song, its name stale in this save, not read) and leave the game's
/// state as the original's (`loadstate`, `loadems`; the file-open hook's
/// segment word, A906, the harness's 1000h higher, left out).
#[test]
fn the_load_with_music_on_as_the_original_leaves_it() {
    MUSIC_CASE.with(|c| *c.borrow_mut() = Some(MUSIC_CASES[0].into()));
    let r = played(Place::Guild, |_| {}, |_, _, _| {});
    MUSIC_CASE.with(|c| c.borrow_mut().take());
    let Some(_) = r else { return };
    let (mine, banks) = BASE.with(|b| b.borrow_mut().take()).unwrap();
    music_calls_check("load", &LOAD_CALLS.with(|c| c.take()));
    let text = std::fs::read_to_string(MUSIC_VECTORS).expect(MUSIC_VECTORS);
    let want = text.lines().find_map(|l| l.strip_prefix("loadstate | ")).expect("a loadstate line");
    let mut d = load_canonical(&game_dir().unwrap(), &mine);
    d[0xA906..0xA908].fill(0);
    assert_eq!(format!("{:x}", fnv(d)), want, "the state after the load with music on differs from the original's");
    let want = text.lines().find_map(|l| l.strip_prefix("loadems | ")).expect("a loadems line");
    assert_eq!(format!("{:x}", fnv(banks)), want, "the EMS banks after the load with music on differ from the original's");
}

/// gen_musicplay.py's cases, with music on: the player's frames, the
/// calls into the music driver (a song started, 2F9F:001F: its file read
/// to the heap, its sequence registered, each timbre it asks for read
/// from SAMPLE.AD, 1194:4943, and installed, its channels' volume set, it
/// started at the music's volume; faded and stopped, 0438; asked after,
/// 000D), DGROUP from the case's start and from the load, and the EMS
/// banks, as the original's. (ARENA_CASE: that case alone.)
#[test]
fn the_music_plays_as_the_original() {
    let only = std::env::var("ARENA_CASE").ok();
    for name in MUSIC_CASES {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        MUSIC_CASE.with(|c| *c.borrow_mut() = Some(name.into()));
        let (place, setup, play) = case(name).unwrap();
        let Some((a, seen)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a);
        }, play) else {
            MUSIC_CASE.with(|c| c.borrow_mut().take());
            return;
        };
        assert_eq!(seen, track_in(MUSIC_VECTORS, name), "{name}: the player's frames differ from the original's");
        music_calls_check(name, &a.engine.m.music.as_ref().expect("the replay").calls);
        state_compared_in(MUSIC_VECTORS, name, false, case(name).unwrap());
        state_compared_in(MUSIC_VECTORS, name, true, case(name).unwrap());
        state_ems_in(MUSIC_VECTORS, name, case(name).unwrap());
        MUSIC_CASE.with(|c| c.borrow_mut().take());
    }
}

thread_local! {
    /// The case `played` runs with music on (gen_musicplay.py's).
    static MUSIC_CASE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    /// The case `played` runs with sound on (gen_soundplay.py's).
    static SOUND_CASE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    /// The calls the load made into the driver, with sound on.
    static LOAD_CALLS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The speech's replay the last case made (`SpeechReplay`).
    static SPEECH_REPLAY: std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<SpeechReplay>>>> = const { std::cell::RefCell::new(None) };
}

/// The screens opened in play whose state is the engine's
/// (`playautomap`: the automap screen, 0000:2CFE..2F0F, run as
/// `automap_screen_begin`, its loop's turns and `automap_screen_end`).
const SCREENS_STATE: [&str; 1] = ["playautomap"];

/// Each screen's case: the player's frames, DGROUP from the case's start
/// and from the load, and the EMS banks, as the original leaves them.
#[test]
fn the_screens_leave_the_state_as_the_original() {
    for name in SCREENS_STATE {
        let (place, setup, play) = case(name).unwrap();
        let Some((_, seen)) = played(place, setup, play) else { return };
        assert_eq!(seen, track(name), "{name}: the player's frames differ from the original's");
        state_case(name);
        state_load_case(name);
        state_ems_case(name);
    }
}

/// The town's welcome (0000:4ACD) keeps what its box covers at the
/// frame's start (4B18: into the load bank [1030] at offset 0, 0000:965E's
/// flag 1): the page as the last frame left it, which the view has as the
/// screen it last showed (`Assets::shown`). Four frames in the street,
/// the screen shown, then an arrival set (5C8 bit 100h, ENTERCTY.$$$ with
/// gen_welcome.py's words) and one more frame: the kept header and rows,
/// as the welcome left them (the harness's at its wait for a key; the
/// frame's 4DD6 loads the condition's picture over them after), against
/// tests/vectors/welcome.txt.
#[test]
fn the_welcome_keeps_the_screen_as_the_original() {
    // The 3D view drawn by the engine's renderer, as the window has it.
    let Some((mut a, _)) = played(Place::Street, |a| a.engine.page = Some(vec![0; 64000]), |a, d, seen| frames(a, d, seen, 4, |_| false)) else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/vectors/welcome.txt");
    let text = std::fs::read_to_string(path).expect("the welcome's vector");
    let f: Vec<&str> = text.lines().next().expect("a line").split(" | ").collect();
    a.compose();
    a.engine.files.put(b"ENTERCTY.$$$", b"You have arrived in a test town.\r\0");
    let v = a.engine.m.mem.r16(DS, 0x05C8) | 0x100;
    a.engine.m.mem.w16(DS, 0x05C8, v);
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let kept = crate::WELCOME_KEPT.with(|k| k.borrow_mut().take()).expect("no welcome");
    let w = |i: usize| u16::from_le_bytes([kept[2 * i], kept[2 * i + 1]]);
    let (x, y, wd, h) = (w(0) & 0x7FFF, w(1), w(2), w(3));
    assert_eq!(format!("{x:x} {y:x} {wd:x} {h:x}"), f[2], "the box's place");
    if let Ok(d) = std::env::var("ARENA_SCREENS") {
        std::fs::write(PathBuf::from(d).join("welcome.view"), &kept[..8 + wd as usize * h as usize]).ok();
    }
    let hash = fnv(kept[..8 + wd as usize * h as usize].iter().copied());
    assert_eq!(format!("{hash:x}"), f[3], "what the welcome kept differs from the original's");
    // The box's working words as 965E leaves them (its place, AX and BX,
    // what came before it leaves: the load's offset and the bank), and
    // the line it laid out last.
    let g: Vec<&str> = text.lines().find(|l| l.starts_with("welcomebox |")).expect("the box's line").split(" | ").collect();
    let d = crate::WELCOME_BOX.with(|k| k.borrow_mut().take()).expect("no welcome box");
    let w = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let words: Vec<String> = (0xA836..0xA85A).step_by(2).chain([0xADB6, 0xADB8]).map(|o| format!("{:x}", w(o))).collect();
    assert_eq!(words.join(" "), g[2], "the welcome box's working words (A836..A858, ADB6, ADB8)");
    let line: String = d[0x929E..].iter().take_while(|&&c| c != 0).map(|c| format!("{c:02x}")).collect();
    assert_eq!(line, g[3], "the welcome box's last line (929E)");
}


/// The two faces that never hide a light behind them (0000:B7A9 returns
/// at once): the crypt's diagonal wall (kind 0Dh, CB37 0Dh) faced from
/// two squares off (`playdiagonal`), and a door's face as the door moves
/// (`playdoormoving`: the first face drawn after the walk steps it, C398
/// bit 1000h, drawn with CCF8 FFFFh). In both a light the rule would
/// call behind the face is still counted. The track, every pixel of the
/// 3D view, and DGROUP and EMS from the case's start, as the original's.
#[test]
fn faces_that_hide_no_lights_as_the_original() {
    for name in ["playdiagonal", "playdoormoving"] {
        let Some((place, setup, play)) = case(name) else { return };
        let Some((_, seen)) = played(place, setup, play) else { return };
        assert_eq!(seen, track(name), "{name}: the player's frames differ from the original's");
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
        state_case(name);
    }
}