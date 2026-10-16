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
    let mut a = app(dir, &Start { hour: None, load: Some((save.to_path_buf(), 1)), ..Default::default() });
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
    let (idx, dac) = take(&mut a);
    assert!(check(name, &idx, &dac), "{name}: differs from the original's");
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

/// Esc: the options screen (2542:000A), drawn alone.
#[test]
fn the_options_screen_as_the_original_draws_it() {
    screen(
        "options",
        |a, d| d.key(a, KeyCode::Escape, None),
        |a| {
            let mut s = hud::Screen::new(a.assets.palette);
            a.options.as_ref().expect("the options screen").draw(&mut s, &a.font);
            let idx = s.indices().to_vec();
            let dac = s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
            (idx, dac)
        },
    );
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
