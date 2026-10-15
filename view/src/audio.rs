//! Sound: the game's VOC files played as the engine asks (`arena::sound`).
//!
//! The engine decides what plays (23BA:002A: a level's sound by number at
//! a square, its volume by the distance; 23BA:012B: a file by name). This
//! is the sound driver's side: the level's sound files by slot
//! (`Machine::sound_names`, from its INF's @SOUND section), decoded from
//! the game's files, and one voice: the original plays one sound at a time
//! and drops a new one while one plays (`Machine::sound_busy`, from how
//! long the last one lasts), but a named file stops the one playing.
//!
//! Playing goes through PulseAudio's simple API, `libpulse-simple.so.0`
//! opened at run time (no build-time audio libraries are needed); without
//! it, or with `--mute`, nothing is opened. With `--sound-log` each sound
//! the engine plays is printed (its name and volume), heard or not.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use arena::machine::Machine;
use arena::sound::Played;

use crate::archive::Archive;

/// The output rate the samples are made at.
const RATE: u32 = 22050;

/// A decoded Creative Voice File: unsigned 8-bit mono samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voc {
    pub rate: u32,
    pub samples: Vec<u8>,
}

/// Decodes a .VOC: "Creative Voice File\x1A", the data's offset (word at
/// 14h), then blocks of a type byte and a 24-bit size: 1 sound data (a
/// time constant, rate 1000000 / (256 - tc), a pack byte 0: 8-bit
/// unsigned PCM), 2 more data, 3 silence (a 16-bit length less 1 and a
/// time constant), 6 and 7 a repeat (played once), 0 the end. Other
/// packings and block types are skipped. None for a file that isn't one.
pub fn decode_voc(d: &[u8]) -> Option<Voc> {
    if d.get(..19)? != b"Creative Voice File" {
        return None;
    }
    let mut at = u16::from_le_bytes([*d.get(0x14)?, *d.get(0x15)?]) as usize;
    let mut voc = Voc { rate: 0, samples: Vec::new() };
    let rate_of = |tc: u8| 1_000_000 / (256 - tc as u32);
    while let Some(&ty) = d.get(at) {
        if ty == 0 {
            break;
        }
        let size = *d.get(at + 1)? as usize | (*d.get(at + 2)? as usize) << 8 | (*d.get(at + 3)? as usize) << 16;
        let body = d.get(at + 4..(at + 4 + size).min(d.len()))?;
        match ty {
            1 if body.len() >= 2 => {
                if voc.rate == 0 {
                    voc.rate = rate_of(body[0]);
                }
                if body[1] == 0 {
                    voc.samples.extend_from_slice(&body[2..]);
                }
            }
            2 => voc.samples.extend_from_slice(body),
            3 if body.len() >= 3 => {
                let n = u16::from_le_bytes([body[0], body[1]]) as usize + 1;
                if voc.rate == 0 {
                    voc.rate = rate_of(body[2]);
                }
                voc.samples.extend(std::iter::repeat_n(0x80, n));
            }
            _ => {}
        }
        at += 4 + size;
    }
    (voc.rate != 0).then_some(voc)
}

/// The samples at RATE, signed 16-bit, at `volume` (0..7Fh full).
fn pcm(voc: &Voc, volume: u8) -> Vec<i16> {
    let n = (voc.samples.len() as u64 * RATE as u64 / voc.rate.max(1) as u64) as usize;
    let gain = volume.min(0x7F) as i32;
    (0..n)
        .map(|i| {
            let s = voc.samples[(i as u64 * voc.rate as u64 / RATE as u64) as usize] as i32 - 0x80;
            (s * 256 * gain / 0x7F) as i16
        })
        .collect()
}

enum Cmd {
    /// Play these samples; with `over` stop the one playing first.
    Play { pcm: Vec<i16>, over: bool },
}

/// The sound driver: the decoded files, the voice, what is playing.
pub struct Audio {
    archive: Rc<Archive>,
    voice: Option<Sender<Cmd>>,
    log: bool,
    files: HashMap<Vec<u8>, Option<Voc>>,
    /// When the sound playing ends.
    until: Instant,
}

impl Audio {
    /// The driver: a voice when `play` and PulseAudio can be opened,
    /// `log` printing each sound.
    pub fn new(archive: Rc<Archive>, play: bool, log: bool) -> Audio {
        let voice = if play { open_voice() } else { None };
        Audio { archive, voice, log, files: HashMap::new(), until: Instant::now() }
    }

    /// Whether the engine should make sounds at all (sound "on", 8F64):
    /// with a voice or a log.
    pub fn on(&self) -> bool {
        self.voice.is_some() || self.log
    }

    /// Before a frame: sound on (as the original's driver, once loaded,
    /// sets 8F64), and the driver's status.
    pub fn before(&mut self, m: &mut Machine) {
        if self.on() {
            m.mem.w16(arena::mem::DS, arena::sound::var::ENABLED, 1);
        }
        m.sound_busy = Instant::now() < self.until;
    }

    /// After a frame: the sounds it played, played.
    pub fn after(&mut self, m: &mut Machine) {
        for p in std::mem::take(&mut m.sounds) {
            let (name, volume, over) = match p {
                Played::Number(s) => match m.sound_names.get(s.slot as usize) {
                    Some(n) => (n.clone(), s.volume, false),
                    None => continue,
                },
                Played::File(n) => (n, 0x7F, true),
            };
            if self.log {
                println!("sound: {} volume {volume:#x}{}", String::from_utf8_lossy(&name), if over { " (over)" } else { "" });
            }
            let Some(voc) = self.file(&name) else { continue };
            self.until = Instant::now() + Duration::from_secs_f64(voc.samples.len() as f64 / voc.rate.max(1) as f64);
            if let Some(v) = &self.voice {
                let _ = v.send(Cmd::Play { pcm: pcm(&voc, volume), over });
            }
        }
    }

    /// A sound file decoded, once (by its name, any case).
    fn file(&mut self, name: &[u8]) -> Option<Voc> {
        let key = name.to_ascii_uppercase();
        let archive = self.archive.clone();
        self.files
            .entry(key.clone())
            .or_insert_with(|| archive.read(&String::from_utf8_lossy(&key)).and_then(|d| decode_voc(&d)))
            .clone()
    }
}

/// The voice: a thread writing to a PulseAudio stream, None where the
/// library or the stream can't be opened.
fn open_voice() -> Option<Sender<Cmd>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let (ok_tx, ok_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || match Pulse::open() {
        Some(p) => {
            let _ = ok_tx.send(true);
            p.run(rx);
        }
        None => {
            let _ = ok_tx.send(false);
        }
    });
    if ok_rx.recv().unwrap_or(false) {
        Some(tx)
    } else {
        eprintln!("no sound: libpulse-simple.so.0 couldn't be opened");
        None
    }
}

#[repr(C)]
struct SampleSpec {
    format: i32,
    rate: u32,
    channels: u8,
}

type PaNew = unsafe extern "C" fn(
    *const std::ffi::c_char,
    *const std::ffi::c_char,
    i32,
    *const std::ffi::c_char,
    *const std::ffi::c_char,
    *const SampleSpec,
    *const std::ffi::c_void,
    *const std::ffi::c_void,
    *mut i32,
) -> *mut std::ffi::c_void;
type PaWrite = unsafe extern "C" fn(*mut std::ffi::c_void, *const std::ffi::c_void, usize, *mut i32) -> i32;
type PaFlush = unsafe extern "C" fn(*mut std::ffi::c_void, *mut i32) -> i32;
type PaFree = unsafe extern "C" fn(*mut std::ffi::c_void);

/// A PulseAudio playback stream (pa_simple), mono S16LE at RATE.
struct Pulse {
    _lib: libloading::Library,
    s: *mut std::ffi::c_void,
    write: PaWrite,
    flush: PaFlush,
    free: PaFree,
}

impl Pulse {
    fn open() -> Option<Pulse> {
        // SAFETY: libpulse-simple's C API, called as documented; the
        // functions stay valid while `_lib` is held.
        unsafe {
            let lib = libloading::Library::new("libpulse-simple.so.0").ok()?;
            let new: PaNew = *lib.get(b"pa_simple_new\0").ok()?;
            let write: PaWrite = *lib.get(b"pa_simple_write\0").ok()?;
            let flush: PaFlush = *lib.get(b"pa_simple_flush\0").ok()?;
            let free: PaFree = *lib.get(b"pa_simple_free\0").ok()?;
            // PA_SAMPLE_S16LE 3, PA_STREAM_PLAYBACK 1.
            let spec = SampleSpec { format: 3, rate: RATE, channels: 1 };
            let mut err = 0;
            let s = new(
                std::ptr::null(),
                c"Arena".as_ptr(),
                1,
                std::ptr::null(),
                c"sounds".as_ptr(),
                &spec,
                std::ptr::null(),
                std::ptr::null(),
                &mut err,
            );
            (!s.is_null()).then_some(Pulse { _lib: lib, s, write, flush, free })
        }
    }

    /// Plays what comes, a chunk at a time so that a named file can stop
    /// the sound playing.
    fn run(self, rx: Receiver<Cmd>) {
        const CHUNK: usize = 1024;
        let mut playing: Vec<i16> = Vec::new();
        let mut at = 0;
        loop {
            let cmd = if at < playing.len() {
                rx.try_recv().ok()
            } else {
                // Nothing playing: wait; the driver gone, stop.
                match rx.recv() {
                    Ok(c) => Some(c),
                    Err(_) => return,
                }
            };
            if let Some(Cmd::Play { pcm, over }) = cmd {
                if over || at >= playing.len() {
                    if over && at < playing.len() {
                        let mut err = 0;
                        // SAFETY: the stream is open.
                        unsafe { (self.flush)(self.s, &mut err) };
                    }
                    playing = pcm;
                    at = 0;
                }
            }
            if at < playing.len() {
                let end = (at + CHUNK).min(playing.len());
                let mut err = 0;
                // SAFETY: the slice is alive for the call; the stream is open.
                unsafe { (self.write)(self.s, playing[at..end].as_ptr().cast(), (end - at) * 2, &mut err) };
                at = end;
            }
        }
    }
}

impl Drop for Pulse {
    fn drop(&mut self) {
        // SAFETY: freed once, by the thread that opened it.
        unsafe { (self.free)(self.s) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_voc() {
        // A header, one data block (tc 131: 8000 Hz, 8-bit), a silence of
        // 3 samples, the end.
        let mut d = b"Creative Voice File\x1A\x1A\x00\x0A\x01\x29\x11".to_vec();
        d.extend([1, 5, 0, 0, 131, 0, 0x80, 0xFF, 0x00]);
        d.extend([3, 3, 0, 0, 2, 0, 131]);
        d.push(0);
        let v = decode_voc(&d).unwrap();
        assert_eq!(v.rate, 8000);
        assert_eq!(v.samples, vec![0x80, 0xFF, 0x00, 0x80, 0x80, 0x80]);
        assert!(decode_voc(b"not a voc").is_none());
    }

    /// The game's own files, when ARENA_EXE points at its directory: every
    /// .VOC in GLOBAL.BSA decodes, its samples at least its first data
    /// block's size less that block's two header bytes.
    #[test]
    fn decodes_the_games_vocs() {
        let Ok(exe) = std::env::var("ARENA_EXE") else { return };
        let dir = std::path::Path::new(&exe).parent().unwrap().to_path_buf();
        let a = Archive::open(&dir);
        let mut n = 0;
        for name in a.names() {
            if !name.ends_with(".VOC") {
                continue;
            }
            let d = a.read(name).unwrap();
            let v = decode_voc(&d).unwrap_or_else(|| panic!("{name}"));
            // The first data block (DRUMS.VOC opens with a repeat, 6).
            let mut at = u16::from_le_bytes([d[0x14], d[0x15]]) as usize;
            let block = |at: usize| d[at + 1] as usize | (d[at + 2] as usize) << 8 | (d[at + 3] as usize) << 16;
            while d[at] != 1 {
                assert_ne!(d[at], 0, "{name}");
                at += 4 + block(at);
            }
            let size = block(at);
            assert!(v.samples.len() >= size - 2, "{name}");
            assert!((4000..=23000).contains(&v.rate), "{name} {}", v.rate);
            n += 1;
        }
        assert!(n >= 70, "{n}");
    }
}
