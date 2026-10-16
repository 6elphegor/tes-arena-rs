//! The rented room's file (1194:BA44 name, C04C read, BA91 write)
//! against harness/gen_roomfile.py's vectors from the original.

mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

const CSEG: u16 = 0x6C00;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// gen_roomfile.py's `setup`, draw for draw: the script's op, the hours
/// and end op 2 writes anew, and the stale name put at 733A.
fn setup(m: &mut Machine, seed: u32) -> (u8, (u16, u32), Vec<u8>) {
    let mut s = Stream(seed);
    let dword = |s: &mut Stream| s.word() as u32 | (s.word() as u32) << 16;
    let v = if s.byte() < 0x40 { 0x4000 } else { 0 };
    m.mem.w16(DS, 0x05C8, v);
    let fd2 = s.word();
    m.mem.w16(DS, 0x0FD2, fd2);
    let n = (s.byte() % 7) as u16;
    m.mem.w16(DS, 0x1387, n);
    m.mem.w16(DS, 0x1AB5, 0);
    m.mem.w16(DS, 0x1AB7, CSEG);
    let key = (fd2 & 0xFF00) | ((fd2 & 0xFF) >> 1);
    let there = s.byte() < 0xC0;
    let at = (s.byte() as u16) % n.max(1);
    for i in 0..n {
        let k = if there && i == at { key } else { s.word() };
        let w = s.word();
        m.mem.w16(CSEG, 4 * i, k);
        m.mem.w16(CSEG, 4 * i + 2, w);
    }
    put(m, CSEG, 4 * n, &[0; 4]);
    for off in [0x0608u16, 0x060A, 0x0610, 0x0612] {
        let v = if s.byte() < 0x80 { s.word() } else { s.word() % 0x4000 };
        m.mem.w16(DS, off, v);
    }
    let hours = if s.byte() < 0x30 { 0 } else { s.word() % 0x200 };
    m.mem.w16(DS, 0x1019, hours);
    let clock = dword(&mut s);
    m.mem.w32(DS, 0x05CC, clock);
    // A quarter of the time at the read's 3Ch minutes' edge.
    let edge = s.byte() < 0x40;
    let end = if edge { clock.wrapping_add(0x3B + (s.byte() % 3) as u32) } else { clock.wrapping_add((s.word() % 0x100) as u32).wrapping_sub(0x40) };
    m.mem.w32(DS, 0x12CA, end);
    let mut stale = b"in".to_vec();
    for _ in 0..4 {
        stale.push(0x30 + s.byte() % 10);
    }
    stale.extend(b".64\0");
    put(m, DS, 0x733A, &stale);
    let t = if s.byte() & 1 != 0 { 0xFF } else { 0 };
    m.mem.w8(DS, 0x1AEA, t);
    let op = s.byte() % 4;
    let again = (s.word() % 0x200, dword(&mut s));
    stale.pop();
    (op, again, stale)
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
    let (mut n, mut bad) = (0, Vec::new());
    for line in include_str!("vectors/roomfile.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "roomfile", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let (op, (hours, end), stale) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = m.clone();
        let mut carries = Vec::new();
        match op {
            0 => {
                m.room_write(&mut fs);
                carries.push(m.room_read(&mut fs) as u8);
            }
            1 => carries.push(m.room_read(&mut fs) as u8),
            2 => {
                m.room_write(&mut fs);
                m.mem.w16(DS, 0x1019, hours);
                m.mem.w32(DS, 0x12CA, end);
                m.room_write(&mut fs);
                carries.push(m.room_read(&mut fs) as u8);
            }
            _ => {
                m.room_write(&mut fs);
                m.mem.w16(DS, 0x1019, 0);
                m.room_write(&mut fs);
                carries.push(m.room_read(&mut fs) as u8);
            }
        }
        let diff = fnv((0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]));
        // The files: the names the case can have touched, the stale one
        // and the last one made (733A as it ends), upper-cased.
        let made = m.mem.cstr(DS, 0x733A).to_ascii_uppercase();
        let mut names = vec![stale.to_ascii_uppercase(), made];
        names.sort();
        names.dedup();
        let mut fl = Vec::new();
        for k in &names {
            if let Some(d) = fs.written(k) {
                fl.extend(k);
                fl.push(0);
                fl.extend((d.len() as u32).to_le_bytes());
                fl.extend(d);
            }
        }
        for k in &names {
            if fs.was_deleted(k) && fs.written(k).is_none() {
                fl.extend(k);
                fl.push(0);
            }
        }
        let got = format!("{:x} {:x} {:x}", diff, fnv(carries.iter().copied()), fnv(fl));
        n += 1;
        if f[2] != got {
            bad.push(format!("{}: want {} got {got} (op {op})", f[1], f[2]));
        }
    }
    assert!(bad.is_empty(), "{} of {n} differ:\n{}", bad.len(), bad[..bad.len().min(10)].join("\n"));
    eprintln!("{n} compared");
}
