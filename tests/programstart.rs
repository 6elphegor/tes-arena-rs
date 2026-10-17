//! The program's start (src/start.rs: `memory_init`, `program_start`,
//! 21EE:0000) and the game's set-up after it (21EE:016C, `game_setup`)
//! against the original's (harness/gen_programstart.py): the heap's
//! blocks after each, and what each changed in DGROUP, byte for byte.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/programstart.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn heap_line(m: &Machine) -> String {
    let (mut off, mut seg) = (m.mem.r16(DS, 0x157), m.mem.r16(DS, 0x159));
    let mut out = Vec::new();
    while (seg | off) != 0 && out.len() < 500 {
        out.push(format!("{seg:x} {:x} {:x}", m.mem.r16(seg, off.wrapping_add(8)), m.mem.r8(seg, off.wrapping_add(0xA))));
        (seg, off) = (m.mem.r16(seg, off.wrapping_add(2)), m.mem.r16(seg, off));
    }
    out.join(" ")
}

fn line(kind: &str, stage: &str) -> Vec<String> {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    text.lines().map(|l| l.split(" | ").map(str::to_string).collect::<Vec<_>>()).find(|f| f[0] == kind && f[1] == stage).unwrap_or_else(|| panic!("no {kind} line for {stage}"))
}

/// What a stage changed in DGROUP against the vector's: the same offsets,
/// the same bytes.
fn check_dgroup(stage: &str, d0: &[u8], m: &Machine) {
    let f = line("dgroup", stage);
    let mut want = vec![false; 0x10000];
    for run in f[2].split_whitespace() {
        let (s, n) = run.split_once('+').unwrap();
        let (s, n) = (usize::from_str_radix(s, 16).unwrap(), usize::from_str_radix(n, 16).unwrap());
        want[s..s + n].iter_mut().for_each(|x| *x = true);
    }
    let d1 = m.mem.segment(DS);
    let changed: Vec<usize> = (0..0x10000).filter(|&i| d0[i] != d1[i]).collect();
    let extra: Vec<String> = changed.iter().filter(|&&i| !want[i]).map(|i| format!("{i:04x}")).collect();
    let missing: Vec<String> = (0..0x10000).filter(|&i| want[i] && d0[i] == d1[i]).map(|i| format!("{i:04x}")).collect();
    assert!(extra.is_empty() && missing.is_empty(), "{stage}: changed here, not in the original: {extra:?}; changed there, not here: {missing:?}");
    let got = fnv((0..0x10000).filter(|&i| want[i]).map(|i| d1[i]));
    assert_eq!(format!("{got:x}"), f[3], "{stage}: DGROUP's changed bytes differ from the original's");
}

#[test]
fn the_start_as_the_original() {
    let Some(image) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: no game directory");
        return;
    };
    let mut m = Machine::new(&image);
    m.memory_init().expect("the memory");
    let d0 = m.mem.segment(DS).to_vec();
    m.program_start(&mut files, &mut arena::start::PortStart).expect("the start");
    assert_eq!(heap_line(&m), line("heap", "start")[2], "the heap after the start");
    assert_eq!(format!("{:x}", fnv((0..0x20).flat_map(|b| m.ems_bank(b)))), line("ems", "start")[2], "the EMS banks after the start");
    check_dgroup("start", &d0, &m);
}

#[test]
fn the_setup_as_the_original() {
    let Some(image) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: no game directory");
        return;
    };
    let mut m = Machine::new(&image);
    m.memory_init().expect("the memory");
    let d0 = m.mem.segment(DS).to_vec();
    m.program_start(&mut files, &mut arena::start::PortStart).expect("the start");
    // main's texts' segment (0000:108B), the working slot (1194:3A72).
    let frame = m.mem.r16(DS, 0x9030);
    m.mem.w16(DS, 0xCB8A, frame);
    m.mem.w8(DS, 0x1A80, 0x40);
    m.mem.w8(DS, 0x1A6C, 0x40);
    m.game_setup(&mut arena::start::SetupPort(&mut files)).expect("the set-up");
    m.store_rng();
    if let Ok(d) = std::env::var("ARENA_DUMP") {
        std::fs::write(d, m.mem.segment(DS)).unwrap();
    }
    assert_eq!(heap_line(&m), line("heap", "setup")[2], "the heap after the set-up");
    assert_eq!(format!("{:x}", fnv((0..0x20).flat_map(|b| m.ems_bank(b)))), line("ems", "setup")[2], "the EMS banks after the set-up");
    check_dgroup("setup", &d0, &m);
}
