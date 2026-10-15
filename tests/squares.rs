//! Differential tests of the walk's bob and footsteps (src/steps.rs) and
//! a level's trigger squares (src/squares.rs) against vectors from the
//! original code (harness/gen_squares.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const TSEG: u16 = 0x7000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn changed(a: &Machine, b: &Machine, seg: u16) -> u64 {
    fnv((0..=0xFFFFu16).filter(|&o| a.mem.r8(seg, o) != b.mem.r8(seg, o)).flat_map(|o| [o as u8, (o >> 8) as u8, b.mem.r8(seg, o)]))
}

/// Mirrors harness/gen_squares.py `setup`: (op, regs).
fn setup(m: &mut Machine, seed: u32) -> (u8, [u16; 3]) {
    let mut st = Stream(seed);
    let s = &mut st;
    let w16 = |m: &mut Machine, at: u16, v: u16| m.mem.w16(DS, at, v);
    let op = s.byte() % 5;
    let x = s.word();
    let y = s.word();
    w16(m, 0xC3D0, x);
    w16(m, 0xC3D4, y);
    let dx = (s.byte() % 0xA0) as u16;
    let dy = (s.byte() % 0xA0) as u16;
    w16(m, 0xE7A2, x.wrapping_add(dx).wrapping_sub(0x50));
    w16(m, 0xE7A4, y.wrapping_add(dy).wrapping_sub(0x50));
    let at = if s.byte() < 0xE0 { (s.byte() % 8) as u16 } else { s.word() };
    w16(m, 0xE7A6, at);
    if s.byte() < 0x40 {
        for i in 0..8 {
            let v = (s.byte() % 7) as u16;
            w16(m, 0xE7A8 + 2 * i, v.wrapping_sub(3));
        }
    }
    let mut f = s.word() & !0xC00;
    if s.byte() < 0x30 {
        f |= 0x800;
    }
    if s.byte() < 0x30 {
        f |= 0x400;
    }
    w16(m, 0xC398, f);
    let mut g = s.word() & !0x4050;
    if s.byte() < 0x40 {
        g |= 0x40;
    }
    if s.byte() < 0x60 {
        g |= 0x10;
    }
    if s.byte() < 0x60 {
        g |= 0x4000;
    }
    w16(m, 0xC39A, g);
    let b = if s.byte() < 0x40 { 0 } else { s.byte() };
    m.mem.w8(DS, 0xC3C4, b);
    let p = if s.byte() < 0xC0 { s.byte() & 1 } else { s.byte() };
    m.mem.w8(DS, 0xE39D, p);
    let v = s.word();
    w16(m, 0xC3D8, v);
    for i in 0..4 {
        let v = s.word();
        w16(m, 0xC39C + 2 * i, v);
    }
    for i in 0..4 {
        let v = s.word();
        w16(m, 0xA914 + 2 * i, v);
    }
    let v = s.word();
    w16(m, 0xCB86, v);
    let sound_hook = s.byte() < 0xC0;
    let text_hook = s.byte() < 0xC0;
    let n = if s.byte() < 0x10 { s.byte() % 64 } else { s.byte() % 9 };
    m.mem.w8(DS, 0xC866, n);
    let mut ents = Vec::new();
    for i in 0..n as u16 {
        let col = s.byte() % 0x40;
        let row = s.byte() % 0x40;
        let a = if s.byte() < 0x40 { 0xFF } else { s.byte() % 6 };
        let b = if s.byte() < 0x40 { 0xFF } else { s.byte() };
        ents.push((col, row));
        for (k, c) in [col, row, a, b].into_iter().enumerate() {
            m.mem.w8(DS, 0xC766 + 4 * i + k as u16, c);
        }
    }
    let (col, row) = if n > 0 && s.byte() < 0xC0 {
        ents[(s.byte() % n) as usize]
    } else {
        let c = s.byte() % 0x40;
        (c, s.byte() % 0x40)
    };
    let (col, row) = (col as u16, row as u16);
    w16(m, 0xCCA4, col);
    w16(m, 0xCCA6, row);
    let sq = if s.byte() < 0xE0 { row << 8 | (col * 2) & 0xFF } else { s.word() };
    w16(m, 0xCB90, sq);
    for at in [0xD73D, 0xD73E] {
        let v = if s.byte() < 0x40 { 0xFF } else { s.byte() % 6 };
        m.mem.w8(DS, at, v);
    }
    let b = s.byte();
    m.mem.w8(DS, 0xC8FE, b);
    let toff = s.word() & 0x0FFF;
    w16(m, 0xCB88, toff);
    let mut t: Vec<u8> = Vec::new();
    for _ in 0..8 {
        match s.byte() % 4 {
            0 => {
                let v = if s.byte() < 0x40 { 0x7B } else { s.byte() };
                t.extend(format!("+{v}\rITEM").bytes());
            }
            1 => {
                let a = s.byte();
                let sp = s.byte() % 3;
                let b = s.byte();
                let c1 = if s.byte() & 1 != 0 { 'C' } else { 'X' };
                let c2 = if s.byte() & 1 != 0 { 'C' } else { 'X' };
                t.extend(format!("^{a}{}{b}\rRIDDLE\r:ANS\r`{c1}\rONE\r`{c2}\rTWO\r", " ".repeat(sp as usize)).bytes());
            }
            2 => t.extend(b"~ONCE\rLINE"),
            _ => {
                let lines = s.byte() % 4;
                t.extend(b"TEXT");
                for _ in 0..lines {
                    t.extend(b"\rL");
                }
                if s.byte() & 1 != 0 {
                    t.extend(b"\r:END");
                }
            }
        }
        t.push(0);
        if s.byte() < 0x30 {
            t.extend(b"`CONT\0");
        }
    }
    let mut regs = [0u16; 3];
    match op {
        2 => regs[1] = row,
        3 => {
            regs[0] = if s.byte() < 0xC0 { row << 8 | col } else { s.word() };
            regs[1] = s.word();
            regs[2] = s.word();
        }
        4 => {
            regs[1] = s.word();
            regs[2] = s.word();
        }
        _ => {}
    }
    m.mem.w16(DS, 0xCB8A, TSEG);
    m.mem.w16(DS, 0x9030, TSEG);
    m.mem.w8(DS, 0x8FA6, 0);
    for (i, &c) in t.iter().enumerate() {
        m.mem.w8(TSEG, toff.wrapping_add(i as u16), c);
    }
    m.mem.w32(DS, 0xCD46, if sound_hook { 0x23BA_002A } else { 0 });
    m.mem.w32(DS, 0xCD52, if text_hook { 0x0000_0628 } else { 0 });
    (op, regs)
}

/// The sound stub of the generator: AX at FFE0, CX at FFE2, its calls
/// counted at FFE4.
fn sound(m: &mut Machine, ax: u16, cx: u16) {
    m.mem.w16(DS, 0xFFE0, ax);
    m.mem.w16(DS, 0xFFE2, cx);
    let n = m.mem.r16(DS, 0xFFE4).wrapping_add(1);
    m.mem.w16(DS, 0xFFE4, n);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/squares.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "squares", "{line}");
        let mut m = base.clone();
        let (op, regs) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        let before = m.clone();
        let mut extra = String::new();
        let ok = match op {
            0 => {
                if let Some((s, c)) = m.walk_bob() {
                    sound(&mut m, s, c);
                }
                true
            }
            1 => {
                m.view_window();
                true
            }
            2 => match m.trigger_squares() {
                Some(Some((s, c))) => {
                    sound(&mut m, s, c);
                    true
                }
                Some(None) => true,
                None => false,
            },
            3 => match m.square_lookup(regs[0], regs[1], regs[2]) {
                Some(l) => {
                    extra = format!(" {} {:x} {:x} {:x}", l.not_found as u8, l.ax, l.bx, l.si);
                    true
                }
                None => false,
            },
            _ => m.level_text(regs[1], regs[2]).is_some(),
        };
        let got = if ok { format!("{:x} {:x}{extra}", changed(&before, &m, DS), changed(&before, &m, TSEG)) } else { "stuck".to_string() };
        assert_eq!(f[3], got, "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n}");
}
