//! The actors: people and creatures in the level, and their update each
//! frame (0000:1854, called from the main loop).
//!
//! There are 23 actor slots of 0x1C bytes at DGROUP 0AB3: 15 for people,
//! then 8 for creatures (from 0C57). A slot holds:
//! - +0 x, +2 a height offset, +4 y (map units, 128 a square);
//! - +8 speed, +A heading (0..511);
//! - +C the sprite (0xC0 plus a view direction 0..4; 0xC5 dying; the
//!   corpse flat, CORPSE + FLAT_BASE, dead), +D the animation frame or
//!   state, +E a frame limit;
//! - +F flags: 1 busy (facing the player, attacking), 2 sprite mirrored,
//!   4 hostile (moves on its own), 0x10 attacking, 0x40/0x80 a creature's
//!   idle animation phase, 0x800 sees the player, 0x4000 inactive;
//! - +12 the offset of its character record in the creature segment
//!   (DGROUP 1042): flags at +40D (0x40 frozen, 0x100 dying, 0x1000
//!   creature), +41A status flags, +2D speed, +4 type, +59 health, +66
//!   spell points, +366 spells known.
//!
//! Each frame the update records every active actor's square in a table
//! (DGROUP 1951), which keeps actors out of each other's squares; then
//! for each actor it decides what to do (`actor_think`), deactivates it
//! when far from the player (respawning hostile ones nearby when map flag
//! 0x8000 is set), advances its animation (`actor_animate`) and picks its
//! sprite. Creatures move through the player's collision code with
//! creature mode on (see `crate::movement`), and so open doors without
//! locks by walking into them.
//!
//! A creature or person whose attack animation reaches its blow hits at
//! the player (`crate::attack`).
//!
//! Creatures and people who see the player cast now and then
//! (`crate::spell::actor_cast`).
//!
//! Not ported: building the frame's draw list (0000:1B7F, FE58) and
//! sound.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::mif::var::{FLOOR_SEG, MAP1_SEG};
use crate::travel::travel_heading;

/// DGROUP variables.
pub mod var {
    /// The actor slots, the first creature slot, and the count.
    pub const ACTORS: u16 = 0x0AB3;
    pub const CREATURES: u16 = 0x0C57;
    pub const COUNT: u16 = 0x17;
    pub const SIZE: u16 = 0x1C;
    /// Each actor's square (word, 0xFFFF none), and the index found by
    /// the last occupancy check (0xFFFF none).
    pub const SQUARES: u16 = 0x1951;
    pub const OCCUPANT: u16 = 0x7A44;
    /// The creature segment (word).
    pub const CREATURE_SEG: u16 = 0x1042;
    /// Counts of active people, active creatures, and hostile ones near
    /// enough to see; the last such one's square.
    pub const PEOPLE_ACTIVE: u16 = 0x0FF7;
    pub const CREATURES_ACTIVE: u16 = 0x0FF9;
    pub const NEAR: u16 = 0x0FFB;
    pub const NEAR_SQUARE: u16 = 0x6206;
    /// Set when a hostile actor is close to the player this frame.
    pub const CLOSE: u16 = 0x101B;
    /// The corpse flat (byte; add FLAT_BASE).
    pub const CORPSE: u16 = 0xC936;
    /// The draw list's next free entry, and the copied-actor list's end
    /// (both reset each frame; filled by code not ported).
    pub const DRAW_NEXT: u16 = 0x651C;
    pub const COPIES_END: u16 = 0x627A;
    /// Steps by heading quarter (x, y words), and square offsets by
    /// heading quarter.
    pub const STEPS: u16 = 0x61F6;
    pub const AHEAD: u16 = 0x61EE;
    /// The nine squares around one (map offsets).
    pub const AROUND: u16 = 0x0DAA;
    /// Spell effects: 16 records of 0x3C bytes; +28 the far pointer of
    /// the record they act on.
    pub const EFFECTS: u16 = 0x0660;
    /// Creatures to mark when they go (8 x 6 bytes: slot, trigger
    /// index, ...), and the trigger records (2 bytes each).
    pub const MARKS: u16 = 0x0622;
    pub const TRIGGER_SQUARES: u16 = 0xC766;
    /// The two side steps a blocked chase tries (x, y each; 0x6000 none).
    pub const SIDESTEPS: u16 = 0xA836;
}
use var::*;

/// Sine and cosine tables (words by heading).
const SIN: u16 = 0xB03C;
const COS: u16 = 0xB13C;
/// Player position, height and square.
const PX: u16 = 0xC3D0;
const PH: u16 = 0xC3D2;
const PY: u16 = 0xC3D4;
const PSQUARE: u16 = 0xCB90;
const MAP_FLAGS: u16 = 0xC398;
const LEVEL_FLAGS: u16 = 0xC39A;
const FLAT_BASE: u16 = 0xC38E;
const FLOOR_SOLID: u16 = 0xCCCE;
const MAP_WIDTH: u16 = 0xC357;
const MAP_DEPTH: u16 = 0xC359;
/// Creature mode for the collision code.
const CREATURE_MODE: u16 = 0xE3A1;

/// Respawn attempts taken to hang (the original tries for ever).
const RESPAWN_LIMIT: u32 = 1 << 16;

/// An actor slot's fields.
mod f {
    pub const X: u16 = 0;
    pub const Z: u16 = 2;
    pub const Y: u16 = 4;
    pub const SPEED: u16 = 8;
    pub const HEADING: u16 = 0xA;
    pub const SPRITE: u16 = 0xC;
    pub const FRAME: u16 = 0xD;
    pub const LIMIT: u16 = 0xE;
    pub const FLAGS: u16 = 0xF;
    pub const RECORD: u16 = 0x12;
    pub const SOUNDED: u16 = 0x18;
}

/// The approximate length of (`a`, `b`): the larger plus a quarter of
/// the smaller (0000:122A, without the subtraction it is sometimes used
/// for).
pub(crate) fn approx(a: u16, b: u16) -> u16 {
    let (a, b) = (if (a as i16) < 0 { a.wrapping_neg() } else { a }, if (b as i16) < 0 { b.wrapping_neg() } else { b });
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    (lo >> 2).wrapping_add(hi)
}

/// The map offset of the square holding (x, y).
fn square(x: u16, y: u16) -> u16 {
    ((y >> 7) << 8).wrapping_add((x >> 7) << 1)
}

impl Machine {
    fn act16(&self, si: u16, field: u16) -> u16 {
        self.mem.r16(DS, si.wrapping_add(field))
    }
    fn act8(&self, si: u16, field: u16) -> u8 {
        self.mem.r8(DS, si.wrapping_add(field))
    }
    fn set_act16(&mut self, si: u16, field: u16, v: u16) {
        self.mem.w16(DS, si.wrapping_add(field), v)
    }
    fn set_act8(&mut self, si: u16, field: u16, v: u8) {
        self.mem.w8(DS, si.wrapping_add(field), v)
    }
    fn act_flags(&self, si: u16) -> u16 {
        self.act16(si, f::FLAGS)
    }
    fn set_act_flags(&mut self, si: u16, v: u16) {
        self.set_act16(si, f::FLAGS, v)
    }
    /// The actor's character record flags (+40D) and other fields.
    fn act_rec16(&self, si: u16, field: u16) -> u16 {
        let seg = self.mem.r16(DS, CREATURE_SEG);
        self.mem.r16(seg, self.act16(si, f::RECORD).wrapping_add(field))
    }
    fn act_rec8(&self, si: u16, field: u16) -> u8 {
        let seg = self.mem.r16(DS, CREATURE_SEG);
        self.mem.r8(seg, self.act16(si, f::RECORD).wrapping_add(field))
    }
    fn actor_is_creature(&self, si: u16) -> bool {
        self.act_rec16(si, 0x40D) & 0x1000 != 0
    }
    fn corpse_sprite(&self) -> u8 {
        (self.mem.r8(DS, CORPSE) as u16).wrapping_add(self.mem.r16(DS, FLAT_BASE)) as u8
    }
    /// The heading from the actor to the player (0000:E06A).
    fn toward_player(&self, si: u16) -> u16 {
        travel_heading(self.act16(si, f::X), self.act16(si, f::Y), self.mem.r16(DS, PX), self.mem.r16(DS, PY))
    }
    /// The approximate distance from the actor to the player.
    fn player_distance(&self, si: u16) -> u16 {
        approx(
            self.mem.r16(DS, PX).wrapping_sub(self.act16(si, f::X)),
            self.mem.r16(DS, PY).wrapping_sub(self.act16(si, f::Y)),
        )
    }

    /// 0000:1854 (far). One frame of the actors. A countdown at 653E
    /// (which keeps creatures peaceful while it runs) goes down first.
    /// None where an actor reaches an attack or a spell, a respawn finds
    /// no square, or the collision code reaches something not ported.
    pub fn actors_update(&mut self, files: &mut Files) -> Option<()> {
        let calm = self.mem.r16(DS, 0x653E);
        if calm != 0 {
            self.mem.w16(DS, 0x653E, calm - 1);
        }
        self.mem.w8(DS, CLOSE, 0);
        let draw = self.mem.r16(DS, crate::info::var::FLAT_NUMBER).wrapping_shl(2);
        self.mem.w16(DS, DRAW_NEXT, draw);
        self.mem.w16(DS, COPIES_END, 0x627C);
        for v in [PEOPLE_ACTIVE, CREATURES_ACTIVE, NEAR] {
            self.mem.w16(DS, v, 0);
        }
        for i in 0..COUNT {
            let si = ACTORS + i * SIZE;
            let mut sq = 0xFFFF;
            if self.actor_present(si) {
                sq = square(self.act16(si, f::X), self.act16(si, f::Y));
                if si >= CREATURES {
                    let n = self.mem.r16(DS, NEAR).wrapping_add(1);
                    self.mem.w16(DS, NEAR, n);
                    self.mem.w16(DS, NEAR_SQUARE, sq);
                }
            }
            self.mem.w16(DS, SQUARES + 2 * i, sq);
        }
        for i in 0..COUNT {
            let si = ACTORS + i * SIZE;
            let di = SQUARES + 2 * i;
            if self.act_flags(si) & 0x4000 != 0 {
                continue;
            }
            let counter = if si < CREATURES { PEOPLE_ACTIVE } else { CREATURES_ACTIVE };
            let n = self.mem.r16(DS, counter).wrapping_add(1);
            self.mem.w16(DS, counter, n);
            self.mem.w16(DS, di, 0xFFFF);
            self.actor_think(files, si)?;
            if !self.actor_near(si) && self.actor_far(si)? {
                // 0000:1903..1951: stays inactive this frame.
            } else {
                self.actor_frame(si)?;
            }
            // 0000:27BF: the actor's copy in the effect list (draw list,
            // not ported).
            self.actor_effect(si);
            if self.actor_present(si) {
                let sq = square(self.act16(si, f::X), self.act16(si, f::Y));
                self.mem.w16(DS, di, sq);
            }
        }
        if self.mem.r16(DS, NEAR) != 0 {
            // 0000:2837: now and then a sound.
            if self.mem.r8(DS, 0x100D) & 0x80 == 0 && self.rng.next() <= 0x7D0 {
                self.mem.w8(DS, 0xC887, 0x20);
                // 0000:284B: the creatures' sound (20h, its slot just set)
                // at the nearest one's square.
                let at = self.mem.r16(DS, NEAR_SQUARE);
                self.sound_at(0x20, at)?;
            }
        }
        Some(())
    }

    /// Active, not dying and not a corpse.
    fn actor_present(&self, si: u16) -> bool {
        let c = self.act8(si, f::SPRITE);
        self.act_flags(si) & 0x4000 == 0 && c != 0xC5 && c != self.corpse_sprite()
    }

    /// 0000:2313. Near enough to stay active: 0x320 for creatures, 0xC00
    /// for people (the larger of the two distances plus a quarter of the
    /// smaller).
    fn actor_near(&self, si: u16) -> bool {
        let d = self.player_distance(si);
        d < if si >= CREATURES { 0x320 } else { 0xC00 }
    }

    /// 0000:1903..1951, an actor too far away: a corpse is cleared; the
    /// actor goes inactive, unless map flag 0x8000 respawns a hostile one
    /// near the player (false then: it carries on with this frame).
    fn actor_far(&mut self, si: u16) -> Option<bool> {
        if self.act8(si, f::SPRITE) == self.corpse_sprite() {
            self.set_act8(si, f::SPRITE, 0xC0);
            let fl = self.act_flags(si) | 0x4000;
            self.set_act_flags(si, fl);
            self.actor_forget(si);
            return Some(true);
        }
        let fl = self.act_flags(si) | 0x4000;
        self.set_act_flags(si, fl);
        if fl & 4 != 0 {
            if si >= CREATURES {
                self.actor_leave(si);
            }
            return Some(true);
        }
        if self.mem.r16(DS, MAP_FLAGS) & 0x8000 == 0 {
            return Some(true);
        }
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 && !self.wild_quarter_ok() {
            return Some(true);
        }
        self.actor_respawn(si)?;
        Some(false)
    }

    /// 217B:0058. Whether the wilderness quarter the player is in (by
    /// x and y against 0x2000) is listed at 392E (count at 392D): the
    /// blocks people may come to (the carry clear).
    pub fn wild_quarter_ok(&self) -> bool {
        let mut bx = 0u16;
        if self.mem.r16(DS, PY) >= 0x2000 {
            bx = 2;
        }
        if self.mem.r16(DS, PX) >= 0x2000 {
            bx += 1;
        }
        let al = self.mem.r8(DS, 0x0604 + bx);
        let n = self.mem.r8(DS, 0x392D) as u16;
        (0..n).any(|i| self.mem.r8(DS, 0x392E + i) == al)
    }

    /// 0000:1954..19E3: the rest of an active actor's frame.
    fn actor_frame(&mut self, si: u16) -> Option<()> {
        let fl = self.act_flags(si) & !0x4000;
        self.set_act_flags(si, fl);
        self.actor_animate(si)?;
        if self.act_flags(si) & 0x4000 != 0 {
            return Some(());
        }
        self.set_act8(si, f::LIMIT, 0xFF);
        let c = self.act8(si, f::SPRITE);
        if c == self.corpse_sprite() || c == 0xC5 {
            return Some(());
        }
        let al = if self.act_flags(si) & 1 != 0 && si < CREATURES { 0xC0 } else { self.actor_view(si) as u8 | 0xC0 };
        self.set_act8(si, f::SPRITE, al);
        if al & 7 != 0 {
            let limit = if self.actor_is_creature(si) { 8 } else { 6 };
            if self.act8(si, f::FRAME) >= limit {
                self.set_act8(si, f::FRAME, 0);
            }
        }
        // (0000:1B7F adds the actor to the draw list.)
        Some(())
    }

    /// 0000:FE7E. Which way the actor is seen from the player: 0 facing
    /// the player .. 4 facing away, in eighths of a turn; flag 2 when it
    /// faces right of the player (the sprite mirrored).
    fn actor_view(&mut self, si: u16) -> u16 {
        let fl = self.act_flags(si) & !2;
        self.set_act_flags(si, fl);
        let a = self.toward_player(si).wrapping_add(0x20) >> 6;
        let b = self.act16(si, f::HEADING).wrapping_add(0x20) >> 6;
        let d = a.wrapping_sub(b);
        if d == 0 {
            return 0;
        }
        if (d as i16) < 0 {
            let d = d.wrapping_neg();
            if d > 4 {
                return 8u16.wrapping_sub(d);
            }
            self.set_act_flags(si, fl | 2);
            d
        } else {
            if d <= 4 {
                return d;
            }
            self.set_act_flags(si, fl | 2);
            8u16.wrapping_sub(d)
        }
    }

    /// 0000:27BF. The actor's entry in the list at 5F6A (effects drawn
    /// with it) goes inactive when its frame reaches its limit.
    fn actor_effect(&mut self, si: u16) {
        let e = si.wrapping_sub(ACTORS).wrapping_add(0x5F6A);
        if self.act_flags(e) & 0x4000 == 0 && self.act8(e, f::FRAME) >= self.act8(e, f::LIMIT) & 0x7F {
            let fl = self.act_flags(e) | 0x4000;
            self.set_act_flags(e, fl);
        }
        // (Otherwise 0000:FE58 adds it to the draw list.)
    }

    /// 0000:269E (2EBD:0084). Spell effects on the actor's record end:
    /// each is cleared and its two last bytes set to 0xFF (and DGROUP
    /// 0D39 gets bit 0x80); the record's low status flags clear.
    pub(crate) fn actor_forget(&mut self, si: u16) {
        let seg = self.mem.r16(DS, CREATURE_SEG);
        let rec = self.act16(si, f::RECORD);
        let key = (seg as u32) << 16 | rec as u32;
        for i in 0..16 {
            let e = EFFECTS + i * 0x3C;
            if self.mem.r32(DS, e + 0x28) == key {
                for k in 0..0x3C {
                    self.mem.w8(DS, e + k, 0);
                }
                self.mem.w8(DS, e + 0x3B, 0xFF);
                self.mem.w8(DS, e + 0x3A, 0xFF);
                let b = self.mem.r8(DS, 0x0D39) | 0x80;
                self.mem.w8(DS, 0x0D39, b);
            }
        }
        let w = self.mem.r16(seg, rec.wrapping_add(0x40D)) & 0xFF00;
        self.mem.w16(seg, rec.wrapping_add(0x40D), w);
    }

    /// 0000:25CE. A hostile creature out of range, with DGROUP 05C8 flag
    /// 0x1000 and not dying: the live creatures are counted again, a
    /// trigger waiting on it gets its square (the row; for the column a
    /// leftover loop count), and a flat (0x82, number at 05E2) is left
    /// at its square or a free one around it. Then its spell effects
    /// end.
    fn actor_leave(&mut self, si: u16) {
        if self.mem.r16(DS, 0x05C8) & 0x1000 != 0 && self.act8(si, f::SPRITE) != 0xC5 {
            let mut n = 0;
            for k in 0..8 {
                if self.act_flags(CREATURES + k * SIZE) & 0x4000 == 0 {
                    n += 1;
                }
            }
            self.mem.w16(DS, CREATURES_ACTIVE, n);
            let al = self.mem.r8(DS, 0x05E2);
            let row = (self.act16(si, f::Y) >> 7) << 8;
            let col = ((self.act16(si, f::X) >> 7) as u8).wrapping_add((self.act16(si, f::X) >> 7) as u8);
            if let Some(k) = (0..8).find(|&k| self.mem.r16(DS, MARKS + k * 6) == si) {
                let m = MARKS + k * 6;
                self.mem.w16(DS, m, 0xFFFF);
                let t = self.mem.r16(DS, m + 2);
                // The column byte is meant to be the square's, but CX
                // still holds the search's loop count here.
                self.mem.w8(DS, t.wrapping_add(TRIGGER_SQUARES), (8 - k) as u8 >> 1);
                self.mem.w8(DS, t.wrapping_add(TRIGGER_SQUARES + 1), (row >> 8) as u8);
            }
            let walls = self.mem.r16(DS, MAP1_SEG);
            if let Some(at) = self.free_around(walls, row | col as u16) {
                self.mem.w16(walls, at, 0x8200 | al as u16);
            }
        }
        self.actor_forget(si);
    }

    /// 0000:2675. The square `at` if empty, else the first empty one of
    /// the nine around it (offsets at AROUND); None if all are taken.
    fn free_around(&self, walls: u16, at: u16) -> Option<u16> {
        if self.mem.r16(walls, at) == 0 {
            return Some(at);
        }
        (0..9u16)
            .map(|k| at.wrapping_add(self.mem.r16(DS, AROUND + 2 * k)))
            .find(|&s| self.mem.r16(walls, s) == 0)
    }

    /// 0000:2054. Puts the actor on a random free square 0x800..0xBFF
    /// units from the player, inside the map, at its centre; a person
    /// also gets a new record number (random) and heading (a quarter).
    pub(crate) fn actor_respawn(&mut self, si: u16) -> Option<()> {
        for _ in 0..RESPAWN_LIMIT {
            let h = self.rng.next() & 0x1FF;
            let r1 = ((self.rng.next() & 0x3FF) + 0x800) as i16 as i32;
            let dx = ((r1 * self.mem.r16(DS, SIN + 2 * h) as i16 as i32) >> 16) as u16;
            let r2 = ((self.rng.next() & 0x3FF) + 0x800) as i16 as i32;
            let dy = ((r2 * self.mem.r16(DS, COS + 2 * h) as i16 as i32) >> 16) as u16;
            let x = dx.wrapping_add(self.mem.r16(DS, PX));
            if (x as i16) < 0 || (x as i16) >= (self.mem.r16(DS, MAP_WIDTH) << 7) as i16 {
                continue;
            }
            let y = dy.wrapping_add(self.mem.r16(DS, PY));
            if (y as i16) < 0 || (y as i16) >= (self.mem.r16(DS, MAP_DEPTH) << 7) as i16 {
                continue;
            }
            let (x, y) = ((x & 0xFF80) + 0x40, (y & 0xFF80) + 0x40);
            if self.square_blocked(square(x, y)) {
                continue;
            }
            self.set_act16(si, f::X, x);
            self.set_act16(si, f::Y, y);
            let r = self.rng.next();
            if si < CREATURES {
                self.set_act16(si, f::RECORD, r & 0x7FFF);
            }
            self.set_act16(si, f::HEADING, r & 0x180);
            return Some(());
        }
        None
    }

    /// 0000:26DC. A wall, or a floor at or above FLOOR_SOLID.
    pub(crate) fn square_blocked(&self, sq: u16) -> bool {
        if self.mem.r16(self.mem.r16(DS, MAP1_SEG), sq) != 0 {
            return true;
        }
        self.mem.r16(self.mem.r16(DS, FLOOR_SEG), sq) >= self.mem.r16(DS, FLOOR_SOLID)
    }

    /// 2826:0A87. Whether an actor stands on square `sq` (by the squares
    /// table); its index goes to OCCUPANT (0xFFFF if none).
    pub(crate) fn square_occupied(&mut self, sq: u16) -> bool {
        self.mem.w16(DS, OCCUPANT, 0xFFFF);
        match (0..COUNT).find(|&i| self.mem.r16(DS, SQUARES + 2 * i) == sq) {
            Some(i) => {
                self.mem.w16(DS, OCCUPANT, i);
                true
            }
            None => false,
        }
    }

    /// 0000:20DC (far). Whether the actor may stand at (x, y): inside the
    /// map, not the player's square, (with +2 zero) not on a floor at or
    /// above FLOOR_SOLID, and no actor there.
    fn actor_may_stand(&mut self, si: u16, x: u16, y: u16) -> bool {
        if (x as i16) < 0 || (y as i16) < 0 {
            return false;
        }
        if (x as i16) >= (self.mem.r16(DS, MAP_WIDTH) << 7) as i16 || (y as i16) >= (self.mem.r16(DS, MAP_DEPTH) << 7) as i16 {
            return false;
        }
        let sq = square(x, y);
        if sq == self.mem.r16(DS, PSQUARE) {
            return false;
        }
        if self.act16(si, f::Z) == 0 && self.mem.r16(self.mem.r16(DS, FLOOR_SEG), sq) >= self.mem.r16(DS, FLOOR_SOLID) {
            return false;
        }
        !self.square_occupied(sq)
    }

    /// 0000:1CEE. Whether the actor leaves the player alone: only with
    /// DGROUP 05BE flag 1 and 653E clear, for creatures of types 0x0C,
    /// 0x0F, 0x14, 0x16, 0x17 and 0x2A (people always do then).
    fn actor_peaceful(&self, si: u16) -> bool {
        if self.mem.r16(DS, 0x05BE) & 1 == 0 || self.mem.r16(DS, 0x653E) != 0 {
            return false;
        }
        if !self.actor_is_creature(si) {
            return true;
        }
        !matches!(self.act_rec8(si, 4), 0x14 | 0x0C | 0x0F | 0x17 | 0x16 | 0x2A)
    }

    /// 0000:1D3C. What the actor does this frame.
    fn actor_think(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let c = self.act8(si, f::SPRITE);
        if self.act_flags(si) & 4 == 0 {
            if c == self.corpse_sprite() {
                self.set_act8(si, f::FRAME, 0xFF);
                return Some(());
            }
            self.actor_idle(si);
            return Some(());
        }
        if c == 0xC5 || c == self.corpse_sprite() {
            return Some(());
        }
        let status = self.act_rec16(si, 0x40D);
        if status & 0x40 != 0 {
            self.set_act8(si, f::FRAME, 0);
            return Some(());
        }
        let speed = ((self.act_rec8(si, 0x2D) as u16 * 0x14) >> 8) + 0x14;
        self.set_act16(si, f::SPEED, speed);
        let creature = status & 0x1000 != 0;
        if creature && self.act8(si, f::FRAME) >= 8 {
            if self.act8(si, f::FRAME) != 0xA {
                return Some(());
            }
            return self.actor_blow(si);
        }
        let mut heading = ((self.rng.next() as i16 >> 13) as u16).wrapping_add(self.act16(si, f::HEADING)) & 0x1FF;
        let mut wander = true;
        if !self.actor_peaceful(si) && self.mem.r16(DS, 0x05BE) & 2 == 0 {
            heading = self.toward_player(si);
            wander = false;
        } else {
            self.set_act8(si, f::FRAME, 0);
        }
        if creature {
            let fl = self.act_flags(si) & !0x10;
            self.set_act_flags(si, fl);
        }
        self.set_act16(si, f::HEADING, heading);
        if !wander && self.actor_close(si) {
            self.mem.w8(DS, CLOSE, 1);
            if creature {
                let fl = self.act_flags(si) | 0x10;
                self.set_act_flags(si, fl);
                let fl = self.act_flags(si) & !1;
                self.set_act_flags(si, fl);
                if self.act8(si, f::FRAME) >= 6 {
                    return Some(());
                }
            } else if self.act_flags(si) & 0x10 != 0 {
                return Some(());
            }
            self.set_act8(si, f::FRAME, 0);
            if self.rng.next() & 7 != 0 {
                return Some(());
            }
            if creature {
                self.set_act8(si, f::FRAME, 8);
            } else {
                self.set_act8(si, f::FRAME, 5);
                let fl = self.act_flags(si) | 0x10;
                self.set_act_flags(si, fl);
            }
            return Some(());
        }
        // 0000:1ED4
        if !wander {
            if self.act_flags(si) & 1 != 0 {
                self.set_act8(si, f::FRAME, 0);
            } else {
                self.actor_chase(files, si)?;
            }
        }
        let fl = self.act_flags(si) & !1;
        self.set_act_flags(si, fl);
        if self.actor_peaceful(si) || self.act_rec16(si, 0x41A) & 0x200 != 0 {
            return Some(());
        }
        if !creature {
            if self.act_rec8(si, 5) & 0x20 == 0 || !self.actor_sees_player(si) {
                let fl = self.act_flags(si) & !1;
                self.set_act_flags(si, fl);
                return Some(());
            }
            let Some((index, cost)) = self.actor_pick_spell(si)? else {
                let fl = self.act_flags(si) & !1;
                self.set_act_flags(si, fl);
                return Some(());
            };
            let fl = self.act_flags(si) | 1;
            self.set_act_flags(si, fl);
            if self.rng.next() % 0x16 != 0 {
                return Some(());
            }
            let (seg, rec) = (self.mem.r16(DS, crate::combat::var::CREATURE_SEG), self.act16(si, f::RECORD));
            let sp = self.mem.r16(seg, rec.wrapping_add(0x66)).wrapping_sub(cost);
            self.mem.w16(seg, rec.wrapping_add(0x66), sp);
            return self.actor_cast(seg, rec, si, index);
        }
        if self.act_rec8(si, 0x366) == 0 || !self.actor_sees_player(si) {
            return Some(());
        }
        if self.act_rec8(si, 4) == 0x15 && self.mem.r16(DS, 0x05B1) & 0x40 != 0 {
            return Some(());
        }
        let fl = self.act_flags(si) | 1;
        self.set_act_flags(si, fl);
        if self.rng.next() % 0x16 != 0 {
            return Some(());
        }
        let n = self.act_rec8(si, 0x366) as u16;
        let r = self.rng.range(0, n)?;
        let index = self.act_rec8(si, 0x367u16.wrapping_add(r));
        let (seg, rec) = (self.mem.r16(DS, crate::combat::var::CREATURE_SEG), self.act16(si, f::RECORD));
        self.actor_cast(seg, rec, si, index)
    }

    /// 0000:1DB3 / 22F5: the actor's record hits at the player
    /// (2AA1:00D8).
    fn actor_blow(&mut self, si: u16) -> Option<()> {
        let rec = self.act16(si, f::RECORD);
        self.mem.w16(DS, crate::combat::var::ATTACKER, rec);
        self.mem.w16(DS, crate::combat::var::DEFENDER, crate::character::PLAYER);
        self.attack_player(0)
    }

    /// 0000:1E1C..1E62: within 0xAA of the player, unless the player is
    /// low down on a square with a positive wall word.
    fn actor_close(&self, si: u16) -> bool {
        if self.player_distance(si) > 0xAA {
            return false;
        }
        if (self.mem.r16(DS, PH) as i16) < 0 {
            let walls = self.mem.r16(DS, MAP1_SEG);
            if self.mem.r16(walls, self.mem.r16(DS, PSQUARE)) as i16 > 0 {
                return false;
            }
        }
        true
    }

    /// 0000:1FA2. A person with flag 0x20 at +5 and at least 200 / (level
    /// + 1) SP picks one of the 16 spells listed at DGROUP 6259 at
    /// random until it can pay for one (cost / (level + 1)); returns the
    /// spell and that price. None where the original never finds one.
    fn actor_pick_spell(&mut self, si: u16) -> Option<Option<(u8, u16)>> {
        if self.act_rec8(si, 5) & 0x20 == 0 {
            return Some(None);
        }
        let per = self.act_rec8(si, 6) as u16 + 1;
        if self.act_rec16(si, 0x66) < 0xC8 / per {
            return Some(None);
        }
        let mut tries = crate::random::CycleCheck::new(&self.rng);
        loop {
            let r = self.rng.range(0, 0x10)?;
            let index = self.mem.r8(DS, 0x6259u16.wrapping_add(r));
            let (gs, spell) = self.spell_record(index);
            let cost = self.mem.r16(gs, spell.wrapping_add(0x32)) / per;
            if self.act_rec16(si, 0x66) >= cost {
                return Some(Some((index, cost)));
            }
            if tries.cycled(&self.rng) {
                return None;
            }
        }
    }

    /// 0000:234B. Clears flag 0x800, then sets it if no wall stands
    /// between the actor's square and the player's (0000:2373).
    fn actor_sees_player(&mut self, si: u16) -> bool {
        let fl = self.act_flags(si) & !0x800;
        self.set_act_flags(si, fl);
        let (ax, bx) = (self.mem.r16(DS, PX) >> 7, self.mem.r16(DS, PY) >> 7);
        let (cx, dx) = (self.act16(si, f::X) >> 7, self.act16(si, f::Y) >> 7);
        let seen = self.line_clear(ax, bx, cx, dx);
        if seen {
            let fl = self.act_flags(si) | 0x800;
            self.set_act_flags(si, fl);
        }
        seen
    }

    /// 0000:2373. Walks the squares from (ax, bx) to (cx, dx) (Bresenham,
    /// starting from the end with the smaller y) and tests each with
    /// 0000:2443; true when none blocks.
    fn line_clear(&self, mut ax: u16, mut bx: u16, mut cx: u16, mut dx: u16) -> bool {
        if bx >= dx {
            std::mem::swap(&mut cx, &mut ax);
            std::mem::swap(&mut dx, &mut bx);
        }
        let walls = self.mem.r16(DS, MAP1_SEG);
        let dy = dx.wrapping_sub(bx);
        let mut ddx = cx.wrapping_sub(ax);
        let start = |ax: u16, bx: u16| (ax.wrapping_add(ax)).wrapping_add(bx << 8);
        let mut di = start(ax, bx);
        let blocked = |di: u16| self.wall_blocks_sight(walls, di);
        let step_x = if (ddx as i16) >= 0 {
            2
        } else {
            ddx = ddx.wrapping_neg();
            0xFFFE
        };
        if ddx == 0 && dy == 0 {
            return true;
        }
        let (mut dxv, mut dyv) = (ddx, dy);
        // Major axis x: x steps every square, y when the error runs over.
        if dxv >= dyv {
            let mut si = dxv.wrapping_neg();
            let n = dxv as u32 + 1;
            for _ in 0..n {
                if blocked(di) {
                    return false;
                }
                si = si.wrapping_add(dyv);
                if (si as i16) >= 0 {
                    si = si.wrapping_sub(dxv);
                    di = di.wrapping_add(0x100);
                    if blocked(di) {
                        return false;
                    }
                }
                di = di.wrapping_add(step_x);
            }
        } else {
            std::mem::swap(&mut dxv, &mut dyv);
            let mut si = dxv.wrapping_neg();
            let n = dxv as u32 + 1;
            for _ in 0..n {
                if blocked(di) {
                    return false;
                }
                si = si.wrapping_add(dyv);
                if (si as i16) >= 0 {
                    si = si.wrapping_sub(dxv);
                    di = di.wrapping_add(step_x);
                    if blocked(di) {
                        return false;
                    }
                }
                di = di.wrapping_add(0x100);
            }
        }
        true
    }

    /// 0000:2443. Whether the wall word at `di` blocks sight: positive
    /// words, raised platforms (0xD), type 9 without bit 0x100, flats
    /// whose F: bit 0 is set, and closed doors (type 0xB, bits 0xE00
    /// clear).
    fn wall_blocks_sight(&self, walls: u16, di: u16) -> bool {
        let w = self.mem.r16(walls, di);
        if w == 0 {
            return false;
        }
        if (w as i16) > 0 {
            return true;
        }
        match (w >> 12) as u8 {
            0xD => true,
            9 => w & 0x100 == 0,
            8 => {
                let flat = (w & 0xFF).wrapping_add(self.mem.r16(DS, FLAT_BASE));
                self.mem.r8(DS, 0xCDA1u16.wrapping_add(flat.wrapping_mul(5))) & 1 != 0
            }
            0xB => w & 0xE00 == 0,
            _ => false,
        }
    }

    /// 0000:26F5. A person going about its business: near the player
    /// (under 0xC8) and with nothing else going on it stops and faces
    /// the player (flag 1, frame 6); otherwise it walks along its
    /// heading, kept on the centre line of its row or column, and turns
    /// a quarter (left or right at random) at a blocked square ahead.
    fn actor_idle(&mut self, si: u16) {
        let busy = self.mem.r16(DS, 0x7A54) != 0x77E && self.mem.r16(DS, 0xCCBA) != 0 && self.mem.r8(DS, 0xAFF9) & 1 != 0;
        if self.player_distance(si) < 0xC8
            && !busy
            && self.mem.r16(DS, 0x05E0) & 0x10 == 0
            && self.mem.r16(DS, 0x05BE) & 1 == 0
        {
            let fl = self.act_flags(si) | 1;
            self.set_act_flags(si, fl);
            if self.act8(si, f::FRAME) < 6 {
                self.set_act8(si, f::FRAME, 6);
            }
            return;
        }
        let fl = self.act_flags(si) & !1;
        self.set_act_flags(si, fl);
        let along = if self.act16(si, f::HEADING) & 0x80 == 0 {
            let x = (self.act16(si, f::X) & 0xFF80) + 0x40;
            self.set_act16(si, f::X, x);
            self.act16(si, f::Y)
        } else {
            let y = (self.act16(si, f::Y) & 0xFF80) + 0x40;
            self.set_act16(si, f::Y, y);
            self.act16(si, f::X)
        } & 0x7F;
        if (0x32..=0x4E).contains(&along) && self.ahead_blocked(si) {
            let turn = if self.rng.next() & 0x8000 != 0 { 0xFF80 } else { 0x80 };
            let h = self.act16(si, f::HEADING).wrapping_add(turn) & 0x1FF;
            self.set_act16(si, f::HEADING, h);
            return;
        }
        let q = (self.act16(si, f::HEADING) >> 7) << 2;
        let x = self.act16(si, f::X).wrapping_add(self.mem.r16(DS, STEPS.wrapping_add(q)));
        let y = self.act16(si, f::Y).wrapping_add(self.mem.r16(DS, STEPS.wrapping_add(q + 2)));
        self.set_act16(si, f::X, x);
        self.set_act16(si, f::Y, y);
    }

    /// 0000:26B1. The square ahead (by heading quarter) is taken by an
    /// actor or blocked.
    fn ahead_blocked(&mut self, si: u16) -> bool {
        let x = self.act16(si, f::X) >> 7;
        let col = (x as u8).wrapping_add(x as u8);
        let row = (self.act16(si, f::Y) >> 7) << 8;
        let q = (self.act16(si, f::HEADING) >> 7) << 1;
        let sq = (row | col as u16).wrapping_add(self.mem.r16(DS, AHEAD.wrapping_add(q)));
        self.square_occupied(sq) || self.square_blocked(sq)
    }

    /// 0000:2025. A step toward the player: along the heading, or if that
    /// gets less than a quarter of the speed, the better of the heading
    /// ±0x64 (0000:24BE); then the position is taken if the actor may
    /// stand there.
    fn actor_chase(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let (x, y) = self.actor_chase_step(files, si)?;
        if self.actor_may_stand(si, x, y) {
            self.set_act16(si, f::X, x);
            self.set_act16(si, f::Y, y);
        }
        Some(())
    }

    /// 0000:24BE. Returns the position for 0000:2025 to check.
    fn actor_chase_step(&mut self, files: &mut Files, si: u16) -> Option<(u16, u16)> {
        let heading = self.act16(si, f::HEADING);
        let (x, y, ok) = self.actor_move(files, si, heading)?;
        if !ok {
            return Some((x, y));
        }
        let moved = approx(x.wrapping_sub(self.act16(si, f::X)), y.wrapping_sub(self.act16(si, f::Y)));
        if moved >= self.act16(si, f::SPEED) >> 2 {
            self.set_act16(si, f::X, x);
            self.set_act16(si, f::Y, y);
            return Some((x, y));
        }
        // The candidates are kept at A836 (left) and A83A (right).
        for k in 0..4 {
            self.mem.w16(DS, SIDESTEPS + 2 * k, 0x6000);
        }
        let (lx, ly, ok) = self.actor_move(files, si, heading.wrapping_add(0x64))?;
        if ok {
            self.mem.w16(DS, SIDESTEPS, lx);
            self.mem.w16(DS, SIDESTEPS + 2, ly);
        }
        let (rx, ry, ok) = self.actor_move(files, si, heading.wrapping_sub(0x64))?;
        if ok {
            self.mem.w16(DS, SIDESTEPS + 4, rx);
            self.mem.w16(DS, SIDESTEPS + 6, ry);
        }
        let left = (self.mem.r16(DS, SIDESTEPS), self.mem.r16(DS, SIDESTEPS + 2));
        let right = (self.mem.r16(DS, SIDESTEPS + 4), self.mem.r16(DS, SIDESTEPS + 6));
        // 0000:122A takes the candidates' coordinates as they are, not
        // their distance from the player.
        let pick = if approx(left.0, left.1) < approx(right.0, right.1) { left } else { right };
        self.set_act16(si, f::X, pick.0);
        self.set_act16(si, f::Y, pick.1);
        Some(pick)
    }

    /// 0000:2566. Moves the actor's position by its speed along
    /// `heading` through the collision code in creature mode; returns
    /// the position reached and whether the actor may stand there.
    fn actor_move(&mut self, files: &mut Files, si: u16, heading: u16) -> Option<(u16, u16, bool)> {
        let b = (heading & 0x1FF) * 2;
        let a = self.act16(si, f::SPEED).wrapping_add(self.act16(si, f::SPEED)) as i16 as i32;
        let dx = ((a * self.mem.r16(DS, SIN + b) as i16 as i32) >> 16) as u16;
        let dy = ((a * self.mem.r16(DS, COS + b) as i16 as i32) >> 16) as u16;
        let (ox, oy) = (self.act16(si, f::X), self.act16(si, f::Y));
        self.mem.w8(DS, CREATURE_MODE, 1);
        // (In creature mode the stairs and their hooks are never reached.)
        let r = self.move_resolve(files, &mut crate::hooks::NoStairsHooks, dx.wrapping_add(ox), dy.wrapping_add(oy), ox, oy);
        self.mem.w8(DS, CREATURE_MODE, 0);
        let (x, y, _) = r?;
        let ok = self.actor_may_stand(si, x, y);
        Some((x, y, ok))
    }

    /// 0000:222D. Advances the actor's animation.
    fn actor_animate(&mut self, si: u16) -> Option<()> {
        let frame = self.act8(si, f::FRAME);
        if si < CREATURES {
            if self.act8(si, f::SPRITE) == self.corpse_sprite() {
                self.set_act8(si, f::FRAME, 0xFF);
                return Some(());
            }
            if self.act_flags(si) & 1 == 0 {
                let d = frame.wrapping_add(1);
                self.set_act8(si, f::FRAME, if d >= 6 { 0 } else { d });
                return Some(());
            }
            if frame <= 6 {
                if self.rng.next() & 7 == 0 {
                    self.set_act8(si, f::FRAME, frame.wrapping_add(1));
                }
                return Some(());
            }
            if self.mem.r16(DS, 0xCCBC) & 7 != 0 {
                return Some(());
            }
            let d = frame.wrapping_add(1);
            self.set_act8(si, f::FRAME, if d > 8 { 6 } else { d });
            return Some(());
        }
        let status = self.act_rec16(si, 0x40D);
        if status & 0x40 != 0 {
            return Some(());
        }
        if status & 0x1000 != 0 {
            return self.creature_animate(si);
        }
        if status & 0x100 != 0 {
            self.set_act8(si, f::FRAME, 0xFF);
            let c = self.corpse_sprite();
            self.set_act8(si, f::SPRITE, c);
            let fl = self.act_flags(si) & !0x20;
            self.set_act_flags(si, fl);
            return Some(());
        }
        if self.act_flags(si) & 0x10 != 0 {
            let d = frame.wrapping_add(1);
            self.set_act8(si, f::FRAME, d);
            if d < 9 {
                return Some(());
            }
            self.actor_blow(si)?;
            self.set_act8(si, f::FRAME, 8);
            let fl = self.act_flags(si) & !0x10;
            self.set_act_flags(si, fl);
            return Some(());
        }
        if frame == 8 {
            return Some(());
        }
        if self.act_flags(si) & 1 == 0 && !self.actor_peaceful(si) {
            let d = frame.wrapping_add(1);
            self.set_act8(si, f::FRAME, d);
            if d < 6 {
                return Some(());
            }
        }
        self.set_act8(si, f::FRAME, 0);
        Some(())
    }

    /// 0000:2137. A creature's animation.
    fn creature_animate(&mut self, si: u16) -> Option<()> {
        let frame = self.act8(si, f::FRAME);
        if self.act8(si, f::SPRITE) == 0xC5 {
            return self.creature_dying(si);
        }
        if self.act_flags(si) & 1 != 0 {
            return Some(());
        }
        if frame < 6 {
            if self.rng.next() & 0x1F == 0 && self.act_rec8(si, 4) != 0x10 {
                let fl = self.act_flags(si) | 0x40;
                self.set_act_flags(si, fl);
            }
            if self.mem.r16(DS, 0xCCBC) & 1 == 0 || self.act_flags(si) & 0x10 != 0 {
                return Some(());
            }
            if self.actor_peaceful(si) {
                self.set_act8(si, f::FRAME, 0);
                return Some(());
            }
            let d = frame.wrapping_add(1);
            self.set_act8(si, f::FRAME, if d >= 6 { 0 } else { d });
            return Some(());
        }
        if frame >= 8 {
            self.set_act8(si, f::SPRITE, 0xC0);
            let d = frame.wrapping_add(1);
            self.set_act8(si, f::FRAME, if d >= 0xC { 0 } else { d });
            return Some(());
        }
        let phase = self.act_flags(si) & 0xC0;
        if phase == 0 {
            self.set_act8(si, f::FRAME, 0);
            return Some(());
        }
        let bl = match phase {
            0x40 => 6,
            0x80 => 0,
            _ => 7,
        };
        self.set_act8(si, f::FRAME, bl);
        let fl = (self.act_flags(si) & 0xFF3F) | (phase + 0x40);
        self.set_act_flags(si, fl);
        Some(())
    }

    /// 0000:2184. A dying creature: while its health is above 0 it
    /// recovers (frames count down to the standing sprite); otherwise
    /// the frames count up to 3, where it is gone (DGROUP 0F83 set) or
    /// stays at frame 2 (sounding once).
    fn creature_dying(&mut self, si: u16) -> Option<()> {
        if self.act_rec16(si, 0x59) as i16 > 0 {
            let d = self.act8(si, f::FRAME).wrapping_sub(1);
            self.set_act8(si, f::FRAME, d);
            if d & 0x80 == 0 {
                return Some(());
            }
            self.set_act8(si, f::SPRITE, 0xC0);
            let seg = self.mem.r16(DS, CREATURE_SEG);
            let at = self.act16(si, f::RECORD).wrapping_add(0x40D);
            let w = self.mem.r16(seg, at) & 0xFEFF;
            self.mem.w16(seg, at, w);
            self.set_act8(si, f::FRAME, 0);
            return Some(());
        }
        let d = self.act8(si, f::FRAME).wrapping_add(1);
        self.set_act8(si, f::FRAME, d);
        if d < 3 {
            return Some(());
        }
        if self.mem.r8(DS, 0x0F83) != 0 {
            let fl = self.act_flags(si) | 0x4000;
            self.set_act_flags(si, fl);
            self.set_act8(si, f::FRAME, 0);
            return Some(());
        }
        self.set_act8(si, f::FRAME, 2);
        if self.act16(si, f::SOUNDED) == 0 {
            let n = self.act16(si, f::SOUNDED).wrapping_add(1);
            self.set_act16(si, f::SOUNDED, n);
            // 0000:21C7: its death sound (14h, at the player's square).
            self.sound_here(0x14)?;
        }
        Some(())
    }
}
