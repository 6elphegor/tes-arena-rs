//! Differential tests of projectiles (src/projectile.rs) against
//! vectors from the original code (harness/gen_projectile.py): a launch
//! and some frames of the projectiles in a seeded world.
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, setup, strike, Stream, ACTORS, CSEG, FRAME, RSIZE, SIZE, WSEG};

const SPSEG: u16 = 0x6000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

struct Plan {
    launch: bool,
    actor: u16,
    cast: u16,
    dx: u16,
    frames: u16,
}

fn raw(s: &mut Stream, n: u16) -> Vec<u8> {
    (0..n).map(|_| s.byte()).collect()
}

/// Mirrors harness/gen_projectile.py `flight`.
fn flight(m: &mut Machine, s: &mut Stream) -> Plan {
    let px = 0x2000 + (s.word() & 0xFFF);
    let py = 0x2000 + (s.word() & 0xFFF);
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let mut spots = Vec::new();
    for i in 0..23u16 {
        let si = ACTORS + i * SIZE;
        let x = px.wrapping_add(s.byte() as u16 * 4).wrapping_sub(0x200);
        let y = py.wrapping_add(s.byte() as u16 * 4).wrapping_sub(0x200);
        spots.push((x, y));
        let z = s.word() & 0x3FF;
        let t = s.byte();
        let sprite = if t < 0x10 { 0xC5 } else { 0xC0 + t % 5 };
        let flags = s.word() & 0x3FFF | if s.byte() < 0x20 { 0x4000 } else { 0 };
        for (o, v) in [(0, x), (2, z), (4, y), (6, 0xC3D0)] {
            m.mem.w16(DS, si + o, v);
        }
        let h = s.word() & 0x1FF;
        m.mem.w16(DS, si + 0xA, h);
        m.mem.w8(DS, si + 0xC, sprite);
        m.mem.w16(DS, si + 0xF, flags);
        if i >= 15 {
            m.mem.w16(DS, si + 0x12, (i - 15) * RSIZE);
        }
        let sq = if s.byte() < 0xE0 { (y >> 7) << 8 | (((x >> 7) * 2) & 0xFF) } else { 0xFFFF };
        m.mem.w16(DS, 0x1951 + 2 * i, sq);
    }
    m.mem.w16(DS, 0xCB90, ((py >> 7) << 8).wrapping_add((px >> 7) * 2));
    for dy in -8i32..8 {
        for dx in -8i32..8 {
            let sq = ((((py >> 7) as i32 + dy) << 8) + ((px >> 7) as i32 + dx) * 2) as u16;
            let t = s.byte();
            let v = match t {
                0..=0xAF => 0,
                0xB0..=0xBF => 0xB000 | s.word() & 0xFFF,
                0xC0..=0xC7 => 0xA000 | s.word() & 0xFFF,
                0xC8..=0xCF => 0x9000 | s.word() & 0x1FF,
                0xD0..=0xD7 => 0x8000 | s.byte() as u16,
                0xD8..=0xDB => 0xD000 | s.byte() as u16,
                _ => s.word() & 0x7FFF,
            };
            m.mem.w16(WSEG, sq, v);
        }
    }
    for i in 0..32 {
        let e = 0xBB0C + 6 * i;
        if s.byte() < 0xA0 {
            m.mem.w16(DS, e, 0x8000);
        } else {
            let x = s.word() & 0x7FFF;
            let (y, r) = (s.word(), s.word());
            m.mem.w16(DS, e, x);
            m.mem.w16(DS, e + 2, y);
            m.mem.w16(DS, e + 4, r);
        }
    }
    let n = (s.byte() % 0x22) as u16;
    m.mem.w16(DS, 0xBC8C, n);
    for i in 0..0x80 {
        if s.byte() < 0x30 {
            let e = 0xD43C + 6 * i;
            let a = s.word();
            let b = s.word() | 1;
            m.mem.w16(DS, e, 0);
            m.mem.w16(DS, e + 2, a);
            m.mem.w16(DS, e + 4, b);
        }
    }
    let b = raw(s, 0x60);
    put(m, DS, 0xCA97, &b);
    let v = s.word() & 4;
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & 0x200;
    m.mem.w16(DS, 0xC39A, v);
    for c in 0..8u16 {
        let sp = c * 0x55;
        let b = raw(s, 0x55);
        put(m, SPSEG, sp, &b);
        let ty = s.byte() % 5;
        m.mem.w8(SPSEG, sp + 0x24, ty);
        let el = s.byte() % 6;
        m.mem.w8(SPSEG, sp + 0x26, el);
        let r = (s.byte() % 8) as u16 * RSIZE;
        let lv = s.byte() as u16;
        let fl = s.word() & 0x21;
        for (o, v) in [(0, sp), (2, SPSEG), (4, r), (6, CSEG), (8, lv), (0xA, fl)] {
            m.mem.w16(DS, 0x7F86 + c * 0x16 + o, v);
        }
    }
    for i in 0..12u16 {
        let sl = 0x7DBC + i * 0x1C;
        let b = raw(s, 0x1C);
        put(m, DS, sl, &b);
        if s.byte() < 0x60 && i < 11 {
            let mut x = px.wrapping_add(s.byte() as u16 * 8).wrapping_sub(0x400);
            let mut y = py.wrapping_add(s.byte() as u16 * 8).wrapping_sub(0x400);
            if s.byte() < 0x80 {
                (x, y) = spots[s.byte() as usize % 23];
            }
            let cast = if s.byte() < 0x60 { 0xFFFE } else { 0x7F86 + (s.byte() % 8) as u16 * 0x16 };
            let heading = if s.byte() < 0x20 { 0xFFFF } else { (s.word() & 0x1FF) * 2 };
            let z = s.word() & 0xFF;
            let light = 0xBB0C + 6 * (s.byte() % 32) as u16;
            for (o, v) in [(0, x), (2, z), (4, y), (6, light), (0xA, heading)] {
                m.mem.w16(DS, sl + o, v);
            }
            let fl = s.word() & 0x1800 | 0x200;
            m.mem.w16(DS, sl + 0xF, fl);
            let life = 1 + s.byte() % 0x1E;
            m.mem.w8(DS, sl + 0x11, life);
            m.mem.w16(DS, sl + 0x12, cast);
            let d = (s.byte() % 3) as u16;
            m.mem.w16(DS, sl + 0x1A, d);
        } else {
            m.mem.w8(DS, sl + 0x11, 0);
        }
    }
    let t = s.byte();
    let occ = if t < 0x40 {
        0xFFFF
    } else if t < 0x80 {
        0x1A4
    } else {
        (s.byte() % 23) as u16
    };
    m.mem.w16(DS, 0x7A44, occ);
    let c = (s.byte() % 12) as u16;
    m.mem.w16(DS, 0x7DB8, c);
    let launch = s.byte() < 0xE0;
    let slot = if s.byte() < 0xC0 { 15 + s.byte() % 8 } else { s.byte() % 23 };
    let actor = ACTORS + slot as u16 * SIZE;
    let cast = 0x7F86 + (s.byte() % 8) as u16 * 0x16;
    let dx = s.word();
    let frames = 1 + (s.byte() % 30) as u16;
    Plan { launch, actor, cast, dx, frames }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    // The timed-spell-flag table (harness/gen_projectile.py TSEG).
    base.mem.w16(DS, 0x1020, 0);
    base.mem.w16(DS, 0x1022, 0x5C00);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/projectile.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "flight", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        strike(&mut m, &mut s, k * RSIZE);
        let plan = flight(&mut m, &mut s);
        m.load_rng();
        let before = m.clone();
        let run = |m: &mut Machine| -> Option<()> {
            if plan.launch {
                m.launch(plan.cast, plan.actor, plan.dx)?;
            }
            for _ in 0..plan.frames {
                m.projectiles_update()?;
            }
            Some(())
        };
        let got = run(&mut m).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("FLIGHT_DUMP").is_ok_and(|v| v == p[0]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
            for o in 0..0x8000u16 {
                if m.mem.r8(CSEG, o) != before.mem.r8(CSEG, o) {
                    eprintln!("c{o:04x} {:02x} {:02x}", before.mem.r8(CSEG, o), m.mem.r8(CSEG, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(match f[2] {
            "fault" => "fault",
            "hang" => "hang",
            _ => "ran",
        })
        .or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 1000);
}

/// Arrow shots (29CF:037F, harness/gen_missile.py): the same world, a
/// shot in place of the launch, then the frames.
#[test]
fn missiles_match_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    base.mem.w16(DS, 0xC3D6, 0xA0);
    base.mem.w16(DS, 0x1020, 0);
    base.mem.w16(DS, 0x1022, 0x5C00);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/missile.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "missile", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let (src, mx, flat) = (hex(p[2]) as u16, hex(p[3]) as u16, hex(p[4]) as u8);
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        strike(&mut m, &mut s, k * RSIZE);
        let plan = flight(&mut m, &mut s);
        m.mem.w16(DS, 0x8F92, mx);
        m.load_rng();
        let before = m.clone();
        let run = |m: &mut Machine| -> Option<()> {
            m.fire_missile(flat, src)?;
            for _ in 0..plan.frames {
                m.projectiles_update()?;
            }
            Some(())
        };
        let got = run(&mut m).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("MISSILE_DUMP").is_ok_and(|v| v == p[0]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            _ if f[2] == "invalid" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(match f[2] {
            "fault" => "fault",
            "hang" => "hang",
            _ => "ran",
        })
        .or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 1000);
}

/// A spell's ball of light from the player (29CF:0270,
/// harness/gen_lightshot.py): the missiles' world, the ball in place of
/// the launch, then the frames.
#[test]
fn light_shots_match_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    base.mem.w16(DS, 0xC3D6, 0xA0);
    base.mem.w16(DS, 0x1020, 0);
    base.mem.w16(DS, 0x1022, 0x5C00);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/lightshot.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "lightshot", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k, ax) = (hex(p[0]) as u32, hex(p[1]) as u16, hex(p[2]) as u16);
        let (height, heading) = (hex(p[3]) as u16, hex(p[4]) as u16);
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        strike(&mut m, &mut s, k * RSIZE);
        let plan = flight(&mut m, &mut s);
        m.mem.w16(DS, 0xC3D2, height);
        m.mem.w16(DS, 0xC3DA, heading);
        m.load_rng();
        let before = m.clone();
        let run = |m: &mut Machine| -> Option<()> {
            m.light_missile(ax)?;
            for _ in 0..plan.frames {
                m.projectiles_update()?;
            }
            Some(())
        };
        let got = run(&mut m).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        match got {
            _ if f[2] == "hang" || f[2] == "invalid" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(match f[2] {
            "fault" => "fault",
            "hang" => "hang",
            _ => "ran",
        })
        .or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 1000);
}

/// The player casting (29CF:071F then 29CF:0000, harness/gen_cast.py):
/// the same world, a random player spell cast from the spellbook, and a
/// waiting cast resolved by a touch.
#[test]
fn casts_match_original() {
    use arena::cast::{NoCastHooks, Touch};
    let Some(img) = common::image() else { return };
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena"));
    let Ok(mut files) = arena::bsa::Files::new(&dir) else { return };
    let mut base = Machine::new(&img);
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    base.mem.w16(DS, 0xC3D6, 0xA0);
    base.mem.w16(DS, 0x1020, 0);
    base.mem.w16(DS, 0x1022, 0x5C00);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/cast.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "cast", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        if std::env::var("CAST_DUMP").is_ok_and(|v| v != p[0]) {
            continue;
        }
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let (idx, touch, slot, mx) = (hex(p[2]) as u8, hex(p[3]), hex(p[4]) as u16, hex(p[5]) as u16);
        let (sp, flags, town) = (hex(p[6]) as u16, hex(p[7]) as u16, hex(p[8]) as u8);
        let spell: Vec<u8> = (0..p[9].len() / 2).map(|i| u8::from_str_radix(&p[9][2 * i..2 * i + 2], 16).unwrap()).collect();
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        strike(&mut m, &mut s, k * RSIZE);
        let plan = flight(&mut m, &mut s);
        m.mem.w16(DS, 0x8F92, mx);
        m.mem.w16(DS, 0x020A, sp);
        let v = m.mem.r16(DS, 0x05BE) & !0x200 | flags;
        m.mem.w16(DS, 0x05BE, v);
        m.mem.w8(DS, 0x05E6, town);
        for (i, &b) in spell.iter().enumerate() {
            m.mem.w8(arena::mem::DATA, 0x2A80 + 0x55 * idx as u16 + i as u16, b);
        }
        if std::env::var("CAST_DUMP").is_ok() {
            eprintln!("lvl {} sp {:#x}", m.mem.r8(DS, 0x1AA), m.mem.r16(DS, 0x20A));
        }
        m.load_rng();
        let before = m.clone();
        let run = |m: &mut Machine, files: &mut arena::bsa::Files| -> Option<()> {
            m.cast_spell(files, &mut NoCastHooks, 0x80 | idx)?;
            if m.mem.r16(DS, 0x0620) != 0 {
                let t = if touch == 0 { Touch::Actor(slot) } else { Touch::Record(DS, 0x1A4) };
                m.resolve_cast(files, t)?;
            }
            for _ in 0..plan.frames {
                m.projectiles_update()?;
            }
            Some(())
        };
        let got = run(&mut m, &mut files).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("CAST_DUMP").is_ok_and(|v| v == p[0]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" || f[2] == "invalid" || f[2] == "lowmem" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(match f[2] {
            "fault" => "fault",
            "hang" => "hang",
            _ => "ran",
        })
        .or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 1500);
}
