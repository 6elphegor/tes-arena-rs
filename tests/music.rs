//! Differential tests of the music's pickers (src/music.rs `town_music`,
//! 2294:04C0; `area_music`, 2294:04F5 with 10B6; `fm_song_name`,
//! 0000:11DA) against vectors from the original code (harness/gen_music.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::music::Song;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const SONGS: [u16; 18] = [
    0x421C, 0x4226, 0x4233, 0x4240, 0x424C, 0x4259, 0x4265, 0x4272, 0x427E, 0x87B3, 0x87BE, 0x87C9, 0x87D5, 0x87E2,
    0x87ED, 0x87FA, 0x8805, 0xFFFF,
];
const NAMES: [&[u8]; 11] =
    [b"night.xmi", b"city.xmi", b"town.XMI", b"combat.xfm", b"a.xmi.b", b"noext", b".xmi", b"x.xmid", b"dungeon1.xmi", b"snow.xm", b"a.b.xmi"];

/// Mirrors harness/gen_music.py `setup` (a conditional's test byte first;
/// a tuple's items before its index); returns the op.
fn setup(m: &mut Machine, seed: u32) -> u8 {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 3;
    let v = s.word() & !4;
    let v = v | if s.byte() & 1 != 0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & !0x10;
    let v = v | if s.byte() < 0x30 { 0x10 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x430F, v);
    let r = s.byte();
    let v = [0, 1, 2, r][s.byte() as usize % 4];
    m.mem.w8(DS, 0xFFBC, v);
    let k = s.byte() % 4;
    let a = s.word() & 0x3FF;
    let b = 0x10B + s.byte() as u16 % 3;
    let c = 0x2F3 + s.byte() as u16 % 2;
    let d = s.word();
    m.mem.w16(DS, 0xCCFE, [a, b, c, d][k as usize]);
    let r = s.byte();
    let v = [0, 1, 0x81, 2, 0x82, 3, r][s.byte() as usize % 7];
    m.mem.w8(DS, 0xBB09, v);
    let r = s.byte();
    let v = [0, 0, 1, 3, 4, r][s.byte() as usize % 6];
    m.mem.w8(DS, 0x05E6, v);
    let v = SONGS[s.byte() as usize % SONGS.len()];
    m.mem.w16(DS, 0x0FF0, v);
    let b = s.byte();
    m.mem.w8(DS, 0xBB0A, b);
    let v = if s.byte() < 0x60 { b } else { s.byte() };
    m.mem.w8(DS, 0x427D, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x1010, v);
    let v = if s.byte() < 0x30 { 0 } else { s.word() };
    m.mem.w16(DS, 0x8F60, v);
    let v = if s.byte() < 0xC0 { SONGS[s.byte() as usize % SONGS.len()] } else { s.word() };
    m.mem.w16(DS, 0x0D56, v);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x430E, v);
    let v = if s.byte() < 0xE0 { s.byte() % 3 } else { s.byte() % 6 };
    m.mem.w8(DS, 0x0A47, v);
    let w = s.word();
    let v = [0, 4, 8, w][s.byte() as usize % 4];
    m.mem.w16(DS, 0x0DA8, v);
    m.mem.w16(DS, 0x91F0, 0);
    let lo = s.word() as u32;
    let seed = lo | (s.word() as u32) << 16;
    m.mem.w32(DS, 0x91F6, seed);
    let name = NAMES[s.byte() as usize % NAMES.len()];
    let mut at = 0xF000u16;
    for &c in name.iter().chain(&[0]) {
        m.mem.w8(DS, at, c);
        at += 1;
    }
    for _ in 0..4 {
        let c = s.byte();
        m.mem.w8(DS, at, c);
        at += 1;
    }
    let v = s.byte();
    m.mem.w8(DS, 0x101B, v);
    op
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/music.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "music", "{line}");
        let mut m = base.clone();
        let op = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        assert_eq!(f[2], op.to_string(), "{line}");
        m.load_rng();
        m.music_status = m.mem.r8(DS, 0xFFBC);
        let before = m.clone();
        let mut si = 0;
        match op {
            0 => m.town_music(),
            1 => m.area_music(),
            _ => si = m.fm_song_name(0xF000),
        }
        m.store_rng();
        let changed = (0..0xFFBCu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let log: Vec<String> = m
            .songs
            .iter()
            .map(|s| match s {
                Song::Stop => "ffff".to_string(),
                Song::Play { at, .. } => format!("{at:x}"),
            })
            .collect();
        let mut got = format!("{:x} {}", fnv(changed), if log.is_empty() { "-".to_string() } else { log.join(",") });
        if op == 2 {
            got += &format!(" {si:x}");
        }
        assert_eq!(f[3], got, "{line}");
        n += 1;
    }
    assert!(n > 2900);
}

/// The file opened for a song: its .xfm arrangement with the FM driver.
#[test]
fn fm_file_names() {
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    let at = 0xF000;
    for (i, &c) in b"night.xmi\0".iter().enumerate() {
        m.mem.w8(DS, at + i as u16, c);
    }
    assert_eq!(m.fm_song_file(at), b"night.xmi");
    m.mem.w8(DS, 0x1010, 1);
    assert_eq!(m.fm_song_file(at), b"night.xfm");
    let si = m.fm_song_name(at);
    assert_eq!(m.mem.cstr(DS, si), b"night.xfm");
}
