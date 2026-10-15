//! Leaving an interior: the staff's flags and the stock files' deletes
//! (src/progression.rs `staff_leave`, 1194:D55F) and the piles forgotten
//! (src/loot_window.rs `piles_forget`, 2433:0199), against vectors from
//! harness/gen_leave.py.

mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::sync::Arc;

const FRAME: u16 = 0xA000;

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

fn hex(s: &str) -> Vec<u32> {
    s.split_whitespace().map(|v| u32::from_str_radix(v, 16).unwrap()).collect()
}

/// EMS on as harness/dos.py `enable_ems` sets it: 20h banks, bank 0
/// mapped into the frame.
fn base(img: &arena::exe::Image) -> Machine {
    let mut m = Machine::new(img);
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    m.mem.w16(DS, 0x8FA8, 0);
    for b in 0..0x20u16 {
        m.mem.w16(DS, 0x9036 + 4 * b, b * 2);
        m.mem.w16(DS, 0x9038 + 4 * b, 0x0600);
    }
    m
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = base(&img);
    let mut n = 0;
    for line in include_str!("vectors/leave.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let mut m = base.clone();
        match f[0] {
            "leave" => {
                for (off, v) in [0x0F76u16, 0x0F79, 0x0F7C, 0x0F7E].into_iter().zip(hex(f[1])) {
                    m.mem.w8(DS, off, v as u8);
                }
                let before = seg_bytes(&m, DS);
                let mut deleted = Vec::new();
                m.staff_leave_with(&mut |_, si| deleted.push(si as u32));
                assert_eq!(deleted, hex(f[2]), "{line}");
                let diff = changes(&before, &seg_bytes(&m, DS));
                let got: Vec<String> = diff
                    .chunks(3)
                    .map(|c| format!("{:x}:{:x}", c[0] as u16 | (c[1] as u16) << 8, c[2]))
                    .collect();
                assert_eq!(got.join(" "), f.get(3).copied().unwrap_or(""), "{line}");
            }
            "forget" => {
                let v = hex(f[1]);
                let (piles, ems, ax, fill) = (v[0] as u8, v[1] != 0, v[2] as u16, v[3]);
                m.mem.w8(DS, 0x0FF2, piles);
                let bank: Vec<u8> = (0..0x10000u32).map(|i| (i.wrapping_mul(31) + fill) as u8).collect();
                if ems {
                    m.ems.resize(7, Arc::new(vec![0; 0x10000]));
                    m.ems[6] = Arc::new(bank);
                } else {
                    m.mem.w8(DS, 0x8FA6, 0);
                    for (i, &b) in bank.iter().enumerate() {
                        m.mem.w8(FRAME, i as u16, b);
                    }
                }
                let before = seg_bytes(&m, DS);
                m.piles_forget(ax);
                let bank6 = m.ems_bank(6);
                let got = format!(
                    "{:x} {:x} {:x}",
                    fnv(changes(&before, &seg_bytes(&m, DS))),
                    fnv(bank6),
                    fnv(seg_bytes(&m, FRAME))
                );
                assert_eq!(got, f[2], "{line}");
            }
            k => panic!("unknown kind {k}"),
        }
        n += 1;
    }
    assert_eq!(n, 800);
}

/// The staff's flags as D55F leaves them, and the deletes through the
/// files: "eq*.??" and "gl*.??" take the stock files and nothing else.
#[test]
fn leave_sets_the_piece_ready_and_deletes_the_stock() {
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    let mut files = Files::default();
    for name in [&b"EQ0.12"[..], b"EQ12.3", b"GL4.56", b"GLOBAL.BSA", b"EQ1.SAV", b"QEQ.12"] {
        files.put(name, b"x");
    }
    m.mem.w8(DS, 0x0F76, 1);
    m.mem.w8(DS, 0x0F79, 1);
    m.staff_leave(&mut files);
    assert_eq!((m.mem.r8(DS, 0x0F7C), m.mem.r8(DS, 0x0F7E)), (1, 1));
    for (name, gone) in [
        (&b"EQ0.12"[..], true),
        (b"EQ12.3", true),
        (b"GL4.56", true),
        
        (b"GLOBAL.BSA", false),
        (b"EQ1.SAV", false),
        (b"QEQ.12", false),
    ] {
        assert_eq!(files.was_deleted(name), gone, "{}", String::from_utf8_lossy(name));
    }
    // Not sought: no flags.
    let mut m = Machine::new(&img);
    m.mem.w8(DS, 0x0F79, 1);
    m.staff_leave(&mut files);
    assert_eq!((m.mem.r8(DS, 0x0F7C), m.mem.r8(DS, 0x0F7E)), (0, 0));
}
