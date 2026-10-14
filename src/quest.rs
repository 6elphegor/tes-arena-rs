//! Quests (segment 1194, 98E8..9CD7): their records, their clock.
//!
//! The player's quests are four records of 34h bytes at QUESTS; 99F8
//! walks four more of 29h bytes at OTHERS with the same fields. A
//! record's word at +4 holds its kind (bits 6-7: 0 none, 1 under way, 3
//! failed), its stage (bits 9-11) and flags (bit 0 warned, bit 5 its
//! goal reached); +8 the time it began (a day's noon), +0Ch its deadline
//! (dwords, minutes like the clock's at TIME); +1Ch its item (FFh none,
//! one of the nine at ITEMS); +24h and +25h places (the high three bits
//! of +25h a province, the low five a place in it).
//!
//! The main quest's step is the byte at 0FDD (`dungeon::var::QUEST_STEP`)
//! just past ITEMS, which 9C74's search can overrun (see `item_free`).
//!
//! The clock checks the quests every hour (`quests_hourly`, 98E8) and the
//! others every day (`quests_daily`, 99F8). A quest past its deadline
//! fails (`fail`, 9A65): it is marked failed and remembered for a hundred
//! days in the table at FAILED, and its place's count in the province's
//! data goes down, the province loaded and written back around it. The
//! texts and the file I/O are `QuestHooks`.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The player's quests (4 of 34h bytes) and the records 99F8 walks
    /// (4 of 29h).
    pub const QUESTS: u16 = 0x0E6E;
    pub const OTHERS: u16 = 0x0DCA;
    /// The quest items held (9 bytes, FFh free).
    pub const ITEMS: u16 = 0x0FD4;
    /// The failed quests (8 of 6 bytes: the stage, FFh free; the place;
    /// the time they are forgotten, a dword).
    pub const FAILED: u16 = 0x0A7E;
    /// The time in minutes (dword).
    pub const TIME: u16 = 0x05CC;
    /// The quest being checked (its record) and its stage, later its
    /// province.
    pub const CURRENT: u16 = 0x1246;
    pub const STAGE: u16 = 0x1244;
    /// The province's data as loaded (1194:2E18), and its places' records
    /// (19h bytes each) from +1Ch.
    pub const PROVINCE_DATA: u16 = 0x1429;
    pub const PLACES: u16 = 0x1445;
    /// The text's number and flag for 1194:A374, its flags, the window's
    /// colour.
    pub const TEXT: u16 = 0x137A;
    pub const TEXT_FLAG: u16 = 0x137E;
    pub const TEXT_FLAG2: u16 = 0x101C;
    pub const TEXT_COLOUR: u16 = 0x9288;
    /// The texts of the others' stages (bytes, + 4FAh).
    pub const STAGE_TEXTS: u16 = 0x6DAC;
}
use var::*;

/// A day in minutes.
const DAY: u32 = 0x5A0;

/// What the quests' clock leaves to the front end.
pub trait QuestHooks {
    /// 1194:A374 with AX 2: text number TEXT (with its flag TEXT_FLAG)
    /// written to the buffer at 1044.
    fn text(&mut self, _m: &mut Machine) {}
    /// 0000:9AE6 (`font` 0) and 9AF8 (1): the text's fonts, AX the font.
    fn font(&mut self, _m: &mut Machine, _which: u8, _ax: u16) {}
    /// 1194:59CC: the message box with the text at DS:`si`.
    fn message_box(&mut self, _m: &mut Machine, _si: u16) {}
    /// 1194:2E18: province `province`'s data loaded to PROVINCE_DATA
    /// (`Machine::city_load_province`, which needs the files).
    fn province_load(&mut self, _m: &mut Machine, _province: u8) {}
    /// 26F7:0CF1: province `province`'s data written back to CITYDATA
    /// (AX the province).
    fn province_save(&mut self, _m: &mut Machine, _province: u16) {}
}

/// `QuestHooks` that do nothing.
pub struct NoQuestHooks;

impl QuestHooks for NoQuestHooks {}

impl Machine {
    /// 1194:9B52. The record's kind (+4 bits 6-7).
    pub fn quest_kind(&self, si: u16) -> u16 {
        (self.mem.r16(DS, si.wrapping_add(4)) & 0xC0) >> 6
    }

    /// 1194:9B5C. The record's stage (+4 bits 9-11).
    pub fn quest_stage(&self, si: u16) -> u16 {
        (self.mem.r16(DS, si.wrapping_add(4)) & 0xE00) >> 9
    }

    fn quest_flags(&self, si: u16) -> u16 {
        self.mem.r16(DS, si.wrapping_add(4))
    }

    fn quest_deadline(&self, si: u16) -> u32 {
        self.mem.r32(DS, si.wrapping_add(0xC))
    }

    /// 1194:98E8, every hour: the quests under way (kind 1) checked
    /// against their deadlines, by stage (unsigned comparisons of the
    /// dwords):
    /// - 3: from a day before the deadline to the deadline, text 1Ah and
    ///   the quest fails; past it, once (flag bit 0), text 19h;
    /// - 4: from a day past the deadline it fails, with text 1Bh when
    ///   bit 5 is set;
    /// - 5 and 1: from the deadline it fails, with text 1Dh or 1Eh when
    ///   bit 5 is set;
    /// - 6: not while bit 5 is set; otherwise, and for the other stages,
    ///   from the deadline it fails without a text.
    ///
    /// The texts are shown with flag 32h. 8A56 is cleared meanwhile.
    pub fn quests_hourly(&mut self, hooks: &mut dyn QuestHooks) {
        let saved = self.mem.r8(DS, 0x8A56);
        self.mem.w8(DS, 0x8A56, 0);
        let mut si = QUESTS;
        for _ in 0..4 {
            self.mem.w16(DS, CURRENT, si);
            self.quest_check(hooks, si);
            si = si.wrapping_add(0x34);
        }
        self.mem.w8(DS, 0x8A56, saved);
    }

    fn quest_check(&mut self, hooks: &mut dyn QuestHooks, si: u16) {
        if self.quest_kind(si) != 1 {
            return;
        }
        let stage = self.quest_stage(si);
        self.mem.w16(DS, STAGE, stage);
        let time = self.mem.r32(DS, TIME);
        let deadline = self.quest_deadline(si);
        let done = self.quest_flags(si) & 0x20 != 0;
        let text = match stage {
            3 => {
                if time < deadline.wrapping_sub(DAY) {
                    return;
                }
                if deadline < time {
                    if self.quest_flags(si) & 1 == 0 {
                        self.quest_message(hooks, 0x19, 0x32);
                        let si = self.mem.r16(DS, CURRENT);
                        let f = self.quest_flags(si) | 1;
                        self.mem.w16(DS, si.wrapping_add(4), f);
                    }
                    return;
                }
                Some(0x1A)
            }
            4 => {
                if time < deadline.wrapping_add(DAY) {
                    return;
                }
                done.then_some(0x1B)
            }
            5 | 1 => {
                if time < deadline {
                    return;
                }
                done.then_some(if stage == 5 { 0x1D } else { 0x1E })
            }
            _ => {
                if stage == 6 && done {
                    return;
                }
                if time < deadline {
                    return;
                }
                None
            }
        };
        if let Some(t) = text {
            self.quest_message(hooks, t, 0x32);
        }
        self.quest_fail(hooks, si);
    }

    /// 1194:99F8, every day: each of the records at OTHERS whose deadline
    /// has passed is cleared (+4 0) unless it has no kind; one under way
    /// first shows its stage's text (1194:9C96: 595h when the stage isn't
    /// 2 and 1013 is set, otherwise the byte at STAGE_TEXTS + stage +
    /// 4FAh), and at stage 3 frees its item. 8A56 is cleared meanwhile.
    pub fn quests_daily(&mut self, hooks: &mut dyn QuestHooks) {
        let saved = self.mem.r8(DS, 0x8A56);
        self.mem.w8(DS, 0x8A56, 0);
        let time = self.mem.r32(DS, TIME);
        let mut si = OTHERS;
        for _ in 0..4 {
            if time > self.quest_deadline(si) {
                let kind = self.quest_kind(si);
                if kind != 0 {
                    if kind == 1 {
                        let stage = self.quest_stage(si);
                        let text = if stage != 2 && self.mem.r8(DS, 0x1013) != 0 {
                            0x595
                        } else {
                            (self.mem.r8(DS, STAGE_TEXTS.wrapping_add(stage)) as u16).wrapping_add(0x4FA)
                        };
                        self.quest_message(hooks, text, 0);
                        if self.quest_stage(si) == 3 {
                            self.quest_item_free(si);
                        }
                    }
                    self.mem.w16(DS, si.wrapping_add(4), 0);
                }
            }
            si = si.wrapping_add(0x29);
        }
        self.mem.w8(DS, 0x8A56, saved);
    }

    /// 1194:9C96 (`flag` 0) / 9C9B (TEXT_FLAG already set, here `flag`):
    /// text `text` written (A374, mode 2) and shown in a message box,
    /// in colour 60h, or EEh with its second font when 5C8 bit 20h is set.
    fn quest_message(&mut self, hooks: &mut dyn QuestHooks, text: u16, flag: u8) {
        self.mem.w8(DS, TEXT_FLAG, flag);
        self.mem.w16(DS, TEXT, text);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        hooks.text(self);
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        let ax = self.mem.r16(DS, 0x102C);
        hooks.font(self, 0, ax);
        if self.mem.r16(DS, 0x05C8) & 0x20 != 0 {
            let ax = self.mem.r16(DS, 0x1038);
            hooks.font(self, 1, ax);
            self.mem.w8(DS, TEXT_COLOUR, 0xEE);
        }
        hooks.message_box(self, 0x1044);
    }

    /// 1194:9C74. The record's item (+1Ch), unless FFh, taken from the
    /// list at ITEMS (its first match set to FFh) and 0D39 bit 80h set.
    /// Without a match the search ends past the list and writes FFh to
    /// its tenth byte, 0FDD: the main quest's step (the original's bug,
    /// kept).
    pub fn quest_item_free(&mut self, si: u16) {
        let item = self.mem.r8(DS, si.wrapping_add(0x1C));
        if item == 0xFF {
            return;
        }
        let i = (0..9u16).find(|&i| self.mem.r8(DS, ITEMS + i) == item).unwrap_or(9);
        self.mem.w8(DS, ITEMS + i, 0xFF);
        let f = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, f);
    }

    /// 1194:9A65. The quest at `si` fails: at stages 3 to 5 its item is
    /// freed; it becomes kind 3; it is remembered in the table at FAILED
    /// in its first free entry (stage FFh) or, the table full, after the
    /// table is moved up an entry, in its seventh (the original's
    /// pointer goes back one entry too many, so the last stays); the
    /// entry holds STAGE's low byte, the place (+25h) and the time a
    /// hundred days on. Then the place's count (+18h of its record at
    /// PLACES) in its province goes down, the province loaded before
    /// and written back after; STAGE becomes the province.
    pub fn quest_fail(&mut self, hooks: &mut dyn QuestHooks, si: u16) {
        if matches!(self.quest_stage(si), 3..=5) {
            self.quest_item_free(si);
        }
        let f = self.quest_flags(si) & 0xFF3F | 0xC0;
        self.mem.w16(DS, si.wrapping_add(4), f);
        let mut di = (0..8u16).map(|i| FAILED + i * 6).find(|&e| self.mem.r8(DS, e) == 0xFF);
        if di.is_none() {
            for i in 0..0x2Au16 {
                let b = self.mem.r8(DS, FAILED + 6 + i);
                self.mem.w8(DS, FAILED + i, b);
            }
            di = Some(FAILED + 0x2A - 6);
        }
        let di = di.unwrap();
        let stage = self.mem.r16(DS, STAGE) as u8;
        self.mem.w8(DS, di, stage);
        let si = self.mem.r16(DS, CURRENT);
        let place = self.mem.r8(DS, si.wrapping_add(0x25));
        self.mem.w8(DS, di + 1, place);
        let t = self.mem.r32(DS, TIME).wrapping_add(0x23280);
        self.mem.w32(DS, di + 2, t);
        let province = (place as u16) >> 5;
        self.mem.w16(DS, STAGE, province);
        hooks.province_load(self, province as u8);
        let at = PLACES.wrapping_add((place as u16 & 0x1F) * 0x19).wrapping_add(0x18);
        let c = self.mem.r8(DS, at).wrapping_sub(1);
        self.mem.w8(DS, at, c);
        let ax = self.mem.r16(DS, STAGE);
        hooks.province_save(self, ax);
    }
}
