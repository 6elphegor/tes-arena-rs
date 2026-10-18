//! DOS's handles as the game's file calls meet them (src/bsa.rs `Dos`)
//! against the original's (harness/gen_doshandles.py): from the game as
//! the start and set-up leave it, scripts of opens that go on after a
//! missing file (handle 2, DOS's error code), seeks, reads, closes and
//! copies, with CON on 2 at first, then 2 closed, free, and handed to the
//! next file opened. Needs ARENA_SAVE (the files opened are the save's).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/doshandles.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn put(m: &mut Machine, at: u16, s: &str) {
    for (i, &b) in s.as_bytes().iter().chain(std::iter::once(&0)).enumerate() {
        m.mem.w8(DS, at + i as u16, b);
    }
}

/// One call of a script, its result as gen_doshandles.py writes it; None
/// where the original waits for the keyboard (DOS reading CON).
fn step(m: &mut Machine, fs: &mut Files, op: &str) -> Option<String> {
    let w: Vec<&str> = op.split(' ').collect();
    arena::giveup::take();
    let r = match w[0] {
        "miss" | "open" => {
            put(m, 0x1044, w[1]);
            m.file_open_or_device(fs, 0x1044).expect("the open");
            format!("{:x}", m.mem.r16(0, 0x6837))
        }
        "seek" => format!("{:x}", m.file_seek(fs, w[1].parse().unwrap(), 0)),
        "read" => {
            let n = u16::from_str_radix(w[1], 16).unwrap();
            let frame = m.mem.r16(DS, 0x9030);
            let ax = m.file_read(fs, frame, 0, n);
            let got: Vec<u8> = if ax <= n { (0..ax).map(|i| m.mem.r8(frame, i)).collect() } else { Vec::new() };
            format!("{ax:x} {:x}", fnv(got))
        }
        // AX after a close that works is AH 3Eh and the wrapper's AL (0
        // here); after one that fails, DOS's error 6.
        "close" => if m.file_close(fs) { "3e00 0".to_string() } else { "6 1".to_string() },
        "copy" => {
            put(m, 0x1044, w[1]);
            put(m, 0x1051, w[2]);
            m.file_copy(fs, 0x1044, 0x1051);
            let words = [0x126Cu16, 0x126E, 0xA836, 0xA838].map(|o| m.mem.r16(DS, o));
            let counts = [0u16, 1, 2, 3].map(|k| m.mem.r16(DS, 0x5F5C + 4 * k));
            let written = fs.written(w[2].as_bytes()).map_or("-".to_string(), |d| format!("{:x}/{:x}", d.len(), fnv(d.iter().copied())));
            let v: Vec<String> = words.iter().chain(counts.iter()).map(|v| format!("{v:x}")).collect();
            format!("{} {written}", v.join(" "))
        }
        _ => panic!("{op}"),
    };
    match arena::giveup::take() {
        Some(g) if g.kind == arena::giveup::Kind::Original && g.why.starts_with("DOS read from a character device") => None,
        _ => Some(r),
    }
}

#[test]
fn the_handles_as_dos_gives_them() {
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
        let mut m = base.clone();
        let mut fs = files.clone();
        let mut got = Vec::new();
        for op in f[2].split("; ") {
            match step(&mut m, &mut fs, op) {
                Some(r) => got.push(r),
                None => {
                    got.push("waits".to_string());
                    break;
                }
            }
        }
        assert_eq!(got.join("; "), f[3], "{}: {}", f[1], f[2]);
        n += 1;
    }
    assert!(n >= 9, "{n} cases");
}
