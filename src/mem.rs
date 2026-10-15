//! Flat real-mode memory with the program image loaded at linear 0, so
//! segment numbers match the disassembly (unrelocated).
//!
//! Segments overlap exactly as they did on the real machine: for example
//! 3276:92A0 is DGROUP 3BA0:0000. Bytes past the end of the image (DGROUP's
//! BSS) start zeroed. Offsets wrap at 64 KiB within a segment.

use crate::exe::{DGROUP_SEG, Image};

pub const DS: u16 = DGROUP_SEG;
/// Far data segment holding rules tables (race bases, class tables).
pub const DATA: u16 = 0x3276;

const SIZE: usize = 0x10_FFF0;

pub struct Memory {
    bytes: Vec<u8>,
}

impl Clone for Memory {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes.clone() }
    }

    /// Copied into this memory's own allocation: no fresh megabyte to
    /// fault in (a front end copying the machine every turn).
    fn clone_from(&mut self, source: &Self) {
        self.bytes.clone_from(&source.bytes);
    }
}

fn lin(seg: u16, off: u16) -> usize {
    seg as usize * 16 + off as usize
}

impl Memory {
    pub fn new(image: &Image) -> Self {
        let mut bytes = vec![0u8; SIZE];
        bytes[..image.bytes.len()].copy_from_slice(&image.bytes);
        Self { bytes }
    }

    pub fn r8(&self, seg: u16, off: u16) -> u8 {
        self.bytes[lin(seg, off)]
    }

    pub fn r16(&self, seg: u16, off: u16) -> u16 {
        u16::from_le_bytes([self.r8(seg, off), self.r8(seg, off.wrapping_add(1))])
    }

    pub fn r32(&self, seg: u16, off: u16) -> u32 {
        self.r16(seg, off) as u32 | (self.r16(seg, off.wrapping_add(2)) as u32) << 16
    }

    pub fn w8(&mut self, seg: u16, off: u16, v: u8) {
        self.bytes[lin(seg, off)] = v;
    }

    pub fn w16(&mut self, seg: u16, off: u16, v: u16) {
        let [a, b] = v.to_le_bytes();
        self.w8(seg, off, a);
        self.w8(seg, off.wrapping_add(1), b);
    }

    pub fn w32(&mut self, seg: u16, off: u16, v: u32) {
        self.w16(seg, off, v as u16);
        self.w16(seg, off.wrapping_add(2), (v >> 16) as u16);
    }

    /// The 64 KiB of segment `seg` (offsets 0..=FFFFh, as `r8` reads
    /// them).
    pub fn segment(&self, seg: u16) -> &[u8] {
        &self.bytes[lin(seg, 0)..=lin(seg, 0xFFFF)]
    }

    /// The 64 KiB of segment `seg`, to write.
    pub fn segment_mut(&mut self, seg: u16) -> &mut [u8] {
        &mut self.bytes[lin(seg, 0)..=lin(seg, 0xFFFF)]
    }

    /// NUL-terminated string at seg:off, without the terminator.
    pub fn cstr(&self, seg: u16, off: u16) -> &[u8] {
        let s = &self.bytes[lin(seg, off)..];
        &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())]
    }

    /// (linear address, new byte) for every byte in `range` that differs
    /// from `before`.
    pub fn diff(&self, before: &Memory, range: std::ops::Range<usize>) -> Vec<(usize, u8)> {
        range.filter(|&i| self.bytes[i] != before.bytes[i]).map(|i| (i, self.bytes[i])).collect()
    }
}

