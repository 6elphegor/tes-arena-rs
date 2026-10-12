//! Loads the program image out of the user's own ACD.EXE.
//!
//! ACD.EXE is packed with PKLITE 1.12 using "extra compression" (literal
//! bytes are XOR-scrambled) and the large-model relocation table. The
//! decompressor follows depklite (refkeen), itself derived from
//! OpenTESArena's ExeUnpacker.

use std::fmt;

/// Segment of DGROUP within the image.
pub const DGROUP_SEG: u16 = 0x3BA0;
pub const IMAGE_LEN: usize = 305_520;
pub const DGROUP_LEN: usize = IMAGE_LEN - DGROUP_SEG as usize * 16;

/// FNV-1a of the unrelocated image of ACD.EXE v1.07 (CD).
const IMAGE_FNV1A: u64 = 0xa135_7621_c374_9625;
const COMPRESSED_START: usize = 752;

#[derive(Debug)]
pub enum Error {
    NotPklite,
    Truncated,
    Corrupt(&'static str),
    UnknownVersion,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotPklite => write!(f, "not a PKLITE-packed ACD.EXE"),
            Error::Truncated => write!(f, "file is truncated"),
            Error::Corrupt(why) => write!(f, "corrupt compressed data: {why}"),
            Error::UnknownVersion => write!(f, "unpacked fine, but this is not the CD v1.07 ACD.EXE"),
        }
    }
}

impl std::error::Error for Error {}

/// The unpacked program, before relocation.
pub struct Image {
    pub bytes: Vec<u8>,
    /// (segment, offset) of every word that holds a segment value.
    pub relocs: Vec<(u16, u16)>,
    pub ss: u16,
    pub sp: u16,
    pub cs: u16,
    pub ip: u16,
}

impl Image {
    pub fn dgroup(&self) -> &[u8] {
        &self.bytes[DGROUP_SEG as usize * 16..]
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    word: u16,
    used: u32,
}

impl Bits<'_> {
    fn byte(&mut self) -> Result<u8, Error> {
        let b = *self.data.get(self.pos).ok_or(Error::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn bit(&mut self) -> Result<bool, Error> {
        let bit = self.word & (1 << self.used) != 0;
        self.used += 1;
        // The next word is fetched eagerly, so it lands before any byte
        // read that follows this bit.
        if self.used == 16 {
            self.used = 0;
            self.word = u16::from_le_bytes([self.byte()?, self.byte()?]);
        }
        Ok(bit)
    }

    /// Walks a tree of (left, right, value) nodes; children are relative.
    fn tree(&mut self, nodes: &[(u8, u8, u8)]) -> Result<u8, Error> {
        let mut i = 0;
        loop {
            let (l, r, _) = nodes[i];
            let step = if self.bit()? { r } else { l };
            if step == 0 {
                return Err(Error::Corrupt("bad code"));
            }
            i += step as usize;
            if let (0, 0, v) = nodes[i] {
                return Ok(v);
            }
        }
    }
}

const fn st(l: u8, r: u8) -> (u8, u8, u8) {
    (l, r, 0)
}
const fn lf(v: u8) -> (u8, u8, u8) {
    (0, 0, v)
}

/// Copy length codes; 25 is the escape for a long count or end marker.
#[rustfmt::skip]
const LENGTHS: [(u8, u8, u8); 47] = [
    st(4, 1),
        st(1, 2), lf(2), lf(3),
    st(1, 6), st(1, 2), lf(4), st(1, 2), lf(5), lf(6),
    st(1, 6), st(1, 2), lf(7), st(1, 2), lf(8), lf(9),
    st(1, 6), st(1, 2), lf(10), st(1, 2), lf(11), lf(12),
    st(1, 6), st(1, 2), lf(25), st(1, 2), lf(13), lf(14),
    st(1, 6), st(1, 2), lf(15), st(1, 2), lf(16), lf(17),
    st(1, 6), st(1, 2), lf(18), st(1, 2), lf(19), lf(20),
    st(1, 4), st(1, 2), lf(21), lf(22),
    st(1, 2), lf(23), lf(24),
];

/// High byte of the copy distance.
#[rustfmt::skip]
const OFFSETS: [(u8, u8, u8); 63] = [
    st(2, 1), lf(0),
    st(1, 12),
        st(1, 4), st(1, 2), lf(1), lf(2),
        st(1, 4), st(1, 2), lf(3), lf(4),
        st(1, 2), lf(5), lf(6),
    st(1, 18),
        st(1, 8), st(1, 4), st(1, 2), lf(7), lf(8), st(1, 2), lf(9), lf(10),
        st(1, 4), st(1, 2), lf(11), lf(12),
        st(1, 2), lf(13), st(1, 2), lf(14), lf(15),
    st(1, 16),
        st(1, 8), st(1, 4), st(1, 2), lf(16), lf(17), st(1, 2), lf(18), lf(19),
        st(1, 4), st(1, 2), lf(20), lf(21),
        st(1, 2), lf(22), lf(23),
    st(1, 8), st(1, 4), st(1, 2), lf(24), lf(25), st(1, 2), lf(26), lf(27),
    st(1, 4), st(1, 2), lf(28), lf(29),
    st(1, 2), lf(30), lf(31),
];

fn le16(b: &[u8], at: usize) -> Result<u16, Error> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(Error::Truncated)
}

pub fn unpack(file: &[u8]) -> Result<Image, Error> {
    if file.len() < COMPRESSED_START || &file[..2] != b"MZ" || &file[0x1e..0x24] != b"PKLITE" {
        return Err(Error::NotPklite);
    }
    let mut bits = Bits { data: file, pos: COMPRESSED_START + 2, word: le16(file, COMPRESSED_START)?, used: 0 };
    let mut out: Vec<u8> = Vec::with_capacity(IMAGE_LEN);

    loop {
        if !bits.bit()? {
            // Literal, scrambled with the bit position it was read at.
            let key = 16 - bits.used as u8;
            out.push(bits.byte()? ^ key);
            continue;
        }
        let mut count = bits.tree(&LENGTHS)? as usize;
        if count == 25 {
            match bits.byte()? {
                0xFE => continue,
                0xFF => break,
                n => count = n as usize + 25,
            }
        }
        let hi = if count != 2 { bits.tree(&OFFSETS)? } else { 0 };
        let dist = u16::from_le_bytes([bits.byte()?, hi]) as usize;
        if dist == 0 || dist > out.len() {
            return Err(Error::Corrupt("copy before start"));
        }
        // Byte by byte: source and destination may overlap.
        let from = out.len() - dist;
        for i in 0..count {
            out.push(out[from + i]);
        }
        if out.len() > IMAGE_LEN {
            return Err(Error::UnknownVersion);
        }
    }

    // Large-model relocation table: blocks of offsets, each block's
    // segment 0x0FFF paragraphs past the previous, ending in 0xFFFF.
    let mut p = bits.pos;
    let mut relocs = Vec::new();
    let mut seg = 0u16;
    loop {
        let n = le16(file, p)?;
        p += 2;
        if n == 0xFFFF {
            break;
        }
        for _ in 0..n {
            relocs.push((seg, le16(file, p)?));
            p += 2;
        }
        seg = seg.wrapping_add(0x0FFF);
    }
    let regs: Vec<u16> = (0..4).map(|i| le16(file, p + 2 * i)).collect::<Result<_, _>>()?;

    let hash = out.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    if out.len() != IMAGE_LEN || hash != IMAGE_FNV1A {
        return Err(Error::UnknownVersion);
    }
    Ok(Image { bytes: out, relocs, ss: regs[0], sp: regs[1], cs: regs[2], ip: regs[3] })
}
