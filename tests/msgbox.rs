//! The message box (0000:965A, `Machine::message_box`) at every kind of
//! place its callers ask for, against the original's
//! (harness/gen_msgbox.py): from the game set up as the program's start
//! leaves it, what the box changed in DGROUP, the heap's blocks after,
//! where its background went and that background's record.
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/msgbox.txt");

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

fn heap_line(m: &Machine) -> String {
    let (mut off, mut seg) = (m.mem.r16(DS, 0x157), m.mem.r16(DS, 0x159));
    let mut out = Vec::new();
    while (seg | off) != 0 && out.len() < 500 {
        out.push(format!("{seg:x} {:x} {:x}", m.mem.r16(seg, off.wrapping_add(8)), m.mem.r8(seg, off.wrapping_add(0xA))));
        (seg, off) = (m.mem.r16(seg, off.wrapping_add(2)), m.mem.r16(seg, off));
    }
    out.join(" ")
}

/// The game started and set up (tests/programstart.rs's stages).
fn set_up() -> Option<Machine> {
    let image = common::image()?;
    let Ok(mut files) = Files::new(&common::game_dir()) else {
        eprintln!("skipping: no game directory");
        return None;
    };
    let mut m = Machine::new(&image);
    m.memory_init().expect("the memory");
    m.program_start(&mut files, &mut arena::start::PortStart).expect("the start");
    let frame = m.mem.r16(DS, 0x9030);
    m.mem.w16(DS, 0xCB8A, frame);
    m.mem.w8(DS, 0x1A80, 0x40);
    m.mem.w8(DS, 0x1A6C, 0x40);
    m.game_setup(&mut arena::start::SetupPort(&mut files)).expect("the set-up");
    Some(m)
}

fn hex(s: &str) -> u16 {
    u16::from_str_radix(s, 16).unwrap()
}

#[test]
fn the_boxes_as_the_original_lays_them() {
    let Some(base) = set_up() else { return };
    let pattern: Vec<u8> = (0..64000u32).map(|i| (i * 7 + 3) as u8).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for l in text.lines().filter(|l| l.starts_with("box |")) {
        let f: Vec<&str> = l.split(" | ").collect();
        let name = f[1];
        let r: Vec<&str> = f[2].split(' ').collect();
        let (ax, bx, cx, dx, di) = (hex(r[0]), hex(r[1]), hex(r[2]), hex(r[3]), hex(r[4]));
        let t: Vec<u16> = f[3].split(' ').map(hex).collect();
        let bytes: Vec<u8> = if f[4] == "-" { Vec::new() } else { (0..f[4].len()).step_by(2).map(|i| u8::from_str_radix(&f[4][i..i + 2], 16).unwrap()).collect() };
        let mut m = base.clone();
        let v = m.mem.r16(DS, t[0]);
        m.mem.w16(DS, 0xB038, v);
        let v = m.mem.r16(DS, t[1]);
        m.mem.w16(DS, 0xB036, v);
        let frame = m.mem.r16(DS, 0x9030);
        let (seg, si) = if r[5] == "ds" { (DS, 0x1044) } else { (frame, 0x8000) };
        for (i, &b) in bytes.iter().chain(std::iter::once(&0)).enumerate() {
            m.mem.w8(seg, si + i as u16, b);
        }
        let d0 = m.mem.segment(DS).to_vec();
        let got = m.message_box((seg, si), (ax, bx), cx, dx, (frame, di), (0x58, 0x2A), Some(&pattern));
        let (rdx, rax) = got.unwrap_or_else(|| panic!("{name}: the box gave up"));
        assert_eq!(format!("{rdx:x}:{rax:x}"), f[5], "{name}: where the background went");
        let d1 = m.mem.segment(DS);
        let changed: Vec<usize> = (0..0x10000).filter(|&i| d0[i] != d1[i]).collect();
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for &i in &changed {
            match runs.last_mut() {
                Some((s, k)) if *s + *k == i => *k += 1,
                _ => runs.push((i, 1)),
            }
        }
        let offs = runs.iter().map(|(s, k)| format!("{s:x}+{k:x}")).collect::<Vec<_>>().join(" ");
        assert_eq!(offs, f[6], "{name}: what the box changed in DGROUP");
        assert_eq!(format!("{:x}", fnv(changed.iter().map(|&i| d1[i]))), f[7], "{name}: DGROUP's changed bytes");
        assert_eq!(heap_line(&m), f[8], "{name}: the heap after");
        if f[9] != "-" {
            let (boff, bseg) = (m.mem.r16(DS, 0xA848), m.mem.r16(DS, 0xA84A));
            let (w, h) = (m.mem.r16(bseg, boff.wrapping_add(4)), m.mem.r16(bseg, boff.wrapping_add(6)));
            let rec: Vec<u8> = (0..8 + w as u32 * h as u32).map(|i| m.mem.r8(bseg, boff.wrapping_add(i as u16))).collect();
            if let Ok(d) = std::env::var("MSGBOX_DUMP") {
                std::fs::write(format!("{d}/{name}.bg"), &rec).unwrap();
            }
            // Wider than the screen, the head only (the generator's notes).
            let got = match f[9].strip_prefix("h:") {
                Some(_) => format!("h:{:x}", fnv(rec[..8].iter().copied())),
                None => format!("{:x}", fnv(rec)),
            };
            assert_eq!(got, f[9], "{name}: the background's record");
        }
        n += 1;
    }
    assert!(n >= 25, "{n} cases");
}

/// The mouse's words 8FC4 writes on 59C8's way (the cursor's x and y, and
/// 8FA8): the front end's (the generator leaves them out of its digest).
const MOUSE: [usize; 6] = [0x8F92, 0x8F93, 0x8F94, 0x8F95, 0x8FA8, 0x8FA9];

#[test]
fn the_message_box_shown_and_put_away_as_the_original() {
    let Some(base) = set_up() else { return };
    let pattern: Vec<u8> = (0..64000u32).map(|i| (i * 7 + 3) as u8).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let mut n = 0;
    for l in text.lines().filter(|l| l.starts_with("shown |")) {
        let f: Vec<&str> = l.split(" | ").collect();
        let name = f[1];
        let (flags, place) = f[2].rsplit_once(' ').unwrap();
        let bytes: Vec<u8> = (0..f[3].len()).step_by(2).map(|i| u8::from_str_radix(&f[3][i..i + 2], 16).unwrap()).collect();
        let mut m = base.clone();
        let v = m.mem.r16(DS, 0x1036);
        m.mem.w16(DS, 0xB038, v);
        let v = m.mem.r16(DS, 0x102C);
        m.mem.w16(DS, 0xB036, v);
        let v = (m.mem.r16(DS, 0x05E0) & !0x800) | if flags.contains("nowait") { 0x800 } else { 0 };
        m.mem.w16(DS, 0x05E0, v);
        let v = (m.mem.r16(DS, 0x05CA) & !2) | if flags.contains("nodissolve") { 2 } else { 0 };
        m.mem.w16(DS, 0x05CA, v);
        m.mem.w8(DS, 0x05E6, hex(place) as u8);
        // The cursor where the harness's mouse is (A0h, 64h).
        m.mem.w16(DS, 0x8F92, 0xA0);
        m.mem.w16(DS, 0x8F94, 0x64);
        for (i, &b) in bytes.iter().chain(std::iter::once(&0)).enumerate() {
            m.mem.w8(DS, 0x1044 + i as u16, b);
        }
        let d0 = m.mem.segment(DS).to_vec();
        m.message_shown((DS, 0x1044), Some(&pattern)).unwrap_or_else(|| panic!("{name}: the box gave up"));
        let d1 = m.mem.segment(DS);
        let mut want = vec![false; 0x10000];
        for run in f[4].split_whitespace() {
            let (s, k) = run.split_once('+').unwrap();
            let (s, k) = (usize::from_str_radix(s, 16).unwrap(), usize::from_str_radix(k, 16).unwrap());
            want[s..s + k].iter_mut().for_each(|x| *x = true);
        }
        let extra: Vec<String> = (0..0x10000).filter(|&i| d0[i] != d1[i] && !want[i]).map(|i| format!("{i:04x}")).collect();
        let missing: Vec<String> = (0..0x10000).filter(|&i| want[i] && d0[i] == d1[i] && !MOUSE.contains(&i)).map(|i| format!("{i:04x}")).collect();
        assert!(extra.is_empty() && missing.is_empty(), "{name}: changed here only {extra:?}; there only {missing:?}");
        let digest = fnv((0..0x10000).filter(|&i| want[i] && !MOUSE.contains(&i)).map(|i| d1[i]));
        assert_eq!(format!("{digest:x}"), f[5], "{name}: DGROUP's changed bytes");
        assert_eq!(heap_line(&m), f[6], "{name}: the heap after");
        assert_eq!(format!("{:x}", fnv((0..0x20).flat_map(|b| m.ems_bank(b)))), f[7], "{name}: the EMS banks after");
        n += 1;
    }
    assert!(n >= 6, "{n} cases");
}
