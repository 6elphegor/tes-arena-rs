//! Needs the original game: set ARENA_EXE, or keep ESarena/ next to this crate.
use arena::mem::{DS, Memory};
use arena::exe::{self, DGROUP_SEG};
use arena::random::{self, ArenaRandom};

mod common;

fn load() -> Option<exe::Image> {
    common::image()
}

#[test]
fn unpacks_acd_exe() {
    let Some(img) = load() else { return };
    assert_eq!(img.relocs.len(), 4044);
    assert_eq!((img.ss, img.sp, img.cs, img.ip), (0x4A8F, 0x0080, 0, 0));
    // Every relocated word must name a segment inside the image.
    for &(seg, off) in &img.relocs {
        let at = seg as usize * 16 + off as usize;
        let v = u16::from_le_bytes([img.bytes[at], img.bytes[at + 1]]);
        assert!((v as usize) * 16 < img.bytes.len(), "{seg:04X}:{off:04X} -> {v:04X}");
    }
    assert!(img.relocs.iter().any(|&(s, o)| {
        let at = s as usize * 16 + o as usize;
        u16::from_le_bytes([img.bytes[at], img.bytes[at + 1]]) == DGROUP_SEG
    }));
}

#[test]
fn memory_and_rng_seed() {
    let Some(img) = load() else { return };
    let mut mem = Memory::new(&img);
    assert_eq!(mem.cstr(DS, 0), b"BethesdaSoftworkRun-TimeLibrary Ver  2.0");
    let mut r = ArenaRandom::from_mem(&mem);
    let first = r.next();
    r.store(&mut mem);
    assert_eq!(mem.r32(DS, random::SEED), r.seed);
    assert_eq!(ArenaRandom::from_mem(&mem).seed, r.seed);
    eprintln!("initial seed {:08X}, first value {first:04X}", ArenaRandom::from_mem(&Memory::new(&img)).seed);
}
