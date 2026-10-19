//! The program's command line and its CD-ROM check, before 21EE:0000:
//! the C start-up's split of the PSP's command tail into argv (0000:0118,
//! `argv_split`); main's 1194:066A on the words past the first (0000:1072,
//! `command_line`): the sound and music drivers' names and their ports,
//! IRQs and DMA channels as ARENA.BAT passes them ("-Ssbpdig.adv -IOS220
//! -IRQS7 -DMAS1 -Msbp1fm.adv -IOM220"); and main's 21EE:0314
//! (`cd_check`): the current drive, MSCDEX asked whether it is a CD drive,
//! and the data path 1AB9 (C:\ARENA_CD) taken from the environment's
//! ARENADATA. harness/gen_cmdline.py runs the same against the original.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// argc (the C start-up's, counted up from the image's 1).
    pub const ARGC: u16 = 0x003D;
    /// Far pointer to argv's table (the image's DGROUP:0051).
    pub const ARGV: u16 = 0x003F;
    /// The environment's segment (the PSP's 2Ch).
    pub const ENV: u16 = 0x0047;
    /// The PSP's segment.
    pub const PSP: u16 = 0x0049;
    /// The -??E option's digit (its 8th character less '0').
    pub const E_DIGIT: u16 = 0x37E9;
    /// The AIL started (0FFD:07D6, once).
    pub const AIL_STARTED: u16 = 0x37EA;
    /// The music driver's file name (-M).
    pub const MUSIC_DRIVER: u16 = 0x37EB;
    /// The sound driver's file name (-S).
    pub const SOUND_DRIVER: u16 = 0x3827;
    /// The music driver's I/O port (-IOM, hex), IRQ (-IRQM) and DMA
    /// channel (-DMAM); the sound driver's 6 bytes on (-IOS, -IRQS,
    /// -DMAS). FFFFh each in the image: the driver's own.
    pub const MUSIC_IO: u16 = 0x3864;
    pub const MUSIC_IRQ: u16 = 0x3866;
    pub const MUSIC_DMA: u16 = 0x3868;
    /// A number's text, copied out of its option.
    pub const NUMBER: u16 = 0x929E;
    /// "Bad command line arguments" and its example ($-ended).
    pub const BAD_ARGS: u16 = 0x3740;
    /// The variable's name searched for ("ARENADATA", 7 compared).
    pub const DATA_VAR: u16 = 0x3A10;
    /// "MSCDEX Driver not installed.$"
    pub const NO_MSCDEX: u16 = 0x3A1A;
    /// The message for a drive that isn't a CD-ROM ($-ended).
    pub const NOT_CD: u16 = 0x3A37;
    /// The data path (C:\ARENA_CD in the image).
    pub const DATA_PATH: u16 = 0x1AB9;
    /// The current drive (0 for A).
    pub const DRIVE: u16 = 0xA836;
}
use var::*;

/// What the command line hands on (all of it the front end's).
pub trait CommandHooks {
    /// 0FFD:07D6: the AIL started, before the first driver's load.
    fn ail_startup(&mut self, _m: &mut Machine) {}
    /// 2F9F:0364 with the name at DS:`name`: the sound driver loaded.
    fn sound_driver(&mut self, _m: &mut Machine, _name: u16) {}
    /// 2F9F:01BF with the name at DS:`name`: the music driver loaded.
    fn music_driver(&mut self, _m: &mut Machine, _name: u16) {}
    /// 0000:6F8C: the screen given back to DOS before a message.
    fn video_restore(&mut self, _m: &mut Machine) {}
    /// int 21h AH 9: the $-ended message at DS:`at` printed.
    fn print(&mut self, _m: &mut Machine, _at: u16) {}
}

/// What 21EE:0314 asks of DOS and MSCDEX.
pub trait CdHooks {
    /// int 21h AH 19h: the current drive (0 for A).
    fn current_drive(&mut self, m: &mut Machine) -> u8;
    /// int 2Fh AX 1500h with BX 0: BX the CD drives (0 with no MSCDEX,
    /// which leaves BX as it was) and CL the first's.
    fn mscdex(&mut self, m: &mut Machine) -> (u16, u8);
    /// int 2Fh AX 150Bh with BX 0 and CX `drive`: (AX, BX); BX ADADh from
    /// MSCDEX 2.0 on, AX non-zero for a CD drive.
    fn is_cd(&mut self, m: &mut Machine, drive: u16) -> (u16, u16);
    /// int 21h AH 9: the $-ended message at DS:`at` printed.
    fn print(&mut self, _m: &mut Machine, _at: u16) {}
}

impl Machine {
    /// 0000:0118. The PSP's command tail (its length at PSP:80h, the text
    /// from 81h, the CR after it) split into argv: from argv's table's
    /// second entry on, a far pointer to each word, argc counted up, the
    /// byte after each word a NUL. A word is what follows a run of
    /// spaces (only 20h: a tab is in the word) up to the next space or
    /// the tail's end; the scan takes the CR too, so spaces before it
    /// leave a last word of the CR alone (made "" by its NUL).
    pub fn argv_split(&mut self) {
        let (mut si, seg) = (self.mem.r16(DS, ARGV).wrapping_add(4), self.mem.r16(DS, ARGV + 2));
        let es = self.mem.r16(seg, PSP);
        let mut cx = self.mem.r8(es, 0x80) as u16;
        if cx == 0 {
            return;
        }
        let mut di = 0x81u16;
        cx += 1;
        loop {
            if cx as i16 <= 0 {
                return;
            }
            // REPE SCASB with AL a space: the spaces skipped.
            let mut zf = false;
            while cx != 0 {
                zf = self.mem.r8(es, di) == 0x20;
                di = di.wrapping_add(1);
                cx -= 1;
                if !zf {
                    break;
                }
            }
            if zf {
                return;
            }
            let argc = self.mem.r16(seg, ARGC).wrapping_add(1);
            self.mem.w16(seg, ARGC, argc);
            di = di.wrapping_sub(1);
            cx = cx.wrapping_add(1);
            self.mem.w16(seg, si, di);
            self.mem.w16(seg, si.wrapping_add(2), es);
            si = si.wrapping_add(4);
            // REPNE SCASB: on to the next space.
            while cx != 0 {
                let space = self.mem.r8(es, di) == 0x20;
                di = di.wrapping_add(1);
                cx -= 1;
                if space {
                    break;
                }
            }
            self.mem.w8(es, di.wrapping_sub(1), 0);
        }
    }

    /// 1194:066A as main calls it (0000:1072, with argc 2 or more): each
    /// word past argv[0] in turn, which must start with '-', upper-cased
    /// in place (a..z), then by its 2nd and 3rd characters, in this order:
    /// - 2nd 'S': the rest the sound driver's name (3827), 'M' the music
    ///   driver's (37EB);
    /// - 3rd 'E': its 8th character less '0' to 37E9 (read past the
    ///   word's end when shorter);
    /// - 2nd 'D': -DMA, the music's (3868) or with 5th 'S' the sound's
    ///   (386E), decimal from the 6th character on;
    /// - 3rd 'R': -IRQ, 3866 or 386C, likewise;
    /// - 3rd 'O': -IO, 3864 or with 4th 'S' 386A, hex from the 5th on;
    /// - else "Bad command line arguments": the screen given back, the
    ///   message printed, and true (the carry: main quits). Options before
    ///   the bad one have taken effect.
    ///
    /// A name or number is copied up to its first byte not above 20h as a
    /// signed byte (so 80h..FFh end it too), NUL-ended; numbers through
    /// 929E. Then, with a name given (its first byte), the AIL started
    /// once (37EA) and the sound driver loaded, then the music's. False
    /// (the carry clear) unless a word was bad.
    pub fn command_line(&mut self, hooks: &mut dyn CommandHooks) -> bool {
        let argc = self.mem.r16(DS, ARGC);
        let mut si = self.mem.r16(DS, ARGV).wrapping_add(4);
        let mut cx = argc.wrapping_sub(1);
        loop {
            let (arg, fs) = (self.mem.r16(DS, si), self.mem.r16(DS, si.wrapping_add(2)));
            let at = |m: &Machine, k: u16| m.mem.r8(fs, arg.wrapping_add(k));
            if at(self, 0) != b'-' {
                hooks.video_restore(self);
                hooks.print(self, BAD_ARGS);
                return true;
            }
            let mut bx = arg;
            loop {
                let c = self.mem.r8(fs, bx);
                if c == 0 {
                    break;
                }
                if c.is_ascii_lowercase() {
                    self.mem.w8(fs, bx, c - 0x20);
                }
                bx = bx.wrapping_add(1);
            }
            if at(self, 1) == b'S' {
                self.option_copy(fs, arg.wrapping_add(2), SOUND_DRIVER);
            } else if at(self, 1) == b'M' {
                self.option_copy(fs, arg.wrapping_add(2), MUSIC_DRIVER);
            } else if at(self, 2) == b'E' {
                let d = at(self, 7).wrapping_sub(b'0');
                self.mem.w8(DS, E_DIGIT, d);
            } else if at(self, 1) == b'D' {
                let to = if at(self, 4) == b'S' { MUSIC_DMA + 6 } else { MUSIC_DMA };
                self.option_copy(fs, arg.wrapping_add(5), NUMBER);
                let v = self.decimal(DS, NUMBER);
                self.mem.w16(DS, to, v);
            } else if at(self, 2) == b'R' {
                let to = if at(self, 4) == b'S' { MUSIC_IRQ + 6 } else { MUSIC_IRQ };
                self.option_copy(fs, arg.wrapping_add(5), NUMBER);
                let v = self.decimal(DS, NUMBER);
                self.mem.w16(DS, to, v);
            } else if at(self, 2) == b'O' {
                let to = if at(self, 3) == b'S' { MUSIC_IO + 6 } else { MUSIC_IO };
                self.option_copy(fs, arg.wrapping_add(4), NUMBER);
                let v = self.hex(DS, NUMBER);
                self.mem.w16(DS, to, v);
            } else {
                hooks.video_restore(self);
                hooks.print(self, BAD_ARGS);
                return true;
            }
            si = si.wrapping_add(4);
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                break;
            }
        }
        for (name, sound) in [(SOUND_DRIVER, true), (MUSIC_DRIVER, false)] {
            if self.mem.r8(DS, name) == 0 {
                continue;
            }
            // 1194:07DF.
            if self.mem.r8(DS, AIL_STARTED) == 0 {
                self.mem.w8(DS, AIL_STARTED, 1);
                hooks.ail_startup(self);
            }
            if sound {
                hooks.sound_driver(self, name);
            } else {
                hooks.music_driver(self, name);
            }
        }
        false
    }

    /// 1194:07CE: fs:`si` copied to DS:`di` up to and with the first
    /// byte not above 20h as a signed byte, which is made a NUL.
    fn option_copy(&mut self, fs: u16, mut si: u16, mut di: u16) {
        loop {
            let c = self.mem.r8(fs, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c as i8 <= 0x20 {
                break;
            }
        }
        self.mem.w8(DS, di.wrapping_sub(1), 0);
    }

    /// 0000:52B6: the decimal at seg:`si` (a '-' or '+' first, then digits
    /// up to anything else), 16 bits, wrapping.
    pub fn decimal(&self, seg: u16, mut si: u16) -> u16 {
        let mut neg = false;
        match self.mem.r8(seg, si) {
            b'-' => {
                si = si.wrapping_add(1);
                neg = true;
            }
            b'+' => si = si.wrapping_add(1),
            _ => {}
        }
        let mut dx = 0u16;
        loop {
            let c = self.mem.r8(seg, si);
            si = si.wrapping_add(1);
            if c == 0 || !c.is_ascii_digit() {
                break;
            }
            dx = dx.wrapping_mul(10).wrapping_add((c - b'0') as u16);
        }
        if neg {
            dx.wrapping_neg()
        } else {
            dx
        }
    }

    /// 1194:07B1: the hex at seg:`si`, digits 0..9 and A..F: a character
    /// less '0' (a borrow ends it), less 7 more past 9, up to Fh; so ':'
    /// to '@' count as 3..9 too, and lower case ends the number.
    fn hex(&self, seg: u16, mut si: u16) -> u16 {
        let mut dx = 0u16;
        loop {
            let c = self.mem.r8(seg, si);
            si = si.wrapping_add(1);
            let Some(mut d) = c.checked_sub(b'0') else { break };
            if d > 9 {
                d = d.wrapping_sub(7);
            }
            if d > 0xF {
                break;
            }
            dx = (dx << 4).wrapping_add(d as u16);
        }
        dx
    }

    /// 21EE:0314. The current drive to A836 (0000:6E02). MSCDEX missing:
    /// its message, true (main quits). The drive not MSCDEX's first, and
    /// either no MSCDEX 2.0 answer (BX not ADADh) or not a CD drive (AX
    /// 0): the first message or the CD-ROM one, true. Then the
    /// environment's strings ([47]'s segment from 0) searched: the first
    /// whose 7 first characters are "ARENADA" in either case (0000:9C5E)
    /// has its text from the 10th character on copied to the data path
    /// 1AB9, NUL included (0000:9CA0: neither the '=' nor the length
    /// checked); false.
    pub fn cd_check(&mut self, hooks: &mut dyn CdHooks) -> bool {
        let drive = hooks.current_drive(self);
        self.mem.w8(DS, DRIVE, drive);
        let (cds, first) = hooks.mscdex(self);
        if cds == 0 {
            hooks.print(self, NO_MSCDEX);
            return true;
        }
        if first != drive {
            let (ax, bx) = hooks.is_cd(self, drive as u16);
            if bx != 0xADAD {
                hooks.print(self, NO_MSCDEX);
                return true;
            }
            if ax == 0 {
                hooks.print(self, NOT_CD);
                return true;
            }
        }
        let es = self.mem.r16(DS, ENV);
        let mut di = 0u16;
        loop {
            if self.names_match(DS, DATA_VAR, es, di, 7) {
                let (mut si, mut to) = (di.wrapping_add(0xA), DATA_PATH);
                for _ in 0..0x10000u32 {
                    let c = self.mem.r8(es, si);
                    self.mem.w8(DS, to, c);
                    si = si.wrapping_add(1);
                    to = to.wrapping_add(1);
                    if c == 0 {
                        break;
                    }
                }
                return false;
            }
            // 0000:9CAC: the string's length; past its NUL.
            let mut n = 0u16;
            while n != 0xFFFF && self.mem.r8(es, di.wrapping_add(n)) != 0 {
                n += 1;
            }
            di = di.wrapping_add(n).wrapping_add(1);
            if self.mem.r8(es, di) == 0 {
                return false;
            }
        }
    }

    /// 0000:9C5E: ds:`si` against es:`di`, a..z taken as A..Z (nothing
    /// else), up to `cx`
    /// characters or a NUL in both: the same (ZF).
    fn names_match(&self, ds: u16, mut si: u16, es: u16, mut di: u16, mut cx: u16) -> bool {
        loop {
            let a = self.mem.r8(ds, si).to_ascii_uppercase();
            let b = self.mem.r8(es, di).to_ascii_uppercase();
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if a != b {
                return false;
            }
            if a == 0 {
                return true;
            }
            cx = cx.wrapping_sub(1);
            if cx == 0 {
                return true;
            }
        }
    }
}
