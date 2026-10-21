//! Differential test of status effects against vectors produced by
//! running the original code (harness/gen_effects.py).
mod common;

use arena::effects::{Applied, FxHooks};
use arena::machine::Machine;
use arena::mem::DS;
use std::collections::HashMap;

const MSEG: u16 = 0x5000;
const OSEG: u16 = 0x5800;
const REGIONS: [(usize, usize); 3] = [
    (0x3276 * 16, 0x3BA0 * 16 + 0x10000),
    (MSEG as usize * 16, MSEG as usize * 16 + 0x1400),
    (OSEG as usize * 16, OSEG as usize * 16 + 0x600),
];

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn named(s: &str) -> HashMap<String, u32> {
    s.split_whitespace().map(|p| p.split_once(':').unwrap()).map(|(k, v)| (k.to_string(), hex(v))).collect()
}

/// Records the stubbed handlers' calls like the emulator's stubs.
#[derive(Default)]
struct Hooks {
    kill_cf: bool,
    kills: u16,
    last: (u16, u16),
    deaths: u16,
    reflections: u16,
    reflected: u16,
}

impl FxHooks for Hooks {
    fn vitals_changed(&mut self, _: &mut Machine, seg: u16, rec: u16) -> bool {
        self.kills += 1;
        self.last = (seg, rec);
        self.kill_cf
    }
    fn player_died(&mut self, _: &mut Machine) {
        self.deaths += 1;
    }
    fn spell_reflected(&mut self, _: &mut Machine, actor: u16) {
        self.reflections += 1;
        self.reflected = actor;
    }
}

/// Runs one routine; None where the port reports the original's fault.
/// Returns the registers the routine defines as results.
fn run(m: &mut Machine, name: &str, r: &HashMap<String, u32>, env: [bool; 5], h: &mut Hooks)
    -> Option<HashMap<&'static str, u32>> {
    let reg = |k: &str| r.get(k).copied().unwrap_or(0) as u16;
    let [_, armoured, h84, h85, h88] = env;
    let mut out = HashMap::new();
    match name {
        "effect_kind" => drop(out.insert("bx", m.effect_kind(reg("ax") as u8) as u32)),
        "apply_effect" => {
            let (seg, off) = (reg("ds"), reg("si"));
            let res = m.apply_effect(seg, off, reg("ax"), reg("bx"), reg("cx"), reg("dx"), reg("bp"));
            let (cf, si) = match res {
                Applied::Refused | Applied::Slot(None) => (0, off),
                Applied::AlreadyActive => (1, off),
                Applied::Slot(Some(s)) => (0, s),
            };
            out.insert("cf", cf);
            out.insert("si", si as u32);
        }
        "end_effect" => m.end_effect(reg("si")),
        "clear_effects_on" => m.clear_effects_on(reg("ds"), reg("si")),
        "find_effect_slot" => {
            let found = m.find_effect_slot(reg("ebx_seg"), reg("ebx_off"), reg("ax") as u8);
            out.insert("cf", found.is_some() as u32);
            out.insert("si", found.unwrap_or(0xA20) as u32);
        }
        "fix_slot_segments" => m.fix_slot_segments(),
        "absorb_damage" => drop(out.insert("ax", m.absorb_damage(reg("ds"), reg("si"), reg("ax")) as u32)),
        "tick_effects" => m.tick_effects(h),
        "apply_disease" => drop(m.apply_disease(reg("ds"), reg("si"), reg("ax"))?),
        "creature_special_attack" => m.creature_special_attack(reg("ds"), reg("si"), armoured)?,
        // The harness's 7C69 stub leaves SI at 0212, and 021B is stubbed.
        "weapon_enchantments" => drop(out.insert("ax", m.weapon_enchantments(reg("ax"), h85, h88.then_some(0x212)).0 as u32)),
        "vampiric_heal" => m.vampiric_heal(reg("ax"), h84),
        "drain_death_check" => m.drain_death_check(h),
        "rebase_drain_snapshots" => m.rebase_drain_snapshots(reg("ax"), reg("bx")),
        "attr_drained" => drop(out.insert("cf", m.attr_drained(reg("bp")) as u32)),
        "tick_spell_flags" => m.tick_spell_flags(),
        "spell_flag_table" => {
            let (seg, di) = m.spell_flag_table(reg("fs"), reg("dx"));
            out.insert("fs", seg as u32);
            out.insert("di", di as u32);
        }
        "spell_stopped" => {
            let cost = m.mem.r16(reg("gs"), reg("si").wrapping_add(0x32));
            let stopped = m.spell_stopped(reg("fs"), reg("edx"), cost, h)?;
            out.insert("cf", stopped as u32);
        }
        _ => panic!("unknown routine {name}"),
    }
    Some(out)
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut count: HashMap<String, usize> = HashMap::new();
    for line in include_str!("vectors/effects.txt").lines() {
        let f: Vec<&str> = line.split(" | ").map(str::trim).collect();
        let (name, regs, rets, hooks, diff) = (f[0], named(f[2]), f[4], f[5], f.get(6).copied().unwrap_or(""));
        let env: Vec<bool> = f[3].split(':').map(|v| v == "1").collect();
        let env = [env[0], env[1], env[2], env[3], env[4]];

        let mut m = base.clone();
        for w in f[1].split_whitespace() {
            let p: Vec<&str> = w.split(':').collect();
            let seg = match p[0] {
                "d" | "D" => DS,
                "m" | "M" => MSEG,
                "o" => OSEG,
                k => panic!("write kind {k}"),
            };
            let off = hex(p[1]);
            let bytes: Vec<u8> = if p.len() == 4 {
                // A segment value: the emulator relocated it.
                (0..hex(p[2])).map(|i| (hex(p[3]) >> (8 * i)) as u8).collect()
            } else {
                (0..p[2].len()).step_by(2).map(|i| u8::from_str_radix(&p[2][i..i + 2], 16).unwrap()).collect()
            };
            for (i, b) in bytes.into_iter().enumerate() {
                m.mem.w8(seg, (off + i as u32) as u16, b);
            }
        }
        m.load_rng();
        let before = m.mem.clone();
        let mut h = Hooks { kill_cf: env[0], ..Default::default() };
        let got = run(&mut m, name, &regs, env, &mut h);

        if rets.starts_with("fault") {
            assert!(got.is_none(), "{name}: original faults, port returned {got:?}\n{line}");
        } else {
            let got = got.unwrap_or_else(|| panic!("{name}: port faulted, original did not\n{line}"));
            let want = named(rets);
            for (k, v) in &got {
                assert_eq!(*v, want[*k], "{name}: {k}\n{line}");
            }
            m.store_rng();
            let want: Vec<(usize, u8)> = diff
                .split_whitespace()
                .map(|p| p.split_once(':').unwrap())
                .map(|(a, v)| (hex(a) as usize, hex(v) as u8))
                .collect();
            let have: Vec<(usize, u8)> =
                REGIONS.iter().flat_map(|&(lo, hi)| m.mem.diff(&before, lo..hi)).collect();
            if have != want {
                let only = |a: &[(usize, u8)], b: &[(usize, u8)]| -> Vec<String> {
                    a.iter().filter(|x| !b.contains(x)).map(|(l, v)| format!("{l:x}:{v:x}")).collect()
                };
                panic!("{name}: memory differs\nport only: {:?}\noriginal only: {:?}\n{line}",
                       only(&have, &want), only(&want, &have));
            }
        }
        let hk: Vec<&str> = hooks.split(' ').collect();
        let k: Vec<u32> = hk[0].split(':').map(hex).collect();
        assert_eq!(h.kills as u32, k[0], "{name}: kill handler calls\n{line}");
        if k[0] != 0 {
            assert_eq!((h.last.0 as u32, h.last.1 as u32), (k[1], k[2]), "{name}: kill handler target\n{line}");
        }
        assert_eq!(h.deaths as u32, hex(hk[1]), "{name}: player deaths\n{line}");
        let rf: Vec<u32> = hk[2].split(':').map(hex).collect();
        assert_eq!(h.reflections as u32, rf[0], "{name}: reflections\n{line}");
        if rf[0] != 0 {
            assert_eq!(h.reflected as u32, rf[1], "{name}: reflecting actor\n{line}");
        }
        *count.entry(name.to_string()).or_default() += 1;
    }
    eprintln!("{count:?}");
    assert_eq!(count.len(), 18);
}

/// A machine with every effect slot free.
fn free_slots(img: &arena::exe::Image, fixes: arena::fixes::Fixes) -> Machine {
    let mut m = Machine::new(img);
    m.fixes = fixes;
    m.mem.w16(DS, 0x1042, MSEG);
    for i in 0..16 {
        m.mem.w8(DS, 0x0660 + i * 0x3C + 0x3B, 0xFF);
    }
    m
}

/// Fixed (kept bug #23): an effect pulsing on the player works its
/// modifiers out on the 256 scale (carry weight 2 x STR in 256ths); the
/// original runs 1194:361D on the percent attributes.
#[test]
fn fixed_effect_pulse_scales_the_players_attributes() {
    use arena::fixes::{EFFECT_SCALE, Fixes};
    let Some(img) = common::image() else { return };
    let p = 0x01A4;
    for fixes in [Fixes::NONE, Fixes::only(EFFECT_SCALE)] {
        let mut m = free_slots(&img, fixes);
        for i in 0..8 {
            m.mem.w8(DS, p + 0x29 + i, 50);
            m.mem.w8(DS, p + 0x31 + i, 50);
        }
        m.mem.w8(DS, p + 0x38, 0xCC);
        m.mem.w16(DS, p + 0x40D, 0);
        let Applied::Slot(Some(s)) = m.apply_effect(DS, p, 0x10, 1, 5, 1, 0xFFFC) else { panic!("not applied") };
        for o in 6..0x14 {
            m.mem.w8(DS, s + o, 0);
        }
        m.tick_effects(&mut Hooks::default());
        assert_eq!(m.mem.r8(DS, p + 0x29), 50, "{fixes:?}");
        let weight = m.mem.r16(DS, p + 0x3B);
        let want = if fixes == Fixes::NONE { 100 } else { 2 * arena::scale::to256(50) as u16 };
        assert_eq!(weight, want, "{fixes:?}");
    }
}

/// Fixed (kept bug #70): a new effect's deltas start at 0; the original
/// keeps what the slot held.
#[test]
fn fixed_new_effects_start_with_no_deltas() {
    use arena::fixes::{EFFECT_DELTAS, Fixes};
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(EFFECT_DELTAS)] {
        let mut m = free_slots(&img, fixes);
        for o in 0..0x3B {
            m.mem.w8(DS, 0x0660 + o, 0x5A);
        }
        let Applied::Slot(Some(s)) = m.apply_effect(MSEG, 0, 0x10, 1, 5, 1, 0xFFFC) else { panic!("not applied") };
        assert_eq!(s, 0x0660);
        let deltas: Vec<u8> = [0u16, 0x14].iter().flat_map(|&b| (6..0x14).map(move |o| b + o)).map(|o| m.mem.r8(DS, s + o)).collect();
        let want = if fixes == Fixes::NONE { 0x5A } else { 0 };
        assert!(deltas.iter().all(|&d| d == want), "{fixes:?}: {deltas:x?}");
    }
}

/// #86 fixed (`REFLECT_LIST`): two targets of one cast that both reflect
/// it each queue the caster in an entry of their own, after the list's
/// targets, the count raised; the original's writes the same entry twice
/// (the count never raised), so the caster is struck once and 29CF:0941
/// goes on into the entry after it, left from an earlier list.
#[test]
fn fixed_two_reflections_queue_the_caster_twice() {
    use arena::effects::spell_flag::{ACTIVE, ACTIVE_COUNT, ACTIVE_SIZE, REFLECT};
    use arena::fixes::{Fixes, REFLECT_LIST};
    const TSEG: u16 = 0x6000;
    const STALE: u32 = 0x1234_5678;
    let Some(img) = common::image() else { return };
    for fixes in [Fixes::NONE, Fixes::only(REFLECT_LIST)] {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        // Two targets in a segment of no creature's (their spell flags'
        // entries at the record's segment, ACTIVE's end: 2CDD:09E0), each
        // reflecting (flag 800h, its strength 100); the caster 1 level up.
        let table = ACTIVE + ACTIVE_COUNT * ACTIVE_SIZE;
        m.mem.w16(TSEG, table + 0x2C + 2, 100);
        for off in [0u16, 0x41E] {
            m.mem.w16(TSEG, off + 0x41A, REFLECT);
        }
        m.mem.w16(DS, 0x1292, 1);
        // The cast's projectile (A886) from the player; the list of two,
        // the entry after its third left from before.
        m.mem.w16(DS, 0xA886, 0xFF00);
        m.mem.w32(DS, 0xFF04, (DS as u32) << 16 | arena::character::PLAYER as u32);
        m.mem.w16(DS, 0x0D5A, 2);
        m.mem.w32(DS, 0x0D5C + 3 * 4, STALE);
        for off in [0u16, 0x41E] {
            assert_eq!(m.spell_stopped(TSEG, off, 0, &mut Hooks::default()), Some(true), "{fixes:?}: reflected");
        }
        let caster = (DS as u32) << 16 | arena::character::PLAYER as u32;
        let list = (m.mem.r16(DS, 0x0D5A), m.mem.r32(DS, 0x0D5C + 2 * 4), m.mem.r32(DS, 0x0D5C + 3 * 4), m.mem.r16(DS, 0x1276));
        if fixes == Fixes::NONE {
            assert_eq!(list, (2, caster, STALE, 2), "the original's: one entry for two, the stale one next");
        } else {
            assert_eq!(list, (4, caster, caster, 2), "fixed: an entry each");
        }
    }
}
