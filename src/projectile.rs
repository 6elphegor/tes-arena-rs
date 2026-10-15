//! Projectiles: spells in flight and arrows (segment 29CF), with the
//! sprite objects and lights they carry (0000:FDEA, E0CA, E0F5) and the
//! wall test they use (0000:F4EB).
//!
//! Twelve projectile slots of 1Ch bytes live at DGROUP 7DBC (count at
//! 7DB8): position (+0 x, +2 z, +4 y), the light (+6, a pointer into the
//! light list), heading x 2 (+A, FFFF when standing still), flat (+C),
//! frame (+D), the flat's flags (+E), flags (+F: 200h always, 800h a
//! creature's, 1000h explodes), frames to live (+11, 0 free), the cast
//! record (+12, FFFE for an arrow, which has no light and no spell) and
//! a launch delay (+1A).
//!
//! A cast record (8 of 16h at 7F86) holds the spell's far pointer (+0),
//! the caster's (+4), the caster's level + 1 (+8) and flags (+A: 1 in
//! use, 20h aimed). Applying a cast to its targets (29CF:0941) is
//! `crate::spell`'s `cast_apply`. Not ported: the
//! sounds (23BA:002A), and the flat a player's spell loads from file
//! (29CF:08DA with AX = 0).

use crate::actors::var::{ACTORS, CREATURES, OCCUPANT, SIZE};
use crate::character::PLAYER;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    pub const SLOTS: u16 = 0x7DBC;
    pub const SLOT_SIZE: u16 = 0x1C;
    pub const NSLOTS: u16 = 12;
    pub const COUNT: u16 = 0x7DB8;
    /// Flat number pairs per element (4 bytes each, from +1).
    pub const ELEMENT_FLATS: u16 = 0x7F21;
    /// The light list: 32 entries of (x, y, radius), x = 8000h free;
    /// the number in use.
    pub const LIGHTS: u16 = 0xBB0C;
    pub const LIGHTS_END: u16 = 0xBBCC;
    pub const LIGHT_COUNT: u16 = 0xBC8C;
    /// Objects by map column: list heads (a word per column) and the
    /// node pool (next, row, object; +4 = 0 free).
    pub const COLUMNS: u16 = 0xD330;
    pub const NODES: u16 = 0xD43C;
    /// Flats: kind per flat (bit 7: flags at frame byte 0Bh), far
    /// pointers (offset, EMS bank) per flat, the EMS frame.
    pub const FLAT_KINDS: u16 = 0xCA97;
    pub const FLAT_DATA: u16 = 0xD0B8;
    pub const FRAME: u16 = 0x9030;
    /// Per flat (5 bytes from the level's first), bit 0: solid.
    pub const FLAT_FLAGS: u16 = 0xCDA1;
    pub const FLAT_BASE: u16 = 0xC38E;
    pub const WALLS: u16 = 0xD806;
    pub const PSQUARE: u16 = 0xCB90;
    pub const HEIGHT: u16 = 0xC3D2;
    pub const SIN: u16 = 0xB03C;
    pub const COS: u16 = 0xB13C;
    /// Targets of a cast: count and far pointers.
    pub const TARGETS: u16 = 0x0D5A;
    /// 21 square offsets an area spell covers.
    pub const AREA: u16 = 0x8451;
}
use var::*;

/// Projectile slot fields.
mod f {
    pub const X: u16 = 0x00;
    pub const Z: u16 = 0x02;
    pub const Y: u16 = 0x04;
    pub const LIGHT: u16 = 0x06;
    pub const HEADING: u16 = 0x0A;
    pub const FLAT: u16 = 0x0C;
    pub const FRAME: u16 = 0x0D;
    pub const FLAT_FLAGS: u16 = 0x0E;
    pub const FLAGS: u16 = 0x0F;
    pub const LIFE: u16 = 0x11;
    pub const CAST: u16 = 0x12;
    pub const F16: u16 = 0x16;
    pub const DELAY: u16 = 0x1A;
}

/// The cast pointer of an arrow.
pub const ARROW: u16 = 0xFFFE;

impl Machine {
    /// 29CF:02E2. Launches the cast at `cast` from actor slot `actor`
    /// (another of the same layout for the player) into the first free
    /// projectile slot; `dx` is DX on entry, which element 5 leaves as
    /// the launch delay + 1. A creature's (actor in a creature slot)
    /// starts at height 3Ch, others at the player's. The z set from the
    /// actor is then overwritten by the sprite setup with just -z/2 of
    /// the actor. None where the original reads a flat from file (not a
    /// creature's) or finds no free slot (it searches on without end).
    pub fn launch(&mut self, cast: u16, actor: u16, dx: u16) -> Option<()> {
        self.launch_with(None, cast, actor, dx)
    }

    /// `launch` for the player's spells, whose casting hands' graphic
    /// loads from the game files (see `projectile_flat`).
    pub fn launch_player(&mut self, files: &mut crate::bsa::Files, cast: u16, actor: u16, dx: u16) -> Option<()> {
        self.launch_with(Some(files), cast, actor, dx)
    }

    fn launch_with(&mut self, files: Option<&mut crate::bsa::Files>, cast: u16, actor: u16, dx: u16) -> Option<()> {
        let si = self.free_projectile()?;
        let w = |m: &mut Self, o: u16, v: u16| m.mem.w16(DS, si.wrapping_add(o), v);
        w(self, f::FLAGS, 0x200);
        let h = self.mem.r16(DS, HEIGHT);
        w(self, f::Z, h);
        let creature = (CREATURES..0xD37).contains(&actor);
        if creature {
            let fl = self.mem.r16(DS, si + f::FLAGS) | 0x800;
            w(self, f::FLAGS, fl);
            w(self, f::Z, 0x3C);
        }
        let (spell_off, spell_seg) = (self.mem.r16(DS, cast), self.mem.r16(DS, cast.wrapping_add(2)));
        let (al, dx) = self.projectile_flat(files, spell_seg, spell_off, creature, dx)?;
        self.mem.w8(DS, si + f::FLAT, al.wrapping_add(0xC));
        w(self, f::CAST, cast);
        w(self, f::F16, 0);
        w(self, f::DELAY, dx.wrapping_sub(1));
        if self.mem.r16(DS, cast.wrapping_add(0xA)) & 0x20 != 0 {
            let fl = self.mem.r16(DS, si + f::FLAGS) | 0x1000;
            w(self, f::FLAGS, fl);
        }
        let heading = self.mem.r16(DS, actor.wrapping_add(0xA)).wrapping_mul(2);
        w(self, f::HEADING, heading);
        let (x, y) = (self.mem.r16(DS, actor), self.mem.r16(DS, actor.wrapping_add(4)));
        let bz = ((self.mem.r16(DS, actor.wrapping_add(2)).wrapping_neg() as i16) >> 1) as u16;
        w(self, f::X, x);
        let z = self.mem.r16(DS, si + f::Z).wrapping_add(bz);
        w(self, f::Z, z);
        w(self, f::Y, y);
        self.mem.w8(DS, si + f::FRAME, 0);
        let flat = self.mem.r8(DS, si + f::FLAT);
        self.mem.w8(DS, si + f::LIFE, 0x1E);
        self.sprite_object(si, x, bz, y, flat, 0x1F4)?;
        // 29CF:036F (02D2): the launch's sound (7) at its square.
        let at = self.projectile_square(si);
        self.sound_at(7, at)?;
        let n = self.mem.r16(DS, COUNT).wrapping_add(1);
        self.mem.w16(DS, COUNT, n);
        Some(())
    }

    /// 29CF:037F. An arrow (flat `flat`) is shot from `from`, an actor
    /// slot, or with 0 the player, aimed by the cursor: the heading turns
    /// by half the cursor's distance from the view's centre (C3D6). It
    /// starts at the shooter's position and height (negated), lives 30
    /// frames, and the original's step of C8h ahead is lost when the
    /// sprite is set up at the shooter's position. None where the slot
    /// search runs away.
    pub fn fire_missile(&mut self, flat: u8, from: u16) -> Option<()> {
        let si = self.free_projectile()?;
        let w = |m: &mut Self, o: u16, v: u16| m.mem.w16(DS, si.wrapping_add(o), v);
        self.mem.w8(DS, si + f::FLAT, flat);
        w(self, f::CAST, ARROW);
        w(self, f::DELAY, 0);
        let fl = self.mem.r16(DS, si + f::FLAGS) & 0xE7FF;
        w(self, f::FLAGS, fl);
        w(self, f::F16, 0x400);
        let r = |m: &Self, a: u16| m.mem.r16(DS, a);
        let (x, z, y) = if from == 0 {
            let d = (r(self, 0x8F92).wrapping_sub(r(self, 0xC3D6)) as i16 >> 1) as u16;
            let h = d.wrapping_add(r(self, 0xC3DA)) & 0x1FF;
            w(self, f::HEADING, h << 1);
            (r(self, 0xC3D0), r(self, HEIGHT), r(self, 0xC3D4))
        } else {
            let h = r(self, from.wrapping_add(0xA)).wrapping_add(r(self, from.wrapping_add(0xA)));
            w(self, f::HEADING, h);
            (r(self, from), r(self, from.wrapping_add(2)), r(self, from.wrapping_add(4)))
        };
        let z = z.wrapping_neg();
        w(self, f::X, x);
        w(self, f::Z, z);
        w(self, f::Y, y);
        self.mem.w8(DS, si + f::FRAME, 0);
        self.mem.w8(DS, si + f::LIFE, 0x1E);
        let h = r(self, si + f::HEADING);
        let step = |m: &Self, t: u16| (((0xC8i32 * m.mem.r16(DS, t.wrapping_add(h)) as i16 as i32) >> 16) as u16);
        let nx = r(self, si).wrapping_add(step(self, SIN));
        w(self, f::X, nx);
        let ny = r(self, si + f::Y).wrapping_add(step(self, COS));
        w(self, f::Y, ny);
        self.sprite_object(si, x, z, y, flat, 0)?;
        // 29CF:0415: the shot's sound at its square: 7, or 2 for an
        // arrow (the cast FFFEh).
        let at = self.projectile_square(si);
        let n = if r(self, si + f::CAST) == 0xFFFE { 2 } else { 7 };
        self.sound_at(n, at)?;
        let n = r(self, COUNT).wrapping_add(1);
        self.mem.w16(DS, COUNT, n);
        Some(())
    }

    /// 29CF:02E4..02F0: the first slot with +11 = 0, searching past the
    /// table if need be.
    fn free_projectile(&self) -> Option<u16> {
        let mut si = SLOTS;
        for _ in 0..0x10000 / SLOT_SIZE as u32 + 1 {
            if self.mem.r8(DS, si + f::LIFE) == 0 {
                return Some(si);
            }
            si = si.wrapping_add(SLOT_SIZE);
        }
        None
    }

    /// 29CF:08DA. The flat of a spell's projectile, from its element
    /// (+26): element 5 gives 5 and leaves DX as it was; others pick one
    /// of two from ELEMENT_FLATS by a random bit, with DX = 1. For a
    /// player's spell (not `creature`) the casting hands' graphic of the
    /// element (fire.cfa, ice.cfa, ... at 7D49) loads first into the EMS
    /// frame ([902E], bank C8F4), their animation starts (7F80 = 0, 8038 =
    /// the file's size / 4 rounded up) and DX is its frame count. None for
    /// a player's spell without the game files.
    fn projectile_flat(
        &mut self,
        files: Option<&mut crate::bsa::Files>,
        seg: u16,
        spell: u16,
        creature: bool,
        dx: u16,
    ) -> Option<(u8, u16)> {
        let element = self.mem.r8(seg, spell.wrapping_add(0x26)) as u16;
        if element == 5 {
            return Some((5, dx));
        }
        let mut bp = 1;
        if !creature {
            let files = files?;
            let name = self.skip_strings(DS, 0x7D49, element);
            let bank = self.mem.r8(DS, 0xC8F4) as u16;
            self.ems_map(bank);
            let (off, fseg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
            // A missing file leaves the frame and the size as they were.
            let _ = self.file_load(files, name, fseg, off);
            bp = self.mem.r8(fseg, off.wrapping_add(0xB)) as u16;
            let size = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE);
            self.mem.w16(DS, 0x8038, size.wrapping_add(3) >> 2);
            self.mem.w8(DS, 0x7F80, 0);
        }
        let r = self.rng.next() & 1;
        let at = ELEMENT_FLATS.wrapping_add(element.wrapping_shl(2)).wrapping_add(r * 2 + 1);
        Some((self.mem.r8(DS, at), bp))
    }

    /// 0000:FDEA. Sets up the sprite object at `si`: position, flat, a
    /// light of `radius` (none for 0, `light_add`), the flat's flags (from
    /// its kind and the EMS frame: byte 0, or byte 0Bh | 80h for kinds
    /// with bit 7, minus 1; FF - 1 for kind 0), and files it under its
    /// map column (`column_add`). None where that search runs away.
    fn sprite_object(&mut self, si: u16, x: u16, z: u16, y: u16, flat: u8, radius: u16) -> Option<()> {
        self.mem.w16(DS, si + f::X, x);
        self.mem.w16(DS, si + f::Z, z);
        self.mem.w16(DS, si + f::Y, y);
        self.mem.w8(DS, si + f::FLAT, flat);
        self.mem.w16(DS, si + f::LIGHT, 0);
        if radius != 0 {
            let l = self.light_add(x, y, radius);
            self.mem.w16(DS, si + f::LIGHT, l);
        } else {
            self.mem.w16(DS, si + f::LIGHT, 0);
        }
        let di = self.mem.r8(DS, si + f::FLAT) as u16;
        let kind = self.mem.r8(DS, FLAT_KINDS.wrapping_add(di));
        let entry = FLAT_DATA.wrapping_add(di << 2);
        self.ems_map(self.mem.r16(DS, entry.wrapping_add(2)));
        let mut al = 0xFFu8;
        if kind != 0 {
            let frame = self.mem.r16(DS, FRAME);
            let off = self.mem.r16(DS, entry);
            al = self.mem.r8(frame, off);
            if kind & 0x80 != 0 {
                al = self.mem.r8(frame, off.wrapping_add(0xB)) | 0x80;
            }
        }
        self.mem.w8(DS, si + f::FLAT_FLAGS, al.wrapping_sub(1));
        let (x, y) = (self.mem.r16(DS, si + f::X), self.mem.r16(DS, si + f::Y));
        self.column_add(x >> 7, y >> 7, si)
    }

    /// 0000:E0CA. Takes a free light (x = 8000h) for (x, y, radius) and
    /// returns it. With the count at 20h it returns 20h; with no free
    /// entry, the end of the list. Neither takes a light.
    fn light_add(&mut self, x: u16, y: u16, radius: u16) -> u16 {
        let n = self.mem.r16(DS, LIGHT_COUNT);
        if n == 0x20 {
            return n;
        }
        let mut si = LIGHTS;
        loop {
            if self.mem.r16(DS, si) == 0x8000 {
                break;
            }
            si = si.wrapping_add(6);
            if si as i16 >= LIGHTS_END as i16 {
                return si;
            }
        }
        self.mem.w16(DS, si, x);
        self.mem.w16(DS, si.wrapping_add(2), y);
        self.mem.w16(DS, si.wrapping_add(4), radius);
        self.mem.w16(DS, LIGHT_COUNT, n.wrapping_add(1));
        si
    }

    /// 0000:E0F5. Frees the light at `si`; the count stops at 0.
    fn light_free(&mut self, si: u16) {
        let n = self.mem.r16(DS, LIGHT_COUNT).wrapping_sub(1);
        self.mem.w16(DS, LIGHT_COUNT, if (n as i16) < 0 { 0 } else { n });
        self.mem.w16(DS, si, 0x8000);
    }

    /// 0000:BFC0. Appends object `obj` at (`col`, `row`) to its column's
    /// list (a head whose object is 0 starts the list over). Nodes come
    /// from the pool (0000:C005), searched on without end: None where
    /// that never finds a free node.
    fn column_add(&mut self, col: u16, row: u16, obj: u16) -> Option<()> {
        let head_at = COLUMNS.wrapping_add(col.wrapping_mul(2));
        let mut head = self.mem.r16(DS, head_at);
        if head != 0 && self.list_word(head.wrapping_add(4))? == 0 {
            self.mem.w16(DS, head_at, 0);
            head = 0;
        }
        let node = if head == 0 {
            let n = self.node_alloc()?;
            self.mem.w16(DS, head_at, n);
            n
        } else {
            let mut p = head;
            for _ in 0..0x10000 {
                let next = self.list_word(p)?;
                if next == 0 {
                    break;
                }
                p = next;
            }
            if self.list_word(p)? != 0 {
                return None; // a cycle: the original walks it for ever
            }
            let n = self.node_alloc()?;
            self.mem.w16(DS, p, n);
            n
        };
        self.list_word(node)?;
        self.mem.w16(DS, node, 0);
        self.list_word(node.wrapping_add(2))?;
        self.mem.w16(DS, node.wrapping_add(2), row);
        self.list_word(node.wrapping_add(4))?;
        self.mem.w16(DS, node.wrapping_add(4), obj);
        Some(())
    }

    /// The word at DGROUP `o` in the object lists. None at offset FFFF,
    /// where a word access raises #GP on a 386 in real mode (the lists
    /// get there only when the node pool has run out).
    fn list_word(&self, o: u16) -> Option<u16> {
        (o != 0xFFFF).then(|| self.mem.r16(DS, o))
    }

    /// 0000:C019. The objects' lists emptied: the 80h column heads 0 and
    /// the 80h nodes free (+4 = 0). The world loop does it after each frame
    /// (0000:A44A, after the frame count), a load in its rebuild (C015).
    pub fn object_lists_clear(&mut self) {
        for i in 0..0x80u16 {
            self.mem.w16(DS, COLUMNS + 2 * i, 0);
            self.mem.w16(DS, NODES + 6 * i + 4, 0);
        }
    }

    /// 0000:C005. The first node from NODES with +4 = 0.
    fn node_alloc(&self) -> Option<u16> {
        let mut si = NODES;
        for _ in 0..0x10000 / 6 + 1 {
            if self.list_word(si.wrapping_add(4))? == 0 {
                return Some(si);
            }
            si = si.wrapping_add(6);
        }
        None
    }

    /// 29CF:0441. A frame of every projectile: a launch delay counts
    /// down; then its life does (freeing the light at the end); moving
    /// ones take two half steps of 32h along the heading, each tested
    /// for a hit (`projectile_hits`). Without a hit the light follows
    /// and the sprite is set up again (without a new light). On a hit
    /// the projectile ends: an exploding one stays as an explosion on
    /// the square's centre and its spell takes effect there
    /// (`impact`); an arrow that hit an actor strikes it (29CF:0568);
    /// any other just loses its light. None where the original faults
    /// or hangs.
    pub fn projectiles_update(&mut self) -> Option<()> {
        for i in 0..NSLOTS {
            let si = SLOTS + i * SLOT_SIZE;
            self.projectile_frame(si)?;
        }
        Some(())
    }

    fn projectile_frame(&mut self, si: u16) -> Option<()> {
        if self.mem.r8(DS, si + f::LIFE) == 0 {
            return Some(());
        }
        let delay = self.mem.r16(DS, si + f::DELAY);
        if delay != 0 {
            self.mem.w16(DS, si + f::DELAY, delay.wrapping_sub(1));
            return Some(());
        }
        let life = self.mem.r8(DS, si + f::LIFE).wrapping_sub(1);
        self.mem.w8(DS, si + f::LIFE, life);
        if life == 0 {
            if self.mem.r16(DS, si + f::CAST) != ARROW {
                self.light_free(self.mem.r16(DS, si + f::LIGHT));
            }
            self.count_add(0xFFFF);
            return Some(());
        }
        if self.mem.r16(DS, si + f::HEADING) as i16 >= 0 {
            for _ in 0..2 {
                let h = self.mem.r16(DS, si + f::HEADING);
                let step = |m: &Self, table: u16| (((m.mem.r16(DS, table.wrapping_add(h)) as i16 as i32) * 0x32) >> 16) as u16;
                let dx = step(self, SIN);
                let x = self.mem.r16(DS, si + f::X).wrapping_add(dx);
                self.mem.w16(DS, si + f::X, x);
                let dy = step(self, COS);
                let y = self.mem.r16(DS, si + f::Y).wrapping_add(dy);
                self.mem.w16(DS, si + f::Y, y);
                if let Some(ax) = self.projectile_hits(si) {
                    return self.projectile_hit(si, ax);
                }
            }
        }
        let (x, z, y) = (self.mem.r16(DS, si + f::X), self.mem.r16(DS, si + f::Z), self.mem.r16(DS, si + f::Y));
        let flat = self.mem.r8(DS, si + f::FLAT);
        let light = self.mem.r16(DS, si + f::LIGHT);
        if self.mem.r16(DS, si + f::CAST) != ARROW {
            self.mem.w16(DS, light, x);
            self.mem.w16(DS, light.wrapping_add(2), y);
        }
        self.sprite_object(si, x, z, y, flat, 0)?;
        self.mem.w16(DS, si + f::LIGHT, light);
        Some(())
    }

    fn count_add(&mut self, d: u16) {
        let n = self.mem.r16(DS, COUNT).wrapping_add(d);
        self.mem.w16(DS, COUNT, n);
    }

    /// 29CF:042E. The square of the projectile at `si`, in CB90's form:
    /// its row high, its column doubled low (8 bits).
    fn projectile_square(&self, si: u16) -> u16 {
        let x = self.mem.r16(DS, si + f::X);
        let y = self.mem.r16(DS, si + f::Y);
        ((y >> 7) << 8) | ((x >> 7) as u8).wrapping_add((x >> 7) as u8) as u16
    }

    /// 29CF:04E1. The projectile at `si` hit something (`ax` as
    /// `projectile_hits` left it).
    fn projectile_hit(&mut self, si: u16, ax: u16) -> Option<()> {
        self.count_add(0xFFFF);
        self.mem.w8(DS, si + f::LIFE, 0);
        if self.mem.r16(DS, si + f::FLAGS) & 0x1000 == 0 {
            if self.mem.r16(DS, si + f::CAST) != ARROW {
                self.light_free(self.mem.r16(DS, si + f::LIGHT));
            } else if ax != 0 {
                // 29CF:0568: the arrow strikes the actor found.
                let a = ACTORS.wrapping_add((self.mem.r8(DS, OCCUPANT) as u16) * SIZE);
                if a >= CREATURES {
                    self.strike_actor(a)?;
                } else {
                    self.strike_person(a, 1)?;
                }
            }
            return Some(());
        }
        // 29CF:050B: an explosion where it hit, its sound (8) first.
        let at = self.projectile_square(si);
        self.sound_at(8, at)?;
        let x = (self.mem.r16(DS, si + f::X) & 0xFF80).wrapping_add(0x40);
        let y = (self.mem.r16(DS, si + f::Y) & 0xFF80).wrapping_add(0x40);
        self.mem.w16(DS, si + f::X, x);
        self.mem.w16(DS, si + f::Y, y);
        let z = self.mem.r16(DS, si + f::Z).wrapping_sub(0xA);
        self.mem.w16(DS, si + f::Z, z);
        self.count_add(1);
        let flat = self.mem.r8(DS, si + f::FLAT).wrapping_sub(0xC);
        self.mem.w8(DS, si + f::FLAT, flat);
        let life = (self.mem.r8(DS, FLAT_KINDS.wrapping_add(flat as u16)) & 0x7F).wrapping_sub(1);
        self.mem.w8(DS, si + f::LIFE, life);
        self.mem.w8(DS, si + f::FRAME, 0);
        self.mem.w8(DS, si + f::FLAT_FLAGS, life);
        self.mem.w16(DS, si + f::HEADING, 0xFFFF);
        self.impact(self.mem.r16(DS, si + f::CAST), x, y)
    }

    /// 29CF:0589. Whether the projectile at `si` hit something, and AX
    /// then: a player's (no flag 800h) hits an actor in a creature slot
    /// on its square (AX 1; people are passed through), then walls
    /// (`wall_hit`, AX 0); a creature's hits walls (AX 0) or the player's
    /// square (AX 0). A creature's always leaves OCCUPANT at 01A4 when it
    /// misses the walls, which then names the player as the target.
    fn projectile_hits(&mut self, si: u16) -> Option<u16> {
        let (x, z, y) = (self.mem.r16(DS, si + f::X), self.mem.r16(DS, si + f::Z), self.mem.r16(DS, si + f::Y));
        let sq = (y >> 7) << 8 | ((x >> 7) as u8).wrapping_mul(2) as u16;
        let creature = self.mem.r16(DS, si + f::FLAGS) & 0x800 != 0;
        if !creature && self.square_occupied(sq) && self.mem.r16(DS, OCCUPANT) >= 0xF {
            return Some(1);
        }
        let (hit, bx) = self.wall_hit(x, y, z);
        if hit {
            return Some(0);
        }
        if !creature {
            return None;
        }
        self.mem.w16(DS, OCCUPANT, PLAYER);
        (bx == self.mem.r16(DS, PSQUARE)).then_some(0)
    }

    /// 0000:F4EB. Whether a projectile at (x, y) and height z hits the
    /// wall plane, and the BX it leaves (the square, mostly). Empty
    /// squares never; raised walls (positive words) unless -z is outside
    /// their bottom..top, or always when both bytes match; for the
    /// others by type: 9 unless bit 100h, 0Dh, 0Ah and 0Bh doors unless
    /// opening (bits E00h), 8 by its flat's solid bit, the rest always.
    /// An open door returns its registers swapped, BX = (x >> 7) * 2.
    fn wall_hit(&self, x: u16, y: u16, z: u16) -> (bool, u16) {
        let col2 = (x >> 7).wrapping_mul(2);
        let sq = ((y >> 7) << 8).wrapping_add(col2);
        let walls = self.mem.r16(DS, WALLS);
        let w = self.mem.r16(walls, sq);
        if w == 0 {
            return (false, sq);
        }
        if (w as i16) > 0 {
            if w as u8 == (w >> 8) as u8 {
                return (true, sq);
            }
            let (top, bottom) = self.move_wall_height(w);
            let bx = (((w >> 8) >> 3) & 0xF) * 2 + if self.mem.r16(DS, 0xC39A) & 0x200 != 0 { 0x20 } else { 0 };
            let d = z.wrapping_neg();
            return (!(d < bottom || d > top), bx);
        }
        match (w >> 12) as u8 {
            9 => (w & 0x100 == 0, sq),
            0xD => (true, sq),
            0xA | 0xB => {
                if w & 0xE00 == 0 {
                    (true, sq)
                } else {
                    (false, col2)
                }
            }
            8 => {
                let b = (w & 0xFF).wrapping_add(self.mem.r16(DS, FLAT_BASE));
                let solid = self.mem.r8(DS, FLAT_FLAGS.wrapping_add(b.wrapping_mul(5))) & 1 != 0;
                (solid, sq)
            }
            _ => (true, sq),
        }
    }

    /// 29CF:09B8. A spell takes effect at (x, y) for the cast at `cast`:
    /// an area spell (target types 3 and 4) on everyone in the area
    /// (2CDD:08D9), another on OCCUPANT: the player (01A4) or a creature
    /// slot's record, while nobody (FFFF) or a person (slots below 0Fh)
    /// takes nothing and the cast is never applied (nor freed).
    pub fn impact(&mut self, cast: u16, x: u16, y: u16) -> Option<()> {
        let (spell, seg) = (self.mem.r16(DS, cast), self.mem.r16(DS, cast.wrapping_add(2)));
        let ty = self.mem.r8(seg, spell.wrapping_add(0x24));
        if ty == 3 || ty == 4 {
            self.mem.w16(DS, TARGETS, 0);
            self.area_targets(x, y);
        } else {
            let o = self.mem.r16(DS, OCCUPANT);
            if o == 0xFFFF {
                return Some(());
            }
            if o == PLAYER {
                self.mem.w16(DS, TARGETS + 2, PLAYER);
                self.mem.w16(DS, TARGETS + 4, DS);
            } else if o < 0xF {
                return Some(());
            } else {
                let a = ACTORS.wrapping_add(o.wrapping_mul(SIZE));
                let rec = self.mem.r16(DS, a.wrapping_add(0x12));
                self.mem.w16(DS, TARGETS + 2, rec);
                self.mem.w16(DS, TARGETS + 4, self.mem.r16(DS, 0x1042));
            }
            self.mem.w16(DS, TARGETS, 1);
        }
        self.cast_apply(cast)
    }

    /// 2CDD:08D9. The targets of an area spell centred on (x, y): the
    /// player if its square is one of the 21 offsets from the centre
    /// (the list then holds just the player), then each creature slot
    /// not peaceful nor of sprite C5h on one of them.
    fn area_targets(&mut self, x: u16, y: u16) {
        let centre = (y >> 7) << 8 | ((x >> 7) as u8).wrapping_mul(2) as u16;
        let within = |m: &Self, sq: u16| (0..21).any(|i| sq == centre.wrapping_add(m.mem.r16(DS, AREA + 2 * i)));
        if within(self, self.mem.r16(DS, PSQUARE)) {
            self.mem.w16(DS, TARGETS, 1);
            self.mem.w16(DS, TARGETS + 2, PLAYER);
            self.mem.w16(DS, TARGETS + 4, DS);
        }
        for i in 0..8 {
            let si = CREATURES + i * SIZE;
            if self.mem.r16(DS, si + 0xF) & 0x4000 != 0 || self.mem.r8(DS, si + 0xC) == 0xC5 {
                continue;
            }
            let (ax, dx) = (self.mem.r16(DS, si), self.mem.r16(DS, si + 4));
            let sq = (dx >> 7) << 8 | ((ax >> 7) as u8).wrapping_mul(2) as u16;
            if within(self, sq) {
                let n = self.mem.r16(DS, TARGETS);
                let at = TARGETS + 2 + n.wrapping_shl(2);
                self.mem.w16(DS, at, self.mem.r16(DS, si + 0x12));
                self.mem.w16(DS, at.wrapping_add(2), self.mem.r16(DS, 0x1042));
                self.mem.w16(DS, TARGETS, n.wrapping_add(1));
            }
        }
    }
}
