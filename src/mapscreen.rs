//! The travel map's screen around the journey: the map button (2542:03DC)
//! that opens it, the screen's start and end (26F7:0000), a click on a
//! province's map (26F7:0625: the nearest location, picked on a press, and
//! the journey it would be shown, 0483), and the search for a place by
//! name or from a list (26F7:03A4).
//!
//! The screen itself runs the window library (3159): the world map
//! (TAMRIEL.MNU, its provinces' regions at DATA:6D90, each opening the
//! province's map, 26F7:0176), then a province's map (its picture, the
//! locations' icons, MAPBTNS.MNU's buttons at DATA:7164: search 03A4,
//! travel 07CD (`Machine::travel_go`), exit). That drawing and the
//! window's input are the front end's; what the handlers do to the game's
//! state is here, with `travel_go` and `travel_pick` in `travelmap`.

use crate::bsa::Files;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::travel::var::{DAYS, DISTANCE, MINUTES};
use crate::travelmap::var::*;
use crate::travelmap::TravelHooks;

/// What the screen's handlers leave to the front end beyond the journey's
/// hooks.
pub trait MapHooks: TravelHooks {
    /// 292B:0362: the picture of a main quest's dungeon (by province `ax`)
    /// shown on arriving at one.
    fn quest_dungeon(&mut self, _m: &mut Machine, _ax: u16) {}
    /// 1194:5C30 with SI 6AD1, CX 14h: the place's name typed at DS:9C62
    /// (at most 14h characters, its NUL after).
    fn type_name(&mut self, _m: &mut Machine) {}
    /// 1194:5730 with AX 8: a pick from the `cx` names at DS:`si` (NUL
    /// after each); its index, or FFFFh (negative) for none.
    fn list(&mut self, _m: &mut Machine, _si: u16, _cx: u16) -> u16 {
        0xFFFF
    }
}

/// `MapHooks` that leave everything to the game.
pub struct NoMapHooks;

impl crate::clock::ClockHooks for NoMapHooks {}
impl TravelHooks for NoMapHooks {}
impl MapHooks for NoMapHooks {}

/// The current location as the screen keeps it at its start, for its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapStart {
    pub id: u8,
    pub kind: u8,
    pub province: u8,
}

/// The names' scratch (9C62) and the clicks' counter (A8C4).
const NAMES: u16 = 0x9C62;
const BLINK: u16 = 0xA8C4;

impl Machine {
    fn ms8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }
    fn ms16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    /// 2542:03DC..03FF. The map button: refused from a boat (in water,
    /// C39A bit 10h, with bit 4000h: "You cannot travel from a boat...",
    /// 5D56) or with monsters near (FFB: 5D2A), the text shown; true when
    /// the map opens.
    pub fn map_button(&mut self, hooks: &mut dyn TravelHooks) -> bool {
        let f = self.ms16(crate::movement::var::LEVEL_FLAGS);
        let refusal = if f & 0x10 != 0 && f & 0x4000 != 0 {
            Some(0x5D56)
        } else if self.ms16(0x0FFB) != 0 {
            Some(0x5D2A)
        } else {
            None
        };
        match refusal {
            Some(si) => {
                hooks.message_box(self, DS, si);
                false
            }
            None => true,
        }
    }

    /// 2542:0411..043A. After the map's screen: when it answered with the
    /// carry (`travel_map_end`), FCE cleared, C994 set and the area left
    /// (C398 bit 80h). The view kept and put back around it (0000:A611,
    /// A602, A5CD, A6B5), the screens (9426, 9466) and the palette named
    /// at 1304 (7530, 9B4E) are the front end's.
    pub fn map_button_done(&mut self, carry: bool) {
        if !carry {
            return;
        }
        self.mem.w16(DS, crate::trigger::var::OUTSIDE_X, 0);
        self.mem.w8(DS, 0xC994, 1);
        let f = self.ms16(crate::movement::var::MAP_FLAGS) | 0x80;
        self.mem.w16(DS, crate::movement::var::MAP_FLAGS, f);
    }

    /// 26F7:0000..0045 and 008C..009B, the screen's state at its start:
    /// nothing chosen (`travel_clear`), no journey made, the place not
    /// clicked, the palette's buffer cleared (1194:55B2), the current
    /// location kept, the pick and the nearest 0, the window's flags
    /// (B7E4 0, B7C4 bit 2) and the colour EEh. The fonts' and the
    /// markers' loads (0000:9AE6, 756C, 9AF8, 7428 into 6ACB and 6AC9)
    /// and the screen's bank (1194:4D74) are the front end's.
    pub fn travel_map_begin(&mut self, hooks: &mut dyn TravelHooks) -> MapStart {
        self.mem.w8(DS, CHOSEN, 0);
        self.travel_clear();
        self.mem.w16(DS, TRAVELLED, 0);
        self.mem.w8(DS, HERE_CLICKED, 0);
        self.travel_screen_clear(hooks);
        let start = MapStart { id: self.ms8(HERE_ID), kind: self.ms8(HERE_KIND), province: self.ms8(HERE_PROVINCE) };
        self.mem.w16(DS, PICK, 0);
        self.mem.w16(DS, NEAREST, 0);
        self.mem.w8(DS, 0xB7E4, 0);
        self.mem.w8(DS, 0x9288, 0xEE);
        let f = self.ms8(0xB7C4) | 2;
        self.mem.w8(DS, 0xB7C4, f);
        start
    }

    /// 26F7:0176..0192's state: province `p`'s map shown (the window's
    /// region B7C6, or the current province at the start): the nearest's
    /// record the first city's, the blink counter 0, SHOWN `p`, the
    /// province's data loaded (1194:2E14) and the palette's buffer
    /// cleared (1194:55B2). Its pictures, icons and window are the front
    /// end's. (Not compared with the original's vectors.)
    pub fn travel_map_province(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, p: u8) -> Option<()> {
        self.mem.w16(DS, 0xA8BA, crate::travelmap::CITIES);
        self.mem.w8(DS, 0xA8C4, 0);
        self.mem.w8(DS, SHOWN, p);
        hooks.province(self, files, p)?;
        self.travel_screen_clear(hooks);
        Some(())
    }

    /// 26F7:00CE..013D, the screen's end: the area to run next (5E6) from
    /// where the player now is (after a journey) or still is: but in the
    /// start dungeon (5E6 2) with no journey made, left as it is; 0 a
    /// place, 3 a dungeon, 2 a dungeon of the main quest (id up to 21h;
    /// below 21h its picture shown, 292B:0362, after the screen's
    /// palette cleared). The carry, the area to be left: clear when
    /// still where the screen started (`start`) with no journey made,
    /// else set (with a journey made there, 6636 cleared). For a dungeon
    /// now, "where it started" is its kind 3, province and id; for a
    /// place only the id is compared (0117 is jumped over). A main
    /// quest's dungeon leaves the province in AL (00FA), so its id is
    /// compared with the province, as in the original. The start
    /// dungeon with no journey: the carry clear. CD00 FFFFh and 7A46 0
    /// either way.
    pub fn travel_map_end(&mut self, hooks: &mut dyn MapHooks, start: MapStart) -> bool {
        let carry = 'carry: {
            if self.ms8(0x05E6) == 2 && self.ms16(TRAVELLED) == 0 {
                break 'carry false;
            }
            self.mem.w8(DS, 0x05E6, 0);
            let mut al = start.id;
            if self.ms8(HERE_KIND) == 3 {
                self.mem.w8(DS, 0x05E6, 3);
                if self.ms8(HERE_ID) <= 0x21 {
                    self.mem.w8(DS, 0x05E6, 2);
                    let ax = self.ms8(HERE_PROVINCE);
                    al = ax;
                    if self.ms8(HERE_ID) != 0x21 {
                        self.travel_screen_clear(hooks);
                        hooks.quest_dungeon(self, ax as u16);
                    }
                }
                if start.kind != 3 || start.province != self.ms8(HERE_PROVINCE) {
                    break 'carry true;
                }
            }
            if al != self.ms8(HERE_ID) {
                break 'carry true;
            }
            if self.ms16(TRAVELLED) == 0 {
                break 'carry false;
            }
            self.mem.w8(DS, 0x6636, 0);
            true
        };
        self.mem.w16(DS, 0xCD00, 0xFFFF);
        self.mem.w16(DS, 0x7A46, 0);
        carry
    }

    /// 26F7:0625's state, a click on (or the cursor over) a province's
    /// map at (8F92, 8F94): the nearest location (`travel_nearest`); bank 4
    /// mapped (the screen's copy, 062E); the current place's marker (26F7:071F) when its province is shown; on
    /// a press (AFF9) the nearest chosen and picked (`travel_pick`), and
    /// the journey shown (`travel_journey`) unless refused; the clicks
    /// counted (A8C4), the colour EEh left (9288, after the label's); every fourth with a choice in the shown province,
    /// the destination's marker. The name's label, the blits and the
    /// markers' pictures are the front end's.
    pub fn travel_click(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks) -> Option<()> {
        let (x, y) = (self.ms16(0x8F92), self.ms16(0x8F94));
        self.travel_nearest(x, y);
        // 062E..0648: the screen kept in bank 4 copied back (the bank
        // stays mapped).
        hooks.bank(self, 4);
        if self.ms8(SHOWN) == self.ms8(HERE_PROVINCE) {
            let pick = self.ms16(PICK);
            self.mem.w16(DS, PICK, self.ms8(HERE_ID) as u16);
            let kind = self.ms8(HERE_KIND) as u16;
            self.travel_marker(kind);
            self.mem.w16(DS, PICK, pick);
        }
        // 0676..06CE: the nearest's name labelled (shadow CEh, text DFh),
        // the colour EEh after.
        self.mem.w8(DS, 0x9288, 0xEE);
        if self.ms8(0xAFF9) != 0 {
            self.mem.w8(DS, CHOSEN, 1);
            let ax = self.ms16(NEAREST);
            self.mem.w16(DS, PICK, ax);
            if self.map_pick(files, hooks, ax)? != 0xFFFF {
                self.map_journey(files, hooks)?;
            }
        }
        let b = self.ms8(BLINK).wrapping_add(1);
        self.mem.w8(DS, BLINK, b);
        if self.ms8(CHOSEN) != 0 && b & 3 != 0 && self.ms8(SHOWN) == self.ms8(DEST_PROVINCE) {
            let kind = self.ms8(DEST_KIND) as u16;
            self.travel_marker(kind);
        }
        Some(())
    }

    /// 26F7:071F's state: a dungeon's marker (kind 3, the pick's low six
    /// bits 20h) builds the identity remap at 929E (100h bytes) and its
    /// colour at 937A, C2h for the dungeon the player is in, else DFh;
    /// the marker itself (0000:9D84, or a frame of the marker's picture
    /// placed over the location, 5828) is the front end's.
    fn travel_marker(&mut self, kind: u16) {
        if kind != 3 || self.ms16(PICK) % 0x40 != 0x20 {
            return;
        }
        for i in 0..0x100u16 {
            self.mem.w8(DS, 0x929E + i, i as u8);
        }
        let here = self.ms16(PICK) == self.ms8(HERE_ID) as u16 && self.ms8(SHOWN) == self.ms8(HERE_PROVINCE);
        self.mem.w8(DS, 0x937A, if here { 0xC2 } else { 0xDF });
    }

    /// 26F7:0483. The journey to the destination shown (not for the
    /// wilderness's own place, HERE_CLICKED), in 1044: where it is ("%s in
    /// %s Province." for a dungeon, 4F4D; "The %s in the %s." in the
    /// Imperial province, 4F61; else "The <kind> of <name>\rin <province>
    /// Province.", 4F74), "The date is" and the date, the days
    /// (1194:2E69, `TravelHooks::days`; the screen's bank put back,
    /// 1194:4D74), "Based on the current weather, it will take %d days
    /// to travel here.", "The total distance is %d km." (3FF9) and "You
    /// should arrive by" with the date after the journey's minutes
    /// (3FF2); a message box; the shown province loaded again (1194:2E14)
    /// and the screen's bank mapped.
    pub fn travel_journey(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks) -> Option<()> {
        const TEXT: u16 = 0x1044;
        const SCRATCH: u16 = 0x92EE;
        if self.ms8(HERE_CLICKED) != 0 {
            return Some(());
        }
        let kind = self.ms8(DEST_KIND);
        if kind == 3 {
            self.sprintf((DS, TEXT), (DATA, 0x4F4D), &[Arg::Str(DS, DEST_NAME), Arg::Str(DS, 0x1429)]);
        } else if self.ms16(DEST_PROVINCE_W) == 8 {
            self.sprintf((DS, TEXT), (DATA, 0x4F61), &[Arg::Str(DS, DEST_NAME), Arg::Str(DS, 0x1429)]);
        } else {
            let name = self.skip_strings(DS, 0x18F5, kind as u16);
            self.sprintf((DS, TEXT), (DATA, 0x4F74), &[Arg::Str(DS, name), Arg::Str(DS, DEST_NAME), Arg::Str(DS, 0x1429)]);
        }
        self.ms_cat(TEXT, DATA, 0x4F1A);
        let now = self.mem.r32(DS, 0x05CC);
        self.log_date(now, SCRATCH);
        self.ms_cat(TEXT, DS, SCRATCH);
        hooks.days(self, files)?;
        let bank = self.ms16(0x1032);
        hooks.show_bank(self, bank);
        self.ms_cat(TEXT, DATA, 0x4F92);
        let days = self.ms16(DAYS);
        self.sprintf((DS, SCRATCH), (DATA, 0x4FB2), &[Arg::Int(days)]);
        self.ms_cat(TEXT, DS, SCRATCH);
        let km = self.ms16(DISTANCE);
        self.sprintf((DS, SCRATCH), (DATA, 0x4FD8), &[Arg::Int(km)]);
        self.ms_cat(TEXT, DS, SCRATCH);
        self.ms_cat(TEXT, DATA, 0x4FF6);
        let then = now.wrapping_add(self.mem.r32(DS, MINUTES));
        self.log_date(then, SCRATCH);
        self.ms_cat(TEXT, DS, SCRATCH);
        hooks.message_box(self, DS, TEXT);
        let p = self.ms8(SHOWN);
        hooks.province(self, files, p)?;
        let bank = self.ms16(0x1032);
        hooks.bank(self, bank);
        Some(())
    }

    /// 26F7:03A4 with 0ABC. The search: the names' scratch (1F4h bytes
    /// at 9C62) cleared, a name typed (`MapHooks::type_name`); one
    /// starting with a letter is looked for among the province's 32
    /// places (0B2E: equal, ignoring the case of a..z, 0000:9C2C), else
    /// (or not found) the places' names listed (0B1B) and one picked
    /// (1194:5730, `MapHooks::list`): PICK its index; then picked
    /// (`travel_pick`, AX the place's x) and, unless refused, chosen
    /// (CHOSEN) and the journey shown. The redraw (1194:5E1D, 0000:710E)
    /// is the front end's.
    pub fn travel_search(&mut self, files: &mut Files, hooks: &mut dyn MapHooks) -> Option<()> {
        for i in 0..0x1F4u16 {
            self.mem.w8(DS, NAMES + i, 0);
        }
        hooks.type_name(self);
        let c = self.ms8(NAMES) & 0xDF;
        let mut found = None;
        if (b'A'..=b'Z').contains(&c) {
            let mut si = crate::travelmap::CITIES;
            for i in 0..0x20u16 {
                if self.sq_streq(si, NAMES).is_some() {
                    found = Some((i, si));
                    break;
                }
                si = si.wrapping_add(0x19);
            }
        }
        let si = match found {
            Some((i, si)) => {
                self.mem.w16(DS, PICK, i);
                si
            }
            None => {
                // 0B1B: each name copied after the last (0000:9CA0 leaves
                // DI past the NUL).
                let mut di = NAMES;
                let mut si = crate::travelmap::CITIES;
                for _ in 0..0x20 {
                    let mut k = 0u16;
                    loop {
                        let b = self.ms8(si.wrapping_add(k));
                        self.mem.w8(DS, di, b);
                        di = di.wrapping_add(1);
                        k = k.wrapping_add(1);
                        if b == 0 {
                            break;
                        }
                    }
                    si = si.wrapping_add(0x19);
                }
                let ax = hooks.list(self, NAMES, 0x20);
                if (ax as i16) < 0 {
                    return Some(());
                }
                self.mem.w16(DS, PICK, ax);
                crate::travelmap::CITIES.wrapping_add(ax.wrapping_mul(0x19))
            }
        };
        let x = self.ms16(si.wrapping_add(0x14));
        if self.map_pick(files, hooks, x)? != 0xFFFF {
            self.mem.w8(DS, CHOSEN, 1);
            self.map_journey(files, hooks)?;
        }
        Some(())
    }

    /// 26F7:03C4 from the screen's handlers (`travel_pick`, or the test's
    /// stand-in).
    fn map_pick(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, ax: u16) -> Option<u16> {
        match hooks.pick_stub(self, ax) {
            Some(ax) => Some(ax),
            None => self.travel_pick(files, hooks, ax),
        }
    }

    /// 26F7:0483 from the screen's handlers (`travel_journey`, or the
    /// test's stand-in).
    fn map_journey(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks) -> Option<()> {
        if hooks.journey_stub(self) {
            return Some(());
        }
        self.travel_journey(files, hooks)
    }

    /// 0000:9BD0: the string at `seg`:`si` appended to the one at DS:`di`.
    fn ms_cat(&mut self, di: u16, seg: u16, si: u16) {
        let mut end = di;
        while self.ms8(end) != 0 {
            end = end.wrapping_add(1);
        }
        let mut i = 0u16;
        loop {
            let c = self.mem.r8(seg, si.wrapping_add(i));
            self.mem.w8(DS, end.wrapping_add(i), c);
            if c == 0 {
                break;
            }
            i = i.wrapping_add(1);
        }
    }
}
