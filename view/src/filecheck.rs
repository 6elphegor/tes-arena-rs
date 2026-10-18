//! The files the view writes against the original's
//! (harness/gen_savefiles.py, tests/vectors/savefiles.txt), for
//! savetest's saves and screentest's screens.
//!
//! The files are as the game finds them: the save folder's (where the
//! view writes a save), and the ones written in play over them (the
//! working slot's, kept in memory: `Files`).

use super::*;
use std::collections::{BTreeMap, BTreeSet};

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/savefiles.txt");

/// Files by upper-case name.
type Files = BTreeMap<String, Vec<u8>>;

/// The files as the game finds them at a moment, and the names it has
/// deleted (and not made again: `Files::deleted_names`, the game
/// folder's working files among them).
#[derive(Clone, Default)]
pub(super) struct Disk {
    files: Files,
    deleted: BTreeSet<String>,
}

/// The files the game finds now: the save folder's (`dir`) that the game
/// hasn't deleted, and the ones written in play over them.
pub(super) fn files_now(a: &App, dir: &Path) -> Disk {
    let mut out = Files::new();
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
    let deleted = a.engine.files.deleted_names().iter().map(|n| String::from_utf8_lossy(n).into_owned()).collect();
    Disk { files: out, deleted }
}

impl Disk {
    /// The file `name` as `from` has it: one the view writes after the
    /// original's stop, running on past a box the original waits at.
    pub(super) fn kept(mut self, from: &Disk, name: &str) -> Self {
        match from.files.get(name) {
            Some(d) => self.files.insert(name.to_string(), d.clone()),
            None => self.files.remove(name),
        };
        self
    }
}

/// The files of `after` made or changed since `before`, as the vectors
/// hold them: SAVEGAME's ceiling plane (after the screen, the palette,
/// the level's block and two planes), which the original takes from
/// 0000:0000 (the interrupt table on) where the level has none (D748 0),
/// left out as 0s, as gen_savefiles.py leaves the original's.
fn changed(a: &App, before: &Files, after: &Files) -> Files {
    const AT: usize = 0xFA00 + 0x300 + 0x0DE7 + 0x10000;
    let no_ceiling = a.engine.m.mem.r16(DS, 0xD748) == 0;
    let mut out: Files = after.iter().filter(|(n, d)| before.get(*n) != Some(*d)).map(|(n, d)| (n.clone(), d.clone())).collect();
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
/// files gone (deleted since, or no longer found).
fn changes(kind: &str, name: &str, made: &Files, before: &Disk, after: &Disk) -> Vec<String> {
    let mut out: Vec<String> = made.iter().map(|(n, d)| format!("{kind} | {name} | {n} {:x} {:x}", d.len(), fnv(d.iter().copied()))).collect();
    let mut gone: BTreeSet<&str> = after.deleted.difference(&before.deleted).map(|n| n.as_str()).collect();
    gone.extend(before.files.keys().filter(|n| !after.files.contains_key(*n)).map(|n| n.as_str()));
    let gone: Vec<&str> = gone.into_iter().collect();
    out.push(format!("{kind}del | {name} | {}", gone.join(" ")).trim_end().to_string());
    out
}

/// A scratch file of the original's: a screen, a box's background or the
/// EMS it covers put by for a while (CHARSHET.$$$, SCREENSV.$$2,
/// TEMPSAVE.$$$, ...: written in the game's folder, read back by the same
/// screen). The view keeps those in memory and never writes the game's
/// folder, so they are left out of the original's.
fn scratch(name: &str) -> bool {
    name.contains(".$$")
}

/// The vectors' lines of `kind` for the case (for a screen, the
/// original's scratch files left out); with none at all, the empty
/// deletions' line.
fn wanted(kind: &str, name: &str) -> Vec<String> {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let del = format!("{kind}del");
    let skip = kind == "screen";
    let mut out: Vec<String> = Vec::new();
    for l in text.lines() {
        let f: Vec<&str> = l.trim_end().split(" | ").collect();
        if f.len() < 2 || f[1] != name {
            continue;
        }
        if f[0] == kind && f.len() == 3 && !(skip && scratch(f[2])) {
            out.push(l.trim_end().to_string());
        } else if f[0] == del {
            let gone: Vec<&str> = f.get(2).map(|g| g.split(' ').filter(|n| !(n.is_empty() || skip && scratch(n))).collect()).unwrap_or_default();
            out.push(format!("{del} | {name} | {}", gone.join(" ")).trim_end().to_string());
        }
    }
    if !out.iter().any(|l| l.starts_with(&del)) {
        out.push(format!("{del} | {name} |"));
    }
    out
}

/// The files made, changed and deleted from `before` to `after` against
/// the vectors' lines of `kind` for `name`: whether they differ (printing
/// how). With ARENA_SAVEFILES set to a folder of gen_savefiles.py
/// --dump's, the view's files are put beside the original's
/// (NAME.KIND.port) and the first bytes that differ printed.
pub(super) fn check_files(a: &App, name: &str, kind: &str, before: &Disk, after: &Disk) -> bool {
    let mut made = changed(a, &before.files, &after.files);
    let (mut before, mut after) = (before.clone(), after.clone());
    if kind == "screen" {
        // (Scratch the view writes too: a town's way in, TMP2.$$$.)
        made.retain(|n, _| !scratch(n));
        for d in [&mut before, &mut after] {
            d.files.retain(|n, _| !scratch(n));
            d.deleted.retain(|n| !scratch(n));
        }
    }
    let (before, after) = (&before, &after);
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
            let mut runs: Vec<(usize, usize)> = Vec::new();
            for i in (0..d.len().min(o.len())).filter(|&i| d[i] != o[i]) {
                match runs.last_mut() {
                    Some(r) if i - r.1 <= 16 => r.1 = i + 1,
                    _ => runs.push((i, i + 1)),
                }
            }
            eprintln!("  {n}: {} vs {} bytes, first differing at {first:x?}, runs {:x?}", d.len(), o.len(), &runs[..runs.len().min(12)]);
        }
    }
    true
}
