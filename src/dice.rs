//! Dice rolls built on the RNG.

use crate::random::ArenaRandom;

/// 1194:55A7. [0, n). None where the original faults (n == 0).
pub fn roll(rng: &mut ArenaRandom, n: u16) -> Option<u16> {
    rng.range(0, n)
}

/// 1194:4892. For n <= 8 this is `roll(n)`, i.e. 0..n-1. Above 8 it rolls
/// m/d dice of 0..d each, where m >= n is the first number divisible by 6,
/// then 5 (6 tried first); totals above n are rerolled. So the result is
/// 0..=n, inclusive, unlike the small case.
pub fn dice(rng: &mut ArenaRandom, n: u16) -> Option<u16> {
    loop {
        let r = if n <= 8 {
            roll(rng, n)?
        } else {
            let (count, sides) = split(n);
            let mut sum = 0u16;
            for _ in 0..count {
                sum = sum.wrapping_add(roll(rng, sides)?);
            }
            sum
        };
        if r <= n {
            return Some(r);
        }
    }
}

/// 1194:48AD. Returns (count, sides) with sides = d + 1.
fn split(n: u16) -> (u16, u16) {
    let mut m = n;
    loop {
        for d in [6u16, 5] {
            if m % d == 0 {
                return (m / d, d + 1);
            }
        }
        m = m.wrapping_add(1);
    }
}
