//! Differential tests of the player's weapon each frame (src/weapon.rs
//! `weapon_frame`, 0000:38CE) against vectors from the original code
//! (harness/gen_weapon.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::weapon::WeaponHooks;
use common::blow::Stream;

const HSEG: u16 = 0x6000;
const BUFSEG: u16 = 0x6800;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// The generator's hooks: the wall trigger and the bow count their calls
/// at FFF4 and FFF6; a building is closed when FFF8 bit 0 is set.
struct Count;

impl WeaponHooks for Count {
    fn wall_trigger(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        let v = m.mem.r16(DS, 0xFFF4).wrapping_add(1);
        m.mem.w16(DS, 0xFFF4, v);
        Some(())
    }
    fn building_closed(&mut self, m: &mut Machine, _kind: u8) -> Option<bool> {
        Some(m.mem.r8(DS, 0xFFF8) & 1 != 0)
    }
    fn fire_bow(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        let v = m.mem.r16(DS, 0xFFF6).wrapping_add(1);
        m.mem.w16(DS, 0xFFF6, v);
        Some(())
    }
}

/// Mirrors harness/gen_weapon.py `setup`; returns each frame's motion.
fn setup(m: &mut Machine, seed: u32) -> Vec<(u16, u16)> {
    let mut st = Stream(seed);
    let s = &mut st;
    let k = s.byte();
    let frame = if k < 0x80 {
        (s.byte() % 0x1E) as u16
    } else if k < 0xC0 {
        0x1E + (s.byte() % 6) as u16
    } else if k < 0xF0 {
        0x20
    } else {
        s.word()
    };
    m.mem.w16(DS, 0x1947, frame);
    let w = s.word() & 0x0054;
    let v = w | if s.byte() < 0xC0 { 0x10 } else { 0 };
    m.mem.w16(DS, 0x05E0, v);
    let v = s.byte() & 3;
    m.mem.w8(DS, 0xAFF9, v);
    let k = s.byte();
    let v = if k < 0x30 {
        6
    } else if k < 0x60 {
        7 + s.byte() % 3
    } else {
        s.byte() % 6
    };
    m.mem.w8(DS, 0x1012, v);
    let v = if s.byte() < 0x20 { s.byte() } else { s.byte() % 0x10 };
    m.mem.w8(DS, 0x0D49, v);
    let v = if s.byte() < 0xA0 { 0 } else { s.byte() % 8 };
    m.mem.w8(DS, 0x680E, v);
    let v = if s.byte() < 0xC0 { 0xFF } else { s.byte() % 12 };
    m.mem.w8(DS, 0x7F80, v);
    let v = s.byte() % 10;
    m.mem.w8(HSEG, 0x0B, v);
    let v = if s.byte() < 0xF0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x0620, v);
    let v = if s.byte() < 0xE0 { 0 } else { s.word() & 0x4010 };
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0x10 { 0x40 } else { 0 };
    m.mem.w16(DS, 0x05B1, v);
    let v = if s.byte() < 0x10 { 0x10 } else { 0 };
    m.mem.w16(DS, 0x05CA, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01D1, v);
    for at in [0xC3D0u16, 0xC3D4] {
        let v = 0x800 + s.word() % 0x4000;
        m.mem.w16(DS, at, v);
    }
    let v = s.word() & 0x1FF;
    m.mem.w16(DS, 0xC3DA, v);
    let v = if s.byte() < 0xE0 { 0x3C } else { 0 };
    m.mem.w16(DS, 0xC3D2, v);
    // The casting hands' graphic: a size, the runs' width, a place (y a
    // multiple of 10h: +8 is also 582C's flags), its bits a pixel, its
    // frames packed at 200h (zeros: runs of one byte).
    let (wd, ht) = (1 + (s.byte() % 0x40) as u16, 1 + (s.byte() % 0x40) as u16);
    let (x, y) = (s.word() % 0x160, ((s.byte() % 0xD) as u16) << 4);
    for (i, v) in [wd, ht, wd, x, y].into_iter().enumerate() {
        m.mem.w16(HSEG, 2 * i as u16, v);
    }
    let v = if s.byte() < 0xC0 { 8 } else { 4 };
    m.mem.w8(HSEG, 0x0A, v);
    for f in 0..10u16 {
        m.mem.w16(HSEG, 0x0C + 2 * f, 0x200);
    }
    let n = 1 + s.byte() % 12;
    (0..n)
        .map(|_| {
            let k = s.byte();
            if k < 0x60 {
                (0, 0)
            } else if k < 0xE0 {
                let dx = (s.byte() as u16).wrapping_sub(0x80).wrapping_mul(2);
                let dy = (s.byte() as u16).wrapping_sub(0x80).wrapping_mul(2);
                (dx, dy)
            } else {
                (s.word(), s.word())
            }
        })
        .collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x902E, 0);
    base.mem.w16(DS, 0x9030, HSEG);
    base.mem.w16(DS, 0xBA54, 0);
    base.mem.w16(DS, 0xBA56, BUFSEG);
    base.mem.w16(DS, 0xD806, 0x6800);
    base.mem.w16(DS, 0x1042, 0x7800);
    base.mem.w16(DS, 0x91F0, 0);
    let mut files = Files::default();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/weapon.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "weapon", "{line}");
        let mut m = base.clone();
        let motions = setup(&mut m, hex(f[1]) as u32);
        m.load_rng();
        let before = m.clone();
        let mut got = Some(());
        for &mo in &motions {
            m.mouse_motion = mo;
            got = got.and_then(|()| m.weapon_frame(&mut files, &mut Count));
        }
        let got = got.map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!("{:x}", fnv(changed))
        });
        if std::env::var("WEAPON_DUMP").is_ok_and(|v| v == f[1]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(if f[2] == "fault" { "fault" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 2500);
}
