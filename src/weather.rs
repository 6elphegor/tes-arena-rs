//! Weather and fog: an area's weather on entering it, and the fog's light.
//!
//! Entering an outdoor area (0000:0F05) sets the climate (DA8: 0, 4 or 8
//! by the terrain at A50) and the weather from the zone's (A5A[A51], as
//! `weather_roll` left it): BB09 0 clear, 1 overcast, 81h rain, 82h snow,
//! 2 snow without the precipitation bit (snow becomes rain in the
//! desert). Some weathers bring fog (F81 and F82, the "now" and "wanted"
//! flags): overcast always, rain and snow on one day in four, weather 7 by
//! a roll. Fog sets the fill colour 64h (a grey) and loads FOG.LGT in
//! place of NORMAL.LGT, whose rows fade to that grey.
//!
//! Each frame the sky's drawing hook asks `fog_step` (2294:0B01) first:
//! in fog by day outdoors (or in a level with fog, CB5C), only the
//! player's light counts, with radius 280h, so the world fades into the
//! fill colour past a few squares; the view is filled with the fog's
//! colour instead of the sky, and the sky's objects are not drawn (C39A
//! bit 20h). The fog's light table is made once (2294:0C36, marked by bit
//! 80h of F81 or CB5C). By night, or out of fog, the normal table comes
//! back (`light_table_reset`).

use crate::light::var::{DARK, FAR, LIMIT, NEAR_COUNT};
use crate::light::LightHooks;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Fog now and fog wanted (bit 80h: its table made; bit 2: the day's
    /// fog).
    pub const FOG: u16 = 0x0F81;
    pub const FOG_WANTED: u16 = 0x0F82;
    /// A level with fog (from its INF; bit 80h: its table made).
    pub const LEVEL_FOG: u16 = 0xCB5C;
    /// The fill colour (the light table's last rows) and the view's
    /// background colour.
    pub const FILL: u16 = 0xC3C6;
    pub const BACKGROUND: u16 = 0xC3C7;
    /// The colour 0000:9A6E fills with.
    pub const FILL_COLOUR: u16 = 0x9288;
    /// The player's light radius.
    pub const PLAYER_LIGHT: u16 = 0xC3E2;
    /// The climate (0, 4, 8), the terrain class, the zone and the zones'
    /// weathers.
    pub const CLIMATE: u16 = 0x0DA8;
    pub const TERRAIN: u16 = 0x0A50;
    pub const ZONE: u16 = 0x0A51;
    pub const ZONE_WEATHER: u16 = 0x0A5A;
    /// The day of the month (& 1Fh).
    pub const DAY: u16 = 0xCCA8;
    /// The weather's roll.
    pub const ROLL: u16 = 0xA836;
}
use var::*;

/// The .LGT names.
pub const NORMAL_LGT: u16 = 0x1A6D;
pub const FOG_LGT: u16 = 0x1A78;

const AREA_FLAGS: u16 = crate::movement::var::MAP_FLAGS;
const LEVEL_FLAGS: u16 = crate::movement::var::LEVEL_FLAGS;

impl Machine {
    fn rows_reset(&mut self) {
        self.mem.w16(DS, LIMIT, 0xE);
        self.mem.w16(DS, DARK, 0xD);
        self.mem.w16(DS, FAR, 0xE);
    }

    /// 0000:0F05. An outdoor area's climate and weather, on entering it:
    /// - the climate DA8 from the terrain A50: 0 for 1 and 4, 4 for 3 and
    ///   5, else 8; CB66 from A4E; a roll to A836;
    /// - the zone's weather w picks the weather (BB09) and fog (F81 and
    ///   F82): 0 clear; 1 and 6 overcast with fog; 2 rain (81h), 3 snow
    ///   (82h), with fog when the day & 3 is 0; 4 snow (2) with fog; 5
    ///   rain with fog; 7 snow (2), fog when the roll is under 3E80h;
    ///   others overcast with fog. Snow in the desert (A50 3) is rain
    ///   (81h);
    /// - the fill colours 70h; 5C8 bit 1 (thunder) cleared, and for rain
    ///   (81h as picked) set again on a roll up to 5DC0h;
    /// - in fog, on days with bit 8 (837A FFFFh, fog flags bit 2) too,
    ///   the fill colours 64h and FOG.LGT loaded;
    /// - PAL.COL loaded and set.
    pub fn weather_set(&mut self, hooks: &mut dyn LightHooks) {
        let terrain = self.mem.r8(DS, TERRAIN);
        let climate = match terrain {
            1 | 4 => 0,
            3 | 5 => 4,
            _ => 8,
        };
        self.mem.w16(DS, CLIMATE, climate);
        let v = self.mem.r16(DS, 0x0A4E);
        self.mem.w16(DS, 0xCB66, v);
        let roll = self.rng.next();
        self.mem.w16(DS, ROLL, roll);
        let w = self.mem.r8(DS, ZONE_WEATHER.wrapping_add(self.mem.r8(DS, ZONE) as u16));
        let quarter = (self.mem.r16(DS, DAY) & 3 == 0) as u8;
        let (fog, weather) = match w {
            0 => (0, 0),
            1 | 6 => (1, 1),
            2 => (quarter, 0x81),
            3 => (quarter, 0x82),
            4 => (1, 2),
            7 => ((self.mem.r16(DS, ROLL) < 0x3E80) as u8, 2),
            5 => (1, 0x81),
            _ => (1, 1),
        };
        self.mem.w8(DS, FOG_WANTED, fog);
        self.mem.w8(DS, FOG, fog);
        self.mem.w8(DS, crate::sky::var::WEATHER, weather);
        if terrain == 3 && (weather == 0x82 || weather == 2) {
            self.mem.w8(DS, crate::sky::var::WEATHER, 0x81);
        }
        self.mem.w8(DS, FILL, 0x70);
        self.mem.w8(DS, BACKGROUND, 0x70);
        let f = self.mem.r16(DS, 0x05C8) & !1;
        self.mem.w16(DS, 0x05C8, f);
        if weather == 0x81 && self.rng.next() <= 0x5DC0 {
            self.mem.w16(DS, 0x05C8, f | 1);
        }
        if self.mem.r8(DS, FOG) != 0 {
            if self.mem.r16(DS, DAY) & 8 != 0 {
                self.mem.w16(DS, 0x837A, 0xFFFF);
                let v = self.mem.r8(DS, FOG) | 2;
                self.mem.w8(DS, FOG, v);
                let v = self.mem.r8(DS, FOG_WANTED) | 2;
                self.mem.w8(DS, FOG_WANTED, v);
            }
            self.mem.w8(DS, FILL, 0x64);
            self.mem.w8(DS, BACKGROUND, 0x64);
            hooks.lgt(self, FOG_LGT);
        }
        hooks.weather_palette(self);
    }

    /// 2294:0B01, the sky's drawing hook's first step. True (CF) when the
    /// sky isn't drawn: in a level with fog (CB5C), or outdoors in the
    /// day (100h..300h) with fog (F81): the row limits reset, the fog's
    /// table made (`fog_table`), the view filled with the background
    /// colour, the player's light 280h and the only one, no daylight,
    /// C39A bit 20h; or with fog wanted (F82) the fog begins
    /// (`fog_start`). Otherwise C39A bit 20h is cleared and, with fog
    /// set, the normal table comes back.
    pub fn fog_step(&mut self, hooks: &mut dyn LightHooks) -> bool {
        let t = self.mem.r16(DS, crate::sky::var::TIME);
        let fog = if self.mem.r8(DS, LEVEL_FOG) != 0 {
            true
        } else if self.mem.r16(DS, AREA_FLAGS) & 4 == 0 || !(0x100..=0x300).contains(&t) {
            false
        } else if self.mem.r8(DS, FOG) != 0 {
            true
        } else if self.mem.r8(DS, FOG_WANTED) != 0 {
            self.fog_start(hooks);
            return true;
        } else {
            false
        };
        if !fog {
            let v = self.mem.r16(DS, LEVEL_FLAGS) & !0x20;
            self.mem.w16(DS, LEVEL_FLAGS, v);
            if self.mem.r8(DS, FOG) != 0 {
                self.light_table_reset(hooks);
            }
            return false;
        }
        self.rows_reset();
        self.fog_table(hooks);
        let c = self.mem.r8(DS, BACKGROUND);
        self.mem.w8(DS, FILL_COLOUR, c);
        hooks.fog_fill(self);
        self.mem.w16(DS, PLAYER_LIGHT, 0x280);
        self.mem.w16(DS, NEAR_COUNT, 1);
        let v = self.mem.r16(DS, AREA_FLAGS) & 0x7FFF;
        self.mem.w16(DS, AREA_FLAGS, v);
        let v = self.mem.r16(DS, LEVEL_FLAGS) | 0x20;
        self.mem.w16(DS, LEVEL_FLAGS, v);
        true
    }

    /// 2294:0BEF. The fog begins: the row limits reset, F81 from F82, the
    /// fill colours 64h, FOG.LGT loaded, and the fog's table made
    /// (`fog_table` with F81 1 for the while).
    pub fn fog_start(&mut self, hooks: &mut dyn LightHooks) {
        self.rows_reset();
        let v = self.mem.r8(DS, FOG_WANTED);
        self.mem.w8(DS, FOG, v);
        self.mem.w8(DS, FILL, 0x64);
        self.mem.w8(DS, BACKGROUND, 0x64);
        hooks.lgt(self, FOG_LGT);
        let saved = self.mem.r8(DS, FOG);
        self.mem.w8(DS, FOG, 1);
        self.fog_table(hooks);
        self.mem.w8(DS, FOG, saved);
    }

    /// 2294:0C36. The fog's light table, once (bit 80h of CB5C in a level
    /// with fog, which begins the fog first, else of F81): the row limits
    /// reset, the table made from FOG.LGT, its rows from 0D00h filled
    /// with the fill colour, and the lamps' colour a grey (22h, 22h,
    /// 25h).
    pub fn fog_table(&mut self, hooks: &mut dyn LightHooks) {
        let level = self.mem.r8(DS, LEVEL_FOG);
        if level != 0 {
            if level & 0x80 != 0 {
                return;
            }
            self.mem.w8(DS, LEVEL_FOG, level | 0x80);
            self.fog_start(hooks);
        } else {
            let f = self.mem.r8(DS, FOG);
            if f & 0x80 != 0 {
                return;
            }
            self.mem.w8(DS, FOG, f | 0x80);
        }
        self.rows_reset();
        self.mem.w16(DS, 0xCCC2, 0);
        hooks.fog_light_table(self);
        let seg = self.mem.r16(DS, 0xCCC2);
        let c = self.mem.r8(DS, FILL);
        for i in 0..0x200u16 {
            self.mem.w8(seg, 0xD00 + i, c);
        }
        for (i, v) in [0x22, 0x22, 0x25].into_iter().enumerate() {
            self.mem.w8(DS, 0x1044 + i as u16, v);
        }
        hooks.palette(self, 0x71, 0x1044);
    }
}
