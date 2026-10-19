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
/// optionssaved) and the files that wrote against the original's; the
/// save loaded back from the title against the original's load of its
/// own, and outdoors its first frames. With ARENA_SAVEFILES set to a
/// folder of gen_savefiles.py --dump's, the states are written beside its
/// dumps.
fn save_case(name: &str, place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) {
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
    // `load_line` loads the original's: DGROUP as the load leaves it, and
    // where there are `loadframe` lines the world frames run on after it.
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
    let dump = std::env::var("ARENA_SAVEFILES").ok().map(PathBuf::from);
    if let Some(dump) = &dump {
        // Beside gen_savefiles.py --dump's NAME.loadds.
        let _ = std::fs::write(dump.join(format!("{name}.loadds.port")), &canon);
    }
    assert_eq!(format!("{:x}", fnv(canon)), digest, "{name}: the save loaded back differs from the original's load of its own");
    // The frames after the load (gen_savefiles.py's `load_frames`): DGROUP
    // in the original's form, stateexclude.txt's offsets left out as well,
    // and the EMS banks, at the top of each.
    let frames: Vec<(usize, &str)> = text
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(" | ").collect();
            (f.len() == 4 && f[0] == "loadframe" && f[1] == name).then(|| (f[2].parse().unwrap(), f[3]))
        })
        .collect();
    let skip = state_excluded();
    page(&mut b);
    let mut bad = Vec::new();
    for (k, want) in frames {
        d.frame(&mut b);
        let (ds, banks) = grab(&b);
        let mut canon = load_canonical(&dir, &ds);
        canon.iter_mut().zip(&skip).filter(|(_, s)| **s).for_each(|(x, _)| *x = 0);
        if let Some(dump) = &dump {
            // Raw, beside gen_savefiles.py --dump's NAME.loadds<k>.
            let _ = std::fs::write(dump.join(format!("{name}.loadds{k}.port")), &ds);
            let _ = std::fs::write(dump.join(format!("{name}.loadems{k}.port")), &banks);
        }
        if format!("{:x} {:x}", fnv(canon), fnv(banks)) != want {
            bad.push(k);
        }
    }
    assert!(bad.is_empty(), "{name}: the frames after the save loaded back differ from the original's: {bad:?}");
}

#[test]
fn a_save_in_the_guild_writes_the_originals_files() {
    save_case("saveguild", Place::Guild, |_| {}, |_, _, _| {});
}

#[test]
fn a_save_in_the_street_writes_the_originals_files() {
    save_case("savestreet", Place::Street, |_| {}, |a, d, seen| frames(a, d, seen, 3, |_| false));
}

#[test]
fn a_save_in_the_wilderness_writes_the_originals_files() {
    save_case("savewild", Place::Wild, |_| {}, |a, d, seen| frames(a, d, seen, 3, |_| false));
}

#[test]
fn a_save_in_a_dungeon_writes_the_originals_files() {
    let (place, setup, play) = case("playdown").unwrap();
    save_case("savedungeon", place, setup, play);
}

#[test]
fn a_save_in_the_crypt_writes_the_originals_files() {
    let (place, setup, play) = case("playcrypt").unwrap();
    save_case("savecrypt", place, setup, play);
}

#[test]
fn a_save_at_night_writes_the_originals_files() {
    let (place, setup, play) = case("playnight").unwrap();
    save_case("savenight", place, setup, play);
}

#[test]
fn a_save_with_a_disease_past_midnight_writes_the_originals_files() {
    let (place, setup, play) = case("playmidnight").unwrap();
    save_case("savemidnight", place, setup, play);
}

#[test]
fn a_save_with_light_cast_writes_the_originals_files() {
    let (place, setup, play) = case("playlight").unwrap();
    save_case("savelight", place, setup, play);
}

/// In the palace (an interior: taverns, temples and stores refuse a
/// save, 25FD:0002), its quest taken.
#[test]
fn a_save_with_a_quest_taken_writes_the_originals_files() {
    let (place, setup, play) = case("playpalaceaccept").unwrap();
    save_case("savequest", place, setup, play);
}

#[test]
fn a_save_in_a_wilderness_dungeon_writes_the_originals_files() {
    let (place, setup, play) = super::areaplaytest::transit_case("wilddungeon").unwrap();
    save_case("savewilddungeon", place, setup, play);
}
