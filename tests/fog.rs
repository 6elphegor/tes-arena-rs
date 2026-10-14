//! Differential tests of weather and fog (src/weather.rs: `weather_set`
//! 0000:0F05, `fog_step` 2294:0B01, `fog_start` 2294:0BEF, with the fog's
//! table 0C36 and the normal table's reset 0B8A) against vectors from the
//! original code (harness/gen_fog.py).
mod common;

use arena::light::LightHooks;
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::Stream;

const TSEG: u16 = 0x6000;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn count(m: &mut Machine, at: u16) {
    let v = m.mem.r16(DS, at).wrapping_add(1);
    m.mem.w16(DS, at, v);
}

/// The generator's stubs: calls counted at FFE0..FFEE; the table's maker
/// puts it at TSEG.
struct Hooks;

impl Hooks {
    fn table(m: &mut Machine) {
        count(m, 0xFFE0);
        count(m, 0xFFEA);
        m.mem.w16(DS, 0xCCC2, TSEG);
        count(m, 0xFFE4);
    }
}

impl LightHooks for Hooks {
    fn light_table(&mut self, m: &mut Machine) {
        Self::table(m);
    }
    fn fog_light_table(&mut self, m: &mut Machine) {
        Self::table(m);
    }
    fn lgt(&mut self, m: &mut Machine, _name: u16) {
        count(m, 0xFFEA);
    }
    fn palette(&mut self, m: &mut Machine, _index: u8, _at: u16) {
        count(m, 0xFFE6);
    }
    fn fog_fill(&mut self, m: &mut Machine) {
        count(m, 0xFFE8);
    }
    fn weather_palette(&mut self, m: &mut Machine) {
        count(m, 0xFFEC);
        count(m, 0xFFEE);
    }
}

/// Mirrors harness/gen_fog.py `setup` (a tuple's items are all drawn
/// before its index; a conditional's test first).
fn setup(m: &mut Machine, seed: u32) {
    let mut st = Stream(seed);
    let s = &mut st;
    let v = s.word() & 0xFFFB;
    let v = v | if s.byte() < 0xE0 { 4 } else { 0 };
    m.mem.w16(DS, 0xC398, v);
    let v = s.word();
    m.mem.w16(DS, 0xC39A, v);
    let v = s.word();
    m.mem.w16(DS, 0x05C8, v);
    for at in [0x0F81, 0x0F82, 0xCB5C] {
        let v = if s.byte() < 0x80 { 0 } else { s.byte() };
        m.mem.w8(DS, at, v);
    }
    let k = (s.byte() % 5) as usize;
    let t = [
        s.byte() as u16,
        0x300 + s.byte() as u16,
        0x100 + s.word() % 0x201,
        s.word() & 0x3FF,
        [0xFF, 0x100, 0x300, 0x301][(s.byte() % 4) as usize],
    ][k];
    m.mem.w16(DS, 0xCCFE, t);
    for at in [0xC3C6, 0xC3C7] {
        let v = s.byte();
        m.mem.w8(DS, at, v);
    }
    for at in [0xC8E7, 0xC8E9, 0xC8EB, 0xC3E2, 0xC3DC] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    let v = s.byte() % 8;
    m.mem.w8(DS, 0x0A50, v);
    let v = s.byte() % 36;
    m.mem.w8(DS, 0x0A51, v);
    for z in 0..36 {
        let v = s.byte() % 10;
        m.mem.w8(DS, 0x0A5A + z, v);
    }
    let v = s.word();
    m.mem.w16(DS, 0x0A4E, v);
    let v = s.word() & 0x1F;
    m.mem.w16(DS, 0xCCA8, v);
    for at in [0x91F6, 0x91F8, 0x837A] {
        let v = s.word();
        m.mem.w16(DS, at, v);
    }
    // The roll's edges, aimed (see gen_fog.py): the seed through the
    // LCG's inverse.
    let k = s.byte() % 4;
    if k == 1 || k == 2 {
        m.mem.w8(DS, 0x0A51, 0);
        m.mem.w8(DS, 0x0A5A, if k == 1 { 7 } else { 5 });
        let r = if k == 1 { 0x3E7Fu32 } else { 0x5DBF } + (s.byte() % 3) as u32;
        let low = s.word() as u32;
        let mut seed = (r << 16 | low).wrapping_mul(INV);
        if k == 2 {
            seed = seed.wrapping_mul(INV);
        }
        m.mem.w32(DS, 0x91F6, seed);
    }
}

/// The inverse of the LCG's multiplier 6D002Dh mod 2^32.
const INV: u32 = 0x3F15_4FA5;

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let base = Machine::new(&img);
    let mut n = 0;
    for line in include_str!("vectors/fog.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "fog", "{line}");
        let mut m = base.clone();
        setup(&mut m, u32::from_str_radix(f[2], 16).unwrap());
        m.load_rng();
        let before = m.clone();
        let cf = match f[1] {
            "0" => {
                m.weather_set(&mut Hooks);
                String::new()
            }
            "1" => format!(" {}", m.fog_step(&mut Hooks) as u8),
            _ => {
                m.fog_start(&mut Hooks);
                String::new()
            }
        };
        m.store_rng();
        let changed = (0..=0xFFFFu16)
            .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
            .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
        let rows = (0xD00..0xF00u16).map(|o| m.mem.r8(TSEG, o));
        assert_eq!(f[3], format!("{:x}{cf}", fnv(changed.chain(rows))), "{line}");
        n += 1;
    }
    assert!(n > 4000);
}
