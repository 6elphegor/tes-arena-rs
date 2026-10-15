//! Shared state of the original program that ported routines run against.

use crate::exe::Image;
use crate::mem::Memory;
use crate::random::ArenaRandom;

#[derive(Clone)]
pub struct Machine {
    pub mem: Memory,
    pub rng: ArenaRandom,
    /// EMS: the 64 KB banks not mapped into the frame (see
    /// `Machine::ems_map`), created as zeros when first needed.
    pub ems: Vec<Vec<u8>>,
    /// The mouse driver's motion counters (mickeys, x and y) since they
    /// were last read (INT 33h function 0Bh; see `weapon`).
    pub mouse_motion: (u16, u16),
    /// The sound driver: playing a sound (its status 2: a new one is
    /// dropped), as the front end says; and the sounds played, for the
    /// front end to take (`sound`).
    pub sound_busy: bool,
    pub sounds: Vec<crate::sound::Sound>,
}

impl Machine {
    pub fn new(image: &Image) -> Self {
        let mem = Memory::new(image);
        let rng = ArenaRandom::from_mem(&mem);
        Self { mem, rng, ems: Vec::new(), mouse_motion: (0, 0), sound_busy: false, sounds: Vec::new() }
    }

    /// Reload the RNG after its DGROUP variables were written directly.
    pub fn load_rng(&mut self) {
        self.rng = ArenaRandom::from_mem(&self.mem);
    }

    /// Write the RNG seed back, so memory matches the original.
    pub fn store_rng(&mut self) {
        self.rng.store(&mut self.mem);
    }

    /// Whether the hook (far pointer) at DS:`at` is set. The original
    /// tests the segment word (0000:A8EF and its inline copies), which is
    /// never 0 for a routine of its own: the image is relocated when it is
    /// loaded, and the hooks into its segment 0000 are written with a
    /// relocated segment (2294:0006, `mov word [C3AA], 0000` with a
    /// relocation). The port keeps the image at segment 0, so those hooks
    /// have segment 0 here. A hook is cleared with the whole pointer
    /// (0000:0971 `mov dword [CD32], 0` and the like), so the port tests
    /// the whole pointer.
    pub fn hook_set(&self, at: u16) -> bool {
        self.mem.r32(crate::mem::DS, at) != 0
    }
}
