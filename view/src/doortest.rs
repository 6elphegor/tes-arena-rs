//! The doors drawn as the original's renderer draws them ajar
//! (`world::doors`): a swinging door's face turned by
//! `doordraw::swing_end`, a sliding, rising or parting door's face drawn
//! from its texture moved over by `doordraw::door_texture` (both checked
//! against the original by tests/doordraw.rs). Without the game's files
//! the tests say so and pass.

use crate::*;
use arena::doordraw::{door_texture, swing_end};
use arena::mem::DS;

fn engine() -> Option<Engine> {
    let dir = match std::env::var("ARENA_EXE") {
        Ok(exe) => PathBuf::from(exe).parent()?.to_path_buf(),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ESarena"),
    };
    if !dir.join("GLOBAL.BSA").exists() {
        eprintln!("skipping: no game files at {}", dir.display());
        return None;
    }
    Some(world::load(&dir, "CITY:8:0"))
}

/// A square with open squares all round it (none on the map's edge).
fn open_square(m: &arena::machine::Machine) -> (u16, u16) {
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let (w, d) = (m.mem.r16(DS, 0xC357).min(128), m.mem.r16(DS, 0xC359).min(128));
    let at = |c: u16, r: u16| m.mem.r16(walls, r << 8 | c << 1);
    for r in 1..d - 1 {
        for c in 1..w - 1 {
            if [(c - 1, r), (c + 1, r), (c, r - 1), (c, r + 1)].iter().all(|&(c, r)| at(c, r) == 0) {
                return (c, r);
            }
        }
    }
    panic!("no open square");
}

/// A swinging door 5 steps open: four faces, each the open square's
/// beside it, turned out by 96 (`swing_end`), the texture whole.
#[test]
fn a_swinging_door_turns_its_faces() {
    let Some(mut e) = engine() else { return };
    let m = &mut e.m;
    let (c, r) = open_square(m);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    m.mem.w16(walls, r << 8 | c << 1, 0xB000 | 5 << 9 | 3);
    let v = world::doors(m);
    assert_eq!(v.len(), 4 * 6);
    // The west neighbour's east face: hinge at the door's corner (c, r).
    let hinge = (c << 7, r << 7);
    let far = swing_end(m, hinge, (c << 7, (r << 7) + 0x80), 5);
    let q = &v[0..6];
    assert_eq!(q[0].pos, [hinge.0 as f32, 0.0, hinge.1 as f32]);
    assert_eq!(q[1].pos, [far.0 as i16 as f32, 0.0, far.1 as i16 as f32]);
    assert!(far.0 < hinge.0, "out into the west square");
    assert_eq!(q[0].layer, 2);
    assert_eq!(q[0].flags, world::SEE_THROUGH);
}

/// A sliding door (kind 1) 3 steps open: its faces in place, drawn from
/// the layer holding its texture moved over as 0000:BAF3 moves it.
#[test]
fn a_sliding_door_draws_its_texture_moved() {
    let Some(mut e) = engine() else { return };
    let m = &mut e.m;
    let (c, r) = open_square(m);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    m.mem.w16(walls, r << 8 | c << 1, 0xB000 | 3 << 9 | 0x40 | 3);
    let textures = world::textures(m);
    let base = world::texture_count(m) as usize;
    let v = world::doors(m);
    assert_eq!(v.len(), 4 * 6);
    assert_eq!(v[0].pos, [(c << 7) as f32, 0.0, (r << 7) as f32]);
    assert_eq!(v[1].pos, [(c << 7) as f32, 0.0, ((r << 7) + 0x80) as f32]);
    let layer = v[0].layer as usize;
    assert!(layer >= base && layer < textures.len(), "a door layer");
    let t = textures[2];
    let mut cols = [0u8; 4096];
    for u in 0..64 {
        for w in 0..64 {
            cols[u * 64 + w] = t[w * 64 + u];
        }
    }
    let moved = door_texture(&cols, 0, 1, 3);
    for u in 0..64 {
        for w in 0..64 {
            assert_eq!(textures[layer][w * 64 + u], moved[u * 64 + w], "texel {u},{w}");
        }
    }
    // Slid over by 3 * 8 columns: the last 24 clear.
    assert!((40..64).all(|u| (0..64).all(|w| textures[layer][w * 64 + u] == 0)));
    // Shut, the texture whole.
    m.mem.w16(walls, r << 8 | c << 1, 0xB000 | 0x40 | 3);
    assert_eq!(world::doors(m)[0].layer, 2);
}
