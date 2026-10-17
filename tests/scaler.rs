//! Differential test of the flats' scaler (src/render.rs `blit_rows`:
//! 0000:78A8 with its rows compiled by 7A2B, or by 9546 after 953E, a
//! light's) against vectors from the original code
//! (harness/gen_scaler.py): the page after, and the row's code it
//! compiled at DGROUP 929E.
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const SSEG: u16 = 0x7000;
const TSEG: u16 = 0x6000;
const PSEG: u16 = 0x8000;
const SRCSEED: u32 = 0x5CA1_E5EE;
const TABSEED: u32 = 0x7AB1_E5EE;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_scaler.py `fixed`.
fn fixed(m: &mut Machine) {
    let mut s = Stream(SRCSEED);
    for o in 0..=0xFFFFu16 {
        let b = s.byte();
        let v = if b < 0x55 { 0 } else { s.byte() };
        m.mem.w8(SSEG, o, v);
    }
    let mut s = Stream(TABSEED);
    for i in 0..0x10100u32 {
        let b = s.byte();
        m.mem.w8(TSEG - 0x10 + (i >> 4) as u16, (i & 0xF) as u16, b);
    }
    m.mem.w16(DS, 0xA900, PSEG);
    for y in 0..0xC8u16 {
        m.mem.w16(DS, 0xA6A0 + 2 * y, y * 0x140);
    }
}

/// Mirrors harness/gen_scaler.py `setup`: (light, window, arguments, odd).
fn setup(seed: u32) -> (bool, [u16; 4], [u16; 14], u8) {
    let mut s = Stream(seed);
    let light = s.byte() & 1 != 0;
    let (left, top) = ((s.byte() % 0x60) as u16, (s.byte() % 0x40) as u16);
    let right = left + (s.byte() as u16 % (0x140 - left));
    let bottom = top + (s.byte() as u16 % (0xC8 - top));
    let w = 1 + (s.byte() % 0x60) as u16;
    let h = 1 + (s.byte() % 0x60) as u16;
    let k = s.byte() % 4;
    let dw = if k == 0 { 1 + (s.byte() % 0x20) as u16 } else { 1 + (s.byte() % 0xC0) as u16 };
    let dh = if k == 1 { 1 + (s.byte() % 0x20) as u16 } else { 1 + (s.byte() % 0xC0) as u16 };
    let x = (left + s.byte() as u16).wrapping_sub(0x30);
    let y = (top + (s.byte() % 0x80) as u16).wrapping_sub(0x20);
    let bpp = if s.byte() < 0xA0 { 8 } else { 1 + (s.byte() % 7) as u16 };
    let mut flags = (s.byte() & 3) as u16;
    if light {
        flags |= 1;
    }
    let mut shade = if s.byte() < 0xC0 { (TSEG, s.word()) } else { (0, 0) };
    let cmap = if bpp != 8 && s.byte() < 0xC0 { (TSEG, s.word()) } else { (0, 0) };
    if light && bpp != 8 && cmap.0 != 0 && shade.0 != 0 {
        shade = (0, 0);
    }
    let srcoff = s.word() & 0x7FFF;
    let odd = if s.byte() < 0x40 { s.byte() & 1 } else { 0 };
    let args = [x, y, dw, dh, shade.1, shade.0, w, h, bpp, flags, srcoff, SSEG, cmap.1, cmap.0];
    (light, [left, top, right, bottom], args, odd)
}

/// Mirrors harness/gen_scaler.py `page_of`.
fn page_of(seed: u32) -> Vec<u8> {
    let mut s = Stream(seed ^ 0xFA6E_0000);
    (0..0x10000).map(|_| s.byte()).collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    fixed(&mut base);
    let (mut lit, mut plain, mut same, mut faults) = (0, 0, 0, 0);
    for line in include_str!("vectors/scaler.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let (light, win, args, odd) = setup(seed);
        if f[2] == "fault" {
            assert!(args[2] == 0 || args[3] == 0, "{line}: the original faults");
            faults += 1;
            continue;
        }
        let mut m = base.clone();
        for (o, v) in [0xA914, 0xA916, 0xA918, 0xA91A].into_iter().zip(win) {
            m.mem.w16(DS, o, v);
        }
        m.mem.w16(0, 0x789E, if light { 0x9546 } else { 0x7A2B });
        let before = page_of(seed);
        let mut page = before.clone();
        arena::render::scaler_alone(&mut m, &mut page, args, odd).unwrap_or_else(|| panic!("{line}: not ported"));
        assert_eq!(f[2], format!("{:x}", fnv(page.iter().copied())), "{line}: light {light} {args:x?} odd {odd}");
        // The row's code 78A8 compiled, left at 929E.
        let rows = (0..0x1000u16).map(|i| m.mem.r8(DS, 0x929E + i));
        assert_eq!(f[3], format!("{:x}", fnv(rows)), "{line}: the row's code: light {light} {args:x?}");
        if page == before {
            same += 1;
        } else if light {
            lit += 1;
        } else {
            plain += 1;
        }
    }
    eprintln!("scaler: {lit} lit, {plain} plain, {same} nothing drawn, {faults} faults");
    assert!(lit > 500 && plain > 500, "{lit} lit, {plain} plain");
}
