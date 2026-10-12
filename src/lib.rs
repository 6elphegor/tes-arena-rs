//! Port of The Elder Scrolls: Arena (ACD.EXE, CD version 1.07) to Rust.
//! Addresses in comments are segment:offset in the unpacked image.

pub mod exe;
pub mod machine;
pub mod mem;
pub mod random;
