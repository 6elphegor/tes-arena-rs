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
//! (24FA:07CD, `use_item`). The paper doll on the right (1194:23E5,
//! `doll`) is drawn first, EQUIP.IMG (the list's panel and the buttons)
//! over it.
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
use arena::use_item::var::{DROP_TEXT, FIRST_LINE, LINES, NOT_OWN, OTHER_LIST, OWN_ITEMS, SELECTED, USABLE_NAMES};
use arena::use_item::DropButton;

/// The list's place and lines (1194:1DE9..1EA6).
pub const LIST_X: i32 = 0x0E;
pub const LIST_Y: i32 = 0x32;
pub const LINE_H: i32 = 0x0B;
pub const LIST_LINES: usize = 7;
const SLOTS: u16 = 40;

/// The original's texts (segment 3276): an item that can't be put on,
/// one a class can't (with the class's name), a second accessory of a
/// kind (with the kind's name).
const CANT_EQUIP: u16 = 0x5A90;
const CLASS_CANT: u16 = 0x3BEB;
const ONLY_ONE: u16 = 0x5A74;

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

/// 0000:0B9C over the spellbook's names (29CF:060B, CX of them): its
/// order table (DGROUP 1044: for each line shown, the book's entry), the
/// names exchange-sorted, each pair (i, j > i) swapped when 9C0A finds
/// them out of order: `repe cmpsb` of the first's bytes against the
/// second's, over the second's length and its NUL, and the last byte's
/// difference taken as signed (its `js`), so a name sorts after one it
/// begins with and bytes 80h apart or more compare backwards. With fewer
/// than two names nothing is sorted and the table is left as it was
/// (None).
pub fn book_order(names: &[Vec<u8>]) -> Option<Vec<u8>> {
    let n = names.len();
    if n <= 1 {
        return None;
    }
    let mut ptr: Vec<usize> = (0..n).collect();
    let mut order: Vec<u8> = (0..n as u8).collect();
    let after = |a: &[u8], b: &[u8]| {
        // 9C0A with SI a, DI b: [si] - [es:di] over len(b) + 1 bytes.
        for k in 0..=b.len() {
            let (x, y) = (a.get(k).copied().unwrap_or(0), b.get(k).copied().unwrap_or(0));
            if x != y || k == b.len() {
                let d = x.wrapping_sub(y);
                return d != 0 && d & 0x80 == 0;
            }
        }
        false
    };
    for i in 0..n - 1 {
        for j in i + 1..n {
            if after(&names[ptr[i]], &names[ptr[j]]) {
                ptr.swap(i, j);
                order.swap(i, j);
            }
        }
    }
    Some(order)
}

/// The spellbook's names in the book's order, as 29CF:0654 copies them.
fn book_names(m: &Machine) -> Vec<Vec<u8>> {
    (0..m.mem.r8(DS, BOOK_COUNT) as u16)
        .map(|k| {
            let (seg, off) = m.spell_record(m.mem.r8(DS, BOOK + k));
            m.mem.cstr(seg, off + arena::save::spell::NAME).to_vec()
        })
        .collect()
}

/// The book's entry for the spellbook's line `line` picked (29CF:0642):
/// the order table written as 0B9C writes it, then read at the line.
pub fn book_pick(m: &mut Machine, line: usize) -> usize {
    if let Some(order) = book_order(&book_names(m)) {
        for (k, &o) in order.iter().enumerate() {
            m.mem.w8(DS, 0x1044 + k as u16, o);
        }
    }
    m.mem.r8(DS, 0x1044 + line as u16) as usize
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
    /// The bar's use button (2542:034D): the items with an effect, a list
    /// in the bar's corner (2542:066A) and the line picked.
    Use(Vec<String>, usize),
}

/// The inventory: the use list when it is up (the names, the line
/// highlighted).
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    pub using: Option<(Vec<String>, usize)>,
    /// The drop button's question up (its text, the answer highlighted:
    /// 0 yes).
    pub asking: Option<(Vec<u8>, usize)>,
    /// Opened from the character sheet (1194:0D67 -> 16BD): its exit goes
    /// back to the sheet.
    pub from_sheet: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Spellbook {
    /// The line picked (3159:06A0's, from the first).
    pub selected: usize,
}

impl Panel {
    /// The spellbook and the use list are lists in the bar's corner
    /// (2542:06F8) over play, in its palette: the HUD's (`bar_list`), not
    /// a screen of their own.
    pub fn over_play(&self) -> bool {
        matches!(self, Panel::Spellbook(_) | Panel::Use(..))
    }

    /// The list such a panel shows (`hud::BAR_LIST`): the spellbook's
    /// spell names (29CF:060B, each the record's name as 0654 copies it)
    /// or the items' (066A, 13h at most), the line picked.
    pub fn bar_list(&self, m: &Machine) -> Option<crate::hud::Menu> {
        let (names, selected) = match self {
            Panel::Spellbook(book) => {
                let names = book_names(m);
                let order = book_order(&names).unwrap_or_else(|| (0..names.len() as u8).collect());
                (order.iter().map(|&o| String::from_utf8_lossy(&names[o as usize]).into_owned()).collect(), book.selected)
            }
            Panel::Use(names, sel) => (names.clone(), *sel),
            _ => return None,
        };
        Some(crate::hud::Menu {
            title: Vec::new(),
            entries: names.into_iter().map(|n| vec![Span { colour: 0xFD, text: n, font: Default::default(), code: None }]).collect(),
            keys: Vec::new(),
            selected,
            list: true,
            kind: crate::hud::BAR_LIST,
        })
    }
}

/// The equipment screen opened on the player's own items (1194:23C7's
/// list; from the sheet 16F4 and 171F): the first line selected (1296
/// 0, 1726) and shown first, no other list.
pub fn open_inventory(m: &mut Machine) -> Panel {
    m.mem.w8(DS, OWN_ITEMS, 1);
    m.mem.w8(DS, OTHER_LIST, 0);
    m.mem.w16(DS, NOT_OWN, 0);
    m.mem.w8(DS, SELECTED, 0);
    m.mem.w8(DS, FIRST_LINE, 0);
    m.mem.w16(DS, LINES, 0);
    Panel::Inventory(Inventory::default())
}

/// The sheet's equipment page ('N'): the equipment screen, its exit
/// back to the sheet.
pub fn equipment_page(m: &mut Machine) -> Panel {
    let mut p = open_inventory(m);
    if let Panel::Inventory(inv) = &mut p {
        inv.from_sheet = true;
    }
    p
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
            eprintln!("equipping gave up: {}", arena::giveup::describe());
        }
    }
}

/// A right click on line `row`: its item selected (1296), or nothing.
pub fn select_line(m: &mut Machine, row: usize) {
    let slot = m.mem.r8(DS, FIRST_LINE) as u16 + row as u16;
    let occupied = item_lines(m).iter().any(|l| l.slot == slot);
    m.mem.w8(DS, SELECTED, if occupied { slot as u8 } else { 0xFF });
}

/// D and the drop button (1194:17D8, `drop_button`): run on a copy
/// first, so a question it asks ("Do you wish to drop the ...?", or the
/// item lost where the player stands) is put up and the game left as it
/// was until it is answered (`drop_answer`); without one it is the
/// game's. Its refusals are said.
pub fn drop_press(m: &mut Machine, inv: &mut Inventory, said: &mut Said) {
    let mut probe = m.clone();
    let mut asked = None;
    let r = probe.drop_button(&mut |p| {
        asked = Some(p.mem.cstr(DS, DROP_TEXT).to_vec());
        false
    });
    match asked {
        Some(text) => inv.asking = Some((text, 0)),
        None => {
            *m = probe;
            drop_said(m, r, said);
        }
    }
}

/// The drop's question answered (`yes`): the button again with that
/// answer.
pub fn drop_answer(m: &mut Machine, yes: bool, said: &mut Said) {
    let r = m.drop_button(&mut |_| yes);
    drop_said(m, r, said);
}

fn drop_said(m: &Machine, r: Option<DropButton>, said: &mut Said) {
    match r {
        Some(DropButton::NoRoom | DropButton::CantDrop | DropButton::DropWhat) => said.text(m.mem.cstr(DS, DROP_TEXT)),
        Some(DropButton::Nothing | DropButton::Dropped) => {}
        None => eprintln!("the drop button gave up: {}", arena::giveup::describe()),
    }
}

/// The drop's question as the original's yes-or-no menu (1194:48E9 ->
/// 020E), entry `sel` highlighted.
pub fn drop_menu(text: &[u8], sel: usize) -> crate::hud::Menu {
    let line = |s: &str| vec![Span { colour: 0x0F, text: s.to_string(), font: Default::default(), code: None }];
    crate::hud::Menu {
        title: crate::hud::lines(text, 0x0F),
        entries: vec![line("Yes"), line("No")],
        keys: b"YN".to_vec(),
        selected: sel,
        list: false,
        kind: 0,
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
        eprintln!("using the item gave up: {}", arena::giveup::describe());
    }
}

/// The use list as a menu, entry `sel` highlighted.
pub fn use_menu(names: &[String], sel: usize) -> crate::hud::Menu {
    crate::hud::Menu {
        title: crate::hud::lines(b"Use which item?", 0x0F),
        entries: names.iter().map(|n| vec![Span { colour: 0x0F, text: n.clone(), font: Default::default(), code: None }]).collect(),
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
            eprintln!("the cast gave up: {}", arena::giveup::describe());
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
            let pics = crate::sheets::loaded();
            // 1194:22F5: the name, race and class (0E19) and the level
            // (+6, from 0) + 1 at (80h, 17h), in ARENAFNT (the sheet's
            // [102C], still selected), colour FEh.
            if let Some(f) = pics.as_ref().and_then(|p| p.arena.as_ref()) {
                crate::sheets::name_texts(screen, f, m);
                let level = m.mem.r8(DS, 0x01A4 + 6) as u16 + 1;
                screen.text_plain(f, 0x80, 0x17, &level.to_string(), 0xFE);
            }
            // 1194:1DC8: the lines in TEENYFNT ([102E]); the selected one
            // (1296) framed in A0h, 12 rows high from 2 above it, 146 wide
            // from 2 left of the text (1194:1F27).
            let teeny = pics.as_ref().and_then(|p| p.teeny.as_ref()).unwrap_or(font);
            let first = m.mem.r8(DS, FIRST_LINE) as u16;
            let selected = m.mem.r8(DS, SELECTED);
            for l in item_lines(m) {
                if l.slot < first || l.slot >= first + LIST_LINES as u16 {
                    continue;
                }
                let y = LIST_Y + (l.slot - first) as i32 * LINE_H;
                if l.slot as u8 == selected {
                    let (fx, fy, fw, fh) = (LIST_X - 2, y - 2, 146, 12);
                    screen.fill(fx, fy, fw, 1, 0xA0, 0xFF);
                    screen.fill(fx, fy + fh - 1, fw, 1, 0xA0, 0xFF);
                    screen.fill(fx, fy, 1, fh, 0xA0, 0xFF);
                    screen.fill(fx + fw - 1, fy, 1, fh, 0xA0, 0xFF);
                }
                screen.text_plain(teeny, LIST_X, y, &l.name, l.colour);
            }
            // 1194:76B4: the armour class at each of the seven places
            // (7719, into 942E), "+%d" or "%d", on the doll at the man's
            // places (5D8D) or the woman's (5DA9): first in 8Ah, then one
            // left in C3h over it.
            if let Some(f) = pics.as_ref().and_then(|p| p.arena.as_ref()) {
                let mut c = m.clone();
                c.player_armor_class();
                let places = if m.mem.r8(DS, 0x01A4 + 7) != 0 { 0x5DA9 } else { 0x5D8D };
                for (colour, dx) in [(0x8A, 0), (0xC3, -1)] {
                    for i in 0..7u16 {
                        let ac = c.mem.r8(DS, 0x942E + i) as i8;
                        let x = m.mem.r16(DS, places + 4 * i) as i16 as i32 + dx;
                        let y = m.mem.r16(DS, places + 4 * i + 2) as i16 as i32;
                        let t = if ac >= 0 { format!("+{ac}") } else { ac.to_string() };
                        screen.text_plain(f, x, y, &t, colour);
                    }
                }
            }
            if let Some((names, sel)) = &inv.using {
                let hud = crate::hud::Hud { menu: Some(use_menu(names, *sel)), ..Default::default() };
                crate::hud::compose_over(screen, font, &hud);
            }
            if let Some((text, sel)) = &inv.asking {
                let hud = crate::hud::Hud { menu: Some(drop_menu(text, *sel)), ..Default::default() };
                crate::hud::compose_over(screen, font, &hud);
            }
        }
        // In the bar's corner: the HUD's list (`Panel::bar_list`).
        Panel::Spellbook(_) | Panel::Use(..) => {}
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
