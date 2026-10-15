//! The main quest walked through its first steps, by the game's own
//! entries, printing its state (the block at 0FDD) at each:
//!
//! 1. a town (as a new game leaves the start dungeon into it, the main
//!    quest's block as a new game has it: 0FDD and 0FE4 FFh, the rest 0)
//!    entered (`city_entered`, 0000:46B2: its lists, the taverns' at
//!    1AB5);
//! 2. its townspeople talked to, standing beside each (`actor_click`,
//!    1194:DB6C), the talk's Rumors and their first kind asked: a roll
//!    within (level + 1) x 2 offers the next step (1194:E4A3, CD9A), its
//!    text shown;
//! 3. the tavern the offer names (0FDE: its place word) entered and its
//!    keeper talked to (`tavern_shop`, 1194:B93C, whose talk runs the
//!    informant, CED7): the informant's deal (haggled, mode 5) tells the
//!    piece's dungeon (CFAF: 0FE2/0FE3).
//!
//!
//! With Firewatch (CITY:3:3) at level 9 the third talk round offers
//! Rajhin's ring (step 0Fh) and the informant at the Haunted Wharf sells
//! the way for 950 gold (0FE6 bit 2, 0FE2 the dungeon's province).
//!
//! Usage: mainquest GAME_DIR [CITY:P:I] [LEVEL]
//! LEVEL is the player's level byte (01AA) for the rumours' roll.
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

/// The main quest's state.
fn state(m: &Machine) -> String {
    let block: Vec<String> = (0..0x13u16).map(|i| format!("{:02x}", m.mem.r8(DS, 0x0FDD + i))).collect();
    format!(
        "step {:02x}, block [{}], done {:04x}, flags {:02x}, gold {}",
        m.mem.r8(DS, 0x0FDD),
        block.join(" "),
        m.mem.r16(DS, 0x0FE7),
        m.mem.r8(DS, 0x0FE6),
        m.mem.r32(DS, 0x05C2),
    )
}

/// Answers the menus from a queue (then leaves them), prints the texts.
struct Talk {
    picks: Vec<u16>,
}

impl Talk {
    fn next(&mut self) -> u16 {
        if self.picks.is_empty() { 0xFFFF } else { self.picks.remove(0) }
    }
}

fn show(m: &Machine, at: u16) -> String {
    String::from_utf8_lossy(m.mem.cstr(DS, at)).replace('\r', " ")
}

impl arena::text::TextHooks for Talk {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn menu(&mut self, m: &mut Machine, si: u16, _bx: u16, _di: u16) -> u16 {
        let p = self.next();
        println!("    [menu] {} -> {}", show(m, si).trim(), p as i16);
        p
    }
}

impl arena::talk::TalkHooks for Talk {
    fn way(&mut self, m: &mut Machine, si: u16, x: u16, y: u16) {
        println!("    [the way to {} at ({x}, {y})]", show(m, si));
    }
}

impl arena::shops::ShopHooks for Talk {
    fn message(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn menu(&mut self, m: &mut Machine, si: u16, _bx: u16, _di: u16) -> u16 {
        let p = self.next();
        println!("    [menu] {} -> {}", show(m, si).trim(), p as i16);
        p
    }
}
impl arena::tavern::TavernHooks for Talk {
    /// 1194:CED7, the informant, his price accepted.
    fn quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        let r = m.quest_informant(files, &mut Quests);
        println!("    [informant] {}", match r {
            Some(true) => "not the informant here",
            Some(false) => "the informant talked to",
            None => "met something not ported",
        });
        r.map(|_| ())
    }
}

/// The haggle: the first price accepted, the shopkeeper's lines printed.
struct Accept;

impl arena::haggle::HaggleIo for Accept {
    fn menu(&mut self) -> Option<u16> {
        Some(0)
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        None
    }
    fn key(&mut self) -> Option<()> {
        Some(())
    }
    fn say(&mut self, m: &mut Machine, si: u16, _kind: u8) {
        println!("    [haggle] {}", show(m, si));
    }
}

/// The quests' hooks: the informant's haggle accepted.
struct Quests;

impl arena::quest::QuestHooks for Quests {
    fn informant_haggle(&mut self, m: &mut Machine, files: &mut Files, price: u32, per: u16, mode: u8) -> Option<bool> {
        let r = m.haggle(files, &mut Accept, price, per, 0x1E, 0x28, 0, mode, 0)?;
        println!("    [informant's haggle, mode {mode}, price {price}] {r:?}");
        Some(r == arena::haggle::Haggle::NoDeal)
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(&a[1]);
    let spec = a.get(2).cloned().unwrap_or_else(|| "CITY:3:3".into());
    let level: u8 = a.get(3).map_or(9, |s| s.parse().expect("LEVEL"));
    let mut engine = world::load(&dir, &spec);
    world::city_people(&mut engine);
    // A frame of the actors: the townspeople take their views (C0h..C4h).
    engine.m.actors_update(&mut engine.files);
    let m = &mut engine.m;
    // The main quest's block as a new game has it, and the player's level.
    for i in 0..0x13u16 {
        m.mem.w8(DS, 0x0FDD + i, 0);
    }
    m.mem.w8(DS, 0x0FDD, 0xFF);
    m.mem.w8(DS, 0x0FE4, 0xFF);
    m.mem.w8(DS, 0x01AA, level);
    // A weapon in the first slot, as a new game gives one (1194:26DC):
    // with no slot occupied the offer's search for a piece (1194:CDEA)
    // runs through all of DGROUP and finds one.
    let slot = arena::inventory::PLAYER_SLOTS;
    m.mem.w8(DS, slot, 1);
    let f = m.mem.r8(DS, slot + arena::item::FLAGS) | arena::item::OCCUPIED | 4;
    m.mem.w8(DS, slot + arena::item::FLAGS, f);
    m.mem.w32(DS, 0x05C2, 1000);
    println!("1. {spec}: {}", state(m));
    if m.city_entered(&mut engine.files, &mut arena::quest::NoQuestHooks).is_none() {
        println!("   the town's entry met something not ported");
        return;
    }
    println!("   taverns {}", m.mem.r16(DS, 0x1387));
    // 2. The townspeople's rumours until the next step is offered.
    let mut offered = false;
    'talk: for round in 0..40 {
        // New townspeople each round, as walking the streets meets them
        // (0000:1793; the rumours' roll is seeded per person, 0151).
        if round > 0 {
            world::city_people(&mut engine);
            engine.m.actors_update(&mut engine.files);
        }
        for slot in 0..15u16 {
            let si = 0x0AB3 + slot * 0x1C;
            if engine.m.mem.r16(DS, si + 0xF) & 0x4000 != 0 {
                continue;
            }
            // Stand beside them.
            let (x, y) = (engine.m.mem.r16(DS, si), engine.m.mem.r16(DS, si + 4));
            engine.m.mem.w16(DS, arena::movement::var::X, x.wrapping_add(0x40));
            engine.m.mem.w16(DS, arena::movement::var::Y, y);
            // Time passes between talks (the timer at 0151 seeds the
            // rumours' roll).
            engine.tick(37);
            engine.m.mem.w16(DS, arena::talk::var::CLICKED, si);
            let mut t = Talk { picks: vec![2, 0, 0xFFFF] };
            let before = engine.m.mem.r8(DS, 0x0FDD);
            if engine.m.actor_click(&mut engine.files, &mut t).is_none() {
                println!("   round {round} slot {slot}: the talk met something not ported");
                continue;
            }
            if engine.m.mem.r8(DS, 0x0FDD) != before {
                println!("2. offered by townsperson {slot} (round {round}): {}", state(&engine.m));
                offered = true;
                break 'talk;
            }
        }
    }
    if !offered {
        println!("2. no townsperson offered the next step");
        return;
    }
    // 3. The informant: 0FDE names the tavern (its word, the talker's as
    // a tavern's keeper is kept at TALKER: kind 4).
    let m = &mut engine.m;
    let g = m.mem.r16(DS, 0x0FDE);
    println!("3. the informant at {g:04x}; the tavern's keeper talked to");
    m.mem.w8(DS, arena::sidequest::var::TALK, 4);
    // (TALKER keeps the place word with its low byte doubled; the
    // informant compares it halved, `sq_talker`.)
    m.mem.w16(DS, arena::sidequest::var::TALKER, g & 0xFF00 | ((g as u8 as u16) << 1) & 0xFF);
    let mut t = Talk { picks: vec![0, 0, 0, 0xFFFF] };
    let r = m.tavern_shop(&mut engine.files, &mut t);
    println!("   {}: {}", if r.is_some() { "done" } else { "met something not ported" }, state(&engine.m));
}
