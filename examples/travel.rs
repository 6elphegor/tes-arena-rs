//! Plans a journey with the ported code: days, distance and the date of
//! arrival between two locations.
//!
//! Usage: travel GAME_DIR FROM_PROVINCE FROM_INDEX TO_PROVINCE TO_INDEX
//!
//! Locations are entries of the game's CITYDATA.65 (index 0..47 in the
//! province's list). The journey starts on the first day of 3E 389, in
//! fair weather.

use arena::bsa::Files;
use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        eprintln!("usage: travel GAME_DIR FROM_PROVINCE FROM_INDEX TO_PROVINCE TO_INDEX");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let n = |i: usize| -> u16 { args[i].parse().expect("number") };
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(&dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buf, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x9030, buf);
    m.mem.w8(DS, 0x1A6C, 65);
    // A location's name and position from its province's list.
    let mut place = |m: &mut Machine, prov: u16, idx: u16| {
        m.city_load_province(&mut files, prov as u8).expect("CITYDATA.65");
        let e = 0x1445 + idx * 25;
        let name = String::from_utf8_lossy(m.mem.cstr(DS, e)).into_owned();
        let province = String::from_utf8_lossy(m.mem.cstr(DS, 0x1429)).into_owned();
        (format!("{name}, {province}"), m.mem.r16(DS, e + 0x14), m.mem.r16(DS, e + 0x16))
    };
    let (from, fx, fy) = place(&mut m, n(2), n(3));
    let (to, tx, ty) = place(&mut m, n(4), n(5));
    m.mem.w8(DS, 0x0A46, n(2) as u8);
    m.mem.w16(DS, 0x0A48, fx);
    m.mem.w16(DS, 0x0A4A, fy);
    m.mem.w8(DS, 0x193D, n(4) as u8);
    m.mem.w16(DS, 0x193E, tx);
    m.mem.w16(DS, 0x1940, ty);
    let days = m.travel_days(&mut files).expect("travel");
    let km = m.mem.r16(DS, 0x3FF9);
    let minutes = m.mem.r32(DS, 0x3FF2);
    m.log_date(minutes, 0xF000);
    let arrival = String::from_utf8_lossy(m.mem.cstr(DS, 0xF000)).trim_end().to_string();
    println!("{from} -> {to}: {km} km, {days} days; leaving on the 1st of Morning Star 3E 389, you arrive {arrival}");
}
