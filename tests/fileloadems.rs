//! A whole file loaded into EMS (0000:7428 with DI FFFDh,
//! `Machine::file_load_ems`) against the original's
//! (harness/gen_fileloadems.py): a file from GLOBAL.BSA, one of the
//! save's, two in a row and a name there is no file of, from the game as
//! the start and set-up leave it: what each load returned, what they
//! changed in DGROUP, the bank left mapped and the EMS banks. Needs
//! ARENA_SAVE (the save's file is loaded).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/fileloadems.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

#[test]
fn files_loaded_into_ems_as_the_original_loads_them() {
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
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for l in text.lines() {
        let f: Vec<&str> = l.split(" | ").collect();
        let name = f[1];
        let mut m = base.clone();
        let mut fs = files.clone();
        let d0 = m.mem.segment(DS).to_vec();
        let mut got = Vec::new();
        for file in f[2].split(',') {
            for (i, &b) in file.as_bytes().iter().chain(std::iter::once(&0)).enumerate() {
                m.mem.w8(DS, 0x1044 + i as u16, b);
            }
            // None: the original's 0:0 with carry set.
            got.push(match m.file_load_ems(&mut fs, 0x1044) {
                Some((seg, off)) => format!("{seg:x}:{off:x} 0"),
                None => "0:0 1".to_string(),
            });
        }
        assert_eq!(got.join(" "), f[3], "{name}: what the loads returned");
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
        assert_eq!(offs, f[4], "{name}: what the loads changed in DGROUP");
        assert_eq!(format!("{:x}", fnv(changed.iter().map(|&i| d1[i]))), f[5], "{name}: DGROUP's changed bytes");
        let ems = format!("{:x} {:x}", m.mem.r16(DS, 0x8FA8), fnv((0..0x20).flat_map(|b| m.ems_bank(b))));
        assert_eq!(ems, f[6], "{name}: the bank mapped and the EMS banks after");
        n += 1;
    }
    assert!(n >= 4, "{n} cases");
}
