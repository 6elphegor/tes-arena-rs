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
}

impl Machine {
    pub fn new(image: &Image) -> Self {
        let mem = Memory::new(image);
        let rng = ArenaRandom::from_mem(&mem);
        Self { mem, rng, ems: Vec::new(), mouse_motion: (0, 0) }
    }

    /// Reload the RNG after its DGROUP variables were written directly.
    pub fn load_rng(&mut self) {
        self.rng = ArenaRandom::from_mem(&self.mem);
    }

    /// Write the RNG seed back, so memory matches the original.
    pub fn store_rng(&mut self) {
        self.rng.store(&mut self.mem);
    }
}
