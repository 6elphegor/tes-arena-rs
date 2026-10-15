//! Differential tests of the game's text (src/text.rs `text`, 1194:A374,
//! and its substitutions 2DA2:000A) against vectors from the original code
//! (harness/gen_text.py). Needs the game directory (POINTER1.DAT,
//! TEMPLATE.DAT, NAMECHNK.DAT). Cases whose text reaches a substitution
//! not ported are counted, not compared (and their number checked).
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::text::TextHooks;
use common::blow::Stream;
use std::path::PathBuf;

const FRAME: u16 = 0x8800;
const OUT_SEG: u16 = 0x7000;
const CSEG: u16 = 0x4C00;
const HEAP: (u16, u16) = (0xA000, 0xF000);
const CHARS: &[u8] = b" abcdefghijklmnopqrstuvwxyz";

fn game_dir() -> PathBuf {
    std::env::var("ARENA_EXE")
        .map(|p| PathBuf::from(p).parent().unwrap().to_path_buf())
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

/// The stubs' counters: the message box at FFE0, the menu at FFE2.
struct Count;

impl TextHooks for Count {
    fn message_box(&mut self, m: &mut Machine, _si: u16) {
        let v = m.mem.r16(DS, 0xFFE0).wrapping_add(1);
        m.mem.w16(DS, 0xFFE0, v);
    }
    fn menu(&mut self, m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        let v = m.mem.r16(DS, 0xFFE2).wrapping_add(1);
        m.mem.w16(DS, 0xFFE2, v);
        0
    }
}

/// Mirrors harness/gen_text.py `setup`: writes the case's state, returns
/// the mode.
fn setup(m: &mut Machine, seed: u32, keys: &[u16]) -> u16 {
    let mut st = Stream(seed);
    let s = &mut st;
    let mode = (s.byte() % 3) as u16;
    for line in include_str!("vectors/text_spec.txt").lines() {
        let line = line.split('#').next().unwrap();
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        let seg = match f[0] {
            "ds" => DS,
            "data" => DATA,
            "c" => CSEG,
            x => panic!("{x}"),
        };
        let at = hex(f[1]) as u16;
        let list = |i: usize| f[i].split(',').map(hex).collect::<Vec<u32>>();
        let v: Vec<u8> = match f[2] {
            "b" => vec![(s.byte() as u32 % hex(f[3])) as u8],
            "w" => ((s.word() as u32 % hex(f[3])) as u16).to_le_bytes().to_vec(),
            "d" => (0..4).map(|_| s.byte()).collect(),
            "bc" => {
                let c = list(3);
                vec![c[s.byte() as usize % c.len()] as u8]
            }
            "wc" => {
                let c = list(3);
                (c[s.byte() as usize % c.len()] as u16).to_le_bytes().to_vec()
            }
            "w2" => {
                let lo = (s.byte() as u32 % hex(f[3])) as u8;
                let hi = (s.byte() as u32 % hex(f[4])) as u8;
                vec![lo, hi]
            }
            "str" => {
                let mut v: Vec<u8> = (0..hex(f[3])).map(|_| CHARS[s.byte() as usize % 27]).collect();
                v.push(0);
                v
            }
            "bytes" => (0..hex(f[3])).map(|_| s.byte()).collect(),
            "seg" => CSEG.to_le_bytes().to_vec(),
            "text" => {
                let t = if s.byte() < 0xF0 {
                    let k = keys[s.word() as usize % keys.len()];
                    if k & 0x8000 != 0 { k & 0xFF } else { k }
                } else {
                    s.word() % 0x700
                };
                t.to_le_bytes().to_vec()
            }
            "aim" => {
                if s.byte() < hex(f[3]) as u8 {
                    for item in f[4].split(',') {
                        let (addr, val) = item.split_once(':').unwrap();
                        let (addr, n) = (hex(addr) as u16, hex(&val[1..]));
                        if val.starts_with('b') {
                            m.mem.w8(seg, addr, n as u8);
                        } else {
                            m.mem.w16(seg, addr, n as u16);
                        }
                    }
                }
                continue;
            }
            x => panic!("{x}"),
        };
        for (i, b) in v.into_iter().enumerate() {
            m.mem.w8(seg, at.wrapping_add(i as u16), b);
        }
    }
    mode
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else {
        eprintln!("skipping: {} not found", dir.display());
        return;
    };
    let p = std::fs::read(dir.join("POINTER1.DAT")).expect("POINTER1.DAT");
    let keys: Vec<u16> = p[..0x2710].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&k| k != 0).collect();
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(base.bsa_open(&mut files));
    base.mem.w16(DS, 0x9030, FRAME);
    base.mem.w8(DS, 0x8FA6, 1);
    base.mem.w16(DS, 0x9032, 0x20);
    base.mem.w16(DS, 0x8FA8, 0);
    let (mut n, mut skipped, mut stuck) = (0, 0, 0);
    for line in include_str!("vectors/text.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "text", "{line}");
        let mut m = base.clone();
        let mut fs = files.clone();
        let mode = setup(&mut m, hex(f[1]), &keys);
        m.mem.w16(DS, 0x91F0, 0);
        m.load_rng();
        let before = m.clone();
        let got = m.text(&mut fs, &mut Count, mode, OUT_SEG, 0).map(|_| {
            m.store_rng();
            // TEXT_DUMP=seed TEXT_DUMP_TO=path: that case's changed DGROUP
            // and 3276 bytes written out, to set beside gen_text.py's DUMP.
            if let (Ok(d), Ok(to)) = (std::env::var("TEXT_DUMP"), std::env::var("TEXT_DUMP_TO")) {
                if hex(f[1]) == hex(&d) {
                    for (seg, ext) in [(DS, "ds"), (DATA, "data")] {
                        let s: String = (0..=0xFFFFu16)
                            .filter(|&o| m.mem.r8(seg, o) != before.mem.r8(seg, o))
                            .map(|o| format!("{o:04x} {:02x} {:02x}\n", before.mem.r8(seg, o), m.mem.r8(seg, o)))
                            .collect();
                        std::fs::write(format!("{to}.{ext}"), s).unwrap();
                    }
                }
            }
            let changed = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DS, o) != before.mem.r8(DS, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DS, o)]);
            let dchanged = (0..=0xFFFFu16)
                .filter(|&o| m.mem.r8(DATA, o) != before.mem.r8(DATA, o))
                .flat_map(|o| [o as u8, (o >> 8) as u8, m.mem.r8(DATA, o)]);
            let text = (0..0x200u16).map(|o| m.mem.r8(OUT_SEG, o));
            let frame = (0..=0xFFFFu16).map(|o| m.mem.r8(FRAME, o));
            let places = (0x8A52..0x8A56u16).map(|o| m.mem.r8(CSEG, o));
            format!("{:x} {:x} {:x} {:x} {:x}", fnv(changed), fnv(dchanged), fnv(text), fnv(frame), fnv(places))
        });
        match got {
            Some(g) => {
                assert_eq!(f[2], g, "{line}");
                n += 1;
            }
            None if f[2] == "stuck" => stuck += 1,
            None => skipped += 1,
        }
    }
    eprintln!("text: {n} compared, {skipped} not ported, {stuck} stuck in both");
    assert!(n > 3500, "{n} compared");
    // A None where the original goes on is only right for the code not
    // ported: these counts are the vectors' (a change that makes the port
    // give up more often shows here).
    assert_eq!((skipped, stuck), (2, 480), "not ported, stuck in both");
}

/// A substitution the port gives up on (%qf, which seeds a name with EBX
/// the original never sets) must leave the nesting and the mapped bank as
/// 2DA2's exit does: playtest round 2 saw the level left raised, so the
/// next text skipped its substitutions and mapped a stale bank, and a
/// quest giver's greeting (56h) ran on into DGROUP from 1044.
#[test]
fn a_substitution_given_up_leaves_the_nesting() {
    use arena::text::var::{BANK, SUB_LEVEL};
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else { return };
    let mut m = Machine::new(&img);
    m.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(m.bsa_open(&mut files));
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    m.mem.w16(DS, BANK, 0x1F);
    m.ems_map(0x11);
    m.mem.w8(DATA, SUB_LEVEL, 0);
    for (i, &b) in b"Ask %qf here\0".iter().enumerate() {
        m.mem.w8(OUT_SEG, i as u16, b);
    }
    let seed = m.rng.seed;
    assert_eq!(m.expand(&mut files, &mut Count, OUT_SEG, 0), None);
    assert_eq!(m.mem.r8(DATA, SUB_LEVEL), 0, "the nesting back");
    assert_eq!(m.mem.r16(DS, 0x8FA8), 0x11 << 2, "the bank mapped before back");
    assert_eq!(m.rng.seed, seed, "the seed back");
    // The next text is substituted from the outer level again.
    for (i, &b) in b"plain\0".iter().enumerate() {
        m.mem.w8(OUT_SEG, i as u16, b);
    }
    assert!(m.expand(&mut files, &mut Count, OUT_SEG, 0).is_some());
    assert_eq!(m.mem.r8(DATA, SUB_LEVEL), 0);
    assert_eq!(m.mem.r16(DS, 0x8FA8), 0x11 << 2);
}

/// %oth (2DA2:0B2B) sets NO_EXPAND around its own text; where that text
/// gives up (here a key missing from the index), the flag is put back as
/// the code's end puts it, so later texts still have their substitutions.
#[test]
fn an_oath_given_up_leaves_no_expand() {
    use arena::text::var::{BANK, NO_EXPAND, SUB_LEVEL};
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else { return };
    let mut m = Machine::new(&img);
    m.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(m.bsa_open(&mut files));
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    m.mem.w16(DS, BANK, 0x1F);
    m.ems_map(0x11);
    m.mem.w8(DATA, SUB_LEVEL, 0);
    m.mem.w8(DS, NO_EXPAND, 0);
    // The province's oath: a text key the index doesn't have.
    m.mem.w8(DS, 0x0A46, 0);
    m.mem.w16(DS, 0x8A60, 0x7FFF);
    for (i, &b) in b"By %oth\0".iter().enumerate() {
        m.mem.w8(OUT_SEG, i as u16, b);
    }
    assert_eq!(m.expand(&mut files, &mut Count, OUT_SEG, 0), None);
    assert_eq!(m.mem.r8(DS, NO_EXPAND), 0, "substitutions not left off");
    assert_eq!(m.mem.r8(DATA, SUB_LEVEL), 0, "the nesting back");
}

/// TEMPLATE.DAT's last text (5DDh, the palace's artifact audience at
/// 0000:4248) has no `#` after it: its byte-a-call read (1194:A44A) goes
/// past the file's end, where the reads leave the text bank as it was,
/// and stops at the first 0 or `#` there. In the original that bank
/// ([1030], the loads' bank) then holds what the town's welcome box
/// covered (0000:4B18..4B21, `box_background`), not what the level's
/// load unpacked through it: after a long walk the port read on into a
/// plane's compressed bytes (a `%|$`, a substitution with no code: None,
/// as the original's 1194:A36C fails from that state too).
#[test]
fn the_last_template_reads_on_into_the_welcome_box() {
    use arena::text::var::BANK;
    let Some(img) = common::image() else { return };
    let dir = game_dir();
    let Ok(mut files) = Files::new(&dir) else { return };
    let mut m = Machine::new(&img);
    m.heap_init(HEAP.0, HEAP.1).unwrap();
    assert!(m.bsa_open(&mut files));
    m.mem.w16(DS, 0x9030, FRAME);
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    m.mem.w16(DS, BANK, 0x1F);
    m.ems_map(0x1F);
    let run = |m: &mut Machine, files: &mut Files| {
        // The walk's seed is long gone; this one picks the second variant.
        m.rng.seed = 0x9E37_79B9;
        m.mem.w16(DS, 0x137A, 0x5DD);
        m.mem.w16(DS, 0x101C, 0);
        m.mem.w16(DS, 0x137E, 0);
        m.text(files, &mut Count, 2, OUT_SEG, 0).map(|_| String::from_utf8_lossy(m.mem.cstr(OUT_SEG, 0)).into_owned())
    };
    // The file ends "complete...&" CR LF CR LF ";last template _ (1501)"
    // CR LF: two variants (the `&`), the second running on into the bank.
    // The bank as a level's load leaves it: compressed bytes (the walk's
    // were IMPERIAL.MIF's) with another `&` and a `%` before a 0: a third
    // variant, and a `%` in the second that the substitutions can't read.
    for i in 0..0x400u16 {
        m.mem.w8(FRAME, i, 0x8E);
    }
    for (i, &b) in b"\x8E%|$&\x8E\x88\x00".iter().enumerate() {
        m.mem.w8(FRAME, 0x151 + i as u16, b);
    }
    assert_eq!(run(&mut m, &mut files), None, "read on into the plane's bytes");
    // The welcome's box keeps the dark view under it there.
    m.mem.w8(FRAME, 8 + 0xD5 * 0x45, 0x8E);
    m.box_background(FRAME, 0, (0x2B, 0x47, 0xD5, 0x45), None);
    assert_eq!(m.mem.r16(FRAME, 0), 0x802B);
    assert_eq!((m.mem.r16(FRAME, 2), m.mem.r16(FRAME, 4), m.mem.r16(FRAME, 6)), (0x47, 0xD5, 0x45));
    assert_eq!(m.mem.r8(FRAME, 8 + 0xD5 * 0x45), 0x8E, "the box's rows only");
    assert_eq!(m.mem.r8(FRAME, 0x151), 0);
    let t = run(&mut m, &mut files).expect("the text");
    assert!(t.starts_with("As you approach the Palace Gates"), "{t:?}");
    assert!(t.trim_end().ends_with("complete..."), "{t:?}");
}
