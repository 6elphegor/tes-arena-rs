//! Differential tests of the save and load menus (src/savemenu.rs
//! `save_menu`, 25FD:0002..010F, `load_menu`, 25FD:0242..028C, and the
//! slot picker 25FD:0718) against vectors from the original code
//! (harness/gen_savemenu.py). The hooks play the generator's script and
//! log what its stubs log.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::savemenu::{MenuHooks, SaveChoice};
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

struct Script {
    names: Option<Vec<u8>>,
    clicks: Vec<(u16, u16, u8)>,
    edits: Vec<Option<Vec<u8>>>,
    log: Vec<Vec<u16>>,
}

/// Mirrors harness/gen_savemenu.py `setup` (a conditional's test byte
/// comes first; a tuple's elements before its index).
fn setup(m: &mut Machine, seed: u32) -> (u8, Script) {
    let mut st = Stream(seed);
    let s = &mut st;
    let op = s.byte() & 1;
    let v = if s.byte() < 0x40 { 4 } else { s.byte() % 5 };
    m.mem.w8(DS, 0x05E6, v);
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0D48, v);
    let v = if s.byte() < 0x40 { s.byte() & 3 } else { 0 };
    m.mem.w8(DS, 0xAFF9, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x191D, v);
    for at in [0x05E0, 0xA902, 0x05C8, 0x102C, 0x1036, 0x1030, 0x061E, 0x7A46] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = s.byte();
    m.mem.w8(DS, 0x1A6C, v);
    let v = s.byte() % 10;
    m.mem.w8(DS, 0x1A80, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC995, v);
    for i in 0..0x1E0 {
        let v = s.byte();
        m.mem.w8(DS, 0x9686 + i, v);
    }
    let mut names = None;
    if s.byte() >= 0x20 {
        let mut all = Vec::new();
        for _ in 0..10 {
            let mut e = vec![0u8; 0x30];
            if s.byte() >= 0x60 {
                let n = (s.byte() % 20) as usize;
                for c in e.iter_mut().take(n) {
                    *c = 0x41 + s.byte() % 26;
                }
            }
            all.extend(e);
        }
        names = Some(all);
    }
    let mut clicks = Vec::new();
    for _ in 0..1 + s.byte() % 4 {
        let (ax, bx) = match s.byte() % 8 {
            0 => (0xFFFF, s.byte() as u16),
            1 => {
                let a = (s.byte() % 5) as u16;
                let b = 0x13B + (s.byte() % 4) as u16;
                let ax = if s.byte() % 2 == 0 { a } else { b };
                (ax, 4 + (s.byte() % 0x80) as u16)
            }
            2 => (5 + (s.byte() as u16) % 0x136, (s.byte() % 4) as u16),
            _ => (5 + s.word() % 0x136, 4 + (s.byte() % 0x9A) as u16),
        };
        let buttons = if s.byte() < 0x40 { s.byte() & 3 } else { 0 };
        clicks.push((ax, bx, buttons));
    }
    let mut edits = Vec::new();
    for _ in 0..1 + s.byte() % 3 {
        if s.byte() < 0x40 {
            edits.push(None);
        } else {
            let n = s.byte() % 0x30;
            edits.push(Some((0..n).map(|_| 0x61 + s.byte() % 26).collect()));
        }
    }
    clicks.reverse();
    edits.reverse();
    (op, Script { names, clicks, edits, log: Vec::new() })
}

impl MenuHooks for Script {
    fn mouse_wait(&mut self, m: &mut Machine) {
        self.log.push(vec![1, 0]);
        m.mem.w8(DS, 0xAFF9, 0);
    }
    fn font(&mut self, _m: &mut Machine, ax: u16, text: bool) {
        self.log.push(vec![if text { 3 } else { 2 }, ax]);
    }
    fn map_bank(&mut self, _m: &mut Machine, bank: u16) {
        self.log.push(vec![4, bank]);
    }
    fn picture(&mut self, _m: &mut Machine, _files: &mut Files, name: u16) {
        self.log.push(vec![6, name]);
    }
    fn names(&mut self, m: &mut Machine, _files: &mut Files, path: u16) {
        self.log.push(vec![5, path]);
        if let Some(n) = &self.names {
            for (i, &b) in n.iter().enumerate() {
                m.mem.w8(DS, 0x9686 + i as u16, b);
            }
        }
    }
    fn fill(&mut self, _m: &mut Machine, ax: u16, bx: u16, cx: u16, dx: u16) {
        self.log.push(vec![7, ax, bx, cx, dx]);
    }
    fn centred(&mut self, _m: &mut Machine, si: u16, ax: u16, bx: u16) {
        self.log.push(vec![8, si, ax, bx]);
    }
    fn show(&mut self, _m: &mut Machine) {
        self.log.push(vec![9, 0]);
    }
    fn click(&mut self, m: &mut Machine) -> (u16, u16) {
        let (ax, bx, buttons) = self.clicks.pop().unwrap_or((0xFFFF, 0, 0));
        self.log.push(vec![10, ax, bx]);
        m.mem.w8(DS, 0xAFF9, buttons);
        (ax, bx)
    }
    fn type_name(&mut self, m: &mut Machine) {
        self.log.push(vec![11, 0]);
        match self.edits.pop().unwrap_or(Some(Vec::new())) {
            None => {
                let f = m.mem.r16(DS, 0xA902) & 0xEFFF;
                m.mem.w16(DS, 0xA902, f);
            }
            Some(t) => {
                for (i, &b) in t.iter().chain([0u8].iter()).enumerate() {
                    m.mem.w8(DS, 0x1044 + i as u16, b);
                }
            }
        }
    }
    fn message(&mut self, _m: &mut Machine, si: u16) {
        self.log.push(vec![12, si]);
    }
    fn delete_matching(&mut self, _m: &mut Machine, _files: &mut Files, si: u16) {
        self.log.push(vec![13, si]);
    }
    fn states_keep(&mut self, _m: &mut Machine, _files: &mut Files) {
        self.log.push(vec![14, 0]);
    }
    fn automap_keep(&mut self, _m: &mut Machine, _files: &mut Files) {
        self.log.push(vec![15, 0]);
    }
    fn load_screen(&mut self, _m: &mut Machine) {
        self.log.push(vec![16, 0]);
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/savemenu.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "savemenu", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let (op, mut script) = setup(&mut m, seed);
        let before = m.clone();
        let mut fs = Files::default();
        let slot = if op == 0 {
            match m.save_menu(&mut fs, &mut script) {
                SaveChoice::Slot(s) => Some(s),
                _ => None,
            }
        } else {
            m.load_menu(&mut fs, &mut script)
        };
        let r: Vec<&str> = f[2].split(' ').collect();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let log: Vec<u8> = script.log.iter().flatten().flat_map(|w| w.to_le_bytes()).collect();
        let end = if slot.is_some() { "slot" } else { "back" };
        assert_eq!(r[0], end, "{line}");
        // AL is the slot at the load's stop (the save's holds a digit).
        if let (Some(s), 1) = (slot, op) {
            assert_eq!(r[1], format!("{s:x}"), "{line}: AL");
        }
        assert_eq!(r[2], format!("{:x}", fnv(changed)), "{line}: DGROUP");
        assert_eq!(r[3], format!("{:x}", fnv(log)), "{line}: calls");
        n += 1;
    }
    assert!(n > 2500, "{n} cases");
}
