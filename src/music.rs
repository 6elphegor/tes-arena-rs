//! Music: which song plays when (segment 2294's pickers).
//!
//! The game plays one XMIDI song at a time through the Miles AIL driver
//! (segment 2F9F wraps it: 001F starts the song named at DS:SI, 0438 fades
//! and stops the one playing, 000D asks how it is doing). What plays is
//! decided here:
//!
//! - entering a town (2294:04C0, `town_music`, called from 0000:46D3 as
//!   `city_entered` ends): the town's own song by its size, city.xmi,
//!   town.xmi or village.xmi (arabcity/arabtown/arab_vlg in the desert),
//!   played once;
//! - each frame (2294:04F5 from 0000:48FE, `area_music`): outdoors, when the
//!   town's song is over, the weather's song (swimming in water, night
//!   before 10Ch and from 2F4h, then sunnyday, overcast, raining, oversnow
//!   or snowing by the weather); in a building the building's song (0FF0,
//!   set by its trigger: palace, temple, tavern...); in a dungeon, once per
//!   dungeon (its seed BB0A against 427D), dungeon1..5 picked at random
//!   (dungeon1 only with the FM driver, 1010);
//! - 2294:10B6 (`music_play`) plays a song unless no music driver is
//!   loaded (8F60), it is the one playing (0D56), or the combat song's
//!   30-frame hold is on (430E, set when the combat song starts: nothing in
//!   this version asks for combat.xmi, so it stays 0).
//!
//! A song is named .xmi; with an FM driver (its timbre files' extension
//! "AD", 2F9F:035B sets 1010) the file opened is the .xfm arrangement
//! (0000:11DA, `fm_song_name`).
//!
//! The driver is the front end's: the songs asked for are recorded on the
//! machine (`Machine::songs`), and the front end says how the playing one
//! is doing (`Machine::music_status`: AIL's 0 stopped, 1 playing, 2 done).

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// The song playing (its name's offset; FFFFh none).
    pub const CURRENT: u16 = 0x0D56;
    /// The music driver, loaded (its far pointer, 8F5E:8F60).
    pub const DRIVER: u16 = 0x8F5E;
    /// The town's song is playing (outdoors waits for it to end).
    pub const TOWN_SONG: u16 = 0x430F;
    /// The combat song's hold (frames).
    pub const COMBAT_HOLD: u16 = 0x430E;
    /// The dungeon whose song was picked (its seed's byte), and the digit
    /// of "dungeon1.xmi" (4285).
    pub const DUNGEON_PICKED: u16 = 0x427D;
    pub const DUNGEON_DIGIT: u16 = 0x4285;
    /// The FM driver (.xfm songs, one dungeon song).
    pub const FM: u16 = 0x1010;
    /// A hostile creature near (cleared each frame here).
    pub const NEAR: u16 = 0x101B;
    /// The building's song (set by its trigger).
    pub const BUILDING_SONG: u16 = 0x0FF0;
    /// The renamed file name (0000:11DA writes it here).
    pub const FM_NAME: u16 = 0x4DF8;
    /// Songs: the towns' (city, town, village, arabcity, arabtown,
    /// arab_vlg), and the outdoor ones.
    pub const TOWN_SONGS: u16 = 0x4310;
    pub const SWIMMING: u16 = 0x4265;
    pub const NIGHT: u16 = 0x421C;
    pub const SUNNYDAY: u16 = 0x4226;
    pub const OVERCAST: u16 = 0x4233;
    pub const RAINING: u16 = 0x4240;
    pub const OVERSNOW: u16 = 0x424C;
    pub const SNOWING: u16 = 0x4259;
    pub const COMBAT: u16 = 0x4272;
    pub const DUNGEON: u16 = 0x427E;
    /// Songs played straight through 2F9F:001F: the opening (0000:124E,
    /// 132B), the character sheet (1194:249B), the vision (0000:0D1B,
    /// 113E, 1194:D3D1..), the ending (1194:D9A8..).
    pub const PERCNTRO: u16 = 0x4D7A;
    pub const EVLINTRO: u16 = 0x4D87;
    pub const SHEET: u16 = 0x3ECF;
    pub const VISION: u16 = 0x809A;
    pub const WINGAME: u16 = 0x80BD;
}
use var::*;

/// What the music driver was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Song {
    /// 2F9F:0438: the song playing faded out and stopped.
    Stop,
    /// 2F9F:001F: the song named at DS:`at` started; `file` the file the
    /// driver opens (after `fm_song_name`).
    Play { at: u16, file: Vec<u8> },
}

impl Machine {
    /// 2F9F:0438.
    pub fn music_stop(&mut self) {
        self.songs.push(Song::Stop);
    }

    /// 2F9F:001F with the name at DS:`si` (the driver's own checks, a
    /// driver loaded and a name, are the front end's).
    pub fn music_start(&mut self, si: u16) {
        let file = self.fm_song_file(si);
        self.songs.push(Song::Play { at: si, file });
    }

    /// The song named at DS:`si` stopped and started again, as the
    /// opening, the sheet, the vision and the ending play theirs (2F9F:0438
    /// then 001F; 0D56 isn't set).
    pub fn music_song(&mut self, si: u16) {
        self.music_stop();
        self.music_start(si);
    }

    /// 1194:DB26's song: after a scene that played its own song, the song
    /// stopped and, in a building (area 4), the building's song started
    /// (25FD:051F), else the song playing before (CURRENT) when there is
    /// one. Memory is left as it is.
    pub fn music_resume(&mut self) {
        self.music_stop();
        if self.mem.r8(DS, crate::trigger::var::AREA) == 4 {
            let si = self.mem.r16(DS, BUILDING_SONG);
            self.music_start(si);
            return;
        }
        let si = self.mem.r16(DS, CURRENT);
        if si != 0 && si != 0xFFFF {
            self.music_start(si);
        }
    }

    /// The file 0000:11DA opens for the name at DS:`si`: the name, with
    /// "xfm" for its extension when it is ".xmi" (the dword at its first
    /// '.') and the FM driver is in use (1010).
    pub fn fm_song_file(&self, si: u16) -> Vec<u8> {
        let name = self.mem.cstr(DS, si).to_vec();
        match name.iter().position(|&c| c == b'.') {
            Some(dot) if self.mem.r8(DS, FM) != 0 && self.mem.r32(DS, si.wrapping_add(dot as u16)) == 0x696D_782E => {
                let mut f = name[..=dot].to_vec();
                f.extend_from_slice(b"xfm");
                f
            }
            _ => name,
        }
    }

    /// 0000:11DA. The file-open name hook: for a name at DS:`si` whose
    /// first '.' starts ".xmi", with the FM driver (1010), the name up to
    /// that '.' and "xfm\0" are copied to FM_NAME, which is returned;
    /// otherwise `si`.
    pub fn fm_song_name(&mut self, si: u16) -> u16 {
        let mut s = si;
        loop {
            let c = self.mem.r8(DS, s);
            if c == 0 {
                return si;
            }
            if c == b'.' {
                break;
            }
            s = s.wrapping_add(1);
        }
        if self.mem.r32(DS, s) != 0x696D_782E || self.mem.r8(DS, FM) == 0 {
            return si;
        }
        let (mut from, mut to) = (si, FM_NAME);
        loop {
            let c = self.mem.r8(DS, from);
            self.mem.w8(DS, to, c);
            from = from.wrapping_add(1);
            to = to.wrapping_add(1);
            if c == b'.' {
                break;
            }
        }
        for (i, &c) in b"xfm\0".iter().enumerate() {
            self.mem.w8(DS, to.wrapping_add(i as u16), c);
        }
        FM_NAME
    }

    /// 2294:04C0. Entering a town (area 0): the song playing stopped, the
    /// town's song marked playing (TOWN_SONG), and the town's song by its
    /// size (0A47: city, town, village; three on in the desert, climate 4)
    /// started.
    pub fn town_music(&mut self) {
        if self.mem.r8(DS, crate::trigger::var::AREA) != 0 {
            return;
        }
        self.music_stop();
        self.mem.w8(DS, TOWN_SONG, 1);
        let mut cx = self.mem.r8(DS, 0x0A47) as u16;
        if self.mem.r16(DS, 0x0DA8) == 4 {
            cx = cx.wrapping_add(3);
        }
        let si = self.skip_strings(DS, TOWN_SONGS, cx);
        self.music_start(si);
    }

    /// 2294:04F5. Each frame's song: outdoors (C398 bit 4) NEAR cleared;
    /// while the town's song plays (TOWN_SONG, the driver's status 1)
    /// nothing, once over the weather's song; indoors a building's song
    /// (area 4: BUILDING_SONG), else (a dungeon) NEAR cleared and, when the
    /// dungeon changed (BB0A against DUNGEON_PICKED), its song.
    pub fn area_music(&mut self) {
        if self.mem.r16(DS, crate::movement::var::MAP_FLAGS) & 4 != 0 {
            self.mem.w8(DS, NEAR, 0);
            if self.mem.r8(DS, TOWN_SONG) != 0 {
                if self.music_status == 1 {
                    return;
                }
                self.mem.w8(DS, TOWN_SONG, 0);
            }
            let t = self.mem.r16(DS, crate::sky::var::TIME);
            let w = self.mem.r8(DS, crate::sky::var::WEATHER);
            let si = if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x10 != 0 {
                SWIMMING
            } else if t <= 0x10C || t >= 0x2F4 {
                NIGHT
            } else {
                match w {
                    0 => SUNNYDAY,
                    1 => OVERCAST,
                    0x81 => RAINING,
                    2 => OVERSNOW,
                    _ => SNOWING,
                }
            };
            self.music_play(si);
            return;
        }
        if self.mem.r8(DS, crate::trigger::var::AREA) == 4 {
            let si = self.mem.r16(DS, BUILDING_SONG);
            self.music_play(si);
            return;
        }
        self.mem.w8(DS, NEAR, 0);
        let al = self.mem.r8(DS, 0xBB0A);
        if al == self.mem.r8(DS, DUNGEON_PICKED) {
            return;
        }
        self.mem.w8(DS, DUNGEON_PICKED, al);
        if self.mem.r8(DS, FM) == 0 {
            let r = self.rng.next() as u8;
            self.mem.w8(DS, DUNGEON_DIGIT, r % 5 + 0x31);
        }
        self.music_play(DUNGEON);
    }

    /// 2294:10B6. The song at DS:`si` played: not without a music driver
    /// (the segment word of DRIVER), when it is the one playing (CURRENT)
    /// or while the combat song holds (COMBAT_HOLD); the combat song starts
    /// a hold of 1Eh. It becomes CURRENT, the one playing is stopped and
    /// it is started.
    pub fn music_play(&mut self, si: u16) {
        if self.mem.r16(DS, DRIVER + 2) == 0 || self.mem.r16(DS, CURRENT) == si || self.mem.r8(DS, COMBAT_HOLD) != 0 {
            return;
        }
        if si == COMBAT {
            self.mem.w8(DS, COMBAT_HOLD, 0x1E);
        }
        self.mem.w16(DS, CURRENT, si);
        self.music_stop();
        self.music_start(si);
    }
}
