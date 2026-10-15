//! Clicking in the view (2826:077E): the left click in the centre region.
//!
//! A spell waiting for a target takes the click first: Passwall-like
//! wall and floor spells (counters DC0, DC2: negative removes, positive
//! creates), Open (DC4) and Lock (DC8) on a door, or a spell cast at a
//! target (0620, `ControlHooks::cast_at_target`). Otherwise the wall
//! under the cursor may run its trigger (`wall_trigger`), and a door
//! within two squares opens: a lock opens with one of the player's keys
//! (12 key bytes at 0652) or, when picking is armed (5CA bit 0), by a
//! lockpicking roll. A click elsewhere is left for the actors' click
//! (C39A bit 80h and the position at CB60 for 1194:DB6C).
//!
//! What is under the cursor comes from the renderer: the wall square
//! (CB92, FFFFh none), the door's screen span and square (CCB2..CCB8,
//! CCC8), and the floor square (`ControlHooks::floor_pick`, the
//! original's 2826:0B52 projecting the cursor onto the floor).

use crate::bsa::Files;
use crate::controls::{var::*, ControlHooks};
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::movement::var::{FLOOR_SEG, FLOOR_SOLID, FRAME_SEG, HEIGHT, LEVEL_FLAGS, LOCKS, LOCK_COUNT, MAP_FLAGS, X, Y};

/// DGROUP variables.
pub mod var {
    /// Spells waiting for a target: walls (DC0) and floors (DC2) to
    /// remove (negative) or create (positive), Open (DC4, its power;
    /// 7D0h always opens weak locks) with the caster's level at DC6,
    /// Lock (DC8, the lock's level), a spell cast at a target (0620,
    /// pointer to a far pointer to the spell), and its cost (F84).
    pub const WALL_SPELL: u16 = 0x0DC0;
    pub const FLOOR_SPELL: u16 = 0x0DC2;
    pub const OPEN_SPELL: u16 = 0x0DC4;
    pub const OPEN_LEVEL: u16 = 0x0DC6;
    pub const LOCK_SPELL: u16 = 0x0DC8;
    pub const TARGET_SPELL: u16 = 0x0620;
    /// Squares changed for a while (32 words) and their kinds (+40h):
    /// walls removed (0) or created (0D01h).
    pub const TEMP_WALLS: u16 = 0xC2C2;
    /// Set once a changed square has been drawn (and stepped) since the
    /// view's drawing began.
    pub const STEPPED: u16 = 0xD73C;
    /// The actor whose square was found by `actor_on` (FFFFh none).
    pub const ACTOR_FOUND: u16 = 0x7A44;
    /// The door drawn under the cursor: x span (CCB2..CCB4), square
    /// (column CCB6, row CCB8) and map offset (CCC8).
    pub const DOOR_X0: u16 = 0xCCB2;
    pub const DOOR_X1: u16 = 0xCCB4;
    pub const DOOR_COL: u16 = 0xCCB6;
    pub const DOOR_ROW: u16 = 0xCCB8;
    pub const DOOR_AT: u16 = 0xCCC8;
    /// The click left for the actors (1194:DB6C).
    pub const ACTOR_CLICK: u16 = 0xCB60;
    /// The player's keys (12 bytes; C5h once used).
    pub const KEYS_HELD: u16 = 0x0652;
    /// Doors the player failed to pick (32 words, FFFFh free).
    pub const PICK_FAILED: u16 = 0x0F88;
    /// The lock being tried.
    pub const LOCK_TRIED: u16 = 0x13CC;
    /// The message: time left, far pointer.
    pub const MESSAGE_TIME: u16 = 0x061E;
    pub const MESSAGE: u16 = 0x061A;
}
use var::*;

/// Messages: "Failure...", "Success...".
const FAILURE: u16 = 0x7BFA;
const SUCCESS: u16 = 0x7C05;

impl Machine {
    fn walls(&self) -> u16 {
        self.mem.r16(DS, crate::mif::var::MAP1_SEG)
    }

    /// 292B:0456 / 0469. "Failure..." or "Success..." for 15 frames.
    pub(crate) fn outcome(&mut self, ok: bool) {
        self.mem.w16(DS, MESSAGE_TIME, 0xF);
        self.mem.w16(DS, MESSAGE, if ok { SUCCESS } else { FAILURE });
        self.mem.w16(DS, MESSAGE + 2, DS);
    }

    /// 0000:D456. Marks the square at map offset `si` as changed for a
    /// while (kind `kind`) in the first free slot, if any.
    fn temp_wall(&mut self, si: u16, kind: u16) {
        let Some(i) = (0..0x20).find(|&i| self.mem.r16(DS, TEMP_WALLS + 2 * i) == 0xFFFF) else {
            return;
        };
        self.mem.w16(DS, TEMP_WALLS + 2 * i, si);
        self.mem.w16(DS, TEMP_WALLS + 0x40 + 2 * i, kind);
        let walls = self.walls();
        let w = self.mem.r16(walls, si) | 0x4040;
        self.mem.w16(walls, si, w);
    }

    /// 2826:0A87. Whether an actor stands on map offset `sq` (the actors'
    /// squares at 1951); ACTOR_FOUND gets its slot.
    pub(crate) fn actor_on(&mut self, sq: u16) -> bool {
        self.mem.w16(DS, ACTOR_FOUND, 0xFFFF);
        let squares = crate::actors::var::SQUARES;
        match (0..0x17).find(|&i| self.mem.r16(DS, squares + 2 * i) == sq) {
            Some(i) => {
                self.mem.w16(DS, ACTOR_FOUND, i);
                true
            }
            None => false,
        }
    }

    /// 2826:0B97. The lock record on map offset `sq`, if any.
    fn lock_on(&self, sq: u16) -> Option<u16> {
        let key = (sq & 0xFF00) | ((sq as u8) >> 1) as u16;
        let n = self.mem.r8(DS, LOCK_COUNT) as u16;
        (0..n).map(|i| LOCKS + 3 * i).find(|&r| self.mem.r16(DS, r) == key)
    }

    fn is_door(&self, sq: u16) -> bool {
        self.mem.r8(self.walls(), sq.wrapping_add(1)) >> 4 == 0xB
    }

    /// 2826:077E. A left click in the centre of the view.
    pub fn view_click(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        let walls = self.walls();
        let floors = self.mem.r16(DS, FLOOR_SEG);
        let spell = self.mem.r16(DS, WALL_SPELL);
        if spell != 0 {
            if (spell as i16) < 0 {
                // 2826:0790: removing a wall (a plain one, not stairs or
                // a trigger).
                let si = self.mem.r16(DS, PICKED);
                if si == 0xFFFF {
                    return Some(());
                }
                let w = self.mem.r16(walls, si);
                let al = w as u8 & 0x3F;
                let special = |m: &Machine| {
                    al == m.mem.r8(DS, crate::movement::var::STAIRS_UP)
                        || al == m.mem.r8(DS, crate::movement::var::STAIRS_DOWN)
                        || (0..0x10).any(|i| m.mem.r8(DS, crate::movement::var::TRIGGERS + i) == al)
                };
                if (w >> 8) as u8 == w as u8 && w != 0 && !special(self) {
                    self.temp_wall(si, 0);
                    self.mem.w16(DS, WALL_SPELL, spell.wrapping_add(1));
                    return Some(());
                }
            } else if let Some(si) = hooks.floor_pick(self) {
                // 2826:07DB: a wall raised from the floor's kind.
                if self.mem.r16(walls, si) == 0 && !self.actor_on(si) {
                    let kind = self.mem.r16(floors, si).wrapping_sub(self.mem.r16(DS, FRAME_SEG));
                    let al = ((kind >> 8) as u8).wrapping_add(1) as u16;
                    self.mem.w16(walls, si, al << 8 | al);
                    self.temp_wall(si, 0x0D01);
                    self.mem.w16(DS, WALL_SPELL, spell.wrapping_sub(1));
                    return Some(());
                }
            }
            self.outcome(false);
            return self.plain_click(files, hooks);
        }
        let spell = self.mem.r16(DS, FLOOR_SPELL);
        if (self.mem.r16(DS, HEIGHT) as i16) > 0 && spell != 0 {
            let solid = self.mem.r16(DS, FLOOR_SOLID);
            if let Some(si) = hooks.floor_pick(self) {
                if !self.actor_on(si) {
                    let fl = self.mem.r16(floors, si);
                    if (spell as i16) < 0 {
                        // 2826:082C: a solid floor becomes a pit.
                        if self.mem.r16(walls, si) == 0 && fl < solid {
                            let v = solid.wrapping_add((self.mem.r8(DS, 0xCB5B) as u16) << 8);
                            self.mem.w16(floors, si, v);
                            self.mem.w16(DS, FLOOR_SPELL, spell.wrapping_add(1));
                            return Some(());
                        }
                    } else if fl >= solid {
                        // 2826:086A: a pit is filled.
                        let v = self.mem.r16(DS, FRAME_SEG);
                        self.mem.w16(floors, si, v);
                        self.mem.w16(DS, FLOOR_SPELL, spell.wrapping_sub(1));
                        return Some(());
                    }
                }
            }
            self.outcome(false);
            return self.plain_click(files, hooks);
        }
        if self.mem.r16(DS, OPEN_SPELL) != 0 {
            // 2826:089D: Open on a locked door.
            let sq = self.mem.r16(DS, PICKED);
            if sq == 0xFFFF {
                self.leave_click();
                return Some(());
            }
            if self.is_door(sq) {
                let Some(rec) = self.lock_on(sq) else {
                    return self.plain_click(files, hooks);
                };
                let level = self.mem.r8(DS, rec + 2);
                let power = self.mem.r16(DS, OPEN_SPELL);
                let opens = if level <= 0x19 && power == 0x7D0 {
                    true
                } else {
                    let al = self.mem.r16(DS, OPEN_LEVEL) as u8;
                    let chance = ((al.wrapping_sub(level) as i8 as i16) * 5).wrapping_add(power as i16);
                    let roll = self.rng.next() % 100;
                    roll as i16 <= chance
                };
                self.mem.w16(DS, OPEN_SPELL, 0);
                if opens {
                    let w = self.mem.r16(walls, sq) | 0x200;
                    self.mem.w16(walls, sq, w);
                    self.mem.w16(DS, rec, 0xFFFF);
                }
                self.outcome(opens);
            }
        }
        if self.mem.r16(DS, LOCK_SPELL) != 0 {
            // 2826:0920: Lock on an unlocked door.
            let sq = self.mem.r16(DS, PICKED);
            if sq == 0xFFFF || !self.is_door(sq) {
                return Some(());
            }
            if self.lock_on(sq).is_some() {
                return self.plain_click(files, hooks);
            }
            let n = self.mem.r8(DS, LOCK_COUNT);
            let rec = LOCKS.wrapping_add(n as u16 * 3);
            self.mem.w16(DS, rec, (sq & 0xFF00) | ((sq as u8) >> 1) as u16);
            let level = self.mem.r16(DS, LOCK_SPELL) as u8;
            self.mem.w8(DS, rec + 2, level);
            self.mem.w8(DS, LOCK_COUNT, n.wrapping_add(1));
            self.mem.w16(DS, LOCK_SPELL, 0);
            self.outcome(true);
            // 2826:096E: the lock's sound (15h).
            self.sound_here(0x15)?;
        }
        let at = self.mem.r16(DS, TARGET_SPELL);
        if at != 0 {
            // 2826:0982: a spell cast at what was clicked.
            let (off, seg) = (self.mem.r16(DS, at), self.mem.r16(DS, at.wrapping_add(2)));
            let kind = self.mem.r8(seg, off.wrapping_add(0x24));
            if self.mem.r16(seg, off.wrapping_add(0x27)) & 0x80 != 0 || matches!(kind, 2..=4) {
                return hooks.cast_at_target(self, files);
            }
        }
        self.plain_click(files, hooks)
    }

    /// 0000:C9FF (its first instruction): the renderer starts drawing
    /// the view, so the changed squares may step again.
    pub fn temp_walls_begin_draw(&mut self) {
        self.mem.w8(DS, STEPPED, 0);
    }

    /// 0000:D3F9. The renderer drew the square at map offset `sq`: if it
    /// is among the first 16 changed squares (the list holds 32), a wall
    /// being removed (kind 0) fades a step (its high byte up to 0Dh), one
    /// being raised (kind 1) a step back (down to 0). Only the first
    /// square drawn after `temp_walls_begin_draw` steps: any square drawn
    /// sets STEPPED. CB59 bit 0 and CB5A (the step) are for the renderer.
    /// The original advances them only as they are drawn.
    pub fn temp_wall_drawn(&mut self, sq: u16) {
        self.temp_wall_step(sq);
        self.mem.w8(DS, STEPPED, 1);
    }

    fn temp_wall_step(&mut self, sq: u16) {
        let Some(i) = (0..0x10).find(|&i| self.mem.r16(DS, TEMP_WALLS + 2 * i) == sq) else {
            return;
        };
        let at = TEMP_WALLS + 0x40 + 2 * i;
        let ax = self.mem.r16(DS, at);
        let (al, mut ah) = (ax as u8, (ax >> 8) as u8);
        let paused = self.mem.r8(DS, STEPPED) != 0;
        match al {
            0 => {
                if !paused {
                    ah = ah.wrapping_add(1).min(0x0D);
                }
            }
            1 => {
                if !paused {
                    ah = ah.wrapping_sub(1);
                    if (ah as i8) < 0 {
                        ah = 0;
                    }
                }
            }
            _ => return,
        }
        let f = self.mem.r8(DS, 0xCB59) | 1;
        self.mem.w8(DS, 0xCB59, f);
        self.mem.w16(DS, at, (ah as u16) << 8 | al as u16);
        self.mem.w8(DS, 0xCB5A, ah);
    }

    /// 0000:D476, each frame. Changed squares whose change is complete
    /// leave the list: a removed wall (kind 0 at 0Dh) is cleared, a raised
    /// one (kind 1 at 0) loses its marks (4040h).
    pub fn temp_walls_frame(&mut self) {
        let walls = self.walls();
        for i in 0..0x20 {
            let at = TEMP_WALLS + 2 * i;
            let sq = self.mem.r16(DS, at);
            if sq == 0xFFFF {
                continue;
            }
            let (lo, hi) = (self.mem.r8(DS, at + 0x40), self.mem.r8(DS, at + 0x41));
            if lo == 0 {
                if hi == 0x0D {
                    self.mem.w16(DS, at, 0xFFFF);
                    self.mem.w16(walls, sq, 0);
                }
            } else if hi == 0 {
                self.mem.w16(DS, at, 0xFFFF);
                let w = self.mem.r16(walls, sq) & 0x3F3F;
                self.mem.w16(walls, sq, w);
            }
        }
    }

    /// 0000:CD66..CE31. The renderer met the door (wall kind 0Bh) at map
    /// offset `sq`: an opening door steps. Its high byte's low nibble is
    /// the state: bits 1-3 how far open (0 shut .. 7), bit 0 closing.
    /// Within two squares of the player it opens a step (and stops
    /// closing); farther, a closing one closes a step and another opens a
    /// step until fully open, where it starts closing. Returns the sound
    /// the door hook (CD46) is called with, if it is set: the door's
    /// moving sound (table C3C8 by the low byte's bits 6-7) as it opens
    /// out of reach, its shutting sound (C3BC) as it shuts. For the
    /// renderer it sets the kind drawn (CB37), CB38 (those bits), BA7F,
    /// the open amount (CCC4) and, while it is open, C398 bit 1000h. The
    /// original's walk over the squares (0000:C980) met each door within
    /// its reach twice a frame.
    pub fn door_drawn(&mut self, sq: u16) -> Option<u16> {
        let walls = self.walls();
        let (lo, hi) = (self.mem.r8(walls, sq), self.mem.r8(walls, sq.wrapping_add(1)));
        self.mem.w8(DS, 0xCB37, 0xB);
        self.mem.w8(DS, 0xBA7F, 1);
        self.mem.w8(DS, 0xCB38, lo >> 6);
        self.mem.w16(DS, 0xCCC4, 0);
        let state = hi & 0xF;
        if state == 0 {
            return None;
        }
        let f = self.mem.r16(DS, MAP_FLAGS) | 0x1000;
        self.mem.w16(DS, MAP_FLAGS, f);
        let away = |p: u16, c: u16| ((p >> 7).wrapping_sub(c) as i16).unsigned_abs() > 2;
        let near = !away(self.mem.r16(DS, X), (sq >> 1) & 0x7F) && !away(self.mem.r16(DS, Y), sq >> 8);
        let mut sound = None;
        let (step, mut closing) = if near {
            (1i8, false)
        } else if state & 1 != 0 {
            (-1, true)
        } else {
            sound = self.door_sound(0xC3C8, lo);
            (1, false)
        };
        let mut open = ((state >> 1) as i8).wrapping_add(step);
        if open >= 7 {
            open = 7;
            if !near {
                closing = true;
            }
        } else if open <= 0 {
            open = 0;
            closing = false;
        }
        self.mem.w16(DS, 0xCCC4, open as u16);
        if open == 0 {
            sound = self.door_sound(0xC3BC, lo);
        }
        self.mem.w8(walls, sq.wrapping_add(1), 0xB0 | (open as u8) << 1 | closing as u8);
        sound
    }

    /// 0000:A8EF with a door's sound from `table` by bits 6-7 of its low
    /// byte: the sound the door hook (CD46) is called with, unless its
    /// segment is 0.
    fn door_sound(&self, table: u16, lo: u8) -> Option<u16> {
        let hook = crate::movement::var::SOUND_HOOK;
        self.hook_set(hook).then(|| self.mem.r8(DS, table + (lo >> 6) as u16) as u16)
    }

    /// 2826:0A4C. The click is left for the actors.
    fn leave_click(&mut self) {
        let f = self.mem.r16(DS, LEVEL_FLAGS) | 0x80;
        self.mem.w16(DS, LEVEL_FLAGS, f);
        let x = self.mem.r16(DS, MOUSE_X).wrapping_add(4);
        let y = self.mem.r16(DS, MOUSE_Y).wrapping_add(4);
        self.mem.w16(DS, ACTOR_CLICK, x);
        self.mem.w16(DS, ACTOR_CLICK + 2, y);
    }

    /// 2826:09AC. A click with no spell waiting: the wall's trigger, then
    /// a door under the cursor (indoors, within 2 squares, not already
    /// opening) opens unless locked; otherwise the actors get the click.
    fn plain_click(&mut self, files: &mut Files, hooks: &mut dyn ControlHooks) -> Option<()> {
        if self.mem.r16(DS, 0x05B1) & 0x40 != 0 {
            return Some(());
        }
        hooks.wall_trigger(self, files)?;
        self.mem.w16(DS, SPEED, 0);
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
            self.leave_click();
            return Some(());
        }
        let x = self.mem.r16(DS, MOUSE_X).wrapping_add(4) as i16;
        if (self.mem.r16(DS, DOOR_X0) as i16) > x || (self.mem.r16(DS, DOOR_X1) as i16) < x {
            self.leave_click();
            return Some(());
        }
        let (col, row) = (self.mem.r16(DS, DOOR_COL), self.mem.r16(DS, DOOR_ROW));
        let sq = (row as u8 as u16) << 8 | (col as u8).wrapping_add(col as u8) as u16;
        let walls = self.walls();
        let near = |p: u16, c: u16| {
            let d = (p >> 7).wrapping_sub(c) as i16;
            d.unsigned_abs() < 2
        };
        if self.mem.r16(walls, sq) & 0xE00 != 0
            || !near(self.mem.r16(DS, X), col)
            || !near(self.mem.r16(DS, Y), row)
        {
            self.leave_click();
            return Some(());
        }
        match self.door_lock(col, row)? {
            Err(level) => return self.locked_message(level),
            Ok(Some(key)) => self.key_message(key, hooks)?,
            Ok(None) => {}
        }
        let at = self.mem.r16(DS, DOOR_AT);
        let w = self.mem.r16(walls, at) | 0x200;
        self.mem.w16(walls, at, w);
        // 2826:0A6F: the door's opening sound (C3C0 by bits 6-7 of its low
        // byte).
        let n = self.mem.r8(DS, 0xC3C0 + (w as u8 >> 6) as u16) as u16;
        self.sound_here(n)
    }

    /// 2826:0AAB for the player: the lock on the door at column `col`,
    /// row `row`. Ok(None): no lock, or it was picked; Ok(Some(level)):
    /// opened with the key `level` (used up); Err(level): it stays
    /// locked. Picking is tried when armed (5CA bit 0), once per door
    /// (failures are remembered at F88). None where the roll divides by
    /// zero or overflows.
    fn door_lock(&mut self, col: u16, row: u16) -> Option<Result<Option<u8>, u8>> {
        let n = self.mem.r8(DS, LOCK_COUNT) as u16;
        let key = (row as u8 as u16) << 8 | col as u8 as u16;
        let Some(rec) = (0..n).map(|i| LOCKS + 3 * i).find(|&r| self.mem.r16(DS, r) == key) else {
            return Some(Ok(None));
        };
        let level = self.mem.r8(DS, rec + 2);
        if self.mem.r8(DS, crate::movement::var::CREATURE_MODE) != 0 {
            return Some(Err(level));
        }
        if let Some(i) = (0..0xC).find(|&i| self.mem.r8(DS, KEYS_HELD + i) == level) {
            self.mem.w8(DS, KEYS_HELD + i, 0xC5);
            self.mem.w16(DS, rec, 0xFFFF);
            return Some(Ok(Some(level)));
        }
        self.mem.w8(DS, LOCK_TRIED, level);
        if self.mem.r16(DS, 0x05CA) & 1 == 0 {
            return Some(Err(level));
        }
        // 2542:028F
        let f = self.mem.r16(DS, 0x05CA);
        if f & 0xFFFE != 0 {
            self.mem.w16(DS, MESSAGE_TIME, 0);
            self.mem.w16(DS, 0x05CA, f & 0xFFFE);
        }
        let picked = self.mem.r16(DS, PICKED);
        if (0..0x20).any(|i| self.mem.r16(DS, PICK_FAILED + 2 * i) == picked) {
            self.outcome(false);
            return Some(Err(level));
        }
        if self.lockpick()?.0 {
            self.outcome(true);
            self.mem.w16(DS, rec, 0xFFFF);
            return Some(Ok(None));
        }
        self.outcome(false);
        if let Some(i) = (0..0x20).find(|&i| self.mem.r16(DS, PICK_FAILED + 2 * i) == 0xFFFF) {
            self.mem.w16(DS, PICK_FAILED + 2 * i, picked);
        }
        Some(Err(level))
    }

    /// 1194:53F9. The player's lockpicking roll against LOCK_TRIED:
    /// the chance is the scaled sum of the player's bytes 2A and 2C,
    /// divided by the class's factor (2A3E), times level + 1, over the
    /// lock's level, clamped to 0..100; success when d100 is at most it.
    /// Returns (success, chance). None where a division faults.
    pub fn lockpick(&mut self) -> Option<(bool, u16)> {
        let v = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, v);
        let v = self.mem.r16(DS, 0x1034);
        self.mem.w16(DS, 0xB038, v);
        let p = crate::character::PLAYER;
        let lock = (self.mem.r8(DS, LOCK_TRIED) as u16).wrapping_mul(100);
        let sum = self.mem.r8(DS, p + 0x2A) as u16 + self.mem.r8(DS, p + 0x2C) as u16;
        let ax = percent(sum);
        let class = self.mem.r8(DS, p + 5) & 0x1F;
        let f = self.mem.r8(DS, 0x2A3E + class as u16) as u16;
        if f == 0 || ax / f > 0xFF {
            return None;
        }
        let ax = ax / f & 0xFF;
        let ax = ax.wrapping_mul(self.mem.r8(DS, p + 6) as u16 + 1) as u32 * 100;
        if lock == 0 || ax / lock as u32 > 0xFFFF {
            return None;
        }
        let chance = (ax / lock as u32) as u16;
        let roll = self.rng.next() % 100;
        let bx = if (chance as i16) < 0 { 0 } else if chance as i16 > 100 { 100 } else { chance };
        Some((roll <= bx, bx))
    }

    /// 2294:0E35. "This lock ..." by how hard it is to pick (a magic
    /// lock, level 20 and up, says so), unless a failure is showing.
    pub(crate) fn locked_message(&mut self, level: u8) -> Option<()> {
        self.mem.w8(DS, LOCK_TRIED, level);
        if self.mem.r16(DS, MESSAGE_TIME) != 0 && self.mem.r16(DS, MESSAGE) == FAILURE {
            return Some(());
        }
        let (_, chance) = self.lockpick()?;
        let n = ((chance as u8 / 5) as i16 - 6).clamp(0, 12) as u16;
        let si = if level >= 0x14 { 0x528B } else { self.skip_strings(DATA, 0x5074, n) };
        self.mem.w16(DS, MESSAGE_TIME, 0xF);
        self.mem.w16(DS, MESSAGE, si);
        self.mem.w16(DS, MESSAGE + 2, DATA);
        Some(())
    }

    /// 2433:0AA1. "You open door with <key> key." (key names at 342B, 12
    /// of them). None for key 0 (the original's divide overflow).
    fn key_message(&mut self, key: u8, hooks: &mut dyn ControlHooks) -> Option<()> {
        self.mem.w8(DS, 0x12CE, 0);
        let ax = (key as u16).wrapping_sub(1);
        if ax / 12 > 0xFF {
            return None;
        }
        let name = self.skip_strings(DS, 0x342B, ax % 12);
        use crate::format::Arg;
        self.sprintf((DS, 0x7979), (DS, 0x4CBC), &[Arg::Str(DS, name)]);
        if self.mem.r8(DS, 0x12BC) != 0 {
            hooks.message_box(self, 0x7979);
            return Some(());
        }
        self.mem.w16(DS, MESSAGE, 0x7979);
        self.mem.w16(DS, MESSAGE + 2, DS);
        self.mem.w16(DS, MESSAGE_TIME, 0x1E);
        Some(())
    }
}

/// 1194:5B07. `v` x 100 / 256, rounded half away from zero.
fn percent(v: u16) -> u16 {
    let p = (v as i16 as i32) * 100;
    let q = p / 256;
    let r = (p % 256).unsigned_abs();
    let q = q as u16;
    if r >= 0x80 {
        if (v as i16) < 0 { q.wrapping_sub(1) } else { q.wrapping_add(1) }
    } else {
        q
    }
}
