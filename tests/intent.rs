//! The modern controls (src/controls.rs `player_steer`) move the player
//! exactly as the original's arrow keys do for the same speed fractions
//! (the keys put the cursor at a region's edge: forward and strafing left
//! are full speed, 256; back 238, strafing right 250 and turning 246).
mod common;

use arena::bsa::Files;
use arena::controls::{key, Intent, NoControlHooks};
use arena::machine::Machine;
use arena::mem::DS;
use arena::movement::var::{HEADING, X, Y};

fn base() -> Option<Machine> {
    let img = common::image()?;
    let mut m = common::controls::base(&img);
    m.mem.w16(DS, 0xC3D0, 0x1040);
    m.mem.w16(DS, 0xC3D4, 0x1040);
    m.mem.w16(DS, 0xC3D2, 0x3C);
    m.mem.w16(DS, 0xC3DA, 0x37);
    m.mem.w16(DS, 0xCB90, (0x1040 >> 7 << 8) + (0x1040 >> 7 << 1));
    // The cursor rests on the view's centre, as the modern controls take
    // it (they set 5E0 bit 40h as its handler does): no region steers, and
    // with no click the handler leaves the speed 0 (2826:015C).
    m.mem.w16(DS, 0xA91A, 0x93);
    m.set_mouse(0xA0, 0x48, 0);
    Some(m)
}

fn state(m: &Machine) -> [u16; 5] {
    [X, Y, HEADING, 0x7A46, 0xCCBA].map(|at| m.mem.r16(DS, at))
}

fn same(keys: &[u8], intent: Intent) {
    let Some(start) = base() else { return };
    let mut files = Files::default();
    let (mut a, mut b) = (start.clone(), start);
    for &k in keys {
        a.set_key(k, true);
    }
    for frame in 0..6 {
        a.player_input(&mut files, &mut NoControlHooks).unwrap();
        b.player_steer(&mut files, &mut NoControlHooks, &intent).unwrap();
        assert_eq!(state(&a), state(&b), "{keys:?} frame {frame}");
    }
    assert_ne!(state(&a)[..3], [0x1040, 0x1040, 0x37], "{keys:?} moved nothing");
}

#[test]
fn forward() {
    same(&[key::UP], Intent { forward: 256, ..Default::default() });
}

#[test]
fn back() {
    same(&[key::DOWN], Intent { forward: -238, ..Default::default() });
}

#[test]
fn strafe() {
    same(&[key::CTRL, key::LEFT], Intent { strafe: -256, ..Default::default() });
    same(&[key::CTRL, key::RIGHT], Intent { strafe: 250, ..Default::default() });
}

#[test]
fn turn() {
    same(&[key::RIGHT], Intent { turn: 246, ..Default::default() });
}
