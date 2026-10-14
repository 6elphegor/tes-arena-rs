//! Differential tests of clicking on people and things (src/talk.rs
//! `actor_click`, 1194:DB6C, with the object click 292B:06DC) against
//! vectors from the original code (harness/gen_talk.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::talk::TalkHooks;
use common::blow::Stream;

const WSEG: u16 = 0x6000;
const FSEG: u16 = 0x7000;
const CSEG: u16 = 0x8000;
const PSEG: u16 = 0x5C00;
const SCRATCH: u16 = 0x400;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The case's answers (harness/gen_talk.py `setup`'s), taken in order.
#[derive(Default)]
struct Script {
    menus: Vec<u16>,
    lists: Vec<u16>,
    cfs: Vec<bool>,
    texts: Vec<(u16, u16)>,
    n: [usize; 4],
    log: Vec<u16>,
}

fn take<T: Copy>(q: &[T], n: &mut usize, default: T) -> T {
    let v = q.get(*n).copied().unwrap_or(default);
    *n += 1;
    v
}

impl Script {
    fn cf(&mut self) -> bool {
        take(&self.cfs, &mut self.n[2], true)
    }
}

impl TalkHooks for Script {
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([1, si]);
    }
    fn text(&mut self, m: &mut Machine, mode: u16) {
        self.log.extend([2, m.mem.r16(DS, 0x137A), mode]);
    }
    fn menu(&mut self, _m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([3, si, bx, di]);
        take(&self.menus, &mut self.n[0], 3)
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16) -> u16 {
        self.log.extend([4, si, cx]);
        take(&self.lists, &mut self.n[1], 0xFFFF)
    }
    fn way(&mut self, _m: &mut Machine, si: u16, x: u16, y: u16) {
        self.log.extend([5, si, x, y]);
    }
    fn loot(&mut self, _m: &mut Machine, _files: &mut Files, ax: u16, _si: u16, _seg: u16, _bx: u16) -> Option<()> {
        self.log.extend([6, ax]);
        Some(())
    }
    fn sound(&mut self, _m: &mut Machine, ax: u16, cx: u16) {
        self.log.extend([7, ax, cx]);
    }
    fn expand(&mut self, m: &mut Machine, di: u16) -> u16 {
        self.log.extend([8, di]);
        let mut di = di;
        while m.mem.r8(DS, di) != 0 {
            di = di.wrapping_add(1);
        }
        di
    }
    fn touch_cast(&mut self, _m: &mut Machine, _files: &mut Files, si: u16) -> Option<bool> {
        self.log.extend([9, si]);
        Some(self.cf())
    }
    fn quest_item(&mut self, _m: &mut Machine, si: u16) -> bool {
        self.log.extend([10, si]);
        self.cf()
    }
    fn quest_person(&mut self, _m: &mut Machine) {
        self.log.push(11);
    }
    fn palace(&mut self, _m: &mut Machine) {
        self.log.push(12);
    }
    fn quest_offer(&mut self, _m: &mut Machine) -> bool {
        self.log.push(13);
        self.cf()
    }
    fn quest_chest(&mut self, _m: &mut Machine, si: u16) -> bool {
        self.log.extend([14, si]);
        self.cf()
    }
    fn shop(&mut self, _m: &mut Machine, _files: &mut Files, kind: u8) -> Option<()> {
        self.log.extend([15, kind as u16]);
        Some(())
    }
    fn wild_flat(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([16, si]);
    }
    fn dungeon_entrance(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        self.log.push(17);
        Some(())
    }
    fn wild_way(&mut self, _m: &mut Machine, kind: u16) {
        self.log.extend([18, kind]);
    }
    fn square_text(&mut self, _m: &mut Machine, ax: u16) -> Option<(u16, u16)> {
        self.log.extend([19, ax]);
        if self.cf() {
            return None;
        }
        Some(take(&self.texts, &mut self.n[3], (0xC766, 0)))
    }
}

/// Mirrors harness/gen_talk.py `setup`.
fn setup(m: &mut Machine, seed: u32) -> Script {
    let mut st = Stream(seed);
    let s = &mut st;
    let w = |m: &mut Machine, o: u16, v: u16| m.mem.w16(DS, o, v);
    let b = |m: &mut Machine, o: u16, v: u8| m.mem.w8(DS, o, v);
    let near = |s: &mut Stream, c: u16| c.wrapping_add(s.word() % 0x380).wrapping_sub(0x1C0);
    let close = |s: &mut Stream, c: u16| c.wrapping_add(s.word() % 0x200).wrapping_sub(0x100);
    let px = 0x1000 + s.word() % 0x2000;
    let py = 0x1000 + s.word() % 0x2000;
    w(m, 0xC3D0, px);
    w(m, 0xC3D4, py);
    w(m, 0xCB90, ((py >> 7) << 8) + ((px >> 7) << 1));
    let base = 0x20 + (s.byte() % 0x40) as u16;
    w(m, 0xC38E, base);
    for o in 0xC934..0xC98Fu16 {
        let v = s.byte() % 0x40;
        b(m, o, v);
    }
    let v = if s.byte() < 0x20 { 0xFF } else { s.byte() % 0x40 };
    b(m, 0xC942, v);
    let a46 = s.byte() % 9;
    b(m, 0x0A46, a46);
    for i in 0..10u16 {
        let v = [0, 5, 7, 1][(s.byte() % 4) as usize];
        b(m, 0x8223 + i, v);
        if i < 8 {
            let v = if s.byte() < 0x80 { a46 } else { s.byte() % 9 };
            b(m, 0x80EC + i, v);
        }
    }
    let v = (if s.byte() < 0x80 { 4 } else { 0 }) | if s.byte() & 1 != 0 { 0x8000 } else { 0 };
    w(m, 0xC398, v);
    let v = (if s.byte() < 0x50 { 0x200 } else { 0 }) | 0x80 | if s.byte() & 1 != 0 { 0x1000 } else { 0 };
    w(m, 0xC39A, v);
    let v = if s.byte() < 0x40 { 0x80 } else { 0 };
    w(m, 0x05BE, v);
    let v = if s.byte() < 0x20 { 0x4000 } else { 0 };
    w(m, 0x05C8, v);
    let v = (if s.byte() < 0x50 { 1 } else { 0 }) | if s.byte() & 1 != 0 { 2 } else { 0 };
    w(m, 0x05CA, v);
    let v = [0, 3, 4, 4][(s.byte() % 4) as usize];
    b(m, 0x05E6, v);
    let v = [1, 4, 5, 6, 7, 2][(s.byte() % 6) as usize];
    b(m, 0x0D48, v);
    let v = s.byte() & 1;
    b(m, 0x0F76, v);
    let v = s.byte() % 10;
    b(m, 0x0F77, v);
    if s.byte() < 0x80 {
        // The quest's city name (80F4, by F77) made the current one's.
        let mut at = m.skip_strings(DS, 0x80F4, v as u16);
        let mut d = 0x0A22u16;
        loop {
            let c = m.mem.r8(DS, at);
            m.mem.w8(DS, d, c);
            at += 1;
            d += 1;
            if c == 0 {
                break;
            }
        }
    }
    for o in [0x0F78, 0x0F79, 0x0F7B, 0x0F7D] {
        let v = s.byte() & 1;
        b(m, o, v);
    }
    let v = s.byte() % 4;
    b(m, 0x0A47, v);
    let v = s.byte();
    b(m, 0x0A4C, v);
    let v = s.byte() % 30;
    b(m, 0x01AA, v);
    let v = s.byte() & 1;
    b(m, 0x01AB, v);
    let v = s.word();
    w(m, 0x0151, v);
    let pick = [0, 0, 0x7D0, s.word()];
    let v = pick[(s.byte() % 4) as usize];
    w(m, 0x0DC4, v);
    let v = (s.byte() & 1) as u16;
    w(m, 0xA902, v);
    let v = s.word();
    w(m, 0x1030, v);
    let v = s.word();
    w(m, 0x91F6, v);
    let v = s.word();
    w(m, 0x91F8, v);
    for o in [0x01A4 + 0x2A, 0x01A4 + 0x2C, 0x01A4 + 6] {
        let v = s.byte();
        b(m, o, v);
    }
    let v = s.byte() % 18;
    b(m, 0x01A4 + 5, v);
    for i in 0..0x20u16 {
        let v = if s.byte() < 0xC0 {
            0xFFFF
        } else {
            let y = near(s, py);
            let x = near(s, px);
            ((y >> 7) << 8).wrapping_add((x >> 7) << 1)
        };
        w(m, 0x0F88 + 2 * i, v);
    }
    let slots: Vec<u16> = (0..23).map(|i| 0x0AB3 + 0x1C * i).collect();
    for k in 0..8u16 {
        let at = 0x0622 + 6 * k;
        let v = if s.byte() < 0x80 { slots[15 + (s.byte() % 8) as usize] } else { 0xFFFF };
        w(m, at, v);
        let v = 0xC766u16.wrapping_add(4 * (s.byte() % 0x20) as u16).wrapping_add(0x389A);
        w(m, at + 2, v);
        let v = if s.byte() < 0x60 { 0x7B } else { s.byte() };
        b(m, at + 4, v);
    }
    let corpse = s.byte() % 0x40;
    b(m, 0xC936, corpse);
    for (i, &si) in slots.iter().enumerate() {
        let v = close(s, px);
        w(m, si, v);
        let v = close(s, py);
        w(m, si + 4, v);
        let k = s.byte() % 4;
        // (The generator's tuples take every byte, whichever is used.)
        let spr = if i >= 15 {
            [0xC0 + s.byte() % 6, (corpse as u16 + base) as u8, 0xC5, s.byte()][k as usize]
        } else {
            [0xC0 + s.byte() % 6, 0xC0 + s.byte() % 6, (base + (s.byte() % 0x40) as u16) as u8, s.byte()][k as usize]
        };
        b(m, si + 0xC, spr);
        let v = s.word();
        let v = v & if s.byte() < 0x40 { 0xFFFF } else { 0xBFFF };
        w(m, si + 0xF, v);
        let v = if i >= 15 { 0x500 * (i as u16 - 15) } else { s.word() };
        w(m, si + 0x12, v);
    }
    for k in 0..8u16 {
        let rec = 0x500 * k;
        for i in 0..0x20 {
            let v = s.byte();
            m.mem.w8(CSEG, rec + i, v);
        }
        let v = s.word();
        m.mem.w16(CSEG, rec + 0x40D, v);
    }
    let rec = 0x3D96;
    let v = close(s, px);
    w(m, rec, v);
    let v = close(s, py);
    w(m, rec + 4, v);
    let kinds = [0xC93Bu16, 0xC93C, 0xC93F, 0xC940, 0xC942, 0xC943, 0xC935, 0xC934];
    let k = s.byte() % 13;
    let v = match k {
        11 | 12 => base + (s.byte() % 3) as u16,
        10 => (s.byte() % 0x40) as u16 + base,
        _ => 0,
    };
    b(m, rec + 0xC, v as u8);
    let table = match k {
        0..=7 => Some(kinds[k as usize]),
        8 => Some(0xC944 + (s.byte() % 16) as u16),
        9 => Some(0xC987 + (s.byte() % 8) as u16),
        _ => None,
    };
    if let Some(t) = table {
        let v = (m.mem.r8(DS, t) as u16).wrapping_add(m.mem.r16(DS, 0xC38E)) as u8;
        b(m, rec + 0xC, v);
    }
    let v = s.word();
    w(m, rec + 0xF, v);
    let v = s.word();
    w(m, rec + 0x12, v);
    let choices = [0, slots[(s.byte() % 15) as usize], slots[15 + (s.byte() % 8) as usize], rec, rec, rec, 0xD7DE];
    let v = choices[(s.byte() % 7) as usize];
    w(m, 0xCB5E, v);
    for dy in -8i32..9 {
        for dx in -8i32..9 {
            let sq = (((((py >> 7) as i32) + dy) << 8) + ((((px >> 7) as i32) + dx) << 1)) as u16;
            let v = s.byte();
            if v < 0x60 {
                let x = 0x8000 | s.word() & 0xFF;
                m.mem.w16(WSEG, sq, x);
            } else if v < 0x90 {
                let x = s.word() & 0x7FFF;
                m.mem.w16(WSEG, sq, x);
            }
            if s.byte() < 0x40 {
                let x = s.word();
                m.mem.w16(FSEG, sq, x);
            }
        }
    }
    for (i, (ptr, cnt)) in [(0x1AB5u16, 0x1387u16), (0x1004, 0x1389), (0x1008, 0x138B)].into_iter().enumerate() {
        let n = (s.byte() % 6) as u16;
        w(m, cnt, n);
        w(m, ptr, 0x100 * i as u16);
        w(m, ptr + 2, PSEG);
        for e in 0..n {
            let at = 0x100 * i as u16 + 4 * e;
            let r = [41u16, 5][(s.byte() & 1) as usize];
            let x = ((px >> 7) + s.byte() as u16 % r).wrapping_sub(r / 2) as u8;
            let y = ((py >> 7) + s.byte() as u16 % r).wrapping_sub(r / 2) as u8;
            let a = s.byte() % 8;
            let c = s.byte() % 6;
            for (j, v) in [x, y, a, c].into_iter().enumerate() {
                m.mem.w8(PSEG, at + j as u16, v);
            }
        }
    }
    let v = s.byte();
    b(m, 0x138D, v);
    let v = s.byte();
    b(m, 0x138E, v);
    let v = s.byte() as u16;
    w(m, 0xC36B, v);
    let v = s.byte() as u16;
    w(m, 0xC36D, v);
    let v = 1 + s.byte() % 4;
    b(m, 0xC343, v);
    let (mut gx, mut gy) = (0, 0);
    for i in 0..4u16 {
        // (Else the one before again.)
        if !(i != 0 && s.byte() < 0x40) {
            gx = near(s, px);
            gy = near(s, py);
        }
        w(m, 0xC344 + 2 * i, gx);
        w(m, 0xC34C + 2 * i, gy);
    }
    for k in 0..4u16 {
        let at = 0x0DCA + 0x29 * k;
        let v = s.word();
        w(m, at + 4, v);
        let r = [257u16, 9][(s.byte() & 1) as usize];
        let v = ((px >> 7) + s.word() % r).wrapping_sub(r / 2) as u8;
        b(m, at + 0x10, v);
        let v = ((py >> 7) + s.word() % r).wrapping_sub(r / 2) as u8;
        b(m, at + 0x11, v);
        let v = s.byte();
        b(m, at + 0x1E, v);
    }
    for i in 0..0x34 {
        let v = s.byte();
        b(m, 0x0F3E + i, v);
    }
    let v = if s.byte() < 0xC0 { 0 } else { s.word() };
    w(m, 0x0F3E + 4, v);
    let mut sc = Script::default();
    for _ in 0..8 {
        sc.menus.push([0, 1, 2, 3, 0xFFFF, 0, 1, 2][(s.byte() % 8) as usize]);
    }
    for _ in 0..8 {
        sc.lists.push(if s.byte() < 0x20 { 0xFFFF } else { (s.byte() % 11) as u16 });
    }
    for _ in 0..16 {
        sc.cfs.push(s.byte() & 1 != 0);
    }
    for _ in 0..4 {
        let r = 0xC766 + 4 * (s.byte() % 0x20) as u16;
        let bx = s.byte() as u16;
        sc.texts.push((r, bx));
    }
    sc
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.heap_init(0xA000, 0xF000).unwrap();
    for (o, v) in [(0xD806u16, WSEG), (0xCCD0, FSEG), (0x1042, CSEG), (0x91F0, 0), (0xCB8E, SCRATCH)] {
        base.mem.w16(DS, o, v);
    }
    let (seg, off) = base.heap_alloc_bytes(SCRATCH).unwrap();
    base.mem.w16(DS, 0xBA54, off);
    base.mem.w16(DS, 0xBA56, seg);
    let dir = std::path::PathBuf::from(std::env::var("ARENA_EXE").map_or_else(
        |_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into(),
        |p| std::path::Path::new(&p).parent().unwrap().to_string_lossy().into_owned(),
    ));
    let Ok(mut files) = Files::new(&dir) else { return };
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    let only = std::env::var("TALK_SEED").ok();
    for line in include_str!("vectors/talk.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "talk", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut sc = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        if only.is_some() {
            let r = |o: u16| m.mem.r16(DS, o);
            eprintln!("dc4 {:x} 5ca {:x} 1aa {:x} seed {:x} clicked {:x} f88 {:x?}", r(0xDC4), r(0x5CA), m.mem.r8(DS, 0x1AA), m.rng.seed, r(0xCB5E), (0..32).map(|i| r(0xF88 + 2 * i)).collect::<Vec<_>>());
        }
        let before = m.clone();
        let got = m.actor_click(&mut files, &mut sc).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let (sseg, soff) = (m.mem.r16(DS, 0xBA56), m.mem.r16(DS, 0xBA54));
            format!(
                "{:x} {:x} {:x} {:x} {:x} {:x}",
                fnv(changed),
                fnv((0..=0xFFFFu16).map(|o| m.mem.r8(WSEG, o))),
                fnv((0..=0xFFFFu16).map(|o| m.mem.r8(FSEG, o))),
                fnv((0..SCRATCH).map(|o| m.mem.r8(sseg, soff.wrapping_add(o)))),
                m.mem.r8(DATA, 0x7387),
                fnv(sc.log.iter().flat_map(|v| v.to_le_bytes())),
            )
        });
        if only.is_some() {
            eprintln!("log {:x?}", sc.log);
            let (sseg, soff) = (m.mem.r16(DS, 0xBA56), m.mem.r16(DS, 0xBA54));
            let hex: String = (0..SCRATCH).map(|o| format!("{:02x}", m.mem.r8(sseg, soff.wrapping_add(o)))).collect();
            eprintln!("scratch {sseg:x}:{soff:x} {hex}");
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" || f[2] == "lowmem" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(f[2].split(' ').count().to_string()).or_default() += 1;
    }
    eprintln!("{counts:?}");
}
