//! Differential tests of the LZSS unpacker (src/faces.rs `lzss_unpack`,
//! 0000:7608) and the faces over the view (`faces_load`, 292B:048F;
//! `faces_draw`, 292B:0530) against vectors from the original code
//! (harness/gen_faces.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FRAME: u16 = 0x8800;
const BUF: u16 = 0x7000;
const SRC: u16 = 0x8000;
const OUT: u16 = 0x7000;
const RING: u16 = 0x5C00;
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

enum Case {
    Lzss { data: Vec<u8>, ring: bool, ring_off: u16, ring_fill: u8, di: u16 },
    Faces { faces: [u8; 8], done: bool, fill: u8 },
}

/// The case. Mirrors gen_faces.py's `setup`.
fn setup(seed: u32) -> Case {
    let mut s = Stream(seed);
    if seed % 3 != 2 {
        let n = if s.byte() < 0xF0 { s.word() % 0x400 } else { s.word() % 0x2000 };
        let data = (0..n).map(|_| s.byte()).collect();
        let ring = s.byte() & 1 == 0;
        let ring_off = if s.byte() < 0x20 { s.word() } else { s.word() % 0x100 };
        let ring_fill = s.byte();
        let di = s.word();
        return Case::Lzss { data, ring, ring_off, ring_fill, di };
    }
    let mut faces = [0xFFu8; 8];
    for f in &mut faces {
        let b = s.byte();
        if b >= 0x80 {
            *f = (s.byte() % 80) | if b & 1 != 0 { 0x80 } else { 0 };
        }
    }
    let done = s.byte() < 0x18;
    let fill = s.byte();
    Case::Faces { faces, done, fill }
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
    base.mem.w16(DS, 0xBA54, 0);
    base.mem.w16(DS, 0xBA56, BUF);
    base.mem.w16(DS, 0xA900, SCREEN);
    for y in 0..200u16 {
        base.mem.w16(DS, 0xA6A0 + 2 * y, y * 0x140);
    }
    let (mut n, mut faults) = (0, 0);
    for line in include_str!("vectors/faces.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f[2] == "fault" {
            // A picture of width or height 0 drawn 65536 times over (a
            // face past its file's pictures): the original's hang.
            faults += 1;
            continue;
        }
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let got = match setup(seed) {
            Case::Lzss { data, ring, ring_off, ring_fill, di } => {
                m.mem.w16(SRC, 0, data.len() as u16);
                for (i, b) in data.iter().enumerate() {
                    m.mem.w8(SRC, 2 + i as u16, *b);
                }
                for o in 0..=0xFFFFu16 {
                    m.mem.w8(RING, o, ring_fill);
                }
                m.mem.w16(DS, 0xADBC, ring_off);
                m.mem.w16(DS, 0xADBE, if ring { RING } else { 0 });
                let ds = seg_bytes(&m, DS);
                m.lzss_unpack(SRC, 0, OUT, di).expect("unpacked");
                let heap: Vec<u8> = (HEAP.0..HEAP.1).flat_map(|s| (0..16u16).map(move |o| (s, o))).map(|(s, o)| m.mem.r8(s, o)).collect();
                format!("{:x} {:x} {:x} {:x}", fnv(seg_bytes(&m, OUT)), fnv(seg_bytes(&m, RING)), fnv(heap), fnv(changes(&ds, &seg_bytes(&m, DS))))
            }
            Case::Faces { faces, done, fill } => {
                for (i, b) in faces.iter().enumerate() {
                    m.mem.w8(DS, 0x0FD5 + i as u16, *b);
                }
                if done {
                    let v = m.mem.r16(DS, 0x05CA) | 0x2000;
                    m.mem.w16(DS, 0x05CA, v);
                }
                for o in 0..=0xFFFFu16 {
                    m.mem.w8(SCREEN, o, fill);
                }
                let ds0 = seg_bytes(&m, DS);
                let mut fs = files.clone();
                match m.faces_load(&mut fs) {
                    Some(()) => {
                        let ds1 = seg_bytes(&m, DS);
                        let r1 = format!("{:x} {:x}", fnv(m.ems_bank(0x19)), fnv(changes(&ds0, &ds1)));
                        m.faces_draw();
                        format!("{r1} {:x} {:x}", fnv(seg_bytes(&m, SCREEN)), fnv(changes(&ds1, &seg_bytes(&m, DS))))
                    }
                    None => format!("gave up: {:?}", arena::giveup::take()),
                }
            }
        };
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 0);
    assert!(faults * 10 < n, "{faults} faults of {n}");
}
