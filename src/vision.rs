//! A speech over a movie (1194:D45D, far D459), as the game's state sees
//! it: the farewell's vision a death plays before the main menu (0000:0D3E,
//! F77 not 9), with the install's speech (SPEECH\nnnn_mmX.VOC).
//!
//! - D45D: the movie's name, the song's and the text's number kept at
//!   12DA, 12DC, 12DE; the colours read to 9A6E (6E38); the page written
//!   to SCREENSV.$$2 (A69E); A91A (C7h meanwhile) and D56 kept over it; a
//!   block of BB8h bytes taken last fit (9CD2) for the text, its segment at
//!   803A; the speech's name made (2F75:0219: "nnnn_00a" at 8F41, the text's
//!   number by 7024); the text (A374 mode 2, 137A the number, 137E and 101C
//!   0) written to the block and cut into lines at each LF and CR (D540:
//!   made NULs, CX their count plus one); with 803C 0 the song stopped and
//!   the one at [12DC] started; the colours black and the page cleared
//!   (55B2); A902 bits 40h and 400h cleared; then 2294:0700. At the end
//!   the block freed, D56 and A91A put back, and with 803C 0 the song
//!   stopped, the view cleared (4EFD), the building's song in an interior
//!   (25FD:051F) or else the song at D56 started again.
//! - 2294:0700: the keys flushed (710E); the lines' far pointer and count
//!   at 43CE, 43D0, 43D2; the speech set up (2F75:000A) and its first part
//!   played (00DC); the font [102C] selected; bank [1030] mapped, the
//!   frame the movie's buffer (A69E), the frames' hook (914A) 2294:076C,
//!   A902 bit 400h set, A91A A7h over the movie (0000:6546); the speech
//!   put away (014E).
//! - The speech's set-up (2F75:000A): with SPEECH\nnnn_00a.VOC there
//!   (67C4; none, nothing more), 8F4E 1, VISION.EMS deleted and made again
//!   with the bank mapped (8FA8, two bytes) and banks 0..3 (FFFFh bytes of
//!   each) written to it; the page's block freed and the page A000:0000 (the
//!   screen itself), the sound's buffer (EB89) freed and taken again,
//!   FFDCh bytes; then two parts loaded (023F: "nnnn_mma" and "..b" each
//!   into the frame with bank [8F5C] mapped, its first word 0 first, 8F5C
//!   counted round 4, the part's digits at 8F46..8F47 counted on).
//! - A part played (00DC): bank [8F5A] mapped; where its first word isn't 0
//!   FFDCh bytes of it copied to the sound's buffer, the sample started
//!   (the driver's, here none) and 8F5A counted; at an even 8F5A two more
//!   parts loaded (023F) and 8F5A kept below 4; bank [1030] mapped again.
//! - The speech put away (014E): with 8F4E set the sound stopped (23BA:000C),
//!   VISION.EMS read back (two bytes to 8F41, banks 0..3), bank [8F41]
//!   shifted right by 4 mapped (bug kept: the bank written was 8FA8's, its
//!   number times 4), the sound's buffer freed and taken again (4E20h
//!   bytes), the page taken again (FA04h bytes); 8F5A and 8F5C 0.
//! - The movie (0000:6546) loops (A902 bit 200h, set by the outer loop) and
//!   at each frame reads the mouse (8FC8: AFF9, 8F92, 8F94); after the frame
//!   (and its wait for the timer, but for the first) the hook (2294:076C):
//!   with no button down 4355 0 and the page's lines drawn (up to three,
//!   from [43CE], centred on A0h from row AAh on, 5364: the front end's);
//!   a button down after none (4355 0 to 1) the page turned where three
//!   lines or more are left (084C: [43CE] past three lines, 43D2 three
//!   fewer, the text's band cleared, the next part played, 00DC), else the
//!   movie ended (A902 bit 100h); a button held, the lines drawn.
//!
//! The pictures (the movie's frames, the lines, the fades) are the front
//! end's (`VisionHooks`).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The movie's name, the song's and the text's number (D45D's DI, SI,
    /// AX).
    pub const MOVIE: u16 = 0x12DA;
    pub const SONG: u16 = 0x12DC;
    pub const TEXT_NUMBER: u16 = 0x12DE;
    /// The text's block (its segment).
    pub const BLOCK: u16 = 0x803A;
    /// 0: the speech's own song, stopped and played again around it.
    pub const OWN_SONG: u16 = 0x803C;
    /// The page's lines: a far pointer to the first shown, how many are
    /// left.
    pub const LINES: u16 = 0x43CE;
    pub const LINES_LEFT: u16 = 0x43D2;
    /// Set while a button is held after the page turned for it.
    pub const HELD: u16 = 0x4355;
    /// "SPEECH\", then the part's name: "nnnn_mmX.VOC" (8F41).
    pub const SPEECH_PATH: u16 = 0x8F3A;
    pub const SPEECH_NAME: u16 = 0x8F41;
    /// "vision.ems": where banks 0..3 are kept meanwhile.
    pub const KEEP_FILE: u16 = 0x8F4F;
    /// Set while the speech is up (its banks kept).
    pub const SPEECH_ON: u16 = 0x8F4E;
    /// The next part to play and to load (banks 0..3).
    pub const PLAYING: u16 = 0x8F5A;
    pub const LOADING: u16 = 0x8F5C;
    /// The sound's buffer (a far pointer).
    pub const SOUND_BUFFER: u16 = 0xEB89;
    /// The bank mapped, times 4.
    pub const BANK: u16 = 0x8FA8;
}
use var::*;

/// What the speech leaves to the front end.
pub trait VisionHooks {
    /// 0000:9012 through 8FC8, at each of the movie's frames: the mouse's
    /// buttons (AFF9) and place (CX in 640ths, DX). By default none down,
    /// the place where it was.
    fn mouse(&mut self, m: &mut Machine) -> (u8, u16, u16) {
        (0, m.mem.r16(DS, 0x8F92).wrapping_mul(2), m.mem.r16(DS, 0x8F94))
    }
    /// 0000:618F's drawing: the movie's frame read (its bytes in the
    /// frame, A69E), shown with the lines the hook draws after.
    fn frame(&mut self, _m: &mut Machine, _first: bool) {}
    /// 0000:5364: the line at `seg`:`si` drawn centred on column A0h at
    /// row `y`.
    fn line(&mut self, _m: &mut Machine, _seg: u16, _si: u16, _y: u16) {}
    /// 1194:55B2: the screen faded out.
    fn fade_out(&mut self, _m: &mut Machine) {}
}

/// `VisionHooks` that click through the pages: the left button down at
/// one read of the mouse and up at the next, from the first.
#[derive(Default)]
pub struct ClickThrough {
    down: bool,
}

impl VisionHooks for ClickThrough {
    fn mouse(&mut self, m: &mut Machine) -> (u8, u16, u16) {
        self.down = !self.down;
        (self.down as u8, m.mem.r16(DS, 0x8F92).wrapping_mul(2), m.mem.r16(DS, 0x8F94))
    }
}

impl Machine {
    /// 1194:D45D: the speech, text `ax` over the movie named at DS:`di`,
    /// the song at DS:`si` (see the module); `dac` the VGA's colours (6-bit)
    /// for 9A6E, None leaves them. None where a block isn't had, a file
    /// doesn't load or a path isn't ported.
    pub fn vision(&mut self, files: &mut Files, hooks: &mut dyn VisionHooks, dac: Option<&[[u8; 3]; 256]>, ax: u16, si: u16, di: u16) -> Option<()> {
        self.mem.w16(DS, MOVIE, di);
        self.mem.w16(DS, SONG, si);
        self.mem.w16(DS, TEXT_NUMBER, ax);
        for (i, c) in dac.iter().flat_map(|d| d.iter()).enumerate() {
            for (k, &v) in c.iter().enumerate() {
                self.mem.w8(DS, 0x9A6E_u16.wrapping_add((i * 3 + k) as u16), v);
            }
        }
        self.screen_save(files);
        let a91a = self.mem.r16(DS, 0xA91A);
        self.mem.w16(DS, 0xA91A, 0xC7);
        let d56 = self.mem.r16(DS, 0x0D56);
        let block = self.heap_alloc_high(0xBB8)?;
        if block == 0 {
            return crate::giveup::not_ported("1194:D486: no block for the speech's text");
        }
        self.mem.w16(DS, BLOCK, block);
        // 2F75:0219: the speech's first part's name.
        for (k, c) in b"_00a".iter().enumerate() {
            self.mem.w8(DS, SPEECH_NAME + 4 + k as u16, *c);
        }
        self.store_utoa(ax, SPEECH_NAME);
        self.mem.w8(DS, 0x137E, 0);
        self.mem.w16(DS, crate::text::var::TEXT, ax);
        self.mem.w8(DS, 0x101C, 0);
        self.text(files, &mut crate::text::NoTextHooks, 2, block, 0)?;
        let lines = self.vision_lines(block);
        let own = self.mem.r8(DS, OWN_SONG) == 0;
        if own {
            self.music_stop();
            self.music_start(self.mem.r16(DS, SONG));
        }
        hooks.fade_out(self);
        self.sheet_black();
        let v = self.mem.r16(DS, 0xA902) & 0xFBBF;
        self.mem.w16(DS, 0xA902, v);
        self.vision_movie(files, hooks, block, lines, self.mem.r16(DS, MOVIE))?;
        self.heap_free(self.mem.r16(DS, BLOCK));
        self.mem.w16(DS, 0x0D56, d56);
        self.mem.w16(DS, 0xA91A, a91a);
        if own {
            self.music_stop();
            // 0000:4EFD: the view filled with colour 0.
            self.mem.w8(DS, 0x9288, 0);
            // 25FD:051F: in an interior (5E6 4) the building's song.
            if self.mem.r8(DS, 0x05E6) == 4 {
                self.music_start(self.mem.r16(DS, crate::music::var::BUILDING_SONG));
            } else if self.mem.r16(DS, 0x0D56) != 0 {
                self.music_start(self.mem.r16(DS, 0x0D56));
            }
        }
        Some(())
    }

    /// 1194:D540: the text at `seg`:0 cut at each LF and CR (made NULs);
    /// their count plus one.
    fn vision_lines(&mut self, seg: u16) -> u16 {
        let (mut si, mut cx) = (0u16, 0u16);
        loop {
            match self.mem.r8(seg, si) {
                0 => break,
                0x0A | 0x0D => {
                    self.mem.w8(seg, si, 0);
                    cx = cx.wrapping_add(1);
                }
                _ => {}
            }
            si = si.wrapping_add(1);
        }
        cx.wrapping_add(1)
    }

    /// 2294:0700: the movie named at DS:`name` with the lines at
    /// `seg`:0 (`count` of them) and the speech (see the module).
    fn vision_movie(&mut self, files: &mut Files, hooks: &mut dyn VisionHooks, seg: u16, count: u16, name: u16) -> Option<()> {
        self.key_flush();
        self.mem.w16(DS, LINES, 0);
        self.mem.w16(DS, LINES + 2, seg);
        self.mem.w16(DS, LINES_LEFT, count);
        self.speech_open(files)?;
        self.speech_next(files)?;
        self.select_font(0x102C);
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        let frame = self.mem.r16(DS, 0x9030);
        self.mem.w16(DS, crate::title::var::MOVIE_BUFFER, frame);
        self.mem.w16(DS, 0x914A, 0x076C);
        self.mem.w16(DS, 0x914C, 0x2294);
        let v = self.mem.r16(DS, 0xA902) | 0x400;
        self.mem.w16(DS, 0xA902, v);
        let a91a = self.mem.r16(DS, 0xA91A);
        self.mem.w16(DS, 0xA91A, 0xA7);
        self.vision_play(files, hooks, name)?;
        self.mem.w16(DS, 0xA91A, a91a);
        self.speech_close(files)
    }

    /// 0000:6546 as the speech's movie runs it: looping (A902 bit 200h),
    /// the mouse read at each frame, the hook (2294:076C) after each, up to
    /// the frame after which the hook ends it (A902 bit 100h), going round
    /// as often as it takes (64F0). None where the file is missing or not
    /// a movie.
    fn vision_play(&mut self, files: &mut Files, hooks: &mut dyn VisionHooks, name: u16) -> Option<()> {
        let Some(handle) = self.file_open_handle(files, name) else {
            return crate::giveup::not_ported("0000:6143: a movie not found (its error drawn)");
        };
        let buf = self.mem.r16(DS, crate::title::var::MOVIE_BUFFER);
        self.file_read(files, buf, 0, 0x80);
        if self.mem.r16(buf, 4) != 0xAF12 {
            self.file_close(files);
            return crate::giveup::not_ported("0000:656D: the speech's movie not a movie");
        }
        let (w, h) = (self.mem.r16(buf, 8), self.mem.r16(buf, 0xA));
        let frames = self.mem.r16(buf, 6);
        let first = self.mem.r32(buf, 0x50);
        let size = w.wrapping_mul(h);
        let (block, off) = self.heap_alloc(((size as u32 + 0xF) >> 4) as u16)?;
        for i in 0..size {
            self.mem.w8(block, off.wrapping_add(i), 0);
        }
        self.file_read(files, buf, 0, 0xA);
        if self.mem.r16(buf, 4) == 0xF100 {
            self.file_read(files, buf, 0, 8);
        }
        self.file_seek(files, 0, first);
        self.mem.w16(DS, crate::title::var::TIMER, 0);
        let mut k = 0u16;
        loop {
            if k == frames {
                // 667F: the frames gone through; with A902 bit 200h round
                // again (64F0 with CX 0): to the header's +50h, its first
                // frame's place read to A836 (4 bytes), there, and the
                // first frame read without its drawing (618F with AX 0;
                // no mouse, no hook); then the frames after it, as many
                // again. Else the movie over.
                if self.mem.r16(DS, 0xA902) & 0x200 == 0 {
                    break;
                }
                self.file_on(files, handle);
                self.file_seek(files, 0, 0x50);
                self.file_read(files, DS, 0xA836, 4);
                self.file_seek(files, 0, self.mem.r32(DS, 0xA836));
                self.movie_frame_read(files, buf, size);
                k = 0;
            }
            // (On the movie's own handle: a page turned loads the speech's
            // parts, opening and closing their files.)
            self.file_on(files, handle);
            self.movie_frame_read(files, buf, size);
            hooks.frame(self, k == 0);
            // 8FC8: the mouse read (9012), its place kept within its limits.
            let (buttons, x, y) = hooks.mouse(self);
            self.mem.w8(DS, crate::controls::var::BUTTONS, buttons);
            self.mem.w16(DS, crate::controls::var::MOUSE_X, x >> 1);
            self.mem.w16(DS, crate::controls::var::MOUSE_Y, y);
            self.mouse_clamp();
            if k != 0 {
                self.mem.w16(DS, crate::title::var::TIMER, 0);
            }
            self.vision_page(files, hooks)?;
            if self.mem.r16(DS, 0xA902) & 0x100 != 0 {
                break;
            }
            k += 1;
        }
        self.file_on(files, handle);
        self.file_close(files);
        self.heap_free(block);
        Some(())
    }

    /// 2294:076C, after each of the movie's frames (no sound driver, 8F62
    /// 0): see the module.
    fn vision_page(&mut self, files: &mut Files, hooks: &mut dyn VisionHooks) -> Option<()> {
        if self.mem.r8(DS, crate::controls::var::BUTTONS) & 3 == 0 {
            self.mem.w8(DS, HELD, 0);
            self.vision_lines_drawn(hooks);
            return Some(());
        }
        if self.mem.r8(DS, HELD) != 0 {
            self.vision_lines_drawn(hooks);
            return Some(());
        }
        self.mem.w8(DS, HELD, 1);
        if self.mem.r16(DS, LINES_LEFT) < 3 {
            if self.mem.r16(DS, 0xA902) & 0x200 == 0 {
                self.mem.w8(DS, HELD, 0);
                self.vision_lines_drawn(hooks);
            } else {
                let v = self.mem.r16(DS, 0xA902) | 0x100;
                self.mem.w16(DS, 0xA902, v);
            }
            return Some(());
        }
        // 084C: past three lines (each scan's count shared, 3E8h).
        let (mut di, seg) = (self.mem.r16(DS, LINES), self.mem.r16(DS, LINES + 2));
        let mut cx = 0x3E8u16;
        for _ in 0..3 {
            while cx != 0 {
                cx -= 1;
                let b = self.mem.r8(seg, di);
                di = di.wrapping_add(1);
                if b == 0 {
                    break;
                }
            }
        }
        self.mem.w16(DS, LINES, di);
        let left = self.mem.r16(DS, LINES_LEFT).wrapping_sub(3);
        self.mem.w16(DS, LINES_LEFT, left);
        // 9A6E: the text's band (0, A7h, 140h x 20h) cleared, 9288 0
        // meanwhile and A91A C7h.
        self.speech_next(files)?;
        Some(())
    }

    /// 2294:07AA: up to three of the lines left drawn (5364), from row AAh
    /// ten rows apart.
    fn vision_lines_drawn(&mut self, hooks: &mut dyn VisionHooks) {
        let n = self.mem.r16(DS, LINES_LEFT);
        if n == 0 || (n as i16) < 0 {
            return;
        }
        let (mut si, seg) = (self.mem.r16(DS, LINES), self.mem.r16(DS, LINES + 2));
        let mut y = 0xAA;
        for _ in 0..n.min(3) {
            hooks.line(self, seg, si, y);
            y += 0xA;
            loop {
                si = si.wrapping_add(1);
                if self.mem.r8(seg, si.wrapping_sub(1)) == 0 {
                    break;
                }
            }
        }
    }

    /// 2F75:000A: the speech set up (see the module).
    fn speech_open(&mut self, files: &mut Files) -> Option<()> {
        self.mem.w8(DS, SPEECH_ON, 0);
        if self.file_open(files, SPEECH_PATH).is_none() {
            return Some(());
        }
        self.file_close(files);
        self.mem.w8(DS, SPEECH_ON, 1);
        let path = self.save_path(KEEP_FILE);
        self.file_delete(files, path);
        self.file_create(files, path);
        self.file_put(files, DS, BANK, 2);
        let frame = self.mem.r16(DS, 0x9030);
        for bank in 0..4 {
            self.map_bank(bank, 0);
            self.file_put(files, frame, 0, 0xFFFF);
        }
        self.file_close(files);
        self.heap_free(self.mem.r16(DS, 0xA900));
        self.mem.w16(DS, 0xA8FE, 0);
        self.mem.w16(DS, 0xA900, 0xA000);
        self.heap_free(self.mem.r16(DS, SOUND_BUFFER + 2));
        self.speech_buffer(0xFFDC)?;
        self.speech_load(files);
        Some(())
    }

    /// 51E2: the sound's buffer taken, `bytes` (EB89, a far pointer).
    fn speech_buffer(&mut self, bytes: u16) -> Option<()> {
        let (seg, off) = self.heap_alloc_bytes(bytes)?;
        self.mem.w16(DS, SOUND_BUFFER, off);
        self.mem.w16(DS, SOUND_BUFFER + 2, seg);
        Some(())
    }

    /// 2F75:023F: the next two parts loaded ("..a" and "..b", see the
    /// module).
    fn speech_load(&mut self, files: &mut Files) {
        let frame = self.mem.r16(DS, 0x9030);
        for c in [b'a', b'b'] {
            self.mem.w8(DS, SPEECH_NAME + 7, c);
            self.map_bank(self.mem.r16(DS, LOADING), 0);
            self.mem.w16(frame, 0, 0);
            let _ = self.file_load(files, SPEECH_PATH, frame, 0);
            let n = self.mem.r16(DS, LOADING).wrapping_add(1);
            self.mem.w16(DS, LOADING, n);
        }
        let n = self.mem.r16(DS, LOADING) & 3;
        self.mem.w16(DS, LOADING, n);
        let at = SPEECH_NAME + 6;
        if self.mem.r8(DS, at) == b'9' {
            let d = self.mem.r8(DS, at - 1).wrapping_add(1);
            self.mem.w8(DS, at - 1, d);
            self.mem.w8(DS, at, b'0');
        } else {
            let d = self.mem.r8(DS, at).wrapping_add(1);
            self.mem.w8(DS, at, d);
        }
    }

    /// 2F75:00DC: the next part played (see the module).
    fn speech_next(&mut self, files: &mut Files) -> Option<()> {
        self.map_bank(self.mem.r16(DS, PLAYING), 0);
        let frame = self.mem.r16(DS, 0x9030);
        if self.mem.r16(frame, 0) != 0 {
            // 7702: to the buffer's segment at offset 0.
            let seg = self.mem.r16(DS, SOUND_BUFFER + 2);
            for i in 0..0xFFDCu16 {
                let b = self.mem.r8(frame, i);
                self.mem.w8(seg, i, b);
            }
            let n = self.mem.r16(DS, PLAYING).wrapping_add(1);
            self.mem.w16(DS, PLAYING, n);
        }
        if self.mem.r16(DS, PLAYING) & 1 == 0 {
            self.speech_load(files);
            let n = self.mem.r16(DS, PLAYING) & 3;
            self.mem.w16(DS, PLAYING, n);
        }
        self.map_bank(self.mem.r16(DS, 0x1030), 0);
        Some(())
    }

    /// 2F75:014E: the speech put away (see the module).
    fn speech_close(&mut self, files: &mut Files) -> Option<()> {
        if self.mem.r8(DS, SPEECH_ON) != 0 {
            let path = self.save_path(KEEP_FILE);
            self.file_open(files, path)?;
            self.file_read(files, DS, SPEECH_NAME, 2);
            let frame = self.mem.r16(DS, 0x9030);
            for bank in 0..4 {
                self.map_bank(bank, 0);
                self.file_read(files, frame, 0, 0xFFFF);
            }
            self.file_close(files);
            self.map_bank(self.mem.r16(DS, SPEECH_NAME) >> 4, 0);
            self.heap_free(self.mem.r16(DS, SOUND_BUFFER + 2));
            self.speech_buffer(0x4E20)?;
            let (seg, off) = self.heap_alloc_bytes(0xFA04)?;
            self.mem.w16(DS, 0xA8FE, off);
            self.mem.w16(DS, 0xA900, seg);
        }
        self.mem.w16(DS, LOADING, 0);
        self.mem.w16(DS, PLAYING, 0);
        Some(())
    }
}
