//! The player's answers to the game: menus picked, hours typed, message
//! boxes read.
//!
//! The game asks in the middle of a frame (a hook must return the pick
//! there and then), while the window can only wait for a key between
//! frames. So a turn (an action, like a click or camping, then a frame) is
//! run from a copy of the engine: when the game asks something not yet
//! answered, the hook leaves it (as Esc would), the turn's result is
//! thrown away, and the question is shown; once the player answers, the
//! turn runs again from the copy with the answers given so far. The game
//! is deterministic, so it asks the same questions in the same order.
//!
//! Message boxes don't wait inside the turn: the turn goes on, and the
//! boxes are shown after it (with the question that stopped it, if any),
//! the game waiting until they are read.

use crate::hud::{self, Menu, Span};
use arena::machine::Machine;
use arena::mem::DS;
use std::collections::VecDeque;

/// A click on the attribute screen (1194:122C's, as creation's):
/// an attribute's line picked (`Machine::creation_select`), or the arrows
/// clicked at a row (`creation_arrow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Select(u16),
    Arrow(u16),
    /// The name typed over the sheet's (1194:1528: into 01AD, 14h
    /// characters at most, its 0 after).
    Name([u8; 0x15]),
}

/// An answer given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// A menu's entry (FFFFh: none, Esc).
    Pick(u16),
    /// Text typed (a script's): Esc where the field doesn't end on it
    /// (it empties the field), its characters, Enter.
    Text(String),
    /// A typed field's passes' inputs (`crate::field::Live`), as typed.
    Keys(Vec<arena::lineedit::EditKey>),
    /// A level's points spent: the clicks, in order.
    Spend(Vec<Click>),
    /// A click at (x, y) of the 320 x 200 screen (the spellmaker's).
    Click(u16, u16),
    /// The travel map shown on the way out of a dungeon (26F7:0000): the
    /// game as its screen ended, and its carry.
    Travel(World, bool),
}

/// The game and its files at a point of a turn: where the travel map
/// began (a question's) or ended (its answer). The turn run again
/// reaches the same point, and an answer's game goes on from there.
#[derive(Clone)]
pub struct World(pub Box<(Machine, arena::bsa::Files)>);

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("World")
    }
}

impl PartialEq for World {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(&*self.0, &*other.0)
    }
}

impl Eq for World {}

/// The game's state when it asked (the level-up screen shows the
/// attributes as they are and as the clicks change them).
#[derive(Clone)]
pub struct Snapshot(pub Box<Machine>);

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Snapshot")
    }
}

/// A question waiting for its answer.
#[derive(Clone, Debug)]
pub enum Need {
    Menu(Menu),
    /// A field to type in (0000:9EE9).
    Text(Ask),
    /// A level's points to spend (1194:122C), on the attribute screen.
    Spend(Snapshot),
    /// A click on the spellmaker's screen (1194:5651), or on its form
    /// open (its record at DATA:form): the screen is `Talk::maker`.
    Spellmaker,
    /// The travel map, from the game as the way out of a dungeon showed
    /// it (26F7:0000 with 1A81 set).
    Travel(World),
}

/// What a typed field's caller shows around it.
#[derive(Clone, Debug)]
pub enum Prompt {
    /// A box with the prompt, centred (965A: 1194:4800's, 5C30's), in
    /// the colour its caller left (9288) and its clip as it asked (A914..
    /// A91A: the rest's bottom 92h, 1194:4252; the turn has put it back
    /// by the time the box is drawn).
    Boxed(String, u8, (i32, i32, i32, i32)),
    /// The dialog's lines (1194:4E88's, at 20h, 91h on, 9 apart, in
    /// TEENYFNT.DAT, colour C0h), the field under them.
    Lines(Vec<Vec<Span>>),
    /// Nothing: the field on the caller's screen (the automap's notes,
    /// the spellmaker's name, the save menu's).
    Bare,
}

/// A typed field asked: what is shown around it, the field (where, how
/// many characters, the text), how its text is drawn, and the game as it
/// asked (the window's editor runs on a copy, `crate::field::Live`).
#[derive(Clone, Debug)]
pub struct Ask {
    pub prompt: Prompt,
    pub field: arena::lineedit::Field,
    pub look: crate::field::Look,
    pub game: Snapshot,
}

/// A turn's answers and what it showed.
#[derive(Clone, Debug, Default)]
pub struct Talk {
    /// The answers still to give, in order.
    pub answers: VecDeque<Answer>,
    /// The question the turn stopped at.
    pub need: Option<Need>,
    /// The message boxes shown, and those since the last answer given.
    pub boxes: Vec<Vec<Vec<Span>>>,
    pub since_answer: Vec<Vec<Vec<Span>>>,
    /// The rest's hours, last shown.
    pub status: Option<String>,
    /// The animations the turn played (1194:D45D, 2294:0895): the file,
    /// the text's number shown with it (as a vision, 1194:D459) and its
    /// colour; the front end plays them after the turn.
    pub movies: Vec<(&'static str, Option<u16>, u8)>,
    /// The spellmaker's screen behind its questions: the game as its last
    /// click was asked, and the form open then (DATA:form). Kept when the
    /// turn stops inside the spellmaker.
    pub maker: Option<(Snapshot, Option<u16>)>,
    /// The shop's screen the turn is inside (`crate::shopui::Backdrop`):
    /// drawn behind its questions.
    pub backdrop: Option<crate::shopui::Backdrop>,
    /// The screen faded to black on the way into a building (1194:55B2,
    /// the trigger's way in 0000:4118): the area's change starts black.
    pub blackout: bool,
}

/// The game's text at DS:`at`, as lines.
pub fn text_at(m: &Machine, at: u16, colour: u8) -> Vec<Vec<Span>> {
    hud::lines(m.mem.cstr(DS, at), colour)
}

/// The menu 1194:020E shows: its title at DS:`si`, its keys (a letter an
/// entry) at DS:`bx`, its entries' table (a word each, 0 ending it) at
/// DS:`di`.
pub fn menu_at(m: &Machine, si: u16, bx: u16, di: u16) -> Menu {
    // 3159:0F2A draws its boxes through 965A, in the colour its caller
    // left (9288) as a message box is.
    let colour = m.mem.r8(DS, 0x9288);
    let title = text_at(m, si, colour);
    let mut entries = Vec::new();
    for i in 0..32u16 {
        let p = m.mem.r16(DS, di.wrapping_add(2 * i));
        if p == 0 {
            break;
        }
        let mut l = text_at(m, p, colour);
        entries.push(l.swap_remove(0));
    }
    let keys = m.mem.cstr(DS, bx).to_vec();
    // 3159:0F2A draws it in the font selected as it is asked (B036).
    Menu { title, entries, keys, selected: 0, list: false, kind: 0, font: Some(hud::GameFont::selected(m)), top: None }
}

impl Talk {
    /// A message box with the text at DS:`si`.
    pub fn message_box(&mut self, m: &Machine, si: u16) {
        if self.need.is_some() {
            return;
        }
        // In the font the caller selected (B036) and the colour it left
        // (9288), as 965A draws it.
        let lines = hud::in_font(text_at(m, si, m.mem.r8(DS, 0x9288)), hud::GameFont::selected(m));
        for l in &lines {
            println!("{}", hud::plain(l));
        }
        // Centred in the clip as it is asked (A914..A91A).
        let r = |at| m.mem.r16(DS, at) as i16 as i32;
        let lines = hud::clipped(lines, (r(0xA914), r(0xA916), r(0xA918), r(0xA91A)));
        self.boxes.push(lines.clone());
        self.since_answer.push(lines);
    }

    /// 1194:4D31's message: the strip under the shop's menu saved (5F8C),
    /// the text at DS:`si` in TEENYFNT.DAT ([102E], 0000:9AE6) and colour
    /// C0h at (23h, 91h) (0000:9E06), read with a click, then the strip
    /// put back (5FC0).
    pub fn strip_message(&mut self, m: &Machine, si: u16) {
        if self.need.is_some() {
            return;
        }
        let text = m.mem.cstr(DS, si).to_vec();
        println!("{}", String::from_utf8_lossy(&text));
        let lines = hud::placed(0x23, 0x91, &text, 0xC0, hud::GameFont::Teeny);
        self.boxes.push(lines.clone());
        self.since_answer.push(lines);
    }

    /// 1194:4E88 with 13D3 0: the dialog's lines in the strip under the
    /// shop's menu (20h, 91h on, 9 apart, TEENYFNT, C0h), three to a page,
    /// each page read with a click.
    pub fn dialog_lines(&mut self, lines: &[Vec<u8>]) {
        if lines.is_empty() {
            return;
        }
        let last = self.dialog_pages(lines);
        self.dialog_page(last, false);
    }

    /// 1194:4E88's pages of `lines` (1194:4E54's split: as many pages as
    /// full threes and one) but the last, each read with a click, "...More"
    /// under it (`hud::more_line`: a page full with lines left, 4EA0); the
    /// last page's lines, which its kind (13D3) shows with its question.
    pub fn dialog_pages<'a>(&mut self, lines: &'a [Vec<u8>]) -> &'a [Vec<u8>] {
        let n = lines.len().saturating_sub(1) / 3 * 3;
        for page in lines[..n].chunks(3) {
            self.dialog_page(page, true);
        }
        &lines[n..]
    }

    fn dialog_page(&mut self, page: &[Vec<u8>], more: bool) {
        if self.need.is_some() {
            return;
        }
        let page: Vec<&[u8]> = page.iter().map(|l| l.as_slice()).collect();
        for l in &page {
            println!("{}", String::from_utf8_lossy(l));
        }
        let mut b = hud::placed_lines(0x20, 0x91, 9, &page, 0xC0, hud::GameFont::Teeny);
        if more {
            b.push(hud::more_line());
        }
        self.boxes.push(b.clone());
        self.since_answer.push(b);
    }


    /// A menu: the next answer, or the question left (FFFFh).
    pub fn menu(&mut self, m: &Machine, si: u16, bx: u16, di: u16) -> u16 {
        if self.need.is_some() {
            return 0xFFFF;
        }
        let menu = menu_at(m, si, bx, di);
        self.ask(menu)
    }

    /// A list of `cx` strings one after another at DS:`si` (1194:5734:
    /// the loot window's), under `title`: the next answer, or the
    /// question left (FFFFh).
    pub fn list(&mut self, m: &Machine, si: u16, cx: u16, title: &str) -> u16 {
        if self.need.is_some() {
            return 0xFFFF;
        }
        let mut at = si;
        let mut entries = Vec::new();
        for _ in 0..cx.min(64) {
            let s = m.mem.cstr(DS, at);
            at = at.wrapping_add(s.len() as u16 + 1);
            // An entry of several parts (the tavern's "\t200%u gp\n%s") on
            // one line: a part without a column of its own starts back at
            // the list's x (2F26:2E12), marked `\t000`.
            let mut entry: Vec<Span> = Vec::new();
            for l in hud::lines(s, 0x0F) {
                if !entry.is_empty() && !l.is_empty() {
                    entry.push(Span { colour: 0x0F, text: "\t000".into(), font: Default::default(), code: None });
                }
                entry.extend(l);
            }
            entries.push(entry);
        }
        let title = if title.is_empty() { Vec::new() } else { hud::lines(title.as_bytes(), 0x0F) };
        let menu = hud::list_menu(m, title, entries);
        self.ask(menu)
    }

    /// A menu made by the front end (the shops' yes or no, the haggle's
    /// moves): the next answer, or the question left (FFFFh).
    pub fn ask_menu(&mut self, menu: Menu) -> u16 {
        if self.need.is_some() {
            return 0xFFFF;
        }
        self.ask(menu)
    }

    /// Lines shown in a box, as a message box's.
    pub fn lines_box(&mut self, lines: Vec<Vec<Span>>) {
        if self.need.is_some() {
            return;
        }
        for l in &lines {
            println!("{}", hud::plain(l));
        }
        self.boxes.push(lines.clone());
        self.since_answer.push(lines);
    }

    /// `menu` asked: the next answer, or the question left (FFFFh).
    fn ask(&mut self, menu: Menu) -> u16 {
        match self.answers.front() {
            Some(Answer::Pick(p)) => {
                // Only an entry shown can be picked (1194:020E, 5734 give
                // an entry or Esc): an answer past them is Esc.
                let p = if (*p as usize) < menu.entries.len() { *p } else { 0xFFFF };
                self.answers.pop_front();
                self.since_answer.clear();
                println!("[{}] -> {}", menu.title.iter().map(|l| hud::plain(l)).collect::<Vec<_>>().join(" ").trim(), p as i16);
                p
            }
            _ => {
                self.need = Some(Need::Menu(menu));
                0xFFFF
            }
        }
    }

    /// A typed field (0000:9EE9): the next answer given to the game's
    /// editor on `m` (`crate::field::replay`), or the question left (the
    /// text as it was).
    pub fn edit(&mut self, m: &mut Machine, field: arena::lineedit::Field, look: crate::field::Look, prompt: Prompt) {
        if self.need.is_some() {
            return;
        }
        let keys = match self.answers.front() {
            Some(Answer::Keys(k)) => k.clone(),
            Some(Answer::Text(t)) => crate::field::keys_of(t, m.mem.r16(DS, arena::lineedit::var::FLAGS) & 0x1000 != 0),
            _ => {
                self.need = Some(Need::Text(Ask { prompt, field, look, game: Snapshot(Box::new(m.clone())) }));
                return;
            }
        };
        self.answers.pop_front();
        self.since_answer.clear();
        if crate::field::replay(m, &field, &look, &keys) != Some(true) {
            eprintln!("the typed field gave up: {}", arena::giveup::describe());
        }
        println!("[typed] {:?}", String::from_utf8_lossy(m.mem.cstr(field.seg, field.off)));
    }
}

impl Talk {
    /// A level's points spent (`LevelUpHooks::spend`): the clicks given,
    /// or the question left. Left, the points still go (in turn, the rest
    /// dropped), as the level-up loops until none are left: the turn is
    /// thrown away anyway.
    pub fn spend(&mut self, m: &mut Machine) {
        let leave = |m: &mut Machine| {
            arena::levelup::spend_in_turn(m);
            m.mem.w8(DS, arena::newgame::var::POINTS, 0);
        };
        if self.need.is_some() {
            return leave(m);
        }
        match self.answers.front() {
            Some(Answer::Spend(clicks)) => {
                let clicks = clicks.clone();
                self.answers.pop_front();
                self.since_answer.clear();
                for c in clicks {
                    match c {
                        Click::Select(r) => m.creation_select(r),
                        Click::Arrow(y) => m.creation_arrow(y),
                        Click::Name(n) => {
                            for (i, &b) in n.iter().enumerate() {
                                m.mem.w8(DS, 0x01AD + i as u16, b);
                            }
                        }
                    }
                }
                println!("[points spent]");
            }
            _ => {
                self.need = Some(Need::Spend(Snapshot(Box::new(m.clone()))));
                leave(m);
            }
        }
    }
}

impl Talk {
    /// The spellmaker's next click (1194:5651), on its screen or on the
    /// form open (DATA:`form`): the click given, or None (the question
    /// left; the caller clicks as leaving does). The screen as asked is
    /// kept (`maker`).
    pub fn spell_click(&mut self, m: &Machine, form: Option<u16>) -> Option<(u16, u16)> {
        if self.need.is_some() {
            return None;
        }
        self.maker = Some((Snapshot(Box::new(m.clone())), form));
        match self.answers.front() {
            Some(Answer::Click(x, y)) => {
                let c = (*x, *y);
                self.answers.pop_front();
                self.since_answer.clear();
                println!("[spellmaker click {}, {}]", c.0, c.1);
                Some(c)
            }
            _ => {
                self.need = Some(Need::Spellmaker);
                None
            }
        }
    }
}

impl Talk {
    /// 26F7:0000 on the way out of a dungeon: the travel map's answer
    /// (the game as it ended taken on, its carry), or the question left
    /// (stay: no journey made).
    pub fn travel(&mut self, m: &mut Machine, files: &mut arena::bsa::Files) -> bool {
        if self.need.is_some() {
            return false;
        }
        match self.answers.front() {
            Some(Answer::Travel(w, carry)) => {
                let carry = *carry;
                m.clone_from(&w.0 .0);
                files.clone_from(&w.0 .1);
                self.answers.pop_front();
                self.since_answer.clear();
                println!("[travel map] -> {}", if carry { "left" } else { "stayed" });
                carry
            }
            _ => {
                self.need = Some(Need::Travel(World(Box::new((m.clone(), files.clone())))));
                false
            }
        }
    }
}

/// `--picks`: entries' numbers, `tTEXT` for text typed, or `cX:Y` for a
/// click at (X, Y) of the screen (the spellmaker's), comma-separated.
pub fn parse_answers(s: &str) -> VecDeque<Answer> {
    s.split(',')
        .filter(|a| !a.is_empty())
        .map(|a| {
            if let Some(t) = a.strip_prefix('t') {
                return Answer::Text(t.to_string());
            }
            if let Some((x, y)) = a.strip_prefix('c').and_then(|c| c.split_once(':')) {
                return Answer::Click(x.parse().expect("--picks cX:Y"), y.parse().expect("--picks cX:Y"));
            }
            Answer::Pick(a.parse::<i32>().expect("--picks N,..,tTEXT,cX:Y") as u16)
        })
        .collect()
}
