//! The program's way back to its main menu after a game's end (0000:109B
//! with 5B1 bit 100h, or 5CA bit 8000h: the options screen's New), as
//! the game's state sees it, up to the menu's wait for its regions
//! (0000:1114: 1194:5CF4 over the three at 3276:6300).
//!
//! - 10B0: 5CA bit 8000h cleared; 108B: the texts' segment (CB8A) the EMS
//!   frame's; 10C9: the clip the whole screen (9A98), the page cleared
//!   (942A; the mouse's cursor, 5422, is the front end's).
//! - The intro (1246): the song stopped and PERCNTRO.XMI started, the
//!   keys flushed and the mouse's buttons waited up (13ED); INTRO.FLC
//!   (2294:0895: bank [1030] mapped, the movie's buffer A69E the frame,
//!   A902 bits 100h..200h cleared, 914A's hook none; `movie_play`); the
//!   screen faded out (55B2); TITLE.IMG loaded on the heap, drawn, faded
//!   in (5E3E) and freed, 118h retraces waited (13BF); bank [1030] mapped
//!   again; QUOTE.IMG into the frame and drawn (56C8 with BX 64h), 1A4h
//!   retraces; with Esc held (ACB6) no more; else SCROLL.FLC (6546
//!   straight, A69E the frame again), then SCROLL01..03.IMG (the digit
//!   at 4DCB counted from '1') each into the frame, drawn and dissolved in
//!   (1393: 742C to [902E], 582C, 1194:4FF7), waited 4A6h, 4A6h and 1A4h
//!   retraces, Esc ending them; the screen faded out.
//! - 10E2: A902 bit 1 cleared, the clip and the page again, MENU.IMG
//!   loaded high, drawn and freed (56CC with BX 40h), the disk's free
//!   space checked (1194:54D4: a box under 5M, the front end's).
//!
//! The movies (0000:6546) as the state sees them: the file opened, its
//! 80h-byte header read into the buffer (the frame, A69E), a block of the
//! width times the height taken from the heap and cleared (533C), the
//! prefix chunk's head read and skipped to the first frame (the header's
//! +50h), the timer (0151) 0; then each frame's 10h-byte head read into
//! the buffer, and each of its chunks' 6-byte head (into the code
//! segment, 64B7) and its bytes (a whole frame's, width times height, for
//! a copy chunk, 10h; a postage stamp, 12h, skipped), the timer waited
//! for and made 0 again after the first; at the end the file closed and
//! the block freed. What the frames' chunks draw into the block and the
//! page, and the colours, are the front end's (`TitleHooks::movie`); so
//! are the waits, a key or a click ending one.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// The names the intro loads (DS offsets).
pub mod var {
    pub const INTRO_FLC: u16 = 0x4E23;
    pub const TITLE_IMG: u16 = 0x4E2D;
    pub const QUOTE_IMG: u16 = 0x4DDD;
    pub const SCROLL_FLC: u16 = 0x4DE7;
    /// "scroll01.img", its digit at 4DCB.
    pub const SCROLL_IMG: u16 = 0x4DC4;
    pub const SCROLL_DIGIT: u16 = 0x4DCB;
    pub const MENU_IMG: u16 = 0x4E0D;
    /// The movies' buffer's segment (0 for one of the heap's).
    pub const MOVIE_BUFFER: u16 = 0xA69E;
    /// The game's timer count.
    pub const TIMER: u16 = 0x0151;
    /// Esc in the held keys' table (ACB5 by scan code).
    pub const ESC_HELD: u16 = 0xACB6;
    /// The mouse's buttons as the driver last gave them.
    pub const BUTTONS: u16 = 0xAFF9;
}
use var::*;

/// The screens' pages, the front end's.
pub trait TitleHooks {
    /// 2294:0895 / 0000:6546: a frame of the movie named at DS:`name`
    /// read, its chunks' bytes in the buffer (the frame); `first` the
    /// movie's first. True where a key or a click ends the movie there.
    fn movie_frame(&mut self, _m: &mut Machine, _name: u16, _first: bool) -> bool {
        false
    }
    /// 1194:55B2: the screen faded out.
    fn fade_out(&mut self, _m: &mut Machine) {}
    /// A picture drawn into the page (5828, its header at seg:off) and
    /// shown: `how` 0 faded in (5E3E, TITLE.IMG), 1 faded in (56C8's 5E50,
    /// QUOTE.IMG), 2 dissolved in (1194:4FF7, the scrolls), 3 shown with
    /// its colours set (56CC's 9B56, MENU.IMG).
    fn picture(&mut self, _m: &mut Machine, _seg: u16, _off: u16, _how: u8) {}
    /// 0000:13BF: `retraces` screen refreshes waited. True where a key or
    /// a click ends the wait.
    fn wait(&mut self, _m: &mut Machine, _retraces: u16) -> bool {
        false
    }
}

/// `TitleHooks` that show nothing and wait for nothing.
pub struct NoTitleHooks;

impl TitleHooks for NoTitleHooks {}

/// The intro's pictures' waits (4D94: SCROLL01..03.IMG's).
const SCROLL_WAITS: [u16; 3] = [0x4A6, 0x4A6, 0x1A4];

impl Machine {
    /// 0000:109B..1114 after a game's end (5B1 bit 100h) or the options
    /// screen's New (5CA bit 8000h): see the module. None where a file
    /// doesn't load or a path isn't ported.
    pub fn title_menu(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks) -> Option<()> {
        if self.mem.r16(DS, 0x05CA) & 0x8000 == 0 && self.mem.r16(DS, 0x05B1) & 0x100 == 0 {
            return crate::giveup::not_ported("0000:10AB: the outer loop run again (no game's end)");
        }
        let v = self.mem.r16(DS, 0x05CA) & 0x7FFF;
        self.mem.w16(DS, 0x05CA, v);
        let frame = self.mem.r16(DS, 0x9030);
        self.mem.w16(DS, 0xCB8A, frame);
        self.clip_whole();
        self.page_clear();
        self.title_intro(files, hooks)?;
        let v = self.mem.r16(DS, 0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, v);
        self.clip_whole();
        self.page_clear();
        // 10FA: 56CC with BX 40h: loaded high (742C with DI FFFEh), the
        // header's words to A836..A83C, drawn, shown with its colours,
        // freed (its block not the frame).
        let seg = self.load_alloc_high(files, MENU_IMG);
        if seg == 0 {
            return crate::giveup::error("0000:10FA: MENU.IMG not loaded");
        }
        self.header_kept(seg, 0);
        self.title_image(seg, 0)?;
        hooks.picture(self, seg, 0, 3);
        self.heap_free(seg);
        Some(())
    }

    /// 56C8's header's words (the image at seg:off) to A836..A83C.
    fn header_kept(&mut self, seg: u16, off: u16) {
        for k in 0..4u16 {
            let v = self.mem.r16(seg, off.wrapping_add(2 * k));
            self.mem.w16(DS, 0xA836 + 2 * k, v);
        }
    }

    /// 0000:5828: the image at seg:off drawn into the page; where the
    /// page isn't the game's own block, only unpacked (its block taken
    /// and freed as the drawing does) for what that leaves in DGROUP.
    fn title_image(&mut self, seg: u16, off: u16) -> Option<()> {
        if self.page_is_block() {
            self.page_image(seg, off);
            return Some(());
        }
        if let crate::imgpack::Unpacked::Block(bs, _) = self.image_unpack(seg, off)? {
            if self.mem.r16(seg, off.wrapping_add(8)) & 0xD != 0 {
                self.heap_free(bs);
            }
        }
        Some(())
    }

    /// 0000:13ED: the keys flushed (7112), the mouse's buttons waited up.
    fn keys_up(&mut self) {
        self.key_flush();
        self.mem.w8(DS, BUTTONS, 0);
    }

    /// 0000:13BF: `n` retraces waited (the front end's), a key or a click
    /// ending it; then 13ED, unless an input ended it with Esc held.
    fn title_wait(&mut self, hooks: &mut dyn TitleHooks, n: u16) {
        let broke = hooks.wait(self, n);
        if !broke || self.mem.r8(DS, ESC_HELD) == 0 {
            self.keys_up();
        }
    }

    /// 0000:1246: the intro (see the module).
    fn title_intro(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks) -> Option<()> {
        self.music_song(crate::music::var::PERCNTRO);
        self.keys_up();
        self.movie_play(files, hooks, INTRO_FLC)?;
        hooks.fade_out(self);
        // 1269: TITLE.IMG on the heap (742C with DI FFFFh), drawn (5828),
        // shown and faded in (9466, 5E3E), freed (6C32).
        let seg = self.load_alloc(files, TITLE_IMG);
        if seg == 0 {
            return crate::giveup::error("0000:1269: TITLE.IMG not loaded");
        }
        self.title_image(seg, 0)?;
        hooks.picture(self, seg, 0, 0);
        self.heap_free(seg);
        self.title_wait(hooks, 0x118);
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        hooks.fade_out(self);
        // 12BC: 56C8 with BX 64h: into the frame's start, the header's
        // words kept, drawn, shown and faded in (5E50).
        let frame = self.mem.r16(DS, 0x9030);
        self.file_load(files, QUOTE_IMG, frame, 0)?;
        self.header_kept(frame, 0);
        self.title_image(frame, 0)?;
        hooks.picture(self, frame, 0, 1);
        self.title_wait(hooks, 0x1A4);
        if self.mem.r8(DS, ESC_HELD) == 0 {
            self.mem.w16(DS, MOVIE_BUFFER, frame);
            let v = self.mem.r16(DS, 0xA902) & 0xFCFF;
            self.mem.w16(DS, 0xA902, v);
            self.keys_up();
            self.movie(files, hooks, SCROLL_FLC)?;
            self.mem.w8(DS, SCROLL_DIGIT, b'1');
            if self.mem.r8(DS, ESC_HELD) == 0 {
                for n in SCROLL_WAITS {
                    // 1393: to the far pointer at 902E, drawn (582C),
                    // dissolved in (1194:4FF7) and shown (946A).
                    let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
                    self.file_load(files, SCROLL_IMG, seg, off)?;
                    self.title_image(seg, off)?;
                    hooks.picture(self, seg, off, 2);
                    self.title_wait(hooks, n);
                    let d = self.mem.r8(DS, SCROLL_DIGIT).wrapping_add(1);
                    self.mem.w8(DS, SCROLL_DIGIT, d);
                    if self.mem.r8(DS, ESC_HELD) != 0 {
                        break;
                    }
                }
            }
        }
        hooks.fade_out(self);
        Some(())
    }

    /// 2294:0895: the movie named at DS:`name` played over bank [1030]:
    /// the frame its buffer (A69E), A902 bits 100h..200h cleared, the
    /// frames' hook (914A) none, then 0000:6546 (`movie`).
    pub fn movie_play(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks, name: u16) -> Option<u16> {
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        let frame = self.mem.r16(DS, 0x9030);
        self.mem.w16(DS, MOVIE_BUFFER, frame);
        let v = self.mem.r16(DS, 0xA902) & 0xFCFF;
        self.mem.w16(DS, 0xA902, v);
        self.mem.w32(DS, 0x914A, 0);
        self.movie(files, hooks, name)
    }

    /// 0000:6546: the movie named at DS:`name` (see the module): AX, 1
    /// played to its end, FFFFh ended by a key or a click, 0 not a movie.
    /// None where the file is missing (the original draws its error,
    /// 66B5) or where the buffer isn't A69E's (an own one, 603C's FFF0h
    /// bytes from the heap: not ported), or a loop (A902 bit 200h).
    pub fn movie(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks, name: u16) -> Option<u16> {
        if self.file_open_handle(files, name).is_none() {
            return crate::giveup::not_ported("0000:6143: a movie not found (its error drawn)");
        }
        let buf = self.mem.r16(DS, MOVIE_BUFFER);
        if buf == 0 {
            return crate::giveup::not_ported("0000:605F: a movie's own buffer from the heap");
        }
        if self.mem.r16(DS, 0xA902) & 0x200 != 0 {
            return crate::giveup::not_ported("0000:667F: a movie looped (A902 bit 200h)");
        }
        // 603C: the header.
        self.file_read(files, buf, 0, 0x80);
        if self.mem.r16(buf, 4) != 0xAF12 {
            self.file_close(files);
            return Some(0);
        }
        let (w, h) = (self.mem.r16(buf, 8), self.mem.r16(buf, 0xA));
        let frames = self.mem.r16(buf, 6);
        let first = self.mem.r32(buf, 0x50);
        // 533C: the frames' block, cleared (77CC).
        let size = w.wrapping_mul(h);
        let (seg, off) = self.heap_alloc(((size as u32 + 0xF) >> 4) as u16)?;
        for i in 0..size {
            self.mem.w8(seg, off.wrapping_add(i), 0);
        }
        // The prefix chunk's head (with its own 8 bytes more), then on to
        // the first frame.
        self.file_read(files, buf, 0, 0xA);
        if self.mem.r16(buf, 4) == 0xF100 {
            self.file_read(files, buf, 0, 8);
        }
        self.file_seek(files, 0, first);
        self.mem.w16(DS, TIMER, 0);
        let mut ax = 1;
        for k in 0..frames {
            self.movie_frame_read(files, buf, size);
            if hooks.movie_frame(self, name, k == 0) {
                // 6697: the keys waited up (the held keys' table all 0)
                // and flushed.
                self.key_flush();
                ax = 0xFFFF;
                break;
            }
            if k != 0 {
                // 6607: the frame's ticks waited for, the count made 0.
                self.mem.w16(DS, TIMER, 0);
            }
        }
        // 6154: closed, the block freed.
        self.file_close(files);
        self.heap_free(seg);
        Some(ax)
    }

    /// 0000:618F's reads: the frame's 10h-byte head into the buffer, and
    /// each chunk's head and bytes (`size` for a copy chunk, 10h).
    fn movie_frame_read(&mut self, files: &mut Files, buf: u16, size: u16) {
        self.file_read(files, buf, 0, 0x10);
        let chunks = self.mem.r16(buf, 6);
        for _ in 0..chunks {
            // 6257: the head into the code segment (64B7).
            self.file_read(files, 0, 0x64B7, 6);
            let len = self.mem.r32(0, 0x64B7);
            let kind = self.mem.r16(0, 0x64BB);
            if (len as i32) <= 6 {
                continue;
            }
            if kind == 0x12 {
                self.file_seek(files, 1, len.wrapping_sub(6));
            } else {
                let n = if kind == 0x10 { size } else { len.wrapping_sub(6) as u16 };
                self.file_read(files, buf, 0, n);
            }
        }
    }
}
