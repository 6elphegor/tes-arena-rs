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

/// The screen's windows' records: the world map's (TAMRIEL.MNU, its
/// provinces' regions at DATA:6D90) and a province's (MAPBTNS.MNU, its
/// buttons' regions at DATA:7164).
const WORLD: u16 = 0x6C56;
const PROVINCE: u16 = 0x6C7C;
/// Where 3159:0E8A keeps the world map's window while a province's runs.
const KEPT: u16 = 0x9686;
/// The markers' segments (MAPBLINK.CIF, MAPOUT.CIF, 0000:7428 with DI
/// FFFEh) and the boxes' tiles' resource (PARCH.CIF, 0000:756C).
const BLINK_SEG: u16 = 0x6ACB;
const OUT_SEG: u16 = 0x6AC9;
const TILES: u16 = 0x1038;

/// A province map's regions under the cursor (B7C6, 3159:019A): its
/// buttons in MAPBTNS.MNU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapRegion {
    /// Region 1: Travel (26F7:07CD).
    Travel,
    /// Region 2: Exit (3159:0006).
    Exit,
    /// Region 3: Search (26F7:03A4).
    Search,
    /// None (or region 0, which has no handler): the map itself.
    Map,
}

impl Machine {
    /// 2542:0400..0405: the world kept while the travel map is up
    /// (0000:A611, `world_keep`, `dac` the colours on the screen) and the
    /// clip put back from BA58 (A602: A606, `clip_back`).
    pub fn map_keep(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>) {
        self.world_keep(files, dac);
        self.clip_back();
    }

    /// 2542:0411..043A (`carry`, the area left) or 043B..0445 with the
    /// far calls: FCE cleared, the view's window (0000:A5CD: A5D1,
    /// `view_window`), C994 set, the world given back (A6B5,
    /// `world_back`), C398 bit 80h; without the carry the window and the
    /// world only. The screens (9426, 9466) and the palette named at 1304
    /// (2542:0446) are the front end's. None where the world can't be
    /// given back (the heap, TEMPSAVE.$$$).
    pub fn map_button_after(&mut self, files: &mut Files, carry: bool) -> Option<()> {
        if carry {
            self.mem.w16(DS, crate::trigger::var::OUTSIDE_X, 0);
        }
        self.view_window();
        if carry {
            self.mem.w8(DS, 0xC994, 1);
        }
        self.world_back(files)?;
        if carry {
            let f = self.ms16(crate::movement::var::MAP_FLAGS) | 0x80;
            self.mem.w16(DS, crate::movement::var::MAP_FLAGS, f);
        }
        Some(())
    }

    /// 0000:56C8 with BX bit 2 (not bit 1): the picture named at
    /// DS:`si` loaded whole to the EMS frame's start (0000:742C), its
    /// header's x, y, width and height to A836..A83C, and unless BX bit
    /// 80h drawn from there (582C, where the page is the heap's block:
    /// `page_is_block`). The page shown (bit 40h) and its colours (set,
    /// or faded to with bit 20h, unless bit 10h) are the front end's.
    /// None where the file can't be loaded, or for another BX.
    pub fn picture_to_frame(&mut self, files: &mut Files, si: u16, bx: u16) -> Option<()> {
        if bx & 2 != 0 || bx & 4 == 0 || bx & 9 != 0 {
            return crate::giveup::not_ported("0000:56C8: a picture to the heap, its background kept, or a routine after");
        }
        let frame = self.ms16(0x9030);
        self.file_load(files, si, frame, 0)?;
        for k in 0..4u16 {
            let v = self.mem.r16(frame, 2 * k);
            self.mem.w16(DS, 0xA836 + 2 * k, v);
        }
        if bx & 0x80 == 0 && self.page_is_block() {
            self.page_image(frame, 0);
        }
        Some(())
    }

    /// 26F7:0000..00A6 whole: the screen's start (`travel_map_begin`'s
    /// state; the clip the whole screen, 9A98; the boxes' tiles PARCH.CIF
    /// registered, 0000:756C, 20 x 20, its slot at 1038 selected, 9AF8;
    /// the markers MAPBLINK.CIF and MAPOUT.CIF loaded from the heap's top
    /// to 6ACB and 6AC9; the screen's bank 5, 1194:4D74), then the world
    /// map's window opened (3159:000C up to its loop: `window_open` on
    /// DATA:6C56, the current province outlined, 26F7:013E). `dac` the
    /// colours on the screen, read into the window's colours' block.
    /// None where a file is missing or the heap has no room.
    pub fn travel_map_open(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, dac: Option<&[[u8; 3]; 256]>) -> Option<MapStart> {
        self.clip_whole();
        let start = self.travel_map_begin(hooks);
        let font = self.ms16(0x102C);
        self.font_selected(font, false);
        let slot = self.resource_add(files, 0x3AE0, 0x14, 0x14)?;
        self.mem.w16(DS, TILES, slot);
        self.font_selected(slot, true);
        let blink = self.load_alloc_high(files, 0x6AA4);
        self.mem.w16(DS, BLINK_SEG, blink);
        let out = self.load_alloc_high(files, 0x6AB1);
        self.mem.w16(DS, OUT_SEG, out);
        self.screen_bank(5);
        self.travel_world_open(files, dac)?;
        Some(start)
    }

    /// 3159:000C on DATA:6C56 up to its loop (or 3159:0494 from 26F7:0251):
    /// the world map's window opened, the current province outlined (its
    /// routine 26F7:013E: OUTPROV.CIF to the frame in bank [1030]).
    fn travel_world_open(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>) -> Option<()> {
        self.window_open(files, WORLD, dac)?;
        self.province_outlined(files)?;
        self.window_opened();
        Some(())
    }

    /// 26F7:013E: OUTPROV.CIF loaded to the frame in bank [1030] (0000:
    /// 7428), its image for the current province (A46: 1194:5CC8) drawn
    /// into the page where the page is the heap's block (5828, its
    /// LZHUF unpacked into a heap block and freed), the bank before
    /// mapped again.
    fn province_outlined(&mut self, files: &mut Files) -> Option<()> {
        let old = self.ms16(crate::level::var::EMS_MAPPED) >> 2;
        let bank = self.ms16(0x1030);
        self.ems_map(bank);
        let frame = self.ms16(0x9030);
        let r = self.file_load(files, 0x6BAA, frame, 0).map(|_| ());
        if r.is_some() && self.page_is_block() {
            let si = self.cif_frame(self.ms8(0x0A46) as u16);
            self.page_image(frame, si);
        }
        self.ems_map(old);
        r
    }

    /// The world map's region under the cursor at (`x`, `y`) (8F92,
    /// 8F94 set, 3159:019A): a province (0..8), 9 the Exit, None.
    pub fn travel_world_region(&mut self, x: u16, y: u16) -> Option<u16> {
        self.mem.w16(DS, 0x8F92, x);
        self.mem.w16(DS, 0x8F94, y);
        self.window_regions();
        let r = self.ms16(crate::window::var::REGION);
        (r != 0xFFFF).then_some(r)
    }

    /// A province map's region under the cursor at (`x`, `y`), as
    /// `travel_world_region` finds it.
    pub fn travel_province_region(&mut self, x: u16, y: u16) -> MapRegion {
        match self.travel_world_region(x, y) {
            Some(1) => MapRegion::Travel,
            Some(2) => MapRegion::Exit,
            Some(3) => MapRegion::Search,
            _ => MapRegion::Map,
        }
    }

    /// 26F7:0176 whole, province `p`'s region clicked on the world map:
    /// its map's state (`travel_map_province`; its picture loaded to the
    /// frame twice, 56C8 with BX 84h then 4, and its icons' pictures, as
    /// 26F7:0268 loads them), bank 4 mapped for the page's copy (its
    /// pixels the front end's), the world's window kept (3159:0E8A at
    /// 9686), B7C4 bit 1, no fades (B7E4), the screen's bank [1032], and
    /// the province's window opened (3159:000C on DATA:6C7C up to its
    /// loop). None where a file is missing or the heap has no room.
    pub fn travel_province_open(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, p: u8, dac: Option<&[[u8; 3]; 256]>) -> Option<()> {
        self.travel_map_province(files, hooks, p)?;
        // 01CD..01E4: bank 4 mapped and the page ([A8FE]) copied whole to
        // the frame's start.
        self.ems_map(4);
        let (poff, pseg, frame) = (self.ms16(0xA8FE), self.ms16(0xA900), self.ms16(0x9030));
        for i in 0..64000u16 {
            let b = self.mem.r8(pseg, poff.wrapping_add(i));
            self.mem.w8(frame, i, b);
        }
        self.window_save(KEPT);
        let f = self.ms8(crate::window::var::FLAGS) | 2;
        self.mem.w8(DS, crate::window::var::FLAGS, f);
        self.mem.w8(DS, crate::window::var::NO_FADE, 1);
        let bank = self.ms16(0x1032);
        self.screen_bank(bank);
        self.window_open(files, PROVINCE, dac)?;
        self.window_opened();
        Some(())
    }

    /// 26F7:0218..0267, the province's window ended (its Exit, or a
    /// journey made: 3159:0006): its loop's end (`window_ended`), the
    /// world's window put back (3159:0EB6 from 9686), B7C4 bit 1; with a
    /// journey made bank 5 mapped and the world's loop to end too
    /// (3159:0006: true); else the world map's window opened again, its
    /// fades on (B7E4 0, the screen's bank 5). None where a file is
    /// missing or the heap has no room.
    pub fn travel_province_close(&mut self, files: &mut Files, dac: Option<&[[u8; 3]; 256]>) -> Option<bool> {
        self.window_ended();
        self.window_restore(KEPT);
        let f = self.ms8(crate::window::var::FLAGS) | 2;
        self.mem.w8(DS, crate::window::var::FLAGS, f);
        if self.ms16(TRAVELLED) != 0 {
            self.ems_map(5);
            self.window_close();
            return Some(true);
        }
        self.mem.w8(DS, crate::window::var::NO_FADE, 0);
        self.screen_bank(5);
        self.travel_world_open(files, dac)?;
        Some(false)
    }

    /// The world map's loop ended (its Exit, or a journey made) and
    /// 26F7:00AE..00C6: its end (`window_ended`), the markers' blocks
    /// freed (6ACB, 6AC9: 0000:6C32), the tiles' resource dropped
    /// (9326), then the screen's end (`travel_map_end`): its carry.
    pub fn travel_map_close(&mut self, hooks: &mut dyn MapHooks, start: MapStart) -> bool {
        self.window_ended();
        let (blink, out) = (self.ms16(BLINK_SEG), self.ms16(OUT_SEG));
        self.heap_free(blink);
        self.heap_free(out);
        let slot = self.ms16(TILES);
        self.resource_drop(slot);
        self.travel_map_end(hooks, start)
    }
    fn ms8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }
    fn ms16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    /// 2542:038F..03BE with 03CF. Whether the journey may start from
    /// here: outside (C398 bit 4) or where 1A81 allows it; else "You
    /// cannot travel from here." (5D0D). The colour 60h set either way
    /// (03AE, 03CF; the fonts' loads 0000:9AE6, 9AF8 are the front end's).
    pub fn map_from_here(&mut self, hooks: &mut dyn TravelHooks) -> bool {
        if self.ms16(crate::movement::var::MAP_FLAGS) & 4 != 0 || self.ms8(0x1A81) != 0 {
            return true;
        }
        self.mem.w8(DS, 0x9288, 0x60);
        hooks.message_box(self, DS, 0x5D0D);
        false
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

    /// 26F7:0176..01CA's state: province `p`'s map shown (the window's
    /// region B7C6, or the current province at the start): the nearest's
    /// record the first city's, the blink counter 0, SHOWN `p`, the
    /// province's data loaded (1194:2E14), the palette's buffer cleared
    /// (1194:55B2), its picture and its icons' pictures loaded to the EMS
    /// frame (`TravelHooks::resource`, 26F7:0268), whose drawing is the
    /// front end's: what it leaves is the markers' half sizes (A8BE,
    /// A8C0: the last list's, the villages' 4 x 4, or in the Imperial
    /// province the dungeons' 7 x 4), the picture's pointer 8F9A clear,
    /// and the word 1294 clear, which the nearest's search (26F7:0BC9)
    /// then takes as "every place", not only the known ones. None where
    /// a file is missing.
    pub fn travel_map_province(&mut self, files: &mut Files, hooks: &mut dyn TravelHooks, p: u8) -> Option<()> {
        self.mem.w16(DS, 0xA8BA, crate::travelmap::CITIES);
        self.mem.w8(DS, 0xA8C4, 0);
        self.mem.w8(DS, SHOWN, p);
        hooks.province(self, files, p)?;
        self.travel_screen_clear(hooks);
        // 0197..01C5: the province's picture (its name the p'th at 6BB6)
        // loaded to the frame in bank [1030], twice.
        let bank = self.ms16(0x1030);
        self.ems_map(bank);
        let name = self.skip_strings(DS, 0x6BB6, p as u16);
        hooks.resource(self, files, name, 0x84)?;
        hooks.resource(self, files, name, 4)?;
        // 0268: the icons, each list's picture loaded to the frame (0325,
        // its segment at 8F9C) and its half size at A8BE, A8C0, and drawn
        // centred on each of its places: the cities (one in the Imperial
        // province), the known dungeons after the first, the staff's
        // piece's dungeon from STAFDUNG.CIF where known (its frame for
        // the province drawn, 1194:5CC8, 5828), the towns and villages
        // but in the Imperial province.
        let known = crate::travelmap::var::KNOWN_ONLY;
        self.mem.w16(DS, known, 0);
        let cities = if p == 8 { 1 } else { 8 };
        self.map_icons(files, 0x6B80, 0x10, 0x0B, cities, crate::travelmap::CITIES)?;
        self.mem.w16(DS, known, 1);
        self.map_icons(files, 0x6B9E, 0x0E, 0x08, 0x0F, 0x1765 + 0x19)?;
        if self.ms8(0x1765 + 0x18) & 2 != 0 {
            self.mem.w32(DS, 0x8F9A, 0);
            let old = self.ms16(crate::level::var::EMS_MAPPED) >> 2;
            let bank = self.ms16(0x1030);
            self.ems_map(bank);
            let frame = self.ms16(0x9030);
            let r = self.file_load(files, 0x6ABC, frame, 0);
            if r.is_some() && self.page_is_block() {
                let si = self.cif_frame(p as u16);
                self.page_image(frame, si);
            }
            self.ems_map(old);
            r?;
        }
        self.mem.w16(DS, known, 0);
        if p != 8 {
            self.map_icons(files, 0x6B89, 0x09, 0x0A, 8, 0x150D)?;
            self.map_icons(files, 0x6B92, 0x08, 0x08, 0x10, 0x15D5)?;
        }
        self.mem.w32(DS, 0x8F9A, 0);
        Some(())
    }

    /// 26F7:0325: the icon named at DS:`si` loaded to the frame in bank
    /// [1030] (0000:7428), the frame's segment at 8F9C, half its width
    /// `w` and height `h` at A8BE and A8C0; then each of the `n` places'
    /// records from DS:`di` (19h bytes each) drawn on, unless only the
    /// known are (1294) and it isn't (bit 2 of its +18h): the picture at
    /// [8F9A] centred on its +14h, +16h, cut to the clip with its 0s left
    /// out (0000:5A66, where the page is the heap's block). The bank
    /// before mapped again.
    fn map_icons(&mut self, files: &mut Files, si: u16, w: u16, h: u16, n: u16, di: u16) -> Option<()> {
        let old = self.ms16(crate::level::var::EMS_MAPPED) >> 2;
        let bank = self.ms16(0x1030);
        self.ems_map(bank);
        let frame = self.ms16(0x9030);
        let r = self.file_load(files, si, frame, 0).map(|_| ());
        if r.is_some() {
            self.mem.w16(DS, 0x8F9C, frame);
            self.mem.w16(DS, 0xA8BE, w >> 1);
            self.mem.w16(DS, 0xA8C0, h >> 1);
            let only_known = self.ms16(crate::travelmap::var::KNOWN_ONLY) != 0;
            if self.page_is_block() {
                let page_seg = self.ms16(0xA900);
                let mut page: Vec<u8> = (0..64000u32).map(|i| self.mem.r8(page_seg, i as u16)).collect();
                let (soff, sseg) = (self.ms16(0x8F9A), self.ms16(0x8F9C));
                let mut rec = di;
                for _ in 0..n {
                    if !only_known || self.ms8(rec.wrapping_add(0x18)) & 2 != 0 {
                        let x = self.ms16(rec.wrapping_add(0x14)).wrapping_sub(self.ms16(0xA8BE));
                        let y = self.ms16(rec.wrapping_add(0x16)).wrapping_sub(self.ms16(0xA8C0));
                        crate::render::clipped_page(self, &mut page, x, y, w, h, sseg, soff);
                    }
                    rec = rec.wrapping_add(0x19);
                }
                for (i, &b) in page.iter().enumerate() {
                    self.mem.w8(page_seg, i as u16, b);
                }
            }
        }
        self.ems_map(old);
        r
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
        if c.is_ascii_uppercase() {
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
                // AX 8h (26F7:0AFB): the list's kind (1194:5734).
                self.list_kind = 0x8;
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
