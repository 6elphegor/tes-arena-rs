//! The save and load menus' screens (25FD:0002 and 0242, with the slot
//! picker 25FD:0718): the port's `Machine::save_menu` / `load_menu` run
//! on a copy of the engine with the clicks and names given so far, again
//! after each new one, as the view answers the game's questions (a turn
//! replayed with its answers). Where the menu wants an input not yet
//! given, the run stops there (the click taken as a cancel, the name as
//! Esc; that run is thrown away) and the screen shows what it drew up to
//! then: LOADSAVE.IMG, NAMES.DAT's names (from the save folder) centred
//! over their boxes, and while a name is typed its box and the game's
//! editor on it (0000:9EE5, `field::Live`, 2Fh characters at most, in
//! ARENAFNT.DAT ([102C]) and colour 0Fh; its passes' inputs the name's
//! answer, given to the editor in the replay). The names are drawn in the
//! font selected as they are (B036: the picker's [102C], ARENAFNT.DAT).

use std::path::{Path, PathBuf};

use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mem::DS;
use arena::savemenu::{var, MenuHooks, SaveChoice};

use crate::field::{Live, Look};
use crate::hud::{Font, GameFont, Screen};
use arena::lineedit::EditKey;

/// The name's field's look: ARENAFNT.DAT ([102C], 25FD:0039), colour 0Fh
/// (25FD:0093).
const LOOK: Look = Look { font: "ARENAFNT.DAT", colour: 0x0F, put_font: true };
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
    /// A text centred at x, at y, in its colour and the font selected.
    Text(i32, i32, Vec<u8>, u8, GameFont),
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
    names: Vec<Vec<EditKey>>,
    /// The name being typed (the game's editor on a copy of the game as
    /// the menu asked).
    pub typing: Option<Box<Live>>,
    draws: Vec<Draw>,
    picture: Option<Image>,
    /// The four fonts the game selects among (`GameFont`).
    fonts: Vec<(GameFont, Font)>,
    /// A message box over the menu, read with a click or a key: an empty
    /// slot's (1194:59C8), before the picker again (25FD:026B).
    pub notice: Option<Vec<Vec<crate::hud::Span>>>,
}

struct Script<'a> {
    clicks: &'a [(u16, u16)],
    names: &'a [Vec<EditKey>],
    dir: &'a Path,
    ci: usize,
    ni: usize,
    starved: bool,
    typing: Option<Box<Live>>,
    draws: Vec<Draw>,
    message: Option<String>,
}

impl MenuHooks for Script<'_> {
    /// 0000:9AE6 (the font, B036) and 9AF8 (`text`: the boxes' tiles,
    /// B038) made current.
    fn font(&mut self, m: &mut Machine, ax: u16, text: bool) {
        m.mem.w16(DS, if text { 0xB038 } else { 0xB036 }, ax);
    }
    fn picture(&mut self, _m: &mut Machine, _files: &mut Files, _name: u16) {
        if !self.starved {
            self.draws.clear();
        }
    }
    /// NAMES.DAT from the save folder (the original's save path); missing,
    /// NAMES is left.
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
            self.draws.push(Draw::Text(ax as i32, bx as i32, m.mem.cstr(DS, si).to_vec(), m.mem.r8(DS, 0x9288), GameFont::selected(m)));
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
    /// 25FD:0098..00B0: the name's typing given to the game's editor; not
    /// given yet, the editor opened for the window (the run stopped:
    /// taken as Esc, FLAGS bit 1000h cleared).
    fn type_name(&mut self, m: &mut Machine) {
        let field = arena::lineedit::fields::save_name(m);
        let given = self.names.get(self.ni).cloned();
        self.ni += 1;
        match given {
            Some(keys) => {
                if crate::field::replay(m, &field, &LOOK, &keys) != Some(true) {
                    eprintln!("the save's name gave up: {}", arena::giveup::describe());
                }
            }
            None => {
                if !self.starved {
                    self.starved = true;
                    self.typing = Live::open(m, field, LOOK).map(Box::new);
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
        let base = Engine::new(engine.m.clone(), engine.files.clone());
        let picture = crate::panels::load_image(archive, &engine.m, "LOADSAVE.IMG");
        let fonts = [(GameFont::A, "FONT_A.DAT"), (GameFont::B, "FONT_B.DAT"), (GameFont::Arena, "ARENAFNT.DAT"), (GameFont::Teeny, "TEENYFNT.DAT")]
            .into_iter()
            .filter_map(|(g, name)| Font::load(archive, name).map(|f| (g, f)))
            .collect();
        let mut ui = SaveUi { kind, base, save_dir: save_dir.to_path_buf(), clicks: Vec::new(), names: Vec::new(), typing: None, draws: Vec::new(), picture, fonts, notice: None };
        let e = ui.replay();
        (ui, e)
    }

    /// The menu run again from the start with the inputs given.
    fn replay(&mut self) -> Ended {
        let mut e = Engine::new(self.base.m.clone(), self.base.files.clone());
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

    /// The slot picked had no game (25FD:028D): `m`, the game as the menu
    /// left it, says so in a message box (1194:59C8, the text at 5F0F in
    /// the font and colour selected, centred in the clip as 59CC centres
    /// it), then the picker is up again (025FD:026B), the click on the
    /// empty slot forgotten.
    pub fn empty_slot(&mut self, m: &Machine) {
        use crate::hud;
        let lines = hud::in_font(crate::dialog::text_at(m, arena::load::var::NO_SAVE, m.mem.r8(DS, 0x9288)), GameFont::selected(m));
        let r = |at| m.mem.r16(DS, at) as i16 as i32;
        self.notice = Some(hud::clipped(lines, (r(0xA914), r(0xA916), r(0xA918), r(0xA91A))));
        self.clicks.pop();
        self.replay();
    }

    /// A click at (x, y) of the screen (the right button cancels, as a
    /// negative x does).
    pub fn click(&mut self, x: i32, y: i32, right: bool) -> Ended {
        if self.notice.take().is_some() {
            return Ended::Open;
        }
        if let Some(l) = &mut self.typing {
            // The right button ends the name's field (A172).
            if right {
                l.button();
            }
            return self.name_ended();
        }
        self.clicks.push(if right { (0xFFFF, 0) } else { (x as u16, y as u16) });
        self.replay()
    }

    /// A key pressed: with no name typed, Esc leaves.
    pub fn key(&mut self, code: winit::keyboard::KeyCode, _text: Option<&str>) -> Ended {
        if self.notice.take().is_some() {
            return Ended::Open;
        }
        if self.typing.is_none() && code == winit::keyboard::KeyCode::Escape {
            self.clicks.push((0xFFFF, 0));
            return self.replay();
        }
        Ended::Open
    }

    /// A key pressed or let go while a name is typed: to the game's
    /// editor (Enter takes the name, Esc the menu again); the field
    /// ended, the menu run again with its typing.
    pub fn field_key(&mut self, scan: u8, down: bool) -> Ended {
        if let Some(l) = &mut self.typing {
            l.key(scan, down);
        }
        self.name_ended()
    }

    fn name_ended(&mut self) -> Ended {
        match self.typing.as_ref().filter(|l| l.ended()).map(|l| l.keys().to_vec()) {
            Some(keys) => {
                self.typing = None;
                self.names.push(keys);
                self.replay()
            }
            None => Ended::Open,
        }
    }

    /// A frame: the name's field's (`Live::frame`).
    pub fn frame(&mut self) -> Ended {
        if let Some(l) = &mut self.typing {
            l.frame();
        }
        self.name_ended()
    }

    pub fn draw(&self, screen: &mut Screen) {
        if let Some(p) = &self.picture {
            screen.blit(p.x, p.y, p.w, p.h, &p.px, 199);
        }
        for d in &self.draws {
            match d {
                Draw::Fill(x, y, w, h, c) => screen.fill(*x, *y, *w, *h, *c, 0xFF),
                Draw::Text(x, y, t, c, g) => {
                    let Some((_, font)) = self.fonts.iter().find(|(f, _)| f == g) else { continue };
                    let s = String::from_utf8_lossy(t);
                    screen.text_plain(font, x - font.width(&s) as i32 / 2, *y, &s, *c);
                }
            }
        }
        // 25FD:0076..0088: the name's box cleared (9A6E, colour 0), then
        // the field.
        if let Some(l) = &self.typing {
            screen.fill(l.field.x as i32 - 0x0A, l.field.y as i32, 0x135, 9, 0, 0xFF);
            l.draw(screen, false);
        }
        if let (Some(lines), Some((_, font))) = (&self.notice, self.fonts.iter().find(|(f, _)| *f == GameFont::Arena)) {
            crate::hud::compose_over(screen, font, &crate::hud::Hud { boxed: Some(lines.clone()), ..Default::default() });
        }
    }
}

/// The file `name` in `dir`, its name's case ignored (the game writes
/// lower case, DOS copies upper).
pub fn read_any_case(dir: &Path, name: &str) -> Option<Vec<u8>> {
    let e = std::fs::read_dir(dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))?;
    std::fs::read(e.path()).ok()
}
