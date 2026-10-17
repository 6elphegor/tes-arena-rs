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
//! identified magic item (flags 1 and 2), worn or not. A click on a line
//! marks it (1296, 1194:1C5E), which D drops (1194:18F6, `drop_item`); a
//! left click on the line marked puts the item on or takes it off
//! (1194:1AFB, `equip_click`); U lists the items with an effect and uses the one picked
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
    // The packed data and the picture in blocks of the copy's heap, as
    // the original loads a file into an allocation (the heap reaches over
    // all of conventional memory, as the original's: fixed segments would
    // be inside it, under the unpacker's tables).
    let mut c = m.clone();
    let (src, src_off) = c.heap_alloc_bytes(u16::try_from(size).ok()?)?;
    let (dst, dst_off) = c.heap_alloc_bytes(u16::try_from(w * h).ok()?)?;
    for (i, &b) in data.iter().enumerate() {
        c.mem.w8(src, src_off.wrapping_add(i as u16), b);
    }
    c.mif_decompress(src, src_off, dst, dst_off, size as u16)?;
    let px = (0..w * h).map(|i| c.mem.r8(dst, dst_off.wrapping_add(i as u16))).collect();
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

/// The quests' lines after the items (1194:1EB8: 1194:A206 then
/// 2433:083D into 1044, `Machine::quest_item_names`), on a copy.
pub fn quest_lines(m: &Machine, files: &arena::bsa::Files) -> Vec<String> {
    let mut c = m.clone();
    let mut fs = files.clone();
    c.mem.w16(DS, arena::questitems::var::COUNT, 0);
    let Some(more) = c.quest_item_names(&mut fs, 0x1044) else {
        eprintln!("the quests' lines gave up: {}", arena::giveup::describe());
        return Vec::new();
    };
    let n = c.mem.r16(DS, arena::questitems::var::COUNT).wrapping_add(more);
    let mut at = 0x1044u16;
    (0..n.min(64))
        .map(|_| {
            let t = c.mem.cstr(DS, at).to_vec();
            at = at.wrapping_add(t.len() as u16 + 1);
            String::from_utf8_lossy(&t).into_owned()
        })
        .collect()
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
    /// An item's description up over the list's foot (1194:1988): its
    /// lines and their colours, until a click.
    pub info: Option<Vec<(String, u8)>>,
    /// Where those lines start when not where 1988 puts them (1BE0's
    /// "can't be equipped" at A0h).
    pub info_y: Option<i32>,
    /// A spell's page up (1194:1B3C, from the spells' page).
    pub page: Option<SpellPage>,
    /// Left by a spell's page's Exit (1194:53B5): the screen put back as
    /// SPELBOOK.$$$ kept it with EQUIP.IMG over its left side, the list
    /// drawn on it (1DC8), but not the name, the level or the spells'
    /// page's pictures (SPELLBK.IMG, EQUIPB.IMG), until the screen is
    /// drawn again.
    pub bare: bool,
}

/// An answer given to the spell's page.
#[derive(Clone, Debug)]
pub enum PageAnswer {
    Click(u16, u16),
    Confirm(u16),
    /// The name's field's passes' inputs (`field::Live`), as typed.
    Keys(Vec<arena::lineedit::EditKey>),
}

/// A spell's page up (`Machine::spell_page`, 1194:51D1): the game as the
/// click that opened it found it, the answers given since (the page run
/// again from there with them, as the dialogs are), the page as its last
/// question found it (1001 FFh, the screen 68D7 drew) and the Delete
/// question when that is what waits (its text, the answer highlighted).
#[derive(Clone, Debug)]
pub struct SpellPage {
    start: crate::dialog::Snapshot,
    answers: Vec<PageAnswer>,
    pub shown: crate::dialog::Snapshot,
    pub asking: Option<(Vec<u8>, usize)>,
    /// The pages drawn on the way (68D7, each setting its colours).
    pub pages: usize,
    /// The name's field asked (1194:65FD), when that is what waits: the
    /// field, its look and the game as it asked it.
    pub typing: Option<(arena::lineedit::Field, crate::field::Look, crate::dialog::Snapshot)>,
}

/// The page's questions answered from `answers`; the first one without
/// its answer is kept (`shown`, `asking`) and the page left as Exit and
/// No leave it.
struct Replay {
    answers: std::collections::VecDeque<PageAnswer>,
    waiting: bool,
    shown: Option<Machine>,
    asking: Option<Vec<u8>>,
    pages: usize,
    typing: Option<(arena::lineedit::Field, crate::field::Look, crate::dialog::Snapshot)>,
}

impl Replay {
    fn wait(&mut self, m: &Machine) {
        self.waiting = true;
        let mut s = m.clone();
        s.mem.w8(DS, arena::spellpage::var::OWN, 0xFF);
        self.shown = Some(s);
    }
}

impl arena::spellpage::SpellPageHooks for Replay {
    fn page(&mut self, _m: &mut Machine) {
        if !self.waiting {
            self.pages += 1;
        }
    }
    fn click(&mut self, m: &mut Machine) -> (u8, u16, u16) {
        if !self.waiting {
            match self.answers.pop_front() {
                Some(PageAnswer::Click(x, y)) => return (1, x, y),
                _ => self.wait(m),
            }
        }
        (1, 0x120, 0xC0)
    }
    /// 1194:65FD: the name at 581A typed over in place on the page
    /// (1194:661B's field: 45h, 30h, the cursor at the click's x less
    /// 45h), in ARENAFNT.DAT ([102C]) and colour A0h, as the spellmaker's.
    fn type_name(&mut self, m: &mut Machine) {
        if self.waiting {
            return;
        }
        let field = arena::lineedit::fields::spell_name(m, arena::spellmaker::var::NAME);
        let look = crate::field::Look { font: "ARENAFNT.DAT", colour: 0xA0, put_font: true };
        match self.answers.pop_front() {
            Some(PageAnswer::Keys(keys)) => {
                if crate::field::replay(m, &field, &look, &keys) != Some(true) {
                    eprintln!("the spell's name gave up: {}", arena::giveup::describe());
                }
            }
            _ => {
                self.wait(m);
                self.typing = Some((field, look, crate::dialog::Snapshot(Box::new(m.clone()))));
            }
        }
    }
    fn confirm(&mut self, m: &mut Machine, si: u16) -> u16 {
        if !self.waiting {
            match self.answers.pop_front() {
                Some(PageAnswer::Confirm(a)) => return a,
                _ => {
                    self.wait(m);
                    self.asking = Some(m.mem.cstr(DS, si).to_vec());
                }
            }
        }
        1
    }
}

impl SpellPage {
    /// The page run from its start with the answers; None once it has
    /// been left (the game then `m`'s, the files `files`').
    fn run(start: &Machine, files: &mut arena::bsa::Files, answers: &[PageAnswer], m: &mut Machine) -> Option<SpellPage> {
        let mut game = start.clone();
        let mut fs = files.clone();
        let mut r = Replay { answers: answers.iter().cloned().collect(), waiting: false, shown: None, asking: None, pages: 0, typing: None };
        if game.spell_page(&mut fs, &mut r).is_none() {
            eprintln!("the spell's page gave up: {}", arena::giveup::describe());
        }
        match r.shown {
            Some(shown) if r.waiting => Some(SpellPage {
                start: crate::dialog::Snapshot(Box::new(start.clone())),
                answers: answers.to_vec(),
                shown: crate::dialog::Snapshot(Box::new(shown)),
                asking: r.asking.map(|t| (t, 0)),
                pages: r.pages,
                typing: r.typing,
            }),
            _ => {
                *m = game;
                *files = fs;
                None
            }
        }
    }

    /// The Delete question's yes-or-no menu (1194:48E9: 020E with the
    /// keys at 4B60 and the entries at 4B63) as it was asked.
    pub fn menu(&self) -> crate::hud::Menu {
        let mut menu = crate::dialog::menu_at(&self.shown.0, 0x1044, 0x4B60, 0x4B63);
        menu.selected = self.asking.as_ref().map_or(0, |a| a.1);
        menu
    }

    /// The Delete question's boxes' tiles: spparts.rci's 3 x 3 (52DB
    /// registers it, 0000:756C with AX 3, BX 3, and makes it B038's).
    pub fn tiles(archive: &crate::archive::Archive) -> Option<crate::hud::Tiles> {
        crate::hud::Tiles::load(archive, "SPPARTS.RCI", 3, 3)
    }

    /// The Delete question's entry under (x, y): its boxes as 0F2A lays
    /// them out in those tiles and ARENAFNT ([102C]).
    pub fn menu_pick(&self, archive: &crate::archive::Archive, x: i32, y: i32) -> Option<usize> {
        let pics = crate::sheets::loaded()?;
        let f = pics.arena.as_ref()?;
        let tiles = SpellPage::tiles(archive)?;
        let menu = self.menu();
        let (_, es) = crate::hud::custom::menu_rects(&tiles, f, &menu.title, &menu.entries);
        es.iter().position(|&(rx, ry, rw, rh)| x >= rx && x < rx + rw && y >= ry && y < ry + rh)
    }

    /// 1194:1B3C: the marked spell's page opened.
    pub fn open(m: &mut Machine, files: &mut arena::bsa::Files) -> Option<SpellPage> {
        let start = m.clone();
        SpellPage::run(&start, files, &[], m)
    }

    /// An answer given: the page as it now stands, None once left.
    pub fn answer(self, a: PageAnswer, m: &mut Machine, files: &mut arena::bsa::Files) -> Option<SpellPage> {
        let mut answers = self.answers;
        answers.push(a);
        SpellPage::run(&self.start.0, files, &answers, m)
    }
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
            font: None,
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
    /// A refusal the equipment screen draws in its list's panel (1194:
    /// 223A: one line centred at 55h, y A0h, colour A0h, until a click),
    /// not in a box.
    pub panel: Option<String>,
}

impl Said {
    fn text(&mut self, bytes: &[u8]) {
        self.boxes.push(crate::hud::lines(bytes, 0x0F));
    }
    /// What 59C8 (59CC) leaves in the game's state for its box
    /// (`Machine::message_shown`); the screen it covers isn't kept here.
    fn shown(m: &mut Machine, text: (u16, u16)) {
        if m.message_shown(text, None).is_none() {
            eprintln!("the message box gave up: {}", arena::giveup::describe());
        }
    }
    fn in_panel(&mut self, bytes: &[u8]) {
        let t = bytes.split(|&c| c == b'\r').next().unwrap_or_default();
        self.panel = Some(String::from_utf8_lossy(t).into_owned());
    }
}

/// A special item's description box (1194:7E44's 59CC, its text in a
/// heap block), in the colour its caller left (9288), the font selected
/// (B036: the list's TEENYFNT) and MARBLE2.CIF's tiles.
impl arena::text::TextHooks for Said {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.message_box_far(m, DS, si);
    }
    fn message_box_far(&mut self, m: &mut Machine, seg: u16, si: u16) {
        let t = m.mem.cstr(seg, si).to_vec();
        let font = crate::hud::GameFont::selected(m);
        let mut lines = crate::hud::lines(&t, m.mem.r8(DS, 0x9288));
        for s in lines.iter_mut().flatten() {
            s.font = font;
        }
        // 7E44 makes [103A] B038's (0000:9AF8) before its box: MARBLE2's
        // tiles (the view keeps no tile records to compare B038 with).
        let r = |at| m.mem.r16(DS, at) as i32;
        lines = crate::hud::in_marble2(lines, (r(0xA914), r(0xA916), r(0xA918), r(0xA91A)));
        self.boxes.push(lines);
        Said::shown(m, (seg, si));
    }
}

impl arena::use_item::UseHooks for Said {
    fn pick(&mut self, _m: &mut Machine, _count: u16) -> u16 {
        self.pick.take().unwrap_or(0xFFFF)
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        let t = m.mem.cstr(DS, si).to_vec();
        self.text(&t);
        Said::shown(m, (DS, si));
    }
    /// 1194:1BE0: "This item can't be equipped." with another list up, for
    /// an accessory or a stack, or an item of type word 0 (+6); otherwise
    /// "%ss cannot equip this item." with the class's name (the thief's
    /// own plural for class 0Ah); in the list's panel (223A).
    fn cant_use(&mut self, m: &mut Machine, si: u16, cat: u16) {
        if m.mem.r16(DS, NOT_OWN) != 0 || cat >= 2 || m.mem.r16(DS, si + 6) == 0 {
            let t = m.mem.cstr(DATA, CANT_EQUIP).to_vec();
            self.in_panel(&t);
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
        self.in_panel(&printf_s(&fmt, &name));
    }
    /// 1194:1B78: "You can only equip one %s." with the kind's name (the
    /// rings' list at 1DD1, or the bracelets' at 2028 with 12A0 set), in
    /// the list's panel (223A).
    fn clash(&mut self, m: &mut Machine, _si: u16, base: u16) {
        let list = if m.mem.r16(DS, 0x12A0) != 0 { 0x2028 } else { 0x1DD1 };
        let at = m.skip_strings(DS, list, base);
        let name = m.mem.cstr(DS, at).to_vec();
        let fmt = m.mem.cstr(DATA, ONLY_ONE).to_vec();
        self.in_panel(&printf_s(&fmt, &name));
    }
}

impl arena::cast::CastHooks for Said {
    fn message_box(&mut self, m: &mut Machine, text: u16) {
        let t = m.mem.cstr(DS, text).to_vec();
        self.text(&t);
        Said::shown(m, (DS, text));
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

/// 1194:2365: the Spellbook button (the Equipment button on the spells'
/// page): for a class with spells (bit 20h) the page turns (OWN_ITEMS
/// toggled), its first line marked and shown first; for another, its
/// text (3276:5AD7) said.
pub fn spells_button(m: &mut Machine, said: &mut Said) {
    if m.mem.r8(DS, 0x01A9) & 0x20 == 0 {
        let t = m.mem.cstr(DATA, 0x5AD7).to_vec();
        said.text(&t);
        return;
    }
    let own = m.mem.r8(DS, OWN_ITEMS) ^ 1;
    m.mem.w8(DS, OWN_ITEMS, own);
    m.mem.w8(DS, SELECTED, 0);
    m.mem.w8(DS, FIRST_LINE, 0);
}

/// 1194:1C5E: the line a click at row `y` marks (1296), by the row
/// alone: the first line's ends at 3Dh, each next 0Bh lower, over the
/// lines shown (the items or the book's spells, at most seven); below
/// them none (FFh).
fn mark_at(m: &mut Machine, y: i32, quests: usize) -> u8 {
    let n = if m.mem.r8(DS, OWN_ITEMS) == 0 { m.mem.r8(DS, BOOK_COUNT) as usize } else { item_lines(m).len() + quests };
    let shown = n.min(LIST_LINES);
    let mark = (0..shown).find(|&i| y <= 0x3D + 0x0B * i as i32).map_or(0xFF, |i| m.mem.r8(DS, FIRST_LINE).wrapping_add(i as u8));
    m.mem.w8(DS, SELECTED, mark);
    mark
}

/// 1194:1988 for a quest's line (1270): "This item is needed to complete
/// your quest..." (DATA:5AF3), or for the last of `lines` with staff
/// pieces held (1F56: 1272) "You currently have %d piece(s) of the Staff
/// of Chaos." (5B36 with F77); three lines, all in FEh (12B2 1).
fn quest_info(m: &mut Machine, lines: usize) -> Vec<(String, u8)> {
    let pieces = m.mem.r8(DS, arena::questitems::var::PIECES);
    let staff = pieces != 0 && m.mem.r8(DS, SELECTED) as usize + 1 == lines;
    let text = if staff {
        m.sprintf((DS, 0x1044), (DATA, 0x5B36), &[arena::format::Arg::Int(pieces as u16)]);
        m.mem.cstr(DS, 0x1044).to_vec()
    } else {
        m.mem.cstr(DATA, 0x5AF3).to_vec()
    };
    text.split(|&c| c == b'\r').take(3).map(|l| (String::from_utf8_lossy(l).into_owned(), 0xFE)).collect()
}

/// Whether a click at row `y` (`right`: the right button) opens the
/// marked spell's page: a left click on the spells' page's line already
/// marked (1194:1AD6 -> 1B3C).
pub fn opens_page(m: &Machine, y: i32, right: bool) -> bool {
    let old = m.mem.r8(DS, SELECTED);
    let mark = mark_at(&mut m.clone(), y, 0);
    !right && m.mem.r8(DS, OWN_ITEMS) == 0 && mark != 0xFF && mark == old
}

/// Inventory line `row` (on screen) put on or taken off, as a left click
/// on it marked does (for scripts).
pub fn equip_line(m: &mut Machine, row: usize, said: &mut Said) {
    let slot = m.mem.r8(DS, FIRST_LINE) as u16 + row as u16;
    if let Some(l) = item_lines(m).into_iter().find(|l| l.slot == slot) {
        m.mem.w8(DS, SELECTED, l.slot as u8);
        if m.equip_click(said, l.slot).is_none() {
            eprintln!("equipping gave up: {}", arena::giveup::describe());
        }
    }
}

/// Line `row` marked (1296), or none (for scripts).
pub fn select_line(m: &mut Machine, row: usize) {
    let slot = m.mem.r8(DS, FIRST_LINE) as u16 + row as u16;
    let occupied = item_lines(m).iter().any(|l| l.slot == slot);
    m.mem.w8(DS, SELECTED, if occupied { slot as u8 } else { 0xFF });
}

/// 1194:1988: the marked item described (`Machine::item_info`, with
/// 12B0 3 and 12B2 0 first, 9288 FEh), its first [12B0] lines (the CRs
/// made NULs, 1194:223A): the first in 55h when 12B2 is left 0, else FEh
/// as the rest. None with no item there, where the description gives
/// up, or for a special item, whose description is a message box of its
/// own (7E44, in `said`) and the carry skips the drawing.
fn item_info(m: &mut Machine, files: &mut arena::bsa::Files, said: &mut Said) -> Option<Vec<(String, u8)>> {
    use arena::use_item::info;
    m.mem.w16(DS, info::ARGS, 0);
    m.mem.w16(DS, info::LINES, 3);
    m.mem.w8(DS, 0x9288, 0xFE);
    // The font the list's drawing (1194:1EB8, its lines through 0000:9AE6)
    // leaves selected: [102E], TEENYFNT, which a special item's box is
    // drawn in.
    let teeny = m.mem.r16(DS, 0x102E);
    m.mem.w16(DS, 0xB036, teeny);
    let it = arena::inventory::PLAYER_SLOTS + m.mem.r8(DS, SELECTED) as u16 * arena::inventory::SLOT_SIZE;
    match m.item_info(files, said, it) {
        None => {
            eprintln!("the description gave up: {}", arena::giveup::describe());
            return None;
        }
        Some(true) => return None,
        Some(false) => {}
    }
    let first = if m.mem.r16(DS, info::ARGS) == 0 { 0x55 } else { 0xFE };
    let mut si = info::TEXT;
    let mut out = Vec::new();
    for i in 0..m.mem.r16(DS, info::LINES) {
        let start = si;
        for _ in 0..500 {
            let c = m.mem.r8(DS, si);
            si = si.wrapping_add(1);
            if c == 0x0D {
                m.mem.w8(DS, si.wrapping_sub(1), 0);
                break;
            }
        }
        let text = String::from_utf8_lossy(m.mem.cstr(DS, start)).into_owned();
        out.push((text, if i == 0 { first } else { 0xFE }));
    }
    Some(out)
}

/// 1194:1AD6: a click on the list (`right`: the right button). The line
/// under it is marked (`mark_at`); a left click on the line already
/// marked puts its item on or takes it off (1AFB).
pub fn line_click(m: &mut Machine, files: &mut arena::bsa::Files, inv: &mut Inventory, y: i32, right: bool, said: &mut Said) {
    let old = m.mem.r8(DS, SELECTED);
    // 1C5E: the items (129D) and the quests' lines (126C, as the list
    // last drew them).
    let quests = if m.mem.r8(DS, OWN_ITEMS) != 0 { quest_lines(m, files) } else { Vec::new() };
    let items = item_lines(m).len();
    let mark = mark_at(m, y, quests.len());
    // A quest's line (1CBF: the mark past the items, 1270): a right click
    // describes it (1988), a left click on it marked again can't equip it
    // (1AFB -> 1BE0).
    if mark != 0xFF && mark as usize >= items && m.mem.r8(DS, OWN_ITEMS) != 0 {
        if right {
            inv.info = Some(quest_info(m, items + quests.len()));
        } else if mark == old {
            let t = String::from_utf8_lossy(m.mem.cstr(DATA, CANT_EQUIP)).trim_end_matches('\r').to_string();
            inv.info = Some(vec![(t, 0xA0)]);
            inv.info_y = Some(0xA0);
        }
        return;
    }
    // 1194:1CE5: a right click on the player's own items describes the
    // line marked.
    if right && mark != 0xFF && m.mem.r8(DS, OWN_ITEMS) != 0 {
        inv.info = item_info(m, files, said);
        inv.info_y = None;
    }
    if mark == 0xFF || mark != old || right {
        return;
    }
    // On the spells' page the line marked again shows its spell's page
    // (1194:1B3C).
    if m.mem.r8(DS, OWN_ITEMS) == 0 {
        inv.page = SpellPage::open(m, files);
        return;
    }
    if m.equip_click(said, mark as u16).is_none() {
        eprintln!("equipping gave up: {}", arena::giveup::describe());
    }
    if let Some(t) = said.panel.take() {
        inv.info = Some(vec![(t, 0xA0)]);
        inv.info_y = Some(0xA0);
    }
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
        font: None,
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
        font: None,
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

/// The inventory line under (x, y) on the 320 x 200 screen, within the
/// list's click record (the first of 1194:2325's five at 3276:57D8: 0Eh,
/// 32h, 8Eh x 4Dh, to 1194:1AC6),
/// as the lines are drawn (the click marks by 1C5E's rows, `mark_at`).
pub fn line_at(x: i32, y: i32) -> Option<usize> {
    if !(0x0E..0x0E + 0x8E).contains(&x) || !(0x32..0x32 + 0x4D).contains(&y) {
        return None;
    }
    let row = ((y - LIST_Y + 2) / LINE_H) as usize;
    Some(row.min(LIST_LINES - 1))
}

/// The list's arrows (their click records after the list's: 8 x 8 at
/// 0Ch, 80h, key '+', 1194:1A4A; at 95h, 80h, key '-', 1A94): the
/// lines moved on (+1) or back (-1).
pub fn arrow_at(x: i32, y: i32) -> Option<i32> {
    if !(0x80..0x88).contains(&y) {
        return None;
    }
    match x {
        0x0C..0x14 => Some(1),
        0x95..0x9D => Some(-1),
        _ => None,
    }
}

/// 1194:1A4A / 1A94: the first line shown one on, while seven or more
/// lines (the items, or the book's spells) leave lines below; or one
/// back, unless at the first.
pub fn arrow(m: &mut Machine, files: &arena::bsa::Files, d: i32) {
    let first = m.mem.r8(DS, FIRST_LINE);
    if d < 0 {
        if first != 0 {
            m.mem.w8(DS, FIRST_LINE, first - 1);
        }
        return;
    }
    // 1A4A: the items (129D) and the quests' lines (126C), or the book's.
    let n = if m.mem.r8(DS, OWN_ITEMS) == 0 { m.mem.r8(DS, BOOK_COUNT) as u16 } else { (item_lines(m).len() + quest_lines(m, files).len()) as u16 };
    if n >= 7 && first as u16 != n - 7 {
        m.mem.w8(DS, FIRST_LINE, first.wrapping_add(1));
    }
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
pub fn draw(screen: &mut Screen, font: &Font, equip: Option<&Image>, doll: &[Image], panel: &Panel, m: &Machine, archive: Option<(&crate::archive::Archive, &arena::bsa::Files)>) {
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
            if let Some(f) = pics.as_ref().and_then(|p| p.arena.as_ref()).filter(|_| !inv.bare) {
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
            let frame = |screen: &mut Screen, y: i32| {
                let (fx, fy, fw, fh) = (LIST_X - 2, y - 2, 146, 12);
                screen.fill(fx, fy, fw, 1, 0xA0, 0xFF);
                screen.fill(fx, fy + fh - 1, fw, 1, 0xA0, 0xFF);
                screen.fill(fx, fy, 1, fh, 0xA0, 0xFF);
                screen.fill(fx + fw - 1, fy, 1, fh, 0xA0, 0xFF);
            };
            if m.mem.r8(DS, OWN_ITEMS) == 0 {
                // 1194:2229: the spells' page, SPELLBK.IMG and EQUIPB.IMG
                // over EQUIP.IMG; 1D5C: the book's spells in its order
                // from FIRST_LINE, seven at most, in 53h, the marked one
                // framed.
                for img in pics.iter().filter(|_| !inv.bare).flat_map(|p| [p.spellbk.as_ref(), p.equipb.as_ref()]).flatten() {
                    screen.blit(img.x, img.y, img.w, img.h, &img.px, 199);
                }
                for (i, name) in book_names(m).iter().enumerate().skip(first as usize).take(LIST_LINES) {
                    let y = LIST_Y + (i as i32 - first as i32) * LINE_H;
                    if i == selected as usize {
                        frame(screen, y);
                    }
                    screen.text_plain(teeny, LIST_X, y, &String::from_utf8_lossy(name), 0x53);
                }
            } else {
                // 1DE9: seven slots from the top (to 28h), the taken ones
                // drawn (126E counting them).
                let shown = (LIST_LINES as u16).min(0x28u16.saturating_sub(first));
                let mut taken = 0;
                for l in item_lines(m) {
                    if l.slot < first || l.slot >= first + shown {
                        continue;
                    }
                    taken += 1;
                    let y = LIST_Y + (l.slot - first) as i32 * LINE_H;
                    if l.slot as u8 == selected {
                        frame(screen, y);
                    }
                    screen.text_plain(teeny, LIST_X, y, &l.name, l.colour);
                }
                // 1EB1: with rows left, the quests' lines in A0h, from the
                // first of them whatever the top (the original's), from
                // the row after the items' at 33h (a pixel below theirs),
                // the one at 126E + the top framed if marked.
                if taken < LIST_LINES {
                    if let Some((_, files)) = archive {
                        for (i, t) in quest_lines(m, files).iter().take(LIST_LINES - taken).enumerate() {
                            let row = taken + i;
                            let y = 0x33 + row as i32 * LINE_H;
                            if (row as u8).wrapping_add(first as u8) == selected {
                                frame(screen, y);
                            }
                            screen.text_plain(teeny, LIST_X, y, t, 0xA0);
                        }
                    }
                }
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
            // 1194:223A: the description's lines in ARENAFNT ([102C])
            // centred at x 55h, from y 90h (95h unless four), 0Bh apart.
            if let (Some(lines), Some(f)) = (&inv.info, pics.as_ref().and_then(|p| p.arena.as_ref())) {
                let y0 = inv.info_y.unwrap_or(if lines.len() == 4 { 0x90 } else { 0x95 });
                for (i, (t, c)) in lines.iter().enumerate() {
                    screen.text_plain(f, 0x55 - (f.width(t) as i32 >> 1), y0 + 0x0B * i as i32, t, *c);
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
            if let Some(p) = &inv.page {
                if let Some(archive) = archive {
                    crate::spellmaker::draw_state(screen, font, archive.0, archive.1, &p.shown.0, None);
                }
                if let (true, Some((archive, _)), Some(f)) = (p.asking.is_some(), archive, pics.as_ref().and_then(|p| p.arena.as_ref())) {
                    if let Some(tiles) = SpellPage::tiles(archive) {
                        let menu = p.menu();
                        crate::hud::custom::draw_menu(screen, &tiles, f, &menu.title, &menu.entries, 0xFE);
                    }
                }
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
