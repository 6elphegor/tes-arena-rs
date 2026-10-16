//! The view's 2D screens against the original's own drawing of them.
//!
//! harness/gen_screens.py starts the original under unicorn, loads the
//! user's saved game (slot 1) from the title, opens each screen by its
//! own routine and reads the page it drew (DGROUP A900) when it first
//! waits for input; tests/vectors/screens.txt holds each page's FNV-1a
//! digest and its palette's. Here the same save is loaded into the view,
//! the same screen opened by the window's keys and clicks, and the
//! layer composed: each pixel's palette colour must be the original's.
//!
//! The pixels compared are a screen's whole page, or, for a box drawn
//! over play, the pixels the original's routine itself drew (the vector's
//! mask: those the same with the page filled first with 00h and with
//! FFh), against the view's layer of the box alone.
//!
//! With ARENA_SCREENS (the directory `gen_screens.py --dump` wrote: each
//! screen's NAME.page, NAME.mask and NAME.dac) a mismatch also writes
//! NAME-diff.png there: the original's, the view's and the pixels that
//! differ, side by side.

use super::*;
use arena::mem::DS;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/screens.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for c in b {
        h = (h ^ c as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// The vectors: per screen, the page's digest, the palette's, the mask's
/// (None: the whole page).
fn vectors() -> std::collections::HashMap<String, (u64, u64, Option<u64>)> {
    let text = std::fs::read_to_string(VECTORS).expect("tests/vectors/screens.txt");
    let mut out = std::collections::HashMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f.len() != 3 || f[0] != "screen" {
            continue;
        }
        let d: Vec<u64> = f[2].split(' ').map(|v| u64::from_str_radix(v, 16).expect("a digest")).collect();
        out.insert(f[1].to_string(), (d[0], d[1], d.get(2).copied()));
    }
    out
}

/// The user's save loaded (ARENA_SAVE, slot 1), as the window would.
fn loaded(dir: &Path, save: &Path) -> (App, Driver) {
    let mut a = app(dir, &Start { hour: None, load: Some((save.to_path_buf(), 1)), save_dir: Some(save.to_path_buf()), ..Default::default() });
    let mut d = Driver::new(&mut a);
    d.frame(&mut a);
    clear(&mut d, &mut a);
    (a, d)
}

/// The composed layer's colours (100h: nothing drawn) and palette as 6-bit
/// DAC values.
fn layer(a: &mut App) -> (Vec<u16>, Vec<u8>) {
    a.compose();
    let idx = a.screen.indices().to_vec();
    let dac = a.screen.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
    (idx, dac)
}

/// The view's pixels and palette against the vector `name`; with
/// ARENA_SCREENS, a diff image for a mismatch. Whether they match.
fn check(name: &str, idx: &[u16], dac: &[u8]) -> bool {
    let v = vectors();
    let Some(&(page, pal, mask)) = v.get(name) else { panic!("no vector for {name}") };
    let dump = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from);
    let orig_mask = dump.as_ref().and_then(|d| std::fs::read(d.join(format!("{name}.mask"))).ok());
    let view_page: Vec<u8> = idx.iter().map(|&c| c as u8).collect();
    let page_ok = match mask {
        None => idx.iter().all(|&c| c < 0x100) && fnv(view_page.iter().copied()) == page,
        Some(m) => {
            let view_mask: Vec<u8> = idx.iter().map(|&c| (c < 0x100) as u8).collect();
            let masked = view_page.iter().zip(&view_mask).map(|(&c, &k)| if k != 0 { c } else { 0 });
            fnv(view_mask.iter().copied()) == m && fnv(masked) == page
        }
    };
    let pal_ok = fnv(dac.iter().copied()) == pal;
    if let (false, Some(d)) = (page_ok && pal_ok, &dump) {
        if let (Ok(orig), Ok(odac)) = (std::fs::read(d.join(format!("{name}.page"))), std::fs::read(d.join(format!("{name}.dac")))) {
            diff_png(&d.join(format!("{name}-diff.png")), &orig, orig_mask.as_deref(), &odac, idx, dac);
        }
        std::fs::write(d.join(format!("{name}.view")), &view_page).ok();
        std::fs::write(d.join(format!("{name}.viewdac")), dac).ok();
    }
    if !pal_ok {
        eprintln!("{name}: the palette differs");
    }
    if !page_ok {
        eprintln!("{name}: the pixels differ");
    }
    page_ok && pal_ok
}

fn diff_png(to: &Path, orig: &[u8], mask: Option<&[u8]>, odac: &[u8], idx: &[u16], dac: &[u8]) {
    let (w, h) = (hud::W, hud::H);
    let rgb = |pal: &[u8], c: u8| {
        let at = c as usize * 3;
        [pal[at] << 2, pal[at + 1] << 2, pal[at + 2] << 2]
    };
    let mut out = vec![0u8; w * 3 * h * 3];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let drawn = mask.is_none_or(|m| m[i] != 0);
            let o = if drawn { rgb(odac, orig[i]) } else { [0x40, 0, 0x40] };
            let v = if idx[i] < 0x100 { rgb(dac, idx[i] as u8) } else { [0x40, 0, 0x40] };
            let same = drawn == (idx[i] < 0x100) && (!drawn || orig[i] as u16 == idx[i]);
            let d = if same { [0, 0, 0] } else { [0xFF, 0xFF, 0] };
            for (k, c) in [o, v, d].iter().enumerate() {
                let at = (y * w * 3 + k * w + x) * 3;
                out[at..at + 3].copy_from_slice(c);
            }
        }
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(to).unwrap()), (w * 3) as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&out[..w * 3 * h * 3]).unwrap();
}

/// The layer of what is drawn over play alone (the boxes, lists and
/// messages: `hud::compose_over` on an empty screen), in the game's
/// colours.
fn over(a: &mut App) -> (Vec<u16>, Vec<u8>) {
    a.compose();
    let hud = a.hud();
    let mut s = hud::Screen::new(a.assets.dac());
    hud::compose_over(&mut s, &a.font, &hud);
    let idx = s.indices().to_vec();
    let dac = s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
    (idx, dac)
}

/// A screen opened from the loaded save by `open`, its layer taken by
/// `take`, checked against the vector `name`.
fn screen(name: &str, open: impl FnOnce(&mut App, &mut Driver), take: impl FnOnce(&mut App) -> (Vec<u16>, Vec<u8>)) {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let (mut a, mut d) = loaded(&dir, &save);
    open(&mut a, &mut d);
    check_fade(name, &mut a);
    let (idx, dac) = take(&mut a);
    assert!(check(name, &idx, &dac), "{name}: differs from the original's");
}

/// The colours the screen's opening showed (`App::fade`, each different
/// from the one before) against the original's on its way to the screen
/// (tests/vectors/screenfades.txt: their count and digest; a screen not
/// there shows one at most); the fade then over.
fn check_fade(name: &str, a: &mut App) {
    check_fade_in("screenfades.txt", name, a);
}

/// `check_fade` against tests/vectors/`file`.
fn check_fade_in(file: &str, name: &str, a: &mut App) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/vectors").join(file);
    let text = std::fs::read_to_string(&path).expect("the fades' vectors");
    let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 4 && f[1] == name).map(|f| (f[2].parse::<usize>().unwrap(), u64::from_str_radix(f[3], 16).unwrap()));
    let mut seen: Vec<u64> = Vec::new();
    let mut raw: Vec<u8> = Vec::new();
    for (dac, _) in a.fade.drain(..) {
        let six: Vec<u8> = dac.iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
        let d = fnv(six.iter().copied());
        if seen.last() != Some(&d) {
            seen.push(d);
            raw.extend(six);
        }
    }
    // With ARENA_SCREENS, the view's colours beside the original's
    // (NAME.fade there).
    if let Ok(dir) = std::env::var("ARENA_SCREENS") {
        let _ = std::fs::write(PathBuf::from(dir).join(format!("{name}.viewfade")), &raw);
    }
    a.fade_old = None;
    match want {
        Some((n, digest)) => assert_eq!((seen.len(), fnv(seen.iter().flat_map(|d| d.to_le_bytes()))), (n, digest), "{name}: its fade differs from the original's"),
        None => assert!(seen.len() <= 1, "{name}: a fade the original doesn't show ({} colours)", seen.len()),
    }
}

/// F1: the character sheet (2542:00BE, 1194:0CF8), its first page.
#[test]
fn the_character_sheet_as_the_original_draws_it() {
    screen("sheet", |a, d| d.key(a, KeyCode::F1, None), layer);
}

/// The sheet's "Next Page": the equipment page (1194:0D67, 16BD).
#[test]
fn the_equipment_page_as_the_original_draws_it() {
    screen(
        "equipment",
        |a, d| {
            d.key(a, KeyCode::F1, None);
            d.click(a, 0x80, 0xB8, MouseButton::Left);
        },
        layer,
    );
}

/// S: the status box (2542:037C, 04B1).
#[test]
fn the_status_box_as_the_original_draws_it() {
    screen("status", |a, d| d.key(a, KeyCode::KeyS, Some("s")), over);
}

/// L: the logbook (2542:02A3, 27E3:0088), its first page.
#[test]
fn the_logbook_as_the_original_draws_it() {
    screen("logbook", |a, d| d.key(a, KeyCode::KeyL, Some("l")), layer);
}

/// C: the spellbook (2542:026D, 056A).
#[test]
fn the_spellbook_as_the_original_draws_it() {
    screen("spellbook", |a, d| d.key(a, KeyCode::KeyC, Some("c")), over);
}

/// U: the items to use (2542:034D).
#[test]
fn the_use_list_as_the_original_draws_it() {
    screen("use", |a, d| d.key(a, KeyCode::KeyU, Some("u")), over);
}

/// C, then its second line clicked once (Air Foot): the line marked (a
/// second click would cast it).
#[test]
fn the_spellbook_with_a_line_picked_as_the_original_draws_it() {
    screen(
        "spellpick",
        |a, d| {
            d.key(a, KeyCode::KeyC, Some("c"));
            d.click(a, 0xF0, 164, MouseButton::Left);
        },
        over,
    );
}

/// U, then its fourth line clicked once (Ring of Shielding): the line
/// marked (a second click would use it).
#[test]
fn the_use_list_with_a_line_picked_as_the_original_draws_it() {
    screen(
        "usepick",
        |a, d| {
            d.key(a, KeyCode::KeyU, Some("u"));
            d.click(a, 0xF0, 181, MouseButton::Left);
        },
        over,
    );
}

/// Esc: the options screen (2542:000A), drawn alone.
#[test]
fn the_options_screen_as_the_original_draws_it() {
    screen("options", |a, d| d.key(a, KeyCode::Escape, None), options_layer);
}

/// The options screen drawn alone, with what it asks over it.
fn options_layer(a: &mut App) -> (Vec<u16>, Vec<u8>) {
    let mut s = hud::Screen::new(a.assets.palette);
    a.options.as_ref().expect("the options screen").draw(&mut s, &a.font);
    let idx = s.indices().to_vec();
    let dac = s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
    (idx, dac)
}

/// Esc, then the options screen's button at (x, y) clicked (the table at
/// 5C2C), as gen_screens.py's script clicks it.
fn options_click(a: &mut App, d: &mut Driver, x: i32, y: i32) {
    d.key(a, KeyCode::Escape, None);
    assert!(a.options.is_some(), "the options screen");
    d.click(a, x, y, MouseButton::Left);
}

/// The options screen and the save menu over it, drawn alone: the menu's
/// LOADSAVE.IMG (143 rows) over OP.IMG (147).
fn options_menu_layer(a: &mut App) -> (Vec<u16>, Vec<u8>) {
    let mut s = hud::Screen::new(a.assets.palette);
    a.saveui_under.as_ref().expect("the options screen under it").draw(&mut s, &a.font);
    a.saveui.as_ref().expect("the save menu").draw(&mut s);
    let idx = s.indices().to_vec();
    let dac = s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
    (idx, dac)
}

/// The options screen's Load (2542:061E): the save menu's slots (25FD:
/// 0718), their names NAMES.DAT's (the game folder's: the save's has none).
#[test]
fn the_options_load_as_the_original_draws_it() {
    screen("optionsload", |a, d| options_click(a, d, 96, 132), options_menu_layer);
}

/// Its Save (0617): the same slots.
#[test]
fn the_options_save_as_the_original_draws_it() {
    screen("optionssave", |a, d| options_click(a, d, 160, 132), options_menu_layer);
}

/// Its Exit (05DB): "Are you sure ?" over the screen.
#[test]
fn the_options_exit_as_the_original_draws_it() {
    screen("optionsexit", |a, d| options_click(a, d, 224, 132), options_layer);
}

/// The mages' guild's spellmaker (1194:6064): Drakain before Daggerfall's
/// guild master, the menu's Spellmaker.
#[test]
fn the_spellmaker_as_the_original_draws_it() {
    screen(
        "spellmaker",
        |a, d| {
            d.click(a, 164, 74, MouseButton::Left);
            d.frame(a);
            assert!(until_asked(d, a, 3), "the guild master asks");
            d.key(a, KeyCode::Digit3, Some("3"));
            assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spellmaker)), "the spellmaker is up");
        },
        layer,
    );
}

/// The mages' guild's menu (1194:A73E): its screen, banner and menu over
/// the view, Drakain before Daggerfall's guild master.
#[test]
fn the_guilds_menu_as_the_original_draws_it() {
    screen(
        "guild",
        |a, d| {
            d.click(a, 164, 74, MouseButton::Left);
            d.frame(a);
            assert!(until_asked(d, a, 3), "the guild master asks");
        },
        layer,
    );
}

/// The guild's menu up, its entry `key` picked: the screen it asks next.
fn guild_pick(a: &mut App, d: &mut Driver, key: KeyCode, text: &str) {
    d.click(a, 164, 74, MouseButton::Left);
    d.frame(a);
    assert!(until_asked(d, a, 3), "the guild master asks");
    d.key(a, key, Some(text));
    assert!(a.pending.is_some(), "the guild asks again");
}

/// The guild's Buy: what kind of item (1194:A73E's next menu) over the
/// gold left.
#[test]
fn the_guilds_buy_as_the_original_draws_it() {
    screen("guildbuy", |a, d| guild_pick(a, d, KeyCode::Digit1, "1"), layer);
}

/// The guild's Detect Magic: the items to look at, a list.
#[test]
fn the_guilds_detect_magic_as_the_original_draws_it() {
    screen("guilddetect", |a, d| guild_pick(a, d, KeyCode::Digit2, "2"), layer);
}

/// R: camping (2542:01E6, 1194:424E), refused in the guild: its box.
#[test]
fn camping_refused_as_the_original_draws_it() {
    screen(
        "camp",
        |a, d| {
            d.key(a, KeyCode::KeyR, Some("r"));
            d.frame(a);
        },
        over,
    );
}

/// Shift+M: the world map (2542:03BF), refused inside: its box.
#[test]
fn the_world_map_refused_as_the_original_draws_it() {
    screen("travelmap", |a, d| super::press(d, a, KeyCode::KeyM, true), over);
}

/// M: the automap (2542:045A, 0000:2CFE), its pen at the mouse where the
/// original's is (A0h, 64h).
#[test]
fn the_automap_as_the_original_draws_it() {
    screen(
        "automap",
        |a, d| {
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            d.key(a, KeyCode::KeyM, Some("m"));
        },
        layer,
    );
}

/// The area left (C398 bit 80h, as its way out sets it): the outer loop
/// on to the next (`areas::next`), its assets made, as the window does.
fn area_left(a: &mut App) {
    let v = a.engine.m.mem.r16(DS, arena::movement::var::MAP_FLAGS) | 0x80;
    a.engine.m.mem.w16(DS, arena::movement::var::MAP_FLAGS, v);
    a.run = areas::next(&mut a.engine, &a.dir, a.run).expect("the next area");
    a.assets = Assets::new(&a.dir, &mut a.engine);
    a.remake_renderer();
    a.last = Some(arena::engine::Frame::Running);
}

/// The trigger's hooks for a door: the way in's blackout (1194:55B2)
/// noted, as the window's turn notes it (`Talk::blackout`).
#[derive(Default)]
struct DoorHooks {
    blackout: bool,
}

impl arena::trigger::TriggerHooks for DoorHooks {
    fn blackout(&mut self, _m: &mut arena::machine::Machine) {
        self.blackout = true;
    }
}

/// The street's first tavern's door clicked from its square's corner (as
/// `into`), the street as the window last drew it; whether the way in
/// faded the screen to black.
fn tavern_door(a: &mut App) -> bool {
    a.assets.drawn(&mut a.engine, 1.6);
    let m = &mut a.engine.m;
    let (off, seg) = (m.mem.r16(DS, 0x1AB5), m.mem.r16(DS, 0x1AB7));
    let sq = m.mem.r16(seg, off);
    let door = (sq & 0xFF00) | ((sq & 0xFF) << 1);
    m.mem.w16(DS, arena::movement::var::X, (((door & 0xFF) >> 1) << 7) + 0x40);
    m.mem.w16(DS, arena::movement::var::Y, ((door >> 8) << 7) + 0x40);
    m.mem.w16(DS, arena::controls::var::PICKED, door);
    let mut hooks = DoorHooks::default();
    m.wall_trigger(&mut a.engine.files, &mut hooks).expect("the door's trigger");
    assert!(m.mem.r16(DS, arena::movement::var::MAP_FLAGS) & 0x80 != 0, "the door leads in");
    hooks.blackout
}

/// The area left as the window leaves it (`App::area_next`,
/// `area_fade`), its colours on the way against the original's
/// (tests/vectors/areafade.txt, harness/gen_areafade.py).
fn area_fade_check(name: &str, a: &mut App, blackout: bool) {
    let was = a.fade_from();
    assert!(a.area_next(), "{name}: no next area");
    a.area_fade(was, blackout);
    check_fade_in("areafade.txt", name, a);
}

/// From the street into the town's first tavern: the screen faded to
/// black on the way in (8), then after the world loop (12: nothing more
/// seen), the tavern's colours set.
#[test]
fn entering_a_building_fades_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else { return };
    let (mut a, _d) = loaded(&dir, &save);
    area_left(&mut a);
    let blackout = tavern_door(&mut a);
    assert!(blackout, "the way in faded nothing");
    area_fade_check("tavern", &mut a, blackout);
}

/// Out of the tavern into the street: the colours faded to black in 12
/// after the world loop, the street's colours set.
#[test]
fn leaving_a_building_fades_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else { return };
    let (mut a, _d) = loaded(&dir, &save);
    area_left(&mut a);
    tavern_door(&mut a);
    area_left(&mut a);
    a.fade.clear();
    let v = a.engine.m.mem.r16(DS, arena::movement::var::MAP_FLAGS) | 0x80;
    a.engine.m.mem.w16(DS, arena::movement::var::MAP_FLAGS, v);
    area_fade_check("leavetavern", &mut a, false);
}

/// The automap (as `the_automap_as_the_original_draws_it` opens it) left
/// by its exit (0000:2E30): faded out in 12, the world drawn and faded in
/// in 12 (2EC8..2EF2).
#[test]
fn leaving_the_automap_fades_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else { return };
    let (mut a, mut d) = loaded(&dir, &save);
    a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
    d.key(&mut a, KeyCode::KeyM, Some("m"));
    a.fade.clear();
    a.fade_old = None;
    assert!(a.map, "the automap not up");
    d.click(&mut a, 0x100, 0xB0, MouseButton::Left);
    assert!(!a.map, "the automap not left");
    check_fade_in("areafade.txt", "automapclose", &mut a);
}

/// In the building first in the town's list at `list` (its far pointer:
/// 1AB5 the taverns, 1004 the temples, 1008 the stores), as
/// gen_screens.py's `place` has the original: out of the guild into the
/// street, the building's door clicked from its square's corner (0000:43B1,
/// PICKED the square; an entry's square is row << 8 | column, 1194:40CF),
/// then in.
fn into(a: &mut App, list: u16) {
    area_left(a);
    let m = &mut a.engine.m;
    let (off, seg) = (m.mem.r16(DS, list), m.mem.r16(DS, list + 2));
    let sq = m.mem.r16(seg, off);
    let door = (sq & 0xFF00) | ((sq & 0xFF) << 1);
    m.mem.w16(DS, arena::movement::var::X, (((door & 0xFF) >> 1) << 7) + 0x40);
    m.mem.w16(DS, arena::movement::var::Y, ((door >> 8) << 7) + 0x40);
    m.mem.w16(DS, arena::controls::var::PICKED, door);
    m.wall_trigger(&mut a.engine.files, &mut arena::trigger::NoTriggerHooks).expect("the door's trigger");
    assert!(m.mem.r16(DS, arena::movement::var::MAP_FLAGS) & 0x80 != 0, "the door leads in");
    area_left(a);
}

/// The tavern's services (1194:B93C, the table at 869F's kind 4) in the
/// town's first tavern, as the keeper's click runs them: its screen,
/// banner and menu.
#[test]
fn the_taverns_menu_as_the_original_draws_it() {
    screen(
        "tavern",
        |a, _| {
            into(a, 0x1AB5);
            super::shop_turn(a, 4);
        },
        layer,
    );
}

/// The temple's (C258, kind 5) in the town's first temple.
#[test]
fn the_temples_menu_as_the_original_draws_it() {
    screen(
        "temple",
        |a, _| {
            into(a, 0x1004);
            super::shop_turn(a, 5);
        },
        layer,
    );
}

/// The equipment store's (F48E, kind 6) in the town's first store.
#[test]
fn the_stores_menu_as_the_original_draws_it() {
    screen(
        "store",
        |a, _| {
            into(a, 0x1008);
            super::shop_turn(a, 6);
        },
        layer,
    );
}

/// R in the street outside the guild (`area_left`): camping refused in a
/// town, its box over the street's colours.
#[test]
fn camping_refused_in_the_street_as_the_original_draws_it() {
    screen(
        "streetcamp",
        |a, d| {
            area_left(a);
            d.key(a, KeyCode::KeyR, Some("r"));
            d.frame(a);
        },
        over,
    );
}

/// Shift+M in the street: the travel map (2542:03BF, 26F7).
#[test]
fn the_world_map_as_the_original_draws_it() {
    screen(
        "worldmap",
        |a, d| {
            area_left(a);
            // The street drawn, as the window draws it before a key (its
            // sky's colours in the DAC the fade starts from).
            a.assets.drawn(&mut a.engine, 1.6);
            super::press(d, a, KeyCode::KeyM, true);
        },
        layer,
    );
}

/// In the building first in the town's list at `list`, its services
/// (`kind`, 869F's) asked, and the menu's entry at row `y` clicked (as
/// gen_screens.py's script clicks it, x A0h): what it asks next.
fn shop_pick(a: &mut App, d: &mut Driver, list: u16, kind: u8, y: i32) {
    into(a, list);
    super::shop_turn(a, kind);
    assert!(a.pending.is_some(), "the shop's menu");
    d.click(a, 0xA0, y, MouseButton::Left);
}

/// The tavern's Buy Drinks: the drinks listed over the gold left.
#[test]
fn the_taverns_drinks_as_the_original_draws_them() {
    screen("taverndrinks", |a, d| shop_pick(a, d, 0x1AB5, 4, 0x43), layer);
}

/// The tavern's Get a Room: the rooms available.
#[test]
fn the_taverns_rooms_as_the_original_draws_them() {
    screen("tavernroom", |a, d| shop_pick(a, d, 0x1AB5, 4, 0x5B), layer);
}

/// The tavern's Rumors: the kind of rumour asked.
#[test]
fn the_taverns_rumour_kinds_as_the_original_draws_them() {
    screen("tavernrumor", |a, d| shop_pick(a, d, 0x1AB5, 4, 0x8B), layer);
}

/// gen_screens.py's RUMOURS: the game's clock (05CC) and timer (0151) as
/// the rumours start, which pick what is heard (1194:E3F7): by the first
/// nothing, by the second what is heard of the quest told (four lines).
const RUMOURS: [(u32, u16); 2] = [(0x0001_2345, 0x0456), (0x0001_2385, 0x043F)];

/// The tavern's rumours, General picked, the clock and timer at `at`.
fn rumour(a: &mut App, d: &mut Driver, at: (u32, u16)) {
    into(a, 0x1AB5);
    a.engine.m.mem.w32(DS, 0x05CC, at.0);
    // The turn's ticks (`Engine::tick`) come before the shop.
    let t = a.engine.frame_ticks();
    a.engine.m.mem.w16(DS, arena::frame::var::TICKS, at.1.wrapping_sub(t));
    super::shop_turn(a, 4);
    assert!(a.pending.is_some(), "the shop's menu");
    d.click(a, 0xA0, 0x8B, MouseButton::Left);
    d.click(a, 0xA0, 0x65, MouseButton::Left);
}

/// The rumours' General picked: nothing heard, in its box.
#[test]
fn a_taverns_rumour_as_the_original_draws_it() {
    screen("tavernrumorgeneral", |a, d| rumour(a, d, RUMOURS[0]), layer);
}

/// The rumours' General picked: what is heard, in its box.
#[test]
fn a_taverns_rumour_heard_as_the_original_draws_it() {
    screen("tavernrumorheard", |a, d| rumour(a, d, RUMOURS[1]), layer);
}

/// The temple's Bless: the donation asked.
#[test]
fn the_temples_blessing_as_the_original_draws_it() {
    screen("templebless", |a, d| shop_pick(a, d, 0x1004, 5, 0x4F), layer);
}

/// The temple's Cure.
#[test]
fn the_temples_cure_as_the_original_draws_it() {
    screen("templecure", |a, d| shop_pick(a, d, 0x1004, 5, 0x67), layer);
}

/// The temple's Heal.
#[test]
fn the_temples_healing_as_the_original_draws_it() {
    screen("templeheal", |a, d| shop_pick(a, d, 0x1004, 5, 0x7F), layer);
}

/// The store's Buy: refused, the save's inventory full.
#[test]
fn the_stores_buy_as_the_original_draws_it() {
    screen("storebuy", |a, d| shop_pick(a, d, 0x1008, 6, 0x43), layer);
}

/// The store's Sell: the items to sell, over the gold left.
#[test]
fn the_stores_sell_as_the_original_draws_it() {
    screen("storesell", |a, d| shop_pick(a, d, 0x1008, 6, 0x5B), layer);
}

/// The store's Repair: the items to repair.
#[test]
fn the_stores_repair_as_the_original_draws_it() {
    screen("storerepair", |a, d| shop_pick(a, d, 0x1008, 6, 0x73), layer);
}

/// The tavern's Get a Room, its first room (Single) picked: the days
/// asked, the field under the dialog's lines.
#[test]
fn the_taverns_days_asked_as_the_original_draws_them() {
    screen(
        "tavernroomdays",
        |a, d| {
            shop_pick(a, d, 0x1AB5, 4, 0x5B);
            d.click(a, 0x80, 0x31, MouseButton::Left);
        },
        layer,
    );
}

/// The tavern's Get a Room, the Single, a day typed (1, Enter): the
/// room's price haggled over (1194:C14E), the seed as gen_screens.py's.
fn room_priced(a: &mut App, d: &mut Driver) {
    into(a, 0x1AB5);
    a.engine.m.mem.w32(DS, arena::random::SEED, SEED);
    a.engine.m.rng.seed = SEED;
    super::shop_turn(a, 4);
    assert!(a.pending.is_some(), "the shop's menu");
    d.click(a, 0xA0, 0x5B, MouseButton::Left);
    d.click(a, 0x80, 0x31, MouseButton::Left);
    d.key(a, KeyCode::Digit1, Some("1"));
    d.key(a, KeyCode::Enter, None);
}

/// The room's price: its line (by the seed) over the haggle's buttons.
#[test]
fn the_taverns_room_price_as_the_original_draws_it() {
    screen("tavernroomprice", room_priced, layer);
}

/// The price accepted (the haggle's Yes, 1194:6001's first button) and
/// the deal confirmed (Yes again): the room taken, the tavern's menu again
/// without its rooms.
fn room_taken(a: &mut App, d: &mut Driver) {
    room_priced(a, d);
    d.click(a, 0x70, 0x85, MouseButton::Left);
    d.click(a, 0x70, 0x85, MouseButton::Left);
}

#[test]
fn the_taverns_menu_with_the_room_taken_as_the_original_draws_it() {
    screen("tavernroomtaken", room_taken, layer);
}

/// The room taken and the tavern's menu left by its Exit (third box), then
/// R: camping (2542:01E6, 1194:424E), its options.
fn rest_asked(a: &mut App, d: &mut Driver) {
    room_taken(a, d);
    d.click(a, 0xA0, 0x87, MouseButton::Left);
    assert!(a.pending.is_none(), "the tavern left");
    d.key(a, KeyCode::KeyR, Some("r"));
    d.frame(a);
}

#[test]
fn camping_in_the_rented_room_as_the_original_draws_it() {
    screen("tavernrest", rest_asked, over);
}

/// Its Camp for a while: the hours to rest asked, the typed field.
#[test]
fn the_hours_to_rest_asked_as_the_original_draws_them() {
    screen(
        "tavernresthours",
        |a, d| {
            rest_asked(a, d);
            d.click(a, 0xA0, 0x64, MouseButton::Left);
        },
        over_typed,
    );
}

/// The player rested as 1194:4345 tests it (gen_screens.py's `healed`):
/// health and spell points at their maximums, the fatigue well within its
/// limit.
fn healed(a: &mut App) {
    let m = &mut a.engine.m;
    let (hp, sp) = (m.mem.r16(DS, 0x01FF), m.mem.r16(DS, 0x020C));
    m.mem.w16(DS, 0x01FD, hp);
    m.mem.w16(DS, 0x020A, sp);
    m.mem.w16(DS, 0x0201, 0x7FFF);
}

/// Rest until healed (the second entry) with the player rested: "You
/// don't need to rest." in a box centred in the rest's clip (A91A 92h).
#[test]
fn no_need_to_rest_as_the_original_draws_it() {
    screen(
        "tavernrestneed",
        |a, d| {
            room_taken(a, d);
            d.click(a, 0xA0, 0x87, MouseButton::Left);
            assert!(a.pending.is_none(), "the tavern left");
            healed(a);
            d.key(a, KeyCode::KeyR, Some("r"));
            d.frame(a);
            d.click(a, 0xA0, 0x7C, MouseButton::Left);
        },
        over,
    );
}

/// `over` with the typed field asked drawn over it (`App::compose`'s
/// last step): a box's prompt and its field.
fn over_typed(a: &mut App) -> (Vec<u16>, Vec<u8>) {
    a.compose();
    let hud = a.hud();
    let mut s = hud::Screen::new(a.assets.dac());
    hud::compose_over(&mut s, &a.font, &hud);
    // The cursor XORed over what play shows (nothing of the layer's
    // there): what is under it decides its colours, which the original's
    // page filled with 00h and then FFh leaves out of what it drew.
    let mut under = None;
    if let Some(l) = &a.live {
        l.draw_text(&mut s);
        under = Some(s.indices().to_vec());
        l.draw(&mut s, true);
    }
    let mut idx = s.indices().to_vec();
    if let Some(under) = under {
        for (c, u) in idx.iter_mut().zip(under) {
            if u == 0x100 {
                *c = 0x100;
            }
        }
    }
    let dac = s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
    (idx, dac)
}

/// In the street (`area_left`, the guild left from SEED as gen_screens.py's
/// `place` leaves it for these: the arrival's townspeople placed alike),
/// the first townsperson (actor slot 0AB3) clicked from their own square:
/// the player there (the real position, BA62, which each frame starts
/// from, and the eye's), CLICKED their slot, as `beside` has the
/// original; then a turn, whose frame's click (1194:DB6C) talks.
fn talked_to(a: &mut App, d: &mut Driver) {
    a.engine.m.mem.w32(DS, arena::random::SEED, SEED);
    a.engine.m.rng.seed = SEED;
    area_left(a);
    let m = &mut a.engine.m;
    let (x, y) = (m.mem.r16(DS, 0x0AB3), m.mem.r16(DS, 0x0AB7));
    for (at, v) in [(arena::movement::var::X, x), (arena::movement::var::Y, y), (0xBA62, x), (0xBA66, y)] {
        m.mem.w16(DS, at, v);
    }
    m.mem.w16(DS, arena::talk::var::CLICKED, 0x0AB3);
    // The turn's tick (`Engine::tick`) moves the timer to gen_screens.py's
    // TALK_TICKS as the talk starts (what is said of oneself is rolled from
    // it, 1194:E273).
    d.frame(a);
    assert!(a.pending.is_some(), "the townsperson's menu");
}

/// A townsperson clicked: what to ask about.
#[test]
fn a_townspersons_talk_as_the_original_draws_it() {
    screen("talk", talked_to, over);
}

/// Its entries clicked (boxes 18h high from 41h): who they are, the
/// places asked after, the rumours.
#[test]
fn a_townspersons_name_as_the_original_draws_it() {
    screen("talkwho", |a, d| { talked_to(a, d); d.click(a, 0xA0, 0x4D, MouseButton::Left); }, over);
}

#[test]
fn a_townspersons_where_is_as_the_original_draws_it() {
    screen("talkwhere", |a, d| { talked_to(a, d); d.click(a, 0xA0, 0x65, MouseButton::Left); }, over);
}

#[test]
fn a_townspersons_rumours_as_the_original_draws_them() {
    screen("talkrumor", |a, d| { talked_to(a, d); d.click(a, 0xA0, 0x7D, MouseButton::Left); }, over);
}

/// The store's Sell, its first item (equipped) picked: whether to sell
/// it still (1194:F6BC).
#[test]
fn the_stores_equipped_sale_as_the_original_draws_it() {
    screen(
        "storesellequipped",
        |a, d| {
            shop_pick(a, d, 0x1008, 6, 0x5B);
            d.click(a, 0x80, 0x25, MouseButton::Left);
        },
        layer,
    );
}

/// gen_screens.py's SEED: the game's random numbers started there as the
/// shop's routine starts, where what is drawn depends on them.
const SEED: u32 = 0x2F6E_1A35;

/// Its Yes: the haggle over the item (1194:09FC, 8020 in mode 4), its
/// line (one of three, by the seed) in the strip, NEGOTBUT.IMG's buttons.
#[test]
fn the_stores_haggle_as_the_original_draws_it() {
    screen(
        "storesellhaggle",
        |a, d| {
            into(a, 0x1008);
            a.engine.m.mem.w32(DS, arena::random::SEED, SEED);
            a.engine.m.rng.seed = SEED;
            super::shop_turn(a, 6);
            assert!(a.pending.is_some(), "the shop's menu");
            d.click(a, 0xA0, 0x5B, MouseButton::Left);
            d.click(a, 0x80, 0x25, MouseButton::Left);
            d.click(a, 0xA0, 0x69, MouseButton::Left);
        },
        layer,
    );
}

/// gen_screens.py's LOOT_SEED: the piles at the square made from it.
const LOOT_SEED: u32 = 1;

/// The loot window (1194:7B16, 2433:0583) as a click on a flat of treasure
/// at the player's square in the guild (071Ah) runs it, AH 0Fh, AL 1 (not
/// the staff's flat), the piles made there first from LOOT_SEED: a turn
/// with the window asked before its frame, as `shop_turn` asks a shop.
fn looted(a: &mut App, _d: &mut Driver) {
    a.engine.m.mem.w32(DS, arena::random::SEED, LOOT_SEED);
    a.engine.m.rng.seed = LOOT_SEED;
    let before = copy(&a.engine);
    let input = TurnIn {
        intent: Default::default(),
        clicked: false,
        camp: false,
        shop: None,
        loot: Some([0x0F01, 0, DS, 0x071A]),
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

/// The piles made at the square and listed: a bag of gold and a staff.
#[test]
fn the_loot_window_as_the_original_draws_it() {
    screen("loot", looted, over);
}

/// Its first line (the gold) clicked: taken, the staff listed again.
#[test]
fn the_loot_window_after_a_take_as_the_original_draws_it() {
    screen(
        "loottake",
        |a, d| {
            looted(a, d);
            d.click(a, 0x60, 0x27, MouseButton::Left);
        },
        over,
    );
}

/// "Who are you?" with the timer (0151) at 456h as the talk runs: the
/// greeting's text (64h) takes the template with %en, which outside an
/// equipment store returns into 2DA2 at the text's offset (2DA2:05DC). The
/// original at these timers (every third: 450h, 453h, 456h, ...) runs on
/// into a box 1320h pixels wide (0000:9727 tiling it, past 20 million
/// steps without asking for input: the harness's probe); the port gives
/// up there as the original's fault, and the view shows nothing.
#[test]
fn who_are_you_at_a_timer_of_456h_gives_up_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let (mut a, mut d) = loaded(&dir, &save);
    a.engine.m.mem.w32(DS, arena::random::SEED, SEED);
    a.engine.m.rng.seed = SEED;
    area_left(&mut a);
    let ft = a.engine.frame_ticks();
    let m = &mut a.engine.m;
    let (x, y) = (m.mem.r16(DS, 0x0AB3), m.mem.r16(DS, 0x0AB7));
    for (at, v) in [(arena::movement::var::X, x), (arena::movement::var::Y, y), (0xBA62, x), (0xBA66, y)] {
        m.mem.w16(DS, at, v);
    }
    m.mem.w16(DS, arena::talk::var::CLICKED, 0x0AB3);
    // The turn's tick comes before the talk: the timer 456h as it runs
    // (and again as the answer's turn runs again from before it).
    m.mem.w16(DS, arena::frame::var::TICKS, 0x456u16.wrapping_sub(ft));
    d.frame(&mut a);
    assert!(a.pending.is_some(), "the townsperson's menu");
    d.click(&mut a, 0xA0, 0x4D, MouseButton::Left);
    assert!(a.pending.is_none() && a.boxes.is_empty(), "nothing said");
    let why = a.engine.m.gave_up.expect("the port gave up");
    assert!(why.starts_with("2DA2:05DC"), "gave up at %en: {why}");
}

/// U, its fourth line (Ring of Shielding) clicked twice: marked, then
/// used, the list gone; what it said on the message line (061A) as the
/// next frame draws it (0000:0511).
#[test]
fn the_message_line_after_a_use_as_the_original_draws_it() {
    screen(
        "usemessage",
        |a, d| {
            d.key(a, KeyCode::KeyU, Some("u"));
            d.click(a, 0xF0, 181, MouseButton::Left);
            d.click(a, 0xF0, 181, MouseButton::Left);
            assert!(a.engine.m.mem.r16(DS, 0x061E) != 0, "a message to show");
            d.frame(a);
        },
        over,
    );
}

/// gen_screens.py's `leveled`: the player's XP (05AD) the least past the
/// next level's threshold (2686:0333's: from the class's first, DATA:47AE,
/// each times 30/16 up to the ninth level's, 24/16 on) and the level's
/// check asked for (05CA bit 100h); then frames, each from SEED, until the
/// minute tick runs 2686:02D8 (its notice in a box).
fn leveled(a: &mut App, d: &mut Driver) {
    use arena::mem::DATA;
    let m = &mut a.engine.m;
    let class = (m.mem.r8(DS, 0x01A9) & 0x1F) as u16;
    let old = m.mem.r8(DS, 0x01AA) as u32;
    let mut t = m.mem.r16(DATA, 0x47AE + class * 2) as u32;
    for lv in 0..old {
        t = t.wrapping_mul(if lv < 8 { 30 } else { 24 }) >> 4;
    }
    m.mem.w32(DS, 0x05AD, t);
    let f = m.mem.r16(DS, 0x05CA) | 0x100;
    m.mem.w16(DS, 0x05CA, f);
    for _ in 0..600 {
        a.engine.m.mem.w32(DS, arena::random::SEED, SEED);
        a.engine.m.rng.seed = SEED;
        d.frame(a);
        if !a.boxes.is_empty() {
            return;
        }
    }
    panic!("no level gained");
}

/// A level gained (2686:02D8 from the minute tick): its notice (1194:5E33,
/// 4F25 in a box).
#[test]
fn a_levels_notice_as_the_original_draws_it() {
    screen("levelup", leveled, over);
}

/// The notice read: the attribute screen with the level's points to spend
/// (1194:1034 on, 14C5).
#[test]
fn the_level_ups_attributes_as_the_original_draws_them() {
    screen(
        "levelupstats",
        |a, d| {
            leveled(a, d);
            d.click(a, 0xA0, 0x64, MouseButton::Left);
            assert!(a.creation.is_some(), "the attribute screen");
        },
        layer,
    );
}

/// The game over (5B1 bit 100h) from the loaded save: the farewell's
/// vision (`Creation::ending`) frame by frame against the original's
/// (harness/gen_vision.py, tests/vectors/vision.txt: from a cleared
/// screen, the fade to black in 8 steps, the screen cleared, the movie's
/// frames with its text), for each staff stage the generator ran.
#[test]
fn the_vision_at_the_end_as_the_original_shows_it() {
    let Some(dir) = game_dir() else { return };
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return;
    };
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/vision.txt")).expect("tests/vectors/vision.txt");
    let mut runs: Vec<(u8, Vec<(u64, u64)>)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        match f.as_slice() {
            ["vision", "script", f77, _] => runs.push((f77.parse().unwrap(), Vec::new())),
            ["vision", _, _, s, p] => runs.last_mut().unwrap().1.push((u64::from_str_radix(s, 16).unwrap(), u64::from_str_radix(p, 16).unwrap())),
            _ => {}
        }
    }
    assert!(!runs.is_empty());
    for (f77, want) in runs {
        let (mut a, _d) = loaded(&dir, &save);
        a.engine.m.mem.w8(DS, 0x0F77, f77);
        let mut c = create::Creation::ending(&mut a.engine, a.assets.archive.clone(), &a.dir, a.assets.palette, None);
        let mut seen: Vec<(u64, u64)> = Vec::new();
        let mut ms = 0;
        while seen.len() < want.len() && ms < 60_000 {
            let mut s = hud::Screen::new(a.assets.palette);
            c.draw(&mut s, &a.engine.m);
            let st = (fnv(s.indices().iter().map(|&i| i as u8)), fnv(s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2])));
            if seen.last() != Some(&st) {
                if let Ok(d) = std::env::var("ARENA_SCREENS") {
                    let raw: Vec<u8> = s.indices().iter().map(|&i| i as u8).chain(s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2])).collect();
                    std::fs::write(PathBuf::from(d).join(format!("vision{f77}-{}.view", seen.len())), raw).unwrap();
                }
                seen.push(st);
            }
            c.advance(1);
            ms += 1;
        }
        let first = seen.iter().zip(&want).position(|(v, o)| v != o);
        assert!(first.is_none() && seen.len() == want.len(), "F77 {f77}: the vision's states differ from the original's at {first:?} ({} seen, {} wanted)", seen.len(), want.len());
    }
}

/// gen_screens.py's `riddled`: a riddle on the player's square as its
/// lookup leaves it (0000:06BE): the text at FF00h in the level's texts'
/// bank (C8FE), shown (3D42) and waiting as a riddle (7C5E), its answers
/// after `:` (7C62), the texts after a wrong answer (the riddle again) and
/// a right one (none), the triggers' numbers, the text's time and square
/// (the player's).
fn riddled(a: &mut App) {
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
}

/// A riddle shown in the frame (0000:0550): its text, then its answer
/// asked under it (292B:05A2's typed field, empty).
#[test]
fn a_riddle_asked_as_the_original_draws_it() {
    screen(
        "riddle",
        |a, d| {
            riddled(a);
            d.frame(a);
            assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Text(_))), "the riddle's answer asked");
        },
        over_typed,
    );
}

/// The riddle's answer typed (water): the field's text under the riddle.
#[test]
fn a_riddles_answer_typed_as_the_original_draws_it() {
    screen(
        "riddletyped",
        |a, d| {
            riddled(a);
            d.frame(a);
            assert!(matches!(a.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Text(_))), "the riddle's answer asked");
            super::type_text(d, a, "water");
        },
        over_typed,
    );
}

/// In the street, Shift+M, then the world map's clicks at each (x, y):
/// as gen_screens.py's script clicks them (the mouse left at the last).
fn world_clicks(a: &mut App, d: &mut Driver, at: &[(i32, i32)]) {
    area_left(a);
    a.assets.drawn(&mut a.engine, 1.6);
    super::press(d, a, KeyCode::KeyM, true);
    for &(x, y) in at {
        d.click(a, x, y, MouseButton::Left);
    }
}

/// The world map's first province (High Rock) clicked: its map, the
/// name of the place nearest the mouse over it.
#[test]
fn a_provinces_map_as_the_original_draws_it() {
    screen("worldprovince", |a, d| world_clicks(a, d, &[(70, 45)]), layer);
}

/// A place on it clicked: the journey's length asked.
#[test]
fn a_journeys_length_as_the_original_draws_it() {
    screen("worldtrip", |a, d| world_clicks(a, d, &[(70, 45), (200, 100)]), layer);
}

/// Its Search: a name asked, after the editor's two passes before its
/// wait (0000:A0D9's blink: from C7h, what the prompt's box leaves, the
/// cursor toggled on at C8h).
#[test]
fn the_maps_search_as_the_original_draws_it() {
    screen(
        "worldsearch",
        |a, d| {
            world_clicks(a, d, &[(70, 45), (43, 175)]);
            for _ in 0..2 {
                a.travel.as_mut().expect("the travel map").advance(0);
            }
        },
        layer,
    );
}

/// The province's Exit: the world map again.
#[test]
fn the_world_map_again_as_the_original_draws_it() {
    screen("worldback", |a, d| world_clicks(a, d, &[(70, 45), (80, 180)]), layer);
}

/// The guild's Buy, then its kind clicked at row `y` (as gen_screens.py's
/// script clicks it, x A0h): the potions, the magic items, the spells.
fn guild_buy(a: &mut App, d: &mut Driver, y: i32) {
    guild_pick(a, d, KeyCode::Digit1, "1");
    d.click(a, 0xA0, y, MouseButton::Left);
}

#[test]
fn the_guilds_potions_as_the_original_draws_them() {
    screen("guildpotions", |a, d| guild_buy(a, d, 0x59), layer);
}

/// The magic items: the inventory full (the save's), a box.
#[test]
fn the_guilds_magic_items_as_the_original_draws_them() {
    screen("guildmagic", |a, d| guild_buy(a, d, 0x71), layer);
}

#[test]
fn the_guilds_spells_as_the_original_draws_them() {
    screen("guildspells", |a, d| guild_buy(a, d, 0x88), layer);
}

/// The potions' first line clicked: how many asked.
#[test]
fn a_guild_potion_picked_as_the_original_draws_it() {
    screen(
        "guildpotionpick",
        |a, d| {
            guild_buy(a, d, 0x59);
            d.click(a, 0x70, 0x26, MouseButton::Left);
        },
        layer,
    );
}

/// The spells' first line clicked: the spell's page.
#[test]
fn a_guild_spell_picked_as_the_original_draws_it() {
    screen(
        "guildspellpick",
        |a, d| {
            guild_buy(a, d, 0x88);
            d.click(a, 0x70, 0x26, MouseButton::Left);
        },
        layer,
    );
}

/// The blessing's donation typed (100, Enter): what the temple says.
#[test]
fn a_temples_blessing_given_as_the_original_draws_it() {
    screen(
        "templedonate",
        |a, d| {
            shop_pick(a, d, 0x1004, 5, 0x4F);
            super::type_text(d, a, "100");
            d.key(a, KeyCode::Enter, None);
        },
        layer,
    );
}
