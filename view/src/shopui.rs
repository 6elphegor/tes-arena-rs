//! The shops in the view: the temple (1194:C258), the tavern (B93C) and
//! the equipment store (F48E) run by the port, their questions answered
//! through `dialog` as the talk's are.
//!
//! - Menus and lists are asked as the game's (`Talk::menu`, `list`).
//! - The dialogs (1194:4E88) show the lines 4E54 split: kind 0 as a box,
//!   kind 1 as a question whose answers follow the picture shown before it
//!   (newold.img: a new job or an old one; else yes or no; Esc leaves),
//!   kind 2 with a number typed.
//! - The store's lists of weapons and armour are made here (the
//!   original's lines, 1194:F142 and F204, aren't ported): each record's
//!   name as Buy names it and its price.
//! - The store's haggling and the tavern's room run the haggle port
//!   (1194:8020, modes 1, 4 and 3; the room's set-up 1194:C14E,
//!   `tavern_room_haggle`), the shopkeeper's lines titling each question
//!   (`questui::HaggleTalk`).
//! - The shops' quests (the tavern's informant and quests, the quests
//!   on entering) go through `questui::QuestFront`.

use crate::Front;
use crate::dialog::Talk;
use crate::questui::{HaggleTalk, QuestFront};
use crate::hud::{self, Menu, Span};
use arena::bsa::Files;
use arena::haggle::Haggle;
use arena::machine::Machine;
use arena::mem::DS;

/// The newold.img picture (a job new or old) the repairs show before
/// asking.
const NEW_OLD: u16 = 0x1346;

/// The shop of building kind `kind` (D48: 4 the tavern, 5 the temple, 6
/// the equipment store), as the talk enters it (`TalkHooks::shop`). The
/// mages' guild (7) isn't ported.
pub fn shop(front: &mut Front, m: &mut Machine, files: &mut Files, kind: u8) -> Option<()> {
    let mut ui = ShopFront { front, files: files.clone(), lines: None, picture: 0 };
    match kind {
        5 => m.temple(files, &mut ui),
        4 => m.tavern_shop(files, &mut ui),
        6 => m.equipment_store(files, &mut ui),
        _ => Some(()),
    }
}

/// The front end's side of a shop. `files` is a copy the hooks without
/// files use (the haggle's dialogue, the greeting's substitutions).
struct ShopFront<'a, 'b> {
    front: &'a mut Front<'b>,
    files: Files,
    /// The store's list made for the next list asked.
    lines: Option<Vec<String>>,
    /// The picture shown last (4B8E).
    picture: u16,
}

fn line(s: &str) -> Vec<Span> {
    vec![Span { colour: 0x0F, text: s.to_string() }]
}

/// The LINES (0FF5) lines 1194:4E54 left at DS:`si`, each ending at a
/// NUL.
fn split_text(m: &Machine, si: u16) -> Vec<Vec<Span>> {
    let n = m.mem.r16(DS, 0x0FF5).min(32);
    let mut at = si;
    let mut out = Vec::new();
    for _ in 0..n {
        let s = m.mem.cstr(DS, at);
        at = at.wrapping_add(s.len() as u16 + 1);
        out.extend(hud::lines(s, 0x0F));
    }
    out
}

impl ShopFront<'_, '_> {
    fn talk(&mut self) -> Option<&mut Talk> {
        self.front.talk.as_deref_mut()
    }

    /// A question the front end makes: the pick, or FFFFh (none, or left).
    fn ask(&mut self, title: Vec<Vec<Span>>, entries: &[&str]) -> u16 {
        let menu = Menu {
            title,
            entries: entries.iter().map(|e| line(e)).collect(),
            keys: entries.iter().map(|e| e.as_bytes()[0]).collect(),
            selected: 0,
        };
        match self.talk() {
            Some(t) => t.ask_menu(menu),
            None => 0xFFFF,
        }
    }
}

impl arena::shops::ShopHooks for ShopFront<'_, '_> {
    fn message(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.front.menu(m, si, bx, di)
    }
    fn dialog(&mut self, m: &mut Machine, kind: u8, di: u16) -> u16 {
        self.dialog_at(m, kind, 0, di)
    }
    fn dialog_at(&mut self, m: &mut Machine, kind: u8, si: u16, di: u16) -> u16 {
        let lines = if si != 0 { split_text(m, si) } else { Vec::new() };
        match kind {
            0 => {
                if let Some(t) = self.talk() {
                    t.lines_box(lines);
                }
                0
            }
            1 => {
                let entries: &[&str] = if self.picture == NEW_OLD { &["New job", "Old job"] } else { &["Yes", "No"] };
                match self.ask(lines, entries) {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                }
            }
            _ => {
                let prompt: String = lines.iter().map(|l| hud::plain(l)).collect::<Vec<_>>().join(" ");
                // Not answered yet: "0", which every shop takes as leaving
                // (the tavern's days are asked again until they are digits,
                // so nothing typed would ask forever); the turn is then
                // thrown away and the question shown.
                let typed = self.talk().and_then(|t| t.typed_text(prompt.trim())).unwrap_or_else(|| "0".into());
                let t = typed.as_bytes();
                let t = &t[..t.len().min(5)];
                for (i, &b) in t.iter().chain([0u8].iter()).enumerate() {
                    m.mem.w8(DS, di.wrapping_add(i as u16), b);
                }
                0
            }
        }
    }
    fn list(&mut self, m: &mut Machine, si: u16, cx: u16, _di: u16) -> u16 {
        if let Some(lines) = self.lines.take() {
            let entries: Vec<&str> = lines.iter().map(String::as_str).collect();
            let menu = Menu { title: Vec::new(), entries: entries.iter().map(|e| line(e)).collect(), keys: Vec::new(), selected: 0 };
            return match self.talk() {
                Some(t) => t.ask_menu(menu),
                None => 0xFFFF,
            };
        }
        match self.talk() {
            Some(t) => t.list(m, si, cx, ""),
            None => 0xFFFF,
        }
    }
    /// 1194:8F1E on entering: the quests ended or offered.
    fn quests(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_talk(files, &mut QuestFront(&mut *self.front))
    }
    fn expand(&mut self, m: &mut Machine, di: u16) -> u16 {
        if let Some(end) = m.expand(&mut self.files, &mut arena::text::NoTextHooks, DS, di) {
            return end;
        }
        let mut di = di;
        while m.mem.r8(DS, di) != 0 {
            di = di.wrapping_add(1);
        }
        di
    }
}

impl arena::store::StoreHooks for ShopFront<'_, '_> {
    /// The greeting, unless its substitution couldn't be made ("%en ": the
    /// text port gives none for it).
    fn greeting(&mut self, m: &mut Machine, si: u16) {
        if !m.mem.cstr(DS, si).contains(&b'%') {
            self.front.message_box(m, si);
        }
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn haggle(&mut self, m: &mut Machine, mode: u8, eax: u32, per: u16, name: u16) -> (u32, bool) {
        let (quality, mood) = (m.mem.r8(DS, 0x13CA), m.mem.r8(DS, 0x13CB));
        let mut io = HaggleTalk::new(self.front.talk.as_deref_mut());
        match m.haggle(&mut self.files, &mut io, eax, per, quality, mood, 0, mode, name) {
            Some(Haggle::Deal(p)) => (p, false),
            _ => (eax, true),
        }
    }
    fn weapon_lines(&mut self, m: &mut Machine) {
        self.lines = Some(store_lines(m, true));
    }
    fn armor_lines(&mut self, m: &mut Machine) {
        self.lines = Some(store_lines(m, false));
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.picture = si;
    }
}

/// The store's weapons or armour, each "name - price gp".
fn store_lines(m: &mut Machine, weapons: bool) -> Vec<String> {
    m.store_list(weapons).into_iter().map(|(name, price)| format!("{name} - {price} gp")).collect()
}

impl arena::tavern::TavernHooks for ShopFront<'_, '_> {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.picture = si;
    }
    /// The room's price (1391) haggled over with the innkeeper (1194:C14E).
    fn haggle(&mut self, m: &mut Machine, files: &mut Files) -> Option<(u32, bool)> {
        m.tavern_room_haggle(files, &mut HaggleTalk::new(self.front.talk.as_deref_mut()))
    }
    /// 1194:CED7: the main quest's informant.
    fn quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_informant(files, &mut QuestFront(&mut *self.front)).map(|_| ())
    }
    /// 1194:8F1E: the quests the talk ends or offers.
    fn quests_talk(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_talk(files, &mut QuestFront(&mut *self.front))
    }
    /// 1194:90B9: another queued quest's text taken.
    fn tavern_quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_take(files, &mut QuestFront(&mut *self.front))
    }
}
