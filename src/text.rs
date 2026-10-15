//! The game's text: what people say and what the windows show (1194:A374)
//! and the substitutions in it (2DA2:000A).
//!
//! A text has a number (TEXT, 137A). POINTER1.DAT holds its key (the
//! number; for the greetings and other numbers up to 4, and 0Eh, the key
//! also carries a letter by the time of day, the weather and the region,
//! A6EA) and its offset in TEMPLATE.DAT, where it runs to the next `#`
//! after a four-digit header line. A text is several variants between
//! `&`s, one picked at random; its characters are filtered (A643), its
//! `%` codes substituted (`expand`), its runs of spaces collapsed (A559)
//! and its lines wrapped at 137E (49FF; meant for the outermost level
//! only, but the test is lost). Then it
//! is shown in a window (mode 0), offered as a menu (1) or copied out (2).
//!
//! All of it works in the EMS bank at 1030 mapped into the frame (9030):
//! the index's tables at 2710h and 7530h, the text at 1770h (each 400h
//! further per nesting level, 6E54), the file read at 0, a variant at
//! 3E80h, the result at 95BEh, the substitutions' buffer at 72FF (from
//! the bank's number: (bank << 10) + EA60h).
//!
//! A substitution (`expand`) finds a `%`, reads its code through the
//! letter tree at 3276:7686 (one to three letters; an unknown one keeps
//! the code's letters but the last, and is replaced by nothing) and calls
//! its routine, whose result replaces the `%` and the code; the scan goes
//! on from the replacement, so a replacement with codes is substituted
//! too. The routines read the game's state: the person clicked (CB5E),
//! the province, city and race (A46, A47, 1A8), the quest (0F3E), the
//! player's name (1AD), names made from seeds (1194:080B), and further
//! texts (A374 again). %qf, which seeds a name with EBX the original
//! leaves over, gives None, as do %en outside a building of kind 6 (its routine
//! returns into the text: it leaves DS pushed) and the loops where the
//! original never ends (a key not in the index, a copy over its own
//! source, codes coming back without end).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// The windows and menus a text is shown in.
pub trait TextHooks {
    /// 1194:59CC: a message box with the text at DS:`si` (colour 9288).
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:020E: a menu, its title at DS:`si`, its keys at `bx`, its
    /// entries' table at `di`. The pick (FFFFh: none).
    fn menu(&mut self, _m: &mut Machine, _si: u16, _bx: u16, _di: u16) -> u16 {
        0xFFFF
    }
}

/// `TextHooks` that show nothing and leave menus.
pub struct NoTextHooks;

impl TextHooks for NoTextHooks {}

/// Variables.
pub mod var {
    /// The text's number, the wrap width (3Ch when 0), no substitutions
    /// when FFh, the font flag (FFh: the second font is left).
    pub const TEXT: u16 = 0x137A;
    pub const WIDTH: u16 = 0x137E;
    pub const NO_EXPAND: u16 = 0x101C;
    pub const FONT: u16 = 0x137F;
    /// The nesting level, the tables' and the text's offsets in the frame,
    /// the match being looked for (the nth by region, 6EAA).
    pub const LEVEL: u16 = 0x6E54;
    pub const KEYS: u16 = 0x6E50;
    pub const OFFSETS: u16 = 0x6E52;
    pub const OUT: u16 = 0x6E55;
    pub const NTH: u16 = 0x6EA9;
    /// The EMS bank the text works in, and the frame it is mapped into.
    pub const BANK: u16 = 0x1030;
    pub const FRAME: u16 = 0x9030;
    /// A message box's text (mode 0 and 1 copy the text here).
    pub const BOX: u16 = 0x9A9E;
    /// In 3276: the substitutions' buffer, their nesting, the start of the
    /// code being replaced, the letter tree, the sex of a name (0, 1;
    /// some codes read it).
    pub const SUB_BUFFER: u16 = 0x72FF;
    pub const SUB_LEVEL: u16 = 0x7307;
    pub const SUB_AT: u16 = 0x730D;
    pub const TREE: u16 = 0x7686;
    pub const SEX: u16 = 0x7387;
}
use var::*;

/// "pointer1.dat", "template.dat", the error text.
const POINTERS: u16 = 0x197F;
const TEMPLATES: u16 = 0x6E57;
const ERROR: u16 = 0x6E64;

/// Where a code's routine left its result: a string at seg:off, or the
/// buffer at SUB_BUFFER written up to DI.
enum Res {
    Str(u16, u16),
    Buf(u16),
}

/// What a code's routine is called with: the text's segment (BP), the
/// rest of the text after the code (SI, DX), the buffer (ES:DI).
struct Ctx {
    bp: u16,
    rest: u16,
    dx: u16,
    es: u16,
    di: u16,
}

/// Bounds for loops the original runs until memory gives it what it
/// looks for: past them the original hangs (or reads on through all of
/// memory), which the port reports as None.
const SCAN: u32 = 0x20000;

/// Substitutions a text can take: the longest template has a few dozen
/// codes, so past this many the replacements keep bringing codes back
/// (or the original has jumped into garbage), where the original never
/// ends; the port gives None.
const SUBSTITUTIONS: u32 = 0x400;

impl Machine {
    fn t8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }

    fn t16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    /// 0000:5C8A's AX: the bank mapped before (when EMS is on and `page`
    /// exists), else `ax` as it was.
    fn text_map(&mut self, page: u16, ax: u16) -> u16 {
        use crate::level::var::{EMS_MAPPED, EMS_ON, EMS_PAGES};
        if self.t8(EMS_ON) == 0 || page >= self.t16(EMS_PAGES) {
            return ax;
        }
        let old = self.t16(EMS_MAPPED) >> 2;
        self.ems_map(page);
        old
    }

    /// 0000:9CA0: strcpy, NUL included; DI past it. None where the copy
    /// runs over its own source ahead of it (the scratch buffer just past
    /// the text: the original then copies for ever).
    fn text_copy(&mut self, ss: u16, mut si: u16, es: u16, mut di: u16) -> Option<(u16, u16)> {
        for _ in 0..0x10000u32 {
            let c = self.mem.r8(ss, si);
            self.mem.w8(es, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return Some((si, di));
            }
        }
        None
    }

    /// 0000:9CB0: the length of the string at es:di.
    /// (With no NUL in the segment the original scans on for ever; here
    /// the length stops at FFFFh.)
    fn text_len(&self, es: u16, di: u16) -> u16 {
        let mut n = 0u16;
        while n != 0xFFFF && self.mem.r8(es, di.wrapping_add(n)) != 0 {
            n += 1;
        }
        n
    }

    /// 0000:9BD4: strcat, DI past the NUL.
    fn text_cat(&mut self, ss: u16, si: u16, es: u16, di: u16) -> Option<u16> {
        let di = di.wrapping_add(self.text_len(es, di));
        Some(self.text_copy(ss, si, es, di)?.1)
    }

    /// `repne scasb` for `al` over `cx` bytes at es:di: DI after, found.
    fn text_scan(&self, es: u16, mut di: u16, al: u8, cx: u16) -> (u16, bool) {
        for _ in 0..cx {
            let c = self.mem.r8(es, di);
            di = di.wrapping_add(1);
            if c == al {
                return (di, true);
            }
        }
        (di, false)
    }

    /// 0000:75B4: `v` in decimal at es:di (its digits made at DS:94F6
    /// down), NUL-terminated; DI past the NUL.
    fn text_ultoa(&mut self, v: u32, es: u16, di: u16) -> u16 {
        let mut si = 0x94F6u16;
        let mut v = v;
        loop {
            self.mem.w8(DS, si, b'0' + (v % 10) as u8);
            si = si.wrapping_sub(1);
            v /= 10;
            if v == 0 {
                break;
            }
        }
        let n = 0x94F6u16.wrapping_sub(si);
        let mut di = di;
        for k in 0..n {
            let c = self.t8(si.wrapping_add(1).wrapping_add(k));
            self.mem.w8(es, di, c);
            di = di.wrapping_add(1);
        }
        self.mem.w8(es, di, 0);
        di.wrapping_add(1)
    }

    /// 0000:7056: `v` (16 bits) in decimal at es:di (its digits made at
    /// DS:A933 down), NUL-terminated; DI at the NUL.
    fn text_utoa(&mut self, v: u16, es: u16, mut di: u16) -> u16 {
        let mut p = 0xA933u16;
        let mut v = v;
        loop {
            self.mem.w8(DS, p, b'0' + (v % 10) as u8);
            p = p.wrapping_sub(1);
            v /= 10;
            if v == 0 {
                break;
            }
        }
        let n = 0xA933u16.wrapping_sub(p);
        for k in 0..n {
            let c = self.t8(p.wrapping_add(1).wrapping_add(k));
            self.mem.w8(es, di, c);
            di = di.wrapping_add(1);
        }
        self.mem.w8(es, di, 0);
        di
    }

    /// 1194:55F2: the region's index (by 5D2 in the table at 5204, a
    /// third of that, found in the table at 5210).
    fn text_region(&self) -> Option<u16> {
        let al = self.t16(0x05D2) as u8;
        let mut bl = 0u8;
        let mut si = 0x5204u16;
        let mut n = 0;
        while self.t8(si) != al {
            si = si.wrapping_add(1);
            bl = bl.wrapping_add(1);
            n += 1;
            if n > SCAN {
                return None;
            }
        }
        let al = bl / 3;
        let mut si = 0x5210u16;
        let mut cx = 0u16;
        while self.t8(si) != al {
            cx = cx.wrapping_add(1);
            si = si.wrapping_add(1);
            if cx as u32 > SCAN {
                return None;
            }
        }
        Some(cx)
    }

    /// 1194:A6EA: the text's letter. In a building of kind 1 (D48), or
    /// from 6:00 to 18:00, by the region and the weather (the table at
    /// 1C5E); otherwise by the region (6E8B).
    fn text_letter(&self) -> Option<u16> {
        let day = self.t8(0x0D48) == 1 || (0x168..=0x438).contains(&((self.mem.r32(DS, 0x05CC) % 0x5A0) as u16));
        let r = self.text_region()?;
        if !day {
            return Some(self.t8(0x6E8B_u16.wrapping_add(r)) as u16);
        }
        let w = self.t8(0xBB09);
        let bl = (w & 0x7F) | ((w & 0x80) >> 5);
        Some(self.t8(0x1C5E_u16.wrapping_add(r << 3).wrapping_add(bl as u16)) as u16)
    }

    /// 1194:A374. Text TEXT shown (`mode` 0, in a window), offered as a
    /// menu (1) or copied to es:di (2); returns AX (a menu's pick) and DI
    /// (past the copy's NUL in mode 2). Without POINTER1.DAT the error
    /// text is shown and its first five characters copied, with a NUL
    /// five past them (and the nesting level stays raised), as in the
    /// original. None where the
    /// key isn't in the index (the original searches forever), a file is
    /// missing or a substitution isn't ported.
    pub fn text(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, mode: u16, es: u16, di: u16) -> Option<(u16, u16)> {
        // Where the port gives up (a code it doesn't make, or the
        // original's fault or hang) the nesting is put back as the
        // routine's exit leaves it, so that later texts don't start one
        // level in (with the buffers of the next level and a substitution
        // that skips its text). The original has no such exit.
        let level = self.t8(LEVEL);
        let r = self.text_body(files, hooks, mode, es, di);
        if r.is_none() {
            self.mem.w8(DS, LEVEL, level);
        }
        r
    }

    fn text_body(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, mode: u16, es: u16, di: u16) -> Option<(u16, u16)> {
        self.mem.w8(DS, NTH, 0);
        let bank = self.t16(BANK);
        self.text_map(bank, 0);
        let lvl = self.t8(LEVEL) as u16;
        self.mem.w16(DS, KEYS, (lvl << 10).wrapping_add(0x2710));
        self.mem.w16(DS, OFFSETS, (lvl << 10).wrapping_add(0x7530));
        self.mem.w16(DS, OUT, (lvl << 10).wrapping_add(0x1770));
        self.mem.w8(DS, LEVEL, (lvl as u8).wrapping_add(1));
        let frame = self.t16(FRAME);
        for t in [KEYS, OFFSETS] {
            let o = self.t16(t);
            for i in 0..0x1388u16 {
                self.mem.w8(frame, o.wrapping_add(i), 0);
            }
        }
        if !self.file_open_or_device(files, POINTERS)? {
            self.mem.w16(DS, 0xB036, self.t16(0x102C));
            self.mem.w8(DS, 0x9288, 0x60);
            hooks.message_box(self, ERROR);
            // 0000:7702 leaves DI past the five, and the NUL goes 5 past
            // that.
            for i in 0..5u16 {
                let c = self.t8(ERROR + i);
                self.mem.w8(es, di.wrapping_add(i), c);
            }
            self.mem.w8(es, di.wrapping_add(10), 0);
            return Some((0, di.wrapping_add(5)));
        }
        for t in [KEYS, OFFSETS] {
            let o = self.t16(t);
            self.file_read(files, frame, o, 0x2710);
        }
        self.file_close(files);
        let bx = self.text_letter()?;
        let t = self.t16(TEXT);
        let mut ax = t;
        if t == 0xE || t <= 4 {
            let b = if (0x61..=0x7A).contains(&bx) { bx - 0x20 } else { bx };
            ax = t | (b.wrapping_sub(0x41) << 8) | 0x8000;
        }
        let mut k = self.t16(KEYS);
        let mut n = 0u32;
        loop {
            while self.mem.r16(frame, k) != ax {
                k = k.wrapping_add(2);
                n += 1;
                if n > SCAN {
                    return None;
                }
            }
            if self.t16(TEXT) > 4 {
                break;
            }
            let cl = self.t8(0x6EAA_u16.wrapping_add(self.t16(0x0DA8) >> 2));
            if self.t8(NTH) == cl {
                break;
            }
            self.mem.w8(DS, NTH, self.t8(NTH).wrapping_add(1));
            k = k.wrapping_add(2);
        }
        let k = k.wrapping_sub(self.t16(KEYS));
        let k = k.wrapping_add(k).wrapping_add(self.t16(OFFSETS));
        let at = self.mem.r32(frame, k);
        self.file_open(files, TEMPLATES)?;
        self.file_seek(files, 0, at);
        let mut si = 0u16;
        let mut n = 0;
        loop {
            self.file_read(files, frame, si, 1);
            let c = self.mem.r8(frame, si);
            if c == b'#' || c == 0 {
                break;
            }
            si = si.wrapping_add(1);
            n += 1;
            if n > SCAN {
                return None;
            }
        }
        self.mem.w8(frame, si.wrapping_add(1), 0);
        self.file_close(files);
        self.text_variant(frame)?;
        self.text_filter(frame);
        if self.t8(NO_EXPAND) != 0xFF {
            let out = self.t16(OUT);
            self.expand(files, hooks, frame, out)?;
            self.mem.w16(DS, OUT, out);
        }
        self.mem.w16(DS, 0xB038, self.t16(0x1036));
        if self.t8(FONT) != 0xFF {
            self.mem.w16(DS, 0xB036, self.t16(0x102C));
        }
        // 1194:A4B0 means to wrap only at the outermost level (or not
        // with 5C8 bit 10h), but A559 leaves AL 0 (the NUL it copied), so
        // the wrap always runs.
        self.text_spaces(frame);
        self.text_wrap(frame)?;
        let res = match mode {
            0 => {
                self.text_copy(frame, 0x95BE, DS, BOX)?;
                self.mem.w8(DS, 0x9288, 0x60);
                hooks.message_box(self, BOX);
                (0, di)
            }
            1 => {
                self.text_copy(frame, 0x95BE, DS, BOX)?;
                self.mem.w8(DS, 0x9288, 0x60);
                (hooks.menu(self, BOX, 0x6E8F, 0x6E92), di)
            }
            _ => {
                let (_, di) = self.text_copy(frame, 0x95BE, es, di)?;
                (0, di)
            }
        };
        self.mem.w8(DS, LEVEL, self.t8(LEVEL).wrapping_sub(1));
        Some(res)
    }

    /// 1194:A5FE. The variants (`&` made NUL) counted; a random one past
    /// the header (6 bytes) copied to 3E80h. None where there is none
    /// (the original divides by zero).
    fn text_variant(&mut self, frame: u16) -> Option<()> {
        let mut si = 0u16;
        let mut bx = 0u16;
        loop {
            let c = self.mem.r8(frame, si);
            if c == 0 {
                break;
            }
            if c == b'&' {
                self.mem.w8(frame, si, 0);
                bx = bx.wrapping_add(1);
            }
            si = si.wrapping_add(1);
        }
        let k = self.rng.range(0, bx)?;
        let si = self.skip_strings(frame, 6, k);
        self.text_copy(frame, si, frame, 0x3E80)?;
        Some(())
    }

    /// 1194:A643. The variant copied to the text (OUT) keeping letters,
    /// digits, `%` and `!.?',_-:`; a line break becomes a space, and a
    /// space is kept only once something was and when the next isn't one.
    fn text_filter(&mut self, frame: u16) {
        let mut si = 0x3E80u16;
        let mut di = self.t16(OUT);
        let mut bx = 0u16;
        loop {
            let mut c = self.mem.r8(frame, si);
            if c == 0 {
                self.mem.w8(frame, di, 0);
                return;
            }
            let keep = 'k: {
                if c == b'%' {
                    break 'k true;
                }
                if c == 0x0A {
                    if bx == 0 {
                        break 'k false;
                    }
                    self.mem.w8(frame, si, b' ');
                    c = b' ';
                }
                if b"!.?',_-:".contains(&c) {
                    break 'k true;
                }
                if c == b' ' {
                    break 'k bx != 0 && self.mem.r8(frame, si.wrapping_add(1)) != b' ';
                }
                c.is_ascii_alphanumeric()
            };
            if keep {
                self.mem.w8(frame, di, c);
                di = di.wrapping_add(1);
                bx = bx.wrapping_add(1);
            }
            si = si.wrapping_add(1);
        }
    }

    /// 1194:A559. The text (OUT) copied to 95BEh with runs of two or three
    /// spaces made one.
    fn text_spaces(&mut self, frame: u16) {
        let mut si = self.t16(OUT);
        let mut di = 0x95BEu16;
        loop {
            let c = self.mem.r8(frame, si);
            if c == 0 {
                self.mem.w8(frame, di, 0);
                return;
            }
            if c == b' ' && self.mem.r8(frame, si.wrapping_add(1)) == b' ' {
                si = si.wrapping_add(1);
                if self.mem.r8(frame, si.wrapping_add(1)) == b' ' {
                    si = si.wrapping_add(1);
                }
            }
            let c = self.mem.r8(frame, si);
            self.mem.w8(frame, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
        }
    }

    /// 1194:49FF on the text at 95BEh: lines broken (a space made a line
    /// break) every WIDTH characters (3Ch when 0, which is then stored),
    /// at the last space before.
    fn text_wrap(&mut self, frame: u16) -> Option<()> {
        let mut si = 0x95BEu16;
        let mut ax = self.text_len(frame, si);
        let mut n = 0u32;
        loop {
            if self.t8(WIDTH) == 0 {
                self.mem.w8(DS, WIDTH, 0x3C);
            }
            let bx = self.t8(WIDTH) as u16;
            if ax <= bx {
                return Some(());
            }
            si = si.wrapping_add(bx);
            ax = ax.wrapping_sub(bx);
            loop {
                let c = self.mem.r8(frame, si);
                if c == 0 {
                    return Some(());
                }
                if c == b' ' {
                    self.mem.w8(frame, si, 0x0A);
                    si = si.wrapping_add(1);
                    break;
                }
                si = si.wrapping_sub(1);
                ax = ax.wrapping_add(1);
                n += 1;
                if n > SCAN {
                    return None;
                }
            }
        }
    }

    /// 2DA2:000A. The `%` codes in the text at es:`di` substituted, the
    /// text rewritten in place; returns DI after (at the text's NUL). The
    /// seed is as it was after. At a nesting of more than one the text is
    /// left, and the nesting count still drops (as in the original). None
    /// where a code's routine isn't ported or the text is empty (the
    /// original then reads its code from the NUL).
    pub fn expand(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, es: u16, di: u16) -> Option<u16> {
        // Where the port gives up, its exit's work is done (as at 2DA2's
        // end): the nesting (SUB_LEVEL) as it was on entry, the seed
        // back, and, at the outer level, the bank mapped before it (8A0C)
        // mapped again. Without it the next substitution starts nested,
        // skips its text and maps a stale bank, and the text routine
        // then reads another bank's bytes as its text (a quest giver's
        // greeting, 56h, ran on into DGROUP from 1044). The original has
        // no such exit.
        let n = self.mem.r8(DATA, SUB_LEVEL);
        let seed = self.rng.seed;
        let r = self.expand_body(files, hooks, es, di);
        if r.is_none() {
            self.mem.w8(DATA, SUB_LEVEL, n);
            self.rng.seed = seed;
            if n <= 1 {
                let bank = self.t16(0x8A0C);
                self.ems_map(bank);
            }
        }
        r
    }

    fn expand_body(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, es: u16, di: u16) -> Option<u16> {
        let saved = self.rng.seed;
        self.mem.w8(DS, 0x8A43, 0);
        let n = self.mem.r8(DATA, SUB_LEVEL);
        let mut di = di;
        if n <= 1 {
            let bank = self.t16(BANK);
            let ax = self.text_map(bank, n as u16);
            self.mem.w16(DS, 0x8A0C, ax);
            self.mem.w16(DATA, SUB_BUFFER, (ax << 10).wrapping_add(0xEA60));
            self.mem.w8(DATA, SUB_LEVEL, n.wrapping_add(1));
            let bp = es;
            self.mem.w16(DATA, SUB_AT, di);
            let mut steps = 0u32;
            loop {
                let len = self.text_len(bp, di);
                di = self.mem.r16(DATA, SUB_AT);
                if len == 0 {
                    return None;
                }
                let (d, found) = self.text_scan(bp, di, b'%', len);
                di = d;
                if !found {
                    break;
                }
                steps += 1;
                if steps > SUBSTITUTIONS {
                    return None;
                }
                let (handler, rest) = self.text_code(bp, di);
                let frame = self.t16(FRAME);
                let buf = self.mem.r16(DATA, SUB_BUFFER);
                let mut c = Ctx { bp, rest, dx: rest, es: frame, di: buf };
                let res = self.text_sub(files, hooks, handler, &mut c)?;
                let (ss, si) = match res {
                    Res::Buf(d) if d != self.mem.r16(DATA, SUB_BUFFER) => (frame, self.mem.r16(DATA, SUB_BUFFER)),
                    Res::Buf(_) => return None,
                    Res::Str(s, o) => (s, o),
                };
                let frame = self.t16(FRAME);
                let buf = self.mem.r16(DATA, SUB_BUFFER);
                let (_, d) = self.text_copy(ss, si, frame, buf)?;
                self.text_copy(bp, c.dx, frame, d.wrapping_sub(1))?;
                let at = self.mem.r16(DATA, SUB_AT);
                self.text_copy(frame, buf, bp, at)?;
                di = at;
            }
        }
        let n = self.mem.r8(DATA, SUB_LEVEL);
        self.mem.w8(DATA, SUB_LEVEL, n.wrapping_sub(1));
        self.rng.seed = saved;
        let bank = self.t16(0x8A0C);
        self.ems_map(bank);
        Some(di)
    }

    /// 2DA2:00EC. The code after the `%` (whose next character is at
    /// es:`di`) read through the letter tree: the routine's offset and
    /// where the rest of the text starts. SUB_AT is set to the `%`.
    fn text_code(&mut self, es: u16, di: u16) -> (u16, u16) {
        self.mem.w16(DATA, SUB_AT, di.wrapping_sub(1));
        let letter = |c: u8| (0x61..0x7A).contains(&c);
        let c = self.mem.r8(es, di) as u16;
        let mut si = self.mem.r16(DATA, TREE.wrapping_add(c.wrapping_sub(0x61).wrapping_mul(2)));
        let mut di = di.wrapping_add(1);
        'outer: loop {
            if !letter(self.mem.r8(es, di)) {
                si = si.wrapping_add(2);
                break;
            }
            loop {
                if self.mem.r8(DATA, si) == 0xFF {
                    while (0x61..=0x7A).contains(&self.mem.r8(es, di)) {
                        di = di.wrapping_add(1);
                    }
                    si = si.wrapping_add(2);
                    di = di.wrapping_add(1);
                    break 'outer;
                }
                if self.mem.r8(DATA, si) != self.mem.r8(es, di) {
                    si = si.wrapping_add(4);
                    continue;
                }
                si = si.wrapping_add(1);
                di = di.wrapping_add(1);
                let c2 = self.mem.r8(es, di);
                if !letter(c2) {
                    si = si.wrapping_add(1);
                    break 'outer;
                }
                if self.mem.r8(DATA, si) == c2 {
                    si = si.wrapping_add(1);
                    di = di.wrapping_add(1);
                    break 'outer;
                }
                di = di.wrapping_sub(1);
                si = si.wrapping_add(3);
                continue 'outer;
            }
        }
        (self.mem.r16(DATA, si), di)
    }

    /// 2DA2:1083. The quest's record: 0F3E when its kind (+4 bits 6..7)
    /// and 8A56 (0 or 1, else 0) share a bit, else the one at F72.
    fn text_quest(&self) -> u16 {
        let ax = (self.t16(0x0F3E + 4) & 0xC0) >> 6;
        let bx = self.t8(0x8A56) as u16;
        let bx = if bx <= 1 { bx } else { 0 };
        if ax & bx != 0 { 0x0F3E } else { self.t16(0x0F72) }
    }

    /// 2DA2:1075: `v` in decimal into the buffer, and a 0 past its NUL.
    fn text_number(&mut self, c: &Ctx, v: u32) -> Res {
        let di = self.text_ultoa(v, c.es, c.di);
        self.mem.w8(c.es, di, 0);
        Res::Buf(di)
    }

    /// 2DA2:1041: text `ax` copied into the buffer (A374 mode 2, its width
    /// 0 and no substitutions, which stay so); DI after.
    fn text_nested(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, c: &Ctx, ax: u16) -> Option<u16> {
        let (b, a) = (self.mem.r16(DATA, SUB_BUFFER), self.mem.r16(DATA, SUB_AT));
        self.mem.w16(DS, TEXT, ax);
        self.mem.w8(DS, WIDTH, 0);
        self.mem.w8(DS, NO_EXPAND, 0xFF);
        let (_, di) = self.text(files, hooks, 2, c.es, c.di)?;
        self.mem.w16(DATA, SUB_AT, a);
        self.mem.w16(DATA, SUB_BUFFER, b);
        Some(di)
    }

    /// 2DA2:0742: line breaks made spaces in the string at es:si.
    fn text_unbreak(&mut self, es: u16, mut si: u16) {
        loop {
            let c = self.mem.r8(es, si);
            if c == 0 {
                return;
            }
            if c == 0x0D || c == 0x0A {
                self.mem.w8(es, si, b' ');
            }
            si = si.wrapping_add(1);
        }
    }

    /// The name at es:di cut at its first space (`plus` 1: the scan runs
    /// over the NUL too; else a name without one loses its last letter),
    /// as several routines do after 1194:0807. DI at the cut.
    fn text_first_name(&mut self, es: u16, di: u16, plus: u16) -> u16 {
        let len = self.text_len(es, di);
        let (d, _) = self.text_scan(es, di, b' ', len.wrapping_add(plus));
        let d = d.wrapping_sub(1);
        self.mem.w8(es, d, 0);
        d
    }

    /// 2DA2:10A5: a word from the list at 3276:`si` (two counts, then
    /// the strings), seeded by the quest (+1Eh, whose low bit is the
    /// sex): one of the first count, or past them for the other sex.
    fn text_word(&mut self, si: u16) -> Option<Res> {
        let q = self.text_quest();
        let v = self.mem.r32(DS, q.wrapping_add(0x1E));
        self.rng.seed = v;
        self.mem.w8(DATA, SEX, v as u8 & 1);
        let a = self.mem.r8(DATA, si);
        let b = self.mem.r8(DATA, si.wrapping_add(1));
        let mut ax = self.rng.range(0, a.wrapping_add(b) as u16)?;
        if (ax as u8 as i8) >= (a as i8) {
            let cl = self.rng.seed as u8 & 1;
            self.mem.w8(DATA, SEX, cl);
            if cl != 0 {
                ax = (ax & 0xFF00) | (ax as u8).wrapping_add(b) as u16;
            }
        }
        Some(Res::Str(DATA, self.skip_strings(DATA, si.wrapping_add(2), ax)))
    }

    /// 2DA2:1156: the titles' list by `al` (none of its low two bits: the
    /// women's, 33F5, and AH 1), else 33CB.
    fn text_titles(al: u8) -> (u16, u8) {
        if al & 3 != 0 { (0x33CB, 0) } else { (0x33F5, 1) }
    }

    /// 2DA2:1167: a title from the list at `si`: by the province (6 in
    /// the Imperial one), else by `al` (0: 5, 2: 0, else random 1..4).
    fn text_title(&mut self, al: u8, si: u16) -> Option<Res> {
        let cx = if self.t8(0x0A46) == 8 {
            6
        } else if al == 0 {
            5
        } else if al == 2 {
            0
        } else {
            self.rng.range(1, 5)?
        };
        Some(Res::Str(DS, self.skip_strings(DS, si, cx)))
    }

    /// 2DA2:1193: a place's kind by `cx` (7: copied from 72FB into the
    /// buffer, else from the list at 7615). Also gives DS:SI as left.
    fn text_kind(&mut self, c: &Ctx, cx: u16) -> Option<(Res, u16)> {
        if cx == 7 {
            let (si, di) = self.text_copy(DATA, 0x72FB, c.es, c.di)?;
            Some((Res::Buf(di), si))
        } else {
            let si = self.skip_strings(DATA, 0x7615, cx);
            Some((Res::Str(DATA, si), si))
        }
    }

    /// 2DA2:111C (far): the holiday of the day `eax` (minutes) falls on,
    /// within the year (7E900h minutes): its index (CX), the day (DX), and
    /// whether it is one (carry); past the last (or after 158h) DX is the
    /// next holiday's day, or the word at DS:`si`.
    fn text_holiday(&self, eax: u32, si: u16) -> (u16, u16, bool) {
        let d = ((eax % 0x7E900) / 0x5A0) as u16;
        if d > 0x158 {
            return (0, self.t16(si), false);
        }
        let mut p = 0x33ADu16;
        let mut cx = 0u16;
        loop {
            let w = self.t16(p);
            if w == d {
                return (cx, d, true);
            }
            if (w as i16) > (d as i16) {
                return (cx, w, false);
            }
            p = p.wrapping_add(2);
            cx = cx.wrapping_add(1);
        }
    }

    /// The seed of the wild's square (0000:2DA2:092B): the position's
    /// chunk numbers from 608/60A, carried past 2000h of `cx`, `dx`.
    fn text_wild_seed(&self, cx: u16, dx: u16) -> u32 {
        let hi = self.t16(0x060A).wrapping_add((dx >= 0x2000) as u16);
        let lo = self.t16(0x0608).wrapping_add((cx >= 0x2000) as u16);
        (hi as u32) << 16 | lo as u32
    }

    /// 2DA2:0421: the date at DS:`di` cut before " in the year" (the
    /// first "in th" after an `i`).
    fn text_cut_year(&mut self, di: u16) -> Option<()> {
        let mut di = di;
        let mut n = 0u32;
        loop {
            di = di.wrapping_add(1);
            // 0000:9BE8: `i` looked for up to the string's NUL.
            let len = self.text_len(DS, di).wrapping_add(1);
            let (d, found) = self.text_scan(DS, di, b'i', len);
            di = if found { d.wrapping_sub(1) } else { d };
            if self.mem.r32(DS, di.wrapping_add(1)) == 0x6874_206E {
                break;
            }
            // A date has a few `i`s; past them the original searches the
            // rest of DGROUP, string after string.
            n += 1;
            if n > 0x100 {
                return None;
            }
        }
        self.mem.w8(DS, di.wrapping_sub(1), 0);
        Some(())
    }

    /// A code's routine (its offset in 2DA2).
    fn text_sub(&mut self, files: &mut Files, hooks: &mut dyn TextHooks, h: u16, c: &mut Ctx) -> Option<Res> {
        let frame = c.es;
        Some(match h {
            // %a: the quest's reward (+18h).
            0x0179 => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x18));
                self.text_number(c, v)
            }
            // %adn, %amn: a title (by a roll on the seed at FDE, rotated,
            // or plus the time) " of " a name of set 9.
            0x0184 | 0x01F9 => {
                let fde = self.mem.r32(DS, 0x0FDE);
                let (s1, s2) = if h == 0x0184 {
                    (fde.rotate_left(4), fde.rotate_right(4))
                } else {
                    let v = fde.wrapping_add(self.mem.r32(DS, 0x05CC));
                    (v, v)
                };
                let saved = self.rng.seed;
                self.rng.seed = s1;
                let r = self.rng.next();
                self.rng.seed = saved;
                let si = self.skip_strings(DATA, 0x730F, r & 7);
                let (_, di) = self.text_copy(DATA, si, DS, 0x9B66)?;
                let (_, di) = self.text_copy(DS, 0x8A5B, DS, di.wrapping_sub(1))?;
                self.npc_name(files, 9, 0, s2, DS, di.wrapping_sub(1))?;
                Res::Str(DS, 0x9B66)
            }
            // %adv, %adj, %des, %pre, %suf: words by the quest's seed.
            0x01E7 => self.text_word(0x73B6)?,
            0x01F0 => self.text_word(0x73FF)?,
            0x0466 => self.text_word(0x7453)?,
            0x0BA6 => self.text_word(0x7499)?,
            0x0D21 => self.text_word(0x74E9)?,
            // %art: the artifact (FDD); %arc (1AA4); %apr (FE5), %mpr
            // (FE3): provinces.
            0x025E => Res::Str(DS, self.skip_strings(DS, 0x1CE4, self.t8(0x0FDD) as u16)),
            0x0277 => Res::Str(DS, self.skip_strings(DS, 0x35F0, self.t8(0x1AA4) as u16)),
            0x02D7 => Res::Str(DS, self.skip_strings(DS, 0x2F06, self.t8(0x0FE5) as u16)),
            0x07B9 => Res::Str(DS, self.skip_strings(DS, 0x2F06, self.t8(0x0FE3) as u16)),
            // %at: the quest's kind and its +31h.
            0x0290 => {
                let q = self.text_quest();
                let cx = ((self.t16(q.wrapping_add(4)) & 0xF000) >> 12).wrapping_sub(7) << 1;
                let cx = (cx & 0xFF00) | (cx as u8).wrapping_add(self.t8(q.wrapping_add(0x31))) as u16;
                Res::Str(DATA, self.skip_strings(DATA, 0x730F, cx))
            }
            // %an: a name of the province's race from the seed at FDE.
            0x02B3 => {
                let seed = self.mem.r32(DS, 0x0FDE).rotate_left(3);
                self.npc_name(files, self.t8(0x0A46), 0, seed, DS, 0x9B66)?;
                Res::Str(DS, 0x9B66)
            }
            // %ba: the quest's +1Ch.
            0x02E5 => {
                let q = self.text_quest();
                let v = self.t16(q.wrapping_add(0x1C)) as u32;
                self.text_number(c, v)
            }
            // %cn: the city's name (A22) in a city or building, else the
            // province's; %cn2: the city nearest the location (1194:0A44),
            // its province loaded (2E14), its square to 8A52/8A54. (The
            // RNG's seed is set from the location's and put back, nothing
            // rolling between.)
            0x02F1 => {
                if self.mem.r8(c.bp, c.rest) == b'2' {
                    c.dx = c.rest.wrapping_add(1);
                    let al = self.text_nearest_city(c.di) as u8;
                    let (province, ah) = (al / 0x20, (al % 0x20) as u16);
                    self.city_load_province(files, province)?;
                    let si = if ah >= 0x10 {
                        0x15D5 + (ah & 0xF) * 0x19
                    } else if ah >= 8 {
                        0x150D + (ah & 7) * 0x19
                    } else {
                        0x1445 + (ah & 7) * 0x19
                    };
                    self.mem.w16(DS, 0x8A52, self.t16(si + 0x14));
                    self.mem.w16(DS, 0x8A54, self.t16(si + 0x16));
                    return Some(Res::Str(DS, si));
                }
                let city = self.t16(0xC39A) & 0x200 == 0 && (self.t8(0x05E6) == 4 || self.t16(0xC398) & 4 != 0);
                if city {
                    Res::Str(DS, 0x0A22)
                } else {
                    Res::Str(DS, self.skip_strings(DS, 0x2F06, self.t8(0x0A46) as u16))
                }
            }
            // %ct: the kind of town; %cll, %cp, %ccs by F77.
            0x03C3 => Res::Str(DS, self.skip_strings(DS, 0x8A1B, self.t8(0x0A47) as u16)),
            0x03D1 => Res::Str(DS, self.skip_strings(DS, 0x81B9, self.t8(0x0F77) as u16)),
            0x03DF => {
                let cx = self.t8(0x80EC_u16.wrapping_add(self.t8(0x0F77) as u16)) as u16;
                Res::Str(DS, self.skip_strings(DS, 0x2F06, cx))
            }
            0x03F2 => Res::Str(DS, self.skip_strings(DS, 0x80F4, self.t8(0x0F77) as u16)),
            // %da: the quest's date (+0Ch), "in the year..." cut.
            0x0400 => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x0C));
                self.log_date(v, BOX);
                self.text_cut_year(BOX)?;
                Res::Str(DS, BOX)
            }
            // %de: the quest's days (+0Ch less +8, 16 bits) plus one.
            0x0446 => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x0C)).wrapping_sub(self.mem.r32(DS, q.wrapping_add(8)));
                let al = ((v as u16) / 0x5A0) as u8;
                self.text_number(c, al.wrapping_add(1) as u32)
            }
            // %di: the direction to (8A52, 8A54).
            0x046F => {
                let h = crate::travel::travel_heading(self.t16(0x8A52), self.t16(0x8A54), self.t16(0xC3D0) >> 7, self.t16(0xC3D4) >> 7);
                let cx = (h.wrapping_add(0x20) >> 6) % 8;
                Res::Str(DATA, self.skip_strings(DATA, 0x7566, cx))
            }
            // %dit: the informant's days left (1194:CFF7: 0FED less the whole
            // days since 0FE9) in decimal (0000:7052).
            0x04A9 => {
                let days = self.mem.r32(DS, 0x05CC).wrapping_sub(self.mem.r32(DS, 0x0FE9)) / 0x5A0;
                let ax = (self.t8(0x0FED) as u16).wrapping_sub(days as u16);
                self.text_utoa(ax, DS, 0x9B66);
                Res::Str(DS, 0x9B66)
            }
            // %dnl: the quest person's first name (2DA2:0866).
            0x04C5 => {
                self.text_quest_name(files, c)?;
                let len = self.text_len(frame, c.di);
                if len == 0 {
                    return None;
                }
                let (d, _) = self.text_scan(frame, c.di, b' ', len);
                self.mem.w8(frame, d.wrapping_sub(1), 0);
                if d == c.di {
                    return None;
                }
                Res::Buf(d)
            }
            // %doc: what the person does (texts 107h.., by CB5E).
            0x04E4 => {
                self.rng.seed = self.t16(0xCB5E) as u32;
                let r = self.rng.range(0, 0x64)?;
                self.rng.seed = self.t16(0x0151) as u32;
                let di = self.text_nested(files, hooks, c, r.wrapping_add(0x107))?;
                self.text_unbreak(frame, c.di);
                Res::Buf(di)
            }
            // %ds: the quest's monster's description.
            0x0513 => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x1E));
                self.rng.seed = v;
                self.mem.w32(DS, 0x8A57, v);
                self.text_copy(DATA, 0x7388, frame, c.di)?;
                let mut di = c.di;
                let roll = |m: &mut Machine| m.rng.next() % 100;
                let put = |m: &mut Machine, di: u16| {
                    for i in 0..4u16 {
                        let b = m.mem.r8(DATA, 0x7308 + i);
                        m.mem.w8(frame, di.wrapping_add(i), b);
                    }
                };
                if roll(self) >= 0x1E {
                    di = di.wrapping_add(0x16);
                } else {
                    let mut bx = 0;
                    if roll(self) & 1 != 0 {
                        bx = roll(self);
                    }
                    put(self, di);
                    if bx & 1 == 0 {
                        di = di.wrapping_add(5);
                        put(self, di);
                    }
                }
                di = self.text_scan(frame, di, b'p', 0xFFFF).0.wrapping_sub(2);
                if roll(self) >= 0x28 {
                    put(self, di);
                }
                di = self.text_scan(frame, di, b's', 0xFFFF).0.wrapping_sub(2);
                if roll(self) >= 0x14 {
                    put(self, di);
                }
                if di == c.di {
                    return None;
                }
                Res::Buf(di)
            }
            // %du, %ti: the quest's days (+0Ch less +8).
            0x058A => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x0C)).wrapping_sub(self.mem.r32(DS, q.wrapping_add(8)));
                self.text_number(c, v / 0x5A0)
            }
            // %ef: a first name from (8A52, 8A54).
            0x05A7 => {
                let seed = (self.t16(0x8A54) as u32) << 16 | self.t16(0x8A52) as u32;
                self.mem.w8(DATA, SEX, 0);
                self.npc_name(files, self.t8(0x0A46), 0, seed, frame, c.di)?;
                let d = self.text_first_name(frame, c.di, 1);
                if d == c.di {
                    return None;
                }
                Res::Buf(d)
            }
            // %en: an equipment store's name. In a building of kind 6
            // (D48) the place's entry (`place_lookup`, 3EB1: FD2 with its
            // low byte halved, among [138B] at [1008]); its first two
            // bytes (its key) go to 8A52 and 8A54 of the entry's segment
            // (DS is still the list's there), and its third byte picks the
            // name's first word (the list at 3276:3F71), its fourth the
            // second word (3276:403E), a space between. (No entry found:
            // the entry is read at offset 0.)
            // Otherwise the original leaves DS pushed (it pops it only on
            // that path) and its RET returns into the text.
            0x05DC => {
                if self.t8(0x0D48) != 6 {
                    return None;
                }
                let key = self.t16(0x0FD2);
                let key = (key & 0xFF00) | ((key & 0xFF) >> 1);
                let (off, seg) = (self.t16(0x1008), self.t16(0x100A));
                let si = self.place_lookup(seg, off, key, self.t16(0x138B));
                self.mem.w16(seg, 0x8A52, self.mem.r8(seg, si) as u16);
                self.mem.w16(seg, 0x8A54, self.mem.r8(seg, si.wrapping_add(1)) as u16);
                let (dl, dh) = (self.mem.r8(seg, si.wrapping_add(2)), self.mem.r8(seg, si.wrapping_add(3)));
                let si = self.skip_strings(DATA, 0x3F71, dl as u16);
                let (_, di) = self.text_copy(DATA, si, c.es, c.di)?;
                self.mem.w8(c.es, di.wrapping_sub(1), 0x20);
                let si = self.skip_strings(DATA, 0x403E, dh as u16);
                let (_, di) = self.text_copy(DATA, si, c.es, di)?;
                Res::Buf(di)
            }
            // %fn: the person's first name.
            0x066E => {
                let r = self.t16(self.t16(0xCB5E).wrapping_add(0x12)) & 0x7FFF;
                let ah = self.mem.r8(DATA, SEX);
                self.npc_name(files, self.t8(0x0A46), ah, r as u32, frame, c.di)?;
                let d = self.text_first_name(frame, c.di, 0);
                if d == c.di {
                    return None;
                }
                Res::Buf(d)
            }
            // %fq: the quest giver's first name.
            0x06A1 => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x1E));
                let (_, ah) = Self::text_titles(v as u8);
                self.mem.w8(DATA, SEX, ah);
                let al = self.t8(q.wrapping_add(0x25)) >> 5;
                self.npc_name(files, al, ah, v, frame, c.di)?;
                let d = self.text_first_name(frame, c.di, 0);
                if d == c.di {
                    return None;
                }
                Res::Buf(d)
            }
            // %g, %g2, %g3: he/she, his/her, him/her by SEX.
            0x06DA => {
                let ch = self.mem.r8(c.bp, c.rest);
                let si = match ch {
                    b'3' => {
                        c.dx = c.dx.wrapping_add(1);
                        0x755A
                    }
                    b'2' => {
                        c.dx = c.dx.wrapping_add(1);
                        0x754F
                    }
                    _ => 0x7545,
                };
                let cx = (self.mem.r8(DATA, SEX) as u16).min(2);
                Res::Str(DATA, self.skip_strings(DATA, si, cx))
            }
            // %hc, %qc, %tc: a city by its number (the player's home, 1AC;
            // the quest's +25h; its +24h): 1194:07F0.
            0x070D => self.text_city(files, self.t8(0x01AC))?,
            0x0C0C => {
                let q = self.text_quest();
                self.text_city(files, self.t8(q.wrapping_add(0x25)))?
            }
            0x0D7A => {
                let q = self.text_quest();
                self.text_city(files, self.t8(q.wrapping_add(0x24)))?
            }
            // %hod: the holiday (texts A9h..).
            0x071E => {
                let (mut cx, _, hit) = self.text_holiday(self.mem.r32(DS, 0x05CC), c.rest);
                if hit {
                    cx = cx.wrapping_add(1);
                    if cx > 0xE {
                        cx = 0;
                    }
                }
                let di = self.text_nested(files, hooks, c, 0xA9u16.wrapping_add(cx))?;
                self.text_unbreak(frame, c.di);
                Res::Buf(di)
            }
            // %jok: a joke (16Bh).
            0x075F => {
                self.rng.seed = self.t16(0x0151) as u32;
                let di = self.text_nested(files, hooks, c, 0x16B)?;
                self.text_unbreak(frame, c.di);
                Res::Buf(di)
            }
            // %lp: the province.
            0x0778 => Res::Str(DS, self.skip_strings(DS, 0x2F06, self.t8(0x0A46) as u16)),
            // %mi: the quest's item (+6).
            0x0786 => {
                let q = self.text_quest();
                Res::Str(DATA, self.skip_strings(DATA, 0x4A3B, self.t8(q.wrapping_add(6)) as u16))
            }
            // %mt: the quest's monster kind (25CA:0323: its +30h among the
            // creatures' names at 3276:465D; DS stays 3276).
            0x07E2 => {
                let q = self.text_quest();
                Res::Str(DATA, self.skip_strings(DATA, 0x465D, self.t8(q.wrapping_add(0x30)) as u16))
            }
            // %mn: the quest monster's name (set 9 + its +30h).
            0x079F => {
                let q = self.text_quest();
                let al = 9u8.wrapping_add(self.t8(q.wrapping_add(0x30)));
                self.mem.w8(DATA, SEX, 2);
                let di = self.npc_name(files, al, 0, self.mem.r32(DS, q), frame, c.di)?;
                Res::Buf(di)
            }
            // %mqt, %qt: the quest's kind of place.
            0x07C7 => {
                let q = self.text_quest();
                let cx = (self.t16(q.wrapping_add(4)) & 0xE00) >> 9;
                Res::Str(DATA, self.skip_strings(DATA, 0x75A4, cx))
            }
            // %n: the person's name (CB5E FFFFh: a name from (8A52, 8A54)).
            0x07F1 => {
                let bx = self.t16(0xCB5E);
                self.mem.w8(DATA, SEX, 0);
                let seed = if bx == 0xFFFF {
                    (self.t16(0x8A52) as u32) << 16 | self.t16(0x8A54) as u32
                } else {
                    if bx >= 0xFFEE {
                        return None;
                    }
                    if self.t16(bx + 0xF) & 0x1000 != 0 {
                        self.mem.w8(DATA, SEX, 1);
                    }
                    (self.t16(bx + 0x12) & 0x7FFF) as u32
                };
                let ah = self.mem.r8(DATA, SEX);
                let di = self.npc_name(files, self.t8(0x0A46), ah, seed, frame, c.di)?;
                Res::Buf(di)
            }
            // %nap, %oap: FEE in decimal.
            0x0840 | 0x0A8E => {
                for i in 0..6u16 {
                    self.mem.w8(DS, 0x9B66 + i, 0);
                }
                self.text_utoa(self.t16(0x0FEE), DS, 0x9B66);
                Res::Str(DS, 0x9B66)
            }
            // %nc: the quest person's name.
            0x0866 => {
                let di = self.text_quest_name(files, c)?;
                Res::Buf(di)
            }
            // %ncl: the person's class.
            0x088D => {
                let si = self.t16(self.t16(0xCB5E).wrapping_add(0x12));
                let cx = (self.t8(si.wrapping_add(5)) & 0x1F) as u16;
                Res::Str(DS, self.skip_strings(DS, 0x2A62, cx))
            }
            // %ne, %nr: the quest's other person.
            0x08A4 => {
                let q = self.text_quest();
                let al = self.t8(q.wrapping_add(0x25)) >> 5;
                let ah = self.t8(q.wrapping_add(0x28));
                let mut ebx = self.mem.r32(DS, q.wrapping_add(0x1E)).wrapping_add(self.mem.r32(DS, q.wrapping_add(8)));
                let bx = (ebx as u16).wrapping_add(self.t16(q.wrapping_add(6))).wrapping_add(self.t16(q.wrapping_add(0x26)));
                ebx = (ebx & 0xFFFF_0000) | bx as u32;
                self.mem.w8(DATA, SEX, ah);
                let di = self.npc_name(files, al, ah, ebx, frame, c.di)?;
                Res::Buf(di)
            }
            // %nh: the next holiday.
            0x08D1 => {
                let (mut cx, _, hit) = self.text_holiday(self.mem.r32(DS, 0x05CC), c.rest);
                if hit {
                    cx = cx.wrapping_add(1);
                    if cx > 0xE {
                        cx = 0;
                    }
                }
                Res::Str(DS, self.skip_strings(DS, 0x3261, cx))
            }
            // %nhd: the next holiday's date.
            0x08EE => {
                let (_, dx, _) = self.text_holiday(self.mem.r32(DS, 0x05CC).wrapping_add(0x5A0), c.rest);
                self.log_date(0x5A0 * dx as u32, BOX);
                self.text_cut_year(BOX)?;
                Res::Str(DS, BOX)
            }
            // %nt: a nearby place's name.
            0x094F => self.text_place(c)?,
            // %o: the quest's place kind (+22h), and 1248 set from where
            // the string was left (in 3276).
            0x0A72 => {
                let q = self.text_quest();
                let (res, si) = self.text_kind(c, self.t16(q.wrapping_add(0x22)))?;
                let ebx = self.mem.r32(DATA, si).wrapping_add(self.mem.r32(DATA, si.wrapping_add(8))).wrapping_add(self.mem.r32(DATA, si.wrapping_add(0x2A)));
                self.mem.w32(DATA, 0x1248, ebx);
                res
            }
            // %oc: the person's occupation (text 106h).
            0x0AB2 => {
                self.rng.seed = (self.t16(0xCB5E) & 0x7FFF) as u32;
                Res::Buf(self.text_nested(files, hooks, c, 0x106)?)
            }
            // %omq: the quest's +4 kind and +6.
            0x0AC8 => {
                let q = self.text_quest();
                let ax = ((self.t16(q.wrapping_add(4)) & 0xF000) >> 12) as u8 as u16 * 10;
                let cx = ax.wrapping_add(self.t8(q.wrapping_add(6)) as u16);
                Res::Str(DATA, self.skip_strings(DATA, 0x4A58, cx))
            }
            // %opp: the opponent (the creature record at 1042:0).
            0x0AF3 => {
                let seg = self.t16(0x1042);
                if self.mem.r16(seg, 0x40D) & 0x1000 == 0 {
                    let cx = (self.mem.r8(seg, 5) & 0x1F) as u16;
                    Res::Str(DS, self.skip_strings(DS, 0x2A62, cx))
                } else {
                    let cx = self.mem.r8(seg, 4) as u16;
                    Res::Str(DATA, self.skip_strings(DATA, 0x465D, cx))
                }
            }
            // %oth: an oath by the province (texts at 8A60).
            0x0B2B => {
                let saved = self.t8(NO_EXPAND);
                self.mem.w8(DS, NO_EXPAND, 0xFF);
                let mut bx = self.t8(0x0A46) as u16;
                // Where the port gives up the flag is put back too (as the
                // code's end does); left FFh, no later text would have its
                // substitutions made. The original has no such exit.
                if bx == 8 {
                    let Some(r) = self.rng.range(0, 8) else {
                        self.mem.w8(DS, NO_EXPAND, saved);
                        return None;
                    };
                    bx = r;
                }
                self.mem.w16(DS, TEXT, self.t16(0x8A60_u16.wrapping_add(bx * 2)));
                let r = self.text(files, hooks, 2, DS, 0x9B66);
                self.mem.w8(DS, NO_EXPAND, saved);
                r?;
                Res::Str(DS, 0x9B66)
            }
            // %pcf: the player's first name; %pcn: the player's name.
            0x0B7D => {
                self.mem.w8(DATA, SEX, self.t8(0x01AB));
                let mut si = 0x01ADu16;
                let mut di = c.di;
                loop {
                    let ch = self.t8(si);
                    si = si.wrapping_add(1);
                    if ch == b' ' || ch == 0 {
                        break;
                    }
                    self.mem.w8(frame, di, ch);
                    di = di.wrapping_add(1);
                }
                self.mem.w8(frame, di, 0);
                Res::Buf(di.wrapping_add(1))
            }
            0x0B99 => {
                self.mem.w8(DATA, SEX, self.t8(0x01AB));
                Res::Str(DS, 0x01AD)
            }
            // %qf (2DA2:0BAF) isn't here: it names a person through 1194:0807
            // with EBX as the seed and never sets it. BX is 8A56 (0 or 1)
            // by then (2DA2:1083), but EBX's high word is whatever code last
            // wrote it, often before the text was asked for, so it falls
            // to the `_` arm (None).
            // %qmn: the quest's monster kind (5E38 among C954).
            0x0C20 => {
                let al = self.t8(0x5E38);
                let (d, found) = self.text_scan(DS, 0xC954, al, 0x40);
                if !found {
                    Res::Str(DS, c.rest)
                } else {
                    let cx = d.wrapping_sub(0xC954);
                    Res::Str(DATA, self.skip_strings(DATA, 0x465D, cx))
                }
            }
            // %r: the quest's +27h (and SEX +28h).
            0x0C50 => {
                let q = self.text_quest();
                self.mem.w8(DATA, SEX, self.t8(q.wrapping_add(0x28)));
                Res::Str(DATA, self.skip_strings(DATA, 0x4931, self.t8(q.wrapping_add(0x27)) as u16))
            }
            // %ra: the player's race.
            0x0C72 => Res::Str(DS, self.skip_strings(DS, 0x2B94, self.t8(0x01A8) as u16)),
            // %rf: the ruler's first name (the Emperor's in the Imperial
            // province).
            0x0C80 => {
                if self.t8(0x0A46) == 8 {
                    let (_, di) = self.text_copy(DS, 0x8A0E, frame, c.di)?;
                    Res::Buf(di)
                } else {
                    let ah = (self.t8(0x0A52) & 3 == 0) as u8;
                    self.mem.w8(DATA, SEX, ah);
                    self.npc_name(files, self.t8(0x0A46), ah, self.mem.r32(DS, 0x0A52), frame, c.di)?;
                    let d = self.text_first_name(frame, c.di, 1);
                    if d == c.di {
                        return None;
                    }
                    Res::Buf(d)
                }
            }
            // %rcn: a random city of the province; %rpn: a random
            // province.
            0x0CC7 => {
                let r = self.rng.next() & 7;
                Res::Str(DS, 0x1445u16.wrapping_add(r * 0x19))
            }
            0x0CDA => {
                let r = self.rng.next() & 7;
                Res::Str(DS, self.skip_strings(DS, 0x2F06, r))
            }
            // %st: the city's... by its seed (A56). The seed pushed is popped
            // with DS at 3276 by then: it lands at 3276:91F6, and the rolled
            // seed stays (until the substitutions' end puts theirs back).
            0x0CED => {
                let saved = self.rng.seed;
                let eax = self.mem.r32(DS, 0x0A56);
                let al = (eax as u8).wrapping_add(self.t8(0x0A5A_u16.wrapping_add(self.t8(0x0A51) as u16))) & 7;
                self.rng.seed = (eax & 0xFFFF_FF00) | al as u32;
                let cx = self.rng.next() & 1;
                let res = Res::Str(DATA, self.skip_strings(DATA, 0x73AC, cx));
                self.mem.w32(DATA, 0x91F6, saved);
                res
            }
            // %sn: a name of set 15h from CB5E.
            0x0D2A => {
                let seed = (self.t16(0xCB5E) & 0x7FFF) as u32;
                self.npc_name(files, 0x15, 0, seed, frame, c.di)?;
                Res::Str(frame, c.di)
            }
            // %t: the ruler's title.
            0x0D47 => {
                let saved = self.rng.seed;
                self.rng.seed = self.mem.r32(DS, 0x0A52);
                let (si, ah) = Self::text_titles(self.rng.seed as u8);
                self.mem.w8(DATA, SEX, ah);
                let res = self.text_title(self.t8(0x0A47), si)?;
                self.rng.seed = saved;
                res
            }
            // %tan: 6D49.
            0x0D68 => Res::Str(DS, 0x6D49),
            // %tem: a temple's name (in a city by 3EB1, not ported).
            0x0D8C => self.text_temple(c)?,
            // %tg: the quest's target kind (+2Eh); 1248 (in 3276) cleared.
            0x0E9E => {
                let q = self.text_quest();
                let (res, _) = self.text_kind(c, self.t8(q.wrapping_add(0x2E)) as u16)?;
                self.mem.w32(DATA, 0x1248, 0);
                res
            }
            // %tl: the quest's place.
            0x0EB4 => self.text_quest_place(c)?,
            // %tq: the quest giver's title.
            0x0FF7 => {
                let q = self.text_quest();
                let v = self.mem.r32(DS, q.wrapping_add(0x1E));
                self.rng.seed = v;
                let (si, ah) = Self::text_titles(v as u8);
                self.mem.w8(DATA, SEX, ah);
                let mut al = (self.t8(q.wrapping_add(0x25)) & 0x1F) >> 3;
                if al == 3 {
                    al = 2;
                }
                self.text_title(al, si)?
            }
            // %tt: the quest's +2Fh less 18h, a class.
            0x1029 => {
                let q = self.text_quest();
                let cx = (self.t8(q.wrapping_add(0x2F)) as u16).wrapping_sub(0x18);
                Res::Str(DS, self.skip_strings(DS, 0x2A62, cx))
            }
            // Any other code: nothing, the code's last letter kept.
            0x103C => {
                c.dx = c.dx.wrapping_sub(1);
                Res::Str(DS, 0x8A44)
            }
            _ => return None,
        })
    }

    /// 2DA2:0866: the quest person's name into the buffer; DI after.
    fn text_quest_name(&mut self, files: &mut Files, c: &Ctx) -> Option<u16> {
        let q = self.text_quest();
        let al = self.t8(q.wrapping_add(0x25)) >> 5;
        let ebx = self.mem.r32(DS, q.wrapping_add(0x1E)).wrapping_add(self.mem.r32(DS, q.wrapping_add(8)));
        let bx = (ebx as u16).wrapping_add(self.t16(q.wrapping_add(6)));
        let bx = (bx & 0xFF00) | (bx as u8).wrapping_add(self.t8(q.wrapping_add(0x2F))) as u16;
        let ebx = (ebx & 0xFFFF_0000) | bx as u32;
        self.mem.w8(DATA, SEX, 0);
        self.npc_name(files, al, 0, ebx, c.es, c.di)
    }

    /// 1194:07F0: city `al` (its province `al >> 5` loaded, 1194:2E18;
    /// its number `al & 1Fh` among the 19h-byte records from 1445).
    fn text_city(&mut self, files: &mut Files, al: u8) -> Option<Res> {
        self.city_load_province(files, al >> 5)?;
        Some(Res::Str(DS, 0x1445 + (al & 0x1F) as u16 * 0x19))
    }

    /// 1194:0A44. The city of the location's kind (A47: 0 the city-states
    /// from 1445, 1 the towns from 150D, else the villages from 15D5)
    /// nearest the location's square (A48, A4A; the larger axis plus a
    /// quarter of the smaller, 0 not counted), as province * 20h + its
    /// number; none nearer than 2710h leaves `di` (the caller's DI).
    fn text_nearest_city(&self, mut di: u16) -> u16 {
        let (mut bp, mut si, cx) = match self.t8(0x0A47) {
            0 => (0u16, 0x1445u16, 8),
            1 => (8, 0x150D, 8),
            _ => (0x10, 0x15D5, 0x10),
        };
        let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
        let mut dx = 0x2710u16;
        for _ in 0..cx {
            let a = abs(self.t16(0x0A48).wrapping_sub(self.t16(si.wrapping_add(0x14))));
            let b = abs(self.t16(0x0A4A).wrapping_sub(self.t16(si.wrapping_add(0x16))));
            let (lo, hi) = if a > b { (b, a) } else { (a, b) };
            let d = (lo >> 2).wrapping_add(hi);
            if d != 0 && dx > d {
                dx = d;
                di = bp;
            }
            si = si.wrapping_add(0x19);
            bp = bp.wrapping_add(1);
        }
        di.wrapping_add((self.t8(0x0A46) as u16) << 5)
    }

    /// 2DA2:094F, %nt: a place's name: in the wild (or a dungeon with
    /// 5C8 bit 4000h) one made from the square's seed, else the quest's
    /// or a random one of the province's; then its region's suffix.
    fn text_place(&mut self, c: &Ctx) -> Option<Res> {
        let mut si = c.rest;
        let (dx, ax) = 'p: {
            let (mut cx, mut dx) = (self.t16(0xC3D0), self.t16(0xC3D4));
            let ab2 = self.t8(0x0AB2);
            if ab2 != 0 {
                let wild = self.t16(0xC39A) & 0x200 != 0;
                if !wild {
                    if self.t8(0x05E6) == 4 {
                        if self.t16(0x05C8) & 0x4000 == 0 {
                            // In a building: its entry among the places at
                            // [1AB5] (3EB1, the key FD2 with its low byte
                            // halved); none found reads offset 0.
                            let key = self.t16(0x0FD2);
                            let key = (key & 0xFF00) | ((key & 0xFF) >> 1);
                            let (off, seg) = (self.t16(0x1AB5), self.t16(0x1AB7));
                            let si = self.place_lookup(seg, off, key, self.t16(0x1387));
                            break 'p (self.mem.r16(seg, si.wrapping_add(2)), self.t8(0x0A4C));
                        }
                        cx = self.t16(0x0610);
                        dx = self.t16(0x0612);
                    } else if self.t8(0x8A56) == 1 {
                        let r = self.rng.range(0, self.t16(0x1387))?;
                        let (off, seg) = (self.t16(0x1AB5), self.t16(0x1AB7));
                        let s = off.wrapping_add(r << 2);
                        break 'p (self.mem.r16(seg, s.wrapping_add(2)), self.t8(0x0A4C));
                    } else if ab2 != 0xFF {
                        break 'p (self.t16(0x0FE0), self.t8(0x0A4C));
                    } else {
                        si = self.t16(0x0F72);
                        break 'p (self.t16(si.wrapping_add(0x12)), self.t8(si.wrapping_add(0x24)));
                    }
                }
                self.rng.seed = self.text_wild_seed(cx, dx);
                let dl = self.rng.range(0, 0x17)? as u8;
                let dh = self.rng.range(0, 0x17)? as u8;
                break 'p ((dh as u16) << 8 | dl as u16, 0xFF);
            }
            (self.t16(0x0FE0), self.t8(0x0A4C))
        };
        let _ = si;
        let mut bp = 0x3DF7u16;
        if self.t8(0x0A46) != 8 {
            bp = if self.text_scan(DS, 0x47E2, ax, 0x3B).1 { 0x3D5D } else { 0x3DF7 };
        }
        let mut di = c.di;
        let mut s = self.skip_strings(DATA, 0x3CB6, dx & 0xFF);
        loop {
            let ch = self.mem.r8(DATA, s);
            s = s.wrapping_add(1);
            if ch == 0 {
                break;
            }
            self.mem.w8(c.es, di, ch);
            di = di.wrapping_add(1);
        }
        self.mem.w8(c.es, di, b' ');
        di = di.wrapping_add(1);
        let (_, di) = self.text_copy(DATA, self.skip_strings(DATA, bp, dx >> 8), c.es, di)?;
        Some(Res::Buf(di))
    }

    /// 2DA2:0D8C, %tem: a temple's name: in the wild (or a dungeon with
    /// 5C8 bit 4000h) one made from the square's seed; elsewhere the
    /// place's entry among the temples at [1004] (3EB1, the key FD2 with
    /// its low byte halved), or when there is none a random one: the
    /// seed the person's (CB5E, 15 bits), the time's (5CC >> 4) when that
    /// is 0, 8A57 when set (then cleared to FFFFFFFFh); the entry's +2
    /// and +3 pick the name's words.
    fn text_temple(&mut self, c: &Ctx) -> Option<Res> {
        let (mut cx, mut dx) = (self.t16(0xC3D0), self.t16(0xC3D4));
        if self.t16(0xC39A) & 0x200 == 0 {
            if self.t8(0x05E6) != 4 || self.t16(0x05C8) & 0x4000 == 0 {
                let key = self.t16(0x0FD2);
                let key = (key & 0xFF00) | ((key & 0xFF) >> 1);
                let (off, seg) = (self.t16(0x1004), self.t16(0x1006));
                let n = self.t16(0x1389);
                let mut si = self.place_lookup(seg, off, key, n);
                if si == 0 {
                    let mut eax = (self.t16(0xCB5E) & 0x7FFF) as u32;
                    if eax == 0 {
                        eax = self.mem.r32(DS, 0x05CC) >> 4;
                    }
                    if self.mem.r32(DS, 0x8A57) != 0xFFFF_FFFF {
                        eax = self.mem.r32(DS, 0x8A57);
                        self.mem.w32(DS, 0x8A57, 0xFFFF_FFFF);
                    }
                    self.rng.seed = eax;
                    let r = self.rng.range(0, n)?;
                    si = off.wrapping_add((r & 0xFF) * 4);
                }
                let dx = self.mem.r16(seg, si.wrapping_add(2));
                return self.text_temple_name(c, dx & 0xFF, dx >> 8);
            }
            cx = self.t16(0x0610);
            dx = self.t16(0x0612);
        }
        self.rng.seed = self.text_wild_seed(cx, dx);
        let dl = self.rng.range(0, 3)?;
        let dh = self.rng.range(0, 5)?;
        self.text_temple_name(c, dl, dh)
    }

    /// 2DA2:0E5C: a temple's name: 3E81's `dl`th, then by `dl` a list
    /// (3EAC, 3EE9, 3F28; past them from where the first string ended),
    /// its `dh`th appended.
    fn text_temple_name(&mut self, c: &Ctx, dl: u16, dh: u16) -> Option<Res> {
        let si = self.skip_strings(DATA, 0x3E81, dl);
        let (mut si, di) = self.text_copy(DATA, si, c.es, c.di)?;
        let di = di.wrapping_sub(1);
        match dl {
            0 => si = 0x3EAC,
            1 => si = 0x3EE9,
            2 => si = 0x3F28,
            _ => {}
        }
        let si = self.skip_strings(DATA, si, dh);
        let (_, di) = self.text_copy(DATA, si, c.es, di)?;
        Some(Res::Buf(di))
    }

    /// 2DA2:0EB4, %tl: the quest's place by its kind (+4 bits 12..15):
    /// a named place with its region's suffix (0), a city (1), a temple
    /// (3), the kind's word (2, 6, 7); 8A52/8A54 its square, 1244 the
    /// record.
    fn text_quest_place(&mut self, c: &mut Ctx) -> Option<Res> {
        let q = self.text_quest();
        let kind = (self.t16(q.wrapping_add(4)) & 0xF000) >> 12;
        self.mem.w16(DS, 0x8A52, self.t8(q.wrapping_add(0x14)) as u16);
        self.mem.w16(DS, 0x8A54, self.t8(q.wrapping_add(0x15)) as u16);
        let dx = self.t16(q.wrapping_add(0x16));
        self.mem.w16(DS, 0x1244, q);
        let (dl, dh) = (dx & 0xFF, dx >> 8);
        Some(match kind {
            0 | 1 => {
                let (first, second) = if kind == 0 { (0x3CB6, None) } else { (0x3F71, Some(0x403E)) };
                let si = self.skip_strings(DATA, first, dl);
                let (_, di) = self.text_copy(DATA, si, c.es, c.di)?;
                self.mem.w8(c.es, di.wrapping_sub(1), b' ');
                self.mem.w8(c.es, di, 0);
                let list = second.unwrap_or_else(|| {
                    let al = self.t8(self.t16(0x1244).wrapping_add(0x24));
                    if self.text_scan(DS, 0x47E2, al, 0x3A).1 { 0x3D5D } else { 0x3DF7 }
                });
                let si = self.skip_strings(DATA, list, dh);
                Res::Buf(self.text_cat(DATA, si, c.es, di)?)
            }
            2 => Res::Str(DATA, self.skip_strings(DATA, 0x7615, 2)),
            3 => {
                let si = self.skip_strings(DATA, 0x3E81, dl);
                let (mut si, di) = self.text_copy(DATA, si, c.es, c.di)?;
                let di = di.wrapping_sub(1);
                match dl {
                    0 => si = 0x3EAC,
                    1 => si = 0x3EE9,
                    2 => si = 0x3F28,
                    _ => {}
                }
                let si = self.skip_strings(DATA, si, dh);
                Res::Buf(self.text_cat(DATA, si, c.es, di)?)
            }
            // (DS is 3276 by now: the word read is 3276:0F76, not the
            // quest record's.)
            6 => {
                if (self.mem.r16(DATA, 0x0F76) & 0xE00) >> 9 == 0 {
                    Res::Str(DATA, 0x72F0)
                } else {
                    Res::Str(DATA, self.skip_strings(DATA, 0x730F, 6))
                }
            }
            7 => Res::Str(DATA, 0x7301),
            // The other code's routine, with DS at 3276 by now.
            _ => {
                c.dx = c.dx.wrapping_sub(1);
                Res::Str(DATA, 0x8A44)
            }
        })
    }
}
