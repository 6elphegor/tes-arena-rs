//! The player's strike on an actor (0000:3B6D, 2AA1:0000) and the kill
//! handler every kind of damage goes through (2AA1:06A0): death, the
//! corpse, experience, and quest targets that die or give up.
//!
//! A swing (0000:3988, driven by the mouse) lands on every actor in
//! reach in front of the player (`swing_hits`): people die at once
//! (`strike_person`), others take a blow (`strike_actor`). A swing that
//! reaches no actor hits the wall ahead (0000:3D43: doors, locks and
//! crime, `Machine::bash` in weapon.rs). The sounds (23BA:002A) are
//! played as the original plays them. A quest target giving up says so
//! (`surrender_text`, then the front end's box: `KillHooks`).

use crate::character::{field, PLAYER};
use crate::combat::{var, Gear, Ptr};
use crate::effects::FxHooks;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::DS;
use crate::travel::travel_heading;

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
    /// The player: position, height above the floor, heading, gold.
    pub const PX: u16 = 0xC3D0;
    pub const PY: u16 = 0xC3D4;
    pub const HEIGHT: u16 = 0xC3D2;
    pub const HEADING: u16 = 0xC3DA;
    pub const GOLD: u16 = 0x05C2;
    /// Actors a swing reached.
    pub const HITS: u16 = 0x67DA;
    pub const GOLD_TEXT: u16 = 0x6702;
    pub const GOLD_FORMAT: u16 = 0x6728;
    /// The critical-hit message pointer and time.
    pub const MESSAGE: u16 = 0x061A;
    pub const MESSAGE_TIME: u16 = 0x061E;
    pub const CRITICAL_TEXT: u16 = 0x7C10;
    /// A quest target's words on giving up, and where they are made.
    pub const SURRENDER: u16 = 0x82E6;
    pub const SURRENDER_LINE: u16 = 0x1044;
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
    /// 0000:39D8..3A82, when a swing lands: every actor not peaceful
    /// (flag 0x4000) nor a corpse, within 0xBE (the larger distance plus
    /// a quarter of the smaller) and 0x30 of the player's heading, is
    /// struck, people (slots before 0C57) by `strike_person`. Nothing
    /// happens when the player is not above the floor (C3D2 <= 0).
    /// Returns false when the swing reached no actor: the original then
    /// hits the wall ahead (0000:3D43, `Machine::bash`). None where a strike
    /// faults or hangs.
    pub fn swing_hits(&mut self, kill: &mut dyn KillHooks) -> Option<bool> {
        self.mem.w16(DS, svar::HITS, 0);
        if self.mem.r16(DS, svar::HEIGHT) as i16 <= 0 {
            return Some(true);
        }
        let (px, py) = (self.mem.r16(DS, svar::PX), self.mem.r16(DS, svar::PY));
        let corpse = self.corpse();
        for i in 0..23u16 {
            let si = svar::ACTORS + i * 0x1C;
            let sprite = self.mem.r8(DS, si + f::SPRITE);
            if self.mem.r16(DS, si + f::FLAGS) & 0x4000 != 0 || sprite == 0xC5 || sprite == corpse {
                continue;
            }
            let (x, y) = (self.mem.r16(DS, si), self.mem.r16(DS, si + 4));
            let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
            let (dx, dy) = (abs(px.wrapping_sub(x)), abs(py.wrapping_sub(y)));
            let (lo, hi) = if dx > dy { (dy, dx) } else { (dx, dy) };
            if (lo >> 2).wrapping_add(hi) > 0xBE {
                continue;
            }
            let mut a = abs(travel_heading(px, py, x, y).wrapping_sub(self.mem.r16(DS, svar::HEADING)));
            if a >= 0x100 {
                a = 0x200u16.wrapping_sub(a);
            }
            if a > 0x30 {
                continue;
            }
            let n = self.mem.r16(DS, svar::HITS).wrapping_add(1);
            self.mem.w16(DS, svar::HITS, n);
            if si < svar::CREATURE_ACTORS {
                self.strike_person(si, 0)?;
            } else {
                self.strike_actor(si, kill)?;
            }
        }
        Some(self.mem.r16(DS, svar::HITS) != 0)
    }

    /// The corpse sprite: DGROUP C936 plus the level's first flat (C38E).
    fn corpse(&self) -> u8 {
        (self.mem.r8(DS, 0xC936) as u16).wrapping_add(self.mem.r16(DS, 0xC38E)) as u8
    }

    /// 0000:3AED. A person struck (by the player's swing, `arg` 0) dies at
    /// once: a corpse, blood, and with `arg` 0 one to four gold pieces
    /// ("On this body you find %d gold pieces."). Someone may have seen
    /// it (`crime_seen`), which with no creatures about calls the guards.
    pub fn strike_person(&mut self, si: u16, arg: u16) -> Option<()> {
        let corpse = self.corpse();
        self.mem.w8(DS, si + f::SPRITE, corpse);
        let fl = self.mem.r16(DS, si + f::FLAGS) & !0x20;
        self.mem.w16(DS, si + f::FLAGS, fl);
        self.mem.w8(DS, si + f::FRAME, 0xFF);
        self.blood(si, 0x18);
        if arg == 0 {
            let gold = self.rng.range(0, 4)?.wrapping_add(1);
            let total = self.mem.r32(DS, svar::GOLD).wrapping_add(gold as u32);
            self.mem.w32(DS, svar::GOLD, total);
            self.sprintf((DS, svar::GOLD_TEXT), (DS, svar::GOLD_FORMAT), &[Arg::Int(gold)]);
        }
        if self.crime_seen() && self.mem.r16(DS, 0x0FF9) == 0 {
            // 0000:3F21: the guards come (through the hook at 67EA).
            self.mem.w8(DS, 0x680F, 0x0F);
            self.mem.w16(DS, 0x67EA, 0x3F3B);
            self.mem.w16(DS, 0x67EC, 0);
        }
        // 0000:3B47: the blow's sound (9) at the person's square.
        let at = self.record_square(si);
        self.sound_at(9, at)?;
        if arg == 0 {
            self.mem.w16(DS, svar::MESSAGE, svar::GOLD_TEXT);
            self.mem.w16(DS, svar::MESSAGE + 2, DS);
            self.mem.w16(DS, svar::MESSAGE_TIME, 0x1E);
        }
        Some(())
    }

    /// 0000:3F50. Whether a crime was seen: a percent roll under 100 -
    /// 10 x the player's level, at least 50. The product is 8-bit times
    /// 8-bit but the subtraction is unsigned 16-bit, so from level 11 on
    /// the chance wraps to over 65000: always seen. Fixed (#9), the
    /// subtraction is signed, so the floor of 50 the MAX was there for
    /// holds: a crime is seen half the time from level 5 on.
    pub fn crime_seen(&mut self) -> bool {
        let level = self.mem.r8(DS, PLAYER + field::LEVEL) as u16;
        let chance = if self.fixed(crate::fixes::CRIME_SEEN) {
            (100 - level as i16 * 10).max(50) as u16
        } else {
            100u16.wrapping_sub(level * 10).max(50)
        };
        self.percent() < chance
    }

    /// 0000:3B6D. The player hits the actor in slot `si` (a creature
    /// slot). It works out whether the player is behind the actor
    /// (0000:3D17, from the heading to the actor's target pointer), but
    /// the `xor ax, ax` before its JAE clears that carry: AX is always 0.
    /// Fixed (#27), AX is 1 when the player is behind the actor, as the
    /// `inc ax` after the JAE was there to make it (DGROUP 1298; nothing
    /// in the game reads it, so no rear-attack effect follows).
    pub fn strike_actor(&mut self, si: u16, kill: &mut dyn KillHooks) -> Option<()> {
        let arg = if self.fixed(crate::fixes::REAR_ATTACK) { self.behind_actor(si) as u16 } else { 0 };
        self.mem.w16(DS, var::ATTACKER, PLAYER);
        let rec = self.mem.r16(DS, si + f::RECORD);
        self.mem.w16(DS, var::DEFENDER, rec);
        self.player_attack(arg, si, kill)
    }

    /// 0000:3D17. Whether the actor in slot `si` has the one it is after
    /// (the position at its +6 pointer) behind it: the heading there
    /// less the actor's (+A) is 80h..17Fh, or -17Fh..-80h, of 200h.
    fn behind_actor(&self, si: u16) -> bool {
        let di = self.mem.r16(DS, si + 6);
        let h = travel_heading(self.mem.r16(DS, si), self.mem.r16(DS, si + 4), self.mem.r16(DS, di), self.mem.r16(DS, di.wrapping_add(4)));
        let ax = h.wrapping_sub(self.mem.r16(DS, si + 0xA)) as i16;
        if ax >= 0 { (0x80..0x180).contains(&ax) } else { (-0x17F..=-0x80).contains(&ax) }
    }

    /// 2AA1:0000. The player (DGROUP 1400 = 01A4) hits the record at 1402
    /// in the creature segment, shown by actor slot `slot`. A weapon that
    /// can't hurt a special creature (see `combat::special_low`) only
    /// makes a sound. None where the original faults or hangs.
    pub fn player_attack(&mut self, arg: u16, slot: u16, kill: &mut dyn KillHooks) -> Option<()> {
        self.mem.w16(DS, svar::ARG, arg);
        self.mem.w16(DS, svar::SLOT, slot);
        let location = self.hit_location()?;
        let att = Ptr::new(DS, self.mem.r16(DS, var::ATTACKER));
        let def = Ptr::new(self.mem.r16(DS, var::CREATURE_SEG), self.mem.r16(DS, var::DEFENDER));
        let armor = self.armor_at(def.seg, def.off, location) as i16;
        let (hit, _) = self.roll_to_hit(att, def, location, &Gear { armor, ..Gear::default() })?;
        // 2AA1:00BE: a miss, or a weapon that can't hurt, sounds 4.
        if !hit {
            return self.sound_here(4);
        }
        let w = Ptr::new(DS, self.equipped_weapon(DS, PLAYER));
        let (cf, ax) = self.special_low(w);
        if !cf {
            if ax != 0xFFFF {
                return self.sound_here(4);
            }
            let (cf, ax) = self.special_high(w);
            if !cf && ax != 0xFFFF {
                return self.sound_here(4);
            }
        }
        // 2AA1:0042: a hit sounds 9.
        self.sound_here(9)?;
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
        self.kill_check(def.seg, def.off, kill)?;
        self.mem.w16(DS, svar::COOLDOWN, 0x14);
        Some(())
    }

    /// The player's side of 2AA1:04A5: weapon (1194:7510), else gauntlets
    /// (1194:7449 from SI = FFFF), else the racial bonus (1194:39ED).
    pub(crate) fn player_gear(&mut self) -> Gear {
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
        // CBW: the flat number's byte is sign-extended.
        let entry = svar::FLAT_DATA.wrapping_add((sprite as u8 as i8 as i16 as u16).wrapping_shl(2));
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
    pub fn kill_check(&mut self, seg: u16, rec: u16, kill: &mut dyn KillHooks) -> Option<bool> {
        self.end_timed_flag();
        if self.mem.r16(seg, rec.wrapping_add(field::HP)) as i16 > 0 {
            self.quest_yield(seg, rec, kill)?;
            return Some(false);
        }
        if self.quest_yield(seg, rec, kill)? {
            return Some(false);
        }
        let xp = self.record_dies(seg, rec)?;
        let flags = self.mem.r16(seg, rec.wrapping_add(field::FLAGS));
        if flags & 0x80 != 0 && self.mem.r8(seg, rec.wrapping_add(field::RACE)) != 0x17 {
            if self.mem.r16(seg, rec.wrapping_add(field::MAX_HP)) as i16 > 0 {
                self.quest_yield(seg, rec, kill)?;
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
    /// quest is marked, its effects end, its actor turns peaceful (flag
    /// 0x4000) and it says so (0819: `KillHooks::surrender`). CF.
    fn quest_yield(&mut self, seg: u16, rec: u16, kill: &mut dyn KillHooks) -> Option<bool> {
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
        // 2AA1:0819..083B: its words.
        kill.surrender(self)?;
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
    pub(crate) fn quest_matching(&self, stage: u16, field: u16, v: u8) -> Option<u16> {
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
    pub(crate) fn pick_quest_item(&mut self, base: u16) -> Option<(u16, u16)> {
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
                return crate::giveup::original("1194:3829: no quest item: the original rolls for ever");
            }
        };
        let free = (0..9).find(|&i| self.mem.r8(DS, svar::QUEST_ITEMS + i) == 0xFF).unwrap_or(9);
        Some((item, free))
    }
}

/// What a kill leaves to the front end.
pub trait KillHooks {
    /// 2AA1:0819..083B: a quest target giving up says so: its words made
    /// (`Machine::surrender_text`) and shown in a message box at 1044
    /// (1194:59C8). By default nothing (the words need the game's files).
    fn surrender(&mut self, _m: &mut Machine) -> Option<()> {
        Some(())
    }
}

/// `KillHooks` that leave the words unsaid.
pub struct NoKillHooks;

impl KillHooks for NoKillHooks {}

/// `KillHooks` that make a quest target's words (`Machine::surrender_text`)
/// and hand their box (1194:59C8, the text at DGROUP SURRENDER_LINE) to
/// `shown`: the kill paths that aren't the swing's (a spell's effect, the
/// clock's).
pub struct Words<'a> {
    pub files: &'a mut crate::bsa::Files,
    pub shown: &'a mut dyn FnMut(&mut Machine, u16),
}

impl KillHooks for Words<'_> {
    fn surrender(&mut self, m: &mut Machine) -> Option<()> {
        m.surrender_text(self.files, &mut crate::text::NoTextHooks)?;
        (self.shown)(m, svar::SURRENDER_LINE);
        Some(())
    }
}

impl Machine {
    /// 2AA1:0819..0833: a quest target's words on giving up: [102C]'s font
    /// selected (0000:9AE6), "%nc gives up his fight, ..." (82E6) copied
    /// to 1044 (9CA0) and its codes made (2DA2:000A). The box (59C8) is
    /// the caller's. None where the codes give up.
    pub fn surrender_text(&mut self, files: &mut crate::bsa::Files, hooks: &mut dyn crate::text::TextHooks) -> Option<()> {
        self.select_font(crate::shops::font::ARENA);
        let mut i = 0u16;
        loop {
            let c = self.mem.r8(DS, svar::SURRENDER.wrapping_add(i));
            self.mem.w8(DS, svar::SURRENDER_LINE.wrapping_add(i), c);
            if c == 0 {
                break;
            }
            i = i.wrapping_add(1);
        }
        self.expand(files, hooks, DS, svar::SURRENDER_LINE)?;
        Some(())
    }
}

/// The game's handlers for the effects engine: the kill handler (with
/// its quest target's words through the `KillHooks`), player death and
/// the blood flash of a reflected spell.
pub struct GameHooks<'a>(pub &'a mut dyn KillHooks);

impl FxHooks for GameHooks<'_> {
    /// 2AA1:06A0. Where the original would hang (no actor slot for a
    /// dying creature, see `kill_check`) this reports no kill.
    fn vitals_changed(&mut self, m: &mut Machine, seg: u16, rec: u16) -> bool {
        m.kill_check(seg, rec, &mut *self.0).unwrap_or(false)
    }

    fn player_died(&mut self, m: &mut Machine) {
        m.player_dies();
    }

    fn spell_reflected(&mut self, m: &mut Machine, actor: u16) {
        m.blood(actor, 0x19);
    }
}
