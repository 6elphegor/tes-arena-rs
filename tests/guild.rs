//! Differential tests of the mages' guild (src/guild.rs `mages_guild`,
//! 1194:A73E) against vectors from the original code
//! (harness/gen_guild.py).
mod common;

use arena::bsa::Files;
use arena::guild::GuildHooks;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::shops::ShopHooks;
use common::blow::Stream;

const FRAME: u16 = 0xA000;

const COUNTS: [&[u8]; 10] = [b"1", b"3", b"10", b"0", b"", b"2x", b"255", b"300", b"7", b"12"];
const CLICKS: [(u8, u16, u16); 18] = [
    (1, 0x30, 0xC0),
    (1, 0x90, 0xC0),
    (1, 0x130, 0xC0),
    (0, 0x90, 0xC0),
    (1, 0x90, 0xC0),
    (1, 0x90, 0xC0),
    (1, 0x90, 0xC0),
    (1, 0x90, 0xC0),
    (1, 0x90, 0x10),
    (1, 0x70, 0xC0),
    (1, 0x100, 0xC0),
    (1, 0x2, 0xC0),
    (1, 0x69, 0xB8),
    (1, 0x78, 0xB8),
    (1, 0xBE, 0xB8),
    (1, 0x11D, 0xB8),
    (1, 0x136, 0xB8),
    (1, 0x137, 0xC0),
];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The front end's answers, as harness/gen_guild.py's stubs give them,
/// and the log they keep.
#[derive(Default)]
struct Script {
    menus: Vec<u16>,
    lists: Vec<u16>,
    yes: Vec<u16>,
    counts: Vec<&'static [u8]>,
    haggles: Vec<(bool, u32)>,
    clicks: Vec<(u8, u16, u16)>,
    file: Option<Vec<u8>>,
    n: [usize; 6],
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
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([8, si as u32, bx as u32, di as u32]);
        let keys = (0..8).take_while(|&i| m.mem.r8(DS, bx + i) != 0).count();
        let v = take(&self.menus, &mut self.n[0], 0xFFFF);
        if v == 0xFFFF { v } else { v % keys.max(1) as u16 }
    }
    fn dialog(&mut self, m: &mut Machine, kind: u8, di: u16) -> u16 {
        self.log.extend([12, kind as u32, if kind == 2 { di as u32 } else { 0 }]);
        if kind == 2 {
            let t = take(&self.counts, &mut self.n[3], b"0");
            for (i, &c) in t.iter().chain(&[0]).enumerate() {
                m.mem.w8(DS, di + i as u16, c);
            }
            return 0;
        }
        take(&self.yes, &mut self.n[2], 1)
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16, _di: u16) -> u16 {
        self.log.extend([9, 1, si as u32, cx as u32]);
        let v = take(&self.lists, &mut self.n[1], 0xFFFF);
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn quest_check(&mut self, _m: &mut Machine) {
        self.log.push(1);
    }
    fn quests(&mut self, _m: &mut Machine) {
        self.log.push(2);
    }
}

impl GuildHooks for Script {
    fn stock_file(&mut self, _m: &mut Machine, _name: u16) -> Option<Vec<u8>> {
        self.log.push(3);
        self.file.clone()
    }
    fn stock_write(&mut self, _m: &mut Machine, _name: u16, bytes: &[u8]) {
        let (a, rest) = bytes.split_at(4);
        let (b, c) = rest.split_at(1);
        for part in [a, b, c] {
            self.log.extend([5, part.len() as u32, fnv(part.iter().copied()) as u32]);
        }
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([10, si as u32]);
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([13, si as u32]);
    }
    fn stock_list(&mut self, _m: &mut Machine, _seg: u16, si: u16, cx: u16) -> u16 {
        self.log.extend([9, 2, si as u32, cx as u32]);
        let v = take(&self.lists, &mut self.n[1], 0xFFFF);
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn haggle(&mut self, _m: &mut Machine, price: u32, quality: u16, factor: u16, per: u16) -> (u32, bool) {
        self.log.extend([14, price & 0xFFFF, price >> 16, quality as u32, factor as u32, per as u32]);
        let (cf, eax) = take(&self.haggles, &mut self.n[4], (true, 0));
        (eax, cf)
    }
    fn spell_screen(&mut self, _m: &mut Machine) {
        self.log.push(16);
    }
    fn click(&mut self, _m: &mut Machine) -> (u8, u16, u16) {
        let (b, x, y) = take(&self.clicks, &mut self.n[5], (1, 0x130, 0xC0));
        self.log.extend([17, b as u32, x as u32, y as u32]);
        (b, x, y)
    }
    fn spellmaker(&mut self, _m: &mut Machine) {
        self.log.push(18);
    }
}

/// The guilds' holidays' days (the shops' byte 7 at 3353, the days at
/// 33AD), as harness/gen_guild.py `guild_holidays` finds them.
fn holidays(m: &Machine) -> Vec<u16> {
    let mut days = Vec::new();
    for i in 0..0x40u16 {
        let d = m.mem.r16(DS, 0x33AD + 2 * i);
        if d > 0x158 {
            break;
        }
        if m.mem.r8(DS, 0x3353 + 6 * i) == 7 {
            days.push(d);
        }
    }
    days
}

/// Mirrors harness/gen_guild.py `setup`: the writes in order (a tuple's
/// items before its index, a conditional's test first).
fn setup(m: &mut Machine, seed: u32, days: &[u16]) -> Script {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| {
        let a = s.word() as u32;
        a | (s.word() as u32) << 16
    };
    let v = if s.byte() < 0xF0 { 0 } else { s.word() };
    m.mem.w16(DS, 0x0FFB, v);
    let v = if s.byte() < 0xF0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x680F, v);
    let v = s.word();
    m.mem.w16(DS, 0xA91A, v);
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    m.mem.w16(DS, 0x91F0, 0);
    let k = s.byte();
    let gold = if k < 0x40 {
        (s.byte() % 60) as u32
    } else if k < 0xC0 {
        (s.word() % 0x1000) as u32
    } else {
        dword(s) % 0x100000
    };
    m.mem.w32(DS, 0x05C2, gold);
    let a = s.byte() & 0x20;
    let b = s.byte() % 0x12;
    m.mem.w8(DS, 0x01A9, a | b);
    let v = s.byte() % 0x20;
    m.mem.w8(DS, 0x01AA, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01D3, v);
    for at in [0x01A4 + 0x2A, 0x01A4 + 0x2C] {
        let v = s.byte() % 0x40;
        m.mem.w8(DS, at, v);
    }
    let now = if s.byte() < 0x40 && !days.is_empty() {
        let day = days[s.byte() as usize % days.len()] as u32;
        let a = (s.word() % 1440) as u32;
        day * 1440 + a + (s.byte() % 4) as u32 * 518400
    } else {
        dword(s) % 0x800000
    };
    m.mem.w32(DS, 0x05CC, now);
    let v = s.byte() % 0x18;
    m.mem.w8(DS, 0x13CC, v);
    let v = s.byte();
    m.mem.w8(DS, 0x13CA, v);
    let v = s.byte();
    m.mem.w8(DS, 0x13CB, v);
    let v = s.byte() % 3;
    m.mem.w8(DS, 0x0A22 + 0x25, v);
    let v = dword(s);
    m.mem.w32(DS, 0x0A4C, v);
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0A46, v);
    let v = dword(s);
    m.mem.w32(DS, 0x0A56, v);
    let v = s.word();
    m.mem.w16(DS, 0xC3D0, v);
    let v = s.word() % 0x200;
    m.mem.w16(DS, 0xC3D2, v);
    m.mem.w8(DS, 0x0D48, 7);
    let v = if s.byte() < 0xC0 { 0 } else { s.byte() };
    m.mem.w8(DS, 0x13F4, v);
    let v = dword(s) % 0x20000;
    m.mem.w32(DS, 0x13FA, v);
    let n_items = [0, 1, 5, 12, 25, 40][s.byte() as usize % 6];
    let mut items = vec![0u8; 0x28 * 0x13];
    for i in 0..n_items {
        let mut it = [0u8; 0x13];
        let kind = s.byte() % 4;
        let flags = match kind {
            0 => {
                let f = 0x40 | 0x0C | 1 | if s.byte() < 0x80 { 2 } else { 0 };
                it[0] = s.byte() % 0x0F;
                let c = (s.byte() % 4) as u16;
                it[1..3].copy_from_slice(&c.to_le_bytes());
                f
            }
            1 => {
                let f = 0x40 | 0x04 | if s.byte() < 0x40 { 2 } else { 0 };
                it[0] = s.byte() % 0x12;
                f
            }
            2 => {
                let f = 0x40 | if s.byte() < 0x40 { 2 } else { 0 };
                it[0] = s.byte() % 0x0B;
                it[0x10] = s.byte() % 3;
                f
            }
            _ => {
                let k = (s.byte() % 3) << 4;
                let f = 0x40 | 0x08 | 1 | k | if s.byte() < 0x80 { 2 } else { 0 };
                it[0] = s.byte() % 4;
                it[0x10] = s.byte() % 3;
                it[0x0F] = s.byte() % 3;
                f
            }
        };
        it[0x0E] = flags;
        let a = s.byte() as u32;
        let b = s.word() as u32;
        let c = (s.word() % 3000) as u32;
        let price = [a, b, c][s.byte() as usize % 3];
        it[0x0A..0x0E].copy_from_slice(&price.to_le_bytes());
        items[i * 0x13..(i + 1) * 0x13].copy_from_slice(&it);
    }
    for (i, &b) in items.iter().enumerate() {
        m.mem.w8(DS, 0x0212 + i as u16, b);
    }
    let known = s.byte() % 8;
    m.mem.w8(DS, 0x050A, known);
    for i in 0..known as u16 {
        let v = s.byte() % 0x2F;
        m.mem.w8(DS, 0x050B + i, v);
    }
    for n in 0..0x30u16 {
        let name: Vec<u8> = if s.byte() < 0xC0 {
            let len = 1 + s.byte() % 8;
            (0..len).map(|_| 0x61 + s.byte() % 26).collect()
        } else {
            Vec::new()
        };
        for (i, &c) in name.iter().chain(&[0]).enumerate() {
            m.mem.w8(DATA, n * 0x55 + 0x34 + i as u16, c);
        }
        let v = if s.byte() < 0xC0 { s.word() % 0x800 } else { s.word() };
        m.mem.w16(DATA, n * 0x55 + 0x32, v);
    }
    let menus = (0..10)
        .map(|_| {
            let k = s.byte() % 8;
            if k == 7 { 0xFFFF } else { (k % 5) as u16 }
        })
        .collect();
    let lists = (0..6).map(|_| if s.byte() < 0x30 { 0xFFFF } else { (s.byte() % 16) as u16 }).collect();
    let yes = (0..3).map(|_| (s.byte() & 1) as u16).collect();
    let counts = (0..4).map(|_| COUNTS[s.byte() as usize % COUNTS.len()]).collect();
    let haggles = (0..4)
        .map(|_| {
            let cf = s.byte() < 0x40;
            let k = s.byte();
            let eax = if k < 0x30 {
                gold
            } else if k < 0xC0 {
                (s.word() % 0x800) as u32
            } else {
                dword(s) % 0x100000
            };
            (cf, eax)
        })
        .collect();
    let clicks = (0..6).map(|_| CLICKS[s.byte() as usize % CLICKS.len()]).collect();
    let file = if s.byte() < 0x60 {
        None
    } else {
        let a = (s.byte() as u32) % 0x5A0;
        let b = 0x5A0 + s.word() as u32;
        let e = s.word() as u32 | 0x8000_0000;
        let age = [a, b, 0x5A0 - 1, 0x5A0, e][s.byte() as usize % 5];
        let mut f = now.wrapping_sub(age).to_le_bytes().to_vec();
        let e = s.byte();
        let count = [0, 1, 3, 8, e][s.byte() as usize % 5];
        f.push(count);
        for _ in 0..count {
            let mut r = [0u8; 0x0B];
            r[0] = if s.byte() < 0xF8 { s.byte() % 3 } else { 3 };
            r[1] = s.byte() % 6;
            r[2] = s.byte() % 3;
            r[3] = s.byte() % 8;
            r[4] = s.byte() % 0x20;
            let p = dword(s) % 0x10000;
            r[5..9].copy_from_slice(&p.to_le_bytes());
            r[9] = s.byte();
            r[10] = s.byte();
            f.extend(r);
        }
        if s.byte() < 0x20 {
            let n = s.byte() as usize % (f.len() + 1);
            f.truncate(n);
        }
        Some(f)
    };
    // Aimed at edges (see harness/gen_guild.py).
    if s.byte() < 0x20 {
        m.mem.w32(DS, 0x0A4C, 0xFFFF_FC18);
    }
    if s.byte() < 0x30 {
        m.mem.w8(DS, 0x13F4, 1);
        let v = 0xC350 + (s.byte() % 3) as u32 - 1;
        m.mem.w32(DS, 0x13FA, v);
    }
    if s.byte() < 0x40 && n_items > 0 {
        let i = (s.byte() as usize % n_items) as u16;
        let v = 9 + (s.byte() % 3) as u32;
        m.mem.w32(DS, 0x0212 + i * 0x13 + 0x0A, v);
        let f = items[i as usize * 0x13 + 0x0E] | 2;
        m.mem.w8(DS, 0x0212 + i * 0x13 + 0x0E, f);
    }
    Script { menus, lists, yes, counts, haggles, clicks, file, ..Default::default() }
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
    assert!(base.bsa_open(&mut files));
    let days = holidays(&base);
    let only = std::env::var("GUILD_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/guild.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "guild", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut sc = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap(), &days);
        m.load_rng();
        let before = (seg_bytes(&m, DS), seg_bytes(&m, DATA), seg_bytes(&m, FRAME), m.ems_bank(0x18));
        let got = m.mages_guild(&mut files, &mut sc).map(|()| {
            m.store_rng();
            let after = seg_bytes(&m, DS);
            if only.is_some() {
                for (o, (a, b)) in before.0.iter().zip(&after).enumerate() {
                    if a != b {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
                eprintln!("log {:?}", sc.log);
                for (o, (a, b)) in before.3.iter().zip(&m.ems_bank(0x18)).enumerate().filter(|(_, (a, b))| a != b).take(10000) {
                    eprintln!("BANK {o:04x} {a:02x} {b:02x}");
                }
                for (o, (a, b)) in before.1.iter().zip(&seg_bytes(&m, DATA)).enumerate() {
                    if a != b {
                        eprintln!("DATA {o:04x} {a:02x} {b:02x}");
                    }
                }
            }
            let d = [
                fnv(changes(&before.0, &after)),
                fnv(changes(&before.1[..0x92A0], &seg_bytes(&m, DATA)[..0x92A0])),
                fnv(changes(&before.2, &seg_bytes(&m, FRAME))),
                fnv(m.ems_bank(0x18)),
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
    eprintln!("guild: {n} compared, {stuck} stuck in both");
    if only.is_none() {
        assert!(n > 3000, "{n} compared");
    }
}
