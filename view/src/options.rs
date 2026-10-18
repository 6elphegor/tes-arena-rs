//! The options screen Esc brings up in play (2542:000A): the port's
//! `Machine::options_screen` run on a copy of the engine with the clicks
//! and answers given so far, again after each new one (as `saveui` runs
//! the save and load menus). Where the screen wants a click not yet
//! given, the run stops there (the click taken as none, Resume; that run
//! is thrown away) and the view shows what it drew up to then: OP.IMG,
//! the volumes in their boxes, the detail bar. Exit's "Are you sure ?"
//! (1194:48DF) is the game's menu (`hud::Menu` from its texts at 4B50).
//!
//! The original runs the save and load menus from inside the screen
//! (25FD:0002, 0242) and leaves after them; here the screen leaves first
//! (A902 bit 0 cleared before the menu, not after) and the view then opens
//! `saveui` on the engine it left. The numbers are drawn in ARENAFNT.DAT,
//! the font [102C] holds (2542:000A selects it first).

use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mem::DS;
use arena::options::{var, OptionsEnd, OptionsHooks};

use crate::hud::{self, Font, Hud, Screen};
use crate::interface::Image;

#[derive(Clone)]
enum Draw {
    Fill(i32, i32, i32, i32, u8),
    Text(i32, i32, Vec<u8>, u8),
}

/// How a replay of the screen ended.
pub enum Ended {
    /// Waiting for a click (or "Are you sure ?"'s answer).
    Open,
    /// Left: the engine after the screen, and how.
    Left(Box<Engine>, OptionsEnd),
}

pub struct OptionsUi {
    base: Engine,
    clicks: Vec<(u16, u16)>,
    answers: Vec<u16>,
    draws: Vec<Draw>,
    /// "Are you sure ?" up, waiting for its answer.
    pub confirm: Option<hud::Menu>,
    picture: Option<Image>,
    font: Option<Font>,
}

struct Script<'a> {
    clicks: &'a [(u16, u16)],
    answers: &'a [u16],
    ci: usize,
    ai: usize,
    starved: bool,
    draws: Vec<Draw>,
    confirm: Option<hud::Menu>,
}

impl OptionsHooks for Script<'_> {
    /// 0000:9AE6 (the font, B036) and 9AF8 (`text`: the boxes' tiles,
    /// B038) made current: Exit's [1026] and [1036] before "Are you sure ?"
    /// (2542:05DB), which is drawn in them.
    fn font(&mut self, m: &mut Machine, ax: u16, text: bool) {
        m.mem.w16(DS, if text { 0xB038 } else { 0xB036 }, ax);
    }
    fn picture(&mut self, _m: &mut Machine, _files: &mut Files, _name: u16) {
        self.draws.clear();
    }
    fn fill(&mut self, m: &mut Machine, ax: u16, bx: u16, cx: u16, dx: u16) {
        if !self.starved {
            self.draws.push(Draw::Fill(ax as i32, bx as i32, cx as i32, dx as i32, m.mem.r8(DS, var::COLOUR)));
        }
    }
    fn centred(&mut self, m: &mut Machine, si: u16, ax: u16, bx: u16) {
        if !self.starved {
            self.draws.push(Draw::Text(ax as i32, bx as i32, m.mem.cstr(DS, si).to_vec(), m.mem.r8(DS, var::COLOUR)));
        }
    }
    fn click(&mut self, m: &mut Machine) -> Option<(u16, u16)> {
        let Some(&(x, y)) = self.clicks.get(self.ci).filter(|_| !self.starved) else {
            self.starved = true;
            return None;
        };
        self.ci += 1;
        m.mem.w16(DS, var::MOUSE_X, x);
        m.mem.w16(DS, var::MOUSE_Y, y);
        Some((x, y))
    }
    fn confirm(&mut self, m: &mut Machine) -> u16 {
        if let Some(&a) = self.answers.get(self.ai) {
            self.ai += 1;
            return a;
        }
        if !self.starved {
            self.starved = true;
            let mut menu = crate::dialog::menu_at(m, var::SURE, var::SURE + 0x10, var::SURE + 0x13);
            // No entry marked: it takes clicks and Y, N, Esc (the view
            // marks a menu's entry only after the arrows move it).
            menu.selected = usize::MAX;
            self.confirm = Some(menu);
        }
        1
    }
}

impl OptionsUi {
    /// The screen opened on (a copy of) `engine`.
    pub fn open(engine: &Engine, archive: &crate::archive::Archive) -> (OptionsUi, Ended) {
        let base = Engine::new(engine.m.clone(), engine.files.clone());
        let picture = crate::panels::load_image(archive, &engine.m, "OP.IMG");
        let font = Font::load(archive, "ARENAFNT.DAT");
        let mut ui = OptionsUi { base, clicks: Vec::new(), answers: Vec::new(), draws: Vec::new(), confirm: None, picture, font };
        let e = ui.replay();
        (ui, e)
    }

    /// The screen run again from the start with the inputs given.
    fn replay(&mut self) -> Ended {
        let mut e = Engine::new(self.base.m.clone(), self.base.files.clone());
        let mut s = Script { clicks: &self.clicks, answers: &self.answers, ci: 0, ai: 0, starved: false, draws: Vec::new(), confirm: None };
        let end = e.m.options_screen(&mut e.files, &mut s);
        let starved = s.starved;
        self.draws = std::mem::take(&mut s.draws);
        self.confirm = s.confirm.take();
        match end {
            _ if starved => Ended::Open,
            // The CyberMan's leaving (not reached here) as Resume.
            None => Ended::Left(Box::new(e), OptionsEnd::Resume),
            Some(end) => Ended::Left(Box::new(e), end),
        }
    }

    /// A click at (x, y) of the screen (`font` the view's, as the menu is
    /// drawn with).
    pub fn click(&mut self, x: i32, y: i32, font: &Font) -> Ended {
        if let Some(menu) = &self.confirm {
            let hud = Hud { menu: Some(menu.clone()), ..Hud::default() };
            return match hud::menu_pick(font, &hud, x, y) {
                Some(i) => self.answer(i as u16),
                None => Ended::Open,
            };
        }
        self.clicks.push((x as u16, y as u16));
        self.replay()
    }

    /// The right button: "Are you sure ?" left (FFFFh, 3159:10D6) while
    /// it is up; the screen itself takes the left button's clicks alone.
    pub fn right(&mut self) -> Ended {
        if self.confirm.is_none() {
            return Ended::Open;
        }
        self.answer(0xFFFF)
    }

    /// A key: "Are you sure ?"'s Y, N, Esc (FFFFh) while it is up. The
    /// screen itself takes no keys (Esc gives 564D's FFFFh, which no box
    /// holds).
    pub fn key(&mut self, code: winit::keyboard::KeyCode) -> Ended {
        use winit::keyboard::KeyCode;
        if self.confirm.is_none() {
            return Ended::Open;
        }
        match code {
            KeyCode::KeyY => self.answer(0),
            KeyCode::KeyN => self.answer(1),
            KeyCode::Escape => self.answer(0xFFFF),
            _ => Ended::Open,
        }
    }

    fn answer(&mut self, a: u16) -> Ended {
        self.answers.push(a);
        self.confirm = None;
        self.replay()
    }

    pub fn draw(&self, screen: &mut Screen, font: &Font) {
        if let Some(p) = &self.picture {
            screen.blit(p.x, p.y, p.w, p.h, &p.px, 199);
        }
        if let Some(f) = &self.font {
            for d in &self.draws {
                match d {
                    Draw::Fill(x, y, w, h, c) => screen.fill(*x, *y, *w, *h, *c, 0xFF),
                    Draw::Text(x, y, t, c) => {
                        let s = String::from_utf8_lossy(t);
                        screen.text_plain(f, x - f.width(&s) as i32 / 2, *y, &s, *c);
                    }
                }
            }
        }
        if let Some(menu) = &self.confirm {
            let hud = Hud { menu: Some(menu.clone()), ..Hud::default() };
            hud::compose_over(screen, font, &hud);
        }
    }
}
