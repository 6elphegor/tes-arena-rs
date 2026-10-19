//! The program's start, as far as the game's state goes. From DOS's
//! hands: the C start-up up to main (0000:0000, `c_start`: the PSP's
//! words, the stack, the heap over the memory DOS gave, argv) and main's
//! head up to 21EE:0000 (0000:102E, `main_head`: the CD-ROM check and the
//! data path, src/cmdline.rs), checked against the original run from its
//! entry (harness/gen_cmdline.py). The game's own run and its tests start
//! instead from the memory as harness/screens.py's `init_memory` lays it
//! out (the heap over conventional memory, EMS on with its frame at
//! F000h: `memory_init`), then 21EE:0000 (`program_start`): the sound
//! buffer, the page and the screen's mode, GLOBAL.BSA opened, the EMS
//! banks, the random seed, the directory's cache, the fonts, the boxes'
//! tiles, the arrow cursor, the list's recolouring tables.
//!
//! The image is unrelocated here (its segments as the disassembly's), the
//! harness's relocated 1000h higher: a segment of the image the start
//! stores (8F72's 21EEh, ...) is 1000h lower than the harness's. The heap
//! lies where the harness's does, so its segments are the same.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// What the start asks of the machine it runs on.
pub trait StartHooks {
    /// INT 10h AH=0Fh's video mode before the start sets 13h (0000:6F21):
    /// 3, the text mode DOS leaves.
    fn video_mode(&mut self, _m: &mut Machine) -> u8 {
        3
    }
    /// A mouse driver present (0000:5384: INT 33h's vector set and its
    /// reset answering): the pointer's drawing chosen from it (9B04).
    fn mouse(&mut self, _m: &mut Machine) -> bool {
        true
    }
}

/// The defaults: text mode before, a mouse.
pub struct PortStart;

impl StartHooks for PortStart {}

/// The heap's ends and the VGA's block (harness/screens.py's HEAP_LOW,
/// VGA, HEAP_HIGH, HEAP_TOP).
pub const HEAP_LOW: u16 = 0x5C00;
pub const VGA: u16 = 0xA000;
pub const HEAP_HIGH: u16 = 0xB000;
pub const HEAP_TOP: u16 = 0xF000;
/// The EMS page frame (9030), above conventional memory.
pub const EMS_FRAME: u16 = 0xF000;

impl Machine {
    /// The memory as the start finds it: the heap over conventional
    /// memory from HEAP_LOW to HEAP_TOP (0000:00CC), the VGA's 64 KB a
    /// block in use (free HEAP_LOW..9FFEh, the VGA's, free HEAP_HIGH..
    /// EFFDh); EMS on (8FA6), 32 banks (9032), the frame at EMS_FRAME
    /// (9030), every bank free (8FAA's words FFFFh, as 0000:5DA0 leaves
    /// them).
    pub fn memory_init(&mut self) -> Option<()> {
        self.heap_init(HEAP_LOW, HEAP_TOP)?;
        let header = |m: &mut Machine, seg: u16, next: u16, prev: u16, size: u16, used: u8| {
            m.mem.w16(seg, 0, 0);
            m.mem.w16(seg, 2, next);
            m.mem.w16(seg, 4, 0);
            m.mem.w16(seg, 6, prev);
            m.mem.w16(seg, 8, size);
            m.mem.w8(seg, 0xA, used);
        };
        let low = VGA - 1 - HEAP_LOW - 1;
        let high = HEAP_TOP - HEAP_HIGH - 3;
        header(self, HEAP_LOW, VGA - 1, 0, low, 0);
        header(self, VGA - 1, HEAP_HIGH, HEAP_LOW, 0x1000, 1);
        header(self, HEAP_HIGH, 0, VGA - 1, high, 0);
        self.mem.w16(DS, crate::heap::var::FREE, low + high);
        self.mem.w16(DS, 0x9030, EMS_FRAME);
        self.mem.w8(DS, 0x8FA6, 1);
        self.mem.w16(DS, 0x9032, 0x20);
        self.mem.w16(DS, 0x8FA8, 0);
        for b in 0..0x20u16 {
            self.mem.w16(DS, 0x8FAA + 2 * b, 0xFFFF);
        }
        Some(())
    }

    /// 21EE:0000. The program's start, once (3B07 bit 1): the sound
    /// driver's buffer, 4E20h bytes (EB89, the allocator's EAX: segment
    /// high); the page (0000:6F06); the CyberMan looked for (2F26:000E:
    /// none); the timer's hook 21EE:028A (8F70); the 3D mode's flag BA80;
    /// EMS (0000:5CCA: on, its bank count; 1030 the last, CB64 the frame);
    /// the random seed from the BIOS's ticks (0000:6FC2 with AX 0); the
    /// BSA's directory cached in bank 16h (0000:6A52); the fonts (0000:
    /// 7504: FONT_A, FONT_B, ARENAFNT, TEENYFNT at 1026, 1028, 102C, 102E),
    /// FONT_B selected (0000:9AE6); the boxes' tiles registered (0000:756C:
    /// BRASS, BRASS2 8 x 8 at 1034 and 103E, MARBLE, MARBLE2 3 x 3 at 1036
    /// and 103A), MARBLE2 the tiles in use (0000:9AF8); the arrow cursor
    /// (ARENARW.IMG, 0000:7428, its 100h bytes to [8F84], 0000:9AC0, the
    /// file's block freed); E39C 1; the pointer's drawing (0000:9B00 with
    /// AL 1); the list's recolouring tables (3159:0EE2 with the pairs at
    /// 3992). None where the start fails (an allocation, too few EMS
    /// banks: the original quits with "File Error").
    pub fn program_start(&mut self, files: &mut Files, hooks: &mut dyn StartHooks) -> Option<()> {
        if self.mem.r8(DS, 0x3B07) & 2 != 0 {
            return Some(());
        }
        let v = self.mem.r8(DS, 0x3B07) | 2;
        self.mem.w8(DS, 0x3B07, v);
        let (seg, off) = self.heap_alloc_bytes(0x4E20)?;
        self.mem.w32(DS, 0xEB89, (seg as u32) << 16 | off as u32);
        self.page_start(files, hooks)?;
        // 2F26:000E: no CyberMan (1194:0067 finds none): 8BEC 0.
        self.mem.w8(DS, 0x8BEC, 0);
        self.mem.w16(DS, 0x8F70, 0x028A);
        self.mem.w16(DS, 0x8F72, 0x21EE);
        self.mem.w8(DS, 0xBA80, 1);
        self.mem.w16(DS, 0x9032, 0x20);
        self.mem.w16(DS, 0x9034, 1);
        // 0000:5CCA: EMS on already, its bank count.
        let pages = self.mem.r16(DS, 0x9032);
        if pages < 0x20 {
            return crate::giveup::original("21EE:0121: too few EMS banks: the original quits");
        }
        self.mem.w16(DS, 0x1030, pages - 1);
        let frame = self.mem.r16(DS, 0x9030);
        self.mem.w16(DS, 0xCB64, frame);
        // 0000:6FC2 with AX 0: the seed the BIOS's ticks (0000:9D64), odd;
        // 91F2 and 91F4 half of AX; no table (AX 0).
        let ticks = self.bios_ticks | 1;
        self.mem.w32(DS, 0x91F6, ticks);
        self.mem.w16(DS, 0x91F2, 0);
        self.mem.w16(DS, 0x91F4, 0);
        self.load_rng();
        self.bsa_cache_dir(files, 0x16);
        for (name, at) in [(0x3A7Cu16, 0x1026u16), (0x3A87, 0x1028), (0x3A92, 0x102C), (0x3A9F, 0x102E)] {
            let seg = self.font_load(files, name);
            self.mem.w16(DS, at, seg);
        }
        let font = self.mem.r16(DS, 0x1028);
        self.mem.w16(DS, 0xB036, font);
        for (name, w, at) in [(0x3AC0u16, 8u16, 0x1034u16), (0x3ACA, 8, 0x103E), (0x3AD5, 3, 0x1036), (0x3AF6, 3, 0x103A)] {
            let slot = self.resource_add(files, name, w, w)?;
            self.mem.w16(DS, at, slot);
        }
        let tiles = self.mem.r16(DS, 0x103A);
        self.mem.w16(DS, 0xB038, tiles);
        // 0000:7428 (DI FFFFh): ARENARW.IMG on the heap; 0000:9AC0 with AX
        // and BX 0 (the cursor's hot spot, 8F96 and 8F98): its first 100h
        // bytes to [8F84]; the block freed (0000:6C32).
        let seg = self.load_alloc(files, 0x3AEA);
        self.mem.w16(DS, 0x8F96, 0);
        self.mem.w16(DS, 0x8F98, 0);
        let (ds, di) = (self.mem.r16(DS, 0x8F86), self.mem.r16(DS, 0x8F84));
        for i in 0..0x100u16 {
            let b = self.mem.r8(seg, i);
            self.mem.w8(ds, di.wrapping_add(i), b);
        }
        self.heap_free(seg);
        self.mem.w8(DS, 0xE39C, 1);
        // 0000:9B00 with AL 1: no CyberMan's mouse (0000:5F64: 9136 0);
        // with a mouse (0000:5384), B03A 1 and the drawing [B02C] (B028).
        self.mem.w16(DS, 0x9136, 0);
        if hooks.mouse(self) {
            self.mem.w8(DS, 0xB03A, 1);
            let v = self.mem.r16(DS, 0xB02C);
            self.mem.w16(DS, 0xB028, v);
        }
        self.recolour_tables(0x3992, 4);
        Some(())
    }

    /// 0000:6F06: the page (FA04h bytes, cleared; its segment A900), the
    /// video mode before (9289) and 13h set, the keyboard's interrupt
    /// taken, the mouse reset, A902 8 (the page drawn to), GLOBAL.BSA
    /// opened (0000:6F3A, `bsa_open`).
    fn page_start(&mut self, files: &mut Files, hooks: &mut dyn StartHooks) -> Option<()> {
        let (seg, _) = self.heap_alloc_bytes(0xFA04)?;
        self.mem.w16(DS, 0xA900, seg);
        for i in 0..0xFA04u16 {
            self.mem.w8(seg, i, 0);
        }
        let mode = hooks.video_mode(self);
        self.mem.w8(DS, 0x9289, mode);
        self.mem.w16(DS, 0xA902, 8);
        self.bsa_open(files);
        Some(())
    }

    /// 0000:7504: the file named at DS:`name` whole in a heap block of its
    /// own (0000:742C with DI FFFFh), its segment in the first free word
    /// of the fonts' table (91AC, 8 words); returns the segment, 0 where
    /// the table is full or the load fails.
    pub fn font_load(&mut self, files: &mut Files, name: u16) -> u16 {
        let Some(k) = (0..8u16).find(|&k| self.mem.r16(DS, 0x91AC + 2 * k) == 0) else { return 0 };
        let seg = self.load_alloc(files, name);
        if seg == 0 {
            return 0;
        }
        self.mem.w16(DS, 0x91AC + 2 * k, seg);
        seg
    }

    /// 3159:0EE2: the two recolouring tables ([BA0E] and [BA12], 100h
    /// bytes each) the identity, then for each of `n` words at DS:`pairs`
    /// the first's low byte to its high byte, the second's the other way.
    pub fn recolour_tables(&mut self, pairs: u16, n: u16) {
        let (es, di) = (self.mem.r16(DS, 0xBA10), self.mem.r16(DS, 0xBA0E));
        let (ds, si) = (self.mem.r16(DS, 0xBA14), self.mem.r16(DS, 0xBA12));
        for b in 0..0x100u16 {
            self.mem.w8(es, di.wrapping_add(b), b as u8);
            self.mem.w8(ds, si.wrapping_add(b), b as u8);
        }
        for k in 0..n {
            let dx = self.mem.r16(DS, pairs.wrapping_add(2 * k));
            let (lo, hi) = (dx & 0xFF, dx >> 8);
            self.mem.w8(es, di.wrapping_add(lo), hi as u8);
            self.mem.w8(ds, si.wrapping_add(hi), lo as u8);
        }
    }
}

/// What the C start-up (0000:0000) and main's head (0000:102E) ask of
/// DOS and the BIOS, and the vectors they take (their own code's, which
/// the port doesn't run).
pub trait CStartHooks {
    /// int 21h AH 30h: DOS's version (AL major, AH minor).
    fn dos_version(&mut self, m: &mut Machine) -> u16;
    /// int 21h AH 35h: interrupt `n`'s vector (segment high).
    fn vector(&mut self, m: &mut Machine, n: u8) -> u32;
    /// int 1Ah AH 0: the BIOS's ticks (CX high, DX low) and AL, non-zero
    /// when midnight has passed since the last read.
    fn bios_ticks(&mut self, m: &mut Machine) -> (u16, u16, u8);
    /// int 10h AH 8 with BH 0: the character's attribute at the cursor.
    fn screen_attribute(&mut self, m: &mut Machine) -> u8;
    /// Midnight passed (the ticks' AL): the BIOS's flag 0040:0070 set to
    /// 1 again, which the read cleared (the BIOS's data, where the port's
    /// unrelocated image lies).
    fn midnight_seen(&mut self, _m: &mut Machine) {}
    /// The vectors and DOS's state set (int 21h AH 25h for interrupts 0
    /// (0000:0151), 8 (0000:018B, the old kept at 0000:0167) and 24h
    /// (21EE:03D9, the old kept at 21EE:03DC); AH 1Ah the DTA at
    /// DGROUP:00D1): the front end's.
    fn vectors_taken(&mut self, _m: &mut Machine) {}
}

impl Machine {
    /// 0000:0000, the C start-up up to main (0000:00C1), the program
    /// loaded with its PSP at `psp` and its stack segment at `ss` (the
    /// image's 4A8Fh where it was loaded): DOS's version to 4B, the PSP's
    /// segment to 49, its environment's (PSP:2Ch) to 47, the top of memory
    /// (PSP:2) to 15D; interrupts 0, 4, 5 and 6's vectors kept at 2D..3B
    /// (0000:01BA); the stack [29] bytes (200h at least) with the heap's
    /// first header past it (159; with less memory than that the original
    /// quits, 0000:0248); its data 4A50:0..4A8E:0 cleared (DGROUP EB00..
    /// EEDF); the BIOS's ticks to 4D/4F (midnight passed: 0040:0070 1,
    /// `midnight_seen`);
    /// the heap over the rest (0000:00CC); argv (0000:0118); the screen's
    /// attribute at the cursor to 15F. None where the original quits.
    pub fn c_start(&mut self, psp: u16, ss: u16, hooks: &mut dyn CStartHooks) -> Option<()> {
        let version = hooks.dos_version(self);
        let top = self.mem.r16(psp, 2);
        let env = self.mem.r16(psp, 0x2C);
        self.mem.w16(DS, 0x4B, version);
        self.mem.w16(DS, 0x49, psp);
        self.mem.w16(DS, 0x47, env);
        self.mem.w16(DS, crate::heap::var::TOP, top);
        for (n, at) in [(0u8, 0x2Du16), (4, 0x31), (5, 0x35), (6, 0x39)] {
            let v = hooks.vector(self, n);
            self.mem.w32(DS, at, v);
        }
        let mut stack = self.mem.r16(DS, 0x29);
        if stack < 0x200 {
            stack = 0x200;
            self.mem.w16(DS, 0x29, stack);
        }
        let paras = (stack >> 4) + 1;
        if top.wrapping_sub(ss) < paras {
            return crate::giveup::original("0000:0248: not enough memory for the stack: the original quits");
        }
        self.mem.w16(DS, crate::heap::var::FIRST + 2, ss.wrapping_add(paras));
        for i in 0..(0x4A8Eu16 - 0x4A50) * 16 {
            self.mem.w8(0x4A50, i, 0);
        }
        let (cx, dx, midnight) = hooks.bios_ticks(self);
        self.mem.w16(DS, 0x4D, dx);
        self.mem.w16(DS, 0x4F, cx);
        if midnight != 0 {
            hooks.midnight_seen(self);
        }
        let first = self.mem.r16(DS, crate::heap::var::FIRST + 2);
        self.heap_init(first, top)?;
        self.argv_split();
        hooks.vectors_taken(self);
        let attr = hooks.screen_attribute(self);
        self.mem.w8(DS, 0x15F, attr);
        Some(())
    }

    /// 0000:102E, main up to 21EE:0000 (0000:1051): its stack pointer to
    /// 1040 (the start-up's top, the [29] bytes', less main's arguments
    /// and the far call's return), the critical error handler taken
    /// (21EE:03A3), the CD-ROM check (21EE:0314, `cd_check`: true, the
    /// carry, and main quits through 0000:10C2), the error handler
    /// 0000:11DA at A904. False where the check passed.
    pub fn main_head(&mut self, cd: &mut dyn crate::cmdline::CdHooks) -> bool {
        let top = ((self.mem.r16(DS, 0x29) >> 4) + 1) << 4;
        self.mem.w16(DS, 0x1040, top.wrapping_sub(10));
        if self.cd_check(cd) {
            return true;
        }
        self.mem.w16(DS, 0xA904, 0x11DA);
        self.mem.w16(DS, 0xA906, 0x0000);
        false
    }

    /// 2294:01F1: the game's resources (from the set-up, 21EE:016C): four
    /// files whole on the heap (0000:7428 with DI FFFFh: the compass at
    /// [4391], the slider at [438D] as far pointers, offset 0; the arrows'
    /// segment at 1002, the keys' at 43D4), 318h bytes for the day's
    /// colours (43C0) and the dither (CD04, `sky_buffers`), C38C 0, the
    /// sky's hook 2294:0CDE at CD72.
    pub fn game_resources(&mut self, files: &mut Files) -> Option<()> {
        for (name, at, far) in [(0x436Au16, 0x4391u16, true), (0x4376, 0x438D, true), (0x4356, 0x1002, false), (0x4361, 0x43D4, false)] {
            let seg = self.load_alloc(files, name);
            if far {
                self.mem.w32(DS, at, (seg as u32) << 16);
            } else {
                self.mem.w16(DS, at, seg);
            }
        }
        self.sky_buffers(files)?;
        self.mem.w16(DS, 0xC38C, 0);
        self.mem.w16(DS, 0xCD72, 0x0CDE);
        self.mem.w16(DS, 0xCD74, 0x2294);
        Some(())
    }
}

/// The set-up's parts (21EE:016C, `game_setup`) as the original runs
/// them: the resources (2294:01F1, `game_resources`), the hands' graphics
/// (0000:3C4C, `weapon_change_load`), the spells (29CF:0691), the effects' flats (21EE:0134,
/// `effect_flats`, and C38E from D432).
pub struct SetupPort<'a>(pub &'a mut Files);

impl crate::newgame::NewGameHooks for SetupPort<'_> {
    fn resources(&mut self, m: &mut Machine) {
        m.game_resources(self.0);
    }
    fn hands(&mut self, m: &mut Machine) {
        m.weapon_change_load(self.0);
    }
    fn spells(&mut self, m: &mut Machine) {
        m.spells_load(self.0);
    }
    fn level_flats(&mut self, m: &mut Machine) {
        m.effect_flats(self.0);
        let v = m.mem.r16(DS, 0xD432);
        m.mem.w16(DS, 0xC38E, v);
    }
}

impl Machine {
    /// 0000:7534: the colours named at DS:`name` read to 929E (0000:742C)
    /// and put at DS:`di`: 300h bytes as they are where the file is 300h
    /// long, else past its 8-byte header each shifted right by 2. None
    /// where the file doesn't load.
    pub fn colours_load(&mut self, files: &mut Files, name: u16, di: u16) -> Option<()> {
        self.file_load(files, name, DS, 0x929E)?;
        let size = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE);
        for i in 0..0x300u16 {
            let b = if size == 0x300 {
                self.mem.r8(DS, 0x929E + i)
            } else {
                self.mem.r8(DS, 0x929E + 8 + i) >> 2
            };
            self.mem.w8(DS, di.wrapping_add(i), b);
        }
        Some(())
    }
}
