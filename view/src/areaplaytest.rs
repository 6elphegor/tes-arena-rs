//! The area transitions against the original's (harness/gen_areaplay.py,
//! tests/vectors/areaplay.txt): the stairs down and back up, the
//! wilderness's blocks crossed each way, a city's gate out and in again,
//! buildings entered and left. Each case is played as the window plays
//! it (the area changed in the frame that leaves, `App::area_next`), and
//! the player's track, the state the frames leave in DGROUP (from the
//! case's start and from the load) and in the EMS banks are the
//! original's; where a box opens over a frame's drawing (the palace's
//! audience), that frame's 3D view is too (`boxview`).

use super::*;

pub(super) const AREAPLAY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/areaplay.txt");

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

thread_local! {
    /// Each frame's 3D view (the engine's page, its first 92h rows) as
    /// the frame left it, for `boxview`.
    static VIEWS: std::cell::RefCell<Vec<Option<u64>>> = const { std::cell::RefCell::new(Vec::new()) };
    /// A box opened over a screen of its own, not the 3D view (the travel
    /// map's journey box): the frame it opened in and that screen's first
    /// 92h rows, for `boxview` in place of the frame's view.
    static BOXED: std::cell::Cell<Option<(usize, u64)>> = const { std::cell::Cell::new(None) };
    /// The game's page as the first province's map opened left it
    /// (`Travel::province_page`), for `provincepage`.
    static PROVINCE: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// `n` frames as gen_areaplay.py's steps play them, a box read with Space
/// after the frame that shows it (its script's), until `until`.
fn frames_until(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, until: impl Fn(&App) -> bool) {
    for _ in 0..n {
        a.compose();
        d.frame(a);
        seen.push(state(a));
        VIEWS.with(|v| v.borrow_mut().push(a.engine.page.as_ref().map(|p| fnv(p[..0x92 * 320].iter().copied()))));
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

/// gen_areaplay.py's WILD_SOUTH: the up arrow held till the window moves
/// (12 frames at most), the player put back before the next quarter
/// (`then`), let go for a frame. (The harness's next step sees the top of
/// the frame again, after `then`: the place it was put.)
fn wild_south(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, then: impl FnOnce(&mut App)) {
    let o = origin(a);
    key(a, KeyCode::ArrowUp, true);
    frames_until(a, d, seen, 12, |a| origin(a) != o);
    then(a);
    *seen.last_mut().unwrap() = state(a);
    key(a, KeyCode::ArrowUp, false);
    frames_until(a, d, seen, 1, |_| false);
}

/// gen_areaplay.py's `before_entrance`: on the square south of WILD065's
/// entrance (row 49, column 12), 20h into it, facing north, the last
/// place the same.
fn before_entrance(a: &mut App) {
    let m = &mut a.engine.m;
    let (x, y) = ((12 << 7) + 0x40, (50 << 7) + 0x20);
    for o in [0xC3D0, 0xBA62, 0x0600] {
        m.mem.w16(DS, o, x);
    }
    for o in [0xC3D4, 0xBA66, 0x0602] {
        m.mem.w16(DS, o, y);
    }
    m.mem.w16(DS, arena::movement::var::HEADING, 0x100);
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
        "travelfast" => (
            Place::Street,
            Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames_until(a, d, seen, 2, |_| false);
                // Shift+M: the travel map; High Rock, the place nearest
                // (200, 100), its journey's box read, Travel.
                key(a, KeyCode::ShiftLeft, true);
                d.key(a, KeyCode::KeyM, Some("M"));
                key(a, KeyCode::ShiftLeft, false);
                // The frame that took the key (the screen up after it).
                let frame = VIEWS.with(|v| v.borrow().len());
                for (x, y) in [(70, 45), (200, 100), (200, 100), (62, 180)] {
                    d.click(a, x, y, MouseButton::Left);
                    // The journey's box opens over the province's map, in
                    // that frame.
                    if let Some(p) = a.travel.as_ref().and_then(|t| t.boxed_over.as_ref()) {
                        BOXED.set(Some((frame, fnv(p[..0x92 * 320].iter().copied()))));
                    }
                    if let Some(p) = a.travel.as_ref().and_then(|t| t.province_page.as_ref()) {
                        PROVINCE.set(Some(fnv(p.iter().copied())));
                    }
                }
                for _ in 0..100 {
                    if a.travel.is_none() {
                        break;
                    }
                    d.wait(a, 100);
                }
                assert!(a.travel.is_none(), "the journey ends the travel map");
                // The area left at that frame's end: the village entered.
                seen.push(state(a));
                frames_until(a, d, seen, 3, |_| false);
            }),
        ),
        "wilddungeon" => (
            Place::Wild,
            Box::new(|a: &mut App| wild_at(a, None, Some(0x2FF8), 0)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                wild_south(a, d, seen, |a| wild_at(a, None, Some(0x2FF8), 0));
                wild_south(a, d, seen, |a| wild_at(a, None, Some(0x2FF8), 0));
                wild_south(a, d, seen, before_entrance);
                frames_until(a, d, seen, 2, |_| false);
                click_leaving(a, d, seen, 10);
                frames_until(a, d, seen, 3, |_| false);
            }),
        ),
        _ => return None,
    })
}

/// The case's track, its state from its start and from the load, and its
/// EMS banks' changes, each against the original's.
fn transit(name: &str) {
    let (place, setup, play) = transit_case(name).unwrap();
    let Some((_, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track_in(AREAPLAY, name), "{name}: the player's frames differ from the original's");
    // The frame a box opened over (0000:8F2A keeping what it covers from
    // this frame's drawing): its 3D view the original's, every pixel
    // (the ceiling's rows too, from the row tables its last DA8E made),
    // the case played again with the engine's page.
    let text = std::fs::read_to_string(AREAPLAY).expect(AREAPLAY);
    for f in text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).filter(|f| f.len() == 3 && f[0] == "boxview" && f[1] == name) {
        let (frame, digest) = f[2].split_once(' ').unwrap();
        let (frame, digest): (usize, u64) = (frame.parse().unwrap(), u64::from_str_radix(digest, 16).unwrap());
        let (place, setup, play) = transit_case(name).unwrap();
        VIEWS.with(|v| v.borrow_mut().clear());
        BOXED.set(None);
        played(
            place,
            |a| {
                a.engine.page = Some(vec![0; 64000]);
                setup(a)
            },
            play,
        );
        let view = BOXED.take().filter(|&(f, _)| f == frame).map(|(_, v)| v).or_else(|| VIEWS.with(|v| v.borrow().get(frame).copied().flatten()));
        assert_eq!(view, Some(digest), "{name}: the 3D view the box opens over (frame {frame}) differs from the original's");
    }
    // The province's map as its picture and its places' icons leave the
    // game's page (26F7:01CD), the first opened.
    for f in text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).filter(|f| f.len() == 3 && f[0] == "provincepage" && f[1] == name) {
        let digest = u64::from_str_radix(f[2], 16).unwrap();
        let (place, setup, play) = transit_case(name).unwrap();
        PROVINCE.set(None);
        played(place, setup, play);
        assert_eq!(PROVINCE.take(), Some(digest), "{name}: the province's map's page differs from the original's");
    }
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

/// The Imperial palace's door at stage 9 (the artifact's audience, text
/// 5DDh, 0000:424F): the box opens in the click's frame after its drawing,
/// and saves what it covers (0000:8F2A, into bank 11h from offset 8) from
/// this frame's view, not the last shown (`EngineHooks::drawn`); a later
/// smaller box over the same buffer leaves the end of it, 5000h..5C1Bh.
#[test]
fn into_the_palace_and_out_as_the_original() {
    transit("transitpalace");
}

/// Back in the street by day nothing steps the light along a face, so
/// a side face's first shades (BAE6/BAE8, 0000:BE02..BE2D) come of the
/// light (A85E) the guild's last frame left: the drawing hands both back.
#[test]
fn into_the_mages_guild_and_out_as_the_original() {
    transit("transitguild");
}

/// Fast travel from the street (Shift+M, High Rock, Wind Keep, Travel):
/// the journey's days, the village entered, its welcome read, the
/// player's frames there the original's.
#[test]
fn fast_travel_frames_as_the_original() {
    let (place, setup, play) = transit_case("travelfast").unwrap();
    let Some((_, seen)) = played(place, setup, play) else { return };
    assert_eq!(seen, track_in(AREAPLAY, "travelfast"), "travelfast: the player's frames differ from the original's");
}

/// The state fast travel leaves: the frame that takes Shift+M run to its
/// end first (its hook C3A8 opens the screen, 2542:040A), the journey,
/// the date, the village and its errand (seeded from the BIOS's ticks),
/// the world kept and put back around the screen (0000:A611, A5CD, A6B5),
/// the screen's windows (3159), resources and last pass and its files in
/// EMS (banks 0, 12h, 15h, 18h), the area left at that frame's end; in
/// the village's fog (F81) its step's flags (2294:0B01: C398 bit 8000h
/// off, C39A bit 20h on) the machine's, so the near lights are made
/// (0000:E12E, C8ED) and the next frames' sky left out (D502). The
/// journey's box opens over the province's map (`Travel::boxed_over`).
/// The province's map's page, its picture and its places' icons drawn by
/// the game (0000:56C8, 26F7:0268), is the original's.
#[test]
fn fast_travel_as_the_original() {
    transit("travelfast");
}

/// WILD065's entrance (trigger 8) clicked after three crossings south:
/// the wilderness's dungeon entered (217B:04DC, 0000:08EC).
#[test]
fn into_a_wilderness_dungeon_as_the_original() {
    transit("wilddungeon");
}
