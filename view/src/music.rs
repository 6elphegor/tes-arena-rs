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
//!   read from the game's files and given, with the music volume (D3A),
//!   to the original's own driver (`fmdrv`: SBP1FM.ADV, the Miles AIL
//!   XMIDI driver, ported routine by routine and run on its own image from
//!   the game's directory), as the game gives it (2F9F:001F): its
//!   sequencer, MIDI interpreter, voice allocation, volume and pitch
//!   arithmetic and register writes are the original's, write for write
//!   (tests/vectors/fmdrv.txt, from the original under the emulator);
//! - the timbres are AIL's Global Timbre Library SAMPLE.AD, installed in
//!   the driver's cache as the game installs them;
//! - the writes go to the Sound Blaster Pro's two OPL2s (`Fm`), each an
//!   OPL3 of Nuked-OPL3's (`nuked-opl3`) in its OPL2 mode: the left chip
//!   heard on the left, the right on the right (the driver writes both
//!   alike but for the operators' levels, which carry the pan); the
//!   driver's timer is served at 120 Hz as AIL's interrupt serves it.
//!
//! `Xmi` parses a song's events for the tests and for the status without
//! a stream (shots), which counts the song's ticks.
//!
//! The sequence's status (AIL's 1 playing, 2 done) goes back to
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
use crate::fmdrv;

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
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timbre {
    pub transpose: u8,
    pub regs: [u8; 11],
}

/// A Global Timbre Library (SAMPLE.AD) by (patch, bank).
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct Bank(pub HashMap<(u8, u8), Timbre>);

#[cfg(test)]
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


/// The card: the original's driver (`fmdrv`, SBP1FM.ADV from the game's
/// directory) and the Sound Blaster Pro's two OPL2s, each an OPL3 of
/// Nuked-OPL3's in its OPL2 mode, the left chip heard on the left and the
/// right on the right.
pub struct Fm {
    driver: fmdrv::Driver,
    gtl: Vec<u8>,
    chips: [nuked_opl3::Opl3Chip; 2],
}

impl Fm {
    /// The driver loaded from its image, with the timbre library
    /// (SAMPLE.AD). None for an image that isn't the driver.
    pub fn new(image: &[u8], gtl: Vec<u8>) -> Option<Fm> {
        let driver = fmdrv::Driver::new(image)?;
        let mut fm = Fm { driver, gtl, chips: [nuked_opl3::Opl3Chip::new(RATE), nuked_opl3::Opl3Chip::new(RATE)] };
        fm.flush();
        Some(fm)
    }

    /// The driver's writes to the chips.
    fn flush(&mut self) {
        for (chip, reg, v) in self.driver.take() {
            self.chips[chip as usize & 1].write_register(reg as u16, v);
        }
    }

    /// A song started as the game starts one (2F9F:001F), at the music
    /// volume (D3A). The game stops a song first (2F9F:0438) but for an
    /// interior's on a load; one still playing is overwritten in the
    /// driver's one state block, which both handles then serve (the new
    /// song at twice its speed), as the original does.
    pub fn play(&mut self, xmi: Vec<u8>, volume: u8) {
        self.driver.play(xmi, &self.gtl, volume);
        self.flush();
    }

    /// The fade of a stop (2F9F:0438: the volume to 0 over 800 ms).
    pub fn fade(&mut self) {
        self.driver.set_volume(0, 800);
    }

    /// The music volume changed.
    pub fn volume(&mut self, volume: u8) {
        self.driver.set_volume(volume as u16, 0);
        self.flush();
    }

    /// Stopped and released (2F9F:0438 after the fade).
    pub fn stop(&mut self) {
        self.driver.stop_sequence();
        self.driver.release_sequence();
        self.flush();
    }

    /// One service of the driver's 120 Hz timer.
    pub fn serve(&mut self) {
        self.driver.serve();
        self.flush();
    }

    /// The sequence's status: 0 none or stopped, 1 playing, 2 done.
    pub fn status(&self) -> u8 {
        self.driver.status() as u8
    }

    fn sample(&mut self) -> [i16; 2] {
        let (mut l, mut r) = ([0i16; 2], [0i16; 2]);
        let _ = self.chips[0].generate_resampled(&mut l);
        let _ = self.chips[1].generate_resampled(&mut r);
        [l[0], r[1]]
    }
}

/// Renders a song's first `seconds` to stereo samples at RATE (the
/// tests').
#[cfg(test)]
pub fn render(fm: &mut Fm, xmi: &[u8], seconds: f32) -> Vec<i16> {
    fm.play(xmi.to_vec(), 100);
    let n = (seconds * RATE as f32) as usize;
    let mut out = Vec::with_capacity(n * 2);
    let mut frac = 0u32;
    for _ in 0..n {
        frac += TICKS;
        while frac >= RATE {
            frac -= RATE;
            fm.serve();
        }
        out.extend(fm.sample());
    }
    out
}

enum Cmd {
    /// Fade out over 800 ms and stop (2F9F:0438).
    Stop,
    /// A song's file, at the music volume.
    Play(Vec<u8>, u8),
    /// The music volume changed (the options screen's arrows: 0FFD:0DE8
    /// at once).
    Volume(u8),
}

/// The music volume (D3A, 0..64h), which the game gives the driver with
/// each song (2F9F:01A3) and the options screen as it changes.
use arena::options::var::MUSIC as MUSIC_VOLUME;

/// The music driver: the songs asked for, played on a thread of its own
/// (its own PulseAudio stream), or only logged.
pub struct Music {
    archive: Rc<Archive>,
    log: bool,
    tx: Option<Sender<Cmd>>,
    status: Arc<AtomicU8>,
    songs: HashMap<Vec<u8>, Option<(Vec<u8>, Xmi)>>,
    /// Without a thread (shots): the song asked for last and the game's
    /// ticks it has played, for its status.
    headless: Option<(u32, u32)>,
    /// The music volume last given the driver.
    volume: Option<u8>,
}

impl Music {
    /// The driver: playing with `play` (when the timbres and a PulseAudio
    /// stream are there), `log` printing each song asked for.
    pub fn new(archive: Rc<Archive>, play: bool, log: bool) -> Music {
        let (tx, status) = if play { open(&archive) } else { (None, Arc::new(AtomicU8::new(0))) };
        Music { archive, log, tx, status, songs: HashMap::new(), headless: None, volume: None }
    }

    /// Whether a driver is loaded (it plays or logs).
    pub fn on(&self) -> bool {
        self.tx.is_some() || self.log
    }

    /// Before a frame: the driver loaded (`load_driver`), the song's
    /// status, and a change of the music volume passed on.
    pub fn before(&mut self, m: &mut Machine) {
        if !self.on() {
            return;
        }
        load_driver(m);
        let v = m.mem.r8(arena::mem::DS, MUSIC_VOLUME);
        if self.volume.is_some_and(|old| old != v) {
            if let Some(tx) = &self.tx {
                let _ = tx.send(Cmd::Volume(v));
            }
        }
        self.volume = Some(v);
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
                    let Some((data, xmi)) = self.song(&file) else { continue };
                    self.headless = Some((if xmi.events.iter().any(|e| matches!(e.1, Ev::Ctrl { c: 116, v: 0, .. })) { u32::MAX } else { xmi.length() }, 0));
                    let v = m.mem.r8(arena::mem::DS, MUSIC_VOLUME);
                    self.volume = Some(v);
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(Cmd::Play(data, v));
                    }
                }
            }
        }
    }

    fn song(&mut self, name: &[u8]) -> Option<(Vec<u8>, Xmi)> {
        let key = name.to_ascii_uppercase();
        let archive = self.archive.clone();
        self.songs
            .entry(key.clone())
            .or_insert_with(|| archive.read(&String::from_utf8_lossy(&key)).and_then(|d| Some((d.clone(), Xmi::parse(&d)?))))
            .clone()
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

/// The thread: SAMPLE.AD and the driver loaded, a stereo PulseAudio
/// stream, the songs played as asked. None where any is missing.
fn open(archive: &Archive) -> (Option<Sender<Cmd>>, Arc<AtomicU8>) {
    let status = Arc::new(AtomicU8::new(0));
    let Some(gtl) = archive.read("SAMPLE.AD") else {
        eprintln!("no music: SAMPLE.AD missing");
        return (None, status);
    };
    let Some(fm) = archive.read("SBP1FM.ADV").and_then(|d| Fm::new(&d, gtl)) else {
        eprintln!("no music: SBP1FM.ADV missing");
        return (None, status);
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let (ok_tx, ok_rx) = std::sync::mpsc::channel();
    let st = status.clone();
    std::thread::spawn(move || match crate::audio::Pulse::open_stream(2, c"music") {
        Some(p) => {
            let _ = ok_tx.send(true);
            run(p, fm, rx, st);
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

/// A song to start after a fade: its file and the music volume.
type Next = (Vec<u8>, u8);

fn run(pulse: crate::audio::Pulse, mut fm: Fm, rx: Receiver<Cmd>, status: Arc<AtomicU8>) {
    const CHUNK: usize = 512;
    let mut frac = 0u32;
    // The fade's samples left (800 ms at the stop), and the song after it.
    let mut fade: Option<(usize, Option<Next>)> = None;
    const FADE: usize = (RATE as usize) * 8 / 10;
    let mut buf = vec![0i16; CHUNK * 2];
    loop {
        let playing = fm.status() == 1 || fade.is_some();
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
                if fm.status() == 1 && fade.is_none() {
                    fm.fade();
                    fade = Some((FADE, None));
                }
            }
            Some(Cmd::Play(x, v)) => match &mut fade {
                Some((_, next)) => *next = Some((x, v)),
                None => fm.play(x, v),
            },
            Some(Cmd::Volume(v)) => fm.volume(v),
            None => {}
        }
        // Released voices still sound their release: the chips run on
        // until the next song, as the card's did.
        for i in 0..CHUNK {
            frac += TICKS;
            while frac >= RATE {
                frac -= RATE;
                fm.serve();
            }
            let s = fm.sample();
            if let Some((left, _)) = &mut fade {
                *left = left.saturating_sub(1);
            }
            buf[2 * i] = s[0];
            buf[2 * i + 1] = s[1];
        }
        status.store(fm.status(), Ordering::Relaxed);
        pulse.write(&buf);
        if let Some((0, next)) = &mut fade {
            let next = next.take();
            fade = None;
            fm.stop();
            if let Some((x, v)) = next {
                fm.play(x, v);
            }
            status.store(fm.status(), Ordering::Relaxed);
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

    /// Ten seconds of a song through the original's driver and the two
    /// chips: not silent, not clipping, both sides sounding. With
    /// MUSIC_WAV=DIR the renders are written there.
    #[test]
    fn renders() {
        let Some(a) = game() else { return };
        let mut fm = Fm::new(&a.read("SBP1FM.ADV").unwrap(), a.read("SAMPLE.AD").unwrap()).unwrap();
        for s in ["CITY", "SUNNYDAY", "DUNGEON1", "TAVERN"] {
            let pcm = render(&mut fm, &a.read(&format!("{s}.XFM")).unwrap(), 10.0);
            let peak = pcm.iter().map(|&v| (v as i32).abs()).max().unwrap();
            let rms = |side: usize| (pcm.iter().skip(side).step_by(2).map(|&v| (v as f64).powi(2)).sum::<f64>() / (pcm.len() / 2) as f64).sqrt();
            let (l, r) = (rms(0), rms(1));
            assert!(l > 20.0 && r > 20.0 && peak > 1000, "{s}: rms {l} {r}, peak {peak}");
            assert!(peak < 32767, "{s}: clipped");
            if let Ok(dir) = std::env::var("MUSIC_WAV") {
                write_wav(&std::path::Path::new(&dir).join(format!("{s}.wav")), &pcm);
                eprintln!("{s}: rms {l:.0} {r:.0}, peak {peak}");
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
}
