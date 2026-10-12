//! Random number generator, ported from ACD.EXE segment 0000.
//!
//! DGROUP variables (segment 3BA0):
//!   91EE  word   read offset into the pre-generated table (bytes)
//!   91F0  word   table segment; 0 = table disabled
//!   91F2  word   words left before the table is regenerated
//!   91F4  word   regenerate count (bytes; refill writes count/2 words)
//!   91F6  dword  LCG seed

use crate::mem::{DS, Memory};

/// Optional table of pre-generated values that `next` reads from instead
/// of stepping the LCG directly.
#[derive(Clone, Debug)]
pub struct Table {
    pub words: Vec<u16>,
    /// Read position in bytes, as the original keeps it.
    pub offset: u16,
    pub remaining: u16,
    pub refill: u16,
}

/// DGROUP offset of the LCG seed.
pub const SEED: u16 = 0x91F6;

#[derive(Clone, Debug)]
pub struct ArenaRandom {
    pub seed: u32,
    pub table: Option<Table>,
}

impl ArenaRandom {
    pub fn new(seed: u32) -> Self {
        Self { seed, table: None }
    }

    /// Takes the seed from DGROUP. The table lives in its own
    /// segment outside DGROUP, so it is not carried over.
    pub fn from_mem(mem: &Memory) -> Self {
        Self::new(mem.r32(DS, SEED))
    }

    pub fn store(&self, mem: &mut Memory) {
        mem.w32(DS, SEED, self.seed);
    }

    /// 0000:6E10. Steps the LCG and returns the high word of the new seed.
    pub fn step(&mut self) -> u16 {
        self.seed = self.seed.wrapping_mul(0x006D_002D);
        (self.seed >> 16) as u16
    }

    /// 0000:6EA8 (far entries 6EA0 and 6EA4).
    pub fn next(&mut self) -> u16 {
        let Some(mut t) = self.table.take() else {
            return self.step();
        };
        let mut value = t.words[(t.offset / 2) as usize];
        t.offset = t.offset.wrapping_add(2);
        t.remaining = t.remaining.wrapping_sub(1);
        if t.remaining == 0 {
            // Regenerate. The original returns the last value written,
            // not the one it just read from the table.
            t.offset = 0;
            t.remaining = t.refill;
            // `loop` with CX=0 runs 65536 times; DI wraps within the segment.
            let n = match t.refill >> 1 {
                0 => 0x10000,
                n => n as usize,
            };
            for i in 0..n {
                value = self.step();
                t.words[i % 0x8000] = value;
            }
        }
        self.table = Some(t);
        value
    }

    /// 0000:6EEA (far) / 6EEE (near). AX=lo, BX=hi.
    /// Returns a value in [lo, hi). The original faults with a divide
    /// error when hi == lo; this returns None instead.
    pub fn range(&mut self, lo: u16, hi: u16) -> Option<u16> {
        let r = self.next();
        let span = hi.wrapping_sub(lo);
        if span == 0 {
            return None;
        }
        Some((r % span).wrapping_add(lo))
    }
}
