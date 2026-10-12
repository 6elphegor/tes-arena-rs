//! Conversions between percent (0..100) and the internal 0..256 scale
//! that attributes use. All round half away from zero.

use crate::mem::{DS, Memory};

/// 1194:5B34. ×256/100, signed.
pub fn to256(v: i16) -> i16 {
    scale(v, 256, 100)
}

/// 1194:5B07. ×100/256, signed.
pub fn to100(v: i16) -> i16 {
    scale(v, 100, 256)
}

fn scale(v: i16, mul: i32, div: i32) -> i16 {
    let p = v as i32 * mul;
    let (q, r) = (p / div, (p % div).abs());
    // The original faults (#DE) if the quotient overflows 16 bits.
    debug_assert!(i16::try_from(q).is_ok(), "divide overflow");
    let q = q as i16;
    if r >= div / 2 { if v < 0 { q.wrapping_sub(1) } else { q.wrapping_add(1) } } else { q }
}

/// 1194:5AA9. ×100/256, unsigned, rounding up at a remainder of 128.
pub fn to100_u(v: u16) -> u16 {
    let p = v as u32 * 100;
    let q = (p >> 8) as u16;
    if p & 0xff >= 0x80 { q.wrapping_add(1) } else { q }
}

/// 1194:1007. Converts `n` DGROUP attribute bytes at `src` to percent, into `dst`.
pub fn attrs_to100(mem: &mut Memory, src: u16, dst: u16, n: u16) {
    for i in 0..n {
        let v = to100(mem.r8(DS, src.wrapping_add(i)) as i16);
        mem.w8(DS, dst.wrapping_add(i), v as u8);
    }
}

/// 1194:1012. Converts `n` percent attribute bytes to the 256 scale.
/// The bytes are read signed, and exactly 100 maps to 255 rather than 256.
pub fn attrs_to256(mem: &mut Memory, src: u16, dst: u16, n: u16) {
    for i in 0..n {
        let a = mem.r8(DS, src.wrapping_add(i)) as i8 as i16;
        let v = if a == 100 { 0xff } else { to256(a) };
        mem.w8(DS, dst.wrapping_add(i), v as u8);
    }
}
