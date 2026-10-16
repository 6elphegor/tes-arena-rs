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

/// The table of the files loaded whole as fonts (0000:7508 puts each
/// one's segment in its first free word, 930E frees only one found
/// there): eight words.
const FONTS: u16 = 0x91AC;
/// FONT_S.DAT's segment while the screen is up (2D6C).
const FONT_S: u16 = 0xA86C;

/// The automap screen opened (0000:2CFE..2D2E, `automap_screen_open`),
/// its font made the current one.
pub fn open(engine: &mut arena::engine::Engine, archive: &crate::archive::Archive) {
    engine.m.automap_screen_open(&mut engine.files);
    select_font(&mut engine.m, archive);
}

/// The automap screen left (0000:2EAA..2EC5): FONT_S.DAT's block freed
/// (930E, only where the fonts' table holds it) and the game's font
/// (102C) the current one again (9AEA).
pub fn close(m: &mut Machine) {
    let seg = m.mem.r16(DS, FONT_S);
    if let Some(i) = (0..8).find(|i| m.mem.r16(DS, FONTS + 2 * i) == seg) {
        m.mem.w16(DS, FONTS + 2 * i, 0);
        m.heap_free(seg);
    }
    let v = m.mem.r16(DS, 0x102C);
    m.mem.w16(DS, arena::mapnotes::var::FONT + 2, v);
}

/// The automap screen's font made the current one, as the screen does
/// (0000:2D66: 7508 loads FONT_S.DAT whole into a block of its own, kept
/// at A86C and in the fonts' table; its redraw selects it, 3035 calling
/// 9AEA, which sets the far pointer B034's segment): the file put in a
/// heap block of the engine's, so the notes' hit test (0000:66F6's
/// widths) measures as the original's does. With the table full 7508
/// gives 0.
fn select_font(m: &mut Machine, archive: &crate::archive::Archive) {
    let Some(d) = archive.read("FONT_S.DAT") else { return };
    let seg = match (0..8).find(|i| m.mem.r16(DS, FONTS + 2 * i) == 0) {
        Some(i) => {
            let paras = (d.len().div_ceil(16)).min(0xFFFF) as u16;
            let Some((seg, _)) = m.heap_alloc(paras) else { return };
            for (i, &b) in d.iter().enumerate().take(0xFFFF) {
                m.mem.w8(seg, i as u16, b);
            }
            m.mem.w16(DS, FONTS + 2 * i, seg);
            seg
        }
        None => 0,
    };
    m.mem.w16(DS, FONT_S, seg);
    m.mem.w16(DS, arena::mapnotes::var::FONT, 0);
    m.mem.w16(DS, arena::mapnotes::var::FONT + 2, seg);
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
