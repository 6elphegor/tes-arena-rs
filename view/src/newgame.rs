//! A new game from the command line (`--new-game SPEC`): the character
//! made as the original's creation makes it (src/newgame.rs) from the
//! choices in SPEC, then the game started in the start dungeon.
//!
//! SPEC is `CLASS:PROVINCE:SEX:FACE:NAME`, each part optional from the
//! right: CLASS a number 0..17 picks from the class list (in its order:
//! the table at 2AEE), or `q` and ten digits 0..2 answers the questions;
//! PROVINCE 0..7 (the race), SEX 0 or 1, FACE 0..9, NAME up to 18
//! characters. The attributes are rolled once and the bonus points spent
//! one at a time over the attributes in turn, as clicks on the arrows
//! would spend them.

use arena::engine::Engine;
use arena::newgame::{Character, ClassPick, NoNewGameHooks};

/// The choices in `spec`.
pub fn parse(spec: &str) -> Character {
    let mut parts = spec.split(':');
    let class = match parts.next().filter(|s| !s.is_empty()) {
        Some(q) if q.starts_with('q') => {
            let mut a = [0u8; 10];
            for (d, c) in a.iter_mut().zip(q[1..].bytes()) {
                *d = c.saturating_sub(b'0').min(2);
            }
            ClassPick::Quiz(a)
        }
        Some(n) => ClassPick::Listed(n.parse::<u8>().expect("--new-game: a class 0..17") % 18),
        None => ClassPick::Listed(0),
    };
    let num = |s: Option<&str>, max: u8| s.and_then(|s| s.parse::<u8>().ok()).unwrap_or(0).min(max);
    let province = num(parts.next(), 7);
    let sex = num(parts.next(), 1);
    let face = num(parts.next(), 9);
    let name = parts.next().filter(|s| !s.is_empty()).unwrap_or("Talin").as_bytes().to_vec();
    Character { class, name, sex, province, face, rerolls: 0, clicks: Vec::new() }
}

/// The character made and the game started: the start dungeon's state as
/// its first run sets it (0000:0C66: the level hooks, its name; 0997). The
/// view has already loaded START.MIF.
pub fn start(engine: &mut Engine, spec: &str) {
    let ch = parse(spec);
    let m = &mut engine.m;
    let Some(class) = m.new_game(&mut engine.files, &mut NoNewGameHooks, &ch) else {
        eprintln!("the character could not be made");
        return;
    };
    if m.game_date().is_none() || m.weather_roll().is_none() {
        eprintln!("the game's date or weather could not be set");
    }
    m.game_start();
    m.start_run(&mut arena::transit::NoAreaHooks);
    let name = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, 0x01AD)).into_owned();
    eprintln!("new game: {name}, class {class:#x}, province {}", ch.province);
}
