//! The routines of segment 0000 the game installs as hooks (far pointers
//! in DGROUP), recognised by their value where the hook is called.
//!
//! The 3D mode (2294:0006, `Machine::mode_3d`) and an area's run set
//! them: the walk's test (CD6A: 0000:4009), the transition (C3AC:
//! 0000:4470, a RETF), the stairs (CD6E: 0000:51B8), the level entered
//! after stairs (C3B8: 0000:44D7), the INF's end (CD2E: 0000:4531) and a
//! level changed (CD32: 0000:0B0B, or 2957 in a made dungeon). The port
//! keeps the image at segment 0, so these have segment 0 here and are
//! told from an unset hook by the whole pointer (`Machine::hook_set`).
//! Any other value set at those hooks is not ported (None).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// The hooks' values: segment 0000 routines, as (segment << 16) | offset.
pub mod value {
    /// 0000:4009, the walk's test (CD6A).
    pub const WALK_TEST: u32 = 0x4009;
    /// 0000:4470, the transition (C3AC): a RETF.
    pub const TRANSITION: u32 = 0x4470;
    /// 0000:51B8, the stairs (CD6E).
    pub const STAIRS: u32 = 0x51B8;
    /// 0000:44D7, a dungeon's level entered by stairs (C3B8).
    pub const LEVEL_ENTERED: u32 = 0x44D7;
    /// 0000:4531, the INF's end (CD2E).
    pub const INF_DONE: u32 = 0x4531;
    /// 0000:0B0B and 0000:2957, a level changed (CD32): the level's
    /// lights, or a made dungeon's level built.
    pub const LEVEL_LIGHTS: u32 = 0x0B0B;
    pub const DUNGEON_LEVEL: u32 = 0x2957;
}

/// What the stairs' hooks (0000:51B8, 44D7) leave to the front end and
/// to code not ported.
pub trait StairsHooks {
    /// 2294:0B8A's I/O (`light::LightHooks::light_table`): the light
    /// table freed, NORMAL.LGT loaded and the table made again.
    fn light_table(&mut self, _m: &mut Machine) {}
    /// 292B:095E: a main quest dungeon's level's locks and triggers kept
    /// in STATES.nn (`Machine::states_keep`), before the level changes.
    fn automap_keep(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 292B:09AC: the new level's locks and triggers loaded from
    /// STATES.nn (`Machine::states_load`).
    fn automap_load(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 1194:9FAC with CX 1: a quest dungeon's last level reached
    /// (`Machine::quest_level`: a side quest's item, or its person and
    /// their name, placed). None where the original hangs or faults.
    fn quest_level(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_level(files, &mut crate::quest::NoQuestHooks).map(|_| ())
    }
    /// 2433:0000 (4C91 FFh): the level's piles of treasure made
    /// (`Machine::piles`). None where the original faults or hangs.
    fn level_piles(&mut self, m: &mut Machine) -> Option<()> {
        m.piles()
    }
    /// 0000:368E: the automap kept (23F1:013C, `Machine::automap_keep`),
    /// then loaded (0000:3503, `Machine::automap_load`).
    fn level_screen(&mut self, _m: &mut Machine, _files: &mut Files) {}
}

/// `StairsHooks` that do nothing.
pub struct NoStairsHooks;

impl StairsHooks for NoStairsHooks {}

/// `light::LightHooks` for 2294:0B8A, from the stairs' hooks.
struct StairsLight<'a>(&'a mut dyn StairsHooks);

impl crate::light::LightHooks for StairsLight<'_> {
    fn light_table(&mut self, m: &mut Machine) {
        self.0.light_table(m);
    }
}

/// The walls of the level (map plane 1) and its floors.
fn walls(m: &Machine) -> u16 {
    m.mem.r16(DS, crate::mif::var::MAP1_SEG)
}

impl Machine {
    /// 0000:4009. Whether the walk's test refuses the position the
    /// registers hold (the carry): AX x, BX y, made into a map square word
    /// as the original makes it (the column doubled in 8 bits, the row in
    /// the high byte) and looked for among the 8 words at 196F. Nothing
    /// writes those words, so in the game they are 0: square 0 (and its
    /// wrapped twins) is refused. See `movement::move_test` for what AX
    /// and BX hold.
    pub fn walk_test(&self, ax: u16, bx: u16) -> bool {
        let al = (((ax >> 7) as u8).wrapping_add((ax >> 7) as u8)) as u16;
        let word = ((bx >> 7) & 0xFF) << 8 | al;
        (0..8).any(|i| self.mem.r16(DS, 0x196F + 2 * i) == word)
    }

    /// 0000:51B8, the stairs' hook (CD6E): the light table set for clear
    /// weather (2294:0B8A, `light_table_reset`), the level's automap kept
    /// (292B:095E) and 3C82 cleared.
    pub fn stairs_hook(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks) {
        self.light_table_reset(&mut StairsLight(&mut *hooks));
        hooks.automap_keep(self, files);
        self.mem.w8(DS, 0x3C82, 0);
    }

    /// 29CF:0CEF. The spells' state cleared: the projectiles' slots'
    /// +11h (12 of 1Ch at 7DBC), 7DB8 and D5A, and bit 0 of +0Ah of the
    /// casts (8 of 16h at 7F86).
    pub(crate) fn spells_clear(&mut self) {
        for i in 0..12u16 {
            self.mem.w8(DS, 0x7DBC + i * 0x1C + 0x11, 0);
        }
        self.mem.w16(DS, 0x7DB8, 0);
        self.mem.w16(DS, 0x0D5A, 0);
        for i in 0..8u16 {
            let at = 0x7F86 + i * 0x16 + 0xA;
            let v = self.mem.r16(DS, at) & !1;
            self.mem.w16(DS, at, v);
        }
    }

    /// 0000:44D7, a dungeon's level entered by stairs (the hook C3B8,
    /// after the level is loaded): the spells' state cleared (29CF:0CEF),
    /// 7A46 cleared, the creatures (slots 15..22 at 0C57) still about
    /// forgotten (0000:269E) and made inactive, the new level's automap
    /// (292B:09AC), the quest's item placed (2294:0E90), the quest's
    /// level (1194:9FAC), its piles of treasure (4C91 FFh, 2433:0000), the
    /// screen
    /// (0000:368E), a broken-into interior's people gone (0000:50B9), a
    /// quest's flat cleared (2294:119C), the floor's marks cleared
    /// (0000:47D7) and the player freed from a wall (0000:2631).
    pub fn level_entered(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks) -> Option<()> {
        self.spells_clear();
        self.mem.w16(DS, 0x7A46, 0);
        for i in 0..8u16 {
            let si = 0x0C57 + i * 0x1C;
            let f = self.mem.r16(DS, si + 0xF);
            if f & 0x4000 == 0 {
                self.actor_forget(si);
                let f = self.mem.r16(DS, si + 0xF) | 0x4000;
                self.mem.w16(DS, si + 0xF, f);
            }
        }
        hooks.automap_load(self, files);
        self.quest_item_place();
        hooks.quest_level(self, files)?;
        self.mem.w8(DS, 0x4C91, 0xFF);
        hooks.level_piles(self)?;
        hooks.level_screen(self, files);
        self.clear_dungeon_flats();
        self.quest_flat_clear();
        self.floor_marks_clear();
        self.player_unstick();
        Some(())
    }

    /// 0000:4767..47D6, the hook CD42's (0000:468B) part indoors (C398
    /// bit 4 clear), unless a game was just loaded (5CA bit 4000h): in a
    /// building (5E6 4) a noble's house furnished (D48 8, 0000:5160) or a
    /// house's flats marked (D48 1, 513A); the level's locks and
    /// triggers (292B:09AC), its lights (0000:081A, 7A46 cleared), its
    /// listed flats marked (25CA:0200), the main quest's item placed
    /// (2294:0E90), a side quest's level (1194:9FAC with CX 1), its piles
    /// of treasure (4C91 FFh, 2433:0000), a quest's flat cleared
    /// (2294:119C) and the floor's marks (0000:47D7); in the Imperial
    /// City's palace (A47 3, A4C's low 5 bits 0, A46 4) the screen
    /// (0000:368E). None where a callee's original hangs or faults.
    pub fn area_entered_inside(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks) -> Option<()> {
        if self.mem.r16(DS, 0x05CA) & 0x4000 != 0 {
            return Some(());
        }
        if self.mem.r8(DS, 0x05E6) == 4 {
            if self.mem.r8(DS, 0x0D48) == 8 {
                self.noble_house_furnish()?;
            }
            if self.mem.r8(DS, 0x0D48) == 1 {
                self.house_flats_mark();
            }
        }
        hooks.automap_load(self, files);
        self.flat_lights();
        self.mem.w16(DS, 0x7A46, 0);
        self.inf_done_hook();
        self.quest_item_place();
        hooks.quest_level(self, files)?;
        self.mem.w8(DS, 0x4C91, 0xFF);
        hooks.level_piles(self)?;
        self.quest_flat_clear();
        self.floor_marks_clear();
        if self.mem.r8(DS, 0x0A47) == 3 && self.mem.r8(DS, 0x0A4C) & 0x1F == 0 && self.mem.r8(DS, 0x0A46) == 4 {
            hooks.level_screen(self, files);
        }
        Some(())
    }

    /// 0000:5160. A noble's house furnished: with the location's key
    /// (0000:386F, `automap_key`) as the RNG's seed (the seed put back
    /// after), a d1Eh picks an entry of the table at 8732 (two bytes: the
    /// count, and the kind's index + 1 into the flats at C954); then for
    /// each of the count (`loop`: 0 runs 65536 times) a random square of
    /// the C765 listed at C6E5 (FFFFh ones rolled again) gets the flat
    /// 82xxh, its column byte doubled in 8 bits. None where the roll
    /// divides by zero (no squares listed) or can only find FFFFh ones.
    fn noble_house_furnish(&mut self) -> Option<()> {
        let saved = self.rng.seed;
        self.rng.seed = self.automap_key();
        let r = self.rng.range(0, 0x1E)?;
        let at = 0x8732u16.wrapping_add(r.wrapping_mul(2));
        let count = self.mem.r8(DS, at) as u32;
        let kind = self.mem.r8(DS, at.wrapping_add(1)).wrapping_sub(1);
        let flat = self.mem.r8(DS, 0xC954u16.wrapping_add(kind as u16));
        self.rng.seed = saved;
        let n = if count == 0 { 0x10000 } else { count };
        let seg = walls(self);
        for _ in 0..n {
            let mut cycle = crate::random::CycleCheck::new(&self.rng);
            let w = loop {
                let r = self.rng.range(0, self.mem.r8(DS, 0xC765) as u16)?;
                let w = self.mem.r16(DS, 0xC6E5u16.wrapping_add(r.wrapping_mul(2)));
                if w != 0xFFFF {
                    break w;
                }
                if cycle.cycled(&self.rng) {
                    return crate::giveup::original("0000:5160: no square for the noble's house: the original rolls for ever");
                }
            };
            let bx = w & 0xFF00 | (w as u8).wrapping_add(w as u8) as u16;
            self.mem.w16(seg, bx, 0x8200 | flat as u16);
        }
        Some(())
    }

    /// 0000:513A. A house's flats marked: unless A52's low two bits are
    /// set, each flat in the map (type 8) numbered 0 becomes 1 (its low
    /// byte incremented), over the plane's 4000h words.
    fn house_flats_mark(&mut self) {
        if self.mem.r8(DS, 0x0A52) & 3 != 0 {
            return;
        }
        let seg = walls(self);
        for i in 0..0x4000u16 {
            let w = self.mem.r16(seg, i * 2);
            if w >> 12 == 8 && w as u8 == 0 {
                self.mem.w8(seg, i * 2, 1);
            }
        }
    }

    /// 2294:0E90. In a dungeon (5E6 3) on its last level, when the place
    /// (A46, A4C's low 5 bits) is one of the quest's two (FE3/FE2,
    /// FE5/FE4), the quest's item is placed (2294:0F11).
    pub fn quest_item_place(&mut self) {
        if self.mem.r8(DS, 0x05E6) != 3 {
            return;
        }
        let last = self.mem.r8(DS, crate::movement::var::LEVELS).wrapping_sub(1);
        if self.mem.r8(DS, crate::movement::var::LEVEL) != last {
            return;
        }
        let place = self.mem.r8(DS, 0x0A46);
        let sub = self.mem.r8(DS, 0x0A4C) & 0x1F;
        let at = |m: &Self, a: u16, b: u16| place == m.mem.r8(DS, a) && sub == m.mem.r8(DS, b);
        if at(self, 0x0FE3, 0x0FE2) || at(self, 0x0FE5, 0x0FE4) {
            self.quest_item_put();
        }
    }

    /// 2294:0F11. The quest's item (flat C93B) put on the last of the
    /// squares listed at C6E5 (count C765, taken), its column byte
    /// doubled in 8 bits; the square kept at FEE.
    fn quest_item_put(&mut self) {
        let n = self.mem.r8(DS, 0xC765).wrapping_sub(1);
        self.mem.w8(DS, 0xC765, n);
        let w = self.mem.r16(DS, 0xC6E5u16.wrapping_add(2 * n as u16));
        let bx = w & 0xFF00 | ((w as u8) << 1) as u16;
        let v = 0x8000 | self.mem.r8(DS, 0xC93B) as u16;
        self.mem.w16(walls(self), bx, v);
        self.mem.w16(DS, 0x0FEE, bx);
    }

    /// 2294:119C. In a city (A47 3) but the Imperial City (A46 8), the
    /// flat C941 (8000h | C941) looked for row by row (C359 rows of C357
    /// words); when found, unless the place is the quest's (A46 the byte
    /// at 80ECh + F77) and the quest isn't on (A4C's low 5 bits 0, or F7D
    /// 0), a word is cleared: the last of the row past the one it was
    /// found in (the original restores DI before adding the row, so
    /// DI - 2 is the row's word FEh, kept).
    pub fn quest_flat_clear(&mut self) {
        if self.mem.r8(DS, 0x0A47) != 3 || self.mem.r8(DS, 0x0A46) == 8 {
            return;
        }
        let want = 0x8000 | self.mem.r8(DS, 0xC941) as u16;
        let seg = walls(self);
        let count = |n: u16| if n == 0 { 0x10000u32 } else { n as u32 };
        let (rows, width) = (self.mem.r16(DS, 0xC359), self.mem.r16(DS, 0xC357));
        // ZF: set by `xor di, di` before the scan (a row of no words leaves
        // it as it was).
        let mut found = true;
        let mut di = 0u16;
        let mut cx = count(rows);
        loop {
            let mut at = di;
            let mut left = width;
            while left != 0 {
                let w = self.mem.r16(seg, at);
                at = at.wrapping_add(2);
                left -= 1;
                found = w == want;
                if found {
                    break;
                }
            }
            di = di.wrapping_add(0x100);
            cx -= 1;
            if cx == 0 || found {
                break;
            }
        }
        if !found {
            return;
        }
        let f77 = self.mem.r8(DS, 0x0F77) as u16;
        let place = self.mem.r8(DS, 0x0A46);
        if place == self.mem.r8(DS, 0x80ECu16.wrapping_add(f77))
            && (self.mem.r8(DS, 0x0A4C) & 0x1F == 0 || self.mem.r8(DS, 0x0F7D) == 0)
        {
            return;
        }
        self.mem.w16(seg, di.wrapping_sub(2), 0);
    }

    /// 0000:47D7. Floor marks cleared: each floor word (16K, of the
    /// floor plane at CCD0) whose low byte is one of the 40h at C954 has
    /// that byte cleared.
    pub fn floor_marks_clear(&mut self) {
        let seg = self.mem.r16(DS, crate::movement::var::FLOOR_SEG);
        for i in 0..0x4000u16 {
            let al = self.mem.r8(seg, 2 * i);
            if al != 0 && (0..0x40).any(|k| self.mem.r8(DS, 0xC954 + k) == al) {
                self.mem.w8(seg, 2 * i, 0);
            }
        }
    }

    /// 0000:2631. A player standing (height not below 0) in a solid wall
    /// (the square's word nonzero with equal bytes) is moved to the first
    /// of the 9 squares around (offsets at DAA) that is empty, to its
    /// centre; the place is kept at BA62, BA66. None empty: left there.
    pub fn player_unstick(&mut self) {
        if (self.mem.r16(DS, crate::movement::var::HEIGHT) as i16) < 0 {
            return;
        }
        let seg = walls(self);
        let bx = self.mem.r16(DS, 0xCB90);
        let w = self.mem.r16(seg, bx);
        if w == 0 || w >> 8 != w & 0xFF {
            return;
        }
        let Some(bx) = (0..9u16)
            .map(|i| bx.wrapping_add(self.mem.r16(DS, 0x0DAA + 2 * i)))
            .find(|&b| self.mem.r16(seg, b) == 0)
        else {
            return;
        };
        let x = (((bx >> 1) & 0x7F) << 7).wrapping_add(0x40);
        let y = ((bx >> 8) << 7).wrapping_add(0x40);
        self.mem.w16(DS, crate::movement::var::X, x);
        self.mem.w16(DS, crate::movement::var::Y, y);
        self.mem.w16(DS, 0xBA62, x);
        self.mem.w16(DS, 0xBA66, y);
    }

    /// 0000:4531, the INF's end (CD2E), calling 25CA:0200: each flat
    /// standing in the map (type 8) whose number is one of the 32h at C954
    /// gets bit 200h.
    pub fn inf_done_hook(&mut self) {
        let seg = walls(self);
        for i in 0..0x4000u16 {
            let hi = self.mem.r8(seg, 2 * i + 1);
            if hi >> 4 != 8 {
                continue;
            }
            let al = self.mem.r8(seg, 2 * i);
            if (0..0x32).any(|k| self.mem.r8(DS, 0xC954 + k) == al) {
                self.mem.w8(seg, 2 * i + 1, hi | 2);
            }
        }
    }

    /// 0000:2957, a made dungeon's level changed (CD32): the quest's
    /// square count (C765) cleared, the level BB0A built (0000:299F,
    /// `dungeon_build_level`), its lights made (0000:081A with 7A46
    /// cleared, `flat_lights`) and its INF (4718) to be loaded.
    pub fn dungeon_level_hook(&mut self, files: &mut Files) -> Option<()> {
        self.dungeon_level_with(|m, level| m.dungeon_build_level(files, level))
    }

    /// 0000:2974, a made dungeon entered (the place hook 0000:45B2 calls
    /// it when entering, 5C8 bit 800h, with C39A bit 800h, which the
    /// dungeon's set-up sets): its level built (0000:2957,
    /// `dungeon_level_hook`) and the player put at its start
    /// (0000:2978, `dungeon_start_position`: the start cell's stairs).
    pub fn dungeon_first_level(&mut self, files: &mut Files) -> Option<()> {
        self.dungeon_level_hook(files)?;
        self.dungeon_start_position();
        Some(())
    }

    /// `dungeon_first_level` without the file I/O, as `dungeon_level`
    /// builds a level (random1.mif's file handle `handle`): returns the
    /// chunks in load order.
    pub fn dungeon_first_level_plan(&mut self, handle: u16) -> Option<Vec<crate::dungeon::Chunk>> {
        let mut chunks = None;
        self.dungeon_level_with(|m, level| {
            chunks = Some(m.dungeon_level(level, handle)?);
            Some(())
        })?;
        self.dungeon_start_position();
        chunks
    }

    /// 0000:2957 with the level's build (0000:299F) `build`.
    fn dungeon_level_with(&mut self, build: impl FnOnce(&mut Self, u8) -> Option<()>) -> Option<()> {
        self.mem.w8(DS, 0xC765, 0);
        let level = self.mem.r8(DS, crate::movement::var::LEVEL);
        build(self, level)?;
        self.flat_lights();
        self.mem.w16(DS, crate::info::var::INFO_PTR, 0x4718);
        Some(())
    }
}
