//! A headless playtest (`--playtest SEED FRAMES [SETUP]`): the game as the
//! command line starts it, SETUP's steps (`at:COL:ROW:HEADING`, `click`,
//! `wait:N`, comma-separated), then FRAMES turns of a seeded random
//! policy: walking, turning, strafing, clicking in the centre, swinging,
//! camping, a fight summoned now and then, a jump to a random free
//! square, the player healed below a third, the game's questions
//! answered at random, and with `--save-dir` a save and a load back. Each turn runs as the scripted shots run theirs
//! (`shot_frame`: the view's hooks, its dialogs left as Esc leaves them,
//! the areas followed).
//!
//! It reports, one line each: a turn that ends in None (the port gives
//! up: something not ported, or where the original faults), a panic, a
//! turn slower than a second, and odd states (the player off the map,
//! its height out of range). The run stops at the first None or panic,
//! the state then being the port's guess. The last line sums it up:
//! `PLAYTEST seed .. frames .. result ..`.

use crate::{Assets, Shot, shot_frame};
use arena::controls::Intent;
use arena::engine::Engine;
use arena::mem::DS;
use arena::movement::var::{HEADING, HEIGHT, X, Y};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::Instant;

/// xorshift64*: the policy's dice.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Where the player is, for the reports.
fn place(engine: &Engine, run: &arena::transit::Run) -> String {
    let r = |at| engine.m.mem.r16(DS, at);
    format!(
        "{run:?} {} at ({:#x}, {:#x}) square ({}, {}) heading {:#x} height {}",
        String::from_utf8_lossy(engine.m.mem.cstr(DS, arena::transit::var::LEVEL_NAME)),
        r(X),
        r(Y),
        r(X) >> 7,
        r(Y) >> 7,
        r(HEADING),
        r(HEIGHT) as i16
    )
}

/// What a save and a load must keep: the level's name, the position and
/// the heading (the run's label may differ: an interior's name pointer).
fn kept(engine: &Engine) -> (Vec<u8>, u16, u16, u16, u16) {
    let r = |at| engine.m.mem.r16(DS, at);
    (engine.m.mem.cstr(DS, arena::transit::var::LEVEL_NAME).to_vec(), r(X), r(Y), r(HEIGHT), r(HEADING))
}

/// The odd states looked for after each turn.
fn odd(engine: &Engine) -> Option<String> {
    let r = |at| engine.m.mem.r16(DS, at);
    let (w, d) = (r(0xC357), r(0xC359));
    let (col, row) = (r(X) >> 7, r(Y) >> 7);
    if w != 0 && d != 0 && (col >= w || row >= d) {
        return Some(format!("off the map: square ({col}, {row}) of {w} x {d}"));
    }
    let h = r(HEIGHT) as i16;
    if !(-200..=2000).contains(&h) {
        return Some(format!("height {h}"));
    }
    None
}

/// What a turn of the policy does.
#[derive(Clone, Copy, Debug)]
enum Act {
    Walk(i16, i16, i16),
    Click,
    Swing,
    Camp,
    Fight,
    SaveLoad,
    Jump,
}

pub fn run(dir: &Path, mut engine: Engine, mut run: arena::transit::Run, seed: u64, frames: usize, setup: &str, saves: Option<(&Path, u8)>) {
    let run = &mut run;
    let assets = Assets::new(dir, &mut engine);
    let audio = crate::audio::Audio::new(assets.archive.clone(), false, false);
    let st = &mut Shot {
        audio,
        movies: Vec::new(),
        assets,
        right: false,
        drag: (0, 0),
        clicked: false,
        camp: false,
        answers: Default::default(),
        message: None,
        frames: 0,
        boxes: Vec::new(),
        need: None,
        status: None,
        panel: None,
        map: false,
        over: false,
        ui: None,
    };
    let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let issues = std::cell::Cell::new(0usize);
    let mut last_odd = None;
    // One turn, caught: Some(true) on, Some(false) stopped (reported).
    let mut turn = |engine: &mut Engine, st: &mut Shot, run: &mut arena::transit::Run, intent: Intent, what: &str, n: usize| -> bool {
        let t = Instant::now();
        let r = catch_unwind(AssertUnwindSafe(|| shot_frame(dir, engine, run, st, &intent)));
        let ms = t.elapsed().as_millis();
        if ms > 1000 {
            println!("SLOW frame {n} ({what}): {ms} ms, {}", place(engine, run));
        }
        match r {
            Ok(Some(())) => {}
            Ok(None) => {
                println!("NONE frame {n} ({what}): {}", place(engine, run));
                return false;
            }
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                println!("PANIC frame {n} ({what}): {msg}; {}", place(engine, run));
                return false;
            }
        }
        if let Some(o) = odd(engine) {
            if last_odd.as_ref() != Some(&o) {
                println!("ODD frame {n} ({what}): {o}; {}", place(engine, run));
                issues.set(issues.get() + 1);
            }
            last_odd = Some(o);
        } else {
            last_odd = None;
        }
        true
    };
    let mut n = 0usize;
    let mut alive = true;
    // The setup's steps.
    for step in setup.split(',').filter(|s| !s.is_empty()) {
        if !alive {
            break;
        }
        if let Some(at) = step.strip_prefix("at:") {
            let v: Vec<u16> = at.split(':').map(|x| x.parse().expect("at:COL:ROW:HEADING")).collect();
            engine.m.mem.w16(DS, X, v[0] << 7 | 0x40);
            engine.m.mem.w16(DS, Y, v[1] << 7 | 0x40);
            engine.m.mem.w16(DS, HEADING, v[2]);
            n += 1;
            alive = turn(&mut engine, st, run, Intent::default(), step, n);
        } else if step == "click" {
            st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
            n += 1;
            alive = turn(&mut engine, st, run, Intent::default(), step, n);
        } else if let Some(k) = step.strip_prefix("wait:") {
            for _ in 0..k.parse::<usize>().expect("wait:N") {
                n += 1;
                alive = alive && turn(&mut engine, st, run, Intent::default(), step, n);
            }
        } else {
            panic!("--playtest setup: {step}?");
        }
    }
    println!("start: {}", place(&engine, run));
    let mut act = Act::Walk(256, 0, 0);
    let mut left = 0;
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    while alive && n < frames && !st.over {
        if left == 0 {
            left = 5 + dice.below(40) as usize;
            act = match dice.below(100) {
                0..=54 => Act::Walk(256, 0, 0),
                55..=69 => Act::Walk(0, 0, (dice.below(9) as i16 - 4) * 32),
                70..=77 => Act::Walk(if dice.below(2) == 0 { 256 } else { -256 }, if dice.below(2) == 0 { 256 } else { -256 }, 0),
                78..=89 => Act::Click,
                90..=95 => Act::Swing,
                96 => Act::Camp,
                97 => Act::Fight,
                98 => Act::Jump,
                _ => Act::SaveLoad,
            };
        }
        left -= 1;
        n += 1;
        let name = match act {
            Act::Walk(..) => "walk",
            Act::Click => "click",
            Act::Swing => "swing",
            Act::Camp => "camp",
            Act::Fight => "fight",
            Act::SaveLoad => "save/load",
            Act::Jump => "jump",
        };
        // A test aid, as --do hp is: the player healed when below a third,
        // so a run goes on past its fights.
        let (hp, max) = (engine.m.mem.r16(DS, 0x01FD), engine.m.mem.r16(DS, 0x01FF));
        if hp < max / 3 {
            engine.m.mem.w16(DS, 0x01FD, max);
            *counts.entry("healed").or_default() += 1;
        }
        // And rested when fatigue (201h) is below a quarter of its most,
        // (strength + endurance) x 64: once exhausted, an hour's rest
        // (2901:0014, 016C) makes up 420 for a heal mod of 0 where the hour
        // takes 660, so the player collapses again and again, the
        // original until its stack runs into its data, the port until its
        // bound of 200 (None). A player rests before that.
        let attr = |i: u16| engine.m.mem.r8(DS, arena::character::PLAYER + arena::character::field::ATTRS + i) as u16;
        let most = (attr(0) + attr(5)) * 64;
        if engine.m.mem.r16(DS, 0x0201) < most / 4 {
            engine.m.mem.w16(DS, 0x0201, most);
            *counts.entry("rested").or_default() += 1;
        }
        *counts.entry(name).or_default() += 1;
        // The game's questions answered at random (talk menus, shops,
        // haggles, rests): menu entries 0..5, now and then a number typed.
        while st.answers.len() < 3 {
            let a = if dice.below(7) == 0 {
                crate::dialog::Answer::Text(dice.below(120).to_string())
            } else {
                crate::dialog::Answer::Pick(dice.below(6) as u16)
            };
            st.answers.push_back(a);
        }
        let mut intent = Intent::default();
        match act {
            Act::Walk(f, s, look) => {
                intent.forward = f;
                intent.strafe = s;
                intent.look = look;
            }
            Act::Click => {
                if left % 4 == 0 {
                    st.clicked = crate::centre_click(&mut engine, &mut st.boxes);
                }
            }
            Act::Swing => {
                if left % 6 == 0 {
                    crate::interface::ready(&mut engine.m);
                }
                st.right = true;
                st.drag = ((dice.below(200) as i16) - 100, 100);
            }
            Act::Camp => {
                if left == 0 || left % 20 == 0 {
                    st.camp = true;
                }
            }
            Act::Fight => {
                if left == 0 {
                    crate::world::summon(&mut engine);
                }
            }
            Act::Jump => {
                // To a random free square of the map (its walls' word 0), a
                // test aid spreading a run over the map, as --at is.
                if left == 0 {
                    let walls = engine.m.mem.r16(DS, arena::mif::var::MAP1_SEG);
                    let (w, d) = (engine.m.mem.r16(DS, 0xC357).min(128) as u64, engine.m.mem.r16(DS, 0xC359).min(128) as u64);
                    for _ in 0..200 {
                        let (c, r) = (dice.below(w.max(1)) as u16, dice.below(d.max(1)) as u16);
                        if engine.m.mem.r16(walls, r << 8 | c << 1) == 0 {
                            engine.m.mem.w16(DS, X, c << 7 | 0x40);
                            engine.m.mem.w16(DS, Y, r << 7 | 0x40);
                            break;
                        }
                    }
                }
            }
            Act::SaveLoad => {
                if let (0, Some((sdir, slot))) = (left, saves) {
                    let pre = place(&engine, run);
                    let msg = crate::save_to(&mut engine, sdir, slot, st.assets.palette);
                    let before = place(&engine, run);
                    let was = kept(&engine);
                    if std::env::var_os("PT_VERBOSE").is_some() {
                        println!("save frame {n}: fighting {} resting {} pre {pre}", engine.m.mem.r8(DS, arena::encounter::var::FIGHTING), engine.m.mem.r8(DS, arena::rest::var::RESTING));
                    }
                    match crate::load_from(&mut engine, dir, sdir, slot) {
                        Ok((_, r)) => {
                            *run = r;
                            st.assets = Assets::new(dir, &mut engine);
                            let after = place(&engine, run);
                            if was != kept(&engine) {
                                println!("SAVELOAD frame {n}: {msg}; before {before}; after {after}");
                                issues.set(issues.get() + 1);
                            }
                        }
                        Err(e) => {
                            println!("SAVELOAD frame {n}: {msg}; load failed: {e}; {before}");
                            issues.set(issues.get() + 1);
                        }
                    }
                }
            }
        }
        alive = turn(&mut engine, st, run, intent, name, n);
    }
    let result = if st.over {
        "over (the game ended)".to_string()
    } else if alive {
        "ok".to_string()
    } else {
        "stopped".to_string()
    };
    println!("end: {}", place(&engine, run));
    println!("PLAYTEST seed {seed} frames {n} result {result} odd {} acts {counts:?}", issues.get());
}
