//! .MIF level files: the decompressor, the city block loader and the
//! whole city (template and blocks; the template loader is `level`).
//!
//! A city block (1194:03FA..0487) is read as a 6-byte, a 0x3D-byte and
//! another 6-byte header, then chunks of a 4-byte tag and a word length.
//! FLOR, MAP1 and MAP2 chunks are decompressed into the buffer segment
//! at DGROUP 9030 and copied, 20 x 20 words, into the floor, wall and
//! ceiling planes at the current block's write offset; any other chunk
//! is skipped.
//!
//! The decompressor (3065:0D2D, compiled C) is Okumura's LZHUF: LZSS
//! with a 4096-byte ring and adaptive Huffman codes, preceded by a word
//! holding the decoded size.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::{Heap, DS};

/// DGROUP variables.
pub mod var {
    /// Plane segments: floor (FLOR), walls (MAP1), ceiling (MAP2), and
    /// the buffer chunks are decompressed into.
    pub const FLOOR_SEG: u16 = 0xCCD0;
    pub const MAP1_SEG: u16 = 0xD806;
    pub const MAP2_SEG: u16 = 0xD748;
    pub const BUFFER_SEG: u16 = 0x9030;
    /// Header scratch, chunk tag (4 bytes) and chunk length.
    pub const HEADER: u16 = 0x929E;
    pub const TAG: u16 = 0xCCFA;
    pub const CHUNK_LEN: u16 = 0xA836;
    /// Write offset of the current block in the planes.
    pub const WRITE_POS: u16 = 0xA888;
}
use var::*;

/// Decompressor state in DGROUP.
pub mod lz {
    /// Bit buffer: bits held (byte), the buffer (word); the encoder's
    /// output buffer, cleared on entry (byte, word).
    pub const GETLEN: u16 = 0xB53E;
    pub const GETBUF: u16 = 0xB540;
    pub const PUTLEN: u16 = 0xB53F;
    pub const PUTBUF: u16 = 0xB542;
    /// Decoded size from the first word of the input.
    pub const TEXTSIZE: u16 = 0xEEE6;
    pub const CODESIZE: u16 = 0xEEE8;
    /// Input end and read pointer, output pointer (far, offset first).
    pub const IN_END: u16 = 0xEECA;
    pub const IN_PTR: u16 = 0xEECE;
    pub const OUT_PTR: u16 = 0xEED2;
    /// Position decoding tables (256 bytes each).
    pub const D_CODE: u16 = 0xB5C4;
    pub const D_LEN: u16 = 0xB6C4;
}

const N: usize = 4096;
const F: usize = 60;
const N_CHAR: usize = 314;
const T: usize = N_CHAR * 2 - 1;
const R: usize = T - 1;
const MAX_FREQ: u16 = 0x8000;

/// Block size in plane words, and the plane row stride in bytes.
const BLOCK: u16 = 20;
const ROW: u16 = 0x100;

/// The decoder's heap tables (malloc'd by the original, freed at the end).
struct Huff {
    text: Vec<u8>,
    freq: Vec<u16>,
    prnt: Vec<u16>,
    son: Vec<u16>,
}

impl Huff {
    /// StartHuff (3065:0550); the ring holds spaces except the last F
    /// bytes, which the original leaves as whatever the heap held (the
    /// port, like the test harness, has zeros).
    fn new() -> Self {
        let mut h = Huff { text: vec![0; N], freq: vec![0; T + 1], prnt: vec![0; T + N_CHAR], son: vec![0; T] };
        h.text[..N - F].fill(b' ');
        for i in 0..N_CHAR {
            h.freq[i] = 1;
            h.son[i] = (i + T) as u16;
            h.prnt[i + T] = i as u16;
        }
        let (mut i, mut j) = (0, N_CHAR);
        while j <= R {
            h.freq[j] = h.freq[i].wrapping_add(h.freq[i + 1]);
            h.son[j] = i as u16;
            h.prnt[i] = j as u16;
            h.prnt[i + 1] = j as u16;
            i += 2;
            j += 1;
        }
        h.freq[T] = 0xFFFF;
        h.prnt[R] = 0;
        h
    }

    /// reconst (3065:060D): halves the frequencies and rebuilds the tree.
    fn reconst(&mut self) {
        let mut j = 0;
        for i in 0..T {
            if self.son[i] as usize >= T {
                self.freq[j] = self.freq[i].wrapping_add(1) >> 1;
                self.son[j] = self.son[i];
                j += 1;
            }
        }
        let mut i = 0;
        for j in N_CHAR..T {
            let f = self.freq[i].wrapping_add(self.freq[i + 1]);
            self.freq[j] = f;
            let mut k = j - 1;
            while self.freq[k] > f {
                k -= 1;
            }
            k += 1;
            self.freq.copy_within(k..j, k + 1);
            self.freq[k] = f;
            self.son.copy_within(k..j, k + 1);
            self.son[k] = i as u16;
            i += 2;
        }
        for i in 0..T {
            let k = self.son[i] as usize;
            self.prnt[k] = i as u16;
            if k < T {
                self.prnt[k + 1] = i as u16;
            }
        }
    }

    /// update (3065:07A4).
    fn update(&mut self, c: usize) {
        if self.freq[R] == MAX_FREQ {
            self.reconst();
        }
        let mut c = self.prnt[c + T] as usize;
        loop {
            self.freq[c] = self.freq[c].wrapping_add(1);
            let k = self.freq[c];
            let mut l = c + 1;
            if k > self.freq[l] {
                // while (k > freq[++l]); l--;
                while k > self.freq[l + 1] {
                    l += 1;
                }
                self.freq[c] = self.freq[l];
                self.freq[l] = k;
                let i = self.son[c] as usize;
                self.prnt[i] = l as u16;
                if i < T {
                    self.prnt[i + 1] = l as u16;
                }
                let j = self.son[l] as usize;
                self.son[l] = i as u16;
                self.prnt[j] = c as u16;
                if j < T {
                    self.prnt[j + 1] = c as u16;
                }
                self.son[c] = j as u16;
                c = l;
            }
            c = self.prnt[c] as usize;
            if c == 0 {
                break;
            }
        }
    }
}

/// Decoder input and output: far pointers into `Machine` memory.
struct Io {
    in_seg: u16,
    in_ptr: u16,
    in_end: u16,
    out_seg: u16,
    out_ptr: u16,
    getbuf: u16,
    getlen: u8,
}

impl Machine {
    /// 3065:001C. Next input byte, or 0xFFFF when the read pointer is at
    /// the end. Only equality ends the input: a pointer already past the
    /// end reads on through the segment.
    fn mif_getc(&self, io: &mut Io) -> u16 {
        if io.in_ptr == io.in_end {
            return 0xFFFF;
        }
        let b = self.mem.r8(io.in_seg, io.in_ptr);
        io.in_ptr = io.in_ptr.wrapping_add(1);
        b as u16
    }

    fn mif_fill(&self, io: &mut Io, zero_eof: bool) {
        while io.getlen <= 8 {
            let mut i = self.mif_getc(io);
            // GetBit treats EOF as 0. GetByte tests it with `or ax,ax;
            // jae`, which is always taken, so EOF goes in as 0xFFFF.
            if zero_eof && (i as i16) < 0 {
                i = 0;
            }
            io.getbuf |= i << (8 - io.getlen);
            io.getlen = io.getlen.wrapping_add(8);
        }
    }

    /// 3065:0455.
    fn mif_getbit(&self, io: &mut Io) -> u16 {
        self.mif_fill(io, true);
        let i = io.getbuf;
        io.getbuf <<= 1;
        io.getlen = io.getlen.wrapping_sub(1);
        ((i as i16) < 0) as u16
    }

    /// 3065:049C.
    fn mif_getbyte(&self, io: &mut Io) -> u16 {
        self.mif_fill(io, false);
        let i = io.getbuf;
        io.getbuf <<= 8;
        io.getlen = io.getlen.wrapping_sub(8);
        i >> 8
    }

    /// 3065:09A1.
    fn mif_decode_char(&self, h: &mut Huff, io: &mut Io) -> usize {
        let mut c = h.son[R] as usize;
        while c < T {
            c += self.mif_getbit(io) as usize;
            c = h.son[c] as usize;
        }
        c -= T;
        h.update(c);
        c
    }

    /// 3065:09D8.
    fn mif_decode_position(&self, io: &mut Io) -> u16 {
        let mut i = self.mif_getbyte(io);
        let c = (self.mem.r8(DS, lz::D_CODE + i) as u16) << 6;
        let mut j = (self.mem.r8(DS, lz::D_LEN + i) as u16).wrapping_sub(2);
        while j != 0 {
            j = j.wrapping_sub(1);
            i = (i << 1).wrapping_add(self.mif_getbit(io));
        }
        c | (i & 0x3F)
    }

    fn mif_putc(&mut self, io: &mut Io, c: u8) {
        self.mem.w8(io.out_seg, io.out_ptr, c);
        io.out_ptr = io.out_ptr.wrapping_add(1);
    }

    /// 3065:0D2D. Decodes `len` bytes at `src_seg`:`src_off` into
    /// `dst_seg`:`dst_off` and returns the original's AX: the low word of
    /// the number of bytes written. The first input word is the size to
    /// decode; 0 returns at once (the original then also leaks its
    /// tables). The size is only checked between matches, so a match can
    /// overshoot it, and output offsets wrap within the segment. The
    /// size word is read even when `len` < 2, which leaves the read
    /// pointer past the end, so the input runs on through the segment.
    pub fn mif_decompress(&mut self, src_seg: u16, src_off: u16, dst_seg: u16, dst_off: u16, len: u16) -> u16 {
        self.mem.w8(DS, lz::PUTLEN, 0);
        self.mem.w16(DS, lz::PUTBUF, 0);
        self.mem.w8(DS, lz::GETLEN, 0);
        self.mem.w16(DS, lz::GETBUF, 0);
        self.mem.w16(DS, lz::CODESIZE, 0);
        let end = src_off.wrapping_add(len);
        self.mem.w16(DS, lz::IN_END, end);
        self.mem.w16(DS, lz::IN_END + 2, src_seg);
        self.mem.w16(DS, lz::OUT_PTR, dst_off);
        self.mem.w16(DS, lz::OUT_PTR + 2, dst_seg);
        let textsize = self.mem.r16(src_seg, src_off);
        self.mem.w16(DS, lz::TEXTSIZE, textsize);
        let mut io = Io {
            in_seg: src_seg,
            in_ptr: src_off.wrapping_add(2),
            in_end: end,
            out_seg: dst_seg,
            out_ptr: dst_off,
            getbuf: 0,
            getlen: 0,
        };
        let mut count = 0u32;
        if textsize != 0 {
            let mut h = Huff::new();
            let mut r = N - F;
            while (textsize as u32) > count {
                let c = self.mif_decode_char(&mut h, &mut io);
                if c < 256 {
                    self.mif_putc(&mut io, c as u8);
                    h.text[r] = c as u8;
                    r = (r + 1) & (N - 1);
                    count = count.wrapping_add(1);
                } else {
                    let i = (r as u16).wrapping_sub(self.mif_decode_position(&mut io)).wrapping_sub(1) as usize & (N - 1);
                    for k in 0..c - 253 {
                        let b = h.text[(i + k) & (N - 1)];
                        self.mif_putc(&mut io, b);
                        h.text[r] = b;
                        r = (r + 1) & (N - 1);
                        count = count.wrapping_add(1);
                    }
                }
            }
        }
        self.mem.w8(DS, lz::GETLEN, io.getlen);
        self.mem.w16(DS, lz::GETBUF, io.getbuf);
        self.mem.w16(DS, lz::IN_PTR, io.in_ptr);
        self.mem.w16(DS, lz::IN_PTR + 2, src_seg);
        self.mem.w16(DS, lz::OUT_PTR, io.out_ptr);
        count as u16
    }

    /// The copy loop shared by 1194:04DD, 0542 and 051A: 20 rows of 20
    /// words from the buffer segment to `plane` at the write offset, rows
    /// 0x100 bytes apart; offsets wrap within the segments.
    fn mif_copy(&mut self, plane: u16, f: impl Fn(u16) -> u16) {
        let (src, dst) = (self.mem.r16(DS, BUFFER_SEG), self.mem.r16(DS, plane));
        let mut row = self.mem.r16(DS, WRITE_POS);
        let mut si = 0u16;
        for _ in 0..BLOCK {
            let mut di = row;
            for _ in 0..BLOCK {
                let w = f(self.mem.r16(src, si));
                self.mem.w16(dst, di, w);
                si = si.wrapping_add(2);
                di = di.wrapping_add(2);
            }
            row = row.wrapping_add(ROW);
        }
    }

    /// 1194:04DD. Floor block: 0x0C00 and 0x0E00 become 0x0D00, and every
    /// word gets the buffer's segment number added (BP holds it).
    pub fn mif_copy_floor(&mut self) {
        let bp = self.mem.r16(DS, BUFFER_SEG);
        self.mif_copy(FLOOR_SEG, |w| (if w == 0x0C00 || w == 0x0E00 { 0x0D00 } else { w }).wrapping_add(bp));
    }

    /// 1194:0542. Wall block, copied as is: the door remapping that
    /// follows the load is jumped over (1194:055D).
    pub fn mif_copy_map1(&mut self) {
        self.mif_copy(MAP1_SEG, |w| w);
    }

    /// 1194:051A. Ceiling block, copied as is.
    pub fn mif_copy_map2(&mut self) {
        self.mif_copy(MAP2_SEG, |w| w);
    }

    /// 1194:05A9. Pass over the whole wall plane (0x4000 words) that
    /// 1194:0292 runs after loading the city template and before the
    /// blocks. Words 8xxx are cleared when BB09 is 2 or 0x82 and their
    /// low byte is one of the five type bytes at C945..C948, C951. These
    /// look like DGROUP variables but are read with DS set to the wall
    /// plane, so they come from the plane's segment, 0x3B09 and more
    /// bytes past the end of its 32 KB.
    /// Words 9x34 and 9x35 become A534/A535 with flags by position, the
    /// row width and height being DGROUP C357/C359; the position tested
    /// is the offset just past the word:
    /// - first row (offset <= 2 * width): bit 0 flipped, | 0x80;
    /// - last row (offset >= 2 * width * height - 2 * width, 16-bit): | 0;
    /// - left column ((offset - 2) % width == 0, a byte offset taken
    ///   modulo the width in words): | 0x40;
    /// - anywhere else: bit 0 flipped (the original skips the flip for
    ///   0x33, which these never are), | 0xC0.
    ///
    /// A width of 0 never reaches the division: every offset then counts
    /// as the last row.
    pub fn mif_wall_pass(&mut self) {
        let seg = self.mem.r16(DS, MAP1_SEG);
        let width = self.mem.r16(DS, MAP_WIDTH);
        let height = self.mem.r16(DS, MAP_HEIGHT);
        let mut si = 0u16;
        for _ in 0..0x4000 {
            let w = self.mem.r16(seg, si);
            si = si.wrapping_add(2);
            if (w as i16) >= 0 {
                continue;
            }
            let low = w as u8;
            match w >> 12 {
                8 => {
                    // DS is the wall plane here: these DGROUP offsets are
                    // read from the plane's segment instead.
                    let variant = self.mem.r8(seg, WALL_VARIANT);
                    if (variant == 0x82 || variant == 2) && CLEARED_TYPES.iter().any(|&o| self.mem.r8(seg, o) == low) {
                        self.mem.w16(seg, si.wrapping_sub(2), 0);
                    }
                }
                9 if (0x34..=0x35).contains(&low) => {
                    let base = (w & 0x3F) | 0xA500;
                    let last = width.wrapping_add(width).wrapping_mul(height).wrapping_sub(width).wrapping_sub(width);
                    let flags = if si <= width.wrapping_add(width) {
                        (base ^ 1) | 0x80
                    } else if si >= last {
                        base
                    } else if si.wrapping_sub(2) % width == 0 {
                        base | 0x40
                    } else {
                        (base ^ 1) | 0xC0
                    };
                    self.mem.w16(seg, si.wrapping_sub(2), flags);
                }
                _ => {}
            }
        }
    }

    /// 1194:0488. Reads the chunk into a fresh (zeroed) buffer and
    /// decompresses it to the buffer segment, offset 0.
    fn mif_read_chunk(&mut self, files: &mut Files) {
        let len = self.mem.r16(DS, CHUNK_LEN);
        // The chunk is read to a heap block of len bytes; it is staged
        // in a scratch segment here, past the end of the image.
        let scratch = CHUNK_SCRATCH;
        for i in 0..((len as u32 + 15) & !15) {
            self.mem.w8(scratch, i as u16, 0);
        }
        self.file_read(files, scratch, 0, len);
        let dst = self.mem.r16(DS, BUFFER_SEG);
        self.mif_decompress(scratch, 0, dst, 0, len);
    }

    /// 1194:03FA..0487. Loads the .MIF named at DS:`name` into the planes
    /// at the write offset. None where the file can't be opened (the
    /// original reports the error through 0000:74E5).
    pub fn mif_load_block(&mut self, files: &mut Files, name: u16) -> Option<()> {
        self.file_open(files, name)?;
        self.file_read(files, DS, HEADER, 6);
        self.file_read(files, DS, HEADER, 0x3D);
        self.file_read(files, DS, HEADER, 6);
        loop {
            if self.file_read(files, DS, TAG, 4) == 0 {
                break;
            }
            self.file_read(files, DS, CHUNK_LEN, 2);
            match &self.mem.r32(DS, TAG).to_le_bytes() {
                b"FLOR" => {
                    self.mif_read_chunk(files);
                    self.mif_copy_floor();
                }
                b"MAP1" => {
                    self.mif_read_chunk(files);
                    self.mif_copy_map1();
                }
                b"MAP2" => {
                    self.mif_read_chunk(files);
                    self.mif_copy_map2();
                }
                _ => {
                    let len = self.mem.r16(DS, CHUNK_LEN);
                    self.file_seek(files, 1, len as u32);
                }
            }
        }
        self.file_close(files);
        Some(())
    }

    /// 1194:0292: builds the current location's city in the planes. It
    /// generates the block plan (`generate_city`), loads the location's
    /// template (`level_load`), runs the wall pass and loads every block.
    /// The Imperial City (province 8) is its template alone. None where
    /// the original faults or hangs, or reaches code not ported (see
    /// `level_load`).
    pub fn mif_build_city(&mut self, files: &mut Files, heap: &mut impl Heap) -> Option<()> {
        // 0000:5C86 maps the EMS page in DGROUP 1030, before and after
        // the template.
        self.ems_map(self.mem.r16(DS, EMS_PAGE))?;
        // The original loads the template after clearing the plan and
        // each block right after naming it. Neither the template nor a
        // block touches the RNG or the plan and name tables, so the plan
        // is generated first. Each name is put back in the template it
        // was opened through, and the templates restored after.
        let blocks = self.generate_city()?;
        self.level_load(files, heap, crate::city::var::TEMPLATE)?;
        self.mif_wall_pass();
        if self.mem.r8(DS, crate::city::var::PROVINCE) == 8 {
            return Some(());
        }
        self.ems_map(self.mem.r16(DS, EMS_PAGE))?;
        let pos = self.mem.r16(DS, WRITE_POS);
        let names: Vec<u8> = (0..NAME_BYTES).map(|i| self.mem.r8(DS, CITY_NAME + i)).collect();
        for b in blocks {
            self.mem.w16(DS, WRITE_POS, b.pos);
            for (i, &c) in b.name.iter().chain(std::iter::once(&0)).enumerate() {
                self.mem.w8(DS, b.name_off + i as u16, c);
            }
            self.mif_load_block(files, b.name_off)?;
        }
        for (i, &b) in names.iter().enumerate() {
            self.mem.w8(DS, CITY_NAME + i as u16, b);
        }
        self.mem.w16(DS, WRITE_POS, pos);
        Some(())
    }
}

/// EMS page mapped around the template load.
const EMS_PAGE: u16 = 0x1030;

/// Wall pass inputs: map width and height in words (DGROUP, through ES),
/// a byte that enables clearing (2 or 0x82; its meaning is not
/// established) and the bytes listing which 8xxx walls it clears (both
/// at these offsets in the wall plane's segment; see `mif_wall_pass`).
const MAP_WIDTH: u16 = 0xC357;
const MAP_HEIGHT: u16 = 0xC359;
const WALL_VARIANT: u16 = 0xBB09;
const CLEARED_TYPES: [u16; 5] = [0xC945, 0xC946, 0xC947, 0xC948, 0xC951];

/// Where `mif_read_chunk` stages a chunk (the original's heap block).
pub const CHUNK_SCRATCH: u16 = 0x5C00;
/// The two block-name templates (long then short), 0x370C..0x3723.
const CITY_NAME: u16 = crate::city::table::NAME_LONG;
const NAME_BYTES: u16 = 0x3723 - 0x370C;
