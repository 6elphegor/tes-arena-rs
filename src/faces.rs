//! The faces over the view (segment 292B): up to eight portraits, one
//! for each byte of FACES that isn't FFh, loaded into EMS bank 19h when
//! the interface is set up again (292B:048F, from 0000:4DD6) and drawn
//! each frame (292B:0530, from 0000:4C86) at the places in PLACES. No
//! code of the game's writes FACES (its bytes are FFh as the program
//! starts), so in play only bank 19h mapped is seen; the routines are
//! ported whole all the same. Their pictures are unpacked by 0000:7608
//! (`lzss_unpack`), the LZSS also used by the image loader (0000:58CA).

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Eight bytes: a face's number (0..7Fh, bit 80h the female
    /// pictures), FFh none.
    pub const FACES: u16 = 0x0FD5;
    /// Eight x, y words: where each face is drawn.
    pub const PLACES: u16 = 0x7BAA;
    /// "faces00.cif", its tens digit at +6.
    pub const NAME: u16 = 0x3EFD;
    /// "facesf00.cif", its tens digit at +7.
    pub const NAME_F: u16 = 0x3F09;
    /// The ring buffer 7608 uses, when its segment (ADBE) isn't 0.
    pub const RING: u16 = 0xADBC;
}

/// The EMS bank the faces' pictures are kept in.
const BANK: u16 = 0x19;

impl Machine {
    /// 0000:7608. Unpacks the LZSS stream at src_seg:src (a word of its
    /// length in bytes, then the stream) to dst_seg:dst, returning the
    /// offset after the last byte written. The 4 KiB ring is the far
    /// pointer at RING when its segment isn't 0, else a block of 1011h
    /// bytes from the heap (0000:51EC), freed after (0000:6C36); its
    /// first FEEh bytes are spaces, the last 12h whatever was there.
    /// Each flag byte's bits, low first: 1 a literal; 0 a reference of
    /// two bytes, the ring's offset (low byte, then the high nibble of
    /// the second) and its length (the low nibble plus 3). The stream
    /// ends when its length runs out, at any byte. Offsets wrap in 16
    /// bits, the segments stay. None where the heap's chain is broken.
    pub fn lzss_unpack(&mut self, src_seg: u16, src: u16, dst_seg: u16, dst: u16) -> Option<u16> {
        let ring_seg = self.mem.r16(DS, var::RING + 2);
        let (rseg, roff) = if ring_seg != 0 { (ring_seg, self.mem.r16(DS, var::RING)) } else { self.heap_alloc_bytes(0x1011)? };
        for i in 0..0xFEEu16 {
            self.mem.w8(rseg, roff.wrapping_add(i), 0x20);
        }
        let mut cx = self.mem.r16(src_seg, src);
        let mut si = src.wrapping_add(2);
        let mut bx = 0xFEEu16;
        let mut dx = 0u16;
        let mut di = dst;
        let mut next = |m: &Machine, cx: &mut u16| -> Option<u8> {
            if *cx == 0 {
                return None;
            }
            *cx -= 1;
            let b = m.mem.r8(src_seg, si);
            si = si.wrapping_add(1);
            Some(b)
        };
        loop {
            dx >>= 1;
            if dx & 0xFF00 == 0 {
                let Some(b) = next(self, &mut cx) else { break };
                dx = 0xFF00 | b as u16;
            }
            if dx & 1 != 0 {
                let Some(b) = next(self, &mut cx) else { break };
                self.mem.w8(dst_seg, di, b);
                di = di.wrapping_add(1);
                self.mem.w8(rseg, roff.wrapping_add(bx), b);
                bx = (bx + 1) & 0xFFF;
            } else {
                let Some(lo) = next(self, &mut cx) else { break };
                let Some(hi) = next(self, &mut cx) else { break };
                let at = lo as u16 | ((hi & 0xF0) as u16) << 4;
                let last = (hi & 0xF) as u16 + 2;
                for k in 0..=last {
                    let b = self.mem.r8(rseg, roff.wrapping_add((at + k) & 0xFFF));
                    self.mem.w8(dst_seg, di, b);
                    di = di.wrapping_add(1);
                    self.mem.w8(rseg, roff.wrapping_add(bx), b);
                    bx = (bx + 1) & 0xFFF;
                }
            }
        }
        if ring_seg == 0 {
            self.heap_free(rseg);
        }
        Some(di)
    }

    /// 292B:048F. Once until 5CA bit 2000h is cleared again (0000:4DD6
    /// clears it): the bit set, bank 19h mapped (0000:5C86), and each
    /// face of FACES loaded into it one after another from offset 0 of
    /// the EMS frame ([9030]): its file's tens digit written into the
    /// name (NAME, or NAME_F for bit 80h), the file loaded to the far
    /// buffer at BA54 (0000:7428), its ones digit'th picture found (each
    /// a 12-byte header whose +0Ah word is its packed length, then that
    /// many bytes), the header copied, its +0Ah made the width times the
    /// height (+4, +6) and the picture unpacked after it (0000:7608).
    /// None where a file fails to load.
    pub fn faces_load(&mut self, files: &mut Files) -> Option<()> {
        let f = self.mem.r16(DS, 0x05CA);
        if f & 0x2000 != 0 {
            return Some(());
        }
        self.mem.w16(DS, 0x05CA, f | 0x2000);
        self.ems_map(BANK);
        let mut dx = 0u16;
        for i in 0..8u16 {
            let al = self.mem.r8(DS, var::FACES + i);
            if al == 0xFF {
                continue;
            }
            let (name, at) = if al & 0x80 != 0 { (var::NAME_F, 7) } else { (var::NAME, 6) };
            let al = al & 0x7F;
            self.mem.w8(DS, name + at, al / 10 + 0x30);
            let frame = (al % 10) as u16;
            let ems = self.mem.r16(DS, 0x9030);
            let (bseg, boff) = (self.mem.r16(DS, 0xBA56), self.mem.r16(DS, 0xBA54));
            self.file_load(files, name, bseg, boff)?;
            let mut si = boff;
            for _ in 0..frame {
                si = si.wrapping_add(self.mem.r16(bseg, si.wrapping_add(0xA))).wrapping_add(0xC);
            }
            for k in 0..0xCu16 {
                let b = self.mem.r8(bseg, si.wrapping_add(k));
                self.mem.w8(ems, dx.wrapping_add(k), b);
            }
            let di = dx.wrapping_add(0xC);
            let ax = self.mem.r16(ems, di.wrapping_sub(8)).wrapping_mul(self.mem.r16(ems, di.wrapping_sub(6)));
            self.mem.w16(ems, di.wrapping_sub(2), ax);
            self.lzss_unpack(bseg, si.wrapping_add(0xA), ems, di)?;
            dx = di.wrapping_add(ax);
        }
        Some(())
    }

    /// of FACES drawn from it at the next of PLACES by 0000:5B04: its
    /// of FACES drawn from it at its place (PLACES) by 0000:5B04: its
    /// picture's rows (the header's +4 width, +6 height), colour 0 left
    /// as it is, into the screen ([A900]) from the row table at A6A0.
    /// The pictures follow each other as 048F left them (+0Ah the size).
    pub fn faces_draw(&mut self) {
        self.ems_map(BANK);
        let ems = self.mem.r16(DS, 0x9030);
        // Not over a heap's blocks: a game the port starts without a page
        // block has A000h there (`page_is_block`).
        let paged = self.page_is_block();
        let mut di = 0u16;
        // The places go on with the faces drawn, not with FACES: the
        // n'th face drawn goes at the n'th place.
        let mut place = var::PLACES;
        for i in 0..8u16 {
            if self.mem.r8(DS, var::FACES + i) == 0xFF {
                continue;
            }
            let x = self.mem.r16(DS, place);
            let y = self.mem.r16(DS, place + 2);
            place += 4;
            let (w, h) = (self.mem.r16(ems, di.wrapping_add(4)), self.mem.r16(ems, di.wrapping_add(6)));
            let screen = self.mem.r16(DS, 0xA900);
            let mut row = self.mem.r16(DS, 0xA6A0u16.wrapping_add(y.wrapping_add(y))).wrapping_add(x);
            let mut si = di.wrapping_add(0xC);
            // `loop` counts: 0 is 65536 times.
            let n = |v: u16| if v == 0 { 0x10000u32 } else { v as u32 };
            for _ in 0..n(h) {
                let mut at = row;
                for _ in 0..n(w) {
                    let b = self.mem.r8(ems, si);
                    si = si.wrapping_add(1);
                    if b != 0 && paged {
                        self.mem.w8(screen, at, b);
                    }
                    at = at.wrapping_add(1);
                }
                row = row.wrapping_add(0x140);
            }
            di = di.wrapping_add(self.mem.r16(ems, di.wrapping_add(0xA))).wrapping_add(0xC);
        }
    }
}
