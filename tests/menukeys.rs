//! Differential tests of the menu keys of play (src/menukeys.rs
//! `menu_keys`, 2542:0100) against vectors from the original code
//! (harness/gen_menukeys.py). The screens the port names are logged as
//! the generator's stubs log their handlers.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::menukeys::{MenuKey, MenuKeyHooks};
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const KEYS: [u16; 13] = [0x01, 0x1E, 0x2E, 0x19, 0x1F, 0x16, 0x32, 0x3B, 0x42, 0x26, 0x13, 0x2A, 0x36];

struct Script {
    masks: u8,
    dl: Vec<u8>,
    carry: bool,
    log: Vec<Vec<u16>>,
}

/// Mirrors harness/gen_menukeys.py `setup` (a conditional's test byte
/// comes first).
fn setup(m: &mut Machine, seed: u32) -> Script {
    let mut s = Stream(seed);
    let density = 1 + s.byte() % 0x40;
    for scan in KEYS {
        let held = s.byte() < density;
        m.mem.w8(DS, 0xACB5 + scan, held as u8);
        let seen = s.byte() & 1;
        m.mem.w8(DS, 0x9208 + scan, seen);
    }
    for at in [0xC39A, 0x05CA, 0x05E0, 0x1947, 0x05C8, 0x061A, 0x061C, 0x061E] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = if s.byte() < 0xC0 { s.byte() } else { 0 };
    m.mem.w8(DS, 0x01AD, v);
    let masks = if s.byte() < 0x10 { s.byte() } else { 0 };
    let mut dl = Vec::new();
    if s.byte() < 0x10 {
        dl = vec![1; 1 + s.byte() as usize % 3];
    }
    dl.push(0);
    dl.reverse();
    let carry = s.byte() < 0x80;
    Script { masks, dl, carry, log: Vec::new() }
}

impl MenuKeyHooks for Script {
    fn cyber_button(&mut self, _m: &mut Machine, mask: u8) -> bool {
        self.log.push(vec![11, mask as u16]);
        self.masks & mask != 0
    }
    fn cyber_buttons(&mut self, _m: &mut Machine) -> u8 {
        let v = self.dl.pop().unwrap_or(0);
        self.log.push(vec![12, v as u16]);
        v
    }
    fn touch_cast(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<bool> {
        self.log.push(vec![9, 1, 0x01A4, self.carry as u16]);
        Some(self.carry)
    }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/menukeys.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "menukeys", "{line}");
        let seed = u32::from_str_radix(f[1], 16).unwrap();
        let mut m = base.clone();
        let mut script = setup(&mut m, seed);
        let before = m.clone();
        let mut fs = Files::default();
        let key = m.menu_keys(&mut fs, &mut script).expect("menu keys");
        // The handlers the generator stubs, by what the port names.
        let code = match key {
            MenuKey::Options => 1,
            MenuKey::Spellbook => 2,
            MenuKey::Status => 3,
            MenuKey::Use => 4,
            MenuKey::TravelMap => 5,
            MenuKey::Automap => 6,
            MenuKey::Log => 7,
            MenuKey::Camp => 8,
            MenuKey::Sheet => 10,
            MenuKey::None | MenuKey::Ready | MenuKey::Pickpocket | MenuKey::TouchCast | MenuKey::Compass => 0,
        };
        if code != 0 {
            script.log.push(vec![code, 0]);
        }
        let r: Vec<&str> = f[2].split(' ').collect();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let log: Vec<u8> = script.log.iter().flatten().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(r[0], format!("{:x}", fnv(changed)), "{line}: DGROUP ({key:?})");
        assert_eq!(r[1], format!("{:x}", fnv(log)), "{line}: calls ({key:?})");
        n += 1;
    }
    assert!(n > 5000, "{n} cases");
}
