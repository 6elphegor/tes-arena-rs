//! Differential tests of the equipment store (src/store.rs
//! `equipment_store`, 1194:F48E) against vectors from the original code
//! (harness/gen_store.py). GLOBAL.BSA stays closed, as in the generator.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::shops::ShopHooks;
use arena::store::StoreHooks;
use common::blow::Stream;

const CSEG: u16 = 0x6C00;
const WSEG: u16 = 0x7C00;
const FSEG: u16 = 0x5C00;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The front end's answers, as harness/gen_store.py's stubs give them,
/// and the log they keep.
#[derive(Default)]
struct Script {
    menus: Vec<u16>,
    lists: Vec<u16>,
    haggles: Vec<(bool, u32)>,
    n: [usize; 3],
    log: Vec<u32>,
}

fn take<T: Copy>(q: &[T], n: &mut usize, default: T) -> T {
    let v = q.get(*n).copied().unwrap_or(default);
    *n += 1;
    v
}

impl ShopHooks for Script {
    fn message(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([6, si as u32]);
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([4, si as u32, bx as u32, di as u32]);
        let keys = (0..8).take_while(|&i| m.mem.r8(DS, bx.wrapping_add(i)) != 0).count();
        let v = take(&self.menus, &mut self.n[0], 0xFFFF);
        if v == 0xFFFF { v } else { v % keys.max(1) as u16 }
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16, _di: u16) -> u16 {
        self.log.extend([5, si as u32, cx as u32]);
        let v = take(&self.lists, &mut self.n[1], 0xFFFF);
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn expand(&mut self, _m: &mut Machine, di: u16) -> u16 {
        self.log.extend([2, di as u32]);
        di
    }
    fn quests(&mut self, _m: &mut Machine) {
        self.log.push(3);
    }
}

impl StoreHooks for Script {
    fn greeting(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([1, si as u32]);
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([7, si as u32]);
    }
    fn haggle(&mut self, _m: &mut Machine, mode: u8, eax: u32, per: u16, name: u16) -> (u32, bool) {
        self.log.extend([8, mode as u32, eax & 0xFFFF, eax >> 16, per as u32, name as u32]);
        let (cf, eax) = take(&self.haggles, &mut self.n[2], (true, 0));
        (eax, cf)
    }
    fn weapon_lines(&mut self, _m: &mut Machine) {
        self.log.push(9);
    }
    fn armor_lines(&mut self, _m: &mut Machine) {
        self.log.push(10);
    }
    fn repair(&mut self, _m: &mut Machine, _files: &mut Files) -> bool {
        self.log.push(11);
        false
    }
}

/// Mirrors harness/gen_store.py `setup`, draw for draw: the writes, and
/// the store's files (by name).
fn setup(m: &mut Machine, seed: u32) -> (Script, Vec<(String, Vec<u8>)>) {
    let mut st = Stream(seed);
    let s = &mut st;
    let w16 = |m: &mut Machine, at: u16, v: u16| m.mem.w16(DS, at, v);
    let w8 = |m: &mut Machine, at: u16, v: u8| m.mem.w8(DS, at, v);
    let v = if s.byte() < 0xF0 { 0 } else { s.word() };
    w16(m, 0x0FFB, v);
    let v = if s.byte() < 0xF0 { 0 } else { s.byte() };
    w8(m, 0x680F, v);
    let v = s.word();
    w16(m, 0xA91A, v);
    let lo = s.word() as u32;
    let v = lo | (s.word() as u32) << 16;
    m.mem.w32(DS, 0x91F6, v);
    w16(m, 0x91F0, 0);
    let k = s.byte();
    let gold = if k < 0x40 {
        (s.byte() % 0x40) as u32
    } else if k < 0xC0 {
        (s.word() % 0x1000) as u32
    } else {
        s.byte() as u32 | (s.byte() as u32) << 8 | (s.byte() as u32) << 16
    };
    m.mem.w32(DS, 0x05C2, gold);
    let v = s.byte();
    w8(m, 0x01D3, v);
    let v = if s.byte() < 0x80 { 0xFF } else { s.byte() };
    w8(m, 0x01CD, v);
    for at in [0x01A4 + 0x2A, 0x01A4 + 0x2C] {
        let v = if s.byte() < 0x60 { 0xFF } else { s.byte() % 0x40 };
        w8(m, at, v);
    }
    let v = s.byte() % 0x12;
    w8(m, 0x01A4 + 0x05, v);
    let v = if s.byte() < 0x80 { s.byte() % 3 } else { s.byte() % 0x20 };
    w8(m, 0x01A4 + 0x06, v);
    let v = s.byte();
    w8(m, 0x01A9, v);
    let n = s.byte() % 41;
    for i in 0..40u16 {
        let at = 0x0212 + 0x13 * i;
        let mut rec = [0u8; 0x13];
        if (i as u8) < n {
            let cat = s.byte() % 4;
            let ty = if s.byte() < 8 {
                0x80 | s.byte() % 8
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
            if s.byte() < 0x30 {
                flags |= 0x80;
            }
            rec[0] = ty;
            let v = if s.byte() < 0xC0 { ((s.byte() % 6) as u16) << 8 } else { s.word() };
            rec[1..3].copy_from_slice(&v.to_le_bytes());
            rec[3] = s.byte();
            rec[4] = s.byte();
            rec[5] = s.byte();
            let health = if s.byte() < 0x20 { 0 } else { s.word() % 0x2000 };
            rec[6..8].copy_from_slice(&health.to_le_bytes());
            let max = if s.byte() < 0x08 { 0 } else { s.word() % 0x2000 + 1 };
            rec[8..10].copy_from_slice(&max.to_le_bytes());
            let kk = s.byte();
            let price = if kk < 0x20 {
                0
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
        }
        for (j, &b) in rec.iter().enumerate() {
            w8(m, at + j as u16, b);
        }
    }
    let day = (s.byte() as u16 % 0x168) as u32;
    let years = (s.byte() % 4) as u32;
    let now = years * 518400 + day * 1440 + (s.word() % 1440) as u32;
    m.mem.w32(DS, 0x05CC, now);
    let v = s.byte() % 3;
    w8(m, 0x0A22 + 0x25, v);
    let v = s.byte() % 8;
    w8(m, 0x0A46, v);
    let v = if s.byte() & 1 != 0 { 0xFF } else { 0 };
    w8(m, 0x1AEA, v);
    let key = s.word();
    w16(m, 0x0FD2, key);
    let count = (s.byte() % 6) as u16;
    w16(m, 0x138B, count);
    w16(m, 0x1008, 0x10);
    w16(m, 0x100A, CSEG);
    let mut cseg = [0u8; 0x200];
    for b in cseg.iter_mut().take(0x80) {
        *b = s.byte();
    }
    let look = (key & 0xFF00) | ((key & 0xFF) >> 1);
    for i in 0..count as usize {
        let at = 0x10 + 4 * i;
        let v = if s.byte() < 0x80 { look } else { s.word() };
        cseg[at..at + 2].copy_from_slice(&v.to_le_bytes());
        let v = s.word();
        cseg[at + 2..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    for (i, &b) in cseg.iter().enumerate() {
        m.mem.w8(CSEG, i as u16, b);
    }
    let word_at = |at: usize| u16::from_le_bytes([cseg[at], cseg[at + 1]]);
    let seedw = (0..count as usize).map(|i| 0x10 + 4 * i).find(|&at| word_at(at) == look).map_or_else(
        || word_at(if count == 0 { 0x10 } else { 0 }),
        |_| look,
    );
    let name = format!("{seedw}.64");
    let stock = format!("EQ{}", name[2..].to_ascii_uppercase());
    let repairs = format!("RE{}", name[2..].to_ascii_uppercase());
    let mut files = Vec::new();
    if s.byte() < 0xA0 {
        let kk = s.byte();
        let age = if kk < 0x40 {
            0x59F + (s.byte() % 3) as u32
        } else if kk < 0xC0 {
            (s.byte() % 0x10) as u32
        } else {
            s.word() as u32
        };
        let mut data = now.wrapping_sub(age).to_le_bytes().to_vec();
        for _ in 0..2 {
            let n = s.byte() % 12;
            data.push(n);
            for _ in 0..n {
                let magic = if s.byte() < 0x60 { 1 } else { 0 };
                let weapon = s.byte() & 1;
                let ty = if weapon != 0 { s.byte() % 18 } else { s.byte() % 11 };
                let mut rec = [0u8; 13];
                rec[0] = s.byte();
                rec[1] = magic;
                rec[2] = weapon;
                rec[3] = s.byte() % 3;
                rec[4] = if s.byte() < 0x60 { 0xFF } else { s.byte() % 8 };
                rec[5] = ty;
                rec[6] = if s.byte() < 0x60 { 0xFF } else { s.byte() % 0x10 };
                rec[7..11].copy_from_slice(&((s.word() % 0x2000) as u32).to_le_bytes());
                rec[11] = s.byte();
                rec[12] = s.byte();
                data.extend(rec);
            }
        }
        if s.byte() < 0x20 {
            let n = s.byte() as usize % data.len();
            data.truncate(n);
        }
        files.push((stock, data));
    }
    if s.byte() < 0x60 {
        let mut rep = vec![0u8; 0x78];
        for i in 0..5 {
            if s.byte() < 0x80 {
                rep[0x18 * i] = 1 + s.byte() % 0xFF;
                for j in 1..0x18 {
                    rep[0x18 * i + j] = s.byte();
                }
            }
        }
        files.push((repairs, rep));
    }
    let mut sc = Script::default();
    for _ in 0..8 {
        let k = s.byte() % 8;
        sc.menus.push(if k == 7 { 0xFFFF } else { (k % 5) as u16 });
    }
    for _ in 0..6 {
        sc.lists.push(if s.byte() < 0x30 { 0xFFFF } else { s.byte() as u16 });
    }
    for _ in 0..4 {
        let cf = s.byte() < 0x40;
        let k = s.byte();
        let eax = if k < 0x30 {
            gold
        } else if k < 0xC0 {
            (s.word() % 0x800) as u32
        } else {
            s.byte() as u32 | (s.byte() as u32) << 8 | (s.byte() as u32) << 16
        };
        sc.haggles.push((cf, eax));
    }
    for i in 0..8 {
        let v = s.byte();
        w8(m, 0x1104 + i, v);
    }
    (sc, files)
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
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FSEG);
    let only = std::env::var("STORE_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    let mut bad = Vec::new();
    let mut stats = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/store.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "store", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut fs = files.clone();
        let (mut sc, put) = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let names: Vec<String> = put.iter().map(|(k, _)| k.clone()).collect();
        for (k, v) in &put {
            fs.put(k.as_bytes(), v);
        }
        m.load_rng();
        let before = m.clone();
        let got = m.equipment_store(&mut fs, &mut sc).map(|()| {
            m.store_rng();
            let diff = |seg: u16| {
                let m = &m;
                let before = &before;
                fnv((0..=0xFFFFu16)
                    .filter(move |&o| m.mem.r8(seg, o) != before.mem.r8(seg, o))
                    .flat_map(move |o| [o as u8, (o >> 8) as u8, m.mem.r8(seg, o)]))
            };
            // The store's files at the end, by name, and those deleted.
            let mut all: Vec<String> = names.clone();
            for name in store_names(&m) {
                if !all.contains(&name) {
                    all.push(name);
                }
            }
            all.sort();
            let mut fl = Vec::new();
            for k in &all {
                if let Some(d) = fs.written(k.as_bytes()) {
                    fl.extend(k.as_bytes());
                    fl.push(0);
                    fl.extend((d.len() as u32).to_le_bytes());
                    fl.extend(d);
                }
            }
            let [stock, repairs] = store_names(&m);
            let names_made = if stock == repairs { vec![stock] } else { vec![stock, repairs] };
            for k in &names_made {
                if fs.was_deleted(k.as_bytes()) {
                    fl.extend(k.as_bytes());
                    fl.push(0);
                }
            }
            if only.is_some() {
                for o in 0..=0xFFFFu16 {
                    if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                        eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                    }
                }
            }
            if only.is_some() {
                for k in &all {
                    if let Some(d) = fs.written(k.as_bytes()) {
                        eprintln!("file {k} {}", d.iter().map(|b| format!("{b:02x}")).collect::<String>());
                    }
                    if fs.was_deleted(k.as_bytes()) {
                        eprintln!("deleted {k}");
                    }
                }
            }
            let log = sc.log.iter().flat_map(|v| v.to_le_bytes());
            format!("{:x} {:x} {:x} {:x} {:x}", diff(DS), diff(FSEG), diff(WSEG), fnv(log), fnv(fl))
        });
        if only.is_some() {
            eprintln!("log {:?}", sc.log);
        }
        if std::env::var("STORE_STATS").is_ok() && got.is_some() {
            let l = &sc.log;
            let has = |pair: [u32; 2]| l.windows(2).any(|w| w == pair);
            let mut tag = |k: &'static str, on: bool| {
                if on {
                    *stats.entry(k).or_default() += 1;
                }
            };
            tag("haggle buy", has([8, 1]));
            tag("haggle sell", has([8, 4]));
            tag("stole", has([7, 0x1044]));
            tag("steal failed", has([7, 0x20FA]));
            tag("inventory full", has([7, 0x19DE]));
            tag("not enough gold", has([6, 0x198C]));
            tag("inventory empty", has([6, 0x19C5]));
            tag("not interested", has([6, 0x1A0D]));
            tag("too heavy", has([7, 0x4B33]));
            tag("weapons listed", l.contains(&9));
            tag("armour listed", l.contains(&10));
            tag("repairs", l.contains(&11));
            tag("guards", m.mem.r8(DS, 0x191D) != before.mem.r8(DS, 0x191D));
            tag("gold down", m.mem.r32(DS, 0x05C2) < before.mem.r32(DS, 0x05C2));
            tag("gold up", m.mem.r32(DS, 0x05C2) > before.mem.r32(DS, 0x05C2));
            tag("stock read", put.iter().any(|(k, _)| k.starts_with("EQ")) && m.mem.r8(DS, 0x12DC) != 0);
            tag("set up", m.mem.r8(DS, 0x8B09) != 0);
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
    eprintln!("store: {n} compared, {stuck} stuck in both, {} differ", bad.len());
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

/// The store's two files' names as it made them (8B09, 8B18).
fn store_names(m: &Machine) -> [String; 2] {
    let name = |at: u16| String::from_utf8_lossy(m.mem.cstr(DS, at)).to_ascii_uppercase();
    [name(0x8B09), name(0x8B18)]
}
