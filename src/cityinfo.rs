//! A place's buildings listed (1194:3EBF / 3EC3), for its quests.
//!
//! For the place AL (the current one at A22, or another of the province
//! built into a block of 38h bytes from the heap, 1194:3B84 with the
//! terrain's climate, 3D91), its plan of blocks is made again from its
//! seed (`city_plan_for_record`) and each block's .MIF header read
//! (1194:4189): the words at +1Bh, +1Dh, +1Fh of the 3Dh bytes after
//! "MHDR" and its size add up the place's taverns (1387), equipment
//! stores (138B) and temples (1389). Kept at 124A/124C/124E, those counts are
//! then capped by the lists made from the current level's map
//! (1194:40CF): every square whose word is a flat kind twice (C918 the
//! taverns', C917 the stores', C91A the temples') gets an entry of four
//! bytes, its square then two random bytes (a d23 and a d23, a d20 and a
//! d10, a d3 and a d2 the temples': d5, d9 or d10 by the first), drawn
//! again while an earlier entry has the same pair, the RNG put back
//! after. The palace's square (C919) and another kind's (C922) are kept
//! at 138D/138F. A place of id 2 or E0h (+2Ah) has the temples' first
//! entry's pair made 1, 7 or 2, 8. For another place that is in fact the
//! current one the header counts stand; its block is freed. (The lists'
//! names follow their addresses: the store finds itself in 1008.)
//!
//! The lists are the current level's even for another place: only the
//! counts are that place's. ES is taken to be DGROUP on entry (the
//! original keeps the caller's in 124E for the current place).

use crate::bsa::Files;
use crate::city::record;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The current place's record and its id (+2Ah).
    pub const RECORD: u16 = 0x0A22;
    pub const PLACE: u16 = 0x0A4C;
    /// The counts: taverns, equipment stores, temples.
    pub const TAVERNS: u16 = 0x1387;
    pub const STORES: u16 = 0x138B;
    pub const TEMPLES: u16 = 0x1389;
    /// The header counts kept (taverns, stores, temples); 124E first
    /// holds the record's segment and 1250 its offset.
    pub const KEPT: u16 = 0x124A;
    /// The lists (far pointers): taverns', stores', temples'.
    pub const LIST_TAVERNS: u16 = 0x1AB5;
    pub const LIST_STORES: u16 = 0x1008;
    pub const LIST_TEMPLES: u16 = 0x1004;
    /// The flat kinds of a tavern, a store, a temple, the palace and C922.
    pub const KIND_TAVERN: u16 = 0xC918;
    pub const KIND_STORE: u16 = 0xC917;
    pub const KIND_TEMPLE: u16 = 0xC91A;
    pub const KIND_PALACE: u16 = 0xC919;
    pub const KIND_C922: u16 = 0xC922;
    /// The palace's square and C922's.
    pub const PALACE: u16 = 0x138D;
    pub const SQUARE_C922: u16 = 0x138F;
    /// The plan's seed kept (8B98) and the map's segment (D806).
    pub const SEED: u16 = 0x8B98;
    pub const MAP: u16 = 0xD806;
    /// Where a block's header is read to, and the terrain map's name and
    /// the screen's file (3DF2).
    pub const HEADER: u16 = 0x929E;
    pub const TERRAIN_NAME: u16 = 0x47B2;
    pub const SCREEN_NAME: u16 = 0x47BE;
    /// The location's world square (3D91).
    pub const WORLD: u16 = 0x47DD;
    /// The terrains' map bytes (seven, 1194:32BB).
    pub const TERRAINS: u16 = 0x1920;
}
use var::*;

/// The re-rolls a list entry may take before the port gives up (where
/// the original would roll on until the RNG gives a pair not yet used).
const ROLL_LIMIT: u32 = 1 << 20;

impl Machine {
    fn ci8(&self, at: u16) -> u8 {
        self.mem.r8(DS, at)
    }

    fn ci16(&self, at: u16) -> u16 {
        self.mem.r16(DS, at)
    }

    /// 1194:3EBF (far) / 3EC3 with AX `ax`: the buildings of place AL
    /// listed (see the module's notes). AH matters only when another
    /// place's record is built (AX 100h is the Imperial City). None where
    /// the original faults (no blocks, a division by 0), hangs (a list
    /// with every pair used), the heap is full or a file is missing.
    pub fn city_info(&mut self, files: &mut Files, ax: u16) -> Option<()> {
        let saved = self.rng.seed;
        let (seg, rec) = if ax as u8 != self.ci8(PLACE) {
            // 0000:9CD2: last fit, AX 0. With no room the record would be
            // built at 0:0, in low memory: not followed.
            let seg = self.heap_alloc_high(0x38)?;
            if seg == 0 {
                return None;
            }
            self.city_location(files, seg, 0, ax)?;
            (seg, 0)
        } else {
            let p = self.ci8(RECORD + record::PROVINCE);
            self.city_load_province(files, p)?;
            (DS, RECORD)
        };
        self.city_plan_for_record(seg, rec)?;
        // dim rows of dim cells (a plan of 0 cells has already faulted).
        let dim = self.ci16(0x113B) as u32;
        for i in 0..dim * dim {
            let cell = self.ci8(crate::city::var::PLAN.wrapping_add(i as u16));
            self.city_info_block(files, cell)?;
        }
        let kept = [self.ci16(TAVERNS), self.ci16(STORES), self.ci16(TEMPLES)];
        for (i, v) in kept.into_iter().enumerate() {
            self.mem.w16(DS, KEPT + 2 * i as u16, v);
        }
        let plan_seed = self.mem.r32(DS, SEED);
        let n = self.city_list(KIND_TAVERN, LIST_TAVERNS, 0x17, 0x17, false)?;
        self.cap(TAVERNS, n);
        self.rng.seed = plan_seed;
        let n = self.city_list(KIND_STORE, LIST_STORES, 0x14, 0x0A, false)?;
        self.cap(STORES, n);
        self.rng.seed = plan_seed;
        let n = self.city_list(KIND_TEMPLE, LIST_TEMPLES, 3, 2, true)?;
        self.cap(TEMPLES, n);
        let palace = self.city_square(KIND_PALACE);
        self.mem.w16(DS, PALACE, palace);
        let other = self.city_square(KIND_C922);
        self.mem.w16(DS, SQUARE_C922, other);
        let kind = self.mem.r8(seg, rec.wrapping_add(record::ID));
        let pair = match kind {
            2 => Some(0x0701),
            0xE0 => Some(0x0802),
            _ => None,
        };
        if let Some(v) = pair {
            let (off, s) = (self.ci16(LIST_TEMPLES), self.ci16(LIST_TEMPLES + 2));
            self.mem.w16(s, off.wrapping_add(2), v);
        }
        if rec != RECORD {
            if self.ci8(PLACE) == kind {
                let k = [self.ci16(KEPT), self.ci16(KEPT + 2), self.ci16(KEPT + 4)];
                self.mem.w16(DS, TAVERNS, k[0]);
                self.mem.w16(DS, STORES, k[1]);
                self.mem.w16(DS, TEMPLES, k[2]);
            }
            self.heap_free(seg);
        }
        self.rng.seed = saved;
        Some(())
    }

    /// `jge`: the count down to `n` when `n` is less, both signed.
    fn cap(&mut self, at: u16, n: u16) {
        if (n as i16) < (self.ci16(at) as i16) {
            self.mem.w16(DS, at, n);
        }
    }

    /// 1194:4189: a cell of the plan (2 and up a block) adds its .MIF
    /// header's counts: its name made as the city's are (`city_block_name`,
    /// two rolls), the file opened, 6 bytes skipped, 3Dh read to HEADER.
    fn city_info_block(&mut self, files: &mut Files, cell: u8) -> Option<()> {
        let Some(name) = self.city_block_name(cell)? else { return Some(()) };
        self.file_open(files, name)?;
        self.file_seek(files, 1, 6);
        self.file_read(files, DS, HEADER, 0x3D);
        for (at, off) in [(TAVERNS, 0x1B), (STORES, 0x1D), (TEMPLES, 0x1F)] {
            let v = self.ci16(at).wrapping_add(self.ci16(HEADER + off));
            self.mem.w16(DS, at, v);
        }
        self.file_close(files);
        Some(())
    }

    /// `repne scasw` over the map's 4000h words for the flat kind at
    /// `kind_at` twice: the square of the first (its row high, its column
    /// low: the word offset with its low byte halved); with none, the
    /// offset past the map's end, 7F7Fh.
    fn city_square(&self, kind_at: u16) -> u16 {
        let map = self.ci16(MAP);
        let word = self.ci8(kind_at) as u16 * 0x101;
        let mut di = 0u16;
        for _ in 0..0x4000 {
            let w = self.mem.r16(map, di);
            di = di.wrapping_add(2);
            if w == word {
                break;
            }
        }
        let pos = di.wrapping_sub(2);
        pos & 0xFF00 | ((pos as u8) >> 1) as u16
    }

    /// 1194:40CF. The list at the far pointer at `list_at` made from the
    /// map's squares of the flat kind at `kind_at` (see the module's
    /// notes): `bl` and `dl` the two dice, `third` the temples' rule for
    /// the second. Returns the squares found; the RNG is put back. None
    /// where every pair the dice can give is already in the list (the
    /// original rolls forever) or the re-rolls pass ROLL_LIMIT.
    fn city_list(&mut self, kind_at: u16, list_at: u16, bl: u8, dl: u8, third: bool) -> Option<u16> {
        let saved = self.rng.seed;
        let (start, gs) = (self.ci16(list_at), self.ci16(list_at + 2));
        let map = self.ci16(MAP);
        let word = self.ci8(kind_at) as u16 * 0x101;
        let mut d = dl;
        let (mut si, mut count, mut found) = (start, 0u16, 0u16);
        let (mut di, mut cx) = (0u16, 0x4000u16);
        loop {
            let mut hit = false;
            while cx != 0 {
                cx -= 1;
                let w = self.mem.r16(map, di);
                di = di.wrapping_add(2);
                if w == word {
                    hit = true;
                    break;
                }
            }
            if !hit {
                break;
            }
            found = found.wrapping_add(1);
            let pos = di.wrapping_sub(2);
            self.mem.w16(gs, si, pos & 0xFF00 | ((pos as u8) >> 1) as u16);
            si = si.wrapping_add(2);
            if count != 0 && self.city_list_full(gs, start, count, bl, d, third) {
                return None;
            }
            let mut rolls = 0u32;
            loop {
                let r1 = self.rng.range(0, bl as u16)? as u8;
                self.mem.w8(gs, si, r1);
                si = si.wrapping_add(1);
                if third {
                    match r1 {
                        0 => d = 5,
                        1 => d = 9,
                        2 => d = 10,
                        _ => {}
                    }
                }
                let r2 = self.rng.range(0, d as u16)? as u8;
                self.mem.w8(gs, si, r2);
                if count != 0 {
                    let w = self.mem.r16(gs, si.wrapping_sub(1));
                    let used = (0..count).any(|k| self.mem.r16(gs, start.wrapping_add(4 * k).wrapping_add(2)) == w);
                    if used {
                        si = si.wrapping_sub(1);
                        rolls += 1;
                        if rolls > ROLL_LIMIT {
                            return None;
                        }
                        continue;
                    }
                }
                break;
            }
            count = count.wrapping_add(1);
            si = si.wrapping_add(1);
        }
        self.rng.seed = saved;
        Some(found)
    }

    /// Whether every pair the dice can give is among the `count` entries
    /// from `start`: the first die 0..`bl`, the second 0..`d` (for the
    /// temples 5, 9, 10 after a first of 0, 1, 2, else any `d` it may
    /// still hold).
    fn city_list_full(&self, gs: u16, start: u16, count: u16, bl: u8, d: u8, third: bool) -> bool {
        let used = |a: u8, b: u8| (0..count).any(|k| self.mem.r16(gs, start.wrapping_add(4 * k).wrapping_add(2)) == (a as u16 | (b as u16) << 8));
        (0..bl).all(|a| {
            let dice: Vec<u8> = match (third, a) {
                (true, 0) => vec![5],
                (true, 1) => vec![9],
                (true, 2) => vec![10],
                (true, _) => vec![d, 5, 9, 10],
                _ => vec![d],
            };
            dice.into_iter().all(|n| (0..n).all(|b| used(a, b)))
        })
    }

    /// 1194:3B84 in full for the record at seg:rec and AX `ax`: the
    /// province loaded as 3C0D loads it (8 while the current place's is,
    /// else the id's), the record built (`city_build_location_with`) with
    /// the terrain's climate (`city_climate`, which loads the record's
    /// province again), and that province loaded twice more: by the map
    /// row's 1194:3183 (3E9E at 3EA0) and the closing one (3C99). The
    /// ported `city_map_y` and `city_local_to_global` leave those loads
    /// out; the data loaded is the same, only the files' state and the
    /// path buffers' toggle (1AEA) see them.
    pub fn city_location(&mut self, files: &mut Files, seg: u16, rec: u16, ax: u16) -> Option<()> {
        let id = if ax == 0x100 { 0 } else { ax as u8 };
        let p1 = if self.ci8(crate::city::var::PROVINCE) == 8 { 8 } else { id >> 5 };
        self.city_load_province(files, p1)?;
        self.city_build_location_with(seg, rec, ax, &mut |m: &mut Machine| {
            let x = m.mem.r16(seg, rec.wrapping_add(record::X));
            let y = m.mem.r16(seg, rec.wrapping_add(record::Y));
            let p = m.mem.r8(seg, rec.wrapping_add(record::PROVINCE));
            m.city_climate(files, x, y, p)
        })?;
        let p = self.mem.r8(seg, rec.wrapping_add(record::PROVINCE));
        self.city_load_province(files, p)?;
        self.city_load_province(files, p)?;
        Some(())
    }

    /// 1194:3D91. The climate of the location at (`x`, `y`) of province
    /// `province`: the province loaded and the place made a world square
    /// (1194:3183, kept at WORLD), then the byte at that square's offset
    /// (y * 140h + x, 16 bits) of TERRAIN.IMG read, the image's 12-byte
    /// header included, made a terrain (`terrain_of`); for 6 the nearest
    /// other terrain (3DF2).
    fn city_climate(&mut self, files: &mut Files, x: u16, y: u16, province: u8) -> Option<u8> {
        self.city_load_province(files, province)?;
        let (gx, gy) = self.city_local_to_global(x, y)?;
        self.mem.w16(DS, WORLD, gx);
        self.mem.w16(DS, WORLD + 2, gy);
        let off = gy.wrapping_mul(0x140).wrapping_add(gx);
        self.file_open(files, TERRAIN_NAME)?;
        self.file_seek(files, 0, off as u32);
        self.file_read(files, DS, HEADER, 1);
        self.file_close(files);
        let t = self.terrain_of(self.ci8(HEADER));
        if t != 6 {
            return Some(t);
        }
        self.city_climate_shore(files, off, t)
    }

    /// 1194:32BB: the index of `b` among the seven terrain bytes, 7 when
    /// none.
    fn terrain_of(&self, b: u8) -> u8 {
        (0..7).find(|&i| self.ci8(TERRAINS + i) == b).unwrap_or(7) as u8
    }

    /// 1194:3DF2. Over terrain 6 (water): bank [1030] mapped, the frame's
    /// 64000 bytes kept in TMP2.$$$, TERRAIN.IMG loaded to the frame and
    /// searched outwards from `off`, a row (140h) below and above, then a
    /// byte right and left, one step further each time, for the first
    /// byte not the water's (1926); its terrain is the climate. The frame
    /// is read back from TMP2.$$$ (TERRAIN.IMG's last 12 bytes stay past
    /// it) and the bank put back. AX `ax` is 5C86's when EMS is off.
    fn city_climate_shore(&mut self, files: &mut Files, off: u16, ax: u8) -> Option<u8> {
        let bank = self.ci16(0x1030);
        let old = self.map_bank(bank, ax as u16);
        let frame = self.ci16(0x9030);
        let path = self.save_path(SCREEN_NAME);
        self.file_write(files, path, frame, 0, 0xFA00);
        self.file_load(files, TERRAIN_NAME, frame, 0)?;
        let water = self.ci8(TERRAINS + 6);
        let at = |m: &Machine, d: u16| m.mem.r8(frame, off.wrapping_add(d));
        let (mut di, mut cx) = (0x140u16, 1u16);
        let found = loop {
            let bx = [di, di.wrapping_neg(), cx, cx.wrapping_neg()].into_iter().find(|&d| at(self, d) != water);
            if let Some(d) = bx {
                break at(self, d);
            }
            di = di.wrapping_add(0x140);
            cx = cx.wrapping_add(1);
            if cx == 0 {
                // Every offset of the frame is water: the original
                // searches forever.
                return None;
            }
        };
        let t = self.terrain_of(found);
        let path = self.save_path(SCREEN_NAME);
        self.file_load(files, path, frame, 0)?;
        self.map_bank(old, t as u16);
        Some(t)
    }

    /// 0000:46B2..4766, the outdoor part of entering an area (the hook
    /// CD42, 0000:468B, after its light, 0390): outside the wilderness
    /// (C39A bit 200h), the place's buildings listed (3EBF with AX the
    /// place, `QuestHooks::city_info`); unless just loaded (5CA bit
    /// 4000h) its quests made (1194:9358, `quests_place`) and the town's
    /// music started (2294:04C0, `QuestHooks::music`). Then each of the
    /// four quests under way (+4 bits 6-7 01b) whose place (+24h) is this
    /// one has its goal (+14h) found again in the new lists by its sort
    /// (+4 bits 12-15, 1194:9B66): a tavern's (0), a temple's (3) or a
    /// store's (1) entry whose pair (+2) is the quest's +16h, its
    /// square; none found leaves the list's first entry's dword (the
    /// original loads it once, before the search); none (6) AAAAh; the
    /// palace (2) its square (138D). A count of 0 searches 65536 entries.
    pub fn city_entered(&mut self, files: &mut Files, hooks: &mut dyn crate::quest::QuestHooks) -> Option<()> {
        if self.ci16(0xC39A) & 0x200 != 0 {
            return Some(());
        }
        hooks.city_info(self, files, self.ci8(PLACE) as u16)?;
        if self.ci16(0x05CA) & 0x4000 == 0 {
            self.quests_place(files, hooks)?;
            hooks.music(self);
        }
        for k in 0..4u16 {
            let si = crate::quest::var::QUESTS + k * 0x34;
            if self.ci16(si + 4) & 0xC0 != 0x40 || self.ci8(si + 0x24) != self.ci8(PLACE) {
                continue;
            }
            let (count, list) = match self.ci16(si + 4) >> 12 {
                0 => (self.ci16(TAVERNS), LIST_TAVERNS),
                3 => (self.ci16(TEMPLES), LIST_TEMPLES),
                1 => (self.ci16(STORES), LIST_STORES),
                6 => {
                    self.mem.w16(DS, si + 0x14, 0xAAAA);
                    continue;
                }
                2 => {
                    let v = self.ci16(PALACE);
                    self.mem.w16(DS, si + 0x14, v);
                    continue;
                }
                _ => continue,
            };
            let (mut di, gs) = (self.ci16(list), self.ci16(list + 2));
            let want = self.ci16(si + 0x16);
            let first = self.mem.r32(gs, di);
            let n = if count == 0 { 0x10000 } else { count as u32 };
            let mut found = None;
            for _ in 0..n {
                if self.mem.r16(gs, di.wrapping_add(2)) == want {
                    found = Some(self.mem.r16(gs, di));
                    break;
                }
                di = di.wrapping_add(4);
            }
            match found {
                Some(v) => self.mem.w16(DS, si + 0x14, v),
                None => self.mem.w32(DS, si + 0x14, first),
            }
        }
        Some(())
    }
}
