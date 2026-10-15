//! The shops in the view: the temple (1194:C258), the tavern (B93C) and
//! the equipment store (F48E) run by the port, their questions answered
//! through `dialog` as the talk's are.
//!
//! - Menus and lists are asked as the game's (`Talk::menu`, `list`).
//! - The dialogs (1194:4E88) show the lines 4E54 split: kind 0 as a box,
//!   kind 1 as a question whose answers follow the picture shown before it
//!   (newold.img: a new job or an old one; else yes or no; Esc leaves),
//!   kind 2 with a number typed.
//! - The store's lists of weapons and armour are the original's lines
//!   (1194:F142 and F204, `Machine::store_lines`): name, hands (or the
//!   armour's kind), weight and price, laid out by their columns.
//! - The store's haggling runs the haggle port (1194:8020, modes 1 and 4):
//!   each round accept, make an offer (typed) or leave; the shopkeeper's
//!   lines aren't ported, so only the price asked first is shown. The
//!   tavern's room (mode 3, not ported) is paid as asked, or left.

use crate::Front;
use crate::dialog::Talk;
use crate::hud::{self, Menu, Span};
use arena::bsa::Files;
use arena::haggle::{Haggle, HaggleIo};
use arena::machine::Machine;
use arena::mem::DS;

/// The newold.img picture (a job new or old) the repairs show before
/// asking.
const NEW_OLD: u16 = 0x1346;

/// The shop of building kind `kind` (D48: 4 the tavern, 5 the temple, 6
/// the equipment store, 7 the mages' guild), as the talk enters it
/// (`TalkHooks::shop`).
pub fn shop(front: &mut Front, m: &mut Machine, files: &mut Files, kind: u8) -> Option<()> {
    let mut ui = ShopFront { front, files: files.clone(), lines: None, picture: 0, written: Vec::new(), button: None };
    let r = match kind {
        5 => m.temple(&mut ui),
        4 => m.tavern_shop(files, &mut ui),
        6 => m.equipment_store(files, &mut ui),
        7 => m.mages_guild(files, &mut ui),
        _ => Some(()),
    };
    // The guild's stock file, written through its hooks, kept with the
    // game's files.
    for (name, bytes) in ui.written {
        files.store(&name, bytes);
    }
    r
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
    /// Files the guild wrote (its stock), for the game's files after.
    written: Vec<(Vec<u8>, Vec<u8>)>,
    /// The guild's spell screen: the button picked (a click inside it).
    button: Option<(u16, u16)>,
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

/// The haggle's moves asked through `Talk`; with no answer yet, leaving.
struct HaggleTalk<'a> {
    talk: Option<&'a mut Talk>,
    asked: u32,
}

impl HaggleIo for HaggleTalk<'_> {
    fn menu(&mut self) -> Option<u16> {
        let Some(t) = self.talk.as_deref_mut() else { return Some(2) };
        let title = vec![line(&format!("The price asked: {} gp", self.asked))];
        let menu = Menu {
            title,
            entries: vec![line("Accept"), line("Make an offer"), line("Leave")],
            keys: b"AML".to_vec(),
            selected: 0,
        };
        Some(match t.ask_menu(menu) {
            0 => 0,
            1 => 1,
            _ => 2,
        })
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        let Some(t) = self.talk.as_deref_mut() else { return Some(Vec::new()) };
        Some(t.typed_text("Your offer?").unwrap_or_default().into_bytes())
    }
    fn key(&mut self) -> Option<()> {
        Some(())
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
        let mut io = HaggleTalk { talk: self.front.talk.as_deref_mut(), asked: eax };
        match m.haggle(&mut self.files, &mut io, eax, per, quality, mood, 0, mode, name) {
            Some(Haggle::Deal(p)) => (p, false),
            _ => (eax, true),
        }
    }
    fn weapon_lines(&mut self, m: &mut Machine) -> Option<()> {
        m.store_lines(true)?;
        self.lines = Some(frame_lines(m, 0x0FA0, m.mem.r8(DS, 0x12C6) as usize));
        Some(())
    }
    fn armor_lines(&mut self, m: &mut Machine) -> Option<()> {
        m.store_lines(false)?;
        self.lines = Some(frame_lines(m, 0x2710, m.mem.r8(DS, 0x12CF) as usize));
        Some(())
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.picture = si;
    }
}

/// The `n` list lines 1194:F142 or F204 wrote one after another into the
/// EMS frame from `at`, each a row: its fields (`\n` between them, each
/// from a column, `\t` and three digits: its x on the 320-wide screen)
/// padded to their columns at about five pixels a character.
fn frame_lines(m: &Machine, at: u16, n: usize) -> Vec<String> {
    let frame = m.mem.r16(DS, 0x9030);
    let mut out = Vec::new();
    let mut at = at;
    for _ in 0..n {
        let raw = m.mem.cstr(frame, at).to_vec();
        at = at.wrapping_add(raw.len() as u16 + 1);
        let mut row = String::new();
        let mut first = None;
        for field in raw.split(|&b| b == b'\n') {
            let (col, text) = match field {
                [b'\t', a, b, c, rest @ ..] if [a, b, c].iter().all(|d| d.is_ascii_digit()) => {
                    let col = ((a - b'0') as usize) * 100 + ((b - b'0') as usize) * 10 + (c - b'0') as usize;
                    (Some(col), rest)
                }
                _ => (None, field),
            };
            if let Some(col) = col {
                let base = *first.get_or_insert(col);
                let want = col.saturating_sub(base) / 5;
                while row.chars().count() < want {
                    row.push(' ');
                }
                if !row.is_empty() && !row.ends_with(' ') {
                    row.push(' ');
                }
            }
            row.push_str(&String::from_utf8_lossy(text));
        }
        out.push(row);
    }
    out
}

/// The mages' guild: its menus, lists, haggling and boxes as the store's;
/// the spell's screen (1194:68A3, its buttons Back, Buy and Exit along
/// its bottom, clicked at y B8h up: x 5..69h, 78h..BEh, 11Dh..136h) asked
/// as a question whose answer is a click inside the button picked.
impl arena::guild::GuildHooks for ShopFront<'_, '_> {
    fn stock_file(&mut self, m: &mut Machine, name: u16) -> Option<Vec<u8>> {
        let name = m.mem.cstr(DS, name).to_vec();
        if let Some((_, b)) = self.written.iter().rev().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            return Some(b.clone());
        }
        self.files.data(&name).map(|d| d.to_vec())
    }
    fn stock_write(&mut self, m: &mut Machine, name: u16, bytes: &[u8]) {
        let name = m.mem.cstr(DS, name).to_vec();
        self.files.store(&name, bytes.to_vec());
        self.written.push((name, bytes.to_vec()));
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.picture = si;
    }
    fn stock_list(&mut self, m: &mut Machine, seg: u16, si: u16, cx: u16) -> u16 {
        let mut entries = Vec::new();
        let mut at = si;
        for _ in 0..cx {
            let raw = m.mem.cstr(seg, at).to_vec();
            at = at.wrapping_add(raw.len() as u16 + 1);
            entries.push(hud::lines(&raw, 0x0F).into_iter().flatten().collect::<Vec<_>>());
        }
        let menu = Menu { title: Vec::new(), entries, keys: Vec::new(), selected: 0 };
        match self.talk() {
            Some(t) => t.ask_menu(menu),
            None => 0xFFFF,
        }
    }
    /// 1194:8020 in mode 1 (buying), the seller named at 1A4C.
    fn haggle(&mut self, m: &mut Machine, price: u32, quality: u16, factor: u16, per: u16) -> (u32, bool) {
        let mut io = HaggleTalk { talk: self.front.talk.as_deref_mut(), asked: price };
        match m.haggle(&mut self.files, &mut io, price, per, quality as u8, factor as u8, 0, 1, 0x1A4C) {
            Some(Haggle::Deal(p)) => (p, false),
            _ => (price, true),
        }
    }
    fn spell_screen(&mut self, m: &mut Machine) {
        let name = String::from_utf8_lossy(m.mem.cstr(DS, 0x57E6)).into_owned();
        let pick = self.ask(vec![line(&name)], &["Buy", "Back", "Exit"]);
        self.button = Some(match pick {
            0 => (0x90, 0xC0),
            1 => (0x30, 0xC0),
            _ => (0x120, 0xC0),
        });
    }
    fn click(&mut self, _m: &mut Machine) -> (u8, u16, u16) {
        let (x, y) = self.button.take().unwrap_or((0x120, 0xC0));
        (1, x, y)
    }
}

impl arena::tavern::TavernHooks for ShopFront<'_, '_> {
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn picture(&mut self, _m: &mut Machine, si: u16) {
        self.picture = si;
    }
    /// The room's price (1391) paid as asked, or the tavern left (the
    /// haggle's mode 3 isn't ported).
    fn haggle(&mut self, m: &mut Machine) -> (u32, bool) {
        let price = m.mem.r32(DS, 0x1391);
        let pick = self.ask(vec![line(&format!("The room costs {price} gold."))], &["Pay", "Leave"]);
        (price, pick != 0)
    }
}
