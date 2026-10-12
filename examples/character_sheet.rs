//! Loads a saved game with the ported code and prints the character.
//!
//! Usage: character_sheet GAME_DIR SAVE_DIR SLOT
//!
//! GAME_DIR is the game directory (ACD.EXE, GLOBAL.BSA); SAVE_DIR holds
//! the save (SAVEENGN.nn for slot SLOT, e.g. 1 for SAVEENGN.01).

use arena::bsa::Files;
use arena::character::field::*;
use arena::character::PLAYER;
use arena::exe;
use arena::inventory::{item, SLOT_COUNT, SLOT_SIZE};
use arena::machine::Machine;
use arena::mem::DS;
use arena::save::spell;
use std::path::PathBuf;

/// Race names in race order, and the player's gold (dword).
const RACE_NAMES: u16 = 0x2B94;
const GOLD: u16 = 0x05C2;
const LOCATION: u16 = 0x0A22;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: character_sheet GAME_DIR SAVE_DIR SLOT");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let slot: u8 = args[3].parse().expect("slot");
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(&dir).expect("game directory");
    files.add_dir(&PathBuf::from(&args[2])).expect("save directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    // The blocks the load reads into, allocated as the game would have.
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (creatures, _) = m.heap_alloc_bytes(0x20F0).expect("heap");
    let (buffer, _) = m.heap_alloc(0x1000).expect("heap");
    let (extra, off) = m.heap_alloc_bytes(0x120).expect("heap");
    m.mem.w16(DS, 0x1042, creatures);
    m.mem.w16(DS, 0x9030, buffer);
    m.mem.w16(DS, 0x1020, off);
    m.mem.w16(DS, 0x1022, extra);
    m.save_load_engine(&mut files, slot).expect("SAVEENGN for that slot");
    m.spells_load(&mut files).expect("spells");

    let s = |m: &Machine, seg: u16, off: u16| String::from_utf8_lossy(m.mem.cstr(seg, off)).into_owned();
    let r8 = |f: u16| m.mem.r8(DS, PLAYER + f);
    let r16 = |f: u16| m.mem.r16(DS, PLAYER + f);
    let race = r8(RACE);
    let race_name = s(&m, DS, m.skip_strings(DS, RACE_NAMES, race as u16 & 7));
    let (cs, co) = m.class_name(DS, PLAYER);
    println!("{}", s(&m, DS, PLAYER + NAME));
    println!(
        "  {} {} {}, level {}",
        if r8(SEX) == 0 { "male" } else { "female" },
        race_name,
        s(&m, cs, co),
        r8(LEVEL) as u16 + 1
    );
    println!("  in {}", s(&m, DS, LOCATION));
    let names = ["STR", "INT", "WIL", "AGI", "SPD", "END", "PER", "LCK"];
    let attrs: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, n)| format!("{n} {}", arena::scale::to100(r8(ATTRS + i as u16) as i16)))
        .collect();
    println!("  {}", attrs.join("  "));
    println!(
        "  health {}/{}  spell points {}/{}  fatigue {} (raw)",
        r16(HP),
        r16(MAX_HP),
        r16(SPELL_POINTS),
        r16(MAX_SPELL_POINTS),
        r16(FATIGUE)
    );
    println!("  experience {}  gold {}", m.mem.r32(DS, PLAYER + XP), m.mem.r32(DS, GOLD));
    println!("  carrying (* equipped):");
    for i in 0..SLOT_COUNT {
        let it = PLAYER + ITEMS + i * SLOT_SIZE;
        let flags = m.mem.r8(DS, it + item::FLAGS);
        if flags & item::OCCUPIED == 0 {
            continue;
        }
        let Some(name) = m.item_name(DS, it) else { continue };
        let name = s(&m, DS, name);
        let has_condition = m.mem.r16(DS, it + item::MAX_HEALTH) != 0;
        let condition = if has_condition { format!(" ({})", s(&m, DS, m.condition_name(DS, it))) } else { String::new() };
        let mark = if flags & item::EQUIPPED != 0 { '*' } else { ' ' };
        println!("   {mark} {name}{condition}");
    }
    // SPELLSG.nn holds the spells in the game (128); SPELLS.nn, reached
    // with bit 7 set, 32 more.
    for (title, indices) in [("spells (base cost)", 0..128u8), ("SPELLS.nn spells", 0x80..0xA0)] {
        let mut list = Vec::new();
        for i in indices {
            let (seg, rec) = m.spell_record(i);
            let name = s(&m, seg, rec + spell::NAME);
            if !name.is_empty() {
                list.push(format!("{name} ({})", m.mem.r16(seg, rec + spell::COST)));
            }
        }
        println!("  {title}:");
        for line in list.chunks(4) {
            println!("    {}", line.join(", "));
        }
    }
}
