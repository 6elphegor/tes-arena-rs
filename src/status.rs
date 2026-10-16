//! The interface bar's own state (segment 2542's buttons, 2542:00BA's
//! table at 5BD2): the status box's text (2542:04B1), with the time of
//! day as words (0000:1486) and the weight carried (1194:4E1B, ported as
//! `carried_weight` in loot_window.rs), and the hand's pickpocketing
//! armed (2542:0279).
//!
//! The conditions 2686:005E appends to the text (`status_conditions`:
//! disease, poison, low health, drunkenness, the spells on the player,
//! fortified attributes). The box itself (0000:965A) and the wait for a
//! key around it are the front end's.

use crate::machine::Machine;
use crate::mem::{DATA, DS};

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
    /// 2686:005E's lines counted, and its formats and lists: "You are
    /// %s.\r", "You have %s.\r", "Your %s is fortified.\r", the
    /// conditions' names (healthy, diseased, poisoned, ...), the
    /// diseases' names (DATA) and the attributes' names.
    pub const LINES: u16 = 0x12B2;
    pub const ARE_FORMAT: u16 = 0x6855;
    pub const HAVE_FORMAT: u16 = 0x6847;
    pub const FORTIFIED_FORMAT: u16 = 0x6830;
    pub const CONDITIONS: u16 = 0x6862;
    pub const DISEASES: u16 = 0x4878;
    pub const ATTRIBUTE_NAMES: u16 = 0x2B00;
    /// The effect slots (10h of 3Ch bytes: +6..+0Dh the attributes
    /// raised, +28h the far target, +3Ah the kind, +3Bh FFh free).
    pub const EFFECT_SLOTS: u16 = 0x0660;
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

    /// 2542:04B1..0515: the status box's text, `status_text`, then the
    /// player's conditions after it (`status_conditions` at its NUL; the
    /// font calls 0000:9AE6/9AF8 between them are the front end's).
    /// Returns the offset of the NUL.
    pub fn status_box(&mut self) -> u16 {
        let end = self.status_text();
        self.status_conditions(end)
    }

    /// 2686:005E. The player's conditions appended at DS:`di` (2542:0515
    /// passes the status text's NUL), each line through 2686:0235: "You
    /// are %s.\r" with the condition's name from the list at CONDITIONS,
    /// a line counted at LINES (12B2, cleared first). In order:
    /// - diseased (+40Dh bit 1): the disease's own name instead, "You have
    ///   %s.\r" (2686:0158): the first of 11h effect slots (one past the
    ///   table's 10h, as the original walks) on the player whose kind
    ///   (+3Ah) is 0Ah or more names it from DATA:4878 (its kind less
    ///   0Ah); found, the line isn't counted and CX is left 1, else CX 0;
    /// - poisoned (bit 2), drunk (20h), paralyzed (40h), cursed (4),
    ///   drained (200h), each with CX its index;
    /// - in critical condition when the health (+59h) is at most a tenth
    ///   of its maximum (+5Bh);
    /// - when 2686:0270 finds a spell on the player (an effect slot whose
    ///   target's offset is the player's, alive (+3Bh not FFh), with none
    ///   of +40Dh bits 1, 20h, 2; or the spell bits +41Ah or +41Ch set):
    ///   each set bit of +41Ah from bit 0 names condition 5 on (the
    ///   empty 0Dh skipped), +41Ch "shielded", +40Dh bit 80h
    ///   "regenerating" (2686:010F; CX kept);
    /// - "healthy" when CX is still 0;
    /// - with 5B1 bit 10h, each attribute an effect slot of kind 4 on the
    ///   player fortifies (+6..+0Dh non-zero): "Your %s is fortified.\r"
    ///   with its name from 2B00 (2686:01C3, not counted); the original
    ///   keeps the player's pointer in EAX, which the first line printed
    ///   clobbers, so only the first such slot is listed.
    ///
    /// The text ends with a NUL; more than 10 lines counted set D39 bit
    /// 80h. Returns DI, at that NUL.
    pub fn status_conditions(&mut self, di: u16) -> u16 {
        const P: u16 = 0x01A4;
        let mut di = di;
        self.mem.w16(DS, LINES, 0);
        let mut cx = 0u16;
        let ax = self.mem.r16(DS, P + 0x40D);
        if ax & 1 != 0 {
            let (d, c) = self.disease_line(di);
            di = d;
            cx = c;
        }
        for (bit, c) in [(2u16, 2u16), (0x20, 4), (0x40, 0x14), (4, 0x15), (0x200, 0x16)] {
            if ax & bit != 0 {
                cx = c;
                di = self.condition_line(di, cx);
            }
        }
        if self.mem.r16(DS, P + 0x59) <= self.mem.r16(DS, P + 0x5B) / 10 {
            cx = 3;
            di = self.condition_line(di, cx);
        }
        if self.spell_on_player() {
            di = self.spell_lines(di);
        }
        if cx == 0 {
            di = self.condition_line(di, cx);
        }
        if self.mem.r16(DS, 0x05B1) & 0x10 != 0 {
            di = self.fortified_lines(di);
        }
        self.mem.w8(DS, di, 0);
        if self.mem.r16(DS, LINES) > 10 {
            let b = self.mem.r8(DS, 0x0D39) | 0x80;
            self.mem.w8(DS, 0x0D39, b);
        }
        di
    }

    /// 2686:0235. "You are %s.\r" at DS:`di` with condition `cx`, a line
    /// counted; DI at its NUL.
    fn condition_line(&mut self, di: u16, cx: u16) -> u16 {
        let name = self.skip_strings(DS, CONDITIONS, cx);
        use crate::format::Arg;
        let end = self.sprintf((DS, di), (DS, ARE_FORMAT), &[Arg::Str(DS, name)]);
        let n = self.mem.r16(DS, LINES).wrapping_add(1);
        self.mem.w16(DS, LINES, n);
        end
    }

    /// Whether the effect slot at `slot` is alive and on the player (its
    /// far target at +28h).
    fn slot_on_player(&self, slot: u16) -> bool {
        self.mem.r8(DS, slot + 0x3B) != 0xFF && self.mem.r16(DS, slot + 0x28) == 0x01A4 && self.mem.r16(DS, slot + 0x2A) == DS
    }

    /// 2686:0158. The disease's line; (DI, CX).
    fn disease_line(&mut self, di: u16) -> (u16, u16) {
        for i in 0..0x11u16 {
            let slot = EFFECT_SLOTS + i * 0x3C;
            let kind = self.mem.r8(DS, slot + 0x3A);
            if self.slot_on_player(slot) && kind >= 0x0A {
                let name = self.skip_strings(DATA, DISEASES, kind as u16 - 0x0A);
                use crate::format::Arg;
                let end = self.sprintf((DS, di), (DS, HAVE_FORMAT), &[Arg::Str(DATA, name)]);
                return (end, 1);
            }
        }
        (di, 0)
    }

    /// 2686:0270. A spell on the player: a slot on it (by its target's
    /// offset alone) while +40Dh has none of bits 1, 20h and 2, else the
    /// spell bits +41Ah or +41Ch.
    pub fn spell_on_player(&self) -> bool {
        const P: u16 = 0x01A4;
        let flags = self.mem.r16(DS, P + 0x40D);
        for i in 0..0x10u16 {
            let slot = EFFECT_SLOTS + i * 0x3C;
            if self.mem.r16(DS, slot + 0x28) != P || self.mem.r8(DS, slot + 0x3B) == 0xFF {
                continue;
            }
            if flags & 1 != 0 || flags & 0x20 != 0 || flags & 2 != 0 {
                continue;
            }
            return true;
        }
        self.mem.r16(DS, P + 0x41A) != 0 || self.mem.r16(DS, P + 0x41C) != 0
    }

    /// 2686:010F. The spells' lines.
    fn spell_lines(&mut self, mut di: u16) -> u16 {
        const P: u16 = 0x01A4;
        let bits = self.mem.r16(DS, P + 0x41A);
        if bits != 0 {
            for k in 0..0x0Du16 {
                let bx = 5 + k;
                if bits & (1 << k) != 0 && bx != 0x0D {
                    di = self.condition_line(di, bx);
                }
            }
        }
        if self.mem.r16(DS, P + 0x41C) != 0 {
            di = self.condition_line(di, 0x12);
        }
        if self.mem.r16(DS, P + 0x40D) & 0x80 != 0 {
            di = self.condition_line(di, 0x13);
        }
        di
    }

    /// 2686:01C3. "Your %s is fortified.\r" for each attribute a kind-4
    /// effect slot on the player raises.
    fn fortified_lines(&mut self, mut di: u16) -> u16 {
        // EAX holds the player's far pointer for the comparison; a line
        // printed leaves AX as the formatter does (AL 0, the format's
        // NUL), so no slot after one that printed is the player's.
        let mut printed = false;
        for i in 0..0x10u16 {
            let slot = EFFECT_SLOTS + i * 0x3C;
            if printed || !self.slot_on_player(slot) || self.mem.r8(DS, slot + 0x3A) != 4 {
                continue;
            }
            for bx in 0..8u16 {
                if self.mem.r8(DS, slot + 6 + bx) != 0 {
                    printed = true;
                    let name = self.skip_strings(DS, ATTRIBUTE_NAMES, bx);
                    use crate::format::Arg;
                    di = self.sprintf((DS, di), (DS, FORTIFIED_FORMAT), &[Arg::Str(DS, name)]);
                }
            }
        }
        di
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
