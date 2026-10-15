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
//! front end's: the decision and the volume are the game's. The loading
//! is ported (`sound_load`) but for the driver's own part, its call 85h
//! readying each sample (AIL's VOC formatting), which is the front
//! end's.
//!
//! The original drops a sound asked for while one is playing (the
//! driver's status 2); the front end says whether one is, as the mouse
//! driver's state is kept (`Machine::sound_busy`). Each sound played is
//! queued in `Machine::sounds` for the front end to take.

use crate::bsa::Files;
use crate::level::var::BANK_FREE;
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
    /// The level's sound slots: 8 bytes each (the EMS bank, FFFFh none;
    /// the sample's far pointer; its length), 64 of them, the 33rd
    /// (FILE_SLOT) the one 23BA:012B plays files from.
    pub const RECORDS: u16 = 0x44F0;
    pub const FILE_SLOT: u16 = 0x45F0;
    /// Where the loader keeps FILE_SLOT while the slots are cleared.
    pub const FILE_SLOT_KEPT: u16 = 0x1044;
    /// The EMS banks the sounds go in: the first and how many.
    pub const BANKS: u16 = 0xC8FC;
    /// The sound driver's buffer (far pointer) and its handle.
    pub const BUFFER: u16 = 0xEB89;
    pub const DRIVER: u16 = 0xEB91;
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
            return crate::giveup::original("23BA:00E8: the volume's divide overflows");
        }
        Some(q as u8)
    }

    /// 23BA:0217, at the hook CD4A after an INF's @SOUND section: with
    /// sound on (ENABLED), the sound files named at DS:`si` (names each
    /// after the last's NUL, ended by an empty one: the INF's LIST) loaded
    /// into the slots at RECORDS, in order. The bank table is kept at
    /// SAVED_FREE and every bank marked full but the sounds' (BANKS:
    /// 0000:FEF4); those, and banks 13h..15h, are then marked empty. The
    /// slots are cleared but FILE_SLOT (kept at FILE_SLOT_KEPT meanwhile).
    /// Each file is read into the driver's buffer (BUFFER, 0000:7428),
    /// given room in EMS (0000:5C22: its bank, FFFFh where there is none,
    /// and its far pointer, 0 then, into its slot), and copied there
    /// (0000:7702) with its length; a file not loaded leaves its slot
    /// clear. The table is then put back but for the sounds' banks
    /// (0000:FECE): banks 13h..15h get their old counts back.
    ///
    /// With no sound banks (C8FD 0) the original's LOOP runs 65536 times,
    /// writing FFFFh over all of DGROUP: None, as the original's fault.
    /// None also where a hook it would call is set (the open, error and
    /// out-of-EMS hooks), or the buffer is one 7428 allocates (an offset
    /// of FFFDh..FFFFh).
    pub fn sound_load(&mut self, files: &mut Files, si: u16) -> Option<()> {
        if self.mem.r16(DS, ENABLED) == 0 {
            return Some(());
        }
        let first = self.mem.r8(DS, BANKS) as u16;
        let count = self.mem.r8(DS, BANKS + 1) as u16;
        if count == 0 {
            return crate::giveup::original("23BA:025A: no sound banks: the LOOP writes FFFFh over all of DGROUP");
        }
        let saved = crate::info::var::SAVED_FREE;
        let (lo, hi) = (first.wrapping_add(first), first.wrapping_add(first).wrapping_add(count.wrapping_add(count)));
        let outside = |bx: u16| bx < lo || bx >= hi;
        for i in 0..0x20u16 {
            let w = self.mem.r16(DS, BANK_FREE + 2 * i);
            self.mem.w16(DS, saved + 2 * i, w);
        }
        for bx in (0..0x40u16).step_by(2) {
            if outside(bx) {
                self.mem.w16(DS, BANK_FREE + bx, 0);
            }
        }
        for bank in 0x13..0x16u16 {
            self.mem.w16(DS, BANK_FREE + 2 * bank, 0xFFFF);
        }
        for i in 0..count {
            self.mem.w16(DS, BANK_FREE.wrapping_add(lo).wrapping_add(2 * i), 0xFFFF);
        }
        let kept: Vec<u8> = (0..8).map(|i| self.mem.r8(DS, FILE_SLOT + i)).collect();
        for (i, &b) in kept.iter().enumerate() {
            self.mem.w8(DS, FILE_SLOT_KEPT + i as u16, b);
        }
        for i in 0..0x200u16 {
            self.mem.w8(DS, RECORDS + i, 0);
        }
        for (i, &b) in kept.iter().enumerate() {
            self.mem.w8(DS, FILE_SLOT + i as u16, b);
        }
        let mut si = si;
        let mut di = RECORDS;
        for _ in 0..0x10000u32 {
            if self.mem.r8(DS, si) == 0 {
                break;
            }
            self.sound_load_one(files, si, di)?;
            di = di.wrapping_add(8);
            let len = self.mem.cstr(DS, si).len() as u16;
            si = si.wrapping_add(len).wrapping_add(1);
        }
        for bx in (0..0x40u16).step_by(2) {
            if outside(bx) {
                let w = self.mem.r16(DS, saved + bx);
                self.mem.w16(DS, BANK_FREE + bx, w);
            }
        }
        Some(())
    }

    /// One file of `sound_load`: the name at DS:`si`, its slot at `di`.
    fn sound_load_one(&mut self, files: &mut Files, si: u16, di: u16) -> Option<()> {
        let (off, seg) = (self.mem.r16(DS, BUFFER), self.mem.r16(DS, BUFFER + 2));
        if off >= 0xFFFD {
            return crate::giveup::not_ported("0000:7428: the sound buffer one it allocates");
        }
        if self.file_load_or_carry(files, si, seg, off)?.is_none() {
            return Some(());
        }
        let size = self.mem.r16(DS, crate::bsa::var::OPEN_SIZE);
        let (at, bank) = match self.ems_alloc(size) {
            Some((at, bank)) => (((self.mem.r16(DS, crate::mif::var::BUFFER_SEG) as u32) << 16) | at as u32, bank),
            None => {
                if self.mem.r32(DS, crate::heap::var::ERROR_HOOK) != 0 {
                    return crate::giveup::not_ported("0000:5C26: out of EMS, the hook at 8F70 set");
                }
                (0, 0xFFFF)
            }
        };
        self.mem.w32(DS, di.wrapping_add(2), at);
        self.mem.w16(DS, di, bank);
        if bank == 0xFFFF {
            return Some(());
        }
        self.mem.w16(DS, di.wrapping_add(6), size);
        let (dseg, mut doff) = ((at >> 16) as u16, at as u16);
        let mut soff = off;
        for _ in 0..size {
            let b = self.mem.r8(seg, soff);
            self.mem.w8(dseg, doff, b);
            soff = soff.wrapping_add(1);
            doff = doff.wrapping_add(1);
        }
        // FFD:0D4C (AIL 85h, the sample readied by the driver): the
        // front end's.
        Some(())
    }
}
