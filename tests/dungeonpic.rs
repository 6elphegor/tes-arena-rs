//! Differential tests of a main quest's dungeon shown on arrival
//! (src/dungeonpic.rs `dungeon_picture`, 292B:0362) against vectors from
//! the original code (harness/gen_dungeonpic.py).
mod common;

use arena::bsa::Files;
use arena::dungeonpic::DungeonPicHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FRAME: u16 = 0x8800;
const HEAP: (u16, u16) = (0xA000, 0xF000);

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn seg_bytes(m: &Machine, seg: u16) -> Vec<u8> {
    (0..=0xFFFFu16).map(|o| m.mem.r8(seg, o)).collect()
}

fn changes(old: &[u8], new: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (a, b)) in old.iter().zip(new).enumerate() {
        if a != b {
            out.extend([i as u8, (i >> 8) as u8, *b]);
        }
    }
    out
}

/// The generator's stubs' log, and its clicks.
struct Log {
    log: Vec<Vec<u8>>,
    clicks: Vec<(u16, u16)>,
}

fn words(tag: &[u8], w: &[u16]) -> Vec<u8> {
    let mut v = tag.to_vec();
    for x in w {
        v.extend(x.to_le_bytes());
    }
    v
}

impl DungeonPicHooks for Log {
    fn fade(&mut self, _m: &mut Machine) {
        self.log.push(b"fade".to_vec());
    }
    fn line(&mut self, _m: &mut Machine, si: u16, ax: u16, bx: u16) {
        self.log.push(words(b"line", &[si, ax, bx]));
    }
    fn shown(&mut self, _m: &mut Machine, seg: u16, off: u16) {
        self.log.push(b"shown".to_vec());
        self.log.push(words(b"fadein", &[seg, off, 12]));
    }
    fn click(&mut self, m: &mut Machine) -> Option<()> {
        let (x, y) = if self.clicks.is_empty() { (0x125, 0xBD) } else { self.clicks.remove(0) };
        m.mem.w16(DS, 0x8F92, x);
        m.mem.w16(DS, 0x8F94, y);
        self.log.push(b"click".to_vec());
        Some(())
    }
}

/// A case: AX, the DGROUP words set and the clicks.
type Case = (u16, Vec<(u16, u16)>, Vec<(u16, u16)>);

/// Mirrors harness/gen_dungeonpic.py `setup`.
fn setup(seed: u32) -> Case {
    let mut s = Stream(seed);
    let k = s.byte();
    let ax = if k < 0xC0 { (s.byte() % 9) as u16 } else if k < 0xF0 { s.byte() as u16 } else { s.word() };
    let words = vec![(0x102E, s.word()), (0x1030, (s.byte() % 0x20) as u16), (0xA902, s.word()), (0x05C8, s.word())];
    let mut clicks = Vec::new();
    for _ in 0..s.byte() % 4 {
        let edge = s.byte() % 5;
        let (mut x, mut y) = (0x110 + (s.byte() % 0x2B) as u16, 0xB7 + (s.byte() % 0x0D) as u16);
        match edge {
            0 => x = 0x10F,
            1 => x = 0x13B,
            2 => y = 0xB6,
            3 => y = 0xC4,
            _ => {
                x = s.word() % 0x140;
                y = s.word() % 0xC8;
                if (0x110..=0x13A).contains(&x) && (0xB7..=0xC3).contains(&y) {
                    x = 0;
                }
            }
        }
        clicks.push((x, y));
    }
    clicks.push((0x110 + (s.byte() % 0x2B) as u16, 0xB7 + (s.byte() % 0x0D) as u16));
    (ax, words, clicks)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: the game directory not found");
        return;
    };
    let mut files = files;
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    base.mem.w16(DS, 0x902E, 0);
    let mut n = 0;
    for line in include_str!("vectors/dungeonpic.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let (ax, words, clicks) = setup(seed);
        let mut m = base.clone();
        for (at, v) in words {
            m.mem.w16(DS, at, v);
        }
        let mut fs = files.clone();
        let mut log = Log { log: Vec::new(), clicks };
        let ds = seg_bytes(&m, DS);
        let got = match m.dungeon_picture(&mut fs, &mut log, ax) {
            Some(()) => format!("{:x} {:x}", fnv(changes(&ds, &seg_bytes(&m, DS))), fnv(log.log.join(&b'.'))),
            None => format!("gave up: {:?}", arena::giveup::take()),
        };
        if std::env::var("PIC_DUMP").is_ok_and(|d| d == f[1]) {
            for (o, (a, b)) in ds.iter().zip(seg_bytes(&m, DS)).enumerate().filter(|(_, (a, b))| *a != b) {
                eprintln!("{o:04x} {a:02x} {b:02x}");
            }
            for l in &log.log {
                eprintln!("{l:?}");
            }
        }
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 0);
}
