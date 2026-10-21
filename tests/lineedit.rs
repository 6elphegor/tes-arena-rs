//! Differential tests of the typed field and the keyboard under it
//! (src/lineedit.rs) against vectors from the original code
//! (harness/gen_lineedit.py).
mod common;

use arena::lineedit::{EditKey, Field};
use arena::machine::Machine;
use arena::mem::DS;
use common::blow::{put, Stream};

fn fnv(b: impl IntoIterator<Item = u8>) -> u64 {
    b.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3))
}

const TEXT_SEG: u16 = 0x6000;
const FONT_SEG: u16 = 0x6800;
const SCREEN: u16 = 0x7000;
const HEAP: (u16, u16) = (0xA000, 0xF000);
const BUTTON: u16 = 0x100;
const KEYCODES: [u8; 24] = [
    0x01, 0x02, 0x0B, 0x0E, 0x10, 0x1C, 0x1D, 0x1E, 0x2A, 0x2C, 0x35, 0x36, 0x38, 0x39, 0x47, 0x4B, 0x4D, 0x4F, 0x53, 0x58, 0x29, 0x0C, 0x34, 0x37,
];

fn font(s: &mut Stream, op: u8) -> Vec<u8> {
    if op == 1 {
        let mut h = 2 + s.byte() % 12;
        if s.byte() < 8 {
            h = s.byte() % 2;
        }
        let mut f = vec![h];
        for _ in 0..0x60 {
            f.push(1 + s.byte() % 10);
        }
        return f;
    }
    let mut f = vec![s.byte() % 14];
    for _ in 0..0x60 {
        let v = s.byte();
        f.push(if v < 2 { 0 } else { 1 + v % 10 });
    }
    f
}

/// (in DGROUP, offset, the text with its NUL and tail, its length).
fn text(s: &mut Stream, room: u16) -> (bool, u16, Vec<u8>, u16) {
    let in_ds = s.byte() < 0x80;
    let off = 0x1044 + (s.byte() % 0x40) as u16;
    let n = s.byte() as u16 % (room + 1);
    let mut t = Vec::new();
    for _ in 0..n {
        let c = s.byte();
        t.push(if c < 0x20 { 0x20 } else { 0x21 + c % 0x5E });
    }
    t.push(0);
    let zero = s.byte() < 0x80;
    for _ in 0..0x40 {
        t.push(if zero { 0 } else { s.byte() });
    }
    (in_ds, off, t, n)
}

fn set_font(m: &mut Machine, f: &[u8]) {
    put(m, FONT_SEG, 0, f);
    m.mem.w16(DS, 0xB034, 0);
    m.mem.w16(DS, 0xB036, FONT_SEG);
}

fn changes(old: &Machine, new: &Machine, seg: u16) -> Vec<u8> {
    (0..=0xFFFFu16)
        .filter(|&o| new.mem.r8(seg, o) != old.mem.r8(seg, o))
        .flat_map(|o| [o as u8, (o >> 8) as u8, new.mem.r8(seg, o)])
        .collect()
}

fn field(m: &mut Machine, seed: u32) -> Option<Option<(u64, u64)>> {
    let mut s = Stream(seed);
    let f = font(&mut s, 0);
    let mut mx = (s.byte() % 0x30) as u16;
    if s.byte() < 0x20 {
        mx = s.word();
    }
    let (in_ds, off, t, _) = text(&mut s, mx.min(0x28));
    let x = s.word() % 320;
    let y = (s.byte() % 200) as u16;
    let mut cx = if s.byte() < 0x80 { 0 } else { s.word() % 0x180 };
    if s.byte() < 0x10 {
        cx = s.word();
        if (0x2000..0x8000).contains(&cx) {
            cx &= 0x1FFF;
        }
    }
    if in_ds {
        cx = 0;
    }
    let mut w = 1 + (s.byte() % 0x80) as u16;
    let h = 1 + (s.byte() % 0x10) as u16;
    if s.byte() < 0x10 {
        w = s.word();
    }
    let v = s.word();
    m.mem.w16(DS, 0xA902, v);
    let v = s.word();
    m.mem.w16(DS, 0xA84C, v);
    m.mem.w8(DS, 0xACD1, 0);
    if !in_ds {
        let mut ct = Vec::new();
        for _ in 0..s.byte() % 0x20 {
            let c = s.byte();
            ct.push(if c < 0x40 { 0x20 } else { 0x21 + c % 0x5E });
        }
        ct.push(0);
        put(m, DS, off, &ct);
    }
    let mut keys = Vec::new();
    for _ in 0..1 + s.byte() % 40 {
        let r = s.byte();
        let k: u16 = match r {
            0..0x60 => 0x20 + (s.byte() % 0x5F) as u16,
            0x60..0x78 => 0x08,
            0x78..0x88 => 0x7F,
            0x88..0x94 => 0x0E,
            0x94..0xA0 => 0x0F,
            0xA0..0xB8 => 0x01,
            0xB8..0xD0 => 0xFF,
            0xD0..0xD8 => 0x1B,
            0xD8..0xE0 => 0x0D,
            0xE0..0xE4 => BUTTON,
            _ => s.byte() as u16,
        };
        keys.push(if k == BUTTON { EditKey::Button } else { EditKey::Key(k as u8) });
    }
    keys.push(EditKey::Key(0x0D));
    set_font(m, &f);
    let seg = if in_ds { DS } else { TEXT_SEG };
    put(m, seg, off, &t);
    let before = m.clone();
    let field = Field { x, y, click: cx, max: mx, seg, off, w, h };
    let ended = m.line_edit(&field, &keys)?;
    assert!(ended, "the keys end with Enter");
    // The generator stubs the text's drawing (9E0A), so the end's redraw
    // (A0A0) leaves ADB6 and ADB8 as they were.
    for o in [0xADB6, 0xADB8] {
        let v = before.mem.r16(DS, o);
        m.mem.w16(DS, o, v);
    }
    let mut d = changes(&before, m, DS);
    d.extend(changes(&before, m, TEXT_SEG));
    let heap = (0..(HEAP.1 - HEAP.0) as u32 * 16).map(|i| m.mem.r8(HEAP.0 + (i >> 4) as u16, (i & 15) as u16));
    Some(Some((fnv(d), fnv(heap))))
}

fn cursor(m: &mut Machine, seed: u32) -> (u64, u64) {
    let mut s = Stream(seed);
    let f = font(&mut s, 1);
    let (in_ds, off, t, n) = text(&mut s, 0x28);
    let cur = s.byte() as u16 % (n + 3);
    let blink = if s.byte() < 0xA0 { s.word() & !7 } else { s.word() };
    let x = s.word() % 320;
    let y = (s.byte() % 200) as u16;
    let colour = s.byte();
    set_font(m, &f);
    for r in 0..200u16 {
        m.mem.w16(DS, 0xA6A0 + 2 * r, r.wrapping_mul(0x140));
    }
    m.mem.w16(DS, 0xA900, SCREEN);
    m.mem.w16(DS, 0xA83E, cur);
    m.mem.w16(DS, 0xA84C, blink);
    m.mem.w16(DS, 0xA836, x);
    m.mem.w16(DS, 0xA838, y);
    m.mem.w8(DS, 0x9288, colour);
    m.mem.w16(DS, 0xA83A, off);
    let seg = if in_ds { DS } else { TEXT_SEG };
    m.mem.w16(DS, 0xA83C, seg);
    put(m, seg, off, &t);
    let before = m.clone();
    let got = m.edit_cursor();
    if let Some(at) = got {
        m.edit_cursor_xor(SCREEN, at);
    }
    let d = changes(&before, m, DS);
    let screen = (0..=0xFFFFu16).map(|o| m.mem.r8(SCREEN, o));
    (fnv(d), fnv(screen))
}

fn keyboard(m: &mut Machine, seed: u32) -> Option<(u64, u64)> {
    let mut s = Stream(seed);
    let a = s.byte() & 0x7F;
    let b = s.byte() & 0x7F;
    for scan in [0x1D, 0x2A, 0x36, 0x38, 0x53, 0x4B, 0x4D, a, b] {
        let v = if s.byte() < 0x40 { 0xFF } else { 0 };
        m.mem.w8(DS, 0xACB5 + scan as u16, v);
    }
    let v = s.byte() % 4;
    m.mem.w8(DS, 0xAC44, v);
    let v = s.word();
    m.mem.w16(DS, 0xA902, v);
    let v = if s.byte() < 0x20 { 0xFFFF } else { 0 };
    m.mem.w16(DS, 0xACB2, v);
    let v = if s.byte() < 0x20 { 0xFFFF } else { 0 };
    m.mem.w16(DS, 0xAC46, v);
    let v = s.byte();
    m.mem.w8(DS, 0xACB4, v);
    let v = 0xAC4C + (s.byte() % 0x65) as u16;
    m.mem.w16(DS, 0xAC48, v);
    let v = 0xAC4C + (s.byte() % 0x64) as u16;
    m.mem.w16(DS, 0xAC4A, v);
    let v = (s.byte() % 0x65) as u16;
    m.mem.w16(DS, 0xACB0, v);
    for i in 0..0x64 {
        let v = s.byte();
        m.mem.w8(DS, 0xAC4C + i, v);
    }
    let mut codes = Vec::new();
    for _ in 0..1 + s.byte() % 8 {
        let r = s.byte();
        codes.push(match r {
            0..0x80 => KEYCODES[s.byte() as usize % KEYCODES.len()],
            0x80..0xC0 => KEYCODES[s.byte() as usize % KEYCODES.len()] | 0x80,
            0xC0..0xC8 => 0xF0,
            _ => s.byte(),
        });
    }
    let reads = s.byte() % 5;
    let before = m.clone();
    for c in codes {
        m.key_event(c)?;
    }
    let got: Vec<u8> = (0..reads).map(|_| m.key_read()).collect();
    Some((fnv(changes(&before, m, DS)), fnv(got)))
}

#[test]
fn matches_original() {
    let Some(img) = common::image() else { return };
    let mut base = Machine::new(&img);
    base.heap_init(HEAP.0, HEAP.1).unwrap();
    let mut n = [0; 3];
    let mut stuck = 0;
    for line in include_str!("vectors/lineedit.txt").lines() {
        let f: Vec<&str> = line.split(" | ").collect();
        assert_eq!(f[0], "lineedit", "{line}");
        let op: usize = f[1].parse().unwrap();
        let seed = u32::from_str_radix(f[2], 16).unwrap();
        let mut m = base.clone();
        let got = match op {
            0 => field(&mut m, seed).map(|r| r.unwrap()),
            1 => Some(cursor(&mut m, seed)),
            _ => keyboard(&mut m, seed),
        };
        match (f[3], got) {
            ("fault" | "hang", None) => stuck += 1,
            (want, Some((a, b))) => assert_eq!(want, format!("{a:x} {b:x}"), "{line}"),
            (want, None) => panic!("{line}: the port gave up where the original gave {want}"),
        }
        n[op] += 1;
    }
    eprintln!("{n:?} compared, {stuck} stuck");
    assert!(n.iter().all(|&c| c > 1900), "{n:?} cases");
}

/// A field opened on `text` (at DS:F800, as the original's click reads
/// DGROUP whatever the text's segment) clicked `click` pixels in,
/// every character 5 wide but A 3 and B 9, a space 4, with `fixes`.
fn opened(img: &arena::exe::Image, fixes: arena::fixes::Fixes, text: &[u8], click: u16) -> Machine {
    let mut m = Machine::new(img);
    m.fixes = fixes;
    m.heap_init(HEAP.0, HEAP.1).unwrap();
    let mut f = vec![5u8; 0x60];
    f[1] = 3;
    f[0x21] = 3;
    f[0x22] = 9;
    set_font(&mut m, &f);
    put(&mut m, DS, 0xF800, text);
    let field = Field { x: 0, y: 0, click, max: 20, seg: DS, off: 0xF800, w: 0x40, h: 8 };
    m.edit_open(&field).expect("opened");
    m
}

/// Fixed (kept bug #36), the click: the cursor goes on the character
/// clicked (on B, 3 + 9 wide, for a click 10 in), where the original
/// counts how many of A's widths fit (3); a first space puts it at 0
/// there.
#[test]
fn fixed_a_click_puts_the_cursor_on_its_character() {
    use arena::fixes::{EDIT_TEXT, Fixes};
    use arena::lineedit::var::CURSOR;
    let Some(img) = common::image() else { return };
    let cursor = |fixes: Fixes, text: &[u8], click: u16| opened(&img, fixes, text, click).mem.r16(DS, CURSOR);
    let fixed = Fixes::only(EDIT_TEXT);
    assert_eq!(cursor(Fixes::NONE, b"ABAA\0", 10), 3);
    assert_eq!(cursor(fixed, b"ABAA\0", 10), 1);
    assert_eq!(cursor(Fixes::NONE, b" AB\0", 6), 0);
    assert_eq!(cursor(fixed, b" AB\0", 6), 1, "past the space (4), on A");
    assert_eq!(cursor(fixed, b"AB\0", 100), 2, "past the end: at it");
    assert_eq!(cursor(fixed, b"AB\0", 0), 0);
}

/// Fixed (kept bug #36), typing inside the text: a NUL follows the new
/// end; the original moves the old NUL into the text and writes none, so
/// the byte after it (X) becomes the text's.
#[test]
fn fixed_typing_inside_the_text_keeps_its_end() {
    use arena::fixes::{EDIT_TEXT, Fixes};
    let Some(img) = common::image() else { return };
    let typed = |fixes: Fixes| {
        let mut m = opened(&img, fixes, b"abc\0X\0", 0);
        assert_eq!(m.edit_key(EditKey::Key(b'z')), Some(false));
        m.mem.cstr(DS, 0xF800).to_vec()
    };
    assert_eq!(typed(Fixes::NONE), b"zabcX");
    assert_eq!(typed(Fixes::only(EDIT_TEXT)), b"zabc");
}

/// Fixed (kept bug #37): with the ring full, a key pressed is down
/// itself (the original marks the last key taken down instead); and the
/// left Shift's press, or a key typed with it, leaves the right Shift's
/// state alone (the original marks it down with the left's).
#[test]
fn fixed_the_key_states_are_the_keys_own() {
    use arena::fixes::{Fixes, KEY_RING};
    use arena::lineedit::var::{FULL, KEYS, LAST};
    let Some(img) = common::image() else { return };
    let full = |fixes: Fixes| {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.key_flush();
        m.mem.w16(DS, FULL, 0xFFFF);
        m.mem.w8(DS, LAST, 0x10);
        m.key_event(0x1E).unwrap();
        (m.mem.r8(DS, KEYS + 0x10), m.mem.r8(DS, KEYS + 0x1E))
    };
    assert_eq!(full(Fixes::NONE), (0xFF, 0), "the last key's state");
    assert_eq!(full(Fixes::only(KEY_RING)), (0, 0xFF));
    let shifts = |fixes: Fixes| {
        let mut m = Machine::new(&img);
        m.fixes = fixes;
        m.key_flush();
        m.key_event(0x2A).unwrap();
        let pressed = m.mem.r8(DS, KEYS + 0x36);
        m.key_event(0x1E).unwrap();
        (pressed, m.mem.r8(DS, KEYS + 0x36), m.key_read())
    };
    let (a, b) = (shifts(Fixes::NONE), shifts(Fixes::only(KEY_RING)));
    assert_eq!((a.0, a.1), (0xFF, 0xFF), "the original's right Shift");
    assert_eq!((b.0, b.1), (0, 0));
    assert_eq!(a.2, b.2, "the key typed shifted either way");
}
