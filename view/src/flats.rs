//! The flats' images, decoded from the files the INF loader read into the
//! flat banks: .IMG (a 12-byte header: x and y offsets, width, height,
//! flags, size; raw pixels when the low flags byte is 0) and .DFA (an
//! animation: frame count, two offsets, width, height and the size of the
//! first frame, which is run-length coded; then for each further frame a
//! chunk: its size (not counting this word), a count, and (offset,
//! count, bytes) updates to the first frame), and .CFA (creatures, people
//! and effects: width, height, the packed width of a row, two offsets,
//! bits per pixel (byte 10) and the frame count (11); from 12 the frames'
//! offsets (the first is the header's size; the file's size at 74); from
//! 76 to the header's end the colour of each packed value. A frame is
//! run-length coded as a DFA's first, to rows of packed values, high bits
//! first).

/// A flat: its frames, `w` x `h` palette indices each, row-major (0 is
/// transparent).
#[derive(Clone, Debug)]
pub struct Flat {
    pub w: usize,
    pub h: usize,
    pub frames: Vec<Vec<u8>>,
}

fn le16(d: &[u8], at: usize) -> usize {
    *d.get(at).unwrap_or(&0) as usize | (*d.get(at + 1).unwrap_or(&0) as usize) << 8
}

/// Decodes a flat file by its kind (as the loader recorded it: 0 an IMG,
/// below 80h a DFA's frame count, 80h and up a CFA). None for what isn't
/// decoded yet (compressed IMGs, CFAs) or doesn't make sense.
pub fn decode(kind: u8, d: &[u8]) -> Option<Flat> {
    match kind {
        0 => img(d),
        1..0x80 => dfa(d),
        _ => cfa(d),
    }
}

pub fn cfa(d: &[u8]) -> Option<Flat> {
    let (w, h, packed) = (le16(d, 0), le16(d, 2), le16(d, 4));
    let (bits, count, header) = (*d.get(10)? as usize, *d.get(11)? as usize, le16(d, 12));
    if w == 0 || h == 0 || w > 320 || h > 200 || !(1..=8).contains(&bits) || count == 0 || count > 31 || header < 76 || packed * 8 < w * bits {
        return None;
    }
    let colours = d.get(76..header)?;
    let mut frames = Vec::with_capacity(count);
    for k in 0..count {
        let rows = unrle(d.get(le16(d, 12 + 2 * k)..)?, packed * h)?;
        let mut f = vec![0u8; w * h];
        for y in 0..h {
            let row = &rows[y * packed..(y + 1) * packed];
            for x in 0..w {
                let mut v = 0usize;
                for b in x * bits..(x + 1) * bits {
                    v = v << 1 | (row[b / 8] >> (7 - b % 8) & 1) as usize;
                }
                f[y * w + x] = *colours.get(v).unwrap_or(&0);
            }
        }
        frames.push(f);
    }
    Some(Flat { w, h, frames })
}

fn img(d: &[u8]) -> Option<Flat> {
    let (w, h, flags) = (le16(d, 4), le16(d, 6), le16(d, 8));
    if w == 0 || h == 0 || w > 320 || h > 200 || flags & 0xFF != 0 || d.len() < 12 + w * h {
        return None;
    }
    Some(Flat { w, h, frames: vec![d[12..12 + w * h].to_vec()] })
}

/// Run-length coding: a byte with bit 7 set repeats the next byte
/// (low 7 bits + 1) times; otherwise that many + 1 bytes follow as they
/// are.
fn unrle(src: &[u8], len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(len);
    let mut i = 0;
    while out.len() < len {
        let b = *src.get(i)?;
        i += 1;
        let n = (b & 0x7F) as usize + 1;
        if b & 0x80 != 0 {
            let v = *src.get(i)?;
            i += 1;
            out.extend(std::iter::repeat_n(v, n));
        } else {
            out.extend_from_slice(src.get(i..i + n)?);
            i += n;
        }
    }
    out.truncate(len);
    Some(out)
}

fn dfa(d: &[u8]) -> Option<Flat> {
    let (count, w, h, packed) = (le16(d, 0), le16(d, 6), le16(d, 8), le16(d, 10));
    if count == 0 || w == 0 || h == 0 || w > 320 || h > 200 {
        return None;
    }
    let first = unrle(d.get(12..)?, w * h)?;
    let mut frames = vec![first.clone()];
    let mut at = 12 + packed;
    for _ in 1..count {
        let mut f = first.clone();
        let size = le16(d, at);
        let chunks = le16(d, at + 2);
        let mut p = at + 4;
        for _ in 0..chunks {
            let (off, n) = (le16(d, p), le16(d, p + 2));
            p += 4;
            for k in 0..n {
                if let Some(px) = f.get_mut(off + k) {
                    *px = *d.get(p + k)?;
                }
            }
            p += n;
        }
        frames.push(f);
        // The size doesn't count its own word.
        at += size + 2;
    }
    Some(Flat { w, h, frames })
}
