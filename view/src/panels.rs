//! The view's screens over the game, which stop it while they are up as
//! the original's do: the inventory and the spellbook. Their rules are
//! the engine's (src/use_item.rs, src/cast.rs); this is their drawing and
//! their input.
//!
//! The inventory is the original's equipment screen (1194:23B1): EQUIP.IMG
//! (171 x 200 at 0, 0; 1194:221A) and the player's items listed over it,
//! seven lines from FIRST_LINE (1297) at x 0Eh, from row 32h, 0Bh apart
//! (1194:1DC8), coloured as the original colours them: 58h an item the
//! class can use (1194:7772), 55h one worn, A0h one it can't, 68h/65h an
//! identified magic item (flags 1 and 2), worn or not. A left click on a
//! line puts the item on or takes it off (1194:1AFB, `equip_click`); a
//! right click selects it (1296), which D drops (1194:18F6,
//! `drop_item`); U lists the items with an effect and uses the one picked
//! (24FA:07CD, `use_item`). The paper doll on the right (1194:23E5) and
//! the screen's buttons are not drawn: the right of the screen is dark.
//!
//! The spellbook lists the player's spells (the records of SPELLSG with a
//! name, 29CF:0679) with their cost at the player's level, and casts the
//! one picked (29CF:071F, `cast_spell`); a spell with a target then waits
//! for a click in the view, which aims it (`ControlHooks::cast_at_target`).
//! Its look (a panel in the game's font) is the view's, not the
//! original's.

use crate::hud::{Font, Screen, Span};
use crate::interface::Image;
use arena::machine::Machine;
use arena::mem::{DATA, DS};
use arena::use_item::var::{FIRST_LINE, LINES, NOT_OWN, OTHER_LIST, OWN_ITEMS, SELECTED, USABLE_NAMES};

/// The list's place and lines (1194:1DE9..1EA6).
pub const LIST_X: i32 = 0x0E;
pub const LIST_Y: i32 = 0x32;
pub const LINE_H: i32 = 0x0B;
pub const LIST_LINES: usize = 7;
const SLOTS: u16 = 40;

/// The original's texts (segment 3276): an item that can't be put on,
/// one a class can't (with the class's name), a second accessory of a
/// kind (with the kind's name), and dropping with nothing selected.
const CANT_EQUIP: u16 = 0x5A90;
const CLASS_CANT: u16 = 0x3BEB;
const ONLY_ONE: u16 = 0x5A74;
const DROP_WHAT: u16 = 0x5ACB;

/// An image of the game's: the codings `interface::image` reads, and
/// type 8 (3065:0D2D, LZHUF: the coded size first) decoded by the port's
/// decompressor on a copy of the machine.
pub fn load_image(archive: &crate::archive::Archive, m: &Machine, name: &str) -> Option<Image> {
    let d = archive.read(name)?;
    if let Some((img, _)) = crate::interface::image(&d) {
        return Some(img);
    }
    image_type8(m, &d)
}

/// An image of coding type 8 (its 12-byte header, then the data the
/// port's LZHUF unpacks), decoded on a copy of the machine.
pub fn image_type8(m: &Machine, d: &[u8]) -> Option<Image> {
    let le = |at: usize| u16::from_le_bytes([*d.get(at).unwrap_or(&0), *d.get(at + 1).unwrap_or(&0)]);
    let (x, y, w, h, flags, size) = (le(0), le(2), le(4) as usize, le(6) as usize, le(8), le(10) as usize);
    if flags & 0xFF != 8 || w == 0 || h == 0 || w > 320 || h > 200 {
        return None;
    }
    let data = d.get(12..12 + size)?;
    let mut c = m.clone();
    const SRC: u16 = 0x6000;
    const DST: u16 = 0x7000;
    for (i, &b) in data.iter().enumerate() {
        c.mem.w8(SRC + (i >> 4) as u16, (i & 15) as u16, b);
    }
    c.mif_decompress(SRC, 0, DST, 0, size as u16)?;
    let px = (0..w * h).map(|i| c.mem.r8(DST, i as u16)).collect();
    Some(Image { x: x as i16 as i32, y: y as i16 as i32, w, h, px })
}

/// An inventory line: the slot, its name, its colour.
#[derive(Clone, Debug)]
pub struct Line {
    pub slot: u16,
    pub name: String,
    pub colour: u8,
}

/// The player's occupied slots, named as lists name them (1194:7388) on a
/// copy of the machine (naming writes the game's name buffer).
pub fn item_lines(m: &Machine) -> Vec<Line> {
    let mut c = m.clone();
    let class = (c.mem.r8(DS, arena::character::PLAYER + arena::character::field::CLASS) & 0x1F) as u16;
    let mut out = Vec::new();
    for slot in 0..SLOTS {
        let si = arena::inventory::PLAYER_SLOTS + slot * arena::inventory::SLOT_SIZE;
        let flags = c.mem.r8(DS, si + arena::item::FLAGS);
        if flags & 0x40 == 0 {
            continue;
        }
        let worn = flags & 0x80 != 0;
        let mut colour = 0xA0;
        if c.class_can_use(DS, si, class) == Some(true) {
            colour = if worn { 0x55 } else { 0x58 };
        }
        if flags & 1 != 0 && flags & 2 != 0 {
            colour = if worn { 0x65 } else { 0x68 };
        }
        let name = c.display_name(si).map(|at| String::from_utf8_lossy(c.mem.cstr(DS, at)).into_owned()).unwrap_or_else(|| "?".into());
        out.push(Line { slot, name, colour });
    }
    out
}

/// A spell of the player's book: its index, name and cost at the
/// player's level (29CF:071F: +32h / (level + 1)).
#[derive(Clone, Debug)]
pub struct Spell {
    pub index: u8,
    pub name: String,
    pub cost: u16,
}

/// The player's spellbook (24FA:09EA): the count at player +366h, then
/// the spells' numbers from +367h, each a record of the spell files.
pub const BOOK_COUNT: u16 = arena::character::PLAYER + 0x366;
pub const BOOK: u16 = arena::character::PLAYER + 0x367;

/// The spells of the player's book, in its order.
pub fn spells(m: &Machine) -> Vec<Spell> {
    let level = m.mem.r8(DS, 0x01AA) as u16 + 1;
    (0..m.mem.r8(DS, BOOK_COUNT) as u16)
        .map(|k| {
            let i = m.mem.r8(DS, BOOK + k);
            let (seg, off) = m.spell_record(i);
            let name = String::from_utf8_lossy(m.mem.cstr(seg, off + arena::save::spell::NAME)).into_owned();
            let cost = m.mem.r16(seg, off + arena::save::spell::COST) / level;
            Spell { index: i, name, cost }
        })
        .collect()
}

/// `--do learn:N`: spell N of the spell files put in the player's book,
/// for trying the spellbook (not how the game teaches spells: the mages'
/// guild sells them).
pub fn learn(m: &mut Machine, index: u8) {
    let n = m.mem.r8(DS, BOOK_COUNT);
    m.mem.w8(DS, BOOK + n as u16, index);
    m.mem.w8(DS, BOOK_COUNT, n + 1);
}

/// A screen up over the game.
#[derive(Clone, Debug)]
pub enum Panel {
    Inventory(Inventory),
    Spellbook(Spellbook),
    /// The character sheet's stats page (`sheets`).
    Sheet,
    /// The logbook (`sheets`).
    Log(crate::sheets::LogBook),
}

/// The inventory: the use list when it is up (the names, the line
/// highlighted).
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    pub using: Option<(Vec<String>, usize)>,
    /// Opened from the character sheet (1194:0D67 -> 16BD): its exit goes
    /// back to the sheet.
    pub from_sheet: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Spellbook {
    pub first: usize,
    pub selected: usize,
}

/// The equipment screen opened on the player's own items (1194:23C7's
/// list): nothing selected, from the first line, no other list.
pub fn open_inventory(m: &mut Machine) -> Panel {
    m.mem.w8(DS, OWN_ITEMS, 1);
    m.mem.w8(DS, OTHER_LIST, 0);
    m.mem.w16(DS, NOT_OWN, 0);
    m.mem.w8(DS, SELECTED, 0xFF);
    m.mem.w8(DS, FIRST_LINE, 0);
    m.mem.w16(DS, LINES, 0);
    Panel::Inventory(Inventory::default())
}

/// What the engine says while the inventory or the spellbook acts: the
/// texts of its message boxes, and the use list's pick.
#[derive(Default)]
pub struct Said {
    pub boxes: Vec<Vec<Vec<Span>>>,
    pub pick: Option<u16>,
}

impl Said {
    fn text(&mut self, bytes: &[u8]) {
        self.boxes.push(crate::hud::lines(bytes, 0x0F));
    }
}

impl arena::use_item::UseHooks for Said {
    fn pick(&mut self, _m: &mut Machine, _count: u16) -> u16 {
        self.pick.take().unwrap_or(0xFFFF)
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        let t = m.mem.cstr(DS, si).to_vec();
        self.text(&t);
    }
    /// 1194:1BE0: "This item can't be equipped." with another list up, for
    /// an accessory or a stack, or an item of type word 0 (+6); otherwise
    /// "%ss cannot equip this item." with the class's name (the thief's
    /// own plural for class 0Ah).
    fn cant_use(&mut self, m: &mut Machine, si: u16, cat: u16) {
        if m.mem.r16(DS, NOT_OWN) != 0 || cat >= 2 || m.mem.r16(DS, si + 6) == 0 {
            let t = m.mem.cstr(DATA, CANT_EQUIP).to_vec();
            self.text(&t);
            return;
        }
        let class = m.mem.r8(DS, arena::character::PLAYER + arena::character::field::CLASS) & 0x1F;
        let name = if class == 0x0A {
            m.mem.cstr(DS, 0x3E82).to_vec()
        } else {
            let (seg, off) = m.class_name(DS, arena::character::PLAYER);
            m.mem.cstr(seg, off).to_vec()
        };
        let fmt = m.mem.cstr(DATA, CLASS_CANT).to_vec();
        self.text(&printf_s(&fmt, &name));
    }
    /// 1194:1B78: "You can only equip one %s." with the kind's name (the
    /// rings' list at 1DD1, or the bracelets' at 2028 with 12A0 set).
    fn clash(&mut self, m: &mut Machine, _si: u16, base: u16) {
        let list = if m.mem.r16(DS, 0x12A0) != 0 { 0x2028 } else { 0x1DD1 };
        let at = m.skip_strings(DS, list, base);
        let name = m.mem.cstr(DS, at).to_vec();
        let fmt = m.mem.cstr(DATA, ONLY_ONE).to_vec();
        self.text(&printf_s(&fmt, &name));
    }
}

impl arena::cast::CastHooks for Said {
    fn message_box(&mut self, m: &mut Machine, text: u16) {
        let t = m.mem.cstr(DS, text).to_vec();
        self.text(&t);
    }
}

/// `fmt` with its one `%s` replaced by `s`.
fn printf_s(fmt: &[u8], s: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] == b'%' && fmt.get(i + 1) == Some(&b's') {
            out.extend_from_slice(s);
            i += 2;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
}

/// A left click on inventory line `row` (on screen): the item there put
/// on or taken off.
pub fn equip_line(m: &mut Machine, row: usize, said: &mut Said) {
    let slot = m.mem.r8(DS, FIRST_LINE) as u16 + row as u16;
    if let Some(l) = item_lines(m).into_iter().find(|l| l.slot == slot) {
        if m.equip_click(said, l.slot).is_none() {
            eprintln!("equipping met something not ported");
        }
    }
}

/// A right click on line `row`: its item selected (1296), or nothing.
pub fn select_line(m: &mut Machine, row: usize) {
    let slot = m.mem.r8(DS, FIRST_LINE) as u16 + row as u16;
    let occupied = item_lines(m).iter().any(|l| l.slot == slot);
    m.mem.w8(DS, SELECTED, if occupied { slot as u8 } else { 0xFF });
}

/// D: the selected item dropped on the player's square, or "Drop what?".
pub fn drop_selected(m: &mut Machine, said: &mut Said) {
    if !m.drop_item() {
        let t = m.mem.cstr(DATA, DROP_WHAT).to_vec();
        said.text(&t);
    }
}

/// The names the use button lists (24FA:0AFE on a copy of the machine).
pub fn usable_names(m: &Machine) -> Vec<String> {
    let mut c = m.clone();
    let n = c.usable_list(arena::character::PLAYER).unwrap_or(0);
    let mut at = USABLE_NAMES;
    (0..n)
        .map(|_| {
            let s = c.mem.cstr(DS, at).to_vec();
            at += s.len() as u16 + 1;
            String::from_utf8_lossy(&s).into_owned()
        })
        .collect()
}

/// The use button with `pick` its list's answer (24FA:07CD).
pub fn use_pick(m: &mut Machine, files: &mut arena::bsa::Files, pick: u16, said: &mut Said) {
    said.pick = Some(pick);
    if m.use_item(files, said).is_none() {
        eprintln!("using the item met something not ported");
    }
}

/// The use list as a menu, entry `sel` highlighted.
pub fn use_menu(names: &[String], sel: usize) -> crate::hud::Menu {
    crate::hud::Menu {
        title: crate::hud::lines(b"Use which item?", 0x0F),
        entries: names.iter().map(|n| vec![Span { colour: 0x0F, text: n.clone() }]).collect(),
        keys: Vec::new(),
        selected: sel,
        list: false,
        kind: 0,
    }
}

/// The spellbook's entry `sel` cast (29CF:071F); a spell with a target
/// then waits for a click in the view, which aims it.
pub fn cast(engine: &mut arena::engine::Engine, sel: usize, said: &mut Said) {
    if let Some(s) = spells(&engine.m).get(sel) {
        if engine.m.cast_spell(&mut engine.files, said, s.index).is_none() {
            eprintln!("the cast met something not ported");
        }
    }
}

/// The scroll of the list (FIRST_LINE) moved by `d` lines, kept in range.
pub fn scroll(m: &mut Machine, d: i32) {
    let n = item_lines(m).last().map_or(0, |l| l.slot as i32 + 1);
    let first = (m.mem.r8(DS, FIRST_LINE) as i32 + d).clamp(0, (n - LIST_LINES as i32).max(0));
    m.mem.w8(DS, FIRST_LINE, first as u8);
}

/// The inventory line under (x, y) on the 320 x 200 screen.
pub fn line_at(x: i32, y: i32) -> Option<usize> {
    if !(LIST_X - 4..171).contains(&x) || y < LIST_Y - 2 {
        return None;
    }
    let row = ((y - LIST_Y + 2) / LINE_H) as usize;
    (row < LIST_LINES).then_some(row)
}

/// The equipment screen's buttons along its bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Exit,
    Spellbook,
    Drop,
}

/// The button under (x, y): as EQUIP.IMG shows them along rows 189..199
/// (the spellbook's where EQUIPB.IMG, 41 x 9 at 64, 190, covers it while
/// another list is up); their exact spans in the original's click table
/// weren't traced.
pub fn button_at(x: i32, y: i32) -> Option<Button> {
    if !(188..200).contains(&y) {
        return None;
    }
    match x {
        0..=45 => Some(Button::Exit),
        60..=110 => Some(Button::Spellbook),
        120..=170 => Some(Button::Drop),
        _ => None,
    }
}

/// The spellbook's layout: (x, y of the first entry, line height, width,
/// lines shown).
pub fn book_layout(font: &Font) -> (i32, i32, i32, i32, usize) {
    let lh = font.h as i32 + 2;
    (40, 30 + lh + 4, lh, 240, 10)
}

/// The panel drawn over the layer (`pics`: EQUIP.IMG).
pub fn draw(screen: &mut Screen, font: &Font, equip: Option<&Image>, doll: &[Image], panel: &Panel, m: &Machine) {
    match panel {
        Panel::Sheet => crate::sheets::draw_sheet(screen, m, doll),
        Panel::Log(log) => crate::sheets::draw_log(screen, log),
        Panel::Inventory(inv) => {
            screen.fill(0, 0, crate::hud::W as i32, crate::hud::H as i32, 0, 0xFF);
            // The page with the body and what is worn on it (`doll`), the
            // list's panel over its left side.
            for img in doll {
                screen.blit(img.x, img.y, img.w, img.h, &img.px, 199);
            }
            if let Some(img) = equip {
                screen.blit(img.x, img.y, img.w, img.h, &img.px, 199);
            }
            let first = m.mem.r8(DS, FIRST_LINE) as u16;
            let selected = m.mem.r8(DS, SELECTED);
            for l in item_lines(m) {
                if l.slot < first || l.slot >= first + LIST_LINES as u16 {
                    continue;
                }
                let y = LIST_Y + (l.slot - first) as i32 * LINE_H;
                if l.slot as u8 == selected {
                    screen.fill(LIST_X - 3, y - 1, 150, LINE_H, 0x08, 0xC0);
                }
                screen.text_plain(font, LIST_X, y, &l.name, l.colour);
            }
            if let Some((names, sel)) = &inv.using {
                let hud = crate::hud::Hud { menu: Some(use_menu(names, *sel)), ..Default::default() };
                crate::hud::compose_over(screen, font, &hud);
            }
        }
        Panel::Spellbook(book) => {
            let list = spells(m);
            let (x, y, lh, w, shown) = book_layout(font);
            let h = lh * (shown as i32 + 1) + 14;
            screen.panel(x - 8, y - lh - 9, w + 16, h, 0x0F);
            let sp = m.mem.r16(DS, 0x020A);
            screen.text(font, x, y - lh - 4, &format!("Spellbook ({sp} spell points)"), 0x0F);
            if list.is_empty() {
                screen.text(font, x, y, "No spells.", 0x0F);
            }
            for (i, s) in list.iter().enumerate().skip(book.first).take(shown) {
                let ly = y + (i - book.first) as i32 * lh;
                if i == book.selected {
                    screen.fill(x - 3, ly - 1, w + 6, lh, 0x08, 0xC0);
                }
                let c = if s.cost > sp { 0xA0 } else { 0x0F };
                screen.text(font, x, ly, &s.name, c);
                let cost = s.cost.to_string();
                screen.text(font, x + w - font.width(&cost) as i32, ly, &cost, c);
            }
        }
    }
}

/// `--do corpse`: a fight summoned (`world::summon`), its first creature
/// (slots 15..22) put a square ahead of the player and killed (its HP 0,
/// the kill check 2AA1:06A0), for trying the loot window on its corpse:
/// a debugging aid, as --at is. Whether one died.
pub fn corpse(engine: &mut arena::engine::Engine) -> bool {
    crate::world::summon(engine);
    let m = &mut engine.m;
    let seg = m.mem.r16(DS, 0x1042);
    for slot in 15..23u16 {
        let si = 0x0AB3 + slot * 0x1C;
        if m.mem.r16(DS, si + 0xF) & 0x4000 != 0 {
            continue;
        }
        let h = m.mem.r16(DS, arena::movement::var::HEADING) as f32 / 512.0 * std::f32::consts::TAU;
        let (x, y) = (m.mem.r16(DS, arena::movement::var::X) as f32, m.mem.r16(DS, arena::movement::var::Y) as f32);
        m.mem.w16(DS, si, (x + h.sin() * 128.0) as u16);
        m.mem.w16(DS, si + 4, (y + h.cos() * 128.0) as u16);
        let rec = m.mem.r16(DS, si + 0x12);
        m.mem.w16(seg, rec + arena::character::field::HP, 0);
        return m.kill_check(seg, rec).is_some();
    }
    false
}

/// `--do give:N`: N items made by the loot generators (a weapon, a piece
/// of armour, a potion, in turn) and given to the player, for trying the
/// inventory: not the game's (nothing in it gives items this way).
pub fn give(m: &mut Machine, n: u16) {
    for i in 0..n {
        m.mem.w8(DS, 0x13F0, 1);
        let ok = match i % 3 {
            0 => m.loot_weapon(10, 0xFF).is_some(),
            1 => {
                m.loot_armor(10, 0xFF);
                true
            }
            _ => m.shop_potion().is_some(),
        };
        if ok {
            m.add_item(DS, arena::loot::ITEM, DS, arena::character::PLAYER);
        }
    }
}
