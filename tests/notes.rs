//! Differential tests of the automap's notes (src/mapnotes.rs: 0000:31FE,
//! 32FE, 23F1:00A3) against vectors from the original code
//! (harness/gen_notes.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mapnotes::NoteHooks;
use arena::mem::DS;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const FONT_SEG: u16 = 0x7000;

fn digits(n: u8) -> [u8; 2] {
    [0x30 + n / 10, 0x30 + n % 10]
}

fn letters(s: &mut Stream, n: u8) -> Vec<u8> {
    (0..n)
        .map(|_| match s.byte() % 8 {
            0 => 0x20,
            1 => 0x80 | s.byte(),
            2 => 0x61 + s.byte() % 26,
            _ => 0x41 + s.byte() % 26,
        })
        .collect()
}

/// The stubs of harness/gen_notes.py: the typing records its call's AX,
/// BX, CX, DX at F0F0..F0F7 and copies the text at F000 to 1044; a note
/// drawn logs its AX, BX at F100 + the count (F0FE, 4 a note).
struct Stubs;

impl NoteHooks for Stubs {
    fn type_text(&mut self, m: &mut Machine, x: u16, y: u16, cursor: u16, max: u16) {
        for (i, v) in [x, y, cursor, max].into_iter().enumerate() {
            m.mem.w16(DS, 0xF0F0 + 2 * i as u16, v);
        }
        for i in 0..0x3Cu16 {
            let b = m.mem.r8(DS, 0xF000 + i);
            m.mem.w8(DS, 0x1044 + i, b);
        }
    }
    fn draw_text(&mut self, m: &mut Machine, x: u16, y: u16, _si: u16) {
        let n = m.mem.r16(DS, 0xF0FE);
        m.mem.w16(DS, 0xF100u16.wrapping_add(n), x);
        m.mem.w16(DS, 0xF102u16.wrapping_add(n), y);
        m.mem.w16(DS, 0xF0FE, n.wrapping_add(4));
    }
}

/// Mirrors harness/gen_notes.py `setup` (a conditional's test byte comes
/// first, a tuple's items before its index); returns (op, the place to
/// add, the file's name).
fn setup(m: &mut Machine, fs: &mut Files, seed: u32) -> (u8, u16, u16, Vec<u8>) {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 3;
    let slot = 1 + s.byte() % 19;
    for (i, &d) in digits(slot).iter().enumerate() {
        m.mem.w8(DS, 0x6643 + i as u16, d);
    }
    let rec = (s.byte() % 16) as usize;
    let pos = rec * 0x2004;
    m.mem.w32(DS, 0x66C6, pos as u32);
    let (left, top) = ((s.byte() % 0x40) as u16, (s.byte() % 0x40) as u16);
    m.mem.w16(DS, 0xA880, left);
    m.mem.w16(DS, 0xA882, top);
    let n = s.byte() % 12;
    let mut typed = letters(s, n);
    typed.push(0);
    while typed.len() < 0x3C {
        typed.push(s.byte());
    }
    if s.byte() < 0x30 {
        typed[0] = 0;
    }
    for (i, &b) in typed.iter().enumerate() {
        m.mem.w8(DS, 0xF000 + i as u16, b);
    }
    let mut f = vec![0u8; 0x20040];
    for i in 0..16 {
        let lo = s.word() as u32;
        let key = lo | (s.word() as u32) << 16;
        f[i * 0x2004..i * 0x2004 + 4].copy_from_slice(&key.to_le_bytes());
    }
    let mut notes: Vec<(Option<(u16, u16)>, Vec<u8>)> = Vec::new();
    for _ in 0..0x40 {
        let k = s.byte() % 4;
        let xy = match k {
            0 => {
                let y = if s.byte() < 0x40 { s.word() } else { 0 };
                (0, y)
            }
            1 => {
                let x = (left + (s.byte() % 0x48) as u16) & 0x7FFF;
                (x, top + (s.byte() % 0x30) as u16)
            }
            2 => {
                let x = 0x8000 | (0xCE + (s.byte() % 0x4A) as u16);
                (x, 0x23 + (s.byte() % 0x90) as u16)
            }
            _ => (s.word(), s.word()),
        };
        let n = s.byte() % 14;
        let mut t = letters(s, n);
        t.push(0);
        let mut place = Some(xy);
        if s.byte() < 0x20 {
            let end = typed.iter().position(|&b| b == 0).unwrap();
            t = typed[..=end].to_vec();
            if s.byte() < 0x80 {
                for c in t.iter_mut() {
                    if (0x41..=0x5A).contains(&(*c & 0xDF)) {
                        *c ^= 0x20;
                    }
                }
            }
            if s.byte() < 0x80 {
                place = None;
            }
        }
        while t.len() < 0x3C {
            t.push(s.byte());
        }
        t.truncate(0x3C);
        notes.push((place, t));
    }
    let mut ax = s.word() % 0x140;
    let mut bx = s.word() % 0xC8;
    let k = s.byte() % 4;
    if k == 1 {
        ax = 0x19 + (s.byte() % 0xFF) as u16;
        bx = 0x23 + (s.byte() % 0x95) as u16;
    } else if k >= 2 {
        let j = (s.byte() % 0x40) as usize;
        match notes[j].0 {
            None | Some((0, _)) => {
                ax = 0x19 + (s.byte() % 0xFF) as u16;
                bx = 0x23 + (s.byte() % 0x95) as u16;
            }
            Some((x, y)) if x & 0x8000 != 0 => {
                ax = (x & 0x7FFF).wrapping_sub((s.byte() % 8) as u16);
                bx = y;
            }
            Some((x, y)) => {
                ax = 0xCDu16.wrapping_sub(3u16.wrapping_mul(x.wrapping_sub(left))).wrapping_sub((s.byte() % 6) as u16);
                bx = 0x28u16.wrapping_add(3u16.wrapping_mul(y.wrapping_sub(top))).wrapping_add((s.byte() % 9) as u16);
            }
        }
    }
    let (add_x, add_y) = if op != 2 {
        (ax, bx)
    } else {
        let x = if s.byte() < 0x80 { s.word() & 0x7FFF } else { 0x8000 | s.word() };
        (x, s.word())
    };
    for (i, (place, t)) in notes.iter().enumerate() {
        let (x, y) = place.unwrap_or((add_x, add_y));
        let at = pos + 4 + i * 0x40;
        f[at..at + 2].copy_from_slice(&x.to_le_bytes());
        f[at + 2..at + 4].copy_from_slice(&y.to_le_bytes());
        f[at + 4..at + 0x40].copy_from_slice(t);
    }
    if s.byte() < 0x30 {
        let a = s.byte() as usize * 0x10;
        let b = (s.byte() % 0x10) as usize;
        f.truncate(pos + 4 + a + b);
    }
    m.mem.w16(DS, 0x8F92, ax);
    m.mem.w16(DS, 0x8F94, bx);
    let b = s.byte();
    m.mem.w8(DS, 0xA888, b);
    let npos = if s.byte() < 0x80 {
        (pos + 4 + (s.byte() % 0x40) as usize * 0x40) as u32
    } else {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    m.mem.w32(DS, 0xA892, npos);
    let fo = s.byte() as u16;
    let wide = s.byte() < 0x33;
    for i in 0..0x80u16 {
        let k = s.byte();
        let v = if wide {
            0x50 + s.byte() % 0x30
        } else if k < 0xC0 {
            3 + s.byte() % 6
        } else if k < 0xE8 {
            0x50 + s.byte() % 0x30
        } else {
            s.byte()
        };
        m.mem.w8(FONT_SEG, fo + i, v);
    }
    m.mem.w16(DS, 0xB034, fo);
    m.mem.w16(DS, 0xB036, FONT_SEG);
    let c = s.byte();
    m.mem.w8(DS, 0x9288, c);
    m.mem.w16(DS, 0xF0FE, 0);
    for i in 0..0x60u16 {
        let b = s.byte();
        m.mem.w8(DS, 0x1044 + i, b);
    }
    let mut name = b"AUTOMAP.".to_vec();
    name.extend(digits(slot));
    fs.put(&name, &f);
    (op, add_x, add_y, name)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = std::path::PathBuf::from(std::env::var("ARENA_EXE").map_or_else(
        |_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into(),
        |p| std::path::Path::new(&p).parent().unwrap().to_string_lossy().into_owned(),
    ));
    let Ok(files) = Files::new(&dir) else { return };
    let base = Machine::new(&img);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/notes.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "notes", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (op, x, y, name) = setup(&mut m, &mut fs, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let done = match op {
            0 => m.note_click(&mut fs, &mut Stubs),
            1 => {
                m.notes_draw(&mut fs, &mut Stubs);
                Some(())
            }
            _ => m.note_add(&mut fs, x, y, 0xF000),
        };
        if done.is_none() {
            assert!(f[3] == "fault" || f[3] == "hang", "{line}: the port gave up");
            stuck += 1;
            continue;
        }
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let got = format!("{:x} {:x}", fnv(changed), fnv(fs.data(&name).unwrap().iter().copied()));
        assert_eq!(f[3], got, "{line}");
        n += 1;
    }
    eprintln!("notes: {n} compared, {stuck} stuck in both");
    assert!(n > 2500, "{n} compared");
}
