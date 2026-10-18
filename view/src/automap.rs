//! The automap screen as the original draws it (0000:2CFE, the map's
//! button with the left button, 2542:045A; the drawing 0000:2F10).
//!
//! AUTOMAP.IMG in its own colours, then the place's name in FONT_A
//! centred on (79h, 17h) in 18h over a shadow in 74h at (7Bh, 19h) (not
//! in a dungeon, 5E6 3). Then 60 x 42 squares of 3 x 3 pixels from (25,
//! 40): the square under column k of a row is (left + 3Bh - k): the map
//! is drawn with x falling to the right. Each square's colour (303C; in
//! the wilderness 3161) comes from the walls and floors planes and from
//! how well it was seen (the automap's bytes, EMS bank 11h: 0, 7, 0Eh,
//! 15h), the player's square an arrow by the heading (23F1:0219). In a
//! city, a dungeon or an interior a second pass (23F1:02D1) blends each
//! square's pixels with its neighbours'. Then the notes (0000:32FE, not
//! where 5E6 is 4 nor in the wilderness; `draw_notes`).
//!
//! The screen's centre is the game's (src/automap.rs: 126C/126E, made by
//! `automap_view_open` and the buttons).

use crate::hud;
use arena::machine::Machine;
use arena::mem::DS;


/// The five buttons (66CA: x0, y0, x1, y1) and the exit (2E30).
pub fn button_at(x: i32, y: i32) -> Option<u16> {
    const B: [(i32, i32, i32, i32); 5] = [(263, 24, 281, 38), (262, 58, 280, 72), (282, 42, 300, 56), (242, 41, 260, 55), (260, 41, 282, 56)];
    B.iter().position(|&(x0, y0, x1, y1)| x >= x0 && x <= x1 && y >= y0 && y <= y1).map(|i| i as u16)
}

/// Whether (x, y) is on the exit (2E30: F0h..120h, A4h..BBh).
pub fn exit_at(x: i32, y: i32) -> bool {
    (0xF0..=0x120).contains(&x) && (0xA4..=0xBB).contains(&y)
}

/// The automap screen over the layer (`open` when it opens; the buttons
/// change its centre).
pub fn draw(screen: &mut hud::Screen, m: &mut Machine) {
    let Some(pics) = crate::sheets::loaded() else { return };
    let mut page = vec![0u8; 0x10000];
    let mut pal = [[0u8; 4]; 256];
    if let Some((img, p)) = &pics.automap {
        pal = *p;
        for r in 0..img.h.min(200) {
            for c in 0..img.w.min(320) {
                page[r * 320 + c] = img.px[r * img.w + c];
            }
        }
    }
    // 0000:2F20..2F47: the squares put in the page by the engine (the
    // seen bytes: EMS bank 11h, mapped at 2F1A).
    let seen = m.ems_bank(arena::automap::var::BANK);
    m.automap_squares(&|o| seen[o as usize], &mut page);
    screen.set_palette(pal);
    screen.fill(0, 0, hud::W as i32, hud::H as i32, 0, 0xFF);
    // (Colour 0 is see-through to `blit`: the fill under it is colour 0.)
    screen.blit(0, 0, 320, 200, &page[..320 * 200], 199);
    if m.mem.r8(DS, 0x05E6) != 3 {
        if let Some(f) = &pics.font_a {
            let name = String::from_utf8_lossy(m.mem.cstr(DS, 0x0A22)).into_owned();
            let half = f.width(&name) as i32 / 2;
            screen.text_plain(f, 0x7B - half, 0x19, &name, 0x74);
            screen.text_plain(f, 0x79 - half, 0x17, &name, 0x18);
        }
    }
}

/// The automap's pen (POINTER.IMG) as 0000:36B6 draws it at the mouse
/// (x, y): its bottom left there (y less its height), colour 0 see-through
/// (5A6A). (36B6 pulls a copy of that place back inside the clip, A918,
/// A91A, only for the background it saves, 8F2A; the pen is drawn where
/// it was, 371E.)
pub fn draw_pointer(screen: &mut hud::Screen, x: i32, y: i32) {
    let Some(pics) = crate::sheets::loaded() else { return };
    let Some(p) = &pics.pointer else { return };
    screen.blit(x, y - p.h as i32, p.w, p.h, &p.px, hud::H as i32 - 1);
}

/// The game's colours (the window's DAC) read as 0000:6E3C reads them.
struct Colours([[u8; 4]; 256]);

impl arena::mapnotes::NoteHooks for Colours {}

impl arena::automapscreen::AutomapScreenHooks for Colours {
    fn palette(&mut self, m: &mut Machine, di: u16) {
        for (i, c) in self.0.iter().enumerate() {
            for (k, v) in c.iter().take(3).enumerate() {
                m.mem.w8(DS, di.wrapping_add((i * 3 + k) as u16), v >> 2);
            }
        }
    }
}

/// The automap screen opened (0000:2CFE..2DB4, `automap_screen_begin`:
/// the screen saved, `dac` the colours it reads, its fonts (FONT_S.DAT
/// whole at A86C, the notes' hit test measuring in it), pictures and
/// buffer, the map drawn into the page) and its loop's first turn, the
/// mouse at `at` (`automap_screen_turn`).
pub fn open(engine: &mut arena::engine::Engine, dac: [[u8; 4]; 256], at: (i32, i32)) {
    let e = engine;
    if e.m.automap_screen_begin(&mut e.files, &mut Colours(dac)).is_none() {
        eprintln!("the automap screen did not start: {}", arena::giveup::describe());
    }
    e.m.automap_screen_turn(at.0 as u16, at.1 as u16, true);
}

/// A turn of the screen's loop, the mouse at `at` (a click's).
pub fn turn(m: &mut Machine, at: (i32, i32)) {
    m.automap_screen_turn(at.0 as u16, at.1 as u16, false);
}

/// The map drawn again after a button or a note (2E75..2E78, 2EA1..2EA4).
pub fn redraw(engine: &mut arena::engine::Engine) {
    let e = engine;
    e.m.automap_screen_draw(&mut e.files, &mut Colours([[0; 4]; 256]));
}

/// The automap screen left (0000:2EAA..2F0F, `automap_screen_end`): its
/// fonts and blocks freed, the game's font (102C) the current one again.
pub fn close(engine: &mut arena::engine::Engine) {
    let e = engine;
    e.m.automap_screen_end(&mut e.files);
}

/// Whether the screen has notes here: not where 5E6 is 4, nor in the
/// wilderness (2F52..2F67 for the drawing, 2E86..2E9D for the click).
pub fn notes_here(m: &Machine) -> bool {
    let wild = m.mem.r16(DS, 0xC398) & 4 != 0 && m.mem.r16(DS, 0xC39A) & 0x200 != 0;
    m.mem.r8(DS, 0x05E6) != 4 && !wild
}

/// The notes over the automap screen (0000:32FE, `notes_draw`, run on a
/// copy: it reads the file and moves its state), each text at its point
/// in colour 18h, in FONT_S.
pub fn draw_notes(screen: &mut hud::Screen, engine: &arena::engine::Engine) {
    struct Collect(Vec<(u16, u16, String)>);
    impl arena::mapnotes::NoteHooks for Collect {
        fn draw_text(&mut self, m: &mut Machine, x: u16, y: u16, si: u16) {
            self.0.push((x, y, String::from_utf8_lossy(m.mem.cstr(DS, si)).into_owned()));
        }
    }
    if !notes_here(&engine.m) {
        return;
    }
    let Some(pics) = crate::sheets::loaded() else { return };
    let Some(f) = pics.small.as_ref().or(pics.teeny.as_ref()) else { return };
    let (mut m, mut fs) = (engine.m.clone(), engine.files.clone());
    let mut c = Collect(Vec::new());
    m.notes_draw(&mut fs, &mut c);
    for (x, y, t) in c.0 {
        screen.text_plain(f, x as i16 as i32, y as i16 as i32, &t, 0x18);
    }
}
