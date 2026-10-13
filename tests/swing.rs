//! Differential tests of a landing swing (src/strike.rs `swing_hits`)
//! against vectors from the original code (harness/gen_swing.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{setup, strike, Stream, ACTORS, CSEG, FRAME, RSIZE, SIZE, WSEG};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

/// Mirrors harness/gen_swing.py `spread`.
fn spread(m: &mut Machine, s: &mut Stream, corpse: u8) {
    let (px, py) = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
    for i in 0..23 {
        let si = ACTORS + i * SIZE;
        if s.byte() < 0x20 {
            continue;
        }
        let x = px.wrapping_add(s.byte() as u16 * 2).wrapping_sub(0x100);
        let y = py.wrapping_add(s.byte() as u16 * 2).wrapping_sub(0x100);
        let t = s.byte();
        let sprite = match t {
            0..=0xF => 0xC5,
            0x10..=0x1F => corpse,
            _ => 0xC0 + t % 5,
        };
        let flags = if s.byte() < 0x30 { s.word() & 0x7FFF } else { s.word() & 0x3FFF };
        m.mem.w16(DS, si, x);
        m.mem.w16(DS, si + 2, 0);
        m.mem.w16(DS, si + 4, y);
        m.mem.w8(DS, si + 0xC, sprite);
        m.mem.w16(DS, si + 0xF, flags);
        if i >= 15 {
            m.mem.w16(DS, si + 0x12, (i - 15) * RSIZE);
        }
    }
    let h = s.word() & 0x1FF;
    m.mem.w16(DS, 0xC3DA, h);
    let height = if s.byte() < 0xE0 { 0x3C } else { s.word() };
    m.mem.w16(DS, 0xC3D2, height);
    let n = if s.byte() < 0x80 { 0 } else { (s.byte() % 8) as u16 };
    m.mem.w16(DS, 0x0FF9, n);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    let classes: Vec<u8> = (0..18).map(|i| base.mem.r8(DS, 0x2A50 + i)).collect();
    let corpse = (base.mem.r8(DS, 0xC936) as u16).wrapping_add(base.mem.r16(DS, 0xC38E)) as u8;
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/swing.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "swing", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let (seed, k) = (hex(p[0]) as u32, hex(p[1]) as u16);
        let mut m = base.clone();
        let mut s = setup(&mut m, seed, k, &classes);
        strike(&mut m, &mut s, k * RSIZE);
        spread(&mut m, &mut s, corpse);
        m.load_rng();
        let before = m.clone();
        let got = m.swing_hits().map(|reached| {
            m.store_rng();
            // The original goes on to the wall (stubbed) when nothing was hit.
            assert_eq!(reached, m.mem.r16(DS, 0x67DA) != 0 || m.mem.r16(DS, 0xC3D2) as i16 <= 0);
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
        if std::env::var("SWING_DUMP").is_ok_and(|v| v == p[0]) {
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
        let hits = m.mem.r16(DS, 0x67DA);
        *counts.entry(if f[2] == "fault" { "fault" } else if hits > 1 { "several" } else if hits == 1 { "one" } else { "none" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("several").copied().unwrap_or(0) > 100);
}
