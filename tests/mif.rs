//! Differential tests of the file layer, the MIF decompressor and the
//! city block loader against vectors from the original code
//! (harness/gen_mif.py). Needs the game directory (ESarena/, next to
//! ACD.EXE): inputs are read from the user's GLOBAL.BSA at test time and
//! the vectors hold only names and digests.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::PathBuf;

const PLANES: [u16; 4] = [0x7000, 0x7800, 0x8000, 0x8800];
const PLANE_VARS: [u16; 4] = [0xCCD0, 0xD806, 0xD748, 0x9030];
const SRC_SEG: u16 = 0x5C00;
const NAME: u16 = 0x370C;
const BUF: u16 = 0xA000;

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

fn seg_digest(m: &Machine, seg: u16) -> u64 {
    fnv((0..=0xFFFFu16).map(|o| m.mem.r8(seg, o)))
}

fn plane_digests(m: &Machine) -> String {
    PLANES.map(|s| format!("{:x}", seg_digest(m, s))).join(" ")
}

fn put(m: &mut Machine, seg: u16, off: u16, bytes: &[u8]) {
    for (i, &b) in bytes.iter().enumerate() {
        m.mem.w8(seg, off.wrapping_add(i as u16), b);
    }
}

/// The BSA's directory, parsed independently of the port.
struct Dir {
    bsa: Vec<u8>,
    entries: Vec<(Vec<u8>, usize, usize)>,
}

impl Dir {
    fn new(bsa: Vec<u8>) -> Self {
        let count = u16::from_le_bytes([bsa[0], bsa[1]]) as usize;
        let d = bsa.len() - 18 * count;
        let mut off = 2;
        let entries = (0..count)
            .map(|i| {
                let r = &bsa[d + 18 * i..d + 18 * i + 18];
                let name = r[..14].split(|&b| b == 0).next().unwrap().to_vec();
                let size = u32::from_le_bytes(r[14..18].try_into().unwrap()) as usize;
                off += size;
                (name, off - size, size)
            })
            .collect();
        Dir { bsa, entries }
    }

    fn file(&self, name: &str) -> &[u8] {
        let (_, off, size) = self.entries.iter().find(|e| e.0 == name.as_bytes()).unwrap();
        &self.bsa[*off..off + size]
    }

    /// Payload of the k-th FLOR/MAP1/MAP2 chunk, walked as 1194:03FA does.
    fn chunk(&self, name: &str, k: usize) -> Vec<u8> {
        let data = self.file(name);
        let (mut p, mut found) = (6 + 0x3D + 6, vec![]);
        while p + 6 <= data.len() {
            let n = u16::from_le_bytes([data[p + 4], data[p + 5]]) as usize;
            let tag = &data[p..p + 4];
            p += 6;
            if [&b"FLOR"[..], b"MAP1", b"MAP2"].contains(&tag) {
                found.push(data[p..(p + n).min(data.len())].to_vec());
            }
            p += n;
        }
        found.swap_remove(k)
    }
}

/// 400 source words for the copy tests (same LCG as the generator).
fn copy_words(seed: u32) -> Vec<u16> {
    let mut x = seed;
    (0..400)
        .map(|_| {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            [0xC00, 0xE00, 0xD00, (x >> 8) as u16][(x >> 16) as usize & 3]
        })
        .collect()
}

/// The wall plane for the wall-pass tests (same LCG and bias as the
/// generator).
fn wall_words(seed: u32) -> Vec<u16> {
    let mut x = seed;
    (0..0x4000)
        .map(|_| {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            let (t, v) = ((x >> 16) & 7, (x >> 4) as u16);
            match t {
                0 | 1 => 0x8000 | (v & 0x0F00) | (v & 0x1F),
                2 | 3 => 0x9000 | (v & 0x0F00) | (0x34 + (v & 1)),
                4 => 0x9000 | (v & 0x0FFF),
                _ => v,
            }
        })
        .collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let bsa = Dir::new(std::fs::read(dir.join("GLOBAL.BSA")).unwrap());
    let mut base = Machine::new(&img);
    assert!(base.bsa_open(&mut files));
    // The harness's heap (harness/dos.py).
    base.heap_init(0xA000, 0xF000).unwrap();
    for (v, s) in PLANE_VARS.iter().zip(PLANES) {
        base.mem.w16(DS, *v, s);
    }
    for s in PLANES {
        for i in 0..=0xFFFFu16 {
            base.mem.w8(s, i, (i as u32 * 7 + 3) as u8);
        }
    }

    let mut counts = std::collections::HashMap::<&str, usize>::new();
    for line in include_str!("vectors/mif.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let (mut m, mut fs) = (base.clone(), files.clone());
        match f[0] {
            "open" => {
                let want = format!("{:x} {:x}", m.mem.r16(DS, 0xA926), m.mem.r32(DS, 0xA922));
                assert_eq!(f[1], want, "{line}");
            }
            "find" => {
                let mut name = (0..f[1].len() / 2).map(|i| hex(&f[1][2 * i..2 * i + 2]) as u8).collect::<Vec<_>>();
                name.push(0);
                put(&mut m, DS, NAME, &name);
                let found = m.bsa_find(&mut fs, NAME);
                let got = format!(
                    "{:x} {:x} {:x} {:x}",
                    !found as u8,
                    m.mem.r32(DS, 0x9198),
                    m.mem.r32(DS, 0x9184),
                    m.mem.r8(DS, 0x9190)
                );
                assert_eq!(f[2], got, "{line}");
            }
            "read" => {
                put(&mut m, DS, NAME, format!("{}\0", f[1]).as_bytes());
                let mut res = vec![format!("o{}", m.file_open(&mut fs, NAME).is_none() as u8)];
                let mut data = vec![];
                for op in f[2].split(',') {
                    if let Some(n) = op.strip_prefix('r') {
                        let n = hex(n) as u16;
                        let fill: Vec<u8> = (0..n as usize).map(|i| i as u8).collect();
                        put(&mut m, DS, BUF, &fill);
                        res.push(format!("{:x}", m.file_read(&mut fs, DS, BUF, n)));
                        data.extend((0..n).map(|i| m.mem.r8(DS, BUF.wrapping_add(i))));
                    } else {
                        let (w, off) = op[1..].split_once(':').unwrap();
                        let r = m.file_seek(&mut fs, w.parse().unwrap(), hex(off) as u32);
                        res.push(format!("{:x}", r as u16));
                    }
                }
                m.file_close(&mut fs);
                assert_eq!(f[3], res.join(" "), "{line}");
                assert_eq!(hex(f[4]), fnv(data), "{line}");
            }
            "lzhuf" => {
                let src = match f[1].split(':').collect::<Vec<_>>()[..] {
                    ["bsa", name, k] => bsa.chunk(name, k.parse().unwrap()),
                    ["cut", name, k, n] => bsa.chunk(name, k.parse().unwrap())[..hex(n) as usize].to_vec(),
                    ["hex", h] => (0..h.len() / 2).map(|i| hex(&h[2 * i..2 * i + 2]) as u8).collect(),
                    _ => panic!("{line}"),
                };
                put(&mut m, SRC_SEG, 0, &src);
                let ax = m.mif_decompress(SRC_SEG, 0, PLANES[3], 0, src.len() as u16).expect("heap");
                assert_eq!(hex(f[2]), ax as u64, "{line}");
                assert_eq!(hex(f[3]), seg_digest(&m, PLANES[3]), "{line}");
                let mut g = vec![m.mem.r8(DS, 0xB53E) as u16, m.mem.r8(DS, 0xB53F) as u16];
                g.extend([0xB540, 0xB542, 0xEEE6, 0xEEE8, 0xEECA, 0xEECC, 0xEECE, 0xEED0, 0xEED2, 0xEED4].map(|o| m.mem.r16(DS, o)));
                let g: Vec<String> = g.iter().map(|v| format!("{v:x}")).collect();
                assert_eq!(f[4], g.join(" "), "{line}");
            }
            "copy" => {
                let p: Vec<&str> = f[1].split(' ').collect();
                let words = copy_words(hex(p[2]) as u32);
                for (i, w) in words.iter().enumerate() {
                    m.mem.w16(PLANES[3], 2 * i as u16, *w);
                }
                m.mem.w16(DS, 0xA888, hex(p[1]) as u16);
                match p[0] {
                    "flor" => m.mif_copy_floor(),
                    "map1" => m.mif_copy_map1(),
                    "map2" => m.mif_copy_map2(),
                    _ => panic!("{line}"),
                }
                assert_eq!(f[2], plane_digests(&m), "{line}");
            }
            "wall" => {
                let p: Vec<u64> = f[1].split(' ').map(hex).collect();
                for (i, w) in wall_words(p[0] as u32).iter().enumerate() {
                    m.mem.w16(PLANES[1], 2 * i as u16, *w);
                }
                m.mem.w8(PLANES[1], 0xBB09, p[1] as u8);
                m.mem.w16(DS, 0xC357, p[2] as u16);
                m.mem.w16(DS, 0xC359, p[3] as u16);
                for (o, t) in [0xC945, 0xC946, 0xC947, 0xC948, 0xC951].iter().zip(&p[4..9]) {
                    m.mem.w8(PLANES[1], *o, *t as u8);
                }
                m.mif_wall_pass();
                assert_eq!(f[2], plane_digests(&m), "{line}");
            }
            "block" => {
                let (name, pos) = f[1].split_once(' ').unwrap();
                m.mem.w16(DS, 0xA888, hex(pos) as u16);
                put(&mut m, DS, NAME, format!("{name}\0").as_bytes());
                let got = match m.mif_load_block(&mut fs, NAME) {
                    Some(()) => plane_digests(&m),
                    None => "fault".into(),
                };
                assert_eq!(f[2], got, "{line}");
            }
            _ => panic!("unknown vector {line}"),
        }
        *counts.entry(f[0]).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert_eq!(counts.len(), 7);
}

/// A machine with the decoder's heap and `src` at SRC_SEG:0.
fn mif_machine(img: &arena::exe::Image, fixes: arena::fixes::Fixes, src: &[u8]) -> Machine {
    let mut m = Machine::new(img);
    m.fixes = fixes;
    m.heap_init(0xA000, 0xF000).unwrap();
    put(&mut m, SRC_SEG, 0, src);
    m
}

/// Fixed (kept bug #77): GetByte takes the end of the input as 0, as
/// GetBit does, so a stream that runs out decodes as if zeros followed;
/// the original's always-taken test lets FFFFh in. And a size of 0 frees
/// the decoder's tables; the original leaks them.
#[test]
fn fixed_mif_end_of_input_and_empty_size() {
    use arena::fixes::{Fixes, MIF_EOF};
    let Some(img) = common::image() else { return };
    let fixed = Fixes::only(MIF_EOF);
    let decode = |fixes: Fixes, src: &[u8], len: u16| {
        let mut m = mif_machine(&img, fixes, src);
        let n = m.mif_decompress(SRC_SEG, 0, PLANES[3], 0, len).unwrap();
        (n, (0..0x400u16).map(|o| m.mem.r8(PLANES[3], o)).collect::<Vec<u8>>())
    };
    let mut differ = 0;
    for seed in 1..60u32 {
        // A random stream claiming more output than it holds.
        let mut s = common::blow::Stream(seed);
        let mut src = vec![0x00, 0x03];
        src.extend((0..40).map(|_| s.byte()));
        let len = src.len() as u16;
        let mut padded = src.clone();
        padded.extend([0; 0x400]);
        let want = decode(Fixes::NONE, &padded, padded.len() as u16);
        assert_eq!(decode(fixed, &src, len), want, "seed {seed}");
        if decode(Fixes::NONE, &src, len) != want {
            differ += 1;
        }
    }
    assert!(differ > 10, "{differ} streams told apart");
    // A size of 0.
    for fixes in [Fixes::NONE, fixed] {
        let mut m = mif_machine(&img, fixes, &[0, 0, 1, 2]);
        let free = m.mem.r16(DS, 0x8F6E);
        m.mif_decompress(SRC_SEG, 0, PLANES[3], 0, 4).unwrap();
        let after = m.mem.r16(DS, 0x8F6E);
        if fixes == Fixes::NONE {
            assert!(after < free, "{after:x} {free:x}");
        } else {
            assert_eq!(after, free);
        }
    }
}
