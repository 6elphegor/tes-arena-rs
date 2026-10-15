//! Music: the game's XMIDI songs played on an emulated FM chip, as the
//! original's Sound Blaster Pro/16 driver (SBP1FM.ADV, the Miles AIL FM
//! driver with timbre files "AD") played them.
//!
//! The engine decides what plays (`arena::music`: the town's song, each
//! frame's song by weather, time and place, the songs of the opening, the
//! sheet, the vision and the ending) and records it on the machine
//! (`Machine::songs`). This is the driver's side:
//!
//! - the song's file (.xfm with this driver, `Machine::fm_song_file`) is
//!   read from the game's files and its XMIDI events parsed (`Xmi`): a
//!   delay is a run of bytes below 80h counted in 120 Hz ticks, a note-on
//!   carries its own duration, FOR/NEXT loops are controllers 116 and 117
//!   (a count, 0 for ever), 114 selects the timbre bank;
//! - the timbres are AIL's Global Timbre Library SAMPLE.AD (`Bank`): (patch,
//!   bank, offset) entries up to bank FFh, each timbre a size word and 12
//!   bytes: a transpose and the OPL2 registers of the modulator (20h, 40h,
//!   60h, 80h, E0h), the feedback/connection (C0h) and the carrier's; bank
//!   127 holds the drums by key, their transpose the note they sound;
//! - the voices are an OPL3 (`nuked-opl3`, Nuked-OPL3's emulation) in
//!   two-operator mode: 18 voices (the Pro's two OPL2s), each note on a
//!   free voice or the oldest's, its pitch from the key, the timbre's
//!   transpose and the channel's bend (2 semitones), its loudness the
//!   timbre's output level scaled by the velocity and the channel's volume
//!   (controller 7), its side by the pan (controller 10), held by the
//!   sustain pedal (64).
//!
//! What follows the original's driver only approximately: its voice
//! allocation and priorities, its velocity and volume curves, its vibrato
//! and its use of the two chips (here a voice sounds on both sides unless
//! panned). The sequence's status (AIL's 1 playing, 2 done) goes back to
//! the engine (`Machine::music_status`), which waits for a town's song to
//! end before the weather's.
//!
//! With `--mute` or `--no-music` nothing plays and no driver is loaded:
//! the engine then sees none (8F60 clear) and the FM flag (1010) stays
//! clear, as with the original's "No Music" (which rolls the dungeon's
//! song at random). With `--sound-log` each song asked for is printed.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use arena::machine::Machine;
use arena::music::Song;

use crate::archive::Archive;

/// The output rate.
pub const RATE: u32 = 22050;
/// XMIDI's tick rate.
const TICKS: u32 = 120;

/// An XMIDI event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    /// A note with its duration in ticks.
    Note { ch: u8, key: u8, vel: u8, dur: u32 },
    Ctrl { ch: u8, c: u8, v: u8 },
    Program { ch: u8, p: u8 },
    /// Pitch bend, 0..3FFFh (2000h centre).
    Bend { ch: u8, v: u16 },
    /// The end of the track.
    End,
}

/// A parsed song: its events with the ticks before each, and the timbres
/// it asks for (TIMB: patch, bank).
#[derive(Clone, Debug, Default)]
pub struct Xmi {
    pub events: Vec<(u32, Ev)>,
    pub timbres: Vec<(u8, u8)>,
}

fn be32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn vlq(d: &[u8], i: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    loop {
        let c = *d.get(*i)?;
        *i += 1;
        v = (v << 7) | (c & 0x7F) as u32;
        if c & 0x80 == 0 {
            return Some(v);
        }
    }
}

impl Xmi {
    /// The first sequence of an XMIDI file (FORM XDIR, CAT XMID, FORM XMID
    /// with TIMB and EVNT). None for a file that isn't one.
    pub fn parse(d: &[u8]) -> Option<Xmi> {
        // The first "FORM" <len> "XMID": its chunks follow.
        let form = (0..d.len().saturating_sub(12)).find(|&i| &d[i..i + 4] == b"FORM" && &d[i + 8..i + 12] == b"XMID")?;
        let mut at = form + 12;
        let mut xmi = Xmi::default();
        let mut evnt = None;
        while at + 8 <= d.len() {
            let tag = &d[at..at + 4];
            let len = be32(d, at + 4)?;
            let body = d.get(at + 8..(at + 8 + len).min(d.len()))?;
            match tag {
                b"TIMB" if body.len() >= 2 => {
                    let n = u16::from_le_bytes([body[0], body[1]]) as usize;
                    for k in 0..n {
                        if let (Some(&p), Some(&b)) = (body.get(2 + 2 * k), body.get(3 + 2 * k)) {
                            xmi.timbres.push((p, b));
                        }
                    }
                }
                b"EVNT" => {
                    evnt = Some(body);
                    break;
                }
                _ => {}
            }
            at += 8 + len + (len & 1);
        }
        let e = evnt?;
        let mut i = 0;
        let mut wait = 0u32;
        while i < e.len() {
            let c = e[i];
            if c < 0x80 {
                wait += c as u32;
                i += 1;
                continue;
            }
            i += 1;
            let ch = c & 0x0F;
            let ev = match c & 0xF0 {
                0x90 => {
                    let (key, vel) = (*e.get(i)?, *e.get(i + 1)?);
                    i += 2;
                    let dur = vlq(e, &mut i)?;
                    Some(Ev::Note { ch, key, vel, dur })
                }
                0x80 | 0xA0 => {
                    i += 2;
                    None
                }
                0xB0 => {
                    let (cc, v) = (*e.get(i)?, *e.get(i + 1)?);
                    i += 2;
                    Some(Ev::Ctrl { ch, c: cc, v })
                }
                0xC0 => {
                    let p = *e.get(i)?;
                    i += 1;
                    Some(Ev::Program { ch, p })
                }
                0xD0 => {
                    i += 1;
                    None
                }
                0xE0 => {
                    let (lo, hi) = (*e.get(i)? as u16, *e.get(i + 1)? as u16);
                    i += 2;
                    Some(Ev::Bend { ch, v: lo | hi << 7 })
                }
                _ => match c {
                    0xFF => {
                        let t = *e.get(i)?;
                        i += 1;
                        let l = vlq(e, &mut i)? as usize;
                        i += l;
                        (t == 0x2F).then_some(Ev::End)
                    }
                    0xF0 | 0xF7 => {
                        let l = vlq(e, &mut i)? as usize;
                        i += l;
                        None
                    }
                    _ => return None,
                },
            };
            if let Some(ev) = ev {
                xmi.events.push((wait, ev));
                wait = 0;
                if ev == Ev::End {
                    break;
                }
            }
        }
        if !matches!(xmi.events.last(), Some((_, Ev::End))) {
            xmi.events.push((wait, Ev::End));
        }
        Some(xmi)
    }

    /// The ticks to the end, loops played once.
    pub fn length(&self) -> u32 {
        self.events.iter().map(|&(w, _)| w).sum()
    }
}

/// A timbre of AIL's FM driver: a transpose and the OPL registers
/// (modulator 20h, 40h, 60h, 80h, E0h; C0h; carrier 20h, 40h, 60h, 80h,
/// E0h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timbre {
    pub transpose: u8,
    pub regs: [u8; 11],
}

/// A Global Timbre Library (SAMPLE.AD) by (patch, bank).
#[derive(Clone, Debug, Default)]
pub struct Bank(pub HashMap<(u8, u8), Timbre>);

impl Bank {
    pub fn parse(d: &[u8]) -> Option<Bank> {
        let mut bank = HashMap::new();
        let mut at = 0;
        loop {
            let (p, b) = (*d.get(at)?, *d.get(at + 1)?);
            if b == 0xFF {
                break;
            }
            let off = u32::from_le_bytes(d.get(at + 2..at + 6)?.try_into().ok()?) as usize;
            let size = u16::from_le_bytes([*d.get(off)?, *d.get(off + 1)?]) as usize;
            if size >= 14 {
                let t = d.get(off + 2..off + 14)?;
                let mut regs = [0; 11];
                regs.copy_from_slice(&t[1..12]);
                bank.insert((p, b), Timbre { transpose: t[0], regs });
            }
            at += 6;
        }
        Some(Bank(bank))
    }
}

/// The operators' register offsets of the nine channels of a register set.
const OPS: [u16; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];
const VOICES: usize = 18;
const PERCUSSION: u8 = 9;

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    on: bool,
    /// Held by the sustain pedal after its note-off.
    held: bool,
    ch: u8,
    /// The key the note was asked with.
    key: u8,
    age: u64,
    timbre: Option<(u8, u8)>,
}

#[derive(Clone, Copy, Debug)]
struct Chan {
    program: u8,
    bank: u8,
    volume: u8,
    pan: u8,
    bend: u16,
    sustain: bool,
}

impl Default for Chan {
    fn default() -> Chan {
        Chan { program: 0, bank: 0, volume: 0x7F, pan: 64, bend: 0x2000, sustain: false }
    }
}

/// The FM driver: the chip, its voices, the MIDI channels' state.
pub struct Synth {
    chip: nuked_opl3::Opl3Chip,
    bank: Bank,
    voices: [Voice; VOICES],
    chans: [Chan; 16],
    /// Each voice's velocity, for volume changes.
    vel: [u8; VOICES],
    age: u64,
}

/// (block, f-number) of a key (fractional, MIDI numbering).
fn fnum(key: f32) -> (u8, u16) {
    let f = 440.0 * 2f32.powf((key - 69.0) / 12.0);
    for block in 0..8u8 {
        let n = f * (1u32 << (20 - block)) as f32 / 49716.0;
        if n < 1024.0 {
            return (block, n.round().min(1023.0) as u16);
        }
    }
    (7, 1023)
}

impl Synth {
    pub fn new(bank: Bank) -> Synth {
        let mut s = Synth { chip: nuked_opl3::Opl3Chip::new(RATE), bank, voices: [Voice::default(); VOICES], chans: [Chan::default(); 16], vel: [0; VOICES], age: 0 };
        // OPL3 mode (105h), waveforms on (01h), no four-operator pairs.
        s.chip.write_register(0x105, 1);
        s.chip.write_register(0x104, 0);
        s.chip.write_register(0x001, 0x20);
        s.chip.write_register(0x0BD, 0);
        s
    }

    fn base(v: usize) -> (u16, u16) {
        ((v / 9) as u16 * 0x100, (v % 9) as u16)
    }

    fn timbre_of(&self, ch: u8, key: u8) -> Option<((u8, u8), Timbre)> {
        let id = if ch == PERCUSSION { (key, 127) } else { (self.chans[ch as usize].program, self.chans[ch as usize].bank) };
        let t = self.bank.0.get(&id).or_else(|| self.bank.0.get(&(id.0, 0)))?;
        Some((id, *t))
    }

    /// The voice's output levels for its velocity and channel volume.
    fn levels(&mut self, v: usize) {
        let voice = self.voices[v];
        let Some(t) = voice.timbre.and_then(|id| self.bank.0.get(&id)).copied() else { return };
        let (bank, c) = Synth::base(v);
        let vol = self.vel[v] as u32 * self.chans[voice.ch as usize].volume as u32 / 0x7F;
        let scale = |reg: u8| -> u8 {
            let tl = (reg & 0x3F) as u32;
            let att = 63 - (63 - tl) * vol / 0x7F;
            (reg & 0xC0) | att as u8
        };
        let (m, k) = (OPS[c as usize], OPS[c as usize] + 3);
        self.chip.write_register(bank + 0x40 + k, scale(t.regs[7]));
        if t.regs[5] & 1 != 0 {
            self.chip.write_register(bank + 0x40 + m, scale(t.regs[1]));
        } else {
            self.chip.write_register(bank + 0x40 + m, t.regs[1]);
        }
    }

    fn pitch(&mut self, v: usize, on: bool) {
        let voice = self.voices[v];
        let Some((_, t)) = self.timbre_of(voice.ch, voice.key) else { return };
        let key = if voice.ch == PERCUSSION {
            if t.transpose != 0 { t.transpose as f32 } else { voice.key as f32 }
        } else {
            voice.key as f32 + t.transpose as i8 as f32
        };
        let bend = (self.chans[voice.ch as usize].bend as f32 - 8192.0) / 8192.0 * 2.0;
        let (block, f) = fnum(key + bend);
        let (bank, c) = Synth::base(v);
        self.chip.write_register(bank + 0xA0 + c, f as u8);
        self.chip.write_register(bank + 0xB0 + c, (f >> 8) as u8 | block << 2 | if on { 0x20 } else { 0 });
    }

    fn pan_bits(&self, ch: u8) -> u8 {
        match self.chans[ch as usize].pan {
            0..=47 => 0x10,
            80.. => 0x20,
            _ => 0x30,
        }
    }

    pub fn note_on(&mut self, ch: u8, key: u8, vel: u8) {
        if vel == 0 {
            self.note_off(ch, key);
            return;
        }
        let Some((id, t)) = self.timbre_of(ch, key) else { return };
        // A free voice, else the oldest (held ones first).
        let v = (0..VOICES)
            .find(|&v| !self.voices[v].on && !self.voices[v].held)
            .or_else(|| (0..VOICES).filter(|&v| self.voices[v].held).min_by_key(|&v| self.voices[v].age))
            .unwrap_or_else(|| (0..VOICES).min_by_key(|&v| self.voices[v].age).unwrap());
        let (bank, c) = Synth::base(v);
        // Off first, so the envelope restarts.
        self.chip.write_register(bank + 0xB0 + c, 0);
        if self.voices[v].timbre != Some(id) {
            let (m, k) = (OPS[c as usize], OPS[c as usize] + 3);
            let r = t.regs;
            for (off, (a, b)) in [(0x20, (r[0], r[6])), (0x60, (r[2], r[8])), (0x80, (r[3], r[9])), (0xE0, (r[4], r[10]))] {
                self.chip.write_register(bank + off + m, a);
                self.chip.write_register(bank + off + k, b);
            }
        }
        self.chip.write_register(bank + 0xC0 + c, (t.regs[5] & 0x0F) | self.pan_bits(ch));
        self.age += 1;
        self.voices[v] = Voice { on: true, held: false, ch, key, age: self.age, timbre: Some(id) };
        self.vel[v] = vel;
        self.levels(v);
        self.pitch(v, true);
    }

    pub fn note_off(&mut self, ch: u8, key: u8) {
        for v in 0..VOICES {
            let voice = self.voices[v];
            if voice.on && voice.ch == ch && voice.key == key {
                if self.chans[ch as usize].sustain {
                    self.voices[v].held = true;
                    self.voices[v].on = false;
                } else {
                    self.voices[v].on = false;
                    self.pitch(v, false);
                }
            }
        }
    }

    pub fn control(&mut self, ch: u8, c: u8, val: u8) {
        let chan = &mut self.chans[ch as usize];
        match c {
            7 => {
                chan.volume = val;
                for v in 0..VOICES {
                    if (self.voices[v].on || self.voices[v].held) && self.voices[v].ch == ch {
                        self.levels(v);
                    }
                }
            }
            10 => chan.pan = val,
            64 => {
                chan.sustain = val >= 64;
                if !chan.sustain {
                    for v in 0..VOICES {
                        if self.voices[v].held && self.voices[v].ch == ch {
                            self.voices[v].held = false;
                            self.pitch(v, false);
                        }
                    }
                }
            }
            114 => chan.bank = val,
            121 => {
                chan.bend = 0x2000;
                chan.sustain = false;
            }
            123 => {
                for v in 0..VOICES {
                    if self.voices[v].ch == ch && (self.voices[v].on || self.voices[v].held) {
                        self.voices[v].on = false;
                        self.voices[v].held = false;
                        self.pitch(v, false);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn program(&mut self, ch: u8, p: u8) {
        self.chans[ch as usize].program = p;
    }

    pub fn bend(&mut self, ch: u8, b: u16) {
        self.chans[ch as usize].bend = b;
        for v in 0..VOICES {
            if self.voices[v].on && self.voices[v].ch == ch {
                self.pitch(v, true);
            }
        }
    }

    /// Every voice off, the channels as at a song's start (2F9F:001F sets
    /// every channel's volume to 7Fh).
    pub fn reset(&mut self) {
        for v in 0..VOICES {
            self.voices[v].on = false;
            self.voices[v].held = false;
            let (bank, c) = Synth::base(v);
            self.chip.write_register(bank + 0xB0 + c, 0);
        }
        self.chans = [Chan::default(); 16];
    }

    /// One stereo sample.
    fn sample(&mut self) -> [i16; 2] {
        let mut s = [0i16; 2];
        let _ = self.chip.generate_resampled(&mut s);
        s
    }
}

/// A song being played: where it is, its notes to end, its loops.
pub struct Player {
    song: Xmi,
    at: usize,
    wait: u32,
    now: u64,
    offs: Vec<(u64, u8, u8)>,
    loops: Vec<(usize, u8)>,
    pub done: bool,
}

impl Player {
    pub fn new(song: Xmi) -> Player {
        let wait = song.events.first().map_or(0, |e| e.0);
        Player { song, at: 0, wait, now: 0, offs: Vec::new(), loops: Vec::new(), done: false }
    }

    /// One 120 Hz tick: the notes ending now, then the events due.
    pub fn tick(&mut self, synth: &mut Synth) {
        let now = self.now;
        self.offs.retain(|&(t, ch, key)| {
            if t <= now {
                synth.note_off(ch, key);
                false
            } else {
                true
            }
        });
        while !self.done && self.wait == 0 {
            let Some(&(_, ev)) = self.song.events.get(self.at) else {
                self.done = true;
                break;
            };
            self.at += 1;
            match ev {
                Ev::Note { ch, key, vel, dur } => {
                    synth.note_on(ch, key, vel);
                    self.offs.push((now + dur.max(1) as u64, ch, key));
                }
                Ev::Ctrl { c: 116, v, .. } => self.loops.push((self.at, v)),
                Ev::Ctrl { c: 117, v, .. } => {
                    if let Some(&(start, count)) = self.loops.last() {
                        if v < 64 {
                            self.loops.pop();
                        } else if count == 0 {
                            self.at = start;
                        } else if count > 1 {
                            self.loops.last_mut().unwrap().1 = count - 1;
                            self.at = start;
                        } else {
                            self.loops.pop();
                        }
                    }
                }
                Ev::Ctrl { ch, c, v } => synth.control(ch, c, v),
                Ev::Program { ch, p } => synth.program(ch, p),
                Ev::Bend { ch, v } => synth.bend(ch, v),
                Ev::End => {
                    self.done = true;
                    for &(_, ch, key) in &self.offs {
                        synth.note_off(ch, key);
                    }
                    self.offs.clear();
                }
            }
            self.wait = self.song.events.get(self.at).map_or(0, |e| e.0);
        }
        self.wait = self.wait.saturating_sub(1);
        self.now += 1;
    }
}

/// Renders a song's first `seconds` to stereo samples at RATE (the
/// tests').
#[cfg(test)]
pub fn render(song: &Xmi, bank: &Bank, seconds: f32) -> Vec<i16> {
    let mut synth = Synth::new(bank.clone());
    let mut player = Player::new(song.clone());
    let n = (seconds * RATE as f32) as usize;
    let mut out = Vec::with_capacity(n * 2);
    let mut frac = 0u32;
    for _ in 0..n {
        frac += TICKS;
        while frac >= RATE {
            frac -= RATE;
            player.tick(&mut synth);
        }
        out.extend(synth.sample());
    }
    out
}

enum Cmd {
    /// Fade out over 800 ms and stop (2F9F:0438).
    Stop,
    Play(Xmi),
}

/// The music driver: the songs asked for, played on a thread of its own
/// (its own PulseAudio stream), or only logged.
pub struct Music {
    archive: Rc<Archive>,
    log: bool,
    tx: Option<Sender<Cmd>>,
    status: Arc<AtomicU8>,
    songs: HashMap<Vec<u8>, Option<Xmi>>,
    /// Without a thread (shots): the song asked for last and the game's
    /// ticks it has played, for its status.
    headless: Option<(u32, u32)>,
}

impl Music {
    /// The driver: playing with `play` (when the timbres and a PulseAudio
    /// stream are there), `log` printing each song asked for.
    pub fn new(archive: Rc<Archive>, play: bool, log: bool) -> Music {
        let (tx, status) = if play { open(&archive) } else { (None, Arc::new(AtomicU8::new(0))) };
        Music { archive, log, tx, status, songs: HashMap::new(), headless: None }
    }

    /// Whether a driver is loaded (it plays or logs).
    pub fn on(&self) -> bool {
        self.tx.is_some() || self.log
    }

    /// Before a frame: the driver loaded (`load_driver`) and the song's
    /// status.
    pub fn before(&mut self, m: &mut Machine) {
        if !self.on() {
            return;
        }
        load_driver(m);
        m.music_status = match (&self.tx, self.headless) {
            (Some(_), _) => self.status.load(Ordering::Relaxed),
            (None, Some((len, played))) => {
                if played < len {
                    1
                } else {
                    2
                }
            }
            (None, None) => 0,
        };
    }

    /// After a frame of `ticks` game ticks (18.2 a second): the songs it
    /// asked for, played.
    pub fn after(&mut self, m: &mut Machine, ticks: u32) {
        if let Some((_, played)) = &mut self.headless {
            *played += ticks * TICKS / 18;
        }
        for s in std::mem::take(&mut m.songs) {
            if !self.on() {
                continue;
            }
            match s {
                Song::Stop => {
                    if self.log {
                        println!("music: stop");
                    }
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(Cmd::Stop);
                    }
                }
                Song::Play { file, .. } => {
                    if self.log {
                        println!("music: {}", String::from_utf8_lossy(&file));
                    }
                    let Some(xmi) = self.song(&file) else { continue };
                    self.headless = Some((if xmi.events.iter().any(|e| matches!(e.1, Ev::Ctrl { c: 116, v: 0, .. })) { u32::MAX } else { xmi.length() }, 0));
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(Cmd::Play(xmi));
                    }
                }
            }
        }
    }

    fn song(&mut self, name: &[u8]) -> Option<Xmi> {
        let key = name.to_ascii_uppercase();
        let archive = self.archive.clone();
        self.songs.entry(key.clone()).or_insert_with(|| archive.read(&String::from_utf8_lossy(&key)).and_then(|d| Xmi::parse(&d))).clone()
    }
}

/// The driver loaded, as the program's start leaves it: its far pointer
/// (8F5E:8F60, 2F9F:01C7) and, its timbre files being "AD", the FM flag
/// (1010, 2F9F:035B).
pub fn load_driver(m: &mut Machine) {
    use arena::mem::DS;
    use arena::music::var::{DRIVER, FM};
    if m.mem.r32(DS, DRIVER) == 0 {
        m.mem.w32(DS, DRIVER, 0x0001_0000);
    }
    m.mem.w8(DS, FM, 1);
}

/// The song a movie of the original's is played with (the routines that
/// show them take the song: 0000:1246 percntro with INTRO, 1323 evlintro
/// with SCROLL, 0D08/0D18 and 113E vision with the farewell and the
/// vision, 1194:D3D1 vision with the dream, D932 vision with the Emperor,
/// D9A8 wingame with the ending).
pub fn movie_song(name: &str) -> Option<u16> {
    use arena::music::var::*;
    match name.to_ascii_uppercase().as_str() {
        "INTRO.FLC" => Some(PERCNTRO),
        "SCROLL.FLC" => Some(EVLINTRO),
        "VISION.FLC" | "CHAOSVSN.FLC" | "JAGAR.FLC" | "MORPH.FLC" => Some(VISION),
        "WALK.FLC" | "AFLC2.FLC" | "NUJAGDTH.FLC" | "STAFF.FLC" | "KING.FLC" | "NUKING.FLC" => Some(WINGAME),
        _ => None,
    }
}

/// The thread: SAMPLE.AD loaded, a stereo PulseAudio stream, the songs
/// played as asked. None where either is missing.
fn open(archive: &Archive) -> (Option<Sender<Cmd>>, Arc<AtomicU8>) {
    let status = Arc::new(AtomicU8::new(0));
    let Some(bank) = archive.read("SAMPLE.AD").and_then(|d| Bank::parse(&d)) else {
        eprintln!("no music: SAMPLE.AD missing");
        return (None, status);
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let (ok_tx, ok_rx) = std::sync::mpsc::channel();
    let st = status.clone();
    std::thread::spawn(move || match crate::audio::Pulse::open_stream(2, c"music") {
        Some(p) => {
            let _ = ok_tx.send(true);
            run(p, Synth::new(bank), rx, st);
        }
        None => {
            let _ = ok_tx.send(false);
        }
    });
    if ok_rx.recv().unwrap_or(false) {
        (Some(tx), status)
    } else {
        eprintln!("no music: libpulse-simple.so.0 couldn't be opened");
        (None, status)
    }
}

fn run(pulse: crate::audio::Pulse, mut synth: Synth, rx: Receiver<Cmd>, status: Arc<AtomicU8>) {
    const CHUNK: usize = 512;
    let mut player: Option<Player> = None;
    let mut frac = 0u32;
    // The fade's samples left (800 ms at the stop), and the song after it.
    let mut fade: Option<(usize, Option<Xmi>)> = None;
    const FADE: usize = (RATE as usize) * 8 / 10;
    let mut buf = vec![0i16; CHUNK * 2];
    loop {
        let playing = player.as_ref().is_some_and(|p| !p.done) || fade.is_some();
        let cmd = if playing {
            rx.try_recv().ok()
        } else {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        };
        match cmd {
            Some(Cmd::Stop) => {
                if player.as_ref().is_some_and(|p| !p.done) && fade.is_none() {
                    fade = Some((FADE, None));
                }
            }
            Some(Cmd::Play(x)) => match &mut fade {
                Some((_, next)) => *next = Some(x),
                None => {
                    synth.reset();
                    player = Some(Player::new(x));
                    status.store(1, Ordering::Relaxed);
                }
            },
            None => {}
        }
        if player.is_none() && fade.is_none() {
            continue;
        }
        for i in 0..CHUNK {
            frac += TICKS;
            while frac >= RATE {
                frac -= RATE;
                if let Some(p) = &mut player {
                    if !p.done {
                        p.tick(&mut synth);
                        if p.done {
                            status.store(2, Ordering::Relaxed);
                        }
                    }
                }
            }
            let mut s = synth.sample();
            if let Some((left, _)) = &mut fade {
                let g = *left as i32;
                s = [(s[0] as i32 * g / FADE as i32) as i16, (s[1] as i32 * g / FADE as i32) as i16];
                *left = left.saturating_sub(1);
            }
            buf[2 * i] = s[0];
            buf[2 * i + 1] = s[1];
        }
        pulse.write(&buf);
        if let Some((0, next)) = &mut fade {
            let next = next.take();
            fade = None;
            synth.reset();
            status.store(0, Ordering::Relaxed);
            player = next.map(|x| {
                status.store(1, Ordering::Relaxed);
                Player::new(x)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Option<Archive> {
        let exe = std::env::var("ARENA_EXE").ok()?;
        let dir = std::path::Path::new(&exe).parent()?.to_path_buf();
        dir.join("GLOBAL.BSA").exists().then(|| Archive::open(&dir))
    }

    const SONGS: [&str; 33] = [
        "ARABCITY", "ARABTOWN", "ARAB_VLG", "CITY", "COMBAT", "CREDITS", "DUNGEON1", "DUNGEON2", "DUNGEON3", "DUNGEON4", "DUNGEON5", "EQUIPMNT",
        "EVIL", "EVLINTRO", "MAGIC_2", "NIGHT", "OVERCAST", "OVERSNOW", "PALACE", "PERCNTRO", "RAINING", "SHEET", "SNEAKING", "SNOWING", "SQUARE",
        "SUNNYDAY", "SWIMMING", "TAVERN", "TEMPLE", "TOWN", "VILLAGE", "VISION", "WINGAME",
    ];

    /// Every song, in both arrangements, parses to its end; the FM ones'
    /// lengths at 120 Hz are a few seconds for the towns' and minutes for
    /// the rest. (SUNNYDAY.XMI, the General MIDI arrangement, holds a
    /// delay of 1.2 million ticks.)
    #[test]
    fn parses_the_songs() {
        let Some(a) = game() else { return };
        let mut notes = 0;
        for s in SONGS {
            for ext in ["XMI", "XFM"] {
                let d = a.read(&format!("{s}.{ext}")).unwrap_or_else(|| panic!("{s}.{ext}"));
                let x = Xmi::parse(&d).unwrap_or_else(|| panic!("{s}.{ext} parses"));
                assert!(matches!(x.events.last(), Some((_, Ev::End))), "{s}.{ext} ends");
                assert!(!x.timbres.is_empty(), "{s}.{ext} timbres");
                let secs = x.length() as f32 / TICKS as f32;
                if ext == "XFM" {
                    assert!((1.0..300.0).contains(&secs), "{s}.{ext}: {secs} s");
                    notes += x.events.iter().filter(|e| matches!(e.1, Ev::Note { .. })).count();
                }
            }
        }
        // The FM arrangements' notes, counted by a separate scan of the
        // files: 20946 in GLOBAL.BSA, less 446 as the game directory's own
        // WINGAME.XFM (798 notes, against the archive's 1244) is read first.
        assert_eq!(notes, 20_500);
        let city = Xmi::parse(&a.read("CITY.XFM").unwrap()).unwrap();
        assert_eq!(city.length(), 707);
    }

    /// SAMPLE.AD: 233 timbres in banks 0, 1 and 127, every one the songs
    /// ask for there.
    #[test]
    fn parses_the_bank() {
        let Some(a) = game() else { return };
        let bank = Bank::parse(&a.read("SAMPLE.AD").unwrap()).unwrap();
        assert_eq!(bank.0.len(), 233);
        assert_eq!(bank.0[&(0, 0)], Timbre { transpose: 0, regs: [0x21, 0x8F, 0xF2, 0x45, 0x00, 0x08, 0x21, 0x06, 0xF2, 0x76, 0x00] });
        assert_eq!(bank.0[&(35, 127)].transpose, 35);
        for s in SONGS {
            let x = Xmi::parse(&a.read(&format!("{s}.XFM")).unwrap()).unwrap();
            for t in &x.timbres {
                assert!(bank.0.contains_key(t), "{s}: timbre {t:?}");
            }
        }
    }

    /// Ten seconds of a song: not silent, not clipping. With MUSIC_WAV=DIR
    /// the renders are written there.
    #[test]
    fn renders() {
        let Some(a) = game() else { return };
        let bank = Bank::parse(&a.read("SAMPLE.AD").unwrap()).unwrap();
        for s in ["CITY", "SUNNYDAY", "DUNGEON1", "TAVERN"] {
            let x = Xmi::parse(&a.read(&format!("{s}.XFM")).unwrap()).unwrap();
            let pcm = render(&x, &bank, 10.0);
            let peak = pcm.iter().map(|&v| (v as i32).abs()).max().unwrap();
            let rms = (pcm.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
            assert!(rms > 20.0 && peak > 1000, "{s}: rms {rms}, peak {peak}");
            assert!(peak < 32767, "{s}: clipped");
            if let Ok(dir) = std::env::var("MUSIC_WAV") {
                write_wav(&std::path::Path::new(&dir).join(format!("{s}.wav")), &pcm);
                eprintln!("{s}: rms {rms:.0}, peak {peak}");
            }
        }
    }

    fn write_wav(path: &std::path::Path, pcm: &[i16]) {
        let mut d = Vec::new();
        let data = (pcm.len() * 2) as u32;
        d.extend_from_slice(b"RIFF");
        d.extend_from_slice(&(36 + data).to_le_bytes());
        d.extend_from_slice(b"WAVEfmt ");
        d.extend_from_slice(&16u32.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&RATE.to_le_bytes());
        d.extend_from_slice(&(RATE * 4).to_le_bytes());
        d.extend_from_slice(&4u16.to_le_bytes());
        d.extend_from_slice(&16u16.to_le_bytes());
        d.extend_from_slice(b"data");
        d.extend_from_slice(&data.to_le_bytes());
        for &v in pcm {
            d.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(path, d).unwrap();
    }

    /// A FOR/NEXT loop of 2 plays its body twice.
    #[test]
    fn loops() {
        let note = Ev::Note { ch: 0, key: 60, vel: 100, dur: 1 };
        let x = Xmi {
            events: vec![(0, Ev::Ctrl { ch: 0, c: 116, v: 2 }), (1, note), (1, Ev::Ctrl { ch: 0, c: 117, v: 127 }), (1, Ev::End)],
            timbres: vec![],
        };
        let mut synth = Synth::new(Bank::default());
        let mut p = Player::new(x);
        let mut ticks = 0;
        while !p.done && ticks < 100 {
            p.tick(&mut synth);
            ticks += 1;
        }
        // The FOR at 0, the note at 1, the NEXT at 2; the note again at 3,
        // the NEXT at 4, the end at 5: six ticks run.
        assert_eq!(ticks, 6);
    }
}
