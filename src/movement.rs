//! Movement and collision (segment 0000, F478..FD00).
//!
//! Positions are in map units, 128 per square; the map planes hold one
//! word per square, rows of 0x100 bytes. To test a position, the square
//! it is in and its eight neighbours are turned into a 16 x 16 mask of
//! 8-unit cells (`move_mask`): each neighbour's wall (or, low down, floor)
//! word is classified (`move_classify`) and, if it blocks, a 16 x 16
//! shape is stamped into the mask at the neighbour's offset (11 cells
//! away, or 13 in creature mode). The cell under the position decides.

use crate::bsa::Files;
use crate::hooks::{StairsHooks, value};
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The cell mask (16 x 16 bytes) and the fill the next stamp uses.
    pub const MASK: u16 = 0x929E;
    pub const FILL: u16 = 0xE3A0;
    /// Set while moving a creature rather than the player.
    pub const CREATURE_MODE: u16 = 0xE3A1;
    /// Player position: x, height, y (words), heading (0..511).
    pub const X: u16 = 0xC3D0;
    pub const HEIGHT: u16 = 0xC3D2;
    pub const Y: u16 = 0xC3D4;
    pub const HEADING: u16 = 0xC3DA;
    /// Floor words below this are solid when low down.
    pub const FLOOR_SOLID: u16 = 0xCCCE;
    /// Ceiling height, and the highest step that can be climbed.
    pub const CEILING: u16 = 0xD80C;
    pub const STEP: u16 = 0xE8C0;
    /// Map offset of the last trigger (type B) square stamped.
    pub const TRIGGER_SQUARE: u16 = 0xE8C2;
    /// Map flags (C398) and level flags (C39A).
    pub const MAP_FLAGS: u16 = 0xC398;
    pub const LEVEL_FLAGS: u16 = 0xC39A;
    /// Flats (type 8 walls): the first flat record of the level, and the
    /// records' F: field (byte 3 of 5; bit 0 blocks). See `crate::info`.
    pub const FLAT_BASE: u16 = 0xC38E;
    pub const FLAT_FLAGS: u16 = 0xCDA1;
    /// Hooks called when a creature meets a closed door (far pointers):
    /// the lock check and the sound. The game sets them to 2826:0AAB and
    /// 23BA:002A (`LOCK_CHECK`, `SOUND`).
    pub const LOCK_HOOK: u16 = 0xCD62;
    pub const SOUND_HOOK: u16 = 0xCD46;
    /// Locks: 3-byte records (square word: row << 8 | column, then the
    /// lock), and their count (byte).
    pub const LOCKS: u16 = 0xC624;
    pub const LOCK_COUNT: u16 = 0xC6E4;
    /// Hook asked before testing a player position (far pointer).
    pub const TEST_HOOK: u16 = 0xCD6A;
    /// Candidate positions (4 x (x, y)): new, new x, new y, old; the old
    /// position is also at +0xC.
    pub const CANDIDATES: u16 = 0xE8A6;
    /// Set by a level change during a move; the trigger found (index+1
    /// into TRIGGERS, 0 none); stair markers (up, down) and the trigger
    /// bytes (16).
    pub const CHANGED: u16 = 0xE39E;
    pub const TRIGGER: u16 = 0xCB6A;
    pub const STAIRS_UP: u16 = 0xC932;
    pub const STAIRS_DOWN: u16 = 0xC933;
    pub const TRIGGERS: u16 = 0xC917;
    /// The level (byte), the level count (byte of C355), and the hooks
    /// called at stairs: leaving the dungeon, and taking stairs.
    pub const LEVEL: u16 = 0xBB0A;
    pub const LEVELS: u16 = 0xC355;
    pub const EXIT_HOOK: u16 = 0xCD4E;
    pub const STAIRS_HOOK: u16 = 0xCD6E;
    /// Called after a level change when set (far pointer).
    pub const LEVEL_HOOK: u16 = 0xC3B8;
    /// Set to 0xFFFE by a level change.
    pub const W_A850: u16 = 0xA850;
    /// Climb counter set when climbing out of a low area.
    pub const CLIMB: u16 = 0xE39F;
    /// The physics frame's hooks (far pointers): input and movement, and
    /// transitions on triggers.
    pub const INPUT_HOOK: u16 = 0xC3B4;
    pub const TRANSITION_HOOK: u16 = 0xC3AC;
    /// The floor height the player settles to, and the fall speed.
    pub const FLOOR: u16 = 0xE8BC;
    pub const FALL: u16 = 0xE8BE;
    /// The floor plane's segment and the EMS frame's (floor kinds are
    /// frame-relative).
    pub const FLOOR_SEG: u16 = 0xCCD0;
    pub const FRAME_SEG: u16 = 0x9030;
    /// Bobbing in water: the step (0..15), the offset (CB68) and tables.
    pub const BOB_STEP: u16 = 0xE8B6;
    pub const BOB: u16 = 0xCB68;
    pub const BOB_TABLE: u16 = 0xE824;
    pub const EYE_TABLE: u16 = 0xE844;
}

/// Sine and cosine (words, 512 entries) of a heading.
const SIN: u16 = 0xB03C;
const COS: u16 = 0xB13C;

/// What a position test tells the resolver (the original's ZF and CF).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Test {
    /// ZF: take this position.
    Take,
    /// CF: stop at the old position.
    Stop,
    /// Neither: try the next candidate.
    Next,
}
use var::*;

/// Neighbour tables: (x cells, y cells, map offset) for the 3 x 3 squares,
/// for the player and for creatures.
const NEIGHBOURS: u16 = 0xE7B8;
const NEIGHBOURS_CREATURE: u16 = 0xE7EE;
/// Shapes (16 x 16 bytes): solid, a flat's, and two halves.
const SOLID: u16 = 0xE3A2;
const FLAT_SHAPE: u16 = 0xE4A2;
const HALF_A: u16 = 0xE5A2;
const HALF_B: u16 = 0xE6A2;
/// Wall heights: bottoms by the low 3 bits of the high byte (three sets
/// of 8 by map flags), tops by its high 4 bits (two sets of 16).
const BOTTOMS: u16 = 0xCB94;
const TOPS: u16 = 0xCBC4;
/// The routines the game installs in the door hooks, as (segment << 16)
/// | offset: the lock check and the sound player (which changes no game
/// state).
pub const LOCK_CHECK: u32 = 0x2826_0AAB;
pub const SOUND: u32 = 0x23BA_002A;

/// Offsets of the square ahead by heading quarter, and the player's
/// square (map offset).
const AHEAD: u16 = 0xE384;
const SQUARE: u16 = 0xCB90;
/// Low down means under this height.
const LOW: i16 = 0x3C;

/// The way out's hooks from the stairs': the question to leave and the
/// level's states kept (292B:095E) theirs.
struct Exit<'a>(&'a mut dyn StairsHooks);

impl crate::transit::AreaHooks for Exit<'_> {
    fn ask_leave(&mut self, m: &mut Machine, files: &mut Files) -> bool {
        self.0.ask_leave(m, files)
    }
    fn screen_colours(&mut self) -> Option<[[u8; 3]; 256]> {
        self.0.screen_colours()
    }
    /// 292B:095E, as the stairs keep a level's states.
    fn automap(&mut self, m: &mut Machine, files: &mut Files) {
        self.0.automap_keep(m, files);
    }
}

impl Machine {
    /// 0000:F697. The mask value of the cell under the position at
    /// DS:`pos` (x, then y): 0 free, otherwise what blocks it. Walls are
    /// taken from the wall plane, or for the player low down from the
    /// floor plane. A closed door (0xB0) met by a creature on a level
    /// with flag 2 is opened (0x200) unless the lock hook refuses: the
    /// game's lock check (2826:0AAB) refuses a creature a door with a
    /// lock. None where a hook is set to anything else (not ported).
    pub fn move_cell(&mut self, pos: u16) -> Option<u8> {
        let x = self.mem.r16(DS, pos);
        let y = self.mem.r16(DS, pos.wrapping_add(2));
        let di = ((y >> 7) << 8).wrapping_add((x >> 7) << 1);
        let creature = self.mem.r8(DS, CREATURE_MODE) != 0;
        let plane = if !creature && (self.mem.r16(DS, HEIGHT) as i16) < LOW {
            crate::mif::var::FLOOR_SEG
        } else {
            crate::mif::var::MAP1_SEG
        };
        let seg = self.mem.r16(DS, plane);
        self.move_mask(seg, di);
        let cell = (((y & 0x7F) >> 3) << 4) + ((x & 0x7F) >> 3);
        let al = self.mem.r8(DS, MASK + cell);
        if al == 0xB0 && creature && self.mem.r16(DS, LEVEL_FLAGS) & 2 != 0 {
            let sq = self.mem.r16(DS, TRIGGER_SQUARE);
            match self.mem.r32(DS, LOCK_HOOK) {
                0 => {}
                LOCK_CHECK => {
                    // 2826:0AAB with E3A1 set: a lock on the square refuses.
                    let key = ((sq >> 8) << 8) | ((sq as u8) >> 1) as u16;
                    let n = self.mem.r8(DS, LOCK_COUNT) as u16;
                    if (0..n).any(|i| self.mem.r16(DS, LOCKS.wrapping_add(3 * i)) == key) {
                        return Some(al);
                    }
                }
                _ => return crate::giveup::not_ported("2826:0AAB: a move hook not ported"),
            }
            if !matches!(self.mem.r32(DS, SOUND_HOOK), 0 | SOUND) {
                return crate::giveup::not_ported("2826:0AAB: the sound hook set to a routine not ported");
            }
            let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
            let w = self.mem.r16(walls, sq) | 0x200;
            self.mem.w16(walls, sq, w);
            return Some(0);
        }
        Some(al)
    }

    /// 0000:FB3A. Builds the cell mask for the square at map offset `di`
    /// of the plane at `seg` and its eight neighbours.
    pub fn move_mask(&mut self, seg: u16, di: u16) {
        for i in 0..0x100 {
            self.mem.w8(DS, MASK + i, 0);
        }
        let table = if self.mem.r8(DS, CREATURE_MODE) != 0 { NEIGHBOURS_CREATURE } else { NEIGHBOURS };
        for k in 0..9u16 {
            let e = table + k * 6;
            let ax = self.mem.r16(DS, e);
            let bx = self.mem.r16(DS, e + 2);
            let at = di.wrapping_add(self.mem.r16(DS, e + 4));
            let w = self.mem.r16(seg, at);
            // CL is the loop counter (9 down to 1) unless set.
            if let Some((shape, fill)) = self.move_classify(w, at, (9 - k) as u8) {
                self.move_stamp(ax, bx, shape, fill);
            }
        }
    }

    /// 0000:FB81. Whether wall word `w` (at map offset `at`) blocks, and
    /// with which shape and fill. `cl` is the fill when none is chosen.
    fn move_classify(&mut self, w: u16, at: u16, cl: u8) -> Option<(u16, u8)> {
        let creature = self.mem.r8(DS, CREATURE_MODE) != 0;
        let height = self.mem.r16(DS, HEIGHT);
        if !creature && (height as i16) < LOW {
            // A floor word: solid below FLOOR_SOLID.
            return (w < self.mem.r16(DS, FLOOR_SOLID)).then_some((SOLID, cl));
        }
        if w == 0 {
            return None;
        }
        let (lo, hi) = (w as u8, (w >> 8) as u8);
        if w & 0x8000 == 0 {
            let flags = self.mem.r16(DS, LEVEL_FLAGS);
            if hi == lo || (flags & 0x400 != 0 && creature) {
                return Some((SOLID, lo));
            }
            let (top, bottom) = self.move_wall_height(w);
            if (bottom as i16) > (height as i16) {
                return None;
            }
            // These exits fill with AL, by now the height being compared.
            let mut ax = top;
            if flags & 0x200 == 0 {
                ax = ax.wrapping_add(0x3C);
                if ax >= self.mem.r16(DS, CEILING) {
                    return Some((SOLID, ax as u8));
                }
            }
            let rise = ax.wrapping_sub(height);
            if (rise as i16) > self.mem.r16(DS, STEP) as i16 {
                return Some((SOLID, rise as u8));
            }
            return None;
        }
        match hi >> 4 {
            8 => {
                if w & 0x200 != 0 {
                    return None;
                }
                let fill = (0x80 | lo).wrapping_add(1);
                let flat = (lo as u16).wrapping_add(self.mem.r16(DS, FLAT_BASE));
                if self.mem.r8(DS, FLAT_FLAGS.wrapping_add(flat.wrapping_mul(5))) & 1 == 0 {
                    return None;
                }
                Some((FLAT_SHAPE, fill))
            }
            0xD => Some((if w & 0x100 == 0 { HALF_A } else { HALF_B }, 0x80 | lo)),
            0xA => (w & 0x100 != 0).then_some((SOLID, lo)),
            9 => (w & 0x100 == 0).then_some((SOLID, lo)),
            0xB => {
                self.mem.w16(DS, TRIGGER_SQUARE, at);
                (w & 0xF00 == 0).then_some((SOLID, 0xB0))
            }
            _ => None,
        }
    }

    /// 0000:FCB9. A wall's top and bottom heights from its high byte.
    pub fn move_wall_height(&self, w: u16) -> (u16, u16) {
        let ah = (w >> 8) as u8;
        let mut bx = ((ah & 7) as u16) * 2;
        if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
            bx += 0x10;
            if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
                bx += 0x10;
            }
        }
        let bottom = self.mem.r16(DS, BOTTOMS + bx);
        let mut bx = ((ah >> 3) & 0xF) as u16 * 2;
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            bx += 0x20;
        }
        (self.mem.r16(DS, TOPS + bx).wrapping_add(bottom), bottom)
    }

    /// 0000:FC5B. Stamps `shape` into the mask, shifted by (`ax`, `bx`)
    /// cells: its set bytes become `fill`; what falls outside the mask's
    /// rows is dropped, and columns are clipped.
    fn move_stamp(&mut self, ax: u16, bx: u16, shape: u16, fill: u8) {
        self.mem.w8(DS, FILL, fill);
        let mut row = bx << 4;
        let mut src = shape;
        for _ in 0..16 {
            if (row as i16) >= 0 && (row as i16) < 0x100 {
                let mut di = MASK.wrapping_add(row);
                let mut si = src;
                let n = if ax == 0 {
                    16
                } else if (ax as i16) > 0 {
                    di = di.wrapping_add(ax);
                    0x10u16.wrapping_sub(ax)
                } else {
                    si = si.wrapping_add(ax.wrapping_neg());
                    0x10u16.wrapping_add(ax)
                };
                // `loop` with CX = 0 runs 65536 times.
                for _ in 0..if n == 0 { 0x10000u32 } else { n as u32 } {
                    if self.mem.r8(DS, si) != 0 {
                        self.mem.w8(DS, di, fill);
                    }
                    si = si.wrapping_add(1);
                    di = di.wrapping_add(1);
                }
            }
            src = src.wrapping_add(16);
            row = row.wrapping_add(16);
        }
    }

    /// 0000:F8E1 (far F8DD). Moves the player by `speed` along heading
    /// `angle` (0..511): x and y get the high words of 2 * speed * sin and
    /// of 2 * speed * cos, then `move_resolve` settles where the player
    /// ends up.
    /// Returns the candidate taken (0 the full move .. 3 none). Stairs
    /// may change level on the way (`move_trigger`, with `hooks` for the
    /// stairs' hooks). None where a hook is set to a routine not ported,
    /// or a level change fails (see `move_change_level`).
    pub fn move_step(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks, speed: u16, angle: u16) -> Option<u16> {
        let (ox, oy) = (self.mem.r16(DS, X), self.mem.r16(DS, Y));
        let a = speed.wrapping_add(speed) as i16 as i32;
        let b = angle.wrapping_add(angle);
        let s = self.mem.r16(DS, SIN.wrapping_add(b)) as i16 as i32;
        let c = self.mem.r16(DS, COS.wrapping_add(b)) as i16 as i32;
        let nx = ox.wrapping_add(((a * s) >> 16) as u16);
        let ny = oy.wrapping_add(((a * c) >> 16) as u16);
        self.mem.w16(DS, X, nx);
        self.mem.w16(DS, Y, ny);
        let (x, y, k) = self.move_resolve(files, hooks, nx, ny, ox, oy)?;
        self.mem.w16(DS, X, x);
        self.mem.w16(DS, Y, y);
        Some(k)
    }

    /// 0000:F478. Tries (nx, ny), (nx, oy) and (ox, ny) in turn and takes
    /// the first the test allows; a test that says stop, or one that
    /// refuses while map flag 0x80 is set, ends at (ox, oy).
    ///
    /// The walk's test (the hook CD6A, `Machine::walk_test`) reads the
    /// position from AX and BX, which hold (nx, ny) only for the first
    /// candidate: the next are tested with what the test before left
    /// there (`move_test`).
    pub(crate) fn move_resolve(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks, nx: u16, ny: u16, ox: u16, oy: u16) -> Option<(u16, u16, u16)> {
        self.mem.w8(DS, CHANGED, 0);
        self.mem.w16(DS, TRIGGER, 0);
        for (i, w) in [nx, ny, nx, oy, ox, ny, ox, oy].into_iter().enumerate() {
            self.mem.w16(DS, CANDIDATES + i as u16 * 2, w);
        }
        let mut regs = (nx, ny);
        for k in 0..3u16 {
            match self.move_test(files, hooks, CANDIDATES + k * 4, &mut regs)? {
                Test::Take => {
                    let p = CANDIDATES + k * 4;
                    return Some((self.mem.r16(DS, p), self.mem.r16(DS, p + 2), k));
                }
                Test::Stop => return Some((ox, oy, 3)),
                Test::Next => {
                    if self.mem.r16(DS, MAP_FLAGS) & 0x80 != 0 {
                        return Some((ox, oy, 3));
                    }
                }
            }
        }
        Some((ox, oy, 3))
    }

    /// 0000:F60A. Tests the candidate at DS:`pos`. `regs` are AX and BX:
    /// the player's test first asks the walk's test (the hook CD6A,
    /// 0000:4009) about the position they hold; a refusal sets AL's bit 0
    /// and leaves BX, so the next candidate asks about the same square.
    /// Otherwise 0000:F697 leaves AX the cell's byte (AH 0) and BX its
    /// index in the mask, which the rest keeps, but for the climb's
    /// heading (`move_climb`) and the cursor's y after stairs
    /// (`move_change_level`). (The ways out of a dungeon, the hook CD4E,
    /// leave BX as their routines do: not modelled; the area ends.)
    fn move_test(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks, pos: u16, regs: &mut (u16, u16)) -> Option<Test> {
        let creature = self.mem.r8(DS, CREATURE_MODE) != 0;
        if !creature {
            match self.mem.r32(DS, TEST_HOOK) {
                0 => {}
                value::WALK_TEST => {
                    if self.walk_test(regs.0, regs.1) {
                        regs.0 |= 1;
                        return Some(Test::Next);
                    }
                }
                _ => return crate::giveup::not_ported("0000:F60A: a walk-test hook not ported"),
            }
        }
        let al = self.move_cell(pos)?;
        let (x, y) = (self.mem.r16(DS, pos), self.mem.r16(DS, pos.wrapping_add(2)));
        *regs = (al as u16, (((y & 0x7F) >> 3) << 4) + ((x & 0x7F) >> 3));
        let free = |al: u8| if al == 0 { Test::Take } else { Test::Next };
        if creature || al == 0 {
            return Some(free(al));
        }
        if al & 0x80 == 0 && self.mem.r16(DS, HEIGHT) as i16 >= LOW {
            let t = self.move_trigger(files, hooks, al, regs)?;
            if t != 0 {
                if self.move_heading_square() {
                    self.mem.w16(DS, TRIGGER, t);
                }
                return Some(free(al));
            }
        }
        // F649
        let f = self.mem.r16(DS, MAP_FLAGS) & !0x200;
        self.mem.w16(DS, MAP_FLAGS, f);
        if self.mem.r16(DS, HEIGHT) as i16 >= LOW {
            return Some(free(al));
        }
        let ox = self.mem.r16(DS, CANDIDATES + 0xC);
        let oy = self.mem.r16(DS, CANDIDATES + 0xE);
        let mut dxs = 0u16;
        let mut ax = self.mem.r16(DS, pos).wrapping_sub(ox);
        if (ax as i16) < 0 {
            dxs = 0xFFFF;
            ax = ax.wrapping_neg();
        }
        let mut dys = 0u16;
        let mut bx = self.mem.r16(DS, pos + 2).wrapping_sub(oy);
        if (bx as i16) < 0 {
            dys = 0xFFFF;
            bx = bx.wrapping_neg();
        }
        let heading = self.mem.r16(DS, HEADING);
        if ax >= bx {
            self.move_climb(dxs, heading.wrapping_sub(0x80) & 0x1FF, pos, regs)
        } else {
            self.move_climb(dys, heading, pos, regs)
        }
    }

    /// 0000:F76F. For a trigger byte, its index + 1 in TRIGGERS (0 if
    /// none). For a stair byte, 0, after a level change when the player
    /// faces the stair square (0000:FDAA) and no level changed yet this
    /// move: up from the first level, or down from the last (or down
    /// with level flag 0x100), there is none.
    ///
    /// The stairs' hook (CD6E, `value::STAIRS`, `Machine::stairs_hook`) is
    /// called before the level changes; past the first level or the last,
    /// the way out instead (`move_exit`).
    fn move_trigger(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks, al: u8, regs: &mut (u16, u16)) -> Option<u16> {
        let stairs = [self.mem.r8(DS, STAIRS_UP), self.mem.r8(DS, STAIRS_DOWN)];
        let Some(i) = stairs.iter().position(|&s| s == al) else {
            return Some((0..16).find(|&i| self.mem.r8(DS, TRIGGERS + i) == al).map_or(0, |i| i + 1));
        };
        if !self.move_facing(al) || self.mem.r8(DS, CHANGED) != 0 {
            return Some(0);
        }
        let stairs = |m: &mut Self, files: &mut Files, hooks: &mut dyn StairsHooks| -> Option<()> {
            match m.mem.r32(DS, STAIRS_HOOK) {
                0 => {}
                value::STAIRS => m.stairs_hook(files, hooks),
                _ => return crate::giveup::not_ported("0000:F76F: the stairs hook set to a routine not ported"),
            }
            Some(())
        };
        let level = self.mem.r8(DS, LEVEL);
        let to = if i == 0 {
            // dec, then the sign of the byte.
            let to = level.wrapping_sub(1);
            if to & 0x80 != 0 {
                return self.move_exit(files, hooks, to);
            }
            to
        } else {
            if self.mem.r16(DS, LEVEL_FLAGS) & 0x100 != 0 {
                return Some(0);
            }
            stairs(self, files, hooks)?;
            let to = level.wrapping_add(1);
            if to >= self.mem.r8(DS, LEVELS) {
                return self.move_exit(files, hooks, to);
            }
            to
        };
        if i == 0 {
            stairs(self, files, hooks)?;
        }
        self.mem.w8(DS, LEVEL, to);
        self.move_change_level(files, hooks, regs)?;
        Some(0)
    }

    /// 0000:F791..F7A3 and F842..F85B: past the first level or the
    /// last, the way out (the hook CD4E) with the level at `to`
    /// meanwhile. The ways out the areas set (0000:4537, 44BD) are
    /// `Machine::area_exit`, recognised by their value: the port loads
    /// the image at segment 0, where the original's are relocated and
    /// pass the hook's test (0000:A8EF, the segment). Their question
    /// (26F7:0000) is the stairs' hooks' `ask_leave`; the rest of their
    /// I/O isn't drawn. None for another hook set.
    fn move_exit(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks, to: u8) -> Option<u16> {
        let ported = matches!(self.mem.r32(DS, EXIT_HOOK), 0x4537 | 0x44BD);
        if self.hook_set(EXIT_HOOK) && !ported {
            return crate::giveup::not_ported("0000:F791: the exit hook set to a routine not ported");
        }
        if ported {
            let level = self.mem.r8(DS, LEVEL);
            self.mem.w8(DS, LEVEL, to);
            self.area_exit(files, &mut Exit(hooks))?;
            self.mem.w8(DS, LEVEL, level);
        }
        Some(0)
    }

    /// 0000:F7B5..F7F5 (and F85E..F89E). After the level's planes and
    /// chunks are reloaded (`level_change`), the player turns round and
    /// the new level's INF, if one was loaded, is loaded, the row tables
    /// made between (0000:DA8E, `rows_note`). The wait for the mouse
    /// buttons to be released (8FC8) is skipped; it leaves BX the cursor's y, kept within
    /// its limits (8F94, 8F78..8F7A), which the next candidate's test
    /// reads (`regs`). Then the hook C3B8: a dungeon's level entered
    /// (`value::LEVEL_ENTERED`, `Machine::level_entered`; the view's
    /// rectangle saved and put back around it, 0000:A606/A5D1, is the
    /// renderer's). None where the reload or the INF load fails, or the
    /// hook at C3B8 is set to a routine not ported.
    fn move_change_level(&mut self, files: &mut Files, hooks: &mut dyn StairsHooks, regs: &mut (u16, u16)) -> Option<()> {
        self.level_change(files)?;
        self.mem.w8(DS, CHANGED, 1);
        self.mem.w16(DS, W_A850, 0xFFFE);
        let h = self.mem.r16(DS, HEADING).wrapping_add(0x100) & 0x1FF;
        self.mem.w16(DS, HEADING, h);
        // F7D3 (F87C): the row tables made before the INF's ceiling.
        self.rows_note();
        let info = self.mem.r16(DS, crate::info::var::INFO_PTR);
        if info != 0 {
            self.info_load(files, info)?;
            self.mem.w16(DS, crate::info::var::INFO_PTR, 0);
        }
        let y = self.mem.r16(DS, 0x8F94);
        let y = if (y as i16) < 0 { 0 } else { y };
        let y = y.max(self.mem.r16(DS, 0x8F78));
        regs.1 = y.min(self.mem.r16(DS, 0x8F7A));
        match self.mem.r32(DS, LEVEL_HOOK) {
            0 => {}
            value::LEVEL_ENTERED => self.level_entered(files, hooks)?,
            _ => return crate::giveup::not_ported("0000:F7B5: the level hook set to a routine not ported"),
        }
        Some(())
    }

    /// 0000:FDAA. Whether the wall square ahead (by heading, from the
    /// player's square) holds byte `al`.
    fn move_facing(&self, al: u8) -> bool {
        let h = self.mem.r16(DS, HEADING).wrapping_add(0x40) & 0x1FF;
        let step = self.mem.r16(DS, AHEAD.wrapping_add((h >> 7) * 2));
        let at = step.wrapping_add(self.mem.r16(DS, SQUARE));
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        self.mem.r8(walls, at) == al
    }

    /// 0000:F913. True (carry) when the heading points within 16 of an
    /// axis.
    fn move_heading_square(&self) -> bool {
        let h = self.mem.r16(DS, HEADING);
        // Bands alternate: below 0x10 carry, 0x10..0x70 none, ..0x90
        // carry, ..0xF0 none, and so on to 0x1F0.
        let bands = [0x10, 0x70, 0x90, 0xF0, 0x110, 0x170, 0x190, 0x1F0];
        match bands.iter().position(|&b| h < b) {
            Some(i) => i % 2 == 0,
            None => true,
        }
    }

    /// 0000:F955. Climbing out of a low area: when moving along an axis
    /// (`sign` set turns the heading round first; within 16 of north or
    /// south), the position is tested at height 0x3C, and if free (and
    /// level flag 0x4000 isn't set) the player rises by 6 (24 with map
    /// flag 1), up to 0x3C. Along no axis BX is left the heading (`regs`);
    /// otherwise the test at 0x3C leaves the cell's index there again.
    /// (0000:DA8E, the view's update, would change it; it is the
    /// renderer's, and the vectors' DA8E is a RET.)
    fn move_climb(&mut self, sign: u16, heading: u16, pos: u16, regs: &mut (u16, u16)) -> Option<Test> {
        let h = if sign != 0 { heading.wrapping_sub(0x100) & 0x1FF } else { heading };
        if (0x10..0x1F0).contains(&h) {
            regs.1 = h;
            return Some(Test::Next);
        }
        let height = self.mem.r16(DS, HEIGHT);
        self.mem.w16(DS, HEIGHT, 0x3C);
        let al = self.move_cell(pos);
        self.mem.w16(DS, HEIGHT, height);
        if al? != 0 || self.mem.r16(DS, LEVEL_FLAGS) & 0x4000 != 0 {
            return Some(Test::Next);
        }
        let mut h = height.wrapping_add(6);
        if self.mem.r16(DS, MAP_FLAGS) & 1 != 0 {
            h = h.wrapping_add(0x12);
        }
        self.mem.w16(DS, HEIGHT, h);
        // F9A5: the row tables made above 0, before the height is held
        // at 3Ch (the drawings after build theirs from the height before).
        if (h as i16) > 0 {
            self.rows_note();
        }
        if (h as i16) >= LOW {
            self.mem.w16(DS, HEIGHT, 0x3C);
            self.mem.w8(DS, CLIMB, 6);
            return Some(if h == 0x3C { Test::Take } else { Test::Next });
        }
        let f = self.mem.r16(DS, MAP_FLAGS) | 0x200;
        self.mem.w16(DS, MAP_FLAGS, f);
        Some(Test::Stop)
    }

    /// 0000:F57C. The player's physics each frame: the climb countdown,
    /// a pending view refresh (C398 bit 0x2000: the row tables made,
    /// `rows_note`), then, when the input hook (C3B4) is set, the input
    /// and movement step and the fall (`fall_step`). A trigger left in
    /// CB6A goes to the transition hook (C3AC) when that is set (the
    /// view window is saved and restored around it, 0000:A606/A5D1, for
    /// the renderer); if the
    /// level changed, the new level is loaded (unless 29CF's stairs
    /// already did, E39E) and the player turns round. The original also
    /// waits for the mouse buttons to be released there.
    pub fn physics_frame(&mut self, files: &mut Files, hooks: &mut dyn PhysicsHooks) -> Option<()> {
        let c = self.mem.r8(DS, CLIMB);
        if c != 0 {
            self.mem.w8(DS, CLIMB, c - 1);
        }
        let f = self.mem.r16(DS, MAP_FLAGS);
        if f & 0x2000 != 0 {
            self.mem.w16(DS, MAP_FLAGS, f & !0x2000);
            // F595.
            self.rows_note();
        }
        if !self.hook_set(INPUT_HOOK) {
            return Some(());
        }
        hooks.input(self, files)?;
        self.fall_step();
        let trigger = self.mem.r16(DS, TRIGGER);
        if trigger == 0 || !self.hook_set(TRANSITION_HOOK) {
            return Some(());
        }
        let level = self.mem.r8(DS, LEVEL);
        // The 3D mode's transition (0000:4470) is a RETF.
        if self.mem.r32(DS, TRANSITION_HOOK) != value::TRANSITION {
            hooks.transition(self, files, trigger)?;
        }
        self.mem.w16(DS, TRIGGER, 0);
        if self.mem.r8(DS, LEVEL) == level {
            return Some(());
        }
        if self.mem.r8(DS, CHANGED) == 0 {
            self.level_change(files)?;
        }
        self.mem.w8(DS, CHANGED, 1);
        self.mem.w16(DS, W_A850, 0xFFFE);
        let h = self.mem.r16(DS, HEADING).wrapping_add(0x100) & 0x1FF;
        self.mem.w16(DS, HEADING, h);
        // F606.
        self.rows_note();
        Some(())
    }

    /// 0000:F9C4. The player's height settles towards the floor under
    /// them: the top of a raised square (3Ch above it, or the wall's top + 3Ch
    /// when standing on it), the ground (3Ch), or for a floor
    /// below FLOOR_SOLID's kind the depth of water or a pit (-80, or -25, -10,
    /// -50 and -1 by the level's flags, bobbing on the way). Falling
    /// moves down by E8BE, doubling every other frame, not while
    /// levitating (C398 bit 0x200), climbing or with level flag 0x8000;
    /// landing low pulls the player off the square's edges. Each change of
    /// height makes the row tables (0000:DA8E, `rows_note`).
    pub fn fall_step(&mut self) {
        let f = self.mem.r16(DS, LEVEL_FLAGS) & !0x10;
        self.mem.w16(DS, LEVEL_FLAGS, f);
        let f = self.mem.r16(DS, MAP_FLAGS) & !0x800;
        self.mem.w16(DS, MAP_FLAGS, f);
        self.mem.w16(DS, BOB, 0);
        let (px, py) = (self.mem.r16(DS, X), self.mem.r16(DS, Y));
        // 0000:F565
        let sq = ((py >> 7) << 8).wrapping_add((px >> 7) << 1);
        let walls = self.mem.r16(DS, crate::mif::var::MAP1_SEG);
        let wall = self.mem.r16(walls, sq);
        let height = self.mem.r16(DS, HEIGHT);
        let (target, wet) = 'floor: {
            if height as i16 >= 0x3C && wall != 0 {
                if (wall >> 8) as u8 != wall as u8 {
                    self.stand_on(wall);
                    break 'floor (self.mem.r16(DS, FLOOR), false);
                }
                break 'floor (0x3C, false);
            }
            let floors = self.mem.r16(DS, FLOOR_SEG);
            let fl = self.mem.r16(floors, sq);
            if fl < self.mem.r16(DS, FLOOR_SOLID) {
                break 'floor (0x3C, false);
            }
            let level = self.mem.r16(DS, LEVEL_FLAGS);
            if level & 1 != 0 && height <= 0x3E {
                break 'floor (0x3C, false);
            }
            if fl.wrapping_sub(self.mem.r16(DS, FRAME_SEG)) == 0xC00 {
                break 'floor (0xFFB0, false);
            }
            let mut bx = if level & 0x4000 != 0 { 0xFFF6 } else { 0xFFE7 };
            if self.mem.r16(DS, MAP_FLAGS) & 4 != 0 {
                bx = 0xFFCE;
                if level & 0x200 != 0 {
                    bx = if level & 0x4000 != 0 { 0xFFFF } else { 0xFFF6 };
                }
            }
            (bx, true)
        };
        self.mem.w16(DS, FLOOR, target);
        let height = self.mem.r16(DS, HEIGHT);
        if (target as i16) < height as i16 {
            if self.mem.r16(DS, MAP_FLAGS) & 0x200 != 0
                || self.mem.r8(DS, CLIMB) != 0
                || self.mem.r16(DS, LEVEL_FLAGS) & 0x8000 != 0
            {
                return;
            }
            let f = self.mem.r16(DS, MAP_FLAGS) | 0x800;
            self.mem.w16(DS, MAP_FLAGS, f);
            let ax = height.wrapping_sub(self.mem.r16(DS, FALL));
            if (target as i16) < ax as i16 {
                if self.mem.r16(DS, 0xCCBC) & 1 != 0 {
                    let v = self.mem.r16(DS, FALL) << 1;
                    self.mem.w16(DS, FALL, v);
                }
                if height == ax {
                    return;
                }
                self.mem.w16(DS, HEIGHT, ax);
                if (ax as i16) < 0x3C {
                    for at in [X, Y] {
                        let v = self.mem.r16(DS, at);
                        let e = v & 0x7F;
                        let e = if e < 0x32 {
                            e + 0x10
                        } else if e > 0x4E {
                            e - 0x10
                        } else {
                            0x40
                        };
                        self.mem.w16(DS, at, (v & 0xFF80).wrapping_add(e));
                    }
                }
                // FAD6.
                self.rows_note();
                return;
            }
        }
        // 0000:FADA: settled on the floor.
        let target = self.mem.r16(DS, FLOOR);
        if self.mem.r16(DS, HEIGHT) != target {
            self.mem.w16(DS, HEIGHT, target);
            // FAE7.
            self.rows_note();
            self.mem.w16(DS, FALL, 4);
        }
        if !wet {
            return;
        }
        let f = self.mem.r16(DS, LEVEL_FLAGS) | 0x10;
        self.mem.w16(DS, LEVEL_FLAGS, f);
        let bx = self.mem.r16(DS, BOB_STEP);
        if self.mem.r16(DS, 0xCCBC) & 1 != 0 {
            self.mem.w16(DS, BOB_STEP, bx.wrapping_add(1) & 0xF);
        }
        let at = bx.wrapping_mul(2);
        let b = self.mem.r16(DS, BOB_TABLE.wrapping_add(at));
        self.mem.w16(DS, BOB, b);
        let e = self.mem.r16(DS, 0xC3D8).wrapping_add(self.mem.r16(DS, EYE_TABLE.wrapping_add(at)));
        self.mem.w16(DS, 0xC3D8, e);
    }

    /// 0000:F73B. Standing on a raised square: its top (+3Ch) becomes the
    /// floor (E8BC) if the player is not below it and, unless the level
    /// has flag 200h, it is not above the ceiling.
    fn stand_on(&mut self, wall: u16) {
        let height = self.mem.r16(DS, HEIGHT);
        if (height.wrapping_sub(0x3C) as i16) < 0 || (wall as i16) < 0 {
            return;
        }
        let (top, bottom) = self.move_wall_height(wall);
        if bottom as i16 > height as i16 {
            return;
        }
        let ax = top.wrapping_add(0x3C);
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 == 0 && ax > self.mem.r16(DS, CEILING) {
            return;
        }
        self.mem.w16(DS, FLOOR, ax);
    }
}

/// The hooks the physics frame calls (far pointers the mode's code sets).
pub trait PhysicsHooks {
    /// Hook C3B4 (2826:0002 in 3D areas): the player's input and movement.
    fn input(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// Hook C3AC (0000:4470): a trigger (CB6A) taking the player
    /// elsewhere.
    fn transition(&mut self, _m: &mut Machine, _files: &mut Files, _trigger: u16) -> Option<()> {
        Some(())
    }
}

/// `PhysicsHooks` that do nothing.
pub struct NoPhysicsHooks;

impl PhysicsHooks for NoPhysicsHooks {}

