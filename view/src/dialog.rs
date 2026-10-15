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
}

/// An answer given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// A menu's entry (FFFFh: none, Esc).
    Pick(u16),
    /// Text typed (the rest's hours).
    Text(String),
    /// A level's points spent: the clicks, in order.
    Spend(Vec<Click>),
}

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
    /// Text to type, its prompt.
    Text(String),
    /// A level's points to spend (1194:122C), on the attribute screen.
    Spend(Snapshot),
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
}

/// The game's text at DS:`at`, as lines.
pub fn text_at(m: &Machine, at: u16, colour: u8) -> Vec<Vec<Span>> {
    hud::lines(m.mem.cstr(DS, at), colour)
}

/// The menu 1194:020E shows: its title at DS:`si`, its keys (a letter an
/// entry) at DS:`bx`, its entries' table (a word each, 0 ending it) at
/// DS:`di`.
pub fn menu_at(m: &Machine, si: u16, bx: u16, di: u16) -> Menu {
    let title = text_at(m, si, 0x0F);
    let mut entries = Vec::new();
    for i in 0..32u16 {
        let p = m.mem.r16(DS, di.wrapping_add(2 * i));
        if p == 0 {
            break;
        }
        let mut l = text_at(m, p, 0x0F);
        entries.push(l.swap_remove(0));
    }
    let keys = m.mem.cstr(DS, bx).to_vec();
    Menu { title, entries, keys, selected: 0 }
}

impl Talk {
    /// A message box with the text at DS:`si`.
    pub fn message_box(&mut self, m: &Machine, si: u16) {
        if self.need.is_some() {
            return;
        }
        let lines = text_at(m, si, 0x0F);
        for l in &lines {
            println!("{}", hud::plain(l));
        }
        self.boxes.push(lines.clone());
        self.since_answer.push(lines);
    }

    /// Plain text in a box (the way told, a dream).
    pub fn note(&mut self, s: &str) {
        if self.need.is_some() {
            return;
        }
        println!("{s}");
        let lines = vec![vec![Span { colour: 0x0F, text: s.to_string() }]];
        self.boxes.push(lines.clone());
        self.since_answer.push(lines);
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
            entries.push(vec![Span { colour: 0x0F, text: String::from_utf8_lossy(s).into_owned() }]);
        }
        let title = if title.is_empty() { Vec::new() } else { hud::lines(title.as_bytes(), 0x0F) };
        let menu = Menu { title, entries, keys: Vec::new(), selected: 0 };
        self.ask(menu)
    }

    /// `menu` asked: the next answer, or the question left (FFFFh).
    fn ask(&mut self, menu: Menu) -> u16 {
        match self.answers.front() {
            Some(Answer::Pick(p)) => {
                let p = *p;
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

    /// Text typed into DS:`at` (up to `max` characters, then a NUL): the
    /// next answer, or the question left (nothing typed).
    pub fn typed(&mut self, m: &mut Machine, at: u16, max: usize, prompt: &str) {
        if self.need.is_some() {
            return;
        }
        match self.answers.front() {
            Some(Answer::Text(t)) => {
                let t = t.clone();
                self.answers.pop_front();
                self.since_answer.clear();
                for (i, b) in t.bytes().take(max).enumerate() {
                    m.mem.w8(DS, at + i as u16, b);
                }
                m.mem.w8(DS, at + t.len().min(max) as u16, 0);
            }
            _ => self.need = Some(Need::Text(prompt.to_string())),
        }
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

/// `--picks`: entries' numbers, or `tTEXT` for text typed, comma-separated.
pub fn parse_answers(s: &str) -> VecDeque<Answer> {
    s.split(',')
        .filter(|a| !a.is_empty())
        .map(|a| match a.strip_prefix('t') {
            Some(t) => Answer::Text(t.to_string()),
            None => Answer::Pick(a.parse::<i32>().expect("--picks N,..,tTEXT") as u16),
        })
        .collect()
}
