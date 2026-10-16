//! Differential tests of the temple (src/shops.rs `temple`, 1194:C258)
//! against vectors from the original code (harness/gen_temple.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use arena::shops::ShopHooks;
use common::blow::Stream;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const TYPED: [&[u8]; 11] = [b"", b"abc", b"0", b"7", b"25", b"60", b"150", b"1000", b"9999", b"-5", b"4x"];

#[derive(Default)]
struct Script {
    menus: Vec<u16>,
    yes: Vec<u16>,
    typed: Vec<&'static [u8]>,
    lists: Vec<u16>,
    n: [usize; 4],
    log: Vec<u16>,
}

fn take<T: Copy>(q: &[T], n: &mut usize, default: T) -> T {
    let v = q.get(*n).copied().unwrap_or(default);
    *n += 1;
    v
}

impl ShopHooks for Script {
    fn message(&mut self, _m: &mut Machine, si: u16) {
        self.log.extend([1, si]);
    }
    fn menu(&mut self, _m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.log.extend([2, si, bx, di]);
        take(&self.menus, &mut self.n[0], 3)
    }
    fn dialog(&mut self, m: &mut Machine, kind: u8, di: u16) -> u16 {
        self.log.extend([3, kind as u16, if kind == 2 { di } else { 0 }]);
        if kind == 2 {
            let t = take(&self.typed, &mut self.n[2], b"");
            for (i, &c) in t.iter().chain(&[0]).enumerate() {
                m.mem.w8(DS, di + i as u16, c);
            }
            return 0;
        }
        take(&self.yes, &mut self.n[1], 1)
    }
    fn list(&mut self, _m: &mut Machine, si: u16, cx: u16, di: u16) -> u16 {
        self.log.extend([4, si, cx, di]);
        let v = take(&self.lists, &mut self.n[3], 0xFFFF);
        if v == 0xFFFF { v } else { v % cx.max(1) }
    }
    fn expand(&mut self, m: &mut Machine, di: u16) -> u16 {
        self.log.extend([5, di]);
        let mut di = di;
        while m.mem.r8(DS, di) != 0 {
            di = di.wrapping_add(1);
        }
        di
    }
    fn quest_check(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        self.log.push(6);
        Some(())
    }
    fn quests(&mut self, _m: &mut Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        self.log.push(7);
        Some(())
    }
}

/// Mirrors harness/gen_temple.py `setup` (a conditional's test byte comes
/// first).
fn setup(m: &mut Machine, seed: u32, holidays: &[u16]) -> Script {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = if s.byte() < 0xF0 { s.byte() % 30 } else { 0xFF };
    m.mem.w8(DS, 0x01AA, v);
    let mx = s.word() % 400;
    m.mem.w16(DS, 0x01FF, mx);
    let v = if s.byte() < 0x40 { mx } else { s.word() % 400 };
    m.mem.w16(DS, 0x01FD, v);
    let g = if s.byte() < 0xC0 {
        (s.word() % 3000) as u32
    } else {
        let lo = s.word() as u32;
        lo | (s.word() as u32) << 16
    };
    m.mem.w32(DS, 0x05C2, g);
    let v = s.word() & 0xFFF8;
    let v = v | (s.byte() & 7) as u16;
    m.mem.w16(DS, 0x05B1, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C6, v);
    let day = if s.byte() < 0x60 { holidays[s.byte() as usize % 16] as u32 } else { (s.word() % 360) as u32 };
    let year = (s.byte() % 8) as u32;
    let t = year.wrapping_mul(0x7E900).wrapping_add(day.wrapping_mul(0x5A0)).wrapping_add((s.word() % 0x5A0) as u32);
    m.mem.w32(DS, 0x05CC, t);
    let last = match s.byte() % 4 {
        0 => 0,
        1 => t.wrapping_sub((s.word() % 0x5A0) as u32),
        2 => t.wrapping_sub(0x5A0).wrapping_sub(if s.byte() < 0x40 { 0 } else { 1 + (s.word() % 0x500) as u32 }),
        _ => {
            let lo = s.word() as u32;
            lo | (s.word() as u32) << 16
        }
    };
    m.mem.w32(DS, 0x13CD, last);
    let v = s.word();
    m.mem.w16(DS, 0xA91A, v);
    let v = s.byte();
    m.mem.w8(DS, 0x0D39, v);
    let v = s.byte();
    m.mem.w8(DS, 0x13C9, v);
    for i in 0..9 {
        let v = s.byte() % 4;
        m.mem.w8(DS, 0x01A4 + 0x40F + i, v);
    }
    for i in 0..16u16 {
        let mut slot: Vec<u8> = (0..0x3C).map(|_| s.byte()).collect();
        if s.byte() < 0x80 {
            slot[0x28..0x2A].copy_from_slice(&0x01A4u16.to_le_bytes());
            slot[0x2A..0x2C].copy_from_slice(&DS.to_le_bytes());
        }
        slot[0x3A] = if s.byte() < 0xE0 { s.byte() % 12 } else { 0xFF };
        for (k, &b) in slot.iter().enumerate() {
            m.mem.w8(DS, 0x0660 + 0x3C * i + k as u16, b);
        }
    }
    let menus = (0..6).map(|_| [0u16, 1, 2, 3, 0xFFFF, 0, 1, 2][s.byte() as usize % 8]).collect();
    let yes = (0..6).map(|_| (s.byte() & 1) as u16).collect();
    let typed = (0..6).map(|_| TYPED[s.byte() as usize % TYPED.len()]).collect();
    let lists = (0..6).map(|_| if s.byte() < 0x30 { 0xFFFF } else { (s.byte() % 8) as u16 }).collect();
    Script { menus, yes, typed, lists, ..Default::default() }
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let holidays: Vec<u16> = (0..16).map(|k| base.mem.r16(DS, 0x33AD + 2 * k)).collect();
    let only = std::env::var("TEMPLE_SEED").ok();
    let (mut n, mut stuck) = (0, 0);
    for line in include_str!("vectors/temple.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "temple", "{line}");
        if only.as_deref().is_some_and(|s| s != f[1]) {
            continue;
        }
        let mut m = base.clone();
        let mut sc = setup(&mut m, u32::from_str_radix(f[1], 16).unwrap(), &holidays);
        let before: Vec<u8> = (0..=0xFFFFu16).map(|o| m.mem.r8(DS, o)).collect();
        let got = m.temple(&mut arena::bsa::Files::default(), &mut sc).map(|()| {
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before[o as usize])
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            if only.is_some() {
                for o in 0..=0xFFFFu16 {
                    if m.mem.r8(DS, o) != before[o as usize] {
                        eprintln!("{o:04x} {:02x} {:02x}", before[o as usize], m.mem.r8(DS, o));
                    }
                }
                eprintln!("log {:?}", sc.log);
            }
            format!("{:x} {:x}", fnv(changed), fnv(sc.log.iter().flat_map(|v| v.to_le_bytes())))
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
    eprintln!("temple: {n} compared, {stuck} stuck in both");
    if only.is_none() {
        assert!(n > 2500, "{n} compared");
    }
}
