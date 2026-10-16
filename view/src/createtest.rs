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
    // The intro and the title passed: the main menu.
    for inp in create::parse_script("enter,enter") {
        c.input(&mut engine, inp);
    }
    engine.m.mem.w32(arena::mem::DS, arena::random::SEED, SEED);
    engine.m.load_rng();
    // The dissolve's generator as the generator's start leaves it.
    engine.m.mem.w16(arena::mem::DS, 0x4B8E, DISSOLVE_SEED);
    // Start, the opening story passed.
    for inp in create::parse_script("S,esc") {
        c.input(&mut engine, inp);
    }
    c.settle();
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
                c.input(&mut engine, inp);
                c.settle();
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
                c.input(&mut engine, inp);
                states = c.film_frames().into_iter().map(|(f, dac)| (f.into_iter().map(u16::from).collect(), dac)).collect();
                c.settle();
            }
        }
    }
    assert!(wrong.is_empty(), "{} of {checked} transitions differ from the original's: {}", wrong.len(), wrong.join(", "));
}
