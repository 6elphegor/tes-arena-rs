//! Fights starting (segment 25CA): the creatures of an encounter, the
//! guards or the night's monsters are placed in the creature actor slots
//! (0C57.., eight of them, whose records are in the creature segment
//! [1042] at slot x 41Eh) around the player.

use crate::actors::var::SQUARES;
use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The creature actor slots (8 of 1Ch).
    pub const CREATURE_SLOTS: u16 = 0x0C57;
    /// Creatures fighting (word).
    pub const FIGHTING: u16 = 0x0FF9;
    /// The fight's kind: 8000h monsters (al nonzero), 1 people.
    pub const FIGHT_KIND: u16 = 0x0FFF;
    /// What a new creature gets: its +16 (DBE) and height (FF3).
    pub const NEW_16: u16 = 0x0DBE;
    pub const NEW_Z: u16 = 0x0FF3;
    /// A colour remap for the fight (bytes F74, F75; 0 none) and the
    /// segment of the remap tables.
    pub const REMAP: u16 = 0x0F74;
    pub const REMAP_SEG: u16 = 0xCCC2;
    /// The music of a fight of people.
    pub const MUSIC: u16 = 0x100D;
    /// Tries at the current spread (`place_near`).
    pub const TRIES: u16 = 0xA886;
}
use var::*;

/// What starting a fight leaves to the front end.
pub trait FightHooks {
    /// 0000:1AF9 / 1A5C: the creatures' graphics load (`people` for a
    /// fight of people).
    fn fight_graphics(&mut self, _m: &mut Machine, _files: &mut Files, _people: bool) -> Option<()> {
        Some(())
    }
    /// 23BA:01C7: the fight's music (`MUSIC`).
    fn fight_music(&mut self, _m: &mut Machine, _music: u8) {}
}

/// `FightHooks` that do nothing.
pub struct NoFightHooks;

impl FightHooks for NoFightHooks {}

impl Machine {
    /// 25CA:0000. Unless a fight is on, the next one is set up: with `al`
    /// nonzero monsters (FIGHT_KIND 8000h, the creature slots lose bit
    /// 8000h), otherwise people (1, the slots get it, and the music
    /// plays). The animation table 6270 is cleared and the graphics load.
    pub fn fight_prepare(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, al: u8) -> Option<()> {
        if self.mem.r16(DS, FIGHTING) != 0 {
            return Some(());
        }
        for i in 0..5 {
            self.mem.w16(DS, 0x6270 + 2 * i, 0);
        }
        self.mem.w16(DS, FIGHT_KIND, 0);
        let monsters = al != 0;
        self.mem.w16(DS, FIGHT_KIND, if monsters { 0x8000 } else { 1 });
        hooks.fight_graphics(self, files, !monsters)?;
        for i in 0..8 {
            let at = CREATURE_SLOTS + i * 0x1C + 0xF;
            let f = self.mem.r16(DS, at);
            self.mem.w16(DS, at, if monsters { f & 0x7FFF } else { f | 0x8000 });
        }
        if !monsters {
            let music = self.mem.r8(DS, MUSIC);
            hooks.fight_music(self, music);
        }
        Some(())
    }

    /// 25CA:006A. Up to `count` creatures (count 0 means 65536) come into
    /// the free creature slots (bit 4000h, the first free first), each
    /// for the record of its slot: on a random free square near the
    /// player (`place_near`, spread 6), facing the player (by quarters),
    /// at a speed from +2Dh of the floor plane (meant to be the record's
    /// speed: FS was left there), walking (34h). The slot
    /// keeps DBE at +16 and the height FF3 for flyers (record +40Dh bit
    /// 1000h, which also sets the slot's bit 8000h). The first eight get
    /// their square in the actors' squares (at 15 + FIGHTING). Stops when
    /// no slot is free. None for a quest's creature (5C8 bit 1000h, not
    /// ported), or where no square is found.
    pub fn place_creatures(&mut self, count: u16) -> Option<()> {
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        let n = if count == 0 { 0x10000u32 } else { count as u32 };
        for _ in 0..n {
            // 25CA:01C2: the first free slot.
            let Some(i) = (0..8u16).find(|&i| self.mem.r16(DS, CREATURE_SLOTS + i * 0x1C + 0xF) & 0x4000 != 0)
            else {
                return Some(());
            };
            let di = CREATURE_SLOTS + i * 0x1C;
            let rec = i * 0x41E;
            let f = self.mem.r16(DS, di + 0xF) & !0x100;
            self.mem.w16(DS, di + 0xF, f);
            let flyer = self.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x1000 != 0;
            let v = self.mem.r16(DS, NEW_16);
            self.mem.w16(DS, di + 0x16, v);
            let z = if flyer { self.mem.r16(DS, NEW_Z) } else { 0 };
            self.mem.w16(DS, di + 2, z);
            self.mem.w16(DS, di + 0x18, 0);
            self.mem.w16(DS, di + 0x12, rec);
            let (x, y, sq) = self.place_near(6)?;
            let fighting = self.mem.r16(DS, FIGHTING);
            if fighting < 8 {
                self.mem.w16(DS, SQUARES.wrapping_add(2 * (fighting + 0xF)), sq);
            }
            if self.mem.r16(DS, 0x05C8) & 0x1000 != 0 {
                return None;
            }
            self.mem.w16(DS, di, x);
            self.mem.w16(DS, di + 4, y);
            let (px, py) = (self.mem.r16(DS, 0xC3D0), self.mem.r16(DS, 0xC3D4));
            let h = crate::travel::travel_heading(x, y, px, py) & 0x1C0;
            self.mem.w16(DS, di + 0xA, h);
            // The speed is meant to come from the record's +2Dh, but the
            // square test (0000:26DC) left FS at the floor plane: it is read
            // there, at the record's offset.
            let floors = self.mem.r16(DS, crate::movement::var::FLOOR_SEG);
            let speed = ((self.mem.r8(floors, rec.wrapping_add(0x2D)) as u16 * 20) >> 8) + 0x14;
            self.mem.w16(DS, di + 8, speed);
            let mut bp = 0xCu16;
            if flyer {
                bp |= 0x8000;
            }
            let remap = self.mem.r8(DS, REMAP);
            if remap != 0 {
                bp |= 0x20;
                let rs = self.mem.r16(DS, REMAP_SEG);
                let second = self.mem.r8(DS, REMAP + 1);
                for k in 0..0x100u16 {
                    self.mem.w8(rs, 0xE00 + k, remap);
                    self.mem.w8(rs, 0xF00 + k, second);
                }
            }
            self.mem.w16(DS, di + 0xF, bp);
            self.mem.w8(DS, di + 0x11, 0x34);
            self.mem.w8(DS, di + 0xC, 0);
            self.mem.w16(DS, FIGHTING, fighting.wrapping_add(1));
        }
        self.clear_dungeon_flats();
        Some(())
    }

    /// 0000:1B19. A random square near the player (each axis the player's
    /// plus a random number shifted right by `spread`), at its centre,
    /// not the player's, with no actor (`actor_on`) and not blocked: 40
    /// tries, then the spread halves (a shift of 0 spreads over the whole
    /// map). Returns (x, y, square). None after the spread reached 0 and
    /// 40 more tries failed (the original keeps trying).
    pub(crate) fn place_near(&mut self, mut spread: u16) -> Option<(u16, u16, u16)> {
        loop {
            self.mem.w16(DS, TRIES, 0);
            loop {
                let rx = ((self.rng.next() as i16) >> (spread & 0x1F).min(15)) as u16;
                let x = rx.wrapping_add(self.mem.r16(DS, 0xC3D0));
                let ry = ((self.rng.next() as i16) >> (spread & 0x1F).min(15)) as u16;
                let y = ry.wrapping_add(self.mem.r16(DS, 0xC3D4));
                let (x, y) = ((x & 0xFF80).wrapping_add(0x40), (y & 0xFF80).wrapping_add(0x40));
                let sq = ((y >> 7) << 8) | (((x >> 7) as u8).wrapping_add((x >> 7) as u8)) as u16;
                let bad = sq == self.mem.r16(DS, 0xCB90) || self.actor_on(sq) || self.square_blocked(sq);
                if !bad {
                    return Some((x, y, sq));
                }
                let t = self.mem.r16(DS, TRIES) + 1;
                self.mem.w16(DS, TRIES, t);
                if t >= 0x28 {
                    break;
                }
            }
            if spread & 0xFF == 0 {
                return None;
            }
            spread = (spread & 0xFF00) | ((spread as u8) >> 1) as u16;
        }
    }

    /// 0000:50B9. In a dungeon (kind 4) once 191D is set, flats (type 8)
    /// that are C943, or one of the eight at C987, are cleared from the
    /// map.
    fn clear_dungeon_flats(&mut self) {
        if self.mem.r8(DS, 0x05E6) != 4 || self.mem.r8(DS, 0x191D) == 0 {
            return;
        }
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let special = self.mem.r8(DS, 0xC943);
        for i in 0..0x4000u16 {
            let w = self.mem.r16(walls, 2 * i);
            if w >> 12 != 8 {
                continue;
            }
            let b = w as u8;
            if b == special || (0..8).any(|k| self.mem.r8(DS, 0xC987 + k) == b) {
                self.mem.w16(walls, 2 * i, 0);
            }
        }
    }
}
