//! Fast travel (1194:2E69): how long a journey takes.
//!
//! Both places are put on the world map (320 x 200 pixels of
//! TERRAIN.IMG) and a line is drawn from here to there (Bresenham, from
//! the end with the smaller row). Each pixel on it has a terrain class
//! (its colour's index in a 7-entry table, else 7) and a weather zone
//! (its province * 4 + the province quarter it is in). A pixel costs
//! 2000 / (the terrain's base for the month, scaled by a percentage for
//! the zone's current weather) hundredths of a day, the month moving on
//! every 30 days of the journey. Days are the total / 100, at least 1
//! and at most 2000; the distance is (the longer side + the shorter / 4)
//! * 20 km.
//!
//! Bugs kept: pixels are addressed from the start of the loaded file,
//! 12 bytes of header included; the two 500-byte line buffers are not
//! bounded, so a longer line writes on through the heap.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Destination (from the travel screen): x, y, province.
    pub const DEST_X: u16 = 0x193E;
    pub const DEST_Y: u16 = 0x1940;
    pub const DEST_PROVINCE: u16 = 0x193D;
    /// The current location: x, y, province (record fields).
    pub const HERE_X: u16 = 0x0A48;
    pub const HERE_Y: u16 = 0x0A4A;
    pub const HERE_PROVINCE: u16 = 0x0A46;
    /// Results: days (word), heading (0..511), distance in km, journey
    /// in minutes (dword), and the raw total (hundredths of a day).
    pub const DAYS: u16 = 0x1927;
    pub const HEADING: u16 = 0x3FF6;
    pub const DISTANCE: u16 = 0x3FF9;
    pub const MINUTES: u16 = 0x3FF2;
    pub const TOTAL: u16 = 0x4000;
    /// The line: pixel counts (two words), buffers of terrain classes and
    /// weather zones (segments), and the end point last drawn to.
    pub const COUNT: u16 = 0x407E;
    pub const COUNT_2: u16 = 0x4082;
    pub const CLASSES: u16 = 0x4080;
    pub const ZONES: u16 = 0x4084;
    pub const END_X: u16 = 0x4094;
    pub const END_Y: u16 = 0x4092;
    /// Current weather per zone (36 bytes), and the zone being worked on.
    pub const WEATHER: u16 = 0x0A5A;
    pub const ZONE: u16 = 0xA858;
    /// The loaded province's bounds (x, y, width, height) and number.
    pub const BOUNDS: u16 = 0x143D;
    pub const LOADED: u16 = 0x191E;
}
use var::*;

/// "terrain.img", the map's row offsets (y * 320), the colours of the
/// seven terrain classes, base costs (class * 12 + month) and weather
/// percentages (class * 8 + weather).
const TERRAIN_IMG: u16 = 0x4086;
const ROWS: u16 = 0xA6A0;
const CLASS_COLOURS: u16 = 0x1920;
const BASE: u16 = 0x349A;
const WEATHER_PCT: u16 = 0x34EE;
const LINE_BYTES: u16 = 500;
/// Weather tables: the season of each month, each zone's climate, each
/// climate's rows of five weathers by season; the season rolled for.
const SEASONS: u16 = 0x3526;
const CLIMATES: u16 = 0x3532;
const CLIMATE_ROWS: u16 = 0x3556;
const SEASON: u16 = 0xA84C;

impl Machine {
    /// 1194:2E69. The days a journey from the current location to the
    /// destination takes, leaving the heading, distance, minutes and
    /// totals in DGROUP. Province data is (re)loaded from the current
    /// slot's CITYDATA as the line crosses provinces. None where a
    /// province file is missing, the heap reports an error, a total
    /// overflows a division (the original faults).
    pub fn travel_days(&mut self, files: &mut Files) -> Option<u16> {
        self.ems_map(self.mem.r16(DS, 0x1030));
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        self.file_load(files, TERRAIN_IMG, buf, 0);
        let (dx, dy) = self.travel_global(files, DEST_X, DEST_Y, DEST_PROVINCE)?;
        let (hx, hy) = self.travel_global(files, HERE_X, HERE_Y, HERE_PROVINCE)?;
        let heading = travel_heading(hx, hy, dx, dy);
        self.mem.w16(DS, HEADING, heading);
        // 2F94: max + min / 4, * 20.
        let ax = hx.wrapping_sub(dx);
        let ax = if (ax as i16) < 0 { ax.wrapping_neg() } else { ax };
        let bx = hy.wrapping_sub(dy);
        let bx = if (bx as i16) < 0 { bx.wrapping_neg() } else { bx };
        let (lo, hi) = if ax >= bx { (bx, ax) } else { (ax, bx) };
        self.mem.w16(DS, DISTANCE, ((lo >> 2).wrapping_add(hi)).wrapping_mul(20));
        self.mem.w16(DS, COUNT_2, 0);
        self.mem.w16(DS, COUNT, 0);
        let classes = self.heap_alloc_high(LINE_BYTES)?;
        self.mem.w16(DS, CLASSES, classes);
        for i in 0..LINE_BYTES {
            self.mem.w8(classes, i, 0);
        }
        let zones = self.heap_alloc_high(LINE_BYTES)?;
        self.mem.w16(DS, ZONES, zones);
        for i in 0..LINE_BYTES {
            self.mem.w8(zones, i, 0);
        }
        self.mem.w16(DS, END_X, hx);
        self.mem.w16(DS, END_Y, hy);
        self.travel_line(files, dx, dy)?;
        let count = self.mem.r16(DS, COUNT);
        self.travel_sum(classes, zones, count)?;
        self.heap_free(classes);
        self.heap_free(zones);
        let days = self.mem.r16(DS, DAYS) as i16;
        let days = days.clamp(1, 2000) as u16;
        self.mem.w16(DS, DAYS, days);
        Some(days)
    }

    /// 1194:3183: a location's world-map pixel: its province is loaded
    /// (1194:2E18) and the local position scaled (`city_local_to_global`).
    fn travel_global(&mut self, files: &mut Files, x: u16, y: u16, prov: u16) -> Option<(u16, u16)> {
        let (x, y, p) = (self.mem.r16(DS, x), self.mem.r16(DS, y), self.mem.r8(DS, prov));
        self.city_load_province(files, p)?;
        self.city_local_to_global(x, y)
    }

    /// 1194:31CA: the line from END_X, END_Y to (`tx`, `ty`), recording
    /// each pixel but the last (`travel_pixel`). END_X, END_Y become the
    /// target unless the two points are the same.
    fn travel_line(&mut self, files: &mut Files, tx: u16, ty: u16) -> Option<()> {
        let (mut ax, mut bx) = (self.mem.r16(DS, END_X), self.mem.r16(DS, END_Y));
        let (mut cx, mut dx) = (tx, ty);
        if bx >= dx {
            std::mem::swap(&mut ax, &mut cx);
            std::mem::swap(&mut bx, &mut dx);
        }
        let dy = dx.wrapping_sub(bx);
        let mut dxx = cx.wrapping_sub(ax);
        let step: u16 = if (dxx as i16) < 0 {
            dxx = dxx.wrapping_neg();
            0xFFFF
        } else {
            if dxx == 0 && dy == 0 {
                return Some(());
            }
            1
        };
        let mut di = self.mem.r16(DS, ROWS.wrapping_add(bx.wrapping_mul(2))).wrapping_add(ax);
        // `loop` with CX = 0 runs 65536 times.
        let n = |c: u16| if c == 0 { 0x10000u32 } else { c as u32 };
        if dxx >= dy {
            let bp = dxx;
            let mut si = dxx.wrapping_neg();
            for _ in 0..n(dxx) {
                self.travel_pixel(files, di)?;
                di = di.wrapping_add(step);
                si = si.wrapping_add(dy);
                if (si as i16) >= 0 {
                    si = si.wrapping_sub(bp);
                    di = di.wrapping_add(320);
                }
            }
        } else {
            let bp = dy;
            let mut si = dy.wrapping_neg();
            for _ in 0..n(dy) {
                self.travel_pixel(files, di)?;
                di = di.wrapping_add(320);
                si = si.wrapping_add(dxx);
                if (si as i16) >= 0 {
                    si = si.wrapping_sub(bp);
                    di = di.wrapping_add(step);
                }
            }
        }
        self.mem.w16(DS, END_Y, ty);
        self.mem.w16(DS, END_X, tx);
        Some(())
    }

    /// 1194:3290: the pixel at `di` gets its terrain class and weather
    /// zone appended to the two line buffers.
    fn travel_pixel(&mut self, files: &mut Files, di: u16) -> Option<()> {
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let colour = self.mem.r8(buf, di);
        let class = (0..7).find(|&i| self.mem.r8(DS, CLASS_COLOURS + i) == colour).unwrap_or(7) as u8;
        let zone = self.travel_zone(files, di)?;
        let c = self.mem.r16(DS, COUNT).wrapping_add(1);
        self.mem.w16(DS, COUNT, c);
        let c2 = self.mem.r16(DS, COUNT_2).wrapping_add(1);
        self.mem.w16(DS, COUNT_2, c2);
        self.mem.w8(self.mem.r16(DS, ZONES), c2.wrapping_sub(1), zone as u8);
        self.mem.w8(self.mem.r16(DS, CLASSES), c.wrapping_sub(1), class);
        Some(())
    }

    /// 1194:3030: the weather zone of map offset `di`: its province * 4,
    /// plus 1 east of the province's middle and 2 south of it.
    fn travel_zone(&mut self, files: &mut Files, di: u16) -> Option<u16> {
        let (x, y) = (di % 320, di / 320);
        let p = self.travel_province(files, x, y)?;
        let mut zone = p << 2;
        self.mem.w16(DS, ZONE, zone);
        let r = |m: &Self, o: u16| m.mem.r16(DS, BOUNDS + o);
        if (x as i16) >= (r(self, 0).wrapping_add(r(self, 4) >> 1)) as i16 {
            zone = zone.wrapping_add(1);
            self.mem.w16(DS, ZONE, zone);
        }
        if (y as i16) >= (r(self, 2).wrapping_add(r(self, 6) >> 1)) as i16 {
            zone = zone.wrapping_add(2);
            self.mem.w16(DS, ZONE, zone);
        }
        Some(zone)
    }

    /// 1194:3080: the province whose bounds hold (x, y): the loaded one
    /// if it does, else each of 0..8 loaded in turn; 0xFFFF if none (the
    /// last loaded stays).
    fn travel_province(&mut self, files: &mut Files, x: u16, y: u16) -> Option<u16> {
        let inside = |m: &Self| {
            let r = |o: u16| m.mem.r16(DS, BOUNDS + o) as i16;
            let (x, y) = (x as i16, y as i16);
            x >= r(0) && x <= r(0).wrapping_add(r(4)) && y >= r(2) && y <= r(2).wrapping_add(r(6))
        };
        if inside(self) {
            return Some(self.mem.r8(DS, LOADED) as u16);
        }
        for p in 0..9u8 {
            self.city_load_province(files, p)?;
            if inside(self) {
                return Some(p as u16);
            }
        }
        Some(0xFFFF)
    }

    /// 1194:3332: the time along the line, `count` pixels.
    fn travel_sum(&mut self, classes: u16, zones: u16, count: u16) -> Option<()> {
        self.mem.w32(DS, TOTAL, 0);
        self.mem.w32(DS, MINUTES, 0);
        for di in 0..if count == 0 { 0x10000u32 } else { count as u32 } {
            let di = di as u16;
            let month = travel_month(self.mem.r32(DS, 0x05CC));
            let q = self.mem.r32(DS, MINUTES) / 3000;
            let mut ax = q as u16;
            let mut bx = month.wrapping_add(ax);
            if (bx as i16) >= 12 {
                ax = bx / 12;
                bx %= 12;
            }
            // AL is the class; AH is left from before.
            ax = (ax & 0xFF00) | self.mem.r8(classes, di) as u16;
            let base = self.mem.r8(DS, BASE.wrapping_add(bx).wrapping_add(ax.wrapping_mul(12))) as u16;
            let zone = self.mem.r8(zones, di) as u16;
            let w = self.mem.r8(DS, WEATHER.wrapping_add(zone)) as u16;
            let pct = self.mem.r8(DS, WEATHER_PCT.wrapping_add(w).wrapping_add(ax.wrapping_mul(8))) as u16;
            let mut bp = base;
            if pct != 0 {
                bp = ((base as u32 * pct as u32) / 100) as u16;
            }
            let t = if bp == 0 { 0 } else { (2000 / bp) as u32 };
            let total = self.mem.r32(DS, TOTAL).wrapping_add(t);
            self.mem.w32(DS, TOTAL, total);
            let m = self.mem.r32(DS, MINUTES).wrapping_add(t);
            self.mem.w32(DS, MINUTES, m);
        }
        let days = self.mem.r32(DS, TOTAL) / 100;
        let minutes = self.mem.r32(DS, MINUTES) / 100;
        if days > 0xFFFF || minutes > 0xFFFF {
            return None;
        }
        self.mem.w16(DS, DAYS, days as u16);
        self.mem.w32(DS, MINUTES, minutes * 1440);
        Some(())
    }
}

impl Machine {
    /// 1194:2FB3. Rolls the weather of all 36 zones for the current
    /// month: each zone's climate (table 3532) and the month's season
    /// (table 3526) pick a row of five weathers (table 3556), and
    /// dice(100) (0..=100) picks one of them: under 10, under 30, up to
    /// 69, up to 89, else the last. None where the dice faults.
    pub fn weather_roll(&mut self) -> Option<()> {
        let month = travel_month(self.mem.r32(DS, 0x05CC));
        let season = self.mem.r8(DS, SEASONS.wrapping_add(month)) as u16;
        self.mem.w16(DS, SEASON, season);
        for zone in 0..36u16 {
            self.mem.w16(DS, crate::city::var::WRITE_POS, zone);
            let climate = self.mem.r8(DS, CLIMATES + zone) as u16;
            let row = self.mem.r16(DS, CLIMATE_ROWS + climate * 2).wrapping_add(season * 5);
            let d = crate::dice::dice(&mut self.rng, 100)? as i16;
            let k = if d > 0x59 {
                4
            } else if d > 0x45 {
                3
            } else if d < 0x0A {
                0
            } else if d < 0x1E {
                1
            } else {
                2
            };
            let w = self.mem.r8(DS, row.wrapping_add(k));
            self.mem.w8(DS, WEATHER + zone, w);
        }
        Some(())
    }
}

/// 1194:32D6: the month (0..11) of a time in minutes.
pub fn travel_month(minutes: u32) -> u16 {
    let m = ((minutes % (360 * 1440)) / (30 * 1440)) as u16;
    if (m as i16) > 12 { 0 } else { m }
}

/// 0000:E06A: the heading from (ax, bx) to (cx, dx), 0..511 for a full
/// turn, from 64 * the smaller side / the larger.
pub fn travel_heading(ax: u16, bx: u16, cx: u16, dx: u16) -> u16 {
    let mut bp = 0u16;
    let mut cx = cx.wrapping_sub(ax);
    if (cx as i16) < 0 {
        bp = 1;
        cx = cx.wrapping_neg();
    }
    let mut dx = dx.wrapping_sub(bx);
    if (dx as i16) < 0 {
        bp |= 2;
        dx = dx.wrapping_neg();
    }
    let mut a = if cx > dx {
        let (num, den) = (dx, if cx == 0 { 1 } else { cx });
        (((num as u32) * 64 / den as u32) as u16).wrapping_neg().wrapping_add(0x7F)
    } else {
        let (num, den) = (cx, if dx == 0 { 1 } else { dx });
        ((num as u32) * 64 / den as u32) as u16
    };
    if bp & 2 != 0 {
        a = a.wrapping_neg().wrapping_add(0xFF);
    }
    if bp & 1 != 0 {
        a = a.wrapping_neg().wrapping_add(0x1FF);
    }
    a
}
