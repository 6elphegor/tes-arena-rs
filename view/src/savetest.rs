//! The files the view writes against the original's
//! (harness/gen_savefiles.py, tests/vectors/savefiles.txt): from the
//! save, into a place as worldtest's cases go (`played_in`), what play
//! wrote on the way; then the game saved there through the options
//! screen as a player saves it, and what that wrote, file by file.
//!
//! The files are as the game finds them: the save folder's (where the
//! view writes a save), and the ones written in play over them (the
//! working slot's, kept in memory: `Files`).

use super::*;
use std::collections::BTreeMap;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/savefiles.txt");

/// Files by upper-case name.
pub(super) type Disk = BTreeMap<String, Vec<u8>>;

/// The files the game finds now: the save folder's (`dir`) that the game
/// hasn't deleted, and the ones written in play over them.
pub(super) fn files_now(a: &App, dir: &Path) -> Disk {
    let mut out = Disk::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_ascii_uppercase();
        if e.file_type().is_ok_and(|t| t.is_file()) && !a.engine.files.was_deleted(name.as_bytes()) {
            out.insert(name, std::fs::read(e.path()).unwrap_or_default());
        }
    }
    for n in a.engine.files.written_names() {
        let data = a.engine.files.written(&n).unwrap_or_default().to_vec();
        out.insert(String::from_utf8_lossy(&n).into_owned(), data);
    }
    out
}

/// The files of `after` made or changed since `before`, as the vectors
/// hold them: SAVEGAME's ceiling plane (after the screen, the palette,
/// the level's block and two planes), which the original takes from
/// 0000:0000 (the interrupt table on) where the level has none (D748 0),
/// left out as 0s, as gen_savefiles.py leaves the original's.
fn changed(a: &App, before: &Disk, after: &Disk) -> Disk {
    const AT: usize = 0xFA00 + 0x300 + 0x0DE7 + 0x10000;
    let no_ceiling = a.engine.m.mem.r16(DS, 0xD748) == 0;
    let mut out: Disk = after.iter().filter(|(n, d)| before.get(*n) != Some(*d)).map(|(n, d)| (n.clone(), d.clone())).collect();
    for (n, d) in out.iter_mut() {
        if no_ceiling && n.starts_with("SAVEGAME.") && d.len() > AT {
            let end = d.len().min(AT + 0x8000);
            d[AT..end].fill(0);
        }
    }
    out
}

/// What changed from `before` to `after`, as the vectors' lines: each
/// file made or changed (`made`: its name, length and digest), then the
/// files gone.
fn changes(kind: &str, name: &str, made: &Disk, before: &Disk, after: &Disk) -> Vec<String> {
    let mut out: Vec<String> = made.iter().map(|(n, d)| format!("{kind} | {name} | {n} {:x} {:x}", d.len(), fnv(d.iter().copied()))).collect();
    let gone: Vec<&str> = before.keys().filter(|n| !after.contains_key(*n)).map(|n| n.as_str()).collect();
    out.push(format!("{kind}del | {name} | {}", gone.join(" ")).trim_end().to_string());
    out
}

/// The vectors' lines of `kind` (play or save) for the case.
fn wanted(kind: &str, name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let del = format!("{kind}del");
    text.lines()
        .filter(|l| {
            let f: Vec<&str> = l.split(" | ").collect();
            f.len() >= 2 && (f[0] == kind || f[0] == del) && f[1] == name
        })
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// The case `name`: into `place`, `setup`, `play`; the files play wrote
/// against the original's; then the game saved (Esc's options screen,
/// its Save, the sixth slot's row, "a" and Enter: gen_screens.py's
/// optionssaved) and the files that wrote against the original's. With
/// ARENA_SAVEFILES set to a folder of gen_savefiles.py --dump's, the
/// first bytes that differ are printed.
fn save_case(name: &str, place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) {
    let Some((mut a, _, save)) = played_in(place, setup, play) else { return };
    let base = BASE_FILES.with(|b| b.borrow().clone());
    let before = files_now(&a, &save);
    let mut bad = check(&a, name, "play", &base, &before);
    let mut d = Driver::new(&mut a);
    options_click(&mut a, &mut d, 160, 132);
    d.click(&mut a, 100, 81, MouseButton::Left);
    d.key(&mut a, KeyCode::KeyA, Some("a"));
    d.key(&mut a, KeyCode::Enter, None);
    assert!(a.saveui.is_none(), "{name}: the menu ended");
    let after = files_now(&a, &save);
    bad |= check(&a, name, "save", &before, &after);
    assert!(!bad, "{name}: the files differ from the original's");
}

/// The lines against the vectors': whether they differ (printing how).
fn check(a: &App, name: &str, kind: &str, before: &Disk, after: &Disk) -> bool {
    let made = changed(a, before, after);
    let got = changes(kind, name, &made, before, after);
    let want = wanted(kind, name);
    if got == want {
        return false;
    }
    eprintln!("{name}: {kind}: got\n  {}\nwanted\n  {}", got.join("\n  "), want.join("\n  "));
    if let Ok(dump) = std::env::var("ARENA_SAVEFILES") {
        for (n, d) in &made {
            let mine = Path::new(&dump).join(format!("{name}.{kind}.port"));
            let _ = std::fs::create_dir_all(&mine);
            let _ = std::fs::write(mine.join(n), d);
            let Ok(o) = std::fs::read(Path::new(&dump).join(format!("{name}.{kind}")).join(n)) else { continue };
            let first = d.iter().zip(&o).position(|(x, y)| x != y);
            let runs = super::differing(d, &o, 16);
            eprintln!("  {n}: {} vs {} bytes, first differing at {first:x?}, runs {:x?}", d.len(), o.len(), &runs[..runs.len().min(12)]);
        }
    }
    true
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
