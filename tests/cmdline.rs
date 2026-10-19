//! The command line and the CD-ROM check (src/cmdline.rs: `argv_split`,
//! 0000:0118; `command_line`, 1194:066A; `cd_check`, 21EE:0314) against
//! the original's (harness/gen_cmdline.py): what each case changed in
//! DGROUP, byte for byte, the PSP after, the carry and what was handed on.
mod common;

use arena::cmdline::{CdHooks, CommandHooks};
use arena::machine::Machine;
use arena::mem::DS;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/cmdline.txt");
/// The cases' PSP and environment (the generator's).
const PSP: u16 = 0x8F00;
const ENV: u16 = 0x8E00;

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// The changed DGROUP bytes as the generator writes them: runs, digest.
fn runs_digest(d0: &[u8], d1: &[u8]) -> (String, String) {
    let changed: Vec<usize> = (0..0x10000).filter(|&i| d0[i] != d1[i]).collect();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &i in &changed {
        match runs.last_mut() {
            Some((s, n)) if *s + *n == i => *n += 1,
            _ => runs.push((i, 1)),
        }
    }
    let r = runs.iter().map(|(s, n)| format!("{s:x}+{n:x}")).collect::<Vec<_>>().join(" ");
    (if r.is_empty() { "-".into() } else { r }, format!("{:x}", fnv(changed.iter().map(|&i| d1[i]))))
}

#[derive(Default)]
struct Log(Vec<String>);

impl Log {
    fn name(m: &Machine, at: u16) -> String {
        m.mem.cstr(DS, at).iter().take(80).map(|b| format!("{b:02x}")).collect()
    }
    fn text(&self) -> String {
        if self.0.is_empty() { "-".into() } else { self.0.join(",") }
    }
}

impl CommandHooks for Log {
    fn ail_startup(&mut self, _m: &mut Machine) {
        self.0.push("startup".into());
    }
    fn sound_driver(&mut self, m: &mut Machine, name: u16) {
        self.0.push(format!("sound:{}", Log::name(m, name)));
    }
    fn music_driver(&mut self, m: &mut Machine, name: u16) {
        self.0.push(format!("music:{}", Log::name(m, name)));
    }
    fn video_restore(&mut self, _m: &mut Machine) {
        self.0.push("video".into());
    }
    fn print(&mut self, _m: &mut Machine, at: u16) {
        self.0.push(format!("print:{at:x}"));
    }
}

struct Cd {
    log: Log,
    drive: u8,
    cds: u16,
    first: u8,
    ax: u16,
    bx: u16,
}

impl CdHooks for Cd {
    fn current_drive(&mut self, _m: &mut Machine) -> u8 {
        self.drive
    }
    fn mscdex(&mut self, _m: &mut Machine) -> (u16, u8) {
        // No MSCDEX leaves BX 0 (and CX as it was: 0, the call's).
        if self.cds == 0 { (0, 0) } else { (self.cds, self.first) }
    }
    fn is_cd(&mut self, _m: &mut Machine, _drive: u16) -> (u16, u16) {
        (self.ax, self.bx)
    }
    fn print(&mut self, _m: &mut Machine, at: u16) {
        self.log.0.push(format!("print:{at:x}"));
    }
}

#[test]
fn the_command_line_as_the_original() {
    let Some(image) = common::image() else { return };
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f[0] != "args" {
            continue;
        }
        let tail = unhex(f[2]);
        let mut m = Machine::new(&image);
        for i in 0..0x100u16 {
            m.mem.w8(PSP, i, 0);
        }
        m.mem.w8(PSP, 0x80, tail.len() as u8);
        for (i, &b) in tail.iter().enumerate() {
            m.mem.w8(PSP, 0x81 + i as u16, b);
        }
        m.mem.w8(PSP, 0x81 + tail.len() as u16, b'\r');
        m.mem.w16(DS, arena::cmdline::var::PSP, PSP);
        let d0 = m.mem.segment(DS).to_vec();
        m.argv_split();
        let mut log = Log::default();
        let carry = if m.mem.r16(DS, arena::cmdline::var::ARGC) >= 2 { (m.command_line(&mut log) as u8).to_string() } else { "-".into() };
        let (runs, digest) = runs_digest(&d0, m.mem.segment(DS));
        let psp = format!("{:x}", fnv((0..0x100u16).map(|i| m.mem.r8(PSP, i))));
        let got = [carry, log.text(), runs, digest, psp];
        assert_eq!(got.join(" | "), f[3..].join(" | "), "args case {}: tail {:?}", f[1], String::from_utf8_lossy(&tail));
        n += 1;
    }
    assert!(n > 300, "only {n} cases");
}

#[test]
fn the_cd_check_as_the_original() {
    let Some(image) = common::image() else { return };
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f[0] != "cd" {
            continue;
        }
        let a: Vec<u16> = f[2].split(' ').map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
        let env = unhex(f[3]);
        let mut m = Machine::new(&image);
        for i in 0..0x200u16 {
            m.mem.w8(ENV, i, 0);
        }
        for (i, &b) in env.iter().enumerate() {
            m.mem.w8(ENV, i as u16, b);
        }
        m.mem.w16(DS, arena::cmdline::var::ENV, ENV);
        let d0 = m.mem.segment(DS).to_vec();
        let mut cd = Cd { log: Log::default(), drive: a[0] as u8, cds: a[1], first: a[2] as u8, ax: a[3], bx: a[4] };
        let carry = (m.cd_check(&mut cd) as u8).to_string();
        let (runs, digest) = runs_digest(&d0, m.mem.segment(DS));
        let got = [carry, cd.log.text(), runs, digest];
        assert_eq!(got.join(" | "), f[4..].join(" | "), "cd case {}: env {:?}", f[1], String::from_utf8_lossy(&env));
        n += 1;
    }
    assert!(n > 50, "only {n} cases");
}

/// The start's DOS and BIOS, as the generator models them.
struct Start {
    version: u16,
    ticks: (u16, u16, u8),
    attr: u8,
    log: Vec<String>,
    midnight: u8,
}

impl arena::start::CStartHooks for Start {
    fn dos_version(&mut self, _m: &mut Machine) -> u16 {
        self.version
    }
    fn vector(&mut self, _m: &mut Machine, n: u8) -> u32 {
        (0x0F00 + n as u32) << 16 | (n as u32 * 0x111) & 0xFFFF
    }
    fn bios_ticks(&mut self, _m: &mut Machine) -> (u16, u16, u8) {
        self.ticks
    }
    fn screen_attribute(&mut self, _m: &mut Machine) -> u8 {
        self.attr
    }
    fn midnight_seen(&mut self, _m: &mut Machine) {
        self.midnight = 1;
    }
    fn vectors_taken(&mut self, _m: &mut Machine) {
        // int 21h AH 1Ah: the DTA at DGROUP:00D1.
        self.log.push("dta:3ba0:d1".into());
    }
}

/// From the stack's segment (4A8Fh) on, DGROUP isn't compared: the
/// start-up's frames, and in the harness the heap's first header.
const STACK: usize = (0x4A8F - 0x3BA0) * 16;

#[test]
fn the_start_up_to_main_as_the_original() {
    let Some(image) = common::image() else { return };
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for line in text.lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        if f[0] != "start" {
            continue;
        }
        let st: Vec<u16> = f[2].split(' ').map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
        let a: Vec<u16> = f[3].split(' ').map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
        let (tail, env) = (unhex(f[4]), unhex(f[5]));
        let mut m = Machine::new(&image);
        for i in 0..0x100u16 {
            m.mem.w8(PSP, i, 0);
        }
        m.mem.w16(PSP, 2, st[0]);
        m.mem.w16(PSP, 0x2C, ENV);
        m.mem.w8(PSP, 0x80, tail.len() as u8);
        for (i, &b) in tail.iter().enumerate() {
            m.mem.w8(PSP, 0x81 + i as u16, b);
        }
        m.mem.w8(PSP, 0x81 + tail.len() as u16, b'\r');
        for i in 0..0x200u16 {
            m.mem.w8(ENV, i, 0);
        }
        for (i, &b) in env.iter().enumerate() {
            m.mem.w8(ENV, i as u16, b);
        }
        let d0 = m.mem.segment(DS).to_vec();
        let mut start = Start { version: st[1], ticks: (st[2], st[3], st[4] as u8), attr: st[5] as u8, log: Vec::new(), midnight: 0 };
        let mut cd = Cd { log: Log::default(), drive: a[0] as u8, cds: a[1], first: a[2] as u8, ax: a[3], bx: a[4] };
        let stop = match m.c_start(PSP, 0x5A8F, &mut start) {
            None => "memory",
            Some(()) if m.main_head(&mut cd) => "quit",
            Some(()) => "main",
        };
        let midnight = start.midnight;
        let log: Vec<String> = start.log.into_iter().chain(cd.log.0).collect();
        let mut d1 = m.mem.segment(DS).to_vec();
        d1[STACK..].copy_from_slice(&d0[STACK..]);
        let (runs, digest) = runs_digest(&d0, &d1);
        let heap = if stop == "memory" {
            "-".to_string()
        } else {
            let first = m.mem.r16(DS, 0x159);
            (0..11u16).map(|i| format!("{:02x}", m.mem.r8(first, i))).collect()
        };
        let psp = format!("{:x}", fnv((0..0x100u16).map(|i| m.mem.r8(PSP, i))));
        let log = if log.is_empty() { "-".to_string() } else { log.join(",") };
        let got = [stop.to_string(), log, runs, digest, heap, psp, format!("{midnight:x}")];
        assert_eq!(got.join(" | "), f[6..].join(" | "), "start case {}", f[1]);
        n += 1;
    }
    assert!(n > 10, "only {n} cases");
}
