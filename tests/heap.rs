//! Differential tests of the game's heap (src/heap.rs) against vectors
//! from the original code (harness/gen_heap.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;

const HEAP_END: u32 = 0x9000;

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let vectors = include_str!("vectors/heap.txt");
    let mut n = 0;
    for line in vectors.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let (start, top) = f[1].split_once(' ').unwrap();
        let (start, top) = (hex(start) as u16, hex(top) as u16);
        let mut m = base.clone();
        m.heap_init(start, top).unwrap();
        let want: Vec<&str> = f[3].split(' ').collect();
        let mut got = Vec::new();
        for op in f[2].split(' ') {
            let p: Vec<&str> = op.split(':').collect();
            let r = match p[0] {
                "a" => m.heap_alloc(hex(p[1]) as u16).map(|(s, o)| format!("{s:x}:{o:x}")),
                "b" => m.heap_alloc_bytes(hex(p[1]) as u16).map(|(s, o)| format!("{s:x}:{o:x}")),
                "h" => m.heap_alloc_high(hex(p[1]) as u16).map(|s| format!("{s:x}:0")),
                "f" => {
                    m.heap_free(hex(p[1]) as u16);
                    Some("-".into())
                }
                "w" => {
                    let l = hex(p[1]);
                    m.mem.w16((l >> 4) as u16, (l & 15) as u16, hex(p[2]) as u16);
                    Some("-".into())
                }
                _ => panic!("{line}"),
            };
            match r {
                Some(r) => got.push(r),
                None => {
                    // The original's int3 and a cycling walk both end here.
                    let last = *want.last().unwrap();
                    assert!(last == "fault" || last == "hang", "{line}\ngot {got:?} then None");
                    got.push(last.into());
                    break;
                }
            }
        }
        assert_eq!(got, want, "{line}");
        // The harness digests up to its largest top, whatever the case's.
        let mem = (start as u32 * 16..HEAP_END * 16)
            .map(|l| m.mem.r8((l >> 4) as u16, (l & 15) as u16))
            .chain(m.mem.r16(DS, 0x8F6E).to_le_bytes());
        assert_eq!(format!("{:x}", fnv(mem)), f[4], "{line}");
        n += 1;
    }
    eprintln!("{n} heap cases");
}

/// Fixed (kept bug #59), last fit: splitting a block points the block
/// after it back at the split's new block; the original leaves it at the
/// block split.
#[test]
fn fixed_last_fit_split_relinks_the_next_block() {
    use arena::fixes::{Fixes, HEAP_LINKS};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(HEAP_LINKS)] {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.heap_init(0x2000, 0x3000).unwrap();
        let (_a, _) = m.heap_alloc(0x100).unwrap();
        let (b, _) = m.heap_alloc(0x100).unwrap();
        let (c, _) = m.heap_alloc(0x100).unwrap();
        // The rest taken, so only B's block is free once B goes.
        let rest = m.mem.r16(c + 0x100, 8);
        m.heap_alloc(rest).unwrap();
        m.heap_free(b);
        let got = m.heap_alloc_high(0x40 * 16).unwrap();
        assert_eq!(got, b);
        let new = b + 0x40;
        let back = m.mem.r16(c - 1, 6);
        assert_eq!(back, if fixes == Fixes::NONE { b - 1 } else { new }, "{fixes:?}");
    }
}

/// Fixed (kept bug #59), first fit: a next pointer to a lower segment is
/// reported as a corrupt list; the original's check compares a header's
/// segment with itself and takes the block it points to.
#[test]
fn fixed_first_fit_checks_the_order() {
    use arena::fixes::{Fixes, HEAP_LINKS};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(HEAP_LINKS)] {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.heap_init(0x2000, 0x3000).unwrap();
        let (a, _) = m.heap_alloc(0x100).unwrap();
        let rest = m.mem.r16(a + 0x100, 8);
        m.heap_alloc(rest).unwrap();
        // The last block's next pointer to a free block below the heap.
        let last = a + 0x100;
        m.mem.w16(last, 0, 0);
        m.mem.w16(last, 2, 0x1000);
        for (o, v) in [(0u16, 0u16), (2, 0), (4, 0), (6, last), (8, 0x80), (0xA, 0)] {
            m.mem.w16(0x1000, o, v);
        }
        let got = m.heap_alloc(0x10);
        if fixes == Fixes::NONE {
            assert_eq!(got, Some((0x1001, 0)));
        } else {
            assert_eq!(got, None);
        }
    }
}
