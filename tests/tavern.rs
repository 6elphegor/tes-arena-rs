//! Differential tests of the tavern (src/tavern.rs `tavern_shop`,
//! 1194:B93C) against vectors from the original code
//! (harness/gen_tavern.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::shops::ShopHooks;
use arena::tavern::TavernHooks;
use common::blow::Stream;

const CSEG: u16 = 0x6C00;
const WSEG: u16 = 0x7C00;

const DAYS: [&[u8]; 10] = [b"1", b"3", b"10", b"0", b"", b"2x", b"255", b"300", b"7", b"12"];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The front end's answers, as harness/gen_tavern.py's stubs give them,
/// and the log they keep.
#[derive(Default)]
struct Script {
    menus: Vec<u16>,
    lists: Vec<u16>,
    yes: Vec<u16>,
    days: Vec<&'static [u8]>,
    room: (bool, u16, u32),
    haggles: Vec<(bool, u32)>,
    n: [usize; 5],
    log: Vec<u32>,
}

fn take<T: Copy>(q: &[T], n: &mut usize, default: T) -> T {
    let v = q.get(*n).copied().unwrap_or(default);
    *n += 1;
    v
}

impl ShopHooks for Script {
    fn message(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([11, si as u32]);
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([13, si as u32]);
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([8, si as u32, bx as u32, di as u32]);
        let keys = (0..8).take_while(|&i| m.mem.r8(DS, bx.wrapping_add(i)) != 0).count();
        let v = take(&self.menus, &mut self.n[0], 0xFFFF);
        if v == 0xFFFF { v } else { v % keys.max(1) as u16 }
    }
    fn dialog(&mut self, m: &mut Machine, kind: u8, di: u16) -> u16 {
        self.log.extend([12, kind as u32, if kind == 2 { di as u32 } else { 0 }]);
        if kind == 2 {
            let t = take(&self.days, &mut self.n[3], b"0");
            for (i, &b) in t.iter().chain(&[0]).enumerate() {
                m.mem.w8(DS, di.wrapping_add(i as u16), b);
            }
            0
        } else {
            take(&self.yes, &mut self.n[2], 1)
        }
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16, _di: u16) -> u16 {
        self.log.extend([9, si as u32, cx as u32]);
        take(&self.lists, &mut self.n[1], 0xFFFF)
    }
}

impl TavernHooks for Script {
    fn files(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) {
        self.log.push(1);
    }
    fn greeting(&mut self, _m: &mut Machine) {
        self.log.push(2);
    }
    fn quest(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        self.log.push(3);
        Some(())
    }
    fn quests_talk(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        self.log.push(4);
        Some(())
    }
    fn tavern_quest(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        self.log.push(5);
        Some(())
    }
    fn room_load(&mut self, m: &mut Machine, _files: &mut Files) -> bool {
        self.log.push(6);
        let (found, hours, end) = self.room;
        if found {
            m.mem.w8(DS, 0x12BC, 1);
            m.mem.w16(DS, 0x1019, hours);
            m.mem.w32(DS, 0x12CA, end);
            false
        } else {
            m.mem.w8(DS, 0x12BC, 0);
            true
        }
    }
    fn room_file(&mut self, m: &mut Machine, _files: &mut Files) {
        self.log.extend([7, m.mem.r16(DS, 0x1019) as u32, m.mem.r16(DS, 0x12CA) as u32, m.mem.r16(DS, 0x12CC) as u32]);
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([10, si as u32]);
    }
    fn haggle(&mut self, m: &mut Machine, _files: &mut Files) -> Option<(u32, bool)> {
        self.log.extend([14, m.mem.r16(DS, 0x1391) as u32, m.mem.r16(DS, 0x1393) as u32]);
        let (cf, eax) = take(&self.haggles, &mut self.n[4], (true, 0));
        Some((eax, cf))
    }
    fn rumours(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<bool> {
        self.log.push(15);
        Some(true)
    }
}

/// Mirrors harness/gen_tavern.py `setup`, draw for draw.
fn setup(m: &mut Machine, seed: u32) -> Script {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = if s.byte() < 0xF0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x0FFB, v);
    let v = if s.byte() < 0xF0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x680F, v);
    let v = s.word();
    m.mem.w16(DS, 0xA91A, v);
    for i in 0..4u16 {
        let v = s.byte();
        m.mem.w8(DS, 0x91F6 + i, v);
    }
    m.mem.w16(DS, 0x91F0, 0);
    let k = s.byte();
    let gold = if k < 0x40 {
        (s.byte() % 40) as u32
    } else if k < 0xC0 {
        (s.word() % 0x800) as u32
    } else {
        let a = s.byte() as u32;
        let b = s.byte() as u32;
        a | b << 8 | (s.byte() as u32) << 16
    };
    m.mem.w32(DS, 0x05C2, gold);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x01A8, v);
    let v = s.byte() % 0x80;
    m.mem.w8(DS, 0x01D2, v);
    for at in [0x01A4 + 0x2A, 0x01A4 + 0x2C] {
        let v = s.byte() % 0x40;
        m.mem.w8(DS, at, v);
    }
    let v = s.byte() % 0x12;
    m.mem.w8(DS, 0x01A4 + 0x05, v);
    let v = if s.byte() < 0x80 { s.byte() % 3 } else { s.byte() % 0x20 };
    m.mem.w8(DS, 0x01A4 + 0x06, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01A4 + 0x2E, v);
    for i in 0..8u16 {
        let v = s.byte();
        m.mem.w8(DS, 0x01A4 + 0x0E + i, v);
        let v = s.byte();
        m.mem.w8(DS, 0x01A4 + 0x16 + i, v);
    }
    let v = s.byte() % 0x40;
    m.mem.w8(DS, 0x1378, v);
    let v = s.byte() % 6;
    m.mem.w8(DS, 0x1379, v);
    let v = s.byte() % 0x18;
    m.mem.w8(DS, 0x13CC, v);
    let t = if s.byte() < 0x40 {
        let day = [0u32, 45, 236][s.byte() as usize % 3];
        let minute = (s.word() % 1440) as u32;
        day * 1440 + minute + (s.byte() % 4) as u32 * 518_400
    } else {
        let a = s.byte() as u32;
        let b = s.byte() as u32;
        a | b << 8 | ((s.byte() % 0x10) as u32) << 16
    };
    m.mem.w32(DS, 0x05CC, t);
    let v = s.byte() % 3;
    m.mem.w8(DS, 0x0A22 + 0x25, v);
    m.mem.w8(DS, 0x0D48, 4);
    let v = if s.byte() < 0xC0 { 4 } else { 3 };
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    let mut v = s.word() & !4;
    if s.byte() < 0x40 {
        v |= 4;
    }
    m.mem.w16(DS, 0xC398, v);
    let v = s.byte() % 4;
    m.mem.w8(DS, 0x0A47, v);
    let key = s.byte();
    let hi = s.byte() as u16;
    m.mem.w16(DS, 0x0FD2, key as u16 | hi << 8);
    let count = s.byte() % 6;
    m.mem.w16(DS, 0x1387, count as u16);
    m.mem.w16(DS, 0x1AB5, 0x10);
    m.mem.w16(DS, 0x1AB7, CSEG);
    let mut cseg = vec![0u8; 0x200];
    for b in cseg.iter_mut().take(0x80) {
        *b = s.byte();
    }
    for i in 0..count as usize {
        let at = 0x10 + 4 * i;
        cseg[at] = if s.byte() < 0x60 { key >> 1 } else { s.byte() };
        for j in 1..4 {
            cseg[at + j] = s.byte();
        }
    }
    for (i, &b) in cseg.iter().enumerate() {
        m.mem.w8(CSEG, i as u16, b);
    }
    let v = s.byte();
    m.mem.w8(DS, 0xC943, v);
    let menus = (0..8)
        .map(|_| {
            let k = s.byte() % 8;
            if k == 7 { 0xFFFF } else { (k % 5) as u16 }
        })
        .collect();
    let lists = (0..4).map(|_| if s.byte() < 0x30 { 0xFFFF } else { (s.byte() % 10) as u16 }).collect();
    let yes = (0..3).map(|_| (s.byte() & 1) as u16).collect();
    let days = (0..4).map(|_| DAYS[s.byte() as usize % DAYS.len()]).collect();
    let found = s.byte() < 0x60;
    let hours = s.word() % 0x80;
    let a = s.byte() as u32;
    let b = s.byte() as u32;
    let end = a | b << 8 | ((s.byte() % 0x10) as u32) << 16;
    let haggles = (0..3)
        .map(|_| {
            let cf = s.byte() < 0x40;
            let k = s.byte();
            let eax = if k < 0x30 {
                gold
            } else if k < 0xC0 {
                (s.word() % 0x400) as u32
            } else {
                let a = s.byte() as u32;
                let b = s.byte() as u32;
                a | b << 8 | (s.byte() as u32) << 16
            };
            (cf, eax)
        })
        .collect();
    Script { menus, lists, yes, days, room: (found, hours, end), haggles, ..Default::default() }
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
    // The stubbed quest hooks read no files: any directory will do.
    let mut files = arena::bsa::Files::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).expect("files");
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    let only = std::env::var("TAVERN_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/tavern.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "tavern", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut sc = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        m.load_rng();
        let before = (seg_bytes(&m, DS), seg_bytes(&m, WSEG));
        let got = m.tavern_shop(&mut files, &mut sc).map(|()| {
            m.store_rng();
            let after = seg_bytes(&m, DS);
            if only.is_some() {
                for (o, (a, b)) in before.0.iter().zip(&after).enumerate() {
                    if a != b {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
                eprintln!("log {:?}", sc.log);
            }
            let d = [
                fnv(changes(&before.0, &after)),
                fnv(changes(&before.1, &seg_bytes(&m, WSEG))),
                fnv(sc.log.iter().flat_map(|v| v.to_le_bytes())),
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
    eprintln!("tavern: {n} compared, {stuck} stuck in both");
    if only.is_none() {
        assert!(n > 3000, "{n} compared");
    }
}

/// 1194:C0B1 (`room_files_expire`): of the slot's rooms' files ("in*.%d"
/// by 1A6C), those whose end (the dword at +2) isn't past the time (5CC)
/// deleted; another slot's, and one still let, kept.
#[test]
fn rooms_whose_time_is_up_are_deleted() {
    let Some(img) = common::image() else { return };
    let mut m = Machine::new(&img);
    let dir = std::env::temp_dir().join(format!("arena-rooms-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut files = Files::new(&dir).expect("files");
    let _ = std::fs::remove_dir_all(&dir);
    let room = |end: u32| [&[24u8, 0][..], &end.to_le_bytes()].concat();
    files.put(b"IN1234.3", &room(1000));
    files.put(b"IN5678.3", &room(5000));
    files.put(b"IN9999.4", &room(1000));
    files.put(b"IN4321.3", &room(2000));
    m.mem.w8(DS, 0x1A6C, 3);
    m.mem.w32(DS, 0x05CC, 2000);
    m.room_files_expire(&mut files);
    assert!(files.was_deleted(b"IN1234.3"), "past");
    assert!(files.was_deleted(b"IN4321.3"), "the time itself");
    assert!(files.exists(b"IN5678.3"), "still let");
    assert!(files.exists(b"IN9999.4"), "another slot's");
    assert_eq!(m.mem.cstr(DS, 0x733A), b"in*.3");
}
