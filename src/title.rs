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
//!   (1393: 742C to [902E], 582C, 1194:4FF7: its generator, 4B8E, stepped
//!   unless 5CA bit 2), waited 4A6h, 4A6h and 1A4h retraces, Esc ending
//!   them; the screen faded out.
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
    /// The opening story's song ("EVLINTRO.XMI"), its pictures (nine of
    /// a name's offset and a wait: "intro01.img" and on) and their names'
    /// digit.
    pub const EVLINTRO: u16 = 0x4D87;
    pub const STORY_PICTURES: u16 = 0x4DA0;
    pub const STORY_DIGIT: u16 = 0x4DD7;
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
        self.menu_screens(files, hooks)
    }

    /// 0000:1059..1089, main after the program's start (21EE:0000) with
    /// no arguments, then on to its menu's wait (108B..1114,
    /// `menu_screens`): the working slot (1194:3A51 with no "STA=" in the
    /// environment), its digits and files (25FD:022A), the game's own
    /// files copied to it (0219), and location 23h's record built at
    /// 0A22 (1194:3AE8: the Imperial City's, 100h, for province 8; 3B84,
    /// `city_location`). None where a file doesn't load or a path isn't
    /// ported.
    pub fn program_main(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks) -> Option<()> {
        self.program_main_driven(files, hooks, None)
    }

    /// `program_main` with a sound driver named (3827, as main's command
    /// line leaves it) loaded where that command line loads it, after the
    /// location (1194:066A's 076D, `sound_driver_main`), `driver`
    /// answering its load.
    pub fn program_main_driven(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks, driver: Option<&mut dyn crate::digital::DigitalDriver>) -> Option<()> {
        self.working_slot_main();
        self.working_files_delete(files);
        self.working_files_fresh(files);
        // 1194:3AE8's dungeon branch (3AEF, 3B0C for A47 3) can't be taken
        // here: A47 is 0 in the image, and main's call (0000:106C) comes
        // before anything writes it (a load sets it, 25FD:182A, and that
        // load's own call at 183F skips 3AE8 for a dungeon; the city's
        // way in clears it first, 0000:0D5B). Checked against the image.
        if self.mem.r8(DS, 0x0A47) == 3 {
            return crate::giveup::not_ported("1194:3AEF: a dungeon's location record at the program's start (3B0C)");
        }
        let id = if self.mem.r8(DS, 0x0A22 + 0x24) == 8 { 0x100 } else { 0x23 };
        self.city_location(files, DS, 0x0A22, id)?;
        if let Some(driver) = driver {
            self.sound_driver_main(files, driver)?;
        }
        self.menu_screens(files, hooks)
    }

    /// 1194:5CF4 (from 0000:1114) as the main menu's pick leaves it: B7C6
    /// the region's index (0 Load, 1 Start, 2 Exit; counted as the
    /// regions are tried, 5D34), and B7C4 bit 1, cleared as the loop
    /// begins, set by the region's handler (3159:0006), which ends it.
    /// The input's own state (the mouse's, the keys') is the front end's.
    pub fn main_menu_pick(&mut self, region: u16) {
        self.mem.w16(DS, 0xB7C6, region);
        let v = self.mem.r8(DS, 0xB7C4) | 1;
        self.mem.w8(DS, 0xB7C4, v);
    }

    /// 0000:117E..11A2, the main menu's Load before its load menu
    /// (25FD:0242): the song stopped (2F9F:0438), the game set up
    /// (21EE:016C), the page cleared (942A; shown, 946A, the front end's),
    /// PAL.COL's colours read to 929E (7534; set, 9B56, the front end's),
    /// 5C8 bit 4 set (the load the title's). None where a file doesn't
    /// load or a path isn't ported.
    pub fn title_load_setup(&mut self, files: &mut Files) -> Option<()> {
        self.music_stop();
        self.game_setup(&mut crate::start::SetupPort(files))?;
        self.page_clear();
        self.colours_load(files, 0x1304, 0x929E)?;
        let v = self.mem.r16(DS, 0x05C8) | 4;
        self.mem.w16(DS, 0x05C8, v);
        Some(())
    }

    /// 0000:108B..1114: the texts' segment the EMS frame's, then the
    /// menu's screens (10C9, see the module) up to its wait.
    fn menu_screens(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks) -> Option<()> {
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

    /// 1194:55B2 as the game's state sees it (from 0000:111A, the menu's
    /// pick): the colours at 929E black and the page cleared; the fade to
    /// them is the front end's.
    pub fn screen_faded(&mut self) {
        self.sheet_black();
    }

    /// 0000:1323, the opening story before creation (from the menu's New,
    /// 1130): the song stopped and EVLINTRO.XMI (4D87) started, the keys
    /// flushed and the buttons waited up (13ED); bank [1030] mapped, the
    /// frame the movie's buffer (A69E), A902 bits 100h..200h cleared,
    /// SCROLL.FLC (6546 straight); the digit at 4DD7 '1'; unless Esc is
    /// held, the nine pictures of the table at 4DA0 (each its name and its
    /// wait) each to the far pointer at 902E, drawn and dissolved in (1393)
    /// and waited for (13BF), the digit counted on, Esc ending them; the
    /// song stopped and the screen faded out (55B2: 929E black, the page
    /// cleared). None where a file doesn't load or a path isn't ported.
    pub fn story(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks) -> Option<()> {
        self.music_song(files, EVLINTRO);
        self.keys_up();
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        let frame = self.mem.r16(DS, 0x9030);
        self.mem.w16(DS, MOVIE_BUFFER, frame);
        let v = self.mem.r16(DS, 0xA902) & 0xFCFF;
        self.mem.w16(DS, 0xA902, v);
        self.movie(files, hooks, SCROLL_FLC)?;
        self.mem.w8(DS, STORY_DIGIT, b'1');
        if self.mem.r8(DS, ESC_HELD) == 0 {
            for k in 0..9u16 {
                let name = self.mem.r16(DS, STORY_PICTURES + 4 * k);
                let wait = self.mem.r16(DS, STORY_PICTURES + 4 * k + 2);
                self.picture_dissolved(files, hooks, name)?;
                self.title_wait(hooks, wait);
                let d = self.mem.r8(DS, STORY_DIGIT).wrapping_add(1);
                self.mem.w8(DS, STORY_DIGIT, d);
                if self.mem.r8(DS, ESC_HELD) != 0 {
                    break;
                }
            }
        }
        self.music_stop();
        self.sheet_black();
        hooks.fade_out(self);
        Some(())
    }

    /// 0000:1393: the picture named at DS:`name` to the far pointer at
    /// 902E (742C), drawn (582C), dissolved in (1194:4FF7, its generator
    /// 4B8E stepped: the hook sees it as the dissolve starts) and shown
    /// (946A).
    fn picture_dissolved(&mut self, files: &mut Files, hooks: &mut dyn TitleHooks, name: u16) -> Option<()> {
        let (off, seg) = (self.mem.r16(DS, 0x902E), self.mem.r16(DS, 0x9030));
        self.file_load(files, name, seg, off)?;
        self.title_image(seg, off)?;
        hooks.picture(self, seg, off, 2);
        self.dissolve();
        Some(())
    }

    /// 0000:1139..117D, the menu's New after creation (1194:2498): the
    /// screen faded out (55B2); creation's vision (1194:D459 with text
    /// 578h, the song at 809A, the movie at 8085; 803C 0, A902 bit 200h
    /// set and 100h cleared, 9288 2Fh; `vision` with `hooks` and the
    /// colours as the fade left them, 929E's); faded out again; 5C8 bit 4
    /// cleared; the game set up (21EE:016C, `game_setup`); the game's own
    /// files copied to the working slot (25FD:0219); 5CA bit 8000h
    /// cleared. Then main (0000:1096..10AB): the song stopped, on to the
    /// start dungeon (0C66, `game_start`). None where a file doesn't load
    /// or a path isn't ported.
    pub fn new_game_begun(&mut self, files: &mut Files, hooks: &mut dyn crate::vision::VisionHooks) -> Option<()> {
        self.sheet_black();
        hooks.fade_out(self);
        self.mem.w8(DS, 0x803C, 0);
        let v = (self.mem.r16(DS, 0xA902) | 0x200) & 0xFEFF;
        self.mem.w16(DS, 0xA902, v);
        self.mem.w8(DS, 0x9288, 0x2F);
        let dac: [[u8; 3]; 256] = std::array::from_fn(|i| std::array::from_fn(|k| self.mem.r8(DS, 0x929E + (i * 3 + k) as u16)));
        self.vision(files, hooks, Some(&dac), 0x578, 0x809A, 0x8085)?;
        self.sheet_black();
        hooks.fade_out(self);
        let v = self.mem.r16(DS, 0x05C8) & 0xFFFB;
        self.mem.w16(DS, 0x05C8, v);
        self.game_setup(&mut crate::start::SetupPort(files))?;
        self.working_files_fresh(files);
        let v = self.mem.r16(DS, 0x05CA) & 0x7FFF;
        self.mem.w16(DS, 0x05CA, v);
        self.music_stop();
        self.game_start();
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
        self.music_song(files, crate::music::var::PERCNTRO);
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
                    // 1194:4FF7's generator (4B8E) stepped (the hook sees
                    // it as the dissolve starts; the pixels its).
                    self.dissolve();
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
            self.movie_frame_decode(files, buf, (seg, off), w, h)?;
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

    /// 0000:618F with AX 2, as 6546 calls it: a frame read and drawn. Its
    /// 10h-byte head into the buffer (`buf`:0), then each chunk's head
    /// into the code segment (64B7) and its bytes into the buffer (its
    /// length less 6; a copy chunk's, 10h, the frame's `w` x `h`; one of 6
    /// bytes or less reads none and is taken on the buffer as it was),
    /// each decoded into the frame's block (`block`, `w` x `h`; 6257) as
    /// it is read; a thumbnail (12h) is skipped (seeked past). Then the
    /// block is drawn at (0, 0) on the page (where it is the game's
    /// block): with A902 bit 400h its 0s left out (5A6A), else the page
    /// cleared first (942A) and all of it drawn (59D8). None where a
    /// chunk's kind is none of the player's (64AC: the original draws its
    /// error).
    fn movie_frame_decode(&mut self, files: &mut Files, buf: u16, block: (u16, u16), w: u16, h: u16) -> Option<()> {
        self.file_read(files, buf, 0, 0x10);
        let chunks = self.mem.r16(buf, 6);
        for _ in 0..chunks {
            self.file_read(files, 0, 0x64B7, 6);
            let len = self.mem.r32(0, 0x64B7);
            let kind = self.mem.r16(0, 0x64BB);
            if (len as i32) > 6 {
                if kind == 0x12 {
                    self.file_seek(files, 1, len.wrapping_sub(6));
                    continue;
                }
                let n = if kind == 0x10 { w.wrapping_mul(h) } else { len.wrapping_sub(6) as u16 };
                self.file_read(files, buf, 0, n);
            }
            match kind {
                4 | 0x0B => self.flc_palette(buf, kind == 4),
                0x0C => self.flc_lc(buf, block, w),
                7 => self.flc_ss2(buf, block, w),
                0x0D => {
                    // 6430: the page (not the block) cleared.
                    if self.page_is_block() {
                        let (off, seg) = (self.mem.r16(DS, 0xA8FE), self.mem.r16(DS, 0xA900));
                        for i in 0..0xFA00u16 {
                            self.mem.w8(seg, off.wrapping_add(i), 0);
                        }
                    }
                }
                0x0F => self.flc_brun(buf, block, w, h),
                0x10 => self.flc_copy(buf, 0, block.0, block.1, w.wrapping_mul(h)),
                _ => return crate::giveup::not_ported("0000:64AC: a movie's chunk of no kind the player has (its error drawn)"),
            }
        }
        if !self.page_is_block() {
            return Some(());
        }
        let page_seg = self.mem.r16(DS, 0xA900);
        if self.mem.r16(DS, 0xA902) & 0x400 == 0 {
            self.page_clear();
        }
        let mut page: Vec<u8> = (0..64000u32).map(|i| self.mem.r8(page_seg, i as u16)).collect();
        if self.mem.r16(DS, 0xA902) & 0x400 != 0 {
            crate::render::clipped_page(self, &mut page, 0, 0, w, h, block.0, block.1);
        } else {
            crate::render::clipped_page_opaque(self, &mut page, 0, 0, w, h, block.0, block.1);
        }
        for (i, &b) in page.iter().enumerate() {
            self.mem.w8(page_seg, i as u16, b);
        }
        Some(())
    }

    /// 0000:7706: `cx` bytes copied from `buf`:`si` to seg:`di`.
    fn flc_copy(&mut self, buf: u16, si: u16, seg: u16, di: u16, cx: u16) {
        for i in 0..cx {
            let b = self.mem.r8(buf, si.wrapping_add(i));
            self.mem.w8(seg, di.wrapping_add(i), b);
        }
    }

    /// 0000:77CE: `cx` bytes at seg:`di` filled with the word `ax`: on an
    /// odd offset its low byte first, then the word over and over a
    /// dword at a time, the last odd bytes its low byte.
    fn flc_fill(&mut self, seg: u16, mut di: u16, mut cx: u16, ax: u16) {
        if cx == 0 {
            return;
        }
        let (lo, hi) = (ax as u8, (ax >> 8) as u8);
        if di & 1 != 0 {
            self.mem.w8(seg, di, lo);
            di = di.wrapping_add(1);
            cx = cx.wrapping_sub(1);
        }
        for _ in 0..cx >> 2 {
            for b in [lo, hi, lo, hi] {
                self.mem.w8(seg, di, b);
                di = di.wrapping_add(1);
            }
        }
        for _ in 0..cx & 3 {
            self.mem.w8(seg, di, lo);
            di = di.wrapping_add(1);
        }
    }

    /// A `loop` count: 0 is 10000h times.
    fn flc_count(cx: u16) -> u32 {
        if cx == 0 { 0x10000 } else { cx as u32 }
    }

    /// 0000:62F0, chunks 4 and 0Bh: the colours at DS:929E (6546's) set
    /// from the packets (a skip of colours, then a count, 0 for 100h, its
    /// high byte the packets' counter's left over; three bytes each,
    /// `six` (chunk 4) shifted down to 6 bits).
    fn flc_palette(&mut self, buf: u16, six: bool) {
        let mut si = 0u16;
        let mut di = 0x929Eu16;
        let mut packets = self.mem.r16(buf, si);
        si = si.wrapping_add(2);
        for _ in 0..Self::flc_count(packets) {
            let skip = self.mem.r8(buf, si) as u16;
            di = di.wrapping_add(skip.wrapping_mul(3));
            let cl = self.mem.r8(buf, si.wrapping_add(1));
            si = si.wrapping_add(2);
            let n = if cl == 0 { 0x100 } else { (packets & 0xFF00) | cl as u16 };
            for _ in 0..Self::flc_count(n) {
                for _ in 0..3 {
                    let mut b = self.mem.r8(buf, si);
                    si = si.wrapping_add(1);
                    if six {
                        b >>= 2;
                    }
                    self.mem.w8(DS, di, b);
                    di = di.wrapping_add(1);
                }
            }
            packets = packets.wrapping_sub(1);
        }
    }

    /// 0000:6354, chunk 0Ch: the lines skipped, then each line's packets
    /// (its first skip read before them, each later one after the packet
    /// before), a negative count a run of one byte, a positive one bytes
    /// copied.
    fn flc_lc(&mut self, buf: u16, (seg, base): (u16, u16), w: u16) {
        let mut di = base.wrapping_add(self.mem.r16(buf, 0).wrapping_mul(w));
        let lines = self.mem.r16(buf, 2);
        let mut si = 4u16;
        for _ in 0..Self::flc_count(lines) {
            let packets = self.mem.r8(buf, si) as u16;
            si = si.wrapping_add(1);
            if packets != 0 {
                let mut bx = self.mem.r8(buf, si) as u16;
                si = si.wrapping_add(1);
                let mut cx = packets;
                loop {
                    let cl = self.mem.r8(buf, si);
                    si = si.wrapping_add(1);
                    if cl & 0x80 != 0 {
                        let n = cl.wrapping_neg() as u16;
                        let at = di.wrapping_add(bx);
                        bx = bx.wrapping_add(n);
                        let al = self.mem.r8(buf, si);
                        si = si.wrapping_add(1);
                        self.flc_fill(seg, at, n, al as u16 * 0x101);
                    } else {
                        let n = cl as u16;
                        let at = di.wrapping_add(bx);
                        bx = bx.wrapping_add(n);
                        self.flc_copy(buf, si, seg, at, n);
                        si = si.wrapping_add(n);
                    }
                    if cx != 1 {
                        bx = bx.wrapping_add(self.mem.r8(buf, si) as u16);
                        si = si.wrapping_add(1);
                    }
                    cx = cx.wrapping_sub(1);
                    if cx == 0 {
                        break;
                    }
                }
            }
            di = di.wrapping_add(w);
        }
    }

    /// 0000:63B4, chunk 7 (word deltas), with its quirks: a line whose
    /// word is negative (bit 4000h: lines skipped; else its low byte put
    /// at the offset before) or 0 isn't counted as one of the chunk's
    /// lines; a run of words on an odd offset is 77CE's.
    fn flc_ss2(&mut self, buf: u16, (seg, base): (u16, u16), w: u16) {
        let mut di = base;
        let mut lines = self.mem.r16(buf, 0);
        let mut si = 2u16;
        loop {
            let ax = self.mem.r16(buf, si);
            si = si.wrapping_add(2);
            if ax & 0x8000 != 0 {
                if ax & 0x4000 != 0 {
                    di = di.wrapping_add(ax.wrapping_neg().wrapping_mul(w));
                } else {
                    self.mem.w8(seg, di.wrapping_sub(1), ax as u8);
                }
                continue;
            }
            if ax == 0 {
                continue;
            }
            let mut bx = self.mem.r8(buf, si) as u16;
            si = si.wrapping_add(1);
            let mut cx = ax;
            loop {
                let cl = self.mem.r8(buf, si);
                si = si.wrapping_add(1);
                if cl & 0x80 != 0 {
                    let n = (cl.wrapping_neg() as u16).wrapping_mul(2);
                    let at = di.wrapping_add(bx);
                    bx = bx.wrapping_add(n);
                    let word = self.mem.r16(buf, si);
                    si = si.wrapping_add(2);
                    self.flc_fill(seg, at, n, word);
                } else {
                    let n = (cl as u16).wrapping_mul(2);
                    let at = di.wrapping_add(bx);
                    bx = bx.wrapping_add(n);
                    self.flc_copy(buf, si, seg, at, n);
                    si = si.wrapping_add(n);
                }
                if cx != 1 {
                    bx = bx.wrapping_add(self.mem.r8(buf, si) as u16);
                    si = si.wrapping_add(1);
                }
                cx = cx.wrapping_sub(1);
                if cx == 0 {
                    break;
                }
            }
            di = di.wrapping_add(w);
            lines = lines.wrapping_sub(1);
            if lines == 0 {
                break;
            }
        }
    }

    /// 0000:644A, chunk 0Fh: each of the `h` lines' runs (its count byte
    /// skipped) until `w` are made: a positive count a run of one byte, a
    /// negative one bytes copied.
    fn flc_brun(&mut self, buf: u16, (seg, base): (u16, u16), w: u16, h: u16) {
        let mut di = base;
        let mut si = 0u16;
        for _ in 0..Self::flc_count(h) {
            si = si.wrapping_add(1);
            let mut bx = 0u16;
            loop {
                let cl = self.mem.r8(buf, si);
                si = si.wrapping_add(1);
                if cl & 0x80 == 0 {
                    let n = cl as u16;
                    let at = di.wrapping_add(bx);
                    bx = bx.wrapping_add(n);
                    let al = self.mem.r8(buf, si);
                    si = si.wrapping_add(1);
                    self.flc_fill(seg, at, n, al as u16 * 0x101);
                } else {
                    let n = cl.wrapping_neg() as u16;
                    let at = di.wrapping_add(bx);
                    bx = bx.wrapping_add(n);
                    self.flc_copy(buf, si, seg, at, n);
                    si = si.wrapping_add(n);
                }
                if (bx as i16) >= (w as i16) {
                    break;
                }
            }
            di = di.wrapping_add(w);
        }
    }

    /// 0000:618F's reads: the frame's 10h-byte head into the buffer, and
    /// each chunk's head and bytes (`size` for a copy chunk, 10h).
    pub(crate) fn movie_frame_read(&mut self, files: &mut Files, buf: u16, size: u16) {
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
