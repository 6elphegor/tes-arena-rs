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

use crate::bsa::Files;
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
    /// 1194:A374 with AX 0 (A36C): text number TEXT shown.
    fn text_shown(&mut self, _m: &mut Machine) {}
    /// 2DA2:000A: the text at DS:`di` has its substitutions made.
    fn expand(&mut self, _m: &mut Machine, _di: u16) {}
    /// 26F7:0C0A's write of the dungeon entry at DS:`si` (19h bytes) to
    /// CITYDATA, in its province (PICK_PROVINCE).
    fn entry_save(&mut self, _m: &mut Machine, _si: u16) {}
    /// 27E3:000A: the text at DS:`si` added to the logbook: the entry made
    /// (`Machine::log_entry`), then the file's appends (not ported).
    fn log_entry(&mut self, m: &mut Machine, si: u16) {
        m.log_entry(si);
    }
    /// 1194:1034 when the main quest ends (0FDD 8Eh).
    fn finale(&mut self, _m: &mut Machine) {}
    /// 1194:42EA: the text saying the inventory is full (AX, from 1026)
    /// written to 19DE, before its message box.
    fn inventory_full(&mut self, _m: &mut Machine, _ax: u16) {}
    /// 1194:3EC3 (and far 3EBF): the place `place`'s buildings listed
    /// (its plan, `Machine::city_plan_for_record`, then each block's .MIF
    /// header read for the counts at 1387/1389/138B and the lists at
    /// 1AB5/1004/1008). Not ported (file I/O): None by default.
    fn city_info(&mut self, _m: &mut Machine, _files: &mut Files, _place: u8) -> Option<()> {
        None
    }
    /// 1194:2E69 (far): the days the journey set up at 193D..1944 takes,
    /// left at 1927 (`Machine::travel_days`).
    fn travel(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.travel_days(files).map(|_| ())
    }
    /// 0000:9D60: the PC's timer ticks (the BIOS's dword at 0:046C).
    fn bios_ticks(&mut self, _m: &mut Machine) -> u32 {
        0
    }
    /// 1194:080B (0807): a name of set AL and sex AH from the RNG seed
    /// `seed`, written at DS:`di` (`Machine::npc_name`).
    fn npc_name(&mut self, m: &mut Machine, files: &mut Files, ax: u16, seed: u32, di: u16) -> Option<()> {
        // (DI after it, which some text codes need, isn't used here.)
        m.npc_name(files, ax as u8, (ax >> 8) as u8, seed, DS, di).map(|_| ())
    }
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

/// The main quest's block (0FDD..0FEF, 13h bytes): its step, the dungeon
/// holding the current piece (place, province), the next dungeon (place,
/// province), flags, the steps done (a word, bits), the chest's square.
pub mod main_quest {
    pub const STEP: u16 = 0x0FDD;
    pub const PLACE: u16 = 0x0FE2;
    pub const PROVINCE: u16 = 0x0FE3;
    pub const NEXT_PLACE: u16 = 0x0FE4;
    pub const NEXT_PROVINCE: u16 = 0x0FE5;
    pub const FLAGS: u16 = 0x0FE6;
    pub const DONE: u16 = 0x0FE7;
    pub const CHEST: u16 = 0x0FEE;
    pub const BYTES: u16 = 0x13;
    /// The pieces by step (16 bytes, 80h set); the step whose piece is
    /// 8Eh ends the quest.
    pub const PIECES: u16 = 0x78C3;
    pub const LAST_PIECE: u8 = 0x8E;
}

/// The item buffer the main quest's piece is made in (13h bytes).
const ITEM: u16 = 0x13DA;
/// The player's record (its item slots from +6Eh).
const PLAYER: u16 = 0x01A4;
/// The place: the area's kind (3 a dungeon), its province, the place in
/// it (low 5 bits).
const AREA: u16 = 0x05E6;
const HERE_PROVINCE: u16 = 0x0A46;
const HERE_PLACE: u16 = 0x0A4C;

impl Machine {
    /// 292B:06C7. The map square (row high, column doubled low) of the
    /// record at `si` (x at +0, y at +4).
    pub fn record_square(&self, si: u16) -> u16 {
        let x = self.mem.r16(DS, si);
        let y = self.mem.r16(DS, si.wrapping_add(4));
        (y >> 7) << 8 | ((x >> 7) as u8).wrapping_mul(2) as u16
    }

    /// 1194:9F04. A quest item lying in the level is picked up: the
    /// quest under way at stage 5 whose place (+14h's low byte) is this
    /// place (A4C's low five bits) is marked (+4 bit 5) and given a new
    /// quest item (`pick_quest_item` of the province's ten, 1194:3829),
    /// with bit 80h when +28h is set, put in the first free slot of the
    /// list at ITEMS (the tenth byte, 0FDD, when it is full) and at +1Ch;
    /// 0D39 bit 80h. The text at 6E0A is copied to 1044, has its
    /// substitutions made and is shown. Returns the carry: set (the item
    /// stays) when no quest wants it. None where the original loops
    /// forever (every item taken).
    pub fn quest_item_taken(&mut self, hooks: &mut dyn QuestHooks) -> Option<bool> {
        let here = self.mem.r8(DS, HERE_PLACE) & 0x1F;
        let Some(si) = (0..4u16)
            .map(|i| QUESTS + i * 0x34)
            .find(|&si| self.quest_stage(si) == 5 && self.quest_kind(si) == 1 && self.mem.r8(DS, si + 0x14) == here)
        else {
            return Some(true);
        };
        let f = self.quest_flags(si) | 0x20;
        self.mem.w16(DS, si + 4, f);
        let province = self.mem.r8(DS, HERE_PROVINCE) as u16;
        let (item, slot) = self.pick_quest_item(province)?;
        let mut al = item as u8;
        if self.mem.r8(DS, si + 0x28) != 0 {
            al |= 0x80;
        }
        self.mem.w8(DS, ITEMS.wrapping_add(slot), al);
        let c = self.mem.r8(DS, 0x0D39) | 0x80;
        self.mem.w8(DS, 0x0D39, c);
        self.mem.w8(DS, si + 0x1C, al);
        let f = self.quest_flags(si) | 0x20;
        self.mem.w16(DS, si + 4, f);
        let ax = self.mem.r16(DS, 0x102C);
        hooks.font(self, 0, ax);
        self.quest_strcpy(0x6E0A, 0x1044);
        hooks.expand(self, 0x1044);
        hooks.message_box(self, 0x1044);
        Some(false)
    }

    /// 0000:9CA0: the string at DS:`si` copied to DS:`di`, NUL included.
    fn quest_strcpy(&mut self, mut si: u16, mut di: u16) {
        loop {
            let b = self.mem.r8(DS, si);
            self.mem.w8(DS, di, b);
            if b == 0 {
                break;
            }
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
        }
    }

    /// 2294:0F33. A chest opened in a dungeon (AREA 3) at the main
    /// quest's chest square (`main_quest::CHEST`; the chest's record at
    /// `si`): in the dungeon of the current piece the chest goes
    /// (`quest_chest_clear`), the next dungeon is revealed
    /// (`quest_next_dungeon`) and this one is done (PROVINCE FFh); in the
    /// next dungeon, at the last step (the step of piece 8Eh) the chest
    /// goes, text 1FAh is shown and the quest ends (the step 8Eh for
    /// 1194:1034, then 0; NEXT_PROVINCE FFh, FLAGS bit 2 clear);
    /// otherwise with the inventory full its text is shown and nothing
    /// else, or the chest goes and the piece is the player's
    /// (`quest_piece`), NEXT_PROVINCE FFh. Returns the carry: set where
    /// the chest was the quest's. None where an item's making fails.
    pub fn quest_chest(&mut self, hooks: &mut dyn QuestHooks, si: u16) -> Option<bool> {
        use main_quest::*;
        if self.mem.r8(DS, AREA) != 3 || self.record_square(si) != self.mem.r16(DS, CHEST) {
            return Some(false);
        }
        let ax = self.mem.r16(DS, 0x102C);
        hooks.font(self, 0, ax);
        let province = self.mem.r8(DS, HERE_PROVINCE);
        let here = self.mem.r8(DS, HERE_PLACE) & 0x1F;
        if province == self.mem.r8(DS, PROVINCE) && here == self.mem.r8(DS, PLACE) {
            self.quest_chest_clear(si);
            self.quest_next_dungeon(hooks)?;
            self.mem.w8(DS, PROVINCE, 0xFF);
            return Some(true);
        }
        if province != self.mem.r8(DS, NEXT_PROVINCE) || here != self.mem.r8(DS, NEXT_PLACE) {
            return Some(false);
        }
        if self.mem.r8(DS, STEP) == self.quest_step_of(LAST_PIECE) as u8 {
            self.quest_chest_clear(si);
            self.mem.w16(DS, TEXT, 0x1FA);
            self.mem.w8(DS, TEXT_FLAG, 0);
            hooks.text_shown(self);
            self.mem.w8(DS, STEP, LAST_PIECE);
            hooks.finale(self);
            self.mem.w8(DS, NEXT_PROVINCE, 0xFF);
            let f = self.mem.r8(DS, FLAGS) & 0xFB;
            self.mem.w8(DS, FLAGS, f);
            self.mem.w8(DS, STEP, 0);
            return Some(true);
        }
        if self.free_slot(DS, PLAYER).is_err() {
            let ax = self.mem.r16(DS, 0x1026);
            hooks.inventory_full(self, ax);
            hooks.message_box(self, 0x19DE);
            return Some(true);
        }
        self.quest_chest_clear(si);
        self.quest_piece(hooks)?;
        self.mem.w8(DS, NEXT_PROVINCE, 0xFF);
        Some(true)
    }

    /// 2294:10A5. The chest at `si` taken off the floor plane (the
    /// segment at D806).
    fn quest_chest_clear(&mut self, si: u16) {
        let sq = self.record_square(si);
        let floor = self.mem.r16(DS, 0xD806);
        self.mem.w16(floor, sq, 0);
    }

    /// 1194:D316. The step whose piece (PIECES) is `piece`: 0..15, or 16
    /// when none is.
    pub fn quest_step_of(&self, piece: u8) -> u16 {
        (0..16u16).find(|&i| self.mem.r8(DS, main_quest::PIECES + i) == piece).unwrap_or(16)
    }

    /// 1194:D140. Text 1F4h is shown; "%adn" made at 929E has its
    /// substitutions made (the next dungeon's name); a random dungeon
    /// entry of the next dungeon's province takes or gives that name
    /// (26F7:0C0A, mode 1: `quest_dungeon_entry`) and its index becomes
    /// NEXT_PLACE, then it is taken again (mode 4); text 1F7h goes to the
    /// log (`quest_log_text`).
    fn quest_next_dungeon(&mut self, hooks: &mut dyn QuestHooks) -> Option<()> {
        use main_quest::*;
        self.mem.w16(DS, TEXT, 0x1F4);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.mem.w8(DS, TEXT_FLAG, 0);
        hooks.text_shown(self);
        self.mem.w32(DS, 0x929E, u32::from_le_bytes(*b"%adn"));
        self.mem.w8(DS, 0x92A2, 0);
        hooks.expand(self, 0x929E);
        let province = self.mem.r8(DS, NEXT_PROVINCE) as u16;
        let ax = self.quest_dungeon_entry(hooks, province, 1, 0, 0x929E)?;
        self.mem.w8(DS, NEXT_PLACE, ax as u8);
        let province = self.mem.r8(DS, NEXT_PROVINCE) as u16;
        self.quest_dungeon_entry(hooks, province, 4, ax, 0x929E)?;
        self.mem.w16(DS, TEXT, 0x1F7);
        self.quest_log_text(hooks);
        Some(())
    }

    /// 26F7:0C0A with its I/O as hooks: province `province` loaded, the
    /// entry picked (`dungeon_pick_entry`), the save's digits written to
    /// 1367, the entry written back (`QuestHooks::entry_save`), the place's
    /// province (A46) loaded again. Returns AX, the entry's index. The
    /// entry written is the index's (modes 0, 1 and 4), the first fixed
    /// one (2) or the second (3 and more but 4).
    fn quest_dungeon_entry(&mut self, hooks: &mut dyn QuestHooks, province: u16, mode: u16, index: u16, name: u16) -> Option<u16> {
        use crate::dungeon::{table::DUNGEON_ENTRIES, var::PICK_INDEX};
        let here = self.mem.r8(DS, HERE_PROVINCE);
        hooks.province_load(self, province as u8);
        self.dungeon_pick_entry(province, mode, index, name)?;
        let entry = match mode {
            2 => DUNGEON_ENTRIES,
            3 | 5.. => DUNGEON_ENTRIES + 0x19,
            _ => DUNGEON_ENTRIES.wrapping_add(self.mem.r16(DS, PICK_INDEX).wrapping_mul(0x19)),
        };
        let slot = self.mem.r8(DS, 0x1A6C);
        let digits = (((slot % 10) as u16) << 8 | (slot / 10) as u16).wrapping_add(0x3030);
        self.mem.w16(DS, 0x1367, digits);
        hooks.entry_save(self, entry);
        hooks.province_load(self, here);
        Some(self.mem.r16(DS, PICK_INDEX))
    }

    /// 1194:CF93. Text TEXT written to 1044 (A374, mode 2) and logged
    /// (27E3:000A).
    fn quest_log_text(&mut self, hooks: &mut dyn QuestHooks) {
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.mem.w8(DS, TEXT_FLAG, 0);
        hooks.text(self);
        hooks.log_entry(self, 0x1044);
    }

    /// 1194:D198. Unless at the last step (the step of piece 8Eh), text
    /// 1F5h is shown, the step's piece is made at ITEM (`quest_artifact`)
    /// and, with a free slot, given to the player (copied, then the
    /// items sorted); then the main quest's block is cleared but for DONE.
    fn quest_piece(&mut self, hooks: &mut dyn QuestHooks) -> Option<()> {
        use main_quest::*;
        if self.mem.r8(DS, STEP) == self.quest_step_of(LAST_PIECE) as u8 {
            return Some(());
        }
        self.mem.w16(DS, TEXT, 0x1F5);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.mem.w8(DS, TEXT_FLAG, 0);
        hooks.text_shown(self);
        let step = self.mem.r8(DS, STEP) as u16;
        self.quest_artifact(step)?;
        if let Ok((slot, _)) = self.free_slot(DS, PLAYER) {
            self.memmove(DS, ITEM, DS, slot, 0x13);
            self.sort_inventory();
        }
        let done = self.mem.r16(DS, DONE);
        for i in 0..BYTES {
            self.mem.w8(DS, STEP + i, 0);
        }
        self.mem.w16(DS, DONE, done);
        Some(())
    }

    /// 1194:D1F1. The piece of step `step` made at ITEM: its kind k
    /// (PIECES' byte without bit 80h) goes to 13C8, and k | 80h picks the
    /// maker: up to 83h armour (1194:E889) and up to 88h a weapon (E8B2),
    /// each of item level k | 80h with the type from the table at 36DD,
    /// and the byte at 36ED + k to 13EA; up to 8Bh a magic item (B848,
    /// level 14h, the forced kind from 36DD; 8Ch kind 3), 13EA FFh, the
    /// item's bits 4-5 (13E8) clear, for kinds 0Ah..0Ch 20h and the byte
    /// at 7870 to 13DE; beyond, a blank item (13DB 40h, or FFFFh with 13E0
    /// 2710h for kind 0Dh; 13E8 bits 6 and 3). Then all: 13E8 bit 0, 13EC
    /// FFh, 13EB from 78B3 + k, 13DA k | 80h, the dword 13E4 100000 and,
    /// but for kind 0Dh, 13E0 and 13E2 25000.
    fn quest_artifact(&mut self, step: u16) -> Option<()> {
        let k = self.mem.r8(DS, main_quest::PIECES.wrapping_add(step)) & 0x7F;
        self.mem.w8(DS, 0x13C8, k);
        let ty = self.mem.r8(DS, 0x36DD + k as u16);
        let ax = (k | 0x80) as u16;
        let magic = |m: &mut Self, forced: u8| -> Option<()> {
            let kind = m.mem.r8(DS, 0x13C8);
            m.shop_magic_item(0x14, forced)?;
            m.mem.w8(DS, 0x13C8, kind);
            let f = m.mem.r8(DS, 0x13E8) & 0xCF;
            m.mem.w8(DS, 0x13E8, f);
            m.mem.w8(DS, 0x13EA, 0xFF);
            if (0x0A..=0x0C).contains(&kind) {
                let f = m.mem.r8(DS, 0x13E8) & 0xCF | 0x20;
                m.mem.w8(DS, 0x13E8, f);
                let b = m.mem.r8(DS, 0x7870 + (kind - 0x0A) as u16);
                m.mem.w8(DS, 0x13DE, b);
            }
            Some(())
        };
        if ax <= 0x88 {
            if ax <= 0x83 {
                self.loot_armor(ax as u8, ty);
            } else {
                self.loot_weapon(ax as u8, ty)?;
            }
            let kind = self.mem.r8(DS, 0x13C8);
            let b = self.mem.r8(DS, 0x36ED + kind as u16);
            self.mem.w8(DS, 0x13EA, b);
        } else if ax <= 0x8B {
            magic(self, ty)?;
        } else if ax == 0x8C {
            magic(self, 3)?;
        } else {
            for i in 0..0x13 {
                self.mem.w8(DS, ITEM + i, 0);
            }
            let f = self.mem.r8(DS, 0x13E8) | 0x40 | 8;
            self.mem.w8(DS, 0x13E8, f);
            self.mem.w16(DS, 0x13DB, 0x40);
            if self.mem.r8(DS, 0x13C8) == 0x0D {
                self.mem.w16(DS, 0x13DB, 0xFFFF);
                self.mem.w16(DS, 0x13E0, 0x2710);
            }
        }
        let f = self.mem.r8(DS, 0x13E8) | 1;
        self.mem.w8(DS, 0x13E8, f);
        self.mem.w8(DS, 0x13EC, 0xFF);
        let kind = self.mem.r8(DS, 0x13C8);
        let b = self.mem.r8(DS, 0x78B3 + kind as u16);
        self.mem.w8(DS, 0x13EB, b);
        self.mem.w8(DS, ITEM, kind | 0x80);
        self.mem.w32(DS, 0x13E4, 100_000);
        if kind != 0x0D {
            self.mem.w16(DS, 0x13E0, 25_000);
            self.mem.w16(DS, 0x13E2, 25_000);
        }
        Some(())
    }
}

/// The level's candidate squares for a quest's flats (x, y bytes; FFFFh
/// used) and their count (a byte); the map's size; the floor plane's
/// segment.
const CANDIDATES: u16 = 0xC6E5;
const CANDIDATE_COUNT: u16 = 0xC765;
const MAP_WIDTH: u16 = 0xC357;
const MAP_DEPTH: u16 = 0xC359;
const FLOOR: u16 = 0xD806;
/// The flats' kinds by a quest's +2Fh / +30h (bytes), and by +28h + 0Bh
/// (the person's).
const QUEST_FLATS: u16 = 0xC953;
const PERSON_FLATS: u16 = 0xC934;
/// The offsets (words) of the squares tried around a quest's person.
const NEARBY: u16 = 0x6DF4;
/// The person's name.
const NAME: u16 = 0x6DD4;

impl Machine {
    /// 1194:9FAC (with CX 1, from the stairs' hook 0000:44D7 and the area
    /// entry 0000:4790): a dungeon's last level reached (5E0 and 5CA lose
    /// bit 400h first). Unless the player is wanted (5C8 bit 4000h), out
    /// of a dungeon (5E6 not 3) or not on its last level (BB0A + 1 not
    /// C355), the first quest under way, its goal not reached, of type 7
    /// (+4 bits 12-15), in this province (+25h's top three bits) and place
    /// (+14h), gets its flats (0F72 marks each quest looked at):
    /// - stages 4 and 6: one of the level's candidates (`quest_candidate`,
    ///   with the quest's seed +0) gets the flat of +30h; at stage 4
    ///   another (the seed rotated four bits) that of +2Fh;
    /// - stage 5: a candidate (kept at 1244) gets the flat of +30h, the
    ///   first free square at the offsets from NEARBY around it the
    ///   person (the kind at PERSON_FLATS + (+28h + 0Bh), with bit 15),
    ///   and the person a name at NAME (set +25h's province, sex +28h,
    ///   the seed +1Eh + +8, its low word plus +6 and +26h); returns the
    ///   name and the person's kind (AX, BX with the carry).
    ///
    /// Returns Some(None) with the carry clear. None where the original
    /// hangs or faults (no free candidate; none at all; no free square
    /// nearby within 8000h tries) or the name's file is missing.
    pub fn quest_level(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks) -> Option<Option<(u16, u16)>> {
        for at in [0x05E0, 0x05CA] {
            let v = self.mem.r16(DS, at) & 0xFBFF;
            self.mem.w16(DS, at, v);
        }
        if self.mem.r16(DS, 0x05C8) & 0x4000 != 0
            || self.mem.r8(DS, AREA) != 3
            || (self.mem.r8(DS, 0xBB0A) as u16).wrapping_add(1) != self.mem.r16(DS, 0xC355)
        {
            return Some(None);
        }
        for i in 0..4u16 {
            let si = QUESTS + i * 0x34;
            self.mem.w16(DS, 0x0F72, si);
            let f = self.quest_flags(si);
            if self.quest_kind(si) != 1 || f & 0x20 != 0 || f >> 12 != 7 {
                continue;
            }
            if self.mem.r8(DS, si + 0x25) >> 5 != self.mem.r8(DS, HERE_PROVINCE)
                || self.mem.r8(DS, si + 0x14) != self.mem.r8(DS, HERE_PLACE) & 0x1F
            {
                continue;
            }
            match self.quest_stage(si) {
                stage @ (4 | 6) => {
                    let seed = self.mem.r32(DS, si);
                    let at = self.quest_take_candidate(seed)?;
                    self.quest_flat(at, self.mem.r8(DS, si + 0x30));
                    if stage == 4 {
                        let at = self.quest_take_candidate(seed.rotate_left(4))?;
                        self.quest_flat(at, self.mem.r8(DS, si + 0x2F));
                    }
                    return Some(None);
                }
                5 => return self.quest_person(files, hooks, si).map(Some),
                _ => {}
            }
        }
        Some(None)
    }

    /// 1194:A0FC with its caller's next lines: a candidate picked
    /// (`quest_candidate`), its square (y high, x low) returned and the
    /// candidate used (FFFFh).
    fn quest_take_candidate(&mut self, seed: u32) -> Option<u16> {
        let e = self.quest_candidate(seed)?;
        let at = self.mem.r16(DS, e);
        self.mem.w16(DS, e, 0xFFFF);
        Some(at)
    }

    /// 1194:A0FC. With the RNG seeded from `seed` (the game's seed put
    /// back after), random candidates (0000:6EEA over the count) until
    /// one is usable: x and y not 0, not FFFFh, inside the map, its
    /// square empty on the floor plane. Returns its offset. None where
    /// the original faults (no candidates) or loops forever.
    fn quest_candidate(&mut self, seed: u32) -> Option<u16> {
        let count = self.mem.r8(DS, CANDIDATE_COUNT) as u16;
        // With no usable candidate at all the original rolls forever:
        // known at once, instead of waiting for the RNG to cycle.
        if !(0..count).any(|r| self.quest_usable(CANDIDATES + 2 * r)) {
            return None;
        }
        let saved = self.rng.seed;
        self.rng.seed = seed;
        let mut tries = crate::random::CycleCheck::new(&self.rng);
        let found = loop {
            let r = self.rng.range(0, count)?;
            let e = CANDIDATES.wrapping_add(r.wrapping_mul(2));
            if self.quest_usable(e) {
                break Some(e);
            }
            if tries.cycled(&self.rng) {
                break None;
            }
        };
        self.rng.seed = saved;
        found
    }

    /// A0FC's test of the candidate at `e`: x and y not 0, not FFFFh,
    /// inside the map, its square empty on the floor plane.
    fn quest_usable(&self, e: u16) -> bool {
        let (x, y) = (self.mem.r8(DS, e), self.mem.r8(DS, e.wrapping_add(1)));
        if x == 0 || y == 0 || self.mem.r16(DS, e) == 0xFFFF {
            return false;
        }
        if x as u16 >= self.mem.r16(DS, MAP_WIDTH) || y as u16 >= self.mem.r16(DS, MAP_DEPTH) {
            return false;
        }
        let sq = (y as u16) << 8 | x.wrapping_mul(2) as u16;
        let floor = self.mem.r16(DS, FLOOR);
        self.mem.r16(floor, sq) == 0
    }

    /// 1194:A160. The square at `at` (y high, x low; kept at 6DF2) gets
    /// the flat kind QUEST_FLATS[`which`] (82xxh on the floor plane);
    /// 5CA bit 400h.
    fn quest_flat(&mut self, at: u16, which: u8) {
        self.mem.w16(DS, 0x6DF2, at);
        let sq = at & 0xFF00 | (at as u8).wrapping_mul(2) as u16;
        let kind = self.mem.r8(DS, QUEST_FLATS + which as u16);
        let floor = self.mem.r16(DS, FLOOR);
        self.mem.w16(floor, sq, 0x8200 | kind as u16);
        let f = self.mem.r16(DS, 0x05CA) | 0x400;
        self.mem.w16(DS, 0x05CA, f);
    }

    /// 1194:9FAC's stage 5 (A081..A0FB). See `quest_level`.
    fn quest_person(&mut self, files: &mut Files, hooks: &mut dyn QuestHooks, si: u16) -> Option<(u16, u16)> {
        let at = self.quest_take_candidate(self.mem.r32(DS, si))?;
        self.mem.w16(DS, STAGE, at);
        let near = self.quest_nearby(at)?;
        let idx = 0x0Bu8.wrapping_add(self.mem.r8(DS, si + 0x28)) as u16;
        let person = self.mem.r8(DS, PERSON_FLATS + idx);
        let floor = self.mem.r16(DS, FLOOR);
        self.mem.w16(floor, near, 0x8000 | person as u16);
        let at = self.mem.r16(DS, STAGE);
        self.quest_flat(at, self.mem.r8(DS, si + 0x30));
        let ax = (self.mem.r8(DS, si + 0x28) as u16) << 8 | (self.mem.r8(DS, si + 0x25) >> 5) as u16;
        let seed = self.mem.r32(DS, si + 0x1E).wrapping_add(self.mem.r32(DS, si + 8));
        let lo = (seed as u16).wrapping_add(self.mem.r16(DS, si + 6)).wrapping_add(self.mem.r16(DS, si + 0x26));
        let seed = seed & 0xFFFF_0000 | lo as u32;
        hooks.npc_name(self, files, ax, seed, NAME)?;
        Some((NAME, self.mem.r8(DS, PERSON_FLATS + idx) as u16))
    }

    /// 1194:A180. The first square around `at` (y high, x low), at the
    /// offsets (words) from NEARBY on, that is empty on the floor plane.
    /// The list has no end: None past 8000h offsets.
    fn quest_nearby(&self, at: u16) -> Option<u16> {
        let sq = at & 0xFF00 | (at as u8).wrapping_mul(2) as u16;
        let floor = self.mem.r16(DS, FLOOR);
        (0..0x8000u16).map(|i| sq.wrapping_add(self.mem.r16(DS, NEARBY.wrapping_add(2 * i)))).find(|&bx| self.mem.r16(floor, bx) == 0)
    }
}

/// The main quest's provinces by step (bytes).
const STEP_PROVINCES: u16 = 0x7883;

impl Machine {
    /// 1194:CD9A. The main quest's next step offered. No (the carry) when
    /// the player holds a piece (`quest_piece_held`) or FLAGS bit 2 is set
    /// (the original then also works out the days left, 1194:CFF7, and
    /// refuses whichever their sign). Otherwise FLAGS loses bits 3 and 0,
    /// and a step is rolled (`quest_roll_step`); no when none comes or the
    /// step's province (the table at 7883) is this one (1194:CE01, whose
    /// call to CEC8 only computes registers). Then the step's dword at
    /// 0FDE is a random one from the table at the far pointer 1AB5
    /// (count at 1387, CE61), its provinces are picked (CE1B,
    /// `dungeon_pick_quest_provinces`), its target set (CE40,
    /// `dungeon_set_quest_target`), the time noted at 0FE9 and 2..7 days
    /// given at 0FED (CF41). Returns the carry. None where the original
    /// faults (an empty table) or loops forever.
    pub fn quest_offer(&mut self) -> Option<bool> {
        use main_quest::*;
        if self.quest_piece_held() {
            return Some(true);
        }
        let f = self.mem.r8(DS, FLAGS);
        if f & 4 != 0 {
            return Some(true);
        }
        self.mem.w8(DS, FLAGS, f & 0xF7 & 0xFE);
        if !self.quest_roll_step()? {
            return Some(true);
        }
        let step = self.mem.r8(DS, STEP) as u16;
        if self.mem.r8(DS, STEP_PROVINCES.wrapping_add(step)) == self.mem.r8(DS, HERE_PROVINCE) {
            return Some(true);
        }
        let count = self.mem.r16(DS, 0x1387);
        let r = self.rng.range(0, count)?;
        let (off, seg) = (self.mem.r16(DS, 0x1AB5), self.mem.r16(DS, 0x1AB7));
        let v = self.mem.r32(seg, off.wrapping_add(r.wrapping_shl(2)));
        self.mem.w32(DS, 0x0FDE, v);
        self.dungeon_pick_quest_provinces()?;
        self.dungeon_set_quest_target();
        let t = self.mem.r32(DS, TIME);
        self.mem.w32(DS, 0x0FE9, t);
        let days = self.rng.range(2, 8)?;
        self.mem.w8(DS, 0x0FED, days as u8);
        Some(false)
    }

    /// 1194:CDEA. Whether one of the player's first n item slots (from
    /// 0212, n the occupied slots' count, 1194:7472) holds a piece (its
    /// type with bit 80h). With none occupied the `loop` runs 65536 times,
    /// stepping 13h bytes through all of DGROUP.
    fn quest_piece_held(&self) -> bool {
        let (n, _) = self.count_items(DS, crate::inventory::PLAYER_SLOTS);
        let count = if n == 0 { 0x10000u32 } else { n as u32 };
        let mut si = crate::inventory::PLAYER_SLOTS;
        for _ in 0..count {
            if self.mem.r8(DS, si) & 0x80 != 0 {
                return true;
            }
            si = si.wrapping_add(0x13);
        }
        false
    }

    /// 1194:CE7F. Up to 32h rolls of a step 0..15 not yet done (DONE's
    /// bit clear) whose piece the player may have (`quest_piece_ok`); one
    /// that is becomes STEP and is marked done. Rolls that meet a done
    /// step count down too, and once all 32h are spent that way DONE is
    /// cleared and one more roll is made (an offered piece that is refused
    /// counts down instead, and ends the rolls at 0). Returns whether a
    /// step was found.
    fn quest_roll_step(&mut self) -> Option<bool> {
        use main_quest::*;
        let mut cx: u16 = 0x32;
        loop {
            let r = self.rng.range(0, 16)?;
            let done = self.mem.r16(DS, DONE);
            if done >> r & 1 == 0 {
                let piece = self.mem.r8(DS, PIECES + r) as u16;
                if self.quest_piece_ok(piece)? {
                    self.mem.w8(DS, STEP, r as u8);
                    let d = self.mem.r16(DS, DONE) | 1 << r;
                    self.mem.w16(DS, DONE, d);
                    return Some(true);
                }
                cx -= 1;
                if cx == 0 {
                    return Some(false);
                }
                continue;
            }
            cx -= 1;
            if cx == 0 {
                self.mem.w16(DS, DONE, 0);
                cx = 1;
            }
        }
    }

    /// 1194:7C83. Whether the player may have piece `piece`: from 89h
    /// yes; below, a trial item at ITEM (its type the piece, armour (to
    /// 83h) or weapon kind in bits 2-3 of 13E8, health 1) that the
    /// player's class (1A9's low five bits) may use (1194:7772).
    fn quest_piece_ok(&mut self, piece: u16) -> Option<bool> {
        if piece >= 0x89 {
            return Some(true);
        }
        self.mem.w8(DS, ITEM, piece as u8);
        let kind = if piece as u8 <= 0x83 { 0 } else { 1 };
        let f = self.mem.r8(DS, ITEM + 0x0E) & 0xF3 | kind << 2;
        self.mem.w8(DS, ITEM + 0x0E, f);
        self.mem.w16(DS, ITEM + 6, 1);
        let class = (self.mem.r8(DS, 0x01A9) & 0x1F) as u16;
        self.class_can_use(DS, ITEM, class)
    }
}
