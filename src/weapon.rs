//! The player's weapon each frame (0000:38CE): drawing and sheathing it,
//! and swinging it.
//!
//! A swing starts when the mouse moves at least 40h mickeys (the
//! larger axis plus a quarter of the smaller) while the right button is
//! down: the drag's direction (16 sectors) picks the swing's first
//! animation frame (tables 67BA, or 67CA for the bare hands, classes
//! 7..9), and the swing then runs a frame at a time; on every fifth frame
//! (frame mod 5 = 4) it lands (`strike::swing_hits`), and with no actor
//! in reach it hits the wall ahead (`bash`, 0000:3D43). A
//! bow (class 6) fires instead when the right button is down with the
//! cursor in the view's centre, then waits (680E) by agility.
//!
//! The mouse's motion counters (INT 33h function 0Bh, mickeys since the
//! last read) are input: the front end adds the mouse's motion to
//! `Machine::mouse_motion`, and each frame's read takes and clears it.
//! A modern front end can set it to a swing's drag directly. The hand
//! drawing (0000:3B8E) and the casting hands' graphics are the
//! renderer's.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::movement::var::{HEADING, LEVEL_FLAGS, LOCKS, LOCK_COUNT, MAP_FLAGS, TRIGGERS};

/// DGROUP variables.
pub mod var {
    /// The hand's animation frame: 0..1Dh swinging, 1Eh.. drawing (20h
    /// held ready), counting down to below 1Eh when sheathing.
    pub const FRAME: u16 = 0x1947;
    /// The bow's wait (byte, frames).
    pub const RELOAD: u16 = 0x680E;
    /// The casting hands' frame (byte; negative when not casting) and
    /// the hands' graphic (far pointer; its frame count at +0Bh).
    pub const CAST_FRAME: u16 = 0x7F80;
    pub const HANDS: u16 = 0x902E;
    /// The weapon class equipped (D49, negative none) and the one whose
    /// hands are shown (1012).
    pub const CLASS: u16 = 0x0D49;
    pub const SHOWN: u16 = 0x1012;
    /// The last swing's drag (mickeys).
    pub const DRAG: u16 = 0x67E4;
    /// First swing frames by drag direction (16 bytes each).
    pub const SWINGS: u16 = 0x67BA;
    pub const SWINGS_HANDS: u16 = 0x67CA;
    /// The bash's target: the square (bit 8000h for a door), its
    /// toughness and the door's lock record.
    pub const BASH_AT: u16 = 0x67DA;
    pub const BASH_LOCK: u16 = 0x67DC;
    pub const BASH_TOUGHNESS: u16 = 0x67DE;
}
use var::*;

/// What the swing leaves to code not ported here.
pub trait WeaponHooks {
    /// 0000:43B1: a wall's trigger (a building broken into).
    fn wall_trigger(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 1194:4FB8: whether the building of trigger kind `kind` is closed
    /// now (`Machine::building_closed`).
    fn building_closed(&mut self, m: &mut Machine, kind: u8) -> Option<bool> {
        Some(m.building_closed(kind))
    }
    /// 29CF:037F: the bow fires an arrow (flat 1Bh) from the player,
    /// aimed by the cursor (`projectile::fire_missile`).
    fn fire_bow(&mut self, m: &mut Machine, _files: &mut Files) -> Option<()> {
        m.fire_missile(0x1B, 0)
    }
}

/// `WeaponHooks` that do nothing.
pub struct NoWeaponHooks;

impl WeaponHooks for NoWeaponHooks {}

impl Machine {
    /// INT 33h function 0Bh: the mouse's motion since the last read.
    fn read_motion(&mut self) -> (u16, u16) {
        std::mem::take(&mut self.mouse_motion)
    }

    /// 29CF:0A28. The casting hands advance a frame, and stop (FFh) after
    /// the graphic's last.
    fn cast_hands(&mut self) {
        let f = self.mem.r8(DS, CAST_FRAME);
        if (f as i8) < 0 {
            return;
        }
        let (off, seg) = (self.mem.r16(DS, HANDS), self.mem.r16(DS, HANDS + 2));
        let frames = self.mem.r8(seg, off.wrapping_add(0xB));
        self.mem.w8(DS, CAST_FRAME, if f >= frames { 0xFF } else { f + 1 });
    }

    /// 0000:38CE. The player's weapon this frame. None where a strike
    /// fails (`swing_hits`) or a hook does.
    pub fn weapon_frame(&mut self, files: &mut Files, hooks: &mut dyn WeaponHooks) -> Option<()> {
        let r = self.mem.r8(DS, RELOAD);
        if r != 0 {
            self.mem.w8(DS, RELOAD, r - 1);
        }
        self.cast_hands();
        if (self.mem.r8(DS, CAST_FRAME) as i8) >= 0 || self.mem.r16(DS, 0x0620) != 0 {
            return Some(());
        }
        let level = self.mem.r16(DS, LEVEL_FLAGS);
        if level & 0x10 != 0 {
            // In water: the hands stay down unless levitating.
            return Some(());
        }
        let flags = self.mem.r16(DS, 0x05E0);
        let buttons = self.mem.r8(DS, crate::controls::var::BUTTONS);
        if self.mem.r8(DS, SHOWN) == 6 {
            // 0000:390D: the bow.
            if flags & 0x10 == 0
                || buttons & 2 == 0
                || self.mem.r16(DS, 0x05B1) & 0x40 != 0
                || flags & 0x40 == 0
                || self.mem.r8(DS, RELOAD) != 0
            {
                return Some(());
            }
            let agility = self.mem.r8(DS, 0x01D1) as u16;
            self.mem.w8(DS, RELOAD, ((0x100 - agility) >> 4) as u8 + 4);
            return hooks.fire_bow(self, files);
        }
        if self.mem.r16(DS, 0x05CA) & 0x10 != 0 || (self.mem.r8(DS, CLASS) as i8) < 0 {
            return Some(());
        }
        let frame = self.mem.r16(DS, FRAME);
        if flags & 0x10 == 0 {
            // 0000:3AD9: sheathing.
            if frame >= 0x1E {
                self.mem.w16(DS, FRAME, frame - 1);
            }
            return Some(());
        }
        if frame >= 0x1E && frame != 0x20 {
            // Drawing.
            self.mem.w16(DS, FRAME, frame + 1);
            return Some(());
        }
        if flags & 4 != 0 {
            if frame < 0x1E {
                let frame = frame + 1;
                self.mem.w16(DS, FRAME, frame);
                match frame % 5 {
                    4 => {
                        if !self.swing_hits()? {
                            self.bash(files, hooks)?;
                        }
                    }
                    0 => {
                        let f = self.mem.r16(DS, 0x05E0) & !4;
                        self.mem.w16(DS, 0x05E0, f);
                        self.mem.w16(DS, FRAME, 0);
                    }
                    _ => {}
                }
                self.read_motion();
                return Some(());
            }
            self.mem.w16(DS, 0x05E0, flags & !4);
        }
        if self.swing_start() {
            self.read_motion();
            return Some(());
        }
        self.read_motion();
        self.mem.w16(DS, FRAME, 0x20);
        Some(())
    }

    /// 0000:3C9F. A drag of at least 40h with the right button down
    /// starts a swing (5E0 bit 4).
    fn swing_start(&mut self) -> bool {
        let stop = |m: &mut Machine| {
            let f = m.mem.r16(DS, 0x05E0) & !4;
            m.mem.w16(DS, 0x05E0, f);
            false
        };
        if self.mem.r16(DS, 0x05B1) & 0x40 != 0 || self.mem.r8(DS, crate::controls::var::BUTTONS) & 2 == 0 {
            return stop(self);
        }
        let (dx, dy) = self.read_motion();
        self.mem.w16(DS, DRAG, dx);
        self.mem.w16(DS, DRAG + 2, dy);
        if crate::actors::approx(dx, dy) < 0x40 {
            return stop(self);
        }
        let f = self.mem.r16(DS, 0x05E0) | 4;
        self.mem.w16(DS, 0x05E0, f);
        // The original measures from (0, dy) to (dx, 0).
        let sector = crate::travel::travel_heading(0, dy, dx, 0) >> 5;
        let shown = self.mem.r8(DS, SHOWN);
        let table = if (7..=9).contains(&shown) { SWINGS_HANDS } else { SWINGS };
        let first = self.mem.r8(DS, table.wrapping_add(sector)) as u16;
        self.mem.w16(DS, FRAME, first);
        true
    }

    /// 0000:3D43. A swing that reached no actor hits the square ahead
    /// (by the heading's nearest of 8 directions). In a city, a plain wall
    /// with a trigger that is a building closed now (houses, or shops at
    /// night: `WeaponHooks::building_closed`) is a break-in: if seen
    /// (`crime_seen`) the guards come, otherwise a roll under strength / 8
    /// opens it (5CA bit 800h, then the wall's trigger). A breakable flat
    /// (C93B) or a locked door is bashed: the weapon's damage (as a blow,
    /// 2AA1:0497) must reach 6, the guards may come, and a roll under
    /// strength x 100 / 256 less the toughness (32h, or the lock's level x
    /// 5) breaks it: the flat becomes C93C (as type 80h), the door's lock
    /// goes and it opens.
    pub fn bash(&mut self, files: &mut Files, hooks: &mut dyn WeaponHooks) -> Option<()> {
        let h = (self.mem.r16(DS, HEADING).wrapping_add(0x20) & 0x1FF) >> 6;
        let ahead = self.mem.r16(DS, 0xE38C + 2 * h).wrapping_add(self.mem.r16(DS, 0xCB90));
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let w = self.mem.r16(walls, ahead);
        let strength = self.mem.r8(DS, 0x01CD) as u16;
        if (w as i16) >= 0 {
            if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 || (w >> 8) as u8 != w as u8 {
                return Some(());
            }
            let Some(i) = (0..0x10).find(|&i| self.mem.r8(DS, TRIGGERS + i) == w as u8) else {
                return Some(());
            };
            let table = if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 { 0x67FE } else { 0x67EE };
            let kind = self.mem.r8(DS, table + i);
            // 0000:50A9: kinds 2 and 3 are always closed.
            if !matches!(kind, 2 | 3) && !hooks.building_closed(self, kind)? {
                return Some(());
            }
            self.mem.w16(DS, crate::controls::var::PICKED, ahead);
            if self.crime_seen() {
                self.call_guards();
                return Some(());
            }
            let roll = self.rng.next() % 100;
            if roll as i16 > (strength >> 3) as i16 {
                return Some(());
            }
            let f = self.mem.r16(DS, 0x05CA) | 0x800;
            self.mem.w16(DS, 0x05CA, f);
            return hooks.wall_trigger(self, files);
        }
        match w >> 12 {
            8 => {
                if w as u8 != self.mem.r8(DS, 0xC93B) {
                    return Some(());
                }
                self.mem.w16(DS, BASH_AT, ahead);
                self.mem.w16(DS, BASH_TOUGHNESS, 0x32);
            }
            0xB => {
                let key = (ahead & 0xFF00) | ((ahead as u8) >> 1) as u16;
                let n = self.mem.r8(DS, LOCK_COUNT) as u16;
                let Some(rec) = (0..n).map(|i| LOCKS + 3 * i).find(|&r| self.mem.r16(DS, r) == key) else {
                    return Some(());
                };
                let level = self.mem.r8(DS, rec + 2) as u16;
                self.mem.w16(DS, BASH_TOUGHNESS, (level * 5) & 0xFFFF);
                self.mem.w16(DS, BASH_LOCK, rec);
                self.mem.w16(DS, BASH_AT, ahead | 0x8000);
            }
            _ => return Some(()),
        }
        let f = self.mem.r16(DS, 0x05CA) | 0x20;
        self.mem.w16(DS, 0x05CA, f);
        let damage = self.player_bash_damage()?;
        let f = self.mem.r16(DS, 0x05CA) & !0x20;
        self.mem.w16(DS, 0x05CA, f);
        if damage < 6 {
            return Some(());
        }
        self.call_guards();
        let roll = self.rng.next() % 100;
        let ax = (((strength as u8 as u16) * 100) >> 8).wrapping_sub(self.mem.r16(DS, BASH_TOUGHNESS));
        if (ax as i16) < 0 || roll as i16 > ax as i16 {
            return Some(());
        }
        let at = self.mem.r16(DS, BASH_AT);
        if (at as i16) >= 0 {
            let v = 0x8000 | self.mem.r8(DS, 0xC93C) as u16;
            self.mem.w16(walls, at, v);
        } else {
            let at = at & 0x7FFF;
            let rec = self.mem.r16(DS, BASH_LOCK);
            self.mem.w16(DS, rec, 0xFFFF);
            let v = self.mem.r16(walls, at) | 0x200;
            self.mem.w16(walls, at, v);
        }
        Some(())
    }

    /// 1194:4FB8. Whether a building of kind `kind` is closed now (137F
    /// keeps the kind, or FFh): today's holiday (1194:4A4A) closes the
    /// kind it names when its next byte is 0; houses and shops (kinds 1,
    /// 5, 6, 7) are open from time of day 100h to 2FFh; others always.
    pub fn building_closed(&mut self, kind: u8) -> bool {
        self.mem.w8(DS, 0x137F, kind);
        if let Some(n) = crate::frame::holiday(self, self.mem.r32(DS, 0x05CC)) {
            let si = 0x3353u16.wrapping_add(n.wrapping_mul(6));
            if self.mem.r8(DS, si) == kind && self.mem.r8(DS, si + 1) == 0 {
                self.mem.w8(DS, 0x137F, 0xFF);
                return true;
            }
        }
        if matches!(kind, 1 | 5 | 6 | 7) {
            let tod = self.mem.r16(DS, 0xCCFE);
            return !(0x100..0x300).contains(&tod);
        }
        self.mem.w8(DS, 0x137F, 0xFF);
        false
    }

    /// 0000:3EBE. Unless a fight is on (0FFB) or this is not a town (a
    /// dungeon of kind 4 except D48 8 and 0Bh counts), everyone becomes
    /// peaceful (1194:42B5) and the guards come in 15 frames (the hook at
    /// 67EA, 0000:3F0C).
    fn call_guards(&mut self) {
        if self.mem.r16(DS, 0x0FFB) != 0 {
            return;
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 == 0
            && (self.mem.r8(DS, 0x05E6) != 4 || matches!(self.mem.r8(DS, 0x0D48), 8 | 0xB))
        {
            return;
        }
        self.calm_everyone();
        self.mem.w8(DS, 0x680F, 0x0F);
        self.mem.w16(DS, 0x67EA, 0x3F0C);
        self.mem.w16(DS, 0x67EC, 0);
    }

    /// 2AA1:0497: the player's blow's damage (multiplier 1); a weapon
    /// worn out breaks.
    fn player_bash_damage(&mut self) -> Option<u16> {
        self.mem.w16(DS, 0x1400, crate::character::PLAYER);
        let gear = self.player_gear();
        let (damage, broken) = self.weapon_damage(DS, DS, 1, &gear)?;
        if let Some(it) = broken {
            self.item_broken(it)?;
        }
        Some(damage)
    }
}
