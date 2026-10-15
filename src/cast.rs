//! The player casting a spell (segment 29CF): from the spellbook
//! (29CF:071F), a spell either takes effect on the player at once or
//! waits for a target (0620, with its prompt over the view), which a
//! click resolves (29CF:0000): a touch on an actor or on the player, an
//! aimed spell launched from the player along the cursor's direction, or
//! an area around the player.

use crate::bsa::Files;
use crate::character::PLAYER;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The cast waiting for a target (a cast record, 0 none).
    pub const PENDING: u16 = 0x0620;
    /// The spell points the cast cost (refunded if cancelled).
    pub const COST: u16 = 0x0F84;
    /// The flags (5E0) saved while a cast waits.
    pub const SAVED_FLAGS: u16 = 0x7DBA;
    /// What the prompt asks for: 0 a touch, 1 aim, FFh an area.
    pub const TARGET_KIND: u16 = 0x0A21;
    /// The fake actor an aimed spell starts from (at the player, along
    /// the cursor's direction).
    pub const AIM: u16 = 0x929E;
}
use var::*;

/// Where a touch spell lands (the caller's AL and SI of 29CF:0000).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touch {
    /// An actor slot clicked (AL 0): its record, with a flash on it.
    Actor(u16),
    /// A record (AL 1): the player at 01A4 from the interface bar.
    Record(u16, u16),
}

/// What casting leaves to the front end.
pub trait CastHooks {
    /// 1194:59C8: a message box with the text at DGROUP `text` (silenced,
    /// or not enough spell points).
    fn message_box(&mut self, _m: &mut Machine, _text: u16) {}
}

/// `CastHooks` that do nothing.
pub struct NoCastHooks;

impl CastHooks for NoCastHooks {}

impl Machine {
    /// 29CF:071F. The player casts spell `index` of their spellbook:
    /// not when silenced (spell flag 200h, "Silence spell prevents spell
    /// casting."), and only with the spell points: the cost is the
    /// spell's +32h / (level + 1) ("Not enough spell points..."). Spells
    /// waiting for a target (Open, Lock, walls and floors) are
    /// cancelled, the cost is taken (and kept at COST), the cast is made
    /// (`player_cast`), and in a town (5E6 0) a spell with a target may
    /// be seen as a crime and bring the guards.
    pub fn cast_spell(&mut self, files: &mut Files, hooks: &mut dyn CastHooks, index: u8) -> Option<()> {
        // 29CF:07A8: the message box's colour and fonts, then the box.
        let message = |m: &mut Machine, hooks: &mut dyn CastHooks, text: u16| {
            m.mem.w8(DS, 0x9288, 0xD4);
            let v = m.mem.r16(DS, 0x102C);
            m.mem.w16(DS, 0xB036, v);
            let v = m.mem.r16(DS, 0x1036);
            m.mem.w16(DS, 0xB038, v);
            hooks.message_box(m, text);
        };
        if self.mem.r16(DS, 0x05BE) & 0x200 != 0 {
            self.mem.w8(DS, 0x9288, 0xD4);
            let v = self.mem.r16(DS, 0x102C);
            self.mem.w16(DS, 0xB036, v);
            message(self, hooks, 0x7D77);
            return Some(());
        }
        for o in [0x0DC0, 0x0DC2, 0x0DC4, 0x0DC8] {
            self.mem.w16(DS, o, 0);
        }
        self.player_weapon_class();
        let (gs, spell) = self.spell_record(index);
        let target = self.mem.r8(gs, spell.wrapping_add(0x24));
        let cost = self.mem.r16(gs, spell.wrapping_add(0x32)) / (self.mem.r8(DS, 0x01AA) as u16 + 1);
        if self.mem.r16(DS, 0x020A) < cost {
            message(self, hooks, 0x7D9D);
            return Some(());
        }
        let sp = self.mem.r16(DS, 0x020A) - cost;
        self.mem.w16(DS, 0x020A, sp);
        self.mem.w16(DS, COST, cost);
        self.player_cast(files, index)?;
        if self.mem.r8(DS, 0x05E6) == 0 && (target as i8) > 0 && self.crime_seen() && self.mem.r16(DS, 0x0FF9) == 0 {
            // 0000:3F21
            self.mem.w8(DS, 0x680F, 0x0F);
            self.mem.w16(DS, 0x67EA, 0x3F3B);
            self.mem.w16(DS, 0x67EC, 0);
        }
        Some(())
    }

    /// 29CF:00FA. A cast record for the player's spell `index` (the first
    /// free one, 29CF:01C4) waits at PENDING if it needs a target
    /// (`target_prompt`): the weapon is put away (5E0 bit 10h saved and
    /// cleared, D49 FFh). Otherwise it takes effect on the player now.
    pub(crate) fn player_cast(&mut self, files: &mut Files, index: u8) -> Option<()> {
        let _ = files;
        let cast = self.free_cast()?;
        self.mem.w16(DS, cast.wrapping_add(0xA), 1);
        let level = self.mem.r8(DS, PLAYER + 6) as u16 + 1;
        self.mem.w16(DS, cast.wrapping_add(8), level);
        self.mem.w16(DS, crate::spell::svar::CASTER_LEVEL, level);
        self.mem.w16(DS, cast.wrapping_add(4), PLAYER);
        self.mem.w16(DS, cast.wrapping_add(6), DS);
        let (gs, spell) = self.spell_record(index);
        self.mem.w16(DS, cast, spell);
        self.mem.w16(DS, cast.wrapping_add(2), gs);
        self.mem.w16(DS, cast.wrapping_add(0x14), 0xFFFF);
        self.mem.w16(DS, PENDING, cast);
        if self.target_prompt(cast, gs, spell) {
            // AX still holds the spells' segment (3276h) from the lookup:
            // its high byte is saved too.
            let b = self.mem.r8(DS, 0x05E0);
            self.mem.w16(DS, SAVED_FLAGS, (crate::mem::DATA & 0xFF00) | b as u16);
            let f = self.mem.r16(DS, 0x05E0) & !0x10;
            self.mem.w16(DS, 0x05E0, f);
            self.mem.w8(DS, 0x0D49, 0xFF);
            return Some(());
        }
        self.mem.w16(DS, PENDING, 0);
        // 29CF:025C
        self.mem.w16(DS, crate::spell::svar::TARGETS, PLAYER);
        self.mem.w16(DS, crate::spell::svar::TARGETS + 2, DS);
        self.mem.w16(DS, crate::spell::svar::TARGET_COUNT, 1);
        self.cast_apply(cast)
    }

    /// 29CF:01C4. The first cast record not in use (searching on past
    /// the eight, as the original does).
    fn free_cast(&self) -> Option<u16> {
        let mut cast = crate::spell::CASTS;
        for _ in 0..0x10000 / 0x16 + 1 {
            if self.mem.r16(DS, cast.wrapping_add(0xA)) & 1 == 0 {
                return Some(cast);
            }
            cast = cast.wrapping_add(0x16);
        }
        crate::giveup::original("29CF:01C4: no free cast record: the original searches on through memory")
    }

    /// 29CF:01D9. Whether the spell needs a target, and its prompt: a
    /// touch (type 1, "Pick spell target..."), aiming (types 2 and 4, or
    /// flag 80h at +27h), or an area (others above 0). Types 0 and below
    /// take effect on the caster.
    fn target_prompt(&mut self, cast: u16, gs: u16, spell: u16) -> bool {
        let ty = self.mem.r8(gs, spell.wrapping_add(0x24));
        let flagged = self.mem.r16(gs, spell.wrapping_add(0x27)) & 0x80 != 0;
        let (text, kind) = if flagged || ty == 2 || ty == 4 {
            (0x7F4A, 1)
        } else if (ty as i8) <= 0 {
            return false;
        } else if ty == 1 {
            (0x7F35, 0)
        } else {
            (0x7F62, 0xFF)
        };
        self.mem.w16(DS, 0x061E, 0xFFFD);
        self.mem.w16(DS, 0x061A, text);
        self.mem.w16(DS, 0x061C, DS);
        let f = self.mem.r16(DS, cast.wrapping_add(0xA)) | 0x20;
        self.mem.w16(DS, cast.wrapping_add(0xA), f);
        self.mem.w16(DS, PENDING, cast);
        self.mem.w8(DS, TARGET_KIND, kind);
        true
    }

    /// 29CF:0000. The cast waiting at PENDING takes effect: a touch spell
    /// (type 1) on `touch`; an area spell (type 3) around its target
    /// actor (+14h, if one) or the player (the casting hands start); one
    /// flagged 80h at +27h on the player; others are launched from the
    /// player along the cursor's direction (`launch_player`). Then the
    /// player's state comes back (29CF:00AC). Returns whether a cast was
    /// waiting (the original's carry). None where a launch or the
    /// effects fail.
    pub fn resolve_cast(&mut self, files: &mut Files, touch: Touch) -> Option<bool> {
        let cast = self.mem.r16(DS, PENDING);
        if cast == 0 {
            return Some(false);
        }
        let (spell, gs) = (self.mem.r16(DS, cast), self.mem.r16(DS, cast.wrapping_add(2)));
        let ty = self.mem.r8(gs, spell.wrapping_add(0x24));
        let target = |m: &mut Machine, seg: u16, rec: u16| {
            m.mem.w16(DS, crate::spell::svar::TARGETS, rec);
            m.mem.w16(DS, crate::spell::svar::TARGETS + 2, seg);
            m.mem.w16(DS, crate::spell::svar::TARGET_COUNT, 1);
        };
        if ty == 1 {
            match touch {
                Touch::Actor(slot) => {
                    self.blood(slot, 0x19);
                    let seg = self.mem.r16(DS, 0x1042);
                    let rec = self.mem.r16(DS, slot.wrapping_add(0x12));
                    target(self, seg, rec);
                }
                Touch::Record(seg, rec) => target(self, seg, rec),
            }
            self.cast_apply(cast)?;
        } else if ty == 3 {
            let at = self.mem.r16(DS, cast.wrapping_add(0x14));
            if (at as i16) >= 0 {
                let (x, y) = (self.mem.r16(DS, at), self.mem.r16(DS, at.wrapping_add(4)));
                self.impact(cast, x, y)?;
            } else {
                self.load_cast_hands(files, gs, spell)?;
                // 29CF:0040: the spell's sound (8) at the player's square.
                self.sound_here(8)?;
                let (x, y) = (self.mem.r16(DS, 0xC3D0), self.mem.r16(DS, 0xC3D4));
                self.impact(cast, x, y)?;
            }
        } else if self.mem.r16(gs, spell.wrapping_add(0x27)) & 0x80 != 0 {
            target(self, DS, PLAYER);
            self.cast_apply(cast)?;
        } else {
            // 29CF:00D2: the fake actor at the player, facing the cursor.
            let r = |m: &Machine, a: u16| m.mem.r16(DS, a);
            for (o, a) in [(0u16, 0xC3D0u16), (2, 0xC3D2), (4, 0xC3D4)] {
                let v = r(self, a);
                self.mem.w16(DS, AIM + o, v);
            }
            let d = (r(self, 0x8F92).wrapping_sub(r(self, 0xC3D6)) as i16 >> 1) as u16;
            let h = d.wrapping_add(r(self, 0xC3DA)) & 0x1FF;
            self.mem.w16(DS, AIM + 0xA, h);
            // DX comes from 00D2's last load: the heading's ax was not
            // it, so it is whatever the caller had; the launch only uses
            // it for element 5, whose delay it sets.
            self.launch_player(files, cast, AIM, 0)?;
        }
        self.cast_done();
        Some(true)
    }

    /// 29CF:08DA with AX 0 for an area spell: only its side effects
    /// matter here (the casting hands load and start, and a random bit is
    /// drawn for the flat it returns).
    fn load_cast_hands(&mut self, files: &mut Files, gs: u16, spell: u16) -> Option<()> {
        let element = self.mem.r8(gs, spell.wrapping_add(0x26)) as u16;
        if element == 5 {
            return Some(());
        }
        let name = self.skip_strings(DS, 0x7D49, element);
        let bank = self.mem.r8(DS, 0xC8F4) as u16;
        self.ems_map(bank);
        let (off, fseg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        let _ = self.file_load(files, name, fseg, off);
        let size = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE);
        self.mem.w16(DS, 0x8038, size.wrapping_add(3) >> 2);
        self.mem.w8(DS, 0x7F80, 0);
        self.rng.next();
        Some(())
    }

    /// 29CF:00AC. After a cast: the weapon class again, the flags saved,
    /// the hands lowered, nothing waiting, no message.
    fn cast_done(&mut self) {
        self.player_weapon_class();
        let b = self.mem.r8(DS, SAVED_FLAGS);
        self.mem.w8(DS, 0x05E0, b);
        self.mem.w8(DS, 0x1012, 0xFF);
        self.mem.w16(DS, 0x1947, 0x1E);
        self.mem.w16(DS, PENDING, 0);
        self.mem.w16(DS, 0x061E, 0);
    }
}
