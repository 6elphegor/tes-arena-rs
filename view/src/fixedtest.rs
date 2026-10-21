//! Fixed mode in play: every case the view plays against the original
//! (worldframes.txt's, the BUSY and FUZZ sessions among them, and
//! areaplay.txt's transits) played again with `Fixes::ALL`, the way the
//! window plays it unless `--original` (docs/kept-bugs.md, "Fixed mode").
//!
//! Its frames are expected to differ from the original's vectors, so
//! nothing here compares with them. What it checks is that the fixes
//! together still play: no panic, no hang (a case over ARENA_FIXED_SECS,
//! 900 by default), no give-up (`arena::giveup`) the faithful run of the
//! same case doesn't also meet, and after every frame a sane state (HP,
//! spell points, gold and level in range, no attribute wrapping; the heap's list as
//! 2294:11F2 checks it; the EMS bank mapped one there is), and a 3D view
//! drawn at the end.
//!
//! Opt-in, as it plays every case twice (faithful, then fixed):
//!
//! ```text
//! ARENA_FIXED=1 ARENA_EXE=.../ACD.EXE ARENA_SAVE=... \
//!     cargo test --release -p arena-view fixed_mode -- --nocapture
//! ```
//!
//! ARENA_CASE picks one case; ARENA_FIXED_FRAMES=DIR also writes the
//! drawing fixes' before/after frames as PNGs there
//! (`fixed_mode_drawing_frames`).

use super::*;
use arena::fixes::{self, Fixes};

/// What a case's play came to.
#[derive(Debug, Default, Clone)]
struct Outcome {
    /// Frames played (the track's length).
    frames: usize,
    /// The player dead at the end.
    died: bool,
    /// The give-ups noted on the way (kind and why).
    gave_up: Vec<String>,
    /// The first of each kind of insane state seen, and its frame.
    insane: Vec<String>,
    /// The panic's message, where it panicked.
    panic: Option<String>,
    /// The case wasn't played (no game files, no save).
    skipped: bool,
    /// The 3D page at the end holds a picture (not one colour).
    drawn: bool,
    secs: f64,
}

thread_local! {
    /// The insane states seen this case, and the frame count so far.
    static INSANE: std::cell::RefCell<(usize, Vec<String>)> = const { std::cell::RefCell::new((0, Vec::new())) };
    /// The attributes and their bases (ATTRS, BASE_ATTRS) as the last
    /// frame left them.
    static ATTRS: std::cell::Cell<Option<[u8; 16]>> = const { std::cell::Cell::new(None) };
}

/// What's out of range in the game's state, each a short line whose
/// first word is its kind.
fn insanity(a: &App) -> Vec<String> {
    use arena::character::{field, PLAYER};
    let m = &a.engine.m;
    let r8 = |f: u16| m.mem.r8(DS, PLAYER + f);
    let r16 = |f: u16| m.mem.r16(DS, PLAYER + f);
    let mut bad = Vec::new();
    let (hp, max_hp) = (r16(field::HP) as i16, r16(field::MAX_HP));
    if max_hp == 0 || max_hp > 3000 {
        bad.push(format!("maxhp {max_hp}"));
    }
    if hp > max_hp as i16 {
        bad.push(format!("hp {hp} over its maximum {max_hp}"));
    }
    let (sp, max_sp) = (r16(field::SPELL_POINTS), r16(field::MAX_SPELL_POINTS));
    if sp > max_sp.max(1) * 2 || max_sp > 6000 {
        bad.push(format!("sp {sp} of {max_sp}"));
    }
    let level = r8(field::LEVEL);
    if level > 40 {
        bad.push(format!("level {level}"));
    }
    // (The attributes are kept on a 256 scale, any byte a value: what's
    // insane is one wrapping, a jump of over 80h in a frame.)
    let attrs: [u8; 16] = std::array::from_fn(|k| r8(field::ATTRS + k as u16));
    ATTRS.with(|p| {
        if let Some(was) = p.get() {
            for k in 0..16 {
                if attrs[k].abs_diff(was[k]) > 0x80 {
                    bad.push(format!("attribute {}{k}: {:#x} to {:#x}", if k < 8 { "" } else { "base " }, was[k], attrs[k]));
                }
            }
        }
        p.set(Some(attrs));
    });
    let gold = m.mem.r32(DS, arena::shops::var::GOLD);
    if gold >= 0x8000_0000 {
        bad.push(format!("gold {gold:#x}"));
    }
    if !m.heap_check() {
        bad.push("heap list broken (2294:11F2's check)".to_string());
    }
    let blocks = heap_blocks(m).len();
    if blocks >= 500 {
        bad.push(format!("heap {blocks} blocks"));
    }
    use arena::level::var::{EMS_MAPPED, EMS_ON, EMS_PAGES};
    if m.mem.r8(DS, EMS_ON) != 0 && m.mem.r16(DS, EMS_MAPPED) >> 2 >= m.mem.r16(DS, EMS_PAGES).max(1) {
        bad.push(format!("ems bank {} mapped of {}", m.mem.r16(DS, EMS_MAPPED) >> 2, m.mem.r16(DS, EMS_PAGES)));
    }
    bad
}

/// `Driver::frame`'s hook: the frame's insane states, the first of each
/// kind kept.
fn frame_checked(a: &App) {
    let bad = insanity(a);
    INSANE.with(|s| {
        let mut s = s.borrow_mut();
        s.0 += 1;
        let k = s.0;
        for b in bad {
            let kind = b.split(' ').next().unwrap_or("").to_string();
            if !s.1.iter().any(|x| x.split(' ').nth(2) == Some(kind.as_str())) {
                s.1.push(format!("frame {k}: {b}"));
            }
        }
    });
}

/// A worldframes.txt or areaplay.txt case.
fn any_case(name: &str) -> Option<Case> {
    case(name).or_else(|| areaplaytest::transit_case(name))
}

/// Every case the view plays that the vectors have a track for.
fn all_cases() -> Vec<String> {
    let mut names = Vec::new();
    for path in [VECTORS, areaplaytest::AREAPLAY] {
        let text = std::fs::read_to_string(path).expect(path);
        for l in text.lines() {
            let f: Vec<&str> = l.split(" | ").collect();
            if f.len() == 3 && f[0] == "track" && !names.iter().any(|n| n == f[1]) {
                names.push(f[1].to_string());
            }
        }
    }
    names.retain(|n| any_case(n).is_some());
    names
}

/// The case played on this thread with `fixes`: what it came to.
fn play_one(name: &str, fixes: Fixes) -> Outcome {
    let t = std::time::Instant::now();
    world::FIXED_HERE.with(|f| f.set(Some(fixes)));
    FRAME_CHECK.with(|c| c.set(Some(frame_checked)));
    INSANE.with(|s| *s.borrow_mut() = (0, Vec::new()));
    ATTRS.with(|p| p.set(None));
    arena::giveup::noted();
    let (place, setup, play) = any_case(name).unwrap();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        }, play)
    }));
    let mut o = Outcome { secs: t.elapsed().as_secs_f64(), ..Default::default() };
    o.gave_up = arena::giveup::noted().iter().map(|g| format!("{}: {}", g.kind.marker(), g.why)).collect();
    o.gave_up.dedup();
    match r {
        Err(e) => o.panic = Some(e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()),
        Ok(None) => o.skipped = true,
        Ok(Some((a, seen))) => {
            o.frames = seen.len();
            o.died = fuzz_dead(&a);
            frame_checked(&a);
            o.drawn = a.engine.page.as_ref().is_none_or(|p| p.iter().any(|&c| c != p[0]));
        }
    }
    o.insane = INSANE.with(|s| std::mem::take(&mut s.borrow_mut().1));
    o
}

/// `play_one` on a thread of its own (the cases' thread-locals fresh),
/// given up on after `limit`.
fn play_threaded(name: &str, fixes: Fixes, limit: std::time::Duration) -> Outcome {
    let (tx, rx) = std::sync::mpsc::channel();
    let n = name.to_string();
    std::thread::Builder::new()
        .name(format!("fixed-{name}"))
        .stack_size(256 << 20)
        .spawn(move || {
            let _ = tx.send(play_one(&n, fixes));
        })
        .unwrap();
    match rx.recv_timeout(limit) {
        Ok(o) => o,
        Err(_) => Outcome { panic: Some(format!("no end after {}s: a hang?", limit.as_secs())), secs: limit.as_secs_f64(), ..Default::default() },
    }
}

/// What's wrong with the fixed run that the faithful one doesn't share.
fn problems(faithful: &Outcome, fixed: &Outcome) -> Vec<String> {
    let mut p = Vec::new();
    if let Some(m) = &fixed.panic {
        if faithful.panic.is_none() {
            p.push(format!("panicked: {m}"));
        }
    }
    for g in &fixed.gave_up {
        if !faithful.gave_up.contains(g) {
            p.push(format!("gave up: {g}"));
        }
    }
    for i in &fixed.insane {
        let what = i.splitn(3, ' ').nth(2).unwrap_or("").split(' ').next().unwrap_or("");
        if !faithful.insane.iter().any(|f| f.splitn(3, ' ').nth(2).unwrap_or("").split(' ').next() == Some(what)) {
            p.push(format!("insane: {i}"));
        }
    }
    if !fixed.drawn && faithful.drawn {
        p.push("the 3D page left blank".to_string());
    }
    p
}

/// Every case, faithful then fixed, `ARENA_FIXED_JOBS` at a time (the
/// machine's threads by default); fails listing each case whose fixed
/// play goes wrong where its faithful play doesn't.
#[test]
fn fixed_mode_plays_every_case() {
    if std::env::var("ARENA_FIXED").is_err() || game_dir().is_none() || std::env::var("ARENA_SAVE").is_err() {
        return;
    }
    let names: Vec<String> = all_cases().into_iter().filter(|n| only_case(n)).collect();
    let limit = std::time::Duration::from_secs(std::env::var("ARENA_FIXED_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(900));
    let jobs = std::env::var("ARENA_FIXED_JOBS").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
    let queue = std::sync::Arc::new(std::sync::Mutex::new(names.clone()));
    let (tx, rx) = std::sync::mpsc::channel();
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (queue, tx) = (queue.clone(), tx.clone());
            std::thread::spawn(move || {
                loop {
                    let Some(name) = queue.lock().unwrap().pop() else { break };
                    let faithful = play_threaded(&name, Fixes::NONE, limit);
                    let fixed = play_threaded(&name, Fixes::ALL, limit);
                    let _ = tx.send((name, faithful, fixed));
                }
            })
        })
        .collect();
    drop(tx);
    let mut bad = Vec::new();
    let mut n = 0;
    for (name, faithful, fixed) in rx {
        n += 1;
        if fixed.skipped || faithful.skipped {
            eprintln!("[{n}/{}] {name}: skipped", names.len());
            continue;
        }
        let p = problems(&faithful, &fixed);
        eprintln!(
            "[{n}/{}] {name}: faithful {} frames{}{} {:.1}s, fixed {} frames{}{} {:.1}s{}",
            names.len(),
            faithful.frames,
            if faithful.died { " dead" } else { "" },
            if faithful.gave_up.is_empty() { String::new() } else { format!(" gave up {:?}", faithful.gave_up) },
            faithful.secs,
            fixed.frames,
            if fixed.died { " dead" } else { "" },
            if fixed.gave_up.is_empty() { String::new() } else { format!(" gave up {:?}", fixed.gave_up) },
            fixed.secs,
            if p.is_empty() { String::new() } else { format!(" PROBLEMS: {}", p.join("; ")) }
        );
        if !faithful.insane.is_empty() {
            eprintln!("    (faithful insane too: {})", faithful.insane.join("; "));
        }
        if let Some(m) = &faithful.panic {
            eprintln!("    (faithful panicked too: {m})");
        }
        if !p.is_empty() {
            bad.push(format!("{name}: {}", p.join("; ")));
        }
    }
    for w in workers {
        let _ = w.join();
    }
    assert!(bad.is_empty(), "fixed mode goes wrong in {} cases:\n{}", bad.len(), bad.join("\n"));
}

/// The colours a page's bytes name (`App::dac`'s).
type Palette = [[u8; 4]; 256];

/// A case's 3D page frame by frame, played with `fixes`, and the colours.
fn pages(name: &str, fixes: Fixes) -> Option<(Vec<Vec<u8>>, Palette)> {
    thread_local! {
        static PAGES: std::cell::RefCell<Vec<Vec<u8>>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    fn grabbed(a: &App) {
        if let Some(p) = &a.engine.page {
            PAGES.with(|v| v.borrow_mut().push(p.clone()));
        }
    }
    world::FIXED_HERE.with(|f| f.set(Some(fixes)));
    FRAME_CHECK.with(|c| c.set(Some(grabbed)));
    PAGES.with(|v| v.borrow_mut().clear());
    let (place, setup, play) = drawing_case(name).or_else(|| any_case(name))?;
    let (a, _) = played(place, |a| {
        a.engine.page = Some(vec![0; 64000]);
        setup(a)
    }, play)?;
    FRAME_CHECK.with(|c| c.set(None));
    Some((PAGES.with(|v| std::mem::take(&mut *v.borrow_mut())), a.assets.dac()))
}

/// The drawing fixes' own scenes: `stormlong`, a storm's bolt (`storm`)
/// through its frames and after; `moonHH-D-DAY`, the wilderness at hour
/// HH facing heading D (hex) on day DAY of the month (CCA8), the moons'
/// pictures made again in its first frame (CB36, the day they were made
/// for, FFh).
fn drawing_case(name: &str) -> Option<Case> {
    if name == "stormlong" {
        return Some((Place::Street, Box::new(|a: &mut App| storm(a, 0, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 12, |_| false))));
    }
    let mut f = name.strip_prefix("moon")?.split('-');
    let (hour, heading, day): (u16, u16, u8) = (f.next()?.parse().ok()?, u16::from_str_radix(f.next()?, 16).ok()?, f.next()?.parse().ok()?);
    Some((
        Place::Wild,
        Box::new(move |a: &mut App| {
            at_hour(a, hour, heading);
            a.engine.m.mem.w8(DS, arena::clock::var::SKY_DAY, day);
            a.engine.m.mem.w8(DS, arena::celestial::var::PHASE_DAY, 0xFF);
        }),
        Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false)),
    ))
}

/// `name`'s frames faithful and with fix `bit` alone, side by side
/// (faithful | fixed | the pixels that differ, white) as PNGs in `dir`:
/// the first frame that differs, one in the middle of those that do, and
/// the last. The files written.
fn dumped(dir: &std::path::Path, tag: &str, name: &str, bit: u8) -> Vec<String> {
    let Some((before, pal)) = pages(name, Fixes::NONE) else { return vec![] };
    let Some((after, _)) = pages(name, Fixes::only(bit)) else { return vec![] };
    let differing: Vec<usize> = (0..before.len().min(after.len())).filter(|&k| before[k] != after[k]).collect();
    eprintln!("{tag} ({name}, fix #{bit}): {} of {} frames differ", differing.len(), before.len().min(after.len()));
    let mut picks: Vec<usize> = [differing.first(), differing.get(differing.len() / 2), differing.last()].into_iter().flatten().copied().collect();
    picks.dedup();
    // (None differing: the last frame, to show the scene alike.)
    if picks.is_empty() && !before.is_empty() {
        picks.push(before.len().min(after.len()) - 1);
    }
    let mut out = Vec::new();
    for k in picks {
        let (b, f) = (&before[k], &after[k]);
        let mut img = vec![0u8; 960 * 200 * 3];
        let mut n = 0;
        for i in 0..64000 {
            let (x, y) = (i % 320, i / 320);
            let d = if b[i] != f[i] {
                n += 1;
                [0xFF; 3]
            } else {
                [0; 3]
            };
            let px = |c: u8| [pal[c as usize][0], pal[c as usize][1], pal[c as usize][2]];
            for (j, c) in [px(b[i]), px(f[i]), d].into_iter().enumerate() {
                let at = (y * 960 + j * 320 + x) * 3;
                img[at..at + 3].copy_from_slice(&c);
            }
        }
        let file = dir.join(format!("{tag}-{name}-frame{k}.png"));
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&file).unwrap()), 960, 200);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&img).unwrap();
        eprintln!("  {} ({n} pixels differ)", file.display());
        out.push(file.display().to_string());
    }
    out
}

/// The drawing fixes' frames, faithful beside fixed, as PNGs in
/// ARENA_FIXED_FRAMES (game frames: never into the repository).
#[test]
fn fixed_mode_drawing_frames() {
    let Some(dir) = std::env::var("ARENA_FIXED_FRAMES").ok().map(PathBuf::from) else { return };
    if game_dir().is_none() || std::env::var("ARENA_SAVE").is_err() {
        return;
    }
    std::fs::create_dir_all(&dir).unwrap();
    let mut runs: Vec<(&str, String, u8)> = vec![
        ("31-arrow", "fuzzarcher1".into(), fixes::ARROW_HEAD_START),
        ("44-ceiling", "playcrypt".into(), fixes::CEILING_STEP),
        ("45-bolt", "playbolt".into(), fixes::BOLT_Y),
        ("45-bolt", "stormlong".into(), fixes::BOLT_Y),
        ("46-snow", "playsnow".into(), fixes::SNOW_BANDS),
    ];
    // (Found by sweeping the days and headings: on day 28 the second
    // moon is up over the wilderness at ten facing 100h and 140h.)
    runs.push(("47-moon", "moon22-100-28".into(), fixes::MOON2_PHASE));
    runs.push(("47-moon", "moon22-140-28".into(), fixes::MOON2_PHASE));
    let only = std::env::var("ARENA_CASE").ok();
    let runs: Vec<_> = runs.into_iter().filter(|r| only.as_deref().is_none_or(|o| o == r.1 || o == r.0)).collect();
    let queue = std::sync::Arc::new(std::sync::Mutex::new(runs));
    let jobs = std::thread::available_parallelism().map_or(4, |n| n.get());
    let threads: Vec<_> = (0..jobs)
        .map(|_| {
            let (dir, queue) = (dir.clone(), queue.clone());
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || {
                    while let Some((tag, name, bit)) = queue.lock().unwrap().pop() {
                        dumped(&dir, tag, &name, bit);
                    }
                })
                .unwrap()
        })
        .collect();
    for t in threads {
        let _ = t.join();
    }
}
