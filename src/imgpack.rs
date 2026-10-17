//! A packed image's unpacking before 0000:5828 draws it (5871..58E0):
//! the image's 12-byte header (x, y, width, height, flags, then a word
//! the packed forms use) and its bytes after it. Flags 1, 4 and 8 unpack
//! into a heap block of width x height bytes that the drawing reads and
//! then frees: 1 by runs (0000:5B5C), 4 by LZSS (0000:7608,
//! `lzss_unpack`, from the word at +0Ah), 8 by LZHUF (3065:0D2D,
//! `mif_decompress`, the length at +0Ah and the stream at +0Ch). Flag 2
//! (with no 1) is drawn by runs straight to the screen (0000:5B9A, the
//! renderer's), leaving any block it took. The renderer draws and frees
//! (src/render/sky.rs `image`).

use crate::machine::Machine;

/// What 5871..58E0 left for the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unpacked {
    /// No packing: the bytes after the header.
    Plain,
    /// The block (segment, offset) the image was unpacked into, to free
    /// after the drawing.
    Block(u16, u16),
    /// Flag 2: drawn by runs from the header's bytes (5B9A); the block
    /// taken for flags 4 or 8 with it, if any, is left taken.
    Runs,
}

impl Machine {
    /// 0000:5B5C. The runs after the header at seg:off into dst_seg:dst:
    /// width x height bytes (16 bits) as a count of bytes; each run's
    /// byte n under 80h copies n + 1 bytes (0000:7706), else repeats the
    /// next byte (n & 7Fh) + 1 times (0000:77CC); until the count is
    /// exactly 0 (a run past it goes on round the 16 bits). The first
    /// run's count keeps CH as the height's high byte, as `mov cl, al`
    /// leaves it. None where the count never lands on 0 (the original
    /// runs on through memory).
    pub fn rle_unpack(&mut self, seg: u16, off: u16, dst_seg: u16, dst: u16) -> Option<()> {
        let w = self.mem.r16(seg, off.wrapping_add(4));
        let h = self.mem.r16(seg, off.wrapping_add(6));
        self.rle_runs(seg, off.wrapping_add(0xC), w, h, dst_seg, dst)
    }

    /// 0000:5B3C (5B65): the runs at seg:si into dst_seg:dst, CX = `cx`
    /// and DX = `dx` as the caller gives them (5B5C's: the width and the
    /// height): cx x dx bytes (16 bits) counted, the first run's count
    /// with DX's high byte (CX after `xchg dx, cx`), as `rle_unpack`.
    pub fn rle_runs(&mut self, seg: u16, si: u16, cx: u16, dx: u16, dst_seg: u16, dst: u16) -> Option<()> {
        let mut si = si;
        let mut di = dst;
        let mut bx = dx.wrapping_mul(cx);
        // CX after `xchg dx, cx`: DX; each run's `mov cl, al` keeps its
        // high byte until a copy or fill leaves CX 0.
        let mut ch = dx & 0xFF00;
        for _ in 0..0x20000 {
            let al = self.mem.r8(seg, si);
            si = si.wrapping_add(1);
            let n = ((al & 0x7F) as u16) + 1;
            bx = bx.wrapping_sub(n);
            let cx = ch | n;
            if al & 0x80 == 0 {
                for _ in 0..cx {
                    let b = self.mem.r8(seg, si);
                    self.mem.w8(dst_seg, di, b);
                    si = si.wrapping_add(1);
                    di = di.wrapping_add(1);
                }
            } else {
                let b = self.mem.r8(seg, si);
                si = si.wrapping_add(1);
                for _ in 0..cx {
                    self.mem.w8(dst_seg, di, b);
                    di = di.wrapping_add(1);
                }
            }
            ch = 0;
            if bx == 0 {
                return Some(());
            }
        }
        crate::giveup::original("0000:5B65: an image's runs never bring its count to 0; the original runs on through memory")
    }

    /// 0000:5871..58E0 for the image whose header is at seg:off (the
    /// capture to 8F9A aside): with flags 1, 4 or 8 a heap block of the
    /// width times the height (16 bits, 0000:51EC); then 1 runs into it,
    /// else 2 leaves the drawing to 5B9A, else 8 LZHUF, 4 LZSS. None
    /// where the heap reports an error or runs never end.
    pub fn image_unpack(&mut self, seg: u16, off: u16) -> Option<Unpacked> {
        let flags = self.mem.r16(seg, off.wrapping_add(8));
        let block = if flags & 0xD != 0 {
            let n = self.mem.r16(seg, off.wrapping_add(4)).wrapping_mul(self.mem.r16(seg, off.wrapping_add(6)));
            Some(self.heap_alloc_bytes(n)?)
        } else {
            None
        };
        if flags & 1 != 0 {
            let (bs, bo) = block.unwrap();
            self.rle_unpack(seg, off, bs, bo)?;
        } else if flags & 2 != 0 {
            return Some(Unpacked::Runs);
        } else if flags & 8 != 0 {
            let (bs, bo) = block.unwrap();
            let len = self.mem.r16(seg, off.wrapping_add(0xA));
            self.mif_decompress(seg, off.wrapping_add(0xC), bs, bo, len)?;
        } else if flags & 4 != 0 {
            let (bs, bo) = block.unwrap();
            self.lzss_unpack(seg, off.wrapping_add(0xA), bs, bo)?;
        }
        Some(match block {
            Some((bs, bo)) => Unpacked::Block(bs, bo),
            None => Unpacked::Plain,
        })
    }
}
