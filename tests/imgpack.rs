//! Differential tests of an image drawn by 0000:5828 with its packed
//! forms (src/imgpack.rs, src/render/sky.rs `image`) against vectors from
//! the original code (harness/gen_imgpack.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const IMG: u16 = 0x8000;
const SCREEN: u16 = 0x5C00;
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

/// Runs covering `total` bytes exactly. Mirrors gen_imgpack.py's `runs`.
fn runs(s: &mut Stream, mut total: usize) -> Vec<u8> {
    let mut out = Vec::new();
    while total > 0 {
        let n = total.min(s.byte() as usize % 0x80 + 1);
        total -= n;
        if s.byte() & 1 != 0 {
            let b = if s.byte() < 0xC0 { s.byte() } else { 0 };
            out.extend([0x80 | (n - 1) as u8, b]);
        } else {
            out.push((n - 1) as u8);
            for _ in 0..n {
                let b = if s.byte() < 0xC0 { s.byte() } else { 0 };
                out.push(b);
            }
        }
    }
    out
}

/// The image, the clip, 9207 and the screen's fill. Mirrors
/// gen_imgpack.py's `setup`.
fn setup(seed: u32) -> (Vec<u8>, [u16; 4], bool, u8) {
    let mut s = Stream(seed);
    let w = (s.byte() % 64 + 1) as usize;
    let h = (s.byte() % 40 + 1) as usize;
    let x = (s.word() % 360).wrapping_sub(30);
    let y = (s.word() % 240).wrapping_sub(30);
    let k = s.byte() % 12;
    let mut flags: u16 = [0, 1, 2, 4, 1, 2, 4, 3, 6, 5, 0, 4][k as usize];
    if s.byte() & 1 != 0 {
        flags |= 0x800;
    }
    let garbage = s.byte() < 0x10;
    let random = |s: &mut Stream, n: usize| (0..n).map(|_| s.byte()).collect::<Vec<u8>>();
    let (body, extra) = if flags & 1 != 0 {
        (runs(&mut s, w * h), 0)
    } else if flags & 2 != 0 {
        (if garbage { random(&mut s, w * h) } else { (0..h).flat_map(|_| runs(&mut s, w)).collect() }, 0)
    } else if flags & 8 != 0 {
        let n = s.byte() as usize % 300 + 2;
        let mut b = vec![(w * h) as u8, ((w * h) >> 8) as u8];
        b.extend(random(&mut s, n));
        (b, n as u16 + 2)
    } else if flags & 4 != 0 {
        let n = s.byte() as usize % (w * h / 9 + 2);
        (random(&mut s, n), n as u16)
    } else {
        (random(&mut s, w * h), 0)
    };
    let clip = if s.byte() < 0xA0 {
        [0, 0, 0x13F, 0xC7]
    } else {
        let l = (s.byte() % 100) as u16;
        let t = (s.byte() % 60) as u16;
        let r = 0x13F - (s.byte() % 100) as u16;
        let b = 0xC7 - (s.byte() % 60) as u16;
        [l, t, r, b]
    };
    let whole = s.byte() < 0x20;
    let fill = s.byte();
    let mut img: Vec<u8> = [x, y, w as u16, h as u16, flags, extra].iter().flat_map(|v| v.to_le_bytes()).collect();
    img.extend(body);
    (img, clip, whole, fill)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    base.mem.w16(DS, 0xA900, SCREEN);
    for y in 0..200u16 {
        base.mem.w16(DS, 0xA6A0 + 2 * y, y * 0x140);
    }
    let (mut n, mut faults, mut cut) = (0, 0, 0);
    for line in include_str!("vectors/imgpack.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let (image, clip, whole, fill) = setup(seed);
        let mut m = base.clone();
        for (i, b) in image.iter().enumerate() {
            m.mem.w8(IMG, i as u16, *b);
        }
        for (i, v) in clip.iter().enumerate() {
            m.mem.w16(DS, 0xA914 + 2 * i as u16, *v);
        }
        m.mem.w8(DS, 0x9207, if whole { 0xFF } else { 0 });
        for o in 0..=0xFFFFu16 {
            m.mem.w8(SCREEN, o, fill);
        }
        let mut page = vec![fill; 0x10000];
        let ds = seg_bytes(&m, DS);
        let gaps = arena::render::image_page(&mut m, &mut page, IMG, 0);
        // The block freed after the drawing can write through links its
        // unpacking ran over: where those land in the screen's segment,
        // they come after the drawing.
        for (o, p) in page.iter_mut().enumerate() {
            let b = m.mem.r8(SCREEN, o as u16);
            if b != fill {
                *p = b;
            }
        }
        if f[2] == "fault" {
            // Runs that never end: the original runs on through memory.
            assert!(!gaps.is_empty(), "{line}: the port went on");
            faults += 1;
            continue;
        }
        if gaps == ["5A6A: a picture cut to nothing"] {
            // Cut to no rows or a negative width: the original's `loop`
            // runs 65536 rows down its segment, which the renderer's page
            // (64000 bytes) leaves as a gap.
            cut += 1;
            continue;
        }
        assert!(gaps.is_empty(), "{line}: {gaps:?}");
        let heap: Vec<u8> = (HEAP.0..HEAP.1).flat_map(|s| (0..16u16).map(move |o| (s, o))).map(|(s, o)| m.mem.r8(s, o)).collect();
        let ds1 = seg_bytes(&m, DS);
        let got = format!("{:x} {:x} {:x}", fnv(page), fnv(heap), fnv(changes(&ds, &ds1)));
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 0);
    assert!(faults * 10 < n, "{faults} faults of {n}");
    assert!(cut * 10 < n, "{cut} cut to nothing of {n}");
}
