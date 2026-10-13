//! Applying a cast spell to its targets (29CF:0941) and the spell's
//! three effects (2CDD:000C and its handlers), with elemental resistance
//! (29CF:0B35) and the cures' "end an effect" (2EBD:0004).
//!
//! A cast record (8 of 16h bytes at DGROUP 7F86) holds the spell's far
//! pointer (+0), the caster's (+4), the caster's level + 1 (+8) and
//! flags (+A, bit 1 in use). The targets are the far pointers at DGROUP
//! D5C, D5A of them.
//!
//! A spell record (55h bytes) holds per effect k (0..2) six words of
//! amounts at +0+2k, +6+2k, +C+2k, +12+2k, +18+2k and +1E+2k (base and
//! per-level values), then the target type (+24), element (+26), flags
//! (+27, word), the effect kinds (+29+k), sub-kinds (+2C+k), parameters
//! (+2F+k) and the cost (+32). The handlers address the spell through
//! SI = spell + 2k, so the element and flags they read for the second and
//! third effects are the bytes at +26+2k and +27+2k: that is kept.
//!
//! The handlers are reached through word tables in DGROUP (83E2 by kind,
//! then one per kind indexed by sub-kind, which run into each other);
//! the port reads the same tables and dispatches on the handler address.
//!
//! Creatures and people cast through `actor_cast` (29CF:0145).
//!
//! Not ported: a spell effect that launches a projectile from the player
//! (kind 17 with flag 0x80, 29CF:0270), which returns None.

use crate::character::{field, PLAYER};
use crate::effects::Applied;
use crate::machine::Machine;
use crate::mem::DS;
use crate::scale::to256;

/// DGROUP variables.
pub mod svar {
    /// The caster's level + 1 for the spell being applied.
    pub const CASTER_LEVEL: u16 = 0x1292;
    /// The cast being applied, the spell's flags (+27) and targets a
    /// reflection added.
    pub const CAST: u16 = 0xA886;
    pub const FLAGS: u16 = 0x844F;
    pub const REFLECTED: u16 = 0x1276;
    /// The target list.
    pub const TARGET_COUNT: u16 = 0x0D5A;
    pub const TARGETS: u16 = 0x0D5C;
    /// Handler tables: by kind, and per kind by sub-kind.
    pub const KINDS: u16 = 0x83E2;
    /// Diseases a spell can give a player-like record (14 bytes).
    pub const DISEASES: u16 = 0x83D5;
    /// Elemental resistance flag bits in +41A by element.
    pub const ELEMENT_FLAGS: u16 = 0x7D44;
    /// The message over the view (far pointer, time).
    pub const MESSAGE: u16 = 0x061A;
    pub const MESSAGE_TIME: u16 = 0x061E;
    /// Effect-engine scratch: restoring towards a bound (1).
    pub const RESTORING: u16 = 0xA838;
    /// Set by `end_effect_flag` when a slot was freed.
    pub const ENDED: u16 = 0x1294;
    /// Player-only spell counters (in combat flag 05E6).
    pub const IN_COMBAT: u16 = 0x05E6;
    /// Far pointer to the timed-spell-flag table.
    pub const TIMERS: u16 = 0x1020;
    pub const CHANGED: u16 = 0x0D39;
}

/// The cast records (8 of 16h bytes).
const CASTS: u16 = 0x7F86;

/// Message texts in DGROUP.
const FAILED: u16 = 0x7BFA;
const SUCCEEDED: u16 = 0x7C05;
const DRAINED: u16 = 0x83B6;

/// The registers a handler gets: the spell at effect k (GS:SI), the
/// target (FS:DX), the parameter (BP) and the sub-kind (CX).
#[derive(Clone, Copy)]
struct Fx {
    gs: u16,
    si: u16,
    fs: u16,
    dx: u16,
    bp: u16,
    cx: u16,
}

/// Registers for 2EBD:0102 after a value helper.
#[derive(Clone, Copy, Default)]
struct Args {
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    bp: u16,
}

impl Machine {
    /// 29CF:0941. Applies the cast at DGROUP `cast` to each target in the
    /// list, and frees the cast record. A target that is the caster gets
    /// nothing from an area spell at the caster (type 3) and a self spell
    /// (type 0 or "negative") without its defences; anyone else may stop
    /// it (`spell_stopped`). A reflection adds the caster to the list.
    /// None where the original faults.
    pub fn cast_apply(&mut self, cast: u16) -> Option<()> {
        let flags = self.mem.r16(DS, cast + 0xA) & !1;
        self.mem.w16(DS, cast + 0xA, flags);
        let mut n = self.mem.r16(DS, svar::TARGET_COUNT);
        if n == 0 {
            return Some(());
        }
        self.mem.w16(DS, svar::CAST, cast);
        let (spell, gs) = (self.mem.r16(DS, cast), self.mem.r16(DS, cast + 2));
        let f = self.mem.r16(gs, spell.wrapping_add(0x27));
        self.mem.w16(DS, svar::FLAGS, f);
        self.mem.w16(DS, svar::REFLECTED, 0);
        let mut di = svar::TARGETS;
        loop {
            let level = self.mem.r16(DS, cast + 8);
            let target = self.mem.r32(DS, di);
            let ty = self.mem.r8(gs, spell.wrapping_add(0x24)) as i8;
            let (toff, tseg) = (target as u16, (target >> 16) as u16);
            let direct = if target == self.mem.r32(DS, cast + 4) {
                if ty == 3 {
                    None
                } else {
                    Some(ty <= 0)
                }
            } else {
                Some(false)
            };
            match direct {
                None => {}
                Some(true) => self.spell_effects(gs, spell, tseg, toff, level)?,
                Some(false) => {
                    let cost = self.mem.r16(gs, spell.wrapping_add(0x32));
                    if !self.spell_stopped(tseg, toff, cost, &mut crate::strike::GameHooks)? {
                        self.spell_effects(gs, spell, tseg, toff, level)?;
                    }
                }
            }
            di = di.wrapping_add(4);
            n = n.wrapping_add(self.mem.r16(DS, svar::REFLECTED));
            self.mem.w16(DS, svar::REFLECTED, 0);
            n = n.wrapping_sub(1);
            if n == 0 {
                return Some(());
            }
        }
    }

    /// 29CF:0145. The actor in slot `slot`, record seg:`rec`, casts spell
    /// `index` (not when silenced, spell flag 200h): a cast record is
    /// taken (29CF:01C4, the first not in use; the original searches on
    /// without end) with the caster's level + 1. A spell with a target
    /// type above 0 is launched from the actor towards the player
    /// (aimed, FFFEh at +14); others take effect on the caster at once,
    /// with the reflected-spell flash on its actor. None where the
    /// original faults or hangs.
    pub fn actor_cast(&mut self, seg: u16, rec: u16, slot: u16, index: u8) -> Option<()> {
        if self.mem.r16(seg, rec.wrapping_add(0x41A)) & 0x200 != 0 {
            return Some(());
        }
        let mut cast = CASTS;
        for _ in 0..0x10000 / 0x16 + 1 {
            if self.mem.r16(DS, cast.wrapping_add(0xA)) & 1 == 0 {
                break;
            }
            cast = cast.wrapping_add(0x16);
        }
        if self.mem.r16(DS, cast.wrapping_add(0xA)) & 1 != 0 {
            return None;
        }
        self.mem.w16(DS, cast.wrapping_add(0xA), 1);
        let level = self.mem.r8(seg, rec.wrapping_add(field::LEVEL)) as u16 + 1;
        self.mem.w16(DS, cast.wrapping_add(8), level);
        self.mem.w16(DS, svar::CASTER_LEVEL, level);
        self.mem.w16(DS, cast.wrapping_add(4), rec);
        self.mem.w16(DS, cast.wrapping_add(6), seg);
        let (gs, spell) = self.spell_record(index);
        self.mem.w16(DS, cast, spell);
        self.mem.w16(DS, cast.wrapping_add(2), gs);
        if self.mem.r8(gs, spell.wrapping_add(0x24)) as i8 > 0 {
            let f = self.mem.r16(DS, cast.wrapping_add(0xA)) | 0x20;
            self.mem.w16(DS, cast.wrapping_add(0xA), f);
            self.mem.w16(DS, cast.wrapping_add(0x14), 0xFFFE);
            let (px, py) = (self.mem.r16(DS, 0xC3D0), self.mem.r16(DS, 0xC3D4));
            let h = crate::travel::travel_heading(self.mem.r16(DS, slot), self.mem.r16(DS, slot.wrapping_add(4)), px, py);
            let saved = self.mem.r16(DS, slot.wrapping_add(0xA));
            self.mem.w16(DS, slot.wrapping_add(0xA), h);
            self.launch(cast, slot, py)?;
            self.mem.w16(DS, slot.wrapping_add(0xA), saved);
            return Some(());
        }
        self.mem.w16(DS, svar::TARGETS, rec);
        self.mem.w16(DS, svar::TARGETS + 2, seg);
        self.mem.w16(DS, svar::TARGET_COUNT, 1);
        self.cast_apply(cast)?;
        self.blood(slot, 0x19);
        Some(())
    }

    /// 2CDD:000C. The spell's three effects on the target at tseg:toff,
    /// with the caster's level + 1 in DGROUP 1292. Kinds of 19h and up
    /// (FFh included) do nothing.
    pub fn spell_effects(&mut self, gs: u16, spell: u16, tseg: u16, toff: u16, level: u16) -> Option<()> {
        self.mem.w16(DS, svar::CASTER_LEVEL, level);
        for k in 0..3u16 {
            let at = spell.wrapping_add(k);
            let kind = self.mem.r8(gs, at.wrapping_add(0x29)) as u16;
            let fx = Fx {
                gs,
                si: spell.wrapping_add(2 * k),
                fs: tseg,
                dx: toff,
                bp: self.mem.r8(gs, at.wrapping_add(0x2F)) as u16,
                cx: self.mem.r8(gs, at.wrapping_add(0x2C)) as u16,
            };
            if kind >= 0x19 {
                continue;
            }
            let h = self.mem.r16(DS, svar::KINDS + 2 * kind);
            self.sp_kind(h, fx)?;
        }
        Some(())
    }

    /// The word at the spell's SI + `f`.
    fn sw(&self, x: &Fx, f: u16) -> u16 {
        self.mem.r16(x.gs, x.si.wrapping_add(f))
    }

    /// A handler from a sub-kind table.
    fn sub_handler(&self, table: u16, x: &Fx) -> u16 {
        self.mem.r16(DS, table.wrapping_add(x.cx.wrapping_mul(2)))
    }

    /// The handler of a kind (83E2).
    fn sp_kind(&mut self, h: u16, x: Fx) -> Option<()> {
        match h {
            0x004F => self.sp_sub(self.sub_handler(0x8414, &x), x).map(drop),
            0x0142 => self.sp_sub(self.sub_handler(0x841E, &x), x).map(drop),
            0x01B3 => self.sp_sub_leaky(self.sub_handler(0x8424, &x), x),
            0x020C => self.sp_sub(self.sub_handler(0x842A, &x), x).map(drop),
            0x0278 => self.sp_sub(self.sub_handler(0x8432, &x), x).map(drop),
            0x030F => self.sp_timed_flag1(x),
            0x037D => self.sp_sub_leaky(self.sub_handler(0x8438, &x), x),
            0x03C1 => self.sp_instant_death(x),
            0x03F1 | 0x0712 => Some(()),
            // 03F2 zeroes DI before its call: always the first entry.
            0x03F2 => self.sp_sub(self.mem.r16(DS, 0x843E), x).map(drop),
            0x0447 => self.sp_timed_flag_by_sub(x),
            0x0497 => self.sp_sub(0x04A6, x).map(drop),
            0x04FA => {
                // The flag is set before DS is restored: in the target's
                // segment when the handler left DS there.
                let (_, at_target) = self.sp_sub(self.sub_handler(0x8442, &x), x)?;
                let seg = if at_target { x.fs } else { DS };
                let c = self.mem.r8(seg, svar::CHANGED) | 0x80;
                self.mem.w8(seg, svar::CHANGED, c);
                Some(())
            }
            0x05CE => {
                let (cf, _) = self.sp_sub(self.sub_handler(0x844A, &x), x)?;
                if cf {
                    self.msg(FAILED);
                } else {
                    self.msg(SUCCEEDED);
                }
                self.sp_changed();
                Some(())
            }
            0x0713 => {
                self.sp_or_flags(&x, 1);
                let (seg, di, ax) = self.sp_0885(&x)?;
                self.mem.w16(seg, di, ax);
                Some(())
            }
            0x0723 => self.sp_levitate(x),
            0x075A => self.sp_kind17(x),
            0x078C => {
                let l = self.mem.r16(DS, svar::CASTER_LEVEL);
                self.mem.w16(DS, 0x0DC8, l);
                Some(())
            }
            0x0793 => {
                let l = self.mem.r16(DS, svar::CASTER_LEVEL);
                self.mem.w16(DS, 0x0DC6, l);
                let ax = self.h08b7(&x)?.wrapping_add(self.sw(&x, 0));
                self.mem.w16(DS, 0x0DC4, ax);
                Some(())
            }
            0x07A3 => self.sp_kind20(x),
            0x07DB => self.sp_kind21(x),
            0x0822 => self.sp_timed_pair(x, 0x400, 0x28, false),
            0x0841 => self.sp_timed_pair(x, 0x800, 0x2C, true),
            0x0863 => self.sp_timed_pair(x, 0x1000, 0x30, true),
            _ => None,
        }
    }

    /// Kinds 2 and 6 call their sub-kind handler without saving DS. Their
    /// tables run on into handlers that can leave DS at the target's
    /// segment; for a target outside DGROUP the original then reads the
    /// next handler and 29CF:0941's variables through it and runs into
    /// data: None.
    fn sp_sub_leaky(&mut self, h: u16, x: Fx) -> Option<()> {
        let (_, at_target) = self.sp_sub(h, x)?;
        if at_target && x.fs != DS {
            return None;
        }
        Some(())
    }

    /// A handler from a sub-kind table; returns its carry flag and
    /// whether it left DS at the target's segment: handlers pop the
    /// target's far pointer into DS:SI, and DS is DGROUP again only after
    /// a successful 2EBD:0102 or a 2EBD:0004.
    fn sp_sub(&mut self, h: u16, x: Fx) -> Option<(bool, bool)> {
        let ds = |t: bool| Some((false, t));
        match h {
            0x00CB | 0x03C0 => ds(false),
            0x00CC => ds(self.sp_curse(x)?),
            0x0061 => ds(self.sp_damage_over_time(x)?),
            0x00A5 => ds(self.sp_paralyse(x)?),
            0x010C => ds(self.sp_drain(x)?),
            0x012F => {
                if self.sp_roll(&x)?.0 {
                    let a = Args { ax: 0x100, ..Args::default() };
                    self.sp_apply(&x, 0x100, a);
                }
                ds(true)
            }
            0x0154 => ds(self.sp_burn(x)?),
            0x0180 | 0x019B => ds(self.sp_drain_points(x, h == 0x0180)?),
            0x01BC => {
                let ax = self.h08b7(&x)?.wrapping_add(self.sw(&x, 0));
                self.mem.w16(x.fs, x.dx.wrapping_add(0x41C), ax);
                if x.dx == PLAYER {
                    self.msg(SUCCEEDED);
                }
                ds(false)
            }
            0x01D6 => self.sp_counter(&x, 0x0DC0, false).and(ds(false)),
            0x01F1 => self.sp_counter(&x, 0x0DC2, false).and(ds(false)),
            0x0386 => self.sp_counter(&x, 0x0DC0, true).and(ds(false)),
            0x03A3 => self.sp_counter(&x, 0x0DC2, true).and(ds(false)),
            0x021C | 0x024E | 0x0263 => {
                if !self.sp_roll(&x)?.0 {
                    return ds(true);
                }
                let flag = match h {
                    0x021C => 1,
                    0x024E => 0x40,
                    _ => 4,
                };
                self.end_effect_flag(x.fs, x.dx, flag);
                ds(false)
            }
            0x022F => {
                if !self.sp_roll(&x)?.0 {
                    return ds(true);
                }
                self.end_effect_flag(x.fs, x.dx, 2);
                self.end_effect_flag(x.fs, x.dx, 0x40);
                ds(false)
            }
            0x0288 => {
                let element = self.sw(&x, 0x26) & 0xFF;
                let (ax, _) = self.val_0bca(&x)?;
                self.hp_change(x.fs, x.dx, ax, element)?;
                ds(true)
            }
            0x02F0 => {
                let (ax, _) = self.val_0bca(&x)?;
                let at = x.dx.wrapping_add(field::FATIGUE);
                let v = self.mem.r16(x.fs, at).wrapping_add(ax);
                self.mem.w16(x.fs, at, v);
                ds(true)
            }
            0x02FC => {
                let (ax, _) = self.val_0bca(&x)?;
                let at = x.dx.wrapping_add(field::SPELL_POINTS);
                let v = self.mem.r16(x.fs, at).wrapping_add(ax);
                self.mem.w16(x.fs, at, if (v as i16) < 0 { 0 } else { v });
                ds(true)
            }
            0x0404 => ds(self.sp_attr_both(x)?),
            0x04A6 => ds(self.sp_attr_drain(x)?),
            0x050F => self.sp_fatigue_gain(x).and(ds(true)),
            0x0548 => {
                let a = self.range_0ba6(&x)?;
                let (b, _) = self.range_0bb4(&x)?;
                let hp = x.dx.wrapping_add(field::HP);
                let ax = b.wrapping_add(a).wrapping_add(self.mem.r16(x.fs, hp));
                let max = self.mem.r16(x.fs, x.dx.wrapping_add(field::MAX_HP));
                self.mem.w16(x.fs, hp, if max < ax { max } else { ax });
                ds(false)
            }
            0x0571 => {
                let a = self.range_0ba6(&x)?;
                let (b, _) = self.range_0bb4(&x)?;
                let sp = x.dx.wrapping_add(field::SPELL_POINTS);
                let v = self.mem.r16(x.fs, sp).wrapping_add(b.wrapping_add(a));
                let max = self.mem.r16(x.fs, x.dx.wrapping_add(field::MAX_SPELL_POINTS));
                self.mem.w16(x.fs, sp, if v > max { max } else { v });
                ds(true)
            }
            0x058F => self.sp_restore_attr(x).and(ds(true)),
            0x05F7 => {
                let bp = x.bp.wrapping_mul(2).wrapping_add(8);
                Some((self.sp_transfer(Fx { bp, ..x })?, true))
            }
            0x0604 => Some((self.sp_transfer(x)?, true)),
            _ => None,
        }
    }

    /// 0D39 bit 0x80 (2CDD:09D4).
    fn sp_changed(&mut self) {
        let c = self.mem.r8(DS, svar::CHANGED) | 0x80;
        self.mem.w8(DS, svar::CHANGED, c);
    }

    /// 292B:0456 / 0469 / 2CDD:04E7: a message over the view.
    fn msg(&mut self, text: u16) {
        self.msg_in(DS, text);
    }

    /// A message written with DS at `seg`: where a handler left DS at
    /// the target's segment, the message lands in that segment.
    fn msg_in(&mut self, seg: u16, text: u16) {
        self.mem.w16(seg, svar::MESSAGE_TIME, 0xF);
        self.mem.w16(seg, svar::MESSAGE, text);
        self.mem.w16(seg, svar::MESSAGE + 2, DS);
    }

    fn sp_or_flags(&mut self, x: &Fx, bit: u16) {
        let at = x.dx.wrapping_add(0x41A);
        let v = self.mem.r16(x.fs, at) | bit;
        self.mem.w16(x.fs, at, v);
    }

    /// 2CDD:089A. Caster level + 1 over the spell's +18 (None: divide by 0).
    fn h089a(&self, x: &Fx) -> Option<u16> {
        let d = self.sw(x, 0x18);
        (d != 0).then(|| self.mem.r16(DS, svar::CASTER_LEVEL) / d)
    }

    /// 2CDD:08B7. `h089a` × +6 (the low word).
    fn h08b7(&self, x: &Fx) -> Option<u16> {
        Some(self.h089a(x)?.wrapping_mul(self.sw(x, 6)))
    }

    /// 2CDD:08C3. (Caster level + 1) / +C × +1E + +1E.
    fn h08c3(&self, x: &Fx) -> Option<u16> {
        let d = self.sw(x, 0xC);
        if d == 0 {
            return None;
        }
        let e = self.sw(x, 0x1E);
        Some((self.mem.r16(DS, svar::CASTER_LEVEL) / d).wrapping_mul(e).wrapping_add(e))
    }

    /// 2CDD:0B77. The success roll: a percent under `h089a` × +6 + +0.
    /// The player-target check meant for the messages compares DX after
    /// the multiply has clobbered it with the product's high word.
    /// Returns (CF, that DX).
    fn sp_roll(&mut self, x: &Fx) -> Option<(bool, u16)> {
        let p = self.h089a(x)? as u32 * self.sw(x, 6) as u32;
        let chance = (p as u16).wrapping_add(self.sw(x, 0));
        let hi = (p >> 16) as u16;
        let cf = self.percent() < chance;
        if hi == PLAYER {
            self.msg(if cf { SUCCEEDED } else { FAILED });
        }
        Some((cf, hi))
    }

    /// 2CDD:0BA6. A random amount in +0 ..= +6.
    fn range_0ba6(&mut self, x: &Fx) -> Option<u16> {
        self.rng.range(self.sw(x, 0), self.sw(x, 6).wrapping_add(1))
    }

    /// 2CDD:0BB4. A random amount in +C ..= +12, times `h089a`; returns
    /// the low word and the high word (DX).
    fn range_0bb4(&mut self, x: &Fx) -> Option<(u16, u16)> {
        let r = self.rng.range(self.sw(x, 0xC), self.sw(x, 0x12).wrapping_add(1))?;
        let p = self.h089a(x)? as u32 * r as u32;
        Some((p as u16, (p >> 16) as u16))
    }

    /// 2CDD:0BCA. The negated sum of both random amounts (and DX).
    fn val_0bca(&mut self, x: &Fx) -> Option<(u16, u16)> {
        let a = self.range_0ba6(x)?;
        let (b, dx) = self.range_0bb4(x)?;
        Some((b.wrapping_add(a).wrapping_neg(), dx))
    }

    /// 2CDD:0BD7. `val_0bca`, with CX = +1E, BX = 1, BP = FFFDh.
    fn val_0bd7(&mut self, x: &Fx) -> Option<Args> {
        let (ax, dx) = self.val_0bca(x)?;
        Some(Args { ax, bx: 1, cx: self.sw(x, 0x1E), dx, bp: 0xFFFD })
    }

    /// 2CDD:0BEE. On a successful roll: CX = +1E × (caster level + 1)
    /// (DX its high word), BX = +12, AX = -+C, BP = FFFDh.
    fn val_0bee(&mut self, x: &Fx) -> Option<Option<Args>> {
        if !self.sp_roll(x)?.0 {
            return Some(None);
        }
        let p = self.sw(x, 0x1E) as u32 * self.mem.r16(DS, svar::CASTER_LEVEL) as u32;
        Some(Some(Args {
            ax: self.sw(x, 0xC).wrapping_neg(),
            bx: self.sw(x, 0x12),
            cx: p as u16,
            dx: (p >> 16) as u16,
            bp: 0xFFFD,
        }))
    }

    /// 2CDD:0C0C. On a successful roll: CX = `h08c3`, AX = 0, BX = DX = 1,
    /// BP = FFFDh.
    fn val_0c0c(&mut self, x: &Fx) -> Option<Option<Args>> {
        if !self.sp_roll(x)?.0 {
            return Some(None);
        }
        Some(Some(Args { ax: 0, bx: 1, cx: self.h08c3(x)?, dx: 1, bp: 0xFFFD }))
    }

    /// 2CDD:0C23 (`neg_first` false) / 0C3E: CX = +1E × (caster level +
    /// 1), AX = +0 and BX = -+6, or AX = -+0 and BX = +6; DX = +18,
    /// BP = FFFFh.
    fn val_0c23(&self, x: &Fx, neg_first: bool) -> Args {
        let cx = self.sw(x, 0x1E).wrapping_mul(self.mem.r16(DS, svar::CASTER_LEVEL));
        let (a, b) = (self.sw(x, 0), self.sw(x, 6));
        let (ax, bx) = if neg_first { (a.wrapping_neg(), b) } else { (a, b.wrapping_neg()) };
        Args { ax, bx, cx, dx: self.sw(x, 0x18), bp: 0xFFFF }
    }

    /// 2EBD:0102 on the target with flag `flag` and the helper's
    /// registers (BX period, CX duration, DX phase-B period, BP phase-B
    /// count). Returns CF and where DS:SI then point: the slot, or the
    /// target when nothing was applied (or for flag 0x100).
    fn sp_apply(&mut self, x: &Fx, flag: u16, a: Args) -> (bool, u16, u16) {
        match self.apply_effect(x.fs, x.dx, flag, a.bx, a.cx, a.dx, a.bp) {
            Applied::Slot(Some(s)) => (false, DS, s),
            Applied::Slot(None) => (false, x.fs, x.dx),
            Applied::Refused | Applied::AlreadyActive => (true, x.fs, x.dx),
        }
    }

    /// 2CDD:00CC. A curse: a player-like record (flag 0x4000) catches one
    /// of 14 diseases instead; others get effect flag 1 lowering
    /// attributes 0, 3, 4 and 5 by +C.
    fn sp_curse(&mut self, x: Fx) -> Option<bool> {
        let Some(a) = self.val_0bee(&x)? else { return Some(true) };
        if self.mem.r16(x.fs, x.dx.wrapping_add(field::FLAGS)) & 0x4000 != 0 {
            // The dice are rolled with DS at the target's segment, where
            // the random generator's state is read: only right for the
            // player.
            let r = crate::dice::roll(&mut self.rng, 0xE)?;
            let id = self.mem.r8(DS, svar::DISEASES.wrapping_add(r)) as u16;
            self.apply_disease(x.fs, x.dx, id)?;
            return Some(true);
        }
        let (cf, seg, si) = self.sp_apply(&x, 1, a);
        if !cf {
            for f in [6, 9, 0xA, 0xB] {
                self.mem.w8(seg, si.wrapping_add(f), a.ax as u8);
            }
        }
        Some(cf)
    }

    /// 2CDD:0061. Damage over time (effect flag 2) by the element at
    /// +26: not for targets with spell flag 0x40 or class byte 0Fh. The
    /// phase A period is the element (BX is reloaded with it).
    fn sp_damage_over_time(&mut self, x: Fx) -> Option<bool> {
        if self.mem.r16(x.fs, x.dx.wrapping_add(0x41A)) & 0x40 != 0 || self.mem.r8(x.fs, x.dx.wrapping_add(5)) == 0xF {
            return Some(false);
        }
        let element = self.sw(&x, 0x26) & 0xFF;
        let Some(a) = self.val_0bee(&x)? else { return Some(true) };
        let ax = self.resist(x.fs, x.dx, a.ax, element);
        let (cf, seg, si) = self.sp_apply(&x, 2, Args { bx: element, ..a });
        if !cf {
            self.mem.w16(seg, si.wrapping_add(0xE), ax);
            self.sp_changed();
        }
        Some(cf)
    }

    /// 2CDD:00A5. Paralysis (flag 40h); not for race 4 or class 11h.
    fn sp_paralyse(&mut self, x: Fx) -> Option<bool> {
        let Some(a) = self.val_0c0c(&x)? else { return Some(true) };
        if self.mem.r8(x.fs, x.dx.wrapping_add(4)) == 4 || self.mem.r8(x.fs, x.dx.wrapping_add(5)) == 0x11 {
            return Some(true);
        }
        Some(self.sp_apply(&x, 0x40, a).0)
    }

    /// 2CDD:010C. Effect flag 4 lowering attributes 1, 2, 6 and 7 by +C.
    fn sp_drain(&mut self, x: Fx) -> Option<bool> {
        let Some(a) = self.val_0bee(&x)? else { return Some(true) };
        let (cf, seg, si) = self.sp_apply(&x, 4, a);
        if !cf {
            for f in [7, 8, 0xC, 0xD] {
                self.mem.w8(seg, si.wrapping_add(f), a.ax as u8);
            }
        }
        Some(cf)
    }

    /// 2CDD:0154. Elemental damage over time (flag 200h): the amount goes
    /// through the magic shield and resistance. 29CF:0C4F leaves CX as
    /// the amount's sign, which becomes the duration.
    fn sp_burn(&mut self, x: Fx) -> Option<bool> {
        let element = self.sw(&x, 0x26) & 0xFF;
        let a = self.val_0bd7(&x)?;
        let cx = if (a.ax as i16) < 0 { 0xFFFF } else { 0 };
        let ax = self.absorb_damage(x.fs, x.dx, a.ax);
        let ax = self.resist(x.fs, x.dx, ax, element);
        let (cf, seg, si) = self.sp_apply(&x, 0x200, Args { bx: element, cx, ..a });
        if !cf {
            self.mem.w16(seg, si.wrapping_add(0xE), ax);
        }
        self.sp_changed();
        Some(cf)
    }

    /// 2CDD:0180 (fatigue, `fatigue` true) / 019B (SP): flag 200h with
    /// the amount as phase A's fatigue (×64) or SP delta. 0180 tests the
    /// carry of its `shl ax, 6` instead of 2EBD:0102's, so it stores
    /// through DS:SI even when nothing was applied (into the target).
    fn sp_drain_points(&mut self, x: Fx, fatigue: bool) -> Option<bool> {
        let a = self.val_0bd7(&x)?;
        let (cf, seg, si) = self.sp_apply(&x, 0x200, a);
        if fatigue {
            if a.ax & 0x0400 == 0 {
                self.mem.w16(seg, si.wrapping_add(0x12), a.ax << 6);
            }
        } else if !cf {
            self.mem.w16(seg, si.wrapping_add(0x10), a.ax);
        }
        Some(cf)
    }

    /// 2CDD:01D6 / 01F1 / 0386 / 03A3. For the player in combat (05E6),
    /// a counter gains (or with `neg` loses) +0; out of combat the spell
    /// fails. Other targets: nothing.
    fn sp_counter(&mut self, x: &Fx, at: u16, neg: bool) -> Option<()> {
        if x.dx != PLAYER {
            return Some(());
        }
        if self.mem.r8(DS, svar::IN_COMBAT) == 0 {
            self.msg(FAILED);
            return Some(());
        }
        let v = self.sw(x, 0);
        let v = if neg { v.wrapping_neg() } else { v };
        let n = self.mem.r16(DS, at).wrapping_add(v);
        self.mem.w16(DS, at, n);
        Some(())
    }

    /// 2CDD:029B. An instant HP change by `ax` (negative hurts): through
    /// the magic shield and the element's resistance; records with flag
    /// 0x80 (but not 0x4000) change their maximum too; HP is capped at
    /// the maximum. A player-like record (0x4000) at 0 HP or less dies
    /// (2686:02B4); any other goes through the kill handler.
    fn hp_change(&mut self, seg: u16, off: u16, ax: u16, element: u16) -> Option<()> {
        let ax = self.absorb_damage(seg, off, ax);
        let ax = self.resist(seg, off, ax, element);
        let flags = self.mem.r16(seg, off.wrapping_add(field::FLAGS));
        let (hp_at, max_at) = (off.wrapping_add(field::HP), off.wrapping_add(field::MAX_HP));
        if flags & 0x80 != 0 && flags & 0x4000 == 0 {
            let m = self.mem.r16(seg, max_at).wrapping_add(ax);
            self.mem.w16(seg, max_at, m);
        }
        let hp = self.mem.r16(seg, hp_at).wrapping_add(ax);
        self.mem.w16(seg, hp_at, hp);
        let max = self.mem.r16(seg, max_at);
        if hp as i16 > max as i16 {
            self.mem.w16(seg, hp_at, max);
        }
        if flags & 0x4000 != 0 {
            self.sp_changed();
            if self.mem.r16(seg, hp_at) as i16 <= 0 {
                self.player_dies();
            }
            return Some(());
        }
        self.kill_check(seg, off).map(drop)
    }

    /// 2CDD:09E0 (`spell_flag_table`): the target's block in the
    /// timed-spell-flag table. For a record that is neither the player
    /// nor in an active creature slot the original executes an INT 3 (a
    /// no-op under DOS) and goes on with the record's segment and offset
    /// 0D37.
    fn timer_block(&self, seg: u16, off: u16) -> Option<(u16, u16)> {
        Some(self.spell_flag_table(seg, off))
    }

    /// 2CDD:0885. The target's timer block (as FS:DI) and `h089a` × +6 +
    /// +0; marks a change.
    fn sp_0885(&mut self, x: &Fx) -> Option<(u16, u16, u16)> {
        let (seg, di) = self.timer_block(x.fs, x.dx)?;
        let ax = self.h08b7(x)?.wrapping_add(self.sw(x, 0));
        self.sp_changed();
        Some((seg, di, ax))
    }

    /// 2CDD:030F. On a percent roll up to `h08b7` + +0 (signed): timed
    /// flag 1's timer (`h08c3`) and strength (FFFFh unless the spell's
    /// flags have 100h, then 0) are set, and the *player's* flag 2. The
    /// flag itself is never set on the target.
    fn sp_timed_flag1(&mut self, x: Fx) -> Option<()> {
        let chance = self.h08b7(&x)?.wrapping_add(self.sw(&x, 0));
        if self.percent() as i16 > chance as i16 {
            self.msg(FAILED);
            return Some(());
        }
        let (seg, di) = self.timer_block(x.fs, x.dx)?;
        let t = self.h08c3(&x)?;
        self.mem.w16(seg, di.wrapping_add(4), t);
        let strength = if self.mem.r16(DS, svar::FLAGS) & 0x100 != 0 { 0 } else { 0xFFFF };
        self.mem.w16(seg, di.wrapping_add(6), strength);
        self.msg(SUCCEEDED);
        let f = self.mem.r16(DS, 0x05BE) | 2;
        self.mem.w16(DS, 0x05BE, f);
        Some(())
    }

    /// 2CDD:03C1. Instant death: on a percent roll of at least `h08b7` +
    /// +0 (the comparison is the wrong way round), HP becomes FFFFh and
    /// the kill handler runs. It leaves DS at the target's segment; for
    /// a target outside DGROUP the original then reads its message and
    /// handler tables through that segment and runs into data: None.
    fn sp_instant_death(&mut self, x: Fx) -> Option<()> {
        let chance = self.h08b7(&x)?.wrapping_add(self.sw(&x, 0));
        if chance > self.percent() {
            self.msg(FAILED);
            return Some(());
        }
        self.mem.w16(x.fs, x.dx.wrapping_add(field::HP), 0xFFFF);
        self.kill_check(x.fs, x.dx)?;
        if x.fs != DS {
            // DS is left at the target's segment: the message and every
            // table read after it go through the wrong segment.
            return None;
        }
        self.msg(SUCCEEDED);
        Some(())
    }

    /// 2CDD:0404. Flag 200h moving attribute `bp & 0Fh`: phase A by -+0,
    /// phase B by +6. The success message's player check looks at the
    /// slot pointer, so it always shows.
    fn sp_attr_both(&mut self, x: Fx) -> Option<bool> {
        let a = self.val_0c23(&x, true);
        let (cf, seg, si) = self.sp_apply(&x, 0x200, Args { bx: a.cx, ..a });
        if cf {
            return Some(true);
        }
        let b = x.bp & 0xF;
        self.mem.w8(seg, si.wrapping_add(b).wrapping_add(6), a.ax as u8);
        let si = si.wrapping_add(0x14);
        self.mem.w8(seg, si.wrapping_add(b).wrapping_add(6), a.bx as u8);
        if si != PLAYER {
            self.msg(SUCCEEDED);
        }
        Some(false)
    }

    /// 2CDD:0447. Timed spell flag `sub + 2`: timer (player level + 1) /
    /// +C × +1E, strength `h08b7` + +0. The level is the player's whoever
    /// the target is.
    fn sp_timed_flag_by_sub(&mut self, x: Fx) -> Option<()> {
        let strength = self.h08b7(&x)?.wrapping_add(self.sw(&x, 0));
        let d = self.sw(&x, 0xC);
        if d == 0 {
            return None;
        }
        let lvl = self.mem.r8(DS, PLAYER + field::LEVEL) as u16 + 1;
        let timer = (lvl / d).wrapping_mul(self.sw(&x, 0x1E));
        let (seg, di) = self.timer_block(x.fs, x.dx)?;
        let di = di.wrapping_add(x.cx.wrapping_shl(2)).wrapping_add(8);
        self.mem.w16(seg, di.wrapping_add(2), strength);
        self.mem.w16(seg, di, timer);
        // SHL AX, CL: the count is masked to 5 bits, so 16 and up clear it.
        let bit = (4u32 << (x.cx & 0x1F)) as u16;
        self.sp_or_flags(&x, bit);
        Some(())
    }

    /// 2CDD:04A6. Effect flag 10h lowering attribute `sub` by +0 in phase
    /// A and -+6 in phase B; refused when a drain already holds that
    /// attribute (2EBD:0657) or the effect can't start, with message
    /// 83B6 (written with DS at the target's segment).
    fn sp_attr_drain(&mut self, x: Fx) -> Option<bool> {
        let a = self.val_0c23(&x, false);
        let attr = x.cx;
        if self.attr_drained(attr) {
            self.msg_in(x.fs, DRAINED);
            return Some(true);
        }
        let (cf, seg, si) = self.sp_apply(&x, 0x10, Args { bx: a.cx, ..a });
        if cf {
            self.msg_in(x.fs, DRAINED);
            return Some(true);
        }
        let b = attr & 0xF;
        self.mem.w8(seg, si.wrapping_add(b).wrapping_add(6), a.ax as u8);
        let si = si.wrapping_add(0x14);
        self.mem.w8(seg, si.wrapping_add(b).wrapping_add(6), a.bx as u8);
        Some(false)
    }

    /// 2CDD:050F. Fatigue gains the amount in percent of 256ths, ×64,
    /// capped at (STR + END) × 64. None where the 32-bit division by 100
    /// overflows (negative amounts).
    fn sp_fatigue_gain(&mut self, x: Fx) -> Option<()> {
        let a = self.range_0ba6(&x)?;
        let (b, _) = self.range_0bb4(&x)?;
        let v = b.wrapping_add(a) as i16 as i32 as u32;
        let q = (v << 8) / 100;
        let q = u16::try_from(q).ok()?;
        let fat = x.dx.wrapping_add(field::FATIGUE);
        let ax = (q << 6).wrapping_add(self.mem.r16(x.fs, fat));
        let cap = (self.mem.r8(x.fs, x.dx.wrapping_add(0x29)) as u16 + self.mem.r8(x.fs, x.dx.wrapping_add(0x2E)) as u16) << 6;
        self.mem.w16(x.fs, fat, if ax > cap { cap } else { ax });
        Some(())
    }

    /// 2EBD:0474 with A838 = 1: attribute `attr` moved by AL (a signed
    /// percent: 0478 starts with CBW, scaled to 256ths), bounded by
    /// `snap`.
    fn attr_toward(&mut self, ax: u16, snap: u16, attr: u16) -> Option<u8> {
        self.mem.w16(DS, svar::RESTORING, 1);
        let d = to256(ax as u8 as i8 as i16) as u16;
        if d == 0 {
            return Some(attr as u8);
        }
        let v = attr.wrapping_add(d) as i16 as i32 as u32;
        let mut bounded = 0;
        Some(self.fx_clamp(v, snap, d, &mut bounded) as u8)
    }

    /// 2CDD:058F. Restores attribute `bp & 0Fh` by the amount (a byte),
    /// up to its base value.
    fn sp_restore_attr(&mut self, x: Fx) -> Option<()> {
        let b = x.bp & 0xF;
        let a = self.range_0ba6(&x)?;
        let (c, _) = self.range_0bb4(&x)?;
        let ax = c.wrapping_add(a) & 0xFF;
        let base = self.mem.r8(x.fs, x.dx.wrapping_add(b).wrapping_add(0x31)) as u16;
        let at = x.dx.wrapping_add(b).wrapping_add(0x29);
        let cur = self.mem.r8(x.fs, at) as u16;
        let v = self.attr_toward(ax, base, cur)?;
        self.mem.w8(x.fs, at, v);
        Some(())
    }

    /// 2CDD:0604. A transfer between the target and the player by -+0:
    /// attribute `bp & 0Fh` below 8 (each up to its base, modifiers
    /// recomputed), 8 HP (the target through `hp_change` by element 4,
    /// the player capped), 0Ch SP (not from creatures or targets without
    /// SP; the cap stores the maximum into itself), others fatigue (×64,
    /// capped at (STR + END) × 64, at least 0 when draining). The player
    /// gets the opposite of what the target gets. Returns CF (only SP can
    /// fail).
    fn sp_transfer(&mut self, x: Fx) -> Option<bool> {
        let a = self.val_0c23(&x, true);
        let b = x.bp & 0xF;
        let passes = [(x.fs, x.dx, a.ax), (DS, PLAYER, a.ax.wrapping_neg())];
        match b {
            0..=7 => {
                for (seg, si, ax) in passes {
                    let base = self.mem.r8(seg, si.wrapping_add(b).wrapping_add(0x31)) as u16;
                    let at = si.wrapping_add(b).wrapping_add(0x29);
                    let cur = self.mem.r8(seg, at) as u16;
                    let v = self.attr_toward(ax, base, cur)?;
                    self.mem.w8(seg, at, v);
                    self.attribute_modifiers_at(seg, si);
                }
                Some(false)
            }
            8 => {
                self.hp_change(x.fs, x.dx, a.ax, 4)?;
                let hp = PLAYER + field::HP;
                let v = self.mem.r16(DS, hp).wrapping_add(a.ax.wrapping_neg());
                self.mem.w16(DS, hp, v);
                let max = self.mem.r16(DS, PLAYER + field::MAX_HP);
                if v as i16 > max as i16 {
                    self.mem.w16(DS, hp, max);
                }
                Some(false)
            }
            0xC => {
                let sp = x.dx.wrapping_add(field::SPELL_POINTS);
                if self.mem.r16(x.fs, x.dx.wrapping_add(field::FLAGS)) & 0x1000 != 0 || self.mem.r16(x.fs, sp) == 0 {
                    return Some(true);
                }
                for (seg, si, ax) in passes {
                    let at = si.wrapping_add(field::SPELL_POINTS);
                    let v = self.mem.r16(seg, at).wrapping_add(ax);
                    self.mem.w16(seg, at, v);
                    if (v as i16) < 0 {
                        self.mem.w16(seg, at, 0);
                    } else {
                        let max_at = si.wrapping_add(field::MAX_SPELL_POINTS);
                        let max = self.mem.r16(seg, max_at);
                        if v as i16 > max as i16 {
                            self.mem.w16(seg, max_at, max);
                        }
                    }
                }
                Some(false)
            }
            _ => {
                for (seg, si, ax) in passes {
                    let at = si.wrapping_add(field::FATIGUE);
                    let d = ax << 6;
                    let v = self.mem.r16(seg, at).wrapping_add(d);
                    self.mem.w16(seg, at, v);
                    if (d as i16) < 0 && (v as i16) < 0 {
                        self.mem.w16(seg, at, 0);
                    } else {
                        let cap = (self.mem.r8(seg, si.wrapping_add(0x29)) as u16 + self.mem.r8(seg, si.wrapping_add(0x2E)) as u16) << 6;
                        if v > cap {
                            self.mem.w16(seg, at, cap);
                        }
                    }
                }
                Some(false)
            }
        }
    }

    /// 2CDD:0723. Levitation: refused when the player is not above the
    /// floor (C3D2 <= 0) on a square with a wall; otherwise flag 80h, its
    /// timer, and 2826:0244 (a pending map change at the player's square,
    /// level flag 4000h).
    fn sp_levitate(&mut self, x: Fx) -> Option<()> {
        if self.mem.r16(DS, 0xC3D2) as i16 <= 0 {
            let walls = self.mem.r16(DS, 0xD806);
            if self.mem.r16(walls, self.mem.r16(DS, 0xCB90)) != 0 {
                self.msg(FAILED);
                return Some(());
            }
        }
        self.sp_or_flags(&x, 0x80);
        let (seg, di, ax) = self.sp_0885(&x)?;
        self.mem.w16(seg, di.wrapping_add(0x1C), ax);
        // 2826:0244
        let f = self.mem.r16(DS, 0xC39A);
        if f & 0x4000 != 0 {
            self.mem.w16(DS, 0xC39A, f & !0x4000);
            let g = self.mem.r16(DS, 0x05CA) & !0x10;
            self.mem.w16(DS, 0x05CA, g);
            let walls = self.mem.r16(DS, 0xD806);
            let v = 0x8000 | self.mem.r8(DS, 0xC942).wrapping_add(1) as u16;
            self.mem.w16(walls, self.mem.r16(DS, 0xCB90), v);
        }
        Some(())
    }

    /// 2CDD:075A. With flag 80h at +27 a projectile from the player (not
    /// ported: None); otherwise timed flag 100h with timer +1E × (caster
    /// level + 1) and strength +0.
    fn sp_kind17(&mut self, x: Fx) -> Option<()> {
        if self.sw(&x, 0x27) & 0x80 != 0 {
            return None; // 29CF:0270
        }
        self.sp_or_flags(&x, 0x100);
        let (seg, di) = self.timer_block(x.fs, x.dx)?;
        let t = self.sw(&x, 0x1E).wrapping_mul(self.mem.r16(DS, svar::CASTER_LEVEL));
        self.mem.w16(seg, di.wrapping_add(0x20), t);
        self.mem.w16(seg, di.wrapping_add(0x22), self.sw(&x, 0));
        Some(())
    }

    /// 2CDD:07A3. Effect flag 80h: period +6, duration +18 × (caster
    /// level + 1), phase A HP delta +0.
    fn sp_kind20(&mut self, x: Fx) -> Option<()> {
        let cx = self.sw(&x, 0x18).wrapping_mul(self.mem.r16(DS, svar::CASTER_LEVEL));
        let ax = self.sw(&x, 0);
        let a = Args { ax, bx: self.sw(&x, 6), cx, dx: 1, bp: 0xFFFD };
        let (cf, seg, si) = self.sp_apply(&x, 0x80, a);
        if !cf {
            self.mem.w16(seg, si.wrapping_add(0xE), ax);
        }
        self.sp_changed();
        Some(())
    }

    /// 2CDD:07DB. Timed flag 200h on a percent roll up to `h08b7` plus 5
    /// × (caster level + 1 - target level), 8-bit; timer `h08c3`.
    fn sp_kind21(&mut self, x: Fx) -> Option<()> {
        let lvl = self.mem.r8(x.fs, x.dx.wrapping_add(6)) as u16;
        let bonus = (lvl.wrapping_sub(self.mem.r16(DS, svar::CASTER_LEVEL)).wrapping_neg() as u8 as u16) * 5;
        let chance = self.h08b7(&x)?.wrapping_add(bonus);
        if self.percent() as i16 > chance as i16 {
            if x.dx == PLAYER {
                self.msg(FAILED);
            }
            return Some(());
        }
        self.sp_or_flags(&x, 0x200);
        let (seg, di) = self.timer_block(x.fs, x.dx)?;
        let t = self.h08c3(&x)?;
        self.mem.w16(seg, di.wrapping_add(0x24), t);
        if x.dx == PLAYER {
            self.msg(SUCCEEDED);
        }
        Some(())
    }

    /// 2CDD:0822 / 0841 / 0863. Timed flag `bit`: strength `h08b7` (plus
    /// +0 with `plus`) at entry `entry` + 2, timer `h08c3` at `entry`.
    fn sp_timed_pair(&mut self, x: Fx, bit: u16, entry: u16, plus: bool) -> Option<()> {
        self.sp_or_flags(&x, bit);
        let mut v = self.h08b7(&x)?;
        let (seg, di) = self.timer_block(x.fs, x.dx)?;
        if plus {
            v = v.wrapping_add(self.sw(&x, 0));
        }
        self.mem.w16(seg, di.wrapping_add(entry + 2), v);
        let t = self.h08c3(&x)?;
        self.mem.w16(seg, di.wrapping_add(entry), t);
        self.sp_changed();
        Some(())
    }

    /// 29CF:0B35. Elemental resistance on `ax` (negative hurts) for the
    /// record at seg:off: first WIL (+2B) / 26 is taken off the amount's
    /// size (0 when that crosses zero). For elements below 5 a percent
    /// roll (8-bit) against the record's resistance byte (+60 + element)
    /// halves it on a miss; race 2 against element 1 and race 0 against
    /// element 4 halve it on a hit and nullify it on a miss (people
    /// only), and the element's timed flag (7D44) halves or nullifies it
    /// likewise. Creature types 11h-13h then double, halve or negate
    /// some elements.
    pub fn resist(&mut self, seg: u16, off: u16, ax: u16, element: u16) -> u16 {
        let r = self.mem.r8(seg, off.wrapping_add(0x2B)) as u16 / 0x1A;
        let mut ax = if (ax as i16) >= 0 {
            let v = ax.wrapping_sub(r);
            if v as i16 <= 0 {
                return 0;
            }
            v
        } else {
            let v = ax.wrapping_add(r);
            if v as i16 >= 0 {
                return 0;
            }
            v
        };
        if element >= 5 {
            return ax;
        }
        let creature = self.mem.r16(seg, off.wrapping_add(field::FLAGS)) & 0x1000 != 0;
        let race = self.mem.r8(seg, off.wrapping_add(4));
        let sar = |v: u16| ((v as i16) >> 1) as u16;
        let roll = self.percent() as u8;
        let hit = roll < self.mem.r8(seg, off.wrapping_add(element).wrapping_add(0x60));
        let racial = !creature && ((race == 2 && element == 1) || (race == 0 && element == 4));
        if racial {
            return if hit { sar(ax) } else { 0 };
        }
        if !hit {
            ax = sar(ax);
        }
        let bit = self.mem.r8(DS, svar::ELEMENT_FLAGS.wrapping_add(element));
        if self.mem.r8(seg, off.wrapping_add(0x41A)) & bit != 0 {
            ax = if hit { sar(ax) } else { 0 };
        }
        if creature {
            match self.mem.r8(seg, off.wrapping_add(4)) {
                0x11 if element == 0 => ax = ax.wrapping_add(ax),
                0x12 | 0x13 if element <= 1 => ax = sar(ax),
                0x12 if element == 3 => ax = ax.wrapping_add(ax),
                0x13 if element == 3 => ax = ax.wrapping_neg(),
                _ => {}
            }
        }
        ax
    }

    /// 2EBD:0004. Ends effect `flag` on the record at seg:off: clears the
    /// flag and frees every slot of that kind aimed at it (kind 0 also
    /// takes the diseases, 10 and up); the kind's counter is decremented
    /// in DGROUP at the record's offset (whatever its segment), and
    /// zeroed in the record when a slot was freed. Flag 100h only clears.
    pub fn end_effect_flag(&mut self, seg: u16, off: u16, flag: u16) {
        let at = off.wrapping_add(field::FLAGS);
        let f = self.mem.r16(seg, at) & !flag;
        self.mem.w16(seg, at, f);
        if flag == 0x100 {
            return;
        }
        let kind = self.effect_kind(flag as u8);
        if kind < 9 {
            let c = off.wrapping_add(kind).wrapping_add(0x40F);
            let v = self.mem.r8(DS, c).wrapping_sub(1);
            self.mem.w8(DS, c, v);
        }
        self.mem.w16(DS, svar::ENDED, 0);
        let bl = kind as u8;
        for i in 0..crate::effects::NSLOTS {
            let s = crate::effects::SLOTS + i * crate::effects::SLOT_SIZE;
            if self.fx_target(s) != (seg, off) {
                continue;
            }
            let k = self.mem.r8(DS, s + crate::effects::slot::KIND);
            if (bl == 0 && (k >= 10 || k == 0)) || (bl != 0 && k == bl) {
                self.mem.w16(DS, svar::ENDED, 1);
                self.clear_slot(s);
            }
        }
        if self.mem.r16(DS, svar::ENDED) != 0 {
            self.mem.w8(seg, off.wrapping_add(kind).wrapping_add(0x40F), 0);
        }
    }
}
