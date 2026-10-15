//! The interface bar's own state (segment 2542's buttons, 2542:00BA's
//! table at 5BD2): the status box's text (2542:04B1), with the time of
//! day as words (0000:1486) and the weight carried (1194:4E1B, ported as
//! `carried_weight` in loot_window.rs), and the hand's pickpocketing
//! armed (2542:0279).
//!
//! The box itself (0000:965A), the wait for a key around it, and the
//! conditions 2686:005E appends to the text (poison, disease, low health,
//! the spells on the player) are the front end's.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP: the status box's text, its format, the time's two formats
/// ("%d:%d %s", "%d:0%d %s") and the place's name.
pub mod var {
    pub const STATUS_TEXT: u16 = 0x941E;
    pub const STATUS_FORMAT: u16 = 0x5B7B;
    pub const TIME_FORMAT: u16 = 0x4E5A;
    pub const TIME_FORMAT_0: u16 = 0x4E63;
    pub const PLACE_NAME: u16 = 0x0A22;
    /// The date's and the time's buffers 04B1 writes them to.
    pub const DATE_TEXT: u16 = 0x1044;
    pub const TIME_TEXT: u16 = 0x1144;
    /// "Select target for pilfering..." (2542:0279's message).
    pub const PILFER_TEXT: u16 = 0x5B5C;
}
use var::*;

/// The times of day's words (DGROUP strings): early morning, in the
/// morning, noon, in the afternoon, in the evening, at night, midnight.
const EARLY_MORNING: u16 = 0x4E6D;
const MORNING: u16 = 0x4E7B;
const NOON: u16 = 0x4E8A;
const AFTERNOON: u16 = 0x4E8F;
const EVENING: u16 = 0x4EA0;
const NIGHT: u16 = 0x4EAF;
const MIDNIGHT: u16 = 0x4EB8;

impl Machine {
    /// 0000:1486. The time of day at DS:`dest` ("9:05 in the morning"):
    /// the clock 5CC's minutes into the day, the hour on a 12-hour dial
    /// (0 shown as 12), the minutes with a 0 before one digit, and the
    /// words for its part of the day: noon and midnight only on the hour,
    /// "at night" from 9 in the evening to before 3 in the morning.
    /// Returns the offset of the NUL.
    pub fn time_text(&mut self, dest: u16) -> u16 {
        let of_day = (self.mem.r32(DS, 0x05CC) % 0x5A0) as u16;
        let (mut h, m) = (of_day / 60, of_day % 60);
        let words = if h >= 12 {
            h -= 12;
            if h == 0 && m == 0 {
                NOON
            } else if h < 6 {
                AFTERNOON
            } else if h < 9 {
                EVENING
            } else {
                NIGHT
            }
        } else if h == 0 && m == 0 {
            MIDNIGHT
        } else if h < 3 {
            NIGHT
        } else if h < 6 {
            EARLY_MORNING
        } else {
            MORNING
        };
        if h == 0 {
            h = 12;
        }
        let fmt = if m > 9 { TIME_FORMAT } else { TIME_FORMAT_0 };
        use crate::format::Arg;
        self.sprintf((DS, dest), (DS, fmt), &[Arg::Int(h), Arg::Int(m), Arg::Str(DS, words)])
    }

    /// 2542:04B1's text: "You are in %s.\rIt is %s.\rThe date is %sYou are
    /// currently carrying %d kg out of %d kg.\r" at STATUS_TEXT, with the
    /// place's name (A22), the time (`time_text` at 1144), the date
    /// (`log_date` at 1044, which ends in a CR), the weight carried and the
    /// most the player can carry (the player's +3B on the 100 scale). The
    /// message box's colour (9288) is FCh. Returns the offset of the NUL,
    /// where 2686:005E appends the conditions.
    pub fn status_text(&mut self) -> u16 {
        self.mem.w8(DS, 0x9288, 0xFC);
        let carried = self.carried_weight();
        let most = crate::scale::to100_u(self.mem.r16(DS, 0x01DF));
        let minutes = self.mem.r32(DS, 0x05CC);
        self.log_date(minutes, DATE_TEXT);
        self.time_text(TIME_TEXT);
        use crate::format::Arg;
        self.sprintf(
            (DS, STATUS_TEXT),
            (DS, STATUS_FORMAT),
            &[Arg::Str(DS, PLACE_NAME), Arg::Str(DS, TIME_TEXT), Arg::Str(DS, DATE_TEXT), Arg::Int(carried), Arg::Int(most)],
        )
    }

    /// 2542:0279. The hand's button: pickpocketing armed (5CA bit 0) with
    /// "Select target for pilfering..." on the message line until it is
    /// used or disarmed (its time FFFDh: held).
    pub fn pilfer_arm(&mut self) {
        self.mem.w16(DS, 0x061E, 0xFFFD);
        self.mem.w16(DS, 0x061A, PILFER_TEXT);
        self.mem.w16(DS, 0x061C, DS);
        let f = self.mem.r16(DS, 0x05CA) | 1;
        self.mem.w16(DS, 0x05CA, f);
    }
}
