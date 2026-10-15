//! The travel map (segment 26F7): a destination picked and the journey
//! made.
//!
//! The map screen (26F7:0000) runs the window library (3159) with its
//! handlers: a click on the map (26F7:0625) finds the location nearest the
//! cursor (`travel_nearest`, 0B7A) and, on a button, picks it
//! (`travel_pick`, 03C4) and shows what the journey would be (0483: the
//! days through 1194:2E69, `Machine::travel_days`); "Travel" (26F7:07CD,
//! `travel_go`) makes the journey: the days pass a day at a time through
//! the clock (1194:2F69/30F1), then some hours, and the destination
//! becomes the current location.
//!
//! The locations of a province are its records (19h bytes) from 1445: 8
//! cities, 8 towns (150D), 16 villages (15D5); its dungeons from 1765. A
//! pick is the record's index: 0..1Fh a place, 20h on a dungeon.
//!
//! On arrival (26F7:08F7, `travel_arrival`) the text the town's welcome
//! shows (0000:4ACD) is written to entercty.$$$: where the player has
//! arrived, the date, the days the journey took and the place's
//! description.
//!
//! The drawing, the window and its texts and the travel animation are
//! `TravelHooks`.

use crate::bsa::Files;
use crate::clock::ClockHooks;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// DGROUP variables.
pub mod var {
    /// A destination chosen (byte), and the current place clicked
    /// (byte; the wilderness's own place).
    pub const CHOSEN: u16 = 0x6C28;
    pub const HERE_CLICKED: u16 = 0x6C41;
    /// The province shown on the map (byte).
    pub const SHOWN: u16 = 0x47E1;
    /// The pick (the record's index), the nearest location to the cursor,
    /// its distance, its record.
    pub const PICK: u16 = 0xA8AE;
    pub const NEAREST: u16 = 0xA8BC;
    pub const NEAREST_DIST: u16 = 0xA8B6;
    pub const NEAREST_REC: u16 = 0xA8BA;
    /// Only dungeons known (word: 1 in the second pass).
    pub const KNOWN_ONLY: u16 = 0x1294;
    /// The destination: its name (14h bytes), province (byte), x, y,
    /// location id (byte), kind (byte: 0 city, 1 town, 2 village, 3
    /// dungeon), flags (byte), heading (word) and province (word).
    pub const DEST_NAME: u16 = 0x1929;
    pub const DEST_PROVINCE: u16 = 0x193D;
    pub const DEST_ID: u16 = 0x1942;
    pub const DEST_KIND: u16 = 0x1943;
    pub const DEST_FLAGS: u16 = 0x1944;
    pub const DEST_HEADING: u16 = 0x1945;
    pub const DEST_PROVINCE_W: u16 = 0x12A8;
    /// A journey made (word), and the travel animation's frame (byte).
    pub const TRAVELLED: u16 = 0x12A2;
    pub const ANIM_FRAME: u16 = 0x40A0;
    /// The current location: province, kind, id (bytes; record at 0A22).
    pub const HERE_PROVINCE: u16 = 0x0A46;
    pub const HERE_KIND: u16 = 0x0A47;
    pub const HERE_ID: u16 = 0x0A4C;
}
use crate::travel::var::{DAYS, DEST_X, DEST_Y};
use var::*;

/// The lists of a province's locations.
const CITIES: u16 = 0x1445;
const TOWNS: u16 = 0x150D;
const VILLAGES: u16 = 0x15D5;
const DUNGEONS: u16 = 0x1765;
const RECORD: u16 = 0x19;

/// What the travel map leaves to the front end.
pub trait TravelHooks: ClockHooks {
    /// 1194:5E1D then 59C8: a message box with the text at `seg`:`si`.
    fn message_box(&mut self, _m: &mut Machine, _seg: u16, _si: u16) {}
    /// 1194:48DB: the question at DS:`si` asked; AX, nonzero to stay.
    fn question(&mut self, _m: &mut Machine, _si: u16) -> u16 {
        0
    }
    /// The palette faded to the buffer at 929E (0000:5E3E, 9426; 1194:55B2
    /// after its buffer is cleared).
    fn screen(&mut self, _m: &mut Machine) {}
    /// 0000:56C8: the resource named at DS:`si` loaded (BX `bx`).
    fn resource(&mut self, _m: &mut Machine, _si: u16, _bx: u16) {}
    /// 0000:5C86: EMS bank `bank` mapped (`Machine::ems_map`).
    fn bank(&mut self, m: &mut Machine, bank: u16) {
        m.ems_map(bank);
    }
    /// 1194:4D74: the screen's bank `bx` shown.
    fn show_bank(&mut self, _m: &mut Machine, _bx: u16) {}
    /// 1194:2E69: the journey's days (`Machine::travel_days`, which leaves
    /// them at 1927 too).
    fn days(&mut self, m: &mut Machine, files: &mut Files) -> Option<u16> {
        m.travel_days(files)
    }
    /// 1194:2F69's file load: the travel animation (named at 4096) to 902E.
    fn animation(&mut self, _m: &mut Machine) {}
    /// 0000:5526 then 9466: frame `frame` of the travel animation drawn.
    fn animation_frame(&mut self, _m: &mut Machine, _frame: u8) {}
    /// 0000:142D: `minutes` minutes pass. False to have the game's clock
    /// run them (with these hooks as its `ClockHooks`).
    fn advance(&mut self, _m: &mut Machine, _files: &mut Files, _minutes: u32) -> Option<bool> {
        Some(false)
    }
    /// 1194:2E14 / 2E18: province `p`'s data loaded
    /// (`Machine::city_load_province`).
    fn province(&mut self, m: &mut Machine, files: &mut Files, p: u8) -> Option<()> {
        m.city_load_province(files, p).map(|_| ())
    }
    /// 1194:3AE8: the location record of location `id` built.
    fn location(&mut self, _m: &mut Machine, _id: u16) {}
    /// 1194:3B0C: a dungeon's location record built (AX `id`, BX `bx`).
    fn dungeon_location(&mut self, _m: &mut Machine, _id: u16, _bx: u16) {}
    /// 26F7:08F7: whether the front end made the arrival's text itself;
    /// false (the default) runs `Machine::travel_arrival`.
    fn arrival_log(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<bool> {
        Some(false)
    }
    /// 1194:A36C with AX 2: text TEXT (137A) copied to DS:`di`
    /// (`Machine::text`).
    fn arrival_text(&mut self, m: &mut Machine, files: &mut Files, di: u16) -> Option<()> {
        m.text(files, &mut crate::text::NoTextHooks, 2, DS, di).map(|_| ())
    }
    /// 25FD:0844 then 0000:9DC2: the file named at DS:`si` deleted.
    fn arrival_delete(&mut self, m: &mut Machine, files: &mut Files, si: u16) {
        m.file_delete(files, si);
    }
    /// 25FD:0844 then 0000:9DD2: `cx` bytes from DS:`di` written to a new
    /// file named at DS:`si`.
    fn arrival_write(&mut self, m: &mut Machine, files: &mut Files, si: u16, di: u16, cx: u16) {
        m.file_write(files, si, DS, di, cx);
    }
    /// 3159:0006: the map's window closed.
    fn close(&mut self, _m: &mut Machine) {}
}

/// `TravelHooks` that leave everything to the game.
pub struct NoTravelHooks;

impl ClockHooks for NoTravelHooks {}
impl TravelHooks for NoTravelHooks {}

impl Machine {
    fn tm8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }
    fn tm16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    /// 26F7:0B7A (with 0BC9). The location of the shown province nearest
    /// (x, y) on the map, by the game's distance (the larger axis plus a
    /// quarter of the smaller), first of equals kept: NEAREST its index,
    /// NEAREST_REC its record, NEAREST_DIST the distance (from 3E8h). The
    /// 32 places, then the 16 dungeons known (+18h bit 2) after them. The
    /// Imperial province (8) has none and is not searched. Returns BX by
    /// NEAREST: 3 a dungeon, 2 a village, then 1 for a city and 0 for a
    /// town (0BBB..0BC7 tests below 8 last, the other way round).
    pub fn travel_nearest(&mut self, x: u16, y: u16) -> u16 {
        if self.tm8(SHOWN) != 8 {
            self.mem.w16(DS, NEAREST_DIST, 0x3E8);
            let mut si = CITIES;
            let mut dx = 0u16;
            self.travel_scan(x, y, &mut si, &mut dx, 0x20);
            self.mem.w16(DS, KNOWN_ONLY, 1);
            self.travel_scan(x, y, &mut si, &mut dx, 0x10);
            self.mem.w16(DS, KNOWN_ONLY, 0);
        }
        let ax = self.tm16(NEAREST);
        if ax >= 0x20 {
            3
        } else if ax >= 0x10 {
            2
        } else if ax < 8 {
            1
        } else {
            0
        }
    }

    /// 26F7:0BC9: `n` records from `si`, the index running on in `dx`.
    fn travel_scan(&mut self, x: u16, y: u16, si: &mut u16, dx: &mut u16, n: u16) {
        for _ in 0..n {
            if self.tm16(KNOWN_ONLY) == 0 || self.tm8(si.wrapping_add(0x18)) & 2 != 0 {
                let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
                let mut a = abs(x.wrapping_sub(self.tm16(si.wrapping_add(0x14))));
                let mut b = abs(y.wrapping_sub(self.tm16(si.wrapping_add(0x16))));
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                let d = (a >> 2).wrapping_add(b);
                if d <= self.tm16(NEAREST_DIST) {
                    self.mem.w16(DS, NEAREST_DIST, d);
                    self.mem.w16(DS, NEAREST_REC, *si);
                    self.mem.w16(DS, NEAREST, *dx);
                }
            }
            *si = si.wrapping_add(RECORD);
            *dx = dx.wrapping_add(1);
        }
    }

    /// 26F7:03C4. The location `ax` of the shown province picked (PICK is
    /// its index too): with the place the player is at (outside the
    /// wilderness, C39A bit 200h), or the dungeon the player is in (by
    /// the record's position, +26h of 0A22), "already here" (26F7:0B5B)
    /// and FFFFh, the choice undone; else the destination is kept: its
    /// province, id (the low byte of province * 20h + the index for a
    /// place), kind, name, position, flags, province (12A8), the heading
    /// to it (1194:3E9A) at DEST_HEADING. HERE_CLICKED is set when the
    /// player is at that place in the wilderness. Returns AX: PICK.
    pub fn travel_pick(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, ax: u16) -> Option<u16> {
        self.mem.w8(DS, HERE_CLICKED, 0);
        let p = self.tm8(SHOWN);
        self.mem.w8(DS, DEST_PROVINCE, p);
        let pick = self.tm16(PICK);
        let mut ax = ax;
        let (si, bx): (u16, u16);
        if pick < 0x20 {
            let cx = ((p as u16) << 5).wrapping_add(pick);
            if cx as u8 == self.tm8(HERE_ID) && self.tm8(HERE_KIND) != 3 {
                self.mem.w8(DS, HERE_CLICKED, 1);
                if self.tm16(crate::movement::var::LEVEL_FLAGS) & 0x200 == 0 {
                    self.mem.w8(DS, HERE_CLICKED, 0);
                    return Some(self.travel_already(hooks));
                }
            }
            self.mem.w8(DS, DEST_ID, cx as u8);
            if ax >= 0x10 {
                ax = ax.wrapping_sub(0x10);
                (si, bx) = (VILLAGES, 2);
            } else if ax >= 8 {
                ax = ax.wrapping_sub(8);
                (si, bx) = (TOWNS, 1);
            } else {
                (si, bx) = (CITIES, 0);
            }
        } else {
            if self.tm8(HERE_KIND) == 3 && p == self.tm8(0x0A22 + 0x24) {
                let rec = self.tm16(NEAREST_REC);
                if self.mem.r32(DS, 0x0A22 + 0x26) == self.mem.r32(DS, rec.wrapping_add(0x14)) {
                    return Some(self.travel_already(hooks));
                }
            }
            self.mem.w8(DS, DEST_ID, pick as u8);
            ax = ax.wrapping_sub(0x20);
            (si, bx) = (DUNGEONS, 3);
        }
        let (x, y) = self.travel_dest(ax, bx, si);
        let prov = self.tm8(DEST_PROVINCE);
        let heading = self.travel_heading_to(files, hooks, x, y, prov)?;
        self.mem.w16(DS, DEST_HEADING, heading);
        Some(self.tm16(PICK))
    }

    /// 26F7:0404 with 0B5B: "already here" (the text at DATA:500C with the
    /// current place's name at 0A22, written to 1044), the choice undone.
    fn travel_already(&mut self, hooks: &mut dyn TravelHooks) -> u16 {
        self.sprintf((DS, 0x1044), (DATA, 0x500C), &[Arg::Str(DS, 0x0A22)]);
        hooks.message_box(self, DS, 0x1044);
        self.mem.w8(DS, CHOSEN, 0);
        0xFFFF
    }

    /// 26F7:05EA. The destination from record `ax` of the list at `si`,
    /// of kind `bx`: kind, province (12A8), position, flags, name.
    /// Returns its position.
    fn travel_dest(&mut self, ax: u16, bx: u16, si: u16) -> (u16, u16) {
        self.mem.w8(DS, DEST_KIND, bx as u8);
        let si = si.wrapping_add(ax.wrapping_mul(RECORD));
        let p = self.tm8(SHOWN) as u16;
        self.mem.w16(DS, DEST_PROVINCE_W, p);
        // Each read after the write before it: a record past the lists
        // (a pick past them wraps round) can lie over these.
        let x = self.tm16(si.wrapping_add(0x14));
        self.mem.w16(DS, DEST_X, x);
        let y = self.tm16(si.wrapping_add(0x16));
        self.mem.w16(DS, DEST_Y, y);
        let f = self.tm8(si.wrapping_add(0x18));
        self.mem.w8(DS, DEST_FLAGS, f);
        // 0000:9CA0: the name copied with its NUL.
        let mut i = 0u16;
        loop {
            let c = self.tm8(si.wrapping_add(i));
            self.mem.w8(DS, DEST_NAME.wrapping_add(i), c);
            if c == 0 {
                break;
            }
            i = i.wrapping_add(1);
        }
        (self.tm16(si.wrapping_add(0x14)), self.tm16(si.wrapping_add(0x16)))
    }

    /// 1194:3E9A: the map row of a province's local (x, y): the
    /// province loaded (1194:3183's 2E18), then 3E9E (`city_map_y`).
    fn travel_heading_to(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, x: u16, y: u16, prov: u8) -> Option<u16> {
        hooks.province(self, files, prov)?;
        let (_, row) = self.city_map_y(x, y)?;
        Some(row)
    }

    /// 26F7:07CD. "Travel" on the map:
    /// - nothing chosen: "select a destination" (DATA:5024);
    /// - the wilderness's own place clicked (HERE_CLICKED): only the end;
    /// - else, when the player is unwell (5B1 bits 0..2), "Considering
    ///   your condition, you may not survive this journey" asked first
    ///   (6B1D; nonzero: the map stays, nothing else); then the shown province is the destination's (12A8), the
    ///   screen cleared (1194:55B2), 5C8 bit 20h set while travelling, the
    ///   days (1194:2E69), past 20 days a d10 less 5 added, a Spellsword's
    ///   (class 0Eh, 26F7:0D50) journey shortened by its level; the days
    ///   pass (1194:2F69), then a d23 + 1 hours more; the destination
    ///   becomes the current location (its record built: 1194:3AE8, a
    ///   dungeon's 3B0C), the arrival logged (26F7:08F7), the choice
    ///   undone;
    /// - the end (26F7:08D2): FC8 FFFFh, 5C8 bit 20h clear, TRAVELLED 1,
    ///   the screen cleared.
    pub fn travel_go(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks) -> Option<()> {
        if self.tm8(CHOSEN) == 0 {
            hooks.message_box(self, DATA, 0x5024);
            return Some(());
        }
        if self.tm8(HERE_CLICKED) == 0 {
            if self.tm16(0x05B1) & 7 != 0 && hooks.question(self, 0x6B1D) != 0 {
                return Some(());
            }
            let p = self.tm16(DEST_PROVINCE_W) as u8;
            self.mem.w8(DS, SHOWN, p);
            self.travel_screen_clear(hooks);
            let bank = self.tm16(0x929E);
            hooks.bank(self, bank);
            hooks.resource(self, 0x6C35, 0x24);
            let f = self.tm16(0x05C8) | 0x20;
            self.mem.w16(DS, 0x05C8, f);
            let ax = hooks.days(self, files)?;
            hooks.show_bank(self, 5);
            let mut bx = 0u16;
            if self.tm16(DAYS) > 0x14 {
                bx = self.rng.range(0, 10)?.wrapping_sub(5);
            }
            let ax = self.travel_class_days(ax.wrapping_add(bx));
            self.travel_days_pass(files, hooks, ax)?;
            let hours = self.rng.range(0, 0x17)?.wrapping_add(1).wrapping_mul(0x3C);
            self.travel_advance(files, hooks, hours as u32)?;
            let kind = self.tm8(DEST_KIND);
            self.mem.w8(DS, HERE_KIND, kind);
            if kind != 3 {
                let p = self.tm8(SHOWN);
                self.mem.w8(DS, HERE_PROVINCE, p);
                let id = self.tm8(DEST_ID) as u16;
                hooks.location(self, id);
            } else {
                let p = self.tm8(SHOWN);
                hooks.province(self, files, p)?;
                let (id, bx) = (self.tm8(DEST_ID) as u16, self.tm16(DEST_Y));
                hooks.dungeon_location(self, id, bx);
            }
            if !hooks.arrival_log(self, files)? {
                self.travel_arrival(files, hooks)?;
            }
            self.travel_clear();
            self.mem.w8(DS, CHOSEN, 0);
        }
        let bank = self.tm16(0x1032);
        hooks.bank(self, bank);
        hooks.close(self);
        self.mem.w16(DS, 0x0FC8, 0xFFFF);
        let f = self.tm16(0x05C8) & 0xFFDF;
        self.mem.w16(DS, 0x05C8, f);
        self.mem.w16(DS, TRAVELLED, 1);
        self.travel_screen_clear(hooks);
        Some(())
    }

    /// 1194:55B6's state: the palette buffer (300h bytes at 929E) cleared;
    /// the fade itself is the front end's.
    fn travel_screen_clear(&mut self, hooks: &mut dyn TravelHooks) {
        for i in 0..0x300u16 {
            self.mem.w8(DS, 0x929E + i, 0);
        }
        hooks.screen(self);
    }

    /// 26F7:0D50. A Spellsword's (class byte 1A9 0Eh) days: `ax` less
    /// ax * 6 / 256 * (level byte 1AA + 1), at least 1. Anyone else's are
    /// the days 1194:2E69 left (DAYS), the jitter lost.
    fn travel_class_days(&mut self, ax: u16) -> u16 {
        if self.tm8(0x01A9) == 0x0E {
            self.mem.w16(DS, DAYS, ax);
            let cut = (ax.wrapping_mul(6) >> 8).wrapping_mul(self.tm8(0x01AA) as u16 + 1);
            let left = self.tm16(DAYS).wrapping_sub(cut);
            self.mem.w16(DS, DAYS, if (left as i16) <= 0 { 1 } else { left });
        }
        self.tm16(DAYS)
    }

    /// 1194:2F69 with 1194:2F4D and 1194:30F1: DAYS `ax`, kept to 1..7D0h (signed);
    /// the travel animation loaded and its frame from 0; then each day 5A0h
    /// minutes pass and the animation steps (frames 0..0Ah).
    fn travel_days_pass(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, ax: u16) -> Option<()> {
        self.mem.w16(DS, DAYS, ax);
        if (self.tm16(DAYS) as i16) <= 1 {
            self.mem.w16(DS, DAYS, 1);
        }
        if (self.tm16(DAYS) as i16) >= 0x7D0 {
            self.mem.w16(DS, DAYS, 0x7D0);
        }
        hooks.bank(self, 0x15);
        hooks.animation(self);
        self.mem.w8(DS, ANIM_FRAME, 0);
        let days = self.tm16(DAYS);
        for _ in 0..days {
            self.travel_advance(files, hooks, 0x5A0)?;
            hooks.bank(self, 0x15);
            let frame = self.tm8(ANIM_FRAME);
            hooks.animation_frame(self, frame);
            let f = frame.wrapping_add(1);
            self.mem.w8(DS, ANIM_FRAME, if f >= 0x0B { 0 } else { f });
        }
        Some(())
    }

    /// 0000:142D for `minutes`, through the hooks.
    fn travel_advance(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, minutes: u32) -> Option<()> {
        if !hooks.advance(self, files, minutes)? {
            self.advance_minutes(files, minutes, hooks)?;
        }
        Some(())
    }

    /// 26F7:08F7. The arrival's text, unless the destination is a dungeon
    /// (DEST_KIND 3): in ARRIVAL (10A8, 19Bh bytes cleared) "You have
    /// arrived in ", then "The <kind> of <name>\rin <province>
    /// Province.\r" (the kind's name the (id mod 20h) / 8th from 18F5,
    /// formatted at 9C62), or in the Imperial province (SHOWN 8) "the
    /// Imperial City. "; "The date is\r" and the date (0000:1508); " It
    /// took %d days to reach your goal. " with the days (1927, at least
    /// 1); and the place's text copied (1194:A36C, AX 2): 258h + province
    /// * 8 + index for a city, 58Eh for a town, 58Fh for a village. The
    /// text's LFs and CRs made spaces, wrapped at 32h (1194:49FF), the
    /// wraps made CRs, a CR and a NUL put after it, and the text (its
    /// length then + 2) written to entercty.$$$ (130C), which the town's
    /// welcome shows; 5C8 bit 100h set.
    pub fn travel_arrival(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks) -> Option<()> {
        const ARRIVAL: u16 = 0x10A8;
        const SCRATCH: u16 = 0x9C62;
        const FILE: u16 = 0x130C;
        self.mem.w8(DS, ARRIVAL, 0);
        if self.tm8(DEST_KIND) == 3 {
            return Some(());
        }
        self.mem.w16(DS, 0x6ACD, SCRATCH);
        for i in 0..0x19Bu16 {
            self.mem.w8(DS, ARRIVAL + i, 0);
        }
        self.arrival_copy(ARRIVAL, DATA, 0x4F05);
        let index = self.tm8(HERE_ID) as u16 % 0x20;
        self.mem.w16(DS, 0x6ACF, index);
        let mut si = 0x6C42;
        if self.tm8(SHOWN) != 8 {
            let kind = self.skip_strings(DS, 0x18F5, index >> 3);
            let scratch = self.tm16(0x6ACD);
            self.sprintf((DS, scratch), (DATA, 0x4F74), &[Arg::Str(DS, kind), Arg::Str(DS, DEST_NAME), Arg::Str(DS, 0x1429)]);
            si = scratch;
        }
        self.arrival_cat(ARRIVAL, DS, si);
        self.arrival_cat(ARRIVAL, DATA, 0x4F1A);
        let scratch = self.tm16(0x6ACD);
        let minutes = self.mem.r32(DS, 0x05CC);
        self.log_date(minutes, scratch);
        self.arrival_cat(ARRIVAL, DS, scratch);
        if (self.tm16(DAYS) as i16) <= 1 {
            self.mem.w16(DS, DAYS, 1);
        }
        let days = self.tm16(DAYS);
        self.sprintf((DS, scratch), (DATA, 0x4F27), &[Arg::Int(days)]);
        self.arrival_cat(ARRIVAL, DS, scratch);
        let text = if index >= 0x10 {
            0x58F
        } else if index >= 8 {
            0x58E
        } else {
            ((self.tm8(HERE_PROVINCE) as u16) << 3).wrapping_add(index).wrapping_add(0x258)
        };
        self.mem.w16(DS, 0x137A, text);
        // repne scasb from ARRIVAL, CX 3E8h, then dec di.
        let end = (0..0x3E8u16).find(|&k| self.tm8(ARRIVAL.wrapping_add(k)) == 0).unwrap_or(0x3E7);
        self.mem.w8(DS, 0x101C, 1);
        self.mem.w8(DS, 0x137E, 0x32);
        hooks.arrival_text(self, files, ARRIVAL.wrapping_add(end))?;
        self.mem.w16(DS, 0x6ACD, 0x1044);
        let len = self.arrival_len(ARRIVAL).wrapping_add(2);
        self.arrival_replace(ARRIVAL, 0x0A, 0x20);
        self.arrival_replace(ARRIVAL, 0x0D, 0x20);
        self.mem.w8(DS, 0x137E, 0x32);
        self.sq_wrap(ARRIVAL)?;
        self.arrival_replace(ARRIVAL, 0x0A, 0x0D);
        // repne scasb, CX 19Ch: DI past the NUL (or past the last byte).
        let di = (0..0x19Cu16).find(|&k| self.tm8(ARRIVAL.wrapping_add(k)) == 0).map_or(0x19C, |k| k + 1);
        let di = ARRIVAL.wrapping_add(di);
        self.mem.w8(DS, di.wrapping_sub(1), 0x0D);
        self.mem.w16(DS, di, 0);
        hooks.arrival_delete(self, files, FILE);
        hooks.arrival_write(self, files, FILE, ARRIVAL, len);
        let f = self.tm16(0x05C8) | 0x100;
        self.mem.w16(DS, 0x05C8, f);
        Some(())
    }

    /// 0000:9CA0 with ES DGROUP: the string at `seg`:`si` copied to DS:`di`
    /// with its NUL.
    fn arrival_copy(&mut self, di: u16, seg: u16, si: u16) {
        let mut i = 0u16;
        loop {
            let c = self.mem.r8(seg, si.wrapping_add(i));
            self.mem.w8(DS, di.wrapping_add(i), c);
            if c == 0 {
                break;
            }
            i = i.wrapping_add(1);
        }
    }

    /// 0000:9BD0: the string at `seg`:`si` appended to the one at DS:`di`.
    fn arrival_cat(&mut self, di: u16, seg: u16, si: u16) {
        let end = di.wrapping_add(self.arrival_len(di));
        self.arrival_copy(end, seg, si);
    }

    /// 0000:9CAC: the length of the string at DS:`di`.
    fn arrival_len(&self, di: u16) -> u16 {
        let mut n = 0u16;
        while self.tm8(di.wrapping_add(n)) != 0 {
            n = n.wrapping_add(1);
        }
        n
    }

    /// 1194:49DF: in the string at `si` (its length first), each `al`
    /// made `bl`.
    fn arrival_replace(&mut self, si: u16, al: u8, bl: u8) {
        let n = self.arrival_len(si);
        for i in 0..n {
            let at = si.wrapping_add(i);
            match self.tm8(at) {
                0 => break,
                c if c == al => self.mem.w8(DS, at, bl),
                _ => {}
            }
        }
    }

    /// 26F7:07A9. The choice undone: the name's 14h bytes, the province,
    /// position, id, flags and heading cleared.
    pub fn travel_clear(&mut self) {
        for i in 0..0x14u16 {
            self.mem.w8(DS, DEST_NAME + i, 0);
        }
        self.mem.w8(DS, DEST_PROVINCE, 0);
        self.mem.w16(DS, DEST_X, 0);
        self.mem.w16(DS, DEST_Y, 0);
        self.mem.w8(DS, DEST_ID, 0);
        self.mem.w8(DS, DEST_FLAGS, 0);
        self.mem.w16(DS, DEST_HEADING, 0);
    }
}
