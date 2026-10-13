//! The player's strike on an actor (0000:3B6D, 2AA1:0000) and the kill
//! handler every kind of damage goes through (2AA1:06A0): death, the
//! corpse, experience, and quest targets that die or give up.
//!
//! Choosing whom a swing hits (0000:3988's loop over the actors in
//! reach) is not ported here; `strike_actor` is what it calls for an
//! actor in a creature slot. Not ported: the sound (23BA:002A) and the
//! text a quest target says when giving up (0000:9AE6, 9CA0, 2DA2:000A,
//! 1194:59C8), which only shows a message.

use crate::character::{field, PLAYER};
use crate::combat::{var, Gear, Ptr};
use crate::effects::FxHooks;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod svar {
    /// Arguments of the player's strike: AX (behind the target), DX
    /// (the actor slot).
    pub const ARG: u16 = 0x1298;
    pub const SLOT: u16 = 0x129E;
    /// Frames until the player can strike again.
    pub const COOLDOWN: u16 = 0x653E;
    /// The effect list beside the actors (1Ch bytes per actor slot).
    pub const EFFECTS: u16 = 0x5F6A;
    pub const ACTORS: u16 = 0x0AB3;
    pub const CREATURE_ACTORS: u16 = 0x0C57;
    /// Blood flat per creature type (from type 1).
    pub const BLOOD: u16 = 0x82A3;
    /// Flats: far pointers (offset, EMS bank) per flat; the EMS frame.
    pub const FLAT_DATA: u16 = 0xD0B8;
    pub const FRAME: u16 = 0x9030;
    /// The wall plane's segment.
    pub const WALLS: u16 = 0xD806;
    /// Bit 0x40: a quest target is being fought; 0x100 set by a kill.
    pub const FLAGS_5CA: u16 = 0x05CA;
    /// The four quest records (34h bytes each).
    pub const QUESTS: u16 = 0x0E6E;
    /// Items a quest target hands over (9 bytes, FF free).
    pub const QUEST_ITEMS: u16 = 0x0FD4;
    pub const QUEST_ITEM_BASE: u16 = 0x0A46;
    pub const PICK_BASE: u16 = 0x128A;
    pub const PICK: u16 = 0x128C;
    /// The critical-hit message pointer and time.
    pub const MESSAGE: u16 = 0x061A;
    pub const MESSAGE_TIME: u16 = 0x061E;
    pub const CRITICAL_TEXT: u16 = 0x7C10;
}

/// Actor slot fields used here.
mod f {
    pub const SPRITE: u16 = 0x0C;
    pub const FRAME: u16 = 0x0D;
    pub const EFFECT_FLAGS: u16 = 0x0E;
    pub const FLAGS: u16 = 0x0F;
    pub const RECORD: u16 = 0x12;
}

impl Machine {
    /// 0000:3B6D. The player hits the actor in slot `si` (a creature
    /// slot). It works out whether the player is behind the actor
    /// (0000:3D17, from the heading to the actor's target pointer), but
    /// the `xor ax, ax` before its JAE clears that carry: AX is always 0.
    pub fn strike_actor(&mut self, si: u16) -> Option<()> {
        self.mem.w16(DS, var::ATTACKER, PLAYER);
        let rec = self.mem.r16(DS, si + f::RECORD);
        self.mem.w16(DS, var::DEFENDER, rec);
        self.player_attack(0, si)
    }

    /// 2AA1:0000. The player (DGROUP 1400 = 01A4) hits the record at 1402
    /// in the creature segment, shown by actor slot `slot`. A weapon that
    /// can't hurt a special creature (see `combat::special_low`) only
    /// makes a sound. None where the original faults or hangs.
    pub fn player_attack(&mut self, arg: u16, slot: u16) -> Option<()> {
        self.mem.w16(DS, svar::ARG, arg);
        self.mem.w16(DS, svar::SLOT, slot);
        let location = self.hit_location()?;
        let att = Ptr::new(DS, self.mem.r16(DS, var::ATTACKER));
        let def = Ptr::new(self.mem.r16(DS, var::CREATURE_SEG), self.mem.r16(DS, var::DEFENDER));
        let armor = self.armor_at(def.seg, def.off, location) as i16;
        let (hit, _) = self.roll_to_hit(att, def, location, &Gear { armor, ..Gear::default() })?;
        if !hit {
            return Some(());
        }
        let w = Ptr::new(DS, self.equipped_weapon(DS, PLAYER));
        let (cf, ax) = self.special_low(w);
        if !cf {
            if ax != 0xFFFF {
                return Some(());
            }
            let (cf, ax) = self.special_high(w);
            if !cf && ax != 0xFFFF {
                return Some(());
            }
        }
        // 2AA1:004E: blood.
        let mut sprite = 0x18;
        let ty = self.mem.r8(def.seg, def.off.wrapping_add(field::RACE)) as u16;
        if self.mem.r16(def.seg, def.off.wrapping_add(field::FLAGS)) & field::FLAG_CREATURE != 0 {
            let mut bx = ty.wrapping_sub(1);
            if bx == 0x29 {
                bx = 0x17;
            }
            sprite = self.mem.r8(DS, svar::BLOOD.wrapping_add(bx)) as u16;
        }
        self.blood(self.mem.r16(DS, svar::SLOT), sprite);

        let m = self.missile_weapon(DS, att.off);
        let mult = self.critical_multiplier(att, &Gear { missile: m.is_some(), missile_cx: m.unwrap_or(0), ..Gear::default() });
        if mult == 3 {
            // 292B:047C, as the attacker is in DGROUP.
            self.mem.w16(DS, svar::MESSAGE_TIME, 0xF);
            self.mem.w16(DS, svar::MESSAGE, svar::CRITICAL_TEXT);
            self.mem.w16(DS, svar::MESSAGE + 2, DS);
        }
        let gear = self.player_gear();
        let (damage, broken) = self.weapon_damage(DS, DS, mult, &gear)?;
        if let Some(it) = broken {
            self.item_broken(it)?;
        }
        let damage = self.creature_damage_taken(damage);
        self.armor_wear(def.seg, def.off, location, damage)?;
        let find = |m: &Self, ty: u8| match m.find_equipped_type(DS, crate::inventory::PLAYER_SLOTS, ty) {
            (true, it) => Some(it),
            _ => None,
        };
        let has_85 = find(self, 0x85).is_some();
        let item_88 = if has_85 { None } else { find(self, 0x88) };
        let (damage, broken) = self.weapon_enchantments(damage, has_85, item_88);
        if let Some(it) = broken {
            self.item_broken(it)?;
        }
        let hp_at = def.off.wrapping_add(field::HP);
        let hp = self.mem.r16(def.seg, hp_at).wrapping_sub(damage);
        self.mem.w16(def.seg, hp_at, hp);
        let has_84 = find(self, 0x84).is_some();
        self.vampiric_heal(damage, has_84);
        self.kill_check(def.seg, def.off)?;
        self.mem.w16(DS, svar::COOLDOWN, 0x14);
        Some(())
    }

    /// The player's side of 2AA1:04A5: weapon (1194:7510), else gauntlets
    /// (1194:7449 from SI = FFFF), else the racial bonus (1194:39ED).
    fn player_gear(&mut self) -> Gear {
        let w = self.equipped_weapon(DS, PLAYER);
        if w == 0xFFFF {
            let (found, g) = self.find_player_equipped(DS, 0xFFFF, 1, 0);
            return Gear { gauntlets: found.then_some(g), ..Gear::default() };
        }
        Gear { weapon: Some(w), weapon_bonus: self.weapon_bonus(DS, PLAYER), ..Gear::default() }
    }

    /// 0000:27E5. Blood (flat `sprite`) on actor slot `si`, in the effect
    /// list beside the actors: at its position, 30 units up, with the
    /// flat's byte 0Bh (read from the EMS frame) plus 0x80 as its flags.
    pub fn blood(&mut self, si: u16, sprite: u16) {
        let e = si.wrapping_sub(svar::ACTORS).wrapping_add(svar::EFFECTS);
        let (x, y) = (self.mem.r16(DS, si), self.mem.r16(DS, si.wrapping_add(4)));
        self.mem.w16(DS, e, x);
        self.mem.w16(DS, e.wrapping_add(2), 0xFFE2);
        self.mem.w16(DS, e.wrapping_add(4), y);
        self.mem.w8(DS, e.wrapping_add(f::SPRITE), sprite as u8);
        self.mem.w16(DS, e.wrapping_add(f::FLAGS), 0);
        let entry = svar::FLAT_DATA.wrapping_add(sprite.wrapping_shl(2));
        let off = self.mem.r16(DS, entry);
        self.ems_map(self.mem.r16(DS, entry.wrapping_add(2)));
        let frame = self.mem.r16(DS, svar::FRAME);
        let b = self.mem.r8(frame, off.wrapping_add(0xB)) | 0x80;
        self.mem.w8(DS, e.wrapping_add(f::EFFECT_FLAGS), b);
        self.mem.w8(DS, e.wrapping_add(f::FRAME), 0);
        let fl = self.mem.r16(DS, e.wrapping_add(f::FLAGS)) | 0x100;
        self.mem.w16(DS, e.wrapping_add(f::FLAGS), fl);
    }

    /// 2AA1:06A0. After the record at seg:`rec` lost HP: at 0 or below it
    /// dies (`record_dies`) unless it is a quest target giving up, and
    /// the player gets its experience, except for records with flag 0x80
    /// (other than type 17h) whose maximum HP is still above 0. Returns
    /// whether the player was given experience (CF). None where the
    /// original hangs.
    pub fn kill_check(&mut self, seg: u16, rec: u16) -> Option<bool> {
        self.end_timed_flag();
        if self.mem.r16(seg, rec.wrapping_add(field::HP)) as i16 > 0 {
            self.quest_yield(seg, rec)?;
            return Some(false);
        }
        if self.quest_yield(seg, rec)? {
            return Some(false);
        }
        let xp = self.record_dies(seg, rec)?;
        let flags = self.mem.r16(seg, rec.wrapping_add(field::FLAGS));
        if flags & 0x80 != 0 && self.mem.r8(seg, rec.wrapping_add(field::RACE)) != 0x17 {
            if self.mem.r16(seg, rec.wrapping_add(field::MAX_HP)) as i16 > 0 {
                self.quest_yield(seg, rec)?;
                return Some(false);
            }
            self.clear_effects_on(seg, rec);
        }
        let at = PLAYER + 0x409;
        let total = self.mem.r32(DS, at).wrapping_add(xp);
        self.mem.w32(DS, at, total);
        let f = self.mem.r16(DS, svar::FLAGS_5CA) | 0x100;
        self.mem.w16(DS, svar::FLAGS_5CA, f);
        Some(true)
    }

    /// 2CDD:034A. With spell flag 2 on the player (05BE), its timer entry
    /// (far pointer at DGROUP 1020) is cleared once its high word is 0.
    fn end_timed_flag(&mut self) {
        if self.mem.r16(DS, 0x05BE) & 2 == 0 {
            return;
        }
        let (off, seg) = (self.mem.r16(DS, 0x1020), self.mem.r16(DS, 0x1022));
        if self.mem.r16(seg, off.wrapping_add(6)) != 0 {
            return;
        }
        self.mem.w16(seg, off.wrapping_add(4), 0);
        self.mem.w16(seg, off.wrapping_add(6), 0);
        let f = self.mem.r16(DS, 0x05BE) & !2;
        self.mem.w16(DS, 0x05BE, f);
    }

    /// 2AA1:06F6. The record at seg:`rec` dies (unless already dead and
    /// without flag 0x80): its effects end (not for creature types 0Eh and
    /// 16h), all its items come off and count as unidentified, a quest
    /// waiting for it (DGROUP 05CA bit 0x40) is marked done, and a
    /// creature's actor becomes a corpse. Returns its experience value.
    fn record_dies(&mut self, seg: u16, rec: u16) -> Option<u32> {
        let flags = self.mem.r16(seg, rec.wrapping_add(field::FLAGS));
        if flags & 0x80 == 0 && flags & 0x100 != 0 {
            return Some(0);
        }
        let ty = self.mem.r8(seg, rec.wrapping_add(field::RACE));
        let creature = flags & field::FLAG_CREATURE != 0;
        if !(creature && (ty == 0xE || ty == 0x16)) {
            self.clear_effects_on(seg, rec);
        }
        let mut it = rec.wrapping_add(crate::inventory::SLOTS);
        for _ in 0..crate::inventory::SLOT_COUNT {
            let at = it.wrapping_add(crate::item::FLAGS);
            let b = self.mem.r8(seg, at) & 0x7F | crate::item::UNIDENTIFIED;
            self.mem.w8(seg, at, b);
            it = it.wrapping_add(crate::item::SIZE);
        }
        if self.mem.r16(DS, svar::FLAGS_5CA) & 0x40 != 0 {
            if let Some(q) = self.quest_waiting_for(self.mem.r8(seg, rec.wrapping_add(field::RACE))) {
                let v = self.mem.r16(DS, q.wrapping_add(4)) | 0x20;
                self.mem.w16(DS, q.wrapping_add(4), v);
                let f = self.mem.r16(DS, svar::FLAGS_5CA) & !0x40;
                self.mem.w16(DS, svar::FLAGS_5CA, f);
            }
        }
        let flags = self.mem.r16(seg, rec.wrapping_add(field::FLAGS)) | 0x100;
        self.mem.w16(seg, rec.wrapping_add(field::FLAGS), flags);
        let xp = self.mem.r32(seg, rec.wrapping_add(0x409));
        if flags & field::FLAG_CREATURE != 0 {
            let slot = self.actor_of(rec)?;
            self.mem.w8(DS, slot + f::SPRITE, 0xC5);
            self.mem.w8(DS, slot + f::FRAME, 0);
            self.corpse_off_door(slot);
        }
        Some(xp)
    }

    /// 2AA1:0842. The actor slot, from the first creature slot on, whose
    /// record is `rec`. The original searches on without end; None when
    /// no slot in DGROUP matches.
    fn actor_of(&self, rec: u16) -> Option<u16> {
        let mut si = svar::CREATURE_ACTORS;
        for _ in 0..0x10000 / 0x1C + 1 {
            if self.mem.r16(DS, si.wrapping_add(f::RECORD)) == rec {
                return Some(si);
            }
            si = si.wrapping_add(0x1C);
        }
        None
    }

    /// 0000:2858. A corpse on a door square is moved half a square off
    /// it, along x when the square north of it is a wall, else along y:
    /// to the far side of the next square if it was in the door's
    /// second half, else back into the previous square.
    fn corpse_off_door(&mut self, si: u16) {
        let (x, y) = (self.mem.r16(DS, si), self.mem.r16(DS, si.wrapping_add(4)));
        let sq = (y >> 7) << 8 | ((x >> 7) as u8).wrapping_mul(2) as u16;
        let walls = self.mem.r16(DS, svar::WALLS);
        if (self.mem.r16(walls, sq) >> 12) as u8 != 0xB {
            return;
        }
        let at = if self.mem.r16(walls, sq.wrapping_sub(0x100)) != 0 { si } else { si.wrapping_add(4) };
        let v = self.mem.r16(DS, at);
        let n = if v & 0x7F >= 0x40 { (v & 0xFF80).wrapping_add(0xC0) } else { (v & 0xFF80).wrapping_sub(0x40) };
        self.mem.w16(DS, at, n);
    }

    /// 2AA1:07A4. A person being fought as a quest target (05CA bit 0x40)
    /// gives up below half its maximum HP, if a quest at stage 4 names
    /// its class (1194:A1D1): it hands over an item (1194:3829), the
    /// quest is marked, its effects end and its actor turns peaceful
    /// (flag 0x4000). CF.
    fn quest_yield(&mut self, seg: u16, rec: u16) -> Option<bool> {
        if self.mem.r16(DS, svar::FLAGS_5CA) & 0x40 == 0
            || self.mem.r16(seg, rec.wrapping_add(field::FLAGS)) & field::FLAG_CREATURE != 0
        {
            return Some(false);
        }
        let half = self.mem.r16(seg, rec.wrapping_add(field::MAX_HP)) >> 1;
        if half as i16 <= self.mem.r16(seg, rec.wrapping_add(field::HP)) as i16 {
            return Some(false);
        }
        let class = (self.mem.r8(seg, rec.wrapping_add(field::CLASS)) & 0x1F).wrapping_add(0x18);
        let Some(q) = self.quest_matching(4, 0x2F, class) else { return Some(false) };
        let (item, slot) = self.pick_quest_item(self.mem.r8(DS, svar::QUEST_ITEM_BASE) as u16)?;
        self.mem.w8(DS, svar::QUEST_ITEMS.wrapping_add(slot), item as u8);
        let c = self.mem.r8(DS, crate::attack::avar::CHANGED) | 0x80;
        self.mem.w8(DS, crate::attack::avar::CHANGED, c);
        self.mem.w8(DS, q.wrapping_add(0x1C), item as u8);
        let v = self.mem.r16(DS, q.wrapping_add(4)) | 0x20;
        self.mem.w16(DS, q.wrapping_add(4), v);
        self.clear_effects_on(seg, rec);
        let a = self.actor_of(rec)?;
        let fl = self.mem.r16(DS, a + f::FLAGS) | 0x4000;
        self.mem.w16(DS, a + f::FLAGS, fl);
        Some(true)
    }

    /// 1194:A19C. A quest at stage 6 waiting for the death of a creature
    /// of type `ty`.
    fn quest_waiting_for(&self, ty: u8) -> Option<u16> {
        self.quest_matching(6, 0x30, ty)
    }

    /// 1194:A19C / A1D1: the first quest of kind 1 (word +4 bits 6-7) at
    /// `stage` (bits 9-11); it counts only if its byte at `field` is
    /// `v`, and the search ends there either way.
    fn quest_matching(&self, stage: u16, field: u16, v: u8) -> Option<u16> {
        let mut q = svar::QUESTS;
        for _ in 0..4 {
            let w = self.mem.r16(DS, q.wrapping_add(4));
            if (w & 0xC0) >> 6 == 1 && (w & 0xE00) >> 9 == stage {
                return (self.mem.r8(DS, q.wrapping_add(field)) == v).then_some(q);
            }
            q = q.wrapping_add(0x34);
        }
        None
    }

    /// 1194:3829. A random item 10 * `base` + 0..9 not yet in the list at
    /// 0FD4 (9 entries, FF free), and the list's first free index (9 when
    /// full, which is past the list).
    fn pick_quest_item(&mut self, base: u16) -> Option<(u16, u16)> {
        let base = base.wrapping_mul(10);
        self.mem.w16(DS, svar::PICK_BASE, base);
        let mut tries = crate::random::CycleCheck::new(&self.rng);
        let item = loop {
            let r = crate::dice::roll(&mut self.rng, 10)?;
            self.mem.w16(DS, svar::PICK, r);
            let v = r.wrapping_add(base);
            let taken = (0..9).map(|i| self.mem.r8(DS, svar::QUEST_ITEMS + i)).any(|b| b != 0xFF && b == v as u8);
            if !taken {
                break v;
            }
            if tries.cycled(&self.rng) {
                return None;
            }
        };
        let free = (0..9).find(|&i| self.mem.r8(DS, svar::QUEST_ITEMS + i) == 0xFF).unwrap_or(9);
        Some((item, free))
    }
}

/// The game's handlers for the effects engine: the kill handler, player
/// death and the blood flash of a reflected spell.
pub struct GameHooks;

impl FxHooks for GameHooks {
    /// 2AA1:06A0. Where the original would hang (no actor slot for a
    /// dying creature, see `kill_check`) this reports no kill.
    fn vitals_changed(&mut self, m: &mut Machine, seg: u16, rec: u16) -> bool {
        m.kill_check(seg, rec).unwrap_or(false)
    }

    fn player_died(&mut self, m: &mut Machine) {
        m.player_dies();
    }

    fn spell_reflected(&mut self, m: &mut Machine, actor: u16) {
        m.blood(actor, 0x19);
    }
}
