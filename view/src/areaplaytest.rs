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

/// Screens' pixels' and colours' digests, in turn.
type Digests = Vec<(u64, u64)>;

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
    /// Each day of the journey as the screen shows it (its pixels' and
    /// its colours' digests), for `journey`.
    static JOURNEY: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The journey's fades before its days and after them, a retrace each
    /// (their pixels' and colours' digests), for `journeyshow`.
    static JOURNEY_FADES: std::cell::RefCell<(Digests, Digests)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
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

/// gen_areaplay.py's `before_wall`: beside the window's first wall
/// `word` (in row order) with a square beside it open or a flat (bit
/// 8000h, a building's sign), south, north, west or east tried in that
/// order: on that square 20h from the wall, centred across, facing it,
/// the last place the same.
fn before_wall(a: &mut App, word: u16) {
    let m = &mut a.engine.m;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let sq = |m: &arena::machine::Machine, r: i32, c: i32| m.mem.r16(walls, ((r as u16 & 0x7F) << 8) | ((c as u16 & 0x7F) << 1));
    let walkable = |v: u16| v == 0 || v & 0x8000 != 0;
    // (row step, column step, x within the square, y within it, heading)
    let sides = [(1, 0, 0x40, 0x20, 0x100), (-1, 0, 0x40, 0x60, 0), (0, -1, 0x60, 0x40, 0x80), (0, 1, 0x20, 0x40, 0x180)];
    let (row, col, (dr, dc, fx, fy, heading)) = (0..128)
        .flat_map(|r| (0..128).map(move |c| (r, c)))
        .filter(|&(r, c)| sq(m, r, c) == word)
        .find_map(|(r, c)| sides.iter().find(|s| walkable(sq(m, r + s.0, c + s.1))).map(|&s| (r, c, s)))
        .expect("no such wall in the window");
    let (x, y) = ((((col + dc) as u16) << 7) + fx, (((row + dr) as u16) << 7) + fy);
    for o in [0xC3D0, 0xBA62, 0x0600] {
        m.mem.w16(DS, o, x);
    }
    for o in [0xC3D4, 0xBA66, 0x0602] {
        m.mem.w16(DS, o, y);
    }
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

/// gen_areaplay.py's `wildtavern`, `wildtemple`: the window crossed north
/// twice (WILD063 in its first row), the player put before the wall
/// `word`, two frames, the door clicked, three frames inside.
fn wild_building(word: u16) -> Case {
    (
        Place::Wild,
        Box::new(|a: &mut App| wild_at(a, None, Some(0x1008), 0x100)),
        Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            wild_south(a, d, seen, |a| wild_at(a, None, Some(0x1008), 0x100));
            wild_south(a, d, seen, |a| before_wall(a, word));
            frames_until(a, d, seen, 2, |_| false);
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
        // High Rock, the place nearest (200, 100), its journey's box read,
        // Travel.
        "travelfast" => (
            Place::Street,
            Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| fast_travel(a, d, seen, &[(70, 45), (200, 100), (200, 100), (62, 180)])),
        ),
        // Poisoned (05B1 bit 0): Travel asks "Considering your
        // condition..." (1194:48DB's menu), its Yes (160, 118) travels
        // anyway, the poison working through the journey's days.
        "travelunwell" => (
            Place::Street,
            Box::new(|a: &mut App| {
                at_square(a, 95, 52, 0x100);
                let v = a.engine.m.mem.r8(DS, 0x05B1) | 1;
                a.engine.m.mem.w8(DS, 0x05B1, v);
            }),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| fast_travel(a, d, seen, &[(70, 45), (200, 100), (200, 100), (62, 180), (160, 118)])),
        ),
        // A main quest's dungeon of High Rock travelled to (Crypt of
        // Hearts, the staff's; Mines of Khuras, the map's), its picture
        // (292B:0362) left by its button, the dungeon entered.
        "travelhearts" => quest_dungeon_case(198, 62),
        "travelkhuras" => quest_dungeon_case(71, 132),
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
        "wildtavern" => wild_building(0x0B0B),
        "wildtemple" => wild_building(0x0C0C),
        // Two crossings east (WILD057 in the window), its crypt's 0909h
        // wall (trigger 1).
        "wildcrypt" => (
            Place::Wild,
            Box::new(|a: &mut App| wild_at(a, Some(0x2FF8), None, 0x80)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                wild_south(a, d, seen, |a| wild_at(a, Some(0x2FF8), None, 0x80));
                wild_south(a, d, seen, |a| before_wall(a, 0x0909));
                frames_until(a, d, seen, 2, |_| false);
                click_leaving(a, d, seen, 10);
                frames_until(a, d, seen, 3, |_| false);
            }),
        ),
        // A flat (800Eh, its low bits trigger 5's) clicked where the
        // player starts: no area left.
        "wildflat" => (
            Place::Wild,
            Box::new(|a: &mut App| before_wall(a, 0x800E)),
            Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
                frames_until(a, d, seen, 2, |_| false);
                click_leaving(a, d, seen, 10);
                frames_until(a, d, seen, 3, |_| false);
            }),
        ),
        _ => return None,
    })
}

/// gen_areaplay.py's `quest_dungeon`: from the street, High Rock's place
/// at (x, y) travelled to and its picture left by its button.
fn quest_dungeon_case(x: i32, y: i32) -> Case {
    (
        Place::Street,
        Box::new(|a: &mut App| at_square(a, 95, 52, 0x100)),
        Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| fast_travel(a, d, seen, &[(70, 45), (x, y), (x, y), (62, 180)])),
    )
}

/// The case's track, its state from its start and from the load, and its
/// EMS banks' changes, each against the original's.
/// Fast travel from the street: two frames, Shift+M (the travel map),
/// `clicks` on its screen, the journey's days to the screen's end, the
/// area left at that frame's end and three frames in the next.
fn fast_travel(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, clicks: &[(i32, i32)]) {
    frames_until(a, d, seen, 2, |_| false);
    key(a, KeyCode::ShiftLeft, true);
    d.key(a, KeyCode::KeyM, Some("M"));
    key(a, KeyCode::ShiftLeft, false);
    // The frame that took the key (the screen up after it).
    let frame = VIEWS.with(|v| v.borrow().len());
    for &(x, y) in clicks {
        d.click(a, x, y, MouseButton::Left);
        // The journey's box opens over the province's map, in that frame.
        if let Some(p) = a.travel.as_ref().and_then(|t| t.boxed_over.as_ref()) {
            BOXED.set(Some((frame, fnv(p[..0x92 * 320].iter().copied()))));
        }
        if let Some(p) = a.travel.as_ref().and_then(|t| t.province_page.as_ref()) {
            PROVINCE.set(Some(fnv(p.iter().copied())));
        }
    }
    // The journey's days, each as the screen shows it.
    if let Some(t) = a.travel.as_mut() {
        let mut m = a.engine.m.clone();
        let seen = t.journey_seen(&a.font, &mut m).into_iter().map(|(px, dac)| (fnv(px), fnv(dac))).collect();
        JOURNEY.set(seen);
        let (lead, tail) = t.journey_fades();
        let digests = |v: Vec<(Vec<u8>, Vec<u8>)>| v.into_iter().map(|(px, dac)| (fnv(px), fnv(dac))).collect::<Vec<_>>();
        JOURNEY_FADES.set((digests(lead), digests(tail)));
    }
    for _ in 0..100 {
        if a.travel.is_none() {
            break;
        }
        // A main quest's dungeon's picture, left by its button (the
        // harness's click at (295, 189), 292B:042B).
        if a.travel.as_ref().is_some_and(|t| t.picture_up()) {
            d.click(a, 295, 189, MouseButton::Left);
        }
        d.wait(a, 100);
    }
    assert!(a.travel.is_none(), "the journey ends the travel map");
    // The picture's button's fade out (292B:0362's 55B2) played, the frame
    // the screen was opened in run on to its end (`screen_ended`).
    for _ in 0..100 {
        if a.fade.is_empty() && a.inside.is_none() {
            break;
        }
        d.wait(a, 20);
    }
    // The area left at that frame's end: the next place entered.
    seen.push(state(a));
    frames_until(a, d, seen, 3, |_| false);
}


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
    // A journey's days as the screen shows them (1194:30F1), each with no
    // wait for the retrace nor read of the timer since the last: the view's
    // pace (a tick a day) is its own.
    let days: Vec<(u64, u64)> = text
        .lines()
        .map(|l| l.split(" | ").collect::<Vec<_>>())
        .filter(|f| f.len() == 3 && f[0] == "journey" && f[1] == name)
        .map(|f| {
            let v: Vec<&str> = f[2].split(' ').collect();
            assert_eq!((v[3], v[4]), ("0", "0"), "{name}: a day of the journey waits in the original");
            (u64::from_str_radix(v[1], 16).unwrap(), u64::from_str_radix(v[2], 16).unwrap())
        })
        .collect();
    if !days.is_empty() {
        let (place, setup, play) = transit_case(name).unwrap();
        JOURNEY.with(|j| j.borrow_mut().clear());
        played(place, setup, play);
        let seen = JOURNEY.with(|j| std::mem::take(&mut *j.borrow_mut()));
        assert_eq!(seen.len(), days.len(), "{name}: the journey's days differ from the original's");
        for (k, (s, o)) in seen.iter().zip(&days).enumerate() {
            assert_eq!(s.1, o.1, "{name}: day {k}'s colours differ from the original's");
            assert_eq!(s.0, o.0, "{name}: day {k}'s screen differs from the original's");
        }
    }
    // The journey as the screen shows it: before the days, each wait for
    // the retrace from Travel's first fade (55B2, then the world map's
    // colours faded in); after them, the last day faded out (55B2).
    let shows: Vec<(bool, u64, u64, usize)> = text
        .lines()
        .map(|l| l.split(" | ").collect::<Vec<_>>())
        .filter(|f| f.len() == 3 && f[0] == "journeyshow" && f[1] == name)
        .map(|f| {
            let v: Vec<&str> = f[2].split(' ').collect();
            (v[0] == "d", u64::from_str_radix(v[1], 16).unwrap(), u64::from_str_radix(v[2], 16).unwrap(), v[3].parse().unwrap())
        })
        .collect();
    if !shows.is_empty() {
        let expand = |runs: &[(bool, u64, u64, usize)]| runs.iter().flat_map(|&(_, s, c, n)| std::iter::repeat_n((s, c), n)).collect::<Vec<_>>();
        let first = shows.iter().position(|s| s.0).unwrap_or(shows.len());
        let last = shows.iter().rposition(|s| s.0).map_or(shows.len(), |i| i + 1);
        let (want_lead, want_tail) = (expand(&shows[..first]), expand(&shows[last..]));
        let (place, setup, play) = transit_case(name).unwrap();
        JOURNEY_FADES.with(|j| *j.borrow_mut() = Default::default());
        played(place, setup, play);
        let (lead, tail) = JOURNEY_FADES.with(|j| std::mem::take(&mut *j.borrow_mut()));
        for (what, seen, want) in [("before the days", lead, want_lead), ("after the days", tail, want_tail)] {
            for (k, (s, o)) in seen.iter().zip(&want).enumerate() {
                assert_eq!(s.1, o.1, "{name}: the journey's colours {what}, retrace {k}, differ from the original's");
                assert_eq!(s.0, o.0, "{name}: the journey's screen {what}, retrace {k}, differs from the original's");
            }
            assert_eq!(seen.len(), want.len(), "{name}: the journey's retraces {what} differ from the original's");
        }
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

/// Poisoned, the journey's question ("Considering your condition, you
/// may not survive this journey", 1194:48DB) answered Yes: the journey
/// made anyway, the days passing with the poison (0000:142D each), the
/// village entered, as the original.
#[test]
fn fast_travel_unwell_as_the_original() {
    transit("travelunwell");
}

/// WILD065's entrance (trigger 8) clicked after three crossings south:
/// the wilderness's dungeon entered (217B:04DC, 0000:08EC).
#[test]
fn into_a_wilderness_dungeon_as_the_original() {
    transit("wilddungeon");
}

/// A tavern in the wilderness (WILD063's 0B0Bh wall, the wilderness's
/// trigger 3) clicked after two crossings north: its interior entered,
/// its welcome read.
#[test]
fn into_a_wilderness_tavern_as_the_original() {
    transit("wildtavern");
}

/// A temple in the wilderness (WILD063's 0C0Ch wall, trigger 4), the same.
#[test]
fn into_a_wilderness_temple_as_the_original() {
    transit("wildtemple");
}

/// A crypt in the wilderness (WILD057's 0909h wall, trigger 1) clicked
/// after two crossings east: its interior entered.
#[test]
fn into_a_wilderness_crypt_as_the_original() {
    transit("wildcrypt");
}

/// A flat whose low bits are a trigger's (800Eh: 0000:43B1 takes any word
/// with bit 8000h) clicked: the original's frames, no area left.
#[test]
fn a_wilderness_flat_clicked_as_the_original() {
    transit("wildflat");
}

/// Crypt of Hearts, High Rock's staff dungeon, travelled to: the journey,
/// the dungeon's picture (292B:0362: its words, the button), the
/// dungeon's first level entered (its MIF and INF), its first frames.
#[test]
fn into_the_crypt_of_hearts_as_the_original() {
    transit("travelhearts");
}

/// Mines of Khuras, High Rock's map dungeon, travelled to as
/// `travelhearts`.
#[test]
fn into_the_mines_of_khuras_as_the_original() {
    transit("travelkhuras");
}
