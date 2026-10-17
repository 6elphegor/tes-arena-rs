//! The spellmaker (1194:6064, `spellmaker`), the mages' guild's
//! "Spellmaker" (1194:A80D: 0000:A611, then this with SI the player, then
//! 0000:A6B5; the two far calls keep and give back the world around the
//! screen and are the front end's).
//!
//! The spell being made is a 55h-byte record at 57E6 (the layout of the
//! spellbook's records in DATA): for each of its three effects (by the
//! slot, 585B) six words, 57E6 + 2i + 0, 6, 0Ch, 12h, 18h, 1Eh, the form's
//! fields; the target (580A), the element (580C), a flags word (580D),
//! the effects (580F + i, FFh none), their kinds (5812 + i) and sub-kinds
//! (5815 + i), the cost (5818) and the name (581A). Each effect's cost is
//! at 58A9 + 2i and its lowest target at 5A84 + 2i (FFFFh: none).
//!
//! `spellmaker`: the slot table and the rest made empty (1194:6A7A), the
//! name's background kept (0x528 bytes on the heap, A890), the cursors'
//! resource registered (0000:756C: spparts.rci), the screen drawn
//! (1194:68D7), then clicks (1194:5651) are matched against seven
//! regions (5711: x0, y0, x1, y1, the routine): the name typed (65FD),
//! the effects (663D: added, modified or deleted through a menu), the
//! target (6A1C) and the element (6A45) picked from lists, the spell
//! cleared after "Are you sure ?" (6A6C), bought (6BD7), or "exit". An
//! effect's form (1194:6105) is a picture (formN.img, or formNa.img)
//! with controls (the records at 57A2: 12h bytes each after a header):
//! arrows that step a field within its range, keeping a linked field at
//! least (or at most) as large, and check boxes that set or clear a bit;
//! every change recomputes the effect's cost (the routine at 58B5, from
//! 57C4 by the form) and shows it as the player's class pays (1194:636B).
//! The spell's cost (1194:6AA9) is the effects' costs, by the class's
//! factors for the three classes that have them (63BB: 53DB, 540D,
//! 53F4), times the target's step over the effects' lowest targets plus
//! two, halved; its price twice that. Bought (6BD7) with a name, an
//! effect and the gold, it goes into the first empty slot of the
//! player's spells (DATA 2A80: 32 of 55h bytes), its number (80h + slot)
//! onto the player's list (+366h, +367h), and the spells' file is
//! written (29CF:06CB).
//!
//! An effect's form shows its description: the effect's entry in
//! SPELLMKR.TXT ("#nn", then lines up to the next '#'), its `%` codes
//! made into a format and its values pushed (1194:6D87): %0..%5 the
//! effect's fields, %a the word at 5873, %b "not" when 580D bit 100h is
//! set (otherwise the code is left as "% ", which the formatter skips),
//! %c the sub-kind's name, %e "caster"/"party" and the rest "follows the
//! caster"/"is a projectile" by 580D's bits 40h and 80h; lines end in
//! '$', made NULs.
//!
//! The front end's (`SpellmakerHooks`): the clicks, lists, menus,
//! message boxes, the typing of the name (0000:9EE5), the drawing and
//! the fonts, and the spells' file's write (25FD:0844, 0000:9DD2). The
//! form's save-under (0000:8F26), its picture (5828) and the screen's
//! routines leave nothing in DGROUP the port keeps. A876, the stack
//! pointer 1194:6DD2 saves, is a stand-in, FFD0h. The original's division faults (a cost
//! routine dividing by 0 or past 16 bits) give None.

use crate::bsa::Files;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// What the spellmaker leaves to the front end.
pub trait SpellmakerHooks {
    /// 1194:5651: the next click on the screen, (x, y) (AX, BX), on the
    /// main screen or on the form whose record is at DATA:`form`. By
    /// default "exit", or the form's done box.
    fn click(&mut self, m: &mut Machine, form: Option<u16>) -> (u16, u16) {
        match form {
            None => (0x128, 0xC0),
            Some(f) => {
                let r = |o: u16| m.mem.r16(DATA, f.wrapping_add(o));
                let x = m.mem.r16(DS, var::FORM_X).wrapping_add(r(0xA));
                let y = m.mem.r16(DS, var::FORM_Y).wrapping_add(r(0xC));
                (x, y)
            }
        }
    }
    /// 1194:5734: a list of `cx` strings at DS:`si`. The pick (FFFFh:
    /// none).
    fn list(&mut self, _m: &mut Machine, _si: u16, _cx: u16) -> u16 {
        0xFFFF
    }
    /// 1194:020E: a menu, its title at `si`, its keys at `bx`, its
    /// entries' table at `di`. The pick (FFFFh: none).
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        0xFFFF
    }
    /// 1194:59CC: a message box with the text at DS:`si` (its colour at
    /// 9288).
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 0000:9EE5: the spell's name typed into DS:`si` (581A, at most 20h
    /// bytes with its NUL).
    fn type_name(&mut self, _m: &mut Machine, _si: u16) {}
    /// The screen drawn (1194:68D7's end).
    fn screen(&mut self, _m: &mut Machine) {}
    /// An effect's form shown (1194:6105), its record at DATA:`form`.
    fn form(&mut self, _m: &mut Machine, _form: u16) {}
    /// 29CF:06CB's write (25FD:0844, 0000:9DD2): the spells' file named
    /// at DS:`name` with DATA:2A80's AA0h bytes.
    fn spells_write(&mut self, m: &mut Machine, files: &mut Files, name: u16) {
        m.file_write(files, name, DATA, var::SPELLS, 0xAA0);
    }
}

/// An effect's name and its description's lines.
pub type EffectText = (Vec<u8>, Vec<Vec<u8>>);

/// `SpellmakerHooks` that do nothing: the first click leaves.
pub struct NoSpellmakerHooks;

impl SpellmakerHooks for NoSpellmakerHooks {}

/// DGROUP variables (DATA where said).
pub mod var {
    /// The spell being made (55h bytes) and its parts.
    pub const SPELL: u16 = 0x57E6;
    pub const TARGET: u16 = 0x580A;
    pub const ELEMENT: u16 = 0x580C;
    pub const FLAGS: u16 = 0x580D;
    pub const EFFECTS: u16 = 0x580F;
    pub const KINDS: u16 = 0x5812;
    pub const SUBKINDS: u16 = 0x5815;
    pub const COST: u16 = 0x5818;
    pub const NAME: u16 = 0x581A;
    /// The effect being edited (0..2).
    pub const SLOT: u16 = 0x585B;
    /// The effects' costs and lowest targets (FFFFh none).
    pub const COSTS: u16 = 0x58A9;
    pub const LOWEST: u16 = 0x5A84;
    /// The current effect's three cost factors (copied from 58B7 + the
    /// effect's entry) and its cost routine.
    pub const FACTORS: u16 = 0x58AF;
    pub const COST_FN: u16 = 0x58B5;
    /// The effects allowed with those chosen (25 bytes).
    pub const ALLOWED: u16 = 0x528F;
    /// The player's record, the player's level + 1.
    pub const PLAYER: u16 = 0xA89A;
    pub const LEVEL: u16 = 0xA898;
    /// The name's background (heap segment), the cursors' resource.
    pub const NAME_BACK: u16 = 0xA890;
    pub const CURSORS: u16 = 0xA89C;
    /// The form: its record (DATA), the fields' base, its picture's
    /// buffer and place and width.
    pub const FORM: u16 = 0xA888;
    pub const FIELDS: u16 = 0xA87C;
    pub const PICTURE: u16 = 0xA87E;
    pub const FORM_X: u16 = 0xA880;
    pub const FORM_Y: u16 = 0xA882;
    pub const FORM_W: u16 = 0xA884;
    /// The gold, the window colour.
    pub const GOLD: u16 = 0x05C2;
    pub const COLOUR: u16 = 0x9288;
    /// The player's spells (DATA): 32 records of 55h bytes.
    pub const SPELLS: u16 = 0x2A80;
}
use var::*;

/// The regions clicked on the screen (DGROUP 5711): x0, y0, x1, y1, the
/// routine (FFFFh: exit).
const REGIONS: u16 = 0x5711;
/// "spparts.rci", "spellmkr.txt".
const CURSOR_FILE: u16 = 0x5AE8;
const TEXT_FILE: u16 = 0x5866;
/// The messages.
const NEED_NAME: u16 = 0x5AAB;
const NEED_EFFECT: u16 = 0x5AC6;
const NO_GOLD: u16 = 0x5A4F;
const NO_ROOM: u16 = 0x5A8A;
const INSCRIBED: u16 = 0x5A1E;
/// The effects' names, the targets', the elements'.
const EFFECT_NAMES: u16 = 0x52C1;
const TARGET_NAMES: u16 = 0x5691;
const ELEMENT_NAMES: u16 = 0x566F;
/// Scratch buffers.
const BUF: u16 = 0x929E;
const TEXT: u16 = 0x1044;
const LIST_NAMES: u16 = 0x9686;
const LIST_INDEX: u16 = 0x9654;
const PICKS: u16 = 0x1238;
const TITLE: u16 = 0x9492;
const PICTURE_NAME: u16 = 0x9514;
const NUMBER: u16 = 0x942E;
/// The cursors' resources: their count and slots (6 bytes: the two
/// words given, the segment).
const RESOURCES: u16 = 0x91BC;

/// A division the CPU faults on (#DE).
fn div32(dx: u16, ax: u16, by: u16) -> Option<(u16, u16)> {
    if by == 0 {
        return crate::giveup::original("a divide by zero");
    }
    let n = (dx as u32) << 16 | ax as u32;
    let q = n / by as u32;
    if q > 0xFFFF {
        return crate::giveup::original("a divide overflows");
    }
    Some((q as u16, (n % by as u32) as u16))
}

/// 16 x 16 bits: (DX, AX).
fn mul(ax: u16, by: u16) -> (u16, u16) {
    let p = ax as u32 * by as u32;
    ((p >> 16) as u16, p as u16)
}

impl Machine {
    fn sg8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }

    fn sg16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    fn f16(&self, o: u16) -> u16 {
        self.mem.r16(DATA, o)
    }

    fn slot(&self) -> u16 {
        self.sg16(SLOT)
    }

    /// 1194:6064 with SI the player's record (1A4). None where the
    /// original faults or hangs.
    pub fn spellmaker(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks, si: u16) -> Option<()> {
        // Where the port gives up (a hook with no answer, a routine not
        // ported), the name's backing block is freed and the cursors
        // dropped, as the end does, so neither is lost. The original has
        // no such exit.
        let mut held = (None, None);
        let r = self.spellmaker_body(files, hooks, si, &mut held);
        if r.is_none() {
            if let Some(back) = held.0 {
                self.heap_free(back);
            }
            if let Some(cursors) = held.1 {
                self.resource_drop(cursors);
            }
        }
        r
    }

    fn spellmaker_body(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks, si: u16, held: &mut (Option<u16>, Option<u16>)) -> Option<()> {
        self.mem.w16(DS, PLAYER, si);
        let level = self.mem.r8(DS, si.wrapping_add(6)) as u16 + 1;
        self.mem.w16(DS, LEVEL, level);
        self.sm_clear();
        self.mem.w32(DS, LOWEST, 0);
        self.mem.w16(DS, LOWEST + 4, 0);
        let (seg, _) = self.heap_alloc_bytes(0x84 * 0xA)?;
        self.mem.w16(DS, NAME_BACK, seg);
        held.0 = Some(seg);
        self.mem.w8(DS, COLOUR, 0xFE);
        let r = self.resource_add(files, CURSOR_FILE, 3, 3)?;
        self.mem.w16(DS, CURSORS, r);
        held.1 = Some(r);
        self.sm_draw(files, hooks)?;
        loop {
            let (ax, bx) = self.sm_click(hooks, None);
            let mut found = None;
            for i in 0..7u16 {
                let r = REGIONS + 10 * i;
                if ax >= self.sg16(r) && bx >= self.sg16(r + 2) && ax <= self.sg16(r + 4) && bx < self.sg16(r + 6) {
                    found = Some(self.sg16(r + 8));
                    break;
                }
            }
            let Some(handler) = found else { continue };
            match handler {
                0xFFFF => break,
                0x65FD => self.sm_name(hooks),
                0x663D => self.sm_effects(files, hooks)?,
                0x6A1C => self.sm_target(files, hooks)?,
                0x6A45 => self.sm_element(files, hooks)?,
                0x6A6C => self.sm_new(files, hooks)?,
                0x6BD7 => self.sm_buy(files, hooks)?,
                _ => return crate::giveup::not_ported("1194:6064: a spellmaker button not in the table"),
            }
        }
        let back = self.sg16(NAME_BACK);
        self.heap_free(back);
        *held = (None, None);
        // 1194:55B6: the palette's buffer cleared (then faded to it, and
        // the screen cleared: the front end's).
        for i in 0..0x300u16 {
            self.mem.w8(DS, BUF + i, 0);
        }
        let r = self.sg16(CURSORS);
        self.resource_drop(r);
        Some(())
    }

    /// The spell's cost and its cost a level as the screen shows them
    /// (1194:6AA9; the target is raised to the effects' lowest).
    pub fn made_spell_cost(&mut self) -> (u16, u16) {
        self.sm_cost()
    }

    /// Each chosen effect's name and its description's lines, as 1194:6C97
    /// lists them (written through the screen's buffers, as there).
    pub fn made_spell_effects(&mut self, files: &mut Files) -> Option<Vec<EffectText>> {
        let saved = self.slot();
        let mut out = Vec::new();
        for i in 0..3u16 {
            self.mem.w16(DS, SLOT, i);
            let si = EFFECTS + i;
            let e = self.sg8(si) as u16;
            if e == 0xFF {
                continue;
            }
            self.sm_effect_name(si, BUF);
            let name = self.mem.cstr(DS, BUF).to_vec();
            let mut bx = self.sg16(0x5471u16.wrapping_add(e.wrapping_add(e)));
            if bx >= 0x32 {
                let k = self.sg8(si.wrapping_add(3)) as u16;
                bx = self.sg8(bx.wrapping_add(k)) as u16;
            }
            let n = self.sm_description(files, bx)?;
            let mut lines = Vec::new();
            let mut p = TEXT;
            for _ in 0..n.min(16) {
                let l = self.mem.cstr(DS, p).to_vec();
                p = p.wrapping_add(l.len() as u16 + 1);
                lines.push(l);
            }
            out.push((name, lines));
        }
        self.mem.w16(DS, SLOT, saved);
        Some(out)
    }

    /// 1194:5651: the click, and 5C8 bit 2 cleared.
    fn sm_click(&mut self, hooks: &mut dyn SpellmakerHooks, form: Option<u16>) -> (u16, u16) {
        let (x, y) = hooks.click(self, form);
        let f = self.sg16(0x05C8) & 0xFFFD;
        self.mem.w16(DS, 0x05C8, f);
        (x, y)
    }

    /// 1194:6A7A (7211): the spell emptied, no effects, no costs, every
    /// effect allowed but 7, 8 and 0Eh.
    fn sm_clear(&mut self) {
        for i in 0..0x55u16 {
            self.mem.w8(DS, SPELL + i, 0);
        }
        for i in 0..3 {
            self.mem.w8(DS, EFFECTS + i, 0xFF);
            self.mem.w16(DS, COSTS + 2 * i, 0);
        }
        self.sm_allow_all();
    }

    /// 1194:7211.
    fn sm_allow_all(&mut self) {
        for i in 0..0x19u16 {
            self.mem.w8(DS, ALLOWED + i, 1);
        }
        for i in [7u16, 8, 0xE] {
            self.mem.w8(DS, ALLOWED + i, 0);
        }
    }

    /// 1194:71F9: the effects the chosen ones allow (each chosen effect's
    /// row of 25 bytes at DATA 6430 ANDed in).
    fn sm_allowed(&mut self) {
        self.sm_allow_all();
        for i in 0..3 {
            let e = self.sg8(EFFECTS + i);
            if e == 0xFF {
                continue;
            }
            let row = 0x6430u16.wrapping_add(e as u16 * 0x19);
            for k in 0..0x19u16 {
                let v = self.sg8(ALLOWED + k) & self.mem.r8(DATA, row.wrapping_add(k));
                self.mem.w8(DS, ALLOWED + k, v);
            }
        }
    }

    /// 1194:5585: SI past `n` strings (each scanned at most 3000 bytes).
    fn skip_strings_at(&self, mut si: u16, n: u16) -> u16 {
        for _ in 0..n {
            let mut k = 0u16;
            loop {
                let c = self.sg8(si);
                si = si.wrapping_add(1);
                k += 1;
                if c == 0 || k == 0xBB8 {
                    break;
                }
            }
        }
        si
    }

    /// 0000:9CA4: the string at DS:si copied to DS:di with its NUL; DI
    /// after the NUL.
    fn sm_copy(&mut self, mut si: u16, mut di: u16) -> u16 {
        loop {
            let c = self.sg8(si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return di;
            }
        }
    }

    /// 1194:6D1F: the name of the effect whose byte is at `si` (580F + i)
    /// written at `di`: the effect's name, then its kind's, then for the
    /// kinds that have them the sub-kind's, separated by spaces.
    fn sm_effect_name(&mut self, si: u16, di: u16) -> u16 {
        let e = self.sg8(si) as u16;
        let s = self.skip_strings_at(EFFECT_NAMES, e);
        let mut di = self.sm_copy(s, di);
        let kind = self.sg8(si.wrapping_add(3)) as u16;
        let list = self.sg16(0x5426u16.wrapping_add(e.wrapping_add(e)));
        if list == 0 {
            return di;
        }
        let s = self.skip_strings_at(list, kind);
        self.mem.w8(DS, di.wrapping_sub(1), b' ');
        di = self.sm_copy(s, di);
        let sub = (self.sg8(si.wrapping_add(6)) & 0xF) as u16;
        let s = if list == 0x563A {
            if sub != 2 {
                return di;
            }
            0x55CD
        } else if list == 0x5652 {
            if kind as u8 == 0 { 0x55CD } else { 0x5561 }
        } else {
            return di;
        };
        let s = self.skip_strings_at(s, sub);
        self.mem.w8(DS, di.wrapping_sub(1), b' ');
        self.sm_copy(s, di)
    }

    /// 1194:6B9D: whether a chosen effect takes an element (5458); when
    /// one does, "None" (5) becomes "Magic" (4); when none does, the
    /// element is "None" and the carry is set (true).
    fn sm_no_element(&mut self) -> bool {
        for i in 0..3 {
            let e = self.sg8(EFFECTS + i);
            if e == 0xFF {
                continue;
            }
            if self.sg8(0x5458u16.wrapping_add(e as u16)) != 0 {
                if self.sg8(ELEMENT) == 5 {
                    self.mem.w8(DS, ELEMENT, 4);
                }
                return false;
            }
        }
        self.mem.w8(DS, ELEMENT, 5);
        true
    }

    /// 1194:6B14: the effects' highest lowest target (DX), or the carry
    /// (DX 0) when the last effect chosen has none (FFFFh).
    fn sm_lowest(&self) -> (u16, bool) {
        let (mut ax, mut dx) = (0u16, 0u16);
        for i in 0..3 {
            if self.sg8(EFFECTS + i) == 0xFF {
                continue;
            }
            let v = self.sg16(LOWEST + 2 * i);
            ax = if v == 0xFFFF { 0xFFFF } else { 0 };
            if dx < v {
                dx = v;
            }
        }
        if ax != 0 { (0, true) } else { (dx, false) }
    }

    /// 1194:6B44: the effects' steps from their lowest targets to `dx`.
    fn sm_steps(&self, dx: u16) -> u16 {
        let mut bx = 0u16;
        for i in 0..3 {
            if self.sg8(EFFECTS + i) == 0xFF {
                continue;
            }
            let mut v = self.sg16(LOWEST + 2 * i);
            if v == 0xFFFF {
                v = 0;
            }
            bx = bx.wrapping_add(dx.wrapping_sub(v));
        }
        bx
    }

    /// 1194:63BB: the player's class's factors by effect (25 bytes), for
    /// the three classes that have them.
    fn sm_class_factors(&self) -> Option<u16> {
        match self.sg8(0x01A9) {
            0x24 => Some(0x53DB),
            0xE5 => Some(0x540D),
            0x22 => Some(0x53F4),
            _ => None,
        }
    }

    /// 1194:6AD8: the effects' costs, each by the class's factor (halved
    /// through 32 bits, 16 kept).
    fn sm_total(&self) -> u16 {
        let Some(si) = self.sm_class_factors() else {
            return self.sg16(COSTS).wrapping_add(self.sg16(COSTS + 2)).wrapping_add(self.sg16(COSTS + 4));
        };
        let mut bp = 0u16;
        for i in 0..3 {
            let e = self.sg8(EFFECTS + i);
            if e == 0xFF {
                continue;
            }
            let f = self.sg8(si.wrapping_add(e as u16)) as u16;
            let (dx, ax) = mul(self.sg16(COSTS + 2 * i), f);
            bp = bp.wrapping_add((((dx as u32) << 16 | ax as u32) >> 1) as u16);
        }
        bp
    }

    /// 1194:6AA9: the spell's cost (AX) and its cost a level (BX); the
    /// target raised to the effects' lowest.
    fn sm_cost(&mut self) -> (u16, u16) {
        self.sm_no_element();
        let (dx, _) = self.sm_lowest();
        if dx as u8 > self.sg8(TARGET) {
            self.mem.w8(DS, TARGET, dx as u8);
        }
        let dx = self.sg8(TARGET) as u16;
        let bx = self.sm_steps(dx).wrapping_add(2);
        let total = self.sm_total();
        let (_, ax) = mul(total, bx);
        let ax = ax >> 1;
        // AX / the level + 1 (at least 1): never past 16 bits.
        let per = ax / self.sg16(LEVEL).max(1);
        (ax, per)
    }

    /// 1194:68D7: the screen (its numbers written as it draws them).
    fn sm_draw(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        self.sm_no_element();
        self.mem.w8(DS, COLOUR, 0xA0);
        self.sm_effects_list(files)?;
        // The gold (75B0) and the level (7052).
        let gold = self.mem.r32(DS, GOLD);
        self.mem.w32(DS, BUF, gold);
        self.dun_ultoa(gold, BUF);
        let level = self.sg16(LEVEL);
        self.sm_itoa(level, BUF);
        let (ax, bx) = if self.sg8(0x1001) == 0xFF || self.sg8(0x101D) == 0xFF {
            let ax = self.sg16(COST);
            (ax, ax / self.sg16(LEVEL).max(1))
        } else {
            self.sm_cost()
        };
        self.sm_itoa(ax.wrapping_add(ax), BUF);
        self.sm_itoa(bx, BUF);
        let f = self.sg16(0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        hooks.screen(self);
        Some(())
    }

    /// 0000:7052: `v` in decimal at DS:`di`, NUL-terminated (through
    /// 7028's buffer ending at A933).
    pub(crate) fn sm_itoa(&mut self, v: u16, di: u16) {
        let digits = self.utoa(v);
        for (i, &d) in digits.iter().enumerate() {
            self.mem.w8(DS, di.wrapping_add(i as u16), d);
        }
        self.mem.w8(DS, di.wrapping_add(digits.len() as u16), 0);
    }

    /// 1194:6C97: the effects listed with their descriptions (A874 the
    /// line; 585B each in turn, then put back).
    fn sm_effects_list(&mut self, files: &mut Files) -> Option<()> {
        self.mem.w16(DS, 0xA874, 0x52);
        let saved = self.slot();
        self.mem.w16(DS, SLOT, 0);
        for i in 0..3u16 {
            let si = EFFECTS + i;
            let e = self.sg8(si) as u16;
            if e != 0xFF {
                self.sm_effect_name(si, BUF);
                let y = self.sg16(0xA874).wrapping_add(0xA);
                self.mem.w16(DS, 0xA874, y);
                let mut bx = self.sg16(0x5471u16.wrapping_add(e.wrapping_add(e)));
                if bx >= 0x32 {
                    let k = self.sg8(si.wrapping_add(3)) as u16;
                    bx = self.sg8(bx.wrapping_add(k)) as u16;
                }
                let lines = self.sm_description(files, bx)?;
                // A line each (0: 65536 of them), the line moving 0Ah.
                let n = if lines == 0 { 0x10000u32 } else { lines as u32 };
                let y = self.sg16(0xA874).wrapping_add((n * 0xA) as u16);
                self.mem.w16(DS, 0xA874, y);
            }
            let s = self.slot().wrapping_add(1);
            self.mem.w16(DS, SLOT, s);
        }
        self.mem.w16(DS, SLOT, saved);
        Some(())
    }

    /// 1194:6D87: the description `n` of SPELLMKR.TXT made at 1044 (its
    /// lines NUL-separated); returns their count (A872). None where the
    /// file is missing or the entry never comes.
    fn sm_description(&mut self, files: &mut Files, n: u16) -> Option<u16> {
        self.mem.w16(DS, FIELDS, 0);
        self.mem.w16(DS, 0xA87A, 0x122E);
        self.mem.w16(DS, 0xA872, n);
        self.file_open(files, TEXT_FILE)?;
        let mut reads = 0u32;
        let mut read = |m: &mut Machine, files: &mut Files, at: u16, k: u16| -> Option<()> {
            reads += 1;
            if reads > 0x40000 {
                return crate::giveup::original("1194:6D87: the description never ends: the original reads on for ever");
            }
            m.file_read(files, DS, at, k);
            Some(())
        };
        loop {
            read(self, files, BUF, 1)?;
            if self.sg8(BUF) != b'#' {
                continue;
            }
            read(self, files, BUF, 2)?;
            self.mem.w8(DS, BUF + 2, 0);
            if self.info_atoi(DS, BUF) == self.sg16(0xA872) {
                break;
            }
        }
        let mut si = BUF;
        read(self, files, si, 2)?;
        loop {
            read(self, files, si, 1)?;
            match self.sg8(si) {
                0x0A => {
                    self.mem.w8(DS, si, b'$');
                    si = si.wrapping_add(1);
                }
                0x0D => {}
                b'#' => break,
                b'%' => {
                    si = si.wrapping_add(1);
                    read(self, files, si, 1)?;
                    let c = self.sg8(si);
                    if c < b'a' {
                        if c != b'%' {
                            self.mem.w8(DS, si, b'd');
                            let ax = (c.wrapping_sub(0x30) as u16).wrapping_mul(6);
                            let at = SPELL.wrapping_add(ax).wrapping_add(self.slot()).wrapping_add(self.slot());
                            let v = self.sg16(at);
                            self.sm_push(&[v]);
                        }
                        si = si.wrapping_add(1);
                        continue;
                    }
                    match c {
                        b'a' => {
                            let v = self.sg16(0x5873);
                            self.sm_push(&[v]);
                            self.mem.w8(DS, si, b'd');
                        }
                        b'b' => {
                            if self.sg16(FLAGS) & 0x100 == 0 {
                                // The 'b' is read over by what follows.
                                continue;
                            }
                            self.sm_push(&[0x58A5, DS]);
                            self.mem.w8(DS, si, b's');
                        }
                        b'c' => {
                            let bx = EFFECTS.wrapping_add(self.slot());
                            let e = self.sg8(bx) as u16;
                            let list = self.sg16(0x5426u16.wrapping_add(e.wrapping_add(e)));
                            let (s, cx) = if (list == 0x563A && self.sg8(bx.wrapping_add(3)) == 2) || list == 0x5652 {
                                let v = self.sg8(bx.wrapping_add(6)) as u16;
                                let s = match (v >> 4) & 0xF {
                                    2 => 0x563A,
                                    3 => 0x5561,
                                    _ => 0x55CD,
                                };
                                (s, v & 0xF)
                            } else {
                                (list, self.sg8(bx.wrapping_add(3)) as u16)
                            };
                            let s = self.skip_strings_at(s, cx);
                            self.sm_push(&[s, DS]);
                            self.mem.w8(DS, si, b's');
                        }
                        b'e' => {
                            let s = if self.sg16(FLAGS) & 0x40 != 0 { 0x589F } else { 0x5898 };
                            self.sm_push(&[s, DS]);
                            self.mem.w8(DS, si, b's');
                        }
                        _ => {
                            let s = if self.sg16(FLAGS) & 0x80 != 0 { 0x5888 } else { 0x5875 };
                            self.sm_push(&[s, DS]);
                            self.mem.w8(DS, si, b's');
                        }
                    }
                    si = si.wrapping_add(1);
                }
                _ => si = si.wrapping_add(1),
            }
        }
        self.mem.w8(DS, si, 0);
        self.file_close(files);
        // The format's arguments, the words pushed (from 122E).
        let mut words = Vec::new();
        let mut p = 0x122Eu16;
        while p != self.sg16(0xA87A) {
            words.push(self.sg16(p));
            p = p.wrapping_add(2);
        }
        let mut args = Vec::new();
        let mut w = words.iter().copied();
        let mut f = BUF;
        loop {
            let c = self.sg8(f);
            f = f.wrapping_add(1);
            if c == 0 {
                break;
            }
            if c != b'%' {
                continue;
            }
            let spec = self.sg8(f);
            f = f.wrapping_add(1);
            match spec {
                b'd' => args.push(Arg::Int(w.next()?)),
                b's' => {
                    let off = w.next()?;
                    let seg = w.next()?;
                    args.push(Arg::Str(seg, off));
                }
                0 => break,
                _ => {}
            }
        }
        self.sprintf((DS, TEXT), (DS, BUF), &args);
        // A876 keeps SP across the sprintf (1194:6DD2, 6E1F); its value is
        // the original's stack's, a stand-in here. (Later scans can read
        // it: a list of targets past the strings, 1194:6A1C.)
        self.mem.w16(DS, 0xA876, 0xFFD0);
        // The lines: each '$' made a NUL (9CB0, then REPNE SCASB).
        let len = self.mem.cstr(DS, TEXT).len() as u16;
        self.mem.w16(DS, 0xA872, 0);
        if len == 0 {
            self.mem.w16(DS, 0xA872, 1);
            self.mem.w8(DS, TEXT.wrapping_sub(1), 0);
            return Some(1);
        }
        let (mut di, mut cx) = (TEXT, len);
        while cx != 0 {
            let mut hit = false;
            while cx != 0 {
                let c = self.sg8(di);
                di = di.wrapping_add(1);
                cx -= 1;
                if c == b'$' {
                    hit = true;
                    break;
                }
            }
            if !hit {
                break;
            }
            let k = self.sg16(0xA872).wrapping_add(1);
            self.mem.w16(DS, 0xA872, k);
            self.mem.w8(DS, di.wrapping_sub(1), 0);
        }
        Some(self.sg16(0xA872))
    }

    /// Words stored at [A87A] (advanced), A87C counting them.
    fn sm_push(&mut self, words: &[u16]) {
        let mut di = self.sg16(0xA87A);
        for &v in words {
            self.mem.w16(DS, di, v);
            di = di.wrapping_add(2);
        }
        self.mem.w16(DS, 0xA87A, di);
        let n = self.sg16(FIELDS).wrapping_add(words.len() as u16);
        self.mem.w16(DS, FIELDS, n);
    }

    /// 1194:65FD: the name typed (its background put back first).
    fn sm_name(&mut self, hooks: &mut dyn SpellmakerHooks) {
        self.mem.w8(DS, COLOUR, 0xA0);
        hooks.type_name(self, NAME);
    }

    /// 1194:663D: the effects' menu.
    fn sm_effects(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        let all = self.mem.r32(DS, EFFECTS) & 0xFF_FFFF;
        if all == 0xFF_FFFF {
            self.mem.w16(DS, SLOT, 0);
            return self.sm_choose(files, hooks);
        }
        let free = (0..3).any(|i| (all >> (8 * i)) as u8 == 0xFF);
        let (a, b) = if free { (0xD6, 0xD8) } else { (0, 0) };
        self.mem.w8(DS, 0x59BE, a);
        self.mem.w8(DS, 0x59C1, b);
        loop {
            let pick = hooks.menu(self, 0x59A1, 0x59B1, 0x59B5);
            if (pick as i16) < 0 {
                return Some(());
            }
            if pick == 0 {
                // Add: the first empty slot, else the menu again.
                let Some(i) = (0..3u16).find(|&i| self.sg8(EFFECTS + i) == 0xFF) else { continue };
                self.mem.w16(DS, SLOT, i);
                for f in [0u16, 6, 0xC, 0x12, 0x18, 0x1E] {
                    self.mem.w16(DS, SPELL + f + 2 * i, 0);
                }
                return self.sm_choose(files, hooks);
            }
            // Modify or delete: the effects named in a list (one is
            // taken without asking).
            self.mem.w16(DS, SLOT, 0);
            let mut di = TEXT;
            let mut bp = 0u16;
            let mut bx = PICKS;
            for k in 0..3u16 {
                let si = EFFECTS + k;
                if self.sg8(si) != 0xFF {
                    bp += 1;
                    self.mem.w8(DS, bx, k as u8);
                    bx += 1;
                    self.sm_effect_name(si, di);
                    // Past the NUL (REPNE SCASB from DI, at most 3E8h).
                    let mut n = 0;
                    loop {
                        let c = self.sg8(di);
                        di = di.wrapping_add(1);
                        n += 1;
                        if c == 0 || n == 0x3E8 {
                            break;
                        }
                    }
                }
                let s = self.slot().wrapping_add(1);
                self.mem.w16(DS, SLOT, s);
            }
            self.mem.w8(DS, di, 0);
            let si = if bp == 1 {
                let si = self.sg8(PICKS) as u16;
                if pick != 1 {
                    self.mem.w8(DS, TARGET, 0);
                }
                si
            } else {
                // AX 3h: the list's kind (1194:5734).
                self.list_kind = 0x3;
                let ax = hooks.list(self, TEXT, bp);
                if (ax as i16) < 0 {
                    return Some(());
                }
                self.sg8(PICKS.wrapping_add(ax)) as u16
            };
            if pick == 1 {
                self.mem.w16(DS, SLOT, si);
                return self.sm_edit(files, hooks);
            }
            // Delete.
            let di = si.wrapping_add(si);
            self.mem.w16(DS, COSTS.wrapping_add(di), 0);
            for f in [EFFECTS, KINDS, SUBKINDS] {
                self.mem.w8(DS, f.wrapping_add(si), 0xFF);
            }
            for f in [0u16, 6, 0xC, 0x12, 0x1E, 0x18] {
                self.mem.w16(DS, SPELL.wrapping_add(f).wrapping_add(di), 0);
            }
            return self.sm_draw(files, hooks);
        }
    }

    /// 1194:6798: the slot's effect chosen (655E), then edited.
    fn sm_choose(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        self.sm_pick_effect(hooks);
        self.sm_edit(files, hooks)
    }

    /// 1194:655E: the slot's effect, its kind and sub-kind picked from
    /// lists of those allowed; nothing picked empties the slot.
    fn sm_pick_effect(&mut self, hooks: &mut dyn SpellmakerHooks) {
        self.sm_allowed();
        // 1194:7251: the allowed effects' names listed.
        let (mut si, mut di, mut dx) = (EFFECT_NAMES, LIST_NAMES, LIST_INDEX);
        for k in 0..0x19u16 {
            let on = self.sg8(ALLOWED + k) != 0;
            if on {
                self.mem.w8(DS, dx, k as u8);
                dx += 1;
            }
            loop {
                let c = self.sg8(si);
                si = si.wrapping_add(1);
                if on {
                    self.mem.w8(DS, di, c);
                    di = di.wrapping_add(1);
                }
                if c == 0 {
                    break;
                }
            }
        }
        self.mem.w8(DS, di, 0);
        // AX 3h: the list's kind (1194:5734).
        self.list_kind = 0x3;
        let mut ax = hooks.list(self, LIST_NAMES, dx - LIST_INDEX);
        if (ax as i16) >= 0 {
            ax = self.sg8(LIST_INDEX.wrapping_add(ax)) as u16;
        }
        let slot = self.slot();
        let empty = |m: &mut Machine| {
            for f in [EFFECTS, KINDS, SUBKINDS] {
                m.mem.w8(DS, f.wrapping_add(slot), 0xFF);
            }
        };
        self.mem.w8(DS, EFFECTS.wrapping_add(slot), ax as u8);
        if (ax as i16) < 0 {
            empty(self);
            return;
        }
        let cx = self.sg16(0x56F5u16.wrapping_add(ax.wrapping_add(ax)));
        let list = self.sg16(0x5426u16.wrapping_add(ax.wrapping_add(ax)));
        if list == 0 {
            return;
        }
        // AX 3h: the list's kind (1194:5734).
        self.list_kind = 0x3;
        let ax = hooks.list(self, list, cx);
        if (ax as i16) < 0 {
            empty(self);
            return;
        }
        self.mem.w8(DS, KINDS.wrapping_add(slot), ax as u8);
        let (s, n) = if list == 0x563A {
            if ax as u8 != 2 {
                return;
            }
            (0x55CD, 8)
        } else if list == 0x5652 {
            if ax as u8 != 0 { (0x5561, 3) } else { (0x55CD, 8) }
        } else {
            return;
        };
        // AX 3h: the list's kind (1194:5734).
        self.list_kind = 0x3;
        let ax = hooks.list(self, s, n);
        let v = (ax as u8) | ((n as u8) << 4);
        self.mem.w8(DS, SUBKINDS.wrapping_add(slot), v);
    }

    /// 1194:679E: the slot's effect set up and its form shown.
    fn sm_edit(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        let slot = self.slot();
        let at = EFFECTS.wrapping_add(slot);
        let e = self.sg8(at) as u16;
        if e == 0xFF {
            return Some(());
        }
        self.sm_allowed();
        let bx = e.wrapping_add(e);
        // The cost factors: by the effect, or by its kind.
        let mut si = self.sg16(0x54D5u16.wrapping_add(bx));
        if si >= 0x5543 {
            let k = self.sg8(at.wrapping_add(3)) as u16;
            si = self.sg16(si.wrapping_add(k.wrapping_add(k)));
        }
        let si = si.wrapping_add(0x58B7);
        for i in 0..3u16 {
            let v = self.sg16(si.wrapping_add(2 * i));
            self.mem.w16(DS, FACTORS + 2 * i, v);
        }
        // The lowest target: fixed (0..5, FFFFh), or by a byte of the
        // spell (its kind) in a table.
        let mut lo = self.sg16(0x54A3u16.wrapping_add(bx));
        if lo > 5 && lo != 0xFFFF {
            let k = self.sg8(0x57E9u16.wrapping_add(slot)) as u16;
            lo = self.sg8(k.wrapping_add(lo)) as i8 as i16 as u16;
        }
        self.mem.w16(DS, LOWEST.wrapping_add(slot.wrapping_add(slot)), lo);
        self.sm_effect_name(at, TITLE);
        // The form: by the effect, or by its kind.
        let mut al = self.sg8(0x5507u16.wrapping_add(e));
        if matches!(al, 0 | 0xFF | 0xFE) {
            let k = self.sg8(at.wrapping_add(3)) as u16;
            let t: u16 = if al == 0 {
                0x55BE
            } else if e == 0 {
                0x5543
            } else {
                0x55A3
            };
            al = self.sg8(t.wrapping_add(k));
        }
        let fmt = if al & 0x80 != 0 { 0x5846 } else { 0x583B };
        self.sprintf((DS, PICTURE_NAME), (DS, fmt), &[Arg::Int((al & 0x7F) as u16)]);
        let mut cx = al as u16;
        if al & 0x80 != 0 {
            cx &= 0x7F;
            cx = match cx {
                4 => 0x10,
                6 => 0x11,
                c => c,
            };
        }
        let i = cx.wrapping_sub(1).wrapping_add(cx.wrapping_sub(1));
        let f = self.sg16(0x57C4u16.wrapping_add(i));
        self.mem.w16(DS, COST_FN, f);
        let form = self.sg16(0x57A2u16.wrapping_add(i));
        self.mem.w16(DATA, form, TITLE);
        self.mem.w16(DATA, form.wrapping_add(2), PICTURE_NAME);
        self.sm_form(files, hooks, form)?;
        self.sm_draw(files, hooks)
    }

    /// 1194:6105: an effect's form, its record at DATA:`form`.
    fn sm_form(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks, form: u16) -> Option<()> {
        self.mem.w8(DS, COLOUR, 0xFF);
        self.mem.w16(DS, FORM, form);
        self.mem.w16(DS, FIELDS, SPELL);
        self.sm_keep_screen();
        let name = self.f16(form.wrapping_add(2));
        let seg = self.load_alloc(files, name);
        self.mem.w16(DS, PICTURE, seg);
        let w = self.mem.r16(seg, 4);
        let h = self.mem.r16(seg, 6);
        let x = 0xA0u16.wrapping_sub(w >> 1);
        let y = 0x64u16.wrapping_sub(h >> 1);
        self.mem.w16(seg, 0, x);
        self.mem.w16(seg, 2, y);
        self.mem.w16(DS, FORM_W, w);
        self.mem.w16(DS, FORM_Y, y);
        self.mem.w16(DS, FORM_X, x);
        // The picture freed, a buffer of its size kept for what it covers.
        let (_, size) = mul(self.mem.r16(seg, 4), self.mem.r16(seg, 6));
        self.heap_free(seg);
        let (buf, _) = self.heap_alloc_bytes(size)?;
        self.mem.w16(DS, PICTURE, buf);
        // Each arrow's field within its range.
        let count = self.f16(form.wrapping_add(4));
        let mut si = form.wrapping_add(0x12);
        for _ in 0..(if count == 0 { 0x10000 } else { count as u32 }) {
            let lo = self.f16(si.wrapping_add(8));
            if (lo as i16) >= 0 {
                let di = self.sm_field(si);
                let v = self.sg16(di) as i16;
                let hi = self.f16(si.wrapping_add(0xA)) as i16;
                if v < lo as i16 {
                    self.mem.w16(DS, di, lo);
                } else if v > hi {
                    self.mem.w16(DS, di, hi as u16);
                }
            }
            si = si.wrapping_add(0x12);
        }
        self.mem.w8(DS, COLOUR, 0xFF);
        let mut si = form.wrapping_add(0x12);
        for _ in 0..(if count == 0 { 0x10000 } else { count as u32 }) {
            self.sm_control(si);
            si = si.wrapping_add(0x12);
        }
        self.sm_cost_fn()?;
        self.sm_show_cost();
        let f = self.sg16(0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        hooks.form(self, form);
        loop {
            let (ax, bx) = self.sm_click(hooks, Some(form));
            let ax = ax.wrapping_sub(self.sg16(FORM_X));
            let bx = bx.wrapping_sub(self.sg16(FORM_Y));
            if bx >= self.f16(form.wrapping_add(0xC)) {
                if bx > self.f16(form.wrapping_add(0x10)) || ax < self.f16(form.wrapping_add(0xA)) || ax > self.f16(form.wrapping_add(0xE)) {
                    continue;
                }
                break;
            }
            let mut hit = None;
            let mut si = form.wrapping_add(0x12);
            for _ in 0..(if count == 0 { 0x10000 } else { count as u32 }) {
                let x = self.f16(si);
                let y = self.f16(si.wrapping_add(2));
                if ax >= x && bx >= y && ax <= x.wrapping_add(9) && bx <= y.wrapping_add(0xD) {
                    hit = Some(si);
                    break;
                }
                si = si.wrapping_add(0x12);
            }
            let Some(si) = hit else { continue };
            let mut dx = self.f16(si.wrapping_add(0x10));
            if bx.wrapping_sub(self.f16(si.wrapping_add(2))) > 5 {
                dx = dx.wrapping_neg();
            }
            let di = self.sm_field(si);
            let mut ax = self.sg16(di);
            let link = self.f16(si.wrapping_add(0xE));
            if (self.f16(si.wrapping_add(8)) as i16) < 0 {
                let bit = self.f16(si.wrapping_add(0xA));
                // A link of 0 or more clears the bit, a negative one sets
                // it (1194:62CD).
                ax = if (link as i16) >= 0 { ax & !bit } else { ax | bit };
            } else {
                ax = ax.wrapping_add(dx);
                if (ax as i16) < self.f16(si.wrapping_add(8)) as i16 || (ax as i16) > self.f16(si.wrapping_add(0xA)) as i16 {
                    continue;
                }
                if link != 0 {
                    let other = si.wrapping_add(link.wrapping_mul(0x12));
                    let bx = self.sm_field(other);
                    let v = self.sg16(bx) as i16;
                    if (link as i16) >= 0 {
                        if ax as i16 >= v {
                            self.mem.w16(DS, bx, ax);
                        }
                    } else if (ax as i16) < v {
                        self.mem.w16(DS, bx, ax);
                    }
                }
            }
            self.mem.w16(DS, di, ax);
            self.sm_control(si);
            self.sm_cost_fn()?;
            self.sm_show_cost();
            if link != 0 {
                self.sm_control(si.wrapping_add(link.wrapping_mul(0x12)));
            }
        }
        let buf = self.sg16(PICTURE);
        self.heap_free(buf);
        self.sm_restore_screen();
        let f = self.sg16(0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, f);
        Some(())
    }

    /// The field a control steps: 57E6 + 2 x 585B + its offset (+0Ch).
    fn sm_field(&self, si: u16) -> u16 {
        let s = self.slot();
        s.wrapping_add(s).wrapping_add(self.f16(si.wrapping_add(0xC))).wrapping_add(self.sg16(FIELDS))
    }

    /// 1194:6B6A: the screen (A8FE) kept in the frame (9030).
    fn sm_keep_screen(&mut self) {
        let (off, seg) = (self.sg16(0xA8FE), self.sg16(0xA900));
        let frame = self.sg16(0x9030);
        for i in 0..64000u16 {
            let b = self.mem.r8(seg, off.wrapping_add(i));
            self.mem.w8(frame, i, b);
        }
    }

    /// 1194:6B88: the screen put back from the frame.
    fn sm_restore_screen(&mut self) {
        let (off, seg) = (self.sg16(0xA8FE), self.sg16(0xA900));
        let frame = self.sg16(0x9030);
        for i in 0..64000u16 {
            let b = self.mem.r8(frame, i);
            self.mem.w8(seg, off.wrapping_add(i), b);
        }
    }

    /// 1194:6410: a control shown: its value written at 942E (a number),
    /// or for a check box (shown when its bit agrees with its link's
    /// sign) the recolouring table made at 929E.
    fn sm_control(&mut self, si: u16) {
        let di = self.sm_field(si);
        let ax = self.sg16(di);
        if (self.f16(si.wrapping_add(8)) as i16) >= 0 {
            self.sm_itoa(ax, NUMBER);
            return;
        }
        let set = self.f16(si.wrapping_add(0xA)) & ax != 0;
        let neg = (self.f16(si.wrapping_add(0xE)) as i16) < 0;
        if set != neg {
            return;
        }
        if self.f16(si.wrapping_add(8)) == 0xFFFF {
            return;
        }
        for i in 0..0x100u16 {
            self.mem.w8(DS, BUF + i, i as u8);
        }
        self.mem.w8(DS, 0x939D, 0xD5);
    }

    /// The cost routine of the form (at 58B5; 57C4's).
    fn sm_cost_fn(&mut self) -> Option<()> {
        let s = self.slot();
        let di = SPELL.wrapping_add(s).wrapping_add(s);
        let w = |m: &Machine, o: u16| m.sg16(di.wrapping_add(o));
        let (f0, f1, f2) = (self.sg16(FACTORS), self.sg16(FACTORS + 2), self.sg16(FACTORS + 4));
        let cost = COSTS.wrapping_add(s).wrapping_add(s);
        let set = |m: &mut Machine, v: u16| m.mem.w16(DS, cost, v);
        let get = |m: &Machine| m.sg16(cost);
        // 1194:70DC: (DX, AX) = (the high word of the second product, the
        // sum), the cost set to it.
        let p70dc = |m: &mut Machine| -> Option<(u16, u16)> {
            let (_, bp) = mul(w(m, 0), f0);
            let (q, _) = div32(0, w(m, 6), w(m, 0x18))?;
            let (dx, ax) = mul(q, f1);
            let ax = ax.wrapping_add(bp);
            set(m, ax);
            Some((dx, ax))
        };
        // 1194:70F6: AX.
        let p70f6 = |m: &Machine| -> Option<u16> {
            let dx = w(m, 0).wrapping_add(w(m, 6)) >> 1;
            let ax = w(m, 0xC).wrapping_add(w(m, 0x12)) >> 1;
            let (q, _) = div32(0, ax, w(m, 0x18))?;
            let (_, bx) = mul(q, f1);
            let (_, ax) = mul(f0, dx);
            Some(ax.wrapping_add(bx))
        };
        // The flags word read at DI + 27h (580D + 2 x the slot).
        let flags = w(self, 0x27);
        match self.sg16(COST_FN) {
            0x6F9C => {
                let ax = p70f6(self)?;
                set(self, ax);
            }
            0x6FA3 => {
                let ax = p70f6(self)?;
                set(self, ax);
                let (_, ax) = mul(w(self, 0x1E), f2);
                let v = get(self).wrapping_add(ax);
                set(self, v);
            }
            0x6FB4 | 0x704D => {
                let (_, ax) = p70dc(self)?;
                set(self, ax);
            }
            0x6FBE => {
                p70dc(self)?;
                let (q, _) = div32(0, w(self, 0x1E), 6)?;
                let (_, ax) = mul(q, f2);
                let ax = ax.wrapping_add(get(self));
                let (dx, ax) = mul(ax, w(self, 0xC));
                let (q, _) = div32(dx, ax, w(self, 0x12))?;
                set(self, q);
            }
            0x6FDF => {
                let (_, ax) = p70dc(self)?;
                let (dx, ax) = mul(ax, w(self, 0x1E));
                let (q, _) = div32(dx, ax, w(self, 0xC))?;
                set(self, q);
            }
            0x6FEF => {
                // 1194:70BA (70C7): the first part, then the target's.
                let (_, ax) = mul(w(self, 0), f0);
                set(self, ax);
                // DIV CL: AL the quotient, AH the remainder.
                let v = w(self, 0x1E);
                let q = v / 5;
                if q > 0xFF {
                    return crate::giveup::original("1194:70BA: the cost's divide overflows");
                }
                let ax = (v % 5) << 8 | q;
                let (_, ax) = mul(ax, f1);
                let ax = ax.wrapping_add(get(self));
                let (dx, ax) = mul(ax, w(self, 0x18));
                let (q, _) = div32(dx, ax, w(self, 6))?;
                set(self, q);
            }
            0x6FF9 | 0x7065 => {
                let (_, ax) = mul(w(self, 0), f0);
                set(self, ax);
            }
            0x7006 => {
                let (_, ax) = mul(w(self, 0), f0);
                set(self, ax);
                let (_, ax) = mul(w(self, 0x1E), f1);
                let mut ax = ax.wrapping_add(get(self));
                if flags & 0x80 != 0 {
                    ax = ax.wrapping_add(ax);
                }
                set(self, ax);
            }
            0x7029 => {
                let (_, ax) = p70dc(self)?;
                let v = get(self).wrapping_add(ax);
                set(self, v);
            }
            0x7033 => {
                let (_, mut ax) = p70dc(self)?;
                if flags & 0x100 == 0 {
                    ax <<= 2;
                }
                let (dx, ax) = mul(ax, w(self, 0x1E));
                let (q, _) = div32(dx, ax, w(self, 0xC))?;
                set(self, q);
            }
            0x7054 => {
                p70dc(self)?;
                let (_, ax) = mul(w(self, 0x1E), f2);
                let v = get(self).wrapping_add(ax);
                set(self, v);
            }
            0x7072 => {
                p70dc(self)?;
                let (q, _) = div32(0, w(self, 0x1E), w(self, 0xC))?;
                let (_, ax) = mul(q, f2);
                let v = get(self).wrapping_add(ax);
                set(self, v);
            }
            0x7088 => {
                let (_, ax) = mul(w(self, 0), f0);
                set(self, ax);
                let (q, _) = div32(0, 0x3Cu16.wrapping_sub(w(self, 6)), 5)?;
                let (_, ax) = mul(q, f1);
                let v = get(self).wrapping_add(ax);
                set(self, v);
                let (q, _) = div32(0, w(self, 0x18), 0x1E)?;
                let (_, ax) = mul(q, f2);
                let v = get(self).wrapping_add(ax);
                set(self, v);
            }
            _ => return crate::giveup::not_ported("1194:70BA: a cost routine not in the table"),
        }
        Some(())
    }

    /// 1194:636B: the effect's cost as the class pays it, written at 929E.
    fn sm_show_cost(&mut self) {
        self.mem.w8(DS, COLOUR, 0xFF);
        let s = self.slot();
        let mut ax = self.sg16(COSTS.wrapping_add(s).wrapping_add(s));
        ax = ax.wrapping_add(ax);
        if let Some(t) = self.sm_class_factors() {
            let e = self.sg8(SPELL.wrapping_add(s).wrapping_add(0x29)) as u16;
            let f = self.sg8(t.wrapping_add(e)) as u16;
            let (dx, lo) = mul(ax, f);
            ax = (((dx as u32) << 16 | lo as u32) >> 1) as u16;
        }
        self.sm_itoa(ax, BUF);
    }

    /// 1194:6A1C: the target picked from those the effects allow.
    fn sm_target(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        let (dx, none) = self.sm_lowest();
        if none {
            return Some(());
        }
        let cx = 5u16.wrapping_sub(dx);
        let si = self.skip_strings_at(TARGET_NAMES, dx);
        // AX 3h: the list's kind (1194:5734).
        self.list_kind = 0x3;
        let ax = hooks.list(self, si, cx);
        if (ax as i16) < 0 {
            return Some(());
        }
        self.mem.w8(DS, TARGET, (ax as u8).wrapping_add(dx as u8));
        self.sm_draw(files, hooks)
    }

    /// 1194:6A45: the element picked (without "None").
    fn sm_element(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        if self.sm_no_element() {
            return Some(());
        }
        self.mem.w8(DS, 0x568C, 0);
        // AX 3h: the list's kind (1194:5734).
        self.list_kind = 0x3;
        let ax = hooks.list(self, ELEMENT_NAMES, 5);
        self.mem.w8(DS, 0x568C, b'N');
        if (ax as i16) < 0 {
            return Some(());
        }
        self.mem.w8(DS, ELEMENT, ax as u8);
        self.sm_draw(files, hooks)
    }

    /// 1194:6A6C: "Are you sure ?" (48E6), then a new spell.
    fn sm_new(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        if hooks.menu(self, 0x4B50, 0x4B60, 0x4B63) != 0 {
            return Some(());
        }
        self.sm_clear();
        self.sm_draw(files, hooks)
    }

    /// 1194:6BD7: the spell bought.
    fn sm_buy(&mut self, files: &mut Files, hooks: &mut dyn SpellmakerHooks) -> Option<()> {
        let say = |m: &mut Machine, hooks: &mut dyn SpellmakerHooks, si: u16| {
            m.mem.w8(DS, COLOUR, 0xD6);
            hooks.message_box(m, si);
        };
        if self.sg8(NAME) == 0 {
            say(self, hooks, NEED_NAME);
            return Some(());
        }
        if self.mem.r32(DS, EFFECTS) & 0xFF_FFFF == 0xFF_FFFF {
            say(self, hooks, NEED_EFFECT);
            return Some(());
        }
        let (cost, _) = self.sm_cost();
        self.mem.w16(DS, COST, cost);
        let (cost, _) = self.sm_cost();
        let price = cost.wrapping_add(cost) as u32;
        if self.mem.r32(DS, GOLD) < price {
            say(self, hooks, NO_GOLD);
            return Some(());
        }
        let Some(slot) = (0..0x20u16).find(|&k| self.mem.r8(DATA, SPELLS + 0x55 * k + 0x34) == 0) else {
            say(self, hooks, NO_ROOM);
            self.sm_clear();
            return self.sm_draw(files, hooks);
        };
        let at = SPELLS + 0x55 * slot;
        for i in 0..0x55u16 {
            let b = self.sg8(SPELL + i);
            self.mem.w8(DATA, at + i, b);
        }
        let p = self.sg16(PLAYER);
        let n = self.mem.r8(DS, p.wrapping_add(0x366));
        self.mem.w8(DS, p.wrapping_add(0x367).wrapping_add(n as u16), slot as u8 | 0x80);
        self.mem.w8(DS, p.wrapping_add(0x366), n.wrapping_add(1));
        let gold = self.mem.r32(DS, GOLD).wrapping_sub(price);
        self.mem.w32(DS, GOLD, gold);
        say(self, hooks, INSCRIBED);
        // 29CF:06CB: the spells' file named by the slot (1A6C's digits at
        // 7F1E) and written.
        let slot_no = self.sg8(0x1A6C);
        self.mem.w8(DS, 0x7F1E, b'0' + slot_no / 10);
        self.mem.w8(DS, 0x7F1F, b'0' + slot_no % 10);
        hooks.spells_write(self, files, 0x7F17);
        self.sm_clear();
        self.sm_draw(files, hooks)
    }

    /// 0000:756C: a resource registered (91BE: the first free of 6-byte
    /// slots, AX and BX kept, its file loaded on the heap, 742C with DI
    /// FFFFh); returns its slot's offset (0 with the table full).
    pub fn resource_add(&mut self, files: &mut Files, si: u16, ax: u16, bx: u16) -> Option<u16> {
        if self.sg16(RESOURCES) == 8 {
            return Some(0);
        }
        let mut di = RESOURCES + 2;
        let mut n = 0u32;
        while self.sg16(di) != 0 {
            di = di.wrapping_add(6);
            n += 1;
            if n > 0x2AAB {
                return crate::giveup::original("0000:756C: the resource list never ends: the original reads on for ever");
            }
        }
        self.mem.w16(DS, di, ax);
        self.mem.w16(DS, di.wrapping_add(2), bx);
        let seg = self.load_alloc(files, si);
        self.mem.w16(DS, di.wrapping_add(4), seg);
        let c = self.sg16(RESOURCES).wrapping_add(1);
        self.mem.w16(DS, RESOURCES, c);
        Some(di)
    }

    /// 0000:9326: a resource dropped (its slot freed, its segment too).
    pub(crate) fn resource_drop(&mut self, bx: u16) {
        self.mem.w16(DS, bx, 0);
        let seg = self.sg16(bx.wrapping_add(4));
        self.heap_free(seg);
        let c = self.sg16(RESOURCES).wrapping_sub(1);
        self.mem.w16(DS, RESOURCES, c);
    }

    /// 0000:742C with DI FFFFh: the file named at DS:`name` loaded on the
    /// heap (0000:51EC); returns its segment (DX), 0 where it fails.
    pub(crate) fn load_alloc(&mut self, files: &mut Files, name: u16) -> u16 {
        if self.file_open_handle(files, name).is_none() {
            return 0;
        }
        let size = self.file_seek(files, 2, 0);
        if size > 0xFFFF {
            self.file_close(files);
            return (size >> 16) as u16;
        }
        self.mem.w16(DS, crate::bsa::var::OPEN_SIZE, size as u16);
        self.file_seek(files, 0, 0);
        let Some((seg, _)) = self.heap_alloc_bytes(size as u16) else {
            self.file_close(files);
            return 0;
        };
        let n = self.file_read(files, seg, 0, size as u16);
        self.file_close(files);
        if n == 0 {
            self.heap_free(seg);
            return 0;
        }
        seg
    }
}
