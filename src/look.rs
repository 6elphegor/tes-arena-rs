//! A right click in the centre of the view (2826:0C15): looking at the
//! square under the cursor. In the wild, an inn's or a temple's wall names
//! the kind of place (its trigger's entry in the table at 67FE: 5 a
//! temple, `%tem`; else `%nt`, an inn); in a town, a tavern's, temple's or
//! store's square names it as the townspeople's directions do (1194:E539,
//! E5BE, E626, one entry each), the mages' guild's wall "Mages Guild". The
//! name goes to the message line (061A, 20 frames) and, in a town, to the
//! automap as a note at the square (23F1:00A3).

use crate::bsa::Files;
use crate::click::var::{MESSAGE, MESSAGE_TIME};
use crate::controls::var::PICKED;
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::talk::var::{CLICKED, PLACES_A, PLACES_A_COUNT, PLACES_B, PLACES_B_COUNT, PLACES_C, PLACES_C_COUNT, WAY};
use crate::talk::TalkHooks;

/// Where the name is written (DGROUP 7979).
pub const NAME: u16 = 0x7979;
const MAP: u16 = 0xD806;
const MAP_FLAGS: u16 = 0xC398;
const LEVEL_FLAGS: u16 = 0xC39A;
const TRIGGERS: u16 = 0xC917;
/// The kind of place of each trigger in the wild (FFh none).
const WILD_KINDS: u16 = 0x67FE;

impl Machine {
    /// 2826:0C15. The square `ax` (its row in AH, its column in AL) looked
    /// at: its name, if any, on the message line (and in a town on the
    /// automap, `TalkHooks::way`). A list's count of 0 is searched 65536
    /// times round, as the original's LOOP does. None where a substitution
    /// (2DA2:000A) isn't ported.
    pub fn look(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, ax: u16) -> Option<()> {
        let seg = self.mem.r16(DS, MAP);
        if self.mem.r16(DS, LEVEL_FLAGS) & 0x200 != 0 {
            let w = self.mem.r16(seg, ax & 0xFF00 | ((ax as u8) << 1) as u16);
            if w as i16 <= 0 || w >> 8 != w & 0xFF {
                return Some(());
            }
            let al = (w & 0x3F) as u8;
            // REPNE SCASB over the 16 triggers: past the last unmatched,
            // the index is the last's.
            let i = (0..0x10u16).find(|&i| self.mem.r8(DS, TRIGGERS + i) == al).unwrap_or(0x0F);
            let kind = self.mem.r8(DS, WILD_KINDS + i);
            if kind == 0xFF || kind == 2 {
                return Some(());
            }
            self.copy_cstr(if kind == 5 { 0x7ACA } else { 0x7ACF }, NAME);
            self.expand_name(files, hooks, NAME)?;
            self.message_name();
            return Some(());
        }
        if self.mem.r16(DS, MAP_FLAGS) & 4 == 0 {
            return Some(());
        }
        if let Some((s, bx, cx)) = self.place_at(PLACES_A, PLACES_A_COUNT, ax) {
            self.tavern_name(s, bx, cx);
        } else if let Some((s, bx, cx)) = self.place_at(PLACES_B, PLACES_B_COUNT, ax) {
            self.temple_name(s, bx, cx);
        } else if let Some((s, bx, _)) = self.place_at(PLACES_C, PLACES_C_COUNT, ax) {
            self.store_name(files, hooks, s, bx)?;
        } else {
            let at = ax & 0xFF00 | ((ax as u8).wrapping_add(ax as u8)) as u16;
            if self.mem.r8(seg, at) & 0x3F != self.mem.r8(DS, TRIGGERS + 2) {
                return Some(());
            }
            self.copy_cstr(0x7AD3, NAME);
        }
        self.message_name();
        let p = self.mem.r16(DS, PICKED);
        hooks.way(self, files, NAME, (p & 0xFF) >> 1, p >> 8);
        Some(())
    }

    /// 2826:0D12. The list of places at the far pointer DS:`list` (`count`
    /// entries of 4 bytes: column, row, two name bytes) searched for the
    /// square `ax`: its segment, offset and CX left (the LOOP's count) when
    /// found.
    fn place_at(&self, list: u16, count: u16, ax: u16) -> Option<(u16, u16, u16)> {
        let (mut bx, s) = (self.mem.r16(DS, list), self.mem.r16(DS, list + 2));
        let mut cx = self.mem.r16(DS, count);
        loop {
            if self.mem.r16(s, bx) == ax {
                return Some((s, bx, cx));
            }
            bx = bx.wrapping_add(4);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                return None;
            }
        }
    }

    /// 0000:9CA0: the string at DS:`si` copied to DS:`di` with its NUL.
    fn copy_cstr(&mut self, mut si: u16, mut di: u16) {
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return;
            }
        }
    }

    /// The string at DATA:`si` copied to DS:`di`: DI after its NUL, or with
    /// `nul` false, at it (not written).
    fn copy_data(&mut self, mut si: u16, mut di: u16, nul: bool) -> u16 {
        loop {
            let c = self.mem.r8(DATA, si);
            si = si.wrapping_add(1);
            if c == 0 && !nul {
                return di;
            }
            self.mem.w8(DS, di, c);
            di = di.wrapping_add(1);
            if c == 0 {
                return di;
            }
        }
    }

    /// 1194:E539. A tavern's name (the entry at `s`:`bx`) at NAME: its
    /// first word (3276:3CB6 by the entry's +2 with CX's high byte `cx`'s),
    /// a space, its second by the region (3D5D, or 3DF7 in the Imperial
    /// province or where A4C is not among the 3Ah at 47E2: 1194:E56B).
    fn tavern_name(&mut self, s: u16, bx: u16, cx: u16) {
        let n = cx & 0xFF00 | self.mem.r8(s, bx.wrapping_add(2)) as u16;
        let si = self.skip_strings(DATA, 0x3CB6, n);
        let mut di = self.copy_data(si, NAME, false);
        self.mem.w8(DS, di, b' ');
        di = di.wrapping_add(1);
        let suffixes = if self.mem.r8(DS, 0x0A46) != 8 && (0..0x3Au16).any(|i| self.mem.r8(DS, 0x47E2 + i) == self.mem.r8(DS, 0x0A4C)) {
            0x3D5D
        } else {
            0x3DF7
        };
        let si = self.skip_strings(DATA, suffixes, self.mem.r8(s, bx.wrapping_add(3)) as u16);
        self.copy_data(si, di, true);
    }

    /// 1194:E5BE. A temple's name at NAME: its first word (3276:3E81 by
    /// +2, CX's high byte `cx`'s) run into its second by +2 (0 3EAC, 1
    /// 3EE9, else 3F28) and +3 (CX's high byte 0 then, the skip's LOOP
    /// having run CX out).
    fn temple_name(&mut self, s: u16, bx: u16, cx: u16) {
        let kind = self.mem.r8(s, bx.wrapping_add(2));
        let si = self.skip_strings(DATA, 0x3E81, cx & 0xFF00 | kind as u16);
        let di = self.copy_data(si, NAME, false);
        let suffixes = match kind {
            0 => 0x3EAC,
            1 => 0x3EE9,
            _ => 0x3F28,
        };
        let si = self.skip_strings(DATA, suffixes, self.mem.r8(s, bx.wrapping_add(3)) as u16);
        self.copy_data(si, di, true);
    }

    /// 1194:E626. A store's name at NAME: two words (3276:3F71 by +2, 403E
    /// by +3) with a space between, their substitutions made (2DA2:000A)
    /// with the store's square at WAY and CLICKED FFFFh meanwhile.
    fn store_name(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, s: u16, bx: u16) -> Option<()> {
        let saved = self.mem.r16(DS, CLICKED);
        self.mem.w16(DS, CLICKED, 0xFFFF);
        self.mem.w16(DS, WAY, self.mem.r8(s, bx) as u16);
        self.mem.w16(DS, WAY + 2, self.mem.r8(s, bx.wrapping_add(1)) as u16);
        let dx = self.mem.r16(s, bx.wrapping_add(2));
        let si = self.skip_strings(DATA, 0x3F71, dx & 0xFF);
        let d = self.copy_data(si, NAME, true);
        self.mem.w8(DS, d.wrapping_sub(1), b' ');
        let si = self.skip_strings(DATA, 0x403E, dx >> 8);
        self.copy_data(si, d, true);
        self.expand_name(files, hooks, NAME)?;
        self.mem.w16(DS, CLICKED, saved);
        Some(())
    }

    /// 2DA2:000A on the text at DS:`di`, the front end's or the port's.
    fn expand_name(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, di: u16) -> Option<()> {
        if hooks.expand(self, di).is_none() {
            self.expand(files, hooks, DS, di)?;
        }
        Some(())
    }

    /// 2826:0C6C/0CED: NAME on the message line for 20 frames.
    fn message_name(&mut self) {
        self.mem.w16(DS, MESSAGE_TIME, 0x14);
        self.mem.w16(DS, MESSAGE, NAME);
        self.mem.w16(DS, MESSAGE + 2, DS);
    }
}
