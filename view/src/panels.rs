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
use arena::use_item::var::{FIRST_LINE, NOT_OWN, OWN_ITEMS, SELECTED, USABLE_NAMES};

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
    /// in the bar's corner (2542:066A), the line picked and the first line
    /// shown (3159:06A0's record +39h and +3Bh, both 0 as it comes up).
    Use(Vec<String>, usize, usize),
}

/// The sheet's equipment page as the window shows it.
#[derive(Clone, Debug, Default)]
pub struct Inventory {
    /// The drop button's question up (its text, the answer the arrows
    /// marked: 0 yes, none until they move).
    pub asking: Option<(Vec<u8>, usize)>,
    /// That question as 1194:48E9's 020E shows it (`dialog::menu_at` on
    /// the game as it asked: the text at DROP_TEXT, the keys at 4B60, the
    /// entries at 4B63, its colour and font).
    pub asked_menu: Option<Box<crate::hud::Menu>>,
    /// On the sheet's own page, the key interrupt's ring (AC46..ACB4) as
    /// the drop's key left it: the answer's key, which the window's key
    /// interrupt puts after it, goes in after the menu's flush instead
    /// (`Machine::sheet_equipment_drop`'s pick), as the original's does;
    /// and the drop's key (None, its button clicked).
    pub drop_ring: Option<Box<(Vec<u8>, Option<u8>)>>,
    /// On the sheet's own page, a drop under way: its question or "Drop
    /// what?" up (1194:18CA), and what it has taken (its loop's inputs,
    /// 2325, and the questions' answers), the drop run again from its
    /// start with them (`App::sheet_drop`).
    pub drop_what: Option<Vec<crate::DropStep>>,
    /// While "Drop what?" waits, the list's first line shown (1297) as its
    /// loop's arrows left it on the drop's copy, the game's untouched
    /// until the drop is done.
    pub drop_first: Option<u8>,
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
    /// Opened from the sheet's equipment page (`Machine::sheet_spell_page`:
    /// its loop's turn ended after), not the view's own.
    sheet: bool,
    answers: Vec<PageAnswer>,
    pub shown: crate::dialog::Snapshot,
    pub asking: Option<(Vec<u8>, usize)>,
    /// The pages drawn on the way (68D7, each setting its colours).
    pub pages: usize,
    /// The name's field asked (1194:65FD), when that is what waits: the
    /// field, its look and the game as it asked it.
    pub typing: Option<Box<(arena::lineedit::Field, crate::field::Look, crate::dialog::Snapshot)>>,
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
    fn run(start: &Machine, sheet: bool, files: &mut arena::bsa::Files, answers: &[PageAnswer], m: &mut Machine) -> Option<SpellPage> {
        let mut game = start.clone();
        let mut fs = files.clone();
        let mut r = Replay { answers: answers.iter().cloned().collect(), waiting: false, shown: None, asking: None, pages: 0, typing: None };
        let left = if sheet { game.sheet_spell_page(&mut fs, &mut r) } else { game.spell_page(&mut fs, &mut r) };
        if left.is_none() {
            eprintln!("the spell's page gave up: {}", arena::giveup::describe());
        }
        match r.shown {
            Some(shown) if r.waiting => Some(SpellPage {
                start: crate::dialog::Snapshot(Box::new(start.clone())),
                sheet,
                answers: answers.to_vec(),
                shown: crate::dialog::Snapshot(Box::new(shown)),
                asking: r.asking.map(|t| (t, 0)),
                pages: r.pages,
                typing: r.typing.map(Box::new),
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

    /// The Delete question's entry a click at y picks (`menu_entry`): its
    /// boxes as 0F2A lays them out in those tiles and ARENAFNT ([102C]).
    pub fn menu_pick(&self, archive: &crate::archive::Archive, _x: i32, y: i32) -> Option<usize> {
        let pics = crate::sheets::loaded()?;
        let f = pics.arena.as_ref()?;
        let tiles = SpellPage::tiles(archive)?;
        let menu = self.menu();
        let (_, es) = crate::hud::custom::menu_rects(&tiles, f, &menu.title, &menu.entries);
        crate::hud::custom::menu_entry(&es, y)
    }

    /// 1194:1B3C from the sheet's equipment page's list
    /// (`Machine::sheet_spell_page`).
    pub fn open_sheet(m: &mut Machine, files: &mut arena::bsa::Files) -> Option<SpellPage> {
        let start = m.clone();
        SpellPage::run(&start, true, files, &[], m)
    }

    /// An answer given: the page as it now stands, None once left.
    pub fn answer(self, a: PageAnswer, m: &mut Machine, files: &mut arena::bsa::Files) -> Option<SpellPage> {
        let mut answers = self.answers;
        answers.push(a);
        SpellPage::run(&self.start.0, self.sheet, files, &answers, m)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Spellbook {
    /// The line picked (3159:06A0's, from the first).
    pub selected: usize,
    /// The first line shown (the list's record +3Bh, 0 as it comes up).
    pub top: usize,
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
        let (names, selected, top) = match self {
            Panel::Spellbook(book) => {
                let names = book_names(m);
                let order = book_order(&names).unwrap_or_else(|| (0..names.len() as u8).collect());
                (order.iter().map(|&o| String::from_utf8_lossy(&names[o as usize]).into_owned()).collect(), book.selected, book.top)
            }
            Panel::Use(names, sel, top) => (names.clone(), *sel, *top),
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
            top: Some(top),
        })
    }

    /// A key on the spellbook or the use list: its line marked and first
    /// line shown moved as 3159:06A0 moves them (`hud::list_key`: the
    /// arrows stop at the ends).
    pub fn bar_list_key(&mut self, m: &Machine, key: crate::hud::ListKey) {
        let Some(mut menu) = self.bar_list(m) else { return };
        crate::hud::list_key(&mut menu, key);
        let top = menu.top.unwrap_or(0);
        match self {
            Panel::Spellbook(book) => (book.selected, book.top) = (menu.selected, top),
            Panel::Use(_, sel, t) => (*sel, *t) = (menu.selected, top),
            _ => {}
        }
    }
}

/// The sheet's equipment page ('N') as the window shows it, its exit
/// back to the sheet; its state the game's (`Machine::sheet_equipment`,
/// 1194:16E0).
pub fn equipment_page() -> Panel {
    Panel::Inventory(Inventory::default())
}

/// For scripts: the sheet opened (the portrait: where the player stands
/// checked, 1194:5F13; `Machine::sheet_open`, `dac` the colours shown,
/// 6-bit) and turned to its equipment page (N), all in the game's state.
pub fn script_page(engine: &mut arena::engine::Engine, dac: &[[u8; 3]; 256]) -> Panel {
    engine.m.drop_place();
    if engine.m.sheet_open(&mut engine.files, Some(dac)).is_none() || engine.m.sheet_equipment(&mut engine.files, Some(b'n')).is_none() {
        eprintln!("the equipment page gave up: {}", arena::giveup::describe());
    }
    equipment_page()
}

/// For scripts: the page's list line `row` (on screen) clicked as its
/// record takes a click (`Machine::sheet_equipment_click`): marked, and
/// with `again` clicked once more, its item put on or taken off (1AFB).
/// A line marked already is clicked only the once `again` asks for.
pub fn script_line(engine: &mut arena::engine::Engine, row: usize, again: bool, said: &mut Said) {
    // A row within the line: its bottom 3Dh, each next 0Bh lower (1C5E).
    let y = 0x3D + 0x0B * row as u16 - 4;
    let slot = engine.m.mem.r8(DS, FIRST_LINE).wrapping_add(row as u8);
    let clicks = match (engine.m.mem.r8(DS, SELECTED) == slot, again) {
        (true, true) | (false, false) => 1,
        (false, true) => 2,
        (true, false) => 0,
    };
    for _ in 0..clicks {
        let mut wait = |_: &mut Machine, lines: &[(Vec<u8>, u8)], _: u16| {
            if let Some((t, _)) = lines.first() {
                said.panel = Some(String::from_utf8_lossy(t).into_owned());
            }
        };
        if engine.m.sheet_equipment_click(&mut engine.files, y, &mut wait).is_none() {
            eprintln!("the list's click gave up: {}", arena::giveup::describe());
            return;
        }
    }
}

/// For scripts: the page's Drop button clicked (`Machine::sheet_equipment_drop`),
/// its question (if it asks) answered `yes` or no, and "Drop what?" left
/// by its exit's key (Esc).
pub fn script_drop(engine: &mut arena::engine::Engine, yes: bool, said: &mut Said) {
    let mut wait = |_: &mut Machine, lines: &[(Vec<u8>, u8)], _: u16| {
        said.boxes.push(lines.iter().flat_map(|(t, _)| crate::hud::lines(t, 0x0F)).collect());
    };
    let r = engine.m.sheet_equipment_drop(&mut engine.files, None, None, &mut |_| if yes { 0 } else { 1 }, &mut wait, &mut |_| arena::charsheet::PageInput::Key(0x1B));
    if r.is_none() {
        eprintln!("the drop gave up: {}", arena::giveup::describe());
    }
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
    /// The page as the frame's input step finds it, for a list that keeps
    /// it (the bar's, 2542:06F8: `App::list_page`).
    pub screen: Option<Vec<u8>>,
    /// The pick given straight to the use button's code, without its list
    /// (as gen_worldframes.py's `used` calls 24FA:0AFE, 1194:7AE7 and
    /// 29CF:07C3).
    pub listless: bool,
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
    /// 24FA:0B78: the list in the bar's corner (`Machine::bar_list`) with
    /// the line the player picked (FFFFh: left).
    fn pick(&mut self, m: &mut Machine, count: u16) -> u16 {
        let pick = self.pick.take().unwrap_or(0xFFFF);
        if self.listless {
            return pick;
        }
        m.bar_list(self.screen.as_deref(), (DS, arena::use_item::var::USABLE_NAMES), count, pick).unwrap_or_else(|| {
            eprintln!("the use list gave up: {}", arena::giveup::describe());
            0xFFFF
        })
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

/// The drop's question as the original's yes-or-no menu (1194:48E9 ->
/// 020E), entry `sel` highlighted.
/// The drop's question as it is up (`Inventory::asked_menu`, else
/// `drop_menu`), the entry the arrows marked.
pub fn drop_question(inv: &Inventory) -> Option<crate::hud::Menu> {
    let (text, sel) = inv.asking.as_ref()?;
    let mut menu = inv.asked_menu.as_deref().cloned().unwrap_or_else(|| drop_menu(text, 0));
    menu.selected = *sel;
    Some(menu)
}

/// The drop question's boxes' tiles: the sheet's, MARBLE2.CIF's 3 x 3
/// ([103A], made B038's as the sheet opens, 1194:0CF8).
pub fn drop_tiles(archive: &crate::archive::Archive) -> Option<crate::hud::Tiles> {
    crate::hud::Tiles::load(archive, "MARBLE2.CIF", 3, 3)
}

/// The font the drop question is drawn in: the one selected as it was
/// asked (B036, `Menu::font`).
fn drop_font(menu: &crate::hud::Menu) -> Option<&'static crate::hud::Font> {
    crate::hud::game_font(menu.font?)
}

/// The drop question's boxes as 3159:0F2A lays them out in those tiles
/// and that font: its title's and its entries'.
#[allow(clippy::type_complexity)]
pub fn drop_rects(archive: &crate::archive::Archive, menu: &crate::hud::Menu) -> Option<(Option<(i32, i32, i32, i32)>, Vec<(i32, i32, i32, i32)>)> {
    let tiles = drop_tiles(archive)?;
    Some(crate::hud::custom::menu_rects(&tiles, drop_font(menu)?, &menu.title, &menu.entries))
}

/// The drop question's entry a click at y picks (`menu_entry`).
pub fn drop_pick(archive: &crate::archive::Archive, menu: &crate::hud::Menu, y: i32) -> Option<usize> {
    crate::hud::custom::menu_entry(&drop_rects(archive, menu)?.1, y)
}

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
        top: None,
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

/// The spellbook's entry `sel` cast (29CF:071F); a spell with a target
/// then waits for a click in the view, which aims it.
pub fn cast(engine: &mut arena::engine::Engine, sel: usize, said: &mut Said) {
    if let Some(s) = spells(&engine.m).get(sel) {
        if engine.m.cast_spell(&mut engine.files, said, s.index).is_none() {
            eprintln!("the cast gave up: {}", arena::giveup::describe());
        }
    }
}

/// The spellbook's list left with line `line` picked (`usize::MAX`:
/// nothing; `Machine::spellbook_list`, its screen `said`'s) and the spell
/// cast (29CF:071F); a spell with a target then waits for a click in the
/// view, which aims it.
pub fn book_cast(engine: &mut arena::engine::Engine, line: usize, said: &mut Said) {
    let pick = if line == usize::MAX { 0xFFFF } else { line as u16 };
    let screen = said.screen.take();
    match engine.m.spellbook_list(&mut engine.files, screen.as_deref(), pick) {
        Some(Some(n)) => {
            if engine.m.cast_spell(&mut engine.files, said, n).is_none() {
                eprintln!("the cast gave up: {}", arena::giveup::describe());
            }
        }
        Some(None) => {}
        None => eprintln!("the spellbook's list gave up: {}", arena::giveup::describe()),
    }
}

/// The spellbook's last spell cast again without the list (C with a
/// Shift, or the right button's release `right`: `Machine::spellbook_last`,
/// 29CF:071F); false where the list is to come up instead.
pub fn book_cast_last(engine: &mut arena::engine::Engine, right: bool, said: &mut Said) -> bool {
    let Some(n) = engine.m.spellbook_last(right) else { return false };
    if engine.m.cast_spell(&mut engine.files, said, n).is_none() {
        eprintln!("the cast gave up: {}", arena::giveup::describe());
    }
    true
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
            let first = inv.drop_first.unwrap_or_else(|| m.mem.r8(DS, FIRST_LINE)) as u16;
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
            if let Some(menu) = drop_question(inv) {
                // 3159:0F2A's boxes in the sheet's tiles and font; the
                // view marks the entry the arrows are on.
                let drawn = archive.and_then(|(a, _)| Some((drop_tiles(a)?, drop_rects(a, &menu)?)));
                match (drawn, drop_font(&menu)) {
                    (Some((tiles, (_, rects))), Some(f)) => {
                        crate::hud::custom::draw_menu(screen, &tiles, f, &menu.title, &menu.entries, 0x60);
                        if let Some(&(x, y, w, h)) = rects.get(menu.selected) {
                            for (fx, fy, fw, fh) in [(x + 2, y + 2, w - 4, 1), (x + 2, y + h - 3, w - 4, 1), (x + 2, y + 2, 1, h - 4), (x + w - 3, y + 2, 1, h - 4)] {
                                screen.fill(fx, fy, fw, fh, 0x60, 0xFF);
                            }
                        }
                    }
                    _ => {
                        let hud = crate::hud::Hud { menu: Some(menu), ..Default::default() };
                        crate::hud::compose_over(screen, font, &hud);
                    }
                }
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
        return m.kill_check(seg, rec, &mut arena::strike::NoKillHooks).is_some();
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
