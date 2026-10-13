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
    /// The fight's kind: 8000h people (al nonzero), 1 monsters.
    pub const FIGHT_KIND: u16 = 0x0FFF;
    /// What a new creature gets: its +16 (DBE) and height (FF3).
    pub const NEW_16: u16 = 0x0DBE;
    pub const NEW_Z: u16 = 0x0FF3;
    /// A colour remap for the fight (bytes F74, F75; 0 none) and the
    /// segment of the remap tables.
    pub const REMAP: u16 = 0x0F74;
    pub const REMAP_SEG: u16 = 0xCCC2;
    /// The music of a fight of monsters.
    pub const MUSIC: u16 = 0x100D;
    /// Tries at the current spread (`place_near`).
    pub const TRIES: u16 = 0xA886;
    /// The guards' rank and level.
    pub const GUARD_RANK: u16 = 0x1294;
    pub const GUARD_LEVEL: u16 = 0x1298;
}
use var::*;

/// What starting a fight leaves to the front end.
pub trait FightHooks {
    /// 0000:1AF9 (people) / 1A5C (monsters): the creatures' graphics
    /// load.
    fn fight_graphics(&mut self, _m: &mut Machine, _files: &mut Files, _people: bool) -> Option<()> {
        Some(())
    }
    /// 23BA:01C7: the fight's music (`MUSIC`).
    fn fight_music(&mut self, _m: &mut Machine, _music: u8) {}
    /// 2686:0423: the text naming who comes (the record at `rec` in the
    /// creature segment), written at 1044 for the front end.
    fn fight_text(&mut self, _m: &mut Machine, _rec: u16) {}
}

/// `FightHooks` that do nothing.
pub struct NoFightHooks;

impl FightHooks for NoFightHooks {}

impl Machine {
    /// 25CA:0000. Unless a fight is on, the next one is set up: with `al`
    /// nonzero people (FIGHT_KIND 8000h, the creature slots lose bit
    /// 8000h; their graphics come from a list, 0000:1AF9), otherwise
    /// monsters (1, the slots get bit 8000h, and the fight's music
    /// plays). The animation table 6270 is cleared and the graphics load.
    pub fn fight_prepare(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, al: u8) -> Option<()> {
        if self.mem.r16(DS, FIGHTING) != 0 {
            return Some(());
        }
        for i in 0..5 {
            self.mem.w16(DS, 0x6270 + 2 * i, 0);
        }
        self.mem.w16(DS, FIGHT_KIND, 0);
        let people = al != 0;
        self.mem.w16(DS, FIGHT_KIND, if people { 0x8000 } else { 1 });
        hooks.fight_graphics(self, files, people)?;
        for i in 0..8 {
            let at = CREATURE_SLOTS + i * 0x1C + 0xF;
            let f = self.mem.r16(DS, at);
            self.mem.w16(DS, at, if people { f & 0x7FFF } else { f | 0x8000 });
        }
        if !people {
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

    /// 1194:33C0, the routine the guards' countdown calls: the people
    /// about calm down (2294:123A), and one to three town guards come
    /// (built by `build_guard` into the first records of the creature
    /// segment) and are placed around the player.
    pub fn guards_arrive(&mut self, files: &mut Files, hooks: &mut dyn FightHooks) -> Option<()> {
        let f = self.mem.r16(DS, 0x05C8) & !0x1000;
        self.mem.w16(DS, 0x05C8, f);
        self.calm_people();
        self.mem.w16(DS, REMAP, 0);
        self.mem.w16(DS, NEW_16, 0);
        self.mem.w8(DS, MUSIC, 0xFF);
        self.mem.w16(DS, NEW_Z, 0);
        // 1194:34BC: the guards' rank by d100 against the table at 41AD.
        let r = self.rng.next() % 100;
        let mut rank = 0u16;
        while r as u8 > self.mem.r8(DS, 0x41AD + rank) {
            rank += 1;
        }
        self.mem.w16(DS, GUARD_RANK, rank);
        // 1194:34A1: their level, 0..2 above the province's base plus the
        // rank.
        let base = (self.mem.r8(DS, 0x41AA + self.mem.r8(DS, 0x0A47) as u16) as u16).wrapping_add(rank);
        let level = self.rng.range(base, base.wrapping_add(3))?;
        self.mem.w16(DS, GUARD_LEVEL, level);
        let n = self.rng.range(0, 3)?.wrapping_add(1);
        for k in 0..n {
            self.build_guard(files, k.wrapping_mul(0x41E))?;
        }
        hooks.fight_text(self, 0);
        self.fight_prepare(files, hooks, 1)?;
        self.place_creatures(n)
    }

    /// 1194:42B5. The fight ends: no creatures fighting (0FF9, 0FFB), every
    /// actor calm (bit 4000h); creatures not yet calm lose their spell
    /// effects (`actor_forget`).
    pub fn calm_everyone(&mut self) {
        self.mem.w16(DS, FIGHTING, 0);
        self.mem.w16(DS, 0x0FFB, 0);
        for i in 0..0x17u16 {
            let si = 0x0AB3 + i * 0x1C;
            if si >= CREATURE_SLOTS {
                if self.mem.r16(DS, si + 0xF) & 0x4000 != 0 {
                    continue;
                }
                self.actor_forget(si);
            }
            let fl = self.mem.r16(DS, si + 0xF) | 0x4000;
            self.mem.w16(DS, si + 0xF, fl);
        }
    }

    /// 0000:3F0C / 3F3B, the guards' hook: their call (a sound), everyone
    /// calms down, and the guards arrive. None for another hook.
    pub fn guards_hook(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, hook: u32) -> Option<()> {
        if hook != 0x3F0C && hook != 0x3F3B {
            return None;
        }
        self.calm_everyone();
        self.guards_arrive(files, hooks)
    }

    /// 2294:123A. The 15 people slots, but corpses, become calm (bit
    /// 4000h).
    fn calm_people(&mut self) {
        let corpse = (self.mem.r8(DS, 0xC936) as u16).wrapping_add(self.mem.r16(DS, 0xC38E)) as u8;
        for i in 0..0xF {
            let si = 0x0AB3 + i * 0x1C;
            if self.mem.r8(DS, si + 0xC) != corpse {
                let f = self.mem.r16(DS, si + 0xF) | 0x4000;
                self.mem.w16(DS, si + 0xF, f);
            }
        }
    }

    /// 1194:3422. A guard at `rec` of the creature segment: a named NPC of
    /// class 10h at GUARD_LEVEL (seeded in a heap block, 1194:35C5), of the
    /// province's race (A46), with the attributes of the province and
    /// rank (3276:3AE3), marked a guard (40Dh bits 400h, 800h), in plain
    /// armour by rank (four pieces, 3276:6169), with a weapon of type 5,
    /// and a shield (3276:616D).
    fn build_guard(&mut self, files: &mut Files, rec: u16) -> Option<()> {
        self.mem.w16(DS, 0x1296, rec);
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        // 1194:35C5
        let seed = self.heap_alloc_high(0x10)?;
        self.mem.w16(DS, 0x4198, seed);
        let level = self.mem.r16(DS, GUARD_LEVEL);
        self.npc_seed(seed, 0, 0x10, level)?;
        self.expand_npc_named(files, seed, 0, seg, rec)?;
        let s = self.mem.r16(DS, 0x4198);
        self.heap_free(s);
        let rank = self.mem.r16(DS, GUARD_RANK);
        let table = 0x3AE3u16
            .wrapping_add((self.mem.r8(DS, 0x0A47) as u16).wrapping_mul(0x18))
            .wrapping_add(rank.wrapping_mul(8));
        let race = self.mem.r8(DS, 0x0A46);
        self.mem.w8(seg, rec.wrapping_add(4), race);
        for i in 0..8 {
            let v = self.mem.r8(crate::mem::DATA, table.wrapping_add(i));
            self.mem.w8(seg, rec.wrapping_add(0x29 + i), v);
        }
        let f = self.mem.r16(seg, rec.wrapping_add(0x40D)) | 0xC00;
        self.mem.w16(seg, rec.wrapping_add(0x40D), f);
        self.equip_guard(rec.wrapping_add(0x6E), rank)?;
        self.mem.w16(DS, NEW_16, 0);
        self.mem.w8(DS, MUSIC, 0xFF);
        Some(())
    }

    /// 1194:34CD. The guard's items from `di` on: four armour pieces of
    /// the rank's level, the weapon (type 5), and the shield; the shield's
    /// level is whatever the weapon left in AL: the low byte of its
    /// health (ITEM + 6).
    fn equip_guard(&mut self, mut di: u16, rank: u16) -> Option<()> {
        use crate::loot::ITEM;
        let data = crate::mem::DATA;
        for b in 0..4 {
            let forced = self.mem.r8(data, 0x6169 + b);
            self.mem.w8(DS, 0x101E, rank as u8);
            self.loot_armor(rank as u8, forced);
            di = self.put_item(di);
        }
        self.loot_weapon(self.mem.r8(DS, ITEM + 6), 5)?;
        di = self.put_item(di);
        self.mem.w8(DS, 0x101E, 0);
        let forced = self.mem.r8(data, 0x616D_u16.wrapping_add(rank));
        self.loot_armor(self.mem.r8(DS, ITEM + 6), forced);
        self.put_item(di);
        Some(())
    }

    /// 1194:39CD. The item made (ITEM, 13h bytes, marked 80h at +0Eh) goes
    /// to `di` in the creature segment; returns the next place.
    fn put_item(&mut self, di: u16) -> u16 {
        use crate::loot::ITEM;
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        let v = self.mem.r8(DS, ITEM + 0xE) | 0x80;
        self.mem.w8(DS, ITEM + 0xE, v);
        for i in 0..0x13 {
            let b = self.mem.r8(DS, ITEM + i);
            self.mem.w8(seg, di.wrapping_add(i), b);
        }
        di.wrapping_add(0x13)
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
