//! Differential tests of a door stepping as the renderer meets it
//! (src/click.rs `door_drawn`, 0000:CD66) against vectors from the
//! original code (harness/gen_door.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x6800;
const STUB: u16 = 0x7000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// Mirrors harness/gen_door.py `setup`; returns the door's offset.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let (col, row) = ((s.byte() & 0x7F) as i32, s.byte() as i32);
    let sq = (row << 8 | col << 1) as u16;
    let mut near = |c: i32| {
        let d = (s.byte() % 7) as i32 - 3;
        ((c + d) << 7 | (s.byte() & 0x7F) as i32) as u16
    };
    let (px, py) = (near(col), near(row));
    m.mem.w16(DS, 0xC3D0, px);
    m.mem.w16(DS, 0xC3D4, py);
    let v = s.word();
    m.mem.w16(DS, 0xC398, v);
    for at in [0xCCC4, 0xCB37, 0xCB38, 0xBA7F, 0xC3BC, 0xC3BE, 0xC3C8, 0xC3CA] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let (off, seg) = [(0, 0), (0x10, STUB), (0, STUB + 1)][s.byte() as usize % 3];
    m.mem.w16(DS, 0xCD46, off);
    m.mem.w16(DS, 0xCD48, seg);
    let w = 0xB000 | s.word() & 0x0FFF;
    m.mem.w16(WSEG, sq, w);
    sq
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    let mut n = 0;
    for line in include_str!("vectors/door.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "door", "{line}");
        let mut m = base.clone();
        let sq = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let sound = m.door_drawn(sq);
        assert_eq!(f[2], sound.map_or("-".into(), |v| format!("{v:x}")), "{line}");
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let word = m.mem.r16(WSEG, sq);
        assert_eq!(f[3], format!("{:x}", fnv(changed.chain(word.to_le_bytes()))), "{line}");
        n += 1;
    }
    assert!(n > 2500);
}
