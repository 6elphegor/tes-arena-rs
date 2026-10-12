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
