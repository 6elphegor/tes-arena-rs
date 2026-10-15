//! The staff's line (STAFF:DIR[:SLOT]): from a saved game, each of the
//! eight pieces as the game leads to it, by its own routines:
//!
//! 1. a rest until healed (2433:12D6): the dream (1194:D340 sets 0F76,
//!    D3AF shows it);
//! 2. the stage's (0F77) person: the town named at 80F4h in the province
//!    at 80ECh, travelled to and the building at 8223h (0 the palace, 7 a
//!    mages' guild, 5 the temple named at 822Bh) gone into through its
//!    door (0000:4054), the ruler (flat 0 or 1) or keeper (C943) clicked
//!    (1194:DB6C, whose talk calls D593): the question answered yes
//!    (D67F: 0F79, 0F78; the first dungeon, CITYDATA's second fixed entry,
//!    made known, 26F7:0C0A mode 3);
//! 3. the building left by its door (0000:44BD: 1194:D55F, 0F7E and 0F7C
//!    with the question taken; 2433:0199);
//! 4. that dungeon travelled to, its levels gone down until one holds the
//!    staff's flat (C941) in its walls, clicked (292B:06DC; the loot
//!    window's 2433:0BF4: 0F7D, text 553h + pieces), and left;
//! 5. the person again: the staff's dungeon (the first fixed entry, mode
//!    2) made known, text +4;
//! 6. that dungeon, its flat clicked: 1194:D363 counts the piece (0F77),
//!    clears 0F76..0F80 and gives 1000 x (level + 1) experience.
//!
//! After the eighth the Imperial City's palace (`crate::palace`): the
//! audience (stage 8), into the palace's lower levels (IMPPAL), the
//! staff's flat there (stage 9, 1194:D986) and the way out.
//!
//! The steps the walkthrough takes instead of walking: travel by the
//! travel map, the player placed beside a door or a flat, and the levels
//! changed by the stairs' own routines without walking to them.
use super::*;

/// The staff's flags 0F76..0F7F.
fn flags(m: &Machine) -> String {
    let b: Vec<String> = (0..10u16).map(|i| format!("{:02x}", m.mem.r8(DS, 0x0F76 + i))).collect();
    format!("stage (F77) {}, F76..F7F [{}], level byte {}, xp {}", m.mem.r8(DS, 0x0F77), b.join(" "), m.mem.r8(DS, 0x01AA), m.mem.r32(DS, arena::character::PLAYER + 0x409))
}

/// The string `n` of the NUL-separated list at DS:`at`.
fn nth(m: &Machine, at: u16, n: u8) -> Vec<u8> {
    let mut si = at;
    for _ in 0..n {
        while m.mem.r8(DS, si) != 0 {
            si += 1;
        }
        si += 1;
    }
    m.mem.cstr(DS, si).to_vec()
}

/// The walls plane's squares (row << 8 | column x 2) whose word `pick`
/// takes.
fn squares(m: &Machine, pick: impl Fn(u16) -> bool) -> Vec<u16> {
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (rows, cols) = (m.mem.r16(DS, 0xC359), m.mem.r16(DS, 0xC357));
    (0..rows).flat_map(|r| (0..cols).map(move |c| r << 8 | c << 1)).filter(|&sq| pick(m.mem.r16(walls, sq))).collect()
}

/// The squares whose wall is a trigger (0000:43B1) of index `pick`.
fn triggers(m: &Machine, pick: impl Fn(u16) -> bool) -> Vec<u16> {
    squares(m, |w| {
        if (w as i16) >= 0 && (w >> 8) as u8 != w as u8 {
            return false;
        }
        (0..0x10u16).find(|&i| m.mem.r8(DS, arena::movement::var::TRIGGERS + i) == w as u8 & 0x3F).is_some_and(&pick)
    })
}

/// The player at square `sq`'s centre (`dx` along x).
fn stand(m: &mut Machine, sq: u16, dx: u16) -> (u16, u16) {
    let (x, y) = (((sq & 0xFF) >> 1) << 7 | 0x40, (sq >> 8) << 7 | 0x40);
    m.mem.w16(DS, arena::movement::var::X, x.wrapping_add(dx));
    m.mem.w16(DS, arena::movement::var::Y, y);
    (x, y)
}

/// Doors' hooks: boxes printed, the rest the game's (the default leave
/// runs 1194:D55F and 2433:0199).
struct Doors;

impl arena::trigger::TriggerHooks for Doors {
    fn message_box(&mut self, m: &mut Machine, text: u16) {
        println!("    [box] {}", show(m, text));
    }
}

/// The door at `sq` used (0000:43B1 through the trigger's table).
fn door(engine: &mut Engine, sq: u16) -> Option<()> {
    let m = &mut engine.m;
    m.mem.w16(DS, arena::controls::var::PICKED, sq);
    stand(m, sq, 0);
    m.wall_trigger(&mut engine.files, &mut Doors)
}

/// The talk's hooks for the staff: the person's question answered yes,
/// the texts' numbers printed; shops are left at once.
pub struct Person;

impl arena::text::TextHooks for Person {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
}

impl arena::talk::TalkHooks for Person {
    fn quest_person(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        println!("    [the quest's person (1194:D593)]");
        m.quest_person_talk(files, &mut Yes)
    }
    fn shop(&mut self, _m: &mut Machine, _files: &mut Files, kind: u8) -> Option<()> {
        println!("    [the shop, kind {kind}, left]");
        Some(())
    }
    fn loot(&mut self, m: &mut Machine, files: &mut Files, ax: u16, si: u16, seg: u16, bx: u16) -> Option<()> {
        println!("    [the loot window, AX {ax:04x}]");
        m.loot_window(files, &mut Loot, ax, si, seg, bx)
    }
}

/// The loot window's boxes and texts printed.
struct Loot;

impl arena::loot_window::LootHooks for Loot {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn text(&mut self, m: &mut Machine, _mode: u16) {
        println!("    [text {:#x}]", m.mem.r16(DS, 0x137A));
    }
    fn last_piece(&mut self, m: &mut Machine) {
        println!("    [the eighth piece: 1194:D986] {}", flags(m));
    }
}

/// The main quest's person's hooks: yes to the question.
struct Yes;

impl arena::quest::QuestHooks for Yes {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        println!("    [box] {}", show(m, si));
    }
    fn text_at(&mut self, m: &mut Machine, files: &mut Files, mode: u16, di: u16) -> Option<u16> {
        let n = m.mem.r16(DS, 0x137A);
        let r = m.text(files, &mut arena::text::NoTextHooks, mode, DS, di).map(|(ax, _)| ax);
        if mode == 2 {
            println!("    [text {n:#x}] {}", show(m, di).chars().take(160).collect::<String>());
        } else {
            println!("    [text {n:#x} shown]");
        }
        r
    }
    fn substitute(&mut self, m: &mut Machine, files: &mut Files, di: u16) -> Option<()> {
        m.expand(files, &mut arena::text::NoTextHooks, DS, di)?;
        let stage = m.mem.r8(DS, 0x0F77);
        println!("    [this temple: {}; the stage's (822Bh): {}]", show(m, di), String::from_utf8_lossy(&nth(m, 0x822B, stage)));
        Some(())
    }
    fn menu(&mut self, m: &mut Machine, si: u16, _bx: u16, _di: u16) -> u16 {
        println!("    [menu] {} -> yes", show(m, si).chars().take(160).collect::<String>());
        0
    }
}

/// The flat `flat` at square `sq` clicked, the player beside it (the
/// record the renderer's hit leaves, 0000:C524, then 1194:DB6C).
fn click(engine: &mut Engine, sq: u16, flat: u8) -> Option<()> {
    let m = &mut engine.m;
    let (x, y) = stand(m, sq, 0x40);
    let rec = arena::talk::var::RECORD;
    for i in 0..0x1Cu16 {
        m.mem.w8(DS, rec + i, 0);
    }
    m.mem.w16(DS, rec, x);
    m.mem.w16(DS, rec + 4, y);
    let base = m.mem.r16(DS, 0xC38E);
    m.mem.w8(DS, rec + 0xC, (flat as u16).wrapping_add(base) as u8);
    m.mem.w16(DS, arena::talk::var::CLICKED, rec);
    m.actor_click(&mut engine.files, &mut Person)
}

/// Into the building of the stage, its person clicked. Whether one
/// answered (0F7B set by D67F).
fn person(engine: &mut Engine, dir: &std::path::Path, run: &mut Run) -> Option<bool> {
    let stage = engine.m.mem.r8(DS, 0x0F77);
    let at = engine.m.mem.r8(DS, 0x8223 + stage as u16);
    // The city's trigger table: 2 mages' guild, 3 temple, 11..13 palace.
    let doors = triggers(&engine.m, |i| match at {
        0 => (11..=13).contains(&i),
        7 => i == 2,
        _ => i == 3,
    });
    println!("   the building (8223h) {at}: {} doors", doors.len());
    let before = engine.m.mem.r8(DS, 0x0F7B) + engine.m.mem.r8(DS, 0x0F79);
    for sq in doors {
        door(engine, sq)?;
        if engine.m.mem.r8(DS, arena::trigger::var::AREA) != 4 {
            println!("   the door at {sq:04x}: not in");
            continue;
        }
        *run = areas::next(engine, dir, *run)?;
        let m = &engine.m;
        let keeper = m.mem.r8(DS, arena::talk::var::KEEPER_FLAT);
        let want: Vec<u8> = if at == 0 { vec![0, 1] } else { vec![keeper] };
        let found: Vec<(u16, u8)> = want.iter().flat_map(|&f| squares(m, |w| w == 0x8000 | f as u16).into_iter().map(move |s| (s, f))).collect();
        println!("   in {} (kind {}): the person's flats {:?}", String::from_utf8_lossy(m.mem.cstr(DS, arena::transit::var::LEVEL_NAME)), m.mem.r8(DS, 0x0D48), found);
        if let Some(&(sq, f)) = found.first() {
            click(engine, sq, f)?;
        }
        println!("   {}", flags(&engine.m));
        let answered = engine.m.mem.r8(DS, 0x0F7B) + engine.m.mem.r8(DS, 0x0F79) != before;
        leave(engine, dir, run, 0)?;
        if answered {
            return Some(true);
        }
    }
    Some(false)
}

/// The way out's question (26F7:0000) answered yes.
struct LeaveYes;

impl arena::transit::AreaHooks for LeaveYes {
    fn ask_leave(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> bool {
        true
    }
}

/// Out of an interior or dungeon from level `at` (where it was
/// entered): by a door there (an INDOORS trigger, 0000:44BD), else by
/// the stairs past that end (0000:F791/F842: the level past it
/// meanwhile, the way out CD4E, its question answered yes); then into
/// what the outer loop picks.
fn leave(engine: &mut Engine, dir: &std::path::Path, run: &mut Run, at: u8) -> Option<()> {
    use arena::movement::var::{LEVEL, LEVELS};
    while engine.m.mem.r8(DS, LEVEL) != at {
        let l = engine.m.mem.r8(DS, LEVEL);
        change_level(engine, if l > at { l - 1 } else { l + 1 })?;
    }
    let m = &mut engine.m;
    if let Some(&sq) = triggers(m, |_| true).first() {
        door(engine, sq)?;
        println!("   left by the door at {sq:04x}: {}", flags(&engine.m));
    } else {
        let past = if at == 0 { 0xFF } else { m.mem.r8(DS, LEVELS) };
        m.mem.w8(DS, LEVEL, past);
        let r = m.area_exit(&mut engine.files, &mut LeaveYes);
        m.mem.w8(DS, LEVEL, at);
        if r.is_none() || m.mem.r16(DS, 0xC398) & 0x80 == 0 {
            println!("   the stairs out didn't leave");
            return None;
        }
        println!("   left by the stairs from level {at}: {}", flags(&engine.m));
    }
    *run = areas::next(engine, dir, *run)?;
    Some(())
}

/// The stairs to level `to` taken as 0000:F76F takes them: the stairs'
/// hook (CD6E), the level reloaded (0000:EBB5), its INF loaded where
/// one was named (F7B5..F7F5), the hook C3B8 (a dungeon's level
/// entered).
fn change_level(engine: &mut Engine, to: u8) -> Option<()> {
    use arena::movement::var::{LEVEL, LEVEL_HOOK};
    let m = &mut engine.m;
    if m.mem.r32(DS, arena::movement::var::STAIRS_HOOK) == arena::hooks::value::STAIRS {
        m.stairs_hook(&mut engine.files, &mut areas::Kept);
    }
    m.mem.w8(DS, LEVEL, to);
    m.level_change(&mut engine.files)?;
    let info = m.mem.r16(DS, arena::info::var::INFO_PTR);
    if info != 0 {
        m.info_load(&mut engine.files, info)?;
        m.mem.w16(DS, arena::info::var::INFO_PTR, 0);
    }
    if m.mem.r32(DS, LEVEL_HOOK) == arena::hooks::value::LEVEL_ENTERED {
        m.level_entered(&mut engine.files, &mut areas::Kept)?;
    }
    Some(())
}

/// A dungeon's levels gone through from the one entered (on to the
/// far end, then back the other way) until one holds the staff's flat
/// (C941, the INF's ITEM 13) in its walls, and it clicked.
fn take_staff(engine: &mut Engine, n: &str) -> Option<()> {
    use arena::movement::var::{LEVEL, LEVELS};
    let (start, levels) = (engine.m.mem.r8(DS, LEVEL), engine.m.mem.r8(DS, LEVELS));
    let order: Vec<u8> = if start == 0 { (0..levels).collect() } else { (0..=start).rev().chain(start + 1..levels).collect() };
    for l in order {
        while engine.m.mem.r8(DS, LEVEL) != l {
            let c = engine.m.mem.r8(DS, LEVEL);
            change_level(engine, if c > l { c - 1 } else { c + 1 })?;
        }
        let m = &mut engine.m;
        let staff = m.mem.r8(DS, 0xC941);
        if let Some(&sq) = squares(m, |w| w == 0x8000 | staff as u16).first() {
            println!("{n}. level {l}: the staff's flat {staff:#x} at {sq:04x}");
            click(engine, sq, staff)?;
            println!("   {}", flags(&engine.m));
            return Some(());
        }
    }
    println!("{n}. no staff's flat on any level");
    None
}

/// The stage's town: CITYDATA's place whose name is the stage's.
fn town(engine: &mut Engine) -> Option<(u8, u8)> {
    let m = &mut engine.m;
    let stage = m.mem.r8(DS, 0x0F77);
    let p = m.mem.r8(DS, 0x80EC + stage as u16);
    let name = nth(m, 0x80F4, stage);
    let here = m.mem.r8(DS, arena::travelmap::var::HERE_PROVINCE);
    m.city_load_province(&mut engine.files, p)?;
    let i = (0..0x20u8).find(|&i| m.mem.cstr(DS, arena::travelmap::CITIES + i as u16 * 0x19).eq_ignore_ascii_case(&name));
    m.city_load_province(&mut engine.files, here)?;
    println!("   the stage's town: {} in province {p}: {i:?}", String::from_utf8_lossy(&name));
    Some((p, i?))
}

/// Daytime (the buildings open): an hour at a time.
fn day(engine: &mut Engine) -> Option<()> {
    for _ in 0..24 {
        let t = engine.m.mem.r16(DS, 0xCCFE);
        if (0x100..0x2C0).contains(&t) {
            break;
        }
        engine.m.advance_minutes(&mut engine.files, 60, &mut arena::clock::NoClockHooks)?;
    }
    println!("   the time of day (CCFE) {:#x}", engine.m.mem.r16(DS, 0xCCFE));
    Some(())
}

/// The town's person visited.
fn visit(engine: &mut Engine, dir: &std::path::Path, run: &mut Run, n: &str) -> Option<()> {
    let (p, i) = town(engine)?;
    *run = trip(engine, dir, *run, p, i, n)?;
    day(engine)?;
    if !person(engine, dir, run)? {
        println!("   no person answered");
        return None;
    }
    Some(())
}

/// A fixed dungeon (CITYDATA entry 20h + `k`) of the stage's province,
/// its staff's flat taken, and left.
fn dungeon(engine: &mut Engine, dir: &std::path::Path, run: &mut Run, k: u8, n: &str) -> Option<()> {
    let stage = engine.m.mem.r8(DS, 0x0F77);
    let p = engine.m.mem.r8(DS, 0x80EC + stage as u16);
    *run = trip(engine, dir, *run, p, 0x20 + k, n)?;
    let at = engine.m.mem.r8(DS, arena::movement::var::LEVEL);
    take_staff(engine, n)?;
    leave(engine, dir, run, at)
}

/// From a new game's town (CITY:P:I, the player raised to `level`) or a
/// saved game (DIR[:SLOT]).
pub fn walk(dir: &std::path::Path, save: &str, level: u8) {
    if save.starts_with("CITY:") {
        let Some((mut engine, mut run)) = new_game(dir, save, level) else { return };
        println!("a new game: {}", flags(&engine.m));
        let _ = stages(&mut engine, dir, &mut run);
        return;
    }
    let v: Vec<&str> = save.split(':').collect();
    let (sdir, slot) = (std::path::Path::new(v[0]), v.get(1).map_or(1, |s| s.parse().expect("SLOT")));
    let mut engine = world::load(dir, "START.MIF");
    areas::start(&mut engine, false);
    let mut run = match areas::load_game(&mut engine, dir, sdir, slot) {
        Ok(r) => r,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    println!("loaded {run:?}: {}", flags(&engine.m));
    let _ = stages(&mut engine, dir, &mut run);
}

fn stages(engine: &mut Engine, dir: &std::path::Path, run: &mut Run) -> Option<()> {
    while engine.m.mem.r8(DS, 0x0F77) < 8 {
        let s = engine.m.mem.r8(DS, 0x0F77);
        println!("== stage {s}");
        if engine.m.mem.r8(DS, 0x0F76) == 0 {
            // No camping in town: out by its gate (0000:43A0) first.
            if engine.m.mem.r8(DS, arena::trigger::var::AREA) == 0 {
                let Some(&sq) = triggers(&engine.m, |i| matches!(i, 7 | 8)).first() else {
                    println!("   no gate");
                    return None;
                };
                door(engine, sq)?;
                *run = areas::next(engine, dir, *run)?;
                println!("   out by the gate at {sq:04x}: {run:?}");
            }
            rest(engine, &format!("{s}.1"));
            if engine.m.mem.r8(DS, 0x0F76) == 0 {
                println!("   no dream");
                return None;
            }
        }
        if engine.m.mem.r8(DS, 0x0F79) == 0 {
            visit(engine, dir, run, &format!("{s}.2"))?;
        }
        if engine.m.mem.r8(DS, 0x0F7D) == 0 {
            dungeon(engine, dir, run, 1, &format!("{s}.4"))?;
        }
        if engine.m.mem.r8(DS, 0x0F7B) == 0 {
            visit(engine, dir, run, &format!("{s}.5"))?;
        }
        dungeon(engine, dir, run, 0, &format!("{s}.6"))?;
        if engine.m.mem.r8(DS, 0x0F77) == s {
            println!("   the piece wasn't counted");
            return None;
        }
    }
    println!("== the eight pieces: {}", flags(&engine.m));
    end(engine, dir, run)
}

/// Stage 8 on: the Imperial City (province 8) travelled to, the
/// palace's door used (0000:4192: at stage 8 the audience, 0000:41A4,
/// into the palace's lower levels; at 9 the artifact's, text 5DDh), the
/// staff's flat taken there (2433:0BF4 at stage 8: D363 and, for the
/// eighth, D986) and the way out (the outer loop: 5B1 bit 100h, the
/// game's end).
fn end(engine: &mut Engine, dir: &std::path::Path, run: &mut Run) -> Option<()> {
    *run = trip(engine, dir, *run, 8, 0, "8")?;
    day(engine)?;
    let Some(&sq) = triggers(&engine.m, |i| (11..=13).contains(&i)).first() else {
        println!("   no palace door");
        return None;
    };
    println!("8. the palace's door at {sq:04x}: {}", flags(&engine.m));
    door(engine, sq)?;
    println!("   {} (5E6 {}, left {})", flags(&engine.m), engine.m.mem.r8(DS, 0x05E6), engine.m.mem.r16(DS, 0xC398) & 0x80 != 0);
    if engine.m.mem.r16(DS, 0xC398) & 0x80 == 0 {
        return Some(());
    }
    *run = areas::next(engine, dir, *run)?;
    let at = engine.m.mem.r8(DS, arena::movement::var::LEVEL);
    take_staff(engine, "8")?;
    println!("   5B1 {:04x}", engine.m.mem.r16(DS, 0x05B1));
    match leave(engine, dir, run, at) {
        Some(()) => println!("   after: {run:?}"),
        None => println!("   the way out ended the run (see above)"),
    }
    Some(())
}
