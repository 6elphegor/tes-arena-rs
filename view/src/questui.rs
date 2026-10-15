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
use crate::hud::{self, Menu, Span};
use crate::{Audience, Front};
use arena::bsa::Files;
use arena::haggle::{Haggle, HaggleIo};
use arena::machine::Machine;
use arena::mem::DS;
use arena::quest::QuestHooks;

/// The haggle's offer prompt (DGROUP 65CA, 1194:80F7).
const OFFER_PROMPT: u16 = 0x65CA;

/// The characters to a line a haggle's line is wrapped at.
const WRAP: usize = 44;

fn line(s: &str) -> Vec<Span> {
    vec![Span { colour: 0x0F, text: s.to_string() }]
}

/// The haggle's moves asked through `Talk`; with no answer yet, leaving.
pub(crate) struct HaggleTalk<'a> {
    talk: Option<&'a mut Talk>,
    /// The line said last, and its kind (0 the asking price, 1 the
    /// confirmation, 2 and 3 refusals, 4 a counter-offer).
    said: Vec<Vec<Span>>,
    kind: u8,
    prompt: String,
}

impl<'a> HaggleTalk<'a> {
    pub(crate) fn new(talk: Option<&'a mut Talk>) -> Self {
        HaggleTalk { talk, said: Vec::new(), kind: 0, prompt: "Your offer?".into() }
    }

    fn ask(&mut self, entries: &[&str]) -> u16 {
        let Some(t) = self.talk.as_deref_mut() else { return 0xFFFF };
        let menu = Menu {
            title: self.said.clone(),
            entries: entries.iter().map(|e| line(e)).collect(),
            keys: entries.iter().map(|e| e.as_bytes()[0]).collect(),
            selected: 0,
        };
        t.ask_menu(menu)
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
            match self.ask(&["Accept", "Haggle", "Leave"]) {
                0 => 0,
                1 => 1,
                _ => 2,
            }
        })
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        let prompt = self.prompt.clone();
        let Some(t) = self.talk.as_deref_mut() else { return Some(Vec::new()) };
        Some(t.typed_text(&prompt).unwrap_or_default().into_bytes())
    }
    fn key(&mut self) -> Option<()> {
        if let Some(t) = self.talk.as_deref_mut() {
            t.lines_box(self.said.clone());
        }
        Some(())
    }
    fn say(&mut self, m: &mut Machine, si: u16, kind: u8) {
        // The original made its line breaks spaces (1194:84C3) and 49FF
        // wraps it; here it is wrapped at spaces to fit the screen.
        let text = String::from_utf8_lossy(m.mem.cstr(DS, si)).into_owned();
        let mut wrapped = String::new();
        let mut col = 0;
        for word in text.split(' ').filter(|w| !w.is_empty()) {
            if col > 0 && col + 1 + word.len() > WRAP {
                wrapped.push('\r');
                col = 0;
            } else if col > 0 {
                wrapped.push(' ');
                col += 1;
            }
            wrapped.push_str(word);
            col += word.len();
        }
        self.said = hud::lines(wrapped.as_bytes(), 0x0F);
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
    /// 2DA2:000A: the text at DS:`di` has its substitutions made.
    fn expand(&mut self, m: &mut Machine, di: u16) {
        let _ = m.expand(&mut self.1, &mut *self.0, DS, di);
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
    /// each ending at a NUL); kind 1 a yes or no (AX 0 yes).
    fn dialog(&mut self, m: &mut Machine, kind: u8) -> u16 {
        let n = m.mem.r16(DS, 0x0FF5).clamp(1, 32);
        let mut at = 0x929Eu16;
        let mut lines = Vec::new();
        for _ in 0..n {
            let s = m.mem.cstr(DS, at);
            at = at.wrapping_add(s.len() as u16 + 1);
            lines.extend(hud::lines(s, 0x0F));
        }
        let Some(t) = self.0.talk.as_deref_mut() else { return 1 };
        if kind != 1 {
            t.lines_box(lines);
            return 0;
        }
        let menu = Menu { title: lines, entries: vec![line("Yes"), line("No")], keys: b"YN".to_vec(), selected: 0 };
        if t.ask_menu(menu) == 0 { 0 } else { 1 }
    }
    fn informant_haggle(&mut self, m: &mut Machine, files: &mut Files, price: u32, per: u16, mode: u8) -> Option<bool> {
        let mut io = HaggleTalk::new(self.0.talk.as_deref_mut());
        let r = m.haggle(files, &mut io, price, per, 0x1E, 0x28, 0, mode, 0)?;
        Some(r == Haggle::NoDeal)
    }
    fn audience(&mut self, m: &mut Machine, files: &mut Files, part: u8) -> Option<()> {
        Audience(&mut *self.0).audience(m, files, part)
    }
}
