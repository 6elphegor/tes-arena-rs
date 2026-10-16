//! Differential tests of an item's description (src/use_item.rs
//! `item_info`, 1194:7CBA) against vectors from the original code
//! (harness/gen_iteminfo.py): each item put in the player's first slot,
//! 12B0 3 and 12B2 0, the changed DGROUP and DATA bytes, the carry and
//! a special item's box (its colour and text) compared by digest; where
//! the original faults or searches for ever, the port gives up. Needs the
//! game directory (an identified stack's text and a special item's are
//! the game's texts), set up as tests/text.rs does.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::text::TextHooks;

const FRAME: u16 = 0x8800;
const HEAP: (u16, u16) = (0xA000, 0xF000);

/// The box stubbed as gen_iteminfo.py does: its colour and text kept.
#[derive(Default)]
struct Shown(Vec<Vec<u8>>);

impl TextHooks for Shown {
    fn message_box_far(&mut self, m: &mut Machine, seg: u16, si: u16) {
        let mut t = vec![m.mem.r8(DS, 0x9288)];
        for i in 0..0x7530u16 {
            match m.mem.r8(seg, si.wrapping_add(i)) {
                0 => break,
                c => t.push(c),
            }
        }
        self.0.push(t);
    }
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for c in b {
        h = (h ^ c as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
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

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: the game directory not found");
        return;
    };
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/iteminfo.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let item: Vec<u8> = (0..f[1].len()).step_by(2).map(|i| u8::from_str_radix(&f[1][i..i + 2], 16).unwrap()).collect();
        let mut m = base.clone();
        for (i, &b) in item.iter().enumerate() {
            m.mem.w8(DS, 0x0212 + i as u16, b);
        }
        m.mem.w16(DS, 0x12B0, 3);
        m.mem.w16(DS, 0x12B2, 0);
        m.mem.w16(DS, 0x91F0, 0);
        m.load_rng();
        let mut fs = files.clone();
        let mut shown = Shown::default();
        let (ds, data) = (seg_bytes(&m, DS), seg_bytes(&m, DATA));
        match m.item_info(&mut fs, &mut shown, 0x0212) {
            Some(carry) => {
                m.store_rng();
                // ITEM_DUMP=<item hex>: that case's changed DGROUP bytes.
                if std::env::var("ITEM_DUMP").is_ok_and(|d| d == f[1]) {
                    for (o, (a, b)) in ds.iter().zip(seg_bytes(&m, DS)).enumerate().filter(|(_, (a, b))| *a != b) {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
                let b = shown.0.first().map_or("-".to_string(), |t| format!("{:x}", fnv(t.iter().copied())));
                let got = format!(
                    "{:x} {:x} {} {b}",
                    fnv(changes(&ds, &seg_bytes(&m, DS))),
                    fnv(changes(&data, &seg_bytes(&m, DATA))),
                    carry as u8
                );
                assert_eq!(f[2], got, "{line}");
                n += 1;
            }
            None => {
                assert!(matches!(f[2], "fault" | "hang"), "{line}: the port gave up: {:?}", arena::giveup::take());
                stuck += 1;
            }
        }
    }
    assert!(n > 0);
    eprintln!("{n} matched, {stuck} given up as the original");
}
