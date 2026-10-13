//! The world loop's frame body (0000:481E, hook C3A8): what happens each
//! frame besides the actors and the player's physics.
//!
//! In order: the speed and level keys, the acceleration's decay without
//! input, the walking and turning speeds from the player's speed and
//! fatigue (2294:1125, also the speed of creatures in a fight), the
//! wilderness flats swapped on entering or leaving the wild (0000:3F7E),
//! levitation and the light radius from the player's spell flags, the
//! weapon (class change 0000:3C4C, then `weapon_frame`), the message's
//! countdown, the guards' countdown (0000:3EFA), the debug keys, the
//! player held under water (2826:0D5F), a holiday's message (2DA2:111C),
//! breath and drowning in deep water, the night's creatures in towns
//! (0000:1793), and the clock: a minute every 90 timer ticks.
//!
//! Drawing (the status bars, compass, automap, keys, hand bobbing, hurt
//! flash's fill, the text over the view, the sky's palette), sound, the
//! menus and message boxes are the front end's: they are hooks
//! (`FrameHooks`) or left out. The original's extra wait for its detail
//! setting (37E9) is the front end's pacing.

use crate::bsa::Files;
use crate::clock::ClockHooks;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{LEVEL_FLAGS, MAP_FLAGS};
use crate::weapon::WeaponHooks;

/// DGROUP variables.
pub mod var {
    /// The BIOS timer's tick count (18.2 a second) and the tick of the
    /// last game minute.
    pub const TICKS: u16 = 0x0151;
    pub const MINUTE_TICK: u16 = 0x884A;
    /// Keys pressed already reported (one byte per scan code).
    pub const KEYS_SEEN: u16 = 0x9208;
    /// The detail setting (extra ticks a frame, 0..9).
    pub const DETAIL: u16 = 0x37E9;
    /// The area flags seen last frame (C39A).
    pub const LAST_AREA: u16 = 0x8848;
    /// The light radius.
    pub const LIGHT: u16 = 0xC3E2;
    /// The guards' countdown (byte) and the routine called at 0 (far).
    pub const GUARDS: u16 = 0x680F;
    pub const GUARDS_HOOK: u16 = 0x67EA;
    /// Hurt flash: the table (offset, 0 none) and its step.
    pub const FLASH: u16 = 0x194B;
    pub const FLASH_STEP: u16 = 0x1949;
}
use var::*;

/// What the frame body leaves to the front end or to code not ported.
pub trait FrameHooks: WeaponHooks + ClockHooks {
    /// 1194:59C8: a message box with the text at DGROUP `text`.
    fn message_box(&mut self, _m: &mut Machine, _text: u16) {}
    /// 1194:A36C: the message box of text number [137A].
    fn message_box_number(&mut self, _m: &mut Machine) {}
    /// The routine at GUARDS_HOOK (0000:3F0C or 3F3B): the guards arrive
    /// (`encounter::guards_hook`, with the fight's graphics, music and
    /// text left out; override to draw them).
    fn guards_arrive(&mut self, m: &mut Machine, files: &mut Files, hook: u32) -> Option<()> {
        m.guards_hook(files, &mut crate::encounter::NoFightHooks, hook)
    }
    /// 1194:DB6C: a click left for the actors (talk, loot, steal).
    fn actor_click(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:3C4C's loading of the hands' graphic for weapon class [1012].
    fn weapon_graphics(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:17F9: the night creatures' graphics are loaded.
    fn night_graphics(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 2542:0100: the menu keys (Esc, the character sheet, the map...).
    fn menu_keys(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:4D00 (1194:9F8A, quests): its carry, which holds back the
    /// quest message of 5CA bit 200h.
    fn quest_carry(&mut self, _m: &mut Machine) -> bool {
        false
    }
    /// 0000:4ACD..4B35's drawing: the town's welcome, and the wait for a
    /// key (1194:5532).
    fn town_welcome(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 1194:D327 (keys ACED and ACF4).
    fn key_d327(&mut self, _m: &mut Machine) {}
}

/// `FrameHooks` that do nothing.
pub struct NoFrameHooks;

impl WeaponHooks for NoFrameHooks {}
impl ClockHooks for NoFrameHooks {
    fn encounter(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
}
impl FrameHooks for NoFrameHooks {}

impl Machine {
    /// 2542:05B8. Whether the key `scan` was pressed since last asked:
    /// held now and not yet reported.
    pub fn key_pressed(&mut self, scan: u16) -> bool {
        if self.mem.r8(DS, crate::controls::var::KEYS.wrapping_add(scan)) == 0 {
            self.mem.w8(DS, KEYS_SEEN.wrapping_add(scan), 0);
            return false;
        }
        if self.mem.r8(DS, KEYS_SEEN.wrapping_add(scan)) != 0 {
            return false;
        }
        self.mem.w8(DS, KEYS_SEEN.wrapping_add(scan), 1);
        true
    }

    fn message(&mut self, time: u16, text: u16) {
        self.mem.w16(DS, 0x061E, time);
        self.mem.w16(DS, 0x061A, text);
        self.mem.w16(DS, 0x061C, DS);
    }

    /// 0000:481E up to the detail wait (4CE7). None where a ported part
    /// fails.
    pub fn frame_body(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) -> Option<()> {
        // The detail keys (- and =) and F4 (5CA bit 2).
        if self.key_pressed(0x0D) {
            let d = self.mem.r8(DS, DETAIL).wrapping_sub(1);
            if (d as i8) < 0 {
                self.mem.w8(DS, DETAIL, 0);
            } else {
                self.mem.w8(DS, DETAIL, d);
                self.message(0x0A, 0x8925);
            }
        }
        if self.key_pressed(0x0C) {
            let d = self.mem.r8(DS, DETAIL).wrapping_add(1);
            if (d as i8) >= 0x0A {
                self.mem.w8(DS, DETAIL, 9);
            } else {
                self.mem.w8(DS, DETAIL, d);
                self.message(0x0A, 0x8938);
            }
        }
        if self.key_pressed(0x3E) {
            let f = self.mem.r16(DS, 0x05CA) ^ 2;
            self.mem.w16(DS, 0x05CA, f);
            self.message(0x0A, 0x8911);
        }
        // No movement input: the acceleration decays.
        if self.mem.r8(DS, crate::controls::var::BUTTONS) & 1 == 0
            && self.mem.r16(DS, 0x05E0) & 0x2000 == 0
            && self.mem.r8(DS, crate::controls::var::JUMP) == 0
        {
            let a = self.mem.r16(DS, crate::controls::var::ACCEL).wrapping_sub(0x40);
            self.mem.w16(DS, crate::controls::var::ACCEL, if (a as i16) < 0 { 0 } else { a });
        }
        let f = self.mem.r16(DS, 0x05CA) & !0x4000;
        self.mem.w16(DS, 0x05CA, f);
        self.area_change();
        // Levitation (spell flag 80h) and light (100h, the timed flag's
        // strength from the table at [1020]).
        let spells = self.mem.r16(DS, 0x05BE);
        let f = self.mem.r16(DS, LEVEL_FLAGS) & !1 | (spells & 0x80 != 0) as u16;
        self.mem.w16(DS, LEVEL_FLAGS, f);
        let mut light = 0x140u16;
        if spells & 0x100 != 0 {
            let (off, seg) = (self.mem.r16(DS, 0x1020), self.mem.r16(DS, 0x1022));
            light = (self.mem.r16(seg, off.wrapping_add(0x22)) << 8).wrapping_add(0x200);
        }
        self.mem.w16(DS, LIGHT, light);
        // 0000:4903: a pending message box.
        if self.mem.r8(DS, 0x191F) != 0 {
            self.mem.w16(DS, 0xCB5E, 0xFFFF);
            self.mem.w8(DS, 0x191F, 0);
            self.mem.w8(DS, 0x101C, 0);
            self.mem.w8(DS, 0x137E, 0);
            hooks.message_box_number(self);
            self.mem.w16(DS, 0xCB5E, 0);
        }
        // 292B:032A: a one-time note for classes without bit 20h.
        if self.mem.r8(DS, 0x01A9) & 0x20 == 0 && self.mem.r16(DS, 0x05CA) & 8 == 0 {
            let f = self.mem.r16(DS, 0x05CA) | 8;
            self.mem.w16(DS, 0x05CA, f);
        }
        self.weapon_change(files, hooks)?;
        self.weapon_frame(files, hooks)?;
        // 0000:0511: the message's countdown (and its drawing's colour
        // and font, 9288 and B036).
        let t = self.mem.r16(DS, 0x061E);
        if t != 0 {
            self.mem.w16(DS, 0x061E, t - 1);
            let v = self.mem.r16(DS, 0x102C);
            self.mem.w16(DS, 0xB036, v);
            self.mem.w8(DS, 0x9288, 0x9F);
        }
        self.speeds();
        // 0000:3EFA: the guards.
        let g = self.mem.r8(DS, GUARDS);
        if g != 0 {
            self.mem.w8(DS, GUARDS, g - 1);
            if g == 1 {
                let hook = self.mem.r32(DS, GUARDS_HOOK);
                hooks.guards_arrive(self, files, hook)?;
            }
        }
        hooks.menu_keys(self, files)?;
        // Debug keys: F2 shows the player's square, F3 cycles 5E4's
        // flash mode.
        if self.key_pressed(0x3C) {
            let sq = self.mem.r16(DS, 0xCB90);
            let col = (sq as u8 >> 1) as i8 as i16 as u16;
            use crate::format::Arg;
            self.sprintf((DS, 0x7979), (DS, 0x8703), &[Arg::Int(col), Arg::Int(sq >> 8)]);
            self.message(0x0F, 0x7979);
        }
        if self.key_pressed(0x3D) {
            let mut al = (self.mem.r8(DS, 0x05E4) >> 1).wrapping_add(1);
            if al >= 3 {
                al = 0;
            }
            let b = self.mem.r8(DS, 0x05E4) & 0xF9 | al << 1;
            self.mem.w8(DS, 0x05E4, b);
            let text = self.skip_strings(DS, 0x8816, al as u16);
            self.message(0x0A, text);
        }
        // 2826:0D5F: held under water in the wild, the player drowns.
        let level = self.mem.r16(DS, LEVEL_FLAGS);
        if self.mem.r16(DS, 0x05B1) & 0x40 != 0 && level & 0x10 != 0 && level & 0x4000 == 0 {
            hooks.message_box(self, 0x7927);
            self.player_dies();
        }
        hooks.actor_click(self, files)?;
        // A holiday's message, once, in a town by day.
        if self.mem.r16(DS, 0xCCBC) > 3 {
            if let Some(n) = holiday(self, self.mem.r32(DS, 0x05CC)) {
                if self.mem.r16(DS, 0x05E0) & 0x100 == 0
                    && self.mem.r16(DS, LEVEL_FLAGS) & 0x200 == 0
                    && self.mem.r16(DS, MAP_FLAGS) & 4 != 0
                {
                    let v = self.mem.r16(DS, 0x102C);
                    self.mem.w16(DS, 0xB036, v);
                    let f = self.mem.r16(DS, 0x05E0) | 0x100;
                    self.mem.w16(DS, 0x05E0, f);
                    self.mem.w16(DS, 0x137A, 0x2F + n);
                    self.mem.w8(DS, 0x101C, 0);
                    self.mem.w8(DS, 0x137E, 0);
                    hooks.message_box_number(self);
                }
            }
        }
        // The compass key (ACE6) toggles 8725.
        if self.mem.r8(DS, 0xACE6) != 0 {
            let v = self.mem.r8(DS, 0x8725) ^ 1;
            self.mem.w8(DS, 0x8725, v);
        }
        self.breath()?;
        self.night_creatures(files, hooks)?;
        self.town_entered(files, hooks);
        if self.mem.r8(DS, 0xACED) != 0 && self.mem.r8(DS, 0xACF4) != 0 {
            hooks.key_d327(self);
            self.message(0x0A, 0x88F4);
        }
        if self.mem.r8(DS, 0xACE5) != 0 && self.mem.r8(DS, 0xACC5) != 0 && self.mem.r8(DS, 0xACE2) != 0 {
            let v = self.mem.r8(DS, 0x8812) ^ 0xFF;
            self.mem.w8(DS, 0x8812, v);
        }
        // A quest's message, once.
        if self.mem.r16(DS, 0x05CA) & 0x200 != 0 && !hooks.quest_carry(self) && self.mem.r16(DS, 0x05E0) & 0x400 == 0 {
            let f = self.mem.r16(DS, 0x05E0) | 0x400;
            self.mem.w16(DS, 0x05E0, f);
            self.mem.w16(DS, 0x137A, 0x05A6);
            self.mem.w8(DS, 0x137E, 0x3C);
            self.mem.w8(DS, 0x7387, 2);
            hooks.message_box_number(self);
        }
        // The clock: a minute each 90 ticks.
        let now = self.mem.r16(DS, TICKS);
        if now.wrapping_sub(self.mem.r16(DS, MINUTE_TICK)) >= 0x5A {
            self.mem.w16(DS, MINUTE_TICK, now);
            self.advance_minutes(files, 1, hooks)?;
        }
        self.hurt_flash();
        let v = self.mem.r16(DS, 0x0D37);
        self.mem.w16(DS, 0xC394, v);
        let f = self.mem.r16(DS, 0x05CA) & !0x200;
        self.mem.w16(DS, 0x05CA, f);
        Some(())
    }

    /// 0000:3F7E. Entering the wild (C39A bit 10h) the flats of kind C942
    /// become C942 + 1 across the map, and back on leaving; entering
    /// makes a sound unless 5CA bit 4.
    fn area_change(&mut self) {
        let area = self.mem.r16(DS, LEVEL_FLAGS);
        let last = self.mem.r16(DS, LAST_AREA);
        let k = self.mem.r8(DS, 0xC942);
        if area & 0x10 == 0 {
            let f = self.mem.r16(DS, 0x05CA) & !4;
            self.mem.w16(DS, 0x05CA, f);
            if last & 0x10 != 0 {
                if k != 0xFF {
                    self.swap_flats(k.wrapping_add(1), k);
                }
                self.mem.w16(DS, LAST_AREA, area);
            }
        } else if last & 0x10 == 0 {
            if k != 0xFF {
                self.swap_flats(k, k.wrapping_add(1));
            }
            self.mem.w16(DS, LAST_AREA, area);
        }
    }

    /// 0000:3FE7. Flats (type 8) with low byte `from` get `to`.
    fn swap_flats(&mut self, from: u8, to: u8) {
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        for i in 0..0x4000u16 {
            let w = self.mem.r16(walls, 2 * i);
            if w >> 12 == 8 && w as u8 == from {
                self.mem.w8(walls, 2 * i, to);
            }
        }
    }

    /// 0000:3C4C. The hands follow the equipped weapon class, unless
    /// casting or 5CA bit 10h: a new class's hands start lowered.
    fn weapon_change(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) -> Option<()> {
        use crate::weapon::var::{CAST_FRAME, CLASS, FRAME, SHOWN};
        let cl = self.mem.r8(DS, CLASS);
        if cl == self.mem.r8(DS, SHOWN)
            || (self.mem.r8(DS, CAST_FRAME) as i8) >= 0
            || self.mem.r16(DS, 0x05CA) & 0x10 != 0
        {
            return Some(());
        }
        self.mem.w8(DS, SHOWN, cl);
        if (cl as i8) < 0 {
            return Some(());
        }
        self.mem.w16(DS, FRAME, 0);
        hooks.weapon_graphics(self, files)
    }

    /// 2294:1125. Walking speed 14h + speed x 20 / 256 x (256 - fatigue
    /// byte 5BD) / 256 (C390, CCBE), turning 0Dh + half the first part
    /// (C392). In a fight (0FFB creatures), each creature slot not calm,
    /// cleared or a corpse gets 14h + its speed (record 2Dh) x 20 / 256.
    fn speeds(&mut self) {
        let bx = (self.mem.r8(DS, 0x01D1) as u16 * 20) >> 8;
        let fatigue = self.mem.r8(DS, 0x05BD) as u32;
        let walk = ((bx as u32 * (0x100 - fatigue)) >> 8) as u16 + 0x14;
        self.mem.w16(DS, 0xC390, walk);
        self.mem.w16(DS, crate::controls::var::WALK, walk);
        self.mem.w16(DS, crate::controls::var::TURN, (bx >> 1) + 0xD);
        let n = self.mem.r16(DS, 0x0FFB);
        let seg = self.mem.r16(DS, 0x1042);
        let corpse = (self.mem.r8(DS, 0xC936) as u16).wrapping_add(self.mem.r16(DS, 0xC38E)) as u8;
        let mut si = 0x0C57u16;
        for _ in 0..n {
            let sprite = self.mem.r8(DS, si.wrapping_add(0xC));
            if self.mem.r16(DS, si.wrapping_add(0xF)) & 0x4000 == 0 && sprite != 0xC5 && sprite != corpse {
                let rec = self.mem.r16(DS, si.wrapping_add(0x12));
                let v = ((self.mem.r8(seg, rec.wrapping_add(0x2D)) as u16 * 20) >> 8) + 0x14;
                self.mem.w16(DS, si.wrapping_add(8), v);
            }
            si = si.wrapping_add(0x1C);
        }
    }

    /// 0000:4A40. In deep water (wild, CB5B 2) without water breathing
    /// (spell flag 4): breath (5C0) runs down by 3, then hit points; at
    /// below 0 the player dies.
    fn breath(&mut self) -> Option<()> {
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x10 == 0
            || self.mem.r8(DS, 0xCB5B) != 2
            || self.mem.r16(DS, 0x05BE) & 4 != 0
        {
            return Some(());
        }
        let b = self.mem.r16(DS, 0x05C0);
        if b != 0 {
            let b = b.wrapping_sub(3);
            self.mem.w16(DS, 0x05C0, if (b as i16) < 0 { 0 } else { b });
            return Some(());
        }
        let hp = self.mem.r16(DS, 0x01FD).wrapping_sub(3);
        self.mem.w16(DS, 0x01FD, hp);
        if (hp as i16) < 0 {
            self.player_dies();
        }
        Some(())
    }

    /// 0000:4A7A. At night (time of day 110h..2F0h) in a town with no
    /// creatures about or overlay up, the night's creatures come: 15
    /// (7 in the wild, where 217B:0058 allows) after their graphics load.
    fn night_creatures(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) -> Option<()> {
        let tod = self.mem.r16(DS, 0xCCFE);
        if self.mem.r16(DS, 0x0FF7) != 0
            || self.mem.r16(DS, 0x0FF9) != 0
            || self.mem.r8(DS, 0x1013) != 0
            || self.mem.r16(DS, MAP_FLAGS) & 4 == 0
            || !(0x110..=0x2F0).contains(&tod)
        {
            return Some(());
        }
        let mut n = 0xF;
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            if !wild_spawn_allowed(self) {
                return Some(());
            }
            n = 7;
        }
        hooks.night_graphics(self, files)?;
        self.spawn_actors(n, 0xA)
    }

    /// 0000:1793. `n` actors (if in a town) are put around the player
    /// (`actor_respawn`) at speed `speed`, walking (2008h), state 34h or
    /// at random 39h with flag 1000h; the actors' squares are refilled.
    fn spawn_actors(&mut self, n: u16, speed: u16) -> Option<()> {
        let n = if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 { 0 } else { n };
        if n == 0 {
            return Some(());
        }
        self.mem.w16(DS, 0xA840, speed);
        let squares = crate::actors::var::SQUARES;
        for i in 0..0x17 {
            self.mem.w16(DS, squares + 2 * i, 0xFFFF);
        }
        let mut si = 0x0AB3u16;
        for i in 0..n {
            self.actor_respawn(si)?;
            let (x, y) = (self.mem.r16(DS, si), self.mem.r16(DS, si + 4));
            let sq = ((y >> 7) << 8).wrapping_add((x >> 7) << 1);
            self.mem.w16(DS, squares.wrapping_add(2 * i), sq);
            self.mem.w16(DS, si + 8, speed);
            self.mem.w16(DS, si + 0xF, 0x2008);
            self.mem.w8(DS, si + 0x11, 0x34);
            if self.rng.next() & 1 != 0 {
                self.mem.w8(DS, si + 0x11, 0x39);
                let f = self.mem.r16(DS, si + 0xF) | 0x1000;
                self.mem.w16(DS, si + 0xF, f);
            }
            si = si.wrapping_add(0x1C);
        }
        Some(())
    }

    /// 0000:4ACD. Entering a town (5C8 bit 100h, not the wild, after the
    /// first frames): the welcome (drawn by the front end), then the
    /// status refresh flags.
    fn town_entered(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) {
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0
            || self.mem.r16(DS, 0x05C8) & 0x100 == 0
            || self.mem.r16(DS, 0xCCBC) <= 3
        {
            return;
        }
        self.mem.w8(DS, 0x9288, 0x60);
        let f = self.mem.r16(DS, 0x05C8) & !0x100;
        self.mem.w16(DS, 0x05C8, f);
        let v = self.mem.r16(DS, 0x1036);
        self.mem.w16(DS, 0xB038, v);
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        hooks.town_welcome(self, files);
        let v = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, v);
        self.mem.w8(DS, 0x1404, 5);
    }

    /// 0000:4BD6. Out of the water the bob stops; the hurt flash steps
    /// through its table of 15 shakes into CB68: as they are in mode 0,
    /// doubled ±1 in mode 1, none in mode 2 (5E4 bits 1-2), which also
    /// ends it at once.
    fn hurt_flash(&mut self) {
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x10 == 0 {
            self.mem.w16(DS, crate::movement::var::BOB, 0);
        }
        if self.mem.r16(DS, FLASH) == 0 {
            return;
        }
        let mode = self.mem.r8(DS, 0x05E4) >> 1;
        if mode == 2 {
            self.mem.w16(DS, FLASH, 0);
            self.mem.w16(DS, FLASH_STEP, 0xFFFF);
        }
        let step = self.mem.r16(DS, FLASH_STEP).wrapping_add(1) & 0xF;
        self.mem.w16(DS, FLASH_STEP, step);
        if step == 0 {
            self.mem.w16(DS, FLASH, 0);
            return;
        }
        let ax = self.mem.r16(DS, (2 * step).wrapping_add(self.mem.r16(DS, FLASH)));
        let ax = match mode {
            0 => ax,
            1 => match ax {
                1 => 2,
                0xFFFF => 0xFFFE,
                v => v,
            },
            _ => 0,
        };
        self.mem.w16(DS, crate::movement::var::BOB, ax);
    }

    /// 0000:5D74. With EMS, the bank table (8FAA) frees every bank but
    /// the level's: the first [9032] - [9034] are taken (FFFFh).
    pub fn ems_reset(&mut self) {
        if self.mem.r8(DS, 0x8FA6) == 0 {
            return;
        }
        let n = self.mem.r16(DS, 0x9032);
        let taken = n.wrapping_sub(self.mem.r16(DS, 0x9034));
        for i in 0..n {
            self.mem.w16(DS, 0x8FAA_u16.wrapping_add(2 * i), 0);
        }
        for i in 0..taken {
            self.mem.w16(DS, 0x8FAA_u16.wrapping_add(2 * i), 0xFFFF);
        }
    }

    /// 0000:E322. In rain or snow (weather BB09 82h), 100 drops (6 bytes
    /// at E05E: x, y, and a drift of 2 either way) at random in the view
    /// (A914..A918 across, A916..A91A - 14h down); otherwise the table
    /// is cleared. None where the view is too small (a divide by zero).
    pub fn weather_drops(&mut self) -> Option<()> {
        if self.mem.r8(DS, 0xBB09) != 0x82 {
            for i in 0..0x12C {
                self.mem.w16(DS, 0xE05E + 2 * i, 0);
            }
            return Some(());
        }
        let r = |m: &Machine, a| m.mem.r16(DS, a);
        let mut si = 0xE05Eu16;
        for _ in 0..100 {
            let cx = r(self, 0xA91A).wrapping_sub(r(self, 0xA916)).wrapping_sub(0x14);
            let v = self.rng.next();
            if cx == 0 {
                return None;
            }
            let y = (v % cx).wrapping_add(r(self, 0xA916));
            self.mem.w16(DS, si.wrapping_add(2), y);
            let cx = r(self, 0xA918).wrapping_sub(r(self, 0xA914));
            let v = self.rng.next();
            if cx == 0 {
                return None;
            }
            let x = (v % cx).wrapping_add(r(self, 0xA914));
            self.mem.w16(DS, si, x);
            let drift = if self.rng.next() & 0x20 != 0 { 0xFFFE } else { 2 };
            self.mem.w16(DS, si.wrapping_add(4), drift);
            si = si.wrapping_add(6);
        }
        Some(())
    }
}

/// 2DA2:111C. The holiday on the day of `minutes` (day of the year from
/// 518400 minutes a year and 1440 a day), as its number in the table at
/// 33AD (sorted days), if any.
pub(crate) fn holiday(m: &Machine, minutes: u32) -> Option<u16> {
    let day = (minutes % 518_400 / 1440) as u16;
    if day > 0x158 {
        return None;
    }
    let mut si = 0x33ADu16;
    let mut n = 0u16;
    loop {
        let d = m.mem.r16(DS, si);
        if d == day {
            return Some(n);
        }
        if d as i16 > day as i16 {
            return None;
        }
        si = si.wrapping_add(2);
        n = n.wrapping_add(1);
    }
}

/// 217B:0058. In the wild, whether the night's creatures may come: the
/// chunk the player is in (by quadrant, the four at 0604) is in the
/// list at 392E (count 392D).
fn wild_spawn_allowed(m: &Machine) -> bool {
    let mut bx = 0u16;
    if m.mem.r16(DS, 0xC3D4) >= 0x2000 {
        bx = 2;
    }
    if m.mem.r16(DS, 0xC3D0) >= 0x2000 {
        bx += 1;
    }
    let al = m.mem.r8(DS, 0x0604 + bx);
    let n = m.mem.r8(DS, 0x392D) as u16;
    (0..n).any(|i| m.mem.r8(DS, 0x392E + i) == al)
}
