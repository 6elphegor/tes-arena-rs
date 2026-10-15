//! Differential tests of the loot window (src/loot_window.rs
//! `loot_window`, 2433:0583) against vectors from the original code
//! (harness/gen_lootwin.py).
mod common;

use arena::bsa::Files;
use arena::loot_window::LootHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const FSEG: u16 = 0x5C00;
const CSEG: u16 = 0x6C00;
const WSEG: u16 = 0x7C00;
const FRAME: u16 = 0xA000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

#[derive(Default)]
struct Script {
    lists: Vec<u16>,
    cfs: Vec<bool>,
    n: [usize; 2],
    log: Vec<u16>,
}

fn take<T: Copy>(q: &[T], n: &mut usize, default: T) -> T {
    let v = q.get(*n).copied().unwrap_or(default);
    *n += 1;
    v
}

impl LootHooks for Script {
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([1, si]);
    }
    fn text(&mut self, m: &mut Machine, mode: u16) {
        self.log.extend([2, m.mem.r16(DS, 0x137A), mode]);
    }
    fn sound(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([3, si]);
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16) -> u16 {
        self.log.extend([4, si, cx]);
        let v = take(&self.lists, &mut self.n[0], 0xFFFF);
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn piece_told(&mut self, _m: &mut Machine) -> bool {
        self.log.push(5);
        take(&self.cfs, &mut self.n[1], true)
    }
    fn last_piece(&mut self, _m: &mut Machine) {
        self.log.push(6);
    }
    fn piles(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        self.log.push(7);
        Some(())
    }
}

/// Writes `b` at seg:off, wrapping in the segment.
fn put(m: &mut Machine, seg: u16, off: u16, b: &[u8]) {
    for (i, &v) in b.iter().enumerate() {
        m.mem.w8(seg, off.wrapping_add(i as u16), v);
    }
}

/// Mirrors harness/gen_lootwin.py `item`.
fn item(s: &mut Stream) -> [u8; 19] {
    let mut b = [0u8; 19];
    let k = s.byte() % 4;
    b[0] = if k == 0 { 0x80 | (s.byte() % 0x10) } else { s.byte() % 0x12 };
    let cat = [0u8, 1, 3][s.byte() as usize % 3];
    b[1] = if s.byte() < 0xC0 { 0 } else { s.byte() };
    b[2] = s.byte() % 0x10;
    if cat == 3 && s.byte() < 0x20 {
        (b[1], b[2]) = (0xFF, 0x7F);
    } else if s.byte() < 0x08 {
        (b[1], b[2]) = ([0x40, 0x80][s.byte() as usize & 1], 0);
    }
    for v in b.iter_mut().take(0xE).skip(3) {
        *v = s.byte();
    }
    b[0xE] = 0x40 | cat << 2 | (s.byte() & 0x33);
    b[0xF] = if s.byte() < 0x80 { 0xFF } else { s.byte() % 9 };
    b[0x10] = s.byte() % 8;
    b[0x11] = s.byte();
    b[0x12] = s.byte();
    b
}

/// Mirrors harness/gen_lootwin.py `items`.
fn items(s: &mut Stream, n_max: u8) -> Vec<u8> {
    let n = (s.byte() as u16 % (n_max as u16 + 1)) as usize;
    let mut out = Vec::new();
    for i in 0..40 {
        if i < n {
            out.extend(item(s));
        } else {
            out.extend([0u8; 19]);
        }
    }
    if n < 39 && s.byte() < 0x40 {
        let it = item(s);
        let at = (n + 1) * 19;
        out[at..at + 19].copy_from_slice(&it);
    }
    out
}

/// Mirrors harness/gen_lootwin.py `setup` (a conditional's test byte
/// comes first); returns (ax, si, seg, bx) and the script.
fn setup(m: &mut Machine, seed: u32) -> ((u16, u16, u16, u16), Script) {
    let mut st = Stream(seed);
    let s = &mut st;
    let kind = s.byte() % 4;
    let it = items(s, 40);
    put(m, DS, 0x0212, &it);
    let v = if s.byte() < 0x40 { [32u8, 96, 160, 224][s.byte() as usize % 4] } else { s.byte() };
    m.mem.w8(DS, 0x01CD, v);
    let lo = s.word();
    let hi = s.word();
    m.mem.w16(DS, 0x05C2, lo);
    m.mem.w16(DS, 0x05C4, hi);
    let v = s.byte();
    m.mem.w8(DS, 0x05BD, v);
    let v = s.byte() as u16 % 0x24;
    m.mem.w16(DS, 0x4D77, v);
    let v = s.byte();
    m.mem.w8(DS, 0x4BF4, v);
    let v = s.word() & !0x200;
    let v = v | if s.byte() < 0x30 { 0x200 } else { 0 };
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word();
    m.mem.w16(DS, 0xC398, v);
    let v = if s.byte() < 0xC0 { 4 } else { s.byte() % 6 };
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() % 12;
    m.mem.w8(DS, 0x0D48, v);
    let v = s.byte() % 4;
    m.mem.w8(DS, 0xBB0A, v);
    let mut bank6 = vec![0u8; 0x10000];
    let mut si = s.word();
    let mut seg = DS;
    let mut bx = s.word();
    let (al, ah);
    match kind {
        0 => {
            al = s.byte() % 30;
            ah = 3;
            for i in 0..12 {
                let k = if s.byte() < 0x60 { 0xC5 } else { s.byte() % 30 };
                m.mem.w8(DS, 0x0652 + i, k);
            }
        }
        1 => {
            let staff = s.byte();
            (al, ah) = (staff, 0x0F);
            m.mem.w8(DS, 0xC941, staff);
            let v = s.byte() % 10;
            m.mem.w8(DS, 0x0F77, v);
            let v = s.byte() & 1;
            m.mem.w8(DS, 0x0F7D, v);
            bx = s.word();
            let v = s.word();
            m.mem.w16(WSEG, bx, v);
            let v = if s.byte() < 0x80 { 0 } else { s.byte() };
            m.mem.w8(FSEG, bx, v);
        }
        2 => {
            ah = s.byte() & 1;
            al = s.byte();
            si = (s.byte() as u16 % 0x30) * 0x100;
            seg = CSEG;
            let head: Vec<u8> = (0..0x6E).map(|_| s.byte()).collect();
            put(m, CSEG, si, &head);
            let n_max = if s.byte() < 0x10 { 40 } else { 8 };
            let it = items(s, n_max);
            put(m, CSEG, si + 0x6E, &it);
            let v = if s.byte() < 0x80 { 0 } else { s.word() };
            m.mem.w16(CSEG, si + 0x407, v);
            let v = s.word();
            m.mem.w16(CSEG, si + 0x40D, v);
        }
        _ => {
            al = s.byte();
            ah = 0x0F;
            let v = al ^ (1 + s.byte() % 0xFF);
            m.mem.w8(DS, 0xC941, v);
            bx = s.word() & 0x7FFE;
            let count = s.byte() % 12;
            m.mem.w8(DS, 0x0FF2, count);
            let level = s.byte() % 4;
            let place = s.byte() % 6;
            let n = count as usize + (s.byte() % 3) as usize;
            for i in 0..n {
                let mut e = [0u8; 0x1C];
                e[0] = 1;
                e[1] = if s.byte() < 0xC0 { place } else { s.byte() % 6 };
                let sq = if s.byte() < 0xC0 { bx } else { s.word() & 0x7FFE };
                e[2..4].copy_from_slice(&sq.to_le_bytes());
                e[4] = if s.byte() < 0xC0 { level } else { s.byte() % 4 };
                let g = s.byte();
                let gold = if g < 0x90 {
                    0xFFFF
                } else if g < 0x98 {
                    1
                } else {
                    s.word() % 300
                };
                e[5..7].copy_from_slice(&gold.to_le_bytes());
                e[7] = if s.byte() < 0xC0 { 0xFF } else { s.byte() % 10 };
                e[8] = s.byte();
                e[9..0x1C].copy_from_slice(&item(s));
                if s.byte() < 0x20 {
                    e[0x13..0x17].fill(0);
                }
                let at = 0xA000 + 0x1C * i;
                bank6[at..at + 0x1C].copy_from_slice(&e);
            }
            if count > 0 && s.byte() < 0x40 {
                let e = &mut bank6;
                e[0xA001] = place;
                e[0xA002..0xA004].copy_from_slice(&bx.to_le_bytes());
                e[0xA004] = level;
                e[0xA005..0xA007].copy_from_slice(&0xFFFFu16.to_le_bytes());
                e[0xA007] = 0xFF;
                e[0xA009 + 0xE] |= 0x0C;
                let mut it = [0u8; 19];
                it.copy_from_slice(&e[0xA009..0xA01C]);
                (it[1], it[2]) = (0xFF, 0x7F);
                it[0xE] = (it[0xE] | 0x4C) & 0xFD;
                put(m, DS, 0x0212, &it);
            }
            m.mem.w8(DS, 0xBB0A, level);
            let v = s.word() & 0x0FFF;
            let v = v | if s.byte() < 0x80 { 0x8000 } else { 0 };
            m.mem.w16(WSEG, bx, v);
            let v = s.byte();
            m.mem.w8(FSEG, bx, v);
        }
    }
    let lists = (0..12).map(|_| if s.byte() < 0x30 { 0xFFFF } else { s.byte() as u16 % 12 }).collect();
    let cfs = (0..4).map(|_| s.byte() & 1 != 0).collect();
    m.ems.resize(7, std::sync::Arc::new(vec![0; 0x10000]));
    m.ems[6] = std::sync::Arc::new(bank6);
    (((ah as u16) << 8 | al as u16, si, seg, bx), Script { lists, cfs, ..Default::default() })
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
    base.mem.w16(DS, 0xCCD0, FSEG);
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
    let dir = std::path::PathBuf::from(std::env::var("ARENA_EXE").map_or_else(
        |_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into(),
        |p| std::path::Path::new(&p).parent().unwrap().to_string_lossy().into_owned(),
    ));
    let Ok(mut files) = Files::new(&dir) else { return };
    let only = std::env::var("LOOTWIN_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/lootwin.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "lootwin", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let ((ax, si, seg, bx), mut sc) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let segs = [DS, CSEG, WSEG, FSEG];
        let before: Vec<Vec<u8>> = segs.iter().map(|&s| seg_bytes(&m, s)).collect();
        let got = m.loot_window(&mut files, &mut sc, ax, si, seg, bx).map(|()| {
            let after: Vec<Vec<u8>> = segs.iter().map(|&s| seg_bytes(&m, s)).collect();
            if only.is_some() {
                for (o, (a, b)) in before[0].iter().zip(&after[0]).enumerate() {
                    if a != b {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
                eprintln!("log {:?}", sc.log);
            }
            let mut d: Vec<u64> = (0..4).map(|k| fnv(changes(&before[k], &after[k]))).collect();
            d.push(fnv(m.ems_bank(6)));
            d.push(fnv(seg_bytes(&m, FRAME)));
            d.push(fnv(sc.log.iter().flat_map(|v| v.to_le_bytes())));
            d.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(" ")
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => {
                assert!(f[2] == "hang" || f[2] == "fault", "{line}: the port gave None");
                stuck += 1;
            }
        }
    }
    eprintln!("lootwin: {n} compared, {stuck} hang or fault in both");
    if only.is_none() {
        assert!(n > 2500, "{n} compared");
    }
}
