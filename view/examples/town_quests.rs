//! A town's quests in play: the city loaded as the view loads it, its
//! entry run (`Machine::city_entered`, 0000:46B2: its buildings listed,
//! its quests made), the records listed, then the giver of the first
//! errand offered talked to in a tavern (`Machine::quests_talk`, 8F1E),
//! its texts printed and the offer taken.
//!
//! Usage: town_quests GAME_DIR CITY:P:I [TICKS]
//! TICKS sets the PC's timer (`Machine::bios_ticks`), which seeds them.
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::var::{OTHERS, QUESTS};
use arena::sidequest::var::{TALK, TALKER};

/// Prints the quests' texts; answers yes.
struct Show;

impl arena::quest::QuestHooks for Show {
    fn text_at(&mut self, m: &mut Machine, files: &mut Files, mode: u16, di: u16) -> Option<u16> {
        let (_, _) = m.text(files, &mut arena::text::NoTextHooks, 2, DS, 0x987A)?;
        let t = String::from_utf8_lossy(m.mem.cstr(DS, 0x987A)).replace('\r', "\n");
        println!("  [{}] {t}", if mode == 1 { "offer" } else { "text" });
        let _ = di;
        Some(0)
    }
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        0
    }
}

fn records(m: &Machine) {
    for (name, base, size) in [("quest", QUESTS, 0x34u16), ("errand", OTHERS, 0x29)] {
        for i in 0..4u16 {
            let si = base + i * size;
            let w = m.mem.r16(DS, si + 4);
            let kind = ["free", "under way", "offered", "failed"][(w >> 6 & 3) as usize];
            println!(
                "{name} {i}: {kind}, stage {}, sort {}, giver {:04x}, goal {:08x}, reward {}, place {:02x}",
                w >> 9 & 7,
                w >> 12,
                m.mem.r16(DS, si + 0x10),
                m.mem.r32(DS, si + 0x14),
                m.mem.r32(DS, si + 0x18),
                m.mem.r8(DS, si + 0x24)
            );
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let mut engine = world::load(&dir, &a[2]);
    engine.m.bios_ticks = a.get(3).map_or(0x1234, |s| s.parse().expect("TICKS"));
    let m = &mut engine.m;
    let counts = |m: &Machine| (m.mem.r16(DS, 0x1387), m.mem.r16(DS, 0x138B), m.mem.r16(DS, 0x1389));
    m.mem.w16(DS, 0x0FC8, 0xFFFF);
    if m.city_entered(&mut engine.files, &mut Show).is_none() {
        println!("the town's quests met something not ported");
        return;
    }
    let (t, te, s) = counts(m);
    println!("taverns {t}, temples {te}, stores {s}");
    records(m);
    let Some(si) = (0..4u16).map(|i| OTHERS + i * 0x29).find(|&si| m.quest_kind(si) == 2) else {
        println!("no errand offered");
        return;
    };
    let g = m.mem.r16(DS, si + 0x10);
    m.mem.w8(DS, TALK, 4);
    m.mem.w16(DS, TALKER, g & 0xFF00 | ((g as u8 as u16) << 1) & 0xFF);
    println!("talking to the giver of errand {} ({g:04x}) in a tavern:", (si - OTHERS) / 0x29);
    if m.quests_talk(&mut engine.files, &mut Show).is_none() {
        println!("the talk met something not ported");
    }
    records(m);
}
