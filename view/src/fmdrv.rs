//! The original's FM music driver, SBP1FM.ADV (Miles AIL 2's XMIDI
//! driver for the Sound Blaster Pro's two OPL2s), routine by routine: its
//! sequencer, its MIDI interpreter, its voice allocation and its register
//! writes, so the music is the one the game played, write for write.
//!
//! The driver keeps its state in its own code segment (tables of the
//! sixteen MIDI channels, the sixteen note slots, the nine OPL voices,
//! the timbre cache's directory), and that state starts as the file has
//! it, so `Driver` holds the driver file's image (read from the game's
//! directory, never shipped) and the routines below address it as the
//! original does (the constants are its offsets). A sequence's state,
//! which the original keeps in the game's memory, is a `Seq`, in one of
//! the driver's eight handles; the game works on the last it registered.
//! Addresses in comments are the driver file's (its code segment's)
//! offsets.
//!
//! The writes are the card's: a register written to both chips (left at
//! 220h/221h, right at 222h/223h) with the same value, except the
//! operators' output levels, written with the left and right values of
//! the channel's pan (sub 122D). `Write` is (chip, register, value), chip
//! 0 the left. harness/gen_fmdrv.py records the same from the original
//! running under the game's AIL code, one driver service (120 Hz) at a
//! time, and the test below compares the two.
//!
//! What the game asks of it (2F9F:001F, `Driver::play`): the sequence
//! registered, the timbres it lists installed from SAMPLE.AD, controller
//! 7 at 127 on channels 1..16, the sequence started and its volume set to
//! the music volume (D3A). Stopping (2F9F:0438) ramps the volume to 0 over
//! 800 ms, then stops and releases the sequence (`set_volume`,
//! `stop_sequence`, `release_sequence`). The timbres stay in the cache
//! from song to song (Arena's 33 songs use 54, well within it: the
//! eviction, ported, never runs).
//!
//! Not ported: the beat and bar counters (AIL's queries only; the tempo
//! meta event and the time signature feed nothing else; of the XMIDI
//! calls the game makes, 2F9F reads back only the state table's and the
//! timbre cache's sizes (96h to EBA1, 99h to EB9F), the sequence's handle
//! (97h to EB00), the timbre asked for (9Bh) and the sequence's status
//! (AEh, 2F9F:000D, src/music.rs), never a count), the trigger
//! callback (controller 119: the game installs none), the indirect
//! controller values (controller 115: Arena's songs use none; the game
//! registers no array), four-operator and OPL3 timbres (sizes 19h; the
//! Pro's timbres are all 0Eh), and the note queue's overrun at its 32nd
//! entry (2FF7: the scan runs off its end, decrements the last entry
//! again and corrupts the driver's code; 32 notes held at once).

/// A register write: (chip, register, value), chip 0 the left OPL2, 1
/// the right.
pub type Write = (u8, u8, u8);

// The driver's tables (offsets in its code segment).
/// The operator register offsets (by operator 0..17).
const OP_REG: usize = 0x5EB;
/// The modulator's and carrier's operators of each voice.
const OP1: usize = 0x5C7;
const OP2: usize = 0x5D9;
/// The channel register offsets of each voice.
const VOICE_REG: usize = 0x633;
/// The velocity curve (by velocity / 8).
const VELOCITY: usize = 0x537;
/// The pan curve (by pan, the right side by 127 - pan).
const PAN: usize = 0x547;
/// F-numbers by semitone and sixteenth.
const FNUM: usize = 0x10D;
/// Block and semitone by note.
const BLOCK: usize = 0x28D;
const SEMITONE: usize = 0x2ED;
/// The registers' values written at the start (1..F5h).
const INIT_REGS: usize = 0x34C;
/// The timbre cache: the use counter (32 bits), each slot's last use (low
/// and high words), offset in the cache, bank, patch and flags (80h used,
/// 40h locked).
const COUNTER: usize = 0x68B;
const T_USE_LO: usize = 0x68F;
const T_USE_HI: usize = 0x80F;
const T_OFFSET: usize = 0x98F;
const T_BANK: usize = 0xB0F;
const T_PATCH: usize = 0xBCF;
const T_FLAGS: usize = 0xC8F;
const TIMBRES: usize = 0xC0;
const CACHE_SIZE: usize = 0xD53;
const CACHE_USED: usize = 0xD55;
/// The last voice given (round robin).
const NEXT_VOICE: usize = 0xD5C;
// The note slots (16): the timbre's offset in the cache (word), state (1
// on), kind (0 two-operator), OPL voice (FFh none), channel, key sounded,
// key asked, transpose, velocity, held by the sustain, pending updates,
// last B0h value, key-on bit, and the timbre's fields as the updates use
// them.
const S_TIMBRE: usize = 0xD61;
const S_RELEASE: usize = 0xDA1;
const S_STATE: usize = 0xDC1;
const S_KIND: usize = 0xDD1;
const S_VOICE: usize = 0xDE1;
const S_CH: usize = 0xDF1;
const S_KEY: usize = 0xE01;
const S_NOTE: usize = 0xE11;
const S_TRANSPOSE: usize = 0xE21;
const S_VELOCITY: usize = 0xE31;
const S_HELD: usize = 0xE41;
const S_UPDATE: usize = 0xE51;
const S_B0: usize = 0xE61;
const S_KEY_ON: usize = 0xE71;
const S_CONNECTION: usize = 0xE81;
const S_KSL1: usize = 0xE91;
const S_KSL2: usize = 0xEA1;
const S_FLAGS1: usize = 0xEB1;
const S_FLAGS2: usize = 0xEC1;
const S_AD1: usize = 0xED1;
const S_AD2: usize = 0xEE1;
const S_SR1: usize = 0xEF1;
const S_SR2: usize = 0xF01;
const S_SCALED: usize = 0xF11;
const S_WAVE: usize = 0xF21;
const S_MULT2: usize = 0xF41;
const S_MULT1: usize = 0xF61;
const S_FEEDBACK: usize = 0xF81;
const S_PRIORITY: usize = 0xFA1;
const S_LEVEL2: usize = 0xFC1;
const S_LEVEL1: usize = 0xFE1;
const S_RANK: usize = 0x116A;
const SLOTS: usize = 16;
// The MIDI channels (16): volume, pan, bend, expression, modulation,
// sustain, priority (controller 112), timbre slot, bank, program, voices
// sounding; the drums' timbre slots by key.
const C_VOLUME: usize = 0x1021;
const C_PAN: usize = 0x1031;
const C_BEND_LO: usize = 0x1041;
const C_BEND_HI: usize = 0x1051;
const C_EXPRESSION: usize = 0x1061;
const C_MODULATION: usize = 0x1071;
const C_SUSTAIN: usize = 0x1081;
const C_PRIORITY: usize = 0x1091;
const C_TIMBRE: usize = 0x10A1;
const C_BANK: usize = 0x10B1;
const C_PROGRAM: usize = 0x10C1;
const DRUMS: usize = 0x10D1;
const C_VOICES: usize = 0x1151;
/// Each OPL voice's channel (FFh free).
const V_CH: usize = 0x1161;
const VOICES: usize = 9;
// The XMIDI layer: the cached controllers (7, 1, 10, 11, 64, 114, 110,
// 111, 112), their defaults and the channels' default programs, each
// controller's row in the caches, the physical channels' controller
// cache (rows of 16), program, bend, notes and locks.
const CTRLS: usize = 0x23BE;
const CTRL_DEFAULTS: usize = 0x23C7;
const PROGRAMS: usize = 0x23CF;
const CTRL_ROW: usize = 0x23D9;
const CC: usize = 0x2505;
const C_PROG: usize = 0x2595;
const C_BEND_L: usize = 0x25A5;
const C_BEND_H: usize = 0x25B5;
const NOTES: usize = 0x25C5;
const LOCKS: usize = 0x25D5;
/// The timbre cache's size the game asks for (the driver's answer to
/// AIL's default-size query, 153D).
const CACHE: usize = 0xE00;

/// A registered sequence (the game's sequence state block).
#[derive(Clone, Debug)]
struct Seq {
    xmi: Vec<u8>,
    timb: Option<usize>,
    evnt: usize,
    /// The next event (+0Ch).
    at: usize,
    /// 0 stopped, 1 playing, 2 done (+1Ah).
    state: u16,
    /// Started once (+18h), release asked while playing (+1Ch).
    started: bool,
    release: bool,
    /// Ticks to the next event (+1Eh), notes in the queue (+20h).
    delta: i16,
    pending: u16,
    /// Volume and tempo (percent), current and target, and their ramps'
    /// accumulators and periods (+24h..+3Ch).
    volume: u16,
    volume_to: u16,
    volume_acc: u32,
    volume_period: u32,
    tempo_acc: u16,
    tempo: u16,
    tempo_to: u16,
    tempo_ramp: u32,
    tempo_period: u32,
    /// FOR loops: where (+50h) and the count left (+60h, FFFFh free).
    loop_at: [usize; 4],
    loop_count: [u16; 4],
    /// The channels' physical channels (+68h), programs (+78h), bends
    /// (+88h, +98h), indirect controller prefix (+A8h), cached
    /// controllers (+B8h, rows of 16).
    map: [u8; 16],
    program: [u8; 16],
    bend_lo: [u8; 16],
    bend_hi: [u8; 16],
    indirect: [u8; 16],
    cc: [u8; 0x90],
    /// The note queue: channel (FFh free), key, ticks left (+148h..).
    q_ch: [u8; 32],
    q_key: [u8; 32],
    q_left: [i32; 32],
}

impl Seq {
    fn new(xmi: Vec<u8>) -> Seq {
        Seq {
            xmi,
            timb: None,
            evnt: 0,
            at: 0,
            state: 0,
            started: false,
            release: false,
            delta: 0,
            pending: 0,
            volume: 100,
            volume_to: 100,
            volume_acc: 0,
            volume_period: 1,
            tempo_acc: 0,
            tempo: 100,
            tempo_to: 100,
            tempo_ramp: 0,
            tempo_period: 1,
            loop_at: [0; 4],
            loop_count: [0xFFFF; 4],
            map: [0; 16],
            program: [0xFF; 16],
            bend_lo: [0xFF; 16],
            bend_hi: [0xFF; 16],
            indirect: [0xFF; 16],
            cc: [0xFF; 0x90],
            q_ch: [0xFF; 32],
            q_key: [0; 32],
            q_left: [0; 32],
        }
    }

    fn byte(&self, at: usize) -> u8 {
        self.xmi.get(at).copied().unwrap_or(0)
    }

    /// A variable-length number at `at`: (value, bytes).
    fn vlq(&self, at: usize) -> (u32, usize) {
        let (mut v, mut n) = (0u32, 0);
        loop {
            let c = self.byte(at + n);
            n += 1;
            v = (v << 7) | (c & 0x7F) as u32;
            if c & 0x80 == 0 || n > 8 {
                return (v, n);
            }
        }
    }

    /// The sequence's state as at registration (2711).
    fn reset(&mut self) {
        self.loop_count = [0xFFFF; 4];
        for ch in 0..16 {
            self.map[ch] = ch as u8;
        }
        self.program = [0xFF; 16];
        self.bend_lo = [0xFF; 16];
        self.bend_hi = [0xFF; 16];
        self.indirect = [0xFF; 16];
        self.cc = [0xFF; 0x90];
        self.q_ch = [0xFF; 32];
        self.delta = 0;
        self.pending = 0;
        self.volume = 100;
        self.volume_to = 100;
        self.tempo = 100;
        self.tempo_to = 100;
        self.tempo_acc = 0;
    }
}

fn be32(d: &[u8], at: usize) -> usize {
    d.get(at..at + 4).map_or(0, |b| u32::from_be_bytes(b.try_into().unwrap()) as usize)
}

/// The `n`th (from 0) XMIDI sequence's FORM in a file (25E7): a FORM XMID,
/// or the n'th in a CAT XMID, past any other chunks (a FORM XDIR).
fn find_sequence(d: &[u8], n: usize) -> Option<usize> {
    let mut at = 0;
    loop {
        let tag = d.get(at..at + 4)?;
        if tag != b"CAT " && tag != b"FORM" {
            return None;
        }
        if d.get(at + 8..at + 12)? == b"XMID" {
            if tag == b"FORM" {
                return (n == 0).then_some(at);
            }
            let mut left = be32(d, at + 4) as i64 - 4;
            let mut sub = at + 12;
            let mut k = n + 1;
            loop {
                if d.get(sub + 8..sub + 12)? == b"XMID" {
                    k -= 1;
                    if k == 0 {
                        return Some(sub);
                    }
                }
                let len = be32(d, sub + 4) + 8;
                left -= len as i64;
                if left < 0 {
                    return None;
                }
                sub += len;
            }
        }
        at += be32(d, at + 4) + 8;
    }
}

/// The timbre (patch, bank) of a Global Timbre Library (SAMPLE.AD): its
/// size word and the bytes after it, as the game's loader returns them
/// (1194:4943).
pub fn gtl_timbre(gtl: &[u8], bank: u8, patch: u8) -> Option<Vec<u8>> {
    let mut at = 0;
    loop {
        let (p, b) = (*gtl.get(at)?, *gtl.get(at + 1)?);
        if b == 0xFF {
            return None;
        }
        if (p, b) == (patch, bank) {
            let off = u32::from_le_bytes(gtl.get(at + 2..at + 6)?.try_into().ok()?) as usize;
            let size = u16::from_le_bytes([*gtl.get(off)?, *gtl.get(off + 1)?]) as usize;
            return gtl.get(off..off + size).map(|t| t.to_vec());
        }
        at += 6;
    }
}

/// Sequence handles (the driver's table at 24D9).
const HANDLES: usize = 8;

/// The driver: its image (state), the timbre cache, the sequence state
/// block (the game has one, EB85, which every song it registers uses),
/// the handles registered on it, the one being served (24FB), the game's
/// (EB00: the last registered), and the writes not yet taken.
pub struct Driver {
    m: Vec<u8>,
    cache: Vec<u8>,
    seq: Option<Seq>,
    handles: [bool; HANDLES],
    cur: usize,
    game: Option<usize>,
    pub writes: Vec<Write>,
}

impl Driver {
    fn b(&self, a: usize) -> u8 {
        self.m[a]
    }
    fn sb(&mut self, a: usize, v: u8) {
        self.m[a] = v;
    }
    fn w(&self, a: usize) -> u16 {
        u16::from_le_bytes([self.m[a], self.m[a + 1]])
    }
    fn sw(&mut self, a: usize, v: u16) {
        self.m[a..a + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn q(&mut self) -> &mut Seq {
        self.seq.as_mut().unwrap()
    }

    /// The game's handle made the current one; false when it has none
    /// (a registration failed: the game's calls then do nothing). A
    /// released handle still names the state block, as the game's calls
    /// after a release read it.
    fn game_seq(&mut self) -> bool {
        match self.game {
            Some(h) if self.seq.is_some() => {
                self.cur = h;
                true
            }
            _ => false,
        }
    }

    /// The driver loaded and initialised (AIL's load, then 31D2): the
    /// chips' registers written from its table, its tables cleared, the
    /// controllers, bends and programs of channels 1..9 set to its
    /// defaults. None for a file too short to be it.
    pub fn new(image: &[u8]) -> Option<Driver> {
        if image.len() < 0x3B00 {
            return None;
        }
        let mut d = Driver { m: image.to_vec(), cache: vec![0; CACHE], seq: None, handles: [false; HANDLES], cur: 0, game: None, writes: Vec::new() };
        d.m[0x2505..0x2505 + 0xC0].fill(0xFF);
        d.m[CTRL_ROW..CTRL_ROW + 0x100].fill(0xFF);
        d.m[0x25D5..0x25E5].fill(0);
        d.m[0x25C5..0x25D5].fill(0);
        for k in 0..9 {
            let c = d.b(CTRLS + k) as usize;
            d.sb(CTRL_ROW + c, (k * 0x10) as u8);
        }
        // 141A: the registers.
        for r in 1..=0xF5usize {
            let v = d.b(INIT_REGS + r);
            d.out(r as u8, v, v);
        }
        d.reset_voices();
        d.sw(CACHE_SIZE, CACHE as u16);
        d.sw(CACHE_USED, 0);
        for k in 0..9 {
            let c = d.b(CTRLS + k);
            let v = d.b(CTRL_DEFAULTS + k);
            if v == 0xFF {
                continue;
            }
            for ch in 1..=9usize {
                let row = d.b(CTRL_ROW + c as usize) as usize + ch;
                d.sb(CC + row, v);
                d.send(0xB0 | ch as u8, c, v);
            }
        }
        for ch in 1..=9usize {
            d.sb(C_BEND_L + ch, 0);
            d.sb(C_BEND_H + ch, 0x40);
            d.send(0xE0 | ch as u8, 0, 0x40);
            let p = d.b(PROGRAMS + ch);
            if p != 0xFF {
                d.sb(C_PROG + ch, p);
                d.send(0xC0 | ch as u8, p, 0);
            }
        }
        Some(d)
    }

    /// 1822: the timbre directory, the channels' timbres, the slots and
    /// the voices cleared.
    fn reset_voices(&mut self) {
        self.sw(COUNTER, 0);
        self.sw(COUNTER + 2, 0);
        self.m[T_FLAGS..T_FLAGS + TIMBRES].fill(0);
        for ch in 0..16 {
            self.sb(C_TIMBRE + ch, 0xFF);
            self.sb(C_VOICES + ch, 0);
            self.sb(C_PROGRAM + ch, 0xFF);
            self.sb(C_BANK + ch, 0);
            self.sb(S_STATE + ch, 0);
        }
        self.m[V_CH..V_CH + VOICES].fill(0xFF);
        self.m[DRUMS..DRUMS + 0x80].fill(0xFF);
        self.sw(NEXT_VOICE, 0xFFFF);
    }

    // The card.

    /// A register written to both chips (11F8; 122D when they differ).
    fn out(&mut self, reg: u8, left: u8, right: u8) {
        self.writes.push((0, reg, left));
        self.writes.push((1, reg, right));
    }

    /// An operator's register (11C1).
    fn op_out(&mut self, op: u8, reg: u8, v: u8) {
        let r = self.b(OP_REG + op as usize).wrapping_add(reg);
        self.out(r, v, v);
    }

    /// A voice's register (11DD).
    fn voice_out(&mut self, voice: usize, reg: u8, v: u8) {
        let r = self.b(VOICE_REG + voice).wrapping_add(reg);
        self.out(r, v, v);
    }

    // The FM layer.

    /// The timbre slot of (bank, patch), FFFFh none (144D).
    fn find_timbre(&self, bank: u8, patch: u8) -> u16 {
        (0..TIMBRES)
            .find(|&s| self.b(T_FLAGS + s) & 0x80 != 0 && self.b(T_BANK + s) == bank && self.b(T_PATCH + s) == patch)
            .map_or(0xFFFF, |s| s as u16)
    }

    /// A slot's pending register updates written (1985): 80h the
    /// operators' flags and multipliers, 40h their output levels (the
    /// volume, expression, velocity and pan applied to the carrier, and
    /// to the modulator in additive mode), 20h the envelopes, 10h the
    /// waveforms, 08h the feedback and connection, 01h the pitch and key.
    fn update(&mut self, si: usize) {
        if self.b(S_VOICE + si) == 0xFF {
            return;
        }
        // 1A00: the four-operator kind (3) is never set; the two-operator
        // timbres go straight to the updates.
        let (mut left, mut right) = (0u8, 0u8);
        if self.b(S_UPDATE + si) & 0x40 != 0 {
            let ch = (self.b(S_CH + si) & 0xF) as usize;
            // "shl ax, 1; mov al, ah; cmp al, 1; sbb al, -1": the product
            // over 128, plus one unless 0.
            let scale = |ax: u16| -> u8 {
                let al = (ax.wrapping_shl(1) >> 8) as u8;
                if al == 0 { 0 } else { al.wrapping_add(1) }
            };
            let v = scale(self.b(C_VOLUME + ch) as u16 * self.b(C_EXPRESSION + ch) as u16);
            let v = scale(v as u16 * self.b(S_VELOCITY + si) as u16);
            let pan = self.b(C_PAN + ch) as usize;
            left = scale(self.b(PAN + pan) as u16 * v as u16);
            right = scale(self.b(PAN + (0x7F - pan as isize).rem_euclid(256) as usize) as u16 * v as u16);
        }
        let voice = self.b(S_VOICE + si) as usize;
        let op1 = self.b(OP1 + voice);
        let op2 = self.b(OP2 + voice);
        let ch = (self.b(S_CH + si) & 0xF) as usize;
        if self.b(S_UPDATE + si) & 0x80 != 0 {
            let vib = if (self.b(C_MODULATION + ch) as i8) >= 0x40 { 0x40 } else { 0 };
            let v1 = (self.w(S_MULT1 + 2 * si) >> 12) as u8 | vib | self.b(S_FLAGS1 + si);
            self.op_out(op1, 0x20, v1);
            let v2 = (self.w(S_MULT2 + 2 * si) >> 12) as u8 | vib | self.b(S_FLAGS2 + si);
            self.op_out(op2, 0x20, v2);
            self.m[S_UPDATE + si] &= 0x7F;
        }
        if self.b(S_UPDATE + si) & 0x40 != 0 {
            let scaled = self.b(S_SCALED + si);
            for (level, ksl, bit, op) in [(S_LEVEL1, S_KSL1, 1, op1), (S_LEVEL2, S_KSL2, 2, op2)] {
                let t = (self.w(level + 2 * si) >> 10) as u8;
                let (mut h, mut l) = (t, t);
                if scaled & bit != 0 {
                    h = (t as u16 * left as u16 / 0x7F) as u8;
                    l = (t as u16 * right as u16 / 0x7F) as u8;
                }
                let k = self.b(ksl + si);
                let r = self.b(OP_REG + op as usize).wrapping_add(0x40);
                self.out(r, (!h & 0x3F) | k, (!l & 0x3F) | k);
            }
            self.m[S_UPDATE + si] &= 0xBF;
        }
        if self.b(S_UPDATE + si) & 0x20 != 0 {
            let (a1, a2, s1, s2) = (self.b(S_AD1 + si), self.b(S_AD2 + si), self.b(S_SR1 + si), self.b(S_SR2 + si));
            self.op_out(op1, 0x60, a1);
            self.op_out(op2, 0x60, a2);
            self.op_out(op1, 0x80, s1);
            self.op_out(op2, 0x80, s2);
            self.m[S_UPDATE + si] &= 0xDF;
        }
        if self.b(S_UPDATE + si) & 0x10 != 0 {
            let w = self.w(S_WAVE + 2 * si);
            self.op_out(op2, 0xE0, w as u8);
            self.op_out(op1, 0xE0, (w >> 8) as u8);
            self.m[S_UPDATE + si] &= 0xEF;
        }
        if self.b(S_UPDATE + si) & 0x08 != 0 {
            let v = ((self.w(S_FEEDBACK + 2 * si) >> 12) as u8 & 0xE) | (self.b(S_CONNECTION + si) & 1);
            self.voice_out(voice, 0xC0, v);
            self.m[S_UPDATE + si] &= 0xF7;
        }
        if self.b(S_UPDATE + si) & 0x01 != 0 {
            if self.b(S_KEY_ON + si) & 0x20 == 0 {
                let v = self.b(S_B0 + si) & 0xDF;
                self.voice_out(voice, 0xB0, v);
            } else {
                let (lo, hi) = self.pitch(si);
                self.voice_out(voice, 0xA0, lo);
                let b0 = hi | self.b(S_KEY_ON + si);
                self.sb(S_B0 + si, b0);
                self.voice_out(voice, 0xB0, b0);
            }
            self.m[S_UPDATE + si] &= 0xFE;
        }
    }

    /// A slot's (A0h, B0h) values (1D2D): the key, the timbre's transpose
    /// and the channel's bend (two semitones) in sixteenths, folded into
    /// eight octaves, looked up.
    fn pitch(&self, si: usize) -> (u8, u8) {
        let ch = self.b(S_CH + si) as usize;
        let bend = ((self.b(C_BEND_HI + ch) as u16) << 7) | self.b(C_BEND_LO + ch) as u16;
        let ax = ((bend.wrapping_sub(0x2000) as i16) >> 5).wrapping_mul(12);
        let mut bx = self.b(S_KEY + si) as i16 + self.b(S_TRANSPOSE + si) as i8 as i16 - 0x18;
        loop {
            bx += 12;
            if bx >= 0 {
                break;
            }
        }
        bx += 12;
        loop {
            bx -= 12;
            if bx <= 0x5F {
                break;
            }
        }
        let ax = (ax as u16).wrapping_add((bx as u8 as u16) << 8);
        let mut ax = (ax.wrapping_add(8) as i16) >> 4;
        ax -= 0xC0;
        loop {
            ax += 0xC0;
            if ax >= 0 {
                break;
            }
        }
        ax += 0xC0;
        loop {
            ax -= 0xC0;
            if ax <= 0x5FF {
                break;
            }
        }
        let note = (ax as u16 >> 4) as usize;
        let semi = self.b(SEMITONE + note) as usize;
        let mut f = self.w(FNUM + semi * 32 + ((ax as usize * 2) & 0x1F)) as i16;
        let mut block = self.b(BLOCK + note).wrapping_sub(1) as i8;
        if f < 0 {
            block = block.wrapping_add(1);
        }
        if block < 0 {
            block += 1;
            f >>= 1;
        }
        let hi = ((block as u8) << 2) | ((f as u16 >> 8) as u8 & 3);
        (f as u8, hi)
    }

    /// A slot's voice keyed off and freed (191A).
    fn release(&mut self, si: usize) {
        if self.b(S_VOICE + si) == 0xFF {
            return;
        }
        self.m[S_KEY_ON + si] &= 0xDF;
        self.m[S_UPDATE + si] |= 1;
        self.update(si);
        let ch = self.b(S_CH + si) as usize;
        self.m[C_VOICES + ch] = self.m[C_VOICES + ch].wrapping_sub(1);
        let v = self.b(S_VOICE + si) as usize;
        let kind = self.b(S_KIND + si);
        if kind == 3 {
            self.sb(V_CH + v + 3, 0xFF);
        }
        self.sb(V_CH + v, 0xFF);
        self.sb(S_VOICE + si, 0xFF);
        if kind == 3 || kind == 0 {
            self.sb(S_STATE + si, 0);
        }
    }

    /// A voice for a slot (18BE): the next free one after the last given,
    /// else one taken from a lower-priority slot.
    fn assign(&mut self, si: usize) {
        let mut v = self.w(NEXT_VOICE);
        for _ in 0..VOICES {
            v = v.wrapping_add(1);
            if v == VOICES as u16 {
                v = 0;
            }
            self.sw(NEXT_VOICE, v);
            if self.b(V_CH + v as usize) == 0xFF {
                self.give(si, v as usize);
                return;
            }
        }
        self.steal();
    }

    fn give(&mut self, si: usize, v: usize) {
        self.sb(S_VOICE + si, v as u8);
        let ch = self.b(S_CH + si);
        self.m[C_VOICES + ch as usize] = self.m[C_VOICES + ch as usize].wrapping_add(1);
        self.sb(V_CH + v, ch);
        if self.b(S_KIND + si) == 3 {
            self.sb(V_CH + v + 3, ch);
        }
        self.sb(S_UPDATE + si, 0xF9);
        self.update(si);
    }

    /// 1E35: each slot ranked by its priority (FFFFh on a channel with
    /// controller 112 at 64 or more, else 7FFFh) less its channel's voices;
    /// while the best slot waiting outranks (or ties) the lowest sounding,
    /// the lowest's voice goes to it.
    fn steal(&mut self) {
        let mut count = 0u16;
        for s in 0..SLOTS {
            if self.b(S_STATE + s) == 0 {
                continue;
            }
            count += 1;
            let ch = (self.b(S_CH + s) & 0xF) as usize;
            let p = if (self.b(C_PRIORITY + ch) as i8) >= 0x40 { 0xFFFF } else { self.w(S_PRIORITY + 2 * s) };
            let p = p.saturating_sub(self.b(C_VOICES + ch) as u16);
            self.sw(S_RANK + 2 * s, p);
        }
        let (mut waiting, mut lowest) = (0, 0);
        loop {
            let (mut best, mut low) = (0u16, 0xFFFFu16);
            for s in 0..SLOTS {
                if self.b(S_STATE + s) == 0 {
                    continue;
                }
                let r = self.w(S_RANK + 2 * s);
                if self.b(S_VOICE + s) == 0xFF {
                    if r >= best {
                        best = r;
                        waiting = s;
                    }
                } else if r <= low {
                    low = r;
                    lowest = s;
                }
            }
            if best < low || best == 0 {
                return;
            }
            let v = self.b(S_VOICE + lowest) as usize;
            self.release(lowest);
            self.give(waiting, v);
            count = count.wrapping_sub(1);
            if count == 0 {
                return;
            }
        }
    }

    /// A slot's timbre fields from its timbre (1F38).
    fn load_timbre(&mut self, si: usize) {
        let t = self.w(S_TIMBRE + 2 * si) as usize;
        let c = |i: usize| self.cache.get(t + i).copied().unwrap_or(0);
        let (b3, b4, b5, b6, b7, b8, b9, ba, bb, bc, bd) = (c(3), c(4), c(5), c(6), c(7), c(8), c(9), c(10), c(11), c(12), c(13));
        self.sb(S_KEY_ON + si, 0x20);
        self.sb(S_KIND + si, 0);
        self.sw(S_RELEASE + 2 * si, 0xFFFF);
        self.sw(S_PRIORITY + 2 * si, 0x7FFF);
        self.sb(S_CONNECTION + si, b8 & 1);
        self.sw(S_FEEDBACK + 2 * si, (b8 as u16) << 12);
        self.sb(S_KSL1 + si, b4 & 0xC0);
        self.sw(S_LEVEL1 + 2 * si, ((!b4 & 0x3F) as u16) << 10);
        self.sb(S_KSL2 + si, ba & 0xC0);
        self.sw(S_LEVEL2 + 2 * si, ((!ba & 0x3F) as u16) << 10);
        self.sb(S_FLAGS1 + si, b3 & 0xF0);
        self.sw(S_MULT1 + 2 * si, (b3 as u16) << 12);
        self.sb(S_FLAGS2 + si, b9 & 0xF0);
        self.sw(S_MULT2 + 2 * si, (b9 as u16) << 12);
        self.sb(S_AD1 + si, b5);
        self.sb(S_SR1 + si, b6);
        self.sb(S_AD2 + si, bb);
        self.sb(S_SR2 + si, bc);
        self.sw(S_WAVE + 2 * si, bd as u16 | (b7 as u16) << 8);
        self.sb(S_SCALED + si, (b8 & 1) | 2);
        self.sb(S_UPDATE + si, 0xF9);
    }

    /// A note-off (202B): the slots of the key on the channel released,
    /// or held while its sustain is down.
    fn note_off(&mut self, ch: u8, key: u8) {
        for si in 0..SLOTS {
            if self.b(S_STATE + si) != 1 || self.b(S_NOTE + si) != key || self.b(S_CH + si) != ch {
                continue;
            }
            if (self.b(C_SUSTAIN + ch as usize) as i8) >= 0x40 {
                self.sb(S_HELD + si, 1);
            } else if matches!(self.b(S_KIND + si), 0 | 3) {
                self.release(si);
                self.sb(S_STATE + si, 0);
            } else {
                self.sw(S_RELEASE + 2 * si, 1);
            }
        }
    }

    /// The sustain released (219A): the held notes' note-offs (by the key
    /// they sound, which for the drums isn't the key they were asked
    /// with: those stay).
    fn sustain_off(&mut self, ch: u8) {
        for si in 0..SLOTS {
            if self.b(S_STATE + si) != 0 && self.b(S_CH + si) == ch && self.b(S_HELD + si) != 0 {
                let key = self.b(S_KEY + si);
                self.note_off(ch, key);
            }
        }
    }

    /// A note-on (2098): the channel's timbre (the drums' by key, bank
    /// 127), a free slot, the key (the drum's its timbre's), a voice.
    fn note_on(&mut self, ch: u8, key: u8, vel: u8) {
        let mut t = self.b(C_TIMBRE + ch as usize);
        if ch == 9 {
            t = self.b(DRUMS + key as usize);
            if t == 0xFF {
                t = self.find_timbre(0x7F, key) as u8;
                self.sb(DRUMS + key as usize, t);
            }
        }
        if t == 0xFF {
            return;
        }
        let t = t as usize;
        let at = self.w(T_OFFSET + 2 * t) as usize;
        let n = (self.w(COUNTER) as u32 | (self.w(COUNTER + 2) as u32) << 16).wrapping_add(1);
        self.sw(COUNTER, n as u16);
        self.sw(COUNTER + 2, (n >> 16) as u16);
        self.sw(T_USE_LO + 2 * t, n as u16);
        self.sw(T_USE_HI + 2 * t, (n >> 16) as u16);
        let Some(si) = (0..SLOTS).find(|&s| self.b(S_STATE + s) == 0) else { return };
        self.sb(S_CH + si, ch);
        self.sb(S_NOTE + si, key);
        let transpose = self.cache.get(at + 2).copied().unwrap_or(0);
        let (k, tr) = if ch == 9 { (transpose, 0) } else { (key, transpose) };
        self.sb(S_KEY + si, k);
        self.sb(S_TRANSPOSE + si, tr);
        let v = self.b(VELOCITY + (vel >> 3) as usize);
        self.sb(S_VELOCITY + si, v);
        self.sw(S_TIMBRE + 2 * si, at as u16);
        self.sb(S_STATE + si, 1);
        self.sb(S_HELD + si, 0);
        let size = u16::from_le_bytes([self.cache.get(at).copied().unwrap_or(0), self.cache.get(at + 1).copied().unwrap_or(0)]);
        match size {
            0x19 => {}
            0x0E => self.load_timbre(si),
            _ => return,
        }
        self.sb(S_VOICE + si, 0xFF);
        self.assign(si);
    }

    /// The channel's sounding slots given updates (22D7).
    fn touch(&mut self, ch: u8, flags: u8) {
        for si in 0..SLOTS {
            if self.b(S_STATE + si) != 0 && self.b(S_CH + si) == ch {
                self.m[S_UPDATE + si] |= flags;
                self.update(si);
            }
        }
    }

    /// A MIDI message to the FM layer (21D5). Notes play on channels
    /// 2..10 (1..9 here) only, 10 the drums.
    fn send(&mut self, status: u8, d1: u8, d2: u8) {
        let ch = status & 0xF;
        let c = ch as usize;
        match status & 0xF0 {
            0xB0 => match d1 {
                0x72 => self.sb(C_BANK + c, d2),
                0x70 => self.sb(C_PRIORITY + c, d2),
                0x71 => {
                    let t = self.b(C_TIMBRE + c);
                    if t != 0xFF {
                        let mut f = self.b(T_FLAGS + t as usize) & 0xBF;
                        if (d2 as i8) >= 0x40 {
                            f |= 0x40;
                        }
                        self.sb(T_FLAGS + t as usize, f);
                    }
                }
                1 => {
                    self.sb(C_MODULATION + c, d2);
                    self.touch(ch, 0x80);
                }
                7 => {
                    self.sb(C_VOLUME + c, d2);
                    self.touch(ch, 0x40);
                }
                0x0B => {
                    self.sb(C_EXPRESSION + c, d2);
                    self.touch(ch, 0x40);
                }
                0x0A => {
                    self.sb(C_PAN + c, d2);
                    self.touch(ch, 0x40);
                }
                0x40 => {
                    self.sb(C_SUSTAIN + c, d2);
                    if (d2 as i8) < 0x40 {
                        self.sustain_off(ch);
                    }
                }
                0x79 => {
                    self.sb(C_SUSTAIN + c, 0);
                    self.sustain_off(ch);
                    self.sb(C_MODULATION + c, 0);
                    self.sb(C_EXPRESSION + c, 0x7F);
                    self.sb(C_BEND_LO + c, 0);
                    self.sb(C_BEND_HI + c, 0x40);
                    self.touch(ch, 0xC1);
                }
                0x7B => {
                    for si in 0..SLOTS {
                        if self.b(S_STATE + si) == 1 && self.b(S_CH + si) == ch {
                            let key = self.b(S_KEY + si);
                            self.note_off(ch, key);
                        }
                    }
                }
                _ => {}
            },
            0xC0 => {
                self.sb(C_PROGRAM + c, d1);
                let t = self.find_timbre(self.b(C_BANK + c), d1);
                self.sb(C_TIMBRE + c, t as u8);
            }
            0xE0 => {
                self.sb(C_BEND_LO + c, d1);
                self.sb(C_BEND_HI + c, d2);
                self.touch(ch, 1);
            }
            0x80 => self.note_off(ch, d1),
            0x90 if (1..=9).contains(&ch) => {
                if d2 == 0 {
                    self.note_off(ch, d1);
                } else {
                    self.note_on(ch, d1, d2);
                }
            }
            _ => {}
        }
    }

    // The timbre cache.

    /// The first timbre the sequence lists that isn't installed, as
    /// bank << 8 | patch, FFFFh none (159A).
    pub fn timbre_request(&self) -> u16 {
        let Some(q) = self.game.and(self.seq.as_ref()) else { return 0xFFFF };
        let Some(t) = q.timb else { return 0xFFFF };
        let n = u16::from_le_bytes([q.byte(t + 8), q.byte(t + 9)]) as usize;
        for k in 0..n {
            let (patch, bank) = (q.byte(t + 10 + 2 * k), q.byte(t + 11 + 2 * k));
            if self.find_timbre(bank, patch) == 0xFFFF {
                return (bank as u16) << 8 | patch as u16;
            }
        }
        0xFFFF
    }

    /// A timbre installed in the cache (173F), the least recently used
    /// ones dropped for room; the channels on its program given it.
    pub fn install_timbre(&mut self, bank: u8, patch: u8, data: &[u8]) {
        let mut slot = self.find_timbre(bank, patch);
        if slot == 0xFFFF {
            if data.len() < 2 {
                return;
            }
            let size = u16::from_le_bytes([data[0], data[1]]) as usize;
            loop {
                let free = (0..TIMBRES).find(|&s| self.b(T_FLAGS + s) & 0x80 == 0);
                let used = self.w(CACHE_USED) as usize;
                match free {
                    Some(s) if size + used <= self.w(CACHE_SIZE) as usize => {
                        self.sw(CACHE_USED, (size + used) as u16);
                        self.sb(T_PATCH + s, patch);
                        self.sb(T_BANK + s, bank);
                        self.sb(T_FLAGS + s, 0x80);
                        let n = self.w(COUNTER) as u32 | (self.w(COUNTER + 2) as u32) << 16;
                        self.sw(T_USE_LO + 2 * s, n as u16);
                        self.sw(T_USE_HI + 2 * s, (n >> 16) as u16);
                        let n = n.wrapping_add(1);
                        self.sw(COUNTER, n as u16);
                        self.sw(COUNTER + 2, (n >> 16) as u16);
                        self.sw(T_OFFSET + 2 * s, used as u16);
                        for (i, &b) in data.iter().take(size).enumerate() {
                            if let Some(c) = self.cache.get_mut(used + i) {
                                *c = b;
                            }
                        }
                        slot = s as u16;
                        break;
                    }
                    _ => {
                        if !self.evict() {
                            return;
                        }
                    }
                }
            }
        }
        for ch in 0..16 {
            if self.b(C_PROGRAM + ch) == patch && self.b(C_BANK + ch) == bank {
                self.sb(C_TIMBRE + ch, slot as u8);
            }
        }
    }

    /// 15F4: the least recently used unlocked timbre dropped, the cache
    /// closed up behind it, the channels and drums on it cleared and the
    /// slots sounding it released. False when there is none to drop.
    fn evict(&mut self) -> bool {
        let mut pick = None;
        let (mut lo, mut hi) = (0xFFFFu16, 0xFFFFu16);
        for s in 0..TIMBRES {
            let f = self.b(T_FLAGS + s);
            if f & 0x80 == 0 || f & 0x40 != 0 {
                continue;
            }
            let (l, h) = (self.w(T_USE_LO + 2 * s), self.w(T_USE_HI + 2 * s));
            if h > hi || (h == hi && l > lo) {
                continue;
            }
            lo = l;
            hi = h;
            pick = Some(s);
        }
        let Some(s) = pick else { return false };
        let at = self.w(T_OFFSET + 2 * s) as usize;
        let size = u16::from_le_bytes([self.cache[at], self.cache[at + 1]]) as usize;
        let used = self.w(CACHE_USED) as usize;
        self.cache.copy_within(at + size..used, at);
        self.sb(T_FLAGS + s, 0);
        self.sw(CACHE_USED, (used - size) as u16);
        for ch in 0..16 {
            if self.b(C_TIMBRE + ch) as usize == s {
                self.sb(C_TIMBRE + ch, 0xFF);
            }
        }
        for k in 0..0x80 {
            if self.b(DRUMS + k) as usize == s {
                self.sb(DRUMS + k, 0xFF);
            }
        }
        for t in 0..TIMBRES {
            if self.b(T_FLAGS + t) & 0x80 != 0 {
                let o = self.w(T_OFFSET + 2 * t) as usize;
                if o > at {
                    self.sw(T_OFFSET + 2 * t, (o - size) as u16);
                }
            }
        }
        for si in 0..SLOTS {
            if self.b(S_STATE + si) == 0 {
                continue;
            }
            let o = self.w(S_TIMBRE + 2 * si) as usize;
            if o == at {
                self.release(si);
                self.sb(S_STATE + si, 0);
            } else if o > at {
                self.sw(S_TIMBRE + 2 * si, (o - size) as u16);
            }
        }
        true
    }

    // The XMIDI layer.

    /// A sequence registered (33F0) in the first free handle, the game's
    /// from now on: the file's `n`th sequence, its TIMB and EVNT chunks
    /// found, in the state block (whatever it held: a song still playing
    /// there is overwritten, its queued notes never ended, and its handle
    /// serves the new song too). False when the file holds none or no
    /// handle is free (the game's handle is then FFFFh: none).
    pub fn register(&mut self, xmi: Vec<u8>, n: usize) -> bool {
        self.game = None;
        let Some(h) = self.handles.iter().position(|&r| !r) else { return false };
        let Some(form) = find_sequence(&xmi, n) else { return false };
        // The fields 33F0 and 2711 don't set (the ramps') carry over.
        let mut q = match self.seq.take() {
            Some(old) => Seq { xmi, timb: None, ..old },
            None => Seq::new(xmi),
        };
        let mut at = form;
        let mut len = 12;
        let evnt = loop {
            at += len;
            let tag = match q.xmi.get(at..at + 4) {
                Some(t) => t.to_vec(),
                None => return false,
            };
            len = be32(&q.xmi, at + 4) + 8;
            match &tag[..] {
                b"TIMB" => q.timb = Some(at),
                b"EVNT" => break at,
                _ => {}
            }
        };
        q.evnt = evnt;
        q.state = 0;
        q.started = false;
        q.release = false;
        q.reset();
        self.seq = Some(q);
        self.handles[h] = true;
        self.game = Some(h);
        self.cur = h;
        true
    }

    /// The game's sequence's state: 0 stopped (or none), 1 playing, 2
    /// done (36AE).
    pub fn status(&self) -> u16 {
        self.game.and(self.seq.as_ref()).map_or(0, |q| q.state)
    }

    /// The game's sequence played from the start (357A).
    pub fn start(&mut self) {
        if !self.game_seq() {
            return;
        }
        if self.q().state == 1 {
            self.stop_sequence();
        }
        let q = self.q();
        q.reset();
        q.at = q.evnt + 8;
        q.state = 1;
        q.started = true;
    }

    /// The game's sequence stopped (360F): its queued notes ended, its
    /// locks and holds undone.
    pub fn stop_sequence(&mut self) {
        if !self.game_seq() || !self.handles[self.cur] || self.q().state != 1 {
            return;
        }
        for i in 0..32 {
            let ch = self.q().q_ch[i];
            if ch == 0xFF {
                continue;
            }
            self.q().q_ch[i] = 0xFF;
            let key = self.q().q_key[i];
            let phys = self.q().map[ch as usize & 0xF];
            self.m[NOTES + phys as usize] = self.m[NOTES + phys as usize].wrapping_sub(1);
            self.send(0x80 | phys, key, 0);
        }
        self.q().pending = 0;
        self.end_of_track();
        self.q().state = 0;
    }

    /// The game's sequence released (3536): its handle freed, or at its
    /// end when it's playing.
    pub fn release_sequence(&mut self) {
        if !self.game_seq() {
            return;
        }
        if !self.handles[self.cur] {
            return;
        }
        if self.q().state == 1 {
            self.q().release = true;
        } else {
            self.handles[self.cur] = false;
        }
    }

    /// The game's sequence's volume (38ED): at once, or ramped over `ms`.
    pub fn set_volume(&mut self, volume: u16, ms: u16) {
        if !self.game_seq() {
            return;
        }
        let q = self.q();
        q.volume_to = volume;
        if ms == 0 {
            q.volume = volume;
            self.apply_volume();
            return;
        }
        let diff = (q.volume_to as i16).wrapping_sub(q.volume as i16).unsigned_abs();
        if diff == 0 {
            return;
        }
        let p = (10 * ms as u32) / diff as u32;
        q.volume_period = if p == 0 { 1 } else { p };
        q.volume_acc = 0;
    }

    /// A controller set through AIL (39AD), channel 1..16.
    pub fn set_controller(&mut self, ch: u8, c: u8, v: u8) {
        if self.game_seq() {
            self.controller(ch.wrapping_sub(1), c, v);
        }
    }

    /// 2A27: the channels' cached volumes sent scaled by the sequence's.
    fn apply_volume(&mut self) {
        for ch in 0..16 {
            let v = self.q().cc[ch];
            if v == 0xFF {
                continue;
            }
            let s = (v as u32 * self.q().volume as u32 / 100).min(0x7F) as u8;
            self.sb(CC + ch, s);
            if self.b(LOCKS + ch) & 0x80 == 0 {
                let phys = self.q().map[ch];
                self.send(0xB0 | phys, 7, s);
            }
        }
    }

    /// 2891: at the end (or a stop), the sustains, locks, protections and
    /// priorities the sequence left on undone (by its own channel
    /// numbers).
    fn end_of_track(&mut self) {
        for ch in 0..16u8 {
            let c = ch as usize;
            if (self.q().cc[0x40 + c] as i8) >= 0x40 {
                self.sb(CC + 0x40 + c, 0);
                self.send(0xB0 | ch, 0x40, 0);
            }
            if (self.q().cc[0x60 + c] as i8) >= 0x40 {
                self.flush_channel(ch);
                let phys = self.q().map[c];
                self.unlock(phys.wrapping_add(1));
                self.q().map[c] = ch;
            }
            if (self.q().cc[0x70 + c] as i8) >= 0x40 {
                self.m[LOCKS + c] &= 0xBF;
            }
            if (self.q().cc[0x80 + c] as i8) >= 0x40 {
                self.send(0xB0 | ch, 0x70, 0);
            }
        }
    }

    /// 27BB: the channel's queued notes ended now, in every registered
    /// sequence (all the one state block).
    fn flush_channel(&mut self, ch: u8) {
        for h in 0..HANDLES {
            if self.handles[h] && self.q().pending != 0 {
                self.flush_queue(ch);
            }
        }
    }

    fn flush_queue(&mut self, ch: u8) {
        for i in 0..32 {
            if self.q().q_ch[i] != ch {
                continue;
            }
            self.q().q_ch[i] = 0xFF;
            let key = self.q().q_key[i];
            let phys = self.q().map[ch as usize & 0xF];
            self.m[NOTES + phys as usize] = self.m[NOTES + phys as usize].wrapping_sub(1);
            self.send(0x80 | phys, key, 0);
            let q = self.q();
            q.pending = q.pending.wrapping_sub(1);
        }
    }

    /// 3A1C: a physical channel 2..9 locked for the sequence (one not
    /// locked or protected with the fewest notes, else one only not
    /// locked), its sustain lifted and notes ended; 1..16, or 0 for none.
    fn lock(&mut self) -> u16 {
        let mut least = 0xFFu8;
        let mut pick = None;
        for mask in [0xC0u8, 0x80] {
            for di in (1..=8).rev() {
                if self.b(LOCKS + di) & mask == 0 && self.b(NOTES + di) < least {
                    least = self.b(NOTES + di);
                    pick = Some(di);
                }
            }
            if pick.is_some() {
                break;
            }
        }
        let Some(si) = pick else { return 0 };
        self.send(0xB0 | si as u8, 0x40, 0);
        self.flush_channel(si as u8);
        self.sb(NOTES + si, 0);
        self.m[LOCKS + si] |= 0x80;
        si as u16 + 1
    }

    /// 3A9F: a locked channel (1..16) given back, its state restored from
    /// the caches.
    fn unlock(&mut self, n: u8) {
        let si = n.wrapping_sub(1) as usize & 0xFF;
        if si >= 16 || self.b(LOCKS + si) & 0x80 == 0 {
            return;
        }
        self.m[LOCKS + si] &= 0x7F;
        self.sb(NOTES + si, 0);
        let st = 0xB0 | si as u8;
        self.send(st, 0x40, 0);
        self.send(st, 0x7B, 0);
        for k in 0..9 {
            let v = self.b(CC + si + 16 * k);
            if v != 0xFF {
                let c = self.b(CTRLS + k);
                self.send(st, c, v);
            }
        }
        let p = self.b(C_PROG + si);
        if p != 0xFF {
            self.send(0xC0 | si as u8, p, 0);
        }
        let (l, h) = (self.b(C_BEND_L + si), self.b(C_BEND_H + si));
        if l != 0xFF && h != 0xFF {
            self.send(0xE0 | si as u8, l, h);
        }
    }

    /// A controller of the sequence's (2A86): cached, the XMIDI ones
    /// (FOR/NEXT loops 116/117, locks 110/111, the indirect prefix 115)
    /// acted on, the others sent, the volume scaled by the sequence's.
    fn controller(&mut self, ch: u8, c: u8, v: u8) {
        let ci = ch as usize & 0xF;
        let mut v = v;
        if self.q().indirect[ci] != 0xFF {
            // The game registers no array of values (a null pointer):
            // the original reads the interrupt table's bytes.
            self.q().indirect[ci] = 0xFF;
            v = 0;
        }
        let row = self.b(CTRL_ROW + c as usize);
        if row != 0xFF {
            let r = row.wrapping_add(ch) as usize;
            self.sb(CC + r, v);
            if let Some(x) = self.q().cc.get_mut(r) {
                *x = v;
            }
        }
        match c {
            7 => {
                let vol = self.q().volume;
                if vol != 100 {
                    v = (v as u32 * vol as u32 / 100).min(0x7F) as u8;
                    self.sb(CC + ci, v);
                }
            }
            0x76 | 0x77 => return,
            0x74 => {
                let q = self.q();
                if let Some(i) = (0..4).find(|&i| q.loop_count[i] == 0xFFFF) {
                    q.loop_count[i] = v as u16;
                    q.loop_at[i] = q.at;
                }
                return;
            }
            0x75 => {
                if (v as i8) >= 0x40 {
                    let q = self.q();
                    if let Some(i) = (0..4).rev().find(|&i| q.loop_count[i] != 0xFFFF) {
                        if q.loop_count[i] != 0 {
                            q.loop_count[i] -= 1;
                            if q.loop_count[i] == 0 {
                                q.loop_count[i] = 0xFFFF;
                                return;
                            }
                        }
                        q.at = q.loop_at[i];
                    }
                }
                return;
            }
            0x6F => {
                self.m[LOCKS + ci] |= 0x40;
                if (v as i8) < 0x40 {
                    self.m[LOCKS + ci] &= 0xBF;
                }
                return;
            }
            0x6E => {
                if (v as i8) >= 0x40 {
                    let r = self.lock().wrapping_sub(1);
                    self.q().map[ci] = if r == 0xFFFF { ch } else { r as u8 };
                } else {
                    self.flush_channel(ch);
                    let phys = self.q().map[ci];
                    self.unlock(phys.wrapping_add(1));
                    self.q().map[ci] = ch;
                }
                return;
            }
            0x73 => {
                self.q().indirect[ci] = v;
                return;
            }
            _ => {}
        }
        if self.b(LOCKS + ci) & 0x80 == 0 {
            let phys = self.q().map[ci];
            self.send(0xB0 | phys, c, v);
        }
    }

    /// One service of the driver's timer (2F0A, 120 Hz): the tempo's
    /// ticks due run (each ending the queued notes whose time is up, then
    /// reading the events due), then the tempo and volume ramps step.
    pub fn serve(&mut self) {
        for h in 0..HANDLES {
            if self.handles[h] && self.seq.as_ref().is_some_and(|q| q.state == 1) {
                self.cur = h;
                self.serve_sequence();
            }
        }
    }

    fn serve_sequence(&mut self) {
        let q = self.q();
        q.tempo_acc = q.tempo_acc.wrapping_add(q.tempo);
        loop {
            let ax = self.q().tempo_acc.wrapping_sub(100);
            if (ax as i16) < 0 {
                break;
            }
            self.q().tempo_acc = ax;
            if self.q().pending != 0 {
                self.expire();
            }
            let q = self.q();
            q.delta = q.delta.wrapping_sub(1);
            if q.delta <= 0 && !self.events() {
                // Ended: released now if that was asked while it played.
                if self.q().release {
                    self.handles[self.cur] = false;
                }
                return;
            }
        }
        let q = self.q();
        if q.tempo != q.tempo_to {
            let (cur, to) = (q.tempo, q.tempo_to);
            let n = ramp(&mut q.tempo_ramp, q.tempo_period);
            if n != 0 {
                q.tempo = step(cur, to, n);
            }
        }
        let q = self.q();
        if q.volume != q.volume_to {
            let (cur, to) = (q.volume, q.volume_to);
            let n = ramp(&mut q.volume_acc, q.volume_period);
            if n != 0 {
                q.volume = step(cur, to, n);
                self.apply_volume();
            }
        }
    }

    /// 2FE8: a tick off each queued note, those at their end sent their
    /// note-offs.
    fn expire(&mut self) {
        for i in 0..32 {
            let ch = self.q().q_ch[i];
            if ch == 0xFF {
                continue;
            }
            let q = self.q();
            q.q_left[i] = q.q_left[i].wrapping_sub(1);
            if q.q_left[i] >= 0 {
                continue;
            }
            q.q_ch[i] = 0xFF;
            let key = q.q_key[i];
            let phys = q.map[ch as usize & 0xF];
            self.m[NOTES + phys as usize] = self.m[NOTES + phys as usize].wrapping_sub(1);
            self.send(0x80 | phys, key, 0);
            let q = self.q();
            q.pending = q.pending.wrapping_sub(1);
            if q.pending == 0 {
                return;
            }
        }
    }

    /// 3073: the events due, up to the next delay (true) or until the
    /// sequence stops (false).
    fn events(&mut self) -> bool {
        loop {
            let q = self.q();
            let at = q.at;
            let st = q.byte(at);
            if st < 0x80 {
                q.at += 1;
                q.delta = st as i16;
                return true;
            }
            let ch = st & 0xF;
            let ci = ch as usize;
            let (d1, d2) = (q.byte(at + 1), q.byte(at + 2));
            let ty = st & 0xF0;
            let len = match ty {
                0xF0 if ch == 0xF => self.meta(),
                0xF0 => {
                    let (n, k) = q.vlq(at + 1);
                    (n as usize).wrapping_add(1 + k)
                }
                0xE0 => {
                    q.bend_lo[ci] = d1;
                    q.bend_hi[ci] = d2;
                    self.sb(C_BEND_L + ci, d1);
                    self.sb(C_BEND_H + ci, d2);
                    self.channel_send(ty, ch, d1, d2);
                    3
                }
                0xD0 => {
                    self.channel_send(ty, ch, d1, d2);
                    2
                }
                0xC0 => {
                    q.program[ci] = d1;
                    self.sb(C_PROG + ci, d1);
                    self.channel_send(ty, ch, d1, d2);
                    2
                }
                0xB0 => {
                    self.controller(ch, d1, d2);
                    3
                }
                0xA0 => {
                    self.channel_send(ty, ch, d1, d2);
                    3
                }
                _ => self.seq_note(),
            };
            let q = self.q();
            q.at = q.at.wrapping_add(len);
            if q.state != 1 {
                return false;
            }
        }
    }

    /// 3119: a channel message sent on the channel's physical channel
    /// unless the channel is locked.
    fn channel_send(&mut self, ty: u8, ch: u8, d1: u8, d2: u8) {
        if self.b(LOCKS + ch as usize) & 0x80 == 0 {
            let phys = self.q().map[ch as usize];
            self.send(ty | phys, d1, d2);
        }
    }

    /// 2C69: a note-on with its duration: queued for its note-off (over
    /// the first entry when the queue is full, which then never ends) and
    /// sent. Its length.
    fn seq_note(&mut self) -> usize {
        let q = self.q();
        let at = q.at;
        let ch = q.byte(at) & 0xF;
        let (key, vel) = (q.byte(at + 1), q.byte(at + 2));
        let (dur, k) = q.vlq(at + 3);
        if self.b(LOCKS + ch as usize) & 0x80 == 0 {
            let q = self.q();
            let i = match q.q_ch.iter().position(|&c| c == 0xFF) {
                Some(i) => {
                    q.pending += 1;
                    i
                }
                None => 0,
            };
            q.q_ch[i] = ch;
            q.q_key[i] = key;
            q.q_left[i] = dur.wrapping_sub(1) as i32;
            let phys = q.map[ch as usize];
            self.m[NOTES + phys as usize] = self.m[NOTES + phys as usize].wrapping_add(1);
            self.send(0x90 | phys, key, vel);
        }
        3 + k
    }

    /// 2D21: a meta event (the end of the track ends the sequence); its
    /// length.
    fn meta(&mut self) -> usize {
        let q = self.q();
        let at = q.at;
        let t = q.byte(at + 1);
        let (n, k) = q.vlq(at + 2);
        let len = (n as usize).wrapping_add(2 + k);
        if t == 0x2F {
            self.end_of_track();
            self.q().state = 2;
        }
        len
    }

    /// The writes since the last call.
    pub fn take(&mut self) -> Vec<Write> {
        std::mem::take(&mut self.writes)
    }

    /// A song started as the game starts one (2F9F:001F): registered,
    /// its timbres installed from the timbre library (all or nothing: a
    /// missing one leaves it unstarted, false), controller 7 at 127 on
    /// every channel, started, at `volume` (D3A).
    pub fn play(&mut self, xmi: Vec<u8>, gtl: &[u8], volume: u8) -> bool {
        if !self.register(xmi, 0) {
            return false;
        }
        loop {
            let r = self.timbre_request();
            if r == 0xFFFF {
                break;
            }
            let (bank, patch) = ((r >> 8) as u8, r as u8);
            match gtl_timbre(gtl, bank, patch) {
                Some(t) => self.install_timbre(bank, patch, &t),
                None => return false,
            }
        }
        for ch in 1..=16 {
            self.set_controller(ch, 7, 0x7F);
        }
        self.start();
        self.set_volume(volume as u16, 0);
        true
    }
}

/// A ramp's accumulator stepped by a service (8.3 ms, in tenths): the
/// periods it has run.
fn ramp(acc: &mut u32, period: u32) -> u16 {
    let mut a = acc.wrapping_add(0x53) as i32;
    let mut n = 0u16;
    while a >= period as i32 {
        a -= period as i32;
        n = n.wrapping_add(1);
    }
    *acc = a as u32;
    n
}

/// `cur` moved `n` towards `to`, not past it.
fn step(cur: u16, to: u16, n: u16) -> u16 {
    let (c, t, n) = (cur as i16, to as i16, n as i16);
    if c < t {
        let v = c.wrapping_add(n);
        if v <= t { v as u16 } else { to }
    } else {
        let v = c.wrapping_sub(n);
        if v >= t { v as u16 } else { to }
    }
}

/// A change of song in `opl_stream`: the next song's file, the service
/// the fade starts after and the one the stop and the next start come
/// after; without a fade the next starts with the first playing on.
#[cfg(test)]
pub struct Then<'a> {
    pub xmi: &'a [u8],
    pub fade: Option<u32>,
    pub stop: u32,
}

/// The register writes of `services` driver services of a song started as
/// the game starts one, each with the number of services before it (0
/// for the start's): what harness/gen_fmdrv.py records from the original.
/// With `then`, the song changes as the game changes songs (2F9F:0438:
/// the volume ramped to 0 over 800 ms, the sequence stopped and
/// released; then 001F).
#[cfg(test)]
pub fn opl_stream(image: &[u8], gtl: &[u8], xmi: &[u8], volume: u8, services: u32, then: Option<Then>) -> Vec<(u32, Write)> {
    let Some(mut d) = Driver::new(image) else { return Vec::new() };
    d.take();
    let mut out = Vec::new();
    d.play(xmi.to_vec(), gtl, volume);
    for s in 0..services {
        if let Some(t) = &then {
            if Some(s) == t.fade {
                d.set_volume(0, 800);
            }
            if s == t.stop {
                if t.fade.is_some() {
                    d.stop_sequence();
                    d.release_sequence();
                }
                d.play(t.xmi.to_vec(), gtl, volume);
            }
        }
        out.extend(d.take().into_iter().map(|w| (s, w)));
        d.serve();
    }
    out.extend(d.take().into_iter().map(|w| (services, w)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::Archive;

    fn game() -> Option<(std::path::PathBuf, Archive)> {
        let exe = std::env::var("ARENA_EXE").ok()?;
        let dir = std::path::Path::new(&exe).parent()?.to_path_buf();
        dir.join("GLOBAL.BSA").exists().then(|| (dir.clone(), Archive::open(&dir)))
    }

    fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
        b.into_iter().fold(0xcbf29ce484222325u64, |h, c| (h ^ c as u64).wrapping_mul(0x100000001b3))
    }

    /// Each song's writes, a window of services at a time, as the
    /// original driver made them (tests/vectors/fmdrv.txt, from
    /// harness/gen_fmdrv.py). With FMDRV_DUMP=DIR each stream is written
    /// there as the generator's dumps are (service chip reg value).
    #[test]
    fn writes_as_the_original() {
        let Some((dir, a)) = game() else { return };
        let image = std::fs::read(dir.join("SBP1FM.ADV")).expect("SBP1FM.ADV");
        let gtl = a.read("SAMPLE.AD").expect("SAMPLE.AD");
        let vectors = include_str!("../../tests/vectors/fmdrv.txt");
        let mut cases = 0;
        let mut bad = Vec::new();
        for line in vectors.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let (head, digests) = line.split_once(" | ").unwrap();
            let f: Vec<&str> = head.split_whitespace().collect();
            let (song, volume, services, window, count): (&str, u8, u32, u32, usize) =
                (f[0], f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap());
            let xmi = a.read(song).unwrap_or_else(|| panic!("{song}"));
            let next: Vec<&str> = f[5].split('@').collect();
            let next_xmi = (next.len() == 3).then(|| a.read(next[0]).unwrap_or_else(|| panic!("{}", next[0])));
            let then = next_xmi.as_deref().map(|x| Then { xmi: x, fade: next[1].parse().ok(), stop: next[2].parse().unwrap() });
            let stream = opl_stream(&image, &gtl, &xmi, volume, services, then);
            if let Ok(d) = std::env::var("FMDRV_DUMP") {
                let text: String = stream.iter().map(|&(s, (c, r, v))| format!("{s} {c} {r:02x} {v:02x}\n")).collect();
                std::fs::write(std::path::Path::new(&d).join(format!("{song}.{volume}.{services}.{}.txt", f[5])), text).unwrap();
            }
            let want: Vec<&str> = digests.split_whitespace().collect();
            let got: Vec<String> = (0..want.len() as u32)
                .map(|k| {
                    let bytes = stream.iter().filter(|w| w.0 / window == k).flat_map(|&(s, (c, r, v))| s.to_le_bytes().into_iter().chain([c, r, v]));
                    format!("{:08x}", fnv(bytes))
                })
                .collect();
            let first = (0..want.len()).find(|&k| got[k] != want[k]);
            if stream.len() != count || first.is_some() {
                bad.push(format!("{song} {volume} {}: {} writes (want {count}), first window differing {first:?}", f[5], stream.len()));
            }
            cases += 1;
        }
        assert!(cases > 0);
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
