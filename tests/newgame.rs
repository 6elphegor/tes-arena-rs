//! Differential tests of a new game's character creation (src/newgame.rs)
//! against vectors from the original code (harness/gen_newgame.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::newgame::{Click, NewGameHooks};
use common::blow::Stream;
use std::path::PathBuf;

const QSEG: u16 = 0x6000;
const FRAME: u16 = 0x7000;
const HEAP: (u16, u16) = (0xA000, 0xF000);

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The stubs' log, as the generator keeps it.
#[derive(Default)]
struct Log(Vec<u16>);

impl NewGameHooks for Log {
    fn message_box(&mut self, _m: &mut Machine, _seg: u16, si: u16) {
        self.0.extend([1, si]);
    }
    fn stats_screen(&mut self, _m: &mut Machine) {
        self.0.push(2);
    }
    fn palette(&mut self, _m: &mut Machine) {
        self.0.push(3);
    }
    fn creation_done(&mut self, _m: &mut Machine) {
        self.0.push(4);
    }
    fn game_screen(&mut self, _m: &mut Machine) {
        self.0.push(5);
    }
    fn resources(&mut self, _m: &mut Machine) {
        self.0.push(6);
    }
    fn hands(&mut self, _m: &mut Machine) {
        self.0.extend([7, 0x01A4]);
    }
    fn spells(&mut self, _m: &mut Machine) {
        self.0.push(9);
    }
    fn level_flats(&mut self, _m: &mut Machine) {
        self.0.push(8);
    }
}

/// What a case's setup gives besides its writes.
#[derive(Default)]
struct Case {
    ax: u16,
    clicks: Vec<Click>,
}

/// Mirrors harness/gen_newgame.py `setup` (a conditional's test byte
/// comes first; a tuple's items in order).
fn setup(m: &mut Machine, seed: u32, op: u32) -> Case {
    let mut st = Stream(seed);
    let s = &mut st;
    let mut c = Case::default();
    m.mem.w16(DS, 0x9030, FRAME);
    let lo = s.word() as u32;
    m.mem.w32(DS, 0x91F6, lo | (s.word() as u32) << 16);
    let bytes = |m: &mut Machine, at: u16, v: &[u8]| {
        for (i, &b) in v.iter().enumerate() {
            m.mem.w8(DS, at.wrapping_add(i as u16), b);
        }
    };
    match op {
        0 => {
            for (at, n) in [(0x01A4u16, 0x41Eu16), (0x0F76, 0xB), (0x0E6E, 0xA4), (0x0DCA, 0xD0), (0x0FD4, 0xA), (0x0652, 0xC), (0x0FDD, 0x13)] {
                let v: Vec<u8> = (0..n).map(|_| s.byte()).collect();
                bytes(m, at, &v);
            }
            for at in [0x12BA, 0x05E0, 0x05C8, 0x0AB2] {
                let v = s.word();
                m.mem.w16(DS, at, v);
            }
            let v = s.byte();
            m.mem.w8(DS, 0x1299, v);
            let lo = s.word() as u32;
            m.mem.w32(DS, 0x05CC, lo | (s.word() as u32) << 16);
        }
        1 => {
            let k = s.byte() % 11;
            m.mem.w8(DS, 0x40C7, k);
            let v: Vec<u8> = (0..10).map(|_| if s.byte() < 0xE0 { 1 + s.byte() % 40 } else { s.byte() }).collect();
            bytes(m, 0x40CB, &v);
        }
        2 | 3 => {
            c.ax = if op == 2 {
                if s.byte() < 0xF0 { 1 + s.byte() as u16 % 40 } else { s.byte() as u16 % 48 }
            } else {
                1 + s.byte() as u16 % 40
            };
            m.mem.w16(DS, 0x40E8, 0);
            m.mem.w16(DS, 0x40EA, QSEG);
        }
        4 => {
            let (a, b, cc) = if s.byte() < 0xC0 {
                let a = s.byte() % 11;
                let b = s.byte() % (11 - a);
                (a, b, 10 - a - b)
            } else {
                (s.byte() % 12, s.byte() % 12, s.byte() % 12)
            };
            bytes(m, 0x40C8, &[a, b, cc]);
        }
        5 => {
            let cls = if s.byte() < 0xE0 { s.byte() % 18 } else { s.byte() };
            c.ax = cls as u16;
            m.mem.w8(DS, 0x01A4 + 5, cls);
            let v: Vec<u8> = (0..40 * 19).map(|_| if s.byte() < 0x20 { s.byte() } else { 0 }).collect();
            bytes(m, 0x01A4 + 0x6E, &v);
            let v = s.byte() % 4;
            m.mem.w8(DS, 0x01AA, v);
        }
        6 => {
            let race = s.byte() % 9;
            let class = s.byte() % 18;
            let class = class | (s.byte() & 0x20);
            let level = s.byte() % 4;
            let sex = s.byte() & 1;
            bytes(m, 0x01A8, &[race, class, level, sex]);
            let v: Vec<u8> = (0..16).map(|_| s.byte()).collect();
            bytes(m, 0x01CD, &v);
            let v = s.byte() & 1;
            m.mem.w8(DS, 0x1299, v);
            let v = s.word();
            m.mem.w16(DS, 0x05C8, v);
        }
        7 => {
            let race = s.byte() % 9;
            let class = s.byte() % 18;
            let class = class | (s.byte() & 0x20);
            let level = s.byte() % 4;
            let sex = s.byte() & 1;
            bytes(m, 0x01A8, &[race, class, level, sex]);
            let rolled: Vec<u8> = (0..8).map(|_| s.byte() % 101).collect();
            let now: Vec<u8> = rolled
                .iter()
                .map(|&r| {
                    if s.byte() < 0xF0 {
                        let add = if s.byte() < 0x80 { s.byte() % 8 } else { 0 };
                        (r as u16 + add as u16).min(100) as u8
                    } else {
                        s.byte()
                    }
                })
                .collect();
            bytes(m, 0x01CD, &now);
            bytes(m, 0x01D5, &rolled);
            let v = s.byte() % 26;
            m.mem.w8(DS, 0x129C, v);
            let v = if s.byte() < 0xC0 { 0x19 } else { s.byte() as u16 % 40 };
            m.mem.w16(DS, 0x9686, v);
            let v = s.byte() % 8;
            m.mem.w8(DS, 0x1296, v);
            let v = 0x38 + 8 * (s.byte() % 8);
            m.mem.w8(DS, 0x1297, v);
            let v = s.word();
            m.mem.w16(DS, 0x05C8, v);
            for _ in 0..1 + s.byte() % 6 {
                if s.byte() < 0x50 {
                    c.clicks.push(Click::Attribute(1 + s.byte() as u16 % 8));
                } else {
                    let y = if s.byte() < 0x20 { s.byte() as u16 } else { 0x30 + s.byte() as u16 % 0x48 };
                    c.clicks.push(Click::Arrows(y));
                }
            }
        }
        8 => {
            let v = s.byte() % 18;
            let v = v | (s.byte() & 0x20);
            m.mem.w8(DS, 0x01A9, v);
            let v: Vec<u8> = (0..16).map(|_| s.byte()).collect();
            bytes(m, 0x01CD, &v);
            let v = if s.byte() < 0xE0 { s.word() % 30 } else { s.word() };
            m.mem.w16(DS, 0x01F3, v);
            let v = s.word() % 60;
            m.mem.w16(DS, 0x01FD, v);
            let v = if s.byte() < 0xE0 { s.word() % 60 } else { s.word() };
            m.mem.w16(DS, 0x01FF, v);
            let v: Vec<u8> = (0..3).map(|_| s.byte()).collect();
            bytes(m, 0x050A, &v);
        }
        9 => {
            let v = if s.byte() < 0xF0 { s.byte() % 10 } else { s.byte() };
            m.mem.w8(DS, 0x129A, v);
            let v = if s.byte() < 0xF0 { s.byte() % 9 } else { s.byte() };
            m.mem.w8(DS, 0x01A8, v);
            let v = if s.byte() < 0xE0 { s.byte() & 1 } else { s.byte() };
            m.mem.w8(DS, 0x01AB, v);
            let v = s.byte();
            m.mem.w8(DS, 0x0FD4, v);
        }
        10 => {
            for at in [0x05C8, 0xA902, 0x05B1, 0x12BA] {
                let v = s.word();
                m.mem.w16(DS, at, v);
            }
            let lo = s.word() as u32;
            m.mem.w32(DS, 0x05AD, lo | (s.word() as u32) << 16);
        }
        11 => {
            let v = if s.byte() < 0xE0 { s.byte() & 1 } else { s.byte() };
            m.mem.w8(DS, 0x3B07, v);
            for at in [0xA902, 0x05C8, 0xCD00, 0x1042] {
                let v = s.word();
                m.mem.w16(DS, at, v);
            }
            let v: Vec<u8> = (0..14).map(|_| s.byte()).collect();
            bytes(m, 0x05CC, &v);
            let lo = s.word() as u32;
            m.mem.w32(DS, 0x1AB5, lo | (s.word() as u32) << 16);
            let v = s.byte();
            m.mem.w8(DS, 0x0A20, v);
        }
        12 => {
            let v: Vec<u8> = (0..16).map(|_| s.byte()).collect();
            bytes(m, 0x0A36, &v);
            let v: Vec<u8> = (0..20).map(|_| s.byte()).collect();
            bytes(m, 0x0A22, &v);
            let v = s.byte() % 5;
            m.mem.w8(DS, 0x05E6, v);
            let v = s.byte();
            m.mem.w8(DS, 0x05E4, v);
            let v = s.byte();
            m.mem.w8(DS, 0xBB0A, v);
            let v = s.word();
            m.mem.w16(DS, 0xC398, v);
        }
        _ => unreachable!(),
    }
    c
}

fn changes(m: &Machine, before: &Machine, seg: u16) -> Vec<u8> {
    (0..=0xFFFFu16)
        .filter(|&o| m.mem.r8(seg, o) != before.mem.r8(seg, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(seg, o)])
        .collect()
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let question = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("QUESTION.TXT"))
        .map(|e| std::fs::read(e.path()).unwrap())
        .expect("QUESTION.TXT");
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    for (i, &b) in question.iter().take(0x10000).enumerate() {
        base.mem.w8(QSEG, i as u16, b);
    }
    let mut n = [0usize; 13];
    for line in include_str!("vectors/newgame.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "newgame", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let op: u32 = f[2].parse().unwrap();
        let mut m = base.clone();
        let mut fs = files.clone();
        let c = setup(&mut m, seed, op);
        m.load_rng();
        let mut log = Log::default();
        let mut before = m.clone();
        let mut ret = String::new();
        let ok = match op {
            0 => {
                m.newgame_reset();
                Some(())
            }
            1 => m.quiz_pick().map(|_| ()),
            2 => m.quiz_find(c.ax).map(|di| ret = format!(" {di:x}")),
            3 => m.quiz_find(c.ax).map(|di| {
                before = m.clone();
                m.quiz_answers(QSEG, di);
            }),
            4 => m.quiz_class(&mut fs).map(|class| ret = format!(" {class:x}")),
            5 => m.class_weapon(c.ax as u8),
            6 => m.creation_roll(&mut log),
            7 => {
                for &k in &c.clicks {
                    m.creation_click(k);
                }
                Some(())
            }
            8 => {
                m.creation_finish();
                Some(())
            }
            9 => {
                m.face_chosen();
                Some(())
            }
            10 => {
                m.creation_end(&mut log);
                Some(())
            }
            11 => m.game_setup(&mut log),
            12 => {
                m.game_start();
                Some(())
            }
            _ => unreachable!(),
        };
        match ok {
            Some(()) => {
                m.store_rng();
                let d = [
                    fnv(changes(&m, &before, DS)),
                    fnv(changes(&m, &base, QSEG)),
                    fnv(log.0.iter().flat_map(|v| v.to_le_bytes())),
                ];
                let got = format!("{:x} {:x} {:x}{ret}", d[0], d[1], d[2]);
                if f[3] != got {
                    let c = changes(&m, &before, DS);
                    for k in c.chunks(3) {
                        let o = k[0] as u16 | (k[1] as u16) << 8;
                        eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), k[2]);
                    }
                    eprintln!("log {:?}", log.0);
                }
                assert_eq!(f[3], got, "{line}");
            }
            None => assert!(f[3] == "hang" || f[3] == "fault", "{line}: the port gave up"),
        }
        n[op as usize] += 1;
    }
    assert!(n.iter().all(|&k| k > 0), "{n:?}");
}
