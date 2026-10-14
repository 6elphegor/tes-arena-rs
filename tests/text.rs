//! Differential tests of the game's text (src/text.rs `text`, 1194:A374,
//! and its substitutions 2DA2:000A) against vectors from the original code
//! (harness/gen_text.py). Needs the game directory (POINTER1.DAT,
//! TEMPLATE.DAT, NAMECHNK.DAT). Cases whose text reaches a substitution
//! not ported are counted, not compared.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::text::TextHooks;
use common::blow::Stream;
use std::path::PathBuf;

const FRAME: u16 = 0x8800;
const OUT_SEG: u16 = 0x7000;
const CSEG: u16 = 0x4C00;
const HEAP: (u16, u16) = (0xA000, 0xF000);
const CHARS: &[u8] = b" abcdefghijklmnopqrstuvwxyz";

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

/// The stubs' counters: the message box at FFE0, the menu at FFE2.
struct Count;

impl TextHooks for Count {
    fn message_box(&mut self, m: &mut Machine, _si: u16) {
        let v = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
        m.mem.w16(DS, 0xFFE0, v);
    }
    fn menu(&mut self, m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        let v = m.mem.r16(DS, 0xFFE2).wrapping_add(1);
        m.mem.w16(DS, 0xFFE2, v);
        0
    }
}

/// Mirrors harness/gen_text.py `setup`: writes the case's state, returns
/// the mode.
fn setup(m: &mut Machine, seed: u32, keys: &[u16]) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let mode = (s.byte() % 3) as u16;
    for line in include_str!("vectors/text_spec.txt").lines() {
        let line = line.split('#').next().unwrap();
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        let seg = match f[0] {
            "ds" => DS,
            "data" => DATA,
            "c" => CSEG,
            x => panic!("{x}"),
        };
        let at = hex(f[1]) as u16;
        let list = |i: usize| f[i].split(',').map(hex).collect::<Vec<u32>>();
        let v: Vec<u8> = match f[2] {
            "b" => vec![(s.byte() as u32 % hex(f[3])) as u8],
            "w" => ((s.word() as u32 % hex(f[3])) as u16).to_le_bytes().to_vec(),
            "d" => (0..4).map(|_| s.byte()).collect(),
            "bc" => {
                let c = list(3);
                vec![c[s.byte() as usize % c.len()] as u8]
            }
            "wc" => {
                let c = list(3);
                (c[s.byte() as usize % c.len()] as u16).to_le_bytes().to_vec()
            }
            "w2" => {
                let lo = (s.byte() as u32 % hex(f[3])) as u8;
                let hi = (s.byte() as u32 % hex(f[4])) as u8;
                vec![lo, hi]
            }
            "str" => {
                let mut v: Vec<u8> = (0..hex(f[3])).map(|_| CHARS[s.byte() as usize % 27]).collect();
                v.push(0);
                v
            }
            "bytes" => (0..hex(f[3])).map(|_| s.byte()).collect(),
            "seg" => CSEG.to_le_bytes().to_vec(),
            "text" => {
                let t = if s.byte() < 0xF0 {
                    let k = keys[s.word() as usize % keys.len()];
                    if k & 0x8000 != 0 { k & 0xFF } else { k }
                } else {
                    s.word() % 0x700
                };
                t.to_le_bytes().to_vec()
            }
            x => panic!("{x}"),
        };
        for (i, b) in v.into_iter().enumerate() {
            m.mem.w8(seg, at.wrapping_add(i as u16), b);
        }
    }
    mode
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let p = std::fs::read(dir.join("POINTER1.DAT")).expect("POINTER1.DAT");
    let keys: Vec<u16> = p[..0x2710].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&k| k != 0).collect();
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (mut n, mut skipped, mut stuck) = (0, 0, 0);
    for line in include_str!("vectors/text.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "text", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let mode = setup(&mut m, hex(f[1]), &keys);
        m.mem.w16(DS, 0x91F0, 0);
        m.load_rng();
        let before = m.clone();
        let got = m.text(&mut fs, &mut Count, mode, OUT_SEG, 0).map(|_| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let dchanged = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DATA, o) != before.mem.r8(DATA, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DATA, o)]);
            let text = (0..0x200u16).map(|o| m.mem.r8(OUT_SEG, o));
            let frame = (0..=0xFFFFu16).map(|o| m.mem.r8(FRAME, o));
            format!("{:x} {:x} {:x} {:x}", fnv(changed), fnv(dchanged), fnv(text), fnv(frame))
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None if f[2] == "hang" || f[2] == "fault" => stuck += 1,
            None => skipped += 1,
        }
    }
    eprintln!("text: {n} compared, {skipped} not ported, {stuck} hang or fault in both");
    assert!(n > 1000, "{n} compared");
}
