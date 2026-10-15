//! The staff made whole (1194:D986): the scenes the loot window
//! (2433:05FB) plays when the eighth piece is taken in the Imperial
//! palace's lower levels, with F77 already 9.
//!
//! What it does to the game is little: the song (D56) and A91A kept over
//! it and put back, the scenes' flags at A902 (bit 400h cleared at the
//! start and the end, bits 100h..200h set for each speech), 803C set
//! before each speech, the text colour (9288) of each speech and of the
//! house's text, and the house's text: text 579h written to 929E (mode
//! 2, 101C FFh meanwhile, 137E 0), the font (B036) set from 102C, and
//! its lines cut at each newline (0Ah, made 0) and drawn one under the
//! other until the NUL that ends it. The screens are `EndHooks`.
//!
//! The area is then left by the loot window (C398 bit 80h; 0F76, 5E6
//! and FCE cleared): the game goes on.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// The scenes, the front end's.
pub trait EndHooks {
    /// 1194:DAF0: the scenes begun (the screen kept, 0000:6E38 to 9A6E,
    /// A69E; the song stopped and WINGAME.XMI, named at 80B0h, played;
    /// 0000:A611).
    fn begin(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 2294:0895: the FLC named at DS:`si` played.
    fn movie(&mut self, _m: &mut Machine, _files: &mut Files, _si: u16) {}
    /// 1194:D459: a speech, text `ax` over the FLC named at DS:`di`
    /// (SI the song's name at 80BDh), in the colour at 9288.
    fn speech(&mut self, _m: &mut Machine, _files: &mut Files, _ax: u16, _si: u16, _di: u16) {}
    /// 1194:55B2: the screen faded out.
    fn fade(&mut self, _m: &mut Machine) {}
    /// 1194:DA25..DA63: the house: the screen cleared (0000:9426, 9466),
    /// the bank at 1030 mapped (0000:5C86), HOUSE.IMG (807Bh) loaded into
    /// the EMS frame (0000:7428), its colours set (5828) and it drawn
    /// (9B4E).
    fn house(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 1194:A36C mode 2: text number 137A written to DS:929E
    /// (`Machine::text`).
    fn text(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.text(files, &mut crate::text::NoTextHooks, 2, DS, 0x929E).map(|_| ())
    }
    /// 0000:5364: the line at DS:`si` drawn centred on column A0h at row
    /// `y`.
    fn line(&mut self, _m: &mut Machine, _si: u16, _y: u16) {}
    /// 1194:DAC4..DACE: the screen shown (0000:9466) and 1194:5532 (the
    /// screen faded in, a key waited for).
    fn shown(&mut self, _m: &mut Machine) {}
    /// 1194:DB26: the scenes' end: the song stopped, the game's screen
    /// back (0000:4EFD), the area's song (D56) played again (25FD:051F),
    /// 0000:A6B5.
    fn end(&mut self, _m: &mut Machine, _files: &mut Files) {}
    /// 0000:9A98: the view (0, 0, 140h x C8h) cleared.
    fn clear(&mut self, _m: &mut Machine) {}
}

/// `EndHooks` that only write the house's text.
pub struct NoEndHooks;

impl EndHooks for NoEndHooks {}

/// The house's text, where it is written and how far apart its lines
/// are drawn.
const TEXT: u16 = 0x929E;
const LINE: u16 = 0x0A;

impl Machine {
    fn end_speech(&mut self, files: &mut Files, hooks: &mut dyn EndHooks, ax: u16, di: u16, colour: u8, set: bool) {
        self.mem.w8(DS, 0x803C, 1);
        let mut f = self.mem.r16(DS, 0xA902);
        if set {
            f |= 0x200;
            f &= 0xFEFF;
        } else {
            f &= 0xFCFF;
        }
        self.mem.w16(DS, 0xA902, f);
        self.mem.w8(DS, 0x9288, colour);
        hooks.speech(self, files, ax, 0x80BD, di);
    }

    /// 1194:D986. The scenes of the staff made whole (see the module):
    /// WALK.FLC and AFLC2.FLC, NUJAGDTH.FLC with text 57Ch in 64h,
    /// STAFF.FLC and KING.FLC, NUKING.FLC with 57Dh and then 57Fh in
    /// 99h, then the house with text 579h in 5Ch. None where the text
    /// does, or the house's text has no end in the segment (the original
    /// reads on for ever).
    pub fn staff_end(&mut self, files: &mut Files, hooks: &mut dyn EndHooks) -> Option<()> {
        let (a91a, d56) = (self.mem.r16(DS, 0xA91A), self.mem.r16(DS, 0x0D56));
        hooks.begin(self, files);
        let f = self.mem.r16(DS, 0xA902) & 0xFBFF;
        self.mem.w16(DS, 0xA902, f);
        hooks.movie(self, files, 0x803D);
        hooks.movie(self, files, 0x8046);
        self.end_speech(files, hooks, 0x57C, 0x8050, 0x64, false);
        hooks.fade(self);
        hooks.movie(self, files, 0x805D);
        hooks.movie(self, files, 0x8067);
        self.end_speech(files, hooks, 0x57D, 0x8070, 0x99, true);
        self.end_speech(files, hooks, 0x57F, 0x8070, 0x99, true);
        hooks.fade(self);
        hooks.house(self, files);
        self.mem.w8(DS, 0x101C, 0xFF);
        self.mem.w16(DS, 0x137A, 0x579);
        self.mem.w8(DS, 0x137E, 0);
        hooks.text(self, files)?;
        self.mem.w8(DS, 0x101C, 0);
        self.mem.w8(DS, 0x9288, 0x5C);
        let font = self.mem.r16(DS, 0x102C);
        self.mem.w16(DS, 0xB036, font);
        // 1194:DA94..DAC2: each line cut at its newline and drawn; the
        // NUL ends it after the line it ends is drawn.
        let (mut si, mut y) = (TEXT, 0x32u16);
        loop {
            let mut p = si;
            let mut steps = 0u32;
            let last = loop {
                match self.mem.r8(DS, p) {
                    0x0A => break false,
                    0 => break true,
                    _ => p = p.wrapping_add(1),
                }
                steps += 1;
                if steps > 0x10000 {
                    return crate::giveup::original("1194:DA9E: the house's text has no end: the original reads on for ever");
                }
            };
            self.mem.w8(DS, p, 0);
            hooks.line(self, si, y);
            si = p.wrapping_add(1);
            y = y.wrapping_add(LINE);
            if last {
                break;
            }
        }
        hooks.shown(self);
        self.mem.w16(DS, 0x0D56, d56);
        hooks.end(self, files);
        self.mem.w16(DS, 0xA91A, a91a);
        hooks.clear(self);
        let f = self.mem.r16(DS, 0xA902) & 0xFBFF;
        self.mem.w16(DS, 0xA902, f);
        Some(())
    }
}
