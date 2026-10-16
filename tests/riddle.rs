//! A riddle's answer (292B:05A2 after its field, `Machine::riddle_answered`)
//! against harness/gen_riddle.py's vectors from the original.

mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::sound::Played;
use common::blow::{put, Stream};

const FRAME: u16 = 0x6C00;
const WALLS: u16 = 0x7C00;
const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// gen_riddle.py's `setup`, draw for draw: the answer typed, which the
/// field leaves at 1044 (with its NUL).
fn setup(m: &mut Machine, seed: u32) -> Vec<u8> {
    let mut s = Stream(seed);
    m.mem.w8(DS, 0x8FA6, 0);
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w16(DS, 0xD806, WALLS);
    let mut answers: Vec<Vec<u8>> = Vec::new();
    for _ in 0..1 + s.byte() % 4 {
        let n = 1 + s.byte() % 8;
        answers.push((0..n).map(|_| LETTERS[s.byte() as usize % LETTERS.len()]).collect());
    }
    let mut text: Vec<u8> = answers.iter().flat_map(|a| [&b":"[..], a, b"\r"].concat()).collect();
    if s.byte() < 0xC0 {
        text.push(b'`');
    }
    if text.last() == Some(&b'`') && s.byte() < 0x80 {
        // Past the end, what reads as another answer: the first's.
        text.push(b':');
        text.extend(&answers[0]);
    }
    if s.byte() < 0x40 {
        // The last answer ended by the NUL instead of its CR.
        let cr = text.iter().rposition(|&c| c == b'\r').unwrap();
        text.truncate(cr);
    }
    let at = 0x100 + s.byte() as u16;
    put(m, FRAME, at - 1, b"?");
    text.push(0);
    put(m, FRAME, at, &text);
    m.mem.w16(DS, 0x7C62, at);
    let pick = s.byte();
    let typed: Vec<u8> = if pick < 0x80 {
        let a = answers[s.byte() as usize % answers.len()].clone();
        a.iter().map(|&c| if s.byte() < 0x40 { c ^ 0x20 } else { c }).collect()
    } else if pick < 0xA0 {
        Vec::new()
    } else if pick < 0xC0 {
        let a = &answers[s.byte() as usize % answers.len()];
        a[..a.len().saturating_sub(1)].to_vec()
    } else {
        let n = 1 + s.byte() % 8;
        (0..n).map(|_| LETTERS[s.byte() as usize % LETTERS.len()]).collect()
    };
    for off in [0x7C64u16, 0x7C66] {
        let v = s.word();
        m.mem.w16(DS, off, v);
    }
    for off in [0x7C68u16, 0x7C6A] {
        let v = s.byte() % 4;
        m.mem.w8(DS, off, v);
    }
    let n = s.byte() % 6;
    m.mem.w8(DS, 0xC6E4, n);
    for i in 0..n as u16 {
        let row = (s.byte() % 64) as u16;
        let sq = row << 8 | s.byte() as u16;
        let t = s.byte() % 4;
        m.mem.w16(DS, 0xC624 + 3 * i, sq);
        m.mem.w8(DS, 0xC626 + 3 * i, t);
        let off = (sq & 0xFF00) | ((sq & 0xFF) * 2 & 0xFF);
        let w = s.word();
        m.mem.w16(WALLS, off, w);
    }
    let sounds: Vec<u8> = (0..4).map(|_| 2 + s.byte() % 0x3E).collect();
    put(m, DS, 0xC3C0, &sounds);
    let e = 0xF000 + s.byte() as u16 * 2;
    m.mem.w16(DS, 0x3D44, e);
    let mut t = typed;
    t.push(0);
    t
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    // The sounds heard as numbers (the original's are a stub's log): on,
    // every number with a slot, at full volume.
    base.mem.w16(DS, arena::sound::var::ENABLED, 1);
    for k in 0..0x40 {
        base.mem.w8(DS, arena::sound::var::SLOTS + k, 0);
    }
    base.mem.w8(DS, arena::sound::var::VOLUME, 0x64);
    let (mut n, mut bad, mut stuck) = (0, Vec::new(), 0);
    for line in include_str!("vectors/riddle.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "riddle", "{line}");
        let mut m = base.clone();
        let typed = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        put(&mut m, DS, arena::riddle::var::TYPED, &typed);
        n += 1;
        let got = match m.riddle_answered() {
            None => "stuck".to_string(),
            Some(()) => {
                // The sounds' own bytes (their square, priority) aren't the
                // original's: its sound is a stub.
                let skip = |o: u16| (0x126E..0x1270).contains(&o) || o == 0x43D6 || o >= 0xFF00;
                let diff = fnv((0..=0xFFFFu16)
                    .filter(|&o| !skip(o) && m.mem.r8(DS, o) != before.mem.r8(DS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]));
                let walls = fnv((0..=0xFFFFu16)
                    .filter(|&o| m.mem.r8(WALLS, o) != before.mem.r8(WALLS, o))
                    .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(WALLS, o)]));
                let numbers: Vec<u8> = m
                    .sounds
                    .iter()
                    .flat_map(|p| match p {
                        Played::Number(s) => s.number.to_le_bytes(),
                        _ => panic!("a sound file"),
                    })
                    .collect();
                let last = if m.sounds.is_empty() { 0 } else { m.mem.r16(DS, arena::sound::var::AT) };
                format!("{:x} {:x} {:x} {:x}", diff, walls, fnv(numbers), last)
            }
        };
        if f[2] == "stuck" {
            stuck += 1;
        }
        if f[2] != got {
            bad.push(format!("{}: want {} got {got}", f[1], f[2]));
        }
    }
    assert!(bad.is_empty(), "{} of {n} differ:\n{}", bad.len(), bad[..bad.len().min(10)].join("\n"));
    eprintln!("{n} compared, {stuck} stuck");
}
