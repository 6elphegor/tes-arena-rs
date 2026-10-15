//! The level .INF loader (0000:EC03): sets a level up from the text file
//! its .MIF names (INFO chunk). Run after the level is loaded, and again
//! on every level change.
//!
//! The file is a list of lines in sections. `#` starts a comment; lines
//! starting `@` start a section, told apart by their fourth character:
//! - @FLOORS and @WALLS ('O', 'L'): texture lines. Each names a .SET
//!   (`#n` after it gives its texture count) or .IMG file and takes the
//!   next texture numbers; the textures the map uses are read into EMS
//!   banks, 4 KB each, transposed (64 x 64). Lines `*KEYWORD` before a
//!   texture line give that texture a role: CEILING (also the ceiling
//!   height and scale), DRYCHASM, WETCHASM, LAVACHASM, LEVELUP,
//!   LEVELDOWN, MENU n (trigger n), BOXSIDE n, BOXCAP n, TOP.
//! - @FLATS ('A'): sprite lines, a name and fields (Y: H: N: F: and
//!   others) into 5-byte records; `*ITEM n` gives item n that flat. The
//!   flat files are read whole into the flat banks (0000:BED8).
//! - @SOUND ('U'): sound names and their numbers; loaded by the hook at
//!   CD4A, if set.
//! - @TEXT ('X'): texts the level's triggers show (`*TEXT n`), copied to
//!   the text bank.
//!
//! The parser keeps its place with a pointer into the file compared
//! against the file's size, so it reads right only from offset 0 of
//! its buffer, as the game loads it.

use crate::bsa::Files;
use crate::level::var::{BANK_FREE, FLAGS, MAP_DEPTH, MAP_FLAGS, MAP_WIDTH};
use crate::machine::Machine;
use crate::mem::DS;
use crate::mif::var::{BUFFER_SEG, FLOOR_SEG, MAP1_SEG, MAP2_SEG};

/// DGROUP variables.
pub mod var {
    /// A copy of the file name.
    pub const NAME: u16 = 0xC2B5;
    /// The wall scales (0x38 words) and their defaults.
    pub const SCALES: u16 = 0xCB94;
    pub const SCALE_DEFAULTS: u16 = 0xCC04;
    /// Texture use flags: USED[i] for texture i + 1, from 0x40 on for
    /// flats (0x200 bytes).
    pub const USED: u16 = 0xA29E;
    pub const FLATS_USED: u16 = 0xA2DE;
    /// Flat records (5 bytes, from index DOOR_BASE on), 0x316 bytes.
    pub const FLATS: u16 = 0xCD9E;
    pub const FLAT_BASE: u16 = 0xC38E;
    /// The banks for textures (first, count), a second range, the flats
    /// and the texts. (Bank free counts: `crate::level::var::BANK_FREE`.)
    pub const TEXTURE_BANKS: u16 = 0xC8F0;
    pub const OTHER_BANKS: u16 = 0xC8F4;
    pub const FLAT_BANKS: u16 = 0xC8F6;
    pub const TEXT_BANK: u16 = 0xC8FE;
    /// Roles: chasms (dry, wet, lava), stairs (up, down).
    pub const CHASMS: u16 = 0xC92F;
    pub const STAIRS: u16 = 0xC932;
    /// Flat of each item (0x60), sound of each number (0x40), texture of
    /// each trigger (16), box sides and caps (16 each), texture of each
    /// text (from C8A7).
    pub const ITEM_FLATS: u16 = 0xC934;
    pub const SOUNDS: u16 = 0xC867;
    pub const TRIGGERS: u16 = 0xC917;
    pub const BOX_SIDES: u16 = 0xCB49;
    pub const BOX_CAPS: u16 = 0xCB39;
    pub const TEXTS: u16 = 0xC8A7;
    pub const TOP: u16 = 0xCD06;
    /// Ceiling: height (and its negation), second number set, texture
    /// address (segment).
    pub const CEILING: u16 = 0xD80C;
    pub const CEILING_NEG: u16 = 0xD80A;
    pub const CEILING_FLAG: u16 = 0xCB5C;
    pub const CEILING_SEG: u16 = 0xCB64;
    /// Parser state: where the next texture goes (offset in the bank),
    /// the next texture number, the bank, a counter, the end of the text.
    pub const SLOT: u16 = 0xA868;
    pub const TEXTURE: u16 = 0xA86A;
    pub const BANK: u16 = 0xA86B;
    pub const COUNT: u16 = 0xA86C;
    pub const END: u16 = 0xA876;
    /// Floors section flag (0x80) and raw-copy bits.
    pub const SECTION: u16 = 0xC8EF;
    /// Name buffer and flat list.
    pub const LIST: u16 = 0x929E;
    /// Keyword table: 10 (string, handler) word pairs.
    pub const KEYWORDS: u16 = 0xE35B;
    pub const TEXT_FLAG: u16 = 0xE35A;
    /// The buffer the file is read to (far pointer); textures are read
    /// 10000 bytes into it.
    pub const BUFFER: u16 = 0xBA54;
    pub const READ_PTR: u16 = 0xA84C;
    /// Byte saved while a number is cut off.
    pub const SAVED: u16 = 0xA84A;
    /// Text destination (far pointer).
    pub const TEXT_PTR: u16 = 0xCB88;
    /// Shading rows copied to the top of each texture bank (segment) and
    /// the fill for the rest.
    pub const SHADES: u16 = 0xCCC2;
    pub const SHADE_FILL: u16 = 0xC3C6;
    /// Flats loaded: far pointers (offset, bank) by flat number from
    /// FLAT_BASE, the number of the next, and a byte each (a DFA's frame
    /// count, a CFA's byte 11 | 0x80, else 0) with a pointer to the next.
    pub const FLAT_DATA: u16 = 0xD0B8;
    pub const FLAT_NUMBER: u16 = 0xD432;
    pub const FLAT_KINDS: u16 = 0xCA97;
    pub const FLAT_KIND_PTR: u16 = 0xCCC0;
    pub const SAVED_FREE: u16 = 0xE2CC;
    /// Hooks: the end of the load, the sounds.
    pub const DONE_HOOK: u16 = 0xCD2E;
    pub const SOUND_HOOK: u16 = 0xCD4A;
    /// Loaded INFO text (offset), cleared when done.
    pub const INFO_PTR: u16 = 0xCB84;
}
use var::*;

/// The bank the directory is cached in during the load.
const DIR_PAGE: u16 = 0x1F;
/// Steps of a scan, of the parse, and 4 KB reads of one texture file
/// (a real one has a few), taken to hang; and the steps of all of them in
/// one load (nested loops would otherwise multiply). A garbage directory
/// entry can make a "file" of up to 4 GB.
const SCAN_LIMIT: u32 = 0x10000;
const PARSE_LIMIT: u32 = 1 << 20;
const READ_LIMIT: u32 = 0x1000;
const LOAD_LIMIT: u32 = 1 << 22;

thread_local! {
    /// Steps left in the current load.
    static STEPS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// One step of a loop; None when the load's steps are used up.
fn tick() -> Option<()> {
    STEPS.with(|s| {
        let n = s.get().checked_sub(1)?;
        s.set(n);
        Some(())
    })
}

/// `loop` with CX = 0 runs 65536 times.
fn count(n: u16) -> u32 {
    if n == 0 { 0x10000 } else { n as u32 }
}

impl Machine {
    /// 0000:EC03. Loads the .INF file named at DS:`name`. A missing
    /// texture or flat file reads as empty (see `file_open_or_device`):
    /// several flats the INFs name, such as rat.dfa, are not in the data.
    /// A missing INF leaves the buffer's old text, which is parsed. None
    /// where the flats don't fit in their banks (the original stops at
    /// an INT 3), a hook is set to a routine not ported (end CD2E, but
    /// the 3D mode's 0000:4531, `inf_done_hook`; sounds CD4A), a scan runs on
    /// without end (the original hangs), or a *TOP line has no number
    /// (its value is then whatever AX held).
    pub fn info_load(&mut self, files: &mut Files, name: u16) -> Option<()> {
        STEPS.with(|s| s.set(LOAD_LIMIT));
        self.bsa_cache_dir(files, DIR_PAGE);
        let mut i = 0u16;
        loop {
            let c = self.mem.r8(DS, name.wrapping_add(i));
            self.mem.w8(DS, NAME.wrapping_add(i), c);
            i = i.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        for i in 0..0x38 {
            let w = self.mem.r16(DS, SCALE_DEFAULTS + 2 * i);
            self.mem.w16(DS, SCALES + 2 * i, w);
        }
        if self.mem.r16(DS, FLAGS) & 0x200 != 0 {
            self.info_scale(0xC0);
        }
        self.info_mark_used();
        for i in 0..0x316 {
            self.mem.w8(DS, FLATS + i, 0);
        }
        self.info_reserve_banks(TEXTURE_BANKS, 0);
        self.mem.w8(DS, STAIRS, 0xFF);
        self.mem.w8(DS, STAIRS + 1, 0xFF);
        self.info_reserve_banks(OTHER_BANKS, 0);
        for (at, n) in [(ITEM_FLATS, 0x60), (SOUNDS, 0x40), (TRIGGERS, 0x10)] {
            for i in 0..n {
                self.mem.w8(DS, at + i, 0xFF);
            }
        }
        let first = self.mem.r8(DS, TEXTURE_BANKS) as u16;
        let frame = self.mem.r16(DS, BUFFER_SEG);
        let mut bank = first;
        for _ in 0..count(self.mem.r8(DS, TEXTURE_BANKS + 1) as u16) {
            self.ems_map(bank);
            self.info_shades(frame);
            bank = bank.wrapping_add(1);
        }
        self.ems_map(first);
        let (off, seg) = (self.mem.r16(DS, BUFFER), self.mem.r16(DS, BUFFER + 2));
        if off >= 0xFFFD {
            return None;
        }
        // A failed load (no such file, 64 KiB or more, nothing read) is
        // ignored: the text left in the buffer is parsed, up to the last
        // size stored at OPEN_SIZE.
        if self.hook_set(crate::bsa::var::OPEN_HOOK) {
            return None;
        }
        if self.file_load(files, name, seg, off).is_none() && self.mem.r32(DS, crate::bsa::var::ERROR_HOOK) != 0 {
            return None;
        }
        let size = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE);
        self.mem.w16(DS, END, size);
        self.mem.w16(DS, SLOT, 0);
        self.mem.w8(DS, TEXTURE, 1);
        self.mem.w8(DS, BANK, first as u8);
        self.info_parse(files, seg, off)?;
        match self.mem.r32(DS, DONE_HOOK) {
            0 => {}
            crate::hooks::value::INF_DONE => self.inf_done_hook(),
            _ => return None,
        }
        self.mem.w16(DS, INFO_PTR, 0);
        if self.mem.r16(DS, FLAGS) & 0x400 == 0 {
            self.mem.w16(DS, crate::bsa::var::DIR_CACHE, 0xFFFF);
        }
        Some(())
    }

    /// The scales times `by` / 256 (16 bits kept).
    fn info_scale(&mut self, by: u16) {
        for i in 0..0x38 {
            let w = self.mem.r16(DS, SCALES + 2 * i) as u32;
            self.mem.w16(DS, SCALES + 2 * i, ((w * by as u32) >> 8) as u16);
        }
    }

    /// Sets the free counts of the banks in the range at `range` (first,
    /// count bytes) to `v`, so nothing is allocated there.
    fn info_reserve_banks(&mut self, range: u16, v: u16) {
        let first = self.mem.r8(DS, range) as u16;
        for i in 0..self.mem.r8(DS, range + 1) as u16 {
            self.mem.w16(DS, BANK_FREE.wrapping_add((first + i) * 2), v);
        }
    }

    /// 0000:B174. With a shading segment set, copies its first 13 rows of
    /// 256 bytes to F000 in the frame and fills the last 3 rows.
    fn info_shades(&mut self, frame: u16) {
        let src = self.mem.r16(DS, SHADES);
        if src == 0 {
            return;
        }
        let fill = self.mem.r8(DS, SHADE_FILL);
        let mut si = 0u16;
        for row in 0..16u16 {
            let di = 0xF000 + row * 0x100;
            for i in 0..0x100 {
                let b = if di < 0xFD00 {
                    let b = self.mem.r8(src, si);
                    si = si.wrapping_add(1);
                    b
                } else {
                    fill
                };
                self.mem.w8(frame, di + i, b);
            }
        }
    }

    /// 0000:F366. Marks the textures and flats the map uses. With flag
    /// 0x200 all are marked. Floor words name a flat (low byte - 1) and
    /// a texture (high byte, less the buffer segment the loader added;
    /// below 12 only); ceiling words a texture (low 6 bits); wall words
    /// a texture when both bytes match, a flat (type 8) or texture
    /// (types 9..0xB, 0xD) when the top bit is set, else a box side and
    /// cap through their tables.
    fn info_mark_used(&mut self) {
        let all = self.mem.r16(DS, FLAGS) & 0x200 != 0;
        for i in 0..0x200 {
            self.mem.w8(DS, USED + i, all as u8);
        }
        if all {
            return;
        }
        self.mem.w8(DS, FLATS_USED, 1);
        self.mem.w8(DS, FLATS_USED + 1, 1);
        let ceiling = self.mem.r16(DS, MAP_FLAGS) & 4 != 0;
        if ceiling {
            self.mem.w8(DS, FLATS_USED + 0x1D, 1);
            self.mem.w8(DS, FLATS_USED + 0x1E, 1);
        }
        let buffer = self.mem.r16(DS, BUFFER_SEG);
        let mark = |m: &mut Self, at: u16| m.mem.w8(DS, at, 1);
        self.info_scan(FLOOR_SEG, |m, w| {
            let mut ax = w;
            if w & 0xFF != 0 {
                let lo = (w as u8).wrapping_sub(1);
                ax = (w & 0xFF00) | lo as u16;
                mark(m, FLATS_USED + lo as u16);
            }
            let t = ax.wrapping_sub(buffer) >> 8;
            if t < 0xC {
                mark(m, USED + t);
            }
        });
        self.info_scan(MAP1_SEG, |m, w| {
            if w == 0 {
                return;
            }
            if w & 0x8000 != 0 {
                match w >> 12 {
                    8 => mark(m, FLATS_USED + (w & 0xFF)),
                    9 | 0xA | 0xB | 0xD => mark(m, USED - 1 + (w & 0x3F)),
                    _ => {}
                }
            } else if w & 0xFF == w >> 8 {
                mark(m, USED - 1 + (w & 0x3F));
            } else {
                let side = m.mem.r8(DS, BOX_SIDES + (w & 0xF)) as u16;
                mark(m, USED - 1 + side);
                let cap = m.mem.r8(DS, BOX_CAPS + ((w >> 4) & 0xF)) as u16;
                mark(m, USED - 1 + cap);
            }
        });
        if ceiling {
            self.info_scan(MAP2_SEG, |m, w| {
                if w != 0 {
                    mark(m, USED - 1 + (w & 0x3F));
                }
            });
        }
    }

    /// 0000:F3CF. Calls `f` with each word of the map plane whose
    /// segment is at DS:`var`.
    fn info_scan(&mut self, var: u16, mut f: impl FnMut(&mut Self, u16)) {
        let seg = self.mem.r16(DS, var);
        let (width, depth) = (self.mem.r16(DS, MAP_WIDTH), self.mem.r16(DS, MAP_DEPTH));
        let mut row = 0u16;
        for _ in 0..count(depth) {
            let mut di = row;
            for _ in 0..count(width) {
                let w = self.mem.r16(seg, di);
                f(self, w);
                di = di.wrapping_add(2);
            }
            row = row.wrapping_add(0x100);
        }
    }

    /// 0000:ECED. The parse loop over the text at `es`:`di`.
    fn info_parse(&mut self, files: &mut Files, es: u16, mut di: u16) -> Option<()> {
        for _ in 0..PARSE_LIMIT {
            tick()?;
            if !self.info_skip(es, &mut di)? {
                return Some(());
            }
            match self.mem.r8(es, di) {
                b'@' => {
                    let section = self.mem.r8(es, di.wrapping_add(3)) & 0xDF;
                    self.info_next_line(es, &mut di)?;
                    match section {
                        b'A' => self.info_flats(files, es, &mut di)?,
                        b'U' => {
                            if self.info_sounds(es, &mut di)? {
                                return Some(());
                            }
                        }
                        b'X' => {
                            if self.info_texts(es, &mut di)? {
                                return Some(());
                            }
                        }
                        b'O' => {
                            let s = self.mem.r8(DS, SECTION) | 0x80;
                            self.mem.w8(DS, SECTION, s);
                        }
                        b'L' => {
                            let s = self.mem.r8(DS, SECTION) & 0x7F;
                            self.mem.w8(DS, SECTION, s);
                        }
                        _ => {}
                    }
                }
                b'*' => self.info_keyword(es, &mut di)?,
                _ => {
                    let mut si = LIST;
                    loop {
                        let c = self.mem.r8(es, di);
                        if c <= 0x20 {
                            break;
                        }
                        self.mem.w8(DS, si, c);
                        si = si.wrapping_add(1);
                        di = di.wrapping_add(1);
                    }
                    self.mem.w8(DS, si, 0);
                    self.info_texture(files, es, di)?;
                }
            }
        }
        None
    }

    /// 0000:F1DE. Skips blanks and comments; false at the end of the
    /// text.
    fn info_skip(&self, es: u16, di: &mut u16) -> Option<bool> {
        let end = self.mem.r16(DS, END);
        for _ in 0..PARSE_LIMIT {
            tick()?;
            if *di >= end {
                return Some(false);
            }
            let c = self.mem.r8(es, *di);
            if c > 0x20 {
                if c != b'#' {
                    return Some(true);
                }
                self.info_to_cr(es, di)?;
                continue;
            }
            *di = di.wrapping_add(1);
        }
        None
    }

    /// Moves to the next CR.
    fn info_to_cr(&self, es: u16, di: &mut u16) -> Option<()> {
        for _ in 0..SCAN_LIMIT {
            tick()?;
            if self.mem.r8(es, *di) == 0x0D {
                return Some(());
            }
            *di = di.wrapping_add(1);
        }
        None
    }

    /// 0000:F204. To the next CR, then `info_skip`.
    fn info_next_line(&self, es: u16, di: &mut u16) -> Option<()> {
        self.info_to_cr(es, di)?;
        self.info_skip(es, di).map(|_| ())
    }

    /// 0000:F211. Past the current word.
    fn info_word_end(&self, es: u16, di: &mut u16) {
        while self.mem.r8(es, *di) > 0x20 {
            *di = di.wrapping_add(1);
        }
    }

    /// 0000:52BA. The decimal number at `seg`:`off`, with an optional
    /// sign, ending at the first other character (16 bits kept).
    pub(crate) fn info_atoi(&self, seg: u16, mut off: u16) -> u16 {
        let mut neg = false;
        match self.mem.r8(seg, off) {
            b'-' => {
                neg = true;
                off = off.wrapping_add(1);
            }
            b'+' => off = off.wrapping_add(1),
            _ => {}
        }
        let mut n = 0u16;
        loop {
            let c = self.mem.r8(seg, off);
            off = off.wrapping_add(1);
            if !c.is_ascii_digit() {
                break;
            }
            n = n.wrapping_mul(10).wrapping_add((c - b'0') as u16);
        }
        if neg { n.wrapping_neg() } else { n }
    }

    /// 0000:F06B. The next number on the line (before a `#` or the CR):
    /// Some((value, the position after the character ending it)), with
    /// `di` left at its first digit. None if there is none, with `di` at
    /// the `#` or CR. The number is cut off with a NUL while it is read,
    /// the byte kept at SAVED.
    fn info_number(&mut self, es: u16, di: &mut u16) -> Option<(u16, u16)> {
        loop {
            let c = self.mem.r8(es, *di);
            if c == b'#' || c == 0x0D {
                return None;
            }
            if c.is_ascii_digit() {
                break;
            }
            *di = di.wrapping_add(1);
        }
        let mut p = *di;
        loop {
            p = p.wrapping_add(1);
            if self.mem.r8(es, p) <= 0x20 {
                break;
            }
        }
        let saved = self.mem.r8(es, p);
        self.mem.w8(es, p, 0);
        self.mem.w8(DS, SAVED, saved);
        let v = self.info_atoi(es, *di);
        self.mem.w8(es, p, self.mem.r8(DS, SAVED));
        Some((v, p.wrapping_add(1)))
    }

    /// 0000:EEF7. A `*KEYWORD` line: the first keyword in the table that
    /// the text starts with is handled; the rest of the line is skipped.
    fn info_keyword(&mut self, es: u16, di: &mut u16) -> Option<()> {
        *di = di.wrapping_add(1);
        for k in 0..10 {
            let (mut p, handler) = (self.mem.r16(DS, KEYWORDS + 4 * k), self.mem.r16(DS, KEYWORDS + 4 * k + 2));
            let mut d = *di;
            let matched = loop {
                let c = self.mem.r8(DS, p);
                p = p.wrapping_add(1);
                if c == 0 {
                    break true;
                }
                if self.mem.r8(es, d) != c {
                    break false;
                }
                d = d.wrapping_add(1);
            };
            if matched {
                self.info_role(handler, es, d)?;
                break;
            }
        }
        self.info_next_line(es, di)
    }

    /// The keyword handlers (0000:EF7A..F06A), by address: they give the
    /// next texture (TEXTURE) a role.
    fn info_role(&mut self, handler: u16, es: u16, mut di: u16) -> Option<()> {
        let t = self.mem.r8(DS, TEXTURE);
        let mark = |m: &mut Self| m.mem.w8(DS, USED - 1 + t as u16, 1);
        // A table slot taking the number on the line (0 if none).
        let slot = |m: &mut Self, at: u16, di: &mut u16| {
            let n = m.info_number(es, di).map_or(0, |(v, _)| v & 0xF);
            m.mem.w8(DS, at + n, t);
        };
        match handler {
            0xEF7A => {
                mark(self);
                let seg = (self.mem.r16(DS, SLOT) >> 4).wrapping_add(self.mem.r16(DS, BUFFER_SEG));
                self.mem.w16(DS, CEILING_SEG, seg);
                let mut dx = di;
                let (mut h, mut d) = (0, di);
                if let Some((v, after)) = self.info_number(es, &mut d) {
                    (h, dx) = (v, after);
                }
                if h <= 0x20 {
                    h = 0x64;
                }
                self.mem.w16(DS, CEILING, h);
                self.mem.w16(DS, CEILING_NEG, h.wrapping_neg());
                let mut d = dx;
                if let Some((v, after)) = self.info_number(es, &mut d) {
                    dx = after;
                    if v >= 0x20 {
                        self.info_scale(v);
                    }
                }
                di = dx;
                self.mem.w8(DS, CEILING_FLAG, 0);
                if self.mem.r8(es, di) == 0x0A {
                    return Some(());
                }
                if self.info_number(es, &mut di).is_some_and(|(v, _)| v != 0) {
                    self.mem.w8(DS, CEILING_FLAG, 1);
                }
            }
            0xEFE6 | 0xEFF5 | 0xF004 => {
                mark(self);
                let i = (handler - 0xEFE6) / 0xF;
                self.mem.w8(DS, CHASMS + i, t);
            }
            0xF013 => self.mem.w8(DS, STAIRS, t),
            0xF01A => self.mem.w8(DS, STAIRS + 1, t),
            0xF034 => slot(self, TRIGGERS, &mut di),
            0xF03F => {
                mark(self);
                slot(self, BOX_SIDES, &mut di);
            }
            0xF055 => {
                mark(self);
                slot(self, BOX_CAPS, &mut di);
            }
            0xF021 => {
                mark(self);
                let (v, _) = self.info_number(es, &mut di)?;
                self.mem.w16(DS, TOP, v);
            }
            _ => return None,
        }
        Some(())
    }

    /// 0000:F0BC. A texture line: the file named at LIST (`di` is past
    /// the name). With flag 0x400, files whose textures the map doesn't
    /// use are only counted: an .IMG takes one texture slot, a .SET as
    /// many as the digit after the `#` on its line says.
    fn info_texture(&mut self, files: &mut Files, es: u16, di: u16) -> Option<()> {
        if self.mem.r16(DS, FLAGS) & 0x400 != 0 {
            let mut p = LIST;
            while self.mem.r8(DS, p) != b'.' {
                p = p.wrapping_add(1);
            }
            let t = self.mem.r8(DS, TEXTURE);
            if self.mem.r8(DS, p.wrapping_add(1)) & 0xDF == b'I' {
                if self.mem.r8(DS, USED.wrapping_add((t as u16).wrapping_sub(1))) == 0 {
                    self.mem.w8(DS, TEXTURE, t.wrapping_add(1));
                    let slot = self.mem.r16(DS, SLOT).wrapping_add(0x1000);
                    self.mem.w16(DS, SLOT, slot);
                    if slot == 0xF000 {
                        self.mem.w16(DS, SLOT, 0);
                        self.info_next_bank();
                    }
                    return Some(());
                }
            } else {
                let mut q = di;
                for _ in 0..SCAN_LIMIT {
                    tick()?;
                    if self.mem.r8(es, q) == b'#' {
                        break;
                    }
                    q = q.wrapping_add(1);
                }
                if self.mem.r8(es, q) != b'#' {
                    return None;
                }
                let n = (self.mem.r8(es, q.wrapping_add(1)) as u16).wrapping_sub(0x30);
                let mut at = (USED - 1).wrapping_add(t as u16);
                let mut used = false;
                for _ in 0..count(n) {
                    if self.mem.r8(DS, at) != 0 {
                        used = true;
                        break;
                    }
                    at = at.wrapping_add(1);
                }
                if !used {
                    self.mem.w8(DS, TEXTURE, t.wrapping_add(n as u8));
                    let old = self.mem.r16(DS, SLOT);
                    let slot = (n << 12).wrapping_add(old);
                    if slot > old {
                        if slot < 0xF000 {
                            self.mem.w16(DS, SLOT, slot);
                            return Some(());
                        }
                        self.mem.w16(DS, SLOT, slot - 0xF000);
                    } else {
                        self.mem.w16(DS, SLOT, slot.wrapping_add(0x1000));
                    }
                    self.info_next_bank();
                    return Some(());
                }
            }
        }
        self.info_read_textures(files)
    }

    /// The next texture bank (0000:F14F).
    fn info_next_bank(&mut self) {
        let b = self.mem.r8(DS, BANK).wrapping_add(1);
        self.mem.w8(DS, BANK, b);
        self.ems_map(b as u16);
    }

    /// 0000:F15C. Reads the file named at LIST 4 KB at a time to the
    /// buffer, 10000 bytes in, and from there to the next texture slots,
    /// transposed unless SECTION has both the floors flag and other bits
    /// set; a full bank moves on to the next.
    fn info_read_textures(&mut self, files: &mut Files) -> Option<()> {
        self.file_open_or_device(files, LIST)?;
        let off = self.mem.r16(DS, BUFFER).wrapping_add(10000);
        let seg = self.mem.r16(DS, BUFFER + 2);
        self.mem.w16(DS, READ_PTR, off);
        self.mem.w16(DS, READ_PTR + 2, seg);
        let frame = self.mem.r16(DS, BUFFER_SEG);
        let mut di = self.mem.r16(DS, SLOT);
        for _ in 0..READ_LIMIT {
            tick()?;
            if self.file_read(files, seg, off, 0x1000) == 0 {
                self.mem.w16(DS, SLOT, di);
                self.file_close(files);
                return Some(());
            }
            let s = self.mem.r8(DS, SECTION);
            let raw = s & 0x7F != 0 && s & 0x80 != 0;
            for i in 0..0x1000u16 {
                let from = if raw { i } else { (i & 0x3F) * 0x40 + (i >> 6) };
                let b = self.mem.r8(seg, off.wrapping_add(from));
                self.mem.w8(frame, di.wrapping_add(i), b);
            }
            let t = self.mem.r8(DS, TEXTURE).wrapping_add(1);
            self.mem.w8(DS, TEXTURE, t);
            di = di.wrapping_add(0x1000);
            if di == 0xF000 {
                di = 0;
                self.info_next_bank();
            }
        }
        None
    }

    /// 0000:ED0D. The @FLATS section: flat lines into records and the
    /// name list at LIST, and `*ITEM n` lines; then the flats are loaded
    /// into the flat banks: the other banks' free counts are set to 0 for
    /// the load and put back after, the flat banks' to 0xFFFF.
    fn info_flats(&mut self, files: &mut Files, es: u16, di: &mut u16) -> Option<()> {
        self.mem.w8(DS, COUNT, 0);
        let mut si = LIST;
        for _ in 0..PARSE_LIMIT {
            tick()?;
            let c = self.mem.r8(es, *di);
            if c == b'*' {
                self.info_word_end(es, di);
                self.info_skip(es, di)?;
                let n = self.info_atoi(es, *di);
                let k = self.mem.r8(DS, COUNT);
                self.mem.w8(DS, ITEM_FLATS.wrapping_add(n), k);
                let at = FLATS_USED + k as u16;
                self.mem.w8(DS, at, self.mem.r8(DS, at).wrapping_add(1));
                if n == 0xE {
                    self.mem.w8(DS, at + 1, self.mem.r8(DS, at + 1).wrapping_add(1));
                }
                self.info_next_line(es, di)?;
                continue;
            }
            if c != b'@' {
                self.info_flat(es, di, &mut si)?;
                if *di != self.mem.r16(DS, END) {
                    continue;
                }
            }
            let base = self.mem.r16(DS, FLAT_BASE);
            self.mem.w16(DS, FLAT_KIND_PTR, base.wrapping_add(FLAT_KINDS));
            let (first, n) = (self.mem.r8(DS, FLAT_BANKS) as u16, self.mem.r8(DS, FLAT_BANKS + 1) as u16);
            for i in 0..0x20 {
                let w = self.mem.r16(DS, BANK_FREE + 2 * i);
                self.mem.w16(DS, SAVED_FREE + 2 * i, w);
            }
            let outside = |bx: u16| bx < first.wrapping_mul(2) || bx >= first.wrapping_mul(2).wrapping_add(n.wrapping_mul(2));
            for bx in (0..0x40).step_by(2) {
                if outside(bx) {
                    self.mem.w16(DS, BANK_FREE + bx, 0);
                }
            }
            let mut bx = first.wrapping_mul(2);
            for _ in 0..count(n) {
                self.mem.w16(DS, BANK_FREE.wrapping_add(bx), 0xFFFF);
                bx = bx.wrapping_add(2);
            }
            self.mem.w16(DS, FLAT_NUMBER, base);
            self.info_load_flats(files)?;
            for bx in (0..0x40).step_by(2) {
                if outside(bx) {
                    let w = self.mem.r16(DS, SAVED_FREE + bx);
                    self.mem.w16(DS, BANK_FREE + bx, w);
                }
            }
            return Some(());
        }
        None
    }

    /// 0000:BED8. Loads the flats named in the list at LIST (names
    /// separated by a NUL, ended by an empty one; with flag 0x400, names
    /// starting `-` are skipped): each file is read whole into EMS
    /// (`ems_alloc`), its far pointer stored at FLAT_DATA, and a byte
    /// from its header in the kinds table. The extension is the
    /// character after the first `.` in the first 13.
    fn info_load_flats(&mut self, files: &mut Files) -> Option<()> {
        let mut si = LIST;
        for _ in 0..PARSE_LIMIT {
            tick()?;
            if self.mem.r8(DS, si) == 0 {
                return Some(());
            }
            if self.mem.r16(DS, FLAGS) & 0x400 == 0 || self.mem.r8(DS, si) != b'-' {
                let mut di = si;
                for _ in 0..13 {
                    let c = self.mem.r8(DS, di);
                    di = di.wrapping_add(1);
                    if c == b'.' {
                        break;
                    }
                }
                let ext = self.mem.r8(DS, di) & 0xDF;
                let slot = self.mem.r16(DS, FLAT_NUMBER).wrapping_shl(2).wrapping_add(FLAT_DATA);
                self.file_open_or_device(files, si)?;
                let size = self.file_seek(files, 2, 0) as u16;
                self.file_seek(files, 0, 0);
                let (off, bank) = self.ems_alloc(size)?;
                let frame = self.mem.r16(DS, BUFFER_SEG);
                self.file_read(files, frame, off, size);
                self.file_close(files);
                self.mem.w16(DS, slot, off);
                self.mem.w16(DS, slot.wrapping_add(2), bank);
                let p = self.mem.r16(DS, FLAT_KIND_PTR);
                self.mem.w8(DS, p, 0);
                match ext {
                    b'D' => self.mem.w8(DS, p, self.mem.r8(frame, off)),
                    b'C' => self.mem.w8(DS, p, self.mem.r8(frame, off.wrapping_add(0xB)) | 0x80),
                    _ => {}
                }
            }
            let p = self.mem.r16(DS, FLAT_KIND_PTR).wrapping_add(1);
            self.mem.w16(DS, FLAT_KIND_PTR, p);
            let n = self.mem.r16(DS, FLAT_NUMBER).wrapping_add(1);
            self.mem.w16(DS, FLAT_NUMBER, n);
            for _ in 0..0xFF {
                let c = self.mem.r8(DS, si);
                si = si.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        }
        None
    }

    /// 0000:F22D. A flat line: its record (5 bytes at FLATS, number
    /// FLAT_BASE + COUNT) is cleared and its fields set, and its name
    /// added to the list at `si` (a leading `-` dropped; with flag 0x400
    /// one is added to flats the map doesn't use).
    fn info_flat(&mut self, es: u16, di: &mut u16, si: &mut u16) -> Option<()> {
        let k = self.mem.r8(DS, COUNT);
        let rec = FLATS.wrapping_add((k as u16).wrapping_add(self.mem.r16(DS, FLAT_BASE)).wrapping_mul(5));
        self.mem.w32(DS, rec, 0);
        let sparse = self.mem.r16(DS, FLAGS) & 0x400 != 0;
        if sparse && self.mem.r8(DS, FLATS_USED + k as u16) == 0 {
            self.mem.w8(DS, *si, b'-');
            *si = si.wrapping_add(1);
        } else if self.mem.r8(es, *di) == b'-' && !sparse {
            *di = di.wrapping_add(1);
        }
        loop {
            let c = self.mem.r8(es, *di);
            if c <= 0x20 {
                break;
            }
            self.mem.w8(DS, *si, c);
            *si = si.wrapping_add(1);
            *di = di.wrapping_add(1);
        }
        self.mem.w8(DS, COUNT, k.wrapping_add(1));
        self.mem.w16(DS, *si, 0);
        *si = si.wrapping_add(1);
        for _ in 0..SCAN_LIMIT {
            tick()?;
            let c = self.mem.r8(es, *di);
            if c == 0x0D {
                return self.info_next_line(es, di);
            }
            if c <= 0x20 {
                *di = di.wrapping_add(1);
                continue;
            }
            *di = di.wrapping_add(2);
            let field = match c & 0xDF {
                b'Y' => 0,
                b'H' => 1,
                b'N' => 2,
                b'F' => 3,
                _ => 4,
            };
            let v = self.info_atoi(es, *di);
            self.mem.w8(DS, rec.wrapping_add(field), v as u8);
            self.info_word_end(es, di);
        }
        None
    }

    /// 0000:EDB1. The @SOUND section: names into the list at LIST, and
    /// SOUNDS[number] = their index; then the sound hook. True at the end
    /// of the text.
    fn info_sounds(&mut self, es: u16, di: &mut u16) -> Option<bool> {
        self.mem.w8(DS, COUNT, 0);
        self.mem.w16(DS, LIST, 0);
        let mut si = LIST;
        let end = self.mem.r16(DS, END);
        for _ in 0..PARSE_LIMIT {
            tick()?;
            let c = self.mem.r8(es, *di);
            if *di >= end || c == b'@' {
                // The files 23BA:0217 (at SOUND_HOOK) would load into the
                // sound slots, in order: the sound driver's, kept for the
                // front end (`Machine::sound_names`).
                self.sound_names = self.sound_list();
                if self.hook_set(SOUND_HOOK) {
                    return None;
                }
                return Some(*di >= end);
            }
            if c == 0x0D {
                *di = di.wrapping_add(2);
                continue;
            }
            if c == b'#' {
                self.info_next_line(es, di)?;
                continue;
            }
            let n = self.info_sound(es, di, &mut si)?;
            let k = self.mem.r8(DS, COUNT).wrapping_sub(1);
            self.mem.w8(DS, SOUNDS + n as u16, k);
        }
        None
    }

    /// The names in the list at LIST, COUNT of them, each after the last's
    /// NUL.
    fn sound_list(&self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut at = LIST;
        for _ in 0..self.mem.r8(DS, COUNT) {
            let name = self.mem.cstr(DS, at).to_vec();
            at = at.wrapping_add(name.len() as u16 + 1);
            out.push(name);
        }
        out
    }

    /// 0000:F2EA. A sound line: its name into the list; returns its
    /// number (low byte; 0 if none).
    fn info_sound(&mut self, es: u16, di: &mut u16, si: &mut u16) -> Option<u8> {
        loop {
            let c = self.mem.r8(es, *di);
            if c <= 0x20 {
                break;
            }
            self.mem.w8(DS, *si, c);
            *si = si.wrapping_add(1);
            *di = di.wrapping_add(1);
        }
        let k = self.mem.r8(DS, COUNT).wrapping_add(1);
        self.mem.w8(DS, COUNT, k);
        self.mem.w16(DS, *si, 0);
        *si = si.wrapping_add(1);
        let mut n = 0u8;
        for _ in 0..SCAN_LIMIT {
            tick()?;
            let c = self.mem.r8(es, *di);
            if c == 0x0D {
                break;
            }
            if c <= 0x20 {
                *di = di.wrapping_add(1);
                continue;
            }
            n = self.info_atoi(es, *di) as u8;
            self.info_to_cr(es, di)?;
            break;
        }
        self.info_next_line(es, di)?;
        Some(n)
    }

    /// 0000:EE19. The @TEXT section: texts (lines up to each CR, kept)
    /// copied to the text bank at TEXT_PTR, each ended with a NUL; `*TEXT
    /// n` lines set TEXTS[n] to the text's index. True at the end of the
    /// text.
    fn info_texts(&mut self, es: u16, di: &mut u16) -> Option<bool> {
        self.mem.w8(DS, TEXT_FLAG, 1);
        let bank = self.mem.r8(DS, TEXT_BANK);
        if bank != 0xFF {
            self.ems_map(bank as u16);
        }
        self.mem.w8(DS, COUNT, 0);
        let start = self.mem.r16(DS, TEXT_PTR);
        let seg = self.mem.r16(DS, TEXT_PTR + 2);
        let mut si = start;
        let end = self.mem.r16(DS, END);
        for _ in 0..PARSE_LIMIT {
            tick()?;
            if *di >= end {
                self.mem.w8(seg, si, 0);
                return Some(true);
            }
            match self.mem.r8(es, *di) {
                b'@' => {
                    self.mem.w8(seg, si, 0);
                    return Some(false);
                }
                b'*' => {
                    self.mem.w8(DS, TEXT_FLAG, 1);
                    if si != start {
                        self.mem.w8(seg, si.wrapping_sub(1), 0);
                    }
                    self.info_word_end(es, di);
                    self.info_skip(es, di)?;
                    let n = self.info_atoi(es, *di);
                    let k = self.mem.r8(DS, COUNT);
                    self.mem.w8(DS, TEXTS.wrapping_add(n), k);
                    self.mem.w8(DS, COUNT, k.wrapping_add(1));
                    self.info_next_line(es, di)?;
                    if self.mem.r8(es, *di) == b'*' {
                        self.mem.w8(seg, si, 0);
                        si = si.wrapping_add(1);
                        self.mem.w8(DS, TEXT_FLAG, 0);
                    }
                }
                0x0D => *di = di.wrapping_add(2),
                0x0A => *di = di.wrapping_add(1),
                _ => {
                    self.mem.w8(DS, TEXT_FLAG, 0);
                    for _ in 0..SCAN_LIMIT {
                        tick()?;
                        let c = self.mem.r8(es, *di);
                        self.mem.w8(seg, si, c);
                        *di = di.wrapping_add(1);
                        si = si.wrapping_add(1);
                        if c == 0x0D {
                            break;
                        }
                    }
                    if self.mem.r8(es, di.wrapping_sub(1)) != 0x0D {
                        return None;
                    }
                    *di = di.wrapping_add(1);
                }
            }
        }
        None
    }
}
