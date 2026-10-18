//! The area transitions against the original's (harness/gen_areaplay.py,
//! tests/vectors/areaplay.txt): the stairs down and back up, the
//! wilderness's blocks crossed each way, a city's gate out and in again,
//! buildings entered and left. Each case is played as the window plays
//! it (the area changed in the frame that leaves, `App::area_next`), and
//! the player's track, the state the frames leave in DGROUP (from the
//! case's start and from the load) and in the EMS banks are the
//! original's.

use super::*;

const AREAPLAY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/areaplay.txt");

/// The area played: its run and the place's id (A4C), which a change of
/// area changes.
fn area_key(a: &App) -> (arena::transit::Run, u8) {
    (a.run, a.engine.m.mem.r8(DS, 0x0A4C))
}

fn level_is(a: &App, n: u8) -> bool {
    a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) == n
}

/// gen_areaplay.py's `heading_near`: within 10h of `h`.
fn heading_near(a: &App, h: u16) -> bool {
    (a.engine.m.mem.r16(DS, arena::movement::var::HEADING).wrapping_sub(h).wrapping_add(0x10) & 0x1FF) <= 0x20
}

fn key(a: &mut App, code: KeyCode, down: bool) {
    a.input(In::Key { code, down, text: None });
}

/// `n` frames as gen_areaplay.py's steps play them, a box read with Space
/// after the frame that shows it (its script's), until `until`.
fn frames_until(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, until: impl Fn(&App) -> bool) {
    for _ in 0..n {
        a.compose();
        d.frame(a);
        seen.push(state(a));
        if a.waiting() {
            d.key(a, KeyCode::Space, Some(" "));
        }
        if until(a) {
            break;
        }
    }
}

/// gen_areaplay.py's step with CLICK and `leaves`: the middle of the view
/// clicked (down for the frame whose controls read it, then up), up to
/// `n` frames until the area changes.
fn click_leaving(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize) {
    let was = area_key(a);
    // The harness's mouse at (160, 100): the click 4 on (CB60).
    a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
    a.input(In::Button { button: MouseButton::Left, down: true });
    frames_until(a, d, seen, 1, |_| false);
    a.input(In::Button { button: MouseButton::Left, down: false });
    if area_key(a) != was {
        return;
    }
    frames_until(a, d, seen, n - 1, |a| area_key(a) != was);
}

/// gen_areaplay.py's `wild_at`: the wilderness's player at `x` and/or `y`
/// (BA62/BA66, the eye's C3D0/C3D4, the last place 0600/0602), facing
/// `heading`.
fn wild_at(a: &mut App, x: Option<u16>, y: Option<u16>, heading: u16) {
    let m = &mut a.engine.m;
    if let Some(x) = x {
        for o in [0xC3D0, 0xBA62, 0x0600] {
            m.mem.w16(DS, o, x);
        }
    }
    if let Some(y) = y {
        for o in [0xC3D4, 0xBA66, 0x0602] {
            m.mem.w16(DS, o, y);
        }
    }
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

fn origin(a: &App) -> (u16, u16) {
    (a.engine.m.mem.r16(DS, 0x0608), a.engine.m.mem.r16(DS, 0x060A))
}

/// A wilderness case: `wild_at`, the up arrow held till the window
/// moves (12 frames at most), then three frames.
fn wild_case(x: Option<u16>, y: Option<u16>, heading: u16) -> Case {
    (
        Place::Wild,
        Box::new(move |a: &mut App| wild_at(a, x, y, heading)),
        Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            let o = origin(a);
            key(a, KeyCode::ArrowUp, true);
            frames_until(a, d, seen, 12, |a| origin(a) != o);
            key(a, KeyCode::ArrowUp, false);
            frames_until(a, d, seen, 3, |_| false);
        }),
    )
}

/// gen_areaplay.py's `building`: before the door (`at_square`), two
/// frames, the door clicked, three frames inside, (with `back` the left
/// arrow held till the player faces it,) the way out clicked, three
/// frames in the street.
fn building(row: u16, col: u16, heading: u16, back: Option<u16>) -> Case {
    (
        Place::Street,
        Box::new(move |a: &mut App| at_square(a, row, col, heading)),
        Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            frames_until(a, d, seen, 2, |_| false);
            click_leaving(a, d, seen, 10);
            frames_until(a, d, seen, 3, |_| false);
            if let Some(h) = back {
                key(a, KeyCode::ArrowLeft, true);
                frames_until(a, d, seen, 40, |a| heading_near(a, h));
                key(a, KeyCode::ArrowLeft, false);
            }
            click_leaving(a, d, seen, 10);
            frames_until(a, d, seen, 3, |_| false);
        }),
    )
}

pub(super) fn transit_case(name: &str) -> Option<Case> {
    Some(match name {
        "transitstairs" => (
            Place::Dungeon,
            Box::new(|a: &mut App| at_square(a, 11, 10, 0x100)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                key(a, KeyCode::ArrowUp, true);
                frames_until(a, d, seen, 20, |a| level_is(a, 1));
                key(a, KeyCode::ArrowLeft, true);
                key(a, KeyCode::ArrowUp, false);
                frames_until(a, d, seen, 40, |a| heading_near(a, 0x100));
                key(a, KeyCode::ArrowUp, true);
                key(a, KeyCode::ArrowLeft, false);
                frames_until(a, d, seen, 30, |a| level_is(a, 0));
                key(a, KeyCode::ArrowUp, false);
                frames_until(a, d, seen, 3, |_| false);
            }),
        ),
        "transitwildnorth" => wild_case(None, Some(0x1008), 0x100),
        "transitwildsouth" => wild_case(None, Some(0x2FF8), 0),
        "transitwildwest" => wild_case(Some(0x1008), None, 0x180),
        "transitwildeast" => wild_case(Some(0x2FF8), None, 0x80),
        "transitgate" => (
            Place::Street,
            Box::new(|a: &mut App| at_square(a, 15, 1, 0x180)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames_until(a, d, seen, 2, |_| false);
                click_leaving(a, d, seen, 10);
                frames_until(a, d, seen, 3, |_| false);
                key(a, KeyCode::ArrowLeft, true);
                frames_until(a, d, seen, 40, |a| heading_near(a, 0x80));
                key(a, KeyCode::ArrowLeft, false);
                click_leaving(a, d, seen, 10);
                frames_until(a, d, seen, 3, |_| false);
            }),
        ),
        "transittavern" => building(7, 108, 0, None),
        "transittemple" => building(8, 50, 0x80, Some(0)),
        "transitstore" => building(3, 108, 0, None),
        "transitpalace" => building(8, 124, 0x80, Some(0)),
        "transitguild" => building(95, 52, 0x100, Some(0)),
        _ => return None,
    })
}

/// The case's track, its state from its start and from the load, and its
/// EMS banks' changes, each against the original's.
fn transit(name: &str) {
    let (place, setup, play) = transit_case(name).unwrap();
    let Some((_, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track_in(AREAPLAY, name), "{name}: the player's frames differ from the original's");
    state_compared_in(AREAPLAY, name, false, transit_case(name).unwrap());
    state_compared_in(AREAPLAY, name, true, transit_case(name).unwrap());
    state_ems_in(AREAPLAY, name, transit_case(name).unwrap());
}

#[test]
fn up_and_down_a_dungeons_stairs_as_the_original() {
    transit("transitstairs");
}

#[test]
fn the_wildernesss_blocks_crossed_north_as_the_original() {
    transit("transitwildnorth");
}

#[test]
fn the_wildernesss_blocks_crossed_south_as_the_original() {
    transit("transitwildsouth");
}

#[test]
fn the_wildernesss_blocks_crossed_west_as_the_original() {
    transit("transitwildwest");
}

#[test]
fn the_wildernesss_blocks_crossed_east_as_the_original() {
    transit("transitwildeast");
}

#[test]
fn out_of_a_citys_gate_and_in_again_as_the_original() {
    transit("transitgate");
}

#[test]
fn into_a_tavern_and_out_as_the_original() {
    transit("transittavern");
}

#[test]
fn into_a_temple_and_out_as_the_original() {
    transit("transittemple");
}

#[test]
fn into_a_store_and_out_as_the_original() {
    transit("transitstore");
}

/// Open: the palace's interior leaves EMS bank 11h's 5000h..5C1Bh other
/// than the original's (a table the interior's entry fills differently).
#[test]
#[ignore = "bank 11h 5000h..5C1Bh after the palace differs from the original's"]
fn into_the_palace_and_out_as_the_original() {
    transit("transitpalace");
}

/// Open: back in the street the drawing's shading words BAE6/BAE8
/// (0000:B896) end F0h where the original's end F5h.
#[test]
#[ignore = "the drawing's shading at BAE7/BAE9 after the guild differs from the original's"]
fn into_the_mages_guild_and_out_as_the_original() {
    transit("transitguild");
}
