//! Haggling and the quests' texts in the view, answered through `dialog`.
//!
//! - `HaggleTalk` answers the haggle (1194:8020): each line the
//!   shopkeeper says (`HaggleIo::say`, the original's 987A as 49FF shows
//!   it) titles the question that follows it: Accept, Haggle or Leave
//!   after an asking price or a counter-offer, Yes or No for the
//!   confirmation, a box for a refusal or the last word; an offer is
//!   typed at the original's prompt (65CA).
//! - `QuestFront` is the quests' hooks (`QuestHooks`): their texts made
//!   by the text port with the view's text hooks (its boxes and menus:
//!   an offer's yes and no), the text windows (1194:4E88) asked as
//!   questions, the informant haggled with as above, and the Emperor's
//!   audience as `Audience` plays it.

use crate::dialog::Talk;
use crate::hud::{self, Span};
use crate::{Audience, Front};
use arena::bsa::Files;
use arena::haggle::{Haggle, HaggleIo};
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;

/// The haggle's offer prompt (DGROUP 65CA, 1194:80F7).
const OFFER_PROMPT: u16 = 0x65CA;

/// The characters to a line 1194:49FF breaks a haggle's line at (137E,
/// 3Ch as 862E leaves it).
const WRAP: usize = 0x3C;

/// 1194:49FF: past every WRAP characters, back to the space before and a
/// line break there; the lines (4E54 splits at the breaks).
fn wrap_49ff(text: &[u8]) -> Vec<Vec<u8>> {
    let mut t = text.to_vec();
    let (mut si, mut ax) = (0usize, t.len());
    'wrap: while ax > WRAP {
        si += WRAP;
        ax -= WRAP;
        loop {
            match t.get(si) {
                None | Some(0) => break 'wrap,
                Some(b' ') => break,
                _ => {
                    si -= 1;
                    ax += 1;
                }
            }
        }
        t[si] = b'\n';
        si += 1;
    }
    t.split(|&c| c == b'\n').map(|l| l.to_vec()).collect()
}

fn line(s: &str) -> Vec<Span> {
    vec![Span { colour: 0x0F, text: s.to_string(), font: Default::default(), code: None }]
}

/// The haggle's moves asked through `Talk`; with no answer yet, leaving.
pub(crate) struct HaggleTalk<'a> {
    talk: Option<&'a mut Talk>,
    /// The line said last, and its kind (0 the asking price, 1 the
    /// confirmation, 2 and 3 refusals, 4 a counter-offer).
    said: Vec<Vec<u8>>,
    kind: u8,
    prompt: String,
    /// The game's files, for the pictures put on the page (1194:4B8E).
    files: Option<Files>,
}

impl<'a> HaggleTalk<'a> {
    pub(crate) fn new(talk: Option<&'a mut Talk>) -> Self {
        HaggleTalk { talk, said: Vec::new(), kind: 0, prompt: "Your offer?".into(), files: None }
    }

    /// With the game's files and its page its own (`paged`): the pictures
    /// loaded and drawn into the page as the original's
    /// (`Machine::shop_picture`), the dialogs' state kept (`shown`).
    pub(crate) fn with_files(mut self, files: Files, paged: bool) -> Self {
        self.files = paged.then_some(files);
        self
    }

    /// 4E88 with 13D3 1 (1194:858C, 85DE): the line said in the strip,
    /// its pages but the last read first ("...More"), the picture's
    /// buttons picked (1194:6001).
    fn ask(&mut self, entries: &[&str]) -> u16 {
        let Some(t) = self.talk.as_deref_mut() else { return 0xFFFF };
        let said: Vec<&[u8]> = t.dialog_pages(&self.said).iter().map(|l| l.as_slice()).collect();
        t.ask_menu(hud::dialog_buttons(&said, entries))
    }
}

impl HaggleIo for HaggleTalk<'_> {
    fn menu(&mut self) -> Option<u16> {
        if self.talk.is_none() {
            return Some(2);
        }
        Some(if self.kind == 1 {
            // The confirmation: 0 yes.
            if self.ask(&["Yes", "No"]) == 0 { 0 } else { 1 }
        } else {
            match self.ask(&["Accept", "Counter", "Reject"]) {
                0 => 0,
                1 => 1,
                _ => 2,
            }
        })
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        Some(Vec::new())
    }
    /// 1194:80F7..810D: the prompt (65CA) split into its lines (4E54,
    /// 137E 3; counted on a copy: the port leaves the drawing's split
    /// out), the field under them (4E88's, 6 characters into 6599) in
    /// TEENYFNT.DAT and colour C0h.
    fn type_offer(&mut self, m: &mut Machine) -> Option<()> {
        let Some(t) = self.talk.as_deref_mut() else { return Some(()) };
        let mut split = m.clone();
        split.mem.w8(DS, 0x137E, 3);
        split.split_lines(OFFER_PROMPT);
        let lines = split.mem.r16(DS, arena::shops::var::LINES);
        let field = arena::lineedit::fields::dialog_at(lines, 6, arena::haggle::var::TYPED);
        let look = crate::field::Look { font: "TEENYFNT.DAT", colour: 0xC0, put_font: true };
        let mut shown = Vec::new();
        let mut at = OFFER_PROMPT;
        for _ in 0..lines.min(16) {
            let s = split.mem.cstr(DS, at);
            shown.push(line(&String::from_utf8_lossy(s)));
            at = at.wrapping_add(s.len() as u16 + 1);
        }
        t.edit(m, field, look, crate::dialog::Prompt::Lines(shown));
        Some(())
    }
    /// 1194:862E's 4E88 with 13D3 0: the line said in the strip, read
    /// with a click.
    fn key(&mut self) -> Option<()> {
        if let Some(t) = self.talk.as_deref_mut() {
            t.dialog_lines(&self.said);
        }
        Some(())
    }
    /// The haggle's buttons (NEGOTBUT.IMG) or its yes and no (YESNO.IMG)
    /// on the shop's screen, as 1194:4B8E puts them at (59h, 7Fh).
    fn picture(&mut self, m: &mut Machine, si: u16) {
        if let Some(f) = self.files.as_mut() {
            if m.shop_picture(f, si).is_none() {
                eprintln!("the haggle's picture did not load: {}", arena::giveup::describe());
            }
        }
        let name = String::from_utf8_lossy(m.mem.cstr(DS, si)).to_ascii_uppercase();
        if let Some(b) = self.talk.as_deref_mut().and_then(|t| t.backdrop.as_mut()) {
            b.picture = Some(name);
        }
    }
    fn shown(&mut self, m: &mut Machine, si: u16) {
        m.split_lines(si);
        if self.files.is_some() {
            m.shop_dialog(si);
            m.shop_strip_back();
        }
    }
    fn say(&mut self, m: &mut Machine, si: u16, kind: u8) {
        // Its line breaks made spaces (1194:84C3), then broken by 49FF
        // (in the game's memory too, as the original leaves it).
        self.said = wrap_49ff(m.mem.cstr(DS, si));
        if m.line_wrap(si).is_none() {
            eprintln!("the haggle's line's wrap gave up: {}", arena::giveup::describe());
        }
        self.kind = kind;
        let p = String::from_utf8_lossy(m.mem.cstr(DS, OFFER_PROMPT)).trim().to_string();
        if !p.is_empty() {
            self.prompt = p;
        }
    }
}

/// The quests' hooks for the view. `files` is a copy the hooks without
/// files use (the texts written or shown, their substitutions).
pub(crate) struct QuestFront<'a, 'b>(pub(crate) &'a mut Front<'b>, pub(crate) Files);

impl QuestHooks for QuestFront<'_, '_> {
    fn text_at(&mut self, m: &mut Machine, files: &mut Files, mode: u16, di: u16) -> Option<u16> {
        m.text(files, &mut *self.0, mode, DS, di).map(|(ax, _)| ax)
    }
    /// 1194:A374 with AX 2: text number TEXT written to the buffer at 1044.
    fn text(&mut self, m: &mut Machine) {
        if m.text(&mut self.1, &mut *self.0, 2, DS, 0x1044).is_none() {
            // A substitution the text port doesn't make (%qf, where the
            // original's leftover EBX seeds a name; no template uses it):
            // nothing written, and its box (`message_box`) left out.
            m.mem.w8(DS, 0x1044, 0);
        }
    }
    /// 1194:A374 with AX 0: text number TEXT shown.
    fn text_shown(&mut self, m: &mut Machine) {
        let _ = m.text(&mut self.1, &mut *self.0, 0, DS, 0x1044);
    }
    /// 1194:4B8E: the picture named at DS:`si` shown (the informant's,
    /// 132F), behind the turn as the shops' pictures are.
    fn picture(&mut self, m: &mut Machine, si: u16) {
        if m.page_is_block() && m.shop_picture(&mut self.1, si).is_none() {
            eprintln!("the quest's picture did not load: {}", arena::giveup::describe());
        }
        let name = String::from_utf8_lossy(m.mem.cstr(DS, si)).to_ascii_uppercase();
        if let Some(b) = self.0.talk.as_deref_mut().and_then(|t| t.backdrop.as_mut()) {
            b.picture = Some(name);
        }
    }
    /// 1194:1034 at the main quest's end (2294:0FB4). CX is what the
    /// text's box left: its last wait for the buttons' release polls the
    /// mouse (0000:9012, INT 33h AX 3, CX halved), so the screen rolls
    /// the cursor's column (8F92) times before 0FDD 8Eh sets 32h points.
    fn finale(&mut self, m: &mut Machine, files: &mut Files) {
        let levels = m.mem.r16(DS, arena::controls::var::MOUSE_X);
        if m.level_up_screen(levels, files, &mut *self.0).is_none() {
            eprintln!("the finale's level-up gave up: {}", arena::giveup::describe());
        }
    }
    /// 2DA2:000A: the text at DS:`di` has its substitutions made.
    fn expand(&mut self, m: &mut Machine, files: &mut Files, di: u16) {
        let _ = m.expand(files, &mut *self.0, DS, di);
    }
    fn substitute(&mut self, m: &mut Machine, files: &mut Files, di: u16) -> Option<()> {
        m.expand(files, &mut *self.0, DS, di).map(|_| ())
    }
    /// 1194:59CC; a box without text (a text the port couldn't make) is
    /// left out.
    fn message_box(&mut self, m: &mut Machine, si: u16) {
        if m.mem.r8(DS, si) != 0 {
            self.0.message_box(m, si);
        }
    }
    fn notice(&mut self, m: &mut Machine, si: u16) {
        self.0.message_box(m, si);
    }
    fn menu(&mut self, m: &mut Machine, si: u16, bx: u16, di: u16) -> u16 {
        self.0.menu(m, si, bx, di)
    }
    /// 1194:4E88: the text's window, its lines at 929E (LINES of them,
    /// each ending at a NUL) in the strip, three to a page with
    /// "...More" between; kind 0 then waits for a click (4F3F), kind 1
    /// takes the picture's buttons the caller showed (6001: YESNO.IMG's,
    /// AX 0 yes, 1 no, 2 the third), as the shops' dialogs do.
    fn dialog(&mut self, m: &mut Machine, kind: u8) -> u16 {
        let n = m.mem.r16(DS, 0x0FF5).clamp(1, 32);
        let mut at = 0x929Eu16;
        let mut raw = Vec::new();
        for _ in 0..n {
            let s = m.mem.cstr(DS, at).to_vec();
            at = at.wrapping_add(s.len() as u16 + 1);
            raw.push(s);
        }
        let Some(t) = self.0.talk.as_deref_mut() else { return 1 };
        let ax = if kind != 1 {
            t.dialog_lines(&raw);
            0
        } else {
            let last: Vec<&[u8]> = t.dialog_pages(&raw).iter().map(|l| l.as_slice()).collect();
            match t.ask_menu(hud::dialog_buttons(&last, &["Yes", "No"])) {
                0 => 0,
                1 => 1,
                _ => 2,
            }
        };
        // The game's state round the answer, as the shops' (each page's
        // strip kept in bank 18h, `Machine::shop_dialog`), the strip put
        // back after it (4F34, 4F55: 5FC0); only where the page is the
        // game's own block.
        if m.page_is_block() {
            m.shop_dialog(0x929E);
            m.shop_strip_back();
        }
        ax
    }
    fn informant_haggle(&mut self, m: &mut Machine, files: &mut Files, price: u32, per: u16, mode: u8, di: u16) -> Option<bool> {
        let mut io = HaggleTalk::new(self.0.talk.as_deref_mut()).with_files(self.1.clone(), m.page_is_block());
        let r = m.haggle(files, &mut io, price, per, 0x1E, 0x28, 0, mode, di)?;
        Some(r == Haggle::NoDeal)
    }
    fn audience(&mut self, m: &mut Machine, files: &mut Files, part: u8) -> Option<()> {
        Audience(&mut *self.0).audience(m, files, part)
    }
}
