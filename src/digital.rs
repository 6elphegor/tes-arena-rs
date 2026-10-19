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

/// The digital driver replayed for the game's sounds (segment 23BA: the
/// AIL calls 23BA:002A and the others make), a test's from the original
/// driver's (gen_soundplay.py's `driver` lines): its status answers
/// (7Ch) taken in turn, and every call the sounds make kept in the
/// lines' form (`7C=3`, `7B:ffff:DIGEST` the buffer played, its 4E20h
/// bytes, `81:VOLUME`, `7D`, `7E`, `85:HANDLE:SEG:OFF`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Replay {
    pub answers: std::collections::VecDeque<u16>,
    pub calls: Vec<String>,
}

/// The driver as the machine's replay answers it (`Machine::digital`):
/// the speech's calls kept and answered as the sounds' are, its 7Bh with
/// the buffer's first 4E20h bytes' digest. (Its load's calls, which the
/// replay doesn't answer, are SBPDIG.ADV's: the first handle, its
/// descriptor at 73h, the device found.)
pub struct Replayed;

impl DigitalDriver for Replayed {
    fn register(&mut self, _m: &mut Machine, _seg: u16, _off: u16) -> u16 {
        0
    }
    fn describe(&mut self, m: &mut Machine, _handle: u16) -> (u16, u16) {
        (m.mem.r16(DS, IMAGE + 2), 0x73)
    }
    fn detect(&mut self, _m: &mut Machine, _handle: u16, _io: u16, _irq: u16, _dma: u16, _drq: u16) -> u16 {
        1
    }
    fn init(&mut self, _m: &mut Machine, _handle: u16, _io: u16, _irq: u16, _dma: u16, _drq: u16) {}
    fn play_voc(&mut self, m: &mut Machine, _handle: u16, seg: u16, off: u16, block: u16) {
        m.digital_call(|m| format!("7B:{block:x}:{:x}", m.digital_digest(seg, off)));
    }
    fn start(&mut self, m: &mut Machine, _handle: u16) {
        m.digital_call(|_| "7D".to_string());
    }
    fn status(&mut self, m: &mut Machine, _handle: u16) -> u16 {
        m.digital_status()
    }
    fn stop(&mut self, m: &mut Machine, _handle: u16) {
        m.digital_call(|_| "7E".to_string());
    }
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
    /// 0FFD:0D52 (7Ch), as the sounds ask it: what the driver is doing.
    /// Replayed, the original's next answer (0 with none left); else 2
    /// (playing) while the front end plays a sound (`sound_busy`), 3
    /// (done) when not.
    pub fn digital_status(&mut self) -> u16 {
        match &mut self.digital {
            Some(r) => {
                let a = r.answers.pop_front().unwrap_or(0);
                r.calls.push(format!("7C={a:x}"));
                a
            }
            None => {
                if self.sound_busy {
                    2
                } else {
                    3
                }
            }
        }
    }

    /// A call the sounds make into the driver with no answer they read
    /// (7Bh, 81h, 7Dh, 7Eh, 85h): kept with a replay; the playing itself
    /// is the front end's (`sounds`).
    pub fn digital_call(&mut self, call: impl FnOnce(&Machine) -> String) {
        if self.digital.is_some() {
            let c = call(self);
            if let Some(r) = &mut self.digital {
                r.calls.push(c);
            }
        }
    }

    /// 0FFD:0D46 (7Bh) kept: the sound buffer at `seg`:`off` played from
    /// its start (block FFFFh), the digest of its 4E20h bytes (its size,
    /// 21EE:000E) as the lines have it.
    pub fn digital_play_voc(&mut self, seg: u16, off: u16) {
        self.digital_call(|m| format!("7B:ffff:{:x}", m.digital_digest(seg, off)));
    }

    /// The digest the replay's 7Bh lines give a buffer at `seg`:`off`:
    /// its first 4E20h bytes (the sound buffer's size, 21EE:000E).
    pub fn digital_digest(&self, seg: u16, off: u16) -> u64 {
        (0..0x4E20u16).fold(0xcbf2_9ce4_8422_2325u64, |h, i| (h ^ self.mem.r8(seg, off.wrapping_add(i)) as u64).wrapping_mul(0x100_0000_01b3))
    }

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
