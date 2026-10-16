//! The view's 2D screens in a dungeon against the original's
//! (harness/gen_dungeonscreens.py, tests/vectors/dungeonscreens.txt).
//!
//! The save is in Daggerfall's mages' guild and reaches no dungeon, so
//! both sides go into one the outer loop's way, from memory: the guild
//! left for the street, then the street left with the next area a
//! dungeon (5E6 3) and the location's dungeon seed (A56) `SEED`.

use super::*;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/dungeonscreens.txt");

/// The location's dungeon seed both sides make it from (a town's record
/// has none: from 0, 0000:28BC's picks of the stairs' chunks never
/// differ).
const SEED: u32 = 0x2F6E_1A35;

/// From the guild to the street, then into the dungeon made from `SEED`.
fn into_dungeon(a: &mut App) {
    area_left(a);
    a.engine.m.mem.w8(DS, arena::trigger::var::AREA, 3);
    a.engine.m.mem.w32(DS, 0x0A56, SEED);
    area_left(a);
    assert_eq!(a.engine.m.mem.r8(DS, arena::trigger::var::AREA), 3, "not in the dungeon");
}

/// A screen opened in the dungeon by `open`, its layer taken by `take`,
/// checked against the vector `name` (and its fade, dungeonfades.txt).
fn dungeon_screen(name: &str, open: impl FnOnce(&mut App, &mut Driver), take: impl FnOnce(&mut App) -> (Vec<u16>, Vec<u8>)) {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let (mut a, mut d) = loaded(&dir, &save);
    into_dungeon(&mut a);
    d.frame(&mut a);
    a.fade.clear();
    a.fade_old = None;
    open(&mut a, &mut d);
    check_fade_in("dungeonfades.txt", name, &mut a);
    let (idx, dac) = take(&mut a);
    assert!(check_in(VECTORS, name, &idx, &dac), "{name}: differs from the original's");
}

/// S: the status box (2542:037C).
#[test]
fn the_status_box_in_a_dungeon_as_the_original_draws_it() {
    dungeon_screen("dungeonstatus", |a, d| d.key(a, KeyCode::KeyS, Some("s")), over);
}

/// M: the automap (2542:045A), nothing of the level seen yet but where
/// the player stands.
#[test]
fn the_automap_in_a_dungeon_as_the_original_draws_it() {
    dungeon_screen(
        "dungeonautomap",
        |a, d| {
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyM, Some("m"));
        },
        layer,
    );
}

/// The automap with the whole level seen (every square of its map in
/// EMS bank 11h 15h): the level's walls, doors and stairs.
#[test]
fn a_dungeons_whole_level_on_the_automap_as_the_original_draws_it() {
    dungeon_screen(
        "dungeonmap",
        |a, d| {
            let m = &mut a.engine.m;
            m.ems_map(arena::automap::var::BANK);
            let seg = m.mem.r16(DS, arena::mif::var::BUFFER_SEG);
            for at in 0..0x4000u16 {
                m.mem.w8(seg, at, 0x15);
            }
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyM, Some("m"));
        },
        layer,
    );
}

/// R: camping (2542:01E6), allowed in a dungeon: its options.
#[test]
fn camping_in_a_dungeon_as_the_original_draws_it() {
    dungeon_screen(
        "dungeoncamp",
        |a, d| {
            d.key(a, KeyCode::KeyR, Some("r"));
            d.frame(a);
        },
        over,
    );
}

/// Shift+M: the world map (2542:03BF), refused: "You cannot travel from
/// here."
#[test]
fn the_world_map_refused_in_a_dungeon_as_the_original_draws_it() {
    dungeon_screen("dungeontravel", |a, d| super::super::press(d, a, KeyCode::KeyM, true), over);
}

/// The loot window as a click on a flat of treasure at the player's
/// square runs it (as screentest's `looted`, at the dungeon's square:
/// row << 8 | column << 1), the piles made there from LOOT_SEED.
fn looted_here(a: &mut App, _d: &mut Driver) {
    let m = &a.engine.m;
    let col = m.mem.r16(DS, arena::movement::var::X) >> 7;
    let row = m.mem.r16(DS, arena::movement::var::Y) >> 7;
    let square = (row << 8) | ((col << 1) & 0xFF);
    a.engine.m.mem.w32(DS, arena::random::SEED, LOOT_SEED);
    a.engine.m.rng.seed = LOOT_SEED;
    let before = copy(&a.engine);
    let input = TurnIn {
        intent: Default::default(),
        clicked: false,
        camp: false,
        shop: None,
        loot: Some([0x0F01, 0, DS, square]),
        key: false,
        tick: 0,
        right: false,
        drag: (0, 0),
        aspect: 1.6,
        note: None,
    };
    a.turn(before, input, Vec::new(), true);
    assert!(a.pending.is_some(), "the piles listed");
}

/// The piles made at the player's square in the dungeon, listed.
#[test]
fn the_loot_window_in_a_dungeon_as_the_original_draws_it() {
    dungeon_screen("dungeonloot", looted_here, over);
}

/// Its first line clicked: taken, the rest listed again.
#[test]
fn the_loot_window_in_a_dungeon_after_a_take_as_the_original_draws_it() {
    dungeon_screen(
        "dungeonloottake",
        |a, d| {
            looted_here(a, d);
            d.click(a, 0x60, 0x27, MouseButton::Left);
        },
        over,
    );
}

/// The first level's up stairs taken: the player turned to them (they
/// are the square north of the start, 0000:F76F's facing test; heading
/// 100h) and walked on with the forward key until the way out (0000:4537)
/// asks whether to leave by the travel map (26F7:0000, 1A81 set).
fn upstairs(a: &mut App, d: &mut Driver) {
    a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100);
    a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
    for _ in 0..10 {
        d.frame(a);
        if a.pending.is_some() {
            break;
        }
    }
    a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
    assert!(a.pending.is_some(), "the way out not asked");
    assert_eq!(a.engine.m.mem.r8(DS, arena::movement::var::LEVEL), 0xFF, "not the way out");
}

#[test]
fn leaving_a_dungeon_by_its_stairs_as_the_original_draws_it() {
    dungeon_screen("dungeonleave", upstairs, layer);
}
