//! Differential tests of a tavern's room haggled over (src/tavern.rs
//! `tavern_room_haggle`, 1194:C14E) against vectors from the original
//! code with a scripted player (harness/gen_tavroom.py).
mod common;

use arena::bsa::Files;
use arena::haggle::HaggleIo;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const PSEG: u16 = 0x6000;

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

/// The scripted player: answers from queues, counting what it used.
struct Script {
    menus: Vec<u16>,
    offers: Vec<Vec<u8>>,
    used: [usize; 3],
}

impl HaggleIo for Script {
    fn menu(&mut self) -> Option<u16> {
        let c = *self.menus.get(self.used[0])?;
        self.used[0] += 1;
        Some(c)
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        let t = self.offers.get(self.used[1])?.clone();
        self.used[1] += 1;
        Some(t)
    }
    fn key(&mut self) -> Option<()> {
        self.used[2] += 1;
        Some(())
    }
}

fn bytes(m: &Machine, ranges: &[(u16, u16)]) -> Vec<u8> {
    ranges.iter().flat_map(|&(o, l)| (0..l).map(move |i| o.wrapping_add(i))).map(|o| m.mem.r8(DS, o)).collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut base = Machine::new(&img);
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, 0x7000);
    let (mut n, mut deals) = (0, 0);
    for line in include_str!("vectors/tavroom.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "tavroom", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let records: Vec<u32> = p[2].split(',').map(hex).collect();
        let v: Vec<u32> = p[3..].iter().map(|s| hex(s)).collect();
        let (set, room, quality, mood, per, price, seed) = (v[0], v[1], v[2], v[3], v[4], v[5], v[6]);
        let mut io = Script {
            menus: f[2].split(' ').map(|c| c.parse().unwrap()).collect(),
            offers: f[3].split(',').map(|o| o.as_bytes().to_vec()).collect(),
            used: [0; 3],
        };
        let mut m = base.clone();
        let mut fs = files.clone();
        for (i, r) in records.iter().enumerate() {
            m.mem.w32(PSEG, 4 * i as u16, *r);
        }
        m.mem.w32(PSEG, 4 * records.len() as u16, 0);
        m.mem.w32(PSEG, 4 * records.len() as u16 + 4, 0);
        m.mem.w16(DS, 0x0FD2, hex(p[0]) as u16);
        m.mem.w16(DS, 0x1387, records.len() as u16);
        m.mem.w16(DS, 0x1AB5, 0);
        m.mem.w16(DS, 0x1AB7, PSEG);
        m.mem.w8(DS, 0x0A46, set as u8);
        m.mem.w8(DS, 0x13C8, room as u8);
        m.mem.w8(DS, 0x13C9, 0);
        m.mem.w8(DS, 0x13CA, quality as u8);
        m.mem.w8(DS, 0x13CB, mood as u8);
        m.mem.w8(DS, 0x01D3, per as u8);
        m.mem.w32(DS, 0x1391, price);
        m.mem.w32(DS, 0x91F6, seed);
        m.mem.w16(DS, 0x91F0, 0);
        m.load_rng();
        let got = match m.tavern_room_haggle(&mut fs, &mut io) {
            None if f[4] == "hang" || f[4] == "fault" => {
                n += 1;
                continue;
            }
            None => panic!("port gave None\n{line}"),
            Some((eax, walked)) => {
                m.store_rng();
                let names = bytes(&m, &[(0x7311, 0x20), (0x1A4C, 0x20), (0x1376, 2), (0x98DE, 4)]);
                let state = bytes(
                    &m,
                    &[
                        (0x1044, 4),
                        (0x1391, 4),
                        (0x13CA, 2),
                        (0x6599, 0xF),
                        (0x65AA, 0x12),
                        (0x91F6, 4),
                        (0x05C2, 4),
                        (0x0FE6, 1),
                        (0x0FEE, 2),
                        (0x9288, 1),
                    ],
                );
                let shown = bytes(&m, &[(0x929E, 0x200), (0x987A, 0x200), (0x137E, 1), (0x13D3, 1)]);
                let result = if walked { "none".to_string() } else { deals += 1; format!("deal {eax:x}") };
                format!("{result} {} {} {} {:x} {:x} {:x}", io.used[0], io.used[1], io.used[2], fnv(names), fnv(state), fnv(shown))
            }
        };
        assert_eq!(f[4], got, "{line}");
        n += 1;
    }
    eprintln!("{n} rooms haggled over, {deals} deals");
    assert!(n > 2500, "{n}");
}
