//! Differential tests of the spellmaker (src/spellmaker.rs `spellmaker`,
//! 1194:6064) against vectors from the original code
//! (harness/gen_spellmaker.py).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::spellmaker::SpellmakerHooks;
use common::blow::Stream;

const FRAME: u16 = 0x6000;
const SCREEN: u16 = 0x7000;
const NAMES: [&[u8]; 6] = [b"Fireball", b"", b"x", b"Healing Touch", b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", b"Zap"];
const REGIONS: [(u16, u16, u16, u16); 7] = [
    (0x45, 0x31, 0xC8, 0x39),
    (0x12, 0x48, 0x34, 0x4F),
    (0x12, 0x3C, 0x31, 0x45),
    (0xCB, 0x31, 0xF2, 0x38),
    (0x0A, 0xBC, 0x35, 0xC5),
    (0xCA, 0xBC, 0xF3, 0xC5),
    (0x11F, 0xBC, 0x131, 0xC5),
];

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

/// A click's answer: the region (0..6, 7 a miss) and its offsets, or on a
/// form the kind (0, 1 an arrow, 2 the done box, 3 a miss), the control
/// and up or down.
type Click = (u8, u8, u8, u8, u8, u8);

/// The front end's answers, as harness/gen_spellmaker.py's stubs give
/// them, and the log they keep.
#[derive(Default)]
struct Script {
    clicks: Vec<Click>,
    lists: Vec<u16>,
    menus: Vec<u16>,
    names: Vec<&'static [u8]>,
    n: [usize; 4],
    log: Vec<u32>,
}

/// Mirrors harness/gen_spellmaker.py `click_at`.
fn click_at(m: &Machine, e: Click, form: Option<u16>) -> (u16, u16) {
    let (r, ox, oy, kind, idx, up) = e;
    let Some(f) = form else {
        if r == 7 {
            return (0x10, 0x10);
        }
        let (x0, y0, x1, y1) = REGIONS[r as usize];
        return (x0 + ox as u16 % (x1 - x0 + 1), y0 + oy as u16 % (y1 - y0 + 1));
    };
    let rd = |o: u16| m.mem.r16(DATA, f.wrapping_add(o));
    let (fx, fy) = (m.mem.r16(DS, 0xA880), m.mem.r16(DS, 0xA882));
    if kind < 2 {
        let n = rd(4).max(1);
        let c = 0x12 + 0x12 * (idx as u16 % n);
        return (fx.wrapping_add(rd(c)).wrapping_add(1), fy.wrapping_add(rd(c + 2)).wrapping_add([2, 8, 5, 6][up as usize]));
    }
    if kind == 2 {
        return (fx.wrapping_add(rd(0xA)), fy.wrapping_add(rd(0xC)));
    }
    (fx.wrapping_add(0xF0), fy.wrapping_add(0x60))
}

impl SpellmakerHooks for Script {
    fn click(&mut self, m: &mut Machine, form: Option<u16>) -> (u16, u16) {
        let e = self.clicks.get(self.n[0]).copied();
        self.n[0] += 1;
        let (x, y) = match e {
            Some(e) => click_at(m, e, form),
            None => match form {
                None => (0x128, 0xC0),
                Some(f) => {
                    let rd = |o: u16| m.mem.r16(DATA, f.wrapping_add(o));
                    (m.mem.r16(DS, 0xA880).wrapping_add(rd(0xA)), m.mem.r16(DS, 0xA882).wrapping_add(rd(0xC)))
                }
            },
        };
        self.log.extend([1, form.unwrap_or(0) as u32, x as u32, y as u32]);
        (x, y)
    }
    /// gen_spellmaker.py stubs 9AE6 and 9AF8, and 541E and 56C8 (the
    /// picture's load).
    fn font_selected(&mut self, _m: &mut Machine, _ax: u16, _tiles: bool) {}
    fn picture(&self) -> bool {
        false
    }
    /// It stubs 8F26 and 9E06 too.
    fn texts(&self) -> bool {
        false
    }
    fn list(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files, si: u16, cx: u16, _di: Option<u16>) -> u16 {
        self.log.extend([2, si as u32, cx as u32]);
        let v = self.lists.get(self.n[1]).copied().unwrap_or(0xFFFF);
        self.n[1] += 1;
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([3, si as u32, bx as u32, di as u32]);
        let keys = m.mem.cstr(DS, bx).len().min(8) as u16;
        let v = self.menus.get(self.n[2]).copied().unwrap_or(0xFFFF);
        self.n[2] += 1;
        if v == 0xFFFF { v } else { v % keys.max(1) }
    }
    fn message_box(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([4, si as u32]);
    }
    fn type_name(&mut self, m: &mut Machine, si: u16) {
        self.log.extend([5, si as u32]);
        let t = self.names.get(self.n[3]).copied().unwrap_or(b"");
        self.n[3] += 1;
        for (i, &c) in t.iter().chain([0u8].iter()).enumerate() {
            m.mem.w8(DS, si.wrapping_add(i as u16), c);
        }
    }
    fn spells_write(&mut self, m: &mut Machine, _files: &mut Files, name: u16) {
        let d = fnv((0..0xAA0u16).map(|i| m.mem.r8(DATA, 0x2A80 + i)));
        self.log.extend([6, name as u32, 0xAA0, d as u32]);
    }
}

/// Mirrors harness/gen_spellmaker.py `setup`: the writes in order (a
/// tuple's items before its index, a conditional's test first).
fn setup(m: &mut Machine, seed: u32) -> Script {
    let mut st = Stream(seed);
    let s = &mut st;
    let dword = |s: &mut Stream| {
        let a = s.word() as u32;
        a | (s.word() as u32) << 16
    };
    let k = s.byte() % 4;
    let extra = s.byte();
    let cls = if k < 3 { [0x24, 0xE5, 0x22][k as usize] } else { extra };
    m.mem.w8(DS, 0x01A9, cls);
    let v = s.byte() % 0x20;
    m.mem.w8(DS, 0x01AA, v);
    let k = s.byte();
    let gold = if k < 0x30 {
        (s.byte() % 40) as u32
    } else if k < 0xC0 {
        (s.word() % 0x2000) as u32
    } else {
        dword(s) % 0x100000
    };
    m.mem.w32(DS, 0x05C2, gold);
    let known = s.byte() % 8;
    m.mem.w8(DS, 0x050A, known);
    for i in 0..known as u16 {
        let v = s.byte();
        m.mem.w8(DS, 0x050B + i, v);
    }
    let v = s.byte() % 10;
    m.mem.w8(DS, 0x1A6C, v);
    for at in [0x1001u16, 0x101D] {
        let v = if s.byte() < 0x20 { 0xFF } else { s.byte() % 0xFF };
        m.mem.w8(DS, at, v);
    }
    for at in [0x5873u16, 0xA902, 0x05C8] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let full = s.byte() < 0x20;
    for n in 0..0x20u16 {
        let occupied = full || s.byte() < 0x60;
        let v = if occupied { 0x61 + s.byte() % 26 } else { 0 };
        m.mem.w8(DATA, 0x2A80 + n * 0x55 + 0x34, v);
    }
    let mut sc = Script::default();
    for _ in 0..s.byte() % 40 {
        let r = [1u8, 1, 1, 5, 5, 2, 3, 0, 4, 1, 5, 2, 6, 7, 1, 3][(s.byte() % 16) as usize];
        let ox = s.byte();
        let oy = s.byte();
        let kind = s.byte() % 4;
        let idx = s.byte();
        let up = s.byte() % 4;
        sc.clicks.push((r, ox, oy, kind, idx, up));
    }
    for _ in 0..24 {
        let v = if s.byte() < 0x18 { 0xFFFF } else { s.byte() as u16 };
        sc.lists.push(v);
    }
    for _ in 0..12 {
        let v = if s.byte() < 0x18 { 0xFFFF } else { (s.byte() % 4) as u16 };
        sc.menus.push(v);
    }
    for _ in 0..4 {
        sc.names.push(NAMES[(s.byte() % 6) as usize]);
    }
    sc
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
    base.heap_init(0xB000, 0xF000).unwrap();
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0xA8FE, 0);
    base.mem.w16(DS, 0xA900, SCREEN);
    let dir = std::path::PathBuf::from(std::env::var("ARENA_EXE").map_or_else(
        |_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into(),
        |p| std::path::Path::new(&p).parent().unwrap().to_string_lossy().into_owned(),
    ));
    let Ok(mut files) = Files::new(&dir) else { return };
    assert!(base.bsa_open(&mut files));
    let only = std::env::var("SPELLMAKER_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/spellmaker.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "spellmaker", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut sc = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap());
        let before = (seg_bytes(&m, DS), seg_bytes(&m, DATA));
        let mut fs = files.clone();
        let got = m.spellmaker(&mut fs, &mut sc, 0x01A4).map(|()| {
            let after = seg_bytes(&m, DS);
            if only.is_some() {
                for (o, (a, b)) in before.0.iter().zip(&after).enumerate() {
                    if a != b {
                        eprintln!("{o:04x} {a:02x} {b:02x}");
                    }
                }
                eprintln!("log {:?}", sc.log);
                for (o, (a, b)) in before.1.iter().zip(&seg_bytes(&m, DATA)).enumerate().take(0x92A0) {
                    if a != b {
                        eprintln!("DATA {o:04x} {a:02x} {b:02x}");
                    }
                }
            }
            let d = [
                fnv(changes(&before.0, &after)),
                fnv(changes(&before.1[..0x92A0], &seg_bytes(&m, DATA)[..0x92A0])),
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
    eprintln!("spellmaker: {n} compared, {stuck} stuck in both");
    if only.is_none() {
        assert!(n > 2500, "{n} compared");
    }
}

/// Where the spellmaker gives up (here a click on a region whose handler
/// isn't one it knows), the name's backing block and the cursors' file are
/// freed, as its end frees them, so the heap doesn't lose them.
#[test]
fn a_spellmaker_given_up_frees_its_blocks() {
    struct Unknown;
    impl SpellmakerHooks for Unknown {
        fn click(&mut self, _m: &mut Machine, _form: Option<u16>) -> (u16, u16) {
            (5, 5)
        }
    }
    let Some(img) = common::image() else { return };
    let empty = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("no-spellmaker");
    std::fs::create_dir_all(&empty).unwrap();
    let mut files = Files::new(&empty).unwrap();
    let mut m = Machine::new(&img);
    m.heap_init(0xB000, 0xF000).unwrap();
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w16(DS, 0xA8FE, 0);
    m.mem.w16(DS, 0xA900, SCREEN);
    // The first region: (0, 0)-(10, 10), handler 1234h (no such routine).
    for (i, v) in [0u16, 0, 10, 10, 0x1234].into_iter().enumerate() {
        m.mem.w16(DS, 0x5711 + 2 * i as u16, v);
    }
    let mut fresh = m.clone();
    let block = fresh.heap_alloc_bytes(0xFFF0).map(|(s, _)| s);
    assert_eq!(m.spellmaker(&mut files, &mut Unknown, 0x2000), None);
    assert_eq!(m.heap_alloc_bytes(0xFFF0).map(|(s, _)| s), block, "the heap as it was");
}

/// #84 fixed (`SPELL_COST`): a spell's cost is its effects' costs times
/// its targets' steps + 2, halved, kept whole up to FFFFh; the original's
/// keeps the product's low word and adds the costs in 16 bits, so a big
/// enough spell wraps to a cheap one.
#[test]
fn fixed_a_big_spells_cost_doesnt_wrap() {
    use arena::fixes::{Fixes, SPELL_COST};
    use arena::spellmaker::var::{COSTS, EFFECTS, LEVEL, LOWEST, TARGET};
    let Some(img) = common::image() else { return };
    // One effect, or two (a class without factors, 01A9 0, adds the three
    // costs as they are), on the caster (target 0, the effects' lowest 0:
    // steps 0, times 2), at level 1.
    let cost = |fixes: Fixes, costs: [u16; 3]| {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.mem.w8(DS, 0x01A9, 0);
        for (i, &c) in costs.iter().enumerate() {
            m.mem.w8(DS, EFFECTS + i as u16, if c == 0 { 0xFF } else { i as u8 });
            m.mem.w16(DS, COSTS + 2 * i as u16, c);
            m.mem.w16(DS, LOWEST + 2 * i as u16, 0);
        }
        m.mem.w8(DS, TARGET, 0);
        m.mem.w16(DS, LEVEL, 1);
        m.made_spell_cost()
    };
    // 9000h x 2 = 12000h: the original's low word 2000h, halved.
    assert_eq!(cost(Fixes::NONE, [0x9000, 0, 0]), (0x1000, 0x1000));
    assert_eq!(cost(Fixes::only(SPELL_COST), [0x9000, 0, 0]), (0x9000, 0x9000));
    // 9000h + 9000h wraps to 2000h in the original; fixed it stops at
    // FFFFh, and so does the cost.
    assert_eq!(cost(Fixes::NONE, [0x9000, 0x9000, 0]), (0x2000, 0x2000));
    assert_eq!(cost(Fixes::only(SPELL_COST), [0x9000, 0x9000, 0]), (0xFFFF, 0xFFFF));
    // Small spells cost the same either way.
    assert_eq!(cost(Fixes::NONE, [0x123, 0x45, 0]), cost(Fixes::only(SPELL_COST), [0x123, 0x45, 0]));
}
