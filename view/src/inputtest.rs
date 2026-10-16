//! The window's input, tested without a window: an `App` made as `main`
//! makes it (`make_app`), fed the inputs winit would give it (`App::input`)
//! and the time (`App::tick`), and checked against the same actions done
//! through the scripted path (`--picks`, `create::parse_script`, the
//! panels' functions) where there is one: the engines' memory equal.
//!
//! The game's files are found as the engine's tests find them: the
//! directory of ARENA_EXE, else ../../ESarena beside the crate; without
//! them the tests say so and pass.

use crate::*;
use std::path::Path;
use std::time::{Duration, Instant};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

fn game_dir() -> Option<PathBuf> {
    let dir = match std::env::var("ARENA_EXE") {
        Ok(exe) => PathBuf::from(exe).parent()?.to_path_buf(),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ESarena"),
    };
    if dir.join("GLOBAL.BSA").exists() {
        Some(dir)
    } else {
        eprintln!("skipping: no game files at {}", dir.display());
        None
    }
}

/// A start as main's: the area (a city spec, or a level), the hour, a
/// square and heading, the answers given first, the save directory.
struct Start<'a> {
    level: &'a str,
    hour: Option<u32>,
    at: Option<(u16, u16, u16)>,
    picks: &'a str,
    new_game: Option<&'a str>,
    save_dir: Option<PathBuf>,
    /// A saved game loaded at the start (`--load`): its folder and slot.
    load: Option<(PathBuf, u8)>,
}

impl Default for Start<'_> {
    fn default() -> Self {
        Start { level: "CITY:8:0", hour: Some(12), at: None, picks: "", new_game: None, save_dir: None, load: None }
    }
}

fn app(dir: &Path, s: &Start) -> App {
    let mut engine = world::load(dir, s.level);
    let mut run = match s.new_game {
        Some(spec) => {
            newgame::start(&mut engine, spec);
            arena::transit::Run::Start
        }
        None => areas::start(&mut engine, s.level.starts_with("CITY:")),
    };
    interface::start(&mut engine.m);
    if let Some((save, slot)) = &s.load {
        // As the window's --load: the title's Load.
        run = areas::load_game_title(&mut engine, dir, save, *slot).expect("the save loads");
    }
    if let Some(h) = s.hour {
        set_hour(&mut engine, h);
    }
    if s.load.is_none() {
        world::city_people(&mut engine);
    }
    if let Some((col, row, heading)) = s.at {
        use arena::movement::var::{HEADING, X, Y};
        engine.m.mem.w16(arena::mem::DS, X, col << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, Y, row << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, HEADING, heading);
        engine.world_loop_start();
    }
    make_app(dir.to_path_buf(), engine, run, None, dialog::parse_answers(s.picks), s.save_dir.clone(), 0, false)
}

/// The inputs and the time, as the window gives them: a 960 x 600 window,
/// three of its pixels a pixel of the 320 x 200 screen.
struct Driver {
    now: Instant,
}

impl Driver {
    fn new(app: &mut App) -> Driver {
        let now = Instant::now();
        app.input(In::Resized(960.0, 600.0));
        app.next = now;
        app.movie_at = now;
        Driver { now }
    }

    /// A game frame's time on (a second: the frame's due).
    fn frame(&mut self, app: &mut App) {
        self.now += Duration::from_secs(1);
        app.tick(self.now);
    }

    fn frames(&mut self, app: &mut App, n: usize) {
        for _ in 0..n {
            self.frame(app);
        }
    }

    /// `ms` of time on, no frame due beyond the one it brings.
    fn wait(&mut self, app: &mut App, ms: u64) {
        self.now += Duration::from_millis(ms);
        app.tick(self.now);
    }

    fn key(&mut self, app: &mut App, code: KeyCode, text: Option<&str>) {
        app.input(In::Key { code, down: true, text: text.map(str::to_string) });
        app.input(In::Key { code, down: false, text: None });
    }

    /// A click at (x, y) of the 320 x 200 screen.
    fn click(&mut self, app: &mut App, x: i32, y: i32, button: MouseButton) {
        app.input(In::Cursor(x as f64 * 3.0 + 1.0, y as f64 * 3.0 + 1.0));
        app.input(In::Button { button, down: true });
        app.input(In::Button { button, down: false });
    }
}

/// Where two engines' memories differ (the first 1 MiB, by 4 KiB
/// segment), for the messages.
fn differs(a: &Engine, b: &Engine) -> Vec<u16> {
    (0..0x10u16)
        .map(|i| i << 12)
        .filter(|&seg| a.m.mem.segment(seg) != b.m.mem.segment(seg))
        .collect()
}

fn same(a: &App, b: &App, what: &str) {
    let d = differs(&a.engine, &b.engine);
    if !d.is_empty() {
        let ds = arena::mem::DS as usize * 16;
        let bytes: Vec<String> = a
            .engine
            .m
            .mem
            .diff(&b.engine.m.mem, 0..0x10_0000)
            .iter()
            .take(24)
            .map(|&(i, v)| {
                let other = b.engine.m.mem.segment((i >> 4) as u16)[i & 15];
                if (ds..ds + 0x10000).contains(&i) { format!("DS:{:04X} {v:02x}/{other:02x}", i - ds) } else { format!("{i:05x} {v:02x}/{other:02x}") }
            })
            .collect();
        panic!("{what}: the engines differ: {}", bytes.join(", "));
    }
    assert_eq!(a.engine.m.ems.len(), b.engine.m.ems.len(), "{what}: EMS banks");
    for (i, (x, y)) in a.engine.m.ems.iter().zip(&b.engine.m.ems).enumerate() {
        assert!(x[..] == y[..], "{what}: EMS bank {i} differs");
    }
}

/// The key that types `c` on the game's first keyboard (A944, with
/// Shift A9C4): a letter, a digit, a space; and whether Shift.
fn key_for(c: u8) -> (KeyCode, bool) {
    use KeyCode as K;
    const LETTERS: [KeyCode; 26] = [
        K::KeyA, K::KeyB, K::KeyC, K::KeyD, K::KeyE, K::KeyF, K::KeyG, K::KeyH, K::KeyI, K::KeyJ, K::KeyK, K::KeyL, K::KeyM, K::KeyN, K::KeyO, K::KeyP, K::KeyQ,
        K::KeyR, K::KeyS, K::KeyT, K::KeyU, K::KeyV, K::KeyW, K::KeyX, K::KeyY, K::KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [K::Digit0, K::Digit1, K::Digit2, K::Digit3, K::Digit4, K::Digit5, K::Digit6, K::Digit7, K::Digit8, K::Digit9];
    match c {
        b'a'..=b'z' => (LETTERS[(c - b'a') as usize], false),
        b'A'..=b'Z' => (LETTERS[(c - b'A') as usize], true),
        b'0'..=b'9' => (DIGITS[(c - b'0') as usize], false),
        _ => (K::Space, false),
    }
}

/// `s` typed key by key into a typed field (Shift held for capitals).
fn type_text(d: &mut Driver, app: &mut App, s: &str) {
    for c in s.bytes() {
        let (k, shift) = key_for(c);
        if shift {
            app.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
        }
        d.key(app, k, Some(&(c as char).to_string()));
        if shift {
            app.input(In::Key { code: KeyCode::ShiftLeft, down: false, text: None });
        }
    }
}

/// The typed field asked and its text as it stands.
fn typed(app: &App) -> String {
    app.live.as_ref().map(|l| String::from_utf8_lossy(&l.text()).into_owned()).unwrap_or_default()
}

fn text(spans: &[Vec<hud::Span>]) -> String {
    spans.iter().map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

/// The menu the game waits on, if it does.
fn menu(app: &App) -> Option<&hud::Menu> {
    match app.pending.as_ref().map(|p| &p.need) {
        Some(dialog::Need::Menu(m)) => Some(m),
        _ => None,
    }
}

/// The engine's frame count (CCBC).
fn frame_no(app: &App) -> u16 {
    app.engine.m.mem.r16(arena::mem::DS, 0xCCBC)
}

/// The scripted twin run as far as `a`: while `a` waits on a question
/// (its engine back before the turn that asked), until `b` asks too;
/// else to `a`'s frame count.
fn as_far_as(b: &mut App, a: &App) {
    let mut d = Driver::new(b);
    for _ in 0..8 {
        let there = if a.pending.is_some() { b.pending.is_some() } else { frame_no(b) >= frame_no(a) };
        if there {
            break;
        }
        d.frame(b);
    }
}

/// Frames until the game asks (at most `n`).
fn until_asked(d: &mut Driver, app: &mut App, n: usize) -> bool {
    for _ in 0..n {
        if app.pending.is_some() {
            return true;
        }
        d.frame(app);
    }
    app.pending.is_some()
}

/// The townsperson two squares ahead in the Imperial City at noon
/// (`--at 59 8 0`), clicked in the view's centre with the mouse free: the
/// talk's menu asked.
fn talk_start(dir: &Path, picks: &str) -> (App, Driver) {
    let mut a = app(dir, &Start { at: Some((59, 8, 0)), picks, ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.click(&mut a, 164, 74, MouseButton::Left);
    assert!(a.click && a.click_point == Some((164, 74)), "the click in the view is kept for the next frame");
    d.frame(&mut a);
    (a, d)
}

#[test]
fn talk_menu_by_arrows_and_enter_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let (mut a, mut d) = talk_start(&dir, "");
    assert!(until_asked(&mut d, &mut a, 3), "clicking the townsperson asks");
    let m = menu(&a).expect("a menu");
    assert!(text(&m.title).contains("ASK ABOUT"), "the talk's menu: {:?}", text(&m.title));
    // Down to "Where is...", up again, Enter: the greeting (entry 0).
    d.key(&mut a, KeyCode::ArrowDown, None);
    assert_eq!(a.selected, 1);
    d.key(&mut a, KeyCode::ArrowUp, None);
    d.key(&mut a, KeyCode::Enter, None);
    assert!(!a.boxes.is_empty(), "the townsperson answers in a box");
    let (mut b, _) = talk_start(&dir, "0");
    as_far_as(&mut b, &a);
    same(&a, &b, "the greeting answered by Enter and by --picks 0");
    assert_eq!(a.boxes.iter().map(|x| text(x)).collect::<Vec<_>>(), b.boxes.iter().map(|x| text(x)).collect::<Vec<_>>());
}

#[test]
fn talk_menu_by_letter_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let (mut a, mut d) = talk_start(&dir, "");
    assert!(until_asked(&mut d, &mut a, 3));
    let m = menu(&a).expect("a menu").clone();
    // Entry 1's key ("Where is..."), typed.
    let letter = (m.keys[1] as char).to_ascii_lowercase().to_string();
    d.key(&mut a, KeyCode::KeyW, Some(&letter));
    let (mut b, _) = talk_start(&dir, "1");
    as_far_as(&mut b, &a);
    same(&a, &b, "entry 1 by its letter and by --picks 1");
    assert_eq!(menu(&a).map(|m| text(&m.title)), menu(&b).map(|m| text(&m.title)));
}

#[test]
fn where_is_shows_its_list_and_tells_the_way_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let (mut a, mut d) = talk_start(&dir, "");
    assert!(until_asked(&mut d, &mut a, 3));
    let m = menu(&a).expect("a menu").clone();
    let letter = (m.keys[1] as char).to_ascii_lowercase().to_string();
    d.key(&mut a, KeyCode::KeyW, Some(&letter));
    // "Where is": 1194:5734 with AX 9 (POPUP11.IMG, the record at 5105).
    let l = menu(&a).expect("the places' list").clone();
    assert!(l.list && l.kind == 9, "a list of kind 9: {} {}", l.list, l.kind);
    assert!(!l.entries.is_empty());
    d.key(&mut a, KeyCode::Enter, None);
    let (mut b, _) = talk_start(&dir, "1,0");
    as_far_as(&mut b, &a);
    same(&a, &b, "a place picked by Enter and by --picks 1,0");
}

#[test]
fn esc_leaves_the_talk_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let (mut a, mut d) = talk_start(&dir, "");
    assert!(until_asked(&mut d, &mut a, 3));
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.pending.is_none(), "Esc leaves the menu");
    let (mut b, _) = talk_start(&dir, "-1");
    as_far_as(&mut b, &a);
    assert!(b.pending.is_none());
    same(&a, &b, "Esc and --picks -1");
    // The game goes on afterwards.
    d.frames(&mut a, 3);
    assert!(a.pending.is_none() && a.boxes.is_empty());
}

#[test]
fn a_click_on_a_menu_entry_picks_it() {
    let Some(dir) = game_dir() else { return };
    let (mut a, mut d) = talk_start(&dir, "");
    assert!(until_asked(&mut d, &mut a, 3));
    // Entry 0 clicked where the HUD draws it.
    let hud = a.hud();
    let m = hud.menu.as_ref().expect("the menu shown");
    let (x, y, lh, w) = hud::menu_layout(&a.font, m);
    let _ = (lh, w);
    let (mut px, mut py) = (x, y);
    let mut found = None;
    'search: for yy in (y - 40).max(0)..(y + 80).min(200) {
        for xx in (x - 10).max(0)..(x + 60).min(320) {
            if hud::menu_pick(&a.font, &hud, xx, yy) == Some(0) {
                found = Some((xx, yy));
                break 'search;
            }
        }
    }
    if let Some((fx, fy)) = found {
        px = fx;
        py = fy;
    }
    assert!(found.is_some(), "entry 0 can be clicked somewhere near ({x}, {y})");
    d.click(&mut a, px, py, MouseButton::Left);
    let (mut b, _) = talk_start(&dir, "0");
    as_far_as(&mut b, &a);
    same(&a, &b, "entry 0 clicked and --picks 0");
}

/// A turn running a shop of building kind `kind`, as `--do shop:KIND`
/// does, from the window's own turn (`App::turn`).
fn shop_turn(app: &mut App, kind: u8) {
    let before = copy(&app.engine);
    let input = TurnIn {
        intent: Default::default(),
        clicked: false,
        camp: false,
        shop: Some(kind),
        key: false,
        tick: 0,
        right: false,
        drag: (0, 0),
        aspect: 1.6,
        note: None,
    };
    app.turn(before, input, Vec::new(), true);
}

#[test]
fn a_store_purchase_answered_by_keys_as_by_script() {
    let Some(dir) = game_dir() else { return };
    // The store's menu: Buy (0); the weapons list: its first line; the
    // haggle: Accept (0). The keys give each in turn; the script all.
    let gold = |app: &mut App| app.engine.m.mem.w32(arena::mem::DS, 0x05C2, 5000);
    let mut a = app(&dir, &Start::default());
    gold(&mut a);
    let mut d = Driver::new(&mut a);
    shop_turn(&mut a, 6);
    let mut asked = Vec::new();
    for _ in 0..12 {
        if !a.boxes.is_empty() {
            d.key(&mut a, KeyCode::Enter, None);
            continue;
        }
        let Some(m) = menu(&a) else { break };
        asked.push(text(&m.title));
        d.key(&mut a, KeyCode::Enter, None);
    }
    assert!(asked.len() >= 2, "the store asked: {asked:?}");
    let picks = vec!["0"; asked.len()].join(",");
    let mut b = app(&dir, &Start { picks: &picks, ..Default::default() });
    gold(&mut b);
    Driver::new(&mut b);
    shop_turn(&mut b, 6);
    while !b.boxes.is_empty() {
        b.boxes.pop_front();
    }
    same(&a, &b, &format!("the store answered by Enter and by --picks {picks} ({asked:?})"));
    let after = |app: &App| app.engine.m.mem.r32(arena::mem::DS, 0x05C2);
    eprintln!("store: asked {asked:?}, gold 5000 -> {}", after(&a));
}

#[test]
fn camping_with_typed_hours_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let start = |picks| Start { level: "START.MIF", hour: None, new_game: Some("0:3:0:2:Talin"), picks, ..Default::default() };
    let mut a = app(&dir, &start(""));
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.key(&mut a, KeyCode::KeyR, None);
    assert!(until_asked(&mut d, &mut a, 3), "R asks how to rest");
    let m = menu(&a).expect("the camp's menu").clone();
    eprintln!("camp menu: {:?}", text(&m.title));
    // "For a while" (entry 0), then the hours typed.
    d.key(&mut a, KeyCode::Enter, None);
    match a.pending.as_ref().map(|p| &p.need) {
        // The prompt's trailing spaces kept: the field's room in its box.
        Some(dialog::Need::Text(dialog::Ask { prompt: dialog::Prompt::Boxed(prompt), field, .. })) => {
            assert!(prompt.starts_with("How many hours") && prompt.ends_with("  "), "prompt: {prompt:?}");
            // 1194:46BF: the field at FCh, 44h, 3 characters.
            assert_eq!((field.x, field.y, field.max), (0xFC, 0x44, 3));
        }
        other => panic!("the hours asked for, not {:?}", other.map(|_| "something else")),
    }
    d.key(&mut a, KeyCode::Digit2, Some("2"));
    assert_eq!(typed(&a), "2");
    d.key(&mut a, KeyCode::Backspace, None);
    d.key(&mut a, KeyCode::Digit2, Some("2"));
    d.key(&mut a, KeyCode::Enter, None);
    let mut b = app(&dir, &start("0,t2"));
    let mut db = Driver::new(&mut b);
    db.frame(&mut b);
    db.key(&mut b, KeyCode::KeyR, None);
    db.frame(&mut b);
    same(&a, &b, "camping for 2 hours by keys and by --picks 0,t2");
    // The rest runs on in the frames that follow, alike.
    d.frames(&mut a, 30);
    db.frames(&mut b, 30);
    same(&a, &b, "the rest's frames");
}

#[test]
fn f5_saves_and_f9_loads_back() {
    let Some(dir) = game_dir() else { return };
    let saves = Scratch::new("f5");
    let mut a = app(&dir, &Start { save_dir: Some(saves.to_path_buf()), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frames(&mut a, 2);
    let at = |app: &App| {
        let v = app.engine.view();
        (v.x, v.y, v.heading)
    };
    d.key(&mut a, KeyCode::F5, None);
    let saved = at(&a);
    assert!(a.boxes.iter().any(|b| !text(b).is_empty()), "a message after saving");
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // Walk away, then load.
    a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
    d.frames(&mut a, 20);
    a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
    assert_ne!(at(&a), saved, "the walk moved the player");
    d.key(&mut a, KeyCode::F9, None);
    assert_eq!(at(&a), saved, "F9 brings the player back where F5 saved");
}

#[test]
fn the_drop_button_asks_and_refuses_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start::default());
    panels::give(&mut a.engine.m, 6);
    let mut d = Driver::new(&mut a);
    let asking = |app: &App| match &app.panel {
        Some(panels::Panel::Inventory(inv)) => inv.asking.as_ref().map(|(t, _)| String::from_utf8_lossy(t).into_owned()),
        _ => None,
    };
    let items = |app: &App| panels::item_lines(&app.engine.m).len();
    d.key(&mut a, KeyCode::KeyI, None);
    // I checks where the player stands as the portrait does (1194:5F13):
    // the start's square has room, but would lose what is dropped (12D0
    // set); the plain path is taken from a square that keeps it.
    assert_eq!(a.engine.m.mem.r16(arena::mem::DS, 0x05E0) & 0x20, 0, "room here");
    assert_eq!(a.engine.m.mem.r8(arena::mem::DS, 0x12D0), 1, "the start's square loses it");
    a.engine.m.mem.w8(arena::mem::DS, 0x12D0, 0);
    // The page opens on its first line (1194:1726); with nothing selected
    // (as after a drop, 1924): "Drop what?".
    assert_eq!(a.engine.m.mem.r8(arena::mem::DS, 0x1296), 0, "the first line selected");
    a.engine.m.mem.w8(arena::mem::DS, 0x1296, 0xFF);
    d.key(&mut a, KeyCode::KeyD, None);
    assert!(a.boxes.iter().any(|b| text(b).contains("Drop what?")), "nothing selected");
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // The first item selected: D asks by its name; N keeps it.
    panels::select_line(&mut a.engine.m, 0);
    let n = items(&a);
    d.key(&mut a, KeyCode::KeyD, None);
    let q = asking(&a).expect("D asks");
    assert!(q.starts_with("Do you wish to drop"), "{q:?}");
    d.key(&mut a, KeyCode::KeyN, None);
    assert!(asking(&a).is_none() && items(&a) == n, "no keeps it");
    // Y drops it.
    d.key(&mut a, KeyCode::KeyD, None);
    d.key(&mut a, KeyCode::KeyY, None);
    assert_eq!(items(&a), n - 1, "yes drops it");
    // Where it would be lost (12D0), the question says so.
    panels::select_line(&mut a.engine.m, 0);
    a.engine.m.mem.w8(arena::mem::DS, 0x12D0, 1);
    d.key(&mut a, KeyCode::KeyD, None);
    assert!(asking(&a).expect("asks").starts_with("This item will be lost here."), "lost");
    d.key(&mut a, KeyCode::Escape, None);
    assert_eq!(items(&a), n - 1, "Esc answers no");
    // No room (5E0 bit 20h): refused.
    let f = a.engine.m.mem.r16(arena::mem::DS, 0x05E0) | 0x20;
    a.engine.m.mem.w16(arena::mem::DS, 0x05E0, f);
    d.key(&mut a, KeyCode::KeyD, None);
    assert!(a.boxes.iter().any(|b| text(b).contains("There is no room to drop this here.")), "no room");
    assert_eq!(items(&a), n - 1);
}

#[test]
fn inventory_by_i_and_a_click_equips_as_panels_do() {
    let Some(dir) = game_dir() else { return };
    let setup = |app: &mut App| panels::give(&mut app.engine.m, 6);
    let mut a = app(&dir, &Start::default());
    setup(&mut a);
    let mut d = Driver::new(&mut a);
    d.key(&mut a, KeyCode::KeyI, None);
    assert!(matches!(a.panel, Some(panels::Panel::Inventory(_))), "I opens the inventory");
    // The first line, as the screen lays it out (`panels::line_at`).
    let (x, y) = (0..200)
        .flat_map(|y| (0..320).map(move |x| (x, y)))
        .find(|&(x, y)| panels::line_at(x, y) == Some(0))
        .expect("line 0 somewhere");
    d.click(&mut a, x, y, MouseButton::Left);
    let mut b = app(&dir, &Start::default());
    setup(&mut b);
    Driver::new(&mut b);
    // I checks the player's place first (1194:5F13), as the portrait does.
    b.engine.m.drop_place();
    b.panel = Some(panels::open_inventory(&mut b.engine.m));
    let mut said = panels::Said::default();
    panels::equip_line(&mut b.engine.m, 0, &mut said);
    same(&a, &b, "line 0 clicked and panels::equip_line(0)");
    assert_eq!(a.boxes.iter().map(|x| text(x)).collect::<Vec<_>>(), said.boxes.iter().map(|x| text(x)).collect::<Vec<_>>(), "the same boxes said");
    // A box up takes the next key (it is read); then I closes the panel.
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    d.key(&mut a, KeyCode::KeyI, None);
    assert!(a.panel.is_none(), "I closes it");
}

#[test]
fn spellbook_by_c_and_a_targeted_cast_by_clicks() {
    let Some(dir) = game_dir() else { return };
    let setup = |app: &mut App| {
        // A caster (class bit 20h): others have the spellbook barred
        // (5CA bit 8, 292B:032A), C then opening nothing.
        let c = app.engine.m.mem.r8(arena::mem::DS, 0x01A9) | 0x20;
        app.engine.m.mem.w8(arena::mem::DS, 0x01A9, c);
        panels::learn(&mut app.engine.m, 0);
        app.engine.m.mem.w16(arena::mem::DS, 0x020A, 500);
        app.engine.m.mem.w16(arena::mem::DS, 0x020C, 500);
    };
    let mut a = app(&dir, &Start { at: Some((59, 8, 0)), ..Default::default() });
    setup(&mut a);
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.key(&mut a, KeyCode::KeyC, None);
    assert!(matches!(a.panel, Some(panels::Panel::Spellbook(_))), "C opens the spellbook");
    let spells = panels::spells(&a.engine.m);
    assert!(!spells.is_empty(), "a spell in the book");
    // Its first line, in the bar's corner (the record at 5AF4's text
    // area from DEh, 99h).
    d.click(&mut a, 0xDE + 4, 0x99 + 3, MouseButton::Left);
    assert!(a.panel.is_none(), "a click on the entry casts and closes the book");
    let mut b = app(&dir, &Start { at: Some((59, 8, 0)), ..Default::default() });
    setup(&mut b);
    let mut db = Driver::new(&mut b);
    db.frame(&mut b);
    let mut said = panels::Said::default();
    panels::cast(&mut b.engine, 0, &mut said);
    same(&a, &b, "spell 0 clicked and panels::cast(0)");
    // Its target clicked in the view: the same click as the script's.
    d.click(&mut a, 164, 74, MouseButton::Left);
    d.frames(&mut a, 2);
    db.click(&mut b, 164, 74, MouseButton::Left);
    db.frames(&mut b, 2);
    same(&a, &b, "the target clicked");
}

#[test]
fn a_captured_click_and_swing_and_look() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    // A click on the bar (below the view) off its buttons captures the
    // mouse.
    d.click(&mut a, 240, 170, MouseButton::Left);
    assert!(a.captured, "the mouse captured");
    // Looking: about a quarter turn for 400 mickeys.
    let h0 = a.engine.view().heading;
    a.input(In::Motion(400.0, 0.0));
    d.frame(&mut a);
    let turned = a.engine.view().heading.wrapping_sub(h0) & 0x1FF;
    assert!((100..=156).contains(&turned), "a quarter turn: {turned:#x}");
    // Q readies the weapon; the right button held and dragged swings.
    d.key(&mut a, KeyCode::KeyQ, None);
    d.frames(&mut a, 3);
    a.input(In::Button { button: MouseButton::Right, down: true });
    a.input(In::Motion(0.0, 100.0));
    d.frame(&mut a);
    a.input(In::Button { button: MouseButton::Right, down: false });
    let mut frames = Vec::new();
    for _ in 0..4 {
        frames.push(a.engine.view().hand_frame);
        d.frame(&mut a);
    }
    assert!(frames.windows(2).any(|w| w[0] != w[1]), "the swing animates: {frames:?}");
    // Esc lets the mouse go.
    d.key(&mut a, KeyCode::Escape, None);
    assert!(!a.captured);
}

#[test]
fn creation_by_keys_and_clicks_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let make = || {
        let mut engine = world::load(&dir, "START.MIF");
        let archive = Rc::new(archive::Archive::open(&dir));
        let c = create::Creation::new(&mut engine, archive, &dir, world::palette(&dir));
        (engine, c)
    };
    let made = |(engine, c): (Engine, create::Creation)| make_app(dir.clone(), engine, arena::transit::Run::Start, Some(c), Default::default(), None, 0, false);
    let mut a = made(make());
    let mut d = Driver::new(&mut a);
    // The scripted twin: the same App, given the script's inputs as
    // `--create` gives them (`Creation::input`).
    let mut b = made(make());
    Driver::new(&mut b);
    // The intro skipped, the title, Start; the opening story skipped;
    // "Select" the class; the list's first class; the name; the gender.
    let script = "enter,enter,S,esc,S,enter,type:Ria,enter,F";
    for (i, inp) in create::parse_script(script).into_iter().enumerate() {
        match inp {
            create::Input::Enter => d.key(&mut a, KeyCode::Enter, None),
            create::Input::Esc => d.key(&mut a, KeyCode::Escape, None),
            create::Input::Back => d.key(&mut a, KeyCode::Backspace, None),
            create::Input::Up => d.key(&mut a, KeyCode::ArrowUp, None),
            create::Input::Down => d.key(&mut a, KeyCode::ArrowDown, None),
            // The name's characters by their keys (the game's editor
            // reads keys); a menu's by the text its key gives.
            create::Input::Char(c) if a.creation.as_ref().is_some_and(|c| c.typing()) => type_text(&mut d, &mut a, &(c as char).to_string()),
            create::Input::Char(c) => {
                let t = (c as char).to_string();
                d.key(&mut a, KeyCode::KeyA, Some(&t));
            }
            create::Input::Click(x, y) => d.click(&mut a, x, y, MouseButton::Left),
            create::Input::Wait(ms) => d.wait(&mut a, ms as u64),
            create::Input::Key(..) => {}
        }
        b.creation_input(inp);
        let (Some(ca), Some(cb)) = (a.creation.as_ref(), b.creation.as_ref()) else { panic!("creation ended early at step {i}") };
        let mut sa = hud::Screen::new(a.assets.palette);
        let mut sb = hud::Screen::new(a.assets.palette);
        // As they settle (one has had time pass in a dissolve).
        ca.draw_page(&mut sa, &a.engine.m);
        cb.draw_page(&mut sb, &b.engine.m);
        assert!(sa.px == sb.px, "step {i} ({inp:?}): the screens differ");
        same(&a, &b, &format!("step {i} ({inp:?})"));
    }
}

#[test]
fn a_level_gained_spent_on_the_attribute_screen() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start { level: "START.MIF", hour: None, new_game: Some("0:3:0:2:Talin"), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let level = |app: &App| app.engine.m.mem.r8(arena::mem::DS, arena::character::PLAYER + arena::character::field::LEVEL);
    let before = level(&a);
    // Experience gained (as `--do xp:N`): the next minute checks the level.
    let at = arena::character::PLAYER + 0x409;
    let xp = a.engine.m.mem.r32(arena::mem::DS, at).wrapping_add(100_000);
    a.engine.m.mem.w32(arena::mem::DS, at, xp);
    let f = a.engine.m.mem.r16(arena::mem::DS, 0x05CA) | 0x100;
    a.engine.m.mem.w16(arena::mem::DS, 0x05CA, f);
    for _ in 0..600 {
        if a.creation.is_some() {
            break;
        }
        // The level's notice read as it comes.
        while !a.boxes.is_empty() {
            d.key(&mut a, KeyCode::Enter, None);
        }
        d.frame(&mut a);
    }
    assert!(a.creation.is_some(), "the attribute screen up for the level's points");
    assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spend(_))));
    let points = |app: &App| app.creation_engine.as_ref().expect("the screen's engine").m.mem.r8(arena::mem::DS, arena::newgame::var::POINTS);
    let start = points(&a);
    assert!(start > 0, "points to spend");
    // Done refused while points are left: a box.
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.creation.is_some() && a.pending.is_some(), "not left with points to spend");
    d.key(&mut a, KeyCode::Enter, None);
    // Each line picked by its letter (the regions at DATA:5312, as
    // create.rs reads them), its points raised with Up until it stops (an
    // attribute's cap) or none are left.
    for line in 1..=8u16 {
        let letter = a.creation_engine.as_ref().unwrap().m.mem.r8(arena::mem::DATA, 0x5312 + line * 0x62 + 0x59);
        d.key(&mut a, KeyCode::KeyA, Some(&(letter as char).to_ascii_lowercase().to_string()));
        for _ in 0..200 {
            let left = points(&a);
            if left == 0 {
                break;
            }
            d.key(&mut a, KeyCode::ArrowUp, None);
            if points(&a) == left {
                break;
            }
        }
    }
    assert_eq!(points(&a), 0, "all {start} points spent");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.creation.is_none(), "the screen left once the points are spent");
    assert!(!matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spend(_))), "the turn replayed with the clicks");
    assert!(level(&a) > before, "the level gained: {before} -> {}", level(&a));
}

#[test]
fn any_key_ends_a_rest() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start { level: "START.MIF", hour: None, new_game: Some("0:3:0:2:Talin"), picks: "0,t8", ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.key(&mut a, KeyCode::KeyR, None);
    d.frames(&mut a, 2);
    let resting = |app: &App| app.engine.m.mem.r8(arena::mem::DS, arena::rest::var::RESTING) != 0;
    assert!(resting(&a), "resting for 8 hours");
    d.key(&mut a, KeyCode::Space, Some(" "));
    d.frames(&mut a, 3);
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    assert!(!resting(&a), "a key ended the rest");
}

#[test]
fn a_movie_takes_keys_and_holds_the_game() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let line = |t: &str| vec![hud::Span { colour: 0x0F, text: t.to_string(), font: Default::default(), code: None }];
    let lines = (0..7).map(|i| line(&format!("line {i}"))).collect();
    let mv = flc::Movie::new(&a.assets.archive, "VISION.FLC", lines).expect("VISION.FLC");
    a.movies.push_back(mv);
    let frame0 = frame_no(&a);
    d.frames(&mut a, 3);
    assert_eq!(frame_no(&a), frame0, "no game frame while a movie is up");
    for _ in 0..10 {
        if a.movies.is_empty() {
            break;
        }
        d.key(&mut a, KeyCode::Space, Some(" "));
    }
    assert!(a.movies.is_empty(), "keys page through the movie's lines to its end");
    d.frames(&mut a, 2);
    assert!(frame_no(&a) > frame0, "the game goes on after it");
}

#[test]
fn the_travel_map_by_shift_m_and_clicks_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let start = Start { level: "CITY:0:0", ..Default::default() };
    let mut a = app(&dir, &start);
    let mut d = Driver::new(&mut a);
    // Shift+M opens it on the world map (2542:03BF, 26F7:0000); the
    // current province's region opens its map (26F7:0176).
    a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
    d.key(&mut a, KeyCode::KeyM, Some("M"));
    a.input(In::Key { code: KeyCode::ShiftLeft, down: false, text: None });
    assert!(a.travel.is_some(), "Shift+M opens the travel map");
    let (px, py) = travel::province_point(a.engine.m.mem.r8(arena::mem::DS, 0x0A46));
    d.click(&mut a, px, py, MouseButton::Left);
    // A click on the map picks the nearest place: the journey's box.
    d.click(&mut a, 135, 58, MouseButton::Left);
    let journey = a.boxes.iter().map(|x| text(x)).collect::<Vec<_>>().join("\n");
    assert!(journey.contains("Shornhelm") && journey.contains("days"), "the journey shown: {journey}");
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // The same through the scripted path's calls (`--do travel,tclick`).
    let mut b = app(&dir, &start);
    Driver::new(&mut b);
    let archive = b.assets.archive.clone();
    let mut t = Some(travel::Travel::open(&mut b.engine, &archive).unwrap_or_else(|_| panic!("it opens")));
    t.as_mut().unwrap().click(&mut b.engine, &archive, px, py, false);
    t.as_mut().unwrap().click(&mut b.engine, &archive, 135, 58, false);
    same(&a, &b, "the place clicked, and Travel::click");
    // Travel: the journey made, its horse running a frame a day
    // (1194:30F1), then the screen ended, the area left.
    d.click(&mut a, 62, 180, MouseButton::Left);
    assert!(a.travel.as_ref().is_some_and(|t| t.journeying()), "the journey's frames play");
    for _ in 0..10 {
        if a.travel.is_none() {
            break;
        }
        d.wait(&mut a, 100);
    }
    assert!(a.travel.is_none(), "the screen ends after the journey");
    t.as_mut().unwrap().click(&mut b.engine, &archive, 62, 180, false);
    t.as_mut().unwrap().skip_journey();
    travel::after(&mut t, &mut b.engine, &mut Vec::new(), true);
    assert!(t.is_none());
    same(&a, &b, "Travel clicked, and Travel::click");
    // The next turns arrive: the town's welcome reads the arrival.
    let mut welcome = String::new();
    for _ in 0..20 {
        d.frame(&mut a);
        if let Some(b) = a.boxes.front() {
            welcome = text(b);
            break;
        }
    }
    assert!(welcome.contains("You have arrived in The City-State of Shornhelm"), "the welcome: {welcome:?}");
    let here = String::from_utf8_lossy(a.engine.m.mem.cstr(arena::mem::DS, 0x0A22)).into_owned();
    assert_eq!(here, "Shornhelm", "the location record");
}

/// The user's save (ARENA_SAVE), copied to a scratch folder so nothing
/// can write to it.
fn save_copy() -> Option<Scratch> {
    let save = PathBuf::from(std::env::var("ARENA_SAVE").ok()?);
    let to = Scratch::new("save");
    for e in std::fs::read_dir(&save).ok()? {
        let e = e.ok()?;
        if e.file_type().ok()?.is_file() {
            std::fs::copy(e.path(), to.join(e.file_name())).ok()?;
        }
    }
    Some(to)
}

/// A scratch folder of the test's own (tests run in parallel in one
/// process, so the process id alone is shared), removed when the test
/// ends, a failed one included.
struct Scratch(PathBuf);

impl Scratch {
    fn new(what: &str) -> Scratch {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let to = std::env::temp_dir().join(format!("arena-inputtest-{what}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&to);
        std::fs::create_dir_all(&to).expect("a scratch folder");
        Scratch(to)
    }
}

impl std::ops::Deref for Scratch {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_spellmaker_clicked_live_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    // Drakain before Daggerfall's guild master: the menu's Spellmaker (2),
    // then the screen's click at (35, 75) (an effect's list), entries 0.
    let start = |picks| Start { hour: None, picks, load: Some((save.to_path_buf(), 1)), ..Default::default() };
    let mut a = app(&dir, &start(""));
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.click(&mut a, 164, 74, MouseButton::Left);
    d.frame(&mut a);
    assert!(until_asked(&mut d, &mut a, 3), "the guild master asks");
    d.key(&mut a, KeyCode::Digit3, Some("3"));
    assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spellmaker)), "the spellmaker waits for a click");
    d.click(&mut a, 35, 75, MouseButton::Left);
    d.key(&mut a, KeyCode::Enter, None);
    d.key(&mut a, KeyCode::Enter, None);
    let mut b = app(&dir, &start("2,c35:75,0,0"));
    let mut db = Driver::new(&mut b);
    db.frame(&mut b);
    db.click(&mut b, 164, 74, MouseButton::Left);
    db.frame(&mut b);
    as_far_as(&mut b, &a);
    same(&a, &b, "the spellmaker by key, click and Enter, and by --picks 2,c35:75,0,0");
    let maker = |app: &App| app.pending.as_ref().and_then(|p| p.maker.as_ref()).map(|(_, form)| *form);
    assert_eq!(maker(&a), maker(&b), "the same form open");
    assert!(maker(&a).flatten().is_some(), "a form open after the clicks");
}

/// Where entry `i` of the question shown can be clicked (`hud::menu_pick`
/// over the screen), if anywhere.
fn entry_at(app: &App, i: usize) -> Option<(i32, i32)> {
    let hud = app.hud();
    (0..200).flat_map(|y| (0..320).map(move |x| (x, y))).find(|&(x, y)| hud::menu_pick(&app.font, &hud, x, y) == Some(i))
}

#[test]
fn a_store_list_line_clicked_as_by_script() {
    let Some(dir) = game_dir() else { return };
    let gold = |app: &mut App| app.engine.m.mem.w32(arena::mem::DS, 0x05C2, 5000);
    let mut a = app(&dir, &Start::default());
    gold(&mut a);
    let mut d = Driver::new(&mut a);
    shop_turn(&mut a, 6);
    // The store's menu (Buy) and its list (weapons) by clicks; the weapons
    // list's second line clicked.
    let mut picks = Vec::new();
    for (step, want) in [0usize, 0, 1].into_iter().enumerate() {
        let m = menu(&a).unwrap_or_else(|| panic!("asked at step {step}"));
        let list = m.list;
        let (x, y) = entry_at(&a, want).unwrap_or_else(|| panic!("entry {want} clickable at step {step} (list {list})"));
        d.click(&mut a, x, y, MouseButton::Left);
        picks.push(want.to_string());
        while !a.boxes.is_empty() {
            d.key(&mut a, KeyCode::Enter, None);
        }
    }
    let picks = picks.join(",");
    let mut b = app(&dir, &Start { picks: &picks, ..Default::default() });
    gold(&mut b);
    Driver::new(&mut b);
    shop_turn(&mut b, 6);
    while !b.boxes.is_empty() {
        b.boxes.pop_front();
    }
    same(&a, &b, &format!("the store's entries clicked and --picks {picks}"));
    assert_eq!(menu(&a).map(|m| text(&m.title)), menu(&b).map(|m| text(&m.title)));
}

#[test]
fn the_wheel_moves_a_menus_entry() {
    let Some(dir) = game_dir() else { return };
    let (mut a, mut d) = talk_start(&dir, "");
    assert!(until_asked(&mut d, &mut a, 3));
    a.input(In::Wheel(-1.0));
    assert_eq!(a.selected, 1, "down a line");
    a.input(In::Wheel(1.0));
    a.input(In::Wheel(1.0));
    let n = menu(&a).unwrap().entries.len();
    assert_eq!(a.selected, n - 1, "up past the first wraps to the last");
}

/// The character sheet by K and by the portrait; its page turned to the
/// equipment by N and by its button, whose exit comes back to it; Esc and
/// its Done button leave.
#[test]
fn the_character_sheet_by_k_and_the_portrait_and_its_pages() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start::default());
    named(&mut a);
    let mut d = Driver::new(&mut a);
    d.key(&mut a, KeyCode::KeyK, None);
    assert!(matches!(a.panel, Some(panels::Panel::Sheet)), "K opens the sheet");
    d.key(&mut a, KeyCode::KeyN, None);
    assert!(matches!(a.panel, Some(panels::Panel::Inventory(panels::Inventory { from_sheet: true, .. }))), "N turns to the equipment page");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(matches!(a.panel, Some(panels::Panel::Sheet)), "its exit comes back to the sheet");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.panel.is_none(), "Esc leaves the sheet");
    // By the mouse: the portrait, the next page's button, the equipment's
    // Exit, the sheet's Done.
    let (x, y) = sheets::bar_centre(8);
    d.click(&mut a, x, y, MouseButton::Left);
    assert!(matches!(a.panel, Some(panels::Panel::Sheet)), "the portrait opens the sheet");
    d.click(&mut a, 0x6A + 0x1A, 0xB2 + 7, MouseButton::Left);
    assert!(matches!(a.panel, Some(panels::Panel::Inventory(_))), "the button turns the page");
    let (ex, ey) = (0..200)
        .flat_map(|y| (0..320).map(move |x| (x, y)))
        .find(|&(x, y)| panels::button_at(x, y) == Some(panels::Button::Exit))
        .expect("the equipment's exit somewhere");
    d.click(&mut a, ex, ey, MouseButton::Left);
    assert!(matches!(a.panel, Some(panels::Panel::Sheet)), "the equipment's exit comes back");
    d.click(&mut a, 10, 0xAA + 8, MouseButton::Left);
    assert!(a.panel.is_none(), "Done leaves");
    assert!(!a.captured, "no click on the sheet captured the mouse");
}

/// The logbook by L and by the scroll; a click turns its page, Exit leaves.
#[test]
fn the_logbook_by_l_and_the_scroll() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    d.key(&mut a, KeyCode::KeyL, None);
    let Some(panels::Panel::Log(log)) = &a.panel else { panic!("L opens the logbook") };
    let want = sheets::open_log(&a.engine.m, &a.engine.files);
    assert_eq!((log.missing, log.pages.len()), (want.missing, want.pages.len()), "the slot's log, as sheets::open_log reads it");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.panel.is_none(), "Esc leaves");
    let (x, y) = sheets::bar_centre(5);
    d.click(&mut a, x, y, MouseButton::Left);
    assert!(matches!(a.panel, Some(panels::Panel::Log(_))), "the scroll opens the logbook");
    // A page of the logbook turned by a click (when there is one more).
    if let Some(panels::Panel::Log(log)) = a.panel.as_mut() {
        log.pages = vec![vec![(false, "a".into())]; 2];
        log.missing = false;
    }
    d.click(&mut a, 160, 100, MouseButton::Left);
    assert!(matches!(&a.panel, Some(panels::Panel::Log(l)) if l.page == 1), "a click turns the page");
    d.click(&mut a, 0x110, 0xB8, MouseButton::Left);
    assert!(a.panel.is_none(), "Exit leaves");
}

/// The automap by M and by the map's button, centred on the player as
/// `automap_view_open`; its arrows and buttons move it as the game's
/// handlers; Exit and Esc leave; the game's keys don't move the player.
#[test]
fn the_automap_by_m_and_the_map_button() {
    let Some(dir) = game_dir() else { return };
    use arena::automap::var::{MAP_X, MAP_Y};
    use arena::mem::DS;
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.key(&mut a, KeyCode::KeyM, None);
    assert!(a.map, "M opens the automap");
    let mut b = a.engine.m.clone();
    b.automap_view_open();
    let at = |m: &arena::machine::Machine| (m.mem.r16(DS, MAP_X), m.mem.r16(DS, MAP_Y));
    assert_eq!(at(&a.engine.m), at(&b), "centred as automap_view_open");
    let place = a.engine.view();
    // Up, the game's forward key: the screen's up button instead.
    d.key(&mut a, KeyCode::ArrowUp, None);
    d.frame(&mut a);
    b.automap_view_button(0);
    b.automap_origin();
    assert_eq!(at(&a.engine.m), at(&b), "Up as the up button");
    assert_eq!((a.engine.view().x, a.engine.view().y), (place.x, place.y), "the player stays");
    // The left button clicked.
    d.click(&mut a, 250, 48, MouseButton::Left);
    b.automap_view_button(3);
    b.automap_origin();
    assert_eq!(at(&a.engine.m), at(&b), "the left button");
    d.click(&mut a, 260, 175, MouseButton::Left);
    assert!(!a.map, "Exit leaves");
    let (x, y) = sheets::bar_centre(1);
    d.click(&mut a, x, y, MouseButton::Left);
    assert!(a.map, "the map's button opens it");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(!a.map, "Esc leaves");
    // The right button is the world map's (not this screen's).
    d.click(&mut a, x, y, MouseButton::Right);
    assert!(!a.map && !a.captured, "the right button opens no automap");
}

/// The automap's font block (FONT_S.DAT, 0000:7508) is the screen's
/// own: leaving frees it and puts the game's font back (2EAA..2EC5), and
/// only a block the fonts' table (91AC) holds is freed (930E). A stale
/// segment at A86C (the INF loader counts in its low byte; another
/// engine's block) frees nothing: once the view freed a remembered block
/// in a title-loaded engine, which was that engine's INF buffer (BA54),
/// and the next level's INF overran the heap's chain.
#[test]
fn the_automap_font_block_freed_on_leaving_and_only_its_own() {
    let Some(dir) = game_dir() else { return };
    use arena::level::var::BUFFER_BLOCK;
    use arena::mem::DS;
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let chain = |m: &arena::machine::Machine| {
        let mut out = Vec::new();
        let (mut s, mut o) = (m.mem.r16(DS, 0x0159), m.mem.r16(DS, 0x0157));
        while out.len() < 4096 {
            out.push((s, m.mem.r16(s, o + 8), m.mem.r8(s, o + 10)));
            let n = m.mem.r32(s, o);
            if n == 0 {
                break;
            }
            (s, o) = ((n >> 16) as u16, n as u16);
        }
        out
    };
    let before = chain(&a.engine.m);
    d.key(&mut a, KeyCode::KeyM, None);
    assert!(a.map);
    let font = a.engine.m.mem.r16(DS, 0xA86C);
    assert_ne!(font, 0, "FONT_S.DAT loaded");
    assert_eq!(a.engine.m.mem.r16(DS, arena::mapnotes::var::FONT + 2), font, "and selected");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(!a.map);
    assert_eq!(chain(&a.engine.m), before, "the block freed on leaving");
    assert_eq!(a.engine.m.mem.r16(DS, arena::mapnotes::var::FONT + 2), a.engine.m.mem.r16(DS, 0x102C), "the game's font back");
    // A86C left naming the INF's buffer: opening and leaving again frees
    // only the screen's own block.
    let buffer = a.engine.m.mem.r16(DS, BUFFER_BLOCK + 2);
    a.engine.m.mem.w16(DS, 0xA86C, buffer);
    d.key(&mut a, KeyCode::KeyM, None);
    a.engine.m.mem.w16(DS, 0xA86C, buffer);
    d.key(&mut a, KeyCode::Escape, None);
    let m = &a.engine.m;
    assert_eq!(m.mem.r8(buffer.wrapping_sub(1), 10) & 1, 1, "the INF's buffer still in use");
    assert!(m.heap_check());
}

/// Each of the bar's buttons clicked does what its handler does.
#[test]
fn each_bar_button_as_the_original_wires_it() {
    let Some(dir) = game_dir() else { return };
    use arena::mem::DS;
    let fresh = || {
        let mut a = app(&dir, &Start::default());
        let d = Driver::new(&mut a);
        (a, d)
    };
    let click = |a: &mut App, d: &mut Driver, i: usize| {
        let (x, y) = sheets::bar_centre(i);
        d.click(a, x, y, MouseButton::Left);
        assert!(!a.captured, "button {i} doesn't capture the mouse");
    };
    // 0, the crossed swords: the weapon readied, as Q (the view's).
    let (mut a, mut d) = fresh();
    let (mut b, mut db) = fresh();
    click(&mut a, &mut d, 0);
    db.key(&mut b, KeyCode::KeyQ, None);
    same(&a, &b, "the swords and Q");
    assert!(a.engine.m.mem.r16(DS, 0x05E0) & 0x10 != 0, "readied");
    // 1, the map: the automap.
    let (mut a, mut d) = fresh();
    click(&mut a, &mut d, 1);
    assert!(a.map, "the map");
    // 2, the hand: picking armed.
    let (mut a, mut d) = fresh();
    click(&mut a, &mut d, 2);
    let (mut b, _) = fresh();
    b.engine.m.pilfer_arm();
    same(&a, &b, "the hand and pilfer_arm");
    // 3, the status: its box, status_text's text.
    let (mut a, mut d) = fresh();
    click(&mut a, &mut d, 3);
    let (mut b, _) = fresh();
    b.engine.m.status_text();
    let want = String::from_utf8_lossy(b.engine.m.mem.cstr(DS, arena::status::var::STATUS_TEXT)).into_owned();
    let got = a.boxes.front().map(|x| text(x)).unwrap_or_default();
    let first = want.split(['\r', '\n']).find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    assert!(!first.is_empty() && got.contains(&first), "the status box: {got:?} / {want:?}");
    // 3 with a condition: poisoned (+40Dh bit 2) is listed after the
    // text (2686:005E), the same text status_box makes.
    let (mut a, mut d) = fresh();
    let f = a.engine.m.mem.r16(DS, 0x01A4 + 0x40D) | 2;
    a.engine.m.mem.w16(DS, 0x01A4 + 0x40D, f);
    click(&mut a, &mut d, 3);
    let got = a.boxes.front().map(|x| text(x)).unwrap_or_default();
    assert!(got.contains("You are poisoned."), "the status box's condition: {got:?}");
    shot(&mut a, "status_poisoned");
    let (mut b, _) = fresh();
    b.engine.m.mem.w16(DS, 0x01A4 + 0x40D, f);
    b.engine.m.status_box();
    let want = String::from_utf8_lossy(b.engine.m.mem.cstr(DS, arena::status::var::STATUS_TEXT)).into_owned();
    assert!(want.contains("You are poisoned.\r"), "status_box: {want:?}");
    // 4, the star: the spellbook.
    let (mut a, mut d) = fresh();
    click(&mut a, &mut d, 4);
    assert!(matches!(a.panel, Some(panels::Panel::Spellbook(_))), "the spellbook");
    // 5, the scroll: the logbook.
    let (mut a, mut d) = fresh();
    click(&mut a, &mut d, 5);
    assert!(matches!(a.panel, Some(panels::Panel::Log(_))), "the logbook");
    // 6, the fist: the items with an effect, when there are any.
    let (mut a, mut d) = fresh();
    let any = !panels::usable_names(&a.engine.m).is_empty();
    click(&mut a, &mut d, 6);
    assert_eq!(matches!(&a.panel, Some(panels::Panel::Use(..))), any, "the use list");
    // 7, the campfire: camping asked, as R.
    let (mut a, mut d) = fresh();
    click(&mut a, &mut d, 7);
    assert!(a.camp, "camp");
    // 8, the portrait: the sheet (00BE: the player named).
    let (mut a, mut d) = fresh();
    named(&mut a);
    click(&mut a, &mut d, 8);
    assert!(matches!(a.panel, Some(panels::Panel::Sheet)), "the sheet");
}

/// The screen as drawn now, written to $SHOT_DIR/NAME.png when SHOT_DIR
/// is set (for looking at; no check).
fn shot(app: &mut App, name: &str) {
    if let Some(dir) = std::env::var_os("SHOT_DIR") {
        app.compose();
        let p = std::path::Path::new(&dir).join(format!("{name}.png"));
        crate::create::Creation::save_png(&app.screen, &p.to_string_lossy());
    }
}

/// The notes the automap screen draws (0000:32FE), on a copy.
fn notes_drawn(app: &App) -> Vec<(u16, u16, String)> {
    struct Collect(Vec<(u16, u16, String)>);
    impl arena::mapnotes::NoteHooks for Collect {
        fn draw_text(&mut self, m: &mut arena::machine::Machine, x: u16, y: u16, si: u16) {
            self.0.push((x, y, String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, si)).into_owned()));
        }
    }
    let (mut m, mut fs) = (app.engine.m.clone(), app.engine.files.clone());
    let mut c = Collect(Vec::new());
    m.notes_draw(&mut fs, &mut c);
    c.0
}

/// A click on the automap's map asks for a note's text (0000:31FE); typed
/// and Enter, the note is in the level's AUTOMAP record and drawn; a click
/// on it again edits it; it outlives a save and a load.
#[test]
fn a_note_added_on_the_automap_by_a_click_and_keys() {
    let Some(dir) = game_dir() else { return };
    let saves = Scratch::new("notes");
    let mut a = app(&dir, &Start { save_dir: Some(saves.to_path_buf()), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let before = notes_drawn(&a);
    d.key(&mut a, KeyCode::KeyM, None);
    assert!(a.map);
    a.compose();
    let bare = a.screen.px.clone();
    d.click(&mut a, 100, 100, MouseButton::Left);
    match a.pending.as_ref().map(|p| &p.need) {
        // Typed in place where clicked (0000:9EE9), not in a box.
        Some(dialog::Need::Text(ask)) => {
            assert!(matches!(ask.prompt, dialog::Prompt::Bare), "in place");
            assert_eq!((ask.field.x, ask.field.y), (100, 100), "the note edited where clicked");
        }
        _ => panic!("a click on the map asks for the note's text"),
    }
    type_text(&mut d, &mut a, "Hi");
    a.compose();
    let field = |px: &[[u8; 4]]| -> Vec<[u8; 4]> { (0..6).flat_map(|r| (0..12).map(move |c| (100 + r) * hud::W + 100 + c)).map(|i| px[i]).collect() };
    assert_ne!(field(&a.screen.px), field(&bare), "the text typed drawn on the map");
    shot(&mut a, "automap_note_typed");
    d.key(&mut a, KeyCode::Enter, None);
    assert!(a.pending.is_none(), "Enter answers");
    assert!(a.map, "the automap stays up");
    let after = notes_drawn(&a);
    eprintln!("notes: {after:?}");
    assert_eq!(after.len(), before.len() + 1, "one note more");
    let note = after.iter().find(|n| n.2 == "Hi").expect("the note drawn").clone();
    // Its square under (100, 100), as 0000:3745 places it (3 pixels a
    // square, drawn from 3763's point).
    assert!((note.0 as i32 - 100).abs() <= 3 && (note.1 as i32 - 100).abs() <= 3, "drawn where clicked: {note:?}");
    // The screen draws it (FONT_S, colour 18h): the pixels at its point
    // change.
    a.compose();
    let near = |px: &[[u8; 4]]| -> Vec<[u8; 4]> {
        (0..6).flat_map(|r| (0..10).map(move |c| (note.1 as usize + r) * hud::W + note.0 as usize + c)).map(|i| px[i]).collect()
    };
    assert_ne!(near(&a.screen.px), near(&bare), "the note drawn on the screen");
    shot(&mut a, "automap_note");
    // Clicked again: the note's text asked for again, now edited.
    d.click(&mut a, note.0 as i32 + 1, note.1 as i32 + 1, MouseButton::Left);
    assert!(a.pending.is_some(), "a click on the note edits it");
    assert_eq!(typed(&a), "Hi", "the note's text to edit");
    // The cursor where the click falls (A1B9): a pixel in, before the
    // first character; Del takes the two out.
    d.key(&mut a, KeyCode::Delete, None);
    d.key(&mut a, KeyCode::Delete, None);
    type_text(&mut d, &mut a, "X");
    d.key(&mut a, KeyCode::Enter, None);
    let edited = notes_drawn(&a);
    assert_eq!(edited.len(), after.len(), "edited, not added");
    assert!(edited.iter().any(|n| n.2 == "X") && !edited.iter().any(|n| n.2 == "Hi"), "{edited:?}");
    // Saved, the map left, loaded: still there.
    d.key(&mut a, KeyCode::Escape, None);
    d.key(&mut a, KeyCode::F5, None);
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // Changed after the save (Y), then loaded: X again.
    let x = edited.iter().find(|n| n.2 == "X").unwrap().clone();
    d.key(&mut a, KeyCode::KeyM, None);
    d.click(&mut a, x.0 as i32 + 1, x.1 as i32 + 1, MouseButton::Left);
    d.key(&mut a, KeyCode::Delete, None);
    type_text(&mut d, &mut a, "Y");
    d.key(&mut a, KeyCode::Enter, None);
    assert!(notes_drawn(&a).iter().any(|n| n.2 == "Y"), "changed after the save");
    d.key(&mut a, KeyCode::Escape, None);
    d.key(&mut a, KeyCode::F9, None);
    let loaded = notes_drawn(&a);
    assert!(loaded.iter().any(|n| n.2 == "X") && !loaded.iter().any(|n| n.2 == "Y"), "the save's note loaded back: {loaded:?}");
}

/// The squares of trigger walls whose trigger's index (in TRIGGERS)
/// `pick` accepts, each with the open squares beside it.
fn trigger_walls(m: &arena::machine::Machine, pick: impl Fn(u16) -> bool) -> Vec<(u16, Vec<(u16, u16)>)> {
    use arena::mem::DS;
    use arena::movement::var::TRIGGERS;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (rows, cols) = (m.mem.r16(DS, 0xC359), m.mem.r16(DS, 0xC357));
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let sq = r << 8 | c << 1;
            let w = m.mem.r16(walls, sq);
            if (w as i16) >= 0 && (w >> 8) as u8 != w as u8 {
                continue;
            }
            let Some(i) = (0..0x10u16).find(|&i| m.mem.r8(DS, TRIGGERS + i) == w as u8 & 0x3F) else { continue };
            if !pick(i) {
                continue;
            }
            let open = [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)]
                .iter()
                .map(|&(dc, dr)| (c as i32 + dc, r as i32 + dr))
                .filter(|&(c, r)| c >= 0 && r >= 0 && (c as u16) < cols && (r as u16) < rows)
                .filter(|&(c, r)| m.mem.r16(walls, (r as u16) << 8 | (c as u16) << 1) == 0)
                .map(|(c, r)| (c as u16, r as u16))
                .collect();
            out.push((sq, open));
        }
    }
    out
}

/// The boxes read (Enter) and a menu left (Esc).
fn clear(d: &mut Driver, a: &mut App) {
    for _ in 0..20 {
        if !a.boxes.is_empty() {
            d.key(a, KeyCode::Enter, None);
        } else if menu(a).is_some() {
            d.key(a, KeyCode::Escape, None);
        } else {
            return;
        }
    }
}

/// Stands beside each wall of `walls` in turn, facing each way, and
/// clicks the view's centre (164, 74) until `done`: a wall's trigger
/// used by the window's own click.
fn click_a_wall(d: &mut Driver, a: &mut App, walls: &[(u16, Vec<(u16, u16)>)], done: impl Fn(&App) -> bool) -> bool {
    use arena::movement::var::{HEADING, X, Y};
    for (_, open) in walls {
        for &(c, r) in open {
            for heading in [0u16, 0x80, 0x100, 0x180] {
                a.engine.m.mem.w16(arena::mem::DS, X, c << 7 | 64);
                a.engine.m.mem.w16(arena::mem::DS, Y, r << 7 | 64);
                a.engine.m.mem.w16(arena::mem::DS, HEADING, heading);
                a.engine.world_loop_start();
                d.frame(a);
                d.click(a, 164, 74, MouseButton::Left);
                d.frames(a, 2);
                if done(a) {
                    return true;
                }
                clear(d, a);
            }
        }
    }
    false
}

/// The user's save (ARENA_SAVE, slot 1: in Daggerfall's mages' guild,
/// the main quest at stage 9) walked out of the guild, travelled to the
/// Imperial City through the travel map's clicks, the town's welcome
/// read, and the palace's door clicked: the artifact's audience, text
/// 5DDh (0000:425F), shown whole in its box -- the last template, read
/// on past TEMPLATE.DAT's end into what the welcome's box kept.
#[test]
fn the_palace_at_stage_9_shows_the_artifacts_audience() {
    use arena::mem::DS;
    use arena::movement::var::MAP_FLAGS;
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let mut a = app(&dir, &Start { hour: None, load: Some((save.to_path_buf(), 1)), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    clear(&mut d, &mut a);
    assert_eq!(a.engine.m.mem.r8(DS, 0x0F77), 9, "the save's stage");
    let outdoors = |a: &App| a.engine.m.mem.r16(DS, MAP_FLAGS) & 4 != 0;
    let exits = trigger_walls(&a.engine.m, |_| true);
    assert!(click_a_wall(&mut d, &mut a, &exits, outdoors), "out of the guild");
    d.frames(&mut a, 2);
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // Shift+M: the world map; the Imperial Province (its region at
    // DATA:6D90 + 8 x 62h: 103, 72, 131 x 84); the Imperial City at
    // (180, 65); the journey's box read; Travel.
    a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
    d.key(&mut a, KeyCode::KeyM, Some("M"));
    a.input(In::Key { code: KeyCode::ShiftLeft, down: false, text: None });
    assert!(a.travel.is_some(), "the travel map");
    d.click(&mut a, 160, 100, MouseButton::Left);
    d.click(&mut a, 180, 65, MouseButton::Left);
    let journey = a.boxes.iter().map(|x| text(x)).collect::<Vec<_>>().join("\n");
    assert!(journey.contains("Imperial City"), "the journey: {journey}");
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    d.click(&mut a, 62, 180, MouseButton::Left);
    // The horse runs a frame a day (1194:30F1), then the screen ends.
    for _ in 0..10 {
        if a.travel.is_none() {
            break;
        }
        d.frame(&mut a);
    }
    assert!(a.travel.is_none(), "travelled");
    let mut welcome = String::new();
    for _ in 0..20 {
        d.frame(&mut a);
        if let Some(b) = a.boxes.front() {
            welcome = text(b);
            break;
        }
    }
    assert!(welcome.contains("You have arrived in the Imperial City"), "the welcome: {welcome:?}");
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // The palace's door: the city's triggers 11..13 (0000:4192).
    let doors = trigger_walls(&a.engine.m, |i| (11..=13).contains(&i));
    assert!(!doors.is_empty(), "the palace's door");
    let audience = |a: &App| a.boxes.iter().any(|b| text(b).contains("Palace Gates"));
    assert!(click_a_wall(&mut d, &mut a, &doors, audience), "the audience");
    let t = a.boxes.iter().map(|x| text(x)).find(|t| t.contains("Palace Gates")).unwrap();
    assert!(t.starts_with("As you approach the Palace Gates") && t.trim_end().ends_with("complete..."), "the audience: {t:?}");
}

/// The user's save (ARENA_SAVE, slot 1) loaded from the title screen's
/// way (`areas::load_game_title`: 5C8 bit 4, the world loop entered
/// afresh) and in play's (`areas::load_game`) onto the same fresh start:
/// the same area, place and game.
#[test]
fn a_save_loaded_from_the_title_as_in_play() {
    use arena::mem::DS;
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let fresh = || {
        let mut e = world::load(&dir, "START.MIF");
        areas::start(&mut e, false);
        e
    };
    let mut a = fresh();
    let ra = areas::load_game(&mut a, &dir, &save, 1).expect("loads in play");
    let mut b = fresh();
    let rb = areas::load_game_title(&mut b, &dir, &save, 1).expect("loads from the title");
    assert_eq!(format!("{ra:?}"), format!("{rb:?}"), "the run");
    // The game's state the same: the character, the place, the flags;
    // the title's way differs where the world loop was entered afresh
    // (its way back, the hook, the ceiling, the sky's time, the rain's
    // drops, the heap's and EMS's bookkeeping).
    let same = |at: u16, n: u16| (at..at + n).all(|o| a.m.mem.r8(DS, o) == b.m.mem.r8(DS, o));
    assert!(same(arena::character::PLAYER, 0x400), "the character");
    assert!(same(0xC3D0, 6) && same(0x05E6, 1) && same(0x0A46, 1) && same(0x05C8, 4), "the place and the flags");
    assert!(b.m.mem.r16(DS, 0xBA7A) != 0 && b.m.mem.r16(DS, 0xBA7C) == 0, "the way back: the outer loop's");
    assert_eq!(b.m.mem.r16(DS, 0x05C8) & 4, 0, "5C8 bit 4 cleared");
    assert_eq!(b.m.mem.r16(DS, 0xBA76), 6, "the hook's offset kept, its segment cleared");
}

/// The options screen's Save (Esc, then its box at 81h..C0h, 76h..92h):
/// the save menu; slot 3's row clicked, a name typed, Enter: the
/// game saved to a scratch folder (NAMES.DAT with the name, SAVEGAME.03);
/// then a fresh program's main menu Load, slot 3's row clicked: the game
/// loaded the title screen's way, where it was saved.
#[test]
fn saved_by_the_menu_then_loaded_from_the_title() {
    use arena::mem::DS;
    use arena::movement::var::{X, Y};
    let Some(dir) = game_dir() else { return };
    let scratch = Scratch::new("savemenu");
    let mut a = app(&dir, &Start { save_dir: Some(scratch.to_path_buf()), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.key(&mut a, KeyCode::Escape, None);
    d.click(&mut a, 0xA0, 0x80, MouseButton::Left);
    assert!(a.options.is_none() && a.saveui.as_ref().is_some_and(|u| u.kind == saveui::Kind::Save), "the save menu is up");
    d.click(&mut a, 160, 4 + 3 * 14 + 3, MouseButton::Left);
    assert!(a.saveui.as_ref().is_some_and(|u| u.typing.is_some()), "slot 3's name is typed");
    type_text(&mut d, &mut a, "Ria");
    d.key(&mut a, KeyCode::Enter, None);
    assert!(a.saveui.is_none(), "the menu closed");
    let names = saveui::read_any_case(&scratch, "NAMES.DAT").expect("NAMES.DAT");
    assert_eq!(&names[3 * 0x30..3 * 0x30 + 4], b"Ria\0", "slot 3's name");
    assert!(saveui::read_any_case(&scratch, "SAVEGAME.03").is_some(), "SAVEGAME.03 written");
    let at = |e: &Engine| (e.m.mem.r16(DS, X), e.m.mem.r16(DS, Y), e.m.mem.r8(DS, arena::trigger::var::AREA));
    let saved = at(&a.engine);

    let mut engine = world::load(&dir, "START.MIF");
    let archive = Rc::new(archive::Archive::open(&dir));
    let c = create::Creation::new(&mut engine, archive, &dir, world::palette(&dir));
    let mut b = make_app(dir.clone(), engine, arena::transit::Run::Start, Some(c), Default::default(), Some(scratch.to_path_buf()), 0, false);
    let mut d = Driver::new(&mut b);
    d.key(&mut b, KeyCode::Enter, None);
    d.key(&mut b, KeyCode::Enter, None);
    d.key(&mut b, KeyCode::KeyL, Some("L"));
    assert!(b.saveui.as_ref().is_some_and(|u| u.kind == saveui::Kind::TitleLoad), "the title's load menu is up");
    let mut shot = hud::Screen::new(b.assets.palette);
    b.saveui.as_ref().unwrap().draw(&mut shot);
    if let Ok(p) = std::env::var("ARENA_SHOT") {
        let data: Vec<u8> = shot.px.iter().flat_map(|c| [c[0], c[1], c[2], 0xFF]).collect();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&p).unwrap()), hud::W as u32, hud::H as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&data).unwrap();
    }
    d.click(&mut b, 160, 4 + 3 * 14 + 3, MouseButton::Left);
    assert!(b.saveui.is_none() && b.creation.is_none(), "loaded and playing");
    assert_eq!(at(&b.engine), saved, "where it was saved");
}

/// Esc in play: the options screen (2542:000A). Its arrows change the
/// sound's and the music's volumes (D3B, D3A), its bar the detail (D37),
/// each as the port's `options_screen` with the same clicks; Resume leaves
/// with them; Load opens the load menu; Exit asks, No stays, Yes quits.
#[test]
fn the_options_screen_by_esc() {
    use arena::mem::DS;
    use arena::options::var::{DETAIL, MUSIC, SOUND};
    let Some(dir) = game_dir() else { return };
    let scratch = Scratch::new("options");
    let mut a = app(&dir, &Start { save_dir: Some(scratch.to_path_buf()), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let before = Engine { m: a.engine.m.clone(), files: a.engine.files.clone() };
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.options.is_some() && !a.captured, "Esc opens the options screen, the mouse let go");
    let mut shot = hud::Screen::new(a.assets.palette);
    a.options.as_ref().unwrap().draw(&mut shot, &a.font);
    if let Ok(p) = std::env::var("ARENA_SHOT_OPTIONS") {
        let data: Vec<u8> = shot.px.iter().flat_map(|c| [c[0], c[1], c[2], 0xFF]).collect();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(p).unwrap()), hud::W as u32, hud::H as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&data).unwrap();
    }
    // Esc on the screen: 564D's FFFFh, in no box: still up.
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.options.is_some(), "Esc doesn't leave the screen");
    // The sound's down arrow twice, the music's once, the detail bar at
    // its 9th step, then Resume.
    let clicks = [(0x30, 0x6A), (0x3E, 0x6F), (0x80, 0x6A), (0xCB + 9 * 5, 0x5E), (0x120, 0x80)];
    for &(x, y) in &clicks {
        d.click(&mut a, x, y, MouseButton::Left);
    }
    assert!(a.options.is_none(), "Resume leaves");
    let v = |m: &arena::machine::Machine| (m.mem.r8(DS, SOUND), m.mem.r8(DS, MUSIC), m.mem.r16(DS, DETAIL));
    let (s0, m0, _) = v(&before.m);
    assert_eq!(v(&a.engine.m), (s0.saturating_sub(2), m0.saturating_sub(1), 15), "the volumes and the detail as set");
    // The same clicks through the port.
    struct Clicks(Vec<(u16, u16)>);
    impl arena::options::OptionsHooks for Clicks {
        fn click(&mut self, m: &mut arena::machine::Machine) -> Option<(u16, u16)> {
            let c = self.0.pop()?;
            m.mem.w16(DS, arena::options::var::MOUSE_X, c.0);
            m.mem.w16(DS, arena::options::var::MOUSE_Y, c.1);
            Some(c)
        }
    }
    let mut b = before;
    let mut script = Clicks(clicks.iter().rev().map(|&(x, y)| (x as u16, y as u16)).collect());
    assert_eq!(b.m.options_screen(&mut b.files, &mut script), Some(arena::options::OptionsEnd::Resume));
    assert_eq!(v(&a.engine.m), v(&b.m), "as the port's screen with the same clicks");
    // The sound's and the music's up arrows: up by one each.
    d.key(&mut a, KeyCode::Escape, None);
    d.click(&mut a, 0x30, 0x50, MouseButton::Left);
    d.click(&mut a, 0x80, 0x50, MouseButton::Left);
    d.click(&mut a, 0x120, 0x80, MouseButton::Left);
    assert_eq!(v(&a.engine.m), (s0.saturating_sub(1), m0, 15), "the up arrows");
    // Load: the load menu.
    d.key(&mut a, KeyCode::Escape, None);
    d.click(&mut a, 0x60, 0x80, MouseButton::Left);
    assert!(a.options.is_none() && a.saveui.as_ref().is_some_and(|u| u.kind == saveui::Kind::Load), "Load: the load menu");
    d.key(&mut a, KeyCode::Escape, None);
    assert!(a.saveui.is_none(), "the load menu left");
    // Exit: "Are you sure ?"; No stays, Yes quits.
    d.key(&mut a, KeyCode::Escape, None);
    d.click(&mut a, 0xE0, 0x80, MouseButton::Left);
    assert!(a.options.as_ref().is_some_and(|u| u.confirm.is_some()), "Exit asks");
    d.key(&mut a, KeyCode::KeyN, Some("n"));
    assert!(a.options.as_ref().is_some_and(|u| u.confirm.is_none()) && !a.quit, "No: the screen again");
    d.click(&mut a, 0xE0, 0x80, MouseButton::Left);
    d.key(&mut a, KeyCode::KeyY, Some("y"));
    assert!(a.options.is_none() && a.quit, "Yes: the program ends");
}

/// The options screen's New: the main menu (5CA bit 8000h's outer loop).
#[test]
fn the_options_screens_new_goes_to_the_main_menu() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    d.key(&mut a, KeyCode::Escape, None);
    d.click(&mut a, 0x20, 0x80, MouseButton::Left);
    assert!(a.options.is_none() && a.creation.is_some(), "New: the main menu");
}

/// A creature's flat (bit 2 of its map word's high byte) ahead of the
/// player and near: the drawing reaches it (0000:CF96), the hook CD56
/// (25CA:0271) brings its creature; behind the player it isn't reached.
#[test]
fn a_creatures_flat_seen_near_comes_alive() {
    use arena::mem::DS;
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start { level: "START.MIF", hour: None, new_game: Some("0:3:0:2:Talin"), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    let m = &mut a.engine.m;
    m.mem.w32(DS, 0xCD56, 0x25CA_0271);
    // A flat of the first creature kind (C954), named if the level has none.
    if m.mem.r8(DS, 0xC954) == 0 {
        m.mem.w8(DS, 0xC954, 0x10);
    }
    let flat = m.mem.r8(DS, 0xC954);
    use arena::movement::var::{HEADING, X, Y};
    let (x, y) = (m.mem.r16(DS, X), m.mem.r16(DS, Y));
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    // A square two away along an axis (empty or a flat's), with the one
    // between empty.
    let (col, row) = ((x >> 7) as i32, (y >> 7) as i32);
    let empty = |m: &arena::machine::Machine, c: i32, r: i32| c >= 0 && r >= 0 && m.mem.r16(walls, (r << 8 | c << 1) as u16) == 0;
    let (dc, dr, heading) = [(0, 1, 0u16), (1, 0, 128), (0, -1, 256), (-1, 0, 384)]
        .into_iter()
        .find(|&(dc, dr, _)| {
            let (c, r) = (col + 2 * dc, row + 2 * dr);
            empty(m, col + dc, row + dr) && c >= 0 && r >= 0 && matches!(m.mem.r16(walls, (r << 8 | c << 1) as u16) >> 12, 0 | 8)
        })
        .unwrap_or_else(|| {
            let around: Vec<String> = (-2..=2).map(|r| (-2..=2).map(|c| format!("{:04x}", m.mem.r16(walls, ((row + r) << 8 | (col + c) << 1) as u16))).collect::<Vec<_>>().join(" ")).collect();
            panic!("an open line from the start: {around:?}")
        });
    let di = ((row + 2 * dr) << 8 | (col + 2 * dc) << 1) as u16;
    m.mem.w16(walls, di, 0x8200 | flat as u16);
    // Facing away first: not reached, nothing comes.
    m.mem.w16(DS, HEADING, (heading + 256) & 0x1FF);
    assert!(world::creature_flats_seen(m).is_empty(), "behind the player, not reached");
    d.frame(&mut a);
    assert_eq!(a.engine.m.mem.r16(DS, 0x05C8) & 0x1000, 0, "nothing came from behind");
    a.engine.m.mem.w16(DS, HEADING, heading);
    assert_eq!(world::creature_flats_seen(&a.engine.m), vec![((col + 2 * dc) as u16, (row + 2 * dr) as u16, di)]);
    d.frame(&mut a);
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    let m = &a.engine.m;
    assert_ne!(m.mem.r16(DS, 0x05C8) & 0x1000, 0, "the flat came alive (5C8 bit 1000h)");
    assert_eq!(m.mem.r8(DS, 0x126C), 1, "its kind, the first");
    assert_eq!(m.mem.r16(DS, arena::encounter::var::FLAT_AT), di, "its word kept");
    assert_ne!(m.mem.r16(DS, arena::encounter::var::FIGHTING), 0, "its creature came");
}

/// The stairs out of a dungeon's first level (the way out 0000:4537):
/// the travel map shown (26F7:0000, a turn's question); left by Esc it
/// stays, a journey made from it leaves.
#[test]
fn the_stairs_out_show_the_travel_map() {
    use arena::mem::DS;
    use arena::movement::var::{HEADING, LEVEL, MAP_FLAGS, STAIRS_UP, X, Y};
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start { level: "START.MIF", hour: None, new_game: Some("0:3:0:2:Talin"), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    // Stairs up planted beside the player, faced, on the first level, the
    // way out a dungeon's (not entered by a trigger nor the wilderness's).
    let m = &mut a.engine.m;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (col, row) = ((m.mem.r16(DS, X) >> 7) as i32, (m.mem.r16(DS, Y) >> 7) as i32);
    let (dc, dr, heading) = [(0, 1, 0u16), (1, 0, 128), (0, -1, 256), (-1, 0, 384)]
        .into_iter()
        .find(|&(dc, dr, _)| m.mem.r16(walls, ((row + dr) << 8 | (col + dc) << 1) as u16) == 0)
        .expect("an open square beside the start");
    let s = m.mem.r8(DS, STAIRS_UP) as u16;
    m.mem.w16(walls, ((row + dr) << 8 | (col + dc) << 1) as u16, s << 8 | s);
    m.mem.w16(DS, HEADING, heading);
    m.mem.w8(DS, LEVEL, 0);
    m.mem.w32(DS, arena::transit::var::EXIT_HOOK, 0x4537);
    if m.mem.r8(DS, 0x05E6) == 4 {
        m.mem.w8(DS, 0x05E6, 2);
    }
    let f = m.mem.r16(DS, 0x05C8) & !0x4080;
    m.mem.w16(DS, 0x05C8, f);
    let here = |a: &App| String::from_utf8_lossy(a.engine.m.mem.cstr(DS, 0x0A22)).into_owned();
    let before = here(&a);
    let walk = |a: &mut App, d: &mut Driver| {
        a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
        for _ in 0..40 {
            d.frame(a);
            if a.travel.is_some() {
                break;
            }
        }
        a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
    };
    walk(&mut a, &mut d);
    assert!(a.travel.is_some(), "the stairs show the travel map");
    // It opens on the world map; Esc ends it, no journey made. The
    // move's candidates (0000:F76F for each) each meet the stairs, so the
    // frame asks again until all are answered, as the original's.
    let mut asked = 0;
    while a.travel.is_some() && asked < 8 {
        asked += 1;
        assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Travel(_))), "a turn's question");
        d.key(&mut a, KeyCode::Escape, None);
    }
    assert!(a.travel.is_none() && a.pending.is_none(), "the screen ended, the turn answered after {asked}");
    assert_eq!(a.engine.m.mem.r16(DS, MAP_FLAGS) & 0x80, 0, "stayed: the area not left");
    d.frames(&mut a, 2);
    assert!(a.travel.is_none(), "not asked again standing");
    // Again, and a journey made: the dungeon left for the place.
    walk(&mut a, &mut d);
    assert!(a.travel.is_some(), "asked again on the stairs");
    let (px, py) = travel::province_point(a.engine.m.mem.r8(DS, 0x0A46));
    d.click(&mut a, px, py, MouseButton::Left);
    d.click(&mut a, 135, 58, MouseButton::Left);
    assert!(!a.boxes.is_empty(), "the journey's box");
    while !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Enter, None);
    }
    d.click(&mut a, 62, 180, MouseButton::Left);
    for _ in 0..20 {
        if a.travel.is_none() {
            break;
        }
        d.wait(&mut a, 100);
    }
    assert!(a.travel.is_none() && a.pending.is_none(), "the screen ended after the journey");
    for _ in 0..20 {
        while !a.boxes.is_empty() {
            d.key(&mut a, KeyCode::Enter, None);
        }
        d.frame(&mut a);
    }
    assert_ne!(here(&a), before, "the dungeon left for the place travelled to");
}

/// A shop's screen behind its questions (1194:4B60, 4BFF): the tavern's
/// name made from "%nt " on the banner; gone once the shop is left.
#[test]
fn a_shops_banner_is_drawn_behind_its_menu() {
    let Some(dir) = game_dir() else { return };
    let mut a = app(&dir, &Start { level: "TAVERN1.MIF", ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    shop_turn(&mut a, 4);
    let b = a.pending.as_ref().and_then(|p| p.backdrop.clone()).expect("the shop's screen behind its menu");
    let title = String::from_utf8_lossy(&b.title).into_owned();
    assert!(!title.is_empty() && !title.contains('%'), "the tavern's name made: {title:?}");
    a.compose();
    let banner = |px: &[[u8; 4]]| -> Vec<[u8; 4]> { (0..14).flat_map(|r| (100..220).map(move |c| r * hud::W + c)).map(|i| px[i]).collect() };
    let up = banner(&a.screen.px);
    shot(&mut a, "shop_banner");
    while a.pending.is_some() || !a.boxes.is_empty() {
        d.key(&mut a, KeyCode::Escape, None);
    }
    a.compose();
    assert_ne!(banner(&a.screen.px), up, "the banner gone with the shop");
}

/// The player given a name (01AD), as every game's player has: the
/// sheet (2542:00BE) only opens for one; the view's bare start has none.
fn named(a: &mut App) {
    a.engine.m.mem.w8(arena::mem::DS, arena::menukeys::var::NAME, b'A');
    a.engine.m.mem.w8(arena::mem::DS, arena::menukeys::var::NAME + 1, 0);
}

/// Presses `code` as the window would (`Driver::key`), Shift held when
/// `shift`.
fn press(d: &mut Driver, a: &mut App, code: KeyCode, shift: bool) {
    if shift {
        a.input(In::Key { code: KeyCode::ShiftLeft, down: true, text: None });
    }
    d.key(a, code, None);
    if shift {
        a.input(In::Key { code: KeyCode::ShiftLeft, down: false, text: None });
    }
}

/// Each of the original's menu keys (2542:0100) does what its handler
/// does: the state-only ones as the engine's own routines (or the bar's
/// button with the same handler), the others open that handler's screen.
#[test]
fn each_original_menu_key_as_its_handler() {
    let Some(dir) = game_dir() else { return };
    use arena::mem::DS;
    let fresh = || {
        let mut a = app(&dir, &Start::default());
        let mut d = Driver::new(&mut a);
        d.frame(&mut a);
        (a, d)
    };
    let bar = |a: &mut App, d: &mut Driver, i: usize, right: bool| {
        let (x, y) = sheets::bar_centre(i);
        d.click(a, x, y, if right { MouseButton::Right } else { MouseButton::Left });
    };
    // A: the weapon readied (0137), as the swords (0486); again, put away.
    let (mut a, mut d) = fresh();
    let (mut b, mut db) = fresh();
    press(&mut d, &mut a, KeyCode::KeyA, false);
    bar(&mut b, &mut db, 0, false);
    same(&a, &b, "A and the swords");
    assert!(a.engine.m.mem.r16(DS, 0x05E0) & 0x10 != 0, "A readies");
    press(&mut d, &mut a, KeyCode::KeyA, false);
    assert!(a.engine.m.mem.r16(DS, 0x05E0) & 0x10 == 0, "A again puts it away");
    // A held (the window's repeats): one press, one toggle.
    let (mut a, _) = fresh();
    for _ in 0..3 {
        a.input(In::Key { code: KeyCode::KeyA, down: true, text: None });
    }
    a.input(In::Key { code: KeyCode::KeyA, down: false, text: None });
    assert!(a.engine.m.mem.r16(DS, 0x05E0) & 0x10 != 0, "a held A toggles once");
    // A with C39A bit 4000h: nothing.
    let (mut a, mut d) = fresh();
    let f = a.engine.m.mem.r16(DS, arena::movement::var::LEVEL_FLAGS) | 0x4000;
    a.engine.m.mem.w16(DS, arena::movement::var::LEVEL_FLAGS, f);
    let was = a.engine.m.mem.r16(DS, 0x05E0);
    press(&mut d, &mut a, KeyCode::KeyA, false);
    assert_eq!(a.engine.m.mem.r16(DS, 0x05E0), was, "A barred");
    // P: picking pockets armed (0279), as the hand.
    let (mut a, mut d) = fresh();
    let (mut b, mut db) = fresh();
    press(&mut d, &mut a, KeyCode::KeyP, false);
    bar(&mut b, &mut db, 2, false);
    same(&a, &b, "P and the hand");
    assert!(a.engine.m.mem.r16(DS, 0x05CA) & 1 != 0, "P arms picking");
    // F8: the compass (01C5: 5C8 bit 2000h).
    let (mut a, mut d) = fresh();
    let was = a.engine.m.mem.r16(DS, 0x05C8);
    press(&mut d, &mut a, KeyCode::F8, false);
    assert_eq!(a.engine.m.mem.r16(DS, 0x05C8), was ^ 0x2000, "F8 flips the compass");
    // C: the spellbook (026D), unless 5CA bit 8 (set for classes that
    // don't cast, as the view's bare start's player).
    let (mut a, mut d) = fresh();
    let f = a.engine.m.mem.r16(DS, 0x05CA) & !8;
    a.engine.m.mem.w16(DS, 0x05CA, f);
    press(&mut d, &mut a, KeyCode::KeyC, false);
    assert!(matches!(a.panel, Some(panels::Panel::Spellbook(_))), "C opens the spellbook");
    let (mut a, mut d) = fresh();
    let f = a.engine.m.mem.r16(DS, 0x05CA) | 8;
    a.engine.m.mem.w16(DS, 0x05CA, f);
    press(&mut d, &mut a, KeyCode::KeyC, false);
    assert!(a.panel.is_none(), "C barred");
    // S: the status box (037C -> 04B1), as the staff.
    let (mut a, mut d) = fresh();
    let (mut b, mut db) = fresh();
    press(&mut d, &mut a, KeyCode::KeyS, false);
    bar(&mut b, &mut db, 3, false);
    assert!(!a.boxes.is_empty(), "S shows the status box");
    assert_eq!(a.boxes.iter().map(|x| text(x)).collect::<Vec<_>>(), b.boxes.iter().map(|x| text(x)).collect::<Vec<_>>(), "S and the staff");
    // U: the use list (034D), as the fist.
    let (mut a, mut d) = fresh();
    let (mut b, mut db) = fresh();
    press(&mut d, &mut a, KeyCode::KeyU, false);
    bar(&mut b, &mut db, 6, false);
    let using = |x: &App| matches!(&x.panel, Some(panels::Panel::Use(..)));
    assert_eq!(using(&a), using(&b), "U and the fist");
    // M: the automap (045A); Shift+M the world map (03BF), as the map's
    // right button.
    let (mut a, mut d) = fresh();
    press(&mut d, &mut a, KeyCode::KeyM, false);
    assert!(a.map, "M opens the automap");
    let (mut a, mut d) = fresh();
    let (mut b, mut db) = fresh();
    press(&mut d, &mut a, KeyCode::KeyM, true);
    bar(&mut b, &mut db, 1, true);
    assert!(!a.map, "Shift+M: no automap");
    assert_eq!((a.travel.is_some(), a.boxes.len()), (b.travel.is_some(), b.boxes.len()), "Shift+M and the map's right button");
    // F1: the sheet (00BE), as the portrait; with no name, nothing.
    let (mut a, mut d) = fresh();
    named(&mut a);
    press(&mut d, &mut a, KeyCode::F1, false);
    assert!(matches!(a.panel, Some(panels::Panel::Sheet)), "F1 opens the sheet");
    let (mut a, mut d) = fresh();
    a.engine.m.mem.w8(DS, arena::menukeys::var::NAME, 0);
    press(&mut d, &mut a, KeyCode::F1, false);
    assert!(a.panel.is_none(), "F1 with no name");
    // L: the logbook (02A3); R: camping (01E6).
    let (mut a, mut d) = fresh();
    press(&mut d, &mut a, KeyCode::KeyL, false);
    assert!(matches!(a.panel, Some(panels::Panel::Log(_))), "L opens the logbook");
    let (mut a, mut d) = fresh();
    press(&mut d, &mut a, KeyCode::KeyR, false);
    assert!(a.camp, "R asks for camping");
    // Esc: the options screen (000A).
    let (mut a, mut d) = fresh();
    press(&mut d, &mut a, KeyCode::Escape, false);
    assert!(a.options.is_some(), "Esc opens the options");
}

/// The movement keys as the original's (2826:0473): Up walks, Left turns,
/// Ctrl+Left strafes; W, A, S and D move nothing (A and S are the
/// game's menu keys).
#[test]
fn the_movement_keys_as_the_originals() {
    let Some(dir) = game_dir() else { return };
    let start = Start { at: Some((59, 8, 0)), ..Default::default() };
    // Against standing still the same frames (the start may settle).
    let run = |code: Option<KeyCode>, ctrl: bool| {
        let mut a = app(&dir, &start);
        let mut d = Driver::new(&mut a);
        d.frame(&mut a);
        if ctrl {
            a.input(In::Key { code: KeyCode::ControlLeft, down: true, text: None });
        }
        if let Some(code) = code {
            a.input(In::Key { code, down: true, text: None });
        }
        d.frames(&mut a, 6);
        let w = a.engine.view();
        (w.x, w.y, w.heading)
    };
    let still = run(None, false);
    let hold = |code: KeyCode, ctrl: bool| {
        let w = run(Some(code), ctrl);
        ((w.0, w.1) != (still.0, still.1), w.2 != still.2)
    };
    assert!(hold(KeyCode::ArrowLeft, false).1, "Left turns");
    assert_eq!(hold(KeyCode::ArrowLeft, true), (true, false), "Ctrl+Left strafes");
    assert!(hold(KeyCode::ArrowUp, false).0, "Up walks");
    for k in [KeyCode::KeyW, KeyCode::KeyD] {
        assert_eq!(hold(k, false), (false, false), "{k:?} moves nothing");
    }
}

/// The frame's own keys read from the game's key state: F4 held over a
/// frame flips 5CA bit 2 (0000:481E).
#[test]
fn the_frames_keys_from_the_key_state() {
    let Some(dir) = game_dir() else { return };
    use arena::mem::DS;
    let mut a = app(&dir, &Start::default());
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    let was = a.engine.m.mem.r16(DS, 0x05CA);
    a.input(In::Key { code: KeyCode::F4, down: true, text: None });
    d.frame(&mut a);
    a.input(In::Key { code: KeyCode::F4, down: false, text: None });
    assert_eq!(a.engine.m.mem.r16(DS, 0x05CA) & 2, (was ^ 2) & 2, "F4 flips 5CA bit 2");
}

#[path = "screentest.rs"]
mod screentest;

#[path = "createtest.rs"]
mod createtest;

/// 0000:0B9C's order (`panels::book_order`): ascending by byte, a name
/// after one it begins with, bytes 80h or more apart backwards (9C0A's
/// signed test); one name is left unsorted.
#[test]
fn the_spellbooks_names_sort_as_0b9c() {
    let order = |names: &[&str]| panels::book_order(&names.iter().map(|n| n.as_bytes().to_vec()).collect::<Vec<_>>());
    assert_eq!(order(&["Light Heal", "Dam. Fire Dart", "Absorb Spell +5"]), Some(vec![2, 1, 0]));
    assert_eq!(order(&["Shield +50", "Shield"]), Some(vec![1, 0]));
    assert_eq!(order(&["Open"]), None);
    // 01h against 90h: 01h - 90h is 71h, positive: the first sorts after.
    assert_eq!(panels::book_order(&[vec![0x01u8], vec![0x90u8]]), Some(vec![1, 0]));
}
