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
    /// While tracking (`track`), DGROUP's offsets written, a bit each;
    /// empty otherwise.
    written: Vec<u64>,
    /// While watching for the drawing (`watch_drawn`), DGROUP's offsets
    /// written, as `written` keeps them; empty otherwise.
    drawn: Vec<u64>,
    /// While watching the EMS frame (`watch_frame`), the bytes written
    /// there, by the bank mapped as each was written.
    frame: Option<FrameWatch>,
}

/// The EMS frame watched: its linear start, the bank mapped, and the
/// bytes written (the bank times 10000h plus the offset).
#[derive(Clone)]
struct FrameWatch {
    base: usize,
    bank: u16,
    written: std::collections::HashSet<u32>,
}

impl Clone for Memory {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes.clone(), written: self.written.clone(), drawn: self.drawn.clone(), frame: self.frame.clone() }
    }

    /// Copied into this memory's own allocation: no fresh megabyte to
    /// fault in (a front end copying the machine every turn).
    fn clone_from(&mut self, source: &Self) {
        self.bytes.clone_from(&source.bytes);
        self.written.clone_from(&source.written);
        self.drawn.clone_from(&source.drawn);
        self.frame.clone_from(&source.frame);
    }
}

fn lin(seg: u16, off: u16) -> usize {
    seg as usize * 16 + off as usize
}

impl Memory {
    pub fn new(image: &Image) -> Self {
        let mut bytes = vec![0u8; SIZE];
        bytes[..image.bytes.len()].copy_from_slice(&image.bytes);
        Self { bytes, written: Vec::new(), drawn: Vec::new(), frame: None }
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
        let at = lin(seg, off);
        self.bytes[at] = v;
        if !self.written.is_empty() || !self.drawn.is_empty() {
            let o = at.wrapping_sub(lin(DS, 0));
            if o < 0x10000 {
                if !self.written.is_empty() {
                    self.written[o >> 6] |= 1 << (o & 63);
                }
                if !self.drawn.is_empty() {
                    self.drawn[o >> 6] |= 1 << (o & 63);
                }
            }
        }
        if let Some(f) = &mut self.frame {
            let o = at.wrapping_sub(f.base);
            if o < 0x10000 {
                f.written.insert((f.bank as u32) << 16 | o as u32);
            }
        }
    }

    /// The writes to the EMS frame at segment `seg` watched from now
    /// (`frame_written`), bank `bank` mapped there.
    pub fn watch_frame(&mut self, seg: u16, bank: u16) {
        self.frame = Some(FrameWatch { base: lin(seg, 0), bank, written: Default::default() });
    }

    /// Bank `bank` mapped into the watched frame (`Machine::ems_map`).
    pub fn frame_mapped(&mut self, bank: u16) {
        if let Some(f) = &mut self.frame {
            f.bank = bank;
        }
    }

    /// The frame's bytes written since `watch_frame` (the bank times
    /// 10000h plus the offset), and the watch ended.
    pub fn frame_written(&mut self) -> std::collections::HashSet<u32> {
        self.frame.take().map(|f| f.written).unwrap_or_default()
    }

    /// DGROUP's writes tracked from now (`written_offsets`), any earlier
    /// forgotten.
    pub fn track(&mut self) {
        self.written = vec![0; 0x10000 / 64];
    }

    /// DGROUP's writes watched from now for the next drawing
    /// (`drawn_written`), beside and apart from `track`'s.
    pub fn watch_drawn(&mut self) {
        self.drawn = vec![0; 0x10000 / 64];
    }

    /// DGROUP's offsets written since `watch_drawn`, and the watch ended.
    pub fn drawn_written(&mut self) -> Vec<u16> {
        let d = std::mem::take(&mut self.drawn);
        (0..d.len() * 64).filter(|&o| d[o >> 6] >> (o & 63) & 1 != 0).map(|o| o as u16).collect()
    }

    /// Writes no longer tracked.
    pub fn untrack(&mut self) {
        self.written = Vec::new();
    }

    /// DGROUP's offsets written since `track` (written to whatever value,
    /// its own included).
    pub fn written_offsets(&self) -> Vec<u16> {
        (0..self.written.len() * 64).filter(|&o| self.written[o >> 6] >> (o & 63) & 1 != 0).map(|o| o as u16).collect()
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

