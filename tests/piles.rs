//! Differential tests of the piles of treasure made (src/piles.rs
//! `piles`, 2433:0000) against vectors from the original code
//! (harness/gen_piles.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const WSEG: u16 = 0x7C00;
const FRAME: u16 = 0xA000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn put(m: &mut Machine, at: u16, data: &[u8]) {
    for (i, &b) in data.iter().enumerate() {
        m.mem.w8(DS, at.wrapping_add(i as u16), b);
    }
}

/// Mirrors harness/gen_piles.py `setup`, draw for draw. Returns bank 6
/// (or the frame's bytes) and whether EMS is on.
fn setup(m: &mut Machine, seed: u32) -> (Vec<u8>, bool) {
    let mut st = Stream(seed);
    let s = &mut st;
    let making = s.byte() < 0xC0;
    m.mem.w8(DS, 0x4C91, if making { 0xFF } else { 0 });
    let ems = s.byte() < 0xC0;
    let count = match s.byte() % 8 {
        0 => 0,
        1 => 0xC8,
        2 => s.byte() % 0xC8,
        _ => s.byte() % 6,
    };
    m.mem.w8(DS, 0x0FF2, count);
    let v = [3, 4, 4, 2][s.byte() as usize % 4];
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() % 0xC;
    m.mem.w8(DS, 0x0D48, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    let spots = match s.byte() % 8 {
        0 => s.byte(),
        1 => s.byte() % 3,
        _ => 2 + s.byte() % 20,
    };
    m.mem.w8(DS, 0xC765, spots);
    let mut v = s.word() & !0x200;
    if s.byte() < 0x20 {
        v |= 0x200;
    }
    m.mem.w16(DS, 0xC39A, v);
    let mut v = s.word() & !4;
    if s.byte() < 0x50 {
        v |= 4;
    }
    m.mem.w16(DS, 0xC398, v);
    let level = if s.byte() < 0xE0 { s.byte() % 8 } else { s.byte() };
    m.mem.w8(DS, 0xBB0A, level);
    let climate = if s.byte() < 0xE0 { s.byte() % 3 } else { s.byte() };
    m.mem.w8(DS, 0x0A47, climate);
    let v = s.word();
    m.mem.w16(DS, 0x12C0, v);
    let v = if s.byte() < 0xC0 { (s.byte() % 8) as u16 } else { s.word() };
    m.mem.w16(DS, 0x12C4, v);
    let seed: Vec<u8> = (0..4).map(|_| s.byte()).collect();
    put(m, 0x91F6, &seed);
    m.mem.w16(DS, 0x91F0, 0);
    for i in 0..4u16 {
        let v = if s.byte() < 0xC0 { 0 } else { s.byte() };
        m.mem.w8(DS, 0x4C8D + i, v);
    }
    let rec: Vec<u8> = (0..0x1C).map(|_| s.byte()).collect();
    put(m, 0x4BF6, &rec);
    let v = if s.byte() < 0xE0 { s.byte() % 8 } else { s.byte() };
    m.mem.w8(DS, 0x4D76, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CC, v);
    let v = s.word();
    m.mem.w16(DS, 0x05CE, v);
    let mut spot = Vec::new();
    for _ in 0..130 {
        let k = s.byte();
        spot.push(if k < 0x20 {
            0xFF
        } else if k < 0x50 {
            0x80 + s.byte() % 0x7F
        } else {
            s.byte() % 0x40
        });
        spot.push(s.byte() % 0x40);
    }
    put(m, 0xC6E5, &spot);
    let flats: Vec<u8> = (0..8).map(|_| s.byte()).collect();
    put(m, 0xC934, &flats);
    let taken: Vec<u8> = (0..0x40).map(|_| s.byte()).collect();
    put(m, 0x1395, &taken);
    let mut bank = vec![0u8; 0x10000];
    for i in 0..count as usize {
        let at = 0xA000 + 0x1C * i;
        for j in 0..0x1C {
            bank[at + j] = s.byte();
        }
        bank[at + 4] = if s.byte() < 0x80 { level } else { s.byte() % 8 };
    }
    bank[0x4D76] = s.byte() % 8;
    m.load_rng();
    (bank, ems)
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
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    // EMS on (harness/dos.py `enable_ems`): 20h banks, bank 0 mapped
    // into the frame; the driver's page maps' far pointers as there.
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    for b in 0..0x20u16 {
        base.mem.w16(DS, 0x9036 + 4 * b, b * 2);
        base.mem.w16(DS, 0x9038 + 4 * b, 0x0600);
    }
    let only = std::env::var("PILES_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/piles.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "piles", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let (bank, ems) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        if ems {
            m.ems.resize(7, vec![0; 0x10000]);
            m.ems[6] = bank;
        } else {
            m.mem.w8(DS, 0x8FA6, 0);
            for (i, &b) in bank.iter().enumerate() {
                m.mem.w8(FRAME, i as u16, b);
            }
        }
        let before = (seg_bytes(&m, DS), seg_bytes(&m, WSEG));
        let got = m.piles().map(|()| {
            m.store_rng();
            let after = seg_bytes(&m, DS);
            if only.is_some() {
                for (o, (a, b)) in before.0.iter().zip(&after).enumerate() {
                    if a != b {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
            }
            let d = [
                fnv(changes(&before.0, &after)),
                fnv(m.ems_bank(6)),
                fnv(seg_bytes(&m, FRAME)),
                fnv(changes(&before.1, &seg_bytes(&m, WSEG))),
            ];
            d.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(" ")
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => {
                assert_eq!(f[2], "stuck", "{line}: the port gave None");
                stuck += 1;
            }
        }
    }
    eprintln!("piles: {n} compared, {stuck} stuck in both");
    if only.is_none() {
        assert!(n > 3000, "{n} compared");
        assert!(stuck < 400, "{stuck} stuck");
    }
}
