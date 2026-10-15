//! Differential tests of the sound played (src/sound.rs `sound_at`,
//! 23BA:002A, and its volume 23BA:00E8) against vectors from the original
//! code (harness/gen_sound.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const SAMPLE_SEG: u16 = 0x7000;
const BUFFER_SEG: u16 = 0x7800;

/// Mirrors harness/gen_sound.py `setup` (a conditional's test byte first,
/// a tuple's items before its index); returns AX and CX.
fn setup(m: &mut Machine, seed: u32) -> (u16, u16) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = if s.byte() < 0x20 { 0 } else { s.word() | 1 };
    m.mem.w16(DS, 0x8F64, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x05E3, v);
    let v = s.word();
    m.mem.w16(DS, 0x05BE, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = if s.byte() < 0xE8 { s.byte() % 101 } else { 200 + s.byte() % 4 };
    m.mem.w8(DS, 0x0D3B, v);
    let me = s.word();
    m.mem.w16(DS, 0xCB90, me);
    for i in 0..0x40u16 {
        let v = if s.byte() < 0x30 { 0xFF } else { s.byte() % 0x20 };
        m.mem.w8(DS, 0xC867 + i, v);
    }
    for i in 0..0x40u16 {
        let v = s.byte();
        m.mem.w8(DS, 0x46F4 + i, v);
    }
    for i in 0..0x20u16 {
        let at = 0x44F0 + i * 8;
        let h = s.word() & 0x7FFF;
        let off = (s.byte() & 0xF0) as u16;
        let len = (s.byte() % 0x40) as u16;
        m.mem.w16(DS, at, h);
        m.mem.w16(DS, at + 2, off);
        m.mem.w16(DS, at + 4, SAMPLE_SEG);
        m.mem.w16(DS, at + 6, len);
    }
    m.mem.w16(DS, 0xEB89, 0);
    m.mem.w16(DS, 0xEB8B, BUFFER_SEG);
    let status = (s.byte() % 3) as u16;
    m.mem.w16(DS, 0xFFE0, status);
    m.sound_busy = status == 2;
    let k = s.byte() % 4;
    let a0 = (s.byte() % 0x30) as u16;
    let a2 = s.word();
    let a3 = (s.byte() & 1) as u16;
    let ax = [a0, 0xFF, a2, a3][k as usize];
    let k = s.byte() % 3;
    let dc = s.byte() % 9;
    let dr = s.byte() % 9;
    let near = (me & 0xFF00 | (me.wrapping_add(dc as u16 * 2).wrapping_sub(8)) & 0xFF)
        .wrapping_add(((dr as i16 - 4) << 8) as u16);
    let any = s.word();
    let cx = [me, near, any][k as usize];
    (ax, cx)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/sound.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "sound", "{line}");
        let mut m = base.clone();
        let (ax, cx) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let got = m.sound_at(ax, cx).map(|_| {
            // The driver's stubs: the volume kept and counted, the play
            // and 0D58 counted.
            if let Some(arena::sound::Played::Number(s)) = m.sounds.last().cloned() {
                m.mem.w16(DS, 0xFFE4, s.volume as u16);
                let c = m.mem.r16(DS, 0xFFE6).wrapping_add(1);
                m.mem.w16(DS, 0xFFE6, c);
                let c = m.mem.r16(DS, 0xFFE8).wrapping_add(2);
                m.mem.w16(DS, 0xFFE8, c);
            }
            let changed = (0..0xFFE0u16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let stubs = (0xFFE4..0xFFEAu16).map(|o| m.mem.r8(DS, o));
            format!("{:x} {:x}", fnv(changed), fnv(stubs))
        });
        match got {
            Some(g) => assert_eq!(f[2], g, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        n += 1;
    }
    assert!(n > 2500);
}
