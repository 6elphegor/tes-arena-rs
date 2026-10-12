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
        "weapon_enchantments" => drop(out.insert("ax", m.weapon_enchantments(reg("ax"), h85, h88) as u32)),
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
