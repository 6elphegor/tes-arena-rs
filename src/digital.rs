//! The digital sound driver, as the game sees it through the AIL (the
//! Miles library in segment 0FFD, which keeps its own state in its own
//! segment): the game loads the driver's file (ARENA.BAT's -S, SBPDIG.ADV)
//! and the AIL calls into it by function number (0FFD:0574, the driver's
//! table of numbers and offsets; with no driver registered under a handle
//! every call returns 0, 0FFD:053C).
//!
//! - main (1194:076D): with a name at 3827, the AIL started once (07DF:
//!   37EA counted, 0FFD:07D6) and the driver loaded (2F9F:0364).
//! - 2F9F:0364: the file loaded where 0000:7412 allocates it (DI FFFFh:
//!   `file_load_alloc`), its far pointer at 8F62 (offset) and 8F64
//!   (segment: sound on); registered (0FFD:0AF7), its handle at EB91;
//!   described (0C1D, 64h), the descriptor's far pointer at EB95; the
//!   command line's I/O port, IRQ and DMA channel (386A, 386C, 386E, each
//!   FFFFh for the driver's own) written over the descriptor's at +0Ch,
//!   +0Eh, +10h; the device detected (0C43, 65h; the answer not looked at)
//!   and the driver initialised (0C49) with the descriptor's four words
//!   from +0Ch.
//!
//! The driver's answers are the front end's (`DigitalDriver`): the
//! speech (src/vision.rs) plays its parts through it and turns its pages
//! as they end.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The sound driver's name (ARENA.BAT's -S), NUL-ended; 0 for none.
    pub const NAME: u16 = 0x3827;
    /// Set once the AIL is started (1194:07DF).
    pub const AIL_STARTED: u16 = 0x37EA;
    /// The command line's I/O port, IRQ and DMA channel for it (FFFFh: the
    /// driver's own).
    pub const SETTINGS: u16 = 0x386A;
    /// The driver's image (a far pointer: offset, then segment, 0 for
    /// none).
    pub const IMAGE: u16 = 0x8F62;
    /// Its handle with the AIL.
    pub const HANDLE: u16 = 0xEB91;
    /// Its descriptor (a far pointer into the image).
    pub const DESCRIPTOR: u16 = 0xEB95;
}
use var::*;

/// The AIL's calls into the digital driver: what the driver answers.
pub trait DigitalDriver {
    /// 0FFD:0AF7: the image at `seg`:`off` registered; its handle (FFFFh
    /// for none).
    fn register(&mut self, m: &mut Machine, seg: u16, off: u16) -> u16;
    /// 0FFD:0C1D (function 64h): the driver's descriptor, (segment,
    /// offset).
    fn describe(&mut self, m: &mut Machine, handle: u16) -> (u16, u16);
    /// 0FFD:0C43 (65h): the device looked for at the I/O port, IRQ, DMA
    /// channel and DRQ given; found or not (the game doesn't look).
    fn detect(&mut self, m: &mut Machine, handle: u16, io: u16, irq: u16, dma: u16, drq: u16) -> u16;
    /// 0FFD:0C49: the driver initialised with them.
    fn init(&mut self, m: &mut Machine, handle: u16, io: u16, irq: u16, dma: u16, drq: u16);
    /// 0FFD:0D46 (7Bh): the VOC file at `seg`:`off` readied to play from
    /// block `block` (FFFFh: its start).
    fn play_voc(&mut self, m: &mut Machine, handle: u16, seg: u16, off: u16, block: u16);
    /// 0FFD:0D58 (7Dh): its playing started.
    fn start(&mut self, m: &mut Machine, handle: u16);
    /// 0FFD:0D52 (7Ch): what it is doing: 0 stopped, 2 playing, 3 done.
    fn status(&mut self, m: &mut Machine, handle: u16) -> u16;
    /// 0FFD:0D5E (7Eh): its playing stopped.
    fn stop(&mut self, m: &mut Machine, handle: u16);
}

/// The AIL with no driver registered: each call into one returns 0
/// (0FFD:053C finds no table; a registration of something not a driver
/// FFFFh).
pub struct NoDriver;

impl DigitalDriver for NoDriver {
    fn register(&mut self, _m: &mut Machine, _seg: u16, _off: u16) -> u16 {
        0xFFFF
    }
    fn describe(&mut self, _m: &mut Machine, _handle: u16) -> (u16, u16) {
        (0, 0)
    }
    fn detect(&mut self, _m: &mut Machine, _handle: u16, _io: u16, _irq: u16, _dma: u16, _drq: u16) -> u16 {
        0
    }
    fn init(&mut self, _m: &mut Machine, _handle: u16, _io: u16, _irq: u16, _dma: u16, _drq: u16) {}
    fn play_voc(&mut self, _m: &mut Machine, _handle: u16, _seg: u16, _off: u16, _block: u16) {}
    fn start(&mut self, _m: &mut Machine, _handle: u16) {}
    fn status(&mut self, _m: &mut Machine, _handle: u16) -> u16 {
        0
    }
    fn stop(&mut self, _m: &mut Machine, _handle: u16) {}
}

impl Machine {
    /// Whether a sound driver is loaded (2294:0770: the dword at 8F62).
    pub fn digital_loaded(&self) -> bool {
        self.mem.r32(DS, IMAGE) != 0
    }

    /// 1194:076D: with a sound driver named (3827), the AIL started once
    /// (07DF: 37EA) and the driver loaded (2F9F:0364, `sound_driver_load`).
    pub fn sound_driver_main(&mut self, files: &mut Files, driver: &mut dyn DigitalDriver) -> Option<()> {
        if self.mem.r8(DS, NAME) == 0 {
            return Some(());
        }
        if self.mem.r8(DS, AIL_STARTED) == 0 {
            self.mem.w8(DS, AIL_STARTED, 1);
        }
        self.sound_driver_load(files, NAME, driver)
    }

    /// 2F9F:0364: the sound driver named at DS:`name` loaded (see the
    /// module). None where its file doesn't load (the original goes on
    /// with 0:0, which the AIL won't register).
    pub fn sound_driver_load(&mut self, files: &mut Files, name: u16, driver: &mut dyn DigitalDriver) -> Option<()> {
        let (seg, off) = self.file_load_alloc(files, name)?;
        self.mem.w16(DS, IMAGE, off);
        self.mem.w16(DS, IMAGE + 2, seg);
        let handle = driver.register(self, seg, off);
        self.mem.w16(DS, HANDLE, handle);
        let (dseg, doff) = driver.describe(self, handle);
        self.mem.w16(DS, DESCRIPTOR, doff);
        self.mem.w16(DS, DESCRIPTOR + 2, dseg);
        for (k, at) in [(0u16, 0xCu16), (2, 0xE), (4, 0x10)] {
            let v = self.mem.r16(DS, SETTINGS + k);
            if v != 0xFFFF {
                self.mem.w16(dseg, doff.wrapping_add(at), v);
            }
        }
        // (Read again for the second: the descriptor as the first left it.)
        let words = |m: &Machine| [0xCu16, 0xE, 0x10, 0x12].map(|k| m.mem.r16(dseg, doff.wrapping_add(k)));
        let [io, irq, dma, drq] = words(self);
        driver.detect(self, handle, io, irq, dma, drq);
        let [io, irq, dma, drq] = words(self);
        driver.init(self, handle, io, irq, dma, drq);
        Some(())
    }
}
