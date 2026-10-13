//! Differential tests of the actors' frame (0000:1854) against vectors
//! from the original code (harness/gen_actors.py): each case loads a
//! level and its INF with EMS on, fills the actors from a seed and runs
//! one frame. Needs the game directory (ESarena/, next to ACD.EXE).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;
use std::path::PathBuf;

const FRAME: u16 = 0x8800;
const NAME: u16 = 0x0A36;
const INFO: u16 = 0xE2BE;
const BLOCK: u16 = 0x3E80;
const CSEG: u16 = 0x7800;
const ACTORS: u16 = 0x0AB3;
const COUNT: u16 = 23;
const SIZE: u16 = 0x1C;
/// The harness's heap (harness/dos.py).
const HEAP: (u16, u16) = (0xA000, 0xF000);
const VARS: [(u16, u16); 21] = [
    (0xC3D2, 2),
    (0x05BE, 2),
    (0x653E, 2),
    (0x05E0, 2),
    (0x7A54, 2),
    (0xCCBA, 2),
    (0xAFF9, 1),
    (0xCCBC, 2),
    (0xC398, 2),
    (0xC39A, 2),
    (0x05C8, 2),
    (0x05E2, 1),
    (0x0F83, 1),
    (0x100D, 1),
    (0x1013, 1),
    (0x05B1, 2),
    (0x0604, 4),
    (0x392D, 1),
    (0x392E, 4),
    (0x7A44, 2),
    (0x91F6, 4),
];

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

fn put(m: &mut Machine, seg: u16, off: u16, b: &[u8]) {
    for (i, &c) in b.iter().enumerate() {
        m.mem.w8(seg, off.wrapping_add(i as u16), c);
    }
}

/// The LCG byte stream of harness/gen_actors.py `stream`.
struct Stream(u32);

impl Stream {
    fn byte(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 16) as u8
    }
    fn word(&mut self) -> u16 {
        let lo = self.byte() as u16;
        lo | (self.byte() as u16) << 8
    }
    fn raw(&mut self, n: u16) -> Vec<u8> {
        (0..n).map(|_| self.byte()).collect()
    }
}

/// Mirrors harness/gen_actors.py `setup`.
fn setup(m: &mut Machine, seed: u32, px: u16, py: u16, corpse: u8) {
    let mut s = Stream(seed);
    let b = s.raw(0x8000);
    put(m, CSEG, 0, &b);
    for i in 0..COUNT {
        let rec = i * 0x500;
        let (a, b, c, d, e) = (s.byte(), s.byte(), s.byte(), s.byte(), s.byte());
        let f40d = (if i >= 15 { 0x1000 } else { 0 }) | (if a & 0xF == 0 { 0x40 } else { 0 }) | (if a & 0xF == 1 { 0x100 } else { 0 });
        m.mem.w16(CSEG, rec + 0x40D, f40d);
        let f41a = if b & 0xC == 0 { (b as u16 & 1) | (b as u16 & 2) << 8 } else { 0 };
        m.mem.w16(CSEG, rec + 0x41A, f41a);
        m.mem.w8(CSEG, rec + 4, c % 0x30);
        m.mem.w8(CSEG, rec + 5, (d & 0x1F) | if d >= 0xC0 { 0x20 } else { 0 });
        m.mem.w8(CSEG, rec + 0x366, if e >= 0xC0 { e & 3 } else { 0 });
    }
    let b = s.raw(0x3C0);
    put(m, DS, 0x0660, &b);
    for k in 0..16 {
        let (t, r) = (s.byte(), s.byte());
        if t < 0x40 {
            m.mem.w16(DS, 0x0660 + k * 0x3C + 0x28, (r as u16 % COUNT) * 0x500);
            m.mem.w16(DS, 0x0660 + k * 0x3C + 0x2A, CSEG);
        }
    }
    let b = s.raw(COUNT * SIZE);
    put(m, DS, 0x5F6A, &b);
    let b = s.raw(8 * 6);
    put(m, DS, 0x0622, &b);
    for k in 0..8 {
        let (t, v) = (s.byte(), s.byte());
        if t < 0x40 {
            m.mem.w16(DS, 0x0622 + k * 6, ACTORS + (t as u16 % COUNT) * SIZE);
            m.mem.w16(DS, 0x0622 + k * 6 + 2, v as u16 & 0x3E);
        }
    }
    let b = s.raw(COUNT * SIZE);
    put(m, DS, ACTORS, &b);
    for i in 0..COUNT {
        let si = ACTORS + i * SIZE;
        let (mask, half) = match s.byte() {
            0..=0x7F => (0x3FF, 0x200),
            0x80..=0xDF => (0xFFF, 0x800),
            _ => (0x1FFF, 0x1000),
        };
        let x = px.wrapping_add(s.word() & mask).wrapping_sub(half);
        let y = py.wrapping_add(s.word() & mask).wrapping_sub(half);
        let v = s.byte();
        let z = if v < 0xC0 { 0 } else { v as u16 & 3 };
        let v = s.byte();
        let k = v % 8;
        let sprite = match k {
            0..=4 => 0xC0 + k,
            5 => 0xC5,
            6 => corpse,
            _ => v,
        };
        let frame = s.byte() % 0xD;
        let mut flags = s.word() & 0x48D7;
        if s.byte() < 0xC0 {
            flags &= !0x4001;
        }
        let heading = s.word() & 0x1FF;
        let sounded = s.byte() as u16 & 1;
        m.mem.w16(DS, si, x);
        m.mem.w16(DS, si + 2, z);
        m.mem.w16(DS, si + 4, y);
        m.mem.w16(DS, si + 0xA, heading);
        m.mem.w8(DS, si + 0xC, sprite);
        m.mem.w8(DS, si + 0xD, frame);
        m.mem.w16(DS, si + 0xF, flags);
        m.mem.w16(DS, si + 0x12, i * 0x500);
        m.mem.w16(DS, si + 0x18, sounded);
    }
}

/// Mirrors harness/gen_actors.py `door_case`; the door's square is the
/// one 0x30 units ahead of (cx, cy)'s square centre.
fn door_case(m: &mut Machine, cx: u16, cy: u16, lock: bool) {
    let si = ACTORS + 15 * SIZE;
    m.mem.w16(DS, si, cx);
    m.mem.w16(DS, si + 2, 0);
    m.mem.w16(DS, si + 4, cy);
    m.mem.w8(DS, si + 0xC, 0xC0);
    m.mem.w8(DS, si + 0xD, 0);
    m.mem.w16(DS, si + 0xF, 4);
    let rec = 15 * 0x500;
    m.mem.w16(CSEG, rec + 0x40D, 0x1000);
    m.mem.w16(CSEG, rec + 0x41A, 0);
    m.mem.w8(CSEG, rec + 4, 1);
    m.mem.w8(CSEG, rec + 0x2D, 0xFF);
    m.mem.w8(CSEG, rec + 0x366, 0);
    if lock {
        // The door is the next square in the direction the creature was
        // moved off its square's centre.
        let (sx, sy) = (cx >> 7, cy >> 7);
        let (ox, oy) = ((cx & 0x7F) as i16 - 0x40, (cy & 0x7F) as i16 - 0x40);
        let (x, y) = (sx.wrapping_add(ox.signum() as u16), sy.wrapping_add(oy.signum() as u16));
        let n = m.mem.r8(DS, 0xC6E4);
        let at = 0xC624 + 3 * n as u16;
        m.mem.w16(DS, at, y << 8 | x);
        m.mem.w8(DS, at + 2, 1);
        m.mem.w8(DS, 0xC6E4, n + 1);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (seg, off) = base.heap_alloc_bytes(BLOCK).unwrap();
    base.mem.w16(DS, 0xBA54, off);
    base.mem.w16(DS, 0xBA56, seg);
    base.mem.w16(DS, 0xCB8A, FRAME);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xCCCE, 0x0C00 + FRAME);
    base.mem.w32(DS, 0xCD62, 0x2826_0AAB);
    base.mem.w32(DS, 0xCD46, 0x23BA_002A);
    // The timed-spell-flag table (harness/gen_actors.py).
    base.mem.w16(DS, 0x1020, 0);
    base.mem.w16(DS, 0x1022, 0x5C00);
    let mut loaded: HashMap<String, (Machine, Files)> = HashMap::new();
    let vectors = include_str!("vectors/actors.txt");
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in vectors.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let door = f[0] == "door";
        assert!(door || f[0] == "actors", "{line}");
        let p: Vec<&str> = f[1].split(' ').collect();
        let kvs = if door { p[7] } else { p[4] };
        let (m0, fs0) = loaded.entry(p[0].to_string()).or_insert_with(|| {
            let mut m = base.clone();
            let mut fs = files.clone();
            put(&mut m, DS, NAME, format!("{}\0", p[0]).as_bytes());
            m.mem.w8(DS, 0xBB0B, 0xFF);
            m.mem.w16(DS, 0xC39A, 0x400);
            m.mem.w16(DS, 0xCB84, 0);
            m.mem.w16(DS, 0xCB88, 0x8000);
            m.mem.w8(DS, 0xC8EF, 0x80);
            m.level_load(&mut fs, NAME).expect(line);
            m.info_load(&mut fs, INFO).expect(line);
            (m, fs)
        });
        let mut m = m0.clone();
        let mut fs = fs0.clone();
        let (px, py) = (hex(p[1]) as u16, hex(p[2]) as u16);
        let corpse = (m.mem.r8(DS, 0xC936) as u16).wrapping_add(m.mem.r16(DS, 0xC38E)) as u8;
        setup(&mut m, hex(p[3]) as u32, px, py, corpse);
        let kv: HashMap<u16, u64> = kvs.split(',').map(|kv| {
            let (k, v) = kv.split_once('=').unwrap();
            (hex(k) as u16, hex(v))
        }).collect();
        for (o, n) in VARS {
            put(&mut m, DS, o, &kv[&o].to_le_bytes()[..n as usize]);
        }
        m.mem.w16(DS, 0xC3D0, px);
        m.mem.w16(DS, 0xC3D4, py);
        m.mem.w16(DS, 0xCB90, ((py >> 7) << 8) + ((px >> 7) << 1));
        if door {
            door_case(&mut m, hex(p[4]) as u16, hex(p[5]) as u16, p[6] == "1");
        }
        m.load_rng();
        let before = m.clone();
        let got = m.actors_update(&mut fs).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let walls = m.mem.r16(DS, 0xD806);
            format!(
                "{:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(walls, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o)))
            )
        });
        if std::env::var("ACTORS_DUMP").is_ok_and(|v| v == p[3]) {
            for o in 0..=0xFFFFu16 {
                if m.mem.r8(DS, o) != before.mem.r8(DS, o) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
            for o in 0..0x8000u16 {
                if m.mem.r8(CSEG, o) != before.mem.r8(CSEG, o) {
                    eprintln!("c{o:04x} {:02x} {:02x}", before.mem.r8(CSEG, o), m.mem.r8(CSEG, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(if f[2] == "fault" { "fault" } else if door { "door" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 100);
}
