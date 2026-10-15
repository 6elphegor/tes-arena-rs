//! Sounds (segment 23BA): which sound is played, and how loud.
//!
//! The game plays one sound at a time, by number, at a map square:
//! `sound_at` (23BA:002A, the routine at the hook CD46 too). The number is
//! the level's: its INF's @SOUND section names the level's sound files
//! and gives each a number (`info_sounds`: the names into the list at
//! 929E in order, SOUNDS[number] = its place in that list). 23BA:0217, at
//! the hook CD4A, then loads those files into the slots at 44F0 (8 bytes
//! each: the EMS handle, the sample's far pointer, its length); that, and
//! the playing itself through the sound driver (segment 0FFD), are the
//! front end's: the decision and the volume are the game's.
//!
//! The original drops a sound asked for while one is playing (the
//! driver's status 2); the front end says whether one is, as the mouse
//! driver's state is kept (`Machine::sound_busy`). Each sound played is
//! queued in `Machine::sounds` for the front end to take.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Sound on (the driver loaded; 0 none).
    pub const ENABLED: u16 = 0x8F64;
    /// Each sound number's slot in the level's list (FFh none): the INF
    /// loader's SOUNDS.
    pub const SLOTS: u16 = crate::info::var::SOUNDS;
    /// Each sound number's priority byte, kept at PRIORITY when played.
    pub const PRIORITIES: u16 = 0x46F4;
    pub const PRIORITY: u16 = 0x43D6;
    /// The square the last sound was asked for at (CB90's form: row high,
    /// column doubled low).
    pub const AT: u16 = 0x126E;
    /// The player's square, and the sound volume (percent).
    pub const PLAYER_AT: u16 = 0xCB90;
    pub const VOLUME: u16 = 0x0D3B;
    /// The footsteps' mutes: 5E3 set, or 5BE bit 80h; in water (C39A bit
    /// 10h) the footsteps are the swimming sound.
    pub const MUTE: u16 = 0x05E3;
    pub const SPELLS: u16 = 0x05BE;
}
use var::*;

/// The swimming sound, played for footsteps in water.
pub const SWIMMING: u16 = 0x16;

/// A sound played: its number (after the footsteps' swap), its slot in
/// the level's list (the INF's sound names in order), its priority byte,
/// and the volume (0..7Fh scaled by the volume percent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sound {
    pub number: u16,
    pub slot: u8,
    pub priority: u8,
    pub volume: u8,
}

/// What the front end is asked to play: a level's sound by number, or a
/// file by name (23BA:012B: FANFARE1.VOC, HALT.VOC, ...; it stops a sound
/// playing to play).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Played {
    Number(Sound),
    File(Vec<u8>),
}

impl Machine {
    /// 23BA:012B. The sound file named at DS:`si` played (with sound on):
    /// loaded into the driver's buffer and played over any playing.
    pub fn sound_file(&mut self, si: u16) {
        if self.mem.r16(DS, ENABLED) == 0 {
            return;
        }
        let name = self.mem.cstr(DS, si).to_vec();
        self.sounds.push(Played::File(name));
    }

    /// 23BA:002A. Sound `ax` at square `cx`: nothing with sound off, for
    /// FFh, or for a footstep (0, 1) when they are muted; a footstep in
    /// water is the swimming sound. The square is kept (AT); a number
    /// without a slot (FFh) plays nothing. With the driver idle the
    /// sound's priority is kept (PRIORITY) and it is played at the volume
    /// of 23BA:00E8 (queued in `sounds`). None where the volume's divide
    /// overflows (a volume percent past 100h / 1.27), as the original
    /// faults.
    ///
    /// The slot's record at 44F0 (its handle FFFFh: not loaded) and the
    /// driver's calls are the front end's: here the record is taken as
    /// loaded.
    pub fn sound_at(&mut self, ax: u16, cx: u16) -> Option<()> {
        if self.mem.r16(DS, ENABLED) == 0 || ax as u8 == 0xFF {
            return Some(());
        }
        let mut ax = ax;
        if ax <= 1 {
            if self.mem.r8(DS, MUTE) != 0 || self.mem.r16(DS, SPELLS) & 0x80 != 0 {
                return Some(());
            }
            if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x10 != 0 {
                ax = SWIMMING;
            }
        }
        self.mem.w16(DS, AT, cx);
        let priority = self.mem.r8(DS, PRIORITIES.wrapping_add(ax));
        let slot = self.mem.r8(DS, SLOTS.wrapping_add(ax));
        if slot == 0xFF || self.sound_busy {
            return Some(());
        }
        self.mem.w8(DS, PRIORITY, priority);
        let volume = self.sound_volume()?;
        self.sounds.push(Played::Number(Sound { number: ax, slot, priority, volume }));
        Some(())
    }

    /// `sound_at` at the player's square (CB90), as most callers ask.
    pub fn sound_here(&mut self, ax: u16) -> Option<()> {
        let at = self.mem.r16(DS, PLAYER_AT);
        self.sound_at(ax, at)
    }

    /// 23BA:00E8. The volume for the square at AT: 7Fh at the player's;
    /// else 7Fh less 8 for each step of the distance in CB90's bytes (the
    /// larger of the two byte differences, signed 8-bit, plus a quarter of
    /// the smaller), 14h if that goes below 0; then that times the volume
    /// percent, over 100 (a byte divide: None where it overflows).
    pub fn sound_volume(&self) -> Option<u8> {
        let at = self.mem.r16(DS, AT);
        let me = self.mem.r16(DS, PLAYER_AT);
        let mut cx: u16 = 0x7F;
        if at != me {
            let abs8 = |v: u8| if (v as i8) < 0 { v.wrapping_neg() } else { v };
            let mut al = abs8((at as u8).wrapping_sub(me as u8));
            let mut ah = abs8(((at >> 8) as u8).wrapping_sub((me >> 8) as u8));
            if al > ah {
                std::mem::swap(&mut al, &mut ah);
            }
            let al = (al >> 2).wrapping_add(ah);
            let ax = (al as u16) << 3;
            cx = 0x7Fu16.wrapping_sub(ax);
            if (cx as i16) < 0 {
                cx = 0x14;
            }
        }
        let product = (cx as u8 as u16) * self.mem.r8(DS, VOLUME) as u16;
        let q = product / 100;
        if q > 0xFF {
            return None;
        }
        Some(q as u8)
    }
}
