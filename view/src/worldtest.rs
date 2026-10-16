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
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
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

/// The player at the middle of a square, facing `heading`.
fn at_square(a: &mut App, row: u16, col: u16, heading: u16) {
    let m = &mut a.engine.m;
    for base in [arena::movement::var::X, 0xBA62] {
        m.mem.w16(DS, base, (col << 7) + 0x40);
        m.mem.w16(DS, base + 4, (row << 7) + 0x40);
    }
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

/// Where a case is played: the dungeon `into_dungeon` makes, or the main
/// quest's Crypt of Hearts (`into_crypt`).
#[derive(Clone, Copy)]
enum Place {
    Dungeon,
    Crypt,
}

/// From the guild to the street, then into the Crypt of Hearts the way
/// the outer loop enters a named level (0000:0993): the next area 2, no
/// interior, its .MIF named at 0A36.
fn into_crypt(a: &mut App) {
    area_left(a);
    let m = &mut a.engine.m;
    m.mem.w8(DS, arena::trigger::var::AREA, 2);
    m.mem.w16(DS, arena::transit::var::INTERIOR, 0);
    for (i, &c) in b"41649375.MIF\0".iter().enumerate() {
        m.mem.w8(DS, 0x0A36 + i as u16, c);
    }
    area_left(a);
}

/// Into the place, `setup` run, then `play` driving frames and
/// collecting the track; both against the original's.
fn world_case(name: &str, place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let (mut a, mut d) = loaded(&dir, &save);
    match place {
        Place::Dungeon => into_dungeon(&mut a),
        Place::Crypt => into_crypt(&mut a),
    }
    a.fade.clear();
    a.fade_old = None;
    setup(&mut a);
    let mut seen = vec![state(&a)];
    play(&mut a, &mut d, &mut seen);
    let want = track(name);
    assert_eq!(seen, want, "{name}: the player's frames differ from the original's");
    check_fade_in("worldframes.txt", name, &mut a);
    let (idx, dac) = layer(&mut a);
    assert!(check_in(VECTORS, name, &idx, &dac), "{name}: differs from the original's");
}

fn frames(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, until: impl Fn(&App) -> bool) {
    for _ in 0..n {
        d.frame(a);
        seen.push(state(a));
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
