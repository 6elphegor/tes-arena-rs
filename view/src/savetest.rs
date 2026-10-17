//! The files the view writes against the original's
//! (harness/gen_savefiles.py, tests/vectors/savefiles.txt): from the
//! save, into a place as worldtest's cases go (`played_in`), what play
//! wrote on the way; then the game saved there through the options
//! screen as a player saves it, and what that wrote, file by file
//! (`check_files`).

use super::*;

/// The case `name`: into `place`, `setup`, `play`; the files play wrote
/// against the original's; then the game saved (Esc's options screen,
/// its Save, the sixth slot's row, "a" and Enter: gen_screens.py's
/// optionssaved) and the files that wrote against the original's; with
/// `compare_load`, the save loaded back from the title against the
/// original's load of its own. With ARENA_SAVEFILES set to a folder of
/// gen_savefiles.py --dump's, the first bytes that differ are printed.
fn save_case(name: &str, compare_load: bool, place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) {
    // The 3D view drawn by the engine's renderer, as the window has it (the
    // rows it compiles stay in DGROUP, where the save menu reads its names).
    let page = |a: &mut App| a.engine.page = Some(vec![0; 64000]);
    let Some((mut a, _, save)) = played_in(place, |a| { page(a); setup(a) }, play) else { return };
    let base = BASE_FILES.with(|b| b.borrow().clone()).expect("the load's files");
    let before = files_now(&a, &save);
    let mut bad = check_files(&a, name, "play", &base, &before);
    let mut d = Driver::new(&mut a);
    options_click(&mut a, &mut d, 160, 132);
    d.click(&mut a, 100, 81, MouseButton::Left);
    d.key(&mut a, KeyCode::KeyA, Some("a"));
    d.key(&mut a, KeyCode::Enter, None);
    assert!(a.saveui.is_none(), "{name}: the menu ended");
    let after = files_now(&a, &save);
    bad |= check_files(&a, name, "save", &before, &after);
    assert!(!bad, "{name}: the files differ from the original's");
    // The save loaded back from the title, as gen_savefiles.py's
    // `load_line` loads the original's: DGROUP as the load leaves it.
    // Outdoors the view makes the area's entry (the hook CD42, 0000:468B:
    // the sky's colours, the dither) before the first frame, the original
    // in it, after where the harness stops: not compared there.
    if !compare_load {
        return;
    }
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/savefiles.txt")).expect("savefiles.txt");
    let prefix = format!("loadplace | {name} | ");
    let Some(want) = text.lines().find_map(|l| l.strip_prefix(prefix.as_str())) else { panic!("{name}: no loadplace line") };
    let (seed, digest) = want.split_once(" | ").expect("seed | digest");
    let dir = game_dir().expect("the game");
    let mut b = app(&dir, &Start { hour: None, load: Some((save.to_path_buf(), 5)), save_dir: Some(save.to_path_buf()), ..Default::default() });
    b.engine.m.rng.seed = u32::from_str_radix(seed, 16).unwrap();
    b.engine.m.store_rng();
    let mut d = Driver::new(&mut b);
    clear(&mut d, &mut b);
    let (ds, _) = grab(&b);
    let canon = load_canonical(&dir, &ds);
    if let Ok(dump) = std::env::var("ARENA_SAVEFILES") {
        // Beside gen_savefiles.py --dump's NAME.loadds.
        let _ = std::fs::write(Path::new(&dump).join(format!("{name}.loadds.port")), &canon);
    }
    assert_eq!(format!("{:x}", fnv(canon)), digest, "{name}: the save loaded back differs from the original's load of its own");
}

#[test]
fn a_save_in_the_guild_writes_the_originals_files() {
    save_case("saveguild", true, Place::Guild, |_| {}, |_, _, _| {});
}

#[test]
fn a_save_in_the_street_writes_the_originals_files() {
    save_case("savestreet", false, Place::Street, |_| {}, |a, d, seen| frames(a, d, seen, 3, |_| false));
}

#[test]
fn a_save_in_the_wilderness_writes_the_originals_files() {
    save_case("savewild", false, Place::Wild, |_| {}, |a, d, seen| frames(a, d, seen, 3, |_| false));
}

#[test]
fn a_save_in_a_dungeon_writes_the_originals_files() {
    let (place, setup, play) = case("playdown").unwrap();
    save_case("savedungeon", true, place, setup, play);
}

#[test]
fn a_save_in_the_crypt_writes_the_originals_files() {
    let (place, setup, play) = case("playcrypt").unwrap();
    save_case("savecrypt", true, place, setup, play);
}
