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
//! breath and drowning in deep water, townspeople in the daytime
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
    /// 0000:4C83, 4DD6: the interface set up again where it changed
    /// (`Machine::interface_setup`; the bar's drawing is the front end's).
    fn interface(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 292B:0341..0358 (in `no_spells`): NOSPELL.IMG loaded to DS:1044 and
    /// put into the page over the spells button (`no_spells_picture`).
    fn no_spells_picture(&mut self, m: &mut Machine, files: &mut Files) {
        m.no_spells_picture(files);
    }
    /// 0000:5011: an effect's words counted down and drawn
    /// (`Machine::effect_words`: the front end draws what it returns in
    /// [102C]'s font centred at x A0h, 0Bh above the view's foot, C3A2,
    /// in colour FDh, then one pixel right in FCh).
    fn effect_words(&mut self, _m: &mut Machine) {}
    /// 0000:4C86, 292B:0530: the faces drawn (`Machine::faces_draw`).
    fn faces(&mut self, _m: &mut Machine) {}
    /// 292B:000E (`Machine::projectile_lights_end`).
    fn projectile_lights_end(&mut self, _m: &mut Machine) {}
    /// 1194:59C8: a message box with the text at DGROUP `text`.
    fn message_box(&mut self, _m: &mut Machine, _text: u16) {}
    /// 1194:A36C: the message box of text number [137A].
    fn message_box_number(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 0000:05B5: the level's text shown (`level_text`: at DS:[3D42] in
    /// the EMS frame) drawn: its lines (to a CR, a `%` code the player's
    /// name at 01AD, a line ending in `:` the riddle's last), each centred
    /// (x A0h) from the view's bottom (C3A2) less its height (3D40), 0Ah
    /// apart, in the font at 102C and colour F0h (`Machine::level_text_lines`).
    fn level_text_draw(&mut self, m: &mut Machine) {
        m.level_text_lines();
    }
    /// 0000:0511: the message line (the text at [061A], while 061E
    /// counts down) drawn on the page, before the click (1194:DB6C) and
    /// the compass (4D7C): centred on x A0h at row 14h in the font at
    /// 102C, in colour 70h, then one pixel right in 9Fh
    /// (`Machine::message_line_draw`).
    fn message_draw(&mut self, m: &mut Machine) {
        m.message_line_draw();
    }
    /// 0000:4D2F: the keys held drawn on the page (`Machine::keys_draw`).
    fn keys(&mut self, m: &mut Machine) -> Option<()> {
        m.keys_draw()
    }
    /// 292B:05A2: the riddle's answer asked (typed under the text: 1044
    /// cleared, `lineedit::fields::riddle`) and looked for
    /// (`Machine::riddle_answered`); by default not asked.
    fn riddle(&mut self, _m: &mut Machine) {}
    /// The routine at GUARDS_HOOK (0000:3F0C or 3F3B): the guards arrive
    /// (`encounter::guards_hook`, with the fight's graphics, music and
    /// text left out; override to draw them).
    fn guards_arrive(&mut self, m: &mut Machine, files: &mut Files, hook: u32) -> Option<()> {
        m.guards_hook(files, &mut crate::encounter::NoFightHooks, hook)
    }
    /// 1194:DB6C: a click left for the actors (talk, loot, steal;
    /// `talk::actor_click`). Its windows and menus are `TalkHooks`: by
    /// default none (every menu escaped); override to give the player's.
    fn actor_click(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.actor_click(files, &mut crate::talk::NoTalkHooks)
    }
    /// 0000:3491: the squares around the player seen on the automap
    /// (`automap::automap_mark`, by default).
    fn automap_mark(&mut self, m: &mut Machine) {
        m.automap_mark();
    }
    /// 2294:04F5: the frame's song picked (`music::area_music`, by
    /// default; the songs asked for are on `Machine::songs`).
    fn area_music(&mut self, m: &mut Machine, files: &mut Files) {
        m.area_music(files);
    }
    /// 0000:3C4C's loading of the hands' graphic for weapon class [1012].
    fn weapon_graphics(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:17F9: the townspeople's graphics are loaded.
    fn townsfolk_graphics(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 2542:0100: the menu keys (Esc, the character sheet, the map...).
    fn menu_keys(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:4D00 (1194:9F8A, quests): its carry, which holds back the
    /// quest message of 5CA bit 200h.
    fn quest_carry(&mut self, m: &mut Machine) -> bool {
        m.quest_carry()
    }
    /// 0000:4B02..4B2B: the town's welcome (ENTERCTY.$$$ loaded to
    /// 1044), its box, and the wait for a key (1194:5532). The box saves
    /// what it covers into the frame (ES:DI [9030]:0, CX 7), bank [1030]
    /// mapped: `Machine::box_background` with the front end's screen.
    fn town_welcome(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 1194:D327 (keys ACED and ACF4: Alt+F5), `Machine::main_quest_reset`.
    fn key_d327(&mut self, m: &mut Machine) {
        m.main_quest_reset();
    }
    /// 1194:4635: a frame of rest, while resting (`rest::rest_frame`,
    /// with the front end's `RestHooks`: its texts and menus, and its
    /// `ClockHooks` for the hours passing). Nothing by default: a front
    /// end that lets the player camp (`Machine::camp`) calls it.
    fn rest(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 0000:4B84, 0390: the light for the time of day
    /// (`Machine::outdoor_light`, indoors the row limits only), with the
    /// front end's `LightHooks`, whose loads (the dither, the sky's images
    /// and the water's frames relit) the original makes inside it. Nothing
    /// by default.
    fn outdoor_light(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 0000:4C7E, 2294:0428: the day's colours loaded where the province's
    /// changed (`Machine::sky_colours`, `sky_colours_load`). Nothing by
    /// default.
    fn sky_colours(&mut self, _m: &mut Machine, _files: &mut Files) {}
}

/// `FrameHooks` that do nothing.
pub struct NoFrameHooks;

impl WeaponHooks for NoFrameHooks {}
impl ClockHooks for NoFrameHooks {
    fn exhausted(&mut self, _m: &mut Machine) -> Option<bool> {
        Some(false)
    }
    fn level_up_screen(&mut self, _m: &mut Machine, _files: &mut Files, _levels: u16) -> Option<()> {
        Some(())
    }
    fn encounter(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_daily(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    fn quests_hourly(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
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

    /// 0000:0550, each frame: the level's text shown (`level_text`) while
    /// its time (3C82) lasts, which runs down once the player has left its
    /// square (0DBC). The font from 102C (0000:9AEA: B036), colour F0h
    /// (9288), the level's bank mapped (C8FE), the lines drawn
    /// (`FrameHooks::level_text_draw`). With the riddle waiting (7C5E) the
    /// text: its answer asked (`riddle`), the time over if the text is no
    /// more (3D42 0), and the riddle cleared.
    pub fn level_text_frame(&mut self, hooks: &mut dyn FrameHooks) {
        use crate::squares::var::{SHOWN, SHOWN_SQUARE, SHOWN_TIME};
        let t = self.mem.r8(DS, SHOWN_TIME);
        if t == 0 {
            return;
        }
        if self.mem.r16(DS, SHOWN_SQUARE) != self.mem.r16(DS, 0xCB90) {
            self.mem.w8(DS, SHOWN_TIME, t.wrapping_sub(1));
        }
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        self.mem.w8(DS, 0x9288, 0xF0);
        let bank = self.mem.r8(DS, 0xC8FE) as u16;
        self.ems_map(bank);
        let si = self.mem.r16(DS, SHOWN);
        hooks.level_text_draw(self);
        if self.mem.r16(DS, 0x7C5E) != si {
            return;
        }
        hooks.riddle(self);
        if self.mem.r16(DS, SHOWN) == 0 {
            self.mem.w8(DS, SHOWN_TIME, 0);
        }
        self.mem.w16(DS, 0x7C5E, 0);
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
        self.area_change()?;
        // 1194:4635: a frame of rest (0000:48C9).
        hooks.rest(self, files)?;
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
        // 0000:48FE: the song (2294:04F5).
        hooks.area_music(self, files);
        // 0000:4903: a pending message box.
        if self.mem.r8(DS, 0x191F) != 0 {
            self.mem.w16(DS, 0xCB5E, 0xFFFF);
            self.mem.w8(DS, 0x191F, 0);
            self.mem.w8(DS, 0x101C, 0);
            self.mem.w8(DS, 0x137E, 0);
            hooks.message_box_number(self, files);
            self.mem.w16(DS, 0xCB5E, 0);
        }
        // 0000:492C: 292B:032A, the bar's spells barred for a class
        // without bit 20h.
        self.no_spells(files, hooks);
        self.weapon_change(files, hooks)?;
        self.weapon_frame(files, hooks)?;
        // 0000:493D: the keys held (4D2F).
        hooks.keys(self)?;
        self.level_text_frame(hooks);
        // 0000:0511: the message's countdown (and its drawing's colour
        // and font, 9288 and B036), its line drawn (`message_draw`).
        let t = self.mem.r16(DS, 0x061E);
        if t != 0 {
            self.mem.w16(DS, 0x061E, t - 1);
            self.mem.w8(DS, 0x9288, 0x70);
            let v = self.mem.r16(DS, 0x102C);
            self.mem.w16(DS, 0xB036, v);
            hooks.message_draw(self);
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
        // 0000:4957: the compass (4D7C).
        self.compass_draw();
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
        // 0000:49D6.
        hooks.automap_mark(self);
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
                    hooks.message_box_number(self, files);
                }
            }
        }
        // The compass key (ACE6) toggles 8725.
        if self.mem.r8(DS, 0xACE6) != 0 {
            let v = self.mem.r8(DS, 0x8725) ^ 1;
            self.mem.w8(DS, 0x8725, v);
        }
        self.breath()?;
        self.townsfolk_return(files, hooks)?;
        self.town_entered(files, hooks);
        if self.mem.r8(DS, 0xACED) != 0 && self.mem.r8(DS, 0xACF4) != 0 {
            hooks.key_d327(self);
            self.message(0x0A, 0x88F4);
        }
        if self.mem.r8(DS, 0xACE5) != 0 && self.mem.r8(DS, 0xACC5) != 0 && self.mem.r8(DS, 0xACE2) != 0 {
            let v = self.mem.r8(DS, 0x8812) ^ 0xFF;
            self.mem.w8(DS, 0x8812, v);
        }
        // 4B84: 0000:0390, the light for the time of day.
        hooks.outdoor_light(self, files);
        // A quest's message, once.
        if self.mem.r16(DS, 0x05CA) & 0x200 != 0 && !hooks.quest_carry(self) && self.mem.r16(DS, 0x05E0) & 0x400 == 0 {
            let f = self.mem.r16(DS, 0x05E0) | 0x400;
            self.mem.w16(DS, 0x05E0, f);
            self.mem.w16(DS, 0x137A, 0x05A6);
            self.mem.w8(DS, 0x137E, 0x3C);
            self.mem.w8(DS, 0x7387, 2);
            hooks.message_box_number(self, files);
        }
        // The clock: a minute each 90 ticks.
        let now = self.mem.r16(DS, TICKS);
        if now.wrapping_sub(self.mem.r16(DS, MINUTE_TICK)) >= 0x5A {
            self.mem.w16(DS, MINUTE_TICK, now);
            self.advance_minutes(files, 1, hooks)?;
        }
        self.hurt_flash();
        // 4C7B: 0000:5011, an effect's words over the view's foot
        // (`effect_words`); 4C7E: 2294:0428, the sky's colours; 4C83:
        // 4DD6, the interface where it changed (`interface_setup`); 4CBB:
        // 292B:000E (`projectile_lights_end`).
        hooks.effect_words(self);
        hooks.sky_colours(self, files);
        hooks.interface(self, files)?;
        // 4C86: 292B:0530, the faces (`faces_draw`).
        hooks.faces(self);
        hooks.projectile_lights_end(self);
        let v = self.mem.r16(DS, 0x0D37);
        self.mem.w16(DS, 0xC394, v);
        // 4CC6..4CDA: the row under the view cleared.
        self.view_row_clear();
        let f = self.mem.r16(DS, 0x05CA) & !0x200;
        self.mem.w16(DS, 0x05CA, f);
        Some(())
    }

    /// 0000:3F7E. Entering water (C39A bit 10h) the flats of kind C942
    /// become C942 + 1 across the map, and back on leaving; entering
    /// makes the splash (sound 17h at the player's square, 3FB8) unless
    /// 5CA bit 4. None where the sound's volume overflows
    /// (`Machine::sound_at`).
    fn area_change(&mut self) -> Option<()> {
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
            if self.mem.r16(DS, 0x05CA) & 4 == 0 {
                self.sound_here(0x17)?;
            }
        }
        Some(())
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

    /// 0000:3C4C with its load ported (`weapon_graphics_load`): as
    /// `weapon_change`, for callers without the frame's hooks (the game's
    /// set-up, 21EE:01CD).
    pub fn weapon_change_load(&mut self, files: &mut Files) -> Option<()> {
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
        self.weapon_graphics_load(files)
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

    /// 0000:4A7A. In the daytime (time of day 110h..2F0h: 100h is six in
    /// the morning, 200h noon) outdoors with no fight on or overlay up,
    /// the townspeople come back: 15 (7 in the wild, where 217B:0058
    /// allows) after their graphics load.
    fn townsfolk_return(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) -> Option<()> {
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
        hooks.townsfolk_graphics(self, files)?;
        self.spawn_actors(n, 0xA)
    }

    /// 0000:1793. `n` actors (if in a town) are put around the player
    /// (`actor_respawn`) at speed `speed`, walking (2008h), state 34h or
    /// at random 39h with flag 1000h; the actors' squares are refilled.
    pub fn spawn_actors(&mut self, n: u16, speed: u16) -> Option<()> {
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
        // 4AFB: the welcome's box keeps what it covers in the load bank,
        // at the frame's start, over what the last load unpacked there.
        let bank = self.mem.r16(DS, 0x1030);
        self.ems_map(bank);
        hooks.town_welcome(self, files);
        let v = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, v);
        self.mem.w8(DS, 0x1404, 5);
    }

    /// 0000:965E's flag 1 (with 8F2A, 7706): a box's background kept at
    /// `seg`:`off` before the box is drawn: its x (bit 15 set: the box's
    /// buffer was given, not allocated), y, width and height, then its
    /// rows from the draw page ([A900], 320 wide), here `screen` (the
    /// front end's, a byte a pixel; none: black). The last template text
    /// (5DDh) reads on past TEMPLATE.DAT's end into what the town's
    /// welcome left here (1194:A44A).
    pub fn box_background(&mut self, seg: u16, off: u16, (x, y, w, h): (u16, u16, u16, u16), screen: Option<&[u8]>) {
        for (i, v) in [x | 0x8000, y, w, h].into_iter().enumerate() {
            self.mem.w16(seg, off.wrapping_add(2 * i as u16), v);
        }
        // 8F2A's rows: from the row table (A6A0) plus x, on 140h a row,
        // each read in the page's segment (or [8F9C]'s with the dword at
        // 8F9A set), wrapping in it; what lies past the page (FA00h on)
        // is the memory after it, as is any other segment's.
        let page = self.mem.r16(DS, 0xA900);
        let from = if self.mem.r32(DS, 0x8F9A) != 0 { self.mem.r16(DS, 0x8F9C) } else { page };
        let mut row = self.mem.r16(DS, 0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
        let mut at = off.wrapping_add(8);
        for _ in 0..h {
            for c in 0..w {
                let o = row.wrapping_add(c);
                let v = match screen {
                    Some(s) if from == page && (o as usize) < s.len() => s[o as usize],
                    None if from == page && o < 0xFA00 => 0,
                    _ => self.mem.r8(from, o),
                };
                self.mem.w8(seg, at, v);
                at = at.wrapping_add(1);
            }
            row = row.wrapping_add(0x140);
        }
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
                return crate::giveup::original("0000:E322: the view has no width: the original divides by zero");
            }
            let y = (v % cx).wrapping_add(r(self, 0xA916));
            self.mem.w16(DS, si.wrapping_add(2), y);
            let cx = r(self, 0xA918).wrapping_sub(r(self, 0xA914));
            let v = self.rng.next();
            if cx == 0 {
                return crate::giveup::original("0000:E322: the view has no width: the original divides by zero");
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

impl Machine {
    /// 0000:5011, each frame: unless a level's text is up (3C82), an
    /// effect's words (1405, `effects::var::MESSAGE_TEXT`) are drawn
    /// (`effect_words_draw`; the words returned) and their count (1428)
    /// goes down; at 0 the words are cleared (23h bytes).
    pub fn effect_words(&mut self) -> Option<Vec<u8>> {
        use crate::effects::var::{MESSAGE, MESSAGE_TEXT};
        if self.mem.r8(DS, 0x3C82) != 0 || self.mem.r8(DS, MESSAGE_TEXT) == 0 {
            return None;
        }
        let text = self.mem.cstr(DS, MESSAGE_TEXT).to_vec();
        self.effect_words_draw();
        let n = self.mem.r8(DS, MESSAGE).wrapping_sub(1);
        self.mem.w8(DS, MESSAGE, n);
        if n == 0 {
            for i in 0..0x23u16 {
                self.mem.w8(DS, MESSAGE_TEXT + i, 0);
            }
        }
        Some(text)
    }

    /// 0000:3C76..3C8C, the hands' graphic for weapon class `class`
    /// (1012): its file the class'th name of the list at 674E (1194:559F)
    /// loaded (742C) to the EMS frame (9030:0) with the hands' bank (C8F4)
    /// mapped (5C8A). None where the file doesn't load.
    pub fn weapon_graphics_load(&mut self, files: &mut Files) -> Option<()> {
        let class = self.mem.r8(DS, crate::weapon::var::CLASS) as u16;
        let name = self.skip_strings(DS, 0x674E, class);
        let bank = self.mem.r8(DS, 0xC8F4) as u16;
        self.ems_map(bank);
        let frame = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        self.file_load(files, name, frame, 0).map(|_| ())
    }

    /// 292B:000E, each frame: with no projectile flying (7DB8), their
    /// lights (BB0C, 60h words) forgotten, 8000h each, and counted none
    /// (BC8C).
    pub fn projectile_lights_end(&mut self) {
        use crate::projectile::var::{COUNT, LIGHTS, LIGHT_COUNT};
        if self.mem.r16(DS, COUNT) != 0 {
            return;
        }
        self.mem.w16(DS, LIGHT_COUNT, 0);
        for i in 0..0x60u16 {
            self.mem.w16(DS, LIGHTS + 2 * i, 0x8000);
        }
    }

    /// 0000:4DD6's state, where the interface changed (D39, its bit 80h
    /// cleared, not 1011): both D39's; 5CA bit 2000h cleared, then set by
    /// 292B:048F (`faces_load`: the faces' pictures into bank 19h); the cursor's
    /// background flag (A902 bit 1, 5422) cleared; with the bow's hands
    /// up (1012 6) the hands made again (1012 FFh, 3C4C: `weapon_change`);
    /// the clip the whole screen (9A9C with 0, 0, 140h, C8h: A914..A91E);
    /// the view's window from the interface's record (4F1B with SI 883E:
    /// C39C..C3A3 its first eight bytes, C3D8 its next word; the view's
    /// click box 1B8C..1B92, its corner two in, 10h less each way; the
    /// mouse kept to 0..134h, 0..C7h, 53AE, and put at the view's middle,
    /// C3D6 and C3D8; C398 bit 2000h, the row tables made again); the
    /// rain's or snow's drops made again in the clip (E322,
    /// `weather_drops`); 5CA bit 8 cleared, then set again by 292B:032A
    /// for a class without 1A9 bit 20h. The bar's picture, the name, the
    /// bars and their texts are the front end's.
    pub fn interface_setup(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) -> Option<()> {
        let al = self.mem.r8(DS, 0x0D39);
        if al == self.mem.r8(DS, 0x1011) {
            return Some(());
        }
        let al = al & 0x7F;
        self.mem.w8(DS, 0x1011, al);
        self.mem.w8(DS, 0x0D39, al);
        let f = self.mem.r16(DS, 0x05CA) & !0x2000;
        self.mem.w16(DS, 0x05CA, f);
        self.faces_load(files)?;
        let f = self.mem.r16(DS, 0xA902);
        self.mem.w16(DS, 0xA902, f & !1);
        if self.mem.r8(DS, 0x1012) == 6 {
            self.mem.w8(DS, 0x1012, 0xFF);
            self.weapon_change(files, hooks)?;
        }
        for (o, v) in [(0xA91Eu16, 0x140u16), (0xA91C, 0xC8), (0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0xC7)] {
            self.mem.w16(DS, o, v);
        }
        // 4E20..4E29: the bar's picture (P1.IMG, named at 12E4) read into
        // the draw page ([A900]) at B7C0h, row 147.
        // Not over a heap's blocks: a game the port starts without a page
        // block has A000h there (`page_is_block`), where B7C0h on is its
        // heap's high block (a playtest's area changes from CITY:1:2 broke
        // the heap's chain). The file's open, read and close leave nothing
        // else behind.
        if self.page_is_block() {
            let page = self.mem.r16(DS, 0xA900);
            self.file_load(files, 0x12E4, page, 0xB7C0)?;
        }
        self.view_record(0x883E);
        self.weather_drops()?;
        let f = self.mem.r16(DS, 0x05CA) & !8;
        self.mem.w16(DS, 0x05CA, f);
        self.no_spells(files, hooks);
        // 4E41..4E7B, the bar's texts and pictures, drawn into the page
        // (`interface`): the font from 102E (9AEA: B036); the player's name
        // (1194:4BC8 with SI 1A4) copied to 1044, cut at its first space,
        // colour F0h (9288), drawn at (11h, 9Ah) (9E06); 12BA cleared, the
        // condition's picture (4E81, `status_picture_load`) and the face
        // (1194:2D5C, `portrait_load`) at (0Eh, A6h); the font from 102C;
        // the page shown (946A, the front end's); the bars (2901:00A8,
        // `bars_draw`).
        let v = self.mem.r16(DS, 0x102E);
        self.mem.w16(DS, 0xB036, v);
        self.name_text(0x01A4);
        if self.page_is_block() {
            self.page_text((DS, 0x1044), 0x11, 0x9A);
        } else {
            self.text_state(0x11, 0x9A, 0x1044);
        }
        self.mem.w16(DS, 0x12BA, 0);
        self.status_picture_load(files, 0x01A4)?;
        self.portrait_load(files, 0x01A4, 0x0E, 0xA6)?;
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        self.bars_draw();
        Some(())
    }

    /// 292B:032A: for a class without 1A9 bit 20h (no spells), once (5CA
    /// bit 8 set): NOSPELL.IMG (named at 7CFA) loaded whole to DS:1044
    /// (7428) and its 19h by 13h bytes put into the page at (5Bh, B1h),
    /// over the bar's spells button (599C; only where the page is the
    /// game's own block). The load's carry isn't looked at (292B:0349):
    /// a file that doesn't load leaves 1044 as it was, drawn all the same.
    pub(crate) fn no_spells(&mut self, files: &mut Files, hooks: &mut dyn FrameHooks) {
        if self.mem.r8(DS, 0x01A9) & 0x20 != 0 || self.mem.r16(DS, 0x05CA) & 8 != 0 {
            return;
        }
        let f = self.mem.r16(DS, 0x05CA) | 8;
        self.mem.w16(DS, 0x05CA, f);
        hooks.no_spells_picture(self, files);
    }

    /// 292B:0341..0358: NOSPELL.IMG (7CFA) loaded whole to DS:1044 (7428)
    /// and put into the page at (5Bh, B1h), 19h by 13h (599C; where the
    /// page is the game's own block).
    pub fn no_spells_picture(&mut self, files: &mut Files) {
        let _ = self.file_load(files, 0x7CFA, DS, 0x1044);
        if self.page_is_block() {
            self.page_rect_put((DS, 0x1044), 0x5B, 0xB1, 0x19, 0x13);
        }
    }

    /// 0000:4E81 with SI `si` (a person's record; nothing with its name,
    /// +9, empty): the condition's picture STATUS.CIF (named at 8856) read
    /// into the frame ([9030]:0) with the load bank ([1030]) mapped, its
    /// picture drawn (0000:3C94's, 582C), and the bank mapped before put
    /// back. The picture: 4 with a spell on (2686:0270's carry), 1 with
    /// +40D bit 1, 2 with bit 2 or 20h, 3 with health (+59) at most a
    /// tenth of its maximum (+5B), the last that holds; else 0.
    pub fn status_picture_load(&mut self, files: &mut Files, si: u16) -> Option<()> {
        if self.mem.r8(DS, si.wrapping_add(9)) == 0 {
            return Some(());
        }
        let old = self.mem.r16(DS, crate::level::var::EMS_MAPPED) >> 2;
        let bank = self.mem.r16(DS, 0x1030);
        self.ems_map(bank);
        let flags = self.mem.r16(DS, si.wrapping_add(0x40D));
        let mut cx = 0u16;
        if si == crate::character::PLAYER && self.spell_on_player() {
            cx = 4;
        }
        if flags & 1 != 0 {
            cx = 1;
        }
        if flags & 2 != 0 || flags & 0x20 != 0 {
            cx = 2;
        }
        if self.mem.r16(DS, si.wrapping_add(0x59)) <= self.mem.r16(DS, si.wrapping_add(0x5B)) / 10 {
            cx = 3;
        }
        let frame = self.mem.r16(DS, 0x9030);
        self.file_load(files, 0x8856, frame, 0)?;
        let mut at = 0u16;
        for _ in 0..cx {
            at = at.wrapping_add(self.mem.r16(frame, at.wrapping_add(0xA))).wrapping_add(0xC);
        }
        self.page_image(frame, at);
        self.ems_map(old);
        Some(())
    }

    /// 1194:2D5C with SI `si` (a person's record), AX `x`, BX `y` and DX
    /// 0: the face's file named (2DAF: FACES0r.CIF at 3EFD, a woman's,
    /// +7 set, FACESF0r.CIF at 3F09; the character two before the '.'
    /// '1' with 12BA set, else '0'; the race, +4, written in before the
    /// '.' by 0000:7024) and read into the frame with bank 17h mapped
    /// (0000:7428); then with bank 17h mapped again its picture +6Bh
    /// found (5CCC) and, BX not FFFFh, its place set to (`x`, `y`), and
    /// drawn (5828, `page_image`). The bank mapped before is put back
    /// each time.
    pub fn portrait_load(&mut self, files: &mut Files, si: u16, x: u16, y: u16) -> Option<()> {
        use crate::level::var::EMS_MAPPED;
        let name = if self.mem.r8(DS, si.wrapping_add(7)) != 0 { 0x3F09u16 } else { 0x3EFD };
        // `repne scasb` for '.' in 14h bytes: DI past it (or past them).
        let past_dot = |m: &Machine| {
            let mut di = name;
            for _ in 0..0x14 {
                let b = m.mem.r8(DS, di);
                di = di.wrapping_add(1);
                if b == b'.' {
                    break;
                }
            }
            di
        };
        let di = past_dot(self);
        let c = if self.mem.r16(DS, 0x12BA) != 0 { b'1' } else { b'0' };
        self.mem.w8(DS, di.wrapping_sub(3), c);
        let di = past_dot(self);
        let race = self.mem.r8(DS, si.wrapping_add(4)) as u16;
        self.store_utoa(race, di.wrapping_sub(2));
        let old = self.mem.r16(DS, EMS_MAPPED) >> 2;
        self.ems_map(0x17);
        let frame = self.mem.r16(DS, 0x9030);
        self.file_load(files, name, frame, 0)?;
        self.ems_map(old);
        let old = self.mem.r16(DS, EMS_MAPPED) >> 2;
        self.ems_map(0x17);
        let mut at = 0u16;
        for _ in 0..self.mem.r8(DS, si.wrapping_add(0x6B)) {
            at = at.wrapping_add(self.mem.r16(frame, at.wrapping_add(0xA))).wrapping_add(0xC);
        }
        if y != 0xFFFF {
            self.mem.w16(frame, at, x);
            self.mem.w16(frame, at.wrapping_add(2), y);
        }
        self.page_image(frame, at);
        self.ems_map(old);
        Some(())
    }

    /// 2901:00A8's state: the three bars (health 1FD of 1FF at x 39h,
    /// colour 1Ah; fatigue 201 of (1CD + 1D2) x 40h at 43h, 9Fh; spell
    /// points 20A of 20C at 4Dh, 3Eh) drawn: each leaves its x at 126C
    /// and, with a maximum not 0, its colour at 9288.
    pub fn bars_state(&mut self) {
        let fatigue = (self.mem.r8(DS, 0x01CD) as u16 + self.mem.r8(DS, 0x01D2) as u16) << 6;
        for (x, max, colour) in [(0x39u16, self.mem.r16(DS, 0x01FF), 0x1Au8), (0x43, fatigue, 0x9F), (0x4D, self.mem.r16(DS, 0x020C), 0x3E)] {
            self.mem.w16(DS, 0x126C, x);
            if max != 0 {
                self.mem.w8(DS, 0x9288, colour);
            }
        }
    }

    /// 1194:4BCC's text: 9288 F0h, the name at `si` + 9 copied to 1044
    /// (0000:9CA0) and cut where a space is in its first 14h bytes, else
    /// at the 14h'th (`repne scasb`, which looks on past the name's end):
    /// the 0 over the space or the last byte looked at.
    pub fn name_text(&mut self, si: u16) {
        self.mem.w8(DS, 0x9288, 0xF0);
        let mut i = 0u16;
        loop {
            let b = self.mem.r8(DS, si.wrapping_add(9).wrapping_add(i));
            self.mem.w8(DS, 0x1044u16.wrapping_add(i), b);
            if b == 0 {
                break;
            }
            i = i.wrapping_add(1);
        }
        let mut di = 0x1044u16;
        for _ in 0..0x14 {
            let b = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if b == 0x20 {
                break;
            }
        }
        self.mem.w8(DS, di.wrapping_sub(1), 0);
    }

    /// 0000:4F1B with SI `si`: the view's window C39C..C3A3 the record's
    /// first eight bytes and C3D8 its next word; the view's click box
    /// (1B8C x, 1B8E y, 1B90 width, 1B92 height) two in from its corner,
    /// 10h smaller each way; the mouse's limits (53AE with 0, 0, 135h,
    /// C8h: 8F74 0, 8F76 134h, 8F78 0, 8F7A C7h; the driver's are the
    /// front end's) and the mouse put at the view's middle (8F92 C3D6,
    /// 8F94 C3D8); C398 bit 2000h, the floor's and ceiling's row tables
    /// to be made again (0000:F587).
    pub fn view_record(&mut self, si: u16) {
        for i in 0..8u16 {
            let b = self.mem.r8(DS, si.wrapping_add(i));
            self.mem.w8(DS, 0xC39C + i, b);
        }
        let v = self.mem.r16(DS, si.wrapping_add(8));
        self.mem.w16(DS, 0xC3D8, v);
        let r = |m: &Machine, o: u16| m.mem.r16(DS, si.wrapping_add(o));
        let (ax, bx) = (r(self, 0), r(self, 2));
        let cx = r(self, 4).wrapping_sub(ax).wrapping_sub(0x10);
        let dx = r(self, 6).wrapping_sub(bx).wrapping_sub(0x10);
        for (o, v) in [(0x1B8Cu16, ax.wrapping_add(2)), (0x1B8E, bx.wrapping_add(2)), (0x1B90, cx), (0x1B92, dx)] {
            self.mem.w16(DS, o, v);
        }
        for (o, v) in [(0x8F78u16, 0u16), (0x8F7A, 0xC7), (0x8F74, 0), (0x8F76, 0x134)] {
            self.mem.w16(DS, o, v);
        }
        let (x, y) = (self.mem.r16(DS, 0xC3D6), self.mem.r16(DS, 0xC3D8));
        self.mem.w16(DS, 0x8F92, x);
        self.mem.w16(DS, 0x8F94, y);
        let f = self.mem.r16(DS, 0xC398);
        self.mem.w16(DS, 0xC398, f | 0x2000);
    }

    /// 0000:E1A2. The rain's 60 drops (4 bytes at E05E: x, y) moved and
    /// drawn: a drop at or left of the view's left + 8, or at or below
    /// its bottom - 8 (unsigned), comes back (a roll: with bit 40h at the
    /// right edge (A918) at a row of the roll mod the view's height less
    /// 14h, not offset by its top; else along the top at a column of the
    /// roll mod its width) and is looked at again; one in view falls by
    /// the speed (E05C, set from E05A before the first; from the 20th on
    /// E058's, from the 40th E056's, the count going down from 3Ch) and
    /// left by half of it, and is drawn where it was, a streak of eight
    /// pixels of 67h stepping a pixel left every other row, into `page`
    /// (the page's segment, A900) where there is one. None where the view
    /// has no width or height (the original divides by zero).
    pub fn rain_drops(&mut self, mut page: Option<&mut [u8]>) -> Option<()> {
        let r = |m: &Machine, a: u16| m.mem.r16(DS, a);
        let v = r(self, 0xE05A);
        self.mem.w16(DS, 0xE05C, v);
        let mut si = 0xE05Eu16;
        let mut cx = 0x3Cu16;
        loop {
            let (x, y) = (r(self, si), r(self, si.wrapping_add(2)));
            if x <= r(self, 0xA914).wrapping_add(8) || y >= r(self, 0xA91A).wrapping_sub(8) {
                let v = self.rng.next();
                if v & 0x40 != 0 {
                    let h = r(self, 0xA91A).wrapping_sub(r(self, 0xA916)).wrapping_sub(0x14);
                    if h == 0 {
                        return crate::giveup::original("0000:E25D: the view has no height: the original divides by zero");
                    }
                    self.mem.w16(DS, si.wrapping_add(2), v % h);
                    let right = r(self, 0xA918);
                    self.mem.w16(DS, si, right);
                } else {
                    let w = r(self, 0xA918).wrapping_sub(r(self, 0xA914));
                    if w == 0 {
                        return crate::giveup::original("0000:E23E: the view has no width: the original divides by zero");
                    }
                    let x = (v % w).wrapping_add(r(self, 0xA914));
                    self.mem.w16(DS, si, x);
                    let top = r(self, 0xA916);
                    self.mem.w16(DS, si.wrapping_add(2), top);
                }
                continue;
            }
            let di = r(self, 0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
            let speed = r(self, 0xE05C);
            self.mem.w16(DS, si.wrapping_add(2), y.wrapping_add(speed));
            self.mem.w16(DS, si, x.wrapping_sub(speed >> 1));
            if let Some(p) = page.as_deref_mut() {
                for k in [0u16, 0x140, 0x27F, 0x3BF, 0x4FF, 0x63E, 0x77E, 0x8BE] {
                    if let Some(px) = p.get_mut(di.wrapping_add(k) as usize) {
                        *px = 0x67;
                    }
                }
            }
            si = si.wrapping_add(4);
            let v = if cx < 0x14 {
                r(self, 0xE056)
            } else if cx < 0x28 {
                r(self, 0xE058)
            } else {
                r(self, 0xE05A)
            };
            self.mem.w16(DS, 0xE05C, v);
            cx -= 1;
            if cx == 0 {
                return Some(());
            }
        }
    }

    /// 0000:E26B. The snow's 100 flakes (6 bytes at E05E: x, y, drift)
    /// moved and drawn: a flake at or left of the view's left, at or
    /// right of its right, or at or below its bottom - 4 (unsigned) comes
    /// back (a roll: its drift 2, or -2 with bit 20h; a column of the
    /// same roll mod the view's width along its top) and is looked at
    /// again; one in view is drawn where it was in colour 10h (the first
    /// 20, counting down from 64h, as 2 x 2; the next 30 as 2 x 2; the
    /// last 50 as one pixel) into `page` where there is one, falls by its
    /// band's speed (E2BA, E2B8, E2B6) and drifts, the other way on a
    /// roll under 3A98h. A flake that came back is banded by what the
    /// original's division left in CX, the view's width, not the count
    /// (kept). None where the view has no width.
    pub fn snow_drops(&mut self, mut page: Option<&mut [u8]>) -> Option<()> {
        let r = |m: &Machine, a: u16| m.mem.r16(DS, a);
        let mut si = 0xE05Eu16;
        for count in (1..=0x64u16).rev() {
            let mut cx = count;
            loop {
                let (x, y) = (r(self, si), r(self, si.wrapping_add(2)));
                if x <= r(self, 0xA914) || x >= r(self, 0xA918) || y >= r(self, 0xA91A).wrapping_sub(4) {
                    let v = self.rng.next();
                    let drift = if v & 0x20 != 0 { 0xFFFE } else { 2 };
                    self.mem.w16(DS, si.wrapping_add(4), drift);
                    let w = r(self, 0xA918).wrapping_sub(r(self, 0xA914));
                    if w == 0 {
                        return crate::giveup::original("0000:E311: the view has no width: the original divides by zero");
                    }
                    let x = (v % w).wrapping_add(r(self, 0xA914));
                    self.mem.w16(DS, si, x);
                    let top = r(self, 0xA916);
                    self.mem.w16(DS, si.wrapping_add(2), top);
                    cx = w;
                    continue;
                }
                let bx = r(self, 0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
                let (cells, speed): (&[u16], u16) = if cx >= 0x50 {
                    (&[0, 1, 0x140, 0x141], r(self, 0xE2BA))
                } else if cx >= 0x32 {
                    (&[0, 1, 0x140, 0x141], r(self, 0xE2B8))
                } else {
                    (&[0], r(self, 0xE2B6))
                };
                if let Some(p) = page.as_deref_mut() {
                    for &k in cells {
                        if let Some(px) = p.get_mut(bx.wrapping_add(k) as usize) {
                            *px = 0x10;
                        }
                    }
                }
                let mut drift = r(self, si.wrapping_add(4));
                if self.rng.next() < 0x3A98 {
                    drift = drift.wrapping_neg();
                }
                self.mem.w16(DS, si, x.wrapping_add(drift));
                self.mem.w16(DS, si.wrapping_add(2), y.wrapping_add(speed));
                break;
            }
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

/// 217B:0058. In the wild, whether people may come: the
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
