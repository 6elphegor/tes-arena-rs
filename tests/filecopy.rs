//! A file's copy (25FD:056F, `Machine::file_copy`) against the original's
//! (harness/gen_filecopy.py), with its source there and missing: from the
//! game as the start and set-up leave it, with a missing file opened and
//! closed (DOS's handle 2 freed, as the game's first missing file frees
//! it; tests/doshandles.rs has the copies before), what the copy changed in
//! DGROUP, the destination as written and the EMS banks. Needs ARENA_SAVE
//! (the files copied are the save's).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/filecopy.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

#[test]
fn the_copies_as_the_original_makes_them() {
    let Some(image) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: no game directory");
        return;
    };
    let Ok(save) = std::env::var("ARENA_SAVE") else {
        eprintln!("skipping: ARENA_SAVE not set");
        return;
    };
    files.add_dir(&PathBuf::from(save)).expect("ARENA_SAVE");
    let mut base = Machine::new(&image);
    base.memory_init().expect("the memory");
    base.program_start(&mut files, &mut arena::start::PortStart).expect("the start");
    let frame = base.mem.r16(DS, 0x9030);
    base.mem.w16(DS, 0xCB8A, frame);
    base.mem.w8(DS, 0x1A80, 0x40);
    base.mem.w8(DS, 0x1A6C, 0x40);
    base.game_setup(&mut arena::start::SetupPort(&mut files)).expect("the set-up");
    for (i, &b) in b"NOSUCH.A\0".iter().enumerate() {
        base.mem.w8(DS, 0x1044 + i as u16, b);
    }
    base.file_open_or_device(&mut files, 0x1044).expect("the open");
    base.file_close(&mut files);
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for l in text.lines() {
        let f: Vec<&str> = l.split(" | ").collect();
        let name = f[1];
        let mut m = base.clone();
        let mut fs = files.clone();
        for (at, s) in [(0x1044u16, f[2]), (0x1051, f[3])] {
            for (i, &b) in s.as_bytes().iter().chain(std::iter::once(&0)).enumerate() {
                m.mem.w8(DS, at + i as u16, b);
            }
        }
        let d0 = m.mem.segment(DS).to_vec();
        m.file_copy(&mut fs, 0x1044, 0x1051);
        let d1 = m.mem.segment(DS);
        let changed: Vec<usize> = (0..0x10000).filter(|&i| d0[i] != d1[i]).collect();
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for &i in &changed {
            match runs.last_mut() {
                Some((s, k)) if *s + *k == i => *k += 1,
                _ => runs.push((i, 1)),
            }
        }
        let offs = runs.iter().map(|(s, k)| format!("{s:x}+{k:x}")).collect::<Vec<_>>().join(" ");
        assert_eq!(offs, f[4], "{name}: what the copy changed in DGROUP");
        assert_eq!(format!("{:x}", fnv(changed.iter().map(|&i| d1[i]))), f[5], "{name}: DGROUP's changed bytes");
        let written = fs.written(f[3].as_bytes()).map_or("-".to_string(), |w| format!("{:x} {:x}", w.len(), fnv(w.iter().copied())));
        assert_eq!(written, f[6], "{name}: the destination as written");
        assert_eq!(format!("{:x}", fnv((0..0x20).flat_map(|b| m.ems_bank(b)))), f[7], "{name}: the EMS banks after");
        n += 1;
    }
    assert!(n >= 3, "{n} cases");
}
