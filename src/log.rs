//! The calendar and the logbook (LOG.nn).
//!
//! Time is counted in minutes from the start of 3E 389: a year is 360
//! days of 1440 minutes, in twelve months of 30 days. A log entry is "&",
//! a date line ending in CR LF, the text, then " *". It is appended to
//! LOG.nn (27E3:0060..0087). Once the log reaches 2000 bytes the game
//! means to drop its oldest entry first (27E3:0169..01AA), but writes the
//! shorter log to a path DOS refuses, so the log only grows
//! (`Machine::log_add`).

use crate::bsa::Files;
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

/// The log's name, "log." (its digits made at the '.', 27E3:01E3).
pub const NAME: u16 = 0x78D6;
/// The bank the log is read into to drop its oldest entry.
const BANK: u16 = 0x1030;
/// The EMS frame's segment.
const FRAME: u16 = 0x9030;

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

    /// 27E3:000E. The text at DS:`text` added to the logbook: LOG.nn made
    /// empty where there is none, its oldest entry dropped where it holds
    /// 2000 bytes or more (`log_trim`), the entry made (`log_entry`) and
    /// appended (0060..0087: opened, 0000:67C4, sought to its end, the
    /// entry written, closed). None where the open gives up.
    pub fn log_add(&mut self, files: &mut Files, text: u16) -> Option<()> {
        if self.log_full(files) {
            self.log_trim(files)?;
        }
        let n = self.log_entry(text);
        self.log_name();
        let path = self.save_path(NAME);
        self.file_open(files, path)?;
        self.file_seek(files, 2, 0);
        self.file_put(files, DS, ENTRY, n);
        self.file_close(files);
        Some(())
    }

    /// 27E3:01E3. The save's digits (1A6C, tens then units) written after
    /// the first '.' of the log's name (searched over at most 10 bytes).
    fn log_name(&mut self) {
        let mut di = NAME;
        for _ in 0..10 {
            let c = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if c == b'.' {
                break;
            }
        }
        let slot = self.mem.r8(DS, 0x1A6C);
        self.mem.w8(DS, di, b'0' + slot / 10);
        self.mem.w8(DS, di.wrapping_add(1), b'0' + slot % 10);
    }

    /// 27E3:01AB. Whether the log holds 2000 bytes or more (DOS find
    /// first, 0000:600C, its size); where there is none it is made empty
    /// (0000:677A, 6742; its path built again) and isn't.
    fn log_full(&mut self, files: &mut Files) -> bool {
        self.log_name();
        let path = self.save_path(NAME);
        match files.found_size(self.mem.cstr(DS, path)) {
            Some(size) => size >= 2000,
            None => {
                let path = self.save_path(NAME);
                self.file_create(files, path);
                self.file_close(files);
                false
            }
        }
    }

    /// 27E3:0169..01AA. The log's oldest entry dropped: the bank at 1030
    /// mapped (0000:5C86), the log read whole into the frame (0000:7428),
    /// and what follows its first '*' written to a new file (0000:9DD2);
    /// the bank before put back. Bug kept: the new file's name is the
    /// path built from the log's path (25FD:0844 given SI as the first
    /// call left it), "C:\ARENA_CD\C:\ARENA_CD\log.nn", which DOS refuses
    /// (error 3, path not found: its third component's name ends at the
    /// ':', `Files::path_error`). So the create fails, nothing is written
    /// and the log is never trimmed: it only grows. The failure goes
    /// unchecked (01A3 goes on). None where the read gives up, or where
    /// 9DD2's 0000:74E5 would call its hook (not ported).
    fn log_trim(&mut self, files: &mut Files) -> Option<()> {
        self.log_name();
        let path = self.save_path(NAME);
        let bank = self.mem.r16(DS, BANK);
        let old = self.map_bank(bank, 0);
        let seg = self.mem.r16(DS, FRAME);
        self.file_load(files, path, seg, 0)?;
        let mut di = 0u16;
        for _ in 0..0xFFFF {
            let c = self.mem.r8(seg, di);
            di = di.wrapping_add(1);
            if c == b'*' {
                break;
            }
        }
        let n = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE).wrapping_sub(di);
        let twice = self.save_path(path);
        if self.file_write(files, twice, seg, di, n) == 0 && self.error_hook_set() {
            return crate::giveup::not_ported("0000:9DD2: the trimmed log's create refused, the error hook set");
        }
        self.map_bank(old, 1);
        Some(())
    }
}
