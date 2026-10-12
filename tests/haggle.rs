//! Differential tests of haggling (src/haggle.rs) against vectors from
//! the original code with a scripted player (harness/gen_haggle.py).
mod common;

use arena::bsa::Files;
use arena::haggle::{Haggle, HaggleIo};
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

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
    let mut n = 0;
    for line in include_str!("vectors/haggle.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let p: Vec<u32> = f[1].split(' ').map(hex).collect();
        let mut io = Script {
            menus: f[2].split(' ').map(|c| c.parse().unwrap()).collect(),
            offers: f[3].split(',').map(|o| o.as_bytes().to_vec()).collect(),
            used: [0; 3],
        };
        let mut m = base.clone();
        let mut fs = files.clone();
        m.mem.w32(DS, 0x91F6, p[6]);
        m.mem.w16(DS, 0x91F0, 0);
        m.load_rng();
        let got = m.haggle(&mut fs, &mut io, p[0], p[1] as u16, p[2] as u8, p[3] as u8, p[5] as u8, p[4] as u8, 0x1A4C);
        let got = match got {
            None if f[4] == "hang" || f[4] == "fault" => {
                n += 1;
                continue;
            }
            None => panic!("port gave None\n{line}"),
            Some(r) => {
                m.store_rng();
                let state: Vec<u8> = [(0x1044u16, 4u16), (0x1391, 4), (0x13CA, 2), (0x6598, 0x10), (0x65AA, 0x12), (0x91F6, 4)]
                    .iter()
                    .flat_map(|&(o, l)| (0..l).map(move |i| (o + i, ())))
                    .map(|(o, _)| m.mem.r8(DS, o))
                    .collect();
                let result = match r {
                    Haggle::Deal(eax) => format!("deal {eax:x}"),
                    Haggle::NoDeal => "none".into(),
                };
                format!("{result} {} {} {} {:x}", io.used[0], io.used[1], io.used[2], fnv(state))
            }
        };
        assert_eq!(f[4], got, "{line}");
        n += 1;
    }
    eprintln!("{n} haggles");
}
