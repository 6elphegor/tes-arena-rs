//! Differential tests of the hook CD42's part indoors (src/hooks.rs
//! `area_entered_inside`, 0000:4767..47D6) against vectors from the
//! original code (harness/gen_inside.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;
use common::level::*;

/// Mirrors harness/gen_inside.py `extras` (a tuple's items before its
/// index; a conditional's test byte first).
fn extras(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed ^ 0x5EED_1234);
    let s = &mut st;
    let v = s.word() & !0x4000;
    let v = v | if s.byte() < 0x20 { 0x4000 } else { 0 };
    m.mem.w16(DS, 0x05CA, v);
    let k = s.byte() % 5;
    let any = s.byte();
    m.mem.w8(DS, 0x05E6, [4, 4, 3, 3, any][k as usize]);
    let k = s.byte() % 4;
    let any = s.byte();
    m.mem.w8(DS, 0x0D48, [8, 8, 1, any][k as usize]);
    let v = s.byte();
    m.mem.w8(DS, 0x0A52, v);
    for i in 0..8u16 {
        if s.byte() < 0x30 {
            m.mem.w16(DS, 0xC6E5 + 2 * i, 0xFFFF);
        }
    }
    if s.byte() < 0x20 {
        m.mem.w8(DS, 0x0A47, 3);
        let v = s.byte() & 0xE0;
        m.mem.w8(DS, 0x0A4C, v);
        m.mem.w8(DS, 0x0A46, 4);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0xCCD0, FSEG);
    let mut n = 0;
    for line in include_str!("vectors/inside.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "inside", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        setup_level(&mut m, seed);
        extras(&mut m, seed);
        m.load_rng();
        let before = m.clone();
        let got = m.area_entered_inside(&mut Files::default(), &mut Counted { level: true }).map(|_| {
            m.store_rng();
            if std::env::var("INSIDE_DUMP").ok().as_deref() == Some(f[1]) {
                let d: Vec<String> = (0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(WSEG, o) != before.mem.r8(WSEG, o))
                    .map(|o| format!("{o:04x}:{:02x}>{:02x}", before.mem.r8(WSEG, o), m.mem.r8(WSEG, o)))
                    .collect();
                eprintln!("{}", d.join(" "));
            }
            let segs: String = [WSEG, FSEG, CSEG]
                .iter()
                .map(|&sg| format!(" {:x}", fnv((0..=0xFFFFu16).map(|o| m.mem.r8(sg, o)))))
                .collect();
            format!("{:x}{segs}", changed(&m, &before))
        });
        match got {
            Some(g) => assert_eq!(f[2], g, "{line}"),
            None => assert!(f[2] == "fault" || f[2] == "hang", "{line}: the port gave None"),
        }
        n += 1;
    }
    assert!(n > 2500, "{n} cases");
}
