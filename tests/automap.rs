//! Differential tests of the automap and the levels' kept states
//! (src/automap.rs) against vectors from the original code
//! (harness/gen_automap.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FRAME: u16 = 0x6000;
const WSEG: u16 = 0x8000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn digits(n: u8) -> [u8; 2] {
    [0x30 + n / 10, 0x30 + n % 10]
}

/// What a case is: the operation, the automap file's kind, the record
/// drawn and the slot (see harness/gen_automap.py `setup`).
struct Case {
    op: u8,
    k: u8,
    rec: usize,
    slot: u8,
}

/// Mirrors harness/gen_automap.py `setup` (a conditional's test byte
/// comes first, a tuple's items before its index).
fn setup(m: &mut Machine, files: &mut Files, seed: u32) -> Case {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() % 5;
    let v = if s.byte() < 0x20 { 1 } else { s.byte() & 0xFE };
    m.mem.w8(DS, 0x05E4, v);
    let v = s.byte();
    m.mem.w8(DS, 0x060C, v);
    let v = s.byte();
    m.mem.w8(DS, 0x060E, v);
    for at in [0x0610, 0x0612] {
        let v = if s.byte() < 0x40 { [0x1FFF, 0x2000][(s.byte() & 1) as usize] } else { s.word() };
        m.mem.w16(DS, at, v);
    }
    let v = s.word();
    m.mem.w16(DS, 0x837A, v);
    let v = if s.byte() < 0x80 { s.byte() % 2 } else { s.byte() };
    m.mem.w8(DS, 0x0A4C, v);
    let v = if s.byte() < 0x80 { 3 } else { s.byte() % 5 };
    m.mem.w8(DS, 0x0A47, v);
    let v = s.byte() % 9;
    m.mem.w8(DS, 0x0A46, v);
    let v = if s.byte() < 0xC0 { s.byte() % 4 } else { s.byte() };
    m.mem.w8(DS, 0xBB0A, v);
    let v = s.word() & !4;
    let v = v | if s.byte() < 0x80 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word() & !0x200;
    let v = v | if s.byte() < 0x40 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = s.byte() % 5;
    m.mem.w8(DS, 0x05E6, v);
    let v = s.word() & !0x4000;
    let v = v | if s.byte() < 0x40 { 0x4000 } else { 0 };
    m.mem.w16(DS, 0x05C8, v);
    let slot = 1 + s.byte() % 19;
    m.mem.w8(DS, 0x1A6C, slot);
    let v = s.byte() % 0x14;
    m.mem.w8(DS, 0x05DA, v);
    let x = s.byte();
    let v = [0, 1, x][(s.byte() % 3) as usize];
    m.mem.w8(DS, 0x6636, v);
    let rec = (s.byte() % 16) as usize;
    let v = if s.byte() < 0xE0 { rec as u32 * 0x2004 } else { s.word() as u32 };
    m.mem.w32(DS, 0x66C6, v);
    let v = s.word();
    m.mem.w16(DS, 0xC3D0, v);
    let v = s.word();
    m.mem.w16(DS, 0xC3D4, v);
    let sd = s.byte() % 20;
    m.mem.w16(DS, 0x7D0D, u16::from_le_bytes(digits(sd)));
    let ad = if op == 2 { slot } else { s.byte() % 20 };
    m.mem.w16(DS, 0x6643, u16::from_le_bytes(digits(ad)));
    for i in 0..0x200u16 {
        let b = s.byte();
        m.mem.w8(WSEG, i, b);
    }
    let mut cells = [0u8; 0x100];
    for c in cells.iter_mut() {
        let x = s.byte();
        *c = [0, 7, 0xE, 0x15, x][(s.byte() % 5) as usize];
    }
    m.mem.w16(DS, 0x8FA8, 0x44);
    for i in 0..0x4000u16 {
        m.mem.w8(FRAME, i, cells[(i & 0xFF) as usize]);
    }
    for _ in 0..s.byte() % 16 {
        let at = 0xC624 + (s.byte() % 0xC0) as u16;
        let b = s.byte();
        m.mem.w8(DS, at, b);
    }
    for _ in 0..s.byte() % 16 {
        let at = 0xC766 + s.byte() as u16;
        let b = s.byte();
        m.mem.w8(DS, at, b);
    }
    let v = s.byte();
    m.mem.w8(DS, 0xC866, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC6E4, v);
    let mut k = s.byte() % 4;
    if k == 0 && op != 2 {
        k = 2;
    }
    let mut am = b"AUTOMAP.".to_vec();
    am.extend(digits(ad));
    if k == 1 {
        let d: Vec<u8> = if s.byte() < 0xC0 {
            let n = s.byte() as usize + 1;
            (0..n).map(|_| s.byte()).collect()
        } else {
            let n = 1 + (s.byte() % 16) as usize;
            let mut d = vec![0u8; 0x20040];
            d.extend((0..n).map(|_| s.byte()));
            d
        };
        files.store(&am, d);
    } else if k >= 2 {
        let mut f = vec![0u8; 0x20040];
        for i in 0..16 {
            let lo = s.word() as u32;
            let hi = s.word() as u32;
            f[i * 0x2004..i * 0x2004 + 4].copy_from_slice(&(lo | hi << 16).to_le_bytes());
        }
        for i in 0..0x100 {
            f[rec * 0x2004 + 0x1004 + i] = s.byte();
        }
        files.store(&am, f);
    }
    if s.byte() < 0xC0 || op == 4 {
        let n = 0x1C2 * (1 + (s.byte() % 80) as usize);
        let d: Vec<u8> = (0..n).map(|_| s.byte()).collect();
        let mut name = b"STATES.".to_vec();
        name.extend(digits(sd));
        files.store(&name, d);
    }
    Case { op, k, rec, slot }
}

fn file_digest(files: &Files, prefix: &[u8]) -> u64 {
    let names: Vec<Vec<u8>> = files.written_names().into_iter().filter(|n| n.starts_with(prefix)).collect();
    if names.is_empty() {
        return 0;
    }
    let mut b = Vec::new();
    for n in names {
        b.extend(&n);
        b.extend(files.data(&n).unwrap().iter());
    }
    fnv(b)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = std::path::PathBuf::from(std::env::var("ARENA_EXE").map_or_else(
        |_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into(),
        |p| std::path::Path::new(&p).parent().unwrap().to_string_lossy().into_owned(),
    ));
    let Ok(files) = Files::new(&dir) else { return };
    let mut base = Machine::new(&img);
    // EMS on (harness/dos.py `enable_ems`): 20h banks, the driver's page
    // maps' far pointers as there.
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    for b in 0..0x20u16 {
        base.mem.w16(DS, 0x9036 + 4 * b, b * 2);
        base.mem.w16(DS, 0x9038 + 4 * b, 0x0600);
    }
    base.mem.w16(DS, 0xD806, WSEG);
    let mut n = 0;
    for line in include_str!("vectors/automap.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "automap", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let mut fs = files.clone();
        let c = setup(&mut m, &mut fs, seed);
        if c.op == 2 && c.k == 3 {
            let mut name = b"AUTOMAP.".to_vec();
            name.extend(digits(c.slot));
            let mut d = fs.data(&name).unwrap().to_vec();
            d[c.rec * 0x2004..c.rec * 0x2004 + 4].copy_from_slice(&m.automap_key().to_le_bytes());
            fs.store(&name, d);
        }
        let before = m.clone();
        let got = if c.op == 0 {
            format!("{:x}", m.automap_key())
        } else {
            match c.op {
                1 => m.automap_mark(),
                2 => m.automap_load(&mut fs),
                3 => m.automap_keep(&mut fs),
                _ if seed & 1 != 0 => m.states_load(&mut fs),
                _ => m.states_keep(&mut fs),
            }
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let frame = (0..0x5000u16).map(|o| m.mem.r8(FRAME, o));
            format!(
                "{:x} {:x} {:x} {:x}",
                fnv(changed),
                fnv(frame),
                file_digest(&fs, b"AUTOMAP."),
                file_digest(&fs, b"STATES.")
            )
        };
        assert_eq!(f[2], got, "{line}");
        n += 1;
    }
    assert!(n > 2500, "{n} compared");
}
