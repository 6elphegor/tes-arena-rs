//! Differential test of a square's cap as the 3D mode draws it
//! (src/render/poly.rs: 0000:CBFF through B2E5, FF26 and the polygon
//! drawer 7C51) against vectors from the original code
//! (harness/gen_capdraw.py): the page and DGROUP's changes.
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const TSEG: u16 = 0x7000;
const RSEG: u16 = 0x6000;
const TEXSEED: u32 = 0x0CA9_0D7A;
const REMAPSEED: u32 = 0x5EED_0A0E;
/// What the digest of DGROUP's changes leaves out: the stack 85AE moves
/// to, the caller's SS:SP it keeps.
const LEFT_OUT: [(u16, u16); 2] = [(0x9600, 0x9750), (0xAF8B, 0xAF8F)];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_capdraw.py `fixed` (the page is the drawing's
/// own, EEh).
fn fixed(m: &mut Machine) {
    let mut s = Stream(TEXSEED);
    for o in 0..=0xFFFFu16 {
        let b = s.byte();
        m.mem.w8(TSEG, o, b);
    }
    let mut s = Stream(REMAPSEED);
    for o in 0..0x100u16 {
        let b = s.byte();
        m.mem.w8(RSEG, o, b);
    }
    m.mem.w8(DS, 0x8FA6, 0);
    m.mem.w16(DS, 0x9030, TSEG);
    m.mem.w16(DS, 0xA900, 0x8000);
    m.mem.w8(DS, 0xBA80, 1);
    for y in 0..0xC8u16 {
        m.mem.w16(DS, 0xA6A0 + 2 * y, y * 0x140);
    }
    for (o, v) in [(0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0xC7), (0xC3D6, 0x9F), (0xCCF4, 0xA0), (0xCCF6, 0xA0)] {
        m.mem.w16(DS, o, v);
    }
}

/// Mirrors harness/gen_capdraw.py `toward`.
fn toward(dx: i32, dz: i32) -> i32 {
    let (ax, az) = (dx.abs(), dz.abs());
    let a = if ax <= az { ax * 64 / az.max(1) } else { 128 - az * 64 / ax };
    if dx >= 0 {
        if dz >= 0 { a } else { 256 - a }
    } else if dz < 0 {
        256 + a
    } else {
        512 - a
    }
}

/// Mirrors harness/gen_capdraw.py `setup`: (DGROUP words, AX, CX).
fn setup(seed: u32) -> (Vec<(u16, u16)>, u16, u16) {
    let mut s = Stream(seed);
    let (col, row) = ((s.byte() & 0x7F) as u16, (s.byte() & 0x7F) as u16);
    let px = (col << 7) + (s.byte() & 0x7F) as u16;
    let pz = (row << 7) + (s.byte() & 0x7F) as u16;
    let heading = s.word() & 0x1FF;
    let eye = (s.byte() as u16).wrapping_sub(0x40);
    let centre = 0x30 + (s.byte() % 0x40) as u16;
    let sc = ((col as i32 + (s.byte() % 7) as i32 - 3) & 0x7F) as u16;
    let sr = ((row as i32 + (s.byte() % 7) as i32 - 3) & 0x7F) as u16;
    let height = (s.word() % 0x400).wrapping_sub(0x200);
    let wind = (s.byte() & 1) as u16;
    let tex = 1 + (s.byte() % 45) as u16;
    let bank = (s.byte() % 16) as u16;
    let remap = if s.byte() % 4 == 0 { RSEG } else { 0 };
    let (mut sc, mut sr, mut height) = (sc, sr, height);
    if s.byte() % 3 == 0 {
        sc = ((col as i32 + (s.byte() % 3) as i32 - 1) & 0x7F) as u16;
        sr = ((row as i32 + (s.byte() % 3) as i32 - 1) & 0x7F) as u16;
        height = ((eye as i32 + (s.byte() % 0x41) as i32 - 0x20) & 0xFFFF) as u16;
    }
    let mut heading = heading;
    if s.byte() % 4 != 0 {
        let dx = (sc << 7) as i32 + 0x40 - px as i32;
        let dz = (sr << 7) as i32 + 0x40 - pz as i32;
        heading = ((toward(dx, dz) + (s.byte() % 97) as i32 - 48) & 0x1FF) as u16;
    }
    let words = vec![(0xC3D0, px), (0xC3D4, pz), (0xC3DA, heading), (0xC3D2, eye), (0xC3D8, centre), (0xA84A, sc), (0xA84E, sr), (0xD804, tex), (0xC8F0, bank), (0xAFEE, remap)];
    (words, height, wind)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    fixed(&mut base);
    let (mut drawn, mut blank, mut faults) = (0, 0, 0);
    let blank_page = fnv(std::iter::repeat_n(0xEEu8, 0x10000));
    for line in include_str!("vectors/capdraw.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let (words, ax, cx) = setup(seed);
        for &(o, v) in &words {
            m.mem.w16(DS, o, v);
        }
        let start = m.mem.segment(DS).to_vec();
        let (m, page, gaps) = arena::render::cap_alone(m, vec![0xEE; 0x10000], ax, cx);
        if f[2] == "fault" {
            assert!(!gaps.is_empty(), "{line}: the original faults, the port drew");
            faults += 1;
            continue;
        }
        assert!(gaps.is_empty(), "{line}: {gaps:?}");
        let after = m.mem.segment(DS);
        let changes = (0..=0xFFFFu16)
            .filter(|&o| start[o as usize] != after[o as usize] && !LEFT_OUT.iter().any(|&(lo, hi)| (lo..hi).contains(&o)))
            .flat_map(|o| {
                let [a, b] = o.to_le_bytes();
                [a, b, after[o as usize]]
            });
        let got = format!("{:x} {:x}", fnv(page.iter().copied()), fnv(changes));
        assert_eq!(f[2], got, "{line}");
        if f[2].starts_with(&format!("{blank_page:x} ")) {
            blank += 1;
        } else {
            drawn += 1;
        }
    }
    eprintln!("caps: {drawn} drawn, {blank} not, {faults} the original's faults");
    assert!(drawn > 900, "{drawn} drawn");
}

/// Fixed (kept bug #44): a floor or ceiling row's dark-run x step
/// (0000:D865, kept at CS:D754) is the x step times D762, as the y
/// step's is; the original's has CCCE (the EMS frame's segment + C00h)
/// for its low word.
#[test]
fn fixed_dark_run_step_is_the_whole_x_step() {
    use arena::fixes::{CEILING_STEP, Fixes};
    let Some(img) = common::image() else { return };
    let (xstep, ystep) = (0x0003_4567u32, 0xFFFE_1234u32);
    for fixes in [Fixes::NONE, Fixes::only(CEILING_STEP)] {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.mem.w16(DS, 0xCCCE, 0xA49F);
        m.mem.w16(DS, 0xC398, 0);
        m.mem.w16(0, 0xD77E, 0);
        m.mem.w16(0, 0xD762, 4);
        m.mem.w16(0, 0xD752, 4);
        m.mem.w16(DS, 0xCD88, RSEG);
        let (m, _) = arena::render::floor_row_alone(m, vec![0; 0x10000], 0, 0, xstep, ystep, 0, TSEG);
        let low = if fixes == Fixes::NONE { 0xA49F } else { xstep & 0xFFFF };
        let want = ((xstep & 0xFFFF_0000 | low) as i32).wrapping_mul(4) as u32;
        assert_eq!(m.mem.r32(0, 0xD754), want, "{fixes:?}");
        assert_eq!(m.mem.r32(0, 0xD758), (ystep as i32).wrapping_mul(4) as u32);
    }
}
