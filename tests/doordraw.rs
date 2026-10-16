//! Differential tests of how the renderer draws a door ajar
//! (src/doordraw.rs: the moved texture 0000:BAF3, the swing 0000:D344)
//! against vectors from the original code (harness/gen_doordraw.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const TSEG: u16 = 0x7000;
const OSEG: u16 = 0x8000;
const TEXSEED: u32 = 0xD00D_0B0B;
const SPAN: u16 = 0x1100;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_doordraw.py `tex_setup`.
fn tex_setup(seed: u32) -> (u8, u16, u16) {
    let mut s = Stream(seed);
    let kind = s.byte() % 4;
    let open = 1 + (s.byte() % 7) as u16;
    let slot = (s.byte() % 15) as u16;
    (kind, open, slot * 0x1000)
}

/// Mirrors harness/gen_doordraw.py `swing_setup`.
fn swing_setup(seed: u32) -> [u16; 6] {
    let mut s = Stream(seed);
    let mut hinge = (s.word(), s.word());
    let far;
    if s.byte() & 1 != 0 {
        let (x, y) = (((s.byte() & 0x7F) as u16) << 7, (s.byte() as u16) << 7);
        let corners = [(0u16, 0u16), (0x80, 0), (0x80, 0x80), (0, 0x80)];
        let k = (s.byte() % 4) as usize;
        hinge = (x + corners[k].0, y + corners[k].1);
        far = (x + corners[(k + 1) % 4].0, y + corners[(k + 1) % 4].1);
    } else {
        far = (s.word(), s.word());
    }
    let open = if s.byte() & 1 != 0 { (s.byte() % 8) as u16 } else { s.word() };
    let kind = if s.byte() & 1 != 0 { 0 } else { s.byte() as u16 };
    [hinge.0, hinge.1, far.0, far.1, open, kind]
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    let mut texs = Stream(TEXSEED);
    for o in 0..=0xFFFFu16 {
        let b = texs.byte();
        base.mem.w8(TSEG, o, b);
    }
    let (mut tex, mut swing) = (0, 0);
    for line in include_str!("vectors/doordraw.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        match f[0] {
            "tex" => {
                let (kind, open, off) = tex_setup(seed);
                for o in 0..SPAN {
                    m.mem.w8(OSEG, o, 0xEE);
                }
                m.mem.w8(DS, 0xCB38, kind);
                m.mem.w16(DS, 0xCCC4, open);
                m.mem.w16(DS, 0x9030, TSEG);
                m.mem.w16(DS, 0xBA54, 0);
                m.mem.w16(DS, 0xBA56, OSEG);
                m.door_texture(off);
                let got = fnv((0..SPAN).map(|o| m.mem.r8(OSEG, o)));
                assert_eq!(f[2], format!("{got:x}"), "{line}");
                tex += 1;
            }
            "swing" => {
                let v = swing_setup(seed);
                for (at, w) in [0xD7AA, 0xD7AE, 0xD7B0, 0xD7B4, 0xCCC4].into_iter().zip(v) {
                    m.mem.w16(DS, at, w);
                }
                m.mem.w8(DS, 0xCB38, v[5] as u8);
                m.door_swing();
                let got = [0xD7B0, 0xD7B4, 0xD7B6, 0xD7BA].map(|at| format!("{:x}", m.mem.r16(DS, at))).join(" ");
                assert_eq!(f[2], got, "{line}");
                swing += 1;
            }
            k => panic!("{k}: {line}"),
        }
    }
    assert!(tex > 1400 && swing > 1400);
}
