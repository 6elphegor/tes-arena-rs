//! Differential tests of the quests' lines in the equipment list (src/
//! questitems.rs `quest_item_names`, 1194:A206 and 2433:083D) against
//! vectors from the original code (harness/gen_questitems.py): the
//! changed DGROUP and DATA bytes and the artifact's count compared by
//! digest. Needs the game directory (NAMECHNK.DAT).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use common::blow::{put, Stream};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn changes(a: &Machine, b: &Machine, seg: u16) -> u64 {
    fnv((0..=0xFFFFu16).filter(|&o| a.mem.r8(seg, o) != b.mem.r8(seg, o)).flat_map(|o| [o as u8, (o >> 8) as u8, b.mem.r8(seg, o)]))
}

/// gen_questitems.py `setup`.
fn setup(m: &mut Machine, seed: u32) {
    let mut s = Stream(seed);
    for (base, size) in [(0x0DCAu16, 0x29u16), (0x0E6E, 0x34)] {
        for i in 0..4 {
            let mut r: Vec<u8> = (0..size).map(|_| s.byte()).collect();
            let mut flags = u16::from_le_bytes([r[4], r[5]]);
            if s.byte() < 0xD0 {
                flags = (flags & !0xC0) | 0x40;
            }
            flags = (flags & !0xE00) | ((s.byte() % 8) as u16) << 9;
            r[4..6].copy_from_slice(&flags.to_le_bytes());
            if s.byte() < 0xE0 {
                r[0x25] = (r[0x25] & 0x1F) | (s.byte() % 8) << 5;
                r[0x28] = s.byte() & 1;
                r[0x27] = s.byte() % 8;
                r[6] = s.byte() % 10;
                r[7] = 0;
                if size == 0x34 {
                    r[0x2F] = 0x18 + s.byte() % 8;
                    r[0x30] = 0;
                }
            }
            put(m, DS, base + i * size, &r);
        }
    }
    let pieces = s.byte() % 9;
    m.mem.w8(DS, 0x0F77, pieces);
    let artifact = if s.byte() < 0x80 { 0 } else { 1 + s.byte() % 0xFF };
    m.mem.w8(DS, 0x0F7F, artifact);
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: the game directory not found");
        return;
    };
    let mut base = Machine::new(&img);
    base.heap_init(0xA000, 0xF000).unwrap();
    assert!(base.bsa_open(&mut files));
    let mut n = 0;
    for line in include_str!("vectors/questitems.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        let mut m = base.clone();
        setup(&mut m, f[1].parse().unwrap());
        m.mem.w16(DS, 0x126C, 0);
        m.mem.w16(DS, 0x91F0, 0);
        m.load_rng();
        let before = m.clone();
        let got = m.quest_item_names(&mut files, 0x1044).map(|ax| {
            m.store_rng();
            if std::env::var("QI_DUMP").is_ok_and(|d| d == f[1]) {
                for o in (0..=0xFFFFu16).filter(|&o| before.mem.r8(DS, o) != m.mem.r8(DS, o)) {
                    eprintln!("{o:04x} {:02x} {:02x}", before.mem.r8(DS, o), m.mem.r8(DS, o));
                }
            }
            format!("{:x} {:x} {ax:x}", changes(&before, &m, DS), changes(&before, &m, DATA))
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None => assert!(matches!(f[2], "fault" | "hang"), "{line}: the port gave up: {:?}", arena::giveup::take()),
        }
    }
    assert!(n > 2500, "{n} compared");
}
