//! The character sheet and the logbook as the original shows them.
//!
//! - The character sheet (the portrait's button, 2542:00BE, then
//!   1194:0CF8): the body and what is worn (`doll`), CHARSTAT.IMG over
//!   its left side, the player's texts where 1194:0DE1 writes them in
//!   ARENAFNT ([102C]) in colour FEh, and PAGE2.IMG, the button to the
//!   equipment page. Its two click records (3276:52B0, 62h bytes each):
//!   (6Ah, B2h, 34h x 0Fh, key 'N') turns to the equipment page (1194:0D67
//!   -> 16BD), whose own exit comes back here; (0, AAh, 1Fh x 11h, Esc)
//!   leaves. The page is in CHARSHT.COL's colours (1194:1695).
//! - The logbook (the scroll's button, 2542:02A3, then 27E3:0088):
//!   LOGBOOK.IMG in its own colours; the slot's LOG file, each entry's " *"
//!   made a line break (27E3:0147), split at its line feeds (1194:4E54),
//!   shown 13 lines a page from (1Eh, 1Eh), 8 rows apart, in TEENYFNT
//!   ([102E]): a line with '&' in its first three bytes is a date in
//!   colour 10h, after the '&', 8 rows lower, the line after it 16 on; the
//!   others in 49h. "More..." at (10Eh, AAh) in 0Fh while pages remain.
//!   No file: "Your logbook is empty." centred on (A0h, 64h) in FONT_A,
//!   61h. Its loop (27E3:01FC: the clicks, Print, Exit, the pages' end)
//!   is the game's (`Machine::log_turn`); the pages drawn here follow it.

use crate::archive::Archive;
use crate::hud::{Font, Screen};
use crate::interface::Image;
use arena::machine::Machine;
use arena::mem::DS;
use arena::menukeys::{MenuKey, NoMenuKeyHooks};
use std::cell::RefCell;
use std::rc::Rc;

const PLAYER: u16 = 0x01A4;
const RACE_NAMES: u16 = 0x2B94;
/// The sheet's text colour (9288 at 1194:0D9E).
const SHEET_COLOUR: u8 = 0xFE;

/// The screens' pictures and fonts, read from the game's files once.
pub struct Pics {
    pub charstat: Option<Image>,
    pub page2: Option<Image>,
    pub logbook: Option<(Image, [[u8; 4]; 256])>,
    pub automap: Option<(Image, [[u8; 4]; 256])>,
    pub arena: Option<Font>,
    pub teeny: Option<Font>,
    pub font_a: Option<Font>,
    /// FONT_S.DAT, the automap's (its notes).
    pub small: Option<Font>,
    /// POINTER.IMG, the automap's pen at the mouse (0000:2D8A).
    pub pointer: Option<Image>,
    /// The equipment screen's spells page (1194:2229): its title and the
    /// button back to the items.
    pub spellbk: Option<Image>,
    pub equipb: Option<Image>,
}

thread_local! {
    static PICS: RefCell<Option<Rc<Pics>>> = const { RefCell::new(None) };
}

/// A raw 320 x 200 picture with its own palette after the pixels (flags
/// 100h: LOGBOOK.IMG, AUTOMAP.IMG), the palette's 6-bit values made 8-bit.
fn with_palette(archive: &Archive, name: &str) -> Option<(Image, [[u8; 4]; 256])> {
    let d = archive.read(name)?;
    let (img, used) = crate::interface::image(&d)?;
    let p = d.get(used..used + 0x300).or_else(|| d.get(12 + img.w * img.h..12 + img.w * img.h + 0x300))?;
    let mut pal = [[0u8; 4]; 256];
    for (i, c) in pal.iter_mut().enumerate() {
        let v = |k: usize| {
            let x = p[3 * i + k] & 0x3F;
            x << 2 | x >> 4
        };
        *c = [v(0), v(1), v(2), 255];
    }
    Some((img, pal))
}

/// The pictures, loaded the first time.
pub fn pics(archive: &Archive, m: &Machine) -> Rc<Pics> {
    PICS.with(|p| {
        p.borrow_mut()
            .get_or_insert_with(|| {
                Rc::new(Pics {
                    charstat: crate::panels::load_image(archive, m, "CHARSTAT.IMG"),
                    page2: crate::panels::load_image(archive, m, "PAGE2.IMG"),
                    logbook: with_palette(archive, "LOGBOOK.IMG"),
                    automap: with_palette(archive, "AUTOMAP.IMG"),
                    arena: Font::load(archive, "ARENAFNT.DAT"),
                    teeny: Font::load(archive, "TEENYFNT.DAT"),
                    font_a: Font::load(archive, "FONT_A.DAT"),
                    small: Font::load(archive, "FONT_S.DAT"),
                    pointer: crate::panels::load_image(archive, m, "POINTER.IMG"),
                    spellbk: crate::panels::load_image(archive, m, "SPELLBK.IMG"),
                    equipb: crate::panels::load_image(archive, m, "EQUIPB.IMG"),
                })
            })
            .clone()
    })
}

/// The pictures if loaded (the screens are opened through `pics`).
pub fn loaded() -> Option<Rc<Pics>> {
    PICS.with(|p| p.borrow().clone())
}

fn blit(screen: &mut Screen, i: &Image) {
    screen.blit(i.x, i.y, i.w, i.h, &i.px, 199);
}

/// 1194:48F5: a value with its sign, "+%u" unless it is negative (DX 0
/// here), then "%d".
fn signed(v: u16) -> String {
    if (v as i16) < 0 { (v as i16).to_string() } else { format!("+{v}") }
}

/// 1194:1027: a word / 5, signed (cwd, idiv).
fn fifth(v: u16) -> u16 {
    ((v as i16) / 5) as u16
}

/// The stats page (1194:0D8E): `doll` the body and what is worn, then
/// CHARSTAT.IMG, the texts and PAGE2.IMG.
pub fn draw_sheet(screen: &mut Screen, m: &Machine, doll: &[Image]) {
    let Some(pics) = loaded() else { return };
    screen.fill(0, 0, crate::hud::W as i32, crate::hud::H as i32, 0, 0xFF);
    for i in doll {
        blit(screen, i);
    }
    if let Some(i) = &pics.charstat {
        blit(screen, i);
    }
    if let Some(f) = &pics.arena {
        sheet_texts(screen, f, m, m.mem.r16(DS, 0x05C8) & 0x10 != 0);
    }
    if let Some(i) = &pics.page2 {
        blit(screen, i);
    }
}

/// 1194:0E19: the name, the race (the nth of 2B94) and the class
/// (2686:0645) at (Bh, 7), (Bh, 10h), (Bh, 19h), in the sheet's colour.
pub fn name_texts(screen: &mut Screen, f: &Font, m: &Machine) {
    let cstr = |seg: u16, off: u16| String::from_utf8_lossy(m.mem.cstr(seg, off)).into_owned();
    let race = m.mem.r8(DS, PLAYER + 4) as u16;
    screen.text_plain(f, 0xB, 7, &cstr(DS, PLAYER + 9), SHEET_COLOUR);
    screen.text_plain(f, 0xB, 0x10, &cstr(DS, m.skip_strings(DS, RACE_NAMES, race)), SHEET_COLOUR);
    let (cs, co) = m.class_name(DS, PLAYER);
    screen.text_plain(f, 0xB, 0x19, &cstr(cs, co), SHEET_COLOUR);
}

/// 1194:0DE1..0FD7, for the sheet and for creation's attribute screen
/// (1194:122F's redraws): `creating` is 5C8 bit 10h as the texts read it
/// (the attributes and maxima as they are, not on the 100 scale).
pub fn sheet_texts(screen: &mut Screen, f: &Font, m: &Machine, creating: bool) {
    name_texts(screen, f, m);
    let c = SHEET_COLOUR;
    let mut s = |x: i32, y: i32, t: &str| screen.text_plain(f, x, y, t, c);
    let b = |o: u16| m.mem.r8(DS, PLAYER + o) as u16;
    let w = |o: u16| m.mem.r16(DS, PLAYER + o);
    let pct = |v: u16| arena::scale::to100(v as i16) as u16;
    // 0DEC: the gold (ltoa) at (2Dh, 90h).
    s(0x2D, 0x90, &m.mem.r32(DS, 0x05C2).to_string());
    // 0E53: the attributes (+29..+30) on the 100 scale (5B07) unless
    // creating, from (1Ah, 34h), 8 rows apart, unsigned (7052).
    for i in 0..8u16 {
        let v = b(0x29 + i);
        s(0x1A, 0x34 + 8 * i as i32, &(if creating { v } else { pct(v) }).to_string());
    }
    // 0E86: health "%d/%d", now (+59; the most while creating) over the
    // most (+5B), at (2Dh, 7Fh).
    let most = w(0x5B);
    let now = if creating { most } else { w(0x59) };
    s(0x2D, 0x7F, &format!("{}/{}", now as i16, most as i16));
    // 0EA3: fatigue, (+5D >> 6) over strength + endurance, both on the
    // 100 scale (5AA9) unless creating, at (2Dh, 87h).
    let mut most = b(0x29) + b(0x2E);
    if !creating {
        most = arena::scale::to100_u(most);
    }
    let now = arena::scale::to100_u(w(0x5D) >> 6);
    s(0x2D, 0x87, &format!("{}/{}", now as i16, most as i16));
    // 0ECE: spell points, now (+66) over the most (+68), at (56h, 3Ch).
    s(0x56, 0x3C, &format!("{}/{}", w(0x66) as i16, w(0x68) as i16));
    // 0EDD: the level (+6, from 0) + 1 at (2Dh, A7h); the experience
    // (+409, ltoa) at (2Dh, 9Eh).
    s(0x2D, 0xA7, &(b(6) + 1).to_string());
    s(0x2D, 0x9E, &m.mem.r32(DS, PLAYER + 0x409).to_string());
    // 0F08: the derived numbers. The most kilos (+3B on the 100 scale,
    // 5AA9; its test against 1FEh sets CX, which the next instruction
    // overwrites) at (91h, 34h); then +39, +4F, +53 as they are.
    s(0x91, 0x34, &arena::scale::to100_u(w(0x3B)).to_string());
    s(0x56, 0x34, &signed(w(0x39)));
    s(0x56, 0x5C, &signed(w(0x4F)));
    s(0x91, 0x5C, &signed(w(0x53)));
    // 0F63..0F87: the 16 words +39..+58 each on the 100 scale (5B07), then
    // divided by 5 (1027), for the rest (the record is put back after).
    let conv = |o: u16| fifth(arena::scale::to100(w(o) as i16) as u16);
    s(0x56, 0x44, &signed(conv(0x43).wrapping_neg()));
    s(0x91, 0x4C, &signed(conv(0x47)));
    s(0x56, 0x4C, &signed(conv(0x49)));
    s(0x56, 0x64, &signed(conv(0x55)));
}

/// What a click on the stats page does (3276:52B0's records).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetClick {
    /// (6Ah, B2h) 34h x 0Fh, or 'N': the equipment page.
    Equipment,
    /// (0, AAh) 1Fh x 11h, or Esc: leave.
    Done,
}

pub fn sheet_click(x: i32, y: i32) -> Option<SheetClick> {
    let inside = |x0, y0, w, h| x >= x0 && x < x0 + w && y >= y0 && y < y0 + h;
    if inside(0x6A, 0xB2, 0x34, 0xF) {
        Some(SheetClick::Equipment)
    } else if inside(0, 0xAA, 0x1F, 0x11) {
        Some(SheetClick::Done)
    } else {
        None
    }
}

/// The logbook's pages: each line, a date or not.
#[derive(Clone, Debug, Default)]
pub struct LogBook {
    pub pages: Vec<Vec<(bool, String)>>,
    pub page: usize,
    /// No log file (or an empty one: then a blank page).
    pub missing: bool,
}

/// The log file of the working slot ("log." and the two digits of 1A6C,
/// 27E3:01E3), made into pages as 27E3:0147, 1194:4E54 and 27E3:02DA do.
pub fn open_log(m: &Machine, files: &arena::bsa::Files) -> LogBook {
    let slot = m.mem.r8(DS, 0x1A6C);
    let name = format!("LOG.{:02}", slot % 100);
    let Some(data) = files.data(name.as_bytes()) else {
        return LogBook { missing: true, ..Default::default() };
    };
    let mut t = data.to_vec();
    // 27E3:0147: each '*' a CR, the byte before it a LF.
    for i in 0..t.len() {
        if t[i] == b'*' {
            t[i] = b'\r';
            if i > 0 {
                t[i - 1] = b'\n';
            }
        }
    }
    let end = t.iter().position(|&c| c == 0).unwrap_or(t.len());
    let lines: Vec<(bool, String)> = t[..end]
        .split(|&c| c == b'\n')
        .map(|l| {
            let l: Vec<u8> = l.iter().copied().filter(|&c| c != b'\r').collect();
            match l.iter().take(3).position(|&c| c == b'&') {
                Some(k) => (true, String::from_utf8_lossy(&l[k + 1..]).into_owned()),
                None => (false, String::from_utf8_lossy(&l).into_owned()),
            }
        })
        .collect();
    let pages = lines.chunks(13).map(|c| c.to_vec()).collect();
    LogBook { pages, page: 0, missing: false }
}

pub fn draw_log(screen: &mut Screen, log: &LogBook) {
    let Some(pics) = loaded() else { return };
    if let Some((img, pal)) = &pics.logbook {
        screen.set_palette(*pal);
        screen.fill(0, 0, crate::hud::W as i32, crate::hud::H as i32, 0, 0xFF);
        blit(screen, img);
    }
    if log.missing {
        if let Some(f) = &pics.font_a {
            let t = "Your logbook is empty.";
            screen.text_plain(f, 0xA0 - f.width(t) as i32 / 2, 0x64, t, 0x61);
        }
        return;
    }
    let Some(f) = &pics.teeny else { return };
    let mut y = 0x1E;
    if let Some(page) = log.pages.get(log.page) {
        for (date, line) in page {
            if *date {
                // 27E3:035E moves the date 8 rows down, but the row is
                // popped back after it (036E) before the 8 and the date's
                // 8 more (036F, 037A): the next line 16 rows on.
                screen.text_plain(f, 0x1E, y + 8, line, 0x10);
                y += 16;
            } else {
                screen.text_plain(f, 0x1E, y, line, 0x49);
                y += 8;
            }
        }
    }
    if log.page + 1 < log.pages.len() {
        screen.text_plain(f, 0x10E, 0xAA, "More...", 0x0F);
    }
}

/// The screen a frame's menu key (2542:0100, in the frame body) or a
/// click on the bar (2542:00BA, in its input step) asks for, which the
/// window opens after that frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Taken {
    /// A bar button's (or the menu key's with its handler).
    Action(BarAction),
    /// Esc: the options screen (2542:000A).
    Options,
}

/// The interface bar's buttons (2542:06CE's table at DGROUP 5BD2: x0, y0,
/// x1, y1 and a handler in 2542), by their icons.
const BAR: [(i32, i32, i32, i32); 9] = [
    (91, 153, 114, 170),  // crossed swords: 0486
    (120, 153, 143, 170), // map: 0380
    (148, 153, 172, 170), // hand: 0279
    (178, 153, 202, 170), // staff and snake: 037C -> 04B1
    (91, 177, 114, 195),  // star: 026D
    (120, 177, 143, 195), // scroll: 02A3
    (148, 177, 172, 195), // fist: 034D
    (178, 177, 202, 195), // campfire: 01E6
    (15, 166, 53, 194),   // the portrait: 00BE
];

/// The centre of bar button `i` (for scripted clicks and tests).
pub fn bar_centre(i: usize) -> (i32, i32) {
    let (x0, y0, x1, y1) = BAR[i.min(8)];
    ((x0 + x1) / 2, (y0 + y1) / 2)
}

/// What a bar button asks of the front end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BarAction {
    /// Done in the game's state already (the weapon readied, picking armed).
    None,
    /// The map with the left button: the automap screen (0000:2CFE),
    /// opened (`automap_view_open`).
    Automap,
    /// The map with the right button: the world map (2542:03BF), not
    /// this screen's.
    WorldMap,
    /// The status box (2542:04B1): its text as lines.
    Status(Vec<Vec<crate::hud::Span>>),
    /// The spellbook (2542:026D -> 056A), with the right button's release
    /// as 7925 holds it where 056A reads it (the bar's right click; the
    /// C key's turn as the last poll left it): with it, or C held with a
    /// Shift, the last spell again (`Machine::spellbook_last`).
    Spells(bool),
    /// The logbook (2542:02A3); true by the bar's scroll button (its
    /// stack a word deeper than the L key's: 12C6).
    Log(bool),
    /// The items with an effect (2542:034D).
    Use,
    /// Camping (2542:01E6 -> 1194:424E).
    Camp,
    /// The character sheet (2542:00BE -> 1194:0CF8).
    Sheet,
}

/// Which bar button holds (x, y) of the 320 x 200 screen: the first, as
/// 06CE looks them up.
pub fn bar_button(x: i32, y: i32) -> Option<usize> {
    BAR.iter().position(|&(x0, y0, x1, y1)| x >= x0 && x <= x1 && y >= y0 && y <= y1)
}

/// The bar button at (x, y) of the 320 x 200 screen (the first that
/// holds it, as 06CE looks them up) and its action; `right` the right
/// button (7924 clear: the map's world map).
pub fn bar_click(m: &mut Machine, files: &mut arena::bsa::Files, x: i32, y: i32, right: bool) -> Option<BarAction> {
    let i = bar_button(x, y)?;
    Some(match i {
        0 => {
            // 0486: as the menu keys' 'A' (2542:0137).
            crate::interface::ready(m);
            BarAction::None
        }
        1 if right => BarAction::WorldMap,
        1 => {
            m.automap_view_open();
            BarAction::Automap
        }
        2 => {
            m.pilfer_arm();
            BarAction::None
        }
        3 => status(m),
        // 026D: unless the spellbook is barred (5CA bit 8).
        4 if m.mem.r16(DS, 0x05CA) & 8 != 0 => BarAction::None,
        4 => BarAction::Spells(right),
        5 => BarAction::Log(true),
        6 => BarAction::Use,
        7 => BarAction::Camp,
        // 00BE, as F1: a touch spell waiting is cast on the player first.
        _ => match m.portrait(files, &mut NoMenuKeyHooks).unwrap_or(MenuKey::None) {
            MenuKey::Sheet => BarAction::Sheet,
            _ => BarAction::None,
        },
    })
}

/// 2542:04B1..052B: the status box's text and the player's conditions,
/// and its box as the game's state has it (`Machine::status_shown`; BX,
/// an offset on the original's stack, 0).
fn status(m: &mut Machine) -> BarAction {
    m.status_box();
    if m.status_shown(0, None).is_none() {
        eprintln!("the status box gave up: {}", arena::giveup::describe());
    }
    let c = m.mem.r8(DS, 0x9288);
    BarAction::Status(crate::hud::lines(m.mem.cstr(DS, arena::status::var::STATUS_TEXT), c))
}

/// What the menu key the engine took (`Machine::menu_keys`, 2542:0100)
/// asks of the front end, as the bar's buttons ask it (they share the
/// handlers); None for Esc's options screen, which isn't the bar's.
pub fn menu_action(m: &mut Machine, key: MenuKey) -> Option<BarAction> {
    Some(match key {
        MenuKey::Options => return None,
        MenuKey::None | MenuKey::Ready | MenuKey::Pickpocket | MenuKey::TouchCast | MenuKey::Compass => BarAction::None,
        MenuKey::Spellbook => BarAction::Spells(m.mem.r8(DS, arena::controls::var::RELEASED_RIGHT) != 0),
        MenuKey::Status => status(m),
        MenuKey::Use => BarAction::Use,
        MenuKey::Automap => BarAction::Automap,
        MenuKey::TravelMap => BarAction::WorldMap,
        MenuKey::Sheet => BarAction::Sheet,
        MenuKey::Log => BarAction::Log(false),
        MenuKey::Camp => BarAction::Camp,
    })
}
