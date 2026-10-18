//! The logbook's screen as the game's state sees it (L or the scroll's
//! button, 2542:02A3, around 27E3:0088).
//!
//! - Its start (2542:02A3..02FD): the colours read to 9A6E (0000:6E3C),
//!   the page written to SCREENSV.$$2 (A69E), bank [1032] mapped, the clip
//!   the whole screen (9A98), the colours black and the page cleared
//!   (1194:55B2), LOGBOOK.IMG loaded into the frame and drawn (0000:56C8
//!   with BX 4), the mouse read (8FC4) and its cursor drawn (77EC); the
//!   bank before mapped again (and the one that leaves, [1032], kept for
//!   the end).
//! - The log (27E3:0088..0136): bank [1030] mapped, ARENAFNT and the
//!   tiles [1034] selected, the slot's name "log.nn" made (01E3) and its
//!   path built; with no file (DOS find first, 0000:600C) or an empty one,
//!   "Your logbook is empty." centred on (A0h, 64h) in FONT_A, 61h. Else
//!   the frame cleared (FA00h bytes) and the file read into it (7428), its
//!   size at 7906, and unless its first byte is a NUL: 137E 13 (the lines
//!   a page), each " *" made LF CR (0147), the lines split at their LFs
//!   (1194:4E54: NULs, their count at FF5, a page each 13; bug kept: FF5
//!   written with DS the frame's, into bank [1030]), then the pages (02DA).
//! - The pages (02DA): SP kept at 7921, A872 the lines' count (one more
//!   than the LFs), A87E the lines on the page. Each page: the page as it
//!   is written to LOG.$$$ (03D6), the wait for the buttons' release
//!   (1194:5E21: AFF9 0), up to 13 lines: each copied to 1044 (9CA0), one
//!   with '&' in its first three bytes a date in colour 10h after the '&'
//!   8 rows lower and the next line 16 rows on, the others in 49h, drawn
//!   in TEENYFNT ([102E]) at x 1Eh from row 1Eh (9E06). A full page with
//!   more than one line left gets "More..." at (10Eh, AAh) in 0Fh and the
//!   loop's turn (01FC): a click off the buttons the next page (A902 bit 0
//!   off, the page read back from LOG.$$$, 0400), Exit or Print the pages'
//!   end. Bug kept: the pages' end is only theirs, and the loop after them
//!   (0137) asks again, so Exit on a page before the last takes two
//!   clicks.
//! - The loop's turn (01FC): the cursor put back (541E), the mouse read
//!   into AFF9 or'd with what it held (bug kept: once a button is seen,
//!   AFF9 stays set until a release wait, so after a click off the
//!   buttons on the last page the cursor alone, moved over Exit or Print,
//!   is a click there), its place (77EC: 8F92, 8F94) and the cursor drawn
//!   (A902 bit 0); Esc down (ACB6) or the CyberMan's button 20h leave.
//!   On the bottom row (B4h..C3h): Print (25h..4Ch), Exit (107h..127h).
//! - Print (0272): nothing with no text in the frame; else each of the
//!   [FF5] lines (DS the frame's: what 4E54 left there) written to PRN
//!   (handle 4, 1194:5187) with a CR LF, then a form feed (78D4): the
//!   screen left. A write that falls short: the page read back, "Error
//!   while printing...." centred on (A0h, 50h) in FONT_A, 10h, and the
//!   wait for a key (1194:5532, the front end's); the screen left too.
//! - The end (27E3:013C, 2542:0302..034C): bank [1030]'s before mapped
//!   again, then the bank 2542 kept, 929E cleared, the colours set from it
//!   (9B4E), SCREENSV.$$2 read back (A7A0), the clip the whole screen and
//!   0D39 bit 80h: the interface made again.
//!
//! The SP kept at 12C6 is the one the original leaves there by its way
//! in: the menu key's (L) or the bar's scroll button's, a word deeper;
//! the pages' at 7921 is left out (the port has no stack).
//! The pixels (drawn only where the page is the game's own heap block,
//! `Machine::page_is_block`), the fades (5E3E), the cursor's drawing and
//! the page's showing (9466) are the front end's.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The log's size as read (7906).
    pub const SIZE: u16 = 0x7906;
    /// The lines a page (137E: 13).
    pub const PER_PAGE: u16 = 0x137E;
    /// The lines' count and those on the page (02DA).
    pub const LINES: u16 = 0xA872;
    pub const ON_PAGE: u16 = 0xA87E;
    /// The lines' count as 1194:4E54 writes it, DS the frame's.
    pub const COUNT: u16 = 0x0FF5;
    /// "More...", LOG.$$$, "Your logbook is empty.", "Error while
    /// printing....", the form feed and LOGBOOK.IMG.
    pub const MORE: u16 = 0x78DF;
    pub const PAGE_FILE: u16 = 0x78E7;
    pub const EMPTY: u16 = 0x78EF;
    pub const PRINT_ERROR: u16 = 0x7908;
    pub const FORM_FEED: u16 = 0x78D4;
    pub const PICTURE: u16 = 0x5CA2;
}

/// The SP 27E3:009C keeps.
const LOG_SP: u16 = 0x12C6;
/// The colours' copy (0000:6E3C at 2542:02A7) and the black ones.
const PALETTE: u16 = 0x9A6E;
const SCRATCH: u16 = 0x929E;
/// The line drawn (9CA0's copy).
const LINE: u16 = 0x1044;
/// The EMS frame's segment.
const FRAME: u16 = 0x9030;
/// The mouse: its buttons, place, the cursor drawn (A902 bit 0), Esc down.
const BUTTONS: u16 = 0xAFF9;
const MOUSE_X: u16 = 0x8F92;
const MOUSE_Y: u16 = 0x8F94;
const ESC_DOWN: u16 = 0xACB6;
/// 1194:5187's CR LF (in 1194's code).
const CRLF: (u16, u16) = (0x1194, 0x54D2);

/// Where the screen's loop is: in the pages' (02DA) at line `si` of the
/// frame with `lines` left and `pages` to go, or in the one after them
/// (0137).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum At {
    Pages { si: u16, lines: u16, pages: u16 },
    After,
}

/// A logbook screen's run, between `log_open` and `log_close`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogRun {
    /// The bank mapped before 27E3:0088's (put back at its end) and the
    /// one 2542:02FC's mapping left (put back after).
    inner: u16,
    outer: u16,
    at: At,
}

impl LogRun {
    /// Whether "More..." is up: the pages' loop waiting.
    pub fn paging(&self) -> bool {
        matches!(self.at, At::Pages { .. })
    }
}

/// What a turn of the loop came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogTurn {
    /// Still waiting (nothing pressed, a click off the buttons on the last
    /// page, Print with nothing to print).
    Waiting,
    /// The next page shown.
    Page,
    /// The pages' loop ended (Exit, Print or Esc on a page before the
    /// last): the loop after them waits.
    PagesEnded,
    /// The screen left (`log_close` next).
    Left,
    /// Print fell short: its message is up, the front end's wait for a key
    /// (1194:5532); then, `true`, the screen left (`log_close` next), or
    /// the pages' loop ended (the loop after them waits).
    PrintFailed(bool),
}

impl Machine {
    /// 2542:02A3..02FD and 27E3:0088 up to its loop's first wait: the
    /// logbook opened (`dac` the VGA's colours, 6-bit, for 9A6E, None
    /// leaves them; `buttons` the mouse's as 8FC4 reads them then;
    /// `from_bar` by the bar's scroll button, not the L key). None where
    /// LOGBOOK.IMG can't be read.
    pub fn log_open(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>, buttons: u8, from_bar: bool) -> Option<LogRun> {
        for (i, c) in dac.iter().flat_map(|d| d.iter()).enumerate() {
            for (k, &v) in c.iter().enumerate() {
                self.mem.w8(DS, PALETTE.wrapping_add((i * 3 + k) as u16), v);
            }
        }
        self.screen_save(files);
        let bank = self.mem.r16(DS, 0x1032);
        let before = self.map_bank(bank, 0);
        self.clip_whole();
        self.sheet_black();
        self.cursor_hidden();
        self.sheet_image_here(files, var::PICTURE)?;
        // 8FC4, 77EC: the mouse read and the cursor drawn.
        self.mem.w8(DS, BUTTONS, buttons);
        self.cursor_drawn();
        let outer = self.map_bank(before, 0);
        // 27E3:0088.
        self.cursor_hidden();
        let bank = self.mem.r16(DS, 0x1030);
        let inner = self.map_bank(bank, 0);
        // 009C: SP kept at 12C6 for the way out (a word the shops use for
        // their own after): the world loop's stack as the frame's menu
        // keys call 2542:02A3 (measured: 5FD0h), or as the bar's click
        // calls it through 06CE's table (5FCEh).
        self.mem.w16(DS, LOG_SP, if from_bar { 0x5FCE } else { 0x5FD0 });
        self.select_font(0x102C);
        self.select_tiles(0x1034);
        let mut run = LogRun { inner, outer, at: At::After };
        self.log_name_made();
        let path = self.save_path(crate::log::NAME);
        if files.found_size(self.mem.cstr(DS, path)).is_none() {
            self.log_empty();
            return Some(run);
        }
        let frame = self.mem.r16(DS, FRAME);
        for i in 0..0xFA00u16 {
            self.mem.w8(frame, i, 0);
        }
        let size = match self.file_load_or_carry(files, path, frame, 0)? {
            Some(_) => self.mem.r16(DS, crate::bsa::var::OPEN_SIZE),
            None => 0,
        };
        self.mem.w16(DS, var::SIZE, size);
        if size == 0 {
            self.log_empty();
            return Some(run);
        }
        if self.mem.r8(frame, 0) == 0 {
            return Some(run);
        }
        self.mem.w8(DS, var::PER_PAGE, 13);
        self.log_breaks(frame);
        let (lines, pages) = self.log_split(frame);
        self.cursor_hidden();
        self.mem.w16(DS, var::LINES, lines);
        self.mem.w16(DS, var::ON_PAGE, 0);
        run.at = self.log_pages(files, 0, lines, pages);
        Some(run)
    }

    /// 27E3:01E3 (the slot's digits after the name's '.').
    fn log_name_made(&mut self) {
        let mut di = crate::log::NAME;
        for _ in 0..10 {
            let c = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if c == b'.' {
                break;
            }
        }
        let slot = self.mem.r8(DS, 0x1A6C);
        self.mem.w8(DS, di, b'0' + slot / 10);
        self.mem.w8(DS, di.wrapping_add(1), b'0' + slot % 10);
    }

    /// 27E3:0117..0132: "Your logbook is empty." centred on (A0h, 64h) in
    /// FONT_A ([1026]), colour 61h (0000:5364).
    fn log_empty(&mut self) {
        self.select_font(0x1026);
        self.mem.w8(DS, 0x9288, 0x61);
        self.log_centred(var::EMPTY, 0xA0, 0x64);
    }

    /// 0000:5364: the text at DS:`si` centred on x `ax` at row `bx`.
    fn log_centred(&mut self, si: u16, ax: u16, bx: u16) {
        let x = ax.wrapping_sub(self.text_width(si) >> 1);
        self.page_text_at(x, bx, (DS, si));
    }

    /// 27E3:0147 with DS the frame's, SI 0: a NUL at the size (ES:7906),
    /// then each '*' a CR and the byte before it a LF.
    fn log_breaks(&mut self, frame: u16) {
        let size = self.mem.r16(DS, var::SIZE);
        self.mem.w8(frame, size, 0);
        let mut si = 0u16;
        loop {
            let c = self.mem.r8(frame, si);
            if c == 0 {
                break;
            }
            if c == b'*' {
                self.mem.w8(frame, si.wrapping_sub(1), b'\n');
                self.mem.w8(frame, si, b'\r');
            }
            si = si.wrapping_add(1);
        }
    }

    /// 1194:4E54 with DS the frame's: each LF a NUL, counted, a page each
    /// [137E] (ES DGROUP's); the count plus one written to DS:FF5 (the
    /// frame's). Returns (CX, BX): the lines and the pages.
    fn log_split(&mut self, frame: u16) -> (u16, u16) {
        let per = self.mem.r8(DS, var::PER_PAGE);
        let (mut cx, mut dl, mut bx) = (0u16, 0u8, 1u16);
        let mut si = 0u16;
        loop {
            let c = self.mem.r8(frame, si);
            if c == 0 {
                break;
            }
            if c == b'\n' {
                self.mem.w8(frame, si, 0);
                cx = cx.wrapping_add(1);
                dl = dl.wrapping_add(1);
                if dl == per {
                    bx = bx.wrapping_add(1);
                    dl = 0;
                }
            }
            si = si.wrapping_add(1);
        }
        cx = cx.wrapping_add(1);
        self.mem.w16(frame, var::COUNT, cx);
        (cx, bx)
    }

    /// 27E3:02F3..03C7 from the top of a page: the page written to
    /// LOG.$$$ (03D6), the release wait (AFF9 0), the page's lines drawn;
    /// where the loop goes next.
    fn log_pages(&mut self, files: &mut Files, mut si: u16, mut lines: u16, pages: u16) -> At {
        let path = self.save_path(var::PAGE_FILE);
        let page = self.mem.r16(DS, 0xA900);
        self.file_write(files, path, page, 0, 0xFA00);
        self.mem.w8(DS, BUTTONS, 0);
        let frame = self.mem.r16(DS, FRAME);
        let mut y = 0x1Eu16;
        loop {
            if self.mem.r16(DS, var::ON_PAGE) >= 13 {
                // 0382: a full page; "More..." unless one line or none is left.
                if lines <= 1 {
                    return At::After;
                }
                break;
            }
            let n = self.mem.r16(DS, var::ON_PAGE).wrapping_add(1);
            self.mem.w16(DS, var::ON_PAGE, n);
            self.select_font(0x102E);
            self.mem.w8(DS, 0x9288, 0x49);
            let mut di = LINE;
            loop {
                let c = self.mem.r8(frame, si);
                self.mem.w8(DS, di, c);
                si = si.wrapping_add(1);
                di = di.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
            let mut at = LINE;
            let date = if self.mem.r8(DS, at) == b'&' {
                true
            } else if self.mem.r8(DS, at.wrapping_add(1)) == b'&' {
                at = at.wrapping_add(1);
                true
            } else if self.mem.r8(DS, at.wrapping_add(2)) == b'&' {
                at = at.wrapping_add(2);
                true
            } else {
                false
            };
            let mut row = y;
            if date {
                self.mem.w8(DS, 0x9288, 0x10);
                at = at.wrapping_add(1);
                row = row.wrapping_add(8);
            }
            self.page_text_at(0x1E, row, (DS, at));
            y = y.wrapping_add(8);
            if self.mem.r8(DS, 0x9288) == 0x10 {
                y = y.wrapping_add(8);
            }
            lines = lines.wrapping_sub(1);
            if lines == 0 {
                // 037F: the lines run out; "More..." where more than two
                // pages were still to go.
                if pages.wrapping_sub(1) <= 1 {
                    return At::After;
                }
                lines = pages.wrapping_sub(1);
                break;
            }
        }
        self.mem.w8(DS, 0x9288, 0x0F);
        self.page_text_at(0x10E, 0xAA, (DS, var::MORE));
        At::Pages { si, lines, pages }
    }

    /// The loop's turn (27E3:01FC) on the mouse read: `buttons` (AFF9's
    /// new bits) at (`x`, `y`), `esc` whether Esc is down (ACB6). Returns
    /// what came of it; after `LogTurn::Left` or `PrintFailed(true)`,
    /// `log_close`.
    pub fn log_turn(&mut self, files: &mut Files, run: &mut LogRun, buttons: u8, x: u16, y: u16, esc: bool) -> LogTurn {
        let done = self.log_poll(files, buttons, x, y, esc);
        match (run.at, done) {
            (_, Poll::Waiting) => LogTurn::Waiting,
            (At::Pages { si, lines, pages }, Poll::Next) => {
                // 03AB: the cursor taken as gone, the page read back.
                let f = self.mem.r16(DS, 0xA902) & 0xFFFE;
                self.mem.w16(DS, 0xA902, f);
                self.cursor_hidden();
                self.log_page_back(files);
                self.mem.w16(DS, var::ON_PAGE, 0);
                let pages = pages.wrapping_sub(1);
                if pages == 0 {
                    run.at = At::After;
                    self.mem.w8(DS, BUTTONS, 0);
                    return LogTurn::PagesEnded;
                }
                run.at = self.log_pages(files, si, lines, pages);
                LogTurn::Page
            }
            (At::Pages { .. }, Poll::Print(false)) => {
                run.at = At::After;
                self.mem.w8(DS, BUTTONS, 0);
                LogTurn::PrintFailed(false)
            }
            (At::Pages { .. }, _) => {
                // 03CB: the release wait, the pages' loop left.
                run.at = At::After;
                self.mem.w8(DS, BUTTONS, 0);
                if esc {
                    // Esc still down: the loop after them leaves at once.
                    return self.log_turn(files, run, 0, x, y, esc);
                }
                LogTurn::PagesEnded
            }
            (At::After, Poll::Next) => LogTurn::Waiting,
            (At::After, Poll::Print(false)) => LogTurn::PrintFailed(true),
            (At::After, _) => LogTurn::Left,
        }
    }

    /// Whether AFF9 holds a button (the next turn takes the cursor's place
    /// as a click: see the module's notes).
    pub fn log_buttons_held(&self) -> bool {
        self.mem.r8(DS, BUTTONS) != 0
    }

    /// 27E3:01FC's body for one read of the mouse.
    fn log_poll(&mut self, files: &mut Files, buttons: u8, x: u16, y: u16, esc: bool) -> Poll {
        self.cursor_hidden();
        let held = self.mem.r8(DS, BUTTONS) | buttons;
        self.mem.w8(DS, BUTTONS, held);
        self.mem.w16(DS, MOUSE_X, x);
        self.mem.w16(DS, MOUSE_Y, y);
        self.cursor_drawn();
        if esc || self.mem.r8(DS, ESC_DOWN) != 0 {
            return Poll::Exit;
        }
        if held == 0 {
            return Poll::Waiting;
        }
        if !(0xB4..=0xC3).contains(&y) || !(0x25..=0x127).contains(&x) {
            return Poll::Next;
        }
        if x <= 0x4C {
            return match self.log_print(files) {
                Some(ok) => Poll::Print(ok),
                None => Poll::Next,
            };
        }
        if x >= 0x107 {
            return Poll::Exit;
        }
        Poll::Next
    }

    /// 27E3:0272..02D6: Print. None with no text in the frame (the turn
    /// a click off the buttons); else whether every write went in (the
    /// failed one's message drawn).
    fn log_print(&mut self, files: &mut Files) -> Option<bool> {
        let frame = self.mem.r16(DS, FRAME);
        if self.mem.r8(frame, 0) == 0 {
            return None;
        }
        let count = self.mem.r16(frame, var::COUNT);
        let mut si = 0u16;
        for _ in 0..count {
            if !self.log_print_line(files, (frame, si)) {
                self.log_print_failed(files);
                return Some(false);
            }
            while self.mem.r8(frame, si) != 0 {
                si = si.wrapping_add(1);
            }
            si = si.wrapping_add(1);
        }
        if !self.log_print_line(files, (DS, var::FORM_FEED)) {
            self.log_print_failed(files);
            return Some(false);
        }
        Some(true)
    }

    /// 1194:5187: the text at `seg`:`si` written to handle 4 (PRN), then
    /// CR LF; whether both writes took all their bytes.
    fn log_print_line(&mut self, files: &mut Files, (seg, si): (u16, u16)) -> bool {
        let mut end = si;
        while self.mem.r8(seg, end) != 0 {
            end = end.wrapping_add(1);
        }
        let n = end.wrapping_sub(si);
        self.file_put_on(files, 4, seg, si, n) == n && self.file_put_on(files, 4, CRLF.0, CRLF.1, 2) == 2
    }

    /// 27E3:02AD..02D0: the page read back (0400), "Error while
    /// printing...." centred on (A0h, 50h) in FONT_A, colour 10h.
    fn log_print_failed(&mut self, files: &mut Files) {
        self.log_page_back(files);
        self.mem.w8(DS, 0x9288, 0x10);
        self.select_font(0x1026);
        self.log_centred(var::PRINT_ERROR, 0xA0, 0x50);
    }

    /// 27E3:0400: LOG.$$$ read back into the page ([A900]:0, FA00h bytes).
    fn log_page_back(&mut self, files: &mut Files) {
        let path = self.save_path(var::PAGE_FILE);
        if self.file_open(files, path).is_some() {
            let page = self.mem.r16(DS, 0xA900);
            self.file_read(files, page, 0, 0xFA00);
        }
        self.file_close(files);
    }

    /// 27E3:013C..0146 and 2542:0302..034C: the screen left (see the
    /// module's notes).
    pub fn log_close(&mut self, files: &mut Files, run: LogRun) {
        self.map_bank(run.inner, 0);
        self.map_bank(run.outer, 0);
        for i in 0..0x300u16 {
            self.mem.w8(DS, SCRATCH + i, 0);
        }
        self.screen_restore(files);
        self.clip_whole();
        let f = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, f);
    }

    /// 0000:541E's state: A902 bit 0 off (the cursor's background put
    /// back, the front end's).
    fn cursor_hidden(&mut self) {
        let f = self.mem.r16(DS, 0xA902);
        if f & 1 != 0 {
            self.mem.w16(DS, 0xA902, f & 0xFFFE);
        }
    }

    /// 0000:5458's state: A902 bit 0 on (the cursor drawn, the front
    /// end's).
    fn cursor_drawn(&mut self) {
        let f = self.mem.r16(DS, 0xA902) | 1;
        self.mem.w16(DS, 0xA902, f);
    }
}

/// What a read of the mouse in 01FC came to.
enum Poll {
    Waiting,
    Next,
    Exit,
    Print(bool),
}
