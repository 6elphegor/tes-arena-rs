//! The music driver, as the game sees it through the AIL (the Miles
//! library in segment 0FFD, which keeps its own state in its own segment
//! and calls into the driver by function number): segment 2F9F's
//! wrappers, which the game calls for its music (src/music.rs picks the
//! songs).
//!
//! - main (1194:0783): with a name at 37EB (ARENA.BAT's -M), the AIL
//!   started once (07DF: 37EA, as for the sound driver) and the driver
//!   loaded (2F9F:01BF, `music_driver_load`).
//! - 2F9F:01BF: the file loaded where 0000:7412 allocates it (DI FFFFh:
//!   `file_load_alloc`), its far pointer at 8F5E (offset) and 8F60
//!   (segment); registered (0FFD:0AF7), its handle at EB93 (FFFFh: the
//!   AIL shut down, 0821, and nothing more); described (0C1D), the
//!   descriptor's far pointer at EB99, its type (+2) an XMIDI driver's, 3
//!   (else shut down); the command line's I/O port, IRQ and DMA channel
//!   (3864, 3866, 3868, each FFFFh for the driver's own) written over the
//!   descriptor's at +0Ch, +0Eh, +10h; the device detected (0C43; not
//!   found: shut down) and the driver initialised (0C49) with the
//!   descriptor's four words from +0Ch; a sequence's state table's size
//!   asked (0D88, EBA1) and the timbre cache's (0D9A, EB9F), which when
//!   not 0 is taken from the heap (0000:51D3, EBA3) and given (0DA0); the
//!   state table taken (EB85); an FM driver (its timbre files' extension,
//!   the descriptor's +4, "AD") noted at 1010.
//! - 2F9F:001F (`music_start`): the song named at DS:SI read whole into a
//!   heap block (7412; its far pointer at EB8D, 0:0 for none: nothing
//!   more); its timbres' file opened (DOS 3Dh itself: "SAMPLE." at 8F66
//!   with the descriptor's extension); the sequence registered (0D8E,
//!   with the state table; its handle at EB00, FFFFh: shut down); each
//!   timbre it asks for (0DA6, bank << 8 | patch, until FFFFh) read from
//!   the file (1194:4943, `timbre_load`: none there, the file closed and
//!   the AIL shut down) and installed (0DAC), its block given back
//!   (6C26); the file closed; channels 1..16's volume (controller 7) set
//!   to 127 (0E0C); the sequence started (0DC4) at the music's volume
//!   (0DE8, D3A, at once).
//! - 2F9F:0438 (`music_stop`): with a song read (EB8D), its volume to 0
//!   over 800 ms (0DE8), 46h retraces waited (0000:6B98: the front end's),
//!   the sequence stopped (0DCA) and released (0D94), its block given back
//!   and EB8D cleared.
//! - 2F9F:000D (`music_state`): the sequence's status (0DD6: 0 stopped,
//!   1 playing, 2 done).
//!
//! The driver is a replay's (`Replay`, on `Machine::music`): the original
//! driver's answers (harness/gen_musicplay.py's `music` lines) taken in
//! turn, and every call kept in the lines' form. Without one, the music
//! is the front end's (`Machine::songs`, `Machine::music_status`): the
//! game in play never has a music driver loaded in its memory.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;
use crate::music::Song;

/// DGROUP variables.
pub mod var {
    /// The music driver's name (ARENA.BAT's -M), NUL-ended; 0 for none.
    pub const NAME: u16 = 0x37EB;
    /// The command line's I/O port, IRQ and DMA channel for it (FFFFh: the
    /// driver's own).
    pub const SETTINGS: u16 = 0x3864;
    /// The driver's image (a far pointer: offset, then segment; 0 none).
    pub const IMAGE: u16 = 0x8F5E;
    /// Its handle with the AIL.
    pub const HANDLE: u16 = 0xEB93;
    /// Its descriptor (a far pointer into the image).
    pub const DESCRIPTOR: u16 = 0xEB99;
    /// A sequence's state table's size, and the table (a far pointer).
    pub const STATE_SIZE: u16 = 0xEBA1;
    pub const STATE: u16 = 0xEB85;
    /// The timbre cache's size, and the cache (a far pointer).
    pub const CACHE_SIZE: u16 = 0xEB9F;
    pub const CACHE: u16 = 0xEBA3;
    /// The song read (a far pointer to its block; 0:0 none).
    pub const SONG: u16 = 0xEB8D;
    /// The sequence's handle.
    pub const SEQUENCE: u16 = 0xEB00;
    /// The timbres' file's name less its extension ("SAMPLE.").
    pub const TIMBRE_FILE: u16 = 0x8F66;
    /// The music's volume (0..64h).
    pub const VOLUME: u16 = 0x0D3A;
    /// 1194:4943's reads: a directory entry (patch, bank, the timbre's
    /// offset as a dword) and a timbre's size.
    pub const TIMBRE_ENTRY: u16 = 0x929E;
    pub const TIMBRE_SIZE: u16 = 0x939E;
}
use var::*;

/// The music driver replayed: the original driver's answers taken in
/// turn (the calls that read one: AF7, C1D, C43, D88, D9A, D8E, DA6, DD6)
/// and every call the music makes kept in gen_musicplay.py's form
/// (`DA6:0=30`, `DAC:BANK:PATCH:DIGEST`, `E0C:0:1:7:7f`...).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Replay {
    pub answers: std::collections::VecDeque<u32>,
    pub calls: Vec<String>,
}

impl Replay {
    /// A replay of a `music` line's calls (`NAME=ANSWER` its answers, in
    /// hex; `SEG:OFF` a far one's; `CALL*N` a call made N times).
    pub fn of(calls: &str) -> Self {
        let mut answers = std::collections::VecDeque::new();
        for tok in calls.split_whitespace() {
            let (call, times) = match tok.rsplit_once('*') {
                Some((c, n)) if !c.is_empty() && n.chars().all(|d| d.is_ascii_digit()) => (c, n.parse().unwrap_or(1)),
                _ => (tok, 1),
            };
            let Some((_, a)) = call.split_once('=') else { continue };
            let v = a.split(':').fold(0u32, |v, w| v << 16 | u32::from_str_radix(w, 16).unwrap_or(0));
            answers.extend(std::iter::repeat_n(v, times));
        }
        Self { answers, calls: Vec::new() }
    }
}

impl Machine {
    /// Whether a music driver is loaded (2F9F:0027: the far pointer at
    /// 8F5E).
    pub fn music_loaded(&self) -> bool {
        self.mem.r16(DS, IMAGE) | self.mem.r16(DS, IMAGE + 2) != 0
    }

    /// A call into the driver with no answer read: kept with a replay.
    fn music_call(&mut self, call: String) {
        if let Some(r) = &mut self.music {
            r.calls.push(call);
        }
    }

    /// A call whose answer is read: the replay's next (0 with none left),
    /// kept with it.
    fn music_answer(&mut self, call: String) -> u32 {
        match &mut self.music {
            Some(r) => {
                let a = r.answers.pop_front().unwrap_or(0);
                r.calls.push(format!("{call}={a:x}"));
                a
            }
            None => 0,
        }
    }

    /// A far answer (DX:AX, the replay's word pair), as (segment, offset).
    fn music_answer_far(&mut self, call: &str) -> (u16, u16) {
        match &mut self.music {
            Some(r) => {
                let a = r.answers.pop_front().unwrap_or(0);
                r.calls.push(format!("{call}={:x}:{:x}", a >> 16, a & 0xFFFF));
                ((a >> 16) as u16, a as u16)
            }
            None => (0, 0),
        }
    }

    /// The music driven: a driver loaded and its answers replayed.
    fn music_driven(&self) -> bool {
        self.music.is_some() && self.music_loaded()
    }

    /// 1194:0783: with a music driver named (37EB), the AIL started once
    /// (07DF: 37EA) and the driver loaded (2F9F:01BF, `music_driver_load`).
    pub fn music_driver_main(&mut self, files: &mut Files) {
        if self.mem.r8(DS, NAME) == 0 {
            return;
        }
        let started = crate::digital::var::AIL_STARTED;
        if self.mem.r8(DS, started) == 0 {
            self.mem.w8(DS, started, 1);
        }
        self.music_driver_load(files, NAME);
    }

    /// 2F9F:01BF: the music driver named at DS:`name` loaded (see the
    /// module). A file that doesn't load leaves 0:0 at 8F5E (0000:7412's
    /// failure), and nothing more.
    pub fn music_driver_load(&mut self, files: &mut Files, name: u16) {
        let (seg, off) = self.file_load_alloc(files, name).unwrap_or((0, 0));
        self.mem.w16(DS, IMAGE, off);
        self.mem.w16(DS, IMAGE + 2, seg);
        if seg | off == 0 {
            return;
        }
        let handle = self.music_answer(format!("AF7:{seg:x}:{off:x}")) as u16;
        self.mem.w16(DS, HANDLE, handle);
        if handle == 0xFFFF {
            self.music_call("821".into());
            return;
        }
        let (dseg, doff) = self.music_answer_far("C1D");
        self.mem.w16(DS, DESCRIPTOR, doff);
        self.mem.w16(DS, DESCRIPTOR + 2, dseg);
        if self.mem.r16(dseg, doff.wrapping_add(2)) != 3 {
            self.music_call("821".into());
            return;
        }
        for (k, at) in [(0u16, 0xCu16), (2, 0xE), (4, 0x10)] {
            let v = self.mem.r16(DS, SETTINGS + k);
            if v != 0xFFFF {
                self.mem.w16(dseg, doff.wrapping_add(at), v);
            }
        }
        let words = |m: &Machine| [0xCu16, 0xE, 0x10, 0x12].map(|k| m.mem.r16(dseg, doff.wrapping_add(k)));
        let [io, irq, dma, drq] = words(self);
        if self.music_answer(format!("C43:{io:x}:{irq:x}:{dma:x}:{drq:x}")) as u16 == 0 {
            self.music_call("821".into());
            return;
        }
        let [io, irq, dma, drq] = words(self);
        self.music_call(format!("C49:{io:x}:{irq:x}:{dma:x}:{drq:x}"));
        let state = self.music_answer("D88".into()) as u16;
        self.mem.w16(DS, STATE_SIZE, state);
        let cache = self.music_answer("D9A".into()) as u16;
        self.mem.w16(DS, CACHE_SIZE, cache);
        if cache != 0 {
            let (cseg, coff) = self.heap_alloc_bytes(cache).unwrap_or((0, 0));
            self.mem.w16(DS, CACHE, coff);
            self.mem.w16(DS, CACHE + 2, cseg);
            self.music_call(format!("DA0:{coff:x}:{cseg:x}:{cache:x}"));
        }
        let (sseg, soff) = self.heap_alloc_bytes(state).unwrap_or((0, 0));
        self.mem.w16(DS, STATE, soff);
        self.mem.w16(DS, STATE + 2, sseg);
        if self.mem.r8(dseg, doff.wrapping_add(4)) == b'A' && self.mem.r8(dseg, doff.wrapping_add(5)) == b'D' {
            self.mem.w8(DS, crate::music::var::FM, 1);
        }
    }

    /// 2F9F:001F with the name at DS:`si`: the song recorded for the front
    /// end (`songs`, the file after `fm_song_name`) and, with the driver
    /// driven, started as the module has it.
    pub fn music_start(&mut self, files: &mut Files, si: u16) {
        let file = self.fm_song_file(si);
        self.songs.push(Song::Play { at: si, file });
        if self.music_driven() && si != 0xFFFF {
            self.music_start_driven(files, si);
        }
    }

    fn music_start_driven(&mut self, files: &mut Files, si: u16) {
        // (A song that doesn't load leaves 0:0 at EB8D, 7412's failure.)
        let (seg, off) = self.file_load_alloc(files, si).unwrap_or((0, 0));
        self.mem.w16(DS, SONG, off);
        self.mem.w16(DS, SONG + 2, seg);
        if seg | off == 0 {
            return;
        }
        let (dseg, doff) = (self.mem.r16(DS, DESCRIPTOR + 2), self.mem.r16(DS, DESCRIPTOR));
        let mut name = self.mem.cstr(DS, TIMBRE_FILE).to_vec();
        name.extend_from_slice(self.mem.cstr(dseg, doff.wrapping_add(4)));
        let h = self.dos_open_raw(files, &name);
        let (sseg, soff) = (self.mem.r16(DS, STATE + 2), self.mem.r16(DS, STATE));
        let seq = self.music_answer(format!("D8E:{seg:x}:{off:x}:0:{sseg:x}:{soff:x}")) as u16;
        self.mem.w16(DS, SEQUENCE, seq);
        if seq == 0xFFFF {
            self.music_call("821".into());
            self.dos_close_on(files, h);
            return;
        }
        loop {
            let t = self.music_answer(format!("DA6:{seq:x}")) as u16;
            if t == 0xFFFF {
                break;
            }
            let (bank, patch) = (t >> 8, t & 0xFF);
            let (tseg, toff) = self.timbre_load(files, h, bank as u8, patch as u8);
            if tseg | toff == 0 {
                self.dos_close_on(files, h);
                self.music_call("821".into());
                return;
            }
            let n = self.mem.r16(tseg, toff);
            let digest = (0..n).fold(0xcbf2_9ce4_8422_2325u64, |d, i| (d ^ self.mem.r8(tseg, toff.wrapping_add(i)) as u64).wrapping_mul(0x100_0000_01b3));
            self.music_call(format!("DAC:{bank:x}:{patch:x}:{digest:x}"));
            self.heap_free(tseg);
        }
        self.dos_close_on(files, h);
        for ch in 1..=16u16 {
            self.music_call(format!("E0C:{seq:x}:{ch:x}:7:7f"));
        }
        self.music_call(format!("DC4:{seq:x}"));
        let volume = self.mem.r8(DS, VOLUME);
        self.music_call(format!("DE8:{seq:x}:{volume:x}:0"));
    }

    /// 1194:4943: the timbre `bank`:`patch` read from the file on handle
    /// `h` (its directory from the start, entries of patch, bank and the
    /// timbre's offset read to 929E until one's bank is FFh; at the
    /// offset its size, read to 939E, and the timbre itself, its size its
    /// first word, into a heap block of that size, 0000:51D3): the block,
    /// or 0:0 where the directory hasn't it.
    pub fn timbre_load(&mut self, files: &mut Files, h: u16, bank: u8, patch: u8) -> (u16, u16) {
        self.file_seek_on(files, h, 0, 0);
        loop {
            self.file_read_on(files, h, DS, TIMBRE_ENTRY, 6);
            if self.mem.r8(DS, TIMBRE_ENTRY + 1) == 0xFF {
                return (0, 0);
            }
            if self.mem.r8(DS, TIMBRE_ENTRY) == patch && self.mem.r8(DS, TIMBRE_ENTRY + 1) == bank {
                break;
            }
        }
        let at = self.mem.r32(DS, TIMBRE_ENTRY + 2);
        self.file_seek_on(files, h, 0, at);
        self.file_read_on(files, h, DS, TIMBRE_SIZE, 2);
        let size = self.mem.r16(DS, TIMBRE_SIZE);
        let (seg, off) = self.heap_alloc_bytes(size).unwrap_or((0, 0));
        self.mem.w16(seg, off, size);
        self.file_read_on(files, h, seg, off.wrapping_add(2), size.wrapping_sub(2));
        (seg, off)
    }

    /// 2F9F:0438: the song recorded stopped for the front end (`songs`)
    /// and, with the driver driven and a song read, faded, stopped and
    /// released as the module has it.
    pub fn music_stop(&mut self) {
        self.songs.push(Song::Stop);
        if !self.music_driven() || self.mem.r16(DS, SONG) | self.mem.r16(DS, SONG + 2) == 0 {
            return;
        }
        let seq = self.mem.r16(DS, SEQUENCE);
        self.music_call(format!("DE8:{seq:x}:0:320"));
        self.music_call(format!("DCA:{seq:x}"));
        self.music_call(format!("D94:{seq:x}"));
        let seg = self.mem.r16(DS, SONG + 2);
        self.heap_free(seg);
        self.mem.w16(DS, SONG, 0);
        self.mem.w16(DS, SONG + 2, 0);
    }

    /// 2542:07BD/07FF, the options screen's music volume: the sequence's
    /// volume set at once (0FFD:0DE8, `volume`), kept with a replay.
    pub fn music_volume(&mut self, volume: u16) {
        let seq = self.mem.r16(DS, SEQUENCE);
        self.music_call(format!("DE8:{seq:x}:{volume:x}:0"));
    }

    /// 2F9F:000D: how the song playing is doing (AIL's 0 stopped, 1
    /// playing, 2 done): the driver's answer (0DD6) driven, else the
    /// front end's (`music_status`).
    pub fn music_state(&mut self) -> u8 {
        if !self.music.is_some() {
            return self.music_status;
        }
        let seq = self.mem.r16(DS, SEQUENCE);
        self.music_answer(format!("DD6:{seq:x}")) as u8
    }
}
