//! Lists the actor slots in use after entering a level and summoning
//! (`people` as a third argument: people), then runs frames until one
//! fails or 200 pass.
#![allow(dead_code)]
#[path = "../src/world.rs"]
mod world;
#[path = "../src/flats.rs"]
mod flats;
#[path = "../src/archive.rs"]
mod archive;
use arena::mem::DS;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut e = world::load(std::path::Path::new(&a[1]), &a[2]);
    {
        let m = &e.m;
        println!("area {:#x} dungeon kind {:#x} 191D {:#x} hour {:#x} 5CA {:#x} FF7 {:#x} FFB {:#x} building {}", m.mem.r8(DS, 0x5E6), m.mem.r8(DS, 0xD48), m.mem.r8(DS, 0x191D), m.mem.r16(DS, 0x5D6), m.mem.r16(DS, 0x5CA), m.mem.r16(DS, 0xFF7), m.mem.r16(DS, 0xFFB), 0);
    }
    let t: Vec<u8> = (0..12).map(|i| e.m.mem.r8(DS, 0x6810 + i)).collect();
    println!("chances {t:?} level {}", e.m.mem.r8(DS, 0x01AA));
    if a.get(3).map(String::as_str) == Some("people") {
        world::summon_people(&mut e);
    } else {
        world::summon(&mut e);
    }
    println!("after: FF7 {:#x} FF9 {:#x} FIGHT {:#x}", e.m.mem.r16(DS, 0xFF7), e.m.mem.r16(DS, 0xFF9), e.m.mem.r16(DS, 0xFFF));
    let m = &e.m;
    println!("names: {:?}", world::creature_names(m));
    let seg = m.mem.r16(DS, 0x1042);
    for slot in 0..23u16 {
        let si = 0x0AB3 + slot * 0x1C;
        let rec = m.mem.r16(DS, si + 0x12);
        let flags = m.mem.r16(DS, si + 0xF);
        if flags & 0x4000 != 0 { continue; }
        println!("slot {slot:2}: x {:#06x} z {:#06x} y {:#06x} sprite {:02x} frame {:02x} flags {flags:04x} rec {rec:04x} type {}",
            m.mem.r16(DS, si), m.mem.r16(DS, si + 2), m.mem.r16(DS, si + 4), m.mem.r8(DS, si + 0xC), m.mem.r8(DS, si + 0xD), m.mem.r8(seg, rec + 4));
    }
    println!("people's list: {:?}", (0..64u16).map(|i| e.m.mem.r8(DS, 0x1044 + i)).map(|c| if c == 0 { ',' } else { c as char }).collect::<String>());
    struct Front;
    impl arena::controls::ControlHooks for Front {}
    impl arena::weapon::WeaponHooks for Front {}
    impl arena::clock::ClockHooks for Front {}
    impl arena::frame::FrameHooks for Front {}
    impl arena::engine::EngineHooks for Front {}
    for n in 0..200 {
        let t = e.frame_ticks();
        e.tick(t);
        if e.frame(arena::engine::Input::Steer(&Default::default()), &mut Front).is_none() {
            println!("frame {n} met code not ported");
            break;
        }
    }
}
