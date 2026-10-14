//! Differential tests of the wilderness's run (src/wild.rs: `wild_setup`
//! 217B:0084, `wild_shift` 0265, `wild_done` 04DC, `wild_leave_city`
//! 027A, `wild_compress` 2294:0D03) against vectors from the original
//! code (harness/gen_wildrun.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::wild::WildHooks;
use common::blow::Stream;

const BUF: u16 = 0x7000;
const FLOOR: u16 = 0xA000;
const WALLS: u16 = 0xB000;
const CEIL: u16 = 0xC000;
const SRC: u16 = 0xD000;
const DST: u16 = 0xE000;
const TEMPLATES: [&str; 8] = ["city1.mif", "cityw1.mif", "cityw3.mif", "townw1.mif", "town2.mif", "villagw4.mif", "village3.mif", "imperial.mif"];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// The stubs' log, as the harness keeps it.
#[derive(Default)]
struct Log(Vec<u8>);

impl WildHooks for Log {
    fn load_block(&mut self, _m: &mut Machine, _files: &mut Files, slot: u8, block: u8) -> Option<()> {
        self.0.push(b'L');
        self.0.extend([slot, block]);
        Some(())
    }
    fn write(&mut self, m: &mut Machine, _files: &mut Files, name: u16, seg: u16, off: u16, n: u16) {
        self.0.push(b'W');
        self.0.extend(m.mem.cstr(DS, name));
        self.0.push(0);
        self.0.extend(n.to_le_bytes());
        let h = fnv((0..n).map(|i| m.mem.r8(seg, off.wrapping_add(i))));
        self.0.extend(h.to_le_bytes());
    }
    fn weather(&mut self, m: &mut Machine) {
        let v = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
        m.mem.w16(DS, 0xFFE0, v);
    }
}

enum Extra {
    None,
    Same,
    Pack(u16, u16),
}

fn put(m: &mut Machine, seg: u16, off: u16, data: &[u8]) {
    for (i, &b) in data.iter().enumerate() {
        m.mem.w8(seg, off.wrapping_add(i as u16), b);
    }
}

/// Mirrors harness/gen_wildrun.py `setup` (a conditional's test byte
/// comes first, a tuple's items before its index).
fn setup(m: &mut Machine, seed: u32, op: &str) -> Extra {
    let mut st = Stream(seed);
    let s = &mut st;
    let w16 = |m: &mut Machine, at: u16, v: u16| m.mem.w16(DS, at, v);
    let edge = |s: &mut Stream| [0x0FFFu16, 0x1000, 0x2FFF, 0x3000, 0x3001, 0x1FFF, 0x2000, 0, 0x3FFF][s.byte() as usize % 9];
    let pos = |s: &mut Stream| {
        if s.byte() < 0x80 {
            edge(s)
        } else if s.byte() < 0xC0 {
            s.word() & 0x3FFF
        } else {
            s.word()
        }
    };
    match op {
        "setup" => {
            let v = s.word() & !0x2000;
            let v = v | if s.byte() < 0x60 { 0x2000 } else { 0 };
            w16(m, 0xC39A, v);
            let lo = s.word() as u32;
            let hi = s.word() as u32;
            m.mem.w32(DS, 0x0A22, lo | hi << 16);
            let t = TEMPLATES[s.byte() as usize % TEMPLATES.len()];
            put(m, DS, 0x0A36, format!("{t}\0").as_bytes());
            let lo = s.word() as u32;
            let hi = s.word() as u32;
            m.mem.w32(DS, 0x0616, lo | hi << 16);
            for at in [0x060C, 0x060E] {
                let v = if s.byte() < 0xC0 { (s.byte() % 0x40) as u16 } else { s.word() };
                w16(m, at, v);
            }
            for at in [0x0610, 0x0612, 0x0614, 0xC3DA, 0xC3D0, 0xC3D4] {
                let v = pos(s);
                w16(m, at, v);
            }
            let cb = s.word();
            let d = [0x80u16, 0x81, 0x100, 0x101, 0xFF, 0x7F, 0x8000, 0, 0x80, 0x180][s.byte() as usize % 10];
            let d = if s.byte() & 1 != 0 { d } else { d.wrapping_neg() };
            let fd = if s.byte() < 0xC0 { cb.wrapping_sub(d) } else { s.word() };
            w16(m, 0xCB90, cb);
            w16(m, 0x0FD2, fd);
            let v = s.word();
            w16(m, 0x05C8, v);
        }
        "shift" => {
            let v = if s.byte() < 0xE0 { 0 } else { s.byte() };
            m.mem.w8(DS, 0xACC2, v);
            for at in [0xC3D0, 0xC3D4, 0x0600, 0x0602] {
                let v = pos(s);
                w16(m, at, v);
            }
            for at in [0x0608, 0x060A] {
                let v = if s.byte() < 0xE0 { (s.byte() % 0x40) as u16 } else { s.word() };
                w16(m, at, v);
            }
            for at in [0x05C8, 0x05CA] {
                let v = s.word();
                w16(m, at, v);
            }
            let lo = s.word() as u32;
            let hi = s.word() as u32;
            m.mem.w32(DS, 0x0604, lo | hi << 16);
            for i in 0..23u16 {
                for k in 0..3 {
                    let v = s.word();
                    w16(m, 0x0AB3 + i * 0x1C + 2 * k, v);
                }
            }
            let grid: Vec<u8> = (0..0x100).map(|_| s.byte() % 8).collect();
            let k = s.word() as usize % 0x100;
            for i in 0..0x1000usize {
                m.mem.w8(BUF, 0xF000 + i as u16, grid[(k + i) % 0x100]);
            }
            if s.byte() < 0x60 {
                return Extra::Same;
            }
        }
        "done" => {
            for at in [0xC3D0, 0xC3D4, 0xC3DA, 0x0608, 0x060A] {
                let v = s.word();
                w16(m, at, v);
            }
            let n = s.byte() % 12;
            let mut v: Vec<u8> = (0..n).map(|_| 1 + s.byte() % 0xFF).collect();
            v.push(0);
            put(m, DS, 0xC2B5, &v);
        }
        "leave" => {
            for (at, t) in [(0xC357u16, [0x80u16, 0x80, 0x40, 0x7F, 0x81, 0]), (0xC359, [0x80, 0x80, 0x40, 0x7F, 0x81, 0])] {
                let v = if s.byte() < 0xE0 { t[s.byte() as usize % 6] } else { s.word() & 0xFF };
                w16(m, at, v);
            }
            let v = if s.byte() < 0xC0 { 0 } else { s.word() & 0x7FFE };
            w16(m, 0x902E, v);
            let v = s.byte() & 1;
            m.mem.w8(DS, 0x1AEA, v);
            let vals = [0x2F2Fu16, 0x2D2D, 0x2E2E, 0xA3A3, 0xB3B3, 0x0000, 0x8080, 0x2F2E, 0x3030, 0x0101];
            for i in 0..0x2000u16 {
                let v = if s.byte() < 0xC0 { vals[s.byte() as usize % 10] } else { s.word() };
                for k in 0..4u16 {
                    m.mem.w16(WALLS, k * 0x4000 + i * 2, v);
                }
            }
            for seg in [CEIL, FLOOR] {
                let row: Vec<u8> = (0..0x100).map(|_| s.byte()).collect();
                for i in 0..0x8000usize {
                    m.mem.w8(seg, i as u16, row[i % 0x100]);
                }
            }
            let v = BUF + (s.byte() % 16) as u16;
            w16(m, 0x9030, v);
        }
        _ => {
            let n: u16 = if s.byte() < 0x80 {
                [0u16, 1, 2, 3, 4, 6, 8, 0x20, 0x41][s.byte() as usize % 9]
            } else {
                let b = s.byte() as u16 * 2;
                b + if s.byte() < 0x30 { (s.byte() & 1) as u16 } else { 0 }
            };
            let alpha: Vec<u16> = {
                let k = 1 + s.byte() % 4;
                (0..k).map(|_| s.word()).collect()
            };
            let mut data = Vec::new();
            while data.len() < n as usize + 4 {
                let v = alpha[s.byte() as usize % alpha.len()];
                let r = 1 + if s.byte() < 0xA0 { s.byte() % 5 } else { 0 };
                for _ in 0..r {
                    data.extend(v.to_le_bytes());
                }
            }
            data.truncate(0x200);
            put(m, SRC, 0, &data);
            let bx = s.word();
            return Extra::Pack(n, bx);
        }
    }
    Extra::None
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    for (var, seg) in [(0x9030u16, BUF), (0xCCD0, FLOOR), (0xD806, WALLS), (0xD748, CEIL)] {
        base.mem.w16(DS, var, seg);
    }
    base.mem.w16(DS, 0x91F0, 0);
    let (mut n, mut hangs) = (0, 0);
    for line in include_str!("vectors/wildrun.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "wildrun", "{line}");
        // Past the harness's instruction cap: not compared (the leave cases,
        // whose clear with a depth of 0 runs 65536 rows, get a larger one).
        if f[3] == "hang" {
            hangs += 1;
            continue;
        }
        let (seed, op) = (u32::from_str_radix(f[1], 16).unwrap(), f[2]);
        let mut m = base.clone();
        let extra = setup(&mut m, seed, op);
        if let Extra::Same = extra {
            let (x, y) = (m.mem.r16(DS, 0x0608), m.mem.r16(DS, 0x060A));
            let at = (y << 6).wrapping_add(x).wrapping_add(0xF000);
            let g = [m.mem.r16(BUF, at), m.mem.r16(BUF, at.wrapping_add(0x40))];
            m.mem.w16(DS, 0x0604, g[0]);
            m.mem.w16(DS, 0x0606, g[1]);
        }
        m.load_rng();
        let before = m.clone();
        let mut files = Files::default();
        let mut log = Log::default();
        let got = match extra {
            Extra::Pack(len, bx) => {
                let ax = m.wild_compress(SRC, 0, DST, 0, len, bx);
                Some(format!("{ax:x} {:x}", fnv((0..0x400u16).map(|o| m.mem.r8(DST, o)))))
            }
            _ => {
                let r = match op {
                    "setup" => m.wild_setup(&mut log),
                    "shift" => m.wild_shift(&mut files, &mut log),
                    "done" => {
                        m.wild_done();
                        Some(())
                    }
                    _ => {
                        m.wild_leave_city(&mut files, &mut log);
                        Some(())
                    }
                };
                r.map(|()| {
                    m.store_rng();
                    let changed = (0..=0xFFFFu16)
                        .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                        .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
                    let buf = m.mem.r16(DS, 0x9030);
                    let parts = [
                        fnv(changed),
                        fnv((0..=0xFFFFu16).map(|o| m.mem.r8(buf, o))),
                        fnv((0..=0xFFFFu16).map(|o| m.mem.r8(WALLS, o))),
                        fnv(log.0.iter().copied()),
                    ];
                    parts.iter().map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" ")
                })
            }
        };
        assert_eq!(f[3], got.as_deref().unwrap_or("fault"), "{line}");
        n += 1;
    }
    eprintln!("wildrun: {n} compared, {hangs} past the cap");
    assert!(n > 3000 && hangs < 50, "{n} {hangs}");
}
