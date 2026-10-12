//! The calendar and the logbook (LOG.nn).
//!
//! Time is counted in minutes from the start of 3E 389: a year is 360
//! days of 1440 minutes, in twelve months of 30 days. A log entry is "&",
//! a date line ending in CR LF, the text, then " *". Appending to the
//! file (27E3:0060..0087) and dropping the oldest entry once the log
//! reaches 2000 bytes (27E3:0169..01E2, through a temporary log.$$$) are
//! file writes and are not ported.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP tables: month lengths in minutes (dwords; the twelfth is 300
/// days, so it takes whatever is left of a year), month and weekday name
/// pointers, ordinal suffixes by last digit and "th" for 11..13, and the
/// date format.
const MONTH_MINUTES: u16 = 0x4ECE;
const MONTH_NAMES: u16 = 0x3180;
const WEEKDAY_NAMES: u16 = 0x3220;
const SUFFIXES: u16 = 0x4FE6;
const TEEN_SUFFIX: u16 = 0x5003;
/// The entry buffer.
pub const ENTRY: u16 = 0x9A6E;

const YEAR_MINUTES: u32 = 360 * 1440;
const FIRST_YEAR: u16 = 389;

impl Machine {
    /// 0000:1508. Writes the date of `minutes` at DS:`dest`, as
    /// "Loredas, 5th of Frostfall in the year 3E 391\r" with a NUL after.
    /// The weekday is the day of the month mod 7, so every month starts
    /// on the same weekday (Tirdas the 1st). Returns the offset of the
    /// NUL.
    pub fn log_date(&mut self, minutes: u32, dest: u16) -> u16 {
        let year = ((minutes / YEAR_MINUTES) as u16).wrapping_add(FIRST_YEAR);
        let mut rest = minutes % YEAR_MINUTES;
        let mut month = 0u16;
        let mut si = MONTH_MINUTES;
        loop {
            let len = self.mem.r32(DS, si);
            if rest < len {
                break;
            }
            rest -= len;
            si = si.wrapping_add(4);
            month += 1;
        }
        let day = ((rest / 1440) as u16).wrapping_add(1);
        let suffix = self.log_ordinal(day as u8);
        let weekday = day % 7;
        let month_name = self.mem.r16(DS, MONTH_NAMES.wrapping_add(month * 2));
        let weekday_name = self.mem.r16(DS, WEEKDAY_NAMES.wrapping_add(weekday * 2));
        // sprintf(dest, "%s, %u%s of %s in the year 3E %d\r", ...); its
        // numbers go through 0000:7028's scratch digits, day then year.
        self.log_digits(day);
        self.log_digits(year);
        let mut text = self.mem.cstr(DS, weekday_name).to_vec();
        text.extend_from_slice(b", ");
        text.extend_from_slice(day.to_string().as_bytes());
        text.extend_from_slice(self.mem.cstr(DS, suffix));
        text.extend_from_slice(b" of ");
        text.extend_from_slice(self.mem.cstr(DS, month_name));
        text.extend_from_slice(b" in the year 3E ");
        text.extend_from_slice((year as i16).to_string().as_bytes());
        text.push(b'\r');
        for (i, &c) in text.iter().chain(std::iter::once(&0)).enumerate() {
            self.mem.w8(DS, dest.wrapping_add(i as u16), c);
        }
        dest.wrapping_add(text.len() as u16)
    }

    /// 0000:7028's scratch: the decimal digits of `n`, written backwards
    /// ending at DGROUP A933 (no terminator); left there after a number
    /// is formatted.
    fn log_digits(&mut self, mut n: u16) {
        let mut di = 0xA933u16;
        loop {
            self.mem.w8(DS, di, b'0' + (n % 10) as u8);
            di = di.wrapping_sub(1);
            n /= 10;
            if n == 0 {
                break;
            }
        }
    }

    /// 1194:5698. The ordinal suffix of `day`: "th" for 11, 12 and 13,
    /// otherwise by the last digit.
    pub fn log_ordinal(&self, day: u8) -> u16 {
        match day {
            11..=13 => TEEN_SUFFIX,
            _ => self.mem.r16(DS, SUFFIXES + (day % 10) as u16 * 2),
        }
    }

    /// 27E3:001A..005F. Makes a log entry of the text at DS:`text` for
    /// the current time (DGROUP 05CC): " *" is appended to the text
    /// itself, then ENTRY gets "&", the date, LF (after the date's CR)
    /// and the text. Returns the entry's length. The text is searched
    /// for its NUL over at most 10000 bytes and the date over 2000, as in
    /// the original; both are assumed to be found.
    pub fn log_entry(&mut self, text: u16) -> u16 {
        let mut end = text;
        while self.mem.r8(DS, end) != 0 {
            end = end.wrapping_add(1);
        }
        self.mem.w16(DS, end, u16::from_le_bytes(*b" *"));
        self.mem.w8(DS, end.wrapping_add(2), 0);
        self.mem.w8(DS, ENTRY, b'&');
        let minutes = self.mem.r32(DS, 0x05CC);
        let nul = self.log_date(minutes, ENTRY + 1);
        self.mem.w8(DS, nul, b'\n');
        let mut di = nul.wrapping_add(1);
        let mut si = text;
        loop {
            let c = self.mem.r8(DS, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                break;
            }
        }
        di.wrapping_sub(1).wrapping_sub(ENTRY)
    }
}
