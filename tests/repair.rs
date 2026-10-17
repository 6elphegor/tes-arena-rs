//! Differential tests of the equipment store's repairs (src/store.rs
//! `store_repair`, 1194:C6AC) against vectors from the original code
//! (harness/gen_repair.py). The repairs' texts are their first variants
//! in the game's TEMPLATE.DAT, read as the generator reads them.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::shops::ShopHooks;
use arena::store::StoreHooks;
use common::blow::Stream;
use std::collections::HashMap;

const FSEG: u16 = 0x5C00;
const WSEG: u16 = 0x7C00;
const REPAIRS: u16 = 0x76E3;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The front end's answers, as harness/gen_repair.py's stubs give them,
/// and the log they keep.
struct Script<'a> {
    texts: &'a HashMap<u16, Vec<u8>>,
    dialogs: Vec<(u16, Vec<u8>)>,
    lists: Vec<u16>,
    n: [usize; 2],
    log: Vec<u32>,
}

impl ShopHooks for Script<'_> {
    fn message(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([6, si as u32]);
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([12, si as u32]);
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16, di: u16) -> u16 {
        self.log.extend([5, si as u32, cx as u32, di as u32]);
        let v = self.lists.get(self.n[1]).copied().unwrap_or(0xFFFF);
        self.n[1] += 1;
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn dialog(&mut self, m: &mut Machine, _kind: u8, di: u16) -> u16 {
        let kind = m.mem.r8(DS, 0x13D3);
        self.log.extend([14, kind as u32]);
        let (ax, typed) = self.dialogs.get(self.n[0]).cloned().unwrap_or((2, Vec::new()));
        self.n[0] += 1;
        if kind == 2 {
            self.log.push(di as u32);
            let t = &typed[..typed.len().min(4)];
            for (i, &b) in t.iter().chain([0u8].iter()).enumerate() {
                m.mem.w8(DS, di.wrapping_add(i as u16), b);
            }
        }
        ax
    }
}

impl StoreHooks for Script<'_> {
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([7, si as u32]);
    }
    fn text(&mut self, m: &mut Machine, _files: &mut Files, di: u16) -> Option<()> {
        let number = m.mem.r16(DS, 0x137A);
        self.log.extend([13, number as u32, m.mem.r8(DS, 0x137E) as u32, m.mem.r8(DS, 0x101C) as u32, 2, di as u32]);
        let t = self.texts.get(&number).cloned().unwrap_or_default();
        for (i, &b) in t.iter().chain([0u8].iter()).enumerate() {
            m.mem.w8(DS, di.wrapping_add(i as u16), b);
        }
        Some(())
    }
}

/// The repairs' texts, 589h..594h: each one's first variant (up to its
/// first '&'), from TEMPLATE.DAT.
fn templates(dir: &std::path::Path) -> Option<HashMap<u16, Vec<u8>>> {
    let d = std::fs::read(dir.join("TEMPLATE.DAT")).ok()?;
    let mut out = HashMap::new();
    let mut i = 0;
    while let Some(p) = d[i..].iter().position(|&c| c == b'#') {
        let start = i + p + 1;
        let digits: Vec<u8> = d[start..].iter().take_while(|c| c.is_ascii_digit()).copied().collect();
        let mut j = start + digits.len();
        if digits.is_empty() || !(d.get(j) == Some(&b'\n') || d.get(j..j + 2) == Some(b"\r\n")) {
            i = start;
            continue;
        }
        j += if d[j] == b'\r' { 2 } else { 1 };
        let end = d[j..].iter().position(|&c| c == b'#').map_or(d.len(), |e| j + e);
        let n: u32 = std::str::from_utf8(&digits).unwrap().parse().unwrap_or(0);
        if (0x589..=0x594).contains(&n) {
            let body = &d[j..end];
            let first = body.split(|&c| c == b'&').next().unwrap_or(&[]);
            out.insert(n as u16, first.to_vec());
        }
        i = end;
    }
    Some(out)
}

/// harness/gen_repair.py `item`.
fn item(s: &mut Stream, special: bool) -> [u8; 0x13] {
    let mut rec = [0u8; 0x13];
    let cat = s.byte() % 4;
    let ty = if special {
        0x80 | (s.byte() % 0x10)
    } else {
        match cat {
            1 => s.byte() % 18,
            0 => s.byte() % 11,
            3 => s.byte() % 15,
            _ => s.byte() % 8,
        }
    };
    let kind = s.byte() % 3;
    let mut flags = 0x40 | cat << 2 | kind << 4;
    if s.byte() < 0x60 {
        flags |= 1;
    }
    if s.byte() < 0x30 {
        flags |= 2;
    }
    if s.byte() < 0x40 {
        flags |= 0x80;
    }
    rec[0] = ty;
    let v = if s.byte() < 0xC0 { ((s.byte() % 6) as u16) << 8 } else { s.word() };
    rec[1..3].copy_from_slice(&v.to_le_bytes());
    rec[3] = s.byte();
    rec[4] = s.byte();
    rec[5] = s.byte();
    let hi = s.word() % 0x2000 + 1;
    let k = s.byte();
    let health = if k < 0x40 {
        hi
    } else if k < 0x50 {
        0
    } else {
        s.word() % 0x2000
    };
    rec[6..8].copy_from_slice(&health.to_le_bytes());
    rec[8..10].copy_from_slice(&hi.to_le_bytes());
    if s.byte() < 0x20 {
        rec[3] = hi as u8;
    }
    let kk = s.byte();
    let price: u32 = if kk < 0x30 {
        (s.byte() % 10) as u32
    } else if kk < 0xE0 {
        (s.word() % 0x4000) as u32
    } else {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    rec[0x0A..0x0E].copy_from_slice(&price.to_le_bytes());
    rec[0x0E] = flags;
    rec[0x0F] = if s.byte() < 0x80 { 0xFF } else { s.byte() % 0x18 };
    rec[0x10] = if s.byte() < 0x80 { 0xFF } else { s.byte() % 8 };
    rec[0x11] = s.byte();
    rec[0x12] = s.byte();
    rec
}

/// Mirrors harness/gen_repair.py `setup`, draw for draw: the writes, and
/// the answers.
#[allow(clippy::type_complexity)]
fn setup(m: &mut Machine, seed: u32) -> (Vec<(u16, Vec<u8>)>, Vec<u16>) {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    let v = dword(s);
    m.mem.w32(DS, 0x91F6, v);
    m.mem.w16(DS, 0x91F0, 0);
    let k = s.byte();
    let gold = if k < 0x40 {
        (s.byte() % 0x40) as u32
    } else if k < 0xC0 {
        (s.word() % 0x1000) as u32
    } else {
        let a = s.byte() as u32;
        let b = s.byte() as u32;
        a | b << 8 | (s.byte() as u32) << 16
    };
    m.mem.w32(DS, 0x05C2, gold);
    let v = if s.byte() < 0x80 { 0xFF } else { s.byte() };
    m.mem.w8(DS, 0x01CD, v);
    let v = s.byte() % 0x12;
    m.mem.w8(DS, 0x01A4 + 0x05, v);
    let v = s.byte();
    m.mem.w8(DS, 0x01A9, v);
    let n = if s.byte() < 0x20 { 40 } else { s.byte() % 41 };
    for i in 0..40u16 {
        let rec = if i < n as u16 {
            let special = s.byte() < 4;
            item(s, special)
        } else {
            [0u8; 0x13]
        };
        for (j, &b) in rec.iter().enumerate() {
            m.mem.w8(DS, 0x0212 + 0x13 * i + j as u16, b);
        }
    }
    let day = (s.byte() as u32) % 0x168;
    let year = (s.byte() % 4) as u32;
    let now = year * 518400 + day * 1440 + (s.word() % 1440) as u32 + 0x10000;
    m.mem.w32(DS, 0x05CC, now);
    let taken = s.byte() % 6;
    for i in 0..5u16 {
        let at = REPAIRS + 0x18 * i;
        let mut rec = [0u8; 0x18];
        if i < taken as u16 || s.byte() < 0x10 {
            rec[0] = if s.byte() < 0xE0 { 1 } else { s.byte() };
            let k = s.byte() % 10;
            let delta: i64 = match k {
                0 => -((s.word() % 0x3000) as i64) - 1,
                8 => (s.word() % 0x168) as i64,
                9 => s.word() as i64,
                _ => [0, 0, 1, 0x168, 0x169, 0x59F, 0x5A0, 0x5A1][k as usize],
            };
            rec[1..5].copy_from_slice(&((now as i64 + delta) as u32).to_le_bytes());
            let special = s.byte() < 0x30;
            rec[5..].copy_from_slice(&item(s, special));
        }
        for (j, &b) in rec.iter().enumerate() {
            m.mem.w8(DS, at + j as u16, b);
        }
    }
    let hi = s.byte();
    let lo = s.byte() % 0x10;
    let hi = if hi == 0x25 { 0x26 } else { hi };
    m.mem.w16(DS, 0x1042, (hi as u16) << 8 | lo as u16);
    let v = if s.byte() & 1 != 0 { 0x10 } else { 0 };
    m.mem.w16(DS, 0xA902, v);
    let mut dialogs = Vec::new();
    for _ in 0..10 {
        let k = s.byte();
        let ax = if k < 0xF0 { [0, 1, 2, 0, 1][(k % 5) as usize] } else { s.word() };
        let kk = s.byte();
        let typed: Vec<u8> = if kk < 0x20 {
            Vec::new()
        } else if kk < 0x30 {
            b"0".to_vec()
        } else if kk < 0x40 {
            let a = 0x30 + s.byte() % 10;
            vec![a, 0x41 + s.byte() % 26]
        } else if kk < 0xA0 {
            (s.byte() % 10).to_string().into_bytes()
        } else {
            (s.word() % 10000).to_string().into_bytes()
        };
        dialogs.push((ax, typed));
    }
    let mut lists = Vec::new();
    for _ in 0..6 {
        lists.push(if s.byte() < 0x30 { 0xFFFF } else { s.byte() as u16 });
    }
    (dialogs.into_iter().collect(), lists)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = std::path::PathBuf::from(std::env::var("ARENA_EXE").map_or_else(
        |_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into(),
        |p| std::path::Path::new(&p).parent().unwrap().to_string_lossy().into_owned(),
    ));
    let Ok(mut files) = Files::new(&dir) else { return };
    // GLOBAL.BSA's handle, 5, taken as the generator's stand-in has it.
    files.bsa_handle_taken();
    let Some(texts) = templates(&dir) else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FSEG);
    let only = std::env::var("REPAIR_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    let mut bad = Vec::new();
    let mut stats = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/repair.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "repair", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut fs = files.clone();
        let (dialogs, lists) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let mut sc = Script { texts: &texts, dialogs, lists, n: [0, 0], log: Vec::new() };
        m.load_rng();
        let before = m.clone();
        let got = m.store_repair(&mut fs, &mut sc).map(|_| {
            m.store_rng();
            if only.is_some() {
                for o in 0..=0xFFFFu16 {
                    if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                        eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                    }
                }
                eprintln!("log {:?}", sc.log);
            }
            let ds = fnv((0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]));
            let log = sc.log.iter().flat_map(|v| v.to_le_bytes());
            format!("{:x} {:x}", ds, fnv(log))
        });
        if std::env::var("REPAIR_STATS").is_ok() && got.is_some() {
            let l = &sc.log;
            let has = |pair: [u32; 2]| l.windows(2).any(|w| w == pair);
            let text = |t: u32| has([13, t]);
            for (k, on) in [
                ("old job listed", l.windows(2).any(|w| w[0] == 5 && w[1] == 0x1044)),
                ("ready", has([6, 0x93CA])),
                ("any minute", text(0x58B)),
                ("days left", text(0x58C)),
                ("hours left", text(0x58D)),
                ("too heavy", has([7, 0x4B33])),
                ("inventory full", has([6, 0x19DE])),
                ("hands full", has([6, 0x77D2])),
                ("offered", text(0x590)),
                ("days typed", text(0x589)),
                ("gold typed", text(0x58A)),
                ("agreed", text(0x593)),
                ("not enough", has([6, 0x198C])),
                ("no", text(0x594)),
                ("filed", (0..5u16).any(|i| m.mem.r8(DS, REPAIRS + i * 0x18) != 0 && before.mem.r8(DS, REPAIRS + i * 0x18) == 0)),
                ("whole", l.windows(2).any(|w| w == [6, 0x1044]) && !text(0x593)),
            ] {
                if on {
                    *stats.entry(k).or_default() += 1;
                }
            }
        }
        match got {
            Some(g) if g == f[2] => n += 1,
            None if f[2] == "stuck" => stuck += 1,
            g => {
                let which: String = match &g {
                    Some(g) if f[2] != "stuck" => g.split(' ').zip(f[2].split(' ')).map(|(a, b)| if a == b { '.' } else { 'X' }).collect(),
                    Some(_) => "port ran, original stuck".into(),
                    None => "port stuck".into(),
                };
                bad.push(format!("{} {which}", f[1]));
            }
        }
    }
    eprintln!("repair: {n} compared, {stuck} stuck in both, {} differ", bad.len());
    if !stats.is_empty() {
        eprintln!("{stats:#?}");
    }
    for b in bad.iter().take(20) {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
    if only.is_none() {
        assert!(n > 2500, "{n} compared");
    }
}
