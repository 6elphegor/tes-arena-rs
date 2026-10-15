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
//! - The store's and the guild's haggling and the tavern's room run the
//!   haggle port (1194:8020, modes 1, 4, 2 and 3; the room's set-up
//!   1194:C14E, `tavern_room_haggle`), the shopkeeper's lines titling
//!   each question (`questui::HaggleTalk`).
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
/// the equipment store, 7 the mages' guild), as the talk enters it
/// (`TalkHooks::shop`).
pub fn shop(front: &mut Front, m: &mut Machine, files: &mut Files, kind: u8) -> Option<()> {
    let mut ui = ShopFront { front, files: files.clone(), lines: None, picture: 0, written: Vec::new(), button: None };
    let r = match kind {
        5 => m.temple(files, &mut ui),
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

/// A shop's screen as the original draws it on entering (1194:4B60,
/// 4BFF) and the picture shown last on it (1194:4B8E).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Backdrop {
    /// The shop's name on the banner.
    pub title: Vec<u8>,
    /// The picture's file, drawn at (59h, 7Fh).
    pub picture: Option<String>,
}

impl Backdrop {
    /// The screen over the view: MENUSCRN.IMG (4B60: at its own place), then the
    /// banner (4BFF): the name's width in FONT_C.DAT (0000:66F6), or in
    /// ARENAFNT.DAT past 118h; MENU_CTR.IMG's tiles, 16 wide, enough for
    /// it, from x A0h less half their width at row 0, MENU_LFT.IMG 5
    /// left of them, MENU_RT.IMG after them; the name in colour 70h
    /// centred on x A0h (0000:5364, no shadow) at row 3 (4 in ARENAFNT).
    /// Then the picture shown last (4B8E: at 59h, 7Fh).
    pub fn draw(&self, screen: &mut hud::Screen, archive: &crate::archive::Archive, m: &Machine) {
        let img = |name: &str| crate::panels::load_image(archive, m, name);
        if let Some(i) = img("MENUSCRN.IMG") {
            screen.blit(i.x, i.y, i.w, i.h, &i.px, 199);
        }
        let title = String::from_utf8_lossy(&self.title).into_owned();
        let (Some(c), Some(a)) = (hud::Font::load(archive, "FONT_C.DAT"), hud::Font::load(archive, "ARENAFNT.DAT")) else { return };
        let (font, row) = if c.width(&title) > 0x118 { (&a, 4) } else { (&c, 3) };
        let w = font.width(&title) as i32;
        let n = (w + 15) >> 4;
        let x0 = 0xA0 - ((n * 16) >> 1);
        if let Some(t) = img("MENU_CTR.IMG") {
            for i in 0..n {
                screen.blit(x0 + i * 16, 0, t.w, t.h, &t.px, 199);
            }
        }
        if let Some(t) = img("MENU_LFT.IMG") {
            screen.blit(x0 - 5, 0, t.w, t.h, &t.px, 199);
        }
        if let Some(t) = img("MENU_RT.IMG") {
            screen.blit(0xA0 + ((n * 16) >> 1), 0, t.w, t.h, &t.px, 199);
        }
        screen.text_plain(font, 0xA0 - (w >> 1), row, &title, 0x70);
        if let Some(t) = self.picture.as_deref().and_then(img) {
            screen.blit(0x59, 0x7F, t.w, t.h, &t.px, 199);
        }
    }
}

/// The front end's side of a shop. `files` is a copy the hooks without
/// files use (the haggle's dialogue, the greeting's substitutions).
struct ShopFront<'a, 'b> {
    front: &'a mut Front<'b>,
    files: Files,
    /// The store's list made for the next list asked.
    lines: Option<Vec<Vec<Span>>>,
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

    /// The shop's screen and name (1194:4B60, 4BFF), behind what follows.
    fn show_banner(&mut self, m: &Machine, si: u16) {
        let title = m.mem.cstr(DS, si).to_vec();
        if let Some(t) = self.talk() {
            t.backdrop = Some(Backdrop { title, picture: None });
        }
    }

    /// A picture (1194:4B8E, its name at DS:`si`) on the shop's screen.
    fn show_picture(&mut self, m: &Machine, si: u16) {
        self.picture = si;
        let name = String::from_utf8_lossy(m.mem.cstr(DS, si)).to_ascii_uppercase();
        if let Some(b) = self.talk().and_then(|t| t.backdrop.as_mut()) {
            b.picture = Some(name);
        }
    }

    /// A question the front end makes: the pick, or FFFFh (none, or left).
    fn ask(&mut self, title: Vec<Vec<Span>>, entries: &[&str]) -> u16 {
        let menu = Menu {
            title,
            entries: entries.iter().map(|e| line(e)).collect(),
            keys: entries.iter().map(|e| e.as_bytes()[0]).collect(),
            selected: 0,
            list: false,
            kind: 0,
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
    fn banner(&mut self, m: &mut Machine, si: u16) {
        self.show_banner(m, si);
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
    fn list(&mut self, m: &mut Machine, si: u16, cx: u16, di: u16) -> u16 {
        // Kinds 6 and 10 draw the text at DI as their title (1194:5734
        // keeps it at A872 for 0000:5364); the others don't read it.
        let title = if di != 0 && matches!(m.list_kind, 6 | 0xA) { String::from_utf8_lossy(m.mem.cstr(DS, di)).into_owned() } else { String::new() };
        if let Some(entries) = self.lines.take() {
            let title = if title.is_empty() { Vec::new() } else { crate::hud::lines(title.as_bytes(), 0x0F) };
            let menu = Menu { title, entries, keys: Vec::new(), selected: 0, list: true, kind: m.list_kind };
            return match self.talk() {
                Some(t) => t.ask_menu(menu),
                None => 0xFFFF,
            };
        }
        match self.talk() {
            Some(t) => t.list(m, si, cx, &title),
            None => 0xFFFF,
        }
    }
    /// 1194:8F1E on entering: the quests ended or offered.
    fn quests(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_talk(files, &mut QuestFront(&mut *self.front, files.clone()))
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
    /// The store's name ("%en " made) on its banner.
    fn greeting(&mut self, m: &mut Machine, si: u16) {
        self.show_banner(m, si);
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
    fn picture(&mut self, m: &mut Machine, si: u16) {
        self.show_picture(m, si);
    }
}

/// The `n` list lines 1194:F142 or F204 wrote one after another into the
/// EMS frame from `at`, each a row of its fields (`row`).
fn frame_lines(m: &Machine, at: u16, n: usize) -> Vec<Vec<Span>> {
    let frame = m.mem.r16(DS, 0x9030);
    let mut out = Vec::new();
    let mut at = at;
    for _ in 0..n {
        let raw = m.mem.cstr(frame, at).to_vec();
        at = at.wrapping_add(raw.len() as u16 + 1);
        out.push(row(&raw));
    }
    out
}

/// A list line's fields (`\n` between them, each from a column: `\t` and
/// three digits, its x on the 320-wide screen) as one row of spans, the
/// columns kept (`hud::column`): a list draws each field at its x.
fn row(raw: &[u8]) -> Vec<Span> {
    hud::lines(raw, 0x0F).into_iter().flatten().collect()
}

/// The mages' guild: its menus, lists, haggling and boxes as the store's;
/// the spell's screen (1194:68A3, its buttons Back, Buy and Exit along
/// its bottom, clicked at y B8h up: x 5..69h, 78h..BEh, 11Dh..136h) asked
/// as a question whose answer is a click inside the button picked.
impl arena::guild::GuildHooks for ShopFront<'_, '_> {
    /// 1194:AD39: the screen and "Mages Guild" (6F11) on its banner.
    fn screen(&mut self, m: &mut Machine) {
        self.show_banner(m, 0x6F11);
    }
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
    fn picture(&mut self, m: &mut Machine, si: u16) {
        self.show_picture(m, si);
    }
    fn stock_list(&mut self, m: &mut Machine, seg: u16, si: u16, cx: u16) -> u16 {
        let mut entries = Vec::new();
        let mut at = si;
        for _ in 0..cx {
            let raw = m.mem.cstr(seg, at).to_vec();
            at = at.wrapping_add(raw.len() as u16 + 1);
            entries.push(row(&raw));
        }
        let menu = Menu { title: Vec::new(), entries, keys: Vec::new(), selected: 0, list: true, kind: m.list_kind };
        match self.talk() {
            Some(t) => t.ask_menu(menu),
            None => 0xFFFF,
        }
    }
    /// 1194:8020 in mode 2 (the guild's calls, 1194:B3A5 and B562, pass DH
    /// 2: MUGUILD.DAT's lines), the seller named at 1A4C. The port maps
    /// the dialogue's EMS bank and puts the stock's back (8665), so the
    /// stock in bank 18h is kept.
    fn haggle(&mut self, m: &mut Machine, price: u32, quality: u16, factor: u16, per: u16) -> (u32, bool) {
        let mut io = HaggleTalk::new(self.front.talk.as_deref_mut());
        match m.haggle(&mut self.files, &mut io, price, per, quality as u8, factor as u8, 0, 2, 0x1A4C) {
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
    /// The spellmaker (1194:6064 with SI the player), its clicks, lists
    /// and name answered through `dialog` (`LiveMaker`); its screen is
    /// drawn behind its questions (`Talk::maker`).
    fn spellmaker(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        let r = m.spellmaker(files, &mut LiveMaker { front: &mut *self.front }, 0x01A4);
        if let Some(t) = self.talk() {
            if t.need.is_none() {
                t.maker = None;
            }
        }
        r
    }
}

/// The spellmaker's hooks in the window: each click (a click on its
/// screen or form), list, menu and name typed is an answer of the turn's
/// (`Talk::spell_click`, `list`, `menu`, `typed`); with none left the
/// click leaves as the port's default does (the screen's "exit", the
/// form's done box), the turn being thrown away.
struct LiveMaker<'a, 'b> {
    front: &'a mut Front<'b>,
}

impl arena::spellmaker::SpellmakerHooks for LiveMaker<'_, '_> {
    fn click(&mut self, m: &mut Machine, form: Option<u16>) -> (u16, u16) {
        if let Some(c) = self.front.talk.as_deref_mut().and_then(|t| t.spell_click(m, form)) {
            return c;
        }
        arena::spellmaker::NoSpellmakerHooks.click(m, form)
    }
    fn list(&mut self, m: &mut Machine, si: u16, cx: u16) -> u16 {
        match self.front.talk.as_deref_mut() {
            Some(t) => t.list(m, si, cx, ""),
            None => 0xFFFF,
        }
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        match self.front.talk.as_deref_mut() {
            Some(t) => t.menu(m, si, bx, di),
            None => 0xFFFF,
        }
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    /// The name typed into DS:`si` (581A, 1Fh characters and a NUL).
    fn type_name(&mut self, m: &mut Machine, si: u16) {
        if let Some(t) = self.front.talk.as_deref_mut() {
            t.typed(m, si, 0x1F, "The spell's name?");
        }
    }
}

impl arena::tavern::TavernHooks for ShopFront<'_, '_> {
    /// 1194:C12A: "%nt " made at 929E (2DA2:000A), the tavern's name, on
    /// its banner (4BFF).
    fn greeting(&mut self, m: &mut Machine) {
        m.mem.w32(DS, 0x929E, 0x2074_6E25);
        m.mem.w16(DS, 0x92A2, 0);
        arena::shops::ShopHooks::expand(self, m, 0x929E);
        self.show_banner(m, 0x929E);
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn picture(&mut self, m: &mut Machine, si: u16) {
        self.show_picture(m, si);
    }
    /// The room's price (1391) haggled over with the innkeeper (1194:C14E).
    fn haggle(&mut self, m: &mut Machine, files: &mut Files) -> Option<(u32, bool)> {
        m.tavern_room_haggle(files, &mut HaggleTalk::new(self.front.talk.as_deref_mut()))
    }
    /// 1194:CED7: the main quest's informant.
    fn quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_informant(files, &mut QuestFront(&mut *self.front, files.clone())).map(|_| ())
    }
    /// 1194:8F1E: the quests the talk ends or offers.
    fn quests_talk(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_talk(files, &mut QuestFront(&mut *self.front, files.clone()))
    }
    /// 1194:90B9: another queued quest's text taken.
    fn tavern_quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_take(files, &mut QuestFront(&mut *self.front, files.clone()))
    }
}
