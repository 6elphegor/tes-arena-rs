//! Differential tests of a cast applied to its targets (src/spell.rs,
//! 29CF:0941) against vectors from the original code
//! (harness/gen_spell.py).
mod common;

use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

const CSEG: u16 = 0x7800;
const SSEG: u16 = 0x6000;
const TSEG: u16 = 0x5C00;
const WSEG: u16 = 0x6800;
const FRAME: u16 = 0x8800;
const PLAYER: u16 = 0x01A4;
const CSIZE: u16 = 0x41E;
const CASTS: u16 = 0x7F86;
const SLOTS: u16 = 0x0660;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

fn raw(s: &mut Stream, n: u16) -> Vec<u8> {
    (0..n).map(|_| s.byte()).collect()
}

/// A far pointer: the player or one of the creatures.
fn target(s: &mut Stream) -> (u16, u16) {
    let k = (s.byte() % 9) as u16;
    if k == 8 { (PLAYER, DS) } else { (k * CSIZE, CSEG) }
}

fn person(m: &mut Machine, s: &mut Stream, seg: u16, off: u16, creature: bool) {
    let b = raw(s, 0x6E);
    put(m, seg, off, &b);
    let ty = if creature { s.byte() % 0x18 } else { s.byte() % 8 };
    m.mem.w8(seg, off + 4, ty);
    let c = s.byte();
    let class = match c {
        0..=0x1F => 0x23,
        0x20..=0x2F => 0x11,
        0x30..=0x3F => 0x0F,
        _ => c,
    };
    m.mem.w8(seg, off + 5, class);
    let level = s.byte() % 30;
    m.mem.w8(seg, off + 6, level);
    let f = s.word() & if creature { 0x00D0 } else { 0x40D0 } | if creature { 0x1000 } else { 0 };
    m.mem.w16(seg, off + 0x40D, f);
    let b = raw(s, 9);
    put(m, seg, off + 0x40F, &b);
    let v = if s.byte() < 0x80 { s.word() } else { 0 };
    m.mem.w16(seg, off + 0x41A, v);
    let v = if s.byte() < 0xA0 { 0 } else { s.byte() as u16 };
    m.mem.w16(seg, off + 0x41C, v);
}

/// Mirrors harness/gen_spell.py `setup`; returns the cast record.
fn setup(m: &mut Machine, seed: u32) -> u16 {
    let mut s = Stream(seed);
    let s = &mut s;
    person(m, s, DS, PLAYER, false);
    for k in 0..8 {
        person(m, s, CSEG, k * CSIZE, true);
        let si = 0x0C57 + k * 0x1C;
        let v = s.word() & 0x3FFF;
        m.mem.w16(DS, si + 0xF, v);
        m.mem.w16(DS, si + 0x12, k * CSIZE);
    }
    for i in 0..18 {
        let v = if s.byte() < 0xF0 { (s.byte() % 8) as u16 } else { 0 };
        m.mem.w16(SSEG, 2 * i, v);
    }
    let ty = (s.byte() % 6).wrapping_sub(1);
    let element = s.byte() % 6;
    put(m, SSEG, 0x24, &[ty, 0, element]);
    let v = s.word();
    m.mem.w16(SSEG, 0x27, v);
    let mut kinds = [0u8; 3];
    for k in kinds.iter_mut() {
        let t = s.byte();
        *k = match t {
            0..=0x17 => 0xFF,
            0x18..=0x1F => 0x19 + t % 8,
            _ => t % 0x19,
        };
    }
    let mut subs = [0u8; 3];
    for v in subs.iter_mut() {
        *v = s.byte() % 6;
    }
    for (k, v) in kinds.iter().zip(subs.iter_mut()) {
        if *k == 13 {
            *v %= 2;
        }
    }
    let params: Vec<u8> = (0..3).map(|_| s.byte() % 16).collect();
    put(m, SSEG, 0x29, &kinds);
    put(m, SSEG, 0x2C, &subs);
    put(m, SSEG, 0x2F, &params);
    let v = s.word();
    m.mem.w16(SSEG, 0x32, v);
    let cast = CASTS + (s.byte() % 8) as u16 * 0x16;
    let (off, seg) = target(s);
    let level = 1 + (s.byte() % 30) as u16;
    let flags = s.word();
    for (i, v) in [0, SSEG, off, seg, level, flags].into_iter().enumerate() {
        m.mem.w16(DS, cast + 2 * i as u16, v);
    }
    let n = 1 + (s.byte() % 3) as u16;
    m.mem.w16(DS, 0x0D5A, n);
    for i in 0..16 {
        let (off, seg) = target(s);
        m.mem.w16(DS, 0x0D5C + 4 * i, off);
        m.mem.w16(DS, 0x0D5E + 4 * i, seg);
    }
    m.mem.w16(DS, 0x1020, 0);
    m.mem.w16(DS, 0x1022, TSEG);
    let b = raw(s, 0x240);
    put(m, TSEG, 0, &b);
    let b = raw(s, 16 * 0x3C);
    put(m, DS, SLOTS, &b);
    for i in 0..16 {
        let at = SLOTS + i * 0x3C;
        if s.byte() < 0x80 {
            m.mem.w8(DS, at + 0x3B, 0xFF);
        } else {
            let (off, seg) = target(s);
            m.mem.w16(DS, at + 0x28, off);
            m.mem.w16(DS, at + 0x2A, seg);
            let k = s.byte() % 12;
            m.mem.w8(DS, at + 0x3A, k);
        }
    }
    let v = 1 + (s.byte() % 30) as u16;
    m.mem.w16(DS, 0x1292, v);
    let v = s.byte() & 1;
    m.mem.w8(DS, 0x05E6, v);
    let b = raw(s, 10);
    put(m, DS, 0x0DC0, &b);
    let v = if s.byte() < 0x80 { 0x3C } else { s.word() };
    m.mem.w16(DS, 0xC3D2, v);
    let sq = ((s.byte() % 64) as u16) << 8 | ((s.byte() % 64) as u16) << 1;
    m.mem.w16(DS, 0xCB90, sq);
    let v = if s.byte() < 0x80 { 0 } else { s.word() };
    m.mem.w16(WSEG, sq, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = s.byte();
    m.mem.w8(DS, 0xC942, v);
    let o = s.byte() as u16;
    m.mem.w16(DS, 0xD0B8 + 4 * 0x19, o);
    m.mem.w16(DS, 0xD0BA + 4 * 0x19, 0);
    let b = raw(s, 0x110);
    put(m, FRAME, 0, &b);
    let b = raw(s, 4);
    put(m, DS, 0x91F6, &b);
    cast
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.mem.w16(DS, 0x1042, CSEG);
    base.mem.w16(DS, 0xD806, WSEG);
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w16(DS, 0x91F0, 0);
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for line in include_str!("vectors/spell.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "cast", "{line}");
        let seed = hex(f[1]) as u32;
        let mut m = base.clone();
        let cast = setup(&mut m, seed);
        m.load_rng();
        let before = m.clone();
        let got = m.cast_apply(cast, &mut arena::strike::NoKillHooks).map(|()| {
            m.store_rng();
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            format!(
                "{:x} {:x} {:x} {:x}",
                fnv(changed),
                fnv((0..0x8000u16).map(|o| m.mem.r8(CSEG, o))),
                fnv((0..0x400u16).map(|o| m.mem.r8(TSEG, o))),
                fnv((0..0x8000u16).map(|o| m.mem.r8(WSEG, o)))
            )
        });
        if std::env::var("SPELL_DUMP").is_ok_and(|v| v == f[1]) {
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
            for o in 0..0x400u16 {
                if m.mem.r8(TSEG, o) != before.mem.r8(TSEG, o) {
                    eprintln!("t{o:04x} {:02x} {:02x}", before.mem.r8(TSEG, o), m.mem.r8(TSEG, o));
                }
            }
        }
        match got {
            _ if f[2] == "hang" => {}
            Some(d) => assert_eq!(f[2], d, "{line}"),
            None => assert_eq!(f[2], "fault", "{line}"),
        }
        *counts.entry(if f[2] == "fault" { "fault" } else { "ran" }).or_default() += 1;
    }
    eprintln!("{counts:?}");
    assert!(counts.get("ran").copied().unwrap_or(0) > 2000);
}

/// A machine for one spell effect: the spell at SSEG:0 with effect 0 of
/// `kind`, `sub` and `param` and amounts `amounts` (+0, +6, +C, +12, +18,
/// +1E), the others none; creatures in CSEG, timers in TSEG, every effect
/// slot free.
fn one_effect(img: &arena::exe::Image, fixes: arena::fixes::Fixes, kind: u8, sub: u8, param: u8, amounts: [u16; 6]) -> Machine {
    let mut m = Machine::new(img);
    m.fixes = fixes;
    m.mem.w16(DS, 0x1042, CSEG);
    m.mem.w16(DS, 0xD806, WSEG);
    m.mem.w16(DS, 0x1020, 0);
    m.mem.w16(DS, 0x1022, TSEG);
    for (i, v) in amounts.into_iter().enumerate() {
        m.mem.w16(SSEG, 6 * i as u16, v);
    }
    put(&mut m, SSEG, 0x24, &[0, 0, 5, 0, 0]);
    put(&mut m, SSEG, 0x29, &[kind, 0xFF, 0xFF, sub, 0, 0, param, 0, 0]);
    for i in 0..16 {
        m.mem.w8(DS, SLOTS + i * 0x3C + 0x3B, 0xFF);
    }
    // A creature at CSEG:0 with some HP.
    m.mem.w16(CSEG, 0x40D, 0x1000);
    m.mem.w16(CSEG, 0x59, 50);
    m
}

/// The message over the view after a cast (0 for none).
fn message(m: &Machine) -> u16 {
    m.mem.r16(DS, 0x061A)
}

const FAILED: u16 = 0x7BFA;
const SUCCEEDED: u16 = 0x7C05;

/// Fixed (kept bug #18): an instant-death spell with a 10% chance kills
/// about one time in ten, where the original's reversed comparison kills
/// nine times in ten; and a kill outside DGROUP goes on (DS restored)
/// where the original runs into data.
#[test]
fn fixed_instant_death_kills_on_a_roll_under_the_chance() {
    use arena::fixes::{Fixes, INSTANT_DEATH};
    let Some(img) = common::image() else { return };
    let kills = |fixes: Fixes| {
        (0..400u32)
            .filter(|&seed| {
                let mut m = one_effect(&img, fixes, 7, 0, 0, [10, 0, 1, 0, 1, 0]);
                m.rng.seed = seed.wrapping_mul(2_654_435_761);
                let got = m.spell_effects(SSEG, 0, CSEG, 0, 1, &mut arena::strike::NoKillHooks);
                if fixes.has(INSTANT_DEATH) {
                    assert!(got.is_some(), "seed {seed}");
                }
                got.is_none() || message(&m) != FAILED
            })
            .count()
    };
    let n = kills(Fixes::NONE);
    assert!((320..400).contains(&n), "the original: {n} of 400 killed");
    let n = kills(Fixes::only(INSTANT_DEATH));
    assert!((15..70).contains(&n), "fixed: {n} of 400 killed");
}

/// Fixed (kept bug #21): the success roll's message shows when the
/// player is the target (the original's check reads the multiply's high
/// word, so it doesn't), and 2CDD:0404's success message only then (the
/// original's looks at the slot pointer and always shows).
#[test]
fn fixed_spell_messages_show_for_the_player() {
    use arena::fixes::{Fixes, SPELL_MESSAGE};
    let Some(img) = common::image() else { return };
    let fixed = Fixes::only(SPELL_MESSAGE);
    // Kind 0 sub 4 (2CDD:012F): a roll that always succeeds, on the player.
    for (fixes, want) in [(Fixes::NONE, 0), (fixed, SUCCEEDED)] {
        let mut m = one_effect(&img, fixes, 0, 4, 0, [100, 0, 1, 0, 1, 0]);
        m.mem.w16(DS, 0x061A, 0);
        m.spell_effects(SSEG, 0, DS, PLAYER, 1, &mut arena::strike::NoKillHooks).unwrap();
        assert_eq!(message(&m), want, "{fixes:?}");
    }
    // Kind 9 (2CDD:0404) on a creature.
    for (fixes, want) in [(Fixes::NONE, SUCCEEDED), (fixed, 0)] {
        let mut m = one_effect(&img, fixes, 9, 0, 0, [1, 1, 1, 1, 1, 1]);
        m.mem.w16(DS, 0x061A, 0);
        m.spell_effects(SSEG, 0, CSEG, 0, 1, &mut arena::strike::NoKillHooks).unwrap();
        assert_eq!(message(&m), want, "{fixes:?}");
    }
}

/// Fixed (kept bug #22): 2CDD:030F's timed flag 2 goes on the target,
/// beside its timer; the original sets it on the player.
#[test]
fn fixed_timed_flag_goes_on_the_target() {
    use arena::fixes::{Fixes, TIMED_FLAG_PLAYER};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(TIMED_FLAG_PLAYER)] {
        let mut m = one_effect(&img, fixes, 5, 0, 0, [100, 0, 1, 0, 1, 3]);
        m.mem.w16(DS, PLAYER + 0x41A, 0);
        m.spell_effects(SSEG, 0, CSEG, 0, 1, &mut arena::strike::NoKillHooks).unwrap();
        let (player, target) = (m.mem.r16(DS, PLAYER + 0x41A), m.mem.r16(CSEG, 0x41A));
        if fixes == Fixes::NONE {
            assert_eq!((player, target), (2, 0));
        } else {
            assert_eq!((player, target), (0, 2));
        }
    }
}

/// Fixed (kept bug #28): the fatigue drain (2CDD:0180) writes nothing
/// into a target that already has the effect; the original tests the
/// wrong carry and stores the delta into the target's record.
#[test]
fn fixed_fatigue_drain_stores_nothing_when_refused() {
    use arena::fixes::{FATIGUE_DRAIN_CARRY, Fixes};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(FATIGUE_DRAIN_CARRY)] {
        let mut m = one_effect(&img, fixes, 1, 1, 0, [0xFFFB, 0xFFFB, 0, 0, 1, 1]);
        m.mem.w16(CSEG, 0x40D, 0x1200);
        m.mem.w16(CSEG, 0x12, 0xBEEF);
        m.spell_effects(SSEG, 0, CSEG, 0, 1, &mut arena::strike::NoKillHooks).unwrap();
        let want = if fixes == Fixes::NONE { 5 << 6 } else { 0xBEEF };
        assert_eq!(m.mem.r16(CSEG, 0x12), want, "{fixes:?}");
    }
}

/// Fixed (kept bug #67): ending an effect decrements the record's own
/// counter, not DGROUP's at the record's offset.
#[test]
fn fixed_end_effect_counts_down_the_record() {
    use arena::fixes::{END_EFFECT_COUNTER, Fixes};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(END_EFFECT_COUNTER)] {
        let mut m = one_effect(&img, fixes, 0xFF, 0, 0, [0; 6]);
        let kind = m.effect_kind(1);
        assert!(kind < 9);
        let rec = 2 * CSIZE;
        let at = rec + kind + 0x40F;
        m.mem.w8(CSEG, at, 3);
        m.mem.w8(DS, at, 7);
        m.end_effect_flag(CSEG, rec, 1);
        let got = (m.mem.r8(CSEG, at), m.mem.r8(DS, at));
        assert_eq!(got, if fixes == Fixes::NONE { (3, 6) } else { (2, 7) }, "{fixes:?}");
    }
}

/// Fixed (kept bug #69): kinds 2 and 6 on a creature outside DGROUP go
/// on after their sub-kind handler; the original reads on through the
/// creature's segment (the port gives up: None).
#[test]
fn fixed_sub_kind_handlers_restore_ds() {
    use arena::fixes::{Fixes, SUB_KIND_DS};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(SUB_KIND_DS)] {
        // Kind 2 sub 3 (2CDD:021C) with a roll that always fails.
        let mut m = one_effect(&img, fixes, 2, 3, 0, [0, 0, 1, 0, 1, 0]);
        let got = m.spell_effects(SSEG, 0, CSEG, 0, 1, &mut arena::strike::NoKillHooks);
        assert_eq!(got.is_some(), fixes != Fixes::NONE, "{fixes:?}");
    }
}

/// Fixed (kept bug #85): Spell Absorption's strength is its per-level
/// part plus its base percent (+0), as Reflection's and Resistance's are;
/// the original's leaves the base out, so "40% + 0 per level" absorbs
/// nothing.
#[test]
fn fixed_absorption_counts_its_base_percent() {
    use arena::fixes::{ABSORB_BASE, Fixes};
    let Some(img) = common::image() else { return };
    let m = Machine::new(&img);
    // Absorption's kind: the one 83E2's table sends to 2CDD:0822.
    let kind = (0..0x19u16).find(|&k| m.mem.r16(DS, 0x83E2 + 2 * k) == 0x0822).expect("Absorption's kind") as u8;
    for fixes in [Fixes::NONE, Fixes::only(ABSORB_BASE)] {
        // 40% + 0% a level, for a round a level.
        let mut m = one_effect(&img, fixes, kind, 0, 0, [40, 0, 1, 0, 1, 1]);
        m.spell_effects(SSEG, 0, DS, PLAYER, 1, &mut arena::strike::NoKillHooks).unwrap();
        // The player's timers: block 0 of 1020's table; entry 28h, 400h's.
        assert_ne!(m.mem.r16(DS, PLAYER + 0x41A) & 0x400, 0, "{fixes:?}: the flag");
        let strength = m.mem.r16(TSEG, 0x28 + 2);
        assert_eq!(strength, if fixes == Fixes::NONE { 0 } else { 40 }, "{fixes:?}");
    }
}
