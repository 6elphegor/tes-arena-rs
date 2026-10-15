//! The save and load menus' screens (25FD:0002 and 0242, with the slot
//! picker 25FD:0718): the port's `Machine::save_menu` / `load_menu` run
//! on a copy of the engine with the clicks and names given so far, again
//! after each new one, as the view answers the game's questions (a turn
//! replayed with its answers). Where the menu wants an input not yet
//! given, the run stops there (the click taken as a cancel, the name as
//! Esc; that run is thrown away) and the screen shows what it drew up to
//! then: LOADSAVE.IMG, NAMES.DAT's names (from the save folder) centred
//! over their boxes, and while a name is typed (0000:9EE5, not ported:
//! the view's own editor, 2Fh characters at most) its box and the text.
//! The names are drawn in FONT_A.DAT: the view doesn't map the menu's
//! font record ([102C]) to its file.

use std::path::{Path, PathBuf};

use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mem::DS;
use arena::savemenu::{var, MenuHooks, SaveChoice};

use crate::hud::{Font, Screen};
use crate::interface::Image;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The save menu (25FD:0002).
    Save,
    /// The load menu in play (25FD:0242).
    Load,
    /// The load menu from the title screen's Load (5C8 bit 4 on the load).
    TitleLoad,
}

#[derive(Clone)]
enum Draw {
    Fill(i32, i32, i32, i32, u8),
    Text(i32, i32, Vec<u8>, u8),
}

/// How a replay of the menu ended.
pub enum Ended {
    /// Waiting for a click or a name.
    Open,
    /// A slot picked and named: the engine after the menu, to save.
    Save(Engine, u8),
    /// A slot picked to load: the engine after the menu.
    Load(Engine, u8),
    /// The menu left, with the message it showed (a refusal) if any.
    Left(Option<String>),
}

pub struct SaveUi {
    pub kind: Kind,
    base: Engine,
    save_dir: PathBuf,
    clicks: Vec<(u16, u16)>,
    names: Vec<Option<Vec<u8>>>,
    /// The name being typed, with its box (BOX_X, BOX_Y).
    pub typing: Option<(Vec<u8>, i32, i32)>,
    draws: Vec<Draw>,
    picture: Option<Image>,
    font: Option<Font>,
}

struct Script<'a> {
    clicks: &'a [(u16, u16)],
    names: &'a [Option<Vec<u8>>],
    dir: &'a Path,
    ci: usize,
    ni: usize,
    starved: bool,
    typing: Option<(Vec<u8>, i32, i32)>,
    draws: Vec<Draw>,
    message: Option<String>,
}

impl MenuHooks for Script<'_> {
    fn picture(&mut self, _m: &mut Machine, _files: &mut Files, _name: u16) {
        if !self.starved {
            self.draws.clear();
        }
    }
    /// NAMES.DAT from the save folder; missing, NAMES is left.
    fn names(&mut self, m: &mut Machine, _files: &mut Files, _path: u16) {
        if let Some(d) = read_any_case(self.dir, "NAMES.DAT") {
            for (i, &b) in d.iter().take(0x1E0).enumerate() {
                m.mem.w8(DS, var::NAMES + i as u16, b);
            }
        }
    }
    fn fill(&mut self, m: &mut Machine, ax: u16, bx: u16, cx: u16, dx: u16) {
        if !self.starved {
            self.draws.push(Draw::Fill(ax as i32, bx as i32, cx as i32, dx as i32, m.mem.r8(DS, 0x9288)));
        }
    }
    fn centred(&mut self, m: &mut Machine, si: u16, ax: u16, bx: u16) {
        if !self.starved {
            self.draws.push(Draw::Text(ax as i32, bx as i32, m.mem.cstr(DS, si).to_vec(), m.mem.r8(DS, 0x9288)));
        }
    }
    fn click(&mut self, _m: &mut Machine) -> (u16, u16) {
        if let Some(&c) = self.clicks.get(self.ci) {
            self.ci += 1;
            return c;
        }
        self.starved = true;
        (0xFFFF, 0)
    }
    fn type_name(&mut self, m: &mut Machine) {
        let given = self.names.get(self.ni).cloned();
        self.ni += 1;
        match given {
            Some(Some(t)) => {
                for (i, &b) in t.iter().take(0x2F).chain([0u8].iter()).enumerate() {
                    m.mem.w8(DS, var::TYPED + i as u16, b);
                }
            }
            other => {
                if other.is_none() && !self.starved {
                    self.starved = true;
                    let (x, y) = (m.mem.r16(DS, var::BOX_X) as i32, m.mem.r16(DS, var::BOX_Y) as i32);
                    self.typing = Some((m.mem.cstr(DS, var::TYPED).to_vec(), x, y));
                }
                let f = m.mem.r16(DS, var::FLAGS) & 0xEFFF;
                m.mem.w16(DS, var::FLAGS, f);
            }
        }
    }
    fn message(&mut self, m: &mut Machine, si: u16) {
        self.message = Some(String::from_utf8_lossy(m.mem.cstr(DS, si)).replace('\r', "\n"));
    }
}

impl SaveUi {
    /// The menu of `kind` opened on (a copy of) `engine`, the saves in
    /// `save_dir`.
    pub fn open(kind: Kind, engine: &Engine, save_dir: &Path, archive: &crate::archive::Archive) -> (SaveUi, Ended) {
        let base = Engine { m: engine.m.clone(), files: engine.files.clone() };
        let picture = crate::panels::load_image(archive, &engine.m, "LOADSAVE.IMG");
        let font = Font::load(archive, "FONT_A.DAT");
        let mut ui = SaveUi { kind, base, save_dir: save_dir.to_path_buf(), clicks: Vec::new(), names: Vec::new(), typing: None, draws: Vec::new(), picture, font };
        let e = ui.replay();
        (ui, e)
    }

    /// The menu run again from the start with the inputs given.
    fn replay(&mut self) -> Ended {
        let mut e = Engine { m: self.base.m.clone(), files: self.base.files.clone() };
        let mut s = Script { clicks: &self.clicks, names: &self.names, dir: &self.save_dir, ci: 0, ni: 0, starved: false, typing: None, draws: Vec::new(), message: None };
        let end = match self.kind {
            Kind::Save => match e.m.save_menu(&mut e.files, &mut s) {
                SaveChoice::Slot(n) if !s.starved => Some(Ok(n)),
                SaveChoice::Refused => Some(Err(s.message.take())),
                SaveChoice::Cancelled if !s.starved => Some(Err(None)),
                _ => None,
            },
            Kind::Load | Kind::TitleLoad => match e.m.load_menu(&mut e.files, &mut s) {
                Some(n) if !s.starved => Some(Ok(n)),
                None if !s.starved => Some(Err(None)),
                _ => None,
            },
        };
        self.draws = std::mem::take(&mut s.draws);
        self.typing = s.typing.take();
        match end {
            None => Ended::Open,
            Some(Ok(n)) if self.kind == Kind::Save => Ended::Save(e, n),
            Some(Ok(n)) => Ended::Load(e, n),
            Some(Err(msg)) => Ended::Left(msg),
        }
    }

    /// A click at (x, y) of the screen (the right button cancels, as a
    /// negative x does).
    pub fn click(&mut self, x: i32, y: i32, right: bool) -> Ended {
        if self.typing.is_some() {
            return Ended::Open;
        }
        self.clicks.push(if right { (0xFFFF, 0) } else { (x as u16, y as u16) });
        self.replay()
    }

    /// A key: while a name is typed, its characters, Backspace, Enter
    /// (the name taken) and Esc (the menu again); otherwise Esc leaves.
    pub fn key(&mut self, code: winit::keyboard::KeyCode, text: Option<&str>) -> Ended {
        use winit::keyboard::KeyCode;
        let Some((t, _, _)) = &mut self.typing else {
            if code == KeyCode::Escape {
                self.clicks.push((0xFFFF, 0));
                return self.replay();
            }
            return Ended::Open;
        };
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let t = t.clone();
                self.names.push(Some(t));
                self.replay()
            }
            KeyCode::Escape => {
                self.names.push(None);
                self.replay()
            }
            KeyCode::Backspace => {
                t.pop();
                Ended::Open
            }
            _ => {
                for c in text.unwrap_or("").bytes().filter(|c| (0x20..0x7F).contains(c)) {
                    if t.len() < 0x2F {
                        t.push(c);
                    }
                }
                Ended::Open
            }
        }
    }

    pub fn draw(&self, screen: &mut Screen) {
        if let Some(p) = &self.picture {
            screen.blit(p.x, p.y, p.w, p.h, &p.px, 199);
        }
        let Some(font) = &self.font else { return };
        for d in &self.draws {
            match d {
                Draw::Fill(x, y, w, h, c) => screen.fill(*x, *y, *w, *h, *c, 0xFF),
                Draw::Text(x, y, t, c) => {
                    let s = String::from_utf8_lossy(t);
                    screen.text_plain(font, x - font.width(&s) as i32 / 2, *y, &s, *c);
                }
            }
        }
        if let Some((t, x, y)) = &self.typing {
            screen.fill(*x, *y, 0x135, 9, 0, 0xFF);
            let s = format!("{}_", String::from_utf8_lossy(t));
            screen.text_plain(font, x + 0x0A, *y, &s, 0x0F);
        }
    }
}

/// The file `name` in `dir`, its name's case ignored (the game writes
/// lower case, DOS copies upper).
pub fn read_any_case(dir: &Path, name: &str) -> Option<Vec<u8>> {
    let e = std::fs::read_dir(dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))?;
    std::fs::read(e.path()).ok()
}
