//! The engine's hooks the view fills in, tested without a window: what
//! the original shows or runs there reaches the front end (`Front`'s
//! boxes kept by `dialog::Talk`) instead of the engine's defaults, which
//! show nothing.
//!
//! The game's files are found as `inputtest` finds them; without them
//! the tests say so and pass.

use crate::*;
use arena::mem::DS;
use arena::quest::main_quest;

fn engine() -> Option<Engine> {
    let dir = match std::env::var("ARENA_EXE") {
        Ok(exe) => PathBuf::from(exe).parent()?.to_path_buf(),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ESarena"),
    };
    if !dir.join("GLOBAL.BSA").exists() {
        eprintln!("skipping: no game files at {}", dir.display());
        return None;
    }
    Some(world::load(&dir, "CITY:8:0"))
}

fn said(talk: &dialog::Talk) -> String {
    let mut s = String::new();
    for b in &talk.boxes {
        for line in b {
            for span in line {
                s.push_str(&span.text);
            }
            s.push('\n');
        }
    }
    s
}

/// The main quest's chest at its last step (2294:0F33): text 1FAh in its
/// box, then 1194:1034's level-up screen as the finale (CD00 FFFFh at
/// its end), the step back to 0.
#[test]
fn the_main_quests_last_chest_shows_its_text_and_the_finale() {
    let Some(mut e) = engine() else { return };
    let m = &mut e.m;
    const RECORD: u16 = 0x3D96;
    let step = (0..16u16).find(|&i| m.mem.r8(DS, main_quest::PIECES + i) == main_quest::LAST_PIECE).unwrap() as u8;
    m.mem.w8(DS, 0x05E6, 3);
    m.mem.w8(DS, 0x0A46, 2);
    m.mem.w8(DS, 0x0A4C, 5);
    m.mem.w16(DS, RECORD, 10 << 7);
    m.mem.w16(DS, RECORD + 4, 12 << 7);
    let square = m.record_square(RECORD);
    m.mem.w16(DS, main_quest::CHEST, square);
    m.mem.w8(DS, main_quest::PROVINCE, 0xFF);
    m.mem.w8(DS, main_quest::NEXT_PROVINCE, 2);
    m.mem.w8(DS, main_quest::NEXT_PLACE, 5);
    m.mem.w8(DS, main_quest::STEP, step);
    let floor = m.mem.r16(DS, 0xD806);
    m.mem.w16(floor, square, 0x1234);
    m.mem.w16(DS, arena::controls::var::MOUSE_X, 7);
    // A character's base attributes (not all FFh, 1194:11C6).
    for i in 0..8 {
        m.mem.w8(DS, 0x01A4 + 0x31 + i, 50);
    }
    m.mem.w16(DS, 0xCD00, 0);
    let mut talk = dialog::Talk::default();
    let mut front = Front { talk: Some(&mut talk), ..Default::default() };
    let files = e.files.clone();
    let mut fs = files.clone();
    assert!(arena::talk::TalkHooks::quest_chest(&mut front, m, &mut fs, RECORD));
    assert_eq!(m.mem.r8(DS, main_quest::STEP), 0);
    assert_eq!(m.mem.r16(floor, square), 0, "the chest taken off the floor");
    assert_eq!(m.mem.r16(DS, 0xCD00), 0xFFFF, "the finale's level-up screen ran to its end");
    assert!(!talk.boxes.is_empty(), "text 1FAh shown");
    eprintln!("{}", said(&talk));
}

/// A staff piece's words (2433:0BF4, text 553h + pieces) through the
/// loot window's text hook.
#[test]
fn a_staff_pieces_words_are_shown() {
    let Some(mut e) = engine() else { return };
    let m = &mut e.m;
    m.mem.w16(DS, 0x137A, 0x553 + 1);
    m.mem.w8(DS, 0x137E, 0);
    let mut talk = dialog::Talk::default();
    let mut front = Front { talk: Some(&mut talk), ..Default::default() };
    let files = e.files.clone();
    arena::loot_window::LootHooks::text(&mut LootFront(&mut front, files), m, 0);
    assert!(!talk.boxes.is_empty());
    eprintln!("{}", said(&talk));
}

/// Drowning in the wild (2826:0D5F): the frame's box with 7927's text.
#[test]
fn drowning_shows_its_box() {
    let Some(mut e) = engine() else { return };
    let m = &mut e.m;
    let mut talk = dialog::Talk::default();
    let mut front = Front { talk: Some(&mut talk), ..Default::default() };
    arena::frame::FrameHooks::message_box(&mut front, m, 0x7927);
    let s = said(&talk);
    assert!(!s.trim().is_empty());
    eprintln!("{s}");
}
