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
    /// Whether it is night for an encounter (1238).
    pub const NIGHT: u16 = 0x1238;
    /// The guards' rank and level.
    pub const GUARD_RANK: u16 = 0x1294;
    pub const GUARD_LEVEL: u16 = 0x1298;
    /// A creature's flat seen (`flat_creature`): its number, its place,
    /// and its map word (a far pointer).
    pub const FLAT_NUMBER: u16 = 0x5E38;
    pub const FLAT_X: u16 = 0x5E3E;
    pub const FLAT_Y: u16 = 0x5E40;
    pub const FLAT_AT: u16 = 0x5E39;
}
use var::*;

/// What starting a fight leaves to the front end.
pub trait FightHooks {
    /// 0000:1AF9 (people) / 1A5C (monsters): the creatures' graphics
    /// load (`Machine::fight_graphics_load`).
    fn fight_graphics(&mut self, m: &mut Machine, files: &mut Files, people: bool) -> Option<()> {
        m.fight_graphics_load(files, people)
    }
    /// 23BA:01C7: the fight's sound (`MUSIC`: the creature's) loaded
    /// into bank 15h and made the file slot (`Machine::fight_sound`).
    fn fight_music(&mut self, m: &mut Machine, files: &mut Files, music: u8) -> Option<()> {
        m.fight_sound(files, music as u16)
    }
    /// 1194:D45D: JAGAR.FLC with text `text` (colour 9288 F5h), Jagar's
    /// words before a main quest's encounter.
    fn jagar(&mut self, _m: &mut Machine, _text: u16) {}
    /// 0000:A611 and A6B5: the view saved and put back around it.
    fn view_saved(&mut self, _m: &mut Machine) {}
    fn view_restored(&mut self, _m: &mut Machine) {}
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
            hooks.fight_music(self, files, music)?;
        }
        Some(())
    }

    /// 25CA:0021..004F: the fight's graphics from the names at DGROUP
    /// 1044, where every caller leaves SI (8B12's monster's file or
    /// people's list, 2686:0423's list): people's by 0000:1AF9, each name
    /// of the list (to an empty one) loaded by 1A5C into bank 34h / 2's
    /// half on; a monster's (0FFF 1: a bank each) by 1A5C with its file
    /// at 8000h of bank 19h too.
    pub fn fight_graphics_load(&mut self, files: &mut Files, people: bool) -> Option<()> {
        let mut si = 0x1044u16;
        if !people {
            return self.actor_graphics(files, si, 0x34, 0x19, 0x8000);
        }
        if self.mem.r16(DS, si) == 0 {
            return Some(());
        }
        loop {
            self.actor_graphics(files, si, 0x34, 0xFFFF, 0)?;
            while self.mem.r8(DS, si) != 0 {
                si = si.wrapping_add(1);
            }
            si = si.wrapping_add(1);
            if self.mem.r8(DS, si) == 0 {
                return Some(());
            }
        }
    }

    /// 25CA:006A. Up to `count` creatures (count 0 means 65536) come into
    /// the free creature slots (bit 4000h, the first free first), each
    /// for the record of its slot: on a random free square near the
    /// player (`place_near`, spread 6), facing the player (by quarters),
    /// at a speed from +2Dh of the floor plane (meant to be the record's
    /// speed: FS was left there), walking (34h). The slot
    /// keeps DBE at +16 and the height FF3 for flyers (record +40Dh bit
    /// 1000h, which also sets the slot's bit 8000h). The first eight get
    /// their square in the actors' squares (at 15 + FIGHTING). A flat's
    /// creature (5C8 bit 1000h, `flat_creature`) stands where the flat
    /// was instead, and may take the square's item (25CA:0233). Stops when
    /// no slot is free. None where no square is found or a square's text
    /// scan never ends.
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
            let (mut x, mut y) = (x, y);
            if self.mem.r16(DS, 0x05C8) & 0x1000 != 0 {
                // 25CA:00C3..00D9: the flat's word cleared, its number at
                // 5E2, the creature where the flat was.
                let (off, fseg) = (self.mem.r16(DS, FLAT_AT), self.mem.r16(DS, FLAT_AT + 2));
                self.mem.w16(fseg, off, 0);
                let v = self.mem.r8(DS, FLAT_NUMBER);
                self.mem.w8(DS, 0x05E2, v);
                (x, y) = (self.mem.r16(DS, FLAT_X), self.mem.r16(DS, FLAT_Y));
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
            self.flat_creature_item(di, rec)?;
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
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        self.person_outfit(seg, 0);
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
            return crate::giveup::not_ported("0000:3F0C: a guards hook not ported");
        }
        // 3F10 / 3F3F: STPTHIEF.VOC or HALT.VOC (23BA:012B).
        self.sound_file(if hook == 0x3F0C { 0x1A97 } else { 0x1A8E });
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
        // The seed's block is freed even where the port gives up (a name
        // file missing), so the heap doesn't lose it.
        let r = self.npc_seed(seed, 0, 0x10, level).and_then(|()| self.expand_npc_named(files, seed, 0, seg, rec));
        let s = self.mem.r16(DS, 0x4198);
        self.heap_free(s);
        r?;
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

    /// 1194:887A, hourly from the clock: a random encounter. Not during
    /// a quest's scene (5CA bit 200h), a fight (0FF7, 0FFB), or inside a
    /// building's square; the area's chance (`encounter_odds`, 25% on the
    /// nights of days F3h and 11Bh) against d100 picks a row of the
    /// encounter table (7 more at night), and `encounter` brings it.
    /// None where unported code is met (a quest's encounter, 5C8 bit
    /// 8000h) or a roll faults.
    pub fn random_encounter(&mut self, files: &mut Files, hooks: &mut dyn FightHooks) -> Option<()> {
        if self.mem.r16(DS, 0x05CA) & 0x200 != 0 || self.mem.r16(DS, 0x0FF7) != 0 || self.mem.r16(DS, 0x0FFB) != 0 {
            return Some(());
        }
        if self.in_building() {
            return Some(());
        }
        let night = self.night();
        self.mem.w16(DS, NIGHT, night);
        let (mut chance, row) = self.encounter_odds(night);
        if chance != 0 && self.night() != 0 {
            let day = self.mem.r32(DS, 0x05CC) % 518_400 / 1440;
            if day == 0xF3 || day == 0x11B {
                chance = 0x19;
            }
        }
        if chance == 0 {
            return Some(());
        }
        let roll = self.rng.next() % 100;
        if roll as u8 >= chance as u8 {
            return Some(());
        }
        let mut row = row;
        if self.mem.r16(DS, NIGHT) != 0 {
            row = row.wrapping_add(7);
        }
        let f = self.mem.r16(DS, 0x05C8) & !0x1000;
        self.mem.w16(DS, 0x05C8, f);
        self.mem.w16(DS, GUARD_RANK, 1);
        if row > 0xC {
            return Some(());
        }
        self.encounter(files, hooks, row)
    }

    /// 1194:8F09. The player's square is a building's (a wall word above
    /// 0 with differing bytes: a raised platform).
    pub(crate) fn in_building(&self) -> bool {
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let w = self.mem.r16(walls, self.mem.r16(DS, 0xCB90));
        (w as i16) > 0 && (w >> 8) as u8 != w as u8
    }

    /// 1194:561C. 1 at night (before 6 or from 18 o'clock).
    pub(crate) fn night(&self) -> u16 {
        let h = self.mem.r16(DS, 0x05D6);
        (!(6..0x12).contains(&h)) as u16
    }

    /// 1194:88DA. The encounter chance (d100 under it) and the table row
    /// by area kind (5E6): towns and the like by the table at 6810 (5 on
    /// with an overlay up, 1013), the wild by its terrain (A50) and night;
    /// dungeons (kind 4) by their kind (D48): 8 only with an overlay,
    /// above 8 as kind 3, below 8 only once 191D is set (a flat 14h).
    /// NIGHT gets what the table's choice leaves in AX.
    fn encounter_odds(&mut self, night: u16) -> (u16, u16) {
        let mut dx = 7u16;
        let mut bx = self.mem.r8(DS, 0x05E6) as u16;
        let overlay = self.mem.r8(DS, 0x1013) != 0;
        let mut ax = night;
        let table = |m: &mut Machine, bx: u16, ax: u16, dx: u16| {
            let chance = m.mem.r8(DS, 0x6810u16.wrapping_add(bx)) as u16;
            m.mem.w16(DS, NIGHT, ax);
            (chance, dx)
        };
        if bx as u8 == 4 {
            let kind = self.mem.r8(DS, 0x0D48) as u16;
            if kind >= 8 {
                if kind == 8 {
                    self.mem.w16(DS, NIGHT, 1);
                    if overlay {
                        return table(self, 7, 0, dx);
                    }
                    return (0, dx);
                }
                bx = 3;
            } else {
                if self.mem.r8(DS, 0x191D) == 0 {
                    return (0, dx);
                }
                if kind != 7 {
                    dx = 6;
                }
                self.mem.w16(DS, NIGHT, 0);
                return (0x14, dx);
            }
        }
        if bx >= 2 {
            ax = 0;
            if overlay {
                bx += 5;
            }
            return table(self, bx, ax, dx);
        }
        dx = 6;
        if bx == 1 {
            dx = self.mem.r8(DS, 0x0A50) as u16;
        } else {
            ax = 0;
        }
        if self.mem.r16(DS, NIGHT) != 0 {
            bx += 5;
        }
        table(self, bx, ax, dx)
    }

    /// 1194:89D4 (row 0..12; the quests' FFFFh is `quest_encounter`). By the
    /// player's level an entry of the row is chosen (`encounter_entry`);
    /// a creature a quest waits for (1194:A19C, A1D1) doesn't come, nor
    /// another kind than those already fighting; otherwise the creatures
    /// are made (`encounter_spawn`), named (`encounter_text`), and placed.
    fn encounter(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, row: u16) -> Option<()> {
        self.mem.w16(DS, 0x12AC, 0);
        let mut level = self.mem.r8(DS, 0x01AA) as u16;
        if self.mem.r16(DS, 0x05C8) & 0x8000 != 0 && self.jagar_words(hooks) {
            // 89F9..8A0A: the main quest's encounter: 12AC set, 5C8 bit
            // 8000h used up, its level the player's plus F77 (8 bits).
            self.mem.w16(DS, 0x12AC, 1);
            let f = self.mem.r16(DS, 0x05C8) & 0x7FFF;
            self.mem.w16(DS, 0x05C8, f);
            level = self.mem.r8(DS, 0x0F77).wrapping_add(self.mem.r8(DS, 0x01AA)) as u16;
        }
        let si = self.encounter_entry(level, row)?;
        let data = crate::mem::DATA;
        let ty = self.mem.r8(data, si.wrapping_add(1));
        if self.quest_matching(6, 0x30, ty).is_some() || self.quest_matching(4, 0x2F, ty).is_some() {
            return Some(());
        }
        self.encounter_bring(files, hooks, ty, si)
    }

    /// 1194:D91C. Jagar's words before a main quest's encounter: none
    /// without F77 (the carry clear); otherwise, with the view's clip
    /// (A91A) at C7h meanwhile and the view saved and put back around it,
    /// JAGAR.FLC with text 570h + F77 - 1 (its low byte added in 8 bits),
    /// A902 bit 200h set and 100h cleared, colour 9288 F5h.
    fn jagar_words(&mut self, hooks: &mut dyn FightHooks) -> bool {
        let f77 = self.mem.r8(DS, 0x0F77);
        if f77 == 0 {
            return false;
        }
        let clip = self.mem.r16(DS, 0xA91A);
        self.mem.w16(DS, 0xA91A, 0xC7);
        hooks.view_saved(self);
        let text = (0x0500 | 0x70u8.wrapping_add(f77) as u16).wrapping_sub(1);
        let f = self.mem.r16(DS, 0xA902) & !0x100 | 0x200;
        self.mem.w16(DS, 0xA902, f);
        self.mem.w8(DS, 0x9288, 0xF5);
        hooks.jagar(self, text);
        hooks.view_restored(self);
        self.mem.w16(DS, 0xA91A, clip);
        true
    }

    /// 1194:8AF0. The first creature's type: a monster's (+4), a person's
    /// class (+5) + 18h.
    pub(crate) fn encounter_current(&self) -> u8 {
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        if self.mem.r16(seg, 0x40D) & 0x1000 != 0 {
            self.mem.r8(seg, 4)
        } else {
            self.mem.r8(seg, 5).wrapping_add(0x18)
        }
    }

    /// 1194:8A27..8A46. The entry at `si` (of type `ty`) brought: not
    /// during a fight with another type (8AF0); the creatures made,
    /// named and placed.
    fn encounter_bring(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, ty: u8, si: u16) -> Option<()> {
        if self.mem.r16(DS, FIGHTING) != 0 && ty != self.encounter_current() {
            return Some(());
        }
        let n = self.encounter_spawn(files, si)?;
        let people = self.encounter_text(n);
        self.fight_prepare(files, hooks, people as u8)?;
        self.place_creatures(n)
    }

    /// 1194:89A7. A quest's encounter: unless one is under way (0FF7),
    /// the entry (`count` creatures of type `ty` at level `level`) put at
    /// 1296 and brought (89D4 with BX FFFFh: GUARD_RANK 0, so the entry
    /// is read in DGROUP).
    pub fn quest_encounter(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, count: u8, ty: u8, level: u8) -> Option<()> {
        if self.mem.r16(DS, 0x0FF7) != 0 {
            return Some(());
        }
        self.mem.w8(DS, 0x1296, count);
        self.mem.w8(DS, 0x1297, ty);
        self.mem.w8(DS, 0x1298, level);
        self.mem.w16(DS, GUARD_RANK, 0);
        self.mem.w16(DS, 0x12AC, 0);
        self.encounter_bring(files, hooks, ty, 0x1296)
    }

    /// 25CA:0271, the hook CD56: the drawing reached a flat marked as a
    /// creature's (bit 2 of its high byte, 25CA:0200) at (`x`, `y`), its
    /// map word at `seg`:`di` and `flat` its number. The word's low byte
    /// is kept (FLAT_NUMBER) unless a flat's creature is fighting (FF9 with
    /// 5C8 bit 1000h); 5CA bit 400h adds 200h; the place is kept. Within
    /// 320h of the player (the larger distance plus a quarter of the
    /// smaller), 126E `di`; outside 5E6 2 and 4, 5CA bit 40h; and when
    /// `flat` is one of the 40h at C954 the flat comes alive: 126C its
    /// place + 1, 5C8 bit 1000h, the word kept (FLAT_AT) and one creature
    /// of that kind at the player's level comes (`quest_encounter`).
    // The arguments are the hook's own (the place, number and map word).
    #[allow(clippy::too_many_arguments)]
    pub fn flat_creature(&mut self, files: &mut Files, hooks: &mut dyn FightHooks, x: u16, y: u16, flat: u8, seg: u16, di: u16) -> Option<()> {
        if self.mem.r16(DS, FIGHTING) == 0 || self.mem.r16(DS, 0x05C8) & 0x1000 == 0 {
            let v = self.mem.r8(seg, di);
            self.mem.w8(DS, FLAT_NUMBER, v);
        }
        if self.mem.r16(DS, 0x05CA) & 0x400 != 0 {
            let v = self.mem.r16(DS, 0x05CA) | 0x200;
            self.mem.w16(DS, 0x05CA, v);
        }
        self.mem.w16(DS, FLAT_X, x);
        self.mem.w16(DS, FLAT_Y, y);
        let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
        let a = abs(x.wrapping_sub(self.mem.r16(DS, 0xC3D0)));
        let b = abs(y.wrapping_sub(self.mem.r16(DS, 0xC3D4)));
        let (near, far) = if a > b { (b, a) } else { (a, b) };
        if (near >> 2).wrapping_add(far) >= 0x320 {
            return Some(());
        }
        self.mem.w16(DS, 0x126E, di);
        let area = self.mem.r8(DS, 0x05E6);
        if area != 2 && area != 4 {
            let v = self.mem.r16(DS, 0x05CA) | 0x40;
            self.mem.w16(DS, 0x05CA, v);
        }
        let Some(k) = (0..0x40u16).find(|&k| self.mem.r8(DS, 0xC954 + k) == flat) else {
            return Some(());
        };
        let kind = (k + 1) as u8;
        self.mem.w8(DS, 0x126C, kind);
        let v = self.mem.r16(DS, 0x05C8) | 0x1000;
        self.mem.w16(DS, 0x05C8, v);
        let at = self.mem.r16(DS, 0x126E);
        self.mem.w16(DS, FLAT_AT, at);
        self.mem.w16(DS, FLAT_AT + 2, seg);
        let level = self.mem.r8(DS, 0x01AA);
        self.quest_encounter(files, hooks, 1, kind, level)
    }

    /// 25CA:0233. A flat's creature (5C8 bit 1000h) placed in slot `di`
    /// (its record `rec`) takes the item of the flat's square: when the
    /// square's lookup (292B:0942, 0000:06BE) gives a `+` text (AX 3),
    /// the first free mark (MARKS, FFFFh) gets the slot, the trigger's
    /// index and the `+` number, and the trigger's square is put out.
    fn flat_creature_item(&mut self, di: u16, rec: u16) -> Option<()> {
        if self.mem.r16(DS, 0x05C8) & 0x1000 == 0 {
            return Some(());
        }
        let (x, y) = (self.mem.r16(DS, FLAT_X) >> 7, self.mem.r16(DS, FLAT_Y) >> 7);
        let key = (y << 8) | (x & 0xFF);
        let l = self.square_lookup(key, y, rec)?;
        if l.ax != 3 {
            return Some(());
        }
        let marks = crate::actors::var::MARKS;
        let Some(e) = (0..8u16).map(|k| marks + 6 * k).find(|&e| self.mem.r16(DS, e) == 0xFFFF) else {
            return Some(());
        };
        self.mem.w16(DS, e + 4, l.bx);
        self.mem.w16(DS, l.si, 0xFFFF);
        self.mem.w16(DS, e + 2, l.si.wrapping_sub(crate::actors::var::TRIGGER_SQUARES));
        self.mem.w16(DS, e, di);
        Some(())
    }

    /// 1194:8A47. The entry for level + 1 (at most 20) in row `row` of the
    /// table at 3276:40B8 (55h entries a row, each a level): a level near
    /// it (`dice`-rolled, folded back below it), whose first two entries
    /// in the row (A836, A838; as they were if fewer) are chosen between
    /// at random; for a main quest's encounter (12AC) a level below the
    /// player's level byte becomes it less a d3. Returns the entry's
    /// offset in the list at 3276:455E (3 bytes: count, type, class).
    fn encounter_entry(&mut self, level: u16, row: u16) -> Option<u16> {
        let data = crate::mem::DATA;
        let lv = (level + 1).min(0x14);
        let si = 0x40B8u16.wrapping_add(row.wrapping_mul(0x55));
        let mut v = crate::dice::roll(&mut self.rng, 4)?.wrapping_add(lv);
        let mut ax;
        loop {
            // 1194:487E
            let r = crate::dice::dice(&mut self.rng, v.wrapping_shl(1))?;
            ax = if r > v { v.wrapping_sub(r.wrapping_sub(v)) } else { r };
            let half = v >> 1;
            if ax >= half {
                break;
            }
            v = half.wrapping_add(half);
            let _ = &mut v;
        }
        if ax == 0 {
            ax = 1;
        }
        if self.mem.r8(DS, 0x01AA) == 0 && ax >= 2 {
            ax = crate::dice::roll(&mut self.rng, 3)?;
        }
        // 8A97..8AAD: a main quest's encounter (12AC) below the player's
        // level byte is brought up to it less a d3 (AL only counts).
        if self.mem.r16(DS, 0x12AC) != 0 && (ax as u8) < self.mem.r8(DS, 0x01AA) {
            let r = crate::dice::roll(&mut self.rng, 3)?;
            ax = (r.wrapping_neg() & 0xFF00) | self.mem.r8(DS, 0x01AA).wrapping_sub(r as u8) as u16;
        }
        let mut found = 0;
        for k in 0..0x55u16 {
            if self.mem.r8(data, si.wrapping_add(k)) == ax as u8 {
                self.mem.w16(DS, 0xA836 + 2 * found, k);
                found += 1;
                if found == 2 {
                    break;
                }
            }
        }
        let pick = if self.rng.next() & 1 != 0 { 0xA838 } else { 0xA836 };
        let k = self.mem.r16(DS, pick);
        Some(0x455Eu16.wrapping_add(k.wrapping_mul(3)))
    }

    /// 1194:8B8D. The entry's creatures, as many as it says but no more
    /// than the free slots: monsters (types below 18h, and 2Ah) by
    /// `spawn_monsters`, people by `spawn_people`. Returns the count.
    fn encounter_spawn(&mut self, files: &mut Files, si: u16) -> Option<u16> {
        let src = if self.mem.r16(DS, GUARD_RANK) != 0 { crate::mem::DATA } else { DS };
        let mut n = self.mem.r8(src, si) as u16;
        let free = self.usable_creature_slots();
        if free == 0 {
            return Some(0);
        }
        if free < n {
            n = free;
        }
        let ty = self.mem.r8(src, si.wrapping_add(1)) as u16;
        let f = self.mem.r16(DS, 0x05C8) & !8;
        self.mem.w16(DS, 0x05C8, f);
        if ty >= 0x18 && ty != 0x2A {
            let level = self.mem.r8(src, si.wrapping_add(2)) as u16;
            self.spawn_people(files, ty, level, n)?;
        } else {
            self.spawn_monsters(ty, n)?;
        }
        let f = self.mem.r16(DS, 0x05C8) & !8;
        self.mem.w16(DS, 0x05C8, f);
        Some(n)
    }

    /// 1194:8C13. `n` people of type `ty` (class from the table at 2A50
    /// by type - 18h) at level `level`: the first built as a named NPC,
    /// boosted, with spell points, gold and equipment (`npc_equip`); the
    /// others are copies of it with their own gold.
    fn spawn_people(&mut self, files: &mut Files, ty: u16, level: u16, n: u16) -> Option<()> {
        self.mem.w16(DS, REMAP, 0);
        self.mem.w16(DS, NEW_16, 0);
        self.mem.w8(DS, MUSIC, 0xFF);
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        let rec = self.alloc_creature_slot().map(|(r, _)| r).unwrap_or_else(|e| e);
        let class = self.mem.r8(DS, 0x2A50u16.wrapping_add(ty.wrapping_sub(0x18))) as u16;
        let seed = self.heap_alloc_high(0x10)?;
        self.mem.w16(DS, 0x4198, seed);
        // The seed's block is freed even where the port gives up.
        let r = self.npc_seed(seed, 0, class, level).and_then(|()| self.expand_npc_named(files, seed, 0, seg, rec));
        let s = self.mem.r16(DS, 0x4198);
        self.heap_free(s);
        r?;
        self.npc_level_boost(seg, rec);
        if self.mem.r16(DS, 0x05CA) & 0x40 != 0 {
            self.mem.w8(seg, rec.wrapping_add(7), 0);
        }
        self.npc_sorcerer_spell_points(seg, rec);
        self.creature_gold_at(seg, rec)?;
        self.npc_equip(seg, rec)?;
        // `dec cx` then `loop`: a count of 0 makes 65535 copies.
        let copies = n.wrapping_sub(1);
        if copies == 0 {
            return Some(());
        }
        self.mem.w16(DS, 0x12A8, rec);
        for _ in 0..copies {
            let di = self.alloc_creature_slot().map(|(r, _)| r).unwrap_or_else(|e| e);
            self.mem.w16(DS, 0x12AA, di);
            for i in 0..0x41E {
                let b = self.mem.r8(seg, rec.wrapping_add(i));
                self.mem.w8(seg, di.wrapping_add(i), b);
            }
            self.creature_gold_at(seg, di)?;
        }
        Some(())
    }

    /// 1194:8B12. Unless already fighting, the text for the first creature
    /// (a person's graphics list, `person_outfit`, whose guard bits
    /// then go to all eight records; a monster's type name at 1044).
    /// Returns whether they are people (the fight's AL).
    pub(crate) fn encounter_text(&mut self, n: u16) -> bool {
        if self.mem.r16(DS, FIGHTING) != 0 {
            return n & 0xFF != 0;
        }
        // 25CA:0170: the first creature slot in use.
        let rec = (0..8u16)
            .find(|&i| self.mem.r16(DS, CREATURE_SLOTS + i * 0x1C + 0xF) & 0x4000 == 0)
            .map_or(0, |i| i * 0x41E);
        let seg = self.mem.r16(DS, crate::creature::CREATURE_SEG);
        if self.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x1000 == 0 {
            self.person_outfit(seg, rec);
            let bits = self.mem.r16(seg, rec.wrapping_add(0x40D)) & 0xC00;
            for k in 0..8u16 {
                let at = (k * 0x41E).wrapping_add(0x40D);
                let v = self.mem.r16(seg, at) | bits;
                self.mem.w16(seg, at, v);
            }
            true
        } else {
            let mut cx = self.mem.r8(seg, rec.wrapping_add(4)) as u16;
            if cx == 0x2A {
                cx = 0x18;
            }
            let mut si = self.skip_strings(DS, 0x2DFE, cx);
            let mut di = 0x1044u16;
            loop {
                let b = self.mem.r8(DS, si);
                self.mem.w8(DS, di, b);
                si = si.wrapping_add(1);
                di = di.wrapping_add(1);
                if b == 0 {
                    break;
                }
            }
            self.mem.w8(DS, di, 0);
            false
        }
    }

    /// 24FA:0313. A person's equipment at the record `rec` (from +6Eh):
    /// unless the class wears none (1194:7757), four armour pieces (types
    /// at 4F10) and up to three more by level (4F14, at levels 5, 10, 15),
    /// each of "level" d100 (the roll left in AL); a weapon by class
    /// (`npc_weapon`, rerolled over the same place while it comes out as
    /// type 0 for a class that isn't 0); and a shield unless the weapon is
    /// two-handed (by class, the list at 3276:3C71).
    pub fn npc_equip(&mut self, seg: u16, rec: u16) -> Option<()> {
        let class = self.mem.r8(seg, rec.wrapping_add(5)) as u16;
        let mut di = rec.wrapping_add(0x6E);
        if !self.class_wears_no_armor(class) {
            let level = self.mem.r8(seg, rec.wrapping_add(6)) as u16;
            let q = self.npc_armor_quality(seg, rec);
            self.mem.w8(DS, 0x101E, q as u8);
            for b in 0..4 {
                let forced = self.mem.r8(DS, 0x4F10 + b);
                di = self.npc_armor(forced, di);
            }
            let mut dx = 5u16;
            for b in 0..3 {
                if level < dx {
                    break;
                }
                let forced = self.mem.r8(DS, 0x4F14 + b);
                di = self.npc_armor(forced, di);
                dx += 5;
            }
        }
        let mut bx = class;
        let mut tries = 0u32;
        let (ax, weapon) = loop {
            // The original rerolls without end for a class whose weapons
            // all come out as type 0.
            tries += 1;
            if tries > 0x10000 {
                return crate::giveup::original("24FA:0313: no equipment found: the original rolls for ever");
            }
            let (ax, w, next) = self.npc_weapon(bx, di)?;
            if w == 0 && ax != 0 {
                bx = ax;
                di = next.wrapping_sub(0x13);
                continue;
            }
            di = next;
            break (ax, w);
        };
        // 24FA:0405: the shield.
        if self.mem.r8(DS, 0x229Eu16.wrapping_add(weapon)) == 2 {
            return Some(());
        }
        let data = crate::mem::DATA;
        let list = self.mem.r16(data, 0x3C71u16.wrapping_add(ax.wrapping_add(ax)));
        let Some(forced) = self.pick_from_list(list)? else {
            return Some(());
        };
        self.mem.w8(DS, 0x101E, 0);
        self.npc_armor(forced as u8, di);
        Some(())
    }

    /// 1194:7C1D. How good a person's armour is (0..2, into 101E): one
    /// less for each d100 at most level x 10 (classes 10h and 11h roll
    /// once; for the classes in the list at 5D78 the level is read at
    /// the list's place + 6 instead of the record's, in the record's
    /// segment; others get 2).
    fn npc_armor_quality(&mut self, seg: u16, rec: u16) -> u16 {
        let class = self.mem.r8(seg, rec.wrapping_add(5));
        let mut dx = 2u16;
        let at = if class == 0x10 || class == 0x11 {
            dx -= 1;
            rec
        } else {
            match (0..4u16).find(|&k| self.mem.r8(DS, 0x5D78 + k) == class) {
                Some(k) => 0x5D78 + k,
                None => return dx,
            }
        };
        let threshold = self.mem.r8(seg, at.wrapping_add(6)) as u16 * 10;
        let roll = self.rng.next() % 100;
        if roll <= threshold {
            dx -= 1;
        }
        dx
    }

    /// 24FA:0453. A piece of plain armour of type `forced` (at "level" the
    /// d100 roll: AL holds it) put at `di`; returns the next place.
    fn npc_armor(&mut self, forced: u8, di: u16) -> u16 {
        let roll = self.rng.next() % 100;
        self.loot_armor(roll as u8, forced);
        self.put_item(di)
    }

    /// 24FA:03AA. A weapon for class `bx` (&1Fh) at `di`: class 0Dh takes
    /// type 11h; otherwise a type from the class's list (3276:3C25) or,
    /// with none, 2..16 at random; magic on a d100 of 2 or less (the
    /// roll is its level, AL). Returns (the class, the type, the next
    /// place).
    fn npc_weapon(&mut self, bx: u16, di: u16) -> Option<(u16, u16, u16)> {
        let class = bx & 0x1F;
        let data = crate::mem::DATA;
        let ty = if class == 0xD {
            0x11
        } else {
            let list = self.mem.r16(data, 0x3C25u16.wrapping_add(class.wrapping_add(class)));
            if list == 0 {
                crate::dice::roll(&mut self.rng, 0xF)?.wrapping_add(2)
            } else {
                self.pick_from_list(list)?.unwrap_or(0)
            }
        };
        self.mem.w16(DS, GUARD_RANK, ty);
        let roll = self.rng.next() % 100;
        let ty = self.mem.r16(DS, GUARD_RANK);
        if roll <= 2 {
            self.loot_magic_weapon(roll as u8, ty)?;
        } else {
            self.loot_weapon(roll as u8, ty as u8)?;
        }
        let next = self.put_item(di);
        Some((class, self.mem.r16(DS, GUARD_RANK), next))
    }

    /// 24FA:0434. A random byte of the FFh-terminated list at 3276:`si`,
    /// None (the original's carry) for an empty list.
    fn pick_from_list(&mut self, si: u16) -> Option<Option<u16>> {
        let data = crate::mem::DATA;
        let mut n = 0u16;
        while self.mem.r8(data, si.wrapping_add(n)) != 0xFF {
            n = n.wrapping_add(1);
            if n == 0 {
                return crate::giveup::original("24FA:0434: the list has no end: the original reads on for ever");
            }
        }
        if n == 0 {
            return Some(None);
        }
        let k = crate::dice::roll(&mut self.rng, n)?;
        Some(Some(self.mem.r8(data, si.wrapping_add(k)) as u16))
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
                return crate::giveup::original("0000:1B19: no free square near the player: the original keeps trying");
            }
            spread = (spread & 0xFF00) | ((spread as u8) >> 1) as u16;
        }
    }

    /// 0000:468B..46A2, the hook CD42's head: the flats past the first 10h
    /// (C38E - 10h of them, 5-byte records from CD9E) flagged 4 in their
    /// fourth byte, then 0000:50B9 (`clear_dungeon_flats`). (0000:4DD6
    /// follows, `interface_setup`.)
    pub fn entered_flats(&mut self) {
        let n = self.mem.r16(DS, 0xC38E).wrapping_sub(0x10);
        let mut si = 0xCD9Eu16;
        for _ in 0..n {
            let v = self.mem.r8(DS, si.wrapping_add(3)) | 4;
            self.mem.w8(DS, si.wrapping_add(3), v);
            si = si.wrapping_add(5);
        }
        self.clear_dungeon_flats();
    }

    /// 0000:50B9. In a dungeon (kind 4) once 191D is set, flats (type 8)
    /// that are C943, or one of the eight at C987, are cleared from the
    /// map.
    pub(crate) fn clear_dungeon_flats(&mut self) {
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
