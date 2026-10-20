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
use crate::hud::{self, Span};
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
    let paged = m.page_is_block();
    let mut ui = ShopFront { front, files: files.clone(), lines: None, picture: 0, written: Vec::new(), paged, screen: false };
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
    /// The gold left's line (1194:545C's text at 1044), until the strip
    /// under it is put back (5FC0).
    pub gold: Option<Vec<u8>>,
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
        // 1194:545C: in the font [1026] (FONT_A.DAT) and colour 41h,
        // centred on x A0h (0000:5364, no shadow) at row 9Ch.
        if let (Some(gold), Some(f)) = (&self.gold, hud::Font::load(archive, "FONT_A.DAT")) {
            let text = String::from_utf8_lossy(gold).into_owned();
            screen.text_plain(&f, 0xA0 - (f.width(&text) as i32 >> 1), 0x9C, &text, 0x41);
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
    /// The draw page is the game's own block (`Machine::page_is_block`):
    /// the screens' state that writes into it is kept as the original's.
    paged: bool,
    /// The temple's screen (4B60) loaded, for its banner after (4BFF).
    screen: bool,
}

/// The LINES (0FF5) lines 1194:4E54 left at DS:`si`, each ending at a
/// NUL.
/// The lines 1194:4E54 split the text at DS:`si` into (LINES of them,
/// each ending at a NUL), as bytes.
fn split_raw(m: &Machine, si: u16) -> Vec<Vec<u8>> {
    let n = m.mem.r16(DS, 0x0FF5).min(32);
    let mut at = si;
    let mut out = Vec::new();
    for _ in 0..n {
        let s = m.mem.cstr(DS, at).to_vec();
        at = at.wrapping_add(s.len() as u16 + 1);
        out.push(s);
    }
    out
}

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
    fn show_banner(&mut self, m: &mut Machine, si: u16) {
        let screen = m.shop_screen(&mut self.files, 0x12EB).is_some();
        self.show_name(m, si, screen);
    }

    /// The shop's name (1194:4BFF) on the screen 4B60 drew (`screen`:
    /// whether it loaded).
    fn show_name(&mut self, m: &mut Machine, si: u16, screen: bool) {
        // The game's state as the two leave it (the page, the font's and
        // the tiles' blocks, the banner's words: `Machine::shop_banner`).
        if !screen || m.shop_banner(&mut self.files, si).is_none() {
            eprintln!("the shop's screen did not load: {}", arena::giveup::describe());
        } else if self.paged {
            // 4CA1..4CBE: the name drawn centred on x A0h at row 3 (4 in
            // [102C]'s font), in the font as yet unfreed.
            let row = if m.mem.r16(DS, 0xB036) == m.mem.r16(DS, 0x102C) { 4 } else { 3 };
            m.page_text_centred(si, 0xA0, row);
        }
        let title = m.mem.cstr(DS, si).to_vec();
        if let Some(t) = self.talk() {
            t.backdrop = Some(Backdrop { title, picture: None, gold: None });
        }
    }

    /// The answer to a dialog (1194:4E88) of kind `kind`, its text at
    /// DS:`si`, a number typed into DS:`di`, as the window's turn gives it.
    fn dialog_answer(&mut self, m: &mut Machine, kind: u8, si: u16, di: u16) -> u16 {
        let lines = if si != 0 { split_text(m, si) } else { Vec::new() };
        let raw = if si != 0 { split_raw(m, si) } else { Vec::new() };
        match kind {
            0 => {
                if let Some(t) = self.talk() {
                    t.dialog_lines(&raw);
                }
                0
            }
            1 => {
                // The lines in the strip, the picture's buttons picked
                // (1194:6001).
                let entries: &[&str] = if self.picture == NEW_OLD { &["New job", "Old job"] } else { &["Yes", "No"] };
                let pick = match self.talk() {
                    Some(t) => {
                        // The pages before the last read first (4E88's
                        // "...More").
                        let last: Vec<&[u8]> = t.dialog_pages(&raw).iter().map(|l| l.as_slice()).collect();
                        t.ask_menu(crate::hud::dialog_buttons(&last, entries))
                    }
                    None => 0xFFFF,
                };
                match pick {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                }
            }
            _ => {
                // The field under the dialog's lines (1194:4F5A), as many
                // characters as the caller's AX.
                let field = arena::lineedit::fields::dialog(m, arena::lineedit::fields::dialog_max(di), di);
                let look = crate::field::Look { font: "TEENYFNT.DAT", colour: 0xC0, put_font: true };
                match self.talk() {
                    Some(t) => {
                        let last = t.dialog_pages(&raw).len();
                        let lines = lines[lines.len() - last.min(lines.len())..].to_vec();
                        t.edit(m, field, look, crate::dialog::Prompt::Lines(lines));
                        // Not answered yet: "0", which every shop takes as
                        // leaving (the tavern's days are asked again until
                        // they are digits, so nothing typed would ask
                        // forever); the turn is then thrown away and the
                        // question shown.
                        if t.need.is_some() {
                            m.mem.w16(DS, di, 0x30);
                        }
                    }
                    None => m.mem.w16(DS, di, 0x30),
                }
                0
            }
        }
    }

    /// The line the player picks from a list (its kind `m.list_kind`), as
    /// the window's turn answers it (FFFFh: none). Kinds 6 and 10 draw
    /// the text at DI as their title (1194:5734 keeps it at A872 for
    /// 0000:5364); the others don't read it.
    fn list_pick(&mut self, m: &mut Machine, si: u16, cx: u16, di: u16) -> u16 {
        let title = if di != 0 && matches!(m.list_kind, 6 | 0xA) { String::from_utf8_lossy(m.mem.cstr(DS, di)).into_owned() } else { String::new() };
        if let Some(entries) = self.lines.take() {
            let title = if title.is_empty() { Vec::new() } else { crate::hud::lines(title.as_bytes(), 0x0F) };
            let menu = crate::hud::list_menu(m, title, entries);
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

    /// A picture (1194:4B8E, its name at DS:`si`) on the shop's screen.
    fn show_picture(&mut self, m: &mut Machine, si: u16) {
        if m.shop_picture(&mut self.files, si).is_none() {
            eprintln!("the shop's picture did not load: {}", arena::giveup::describe());
        }
        self.picture = si;
        let name = String::from_utf8_lossy(m.mem.cstr(DS, si)).to_ascii_uppercase();
        if let Some(b) = self.talk().and_then(|t| t.backdrop.as_mut()) {
            b.picture = Some(name);
        }
    }
}

impl arena::shops::ShopHooks for ShopFront<'_, '_> {
    fn message(&mut self, m: &mut Machine, si: u16) {
        // 4D31 leaves its font ([102E]: B036) and colour selected.
        if let Some(t) = self.talk() {
            t.strip_message(m, si);
        }
        if self.paged {
            m.shop_message(si);
        }
    }
    fn shop_screen(&mut self, m: &mut Machine) {
        self.screen = m.shop_screen(&mut self.files, 0x12EB).is_some();
    }
    fn banner(&mut self, m: &mut Machine, si: u16) {
        self.show_name(m, si, self.screen);
    }
    fn picture(&mut self, m: &mut Machine, si: u16) {
        self.show_picture(m, si);
    }
    fn screen_saved(&mut self, m: &mut Machine, files: &mut Files) {
        // Only for a page that is the game's own block (`paged`): a game
        // started without one has heap blocks where [A900] points.
        // The page is the game's as the frame left it (its 3D view and
        // interface drawn there), which the shop's screen is drawn over
        // and its lists keep.
        if !self.paged {
            return;
        }
        m.shop_screen_saved(files, self.front.dac.as_ref());
    }
    fn screen_back(&mut self, m: &mut Machine, files: &mut Files) {
        if self.paged {
            m.shop_screen_back(files);
        }
    }
    fn buffer_freed(&mut self, m: &mut Machine) {
        m.buffer_free();
    }
    fn buffer_made(&mut self, m: &mut Machine) {
        if m.buffer_make().is_none() {
            eprintln!("the buffer at BA54 found no room: {}", arena::giveup::describe());
        }
    }
    fn gold_left(&mut self, m: &mut Machine) {
        if self.paged {
            m.shop_strip_save();
        }
        m.gold_left();
        if self.paged {
            m.page_text_centred(arena::shops::GOLD_LINE, 0xA0, 0x9C);
        }
        let text = m.mem.cstr(DS, arena::shops::GOLD_LINE).to_vec();
        // Once a question is left unanswered the turn is shown as it was.
        if let Some(b) = self.talk().filter(|t| t.need.is_none()).and_then(|t| t.backdrop.as_mut()) {
            b.gold = Some(text);
        }
    }
    fn strip_back(&mut self, m: &mut Machine) {
        if self.paged {
            m.shop_strip_back();
        }
        if let Some(b) = self.talk().filter(|t| t.need.is_none()).and_then(|t| t.backdrop.as_mut()) {
            b.gold = None;
        }
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.front.menu(m, si, bx, di)
    }
    fn dialog(&mut self, m: &mut Machine, kind: u8, di: u16) -> u16 {
        self.dialog_at(m, kind, 0, di)
    }
    fn dialog_at(&mut self, m: &mut Machine, kind: u8, si: u16, di: u16) -> u16 {
        // The game's state round the answer, in 4E88's order: the lines
        // drawn, each page's strip kept first (`Machine::shop_dialog`),
        // then the answer (a typed number drawn over them, 9EE5, in the
        // font and colour they leave), then the strip put back (4F34,
        // 4F55, 4F84: 5FC0).
        if self.paged && si != 0 {
            m.shop_dialog(si);
        }
        let ax = self.dialog_answer(m, kind, si, di);
        if self.paged {
            m.shop_strip_back();
        }
        ax
    }
    fn list(&mut self, m: &mut Machine, si: u16, cx: u16, di: u16) -> u16 {
        let pick = self.list_pick(m, si, cx, di);
        let reads = self.talk().and_then(|t| t.list_reads.take());
        if !self.paged {
            return pick;
        }
        m.list_reads = reads;
        // A list asked: the copy of the game as its loop starts, for the
        // window to follow the loop.
        m.list_capture = self.talk().is_some_and(|t| t.need.is_some());
        // The game's state round the pick (`Machine::list_window`: the
        // screen kept in the load bank, the panel, the window's record).
        let kind = m.list_kind;
        let seg = m.list_segment(kind);
        match m.list_window(&mut self.files, kind, (seg, si), cx, di, pick) {
            Some(ax) => ax,
            None => {
                eprintln!("the list's state gave up: {}", arena::giveup::describe());
                pick
            }
        }
    }
    /// 1194:D961 on entering: the main quest's person, if the stage's is
    /// in this province.
    fn quest_check(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.main_quest_check(files, &mut QuestFront(&mut *self.front, files.clone())).map(|_| ())
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
    /// The store's name ("%en " made, after the screen: 1194:FA06) on
    /// its banner.
    fn greeting(&mut self, m: &mut Machine, si: u16) {
        self.show_name(m, si, self.screen);
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn haggle(&mut self, m: &mut Machine, mode: u8, eax: u32, per: u16, name: u16) -> (u32, bool) {
        let (quality, mood) = (m.mem.r8(DS, 0x13CA), m.mem.r8(DS, 0x13CB));
        let mut io = HaggleTalk::new(self.front.talk.as_deref_mut()).with_files(self.files.clone(), self.paged);
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
/// its bottom, clicked at y B8h up: x 5..69h, 78h..BEh, 11Dh..136h) shown
/// as the spellmaker's is, a click on it the answer.
impl arena::guild::GuildHooks for ShopFront<'_, '_> {
    /// 1194:AD39: the screen and "Mages Guild" (6F11) on its banner.
    fn screen(&mut self, m: &mut Machine) {
        self.show_banner(m, 0x6F11);
    }
    fn stock_file(&mut self, m: &mut Machine, name: u16) -> Option<Vec<u8>> {
        // Its path built as the original opens it (25FD:0844).
        m.save_path(name);
        let name = m.mem.cstr(DS, name).to_vec();
        if let Some((_, b)) = self.written.iter().rev().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            return Some(b.clone());
        }
        self.files.data(&name).map(|d| d.to_vec())
    }
    fn stock_write(&mut self, m: &mut Machine, name: u16, bytes: &[u8]) {
        m.save_path(name);
        let name = m.mem.cstr(DS, name).to_vec();
        self.files.store(&name, bytes.to_vec());
        self.written.push((name, bytes.to_vec()));
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    fn stock_list(&mut self, m: &mut Machine, seg: u16, si: u16, cx: u16) -> u16 {
        let mut entries = Vec::new();
        let mut at = si;
        for _ in 0..cx {
            let raw = m.mem.cstr(seg, at).to_vec();
            at = at.wrapping_add(raw.len() as u16 + 1);
            entries.push(row(&raw));
        }
        let menu = crate::hud::list_menu(m, Vec::new(), entries);
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
        let mut io = HaggleTalk::new(self.front.talk.as_deref_mut()).with_files(self.files.clone(), self.paged);
        match m.haggle(&mut self.files, &mut io, price, per, quality as u8, factor as u8, 0, 2, 0x1A4C) {
            Some(Haggle::Deal(p)) => (p, false),
            _ => (price, true),
        }
    }
    /// The page is drawn behind the click asked (`click`: the screen kept
    /// as the spellmaker's, `Talk::maker`, 68D7 drawing BUYSPELL.IMG for
    /// 101D FFh).
    /// 1194:68A3's level the page divides by (A898, A89A: the player's
    /// plus one), set before 68D7 draws it.
    fn spell_screen(&mut self, m: &mut Machine) {
        let level = (m.mem.r8(DS, 0x01AA) as u16).wrapping_add(1);
        m.mem.w16(DS, 0xA89A, level);
        m.mem.w16(DS, 0xA898, level);
    }
    /// Each poll of the page: a click on it asked (as the spellmaker's,
    /// `Talk::spell_click`, the screen kept as 68D7 drew it: 101D FFh,
    /// which 1194:A933 clears after); unanswered, "exit", as leaving does.
    fn click(&mut self, m: &mut Machine) -> (u8, u16, u16) {
        let mut drawn = m.clone();
        drawn.mem.w8(DS, 0x101D, 0xFF);
        match self.talk().and_then(|t| t.spell_click(&drawn, None)) {
            Some((x, y)) => (1, x, y),
            None => (1, 0x120, 0xC0),
        }
    }
    /// The spellmaker as the guild's menu runs it (1194:A80D): the world
    /// kept (0000:A611, `Machine::world_keep`, with the screen's colours),
    /// A902 bit 0 cleared, 6064 with SI the player, its clicks, lists and
    /// name answered through `dialog` (`LiveMaker`; its screen drawn behind
    /// its questions, `Talk::maker`), A902 bit 0 cleared again, the world
    /// given back (A6B5, `Machine::world_back`).
    fn spellmaker(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        let dac = self.front.dac;
        m.world_keep(files, dac.as_ref());
        let f = m.mem.r16(DS, 0xA902) & 0xFFFE;
        m.mem.w16(DS, 0xA902, f);
        let r = m.spellmaker(files, &mut LiveMaker { front: &mut *self.front }, 0x01A4);
        let f = m.mem.r16(DS, 0xA902) & 0xFFFE;
        m.mem.w16(DS, 0xA902, f);
        let r = r.and_then(|()| m.world_back(files));
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
    /// The pick asked through `dialog`, the list's loop run over its reads
    /// (`Machine::list_window`) as the talk's lists are; where the port
    /// doesn't know the DI the list stores (A872), the pick alone.
    fn list(&mut self, m: &mut Machine, files: &mut Files, si: u16, cx: u16, di: Option<u16>) -> u16 {
        match di {
            Some(di) => {
                // The screen the list covers is the game's page as 68D7
                // and the name's field left it (its picture, its texts,
                // the name typed): the talk's lists take the window's
                // screen into the page first (`box_screen`), which here
                // would put back what the window last showed over it.
                if m.page_is_block() {
                    let page = m.mem.r16(DS, 0xA900);
                    self.front.drawn = Some((0..0xFA00u16).map(|i| m.mem.r8(page, i)).collect());
                }
                arena::talk::TalkHooks::list(&mut *self.front, m, files, si, cx, di)
            }
            None => {
                eprintln!("the spellmaker's list: DI not known, the list's state not run");
                match self.front.talk.as_deref_mut() {
                    Some(t) => t.list(m, si, cx, ""),
                    None => 0xFFFF,
                }
            }
        }
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.front.menu(m, si, bx, di)
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    /// 1194:65FD: the name at DS:`si` (581A) edited in place on the
    /// spellmaker's screen, in ARENAFNT.DAT ([102C]) and colour A0h.
    fn type_name(&mut self, m: &mut Machine, si: u16) {
        if let Some(t) = self.front.talk.as_deref_mut() {
            let look = crate::field::Look { font: "ARENAFNT.DAT", colour: 0xA0, put_font: true };
            t.edit(m, arena::lineedit::fields::spell_name(m, si), look, crate::dialog::Prompt::Bare);
        }
    }
}

impl arena::tavern::TavernHooks for ShopFront<'_, '_> {
    /// 1194:C0B1: the slot's rooms' files whose time is up deleted.
    fn files(&mut self, m: &mut Machine, files: &mut arena::bsa::Files) {
        m.room_files_expire(files);
    }
    /// 1194:C01B, C12A: the screen (4B60), then "%nt " made at 929E
    /// (2DA2:000A, its buffer in the bank 4B60 left mapped before its
    /// own), the tavern's name, on its banner (4BFF).
    fn greeting(&mut self, m: &mut Machine) {
        let screen = m.shop_screen(&mut self.files, 0x12EB).is_some();
        m.mem.w32(DS, 0x929E, 0x2074_6E25);
        m.mem.w16(DS, 0x92A2, 0);
        arena::shops::ShopHooks::expand(self, m, 0x929E);
        self.show_name(m, 0x929E, screen);
    }
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        self.front.message_box(m, si);
    }
    /// The room's price (1391) haggled over with the innkeeper (1194:C14E).
    fn haggle(&mut self, m: &mut Machine, files: &mut Files) -> Option<(u32, bool)> {
        m.tavern_room_haggle(files, &mut HaggleTalk::new(self.front.talk.as_deref_mut()).with_files(files.clone(), self.paged))
    }
    /// 1194:CED7: the main quest's informant.
    fn quest(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_informant(files, &mut QuestFront(&mut *self.front, files.clone()), 0xFFFE).map(|_| ())
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
