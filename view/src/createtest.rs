//! Character creation's screens against the original's own drawing of
//! them.
//!
//! harness/gen_createshot.py runs the original's creation (1194:2498)
//! under unicorn with a script of keys and clicks, and keeps the screen
//! (the VGA's memory) and the colours (the DAC) as they are whenever an
//! input is taken, as the player would see them answering;
//! tests/vectors/createshot.txt holds each script and each screen's and
//! palette's FNV-1a digest. Here the same script is given to `--create`'s
//! `Creation` from the same random seed, and before each input the layer
//! it draws must be the original's screen, pixel for pixel in the
//! palette's indices, in the same colours.
//!
//! A few screens are known not to be exact yet (`KNOWN`, with how many
//! pixels may differ and why).
//!
//! With ARENA_CREATESHOTS (a directory) each of the view's screens is
//! written there as SCRIPT-STEP.view: its 64000 indices, then the 768
//! colours (6-bit), as gen_createshot.py's DUMP_DIR has the original's
//! (SCRIPT-STEP.raw: a known difference is then held to its bound).

use super::*;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/createshot.txt");
/// The seed the generator starts creation from (DGROUP 91F6).
const SEED: u32 = 0x1234_5678;
/// The dissolve's generator (DGROUP 4B8E) at creation's start there.
const DISSOLVE_SEED: u16 = 0x1235;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for c in b {
        h = (h ^ c as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// A script: its name, its inputs and, per step (and "end"), the
/// screen's and palette's digests.
struct Script {
    name: String,
    inputs: String,
    shots: Vec<(String, u64, u64)>,
}

fn scripts() -> Vec<Script> {
    let text = std::fs::read_to_string(VECTORS).expect("tests/vectors/createshot.txt");
    let mut out: Vec<Script> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f.len() < 4 || f[0] != "createshot" {
            continue;
        }
        if f[2] == "script" {
            out.push(Script { name: f[1].to_string(), inputs: f[3].to_string(), shots: Vec::new() });
        } else if f.len() == 6 {
            let s = out.last_mut().expect("a script line first");
            let h = |v: &str| u64::from_str_radix(v, 16).expect("a digest");
            s.shots.push((f[2].to_string(), h(f[4]), h(f[5])));
        }
    }
    out
}

/// The layer creation draws, as palette indices (100h: nothing) and the
/// 6-bit colours.
fn drawn(c: &create::Creation, m: &arena::machine::Machine, palette: [[u8; 4]; 256]) -> (Vec<u16>, Vec<u8>) {
    let mut s = hud::Screen::new(palette);
    c.draw(&mut s, m);
    let dac = s.palette().iter().flat_map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]).collect();
    (s.indices().to_vec(), dac)
}

/// Creation at its first question, as the main menu's Start leaves it,
/// from the generator's seed.
fn at_first_question(dir: &Path) -> (Engine, create::Creation) {
    let mut engine = world::load(dir, "START.MIF");
    let archive = Rc::new(archive::Archive::open(dir));
    hud::init_style(&archive, &engine.m);
    let mut c = create::Creation::new(&mut engine, archive, dir, world::palette(dir));
    // The intro's movie skipped and the rest of it left (Esc at the
    // title): the main menu.
    for inp in create::parse_script("enter,esc") {
        c.script_input(&mut engine, inp);
    }
    engine.m.mem.w32(arena::mem::DS, arena::random::SEED, SEED);
    engine.m.load_rng();
    // Start, the opening story passed (Esc ends its movie, then the
    // story at its first picture).
    for inp in create::parse_script("S,esc,esc") {
        c.script_input(&mut engine, inp);
    }
    c.settle(&mut engine.m);
    // Creation from 1194:24A6 on the game as it is, as the generator runs
    // the original's: not the menu's pick and the story's state, which
    // play gives the game (`Creation::begin_at_creation`).
    c.begin_at_creation(&mut engine);
    // The dissolve's generator as the generator's start leaves it (the
    // story's first picture dissolved in has stepped it).
    engine.m.mem.w16(arena::mem::DS, 0x4B8E, DISSOLVE_SEED);
    (engine, c)
}

/// Screens known not to be exact yet: (script-step, the most pixels that
/// may differ, why). Their colours must still be the original's.
const KNOWN: &[(&str, usize, &str)] = &[
];

#[test]
fn creation_draws_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let dump = std::env::var("ARENA_CREATESHOTS").ok().map(PathBuf::from);
    let palette = world::palette(&dir);
    let mut wrong = Vec::new();
    let mut checked = 0;
    for s in scripts() {
        let (mut engine, mut c) = at_first_question(&dir);
        let inputs = create::parse_script(&s.inputs);
        for (step, page, pal) in &s.shots {
            let at = if step.as_str() == "end" { inputs.len() } else { step.parse().expect("a step") };
            let (idx, dac) = drawn(&c, &engine.m, palette);
            if let Some(d) = &dump {
                let mut raw: Vec<u8> = idx.iter().map(|&i| i as u8).collect();
                raw.extend_from_slice(&dac);
                std::fs::write(d.join(format!("{}-{step}.view", s.name)), raw).ok();
            }
            let name = format!("{}-{step}", s.name);
            let page_ok = idx.iter().all(|&i| i < 0x100) && fnv(idx.iter().map(|&i| i as u8)) == *page;
            // A known difference: within its bound, checked against the
            // original's screen where ARENA_CREATESHOTS has it (the
            // generator's DUMP_DIR), else taken as it is.
            let page_ok = page_ok
                || KNOWN.iter().find(|k| k.0 == name).is_some_and(|k| {
                    let orig = dump.as_ref().and_then(|d| std::fs::read(d.join(format!("{name}.raw"))).ok());
                    orig.is_none_or(|o| idx.iter().zip(&o).filter(|&(&v, &o)| v != o as u16).count() <= k.1)
                });
            let pal_ok = fnv(dac.iter().copied()) == *pal;
            checked += 1;
            if !(page_ok && pal_ok) {
                wrong.push(format!("{name}{}{}", if page_ok { "" } else { " pixels" }, if pal_ok { "" } else { " colours" }));
            }
            if let Some(&inp) = inputs.get(at) {
                c.script_input(&mut engine, inp);
                c.settle(&mut engine.m);
            }
        }
    }
    assert!(wrong.is_empty(), "{} of {checked} screens differ from the original's: {}", wrong.len(), wrong.join(", "));
}

const FRAMES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/createframes.txt");

/// What the original shows between two screens answered (gen_createshot's
/// FRAMES): by script and step, the count of states and their digest.
fn frames() -> std::collections::HashMap<String, (usize, u64)> {
    let text = std::fs::read_to_string(FRAMES).expect("tests/vectors/createframes.txt");
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(" | ").collect();
            (f.len() == 5 && f[0] == "createframes").then(|| (format!("{}-{}", f[1], f[2]), (f[3].parse().expect("a count"), u64::from_str_radix(f[4], 16).expect("a digest"))))
        })
        .collect()
}

/// The screens between: each input's change frame by frame (dissolves,
/// fades, colours set, pictures shown), as the generator samples the
/// original's (each state different from the one before, the first not
/// the screen answered, the last not the next). Step 0, the start's own,
/// has no screen answered before it and isn't compared.
#[test]
fn creation_animates_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let want = frames();
    let palette = world::palette(&dir);
    let mut wrong = Vec::new();
    let mut checked = 0;
    for s in scripts() {
        let (mut engine, mut c) = at_first_question(&dir);
        let inputs = create::parse_script(&s.inputs);
        let mut before: Option<(Vec<u16>, Vec<u8>)> = None;
        let mut states: Vec<(Vec<u16>, Vec<u8>)> = Vec::new();
        for (step, _, _) in &s.shots {
            let at = if step.as_str() == "end" { inputs.len() } else { step.parse().expect("a step") };
            let now = drawn(&c, &engine.m, palette);
            let name = format!("{}-{step}", s.name);
            if let (Some(&(n, digest)), Some(b)) = (want.get(&name), &before) {
                let mut seen: Vec<&(Vec<u16>, Vec<u8>)> = Vec::new();
                for st in &states {
                    if seen.last().is_none_or(|l| *l != st) {
                        seen.push(st);
                    }
                }
                if seen.first().is_some_and(|f| *f == b) {
                    seen.remove(0);
                }
                if seen.last().is_some_and(|l| **l == now) {
                    seen.pop();
                }
                let got = fnv(seen.iter().flat_map(|(idx, dac)| {
                    let a = fnv(idx.iter().map(|&i| i as u8)).to_le_bytes();
                    let b = fnv(dac.iter().copied()).to_le_bytes();
                    a.into_iter().chain(b)
                }));
                if let Ok(d) = std::env::var("ARENA_CREATESHOTS") {
                    let raw: Vec<u8> = seen.iter().flat_map(|(idx, dac)| idx.iter().map(|&i| i as u8).chain(dac.iter().copied()).collect::<Vec<u8>>()).collect();
                    std::fs::write(PathBuf::from(d).join(format!("{name}.vframes")), raw).ok();
                }
                checked += 1;
                if (seen.len(), got) != (n, digest) {
                    wrong.push(format!("{name} ({} states, want {n})", seen.len()));
                }
            }
            before = Some(now.clone());
            if let Some(&inp) = inputs.get(at) {
                c.script_input(&mut engine, inp);
                states = c.film_frames().into_iter().map(|(f, dac)| (f.into_iter().map(u16::from).collect(), dac)).collect();
                c.settle(&mut engine.m);
            }
        }
    }
    assert!(wrong.is_empty(), "{} of {checked} transitions differ from the original's: {}", wrong.len(), wrong.join(", "));
}

const INTRO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/introframes.txt");

/// A run of harness/gen_introframes.py: its keys (as `Input`s) and each
/// state the original showed (screen and colours digests; the waits for
/// the retrace and the timer's ticks it was seen at), the dissolve's
/// generator at its end.
struct IntroRun {
    name: String,
    /// Each key, the state it's pressed in when the harness pressed it as
    /// a state was first seen (`sN`; else at a poll), and how many waits
    /// for the retrace it's held.
    keys: Vec<(create::Input, Option<usize>, u32)>,
    states: Vec<(u64, u64, u32, u32)>,
    seed: u16,
}

fn intro_runs() -> Vec<IntroRun> {
    let text = std::fs::read_to_string(INTRO).expect("tests/vectors/introframes.txt");
    let mut out: Vec<IntroRun> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        match f.as_slice() {
            ["introframes", name, "script", keys] => {
                let keys = keys
                    .split(',')
                    .filter(|k| *k != "-")
                    .map(|k| {
                        let inp = if k.starts_with("1b@") { create::Input::Esc } else { create::Input::Enter };
                        let (at, held) = k[3..].split_once('+').expect("KEY@POLL+HELD");
                        (inp, at.strip_prefix('s').map(|n| n.parse().expect("a state")), held.parse().expect("held"))
                    })
                    .collect();
                out.push(IntroRun { name: name.to_string(), keys, states: Vec::new(), seed: 0 });
            }
            ["introframes", _, "end", seed] => out.last_mut().expect("a script line").seed = u16::from_str_radix(seed, 16).expect("a seed"),
            ["introframes", _, _, s, c, r, t] => {
                let h = |v: &str| u64::from_str_radix(v, 16).expect("a digest");
                out.last_mut().expect("a script line").states.push((h(s), h(c), r.parse().expect("retraces"), t.parse().expect("ticks")));
            }
            _ => {}
        }
    }
    out
}

/// The intro (0000:10C9 on to the main menu: the movies, the pictures
/// faded in and held, the scrolls dissolved in, each change through the
/// fade to black; MENU.IMG's colours set), and the opening story after
/// the menu's Start (1323, and on into creation's first screen: 1194:25B9),
/// frame by frame against the original's
/// (harness/gen_introframes.py): each state in turn, and how long it
/// shows (a wait for the retrace `RETRACE_MS`, a timer tick
/// `flc::TICK_MS`; the view's time passing a millisecond at a time).
/// The keys are taken while the original took them (in the state the
/// original's first differs from its run without keys, and the next):
/// how long those states show is the player's. The dissolve's generator
/// ends as the original's.
#[test]
fn the_intro_plays_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let palette = world::palette(&dir);
    let runs = intro_runs();
    assert!(!runs.is_empty());
    let none = |story: bool| runs.iter().find(|r| r.name == if story { "story" } else { "none" }).expect("the runs without keys");
    let mut wrong = Vec::new();
    for run in &runs {
        let story = run.name.starts_with("story");
        // The states the keys are pressed in: where this run first
        // differs from the one without keys, and (a wait's key keeps the
        // states in step) where it differs next; a movie's frame there (a
        // key seen after the next frame is read, shown) the one before.
        let plain = none(story);
        let polled = run.keys.iter().filter(|k| k.1.is_none()).count();
        let mut at = Vec::new();
        for (i, (a, b)) in run.states.iter().zip(&plain.states).enumerate() {
            if at.len() < polled && a != b {
                at.push(if b.3 > 0 { i - 1 } else { i });
                if a.0 != b.0 || a.1 != b.1 {
                    break;
                }
            }
        }
        assert_eq!(at.len(), polled, "{}: where its keys are taken", run.name);
        let mut engine = world::load(&dir, "START.MIF");
        let archive = Rc::new(archive::Archive::open(&dir));
        hud::init_style(&archive, &engine.m);
        let mut c = create::Creation::new(&mut engine, archive, &dir, palette);
        engine.m.mem.w16(arena::mem::DS, 0x4B8E, DISSOLVE_SEED);
        if story {
            // The menu (the intro passed), then Start.
            for inp in create::parse_script("enter,esc") {
                c.script_input(&mut engine, inp);
            }
            c.settle(&mut engine.m);
            c.input(&mut engine, create::Input::Char(b'S'));
            engine.m.mem.w16(arena::mem::DS, 0x4B8E, DISSOLVE_SEED);
        }
        // Each state the view shows and for how long (ms).
        let mut seen: Vec<(u64, u64, u32)> = Vec::new();
        let mut phase = None;
        let mut polls = at.iter();
        let mut keys = run.keys.iter().map(|&(inp, s, held)| (inp, s.unwrap_or_else(|| *polls.next().expect("a poll's state")), s.is_some(), held)).collect::<Vec<_>>().into_iter().peekable();
        // An Esc's let go, the milliseconds to it (a short hold's).
        let mut let_go: Option<u32> = None;
        for _ in 0..200_000 {
            if c.phase() != phase.unwrap_or_default() || phase.is_none() {
                phase = Some(c.phase());
                let (idx, dac) = drawn(&c, &engine.m, palette);
                let st = (fnv(idx.iter().map(|&i| i as u8)), fnv(dac.iter().copied()));
                if seen.last().is_none_or(|l| (l.0, l.1) != st) {
                    seen.push((st.0, st.1, 0));
                }
            }
            seen.last_mut().expect("a state").2 += 1;
            // A key in its state, half its time on (the run's own).
            let from = seen.iter().position(|s| (s.0, s.1) == (run.states[1].0, run.states[1].1));
            // (A state's key, `sN`, as the state is first shown.)
            if let (Some(&(inp, k, first, held)), Some(from)) = (keys.peek(), from) {
                let (i, want) = (seen.len() - 1 - from + 1, &run.states[k]);
                let time = if first { 1 } else { (want.2 * RETRACE + want.3 * TICK) / 2 };
                if i == k && (seen[seen.len() - 1].0, seen[seen.len() - 1].1) == (want.0, want.1) && seen[seen.len() - 1].2 >= time {
                    c.input(&mut engine, inp);
                    keys.next();
                    phase = None;
                    if inp == create::Input::Esc && held < 100 {
                        let_go = Some(held * RETRACE);
                    }
                }
            }
            if let Some(ms) = &mut let_go {
                if *ms == 0 {
                    c.input(&mut engine, create::Input::Key(0x01, false));
                    let_go = None;
                } else {
                    *ms -= 1;
                }
            }
            if from.is_some_and(|f| seen.len() - f >= run.states.len() + 1) {
                break;
            }
            c.advance(&mut engine.m, 1);
        }
        // From the movie's first frame (the screen before it is each
        // one's own).
        let Some(from) = seen.iter().position(|s| (s.0, s.1) == (run.states[1].0, run.states[1].1)) else {
            wrong.push(format!("{}: its first frame not shown", run.name));
            continue;
        };
        let view = &seen[from..];
        let want = &run.states[1..];
        if let Some(i) = want.iter().zip(view).position(|(w, v)| (w.0, w.1) != (v.0, v.1)).or((view.len() < want.len()).then_some(view.len())) {
            wrong.push(format!("{}: state {} differs ({} shown, {} wanted)", run.name, i + 1, view.len(), want.len()));
            continue;
        }
        // How long each shows, but the last (the run's end) and those the
        // keys came in.
        for (i, (w, v)) in want.iter().zip(view).enumerate().take(want.len() - 1) {
            if at.contains(&(i + 1)) {
                continue;
            }
            let ms = w.2 * RETRACE + w.3 * TICK;
            if v.2.abs_diff(ms) > 2 {
                wrong.push(format!("{}: state {} shows {} ms, the original's {ms} ({} retraces, {} ticks)", run.name, i + 1, v.2, w.2, w.3));
                break;
            }
        }
        let seed = engine.m.mem.r16(arena::mem::DS, 0x4B8E);
        if seed != run.seed {
            wrong.push(format!("{}: the dissolve's generator {seed:04x}, the original's {:04x}", run.name, run.seed));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

const RETRACE: u32 = crate::film::RETRACE_MS;
const TICK: u32 = crate::flc::TICK_MS;

/// The main menu's Load and Exit (0000:1114's pick, then 111A's 55B2:
/// the menu faded to black in 8) as the original shows them up to 117E
/// and 11B1 (harness/gen_introframes.py's `menupick` runs): each state,
/// and the waits for the retrace it shows at; the pick's outcome only
/// once they've played.
#[test]
fn the_menus_load_and_exit_fade_as_the_original() {
    let Some(dir) = game_dir() else { return };
    let text = std::fs::read_to_string(INTRO).expect("tests/vectors/introframes.txt");
    for (name, key) in [("menuload", b'L'), ("menuexit", b'E')] {
        let want: Vec<(u64, u64, u32)> = text
            .lines()
            .map(|l| l.split(" | ").collect::<Vec<_>>())
            .filter(|f| f.len() == 7 && f[0] == "menupick" && f[1] == name)
            .map(|f| (u64::from_str_radix(f[3], 16).unwrap(), u64::from_str_radix(f[4], 16).unwrap(), f[5].parse().unwrap()))
            .collect();
        assert!(!want.is_empty(), "{name}: the original's run");
        let mut engine = world::load(&dir, "START.MIF");
        let archive = Rc::new(archive::Archive::open(&dir));
        hud::init_style(&archive, &engine.m);
        let mut c = create::Creation::new(&mut engine, archive, &dir, world::palette(&dir));
        for inp in create::parse_script("enter,esc") {
            c.script_input(&mut engine, inp);
        }
        c.settle(&mut engine.m);
        c.input(&mut engine, create::Input::Char(key));
        let mut seen: Vec<(u64, u64, u32)> = Vec::new();
        for (px, dac) in c.film_frames() {
            let st = (fnv(px.iter().copied()), fnv(dac.iter().copied()));
            match seen.last_mut() {
                Some(l) if (l.0, l.1) == st => l.2 += 1,
                _ => seen.push((st.0, st.1, 1)),
            }
        }
        assert_eq!(seen, want, "{name}: the fade differs from the original's");
        assert!(c.outcome().is_none(), "{name}: the menu's pick waits for its fade");
        c.advance(&mut engine.m, RETRACE * want.iter().map(|w| w.2).sum::<u32>());
        assert!(c.outcome().is_some(), "{name}: the menu's pick once its fade has played");
    }
}