//! A worn item on the character sheet's doll (1194:1F8D,
//! `Machine::sheet_item`) against the original's
//! (harness/gen_sheetitem.py): plain and coloured weapons and armour
//! (the colouring through bank 4, 20BF), helmets by race and sex (2020:
//! the head's colours, race 7's own picture by its armour's kind), an
//! accessory and special items, from the game as the start and set-up
//! leave it: what each changed in DGROUP, the bank left mapped, the EMS
//! banks and the page. Needs ARENA_SAVE.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/sheetitem.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// gen_sheetitem.py's `cases`: (name, race, sex, type, flags, colour, DI).
fn cases() -> Vec<(String, u8, u8, u8, u8, u8, u16)> {
    let mut out = Vec::new();
    for t in [0u8, 3, 7, 11, 15] {
        out.push((format!("weapon{t}"), 0, 0, t, 0xC4, 0));
        for c in [0u8, 1, 2, 3, 5, 7] {
            out.push((format!("weapon{t}c{c}"), 0, 0, t, 0xC5, c));
        }
    }
    for t in [0u8, 2, 4, 6, 8] {
        let kinds: &[u8] = if t != 8 { &[0, 1, 2] } else { &[0, 1] };
        for &k in kinds {
            out.push((format!("armour{t}k{k}"), 0, 1, t, 0xC0 | k << 4, 0));
            out.push((format!("armour{t}k{k}c2"), 0, 1, t, 0xC1 | k << 4, 2));
        }
    }
    for race in 0..8u8 {
        for sex in [0u8, 1] {
            out.push((format!("helmet{race}s{sex}"), race, sex, 5, 0xC0, 0));
        }
    }
    for k in [1u8, 2] {
        out.push((format!("helmet7k{k}"), 7, 0, 5, 0xC0 | k << 4, 0));
    }
    out.push(("helmet7c4".into(), 7, 0, 5, 0xC1, 4));
    out.push(("ring".into(), 0, 0, 3, 0xC8, 0));
    out.push(("special2".into(), 0, 0, 0x82, 0xC4, 0));
    out.push(("special5".into(), 3, 0, 0x85, 0xC0, 0));
    let mut out: Vec<_> = out.into_iter().map(|(n, r, s, t, f, c)| (n, r, s, t, f, c, 0)).collect();
    out.push(("helmet7di".into(), 7, 1, 5, 0xC0, 0, 0x929E));
    out
}

#[test]
fn worn_items_on_the_doll_as_the_original_draws_them() {
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
    let lines: Vec<&str> = text.lines().collect();
    let all = cases();
    assert_eq!(lines.len(), all.len(), "the cases");
    for (l, (name, race, sex, ty, flags, colour, di)) in lines.iter().zip(all) {
        let f: Vec<&str> = l.split(" | ").collect();
        assert_eq!(f[1], name);
        let mut m = base.clone();
        let mut fs = files.clone();
        let d0 = m.mem.segment(DS).to_vec();
        m.mem.w8(DS, 0x1A8, race);
        m.mem.w8(DS, 0x1AB, sex);
        m.mem.w8(DS, 0x212, ty);
        m.mem.w8(DS, 0x212 + 0x0E, flags);
        m.mem.w8(DS, 0x212 + 0x10, colour);
        m.sheet_clothes();
        m.equip_frames(&mut fs).expect("the equipment's frames");
        m.sheet_item(&mut fs, 0x212, di).unwrap_or_else(|| panic!("{name}: gave up: {}", arena::giveup::describe()));
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
        assert_eq!(offs, f[2], "{name}: what it changed in DGROUP");
        assert_eq!(format!("{:x}", fnv(changed.iter().map(|&i| d1[i]))), f[3], "{name}: DGROUP's changed bytes");
        let page = m.mem.r16(DS, 0xA900);
        let after = format!(
            "{:x} {:x} {:x}",
            m.mem.r16(DS, 0x8FA8),
            fnv((0..0x20).flat_map(|b| m.ems_bank(b))),
            fnv((0..0xFA00u16).map(|i| m.mem.r8(page, i)))
        );
        assert_eq!(after, f[4], "{name}: the bank mapped, the EMS banks and the page after");
    }
}
