//! arena-view: walk a level of Arena, drawn with wgpu, with the port's
//! engine running the game.
//!
//! Usage: arena-view GAME_DIR LEVEL.MIF [--fight] [--people] [--click] [--gpu-world] [--hour H] [--at COL ROW HEADING] [--wait N] [--do STEPS] [--picks N,..] [--save-dir DIR] [--slot N] [--shot OUT.png [FRAMES [TURN]]]
//!
//! Keys are the original's. Its menu keys (2542:0100, in this order, the
//! first pressed taken: `arena::menukeys`, run on the engine at the
//! press, the key's state set where the game keeps it):
//!
//! | Key (scan) | Handler | Action |
//! |---|---|---|
//! | Esc (01) | 000A | the options screen |
//! | A (1E) | 0137 | the weapon readied or put away (unless C39A bit 4000h) |
//! | C (2E) | 026D | the spellbook, unless 5CA bit 8 |
//! | P (19) | 0279 | picking pockets armed |
//! | S (1F) | 037C | the status box |
//! | U (16) | 034D | the use list |
//! | M (32) | 045A / 03BF | the automap; with Shift the world map |
//! | F1 (3B) | 00BE | a touch spell waiting cast on the player, else the sheet |
//! | F8 (42) | 01C5 | the compass (5C8 bit 2000h) |
//! | L (26) | 02A3 | the logbook |
//! | R (13) | 01E6 | camping |
//!
//! The frame body reads its own keys from the key state (- and =, F2,
//! F3, F4, N, B+Q+X, Alt+F5). Moving (2826:0473): Up/Down forward and
//! back, Left/Right turn, Ctrl+Left/Right strafe, J jumps (Shift for a
//! running jump). The view's extras, on keys the game leaves alone: the
//! mouse turns (click the window to capture it, Esc to release), Space
//! jumps, E or a click (captured) clicks the centre of the view (opens a
//! door), Q readies as A, I the inventory, K the sheet, F5 (without Alt)
//! and F9 the quick save and load, F an encounter, Shift+F people. With --shot it renders one
//! picture after turning by TURN (heading units, 512 a turn) and FRAMES
//! frames (default 0) of walking forward, and exits; --wait stands still
//! N frames before the walk, --click clicking first; --at puts the
//! player in a square's centre, facing HEADING, a debugging aid; --hour
//! sets the clock to that hour. --fight brings a random encounter at the
//! start (F does in the window); --people (Shift+F) rolls until people come.
//! The 3D view is the original's renderer's (`arena::render`); --gpu-world
//! draws it with the view's own polygons instead.
//! --do runs STEPS before the shot, comma-separated: `click`, `turn:N`,
//! `walk:N` (frames), `wait:N`, `at:COL:ROW:HEADING` (as --at, then a
//! frame), `camp` (R), `save` (F5), `load` (F9), `key:N` (the key with scan
//! code N down for a frame: 60 is F2, the player's square on the message
//! line), `shop:KIND` (a debugging aid: the shop of building kind KIND
//! entered, as talking to its keeper enters it: 4 a tavern, 5 a temple, 6
//! an equipment store) and `quest` (a debugging aid: the talk pointed at
//! the giver of the first errand offered, then `shop:4`, the tavern's
//! talk offering it). --picks answers the game's questions in order: a menu's entry
//! by its number (talking to someone: 0 the greeting, 1 "Where is...", 2
//! rumours; camping: 0 for a while, 1 until healed), `tTEXT` text typed
//! (the hours to rest); after them a question is left, as Esc leaves it,
//! and a shot shows it.
//!
//! In the window the game's texts are drawn in its own font (ARENAFNT.DAT):
//! the message line at the top, message boxes (Enter, Space or a click
//! reads them), menus (arrows and Enter, an entry's letter or number, a
//! click; Esc leaves them), the hours typed (digits, Enter). R camps; F5
//! saves to slot --slot N (default 0) in --save-dir DIR; F9 loads it back
//! as the game loads in play (src/load.rs), into whatever area it was
//! saved in; --load loads it at the start. Esc opens the game's options
//! screen (`options`: the volumes, the detail, New, Exit), whose Save and
//! Load open the game's save and load menus on --save-dir (`saveui`: a
//! slot clicked, a name typed, Enter; Esc or the right button leaves), and
//! the main menu's Load opens the load menu, loading the title screen's
//! way. While the game waits for an
//! answer no time passes.
//!
//! The player's interface is drawn from the game's own graphics as the
//! original places it (`interface`): the world in the 320 x 147 above the
//! bar, the bar with the portrait, the condition, the name and the three
//! bars, the compass (F8 hides it), the weapon in hand and the casting
//! hands. Q readies or puts away the weapon (the original's A); holding
//! the right button (captured) and dragging swings it; let go without a
//! drag, it looks (`look_centre`, 2826:0C15). --do gains
//! `ready`, `compass`, `swing:DX:DY` (the right button down with a drag
//! of DX, DY mickeys for a turn: a swing starts only from a drag of 40h
//! (64) or more, 0000:3C9F, so `swing:0:60` does nothing) and `cast:N`
//! (spell N of the spellbook cast). E.g. `--sound-log --do
//! ready,wait:2,swing:0:100,wait:3` logs the swing's swish.voc.
//! `storm:N` runs up to N frames, the sky drawn after each as the
//! window draws it, until the lightning flashes (outdoors, raining at
//! night, with thunder: e.g. CITY:8:0:1:2 --hour 22), for a shot of it.
//!
//! I opens the inventory (`panels`: the original's equipment screen):
//! a left click on an item puts it on or takes it off, a right click
//! selects it, D (or the Drop button) drops it, U lists the items with an
//! effect to use one (arrows, Enter), Up/Down scroll, Esc or Exit closes.
//! C opens the spellbook: a click or Enter casts the spell; one with a
//! target is then aimed by a click in the view. A corpse or chest clicked
//! shows its loot in a list (the loot window): a pick takes the item.
//! Uncaptured, a left click in the view clicks where it is (a corpse on
//! the floor); another button captures the mouse. --do gains `inv`,
//! `equip:N` and `select:N` (the list's line N), `drop`, `usable` (the
//! use list up), `use:N`, `spells`, `book` (the spellbook printed),
//! `spell:N` (its entry N cast), `close`, `clickat:X:Y` (a click at X, Y
//! of the 320 x 200 screen), and as debugging aids `give:N` (N items
//! made and given), `learn:N` (spell N put in the book), `sp:N` (spell
//! points), `gold:N`, `hp:N` (the health), `day:N` (the clock, 5CC, N days
//! on), `mday:N` (the day of the month, CCA8: the moons' places and
//! phases), `material:N` (every worn piece of armour enchanted, of
//! material N: the doll's colours), `wear:N` (item N's condition halved), `class:N`, `corpse` (a
//! creature killed a square ahead), `fatigue:N` (the player's fatigue; at
//! 0 the next minute exhausts them) and `xp:N` (N experience gained: the
//! next minute checks the level, and a level gained brings the level-up
//! screen).
//! M shows the automap (`automap`; `--do map`): the squares the game has
//! marked seen, kept per area in AUTOMAP.nn (src/automap.rs).
//!
//! A frame that leaves the area (a building's door, an interior's way
//! out) goes on to the next area as the game's outer loop picks it, and
//! the view loads it (`areas`).
//!
//! --new-game SPEC makes a character as the original's creation does and
//! starts the game in the start dungeon (LEVEL.MIF may be left out:
//! START.MIF): SPEC is `CLASS:PROVINCE:SEX:FACE:NAME`, CLASS 0..17 from
//! the class list or `q` and ten answers 0..2 to the questions (see
//! `newgame`).
//!
//! --create [SCRIPT] makes the character in the window, on the original's
//! creation screens (`create`): how the class is chosen, the questions or
//! the list, the name, the gender, the province on the map, what the
//! player is told, the attributes, the face, the vision's text; then the
//! game starts in the start dungeon. SCRIPT gives keys and clicks first,
//! comma-separated: a character, `enter`, `esc`, `back`, `up`, `down`,
//! `type:TEXT`, `click:X:Y` (on the 320 x 200 screen); with --shot the
//! screen they reach is the picture (the game's, once creation is over).
//! The menu's Start plays the opening story first (SCROLL.FLC, then
//! INTRO01..09.IMG, 5 s each); a key ends it and is spent doing so, so a
//! script gives one (e.g. `esc`) after `S`.
//!
//! Sound (`audio`): the game's sounds play through PulseAudio (opened at
//! run time; nothing without it); --mute plays none, --sound-log prints
//! each the game plays (shots play none, and with --sound-log print
//! them). Music (`music`): the songs the game picks (the town's, then the
//! weather's and time's; a building's; a dungeon's) play on an emulated
//! FM chip with the game's timbres (SAMPLE.AD), as its Sound Blaster
//! driver played them, on a stream of their own; --no-music (or --mute)
//! plays none, and then no music driver is loaded, as with the original's
//! "No Music"; --sound-log prints the songs asked for. The animations the game plays (the dream, the Emperor's
//! audience, the quest's end) show over it, a key their next lines or
//! their end; `--do movie:NAME[:TEXT[:COLOUR]]` queues one (a debugging
//! aid; NAME may be a still .IMG, shown as the ending's house is).

mod archive;
mod areas;
mod audio;
mod automap;
mod bench;
mod create;
mod dialog;
mod doll;
mod film;
mod flats;
mod flc;
mod field;
mod fmdrv;
mod hud;
#[cfg(test)]
mod inputtest;
#[cfg(test)]
mod hooktest;
#[cfg(test)]
mod doortest;
mod interface;
mod light;
mod music;
mod newgame;
mod panels;
mod playtest;
mod prof;
mod questui;
mod render;
mod options;
mod saveui;
mod sheets;
mod shopui;
mod spellmaker;
mod travel;
mod world;

use arena::controls::{ControlHooks, Intent};
use arena::engine::{Engine, EngineHooks, Input};
use light::{LightNow, LightState, ENTRY_LIGHT, SKY_NAMES, GameSkyLoad, sky_objects_made};
use render::{Camera, Renderer, Scene};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
thread_local! {
    /// The load bank as the town's welcome left it (`town_welcome`).
    static WELCOME_KEPT: std::cell::RefCell<Option<Vec<u8>>> = const { std::cell::RefCell::new(None) };
    /// DGROUP as the town's welcome box left it (`town_welcome`).
    static WELCOME_BOX: std::cell::RefCell<Option<Vec<u8>>> = const { std::cell::RefCell::new(None) };
}

/// The front end's hooks. With the sprites (`sheet`, animations at
/// `tick`) the drawing makes the hit test for a click left for the actors
/// (`clicked`); with the light state the stairs make the light table
/// again; with `talk` the game's questions are answered and its boxes
/// kept (`dialog`), else its menus are left and its texts printed.
#[derive(Default)]
struct Front<'a> {
    sheet: Option<&'a world::SpriteSheet>,
    light: Option<&'a mut LightState>,
    tick: usize,
    clicked: bool,
    talk: Option<&'a mut dialog::Talk>,
    /// The window's width over its height (the click's line; 0: 1.6).
    aspect: f32,
    /// An effect's words the frame drew (0000:5011), with the row: the
    /// HUD shows them till the next frame.
    effect: Option<(Vec<u8>, i32)>,
    /// The screen as last shown (`Assets::shown`): the page at the frame's
    /// start.
    shown: Option<&'a [u8]>,
    /// The screen as the frame's drawing leaves the page (`drawn`): its 3D
    /// view this frame's, the rest `shown`'s; None till the frame draws.
    drawn: Option<Vec<u8>>,
    /// A left click in the view waiting for the frame's input step
    /// (`ControlHooks::take_click`): where on the 320 x 200 screen.
    click: Option<(i32, i32)>,
    /// The colours on the screen (`Assets::dac`), as the VGA's 6-bit
    /// values: what the shops read on entering (1194:5496).
    dac: Option<[[u8; 3]; 256]>,
    /// The interface's pictures (`Assets::interface`), for the hands and
    /// the compass the frame body draws on the page before the click.
    interface: Option<&'a mut interface::Interface>,
}

impl Front<'_> {
    /// 2542:01E6: camping's checks and its rest begun (`Machine::camp`),
    /// its menu and hours asked as the turn's questions.
    fn camp(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if m.camp(files, self).is_none() {
            eprintln!("camping gave up: {}", arena::giveup::describe());
        }
    }

    /// The screen a menu key or the bar's button asks for, inside the
    /// frame (`Talk::screen`): the travel map's journey and the options
    /// screen's Load and New too, the rest of the frame run on what they
    /// leave (2542:0434 sets C398 bit 80h, 0627 with 5CA bit 8000h: the
    /// frame's end leaves the world loop).
    fn taken(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, taken: Option<sheets::Taken>) {
        if let (Some(t), Some(talk)) = (taken, self.talk.as_deref_mut()) {
            talk.screen(m, files, t);
        }
    }

    /// The screen a box covers as it opens: the page as the frame's drawing
    /// left it, or before the frame draws the one last shown.
    fn box_screen(&self) -> Option<&[u8]> {
        self.drawn.as_deref().or(self.shown)
    }

    /// The screen a message box or a menu keeps what it covers of: none
    /// where the page is the game's own block (the box reads the page, on
    /// which it then draws and puts back: `Machine::box_background`),
    /// else `box_screen`.
    fn box_under(&self, m: &arena::machine::Machine) -> Option<&[u8]> {
        if m.page_is_block() {
            None
        } else {
            self.box_screen()
        }
    }

    /// 1194:59C8: a message box with the text at DS:`si`, shown (`show`)
    /// and what it leaves in the game's state (`Machine::message_shown`:
    /// the box laid out, its background kept, the dissolve, put away
    /// after the player's click unless 5E0 bit 800h).
    fn message_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        self.show(m, si);
        if m.message_wait_box((arena::mem::DS, si), self.box_under(m)).is_none() {
            eprintln!("the message box gave up: {}", arena::giveup::describe());
            return;
        }
        // The page as the box's wait (1194:5536) finds it: what the window
        // shows of the box.
        self.box_page(m);
        m.message_waited();
    }

    /// The game's page as it stands (the game's own block: the box the
    /// engine just drew on it) given the box last kept (`Talk::paged`).
    fn box_page(&mut self, m: &arena::machine::Machine) {
        if let (Some(t), Some(p)) = (self.talk.as_deref_mut(), game_page(m)) {
            t.paged(std::sync::Arc::new(p));
        }
    }

    /// The box with the text at DS:`si` kept for the window (`dialog`), or
    /// printed.
    fn show(&mut self, m: &mut arena::machine::Machine, si: u16) {
        match self.talk.as_deref_mut() {
            Some(t) => t.message_box(m, si),
            None => println!("{}", String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, si)).replace('\r', "\n")),
        }
    }

    /// A menu (1194:020E): its title (DS:`si`) and entries (the words at
    /// DS:`di`, to a 0) in boxes, as the game's state has them round the
    /// pick (`Machine::menu_boxes`, `menu_closed`); a menu too big for
    /// the screen isn't shown, FFFFh.
    fn menu(&mut self, m: &mut arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        use arena::mem::DS;
        let colour = m.mem.r8(DS, 0x9288);
        let Some(boxes) = m.menu_boxes(si, di, self.box_under(m)) else {
            eprintln!("the menu gave up: {}", arena::giveup::describe());
            return 0xFFFF;
        };
        // The lines are drawn from the colour the caller left (the boxes'
        // colour codes change 9288 as they are drawn).
        let drawn = m.mem.r8(DS, 0x9288);
        m.mem.w8(DS, 0x9288, colour);
        let page = game_page(m);
        let pick = match (&boxes, self.talk.as_deref_mut()) {
            (Some(b), Some(t)) => {
                // The page only for the question this menu asks: a turn
                // runs on past one left unanswered (a shop's buttons,
                // 1194:6001), and may reach a menu after it.
                let asking = t.need.is_none();
                let pick = t.menu(m, si, bx, di);
                if asking && t.need.is_some() && t.need_page.is_none() {
                    t.need_page = page.map(std::sync::Arc::new);
                }
                m.menu_reads = t.menu_reads.take();
                m.menu_answered(b, bx, pick)
            }
            _ => 0xFFFF,
        };
        m.mem.w8(DS, 0x9288, drawn);
        m.menu_closed(boxes.as_ref());
        pick
    }

    /// An animation played (with a text's number, as a vision).
    fn movie(&mut self, name: &'static str, text: Option<u16>, colour: u8) {
        match self.talk.as_deref_mut() {
            Some(t) => t.movies.push((name, text, colour)),
            None => println!("(the animation {name})"),
        }
    }
}

/// The Emperor's audience's screens (`QuestHooks::audience`, 0000:41B6):
/// each part run on the game (`Machine::audience_part`, the colours the
/// screen's), and shown as MORPH.FLC (2294:0895), then JAGAR.FLC with
/// text 5A7h in 39h (1194:D459).
struct Audience<'a, 'b>(&'a mut Front<'b>);

impl arena::quest::QuestHooks for Audience<'_, '_> {
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        self.0.dac
    }
    fn audience(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, part: u8) -> Option<()> {
        let dac = self.0.dac;
        m.audience_part(files, dac.as_ref(), part)?;
        match part {
            0 => self.0.movie("MORPH.FLC", None, 0),
            1 => self.0.movie("JAGAR.FLC", Some(0x5A7), 0x39),
            _ => {}
        }
        Some(())
    }
}

/// The stairs' hooks: the light table made again from NORMAL.LGT
/// (2294:0B8A); a main quest dungeon's levels' locks and triggers kept
/// in STATES.nn, and the automap's keep and load (0000:368E), as
/// `areas::Kept`.
impl arena::hooks::StairsHooks for Front<'_> {
    fn light_table(&mut self, m: &mut arena::machine::Machine) {
        if let Some(l) = self.light.as_deref_mut() {
            l.table(m, arena::weather::NORMAL_LGT);
        }
    }
    fn automap_keep(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        arena::hooks::StairsHooks::automap_keep(&mut areas::Kept, m, files);
    }
    fn automap_load(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        arena::hooks::StairsHooks::automap_load(&mut areas::Kept, m, files);
    }
    fn level_screen(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        arena::hooks::StairsHooks::level_screen(&mut areas::Kept, m, files);
    }
    /// 26F7:0000, the stairs out of a dungeon: the travel map shown
    /// (`dialog::Talk::travel`, the turn run again with the screen's
    /// end); its carry leaves. Without the dialog, stays.
    fn ask_leave(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> bool {
        match self.talk.as_deref_mut() {
            Some(t) => t.travel(m, files),
            None => false,
        }
    }
    /// The colours A611 reads: the screen's.
    fn map_keep(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        m.map_keep(files, self.dac.as_ref());
    }
}
/// How the sheet's drop came out (`App::sheet_drop`): a question asked
/// (its text), "Drop what?" waiting in its loop (its lines and their first
/// row), or done.
enum SheetDrop {
    Asked(Vec<u8>, hud::Menu),
    /// "Drop what?" waiting: its lines and their row, and the list's first
    /// line shown (1297) as its loop's arrows left it.
    Waiting((Vec<(String, u8)>, u16), u8),
    Done,
}

/// What the drop on the sheet's own page has taken since its key (or its
/// button): "Drop what?"'s loop's inputs (2325) and the question's answers
/// (yes, and the answer's key if typed), in their order.
#[derive(Clone, Copy, Debug)]
pub enum DropStep {
    Input(arena::charsheet::PageInput),
    Answer(bool, Option<u8>),
}

impl ControlHooks for Front<'_> {
    /// The window's click, in the frame's input step where the original
    /// takes it (after that frame's actors): the mouse put 4 up and left
    /// of it (the click, CB60, is the cursor + 4), then the click
    /// (`Machine::view_click`); one left for the actors (C39A bit 80h) is
    /// found by the next frame's drawing.
    fn take_click(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        let Some((x, y)) = self.click.take() else { return Some(()) };
        m.set_mouse((x - 4).max(0) as u16, (y - 4).max(0) as u16, 0);
        m.view_click(files, self)
    }
    /// 2542:00BA: the window's click on the bar, in the frame's input step
    /// where the original takes it (after that frame's actors and
    /// drawing): its button's handler (`sheets::bar_click`), camping's in
    /// place; a screen it asks for opens inside the frame (`taken`).
    fn bar_click(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<bool> {
        let Some((x, y, right)) = self.talk.as_deref_mut().and_then(|t| t.bar.take()) else { return Some(false) };
        let taken = match sheets::bar_click(m, files, x, y, right) {
            None | Some(sheets::BarAction::None) => None,
            Some(sheets::BarAction::Camp) => {
                self.camp(m, files);
                None
            }
            Some(a) => Some(sheets::Taken::Action(a)),
        };
        self.taken(m, files, taken);
        Some(true)
    }
    /// 2826:0008: the stat bars drawn into the game's page (`bars_draw`;
    /// the window draws its own).
    fn bars(&mut self, m: &mut arena::machine::Machine) {
        m.bars_draw();
    }
    /// A click on a building's door, a city's gate or an interior's way
    /// out (0000:43B1).
    fn wall_trigger(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wall_trigger(files, self)
    }
    /// The wilderness's chunks follow the player (217B:0265).
    fn wild_recentre(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_shift(files, &mut arena::wild::PortWild)
    }
}
/// The game's draw page ([A900], FA00h bytes), where it is the game's own
/// block: it holds what the original's does (the 3D view, the interface,
/// the hands, compass and message line the engine draws), which the boxes
/// and lists cover. None where the page isn't (the view's composition
/// stands in).
fn game_page(m: &arena::machine::Machine) -> Option<Vec<u8>> {
    let seg = m.mem.r16(arena::mem::DS, 0xA900);
    m.page_is_block().then(|| m.mem.segment(seg)[..0xFA00].to_vec())
}
impl arena::weapon::WeaponHooks for Front<'_> {
    /// A building broken into by the swing.
    fn wall_trigger(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wall_trigger(files, self)
    }
    /// A quest target's words on giving up, the game's box (1194:59C8).
    fn surrender_box(&mut self, m: &mut arena::machine::Machine, text: u16) {
        Front::message_box(self, m, text);
    }
}
/// The triggers' messages are the game's boxes; entering an area isn't
/// drawn yet (the frame ends: `Frame::Left`).
impl arena::trigger::TriggerHooks for Front<'_> {
    fn message_box(&mut self, m: &mut arena::machine::Machine, text: u16) {
        Front::message_box(self, m, text);
    }
    /// 1194:55B2: the colours faded to black in 8 (the window plays it
    /// as the area changes, `App::area_fade`).
    fn blackout(&mut self, _m: &mut arena::machine::Machine) {
        if let Some(t) = self.talk.as_deref_mut() {
            t.blackout = true;
        }
    }
    /// 0000:41A4..4247: the audience, its screens as animations.
    fn palace_audience(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.emperor_audience(files, &mut Audience(self))
    }
    /// 0000:425F: at stage 9 the artifact's audience, text 5DDh (137A)
    /// shown in its box (1194:A36C, AX 0).
    fn artifact_audience(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.text(files, self, 0, arena::mem::DS, 0).map(|_| ())
    }
    /// A city's gate out: the city written as the wilderness's blocks
    /// 1..4 (217B:027A; the files are kept in memory).
    fn leave_city(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_leave_city(files, &mut arena::wild::PortWild);
        Some(())
    }
    /// A dungeon's entrance in the wilderness: the wilderness left
    /// (217B:04DC); the dungeon is made after (`areas::next`).
    fn enter_dungeon(&mut self, m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_done();
        Some(())
    }
}
/// A fight's hooks through the front end: Jagar's words before a main
/// quest's encounter (1194:D91C) played as the original plays them.
struct FightFront<'a, 'b>(&'a mut Front<'b>);

impl arena::encounter::FightHooks for FightFront<'_, '_> {
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        self.0.dac
    }
    /// The words as the game runs them (`FightHooks::jagar`'s default),
    /// then shown.
    fn jagar(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, text: u16) -> Option<()> {
        let dac = self.0.dac;
        m.vision(files, &mut arena::vision::ClickThrough::default(), dac.as_ref(), text, 0x80A5, 0x8090)?;
        // The click that ended the pages let go, as the mouse driver
        // reports it at the frame's next read (the window clicks them
        // through whole).
        let (x, y) = (m.mem.r16(arena::mem::DS, arena::controls::var::MOUSE_X), m.mem.r16(arena::mem::DS, arena::controls::var::MOUSE_Y));
        m.set_mouse(x, y, 0);
        self.0.movie("JAGAR.FLC", Some(text), 0xF5);
        Some(())
    }
}

/// The clock's: exhaustion's boxes, the level-up screen (its points spent
/// on the attribute screen, `LevelUpHooks`), the quests' deadlines and
/// ambushes with their texts.
impl arena::clock::ClockHooks for Front<'_> {
    /// A quest target's words on giving up to a spell's lingering effect,
    /// the game's box (1194:59C8).
    fn surrender_box(&mut self, m: &mut arena::machine::Machine, text: u16) {
        Front::message_box(self, m, text);
    }
    /// 1194:887A with the fight's hooks the front end's (`FightFront`).
    fn encounter(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.random_encounter(files, &mut FightFront(self))
    }
    fn exhaustion_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    fn level_up_screen(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, levels: u16) -> Option<()> {
        arena::levelup::level_up_around(m, files, levels, self)
    }
    /// 1194:99F8: the quests' deadlines by the day, their texts in boxes
    /// (`questui::QuestFront`).
    fn quests_daily(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quests_daily(&mut questui::QuestFront(self, files.clone()));
        Some(())
    }
    /// 1194:98E8: the quests' deadlines by the hour, likewise.
    fn quests_hourly(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quests_hourly(files, &mut questui::QuestFront(self, files.clone()));
        Some(())
    }
    /// 1194:86F2: a quest's ambush, its text shown (the fight's
    /// graphics loaded as `NoFightHooks` loads them, its music the
    /// view's own).
    fn encounter_minute(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        let fs = files.clone();
        m.quest_ambush(files, &mut arena::encounter::NoFightHooks, &mut questui::QuestFront(self, fs))
    }
}
impl arena::levelup::LevelUpHooks for Front<'_> {
    /// The colours A611 reads: the screen's.
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        self.dac
    }
    fn message_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    /// 1194:122C: the points spent on the attribute screen (`dialog`'s
    /// question, answered with the clicks), or in turn without one.
    fn spend(&mut self, m: &mut arena::machine::Machine) {
        match self.talk.as_deref_mut() {
            Some(t) => t.spend(m),
            None => arena::levelup::spend_in_turn(m),
        }
    }
}
/// The automap's notes: the text at 1044 edited in place on the map by
/// the game's editor (0000:32A5, `arena::lineedit::fields::note`; the
/// window's `field::Live` while it is typed), in FONT_S.DAT (which the
/// view loads for the screen) and colour 6, the rest drawn by the
/// screen's own pass.
impl arena::mapnotes::NoteHooks for Front<'_> {
    fn type_text(&mut self, m: &mut arena::machine::Machine, x: u16, y: u16, cursor: u16, max: u16) {
        if let Some(t) = self.talk.as_deref_mut() {
            let field = arena::lineedit::fields::note(m, x, y, cursor, max);
            let look = field::Look { font: "FONT_S.DAT", colour: 6, put_font: false };
            t.edit(m, field, look, dialog::Prompt::Bare);
        }
    }
}
impl arena::frame::FrameHooks for Front<'_> {
    /// 2542:0100, the menu keys, where the frame body asks them (0000:4952:
    /// after the frame's actors, drawing and physics, before the rest of
    /// its body): what only changes the game's state done there (the
    /// weapon readied, picking armed, a touch spell cast, the compass),
    /// camping's in place (2542:01E6); a screen opens inside the frame
    /// (`taken`).
    fn menu_keys(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        use arena::menukeys::{MenuKey, NoMenuKeyHooks};
        let key = m.menu_keys(files, &mut NoMenuKeyHooks).unwrap_or(MenuKey::None);
        let taken = match key {
            MenuKey::None | MenuKey::Ready | MenuKey::Pickpocket | MenuKey::TouchCast | MenuKey::Compass => None,
            MenuKey::Camp => {
                self.camp(m, files);
                None
            }
            MenuKey::Options => Some(sheets::Taken::Options),
            k => sheets::menu_action(m, k).map(sheets::Taken::Action),
        };
        self.taken(m, files, taken);
        Some(())
    }
    fn interface(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.interface_setup(files, self)
    }
    /// 0000:4B84: 0390 with the view's `LightState`, its loads made in it
    /// (`LightNow`).
    fn outdoor_light(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if let Some(light) = self.light.as_deref_mut() {
            m.outdoor_light(&mut LightNow { light, files });
        }
    }
    /// 0000:4C7E, 2294:0428: a new day's colours loaded.
    fn sky_colours(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if let Some(name) = m.sky_colours() {
            if m.sky_colours_load(files, name).is_none() {
                eprintln!("the sky's colours did not load");
            }
        }
    }
    /// 0000:5011: an effect's words counted down; drawn at 0Bh above the
    /// view's foot (C3A2) while they last.
    fn effect_words(&mut self, m: &mut arena::machine::Machine) {
        self.effect = m.effect_words().map(|t| (t, m.mem.r16(arena::mem::DS, 0xC3A2) as i16 as i32 - 0xB));
    }
    /// 0000:3C4C's load: the hands' graphic for the weapon class into
    /// their bank (`Machine::weapon_graphics_load`; the view draws the
    /// hands from its own copy of the files).
    fn weapon_graphics(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        if m.weapon_graphics_load(files).is_none() {
            eprintln!("the hands' graphic did not load: {}", arena::giveup::describe());
        }
        Some(())
    }
    /// 292B:0530: the faces drawn (none in play: FACES stays FFh), bank
    /// 19h mapped.
    fn faces(&mut self, m: &mut arena::machine::Machine) {
        m.faces_draw();
    }
    /// 292B:000E: no projectile flying, their lights forgotten.
    fn projectile_lights_end(&mut self, m: &mut arena::machine::Machine) {
        m.projectile_lights_end();
    }
    /// 0000:17F9: the townspeople's graphics loaded, as they come back
    /// (`Machine::townsfolk_load`).
    fn townsfolk_graphics(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.townsfolk_load(files)
    }
    /// 1194:59C8: the text at DGROUP `text` in a box (drowning in the
    /// wild, 7927).
    fn message_box(&mut self, m: &mut arena::machine::Machine, text: u16) {
        Front::message_box(self, m, text);
    }
    /// 1194:A36C with AX 0: text number [137A] shown in its box (a
    /// building's words on the way in, 0000:4903; a holiday's; a quest's).
    fn message_box_number(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if m.text(files, self, 0, arena::mem::DS, 0).is_none() {
            eprintln!("the text gave up: {}", arena::giveup::describe());
        }
    }
    /// 0000:4ACD's welcome: ENTERCTY.$$$ (named at 130C, the arrival's
    /// text, `Machine::travel_arrival`) loaded to 1044 and shown in its
    /// box (`Machine::town_welcome_box`), what the box covers kept at the
    /// frame's start (0000:4B18) as the original keeps it: the page then,
    /// the screen last shown (the cursor, which the original takes off
    /// the page between frames, is never on the view's).
    fn town_welcome(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if m.town_welcome_box(files, self.shown).is_some() {
            // What it kept, as the box's wait for a key has it (the
            // frame's interface set-up after loads the condition's
            // picture over it, 4DD6 with D39 bit 80h).
            #[cfg(test)]
            {
                let frame = m.mem.r16(arena::mem::DS, 0x9030);
                WELCOME_KEPT.with(|k| *k.borrow_mut() = Some(m.mem.segment(frame).to_vec()));
                WELCOME_BOX.with(|k| *k.borrow_mut() = Some(m.mem.segment(arena::mem::DS).to_vec()));
            }
            self.show(m, 0x1044);
        }
    }
    /// 1194:DB6C with the view's own talk hooks, on the page as the frame
    /// body leaves it by then: the hands (0000:38CE, from 493A) and the
    /// compass (4D7C, from 4957) drawn, which the boxes and lists a click
    /// opens cover.
    fn actor_click(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        if let Some(page) = game_page(m) {
            self.drawn = Some(page);
        } else if let (Some(i), Some(page)) = (self.interface.as_deref_mut(), self.drawn.as_mut()) {
            i.frame_body_on(page, m);
        }
        m.actor_click(files, self)
    }
    /// 0000:0511: the message line drawn on the frame's page (`drawn`),
    /// where the boxes and lists a click opens find it.
    fn message_draw(&mut self, m: &mut arena::machine::Machine) {
        if game_page(m).is_none() {
            if let (Some(i), Some(page)) = (self.interface.as_deref_mut(), self.drawn.as_mut()) {
                i.message_on(page, m);
            }
        }
        m.message_line_draw();
        if let Some(page) = game_page(m) {
            self.drawn = Some(page);
        }
    }
    /// 1194:4635: a frame of rest, while resting.
    fn rest(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.rest_frame(files, self)
    }
    /// 0000:05B5: the lines drawn on the game's page
    /// (`Machine::level_text_lines`; the window's are the layer's,
    /// `hud::level_text`); what the drawing leaves is the row of the last
    /// (9E0A's [ADB8]), which the riddle's field is put under.
    fn level_text_draw(&mut self, m: &mut arena::machine::Machine) {
        m.level_text_lines();
    }
    /// 292B:05A2: the answer typed under the riddle's text (1044 cleared,
    /// 0000:9EE5 at 32h, [ADB8] + 0Ah, in the frame's font [102C] and
    /// colour F0h), then looked for among the riddle's answers.
    fn riddle(&mut self, m: &mut arena::machine::Machine) {
        m.mem.w8(arena::mem::DS, arena::riddle::var::TYPED, 0);
        if let Some(t) = self.talk.as_deref_mut() {
            let look = field::Look { font: "ARENAFNT.DAT", colour: 0xF0, put_font: true };
            t.edit(m, arena::lineedit::fields::riddle(m), look, dialog::Prompt::Bare);
        }
        if m.riddle_answered().is_none() {
            eprintln!("the riddle gave up: {}", arena::giveup::describe());
        }
    }
}
/// The game's texts (1194:A374, made by the port) in their windows: the
/// boxes and menus of `dialog`.
impl arena::text::TextHooks for Front<'_> {
    fn message_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    fn menu(&mut self, m: &mut arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        Front::menu(self, m, si, bx, di)
    }
}
/// Clicking on people and things: the way told is a note; the lists
/// ("Where is", kind 9) asked through `dialog`.
impl arena::talk::TalkHooks for Front<'_> {
    /// A quest target's words on giving up to a touch spell, the game's
    /// box (1194:59C8).
    fn surrender_box(&mut self, m: &mut arena::machine::Machine, text: u16) {
        Front::message_box(self, m, text);
    }
    /// The pick asked through `dialog`; the game's state round it
    /// (`Machine::list_window`: the screen kept in the load bank, the
    /// picture, the window's record) where the page is the game's own
    /// block.
    fn list(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, si: u16, cx: u16, di: u16) -> u16 {
        let (pick, reads) = match self.talk.as_deref_mut() {
            Some(t) => (t.list(m, si, cx, ""), t.list_reads.take()),
            None => return 0xFFFF,
        };
        if !m.page_is_block() {
            return pick;
        }
        m.list_reads = reads;
        // A list asked: the copy of the game as its loop starts, for the
        // window to follow the loop (`list_followed`).
        m.list_capture = self.talk.as_deref().is_some_and(|t| t.need.is_some());
        // The page as it stands (the screen a box covers: `box_screen`) is
        // the game's page block, which the list keeps in the load bank.
        let page = m.mem.r16(arena::mem::DS, 0xA900);
        if let Some(s) = self.box_screen().map(|s| s[..0xFA00].to_vec()) {
            for (i, b) in s.into_iter().enumerate() {
                m.mem.w8(page, i as u16, b);
            }
        }
        let kind = m.list_kind;
        let seg = m.list_segment(kind);
        let ax = match m.list_window(files, kind, (seg, si), cx, di, pick) {
            Some(ax) => ax,
            None => {
                eprintln!("the list's state gave up: {}", arena::giveup::describe());
                pick
            }
        };
        // The page as the list leaves it (its picture drawn, the lines'
        // box not put back: 5105's flags), which what follows covers.
        self.drawn = Some((0..0xFA00u16).map(|i| m.mem.r8(page, i)).collect());
        ax
    }
    /// 23F1:00A3: the place's name noted on the automap at its square,
    /// as the original notes the way told (and a place looked at).
    /// 217B:0682: the way in the wild, told in the game's box.
    fn wild_way(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, kind: u16) -> Option<()> {
        m.wild_way(files, self, kind)
    }
    fn way(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, si: u16, x: u16, y: u16) {
        if m.note_add(files, x, y, si).is_none() {
            eprintln!("the note gave up: {}", arena::giveup::describe());
        }
    }
    /// The loot window (2433:0583) with its boxes and lists answered
    /// through `dialog`.
    fn loot(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, ax: u16, si: u16, seg: u16, bx: u16) -> Option<()> {
        m.loot_window(files, &mut LootFront(self, files.clone()), ax, si, seg, bx)
    }
    /// 1194:D593: the quest's person speaks (`questui::QuestFront`).
    fn quest_person(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quest_person_talk(files, &mut questui::QuestFront(self, files.clone()))
    }
    /// 1194:8F1E (from DCE1): the palace's quests.
    fn palace(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quests_talk(files, &mut questui::QuestFront(self, files.clone()))
    }
    /// 1194:9F04: a level's quest item taken, its texts shown
    /// (`questui::QuestFront`).
    fn quest_item(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, _si: u16) -> bool {
        m.quest_item_taken(files, &mut questui::QuestFront(self, files.clone())).unwrap_or(true)
    }
    /// 2294:0F33: the main quest's chest, its texts shown and, at the
    /// last piece, the finale's level-up screen.
    fn quest_chest(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, si: u16) -> bool {
        m.quest_chest(files, &mut questui::QuestFront(self, files.clone()), si).unwrap_or(false)
    }
    /// A shop entered: its questions through `dialog` (`shopui`).
    fn shop(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, kind: u8) -> Option<()> {
        shopui::shop(self, m, files, kind)
    }
    /// A dungeon's keeper in the wilderness: the view cleared (0000:4EFD),
    /// then in (08EC).
    fn dungeon_entrance(&mut self, m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        m.view_clear();
        m.wild_dungeon_entry();
        Some(())
    }
}
/// The loot window's boxes and lists (1194:59CC, 5734), through the
/// front end's `dialog`; `Files` a copy for its texts.
struct LootFront<'a, 'b>(&'a mut Front<'b>, arena::bsa::Files);

impl arena::loot_window::LootHooks for LootFront<'_, '_> {
    fn message_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        self.0.message_box(m, si);
    }
    /// 1194:A36C: text number [137A] shown (a staff piece's words, 2433:0C58)
    /// or written (mode 2) to 1044.
    fn text(&mut self, m: &mut arena::machine::Machine, mode: u16) {
        if m.text(&mut self.1, &mut *self.0, mode, arena::mem::DS, 0x1044).is_none() {
            eprintln!("the text gave up: {}", arena::giveup::describe());
        }
    }
    /// 1194:5734 with kind Bh: the pick through `dialog` and the game's
    /// state round it (`Machine::list_window`), as the talk's lists.
    fn list(&mut self, m: &mut arena::machine::Machine, si: u16, cx: u16, di: u16) -> u16 {
        arena::talk::TalkHooks::list(&mut *self.0, m, &mut self.1, si, cx, di)
    }
    /// 1194:D986: the eighth piece, the staff made whole
    /// (`Machine::staff_end`): its scenes through `EndFront`.
    fn last_piece(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.staff_end(files, &mut EndFront(self.0))
    }
}
/// The staff made whole's scenes (`arena::staffend`): WALK.FLC and
/// AFLC2.FLC, NUJAGDTH.FLC with text 57Ch in 64h, STAFF.FLC and
/// KING.FLC, NUKING.FLC with 57Dh and then 57Fh in 99h, then the house
/// (HOUSE.IMG in its own colours, text 579h in 5Ch), as movies.
struct EndFront<'a, 'b>(&'a mut Front<'b>);

impl EndFront<'_, '_> {
    /// The movie named at DS:`si`, as the view's names are kept.
    fn named(m: &arena::machine::Machine, si: u16) -> Option<&'static str> {
        let name = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, si)).to_ascii_uppercase();
        ["WALK.FLC", "AFLC2.FLC", "NUJAGDTH.FLC", "STAFF.FLC", "KING.FLC", "NUKING.FLC"].into_iter().find(|n| *n == name)
    }
}

impl arena::staffend::EndHooks for EndFront<'_, '_> {
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        self.0.dac
    }
    /// Each scene as the game runs it (`EndHooks`' defaults), then shown.
    fn movie(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, si: u16) -> Option<()> {
        let name = Self::named(m, si);
        m.movie_play(files, &mut arena::title::NoTitleHooks, si)?;
        if let Some(name) = name {
            self.0.movie(name, None, 0);
        }
        Some(())
    }
    fn speech(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, ax: u16, si: u16, di: u16) -> Option<()> {
        let (name, colour) = (Self::named(m, di), m.mem.r8(arena::mem::DS, 0x9288));
        m.end_speech_play(files, ax, si, di)?;
        if let Some(name) = name {
            self.0.movie(name, Some(ax), colour);
        }
        Some(())
    }
    fn house(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.end_house(files)?;
        self.0.movie("HOUSE.IMG", Some(0x579), 0x5C);
        Some(())
    }
}
/// Resting (src/rest.rs): its menu and the hours typed through `dialog`,
/// its texts boxes, the hours shown kept for the status line.
impl arena::rest::RestHooks for Front<'_> {
    fn menu(&mut self, m: &mut arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        Front::menu(self, m, si, bx, di)
    }
    /// 1194:46BF's 4800: the prompt's box, the field at FCh, 44h in
    /// ARENAFNT.DAT ([102C], which 46BF selected) and colour 60h. The
    /// box's state as the game has it round the field
    /// (`Machine::prompt_shown`, `prompt_closed`).
    fn type_hours(&mut self, m: &mut arena::machine::Machine) {
        if let Some(t) = self.talk.as_deref_mut() {
            use arena::mem::DS;
            let typed = arena::rest::var::TYPED;
            if m.prompt_shown((DS, REST_PROMPT), (0xFC, 0x44), (DS, typed), 3, self.drawn.as_deref().or(self.shown)).is_none() {
                eprintln!("the prompt's box gave up: {}", arena::giveup::describe());
                return;
            }
            let prompt = String::from_utf8_lossy(m.mem.cstr(DS, REST_PROMPT)).to_string();
            let look = field::Look { font: "ARENAFNT.DAT", colour: 0x60, put_font: true };
            t.edit(m, arena::lineedit::fields::rest(), look, dialog::Prompt::Boxed(prompt, m.mem.r8(DS, 0x9288), game_clip(m)));
            m.prompt_closed();
        }
    }
    fn message(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    /// 1194:4659: drawn on the game's page (`Machine::rest_hours_draw`),
    /// and kept for the status line.
    fn hours_text(&mut self, m: &mut arena::machine::Machine) {
        m.rest_hours_draw();
        let s = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, arena::rest::var::TEXT)).into_owned();
        match self.talk.as_deref_mut() {
            Some(t) => t.status = Some(s),
            None => println!("{s}"),
        }
    }
    /// The colours A611 and D45D read: the screen's.
    fn dac(&mut self) -> Option<[[u8; 3]; 256]> {
        self.dac
    }
    /// 1194:D3AF: the first level's dream (text 5DCh) on CHAOSVSN.FLC
    /// (DI 4E16h), the others (50Eh + F77) on VISION.FLC (DI 8085h), each
    /// in the colour the port set at 9288 (33h, then 2Fh), shown as the
    /// window's movie; the game's D45D run on the game (`Machine::vision`,
    /// its pages clicked through).
    fn dream(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, ax: u16, si: u16, di: u16) -> Option<()> {
        let flc = if ax == 0x5DC { "CHAOSVSN.FLC" } else { "VISION.FLC" };
        self.movie(flc, Some(ax), m.mem.r8(arena::mem::DS, arena::rest::var::COLOUR));
        m.vision(files, &mut arena::vision::ClickThrough::default(), self.dac.as_ref(), ax, si, di)
    }
}

/// The 3D view's rows on the page, above the interface's bar (the clip's
/// bottom, A91A, 92h).
const VIEW_ROWS: usize = 0x92;
/// "How many hours do you wish to rest?" (DGROUP), which 1194:4800 shows.
const REST_PROMPT: u16 = 0x4984;
impl EngineHooks for Front<'_> {
    /// 0000:CF96: the creatures' flats the page's drawing reached, brought
    /// to life where the drawing reaches them (`flats_brought`; a flat
    /// that gives up is printed and the frame goes on, as before).
    fn creature_flats(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, reached: &[arena::render::Creature]) -> Option<()> {
        let _ = flats_brought(m, files, self, reached);
        Some(())
    }
    /// The hook CD1A (2294:0A5D) in a level with fog (CB5C): its first
    /// step, the fog's (2294:0B01), on the game's memory with the files
    /// (FOG.LGT, the light table made: `LightState`), as the drawing's
    /// stage A33E runs it. Outdoors the frame's lights run it
    /// (`Assets::lights`).
    fn sky(&mut self, m: &mut arena::machine::Machine) {
        if m.mem.r8(arena::mem::DS, arena::weather::var::LEVEL_FOG) != 0 {
            if let Some(light) = self.light.as_deref_mut() {
                m.fog_step(light);
            }
        }
    }
    /// The page the frame drew: the 3D view's rows (above the interface)
    /// and the row under them as the drawing leaves it (0000:4CC0's clear
    /// comes after the frame body's screens and clicks) over the screen
    /// last shown, as the original's page holds them from then in the
    /// frame (its interface; none of the view's HUD).
    fn drawn(&mut self, m: &mut arena::machine::Machine, page: &[u8]) {
        // The game's own page holds the original's (`page_view_put` has
        // just put the view there, over its interface).
        if let Some(p) = game_page(m) {
            self.drawn = Some(p);
            return;
        }
        if let Some(shown) = self.shown {
            let mut s = shown.to_vec();
            let n = ((VIEW_ROWS + 1) * 320).min(s.len()).min(page.len());
            s[..n].copy_from_slice(&page[..n]);
            self.drawn = Some(s);
        }
    }
    /// Only with the view's own GPU world (no engine page: --gpu-world),
    /// a stand-in for what the original's walk leaves the game; with the
    /// page the engine takes the drawing's own (`render::Walked`): the
    /// changed squares within the view's reach (C394 squares) step, seen
    /// or not.
    fn draw(&mut self, m: &mut arena::machine::Machine) {
        use arena::click::var::{DOOR_AT, DOOR_COL, DOOR_ROW, DOOR_X0, DOOR_X1};
        use arena::mem::DS;
        use arena::movement::var::LEVEL_FLAGS;
        // The projectiles' animations, which the original's drawing steps
        // (0000:C23A): here every one in flight steps, seen or not.
        for i in 0..arena::projectile::var::NSLOTS {
            let si = arena::projectile::var::SLOTS + i * arena::projectile::var::SLOT_SIZE;
            if m.mem.r8(DS, si + 0x11) != 0 {
                m.drawn_step(si);
            }
        }
        // The click left for the actors (C39A bit 80h, set by the input
        // step's click, `take_click`, which the next input step clears; or
        // a click made before the frame, `view_click` between turns, whose
        // input step clears it first) stands while it is drawn, as in the
        // original: the sprite under it becomes CB5E (0000:C4FA), made ready
        // for 1194:DB6C.
        let left = m.mem.r16(DS, LEVEL_FLAGS) & 0x80 != 0;
        if let (true, Some(sheet)) = (std::mem::take(&mut self.clicked) || left, self.sheet) {
            let f = m.mem.r16(DS, LEVEL_FLAGS) | 0x80;
            m.mem.w16(DS, LEVEL_FLAGS, f);
            let v = arena::engine::View::of(m);
            let eye = (v.x as f32, v.y as f32, v.height as f32 + v.bob as f32);
            // On a hit the drawing calls the hook at CD3A (0000:C524), which
            // the 3D mode sets to 2294:047B (`clicked_flat`).
            let hit = match world::hit(m, sheet, eye, v.heading as f32, self.tick, world::click_ray(m, self.aspect)) {
                Some(world::Target::Slot(si)) => Some(si),
                Some(world::Target::Flat { col, row, n }) => {
                    world::flat_drawn(m, col, row, n);
                    Some(arena::talk::var::FLAT_DRAWN)
                }
                None => None,
            };
            if let Some(si) = hit {
                m.mem.w16(DS, arena::talk::var::CLICKED, si);
                m.clicked_flat();
            }
        }
        // What a click in the centre would meet (the original's renderer
        // left the square and, for a door, its span under the cursor).
        if let Some(sq) = world::pick(m, m.mem.r16(DS, 0xC394)) {
            m.mem.w16(DS, arena::controls::var::PICKED, sq);
            let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
            if m.mem.r16(walls, sq) >> 12 == 0xB {
                m.mem.w16(DS, DOOR_X0, 0);
                m.mem.w16(DS, DOOR_X1, 319);
                m.mem.w16(DS, DOOR_COL, (sq & 0xFF) >> 1);
                m.mem.w16(DS, DOOR_ROW, sq >> 8);
                m.mem.w16(DS, DOOR_AT, sq);
            }
        }
        let reach = m.mem.r16(DS, 0xC394) as i32;
        let (col, row) = (m.mem.r16(DS, 0xCCA4) as i32, m.mem.r16(DS, 0xCCA6) as i32);
        for i in 0..0x20 {
            let sq = m.mem.r16(DS, arena::click::var::TEMP_WALLS + 2 * i);
            if sq == 0xFFFF {
                continue;
            }
            let (c, r) = ((sq & 0xFF) as i32 >> 1, (sq >> 8) as i32);
            if (c - col).abs() <= reach && (r - row).abs() <= reach {
                m.temp_walls_begin_draw();
                m.temp_wall_drawn(sq);
            }
        }
    }
}

fn camera(engine: &Engine) -> Camera {
    let v = engine.view();
    Camera { x: v.x as f32, y: v.y as f32, height: v.height as f32 + v.bob as f32, heading: v.heading as f32 }
}

/// What drawing a level needs, read once from the engine after loading.
struct Assets {
    /// The lightning's flash colour of the last game frame (the sky's hook
    /// drawing it), for the sky.
    flash: std::cell::Cell<Option<u8>>,
    /// What the last game frame left to draw (`view_frame`): the sky, the
    /// lights and the sky's objects. Redraws between frames reuse it, so
    /// the sky's hook (lightning) steps once a frame, as in the original.
    frame_view: Option<(render::Sky, render::Lights, Vec<world::Vertex>)>,
    /// The moons' phases' frames (0000:E6CB), kept until the day changes.
    moons: [usize; 2],
    palette: [[u8; 4]; 256],
    /// CHARSHT.COL: the colours the character sheet's screens (the
    /// equipment screen, the spellbook) are shown in; 1194:1695 loads it
    /// and sets the whole palette from it.
    charsheet: [[u8; 4]; 256],
    textures: Vec<[u8; 4096]>,
    shading: Vec<u8>,
    mesh: Vec<world::Vertex>,
    sheet: world::SpriteSheet,
    archive: Rc<archive::Archive>,
    /// DITHER.IMG then DITHER2.IMG.
    dither: Vec<u8>,
    /// The sky's .COL as the game keeps it: its name's offset and its
    /// colours, the first eight again past the end.
    colours: Option<(u16, Vec<u8>)>,
    /// What the day and night switch and the fog left.
    light: LightState,
    /// The player's interface (`interface`).
    interface: interface::Interface,
    /// The screen last composed (`App::compose`), a byte a pixel: the page
    /// as the original holds it at a frame's start, which a box keeps what
    /// it covers from (`Front::town_welcome`).
    shown: Vec<u8>,
}

/// The sky's objects drawn this frame (2294:0A5D's draws), as quads.
/// `fog` is this frame's fog step's answer (`LightState::fog`), already
/// run with the drawing's lights.
struct SkyDraw<'a> {
    sheet: &'a world::SpriteSheet,
    images: &'a HashMap<u16, String>,
    focal: f32,
    fog: bool,
    out: Vec<world::Vertex>,
    /// The lightning's flash: the colour the sky above the horizon is
    /// filled with (2294:05A6, its colour at 9288).
    flash: Option<u8>,
}

impl SkyDraw<'_> {
    fn quad(&mut self, m: &arena::machine::Machine, name: Option<&String>, frame: usize, x: u16, y: u16) {
        use arena::mem::DS;
        let Some(f) = name.and_then(|n| self.sheet.sky.get(n)) else { return };
        let rect = self.sheet.rects[f.first + frame % f.count];
        let r = |at| m.mem.r16(DS, at) as i16;
        world::sky_quad(&mut self.out, rect, x as i16, y as i16, r(0xC3D6), r(0xC3D8), r(0xC3A2), self.focal);
    }
}

impl arena::skyobj::SkyHooks for SkyDraw<'_> {
    fn fog(&mut self, _m: &mut arena::machine::Machine) -> bool {
        self.fog
    }
    fn draw_image(&mut self, m: &mut arena::machine::Machine, seg: u16, off: u16) {
        let name = if seg == m.mem.r16(arena::mem::DS, 0x9030) { self.images.get(&off) } else { None };
        let name = name.cloned().or_else(|| (m.mem.r8(arena::mem::DS, 0x7BD2) == 3).then(|| "VOLCANO3.IMG".to_string()));
        self.quad(m, name.as_ref(), 0, m.mem.r16(seg, off), m.mem.r16(seg, off + 2));
    }
    fn draw_dfa(&mut self, m: &mut arena::machine::Machine, _seg: u16, _off: u16, frame: u16, x: u16, y: u16) {
        self.quad(m, Some(&"VOLCANO2.DFA".to_string()), frame as usize, x, y);
    }
    /// The flash: the whole sky above the horizon (the view fills the sky
    /// with it, as fog does).
    fn fill(&mut self, m: &mut arena::machine::Machine, _x0: u16, _y0: u16, _x1: u16, _y1: u16) {
        self.flash = Some(m.mem.r8(arena::mem::DS, arena::skyobj::var::FILL));
    }
    /// A bolt (0000:78A8): which of the six is told by its offset in the
    /// sky's bank (the table at D8AE, `sky_images`), drawn at its header's
    /// place (+6, +8, set by the lightning).
    fn bolt(&mut self, m: &mut arena::machine::Machine, seg: u16, off: u16, frame: u16) {
        let i = (0..6u16).find(|&i| m.mem.r16(arena::mem::DS, arena::skyobj::var::BOLTS + 4 * i) == off);
        if let Some(i) = i {
            let name = format!("LGLIT0{}.CFA", i + 1);
            self.quad(m, Some(&name), frame as usize, m.mem.r16(seg, off + 6), m.mem.r16(seg, off + 8));
        }
    }
}

/// The sky's bodies drawn this frame (0000:E41A's draws), as quads: the
/// sun's and the moons' pictures (`moons`, their phases' frames, kept
/// across frames as the original keeps the decoded pictures), the stars'
/// images and pixels.
struct CelestialDraw<'a> {
    sheet: &'a world::SpriteSheet,
    focal: f32,
    moons: &'a mut [usize; 2],
    out: Vec<world::Vertex>,
}

impl CelestialDraw<'_> {
    fn quad(&mut self, m: &arena::machine::Machine, rect: world::Rect, x: u16, y: u16, flags: u32) {
        use arena::mem::DS;
        let r = |at| m.mem.r16(DS, at) as i16;
        world::sky_quad_flags(&mut self.out, rect, x as i16, y as i16, r(0xC3D6), r(0xC3D8), r(0xC3A2), self.focal, flags);
    }

    fn named(&mut self, m: &arena::machine::Machine, name: &str, frame: usize, x: u16, y: u16, flags: u32) {
        let Some(f) = self.sheet.sky.get(name).copied() else { return };
        let rect = self.sheet.rects[f.first + frame % f.count];
        self.quad(m, rect, x, y, flags);
    }
}

impl arena::celestial::CelestialHooks for CelestialDraw<'_> {
    fn moon_phase(&mut self, _m: &mut arena::machine::Machine, name: u16, frame: u16, _dest: u16) {
        let which = if name == 0xD871 { 0 } else { 1 };
        self.moons[which] = frame as usize;
    }
    fn draw(&mut self, m: &mut arena::machine::Machine, si: u16, x: u16, y: u16, _w: u16, _h: u16) {
        use arena::celestial::var::{MOON1, MOON2, SUN};
        let (name, frame) = match si {
            SUN => ("SUN.IMG", 0),
            MOON1 => ("MOON01.DFA", self.moons[0]),
            MOON2 => ("MOON02.DFA", self.moons[1]),
            _ => return,
        };
        self.named(m, name, frame, x, y, world::BODY);
    }
    fn star_pixel(&mut self, m: &mut arena::machine::Machine, x: u16, y: u16, colour: u16) {
        let Some(f) = self.sheet.sky.get(world::STAR_PIXELS).copied() else { return };
        if !(0x40..0x50).contains(&colour) {
            return;
        }
        let strip = self.sheet.rects[f.first];
        let rect = world::Rect { x: strip.x + (colour - 0x40) as u32, w: 1, ..strip };
        self.quad(m, rect, x, y, world::BODY | world::DARK_SKY);
    }
    fn star_image(&mut self, m: &mut arena::machine::Machine, x: u16, y: u16, image: u16) {
        use arena::celestial::var::STAR_IMAGES;
        let Some(i) = (0..8u16).find(|&i| m.mem.r16(arena::mem::DS, STAR_IMAGES + 2 * i) == image) else { return };
        self.named(m, &format!("STAR{}.IMG", i + 1), 0, x, y, world::BODY | world::DARK_SKY);
    }
}

impl Assets {
    /// The colours on the screen as the original's DAC holds them: PAL.COL
    /// but, after a frame outdoors, entries 1..8 the sky's colours for the
    /// time of day (2294:0341) and, once the game has set it, 71h the
    /// lamps' (0000:9B5B): anything drawn in those colours shows them.
    fn dac(&self) -> [[u8; 4]; 256] {
        Self::dac_with(self.palette, self.frame_view.as_ref().map(|v| &v.0))
    }

    fn dac_with(palette: [[u8; 4]; 256], sky: Option<&render::Sky>) -> [[u8; 4]; 256] {
        let mut p = palette;
        if let Some(sky) = sky {
            if sky.outdoors {
                for (i, c) in sky.colours.iter().enumerate() {
                    p[1 + i] = [c[0], c[1], c[2], 255];
                }
            }
            if let Some(l) = sky.lamp {
                p[0x71] = [l[0], l[1], l[2], 255];
            }
        }
        p
    }

    /// The colours a game turn finds: `dac`, but before the area's first
    /// frame has drawn, those the original's DAC holds in that frame: the
    /// sky's colours from the window the frame's 2294:0341 sets (from the
    /// sky's time, CCFE; it runs before the minute's tick, whose encounter
    /// may read them, 1194:D91C) and 71h as the light state last set it.
    fn dac_turn(&mut self, m: &mut arena::machine::Machine) -> [[u8; 4]; 256] {
        if self.frame_view.is_some() {
            return self.dac();
        }
        let mut c = m.clone();
        c.sky_step();
        let sky = self.sky(&mut c);
        Self::dac_with(self.palette, Some(&sky))
    }

    /// Also enters the area's sky (0000:464A: its objects made).
    fn new(dir: &std::path::Path, engine: &mut Engine) -> Self {
        prof::time("assets.new", || Self::make(dir, engine))
    }

    /// The area's entry's loads made by the view, where no entry made
    /// them (`Assets::make`): outdoors the sky's images (0000:A4E9's
    /// E5C9) and objects (464A), the water's frames (A50B), the interface
    /// (46A2), the day's colours (06B8) and outdoors the light for the
    /// time of day (46AF's 0390), its loads made in it.
    fn entry_loads(dir: &std::path::Path, engine: &mut Engine) -> LightState {
        if prof::time("assets.sky_images", || engine.m.sky_images(&mut engine.files)).is_none() {
            eprintln!("the sky's images did not load");
        }
        SKY_NAMES.with(|n| n.borrow_mut().clear());
        prof::time("assets.area_sky", || engine.m.area_sky(&mut GameSkyLoad { files: &mut engine.files }));
        let mut light = LightState::new(dir, &engine.m);
        if engine.m.water_frames(&mut engine.files, 0xBA17).is_none() {
            eprintln!("the water's frames did not load");
        }
        let _ = engine.m.interface_setup(&mut engine.files, &mut Front::default());
        if let Some(name) = engine.m.sky_colours() {
            if engine.m.sky_colours_load(&mut engine.files, name).is_none() {
                eprintln!("the sky's colours did not load");
            }
        }
        if engine.m.mem.r16(arena::mem::DS, 0xC398) & 4 != 0 {
            engine.m.outdoor_light(&mut LightNow { light: &mut light, files: &mut engine.files });
        }
        light
    }

    fn make(dir: &std::path::Path, engine: &mut Engine) -> Self {
        let flats = prof::time("assets.flats", || world::flats(&engine.m));
        let archive = prof::time("assets.archive", || Rc::new(archive::Archive::open(dir)));
        let sheet = prof::time("assets.sheet", || world::sprite_sheet(&flats, &archive, &world::creature_names(&engine.m)));
        interface::enter(&mut engine.m);
        // The area's entry made its loads where the original makes them
        // (`Areas`: the sky's images, its objects, the water's frames, the
        // interface, the day's colours and 46AF's light) and handed its
        // light over. Without one (a level the view starts in, the assets
        // made again after stairs or the wilderness's blocks), they are
        // made here.
        let light = match ENTRY_LIGHT.with(|l| l.borrow_mut().take()) {
            Some(light) => light,
            None => Self::entry_loads(dir, engine),
        };
        Assets {
            palette: world::palette(dir),
            charsheet: world::palette_file(dir, "CHARSHT.COL"),
            textures: prof::time("assets.textures", || world::textures(&engine.m)),
            shading: world::shading(&engine.m),
            mesh: prof::time("assets.mesh", || world::mesh(&engine.m)),
            sheet,
            dither: ["DITHER.IMG", "DITHER2.IMG"].iter().flat_map(|n| {
                let mut d = archive.read(n).unwrap_or_default();
                d.resize(800, 0);
                d
            }).collect(),
            interface: prof::time("assets.interface", || interface::Interface::new(archive.clone())),
            archive,
            colours: None,
            light,
            flash: std::cell::Cell::new(None),
            frame_view: None,
            moons: [0, 0],
            shown: vec![0; 64000],
        }
    }

    /// The level's own assets made again after a level change (stairs):
    /// the textures, the shading rows, the walls and the flats. The sky
    /// and the light state stay the area's.
    fn level(&mut self, engine: &Engine) {
        let t = Instant::now();
        self.level_make(engine);
        if prof::on() {
            prof::add("assets.level", t.elapsed());
        }
    }

    fn level_make(&mut self, engine: &Engine) {
        let flats = world::flats(&engine.m);
        self.sheet = world::sprite_sheet(&flats, &self.archive, &world::creature_names(&engine.m));
        self.textures = world::textures(&engine.m);
        self.shading = world::shading(&engine.m);
        self.mesh = world::mesh(&engine.m);
    }

    fn scene(&self) -> Scene<'_> {
        Scene { palette: self.palette, textures: &self.textures, shading: &self.shading, mesh: &self.mesh, sprites: &self.sheet, dither: &self.dither }
    }

    fn renderer(&self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Renderer {
        Renderer::new(device, queue, format, &self.scene())
    }

    /// `r` made over for these assets (its pipelines and, where it can,
    /// its sprite atlas kept: `Renderer::reload`).
    fn reload_renderer(&self, r: &mut Renderer, device: &wgpu::Device, queue: &wgpu::Queue) {
        prof::time("renderer.reload", || r.reload(device, queue, &self.scene()));
    }

    /// The sky after a frame (the frame body's 0390 and 2294:0428 made the
    /// light and the colours: `Front::outdoor_light`, `sky_colours`):
    /// outdoors, palette entries 1..8 from the day's colours at the window
    /// for the time of day, the textures darkened by night and the lamps
    /// lit.
    fn sky(&mut self, m: &mut arena::machine::Machine) -> render::Sky {
        use arena::sky::var::{COLOURS, DITHER, DITHER_AFTERNOON, WINDOW};
        let outdoors = m.mem.r16(arena::mem::DS, 0xC398) & 4 != 0;
        if !outdoors {
            // Indoors no sky, but 71h as the game set it (a level's fog
            // greys it: 2294:0C36).
            return render::Sky { lamp: self.light.lamp, ..Default::default() };
        }
        let name = m.mem.r16(arena::mem::DS, COLOURS);
        if self.colours.as_ref().map(|c| c.0) != Some(name) {
            let file = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, name)).into_owned();
            // An 8-byte header, then the colours.
            let mut c = self.archive.read(&file).map(|d| d.get(8..).unwrap_or_default().to_vec()).unwrap_or_default();
            c.resize(0x300, 0);
            let first: Vec<u8> = c[..0x18].to_vec();
            c.extend(first);
            self.colours = Some((name, c));
        }
        let c = &self.colours.as_ref().unwrap().1;
        let w = m.mem.r16(arena::mem::DS, WINDOW) as usize;
        let mut colours = [[0u8; 3]; 8];
        for (i, col) in colours.iter_mut().enumerate() {
            for (k, v) in col.iter_mut().enumerate() {
                *v = *c.get(w + 3 * i + k).unwrap_or(&0);
            }
        }
        render::Sky { outdoors, afternoon: m.mem.r16(arena::mem::DS, DITHER) == DITHER_AFTERNOON, colours, dark: self.light.dark, lamp: self.light.lamp, fog: None }
    }

    /// The sky and the lights after a frame, in the original's order: the
    /// frame's day and night switch (`sky`), the drawing's lights
    /// (`lights`), then the sky's hook, whose fog step may fill the view
    /// in place of the sky.
    fn light_frame(&mut self, m: &mut arena::machine::Machine) -> (render::Sky, render::Lights) {
        let mut sky = self.sky(m);
        let lights = self.lights(m);
        sky.fog = self.light.fog;
        (sky, lights)
    }

    /// The lights after a frame (the drawing's set-up, 0000:A2EC..A303 and
    /// E106): the player's light at the player, the near list made again
    /// if the player moved to another place (`lights_near`), then the
    /// near and the moving lights; outdoors the sky's hook's fog step
    /// (2294:0B01, from 0000:D513), which in fog leaves only the player's
    /// light. None in full daylight (C398 bit 8000h). The view's own work
    /// after the frame, where the original does none: DGROUP is left as the
    /// frame left it (the player's light stays at the eye of the frame's
    /// start, A2EC's, until the next frame's).
    fn lights(&mut self, m: &mut arena::machine::Machine) -> render::Lights {
        use arena::mem::DS;
        // The EMS too (the fog's light table maps banks, 0000:AB2A): the
        // banks and the frame as they were, with the bank DGROUP says is
        // mapped (8FA8), so the next mapping keeps the frame's bytes in
        // their own bank.
        let kept = m.mem.segment(DS).to_vec();
        let frame = m.mem.r16(DS, arena::mif::var::BUFFER_SEG);
        let (banks, framed) = (m.ems.clone(), m.mem.segment(frame).to_vec());
        let lights = self.lights_made(m);
        m.ems = banks;
        m.mem.segment_mut(frame).copy_from_slice(&framed);
        m.mem.segment_mut(DS).copy_from_slice(&kept);
        m.mem.frame_mapped(m.mem.r16(DS, arena::level::var::EMS_MAPPED) >> 2);
        lights
    }

    fn lights_made(&mut self, m: &mut arena::machine::Machine) -> render::Lights {
        use arena::light::var::*;
        use arena::mem::DS;
        let (x, y) = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
        m.mem.w16(DS, NEAR, x);
        m.mem.w16(DS, NEAR + 2, y);
        m.lights_near();
        m.moving_lights();
        self.light.fog = None;
        if m.mem.r16(DS, 0xC398) & 4 != 0 && m.fog_step(&mut self.light) && self.light.fog.is_none() {
            // The fog's first frame: no fill, the background colour.
            self.light.fog = Some(m.mem.r8(DS, arena::weather::var::BACKGROUND));
        }
        if m.mem.r16(DS, 0xC398) & 0x8000 != 0 {
            return render::Lights::default();
        }
        let at = |m: &arena::machine::Machine, list: u16, i: u16| {
            let e = list + 6 * i;
            [m.mem.r16(DS, e), m.mem.r16(DS, e + 2), m.mem.r16(DS, e + 4)]
        };
        let mut lights: Vec<[u16; 3]> = (0..m.mem.r16(DS, NEAR_COUNT).min(64)).map(|i| at(m, NEAR, i)).collect();
        lights.extend((0..m.mem.r16(DS, MOVING_COUNT).min(32)).map(|i| at(m, MOVING, i)));
        lights.truncate(64);
        render::Lights { lights, rows: [m.mem.r16(DS, LIMIT), m.mem.r16(DS, DARK), m.mem.r16(DS, FAR)], behind: if m.mem.r16(DS, 0xC398) & 4 != 0 { 2 } else { 8 } }
    }

    /// This frame's billboards, animations stepping every `ANIMATE` ticks,
    /// and the sky's objects (2294:0A5D) for a view `aspect` wide. After
    /// `light_frame`, whose fog step the sky's hook runs first.
    fn sprites(&self, engine: &mut Engine, cam: Camera, ticks: usize, _aspect: f32) -> Vec<world::Vertex> {
        let mut v = world::sprites(&engine.m, &self.sheet, cam.heading, ticks / ANIMATE);
        v.extend(world::doors(&engine.m));
        if let Some((_, _, sky)) = &self.frame_view {
            v.extend(sky.iter().copied());
        }
        v
    }

    /// The sky's loads its steps asked for (2294:0428's .COL to 43C0,
    /// 2294:0341's dither to CD04), into the game's memory: the original's
    /// renderer draws the sky from them.
    fn sky_loads(&mut self, engine: &mut Engine) {
        if std::mem::take(&mut self.light.restore) && engine.m.water_frames(&mut engine.files, 0xBA17).is_none() {
            eprintln!("the water's frames did not load");
        }
        if let Some(name) = self.light.colours.take() {
            if engine.m.sky_colours_load(&mut engine.files, name).is_none() {
                eprintln!("the sky's colours did not load");
            }
        }
        if let Some(name) = self.light.dither.take() {
            if engine.m.sky_dither_load(&mut engine.files, name).is_none() {
                eprintln!("the sky's dither did not load");
            }
        }
        if std::mem::take(&mut self.light.objects) {
            sky_objects_made(&mut engine.m, &mut engine.files);
        }
        if let Some(level) = self.light.relight.take() {
            engine.m.sky_images_relight(level);
            if engine.m.water_relight(&mut engine.files, level).is_none() {
                eprintln!("the water's frames did not load");
            }
        }
    }

    /// After a game frame: the frame's sky and lights (`light_frame`), then
    /// the sky's hook (2294:0A5D: its objects and the lightning, the flash
    /// filling the sky) for a view `aspect` wide, kept for the redraws
    /// until the next frame.
    fn view_frame(&mut self, engine: &mut Engine, aspect: f32) {
        // The bank the frame left mapped (8FA8: the faces' 19h, 292B:0530),
        // mapped again at the end: this is the view's own work after the
        // frame, where the original maps none.
        let mapped = engine.m.mem.r16(arena::mem::DS, arena::level::var::EMS_MAPPED);
        self.view_frame_drawn(engine, aspect);
        engine.m.ems_map(mapped >> 2);
    }

    fn view_frame_drawn(&mut self, engine: &mut Engine, aspect: f32) {
        let (mut sky, lights) = self.light_frame(&mut engine.m);
        self.sky_loads(engine);
        let focal = world::TAN_HALF * aspect;
        // 0000:D4FA..D50D: outdoors, not in fog (C39A bit 20h), the sky's
        // bodies, then the sky's hook.
        let mut bodies = Vec::new();
        let flags = (engine.m.mem.r16(arena::mem::DS, 0xC398), engine.m.mem.r16(arena::mem::DS, 0xC39A));
        if flags.0 & 4 != 0 && flags.1 & 0x20 == 0 {
            // The moons' frames as the phases last made (PHASE_DAY): the
            // engine makes them before the frame's drawing where it draws
            // the page (`Machine::moon_phases`), so the hook below may not
            // see the day change.
            let day = engine.m.mem.r8(arena::mem::DS, arena::celestial::var::PHASE_DAY) as usize;
            self.moons = [day, (day + 0xE) & 0x1F];
            let mut cd = CelestialDraw { sheet: &self.sheet, focal, moons: &mut self.moons, out: Vec::new() };
            // The view's own pass, not the game's (whose drawing ran E41A
            // on its copy): run on a copy of the machine, so nothing it
            // writes is left. Left, its PHASE_DAY kept the engine's
            // `moon_phases` from making a new day's moons; and its writes,
            // even put back, counted as the machine's since the drawing
            // (`Memory::drawn_written`), which ended the drawing's own
            // there (the moons' places, E042: the last outdoor frame's
            // lost on going indoors).
            let ok = engine.m.clone().celestial(&mut cd).is_some();
            if ok {
                bodies = cd.out;
            }
        }
        let names = SKY_NAMES.with(|n| n.borrow().clone());
        let mut draw = SkyDraw { sheet: &self.sheet, images: &names, focal, fog: self.light.fog.is_some(), out: Vec::new(), flash: None };
        // The view's own pass too (the game's drawing ran 2294:0A5D on its
        // copy, whose DGROUP the pass stands in for: the bolt's steps, the
        // flash): what it writes in EMS is put back as it found it, the
        // bank mapped before mapped back. The drawing's own writes there
        // are the copy's (`Engine::page_ems`); left, the images' x and
        // flags the pass sets stood over the images a relight in the frame
        // body had just read again.
        let (mapped, seg, frame, banks) = {
            let m = &engine.m;
            let seg = m.mem.r16(arena::mem::DS, arena::mif::var::BUFFER_SEG);
            (m.mem.r16(arena::mem::DS, arena::level::var::EMS_MAPPED), seg, m.mem.segment(seg).to_vec(), m.ems.clone())
        };
        // On the EMS the drawing started from (`Engine::page_ems_prior`):
        // the earlier drawings' changes there, the bolt's y 2294:05A6 never
        // puts back, which it reads into DGROUP.
        if engine.page.is_some() {
            arena::render::ems_prior_onto(&mut engine.m, &engine.page_ems_prior);
        }
        let drawn = engine.m.sky_draw(&mut draw).is_some();
        engine.m.ems_map(mapped >> 2);
        engine.m.ems = banks;
        engine.m.mem.segment_mut(seg).copy_from_slice(&frame);
        if drawn {
            bodies.extend(draw.out);
        }
        let quads = bodies;
        self.flash.set(draw.flash);
        if let Some(c) = draw.flash {
            sky.fog = Some(c);
        }
        self.frame_view = Some((sky, lights, quads));
    }

    /// The sky and the lights to draw: the last frame's (`view_frame`),
    /// made now if no frame has run since the assets were made.
    fn drawn(&mut self, engine: &mut Engine, aspect: f32) -> (render::Sky, render::Lights) {
        if self.frame_view.is_none() {
            self.view_frame(engine, aspect);
        }
        let (sky, lights, _) = self.frame_view.as_ref().unwrap();
        (*sky, lights.clone())
    }
}

/// Timer ticks an animation frame lasts.
const ANIMATE: usize = 4;

/// `--mute` (no sound played) and `--sound-log` (each sound printed),
/// for the window and the shots alike.
static MUTE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SOUND_LOG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--no-music`: the sounds without the music (`music`).
static NO_MUSIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// --gpu-world: the 3D view drawn by the view's renderer, not the
/// original's (`Engine::page`).
static GPU_WORLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    MUTE.store(args.iter().any(|a| a == "--mute"), std::sync::atomic::Ordering::Relaxed);
    SOUND_LOG.store(args.iter().any(|a| a == "--sound-log"), std::sync::atomic::Ordering::Relaxed);
    NO_MUSIC.store(args.iter().any(|a| a == "--no-music"), std::sync::atomic::Ordering::Relaxed);
    GPU_WORLD.store(args.iter().any(|a| a == "--gpu-world"), std::sync::atomic::Ordering::Relaxed);
    args.retain(|a| a != "--mute" && a != "--sound-log" && a != "--no-music" && a != "--gpu-world");
    let fight = args.iter().any(|a| a == "--fight");
    let click = args.iter().any(|a| a == "--click");
    let people = args.iter().any(|a| a == "--people");
    let load_at_start = args.iter().any(|a| a == "--load");
    args.retain(|a| a != "--fight" && a != "--click" && a != "--people" && a != "--load");
    let mut wait = 0;
    if let Some(i) = args.iter().position(|a| a == "--wait") {
        wait = args.get(i + 1).and_then(|v| v.parse().ok()).expect("--wait N");
        args.drain(i..i + 2);
    }
    let mut hour = None;
    if let Some(i) = args.iter().position(|a| a == "--hour") {
        hour = Some(args[i + 1].parse::<u32>().expect("--hour H"));
        args.drain(i..i + 2);
    }
    let mut answers = Default::default();
    if let Some(i) = args.iter().position(|a| a == "--picks") {
        answers = dialog::parse_answers(&args[i + 1]);
        args.drain(i..i + 2);
    }
    let mut save_dir = None;
    if let Some(i) = args.iter().position(|a| a == "--save-dir") {
        save_dir = Some(PathBuf::from(&args[i + 1]));
        args.drain(i..i + 2);
    }
    let mut slot = 0u8;
    if let Some(i) = args.iter().position(|a| a == "--slot") {
        slot = args[i + 1].parse().expect("--slot N");
        args.drain(i..i + 2);
    }
    let mut steps = String::new();
    if let Some(i) = args.iter().position(|a| a == "--do") {
        steps = args[i + 1].clone();
        args.drain(i..i + 2);
    }
    // The spellmaker's script for `--do spellmaker` (view/src/spellmaker.rs).
    let mut maker_script = String::new();
    if let Some(i) = args.iter().position(|a| a == "--spellmaker") {
        maker_script = args[i + 1].clone();
        args.drain(i..i + 2);
    }
    let mut new_game = None;
    if let Some(i) = args.iter().position(|a| a == "--new-game") {
        new_game = Some(args.get(i + 1).cloned().unwrap_or_default());
        args.drain(i..(i + 2).min(args.len()));
        // The start dungeon's level when none is given.
        if args.get(2).is_none_or(|a| a.starts_with("--")) {
            args.insert(2.min(args.len()), "START.MIF".into());
        }
    }
    // --create [SCRIPT]: creation in the window, its keys and clicks
    // (`create::parse_script`) given first.
    let mut create_script = None;
    if let Some(i) = args.iter().position(|a| a == "--create") {
        let script = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned();
        args.drain(i..i + 1 + script.is_some() as usize);
        create_script = Some(script.unwrap_or_default());
        if args.get(2).is_none_or(|a| a.starts_with("--")) {
            args.insert(2.min(args.len()), "START.MIF".into());
        }
    }
    // --bench TURNS [SETUP]: the window's turns and redraws timed,
    // headless (`bench`).
    let mut bench = None;
    if let Some(i) = args.iter().position(|a| a == "--bench") {
        let turns: usize = args[i + 1].parse().expect("--bench TURNS [SETUP]");
        let setup = args.get(i + 2).filter(|a| !a.starts_with("--")).cloned();
        args.drain(i..i + 2 + setup.is_some() as usize);
        bench = Some((turns, setup.unwrap_or_default()));
    }
    // --playtest SEED FRAMES [SETUP]: a headless playtest (`playtest`).
    let mut playtest = None;
    if let Some(i) = args.iter().position(|a| a == "--playtest") {
        let seed: u64 = args[i + 1].parse().expect("--playtest SEED FRAMES [SETUP]");
        let frames: usize = args[i + 2].parse().expect("--playtest SEED FRAMES [SETUP]");
        let setup = args.get(i + 3).filter(|a| !a.starts_with("--")).cloned();
        args.drain(i..i + 3 + setup.is_some() as usize);
        playtest = Some((seed, frames, setup.unwrap_or_default()));
    }
    let mut at = None;
    if let Some(i) = args.iter().position(|a| a == "--at") {
        let v: Vec<u16> = args[i + 1..i + 4].iter().map(|v| v.parse().expect("--at COL ROW HEADING")).collect();
        at = Some((v[0], v[1], v[2]));
        args.drain(i..i + 4);
    }
    if args.len() < 3 {
        eprintln!("usage: arena-view GAME_DIR LEVEL.MIF [--fight] [--people] [--click] [--hour H] [--at COL ROW HEADING] [--wait N] [--do STEPS] [--picks N,..] [--save-dir DIR] [--slot N] [--new-game SPEC | --create [SCRIPT]] [--shot OUT.png [FRAMES [TURN]]]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    // random1.mif is the dungeon builder's chunk file, not a level: the
    // original names it once (DGROUP 6540, read only at 0000:29A3), where
    // 299F copies its levels into a dungeon's grid of chunks and walls the
    // map's edges in (`dun_border`). Loaded whole as a level (0000:E87C's
    // planes cleared to 0), a chunk's open edges let the player walk off
    // the map, which the original never meets (a playtest of seed 17
    // there walked off it 44 times).
    if args[2].eq_ignore_ascii_case("RANDOM1.MIF") {
        eprintln!("RANDOM1.MIF is the dungeon builder's chunk file, which the game never plays as a level; a wilderness dungeon is reached through a city's gate (the playtest's PT_FOCUS=dungeon)");
        std::process::exit(2);
    }
    let started = Instant::now();
    // A save loaded as the program starts: the game as the original reaches
    // its title (`world::boot`), no level of the view's own.
    let title = load_at_start && new_game.is_none() && create_script.is_none();
    let mut engine = prof::time("start.load", || if title { world::boot(&dir) } else { world::load(&dir, &args[2]) });
    // The music driver loads at the program's start (2F9F:01C7, 035B),
    // before the first area picks a song.
    if SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed) || (!MUTE.load(std::sync::atomic::Ordering::Relaxed) && !NO_MUSIC.load(std::sync::atomic::Ordering::Relaxed)) {
        music::load_driver(&mut engine.m);
    }
    let mut creation = None;
    let mut run = match (&new_game, &create_script) {
        (Some(spec), _) => {
            newgame::start(&mut engine, spec);
            arena::transit::Run::Start
        }
        (None, Some(script)) => {
            let archive = Rc::new(archive::Archive::open(&dir));
            let mut c = create::Creation::new(&mut engine, archive, &dir, world::palette(&dir));
            for inp in create::parse_script(script) {
                c.script_input(&mut engine, inp);
            }
            if c.done() {
                newgame::play(&mut engine);
            } else {
                creation = Some(c);
            }
            arena::transit::Run::Start
        }
        (None, None) if title => arena::transit::Run::Start,
        (None, None) => areas::start(&mut engine, args[2].starts_with("CITY:"), !load_at_start),
    };
    if !title {
        interface::start(&mut engine.m);
    }
    if load_at_start {
        let save = save_dir.as_deref().expect("--load needs --save-dir");
        // A save loaded as the program starts is the title's Load (0000:117E).
        match areas::load_game_title(&mut engine, &dir, save, slot) {
            Ok(r) => {
                run = r;
                eprintln!("loaded slot {slot}: {r:?} ({})", String::from_utf8_lossy(engine.m.mem.cstr(arena::mem::DS, arena::transit::var::LEVEL_NAME)));
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    if let Some(h) = hour {
        set_hour(&mut engine, h);
    }
    if !load_at_start {
        world::city_people(&mut engine);
    }
    if let Some((col, row, heading)) = at {
        use arena::movement::var::{HEADING, X, Y};
        engine.m.mem.w16(arena::mem::DS, X, col << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, Y, row << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, HEADING, heading);
        engine.world_loop_start();
    }
    if fight {
        world::summon(&mut engine);
    }
    if people {
        world::summon_people(&mut engine);
    }
    if let Some((seed, frames, setup)) = playtest {
        // The 3D view drawn as the window's is: the renderer's walk picks
        // the creature flats play acts on (`creature_flats`), so a
        // playtest without the page would play another game.
        if !GPU_WORLD.load(std::sync::atomic::Ordering::Relaxed) {
            engine.page = Some(vec![0; 64000]);
        }
        playtest::run(&dir, engine, run, seed, frames, &setup, save_dir.as_deref().map(|d| (d, slot)));
        return;
    }
    if args.get(3).map(String::as_str) == Some("--shot") {
        let out = args.get(4).expect("--shot needs a file");
        // Creation not over: its screen as the script left it.
        if let Some(c) = &creation {
            let mut screen = hud::Screen::new(world::palette(&dir));
            c.draw(&mut screen, &engine.m);
            create::Creation::save_png(&screen, out);
            println!("{out}: creation");
            return;
        }
        let frames: usize = args.get(5).map_or(0, |v| v.parse().expect("frames"));
        let turn: i16 = args.get(6).map_or(0, |v| v.parse().expect("turn"));
        // The shot's 3D view drawn as the window's is (`arena::render`).
        if !GPU_WORLD.load(std::sync::atomic::Ordering::Relaxed) {
            engine.page = Some(vec![0; 64000]);
        }
        shot(&dir, engine, run, out, click, wait, frames, turn, &steps, answers, save_dir.as_deref().map(|d| (d, slot)), &maker_script);
        return;
    }
    let play = !MUTE.load(std::sync::atomic::Ordering::Relaxed);
    // The 3D view drawn by the original's renderer (`arena::render`), or
    // with --gpu-world by the view's own polygons, for comparison.
    if !GPU_WORLD.load(std::sync::atomic::Ordering::Relaxed) {
        engine.page = Some(vec![0; 64000]);
    }
    let mut app = make_app(dir, engine, run, creation, answers, save_dir, slot, play);
    if create_script.is_some() && app.creation.is_some() {
        // The menu's Load goes on from the game as the original's program
        // start leaves it at the menu (`world::title`), not the start
        // dungeon creation draws over.
        app.title_engine = world::title(&app.dir);
    }
    if let Some((turns, setup)) = bench {
        bench::run(&mut app, turns, &setup, started);
        return;
    }
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    event_loop.run_app(&mut app).expect("run");
}


/// The clock at hour `h` of the first day (its calendar fields agreeing,
/// so no hour passes at once), and the sky's time (`--hour`).
fn set_hour(engine: &mut Engine, h: u32) {
    use arena::clock::var::{HOUR, MINUTE, MINUTES};
    engine.m.mem.w32(arena::mem::DS, MINUTES, h % 24 * 60);
    engine.m.mem.w16(arena::mem::DS, HOUR, (h % 24) as u16);
    engine.m.mem.w16(arena::mem::DS, MINUTE, 0);
    engine.m.sky_time().expect("sky time");
}

/// The window's application over `engine` (its area's `run`, creation's
/// screens if under way, the answers given first, the save directory and
/// slot), with no window yet (`resumed` makes it). `play`: sound and
/// music played (else none, as with --mute).
#[allow(clippy::too_many_arguments)]
fn make_app(dir: PathBuf, mut engine: Engine, run: arena::transit::Run, creation: Option<create::Creation>, answers: std::collections::VecDeque<dialog::Answer>, save_dir: Option<PathBuf>, slot: u8, play: bool) -> App {
    let assets = Assets::new(&dir, &mut engine);
    let font = hud::Font::load(&assets.archive, FONT).expect("ARENAFNT.DAT");
    hud::init_style(&assets.archive, &engine.m);
    let screen = hud::Screen::new(assets.palette);
    let audio = audio::Audio::new(assets.archive.clone(), play, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let play_music = play && !NO_MUSIC.load(std::sync::atomic::Ordering::Relaxed);
    let music = music::Music::new(assets.archive.clone(), play_music, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    App {
        remade: 0,
        spare: None,
        audio,
        music,
        creation_song: None,
        movies: Default::default(),
        assets,
        start: Instant::now(),
        engine,
        gpu: None,
        keys: Default::default(),
        look: 0.0,
        captured: false,
        click: false,
        clicked: false,
        click_point: None,
        queued_click: None,
        queued_bar: None,
        released: Vec::new(),
        menu_held: Vec::new(),
        look_click: false,
        right_drag: 0.0,
        ring_screen: None,
        camp: false,
        key: false,
        message: None,
        next: Instant::now(),
        last: Some(arena::engine::Frame::Running),
        dir,
        run,
        script: answers,
        pending: None,
        inside: None,
        screen_press: false,
        box_backdrop: None,
        boxes: Default::default(),
        selected: 0,
        list_top: 0,
        list_reads: Vec::new(),
        menu_reads: Vec::new(),
        list_live: None,
        list_page: None,
        #[cfg(test)]
        own_drawing: false,
        live: None,
        status: None,
        status_held: false,
        effect: None,
        font,
        screen,
        cursor: (0.0, 0.0),
        size: (960.0, 600.0),
        drag: (0.0, 0.0),
        save_dir,
        slot,
        panel: None,
        sheet_run: None,
        page_after_sheet: false,
        log_run: None,
        ring_kept: false,
        ring_box_kept: false,
        status_box: false,
        journey_black: false,
        travel: None,
        saveui: None,
        saveui_under: None,
        options: None,
        quit: false,
        equip: None,
        map: false,
        creation,
        creation_engine: None,
        title_engine: None,
        creation_from_title: false,
        movie_at: Instant::now(),
        fade: Default::default(),
        fade_ms: 0,
        fade_old: None,
        ending: false,
        blackout: false,
    }
}

/// The program's main back to its menu on a copy of `engine` as a game's
/// end or the options screen's New leaves it (0000:109B..1114,
/// `Machine::title_menu`; the screens are creation's own); None where
/// the port can't follow it.
fn title_engine(engine: &Engine) -> Option<Engine> {
    let mut e = Engine::new(engine.m.clone(), engine.files.clone());
    // The drawing's writes, done on its copy, carried as the machine's
    // (`Engine::page_state`), and its page, which a game the menu's Load
    // loads draws on.
    e.page = engine.page.clone();
    e.page_wrote = engine.page_wrote.clone();
    e.page_ems_wrote = engine.page_ems_wrote.clone();
    e.page_ems_prior = engine.page_ems_prior.clone();
    match e.m.title_menu(&mut e.files, &mut arena::title::NoTitleHooks) {
        Some(()) => Some(e),
        None => {
            eprintln!("the way back to the menu gave up: {}", arena::giveup::describe());
            None
        }
    }
}

/// The main menu's Load on the title's state (its pick, 1194:5CF4's
/// `Machine::main_menu_pick`; 0000:117E, `Machine::title_load_setup`:
/// the song stopped, the game set up, the page cleared, PAL.COL's colours
/// read to 929E); then the load menu (25FD:0242, `saveui`).
fn title_load_setup(e: &mut Engine) {
    e.m.main_menu_pick(0);
    if e.m.title_load_setup(&mut e.files).is_none() {
        eprintln!("the title's Load's set-up gave up: {}", arena::giveup::describe());
    }
}

/// The key a list takes (3159:06A0: the arrows, PgUp, PgDn, Home, End).
fn list_key_of(code: winit::keyboard::KeyCode) -> Option<hud::ListKey> {
    use winit::keyboard::KeyCode;
    match code {
        KeyCode::ArrowUp => Some(hud::ListKey::Up),
        KeyCode::ArrowDown => Some(hud::ListKey::Down),
        KeyCode::PageUp => Some(hud::ListKey::PageUp),
        KeyCode::PageDown => Some(hud::ListKey::PageDown),
        KeyCode::Home => Some(hud::ListKey::Home),
        KeyCode::End => Some(hud::ListKey::End),
        _ => None,
    }
}

/// The game's font for its texts (`hud`).
const FONT: &str = "ARENAFNT.DAT";

/// Whether the wilderness's blocks were loaded again in a frame (217B:0265
/// changed the window of four blocks it had, `window` before).
fn wild_moved(engine: &Engine, window: u32) -> bool {
    use arena::mem::DS;
    engine.m.mem.r16(DS, arena::movement::var::LEVEL_FLAGS) & 0x200 != 0 && engine.m.mem.r32(DS, arena::wild::var::WINDOW) != window
}

/// What a turn is given: the frame's intent, a click left for the actors
/// (found by this frame's drawing), camping asked for before the frame,
/// a key pressed (it ends a rest, AFF9), the animation tick, the right
/// button held (AFF9 bit 2) and the mouse's motion since the last turn
/// (mickeys, `Machine::mouse_motion`): a drag with it swings the weapon.
#[derive(Clone, Copy, Default)]
struct TurnIn {
    intent: Intent,
    clicked: bool,
    camp: bool,
    /// A shop of this building kind entered before the frame (`--do
    /// shop:KIND`).
    shop: Option<u8>,
    /// The loot window (1194:7B16, 2433:0583) run before the frame with
    /// AX, SI, its segment and BX as a click passes them (a debugging aid
    /// and the screen check's way in, as `shop` is).
    loot: Option<[u16; 4]>,
    /// The click (1194:DB6C) run before the frame with the person or
    /// thing at CLICKED (the state check's way in, as `shop` is).
    talk: bool,
    key: bool,
    tick: usize,
    right: bool,
    drag: (i16, i16),
    /// The window's width over its height (the click's line).
    aspect: f32,
    /// A click on the automap screen's map at this point (0000:31FE): the
    /// note there edited or one begun, its text asked for; no frame runs.
    note: Option<(u16, u16)>,
    /// A left click in the view at this point of the 320 x 200 screen,
    /// taken in the frame's input step (`Front::take_click`).
    click: Option<(i32, i32)>,
    /// A click on the bar's button at this point (and whether the right
    /// button's), taken in the frame's input step (`Front::bar_click`).
    bar: Option<(i32, i32, bool)>,
}

/// What a turn did besides the engine's state: how its frame ended, and
/// whether the level's assets must be made again (stairs, the
/// wilderness's blocks).
struct TurnOut {
    frame: Option<arena::engine::Frame>,
    level_changed: bool,
    /// The effect's words the frame drew (`Front::effect`).
    effect: Option<(Vec<u8>, i32)>,
}

/// One turn (`dialog`): camping if asked, then a frame, with `talk`
/// answering the game's questions.
fn run_turn(engine: &mut Engine, assets: &mut Assets, input: &TurnIn, talk: &mut dialog::Talk) -> TurnOut {
    use arena::mem::DS;
    if let Some((x, y)) = input.note {
        // The automap screen is up: the world waits (no frame).
        engine.m.mem.w16(DS, arena::mapnotes::var::CLICK_X, x);
        engine.m.mem.w16(DS, arena::mapnotes::var::CLICK_Y, y);
        let mut hooks = Front { sheet: Some(&assets.sheet), light: Some(&mut assets.light), tick: input.tick, clicked: false, talk: Some(talk), aspect: input.aspect, effect: None, shown: Some(&assets.shown), drawn: None, click: None, dac: None, interface: Some(&mut assets.interface) };
        if engine.m.note_click(&mut engine.files, &mut hooks).is_none() {
            eprintln!("the note met a fault of the original's");
        }
        return TurnOut { frame: Some(arena::engine::Frame::Running), level_changed: false, effect: None };
    }
    let level = engine.m.mem.r8(DS, arena::movement::var::LEVEL);
    let window = engine.m.mem.r32(DS, arena::wild::var::WINDOW);
    let dac = assets.dac_turn(&mut engine.m).map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]);
    let mut hooks = Front { sheet: Some(&assets.sheet), light: Some(&mut assets.light), tick: input.tick, clicked: input.clicked, talk: Some(talk), aspect: input.aspect, effect: None, shown: Some(&assets.shown), drawn: None, click: input.click, dac: Some(dac), interface: Some(&mut assets.interface) };
    if input.camp && engine.m.camp(&mut engine.files, &mut hooks).is_none() {
        eprintln!("camping gave up: {}", arena::giveup::describe());
    }
    if let Some(kind) = input.shop {
        if shopui::shop(&mut hooks, &mut engine.m, &mut engine.files, kind).is_none() {
            eprintln!("the shop gave up: {}", arena::giveup::describe());
        }
    }
    if input.talk {
        use arena::frame::FrameHooks;
        if hooks.actor_click(&mut engine.m, &mut engine.files).is_none() {
            eprintln!("the click gave up: {}", arena::giveup::describe());
        }
    }
    if let Some([ax, si, seg, bx]) = input.loot {
        use arena::talk::TalkHooks;
        if hooks.loot(&mut engine.m, &mut engine.files, ax, si, seg, bx).is_none() {
            eprintln!("the loot window gave up: {}", arena::giveup::describe());
        }
    }
    if engine.m.mem.r8(DS, arena::rest::var::RESTING) != 0 {
        engine.m.mem.w8(DS, arena::rest::var::KEY, input.key as u8);
    }
    let buttons = engine.m.mem.r8(DS, arena::controls::var::BUTTONS) & !2 | (input.right as u8) << 1;
    engine.m.mem.w8(DS, arena::controls::var::BUTTONS, buttons);
    let (mx, my) = engine.m.mouse_motion;
    engine.m.mouse_motion = (mx.wrapping_add(input.drag.0 as u16), my.wrapping_add(input.drag.1 as u16));
    let frame = prof::time("engine.frame", || engine.frame(Input::Steer(&input.intent), &mut hooks));
    // A428..A444: the frame's waits for the timer after its body (the
    // ticks a level's flags ask, C39A as the body left it), so what the
    // body did to 0151 (a movie's resets) is under them.
    let t = engine.frame_ticks();
    engine.tick(t);
    if frame == Some(arena::engine::Frame::Running) && !(engine.page.is_some() && engine.page_gaps.is_empty()) {
        creature_flats(engine, &mut hooks);
    }
    let stairs = engine.m.mem.r8(DS, arena::movement::var::LEVEL) != level;
    let level_changed = stairs || wild_moved(engine, window);
    let effect = hooks.effect.take();
    prof::time("view_frame", || assets.view_frame(engine, input.aspect));
    TurnOut { frame, level_changed, effect }
}

/// The drawing's creatures' flats (0000:CF96): each one reached goes to
/// the hook CD56 (25CA:0271, `Machine::flat_creature`) with its square's
/// centre, its number and its word, as the original's drawing calls it.
fn creature_flats(engine: &mut Engine, hooks: &mut Front<'_>) {
    use arena::mem::DS;
    // Without the original's renderer's page (the GPU world, or a page
    // that met something not ported) the view's own guess of the flats
    // its walk would reach (`world::creature_flats_seen`), after the
    // frame; with it, `Front::creature_flats` runs them where the
    // drawing does.
    let seg = engine.m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let reached: Vec<arena::render::Creature> = world::creature_flats_seen(&engine.m).into_iter().map(|(col, row, di)| (col << 7 | 0x40, row << 7 | 0x40, engine.m.mem.r8(seg, di), seg, di)).collect();
    let _ = flats_brought(&mut engine.m, &mut engine.files, hooks, &reached);
}

/// The hook CD56 (25CA:0271, `Machine::flat_creature`), when the 3D mode
/// set it, for each flat the walk reached, in its order. The drawing
/// calls it as its walk meets the flat (0000:CF96): a flat brought to
/// life on the way has its square's word cleared (25CA:0233), so a later
/// visit to that square in the same walk finds no creature's flat there
/// and calls nothing.
fn flats_brought(m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, hooks: &mut Front<'_>, reached: &[arena::render::Creature]) -> Option<()> {
    use arena::mem::DS;
    if m.mem.r32(DS, 0xCD56) != 0x25CA_0271 {
        return Some(());
    }
    for &(x, y, flat, seg, di) in reached {
        let w = m.mem.r16(seg, di);
        if w >> 12 != 8 || (w >> 8) & 2 == 0 || w as u8 != flat {
            continue;
        }
        if m.flat_creature(files, &mut FightFront(hooks), x, y, flat, seg, di).is_none() {
            eprintln!("a creature's flat gave up: {}", arena::giveup::describe());
            return None;
        }
    }
    Some(())
}

/// The animations a turn played (`dialog::Talk::movies`), made: a text's
/// lines through the text port (1194:D459), the files from the game's.
/// With `from` (the screen shown and its colours), a movie with text
/// comes in as 1194:D459 brings it (`Movie::lead_in`).
fn movies_of(engine: &mut Engine, archive: &archive::Archive, played: Vec<(&'static str, Option<u16>, u8)>, from: Option<(&[u8], film::Dac)>) -> Vec<flc::Movie> {
    let mut first = true;
    played
        .into_iter()
        .filter_map(|(name, text, colour)| {
            // Its song (the routines that show them take one).
            if let Some(song) = music::movie_song(name) {
                engine.m.music_song(&mut engine.files, song);
            }
            if name.ends_with(".IMG") {
                // A still with its text all at once (the ending's house).
                let (img, pal) = create::image_with_palette(&engine.m, archive, name)?;
                let mut px = vec![0u8; 320 * 200];
                for row in 0..img.h {
                    for col in 0..img.w {
                        let (x, y) = (img.x as usize + col, img.y as usize + row);
                        if x < 320 && y < 200 {
                            px[y * 320 + x] = img.px[row * img.w + col];
                        }
                    }
                }
                let lines = text.map(|t| create::house_text(engine, t, colour)).unwrap_or_default();
                return Some(flc::Movie::still(flc::Flc::still(px, pal), lines));
            }
            let lines = text.map(|t| create::vision_text(engine, t, colour)).unwrap_or_default();
            let mut mv = flc::Movie::new(archive, name, lines)?;
            // The first from the screen shown; one after it from what the
            // one before showed last (`lead_later`, `App::next_movie`).
            let earlier = !std::mem::replace(&mut first, false);
            Some(match (text, from) {
                (Some(_), Some(_)) if earlier => {
                    mv.lead_later = true;
                    mv
                }
                (Some(_), Some((shown, dac))) => mv.lead_in(shown, dac),
                _ => mv,
            })
        })
        .collect()
}

/// The engine copied, for a turn to run again (`dialog`).
fn copy(engine: &Engine) -> Engine {
    prof::time("turn.copy", || Engine::new(engine.m.clone(), engine.files.clone()))
}

/// `src` copied into `dst`, reusing `dst`'s allocations (`copy` makes
/// new ones: a megabyte and more of fresh pages to fault in).
fn copy_into(dst: &mut Engine, src: &Engine) {
    prof::time("turn.copy", || {
        dst.m.clone_from(&src.m);
        dst.files.clone_from(&src.files);
    });
}

/// The window the game centres its boxes in (A914, A916, A918, A91A).
fn game_clip(m: &arena::machine::Machine) -> (i32, i32, i32, i32) {
    let r = |at| m.mem.r16(arena::mem::DS, at) as i16 as i32;
    (r(0xA914), r(0xA916), r(0xA918), r(0xA91A))
}

/// An effect's words a frame drew (0000:5011) as the HUD's: the text and
/// its row.
fn effect_of(effect: &Option<(Vec<u8>, i32)>) -> Option<(String, i32)> {
    effect.as_ref().map(|(t, y)| (String::from_utf8_lossy(t).into_owned(), *y))
}

/// The message line (061A, shown while 061E counts down) as the HUD's.
fn message_line(engine: &Engine) -> Option<Vec<Vec<hud::Span>>> {
    engine.view().message.map(|(seg, off)| hud::lines(engine.m.mem.cstr(seg, off), 0x0F))
}

/// The main quest's state for `--do mq` (a debugging aid): its block at
/// 0FDD (the step, the pieces' places, flags, the steps done at 0FE7,
/// the chest's square), the dream's bytes (F76, F77, F7A), the place
/// (the location record at 0A22: its name, id, province), the area (5E6)
/// and the day (the clock 5CC in days).
fn main_quest_state(m: &arena::machine::Machine) -> String {
    use arena::mem::DS;
    let block: Vec<String> = (0..0x13u16).map(|i| format!("{:02x}", m.mem.r8(DS, 0x0FDD + i))).collect();
    format!(
        "main quest: step {:02x}, block [{}], done {:04x}, dream {} count {} shown {}; at {} (id {:02x}, province {}), area {}, level byte {}, xp {}, 5CA {:04x} FFB {:04x} 1013 {:02x}, minute {}, day {}",
        m.mem.r8(DS, 0x0FDD),
        block.join(" "),
        m.mem.r16(DS, 0x0FE7),
        m.mem.r8(DS, 0x0F76),
        m.mem.r8(DS, 0x0F77),
        m.mem.r8(DS, 0x0F7A),
        String::from_utf8_lossy(m.mem.cstr(DS, 0x0A22)),
        m.mem.r8(DS, 0x0A22 + 0x2A),
        m.mem.r8(DS, 0x0A22 + 0x24),
        m.mem.r8(DS, 0x05E6),
        m.mem.r8(DS, 0x01AA),
        m.mem.r32(DS, 0x05AD),
        m.mem.r16(DS, 0x05CA),
        m.mem.r16(DS, 0x0FFB),
        m.mem.r8(DS, 0x1013),
        m.mem.r32(DS, 0x05CC),
        m.mem.r32(DS, 0x05CC) / 1440,
    )
}

/// The message line printed when a new one comes up.
fn print_message(engine: &Engine, last: &mut Option<(u16, u16)>) {
    let msg = engine.view().message;
    if msg != *last {
        if let Some((seg, off)) = msg {
            println!("{}", String::from_utf8_lossy(engine.m.mem.cstr(seg, off)));
        }
        *last = msg;
    }
}

/// What a scripted run keeps between turns: the area's assets (made
/// afresh in each area, as the window does), a click left for the next
/// frame's drawing, camping asked for, the answers still to give
/// (`--picks`), the frames run, and what the HUD shows at the end: the
/// boxes of the last turn that showed any, and a question left.
struct Shot {
    audio: audio::Audio,
    music: music::Music,
    /// The animations the turns played (`--do`); the first is drawn.
    movies: Vec<flc::Movie>,
    assets: Assets,
    /// The right button and a drag for the next turn (`swing`).
    right: bool,
    drag: (i16, i16),
    clicked: bool,
    camp: bool,
    /// A shop to enter in the next turn (`shop:KIND`).
    shop: Option<u8>,
    answers: std::collections::VecDeque<dialog::Answer>,
    message: Option<(u16, u16)>,
    frames: usize,
    boxes: Vec<hud::Said>,
    need: Option<dialog::Need>,
    /// The spellmaker's screen when the turn stopped inside it.
    maker_screen: Option<(dialog::Snapshot, Option<u16>)>,
    /// The shop's screen behind the question asked (`Talk::backdrop`).
    backdrop: Option<shopui::Backdrop>,
    status: Option<String>,
    /// The effect's words the last frame drew (0000:5011).
    effect: Option<(Vec<u8>, i32)>,
    /// A panel up at the end (`inv`, `spells`).
    panel: Option<panels::Panel>,
    /// The automap shown (`--do map`).
    map: bool,
    /// The game is over (the player died, or the main quest ended):
    /// later steps do nothing.
    over: bool,
    /// A screen up (the farewell and the main menu, a level's attribute
    /// screen) and the engine it acts on: fed by `ui:` steps, the shot's
    /// picture.
    ui: Option<(create::Creation, Engine)>,
    /// The travel map up (`travel`, `tclick:X:Y`, `tkey:K`), and where
    /// the cursor last was.
    travel: Option<travel::Travel>,
    cursor: (i32, i32),
    /// A click on the automap screen's map for the next turn (a note).
    note: Option<(u16, u16)>,
}

/// One turn with `intent`, its questions answered from `--picks` (then
/// left, as Esc leaves them: the turn goes on); a frame leaving the area
/// goes on to the next (`areas::next`, None where it can't).
fn shot_frame(dir: &std::path::Path, engine: &mut Engine, run: &mut arena::transit::Run, st: &mut Shot, intent: &Intent) -> Option<()> {
    if st.over {
        return Some(());
    }
    st.frames += 1;
    let input = TurnIn {
        click: None,
        intent: *intent,
        clicked: std::mem::take(&mut st.clicked),
        camp: std::mem::take(&mut st.camp),
        shop: st.shop.take(),
        loot: None,
        talk: false,
        key: false,
        tick: st.frames * 4,
        right: std::mem::take(&mut st.right),
        drag: std::mem::take(&mut st.drag),
        aspect: 1.6,
        note: st.note.take(),
        bar: None,
    };
    let mut talk = dialog::Talk { answers: std::mem::take(&mut st.answers), ..Default::default() };
    st.audio.before(&mut engine.m);
    st.music.before(&mut engine.m);
    let ticks = engine.frame_ticks() as u32;
    let out = run_turn(engine, &mut st.assets, &input, &mut talk);
    st.audio.after(&mut engine.m);
    let played = std::mem::take(&mut talk.movies);
    let archive = st.assets.archive.clone();
    st.movies.extend(movies_of(engine, &archive, played, None));
    st.music.after(&mut engine.m, ticks);
    st.answers = talk.answers;
    if !talk.boxes.is_empty() {
        st.boxes = if talk.need.is_some() { talk.since_answer } else { talk.boxes };
    }
    // Kept as the question is (`need`): set by the turn that asked.
    if talk.need.is_some() {
        st.maker_screen = talk.maker.take();
        st.backdrop = talk.backdrop.take();
    }
    if let Some(need) = talk.need {
        if let (dialog::Need::Spend(snap), None) = (&need, &st.ui) {
            let mut e = Engine::new((*snap.0).clone(), engine.files.clone());
            let c = create::Creation::level_up(&mut e.m, st.assets.archive.clone(), dir, st.assets.palette);
            st.ui = Some((c, e));
        }
        st.need = Some(need);
    }
    if talk.status.is_some() {
        st.status = talk.status;
    }
    print_message(engine, &mut st.message);
    st.effect = out.effect.clone();
    if out.level_changed {
        eprintln!("the level changed (stairs, or the wilderness's blocks)");
        st.assets.level(engine);
    }
    if out.frame? == arena::engine::Frame::Left {
        let next = areas::next(engine, dir, *run);
        if next.is_none() && engine.m.mem.r16(arena::mem::DS, 0x05B1) & 0x100 != 0 {
            // The ending's text is on the message line; the farewell's
            // vision and the main menu come up.
            st.over = true;
            print_message(engine, &mut st.message);
            let c = create::Creation::ending(engine, st.assets.archive.clone(), dir, st.assets.palette, None);
            let mut fresh = world::load(dir, "START.MIF");
            fresh.files.disk_from(&engine.files);
            st.ui = Some((c, fresh));
            return Some(());
        }
        *run = next?;
        st.assets = Assets::new(dir, engine);
    }
    Some(())
}

/// Renders one frame offscreen to a PNG, after `frames` frames of
/// walking forward, then `steps` (see `--do`), with the HUD: the message
/// line, the last boxes, and the question left unanswered, if any.
#[allow(clippy::too_many_arguments)]
fn shot(dir: &std::path::Path, mut engine: Engine, mut run: arena::transit::Run, out: &str, click: bool, wait: usize, frames: usize, turn: i16, steps: &str, answers: std::collections::VecDeque<dialog::Answer>, saves: Option<(&std::path::Path, u8)>, maker_script: &str) {
    let mut maker: Option<spellmaker::Maker> = None;
    let run = &mut run;
    let assets = Assets::new(dir, &mut engine);
    // The texts' style from the start, as the window has it: the boxes
    // the turns show are drawn in it.
    hud::init_style(&assets.archive, &engine.m);
    // Shots play nothing; with --sound-log they print what would play.
    let audio = audio::Audio::new(assets.archive.clone(), false, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let music = music::Music::new(assets.archive.clone(), false, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let st = &mut Shot { audio, music, movies: Vec::new(), assets, right: false, drag: (0, 0), clicked: false, camp: false, shop: None, answers, message: None, frames: 0, boxes: Vec::new(), need: None, maker_screen: None, backdrop: None, status: None, effect: None, panel: None, map: false, over: false, ui: None, travel: None, cursor: (160, 100), note: None };
    // One frame turning (or not) before the walk: the actors pick their
    // sprites in a frame.
    let intent = Intent { look: turn, ..Default::default() };
    shot_frame(dir, &mut engine, run, st, &intent).expect("area");
    // A click is acted on in the next frame (its drawing finds what is
    // under it), so one is waited at least.
    if click {
        st.clicked = centre_click(&mut engine, &mut st.boxes);
    }
    let wait = if click { wait.max(1) } else { wait };
    for _ in 0..wait {
        shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
    }
    let walk = Intent { forward: 256, ..Default::default() };
    for _ in 0..frames {
        shot_frame(dir, &mut engine, run, st, &walk).expect("area");
    }
    for step in steps.split(',').filter(|s| !s.is_empty()) {
        if let Some(at) = step.strip_prefix("at:") {
            // A square's centre and a heading, as --at, then a frame
            // standing (the wilderness's window follows in its input step).
            let v: Vec<u16> = at.split(':').map(|x| x.parse().expect("--do at:COL:ROW:HEADING")).collect();
            use arena::movement::var::{HEADING, X, Y};
            engine.m.mem.w16(arena::mem::DS, X, v[0] << 7 | 0x40);
            engine.m.mem.w16(arena::mem::DS, Y, v[1] << 7 | 0x40);
            engine.m.mem.w16(arena::mem::DS, HEADING, v[2]);
            engine.world_loop_start();
            shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            continue;
        }
        if step == "travel" || step.starts_with("tclick:") || step.starts_with("tkey:") {
            // The travel map: opened as the map button opens it (Shift+M),
            // a click on it at (X, Y), a key (esc, enter, s, t, a letter).
            let archive = st.assets.archive.clone();
            if step == "travel" {
                match travel::Travel::open(&mut engine, &archive, Some(&st.assets.dac().map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]))) {
                    Ok(t) => st.travel = Some(t),
                    Err(b) => st.boxes = vec![b.into()],
                }
            } else if let Some(at) = step.strip_prefix("tclick:") {
                let (x, y) = at.split_once(':').expect("--do tclick:X:Y");
                let (x, y) = (x.parse().expect("--do tclick:X:Y"), y.parse().expect("--do tclick:X:Y"));
                st.cursor = (x, y);
                if let Some(t) = &mut st.travel {
                    t.click(&mut engine, &archive, x, y, false);
                }
            } else if let Some(k) = step.strip_prefix("tkey:") {
                use winit::keyboard::KeyCode;
                let (code, text) = match k {
                    "esc" => (KeyCode::Escape, None),
                    "enter" => (KeyCode::Enter, None),
                    "s" => (KeyCode::KeyS, Some("s")),
                    "t" => (KeyCode::KeyT, Some("t")),
                    _ => (KeyCode::KeyA, Some(k)),
                };
                if let Some(t) = &mut st.travel {
                    t.key(&mut engine, code, text, (160, 100));
                }
            }
            if let Some(t) = &mut st.travel {
                t.skip_journey();
            }
            let mut boxes = Vec::new();
            travel::after(&mut st.travel, &mut engine, &mut boxes, true);
            if !boxes.is_empty() {
                st.boxes = boxes.into_iter().map(Into::into).collect();
            }
            continue;
        }
        if let Some(at) = step.strip_prefix("clickat:") {
            // A click at (X, Y) of the 320 x 200 screen, found by the next
            // frame's drawing.
            let (x, y) = at.split_once(':').expect("--do clickat:X:Y");
            let at = (x.parse().expect("--do clickat:X:Y"), y.parse().expect("--do clickat:X:Y"));
            st.clicked = click_at(&mut engine, &mut st.boxes, at);
            shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            continue;
        }
        if let Some(t) = step.strip_prefix("ui:") {
            // A key or click (as --create's script) for the screen up, or
            // time passing for its movie (`ui:ms:N`).
            if let Some((c, e)) = &mut st.ui {
                match t.strip_prefix("ms:") {
                    Some(ms) => c.advance(&mut e.m, ms.parse().expect("--do ui:ms:N")),
                    None => {
                        for inp in create::parse_script(t) {
                            c.script_input(e, inp);
                        }
                    }
                }
                if let Some(o) = c.outcome() {
                    println!("ui: {o:?}");
                    // A level's points spent: the screen closes (the
                    // window replays the turn with them; here the turn
                    // went on as Esc leaves the question).
                    if let create::Outcome::Spent(_) = o {
                        st.ui = None;
                        st.need = None;
                    }
                }
            }
            continue;
        }
        if let Some(spec) = step.strip_prefix("movie:") {
            // A debugging aid: an animation (or a still .IMG) queued as the
            // game's hooks queue them (NAME, or NAME:TEXT with the text's
            // number in hex, in colour 33h or NAME:TEXT:COLOUR's, hex).
            let mut parts = spec.split(':');
            let name = parts.next().unwrap_or_default();
            let text = parts.next().map(|t| u16::from_str_radix(t, 16).expect("--do movie:NAME:TEXT"));
            let colour = parts.next().map_or(0x33, |c| u8::from_str_radix(c, 16).expect("--do movie:NAME:TEXT:COLOUR"));
            let name: &'static str = Box::leak(name.to_ascii_uppercase().into_boxed_str());
            let archive = st.assets.archive.clone();
            st.movies.extend(movies_of(&mut engine, &archive, vec![(name, text, colour)], None));
            continue;
        }
        if let Some(d) = step.strip_prefix("swing:") {
            // The right button down with a drag (mickeys) for a turn.
            let (dx, dy) = d.split_once(':').expect("--do swing:DX:DY");
            st.right = true;
            st.drag = (dx.parse().expect("--do swing:DX:DY"), dy.parse().expect("--do swing:DX:DY"));
            shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            continue;
        }
        let (what, n) = step.split_once(':').map_or((step, 1), |(a, n)| (a, n.parse().expect("--do: a number")));
        match what {
            "click" => {
                st.clicked = centre_click(&mut engine, &mut st.boxes);
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            }
            "ready" => interface::ready(&mut engine.m),
            "cast" => {
                // Spell N of the spellbook cast (29CF:071F), as the
                // spellbook's pick would.
                if engine.m.cast_spell(&mut engine.files, &mut arena::cast::NoCastHooks, n as u8).is_none() {
                    eprintln!("the cast gave up: {}", arena::giveup::describe());
                }
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            }
            "compass" => interface::toggle_compass(&mut engine.m),
            "mq" => println!("{}", main_quest_state(&engine.m)),
            "storm" => {
                // Frames until the sky's hook flashes (the window draws the
                // sky every frame; a shot only at its end).
                for _ in 0..n {
                    shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
                    if st.assets.flash.get().is_some() {
                        break;
                    }
                }
            }
            "spellmaker" => {
                // The guild's spellmaker (1194:6064) run from --spellmaker's
                // script; its last screen is the shot.
                let mut mk = spellmaker::Maker::new(maker_script);
                if let Some(g) = mk.gold {
                    engine.m.mem.w32(arena::mem::DS, 0x05C2, g);
                }
                if engine.m.spellmaker(&mut engine.files, &mut mk, 0x01A4).is_none() {
                    eprintln!("the spellmaker gave up: {}", arena::giveup::describe());
                }
                for t in &mk.messages {
                    println!("{t}");
                }
                maker = Some(mk);
            }
            // The inventory (I) and the spellbook (C), up until `close`;
            // their actions as the window's keys and clicks make them.
            "inv" => {
                sheets::pics(&st.assets.archive, &engine.m);
                let dac = st.assets.dac().map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]);
                st.panel = Some(panels::script_page(&mut engine, &dac));
            }
            "spells" => st.panel = Some(panels::Panel::Spellbook(Default::default())),
            "close" => st.panel = None,
            "give" => panels::give(&mut engine.m, n as u16),
            "learn" => panels::learn(&mut engine.m, n as u8),
            "corpse" => println!("corpse: {}", panels::corpse(&mut engine)),
            "fatigue" => engine.m.mem.w16(arena::mem::DS, 0x0201, n as u16),
            "xp" => {
                let at = arena::character::PLAYER + 0x409;
                let v = engine.m.mem.r32(arena::mem::DS, at).wrapping_add(n as u32);
                engine.m.mem.w32(arena::mem::DS, at, v);
                let f = engine.m.mem.r16(arena::mem::DS, 0x05CA) | 0x100;
                engine.m.mem.w16(arena::mem::DS, 0x05CA, f);
            }
            "wear" => {
                // Item N's condition (+6) halved, a debugging aid.
                let at = arena::inventory::PLAYER_SLOTS + n as u16 * 0x13 + 6;
                let v = engine.m.mem.r16(arena::mem::DS, at) / 2;
                engine.m.mem.w16(arena::mem::DS, at, v);
            }
            "material" => {
                // Every worn piece of armour enchanted (+E bit 0) and of
                // material N (+10), a debugging aid for the doll's colours.
                use arena::inventory::{PLAYER_SLOTS, SLOT_SIZE};
                for i in 0..40u16 {
                    let si = PLAYER_SLOTS + i * SLOT_SIZE;
                    let f = engine.m.mem.r8(arena::mem::DS, si + 0xE);
                    if f & 0xC0 == 0xC0 && f & 0xC == 0 {
                        engine.m.mem.w8(arena::mem::DS, si + 0xE, f | 1);
                        engine.m.mem.w8(arena::mem::DS, si + 0x10, n as u8);
                    }
                }
            }
            "mday" => {
                // The day of the month (CCA8, 0..1Fh), a debugging aid for
                // the moons' places and phases.
                engine.m.mem.w16(arena::mem::DS, 0xCCA8, n as u16 & 0x1F);
            }
            "day" => {
                // The clock N days on, a debugging aid (the calendar's
                // other fields are left).
                let t = engine.m.mem.r32(arena::mem::DS, 0x05CC).wrapping_add(n as u32 * 1440);
                engine.m.mem.w32(arena::mem::DS, 0x05CC, t);
            }
            "hp" => {
                // The player's health, a debugging aid.
                engine.m.mem.w16(arena::mem::DS, 0x01FD, n as u16);
            }
            "gold" => {
                // The player's gold, a debugging aid.
                engine.m.mem.w32(arena::mem::DS, 0x05C2, n as u32);
            }
            "sp" => {
                // The spell points and their maximum, a debugging aid.
                engine.m.mem.w16(arena::mem::DS, 0x020A, n as u16);
                engine.m.mem.w16(arena::mem::DS, 0x020C, n as u16);
            }
            "class" => {
                // The player's class (the low five bits of +5), a debugging
                // aid as --at is.
                let at = arena::character::PLAYER + arena::character::field::CLASS;
                let b = engine.m.mem.r8(arena::mem::DS, at) & 0xE0 | (n as u8 & 0x1F);
                engine.m.mem.w8(arena::mem::DS, at, b);
            }
            "equip" | "select" | "drop" | "use" | "spell" => {
                let mut said = panels::Said::default();
                match what {
                    "equip" => panels::script_line(&mut engine, n as usize, true, &mut said),
                    "select" => panels::script_line(&mut engine, n as usize, false, &mut said),
                    // The drop button, its question (if it asks) answered
                    // yes, or no with drop:1.
                    "drop" => panels::script_drop(&mut engine, n == 0, &mut said),
                    "use" => panels::use_pick(&mut engine.m, &mut engine.files, n as u16, &mut said),
                    _ => {
                        st.panel = None;
                        panels::cast(&mut engine, n as usize, &mut said);
                    }
                }
                if !said.boxes.is_empty() {
                    st.boxes = said.boxes;
                }
            }
            "usable" => println!("usable: {:?}", panels::usable_names(&engine.m)),
            "book" => {
                for (i, sp) in panels::spells(&engine.m).iter().enumerate() {
                    println!("spell {i}: {} ({}), cost {}", sp.name, sp.index, sp.cost);
                }
            }
            "map" => {
                if st.map {
                    automap::close(&mut engine);
                } else {
                    sheets::pics(&st.assets.archive, &engine.m);
                    automap::open(&mut engine, st.assets.dac(), (160, 100));
                }
                st.map = !st.map;
            }
            // The automap screen's button N (0 up, 1 down, 2 right, 3 left,
            // 4 the centre).
            "mapbtn" => {
                engine.m.automap_view_button(n as u16);
                engine.m.automap_origin();
            }
            // The character sheet (the portrait), its page turned to the
            // equipment ('N'), the logbook (the scroll) and its next page.
            "sheet" => {
                sheets::pics(&st.assets.archive, &engine.m);
                engine.m.drop_place();
                let dac = st.assets.dac().map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]);
                if engine.m.sheet_open(&mut engine.files, Some(&dac)).is_none() {
                    eprintln!("the character sheet gave up: {}", arena::giveup::describe());
                }
                st.panel = Some(panels::Panel::Sheet);
            }
            "page" => {
                if engine.m.sheet_equipment(&mut engine.files, Some(b'n')).is_none() {
                    eprintln!("the equipment page gave up: {}", arena::giveup::describe());
                }
                st.panel = Some(panels::equipment_page());
            }
            "log" => {
                sheets::pics(&st.assets.archive, &engine.m);
                st.panel = Some(panels::Panel::Log(sheets::open_log(&engine.m, &engine.files)));
            }
            "nextpage" => {
                if let Some(panels::Panel::Log(log)) = st.panel.as_mut() {
                    if log.page + 1 < log.pages.len() {
                        log.page += 1;
                    }
                }
            }
            // The bar's button N clicked at its centre (`sheets::bar_click`).
            "bar" => {
                sheets::pics(&st.assets.archive, &engine.m);
                let (x, y) = sheets::bar_centre(n as usize);
                match sheets::bar_click(&mut engine.m, &mut engine.files, x, y, false) {
                    Some(sheets::BarAction::Automap) if !st.map => {
                        automap::open(&mut engine, st.assets.dac(), (x, y));
                        st.map = true;
                    }
                    Some(sheets::BarAction::Status(lines)) => st.boxes = vec![lines.into()],
                    Some(sheets::BarAction::Spells(_)) => st.panel = Some(panels::Panel::Spellbook(Default::default())),
                    Some(sheets::BarAction::Log(_)) => st.panel = Some(panels::Panel::Log(sheets::open_log(&engine.m, &engine.files))),
                    Some(sheets::BarAction::Sheet) => st.panel = Some(panels::Panel::Sheet),
                    Some(a) => println!("bar {n}: {a:?}"),
                    None => {}
                }
            }
            "camp" => {
                st.camp = true;
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            }
            "shop" => {
                st.shop = Some(n as u8);
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            }
            "quest" => {
                // The talk pointed at the first offered errand's giver (as
                // view/examples/town_quests.rs does), then the tavern.
                use arena::mem::DS;
                use arena::sidequest::var::{TALK, TALKER};
                let offered = |m: &arena::machine::Machine| (0..4u16).map(|i| arena::quest::var::OTHERS + i * 0x29).find(|&si| m.quest_kind(si) == 2);
                if offered(&engine.m).is_none() {
                    // A town the view starts in hasn't had its quests made
                    // (that is the area's entry, 0000:46B2): made here.
                    engine.m.mem.w16(DS, arena::sidequest::var::PLACE_SEEN, 0xFFFF);
                    if engine.m.city_entered(&mut engine.files, &mut arena::quest::NoQuestHooks).is_none() {
                        eprintln!("the town's quests gave up: {}", arena::giveup::describe());
                    }
                }
                let m = &mut engine.m;
                let giver = offered(m);
                match giver {
                    Some(si) => {
                        let g = m.mem.r16(DS, si + 0x10);
                        m.mem.w8(DS, TALK, 4);
                        m.mem.w16(DS, TALKER, g & 0xFF00 | ((g as u8 as u16) << 1) & 0xFF);
                        st.shop = Some(4);
                        shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
                        let state = ["free", "under way", "offered", "failed"][engine.m.quest_kind(si) as usize & 3];
                        println!("quest: errand {} {state}", (si - arena::quest::var::OTHERS) / 0x29);
                    }
                    None => println!("no errand offered"),
                }
            }
            "save" | "load" => {
                let (sdir, slot) = saves.expect("--do save/load needs --save-dir");
                let msg = if what == "save" {
                    save_to(&mut engine, sdir, slot, st.assets.dac(), None)
                } else {
                    match load_from(&mut engine, dir, sdir, slot) {
                        Ok((m, r)) => {
                            *run = r;
                            st.assets = Assets::new(dir, &mut engine);
                            m
                        }
                        Err(m) => m,
                    }
                };
                println!("{msg}");
                st.boxes = vec![vec![vec![hud::Span { colour: 0x0F, text: msg, font: Default::default(), code: None }]].into()];
            }
            "key" => {
                // A key (its scan code) down for a frame, through the key
                // interrupt (0000:718A) as the window gives its keys.
                let _ = engine.m.key_event(n as u8);
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
                let _ = engine.m.key_event(n as u8 | 0x80);
            }
            "turn" => shot_frame(dir, &mut engine, run, st, &Intent { look: n as i16, ..Default::default() }).expect("area"),
            "walk" | "wait" => {
                let intent = if what == "walk" { walk } else { Intent::default() };
                for _ in 0..n {
                    shot_frame(dir, &mut engine, run, st, &intent).expect("area");
                }
            }
            _ => panic!("--do: {step}?"),
        }
    }
    if let Some(t) = &st.travel {
        // The travel map up is the picture, its last box over it.
        let mut screen = hud::Screen::new(st.assets.palette);
        if let Some(font) = hud::Font::load(&st.assets.archive, FONT) {
            hud::init_style(&st.assets.archive, &engine.m);
            t.draw(&mut screen, &font, &mut engine.m, st.cursor.0, st.cursor.1);
            let hud = hud::Hud { boxed: st.boxes.last().map(|b| b.lines.clone()), ..Default::default() };
            hud::compose_over(&mut screen, &font, &hud);
        }
        create::Creation::save_png(&screen, out);
        println!("{out}: the travel map");
        return;
    }
    if let Some((c, e)) = &st.ui {
        // The screen up is the picture.
        let mut screen = hud::Screen::new(st.assets.palette);
        c.draw(&mut screen, &e.m);
        create::Creation::save_png(&screen, out);
        println!("{out}: screen up");
        return;
    }
    let (w, h) = (640u32, 400u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = st.assets.renderer(&device, &queue, format);
    let (sky, lights) = st.assets.drawn(&mut engine, w as f32 / h as f32);
    let cam = camera(&engine);
    let sprites = st.assets.sprites(&mut engine, cam, (wait + frames) * 4, w as f32 / h as f32);
    r.set_sky(sky);
    r.set_lights(lights);
    r.set_window(interface::window(&engine.m));
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    r.draw(&device, &queue, &view, w, h, cam, &sprites);
    if let Some(font) = hud::Font::load(&st.assets.archive, FONT) {
        hud::init_style(&st.assets.archive, &engine.m);
        let mut hud = hud::Hud { message: message_line(&engine), boxed: st.boxes.last().map(|b| b.lines.clone()), status: None, counters: hud::spell_counters(&engine.m), level: hud::level_text(&engine.m), effect: effect_of(&st.effect), clip: game_clip(&engine.m), ..Default::default() };
        if engine.m.mem.r8(arena::mem::DS, arena::rest::var::RESTING) != 0 {
            hud.status = st.status.clone();
        }
        if let Some(list) = st.panel.as_ref().and_then(|p| p.bar_list(&engine.m)) {
            hud.menu = Some(list);
        }
        // A typed field asked: the game's editor opened on the game as it
        // asked, its text and cursor drawn over the rest.
        let mut live = None;
        match st.need.clone() {
            Some(dialog::Need::Menu(mut m)) => {
                if !m.list {
                    m.selected = usize::MAX;
                }
                hud.menu = Some(m)
            }
            Some(dialog::Need::Text(a)) => {
                hud.typing = Some(a.prompt.clone());
                live = field::Live::open(&a.game.0, a.field, a.look);
            }
            Some(dialog::Need::Spend(_) | dialog::Need::Spellmaker | dialog::Need::Travel(_) | dialog::Need::Screen(..)) | None => {}
        }
        let draw_live = |screen: &mut hud::Screen| {
            if let Some(l) = &live {
                l.draw(screen, true);
            }
        };
        let sheet = st.panel.as_ref().is_some_and(|p| !p.over_play());
        let mut screen = hud::Screen::new(if sheet { st.assets.charsheet } else { st.assets.dac() });
        if let Some(mk) = &maker {
            spellmaker::draw(&mut screen, &font, &st.assets.archive, &engine.files, mk);
            hud::Overlay::new(&device, format).draw(&device, &queue, &view, &screen);
        } else if let Some((snap, form)) = &st.maker_screen {
            // The turn stopped inside the guild's spellmaker: its screen,
            // with the question asked over it.
            spellmaker::draw_state(&mut screen, &font, &st.assets.archive, &engine.files, &snap.0, *form);
            hud::compose_over(&mut screen, &font, &hud);
            draw_live(&mut screen);
            hud::Overlay::new(&device, format).draw(&device, &queue, &view, &screen);
        } else if let Some(b) = st.backdrop.as_ref().filter(|_| st.need.is_some()) {
            b.draw(&mut screen, &st.assets.archive, &engine.m);
            hud::compose_over(&mut screen, &font, &hud);
            draw_live(&mut screen);
            hud::Overlay::new(&device, format).draw(&device, &queue, &view, &screen);
        } else {
        // The original's 3D view under the interface, as the window's
        // (where its renderer reached no path not ported).
        if let (false, Some(page), true) = (sheet, &engine.page, engine.page_gaps.is_empty()) {
            screen.underlay(page);
        }
        if !engine.page_gaps.is_empty() {
            eprintln!("the 3D view is the view's own: not ported {:?}", engine.page_gaps);
        }
        st.assets.interface.draw(&mut screen, &font, &engine.m);
        if let Some(panel) = &st.panel {
            let equip = panels::load_image(&st.assets.archive, &engine.m, "EQUIP.IMG");
            let doll = doll::images(&st.assets.archive, &engine.m);
            panels::draw(&mut screen, &font, equip.as_ref(), &doll, panel, &engine.m, Some((&st.assets.archive, &engine.files)));
        }
        hud::compose_over(&mut screen, &font, &hud);
        if st.map {
            automap::draw(&mut screen, &mut engine.m);
            automap::draw_notes(&mut screen, &engine);
            automap::draw_pointer(&mut screen, st.cursor.0, st.cursor.1);
        }
        draw_live(&mut screen);
        // An animation the turns played: its first frame and lines.
        if let Some(mv) = st.movies.first() {
            println!("animations played: {}", st.movies.len());
            mv.draw(&mut screen, &font);
        }
        hud::Overlay::new(&device, format).draw(&device, &queue, &view, &screen);
        }
    }
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (w * h * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = buf.slice(..).get_mapped_range().to_vec();
    let file = std::fs::File::create(out).expect("create");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().expect("png").write_image_data(&data).expect("png");
    let v = engine.view();
    println!("{out}: at ({:#x}, {:#x}) heading {:#x}, {} triangles", v.x, v.y, v.heading, st.assets.mesh.len() / 3);
}

/// A left click in the centre of the view (`Machine::view_click`), on
/// what the last frame's drawing left under it; its boxes (a door locked,
/// a shop closed) added to `boxes`. Whether it is left for the actors
/// (C39A bit 80h), for the next frame's drawing to find. (A question it
/// asks is left, as Esc leaves it.)
fn centre_click(engine: &mut Engine, boxes: &mut Vec<hud::Said>) -> bool {
    click_at(engine, boxes, (164, 74))
}

/// A right click in the centre of the view (2826:0C15, `Machine::look`):
/// the square the drawing would leave under it (as for a left click,
/// within the drawn distance C394) named on the message line, and in a
/// town noted on the automap.
fn look_centre(engine: &mut Engine) {
    use arena::mem::DS;
    let Some(sq) = world::pick(&engine.m, engine.m.mem.r16(DS, 0xC394)) else { return };
    engine.m.mem.w16(DS, arena::controls::var::PICKED, sq);
    if (sq as i16) <= 0 {
        return;
    }
    look_at(engine, sq & 0xFF00 | (sq as u8 >> 1) as u16);
}

/// 2826:0C15 on the square `ax` (its row in AH, its column in AL).
fn look_at(engine: &mut Engine, ax: u16) {
    let mut talk = dialog::Talk::default();
    let mut hooks = Front { talk: Some(&mut talk), ..Default::default() };
    if engine.m.look(&mut engine.files, &mut hooks, ax).is_none() {
        eprintln!("looking gave up: {}", arena::giveup::describe());
    }
}

/// A left click at (x, y) of the 320 x 200 screen in the view: the mouse
/// put 4 up and left of it, as the click (CB60, 2826:0A52) is the cursor
/// + 4; else as `centre_click`.
fn click_at(engine: &mut Engine, boxes: &mut Vec<hud::Said>, (x, y): (i32, i32)) -> bool {
    engine.m.set_mouse((x - 4).max(0) as u16, (y - 4).max(0) as u16, 0);
    let mut talk = dialog::Talk::default();
    let mut hooks = Front { talk: Some(&mut talk), ..Default::default() };
    // The original takes the click in the input step, during the physics,
    // when the position is the player's (BA62, 0000:A3AA), not the eye's
    // the frame leaves in C3D0 (A919): the click from there, and the eye
    // put back unless the click moved the player.
    use arena::mem::DS;
    let words = |m: &arena::machine::Machine, at: u16| [m.mem.r16(DS, at), m.mem.r16(DS, at + 2), m.mem.r16(DS, at + 4)];
    let put = |m: &mut arena::machine::Machine, at: u16, w: [u16; 3]| (0..3).for_each(|i| m.mem.w16(DS, at + 2 * i as u16, w[i]));
    let (eye, real) = (words(&engine.m, 0xC3D0), words(&engine.m, 0xBA62));
    put(&mut engine.m, 0xC3D0, real);
    if engine.m.view_click(&mut engine.files, &mut hooks).is_none() {
        eprintln!("the click gave up: {}", arena::giveup::describe());
    }
    let now = words(&engine.m, 0xC3D0);
    put(&mut engine.m, 0xBA62, now);
    if now == real {
        put(&mut engine.m, 0xC3D0, eye);
    }
    boxes.extend(talk.boxes);
    engine.m.mem.r16(arena::mem::DS, arena::movement::var::LEVEL_FLAGS) & 0x80 != 0
}

/// Saving writes the game's files into the save directory (by their
/// names, the game's directory dropped); the VGA's palette is the view's.
/// The screen saved with the level (0xFA00 bytes from A8FE, the page as
/// the save writes it) is `shot`, the view's screen at that moment
/// (`App::save_shot`); without one it is left black.
struct SaveDir<'a> {
    dir: &'a std::path::Path,
    palette: [[u8; 4]; 256],
    shot: Option<&'a [u8]>,
    /// The files written (their DOS names, their bytes), for the game's
    /// `Files` to have them too (`save_to`).
    written: Vec<(Vec<u8>, Vec<u8>)>,
}

impl SaveDir<'_> {
    fn name(m: &arena::machine::Machine, path: u16) -> String {
        let p = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, path)).into_owned();
        p.rsplit(['\\', '/']).next().unwrap_or(&p).to_string()
    }
    fn path(&self, m: &arena::machine::Machine, path: u16) -> PathBuf {
        dos_file(self.dir, &Self::name(m, path))
    }
}

/// The save folder's file of the DOS name `name`: the one there, whatever
/// the case of its letters (DOS's names have none; the folder may be on a
/// file system where they do, and the game's own paths are lower case
/// where the copies' are upper), else the name in upper case, as DOS
/// keeps it. Without this a save wrote saveengn.05 beside the working
/// slot's SAVEENGN.05 copied just before, and a load could find the old.
fn dos_file(dir: &std::path::Path, name: &str) -> PathBuf {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
        .unwrap_or_else(|| dir.join(name.to_ascii_uppercase()))
}

impl arena::save_write::SaveHooks for SaveDir<'_> {
    fn delete(&mut self, m: &arena::machine::Machine, path: u16) {
        // Every file of the name, whatever its case (DOS has one).
        let name = Self::name(m, path);
        for e in std::fs::read_dir(self.dir).into_iter().flatten().flatten() {
            if e.file_name().to_string_lossy().eq_ignore_ascii_case(&name) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    fn write(&mut self, m: &arena::machine::Machine, path: u16, data: &[u8]) {
        let p = self.path(m, path);
        let mut data = data.to_vec();
        if p.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_uppercase().starts_with("SAVEGAME")) {
            let n = data.len().min(0xFA00);
            match self.shot {
                Some(shot) => data[..n].copy_from_slice(&shot[..n]),
                None => data[..n].fill(0),
            }
        }
        if let Err(e) = std::fs::write(&p, &data) {
            eprintln!("{}: {e}", p.display());
        }
        if let Some(n) = p.file_name() {
            self.written.push((n.to_string_lossy().as_bytes().to_vec(), data));
        }
    }
    fn palette(&mut self, m: &mut arena::machine::Machine) {
        for (i, c) in self.palette.iter().enumerate() {
            for (k, v) in c.iter().take(3).enumerate() {
                m.mem.w8(arena::mem::DS, arena::save_write::var::PALETTE + (i * 3 + k) as u16, v >> 2);
            }
        }
    }
}

/// The game saved to `slot` in `dir` (25FD:0113, `save_game`): NAMES.DAT,
/// SAVEGAME.nn and SAVEENGN.nn. What happened, to show.
fn save_to(engine: &mut Engine, dir: &std::path::Path, slot: u8, palette: [[u8; 4]; 256], shot: Option<&[u8]>) -> String {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return format!("{}: {e}", dir.display());
    }
    // (The saved position, BA62, which saving the level makes the
    // player's, is the world loop's (A2A3), the engine's frame's: the
    // player's place, not the eye's 1Eh behind it in C3D0.)
    engine.m.mem.w8(arena::mem::DS, arena::save::var::SLOT, slot);
    // As 25FD:010B..0110: the automap written (23F1:0145), the working
    // slot's files (CITYDATA, SPELLS, AUTOMAP, ...) copied to the slot's
    // (25FD:0612) and kept on disk with the save; then the save itself.
    engine.m.automap_write(&mut engine.files);
    for name in engine.m.slot_files_out(&mut engine.files) {
        let Some(data) = engine.files.written(&name) else { continue };
        let p = dos_file(dir, &String::from_utf8_lossy(&name));
        if let Err(e) = std::fs::write(&p, data) {
            eprintln!("{}: {e}", p.display());
        }
    }
    let mut out = SaveDir { dir, palette, shot, written: Vec::new() };
    engine.m.save_game(&mut out);
    // The game's files have them too: a load reads the files written in
    // play before the directories' (`Files`), and the slot's copies made
    // above (25FD:0612 copies every working "*.64", SAVEGAME's and SAVEENGN's of an
    // earlier load included) would otherwise hide the save just made.
    for (name, data) in out.written {
        engine.files.put(&name, &data);
    }
    // The save's end (25FD:01CB..020C): the wilderness's blocks copied to
    // the slot's names (kept on disk too), the slot back to the working
    // slot, an interior's automap loaded.
    for name in engine.m.save_finish(&mut engine.files) {
        let Some(data) = engine.files.written(&name) else { continue };
        let p = dos_file(dir, &String::from_utf8_lossy(&name));
        if let Err(e) = std::fs::write(&p, data) {
            eprintln!("{}: {e}", p.display());
        }
    }
    // The CITYDATA check after it (25FD:020C, 242A:000E).
    let mut bad = false;
    let _ = engine.m.citydata_check(&mut engine.files, &mut |_, _| bad = true);
    if bad {
        return "The save's CITYDATA failed the game's check.".to_string();
    }
    format!("Saved to slot {slot} in {}.", dir.display())
}

/// The game of `slot` in `dir` loaded back as the game loads in play
/// (`areas::load_game`), in whatever area it was saved. Returns the
/// message and the loaded area's run; the caller makes the assets again.
fn load_from(engine: &mut Engine, game: &std::path::Path, dir: &std::path::Path, slot: u8) -> Result<(String, arena::transit::Run), String> {
    let run = areas::load_game(engine, game, dir, slot)?;
    Ok((format!("Loaded slot {slot}."), run))
}

struct Gpu {
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    overlay: hud::Overlay,
}

/// The movement keys held, as the original's (2826:0473): Up and Down
/// walk, Left and Right turn, or strafe with Ctrl; J jumps (Space too,
/// the view's), a running jump with Shift.
#[derive(Default)]
struct Keys {
    forward: bool,
    back: bool,
    turn_left: bool,
    turn_right: bool,
    ctrl: bool,
    jump: bool,
    space: bool,
    run: bool,
    /// The right button held (captured): the mouse's motion swings.
    swing: bool,
}

/// A turn stopped at a question: the engine as it was before it, what it
/// was given, the answers given so far, and the question.
struct Pending {
    before: Engine,
    input: TurnIn,
    given: Vec<dialog::Answer>,
    need: dialog::Need,
    /// The spellmaker's screen behind the question, when the turn stopped
    /// inside it (`Talk::maker`).
    maker: Option<(dialog::Snapshot, Option<u16>)>,
    /// The shop's screen behind the question (`Talk::backdrop`).
    backdrop: Option<shopui::Backdrop>,
    /// The game's page with the question drawn on it (`Talk::need_page`).
    page: Option<std::sync::Arc<Vec<u8>>>,
}

/// A turn stopped inside its frame at a screen a menu key or the bar's
/// button opened (`dialog::Need::Screen`): the turn, run again once the
/// screen ends with the game as it left it (`App::screen_ended`).
struct Inside {
    before: Engine,
    input: TurnIn,
    given: Vec<dialog::Answer>,
}

struct App {
    /// How often the renderer was made again (`remake_renderer`; a
    /// headless bench keeps its own).
    remade: u32,
    /// The last standing turn's copy of the engine, the next turn's to
    /// fill (its allocations reused).
    spare: Option<Engine>,
    /// The sound driver (`audio`).
    audio: audio::Audio,
    /// The music driver (`music`), and the song the creation's screens
    /// asked for last.
    music: music::Music,
    creation_song: Option<u16>,
    /// Animations the game played, shown over it in turn (a key the next
    /// lines or the end); the game waits meanwhile.
    movies: std::collections::VecDeque<flc::Movie>,
    /// The automap shown (M).
    map: bool,
    assets: Assets,
    start: Instant,
    engine: Engine,
    gpu: Option<Gpu>,
    keys: Keys,
    look: f64,
    captured: bool,
    /// A click waiting for the next frame, and one left for the actors
    /// that the frame after it draws.
    click: bool,
    clicked: bool,
    /// Where the waiting click is on the 320 x 200 screen (None: the
    /// centre, as with the mouse captured).
    click_point: Option<(i32, i32)>,
    /// The click taken: the next frame's input step takes it (`TurnIn::click`).
    queued_click: Option<(i32, i32)>,
    /// A click on the bar's button: the frame it runs takes it in its
    /// input step (`TurnIn::bar`).
    queued_bar: Option<(i32, i32, bool)>,
    /// The keys let go while a turn waits on its question: its run again
    /// starts from the game before them, so they are let go again once it
    /// stands.
    released: Vec<u8>,
    /// The menu keys pressed that no frame has seen yet (none could run:
    /// a fade, a screen up), and whether let go since: held for the game
    /// till the next frame asks for them, as a key held to it would be.
    menu_held: Vec<(u8, bool)>,
    /// A right click in the view (the button let go with less of a drag
    /// than a swing needs, 0000:3C9F's 40h): looking at the square under
    /// the centre (2826:0C15) after the next frame; the drag so far.
    look_click: bool,
    right_drag: f64,
    /// The screen up as the ring of keys typed last saw it (`ring_screen`):
    /// none, a box alone (true) or another (false).
    ring_screen: Option<bool>,
    /// Camping asked for (R), and a key pressed (it ends a rest).
    camp: bool,
    key: bool,
    /// The message line last printed.
    message: Option<(u16, u16)>,
    next: Instant,
    /// How the last frame ended (reported when it changes).
    last: Option<arena::engine::Frame>,
    dir: PathBuf,
    /// The area's run, for the next.
    run: arena::transit::Run,
    /// Answers given before asking (`--picks`).
    script: std::collections::VecDeque<dialog::Answer>,
    /// A question waiting; boxes to read (the game waits for both); the
    /// menu's entry highlighted; the typed field asked, as typed so far
    /// (`field::Live`); the rest's hours.
    pending: Option<Pending>,
    /// The turn whose frame a screen is up inside (a menu key's, the
    /// bar's button's), to run again when it ends.
    inside: Option<Inside>,
    /// The press a screen ended on (the logbook's, the automap's exit),
    /// left in AFF9 by its loop's last read (0000:8FC4) for the rest of
    /// the frame; the next frame's read finds the button let go.
    screen_press: bool,
    /// The shop's screen behind the boxes a turn left after the shop ended
    /// (`Talk::backdrop`): the original's box (42EE) waits over the screen
    /// before the shop's way out (stealing, the guards called).
    box_backdrop: Option<shopui::Backdrop>,
    boxes: std::collections::VecDeque<hud::Said>,
    selected: usize,
    /// A list's first line shown (3159:06A0's +3Bh, 0 as it comes up,
    /// 0718), moved with `selected` by `hud::list_key`, `list_click`.
    list_top: usize,
    /// The mouse's reads and keys a list asked has taken so far, as its
    /// loop (3159:06A0) would read them (`list_read`); given the game's
    /// list with its answer (`dialog::Answer::List`).
    list_reads: Vec<arena::listwindow::ListRead>,
    /// The mouse's reads and the keys typed before each since the menu was
    /// asked, the menu's loop's (3159:0F2A), given the game's menu with
    /// its answer (`dialog::Answer::Menu`).
    menu_reads: Vec<arena::menubox::MenuRead>,
    /// A copy of the game as the list asked (or the spellbook's or the
    /// use list in the bar) started its loop (`arena::listwindow::
    /// ListCapture`): what the loop makes of the reads so far is what the
    /// window shows, and its end the list's answer (`list_followed`).
    list_live: Option<Box<arena::listwindow::ListCapture>>,
    /// The game's page as the list's loop has drawn it over the reads so
    /// far (`ListCapture::page`): what the window shows of the list.
    list_page: Option<std::sync::Arc<Vec<u8>>>,
    /// The window's own drawing composed even where a box shows the
    /// game's page (the screen tests' layer, whose mask is the original's).
    #[cfg(test)]
    own_drawing: bool,
    live: Option<field::Live>,
    status: Option<String>,
    /// `status` kept under the boxes of the turn that drew it last.
    status_held: bool,
    /// The effect's words the last frame drew (0000:5011).
    effect: Option<(Vec<u8>, i32)>,
    font: hud::Font,
    screen: hud::Screen,
    /// The cursor in the window, and the window's size (pixels).
    cursor: (f64, f64),
    size: (f64, f64),
    /// The mouse's motion with the right button held, since the last turn.
    drag: (f64, f64),
    /// Where saves go (F5) and come from (F9), and the slot.
    save_dir: Option<PathBuf>,
    slot: u8,
    /// A screen up over the game (I, C), which stops it; EQUIP.IMG once
    /// loaded.
    panel: Option<panels::Panel>,
    /// The character sheet's run up (`Machine::sheet_open`): B7C4 as it
    /// found it, for `sheet_close`.
    sheet_run: Option<u8>,
    /// The view's I pressed: the sheet's equipment page wanted once the
    /// sheet its portrait click opens is up (`inventory_key`).
    page_after_sheet: bool,
    /// The logbook's run up (`Machine::log_open`), for its loop's turns
    /// and `log_close`.
    log_run: Option<arena::logbook::LogRun>,
    /// The ring of keys as the sheet's end left it (its loop's flush and
    /// the key read, `Machine::sheet_close`): not forgotten again as the
    /// screen goes (`ring_screen`).
    ring_kept: bool,
    /// The status box up: its loop (2542:0530..0569) waits on the mouse
    /// and Esc's key alone, forgetting no keys typed, so the ring isn't
    /// forgotten as it comes up (`ring_screen`).
    ring_box_kept: bool,
    /// The status box is the box up: its loop (2542:0530..0569) leaves
    /// only on a click or on Esc's key held, so Enter and Space don't read
    /// it as they read the game's other boxes.
    status_box: bool,
    /// A journey just ended: the screen black (`fade_from`).
    journey_black: bool,
    /// The travel map up (`travel`).
    travel: Option<travel::Travel>,
    /// The save or load menu up (`saveui`: the options screen's Save and
    /// Load, the main menu's Load).
    saveui: Option<saveui::SaveUi>,
    /// The options screen the save menu was opened from (its Load or
    /// Save), drawn under it: LOADSAVE.IMG's 143 rows leave OP.IMG's last
    /// four.
    saveui_under: Option<options::OptionsUi>,
    /// The options screen up (`options`: Esc in play).
    options: Option<options::OptionsUi>,
    /// The options screen's Exit confirmed: the program ends.
    quit: bool,
    equip: Option<Option<interface::Image>>,
    /// Character creation under way (`--create`): its screens instead of
    /// the game, which starts when it ends. Also the main menu after a
    /// game's end, and the attribute screen of a level gained.
    creation: Option<create::Creation>,
    /// The engine those screens act on when it isn't the game's: for a
    /// level's points, a copy of the state the game asked in (the turn
    /// replays with the clicks).
    creation_engine: Option<Engine>,
    /// The game's state as the program's main leaves it at its menu
    /// after a game's end or the options screen's New (`title_engine`:
    /// 0000:109B..1114, `Machine::title_menu`): the menu's Load goes on
    /// from it (`open_saveui`), and so does creation from the menu's Start
    /// (`creation_from_title`).
    title_engine: Option<Engine>,
    /// Creation going on from `title_engine` (the menu's Start took it):
    /// the game then starts as the original's menu starts it.
    creation_from_title: bool,
    /// When the screens' movie last advanced.
    movie_at: Instant,
    /// A screen opening as the original opens it (`fade_open`): the
    /// colours at each wait for the retrace, one a retrace (14 ms), and
    /// whether the screen before is still shown (`fade_old`).
    fade: std::collections::VecDeque<(film::Dac, bool)>,
    fade_ms: u32,
    fade_old: Option<hud::Screen>,
    /// The game over: its farewell (`game_over`) once the world has faded
    /// out.
    ending: bool,
    /// The screen faded to black by the turn (`Talk::blackout`), for the
    /// area's change (`area_fade`).
    blackout: bool,
}

impl App {
    /// The movies all shown: the song before them back (1194:DB26).
    fn movies_over(&mut self) {
        // The next one's way in from what the one ended showed last.
        if let Some(mv) = self.movies.front_mut().filter(|m| m.lead_later) {
            mv.lead_from(&self.assets.shown, *self.screen.palette());
        }
        if self.movies.is_empty() {
            self.engine.m.music_resume(&mut self.engine.files);
            self.music.after(&mut self.engine.m, 0);
        }
    }

    /// The creation's song (`create::Creation::song`), started when it
    /// changes.
    fn creation_music(&mut self) {
        let Some(c) = &self.creation else {
            self.creation_song = None;
            return;
        };
        let song = c.song();
        if song != self.creation_song {
            self.creation_song = song;
            if let Some(s) = song {
                self.music.before(&mut self.engine.m);
                self.engine.m.music_song(&mut self.engine.files, s);
                self.music.after(&mut self.engine.m, 0);
            }
        }
    }

    /// A key or click for creation; at its end the game started, in the
    /// start dungeon, its assets made again.
    /// For tests: creation's change passed and a key left in the buffer
    /// taken (`Creation::settle`), as a script's keys come one a screen.
    #[cfg(test)]
    fn creation_settle(&mut self) {
        let Some(c) = &mut self.creation else { return };
        match &mut self.creation_engine {
            Some(e) => c.settle(&mut e.m),
            None => c.settle(&mut self.engine.m),
        }
    }

    fn creation_input(&mut self, inp: create::Input) {
        let Some(c) = &mut self.creation else { return };
        match &mut self.creation_engine {
            Some(e) => c.input(e, inp),
            None => c.input(&mut self.engine, inp),
        }
        // The menu's Start: creation goes on from the game as the program's
        // start left it at the menu (0000:1130), where there is one, its
        // 3D view drawn on as before.
        if c.take_started() {
            if let Some(mut t) = self.title_engine.take() {
                t.keep_drawing(&mut self.engine);
                self.engine = t;
                self.creation_from_title = true;
            }
        }
        self.creation_outcome();
    }

    /// What the screens up ended in: the game started (a character made),
    /// a game loaded (the main menu's Load), the program left (Exit), or
    /// a level's points answered (the turn replayed with the clicks).
    fn creation_outcome(&mut self) {
        let Some(outcome) = self.creation.as_ref().and_then(|c| c.outcome()) else { return };
        match outcome {
            create::Outcome::Play => {
                self.creation = None;
                self.title_engine = None;
                // From the menu's New on the program's own start: the
                // rest of it as the original's (`areas::new_game_from_menu`);
                // else (a start dungeon of the view's own) `newgame::play`.
                if std::mem::take(&mut self.creation_from_title) {
                    if areas::new_game_from_menu(&mut self.engine, &self.dir).is_none() {
                        self.say("The new game could not be started.");
                    }
                } else {
                    newgame::play(&mut self.engine);
                }
                self.run = arena::transit::Run::Start;
                self.assets = Assets::new(&self.dir, &mut self.engine);
                self.remake_renderer();
                self.last = Some(arena::engine::Frame::Running);
            }
            create::Outcome::Load => {
                // The load menu from the title (`saveui`), then the
                // title-screen load (`areas::load_game_menu`).
                if self.save_dir.is_some() {
                    self.open_saveui(saveui::Kind::TitleLoad);
                } else {
                    eprintln!("No save directory (--save-dir).");
                    if let Some(c) = &mut self.creation {
                        c.back_to_menu();
                    }
                }
            }
            create::Outcome::Exit => std::process::exit(0),
            create::Outcome::Spent(clicks) => {
                self.creation = None;
                self.creation_engine = None;
                self.answer(dialog::Answer::Spend(clicks));
            }
        }
    }

    /// The game over (0000:0CE9..0D3E): the world's colours, as last
    /// shown, faded to black in 8 steps over it (1194:55B2, the window's
    /// fade tinting the world), then the farewell (`game_over`).
    fn game_ending(&mut self) {
        let (queued, from, before) = self.fade_from();
        self.fade = queued;
        let mut film = film::Film::new(Vec::new(), from);
        film.fade(&[[0, 0, 0, 0xFF]; 256], 8);
        self.fade.extend(film.frames.into_iter().map(|(_, dac)| (dac, true)));
        self.fade_old = Some(before);
        self.ending = true;
    }

    /// The farewell after the fade (`game_ending`): the screen cleared,
    /// the vision played, then the intro and the main menu, creation's on
    /// a fresh engine (the start dungeon loaded, as a new game begins),
    /// the game's own state as the program's main takes it back to its
    /// menu kept for the menu's Load (`title_engine`).
    fn game_over(&mut self) {
        self.title_engine = title_engine(&self.engine);
        let archive = self.assets.archive.clone();
        let mut c = create::Creation::ending(&mut self.engine, archive, &self.dir, self.assets.palette, None);
        c.faded();
        let mut fresh = world::load(&self.dir, "START.MIF");
        fresh.files.disk_from(&self.engine.files);
        // The new game's 3D view drawn as the last game's was (its page;
        // the fresh machine is another's memory, so not its writes).
        fresh.page = self.engine.page.take();
        self.engine = fresh;
        self.creation = Some(c);
        self.movie_at = Instant::now();
    }

    /// Whether the game waits for the player (a box, a question).
    fn waiting(&self) -> bool {
        self.pending.is_some() || !self.boxes.is_empty()
    }

    /// The screen up over play, if one: a box alone (true: the original's
    /// wait for a key or a click, 1194:5536) or another (false: a menu, a
    /// list, a panel, the travel map, a movie, creation, the options).
    fn screen_up(&self) -> Option<bool> {
        let other = self.pending.is_some() || self.panel.is_some() || self.creation.is_some() || !self.movies.is_empty() || self.travel.is_some() || self.saveui.is_some() || self.options.is_some();
        if other {
            Some(false)
        } else if !self.boxes.is_empty() {
            Some(true)
        } else {
            None
        }
    }

    /// The ring of keys typed (AC4C) as the screens leave it: each of the
    /// original's screens forgets it as it begins (0000:7112, from the
    /// menus 3159:002F, the boxes 1194:5536, the travel map 26F7:03BE, the
    /// movies 2294:0701 and the others); its menus, lists and panels read
    /// the keys typed while they are up out of it (0000:7138), so it is
    /// empty as they end; a box only waits for one (1194:5545), which is
    /// left in it. Called before each input and after each tick, so a
    /// screen's beginning is seen before the next key.
    fn ring_screen(&mut self) {
        // (The logbook neither forgets the ring nor reads it: 27E3:01FC
        // asks only Esc's key down, so the key that opened it stays.)
        let log = matches!(self.panel, Some(panels::Panel::Log(_))) && self.pending.is_none() && self.boxes.is_empty();
        let now = if log { self.ring_screen } else { self.screen_up() };
        if std::mem::take(&mut self.ring_kept) && now.is_none() {
            self.ring_screen = now;
            return;
        }
        if now == Some(true) && std::mem::take(&mut self.ring_box_kept) {
            self.ring_screen = now;
            return;
        }
        if now != self.ring_screen && !(self.ring_screen == Some(true) && now.is_none()) {
            self.engine.m.key_flush();
        }
        self.ring_screen = now;
    }

    /// One engine turn with the keys and the mouse as an Intent.
    fn step(&mut self) {
        if self.screen_up().is_some() || !self.fade.is_empty() || self.inside.is_some() {
            return;
        }
        // The press a screen ended on let go by this frame's read.
        if std::mem::take(&mut self.screen_press) {
            self.engine.m.mem.w8(arena::mem::DS, arena::controls::var::BUTTONS, 0);
        }
        let k = &self.keys;
        let axis = |p: bool, n: bool| (p as i16 - n as i16) * 256;
        let look = self.look.round() as i16;
        self.look -= look as f64;
        let sideways = axis(k.turn_right, k.turn_left);
        let intent = Intent {
            forward: axis(k.forward, k.back),
            strafe: if k.ctrl { sideways } else { 0 },
            turn: if k.ctrl { 0 } else { sideways },
            look,
            jump: k.jump || k.space,
            run: k.run,
        };
        let tick = (self.start.elapsed().as_secs_f64() * 18.2) as usize;
        let drag = std::mem::take(&mut self.drag);
        let input = TurnIn {
            intent,
            clicked: std::mem::take(&mut self.clicked),
            camp: std::mem::take(&mut self.camp),
            shop: None,
            loot: None,
            talk: false,
            key: std::mem::take(&mut self.key),
            tick,
            right: self.keys.swing,
            drag: (drag.0.round() as i16, drag.1.round() as i16),
            aspect: (self.size.0 / self.size.1.max(1.0)) as f32,
            note: None,
            click: self.queued_click.take(),
            bar: self.queued_bar.take(),
        };
        // The engine before the turn, for a question to run it again: into
        // the last standing turn's copy, its allocations reused.
        let before = match self.spare.take() {
            Some(mut e) => {
                copy_into(&mut e, &self.engine);
                e
            }
            None => copy(&self.engine),
        };
        self.turn(before, input, Vec::new(), true);
        // The menu keys held for it seen: those let go, let go now.
        for (scan, gone) in std::mem::take(&mut self.menu_held) {
            if gone {
                self.let_go(scan);
            }
        }
        // A click now is the next frame's: its input step takes it (as the
        // original's controls read the mouse), after that frame's actors.
        if !self.waiting() && std::mem::take(&mut self.click) {
            self.queued_click = Some(self.click_point.take().unwrap_or((164, 74)));
        }
        if !self.waiting() && std::mem::take(&mut self.look_click) {
            look_centre(&mut self.engine);
        }
    }

    /// The turn `input` run from `before` with the answers `given` (and
    /// the script's): stopped at a question, it waits for its answer;
    /// otherwise it stands, and its frame's end is followed. `fresh`: the
    /// engine is `before` already (a new turn), not to be copied back.
    fn turn(&mut self, before: Engine, input: TurnIn, given: Vec<dialog::Answer>, fresh: bool) {
        let mut answers: std::collections::VecDeque<dialog::Answer> = given.iter().cloned().collect();
        let scripted = self.script.len();
        answers.extend(self.script.iter().cloned());
        if !fresh {
            copy_into(&mut self.engine, &before);
        }
        let mut talk = dialog::Talk { answers, bar: input.bar, ..Default::default() };
        self.audio.before(&mut self.engine.m);
        self.music.before(&mut self.engine.m);
        let ticks = self.engine.frame_ticks() as u32;
        let out = run_turn(&mut self.engine, &mut self.assets, &input, &mut talk);
        // (A turn stopped at a question runs again: the rerun's.)
        self.blackout |= talk.blackout && talk.need.is_none();
        // A turn stopped at a question is run again once answered: its
        // sounds and songs are the rerun's.
        if talk.need.is_none() {
            self.audio.after(&mut self.engine.m);
            let played = std::mem::take(&mut talk.movies);
            let archive = self.assets.archive.clone();
            let from = Some((&self.assets.shown[..], *self.screen.palette()));
            let mv = movies_of(&mut self.engine, &archive, played, from);
            self.movies.extend(mv);
            self.music.after(&mut self.engine.m, ticks);
        } else {
            self.engine.m.songs.clear();
        }
        // The script's answers used become given ones.
        let used = (given.len() + scripted).saturating_sub(talk.answers.len());
        let mut given = given;
        while given.len() < used {
            match self.script.pop_front() {
                Some(a) => given.push(a),
                None => break,
            }
        }
        if !talk.since_answer.is_empty() {
            self.box_backdrop = talk.backdrop.take();
        }
        // The rest's hours drawn in the turn whose boxes end it stay
        // under them: the original's page keeps them while its box waits.
        self.status_held = talk.status.is_some() && !talk.since_answer.is_empty();
        self.boxes.extend(talk.since_answer);
        if talk.status.is_some() {
            self.status = talk.status;
        }
        if let Some(dialog::Need::Screen(t, w)) = talk.need {
            // A screen inside the frame: opened on the game as the frame
            // asked for it; the turn runs again as it ends, the game it
            // leaves taken on there (`screen_ended`).
            self.engine.m.clone_from(&w.0 .0);
            self.engine.files.clone_from(&w.0 .1);
            self.inside = Some(Inside { before, input, given });
            self.open_taken(t);
            self.screen_ended();
            return;
        }
        if let Some(need) = talk.need {
            self.list_live = self.engine.m.list_captured.take();
            self.list_page = self.list_live.as_ref().and_then(|l| l.page(&[])).map(std::sync::Arc::new);
            self.engine.m.list_capture = false;
            copy_into(&mut self.engine, &before);
            self.selected = 0;
            self.list_top = 0;
            self.list_reads.clear();
            self.menu_reads.clear();
            // A typed field: the game's editor opened on a copy of the
            // game as it asked.
            self.live = match &need {
                dialog::Need::Text(a) => field::Live::open(&a.game.0, a.field, a.look),
                _ => None,
            };
            let mut forced = None;
            if let dialog::Need::Travel(w) = &need {
                // The travel map from the game as the way out showed it.
                self.engine.m.clone_from(&w.0 .0);
                self.engine.files.clone_from(&w.0 .1);
                forced = Some(travel::Travel::forced(&mut self.engine, &self.assets.archive));
            }
            // The spellmaker's screen with its colours set (1194:6064,
            // 0000:57DD); the travel map as 26F7:0000 opens it, the screen
            // faded out in 8 steps, its colours in in 6 (as `open_travel`).
            // (A level's attribute screen fades in after its notice,
            // `box_read`.)
            let fade = match &need {
                dialog::Need::Spellmaker => Some((0, 0)),
                dialog::Need::Travel(_) => Some((8, 6)),
                _ => None,
            };
            let maker = talk.maker.take();
            let backdrop = talk.backdrop.take();
            let page = talk.need_page.take();
            let take = move |a: &mut App| {
                if forced.is_some() {
                    a.travel = forced;
                }
                if let dialog::Need::Spend(snap) = &need {
                    let mut e = Engine::new((*snap.0).clone(), a.engine.files.clone());
                    let c = create::Creation::level_up(&mut e.m, a.assets.archive.clone(), &a.dir, a.assets.palette);
                    a.creation = Some(c);
                    a.creation_engine = Some(e);
                }
                a.pending = Some(Pending { before, input, given, need, maker, backdrop, page });
            };
            match fade {
                Some((steps, fade_in)) => self.fade_open(steps, fade_in, take),
                None => take(self),
            }
            if self.captured {
                self.release();
            }
            return;
        }
        // The turn stands: its copy is the next turn's to fill.
        self.spare = Some(before);
        for scan in std::mem::take(&mut self.released) {
            self.key_up(scan);
        }
        print_message(&self.engine, &mut self.message);
        self.effect = out.effect.clone();
        if out.level_changed {
            self.assets.level(&self.engine);
            self.remake_renderer();
        }
        let frame = out.frame;
        if frame != self.last {
            match frame {
                Some(arena::engine::Frame::Running) => {}
                Some(arena::engine::Frame::Left) => {
                    if self.area_left() {
                        return;
                    }
                    if self.engine.m.mem.r16(arena::mem::DS, 0x05CA) & 0x8000 != 0 {
                        self.new_game_menu();
                        return;
                    }
                }
                None => eprintln!("the frame ended: None ({})", arena::giveup::describe()),
            }
            self.last = frame;
        }
    }

    /// A screen up inside a turn's frame (`inside`) ended, nothing else
    /// up: the turn runs again, the game as the screen left it taken on
    /// where the frame asked for it, and the rest of the frame after.
    /// (After the screen's last fade: the original's frame goes on once
    /// the screen's way out is drawn, its keys let go by then.)
    fn screen_ended(&mut self) {
        if self.inside.is_none() || self.screen_up().is_some() || self.map || !self.fade.is_empty() {
            return;
        }
        let Inside { before, input, mut given } = self.inside.take().unwrap();
        given.push(dialog::Answer::Screen(dialog::World(Box::new((self.engine.m.clone(), self.engine.files.clone())))));
        self.turn(before, input, given, false);
    }

    /// The answer to the question waiting: its turn runs again.
    fn answer(&mut self, a: dialog::Answer) {
        let Some(p) = self.pending.take() else { return };
        let mut given = p.given;
        given.push(a);
        self.turn(p.before, p.input, given, false);
    }

    /// The 320 x 200 layer of a redraw (the HUD, a screen up, an
    /// animation), and the camera the world is drawn from.
    fn compose(&mut self) -> Camera {
        let cam = camera(&self.engine);
        let hud = prof::time("redraw.hud", || self.hud());
        let t = std::time::Instant::now();
        self.screen.clear();
        if let Some(mv) = self.movies.front() {
            mv.draw(&mut self.screen, &self.font);
        } else if let Some(ui) = &self.saveui {
            self.screen.set_palette(self.assets.dac());
            // From the options screen's Load or Save (2542:061E, 0617):
            // LOADSAVE.IMG's 143 rows over OP.IMG's 147, the options
            // screen's last rows left under it.
            if let Some(o) = &self.saveui_under {
                self.assets.interface.draw(&mut self.screen, &self.font, &self.engine.m);
                o.draw(&mut self.screen, &self.font);
            }
            ui.draw(&mut self.screen);
        } else if let Some(ui) = &self.options {
            // OP.IMG over what was on the screen (the interface below it).
            self.screen.set_palette(self.assets.dac());
            self.assets.interface.draw(&mut self.screen, &self.font, &self.engine.m);
            ui.draw(&mut self.screen, &self.font);
        } else if let Some(c) = self.creation.as_ref().filter(|_| !self.level_notice()) {
            c.draw(&mut self.screen, &self.creation_engine.as_ref().unwrap_or(&self.engine).m);
        } else if let Some(page) = self.box_page_shown() {
            // A box over play the engine drew on the game's page (which
            // holds the 3D view and the interface as the original's does):
            // the page as the box's wait found it, all the original shows.
            self.screen.set_palette(self.assets.dac());
            self.screen.underlay(&page);
        } else if let Some((snap, form)) = self.pending.as_ref().and_then(|p| p.maker.as_ref()) {
            // Inside the guild's spellmaker: its screen, the question asked
            // (a list, the name) over it.
            spellmaker::draw_state(&mut self.screen, &self.font, &self.assets.archive, &self.engine.files, &snap.0, *form);
            hud::compose_over(&mut self.screen, &self.font, &hud);
        } else if let Some(b) = self.pending.as_ref().and_then(|p| p.backdrop.as_ref()).or(self.box_backdrop.as_ref().filter(|_| !self.boxes.is_empty())) {
            // Inside a shop: its screen and banner, the question (or the
            // boxes it left) over it.
            self.screen.set_palette(self.assets.dac());
            b.draw(&mut self.screen, &self.assets.archive, &self.engine.m);
            hud::compose_over(&mut self.screen, &self.font, &hud);
        } else if let Some(t) = &self.travel {
            let (x, y) = self.screen_point();
            t.draw(&mut self.screen, &self.font, &mut self.engine.m, x, y);
            // Its boxes in its own tiles (`Travel::draw_box`).
            let boxed = hud.boxed.clone();
            hud::compose_over(&mut self.screen, &self.font, &hud::Hud { boxed: None, ..hud });
            if let Some(lines) = boxed {
                t.draw_box(&mut self.screen, &self.font, &lines);
            }
        } else {
            let sheet = self.panel.as_ref().is_some_and(|p| !p.over_play());
            self.screen.set_palette(if sheet { self.assets.charsheet } else { self.assets.dac() });
            // The original's 3D view (the engine's page) under the
            // interface, over the view's own world; where the renderer
            // reached a path not ported (outdoors, the sky, water...),
            // the view's own world shows instead.
            if let (false, Some(page), true) = (sheet, &self.engine.page, self.engine.page_gaps.is_empty()) {
                self.screen.underlay(page);
            }
            self.assets.interface.draw(&mut self.screen, &self.font, &self.engine.m);
            if self.panel.is_some() {
                self.equip_image();
                let equip = self.equip.as_ref().and_then(|e| e.as_ref());
                let doll = doll::images(&self.assets.archive, &self.engine.m);
                panels::draw(&mut self.screen, &self.font, equip, &doll, self.panel.as_ref().unwrap(), &self.engine.m, Some((&self.assets.archive, &self.engine.files)));
            }
            hud::compose_over(&mut self.screen, &self.font, &hud);
            if self.map {
                automap::draw(&mut self.screen, &mut self.engine.m);
                automap::draw_notes(&mut self.screen, &self.engine);
                let (x, y) = self.screen_point();
                automap::draw_pointer(&mut self.screen, x, y);
            }
        }
        // The typed field asked, over what its caller shows (its prompt's
        // box or lines are the HUD's; boxes to read cover it).
        if self.movies.is_empty() && self.boxes.is_empty() {
            if let Some(l) = &self.live {
                l.draw(&mut self.screen, false);
            }
        }
        // A screen opening: the screen before while it fades, then the new
        // one's colours set.
        if let Some(&(dac, old)) = self.fade.front() {
            if let Some(o) = self.fade_old.as_ref().filter(|_| old) {
                self.screen = o.clone();
            }
            self.screen.recolour(dac);
        }
        self.assets.shown.clear();
        self.assets.shown.extend(self.screen.indices().iter().map(|&c| c as u8));
        if prof::on() {
            prof::add("redraw.screen", t.elapsed());
        }
        cam
    }

    /// The page as the save menu leaves it when the game is written
    /// (25FD:0110, A8FE): the interface, the options screen and the menu
    /// over it, as `compose` draws them, and "Kindly waiteth..." (0803:
    /// the text at 5F26 in the fonts and colour it selects) boxed over
    /// them.
    fn save_shot(&mut self) -> Vec<u8> {
        let mut s = hud::Screen::new(self.assets.palette);
        s.set_palette(self.assets.dac());
        if let Some(o) = &self.saveui_under {
            self.assets.interface.draw(&mut s, &self.font, &self.engine.m);
            o.draw(&mut s, &self.font);
        }
        if let Some(ui) = &self.saveui {
            ui.draw(&mut s);
        }
        let m = &self.engine.m;
        let lines = hud::in_font(dialog::text_at(m, arena::savemenu::var::WAIT, m.mem.r8(arena::mem::DS, 0x9288)), hud::GameFont::selected(m));
        // Centred in the clip it's asked in (965A), as play left it: the
        // 3D view's after a wilderness dungeon's way in.
        let r = |o: u16| m.mem.r16(arena::mem::DS, o) as i16 as i32;
        let lines = hud::clipped(lines, (r(0xA914), r(0xA916), r(0xA918), r(0xA91A)));
        hud::compose_over(&mut s, &self.font, &hud::Hud { boxed: Some(lines), ..Default::default() });
        s.indices().iter().map(|&c| c as u8).collect()
    }

    fn remake_renderer(&mut self) {
        self.remade += 1;
        if let Some(g) = &mut self.gpu {
            self.assets.reload_renderer(&mut g.renderer, &g.device, &g.queue);
        }
    }

    fn release(&mut self) {
        if let Some(g) = &self.gpu {
            let _ = g.window.set_cursor_grab(winit::window::CursorGrabMode::None);
            g.window.set_cursor_visible(true);
        }
        self.captured = false;
    }

    /// F5: the game saved to the slot in the save directory.
    fn save(&mut self) {
        let msg = match self.save_dir.clone() {
            Some(dir) => {
                // The port's own key: the screen as the window shows it.
                let shot: Vec<u8> = self.screen.indices().iter().map(|&c| c as u8).collect();
                save_to(&mut self.engine, &dir, self.slot, self.assets.dac(), Some(&shot))
            }
            None => "No save directory (--save-dir).".to_string(),
        };
        self.say(&msg);
    }

    /// The save or load menu opened on the engine (`saveui`).
    fn open_saveui(&mut self, kind: saveui::Kind) {
        self.saveui_under = None;
        let Some(dir) = self.save_dir.clone() else {
            self.say("No save directory (--save-dir).");
            return;
        };
        let base = match (kind, self.title_engine.as_mut()) {
            (saveui::Kind::TitleLoad, Some(e)) => {
                title_load_setup(e);
                &*e
            }
            _ => &self.engine,
        };
        let (ui, e) = saveui::SaveUi::open(kind, base, &dir, &self.assets.archive);
        self.saveui = Some(ui);
        self.saveui_ended(e);
    }

    /// Slot `slot` of `dir` loaded on `engine`, the load menu's copy of
    /// the machine (in play `areas::load_game`, from the title
    /// `load_game_menu`), with the drawing of the engine it stands in
    /// for, which the load carries on (`Engine::keep_drawing`) and a load
    /// that fails gives back.
    fn load_slot(&mut self, engine: &mut Engine, dir: &std::path::Path, slot: u8, title: bool) -> Result<arena::transit::Run, String> {
        let game = self.dir.clone();
        let under = match (title, self.title_engine.as_mut()) {
            (true, Some(t)) => t,
            _ => &mut self.engine,
        };
        engine.keep_drawing(under);
        let r = if title { areas::load_game_menu(engine, &game, dir, slot) } else { areas::load_game(engine, &game, dir, slot) };
        if r.is_err() {
            let under = match (title, self.title_engine.as_mut()) {
                (true, Some(t)) => t,
                _ => &mut self.engine,
            };
            under.keep_drawing(engine);
        }
        r
    }

    /// The game `load_slot` loaded played on: its area's run, its assets.
    fn game_loaded(&mut self, engine: Engine, run: arena::transit::Run, slot: u8) {
        self.engine = engine;
        self.title_engine = None;
        self.creation = None;
        self.run = run;
        self.assets = Assets::new(&self.dir, &mut self.engine);
        self.remake_renderer();
        self.last = Some(arena::engine::Frame::Running);
        // As the game's menu: play goes on without a word.
        println!("Loaded slot {slot}.");
    }

    /// What the menu's last input ended in: still open, a slot to save
    /// (`save_to` on the engine after the menu), a slot to load
    /// (in play `areas::load_game`, from the title `load_game_menu`), or
    /// the menu left (a title's back to the main menu).
    fn saveui_ended(&mut self, e: saveui::Ended) {
        let Some(kind) = self.saveui.as_ref().map(|u| u.kind) else { return };
        let Some(dir) = self.save_dir.clone() else { return };
        let title = kind == saveui::Kind::TitleLoad;
        match e {
            saveui::Ended::Open => return,
            saveui::Ended::Save(engine, slot) => {
                self.engine.m.clone_from(&engine.m);
                self.engine.files.clone_from(&engine.files);
                // The game's menu says nothing once the game is saved (only
                // its wait, 25FD:0803, while the disk is used); a failure
                // the original wouldn't have is told.
                let shot = self.save_shot();
                let msg = save_to(&mut self.engine, &dir, slot, self.assets.dac(), Some(&shot));
                if msg.starts_with("Saved to slot") {
                    println!("{msg}");
                } else {
                    self.say(&msg);
                }
            }
            saveui::Ended::Load(mut engine, slot) => {
                let r = self.load_slot(&mut engine, &dir, slot, title);
                match r {
                    Ok(run) => self.game_loaded(engine, run, slot),
                    // An empty slot: the game's message box over the
                    // menu, then its picker again (25FD:028D, 026B).
                    Err(msg) if msg.as_bytes() == engine.m.mem.cstr(arena::mem::DS, arena::load::var::NO_SAVE) => {
                        println!("{msg}");
                        if let Some(ui) = &mut self.saveui {
                            ui.empty_slot(&engine.m);
                        }
                        return;
                    }
                    Err(msg) => {
                        eprintln!("{msg}");
                        self.say(&msg);
                        if let Some(c) = &mut self.creation {
                            c.back_to_menu();
                        }
                    }
                }
            }
            saveui::Ended::Left(mut engine, msg) => {
                // The game goes on as the menu left it (a load menu left
                // has deleted log.64, re*.64 and in*.64), its drawing
                // carried on; the title's, from the main menu after a
                // game's end, kept for its Load.
                if let (true, Some(t)) = (title, self.title_engine.as_mut()) {
                    engine.keep_drawing(t);
                    self.title_engine = Some(*engine);
                } else {
                    engine.keep_drawing(&mut self.engine);
                    self.engine = *engine;
                }
                if let Some(m) = msg {
                    self.say(&m);
                }
                if let Some(c) = &mut self.creation {
                    c.back_to_menu();
                }
            }
        }
        self.saveui = None;
    }

    /// The options screen's New, once the frame after it left the area
    /// and the outer loop went its way out (`areas::next`, 5CA bit
    /// 8000h): the intro and the main menu, creation's on a fresh engine
    /// (the start dungeon loaded, as a new game begins), the game's own
    /// state as the program's main takes it back to its menu kept for the
    /// menu's Load (`title_engine`).
    fn new_game_menu(&mut self) {
        self.title_engine = title_engine(&self.engine);
        let archive = self.assets.archive.clone();
        let c = create::Creation::main_menu(&mut self.engine, archive, &self.dir, self.assets.palette);
        let mut fresh = world::load(&self.dir, "START.MIF");
        fresh.files.disk_from(&self.engine.files);
        // The new game's 3D view drawn as the last game's was (its page;
        // the fresh machine is another's memory, so not its writes).
        fresh.page = self.engine.page.take();
        self.engine = fresh;
        self.creation = Some(c);
        self.movie_at = Instant::now();
    }

    /// Esc: the options screen opened on the engine (`options`).
    fn open_options(&mut self) {
        let (ui, e) = options::OptionsUi::open(&self.engine, &self.assets.archive);
        self.options = Some(ui);
        self.options_ended(e);
    }

    /// What the options screen's last input ended in: still open, or left
    /// with the engine after it (the volumes and the detail as set); then
    /// Save and Load open the game's menus, New goes to the main menu (5CA
    /// bit 8000h: the program's outer loop, 0000:109B, as a game's end
    /// without its vision), Exit ends the program.
    fn options_ended(&mut self, e: options::Ended) {
        let options::Ended::Left(engine, end) = e else { return };
        let under = self.options.take();
        // The game as the screen left it, on this engine (its page and the
        // drawing's record of what it wrote kept). The keys went to this
        // engine while the screen was up: their state is taken on (the
        // key interrupt's table, ACB5), their latches (9208) left as the
        // frame's menu keys set them: a key let go has its latch cleared
        // by the next check that asks it (2542:05D1), not at the screen's
        // end.
        let keys = arena::controls::var::KEYS;
        let held: Vec<u8> = (0..0x80u16).map(|scan| self.engine.m.mem.r8(arena::mem::DS, keys + scan)).collect();
        self.engine.m.clone_from(&engine.m);
        self.engine.files.clone_from(&engine.files);
        for (scan, &down) in held.iter().enumerate().skip(1) {
            self.engine.m.mem.w8(arena::mem::DS, keys + scan as u16, down);
        }
        match end {
            arena::options::OptionsEnd::Resume => {}
            arena::options::OptionsEnd::Save => {
                self.open_saveui(saveui::Kind::Save);
                self.saveui_under = under;
            }
            arena::options::OptionsEnd::Load => {
                self.open_saveui(saveui::Kind::Load);
                self.saveui_under = under;
            }
            // New left the area (C398 bit 80h, 5CA bit 8000h): the rest
            // of the frame the screen opened in (`screen_ended`) ends the
            // world loop, and the outer loop's way out takes the game to
            // the main menu (`new_game_menu`).
            arena::options::OptionsEnd::NewGame => {}
            // (21EE:01ED: the program ends there, nothing of the frame
            // after it.)
            arena::options::OptionsEnd::Quit => {
                self.inside = None;
                self.quit = true;
            }
        }
    }

    /// F9: the slot's game loaded back (`load_from`).
    fn load(&mut self) {
        let Some(dir) = self.save_dir.clone() else {
            self.say("No save directory (--save-dir).");
            return;
        };
        match load_from(&mut self.engine, &self.dir.clone(), &dir, self.slot) {
            Ok((msg, run)) => {
                self.run = run;
                self.assets = Assets::new(&self.dir, &mut self.engine);
                self.remake_renderer();
                self.say(&msg);
            }
            Err(msg) => self.say(&msg),
        }
    }

    /// The cursor on the 320 x 200 screen.
    fn screen_point(&self) -> (i32, i32) {
        let (w, h) = (self.size.0.max(1.0), self.size.1.max(1.0));
        ((self.cursor.0 / w * hud::W as f64) as i32, (self.cursor.1 / h * hud::H as f64) as i32)
    }

    /// The map button (Shift+M, its right click): the travel map, or the
    /// box refusing it.
    fn open_travel(&mut self) {
        let dac = self.assets.dac().map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]);
        match travel::Travel::open(&mut self.engine, &self.assets.archive, Some(&dac)) {
            Ok(t) => {
                // 26F7:0000: the screen faded out in 8 steps, the map's
                // colours faded in in 6.
                self.fade_open(8, 6, |a| a.travel = Some(t));
                if self.captured {
                    self.release();
                }
            }
            Err(b) => self.boxes.push_back(b.into()),
        }
        self.travel_after();
    }

    /// An input to the travel map, then its boxes shown and its end
    /// followed (`travel::after`).
    fn travel_input(&mut self, f: impl FnOnce(&mut travel::Travel, &mut Engine, &archive::Archive)) {
        let Some(shown) = self.travel.as_ref().map(|t| t.shown()) else { return self.travel_after() };
        let was = self.fade_from();
        let pic = self.travel.as_ref().is_some_and(|t| t.picture_up());
        if let Some(t) = &mut self.travel {
            f(t, &mut self.engine, &self.assets.archive);
            // A journey made: its fades from the screen as the click found
            // it (26F7:081E's 55B2 on).
            if t.journeying() {
                t.journey_from(&was.2);
            }
        }
        let pic_now = self.travel.as_ref().is_some_and(|t| t.picture_up());
        if pic != pic_now {
            if pic_now {
                // A main quest's dungeon reached (292B:0362): the map faded
                // out in 8 steps (26F7:00CE's 1194:55B2, the picture's own
                // 55B2 then black to black), the picture in in 12 (5E3E).
                self.fade_to(was, 8, 12);
            } else {
                // Its button: faded out in 8 (55B2), the area left from
                // black.
                let (queued, from, before) = was;
                self.fade = queued;
                let mut film = film::Film::new(Vec::new(), from);
                film.fade(&[[0, 0, 0, 0xFF]; 256], 8);
                self.fade.extend(film.frames.into_iter().map(|(_, dac)| (dac, true)));
                self.fade_old = Some(before);
            }
            return self.travel_after();
        }
        match (shown, self.travel.as_ref().and_then(|t| t.shown_now())) {
            // 26F7:0176: a province's map opened, the world map faded out
            // in 8 steps (1194:55B2) and the map's colours set, twice (its
            // picture's two loads, 0000:56C8 with BX 84h, then 4: 9B56 again,
            // whose first wait shows them whole).
            (None, Some(Some(_))) => {
                self.fade_to(was, 8, 0);
                let to = *self.screen.palette();
                self.fade.push_back((to, false));
            }
            // Its Exit (26F7:0231..0251): the screen cleared (9426) and the
            // world map's window shown again (3159:0494 on DATA:6C56),
            // faded out in 6 steps and its colours in in 6.
            (Some(_), Some(None)) => self.fade_to(was, 6, 6),
            _ => self.fade = was.0,
        }
        self.travel_after();
    }

    fn travel_after(&mut self) {
        let mut boxes = Vec::new();
        // Shown on the way out of a dungeon (a turn's question): its end
        // is the answer, the turn run again with the game as it ended.
        let forced = matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Travel(_)));
        let journeyed = self.travel.as_ref().is_some_and(|t| t.journeyed);
        let carry = travel::after(&mut self.travel, &mut self.engine, &mut boxes, !forced);
        self.boxes.extend(boxes.into_iter().map(hud::Said::from));
        // After a journey the screen is black (26F7:08F1's 55B2): the area's
        // change starts from there.
        self.journey_black |= journeyed && carry.is_some();
        // (Opened by the map button inside the frame: its end runs the
        // rest of that frame (`screen_ended`), which a journey's C398 bit
        // 80h (2542:0434) ends the world loop at, 0000:A453.)
        if let (true, Some(carry)) = (forced, carry) {
            // The key that ended the screen is the screen's: the turn runs
            // again at once, while it is down, so its latch is set (as
            // 2542:05B8 sets it), and the frame's body after the screen
            // doesn't take it for a menu key (the player's would be let
            // go by then). Only the menu keys' latches: the frame asks no
            // other's (an arrow held into the stairs keeps its own).
            use arena::menukeys::key;
            for scan in [key::ESC, key::A, key::C, key::P, key::S, key::U, key::M, key::F1, key::F8, key::L, key::R] {
                if self.engine.m.mem.r8(arena::mem::DS, arena::controls::var::KEYS + scan) != 0 {
                    self.engine.m.key_pressed(scan);
                }
            }
            let world = dialog::World(Box::new((self.engine.m.clone(), self.engine.files.clone())));
            self.answer(dialog::Answer::Travel(world, carry));
        }
    }

    /// The view's I: the original's own inputs for the equipment page, the
    /// bar's portrait clicked (as K) and N typed once the sheet is up.
    fn inventory_key(&mut self) {
        let (x, y) = sheets::bar_centre(8);
        // (Set first: the frame the click runs may open the sheet at once.)
        self.page_after_sheet = true;
        if !self.bar(x, y, false) || !matches!(self.panel, None | Some(panels::Panel::Sheet)) {
            self.page_after_sheet = false;
        }
    }

    /// EQUIP.IMG, loaded the first time.
    fn equip_image(&mut self) {
        if self.equip.is_none() {
            self.equip = Some(panels::load_image(&self.assets.archive, &self.engine.m, "EQUIP.IMG"));
        }
    }

    /// A key with a panel up: see `panels`.
    fn panel_key(&mut self, code: winit::keyboard::KeyCode, text: Option<&str>) {
        use winit::keyboard::KeyCode;
        let mut said = panels::Said::default();
        if matches!(self.panel, Some(panels::Panel::Use(..) | panels::Panel::Spellbook(_))) {
            said.screen = Some(self.list_page());
            // The key as the list's loop reads it, held and let go.
            use arena::listwindow::list_keys::{ENTER, ESC};
            match code {
                KeyCode::Enter | KeyCode::NumpadEnter => self.list_tap(ENTER.0),
                KeyCode::Escape => self.list_tap(ESC.0),
                _ => {
                    if let Some(k) = list_key_of(code) {
                        self.list_key_read(k);
                    }
                }
            }
            self.engine.m.list_reads = Some(self.list_reads.clone());
            if self.bar_list_followed(&mut said) {
                return;
            }
        }
        let m = &mut self.engine.m;
        let mut cast = None;
        let mut page = false;
        let mut back: Option<Option<u8>> = None;
        let mut sheet_drop = None;
        let mut log_poll = false;
        let mut page_key = None;
        match self.panel.as_mut() {
            Some(panels::Panel::Inventory(inv)) => {
                // A spell's page waits for clicks (51D1's 8FC4 loop); its
                // Delete question takes its keys too (020E: Y, N at 4B60;
                // Esc leaves it).
                if let Some(page) = inv.page.clone() {
                    let a = match code {
                        KeyCode::KeyY if page.asking.is_some() => panels::PageAnswer::Confirm(0),
                        KeyCode::KeyN if page.asking.is_some() => panels::PageAnswer::Confirm(1),
                        KeyCode::Escape if page.asking.is_some() => panels::PageAnswer::Confirm(0xFFFF),
                        _ => return,
                    };
                    self.page_answer(page, a);
                    return;
                }
                // "Drop what?" waits in its loop (1194:2325, 5DE2): any key
                // typed is its turn's (in the ring whether or not one of
                // its first five records takes it: the list's Space, the
                // arrows', the drop's D and the exit's Esc).
                if inv.drop_what.is_some() && inv.asking.is_none() {
                    let c = match code {
                        KeyCode::NumpadAdd | KeyCode::Equal => Some(b'+'),
                        KeyCode::Minus | KeyCode::NumpadSubtract => Some(b'-'),
                        KeyCode::Escape => Some(0x1B),
                        _ => text.and_then(|t| t.bytes().next()).filter(u8::is_ascii),
                    };
                    if let Some(c) = c {
                        self.sheet_drop_what(arena::charsheet::PageInput::Key(c));
                    }
                    return;
                }
                // An item's description waits for a key or a click
                // (1194:5536: ACB0 or AFF9), which does nothing more.
                if inv.info.take().is_some() {
                    inv.info_y = None;
                    return;
                }
                if let Some((_, sel)) = inv.asking.as_mut() {
                    // The drop's yes or no (020E: its keys Y and N, Esc no).
                    let answer = match code {
                        // (No entry marked till the arrows move one.)
                        KeyCode::ArrowUp | KeyCode::ArrowDown => {
                            *sel = if *sel == 0 { 1 } else { 0 };
                            None
                        }
                        KeyCode::Enter | KeyCode::NumpadEnter if *sel < 2 => Some(*sel == 0),
                        KeyCode::KeyY => Some(true),
                        KeyCode::KeyN | KeyCode::Escape => Some(false),
                        _ => None,
                    };
                    if let Some(yes) = answer {
                        inv.asking = None;
                        inv.asked_menu = None;
                        let typed = match code {
                            KeyCode::KeyY => Some(b'y'),
                            KeyCode::KeyN => Some(b'n'),
                            KeyCode::Escape => Some(0x1B),
                            KeyCode::Enter | KeyCode::NumpadEnter => Some(0x0D),
                            _ => None,
                        };
                        sheet_drop = Some((Some(b'd'), Some((yes, typed))));
                    }
                } else if code == KeyCode::KeyI {
                    // The view's I: the page left as its Exit clicked.
                    back = Some(None);
                } else {
                    // The page's own loop (5CF4 over 3276:57D8): the key
                    // typed goes to the record that takes it.
                    page_key = match code {
                        KeyCode::NumpadAdd | KeyCode::Equal => Some(b'+'),
                        KeyCode::Minus | KeyCode::NumpadSubtract => Some(b'-'),
                        KeyCode::Escape => Some(0x1B),
                        KeyCode::Space => Some(b' '),
                        KeyCode::KeyD => Some(b'd'),
                        KeyCode::KeyS => Some(b's'),
                        _ => text.and_then(|t| t.bytes().next()).filter(u8::is_ascii),
                    };
                }
            }
            Some(p @ panels::Panel::Spellbook(_)) => {
                match code {
                    // The library's loop left with nothing picked (FFFFh).
                    KeyCode::Escape => cast = Some(usize::MAX),
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        if let panels::Panel::Spellbook(book) = p {
                            cast = Some(book.selected);
                        }
                    }
                    // The list's keys as 3159:06A0 takes them.
                    _ => {
                        if let Some(k) = list_key_of(code) {
                            p.bar_list_key(m, k);
                        }
                    }
                }
            }
            Some(p @ panels::Panel::Use(..)) => {
                let panels::Panel::Use(_, sel, _) = p else { unreachable!() };
                let sel = *sel;
                match code {
                    // The library's loop left with nothing picked: the list's
                    // state made all the same (FFFFh).
                    KeyCode::Escape => {
                        self.panel = None;
                        panels::use_pick(m, &mut self.engine.files, 0xFFFF, &mut said);
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        let pick = sel as u16;
                        self.panel = None;
                        panels::use_pick(m, &mut self.engine.files, pick, &mut said);
                    }
                    _ => {
                        if let Some(k) = list_key_of(code) {
                            p.bar_list_key(m, k);
                        }
                    }
                }
            }
            Some(panels::Panel::Sheet) => match code {
                // 3276:52B0's records: 'N' the equipment page, Esc leaves.
                KeyCode::KeyN => page = true,
                KeyCode::Escape => self.sheet_end(Some(0x1B)),
                KeyCode::KeyK => self.sheet_end(None),
                _ => {}
            },
            // 27E3:01FC reads no keys but Esc down (ACB6, which the key
            // interrupt has set already).
            Some(panels::Panel::Log(_)) => log_poll = code == KeyCode::Escape,
            None => {}
        }
        if log_poll {
            self.log_turn(0);
        }
        if page {
            self.equipment_page(Some(b'n'));
        }
        if let Some((key, answer)) = sheet_drop {
            self.sheet_drop_asked(key, answer);
        }
        if let Some(key) = back {
            self.equipment_back(key);
        }
        if let Some(c) = page_key {
            self.page_input(arena::charsheet::PageInput::Key(c), false, &mut said);
        }
        if let Some(line) = cast {
            self.panel = None;
            panels::book_cast(&mut self.engine, line, &mut said);
        }
        self.engine.m.list_reads = None;
        self.boxes.extend(said.boxes);
    }

    /// The sheet's equipment page (1194:0D67), `key` typed (`n`) or its
    /// button clicked (None): in the game's state the sheet's run turned
    /// to it (`Machine::sheet_equipment`); its colours set again (9B56).
    fn equipment_page(&mut self, key: Option<u8>) {
        self.fade_open(0, 0, |a| {
            let e = &mut a.engine;
            if e.m.sheet_equipment(&mut e.files, key).is_none() {
                eprintln!("the equipment page gave up: {}", arena::giveup::describe());
            }
            a.panel = Some(panels::equipment_page());
        });
    }

    /// D or the Drop button on the sheet's own equipment page
    /// (`Machine::sheet_equipment_drop`), `key` typed (`d`) or None for a
    /// click, `steps` what it has taken since: it runs first on a copy,
    /// and a question it asks past the answers (its text) or "Drop what?"
    /// waiting past the inputs (its lines) comes back for the window to
    /// put up, the game left as it was; with all it waits for taken, it
    /// runs again as the game's.
    fn sheet_drop(&mut self, key: Option<u8>, steps: &[DropStep]) -> SheetDrop {
        use arena::charsheet::PageInput;
        enum Stop {
            Asked(Vec<u8>, hud::Menu),
            Waiting,
        }
        let screen = self.assets.shown.clone();
        let lines_of = |lines: &[(Vec<u8>, u8)], top: u16| (lines.iter().map(|(t, c)| (String::from_utf8_lossy(t).into_owned(), *c)).collect(), top);
        let (mut m, mut f) = (self.engine.m.clone(), self.engine.files.clone());
        let mut stop: Option<Stop> = None;
        let mut shown = None;
        {
            let next = std::cell::RefCell::new(steps.iter().copied());
            let stop = std::cell::RefCell::new(&mut stop);
            let _ = m.sheet_equipment_drop(&mut f, key, Some(&screen), &mut |p| {
                match next.borrow_mut().next() {
                    Some(DropStep::Answer(yes, typed)) if stop.borrow().is_none() => {
                        // The answer's key read by the menu's pick (3159:10BE).
                        if let Some(c) = typed {
                            p.ring_typed(c);
                        }
                        if yes { 0 } else { 1 }
                    }
                    _ => {
                        if stop.borrow().is_none() {
                            let text = arena::use_item::var::DROP_TEXT;
                            **stop.borrow_mut() = Some(Stop::Asked(p.mem.cstr(arena::mem::DS, text).to_vec(), dialog::menu_at(p, text, 0x4B60, 0x4B63)));
                        }
                        1
                    }
                }
            }, &mut |_, lines, top| shown = Some(lines_of(lines, top)), &mut |_| match next.borrow_mut().next() {
                Some(DropStep::Input(i)) if stop.borrow().is_none() => i,
                _ => {
                    // Past the inputs: the loop left by its exit (Esc) on the copy.
                    if stop.borrow().is_none() {
                        **stop.borrow_mut() = Some(Stop::Waiting);
                    }
                    PageInput::Key(0x1B)
                }
            });
        }
        match stop {
            Some(Stop::Asked(text, menu)) => return SheetDrop::Asked(text, menu),
            Some(Stop::Waiting) => return SheetDrop::Waiting(shown.unwrap_or_default(), m.mem.r8(arena::mem::DS, arena::use_item::var::FIRST_LINE)),
            None => {}
        }
        let e = &mut self.engine;
        // A refusal said in the list's panel (223A) waits there for a
        // click or a key (5536).
        let mut shown: Option<(Vec<(String, u8)>, u16)> = None;
        let next = std::cell::RefCell::new(steps.iter().copied());
        if e.m.sheet_equipment_drop(&mut e.files, key, Some(&screen), &mut |m| match next.borrow_mut().next() {
            Some(DropStep::Answer(yes, typed)) => {
                if let Some(c) = typed {
                    m.ring_typed(c);
                }
                if yes { 0 } else { 1 }
            }
            _ => 1,
        }, &mut |_, lines, top| shown = Some(lines_of(lines, top)), &mut |_| match next.borrow_mut().next() {
            Some(DropStep::Input(i)) => i,
            _ => PageInput::Key(0x1B),
        }).is_none() {
            eprintln!("the drop button gave up: {}", arena::giveup::describe());
        }
        if let (Some((lines, top)), Some(panels::Panel::Inventory(inv))) = (shown, self.panel.as_mut()) {
            if steps.is_empty() {
                inv.info = Some(lines);
                inv.info_y = Some(top as i32);
            }
        }
        SheetDrop::Done
    }

    /// S or the Spells button on the sheet's own equipment page
    /// (`Machine::sheet_equipment_spells`), `key` typed (`s`) or None for
    /// a click: a caster's list turned to the spells or back; anyone
    /// else's box (3276:5AD7) put up.
    fn sheet_spells(&mut self, key: Option<u8>) {
        let screen = self.assets.shown.clone();
        let caster = self.engine.m.mem.r8(arena::mem::DS, 0x01A9) & 0x20 != 0;
        let e = &mut self.engine;
        if e.m.sheet_equipment_spells(&mut e.files, key, Some(&screen)).is_none() {
            eprintln!("the spells button gave up: {}", arena::giveup::describe());
        }
        if !caster {
            let t = self.engine.m.mem.cstr(arena::mem::DATA, 0x5AD7).to_vec();
            self.boxes.push_back(hud::lines(&t, 0x0F).into());
        }
        if let Some(panels::Panel::Inventory(inv)) = self.panel.as_mut() {
            inv.bare = false;
        }
    }

    /// `sheet_drop`, its question put up on the inventory, or "Drop
    /// what?"'s lines while its loop waits (`Inventory::drop_what`), with
    /// `answer` (yes, and its key if typed) the question's.
    fn sheet_drop_asked(&mut self, key: Option<u8>, answer: Option<(bool, Option<u8>)>) {
        const RING: std::ops::Range<u16> = 0xAC46..0xACB5;
        let mut key = key;
        let mut steps = match self.panel.as_ref() {
            Some(panels::Panel::Inventory(inv)) => inv.drop_what.clone().unwrap_or_default(),
            _ => Vec::new(),
        };
        if let Some((yes, typed)) = answer {
            steps.push(DropStep::Answer(yes, typed));
        }
        if !steps.is_empty() {
            let kept = match self.panel.as_mut() {
                Some(panels::Panel::Inventory(inv)) => inv.drop_ring.take(),
                _ => None,
            };
            if let Some((ring, k)) = kept.map(|b| *b) {
                for (at, b) in RING.zip(ring) {
                    self.engine.m.mem.w8(arena::mem::DS, at, b);
                }
                key = k;
            }
        }
        let run = self.sheet_drop(key, &steps);
        let ring: Vec<u8> = RING.map(|at| self.engine.m.mem.r8(arena::mem::DS, at)).collect();
        if let Some(panels::Panel::Inventory(inv)) = self.panel.as_mut() {
            match run {
                SheetDrop::Asked(text, menu) => {
                    inv.asking = Some((text, usize::MAX));
                    inv.asked_menu = Some(Box::new(menu));
                    inv.drop_ring = Some(Box::new((ring, key)));
                    inv.drop_what = Some(steps);
                    inv.drop_first = None;
                }
                SheetDrop::Waiting((lines, top), first) => {
                    inv.info = Some(lines);
                    inv.info_y = Some(top as i32);
                    inv.drop_ring = Some(Box::new((ring, key)));
                    inv.drop_what = Some(steps);
                    inv.drop_first = Some(first);
                }
                SheetDrop::Done => {
                    inv.drop_what = None;
                    inv.drop_first = None;
                }
            }
        }
    }

    /// An input while "Drop what?" waits on the sheet's own page (its
    /// loop's, 1194:2325): the drop run again with it after the others.
    fn sheet_drop_what(&mut self, input: arena::charsheet::PageInput) {
        if let Some(panels::Panel::Inventory(inv)) = self.panel.as_mut() {
            inv.info = None;
            inv.info_y = None;
            if let Some(steps) = inv.drop_what.as_mut() {
                steps.push(DropStep::Input(input));
            }
        }
        self.sheet_drop_asked(None, None);
    }

    /// The sheet's equipment page left by its exit, `key` typed (Esc) or
    /// clicked (None): back to the sheet (`Machine::sheet_equipment_close`).
    fn equipment_back(&mut self, key: Option<u8>) {
        self.panel = Some(panels::Panel::Sheet);
        if self.sheet_run.is_some() {
            let e = &mut self.engine;
            if e.m.sheet_equipment_close(&mut e.files, key).is_none() {
                eprintln!("the equipment page's end gave up: {}", arena::giveup::describe());
            }
        }
    }

    /// An answer given to the spell's page `page` (up on the inventory).
    /// A page shown sets its colours (1194:68D7, 9B56's parts); Exit fades
    /// to black in 8 (1194:55B2) and the equipment screen's colours are
    /// set again.
    fn page_answer(&mut self, page: panels::SpellPage, a: panels::PageAnswer) {
        let after = page.clone().answer(a.clone(), &mut self.engine.m.clone(), &mut self.engine.files.clone());
        let drawn = page.pages;
        let set = |s: &mut Self| {
            if let Some(panels::Panel::Inventory(inv)) = s.panel.as_mut() {
                inv.page = page.answer(a, &mut s.engine.m, &mut s.engine.files);
                inv.bare |= inv.page.is_none();
                // Its name typed over (1194:65FD): the game's editor
                // opened on the game as the page asked it.
                if let Some((field, look, game)) = inv.page.as_ref().and_then(|p| p.typing.clone()).map(|b| *b) {
                    s.live = field::Live::open(&game.0, field, look);
                }
            }
        };
        match after {
            None => self.fade_open(8, 1, set),
            Some(p) if p.pages > drawn => self.fade_open(0, 1, set),
            Some(_) => set(self),
        }
    }

    /// The page a list in the bar's corner keeps (2542:06F8, from the
    /// frame's input step): the frame's drawing's rows, 0..146 (row 146
    /// as the drawing leaves it, before C3A2's clear; no compass yet,
    /// 0000:4D7C comes after), over the interface as shown. Without the
    /// engine's page, the screen as shown.
    fn list_page(&self) -> Vec<u8> {
        let mut s = self.assets.shown.clone();
        if let (Some(page), true) = (&self.engine.page, self.engine.page_gaps.is_empty()) {
            let n = ((VIEW_ROWS + 1) * 320).min(s.len()).min(page.len());
            s[..n].copy_from_slice(&page[..n]);
        }
        s
    }

    /// The spellbook's or the use list's loop as it starts, on a copy of
    /// the game (the list run there with nothing picked): `list_live`.
    fn bar_list_capture(&mut self) {
        let mut m = self.engine.m.clone();
        let mut files = self.engine.files.clone();
        let screen = self.list_page();
        m.list_reads = None;
        m.list_capture = true;
        match &self.panel {
            Some(panels::Panel::Spellbook(_)) => {
                let _ = m.spellbook_list(&mut files, Some(&screen), 0xFFFF);
            }
            Some(panels::Panel::Use(..)) => {
                let mut said = panels::Said { screen: Some(screen), ..Default::default() };
                panels::use_pick(&mut m, &mut files, 0xFFFF, &mut said);
            }
            _ => {}
        }
        self.list_live = m.list_captured.take();
    }

    /// The spellbook or the use list up, with its copy (`list_live`): the
    /// line marked and the first shown as its loop has them over the
    /// reads so far, and where they end it, its answer taken (the spell
    /// cast, the item used; FFFFh nothing); the panel's own model
    /// otherwise (false).
    fn bar_list_followed(&mut self, said: &mut panels::Said) -> bool {
        let Some(live) = self.list_live.as_ref() else { return false };
        let (mark, top, end) = live.live(&self.list_reads);
        let mark = if mark == 0xFFFF { usize::MAX } else { mark as usize };
        let spellbook = match self.panel.as_mut() {
            Some(panels::Panel::Spellbook(book)) => {
                (book.selected, book.top) = (mark, top as usize);
                true
            }
            Some(panels::Panel::Use(_, sel, t)) => {
                (*sel, *t) = (mark, top as usize);
                false
            }
            _ => return false,
        };
        if let Some(ax) = end {
            self.panel = None;
            self.list_live = None;
            if spellbook {
                panels::book_cast(&mut self.engine, if ax == 0xFFFF { usize::MAX } else { ax as usize }, said);
            } else {
                panels::use_pick(&mut self.engine.m, &mut self.engine.files, ax, said);
            }
        }
        self.engine.m.list_reads = None;
        self.boxes.extend(std::mem::take(&mut said.boxes));
        true
    }

    /// A click with a panel up (`right`: the right button): an inventory
    /// line's item put on or off, or selected; the use list's entry used;
    /// the spellbook's entry cast.
    fn panel_click(&mut self, right: bool) {
        let (x, y) = self.screen_point();
        let mut said = panels::Said::default();
        if matches!(self.panel, Some(panels::Panel::Use(..) | panels::Panel::Spellbook(_))) {
            said.screen = Some(self.list_page());
            // The click as the list's loop reads it, down and up (the
            // reads kept while it is up: its loop takes them when it ends).
            self.list_read(if right { 2 } else { 1 }, 0);
            self.list_read(0, 0);
            self.engine.m.list_reads = Some(self.list_reads.clone());
            if self.bar_list_followed(&mut said) {
                return;
            }
        }
        let mut cast = None;
        let mut sheet_drop = None;
        let mut log_poll = false;
        let mut page_click = false;
        match self.panel.as_mut() {
            Some(panels::Panel::Inventory(inv)) => {
                // A spell's page up (1194:51D1): the click its question
                // waits for, or the Delete question's yes or no.
                if let Some(page) = inv.page.clone() {
                    let answer = match &page.asking {
                        // Its Delete question (020E): the right button
                        // leaves it, as Esc.
                        Some(_) if right => Some(panels::PageAnswer::Confirm(0xFFFF)),
                        Some(_) => page.menu_pick(&self.assets.archive, x, y).map(|i| panels::PageAnswer::Confirm(i as u16)),
                        None if !right => Some(panels::PageAnswer::Click(x as u16, y as u16)),
                        None => None,
                    };
                    if let Some(a) = answer {
                        self.page_answer(page, a);
                    }
                    return;
                }
                // "Drop what?" waits in its loop (1194:2325): a click of
                // either button, on its records' rectangles or not.
                if inv.drop_what.is_some() && inv.asking.is_none() {
                    self.sheet_drop_what(arena::charsheet::PageInput::Click(x as u16, y as u16));
                    return;
                }
                // An item's description waits for a click (1194:5536),
                // which does nothing more.
                if inv.info.take().is_some() {
                    inv.info_y = None;
                    return;
                }
                if let Some(menu) = panels::drop_question(inv) {
                    // The drop's yes or no (020E): the entry by the click's
                    // y as its boxes are laid out (`menu_pick`); the right
                    // button leaves it, as Esc.
                    let pick = if right { Some((1, Some(0x1B))) } else { panels::drop_pick(&self.assets.archive, &menu, y).map(|i| (i, None)) };
                    if let Some((i, typed)) = pick {
                        inv.asking = None;
                        inv.asked_menu = None;
                        sheet_drop = Some((None, Some((i == 0, typed))));
                    }
                } else {
                    // The page's own loop (5CF4 over 3276:57D8): the click
                    // goes to the record whose rectangle takes it.
                    page_click = true;
                }
            }
            // A line of the list in the bar's corner clicked (2542:06F8):
            // a line not marked is marked; the line marked is picked.
            // A right click leaves the list (5AF4's flag 4000h), nothing
            // picked.
            Some(panels::Panel::Spellbook(_)) if right => cast = Some(usize::MAX),
            Some(panels::Panel::Use(..)) if right => {
                self.panel = None;
                panels::use_pick(&mut self.engine.m, &mut self.engine.files, 0xFFFF, &mut said);
            }
            Some(p @ (panels::Panel::Spellbook(_) | panels::Panel::Use(..))) => {
                let hud = hud::Hud { menu: p.bar_list(&self.engine.m), ..Default::default() };
                // (A line past the lines, the box's part row scrolled to
                // the end, picks nothing here: 2542:06F8 has no 5988.)
                let n = hud.menu.as_ref().map_or(0, |m| m.entries.len());
                if let Some(i) = hud::menu_pick(&self.font, &hud, x, y).filter(|&i| i < n) {
                    let marked = match p {
                        panels::Panel::Spellbook(book) => &mut book.selected,
                        panels::Panel::Use(_, sel, _) => sel,
                        _ => unreachable!(),
                    };
                    if *marked != i {
                        *marked = i;
                    } else if matches!(p, panels::Panel::Spellbook(_)) {
                        cast = Some(i);
                    } else {
                        self.panel = None;
                        panels::use_pick(&mut self.engine.m, &mut self.engine.files, i as u16, &mut said);
                    }
                }
            }
            Some(panels::Panel::Sheet) => match sheets::sheet_click(x, y) {
                Some(sheets::SheetClick::Equipment) => self.equipment_page(None),
                Some(sheets::SheetClick::Done) => self.sheet_end(None),
                None => {}
            },
            Some(panels::Panel::Log(_)) => log_poll = true,
            None => {}
        }
        if log_poll {
            self.log_turn(if right { 2 } else { 1 });
        }
        if let Some((key, answer)) = sheet_drop {
            self.sheet_drop_asked(key, answer);
        }
        if page_click {
            self.page_input(arena::charsheet::PageInput::Click(x as u16, y as u16), right, &mut said);
        }
        if let Some(line) = cast {
            self.panel = None;
            panels::book_cast(&mut self.engine, line, &mut said);
        }
        self.engine.m.list_reads = None;
        self.boxes.extend(said.boxes);
    }

    /// The equipment page's own loop (5CF4 over 3276:57D8) given a key
    /// typed or a click (`right`: the right button, AFF9 2): the record
    /// that takes it (`Machine::sheet_equipment_record`) runs its handler.
    /// The list's (1AC6: a line marked, put on or taken off; a right
    /// click's description, 1988) shows what it says in the list's panel
    /// (223A), which waits for a click or a key (5536); a spell's line
    /// marked already opens its page (1B3C), which waits for clicks of
    /// its own (51D1). No record: the turn reads again.
    fn page_input(&mut self, input: arena::charsheet::PageInput, right: bool, said: &mut panels::Said) {
        use arena::charsheet::PageInput;
        if self.sheet_run.is_none() {
            return;
        }
        let key = match input {
            PageInput::Key(k) => Some(k),
            PageInput::Click(..) => None,
        };
        match self.engine.m.sheet_equipment_record(input) {
            Some(0) => {
                let mut shown: Option<(Vec<(String, u8)>, u16)> = None;
                let mut wait = |_: &mut arena::machine::Machine, lines: &[(Vec<u8>, u8)], top: u16| {
                    shown = Some((lines.iter().map(|(t, c)| (String::from_utf8_lossy(t).into_owned(), *c)).collect(), top));
                };
                let e = &mut self.engine;
                let done = match input {
                    PageInput::Key(k) => e.m.sheet_equipment_list_typed(&mut e.files, k, &mut wait),
                    PageInput::Click(_, y) if right => e.m.sheet_equipment_describe(&mut e.files, said, y, &mut wait),
                    PageInput::Click(_, y) => e.m.sheet_equipment_click(&mut e.files, y, &mut wait),
                };
                match done {
                    None => eprintln!("the list's click gave up: {}", arena::giveup::describe()),
                    Some(false) => {
                        // The spell's page sets its colours (68D7).
                        self.fade_open(0, 1, move |s| {
                            if let Some(panels::Panel::Inventory(inv)) = s.panel.as_mut() {
                                inv.page = panels::SpellPage::open_sheet(&mut s.engine.m, &mut s.engine.files);
                            }
                        });
                        return;
                    }
                    Some(true) => {}
                }
                let panel = said.panel.take();
                if let Some(panels::Panel::Inventory(inv)) = self.panel.as_mut() {
                    if let Some((lines, top)) = shown {
                        inv.info = Some(lines);
                        inv.info_y = Some(top as i32);
                    } else if let Some(t) = panel {
                        // A refusal's text (1BE0, 1B78) in the list's panel.
                        inv.info = Some(vec![(t, 0xA0)]);
                        inv.info_y = Some(0xA0);
                    }
                }
            }
            Some(r @ (1 | 2)) => {
                let e = &mut self.engine;
                if e.m.sheet_equipment_scroll(&mut e.files, key, r == 1).is_none() {
                    eprintln!("the list's arrow gave up: {}", arena::giveup::describe());
                }
            }
            Some(3) => self.sheet_drop_asked(key, None),
            Some(4) => self.equipment_back(key),
            Some(5) => self.sheet_spells(key),
            _ => self.engine.m.sheet_equipment_none(key),
        }
    }

    /// A click on the interface bar: on a button (2542:06CE's table), the
    /// frame run now takes it in its input step (`Front::bar_click`) and
    /// the screen it asks for opens inside it (`open_taken`); whether on a
    /// button.
    fn bar(&mut self, x: i32, y: i32, right: bool) -> bool {
        if sheets::bar_button(x, y).is_none() {
            return false;
        }
        self.queued_bar = Some((x, y, right));
        self.step();
        true
    }

    /// The screen a frame's menu key (2542:0100) or bar button asked for
    /// opened (inside the frame, `Talk::screen`; the travel map and the
    /// options screen after it), the mouse let go for it.
    fn open_taken(&mut self, t: sheets::Taken) {
        self.release();
        sheets::pics(&self.assets.archive, &self.engine.m);
        match t {
            sheets::Taken::Action(a) => self.bar_action(a),
            sheets::Taken::Options => self.open_options(),
        }
    }

    /// A key let go (`key_up`), and again after the turn waiting on its
    /// question runs again.
    fn let_go(&mut self, scan: u8) {
        self.key_up(scan);
        if self.pending.is_some() {
            self.released.push(scan);
        }
    }

    /// A key let go: through the key interrupt (0000:718A), its latch
    /// (2542:05B8) cleared too, as the original's next frame asking for
    /// it clears it, so its next press is taken.
    fn key_up(&mut self, scan: u8) {
        if self.engine.m.key_event(scan | 0x80).is_none() {
            eprintln!("the key interrupt met a fault of the original's: {}", arena::giveup::describe());
        }
        self.engine.m.key_pressed(scan as u16);
    }

    /// The automap screen left (0000:2EC8..2EF2): faded out in 12 steps,
    /// the world drawn and faded in to its colours (999E) in 12.
    fn close_automap(&mut self) {
        self.fade_open(12, 12, |a| {
            automap::close(&mut a.engine);
            a.map = false;
        });
    }

    /// What a bar button (or the menu key with its handler) asks for.
    fn bar_action(&mut self, a: sheets::BarAction) {
        match a {
            sheets::BarAction::None => {}
            sheets::BarAction::WorldMap => self.open_travel(),
            // 0000:2D3E..2DAE: the screen faded out in 12 steps, the
            // automap drawn and faded in in 12.
            sheets::BarAction::Automap => {
                // The mouse where the screen's loop first reads it (the view's
                // centre while the window holds it).
                let at = if self.captured { (160, 100) } else { self.screen_point() };
                self.fade_open(12, 12, move |a| {
                    automap::open(&mut a.engine, a.assets.dac(), at);
                    a.map = true;
                })
            }
            sheets::BarAction::Status(lines) => {
                self.status_box = self.boxes.is_empty();
                self.boxes.push_back(lines.into());
                self.ring_box_kept = true;
            }
            // 056A: the last spell again (C with a Shift, or the right
            // button's release), else the book's list.
            sheets::BarAction::Spells(right) => {
                let mut said = panels::Said::default();
                if !panels::book_cast_last(&mut self.engine, right, &mut said) {
                    self.panel = Some(panels::Panel::Spellbook(Default::default()));
                    self.list_reads.clear();
                    self.bar_list_capture();
                }
                self.boxes.extend(said.boxes);
            }
            // 27E3:0088: the screen faded out in 8 steps first.
            sheets::BarAction::Log(from_bar) => self.fade_open(8, 0, move |a| {
                a.log_begin(from_bar);
                a.panel = Some(panels::Panel::Log(sheets::open_log(&a.engine.m, &a.engine.files)));
            }),
            // 2542:066A: no list without an item to use (127C 0).
            sheets::BarAction::Use => {
                let names = panels::usable_names(&self.engine.m);
                if !names.is_empty() {
                    self.panel = Some(panels::Panel::Use(names, 0, 0));
                    self.list_reads.clear();
                    self.bar_list_capture();
                }
            }
            sheets::BarAction::Camp => self.camp = true,
            // 1194:0CF8: the screen faded out in 8 steps first.
            sheets::BarAction::Sheet => self.fade_open(8, 0, |a| {
                a.sheet_begin();
                a.panel = Some(panels::Panel::Sheet);
                // The view's I: N typed on the sheet.
                if std::mem::take(&mut a.page_after_sheet) {
                    a.equipment_page(Some(b'n'));
                }
            }),
        }
    }

    /// A click at (x, y) on the automap screen's map: a turn adding or
    /// editing the note there (0000:31FE), its text asked for as other
    /// typed answers are; outside the map nothing (0000:346B).
    fn note_turn(&mut self, x: i32, y: i32) {
        let before = copy(&self.engine);
        let input = TurnIn {
            click: None,
            intent: Default::default(),
            clicked: false,
            camp: false,
            shop: None,
            loot: None,
            talk: false,
            key: false,
            tick: 0,
            right: false,
            drag: (0, 0),
            aspect: (self.size.0 / self.size.1.max(1.0)) as f32,
            note: Some((x as u16, y as u16)),
            bar: None,
        };
        self.turn(before, input, Vec::new(), true);
    }

    /// A screen opened by `open` as the original opens it: the screen
    /// shown faded to black in `steps` (0000:5E3E; 0: none), then the new
    /// screen's colours faded in in `fade_in` steps (0, 1: set at once,
    /// 9B56's three parts), each wait for the retrace a frame (`fade`).
    fn fade_open(&mut self, steps: i16, fade_in: i16, open: impl FnOnce(&mut Self)) {
        let was = self.fade_from();
        open(self);
        self.fade_to(was, steps, fade_in);
    }

    /// Where a screen's change starts: the fade under way (taken: the
    /// change starts where it ends), the colours shown and the screen.
    fn fade_from(&mut self) -> (std::collections::VecDeque<(film::Dac, bool)>, film::Dac, hud::Screen) {
        let queued = std::mem::take(&mut self.fade);
        self.compose();
        if std::mem::take(&mut self.journey_black) {
            let black = [[0, 0, 0, 0xFF]; 256];
            return (queued, black, hud::Screen::new(black));
        }
        let mut from = *self.screen.palette();
        // The world seen outdoors: its sky's colours in entries 1..8 and the
        // lamps' in 71h, as the game sets them in the DAC (`set_sky`).
        let world = self.panel.as_ref().is_none_or(|p| p.over_play()) && !self.map && self.travel.is_none() && (self.creation.is_none() || self.level_notice()) && self.movies.is_empty();
        // (The frame's view as last drawn: drawing one runs the game's view
        // routines, which a screen opening mustn't.)
        if let Some((sky, _, _)) = self.assets.frame_view.as_ref().filter(|_| world) {
            if sky.outdoors {
                for (i, c) in sky.colours.iter().enumerate() {
                    from[1 + i] = [c[0], c[1], c[2], from[1 + i][3]];
                }
                if let Some(c) = sky.lamp {
                    from[0x71] = [c[0], c[1], c[2], from[0x71][3]];
                }
            }
        }
        (queued, from, self.screen.clone())
    }

    /// The area left (C398 bit 80h): on to the next area, whose assets
    /// are made afresh, its colours faded (`area_fade`), or the game's
    /// end; whether either.
    fn area_left(&mut self) -> bool {
        let was = self.fade_from();
        let blackout = std::mem::take(&mut self.blackout);
        if self.area_next() {
            self.area_fade(was, blackout);
            return true;
        }
        self.fade = was.0;
        if self.engine.m.mem.r16(arena::mem::DS, 0x05B1) & 0x100 != 0 {
            self.game_ending();
            return true;
        }
        false
    }

    /// On to the next area (`areas::next`), its assets made afresh;
    /// whether there is one.
    fn area_next(&mut self) -> bool {
        let Some(run) = areas::next(&mut self.engine, &self.dir, self.run) else { return false };
        self.run = run;
        self.assets = Assets::new(&self.dir, &mut self.engine);
        self.remake_renderer();
        self.last = Some(arena::engine::Frame::Running);
        true
    }

    /// An area's change as the original shows it (checked against
    /// tests/vectors/areafade.txt): on the way into a building the screen
    /// faded to black in 8 first (1194:55B2, `blackout`); after the world
    /// loop the colours faded to black in 12 (0000:0AEA..0AFC, from black
    /// after a blackout: nothing seen); the next area's colours then set
    /// in its 9B56's three parts before its first frame. (The world's own
    /// way back from a screen, 0000:A6B5, fades in in 12 unless C994 is
    /// set: not an area's change.)
    fn area_fade(&mut self, was: (std::collections::VecDeque<(film::Dac, bool)>, film::Dac, hud::Screen), blackout: bool) {
        let (queued, from, before) = was;
        self.compose();
        let to = *self.screen.palette();
        self.fade = queued;
        let black = [[0, 0, 0, 0xFF]; 256];
        let mut film = film::Film::new(Vec::new(), from);
        if blackout {
            film.fade(&black, 8);
        }
        film.fade(&black, 12);
        let out = film.frames.len();
        film.colours(&to);
        self.fade.extend(film.frames.into_iter().enumerate().map(|(i, (_, dac))| (dac, i < out)));
        self.fade_old = Some(before);
    }

    /// The change from `was` (`fade_from`) to the screen now played
    /// (`fade_open`).
    fn fade_to(&mut self, was: (std::collections::VecDeque<(film::Dac, bool)>, film::Dac, hud::Screen), steps: i16, fade_in: i16) {
        let (queued, from, before) = was;
        self.compose();
        let to = *self.screen.palette();
        self.fade = queued;
        let mut film = film::Film::new(Vec::new(), from);
        if steps > 0 {
            film.fade(&[[0, 0, 0, 0xFF]; 256], steps);
        }
        let out = film.frames.len();
        if fade_in > 1 {
            film.fade(&to, fade_in);
        } else {
            film.colours(&to);
        }
        self.fade.extend(film.frames.into_iter().enumerate().map(|(i, (_, dac))| (dac, i < out)));
        if out > 0 {
            self.fade_old = Some(before);
        }
    }

    /// The sheet's run in the game's state (2542:00E0..00F0, 1194:0CF8
    /// to its buttons' loop: `Machine::sheet_open`), the colours read as
    /// shown.
    fn sheet_begin(&mut self) {
        let dac = self.assets.dac().map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]);
        let e = &mut self.engine;
        self.sheet_run = e.m.sheet_open(&mut e.files, Some(&dac));
        if self.sheet_run.is_none() {
            eprintln!("the character sheet gave up: {}", arena::giveup::describe());
        }
    }

    /// The logbook's run in the game's state (2542:02A3..02FD, 27E3:0088
    /// to its loop: `Machine::log_open`), the colours read as shown;
    /// `from_bar` by the bar's scroll button.
    fn log_begin(&mut self, from_bar: bool) {
        let dac = self.assets.dac().map(|c| [c[0] >> 2, c[1] >> 2, c[2] >> 2]);
        let e = &mut self.engine;
        self.log_run = e.m.log_open(&mut e.files, Some(&dac), 0, from_bar);
        if self.log_run.is_none() {
            eprintln!("the logbook gave up: {}", arena::giveup::describe());
        }
    }

    /// A turn of the logbook's loop (27E3:01FC: `Machine::log_turn`), the
    /// mouse read with `buttons` new at the cursor: the next page turned,
    /// the pages' loop ended or the screen left (`log_end`).
    fn log_turn(&mut self, buttons: u8) {
        let Some(mut run) = self.log_run else { return };
        let (x, y) = self.screen_point();
        let e = &mut self.engine;
        let turn = e.m.log_turn(&mut e.files, &mut run, buttons, x as u16, y as u16, false);
        self.log_run = Some(run);
        match turn {
            arena::logbook::LogTurn::Page => {
                if let Some(panels::Panel::Log(log)) = self.panel.as_mut() {
                    log.page += 1;
                }
            }
            arena::logbook::LogTurn::Left => self.log_end(),
            arena::logbook::LogTurn::PrintFailed(left) => {
                self.say("Error while printing....");
                if left {
                    self.log_end();
                }
            }
            _ => {}
        }
    }

    /// The logbook left (`Machine::log_close`). The buttons its last read
    /// left in AFF9 (the click that left, `log_turn`'s) stay for the rest
    /// of the frame it opened in (`screen_press`).
    fn log_end(&mut self) {
        self.panel = None;
        if let Some(run) = self.log_run.take() {
            let e = &mut self.engine;
            e.m.log_close(&mut e.files, run);
            self.screen_press = e.m.mem.r8(arena::mem::DS, arena::controls::var::BUTTONS) != 0;
        }
    }

    /// The sheet left by its exit, `key` typed (Esc) or clicked (None):
    /// the run's end (`Machine::sheet_close`).
    fn sheet_end(&mut self, key: Option<u8>) {
        self.panel = None;
        if let Some(saved) = self.sheet_run.take() {
            let e = &mut self.engine;
            self.ring_kept = true;
            if e.m.sheet_close(&mut e.files, saved, key).is_none() {
                eprintln!("the character sheet's end gave up: {}", arena::giveup::describe());
            }
        }
    }

    /// A box of the view's own.
    fn say(&mut self, s: &str) {
        println!("{s}");
        self.boxes.push_back(vec![vec![hud::Span { colour: 0x0F, text: s.to_string(), font: Default::default(), code: None }]].into());
    }

    /// What the HUD shows now.
    fn hud(&self) -> hud::Hud {
        let mut h = hud::Hud { message: message_line(&self.engine), boxed: self.boxes.front().map(|b| b.lines.clone()), counters: hud::spell_counters(&self.engine.m), level: hud::level_text(&self.engine.m), effect: effect_of(&self.effect), clip: game_clip(&self.engine.m), ..Default::default() };
        if self.engine.m.mem.r8(arena::mem::DS, arena::rest::var::RESTING) != 0 || (self.status_held && !self.boxes.is_empty()) {
            h.status = self.status.clone();
        }
        if let Some(list) = self.panel.as_ref().and_then(|p| p.bar_list(&self.engine.m)) {
            h.menu = Some(list);
        }
        if self.boxes.is_empty() {
            match self.pending.as_ref().map(|p| &p.need) {
                Some(dialog::Need::Menu(m)) => {
                    let mut m = m.clone();
                    m.selected = self.selected.min(m.entries.len().saturating_sub(1));
                    if m.list {
                        m.selected = self.selected;
                        m.top = Some(self.list_top);
                    }
                    // A menu's boxes mark no entry (3159:0F2A's loop has
                    // no arrows).
                    if !m.list {
                        m.selected = usize::MAX;
                    }
                    h.menu = Some(m);
                }
                Some(dialog::Need::Text(a)) => h.typing = Some(a.prompt.clone()),
                // A level's points: the attribute screen (`creation`); the
                // spellmaker's screen is drawn behind (`maker_screen`).
                Some(dialog::Need::Spend(_) | dialog::Need::Spellmaker | dialog::Need::Travel(_) | dialog::Need::Screen(..)) | None => {}
            }
        }
        h
    }

    /// A key while the game waits: a box read; a menu's entry moved to,
    /// picked (Enter, its key letter, its number) or none (Esc); hours
    /// typed.
    fn dialog_key(&mut self, code: winit::keyboard::KeyCode, text: Option<&str>) {
        use winit::keyboard::KeyCode;
        if !self.boxes.is_empty() {
            let keys = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space) && !self.status_box;
            if keys || code == KeyCode::Escape {
                self.box_read();
            }
            return;
        }
        let Some(p) = &self.pending else { return };
        // A menu's loop takes the keys typed through the key interrupt
        // (3159:10DD, 7138): pressed and let go before its next read.
        if matches!(&p.need, dialog::Need::Menu(m) if !m.list && m.kind != hud::DIALOG_BUTTONS) {
            if let Some(scan) = interface::scan_code(code) {
                self.menu_read(0, &[scan, scan | 0x80]);
            }
        }
        let Some(p) = &self.pending else { return };
        match &p.need {
            dialog::Need::Menu(menu) => {
                let n = menu.entries.len().max(1);
                let list_key = list_key_of(code);
                match code {
                    // A list's keys move its mark and its lines as
                    // 3159:06A0 does (`hud::list_key`); a menu's loop
                    // (0F2A) has no arrows.
                    _ if menu.list && list_key.is_some() => {
                        if let Some(k) = list_key {
                            self.list_key_read(k);
                            if self.list_followed().is_none() {
                                self.list_moved(|m| hud::list_key(m, k));
                            }
                        }
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter if menu.list => {
                        let s = self.selected.min(n - 1) as u16;
                        self.list_tap(arena::listwindow::list_keys::ENTER.0);
                        self.list_ended(s);
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        let s = self.selected.min(n - 1) as u16;
                        self.menu_answer(s);
                    }
                    // Esc leaves a list as the right button does, where it
                    // does (`hud::leaves`).
                    KeyCode::Escape if menu.list && hud::leaves(menu) => {
                        self.list_tap(arena::listwindow::list_keys::ESC.0);
                        self.list_ended(0xFFFF);
                    }
                    KeyCode::Escape if !menu.list => {
                        self.menu_read(0, &[]);
                        self.menu_answer(0xFFFF);
                    }
                    KeyCode::Escape => {}
                    _ => {
                        let Some(c) = text.and_then(|t| t.bytes().next()) else { return };
                        let up = c.to_ascii_uppercase();
                        let by_key = menu.keys.iter().position(|&k| k.to_ascii_uppercase() == up);
                        let by_number = (c as char).to_digit(10).filter(|&d| d >= 1).map(|d| d as usize - 1).filter(|&i| i < menu.entries.len());
                        if let Some(i) = by_key.or(by_number) {
                            self.menu_read(0, &[]);
                            self.menu_answer(i as u16);
                        }
                    }
                }
            }
            // The attribute screen takes the keys (`creation_input`).
            dialog::Need::Spend(_) => {}
            // The travel map takes the keys (`travel`).
            dialog::Need::Travel(_) | dialog::Need::Screen(..) => {}
            // On the spellmaker, Esc clicks as leaving does: its "exit",
            // or the form's done box.
            dialog::Need::Spellmaker => {
                if code == KeyCode::Escape {
                    if let Some((snap, form)) = &p.maker {
                        let mut m = (*snap.0).clone();
                        let (x, y) = arena::spellmaker::SpellmakerHooks::click(&mut arena::spellmaker::NoSpellmakerHooks, &mut m, *form);
                        self.answer(dialog::Answer::Click(x, y));
                    }
                }
            }
            // The typed field takes the keys, pressed and let go
            // (`field_key`).
            dialog::Need::Text(_) => {}
        }
    }

    /// A key pressed or let go while a typed field is asked: through the
    /// game's key interrupt to its editor (`field::Live`); the field
    /// ended, its passes' inputs are the answer.
    fn field_key(&mut self, code: winit::keyboard::KeyCode, down: bool) {
        let Some(l) = &mut self.live else { return };
        if let Some(scan) = interface::scan_code(code) {
            l.key(scan, down);
        }
        self.field_ended();
    }

    /// The typed field's end: its inputs answer the question.
    fn field_ended(&mut self) {
        if let Some(keys) = self.live.as_ref().filter(|l| l.ended()).map(|l| l.keys().to_vec()) {
            self.live = None;
            // A spell's page's name: the page's answer.
            if let Some(panels::Panel::Inventory(inv)) = self.panel.as_ref() {
                if let Some(page) = inv.page.clone().filter(|p| p.typing.is_some()) {
                    self.page_answer(page, panels::PageAnswer::Keys(keys));
                    return;
                }
            }
            self.answer(dialog::Answer::Keys(keys));
        }
    }

    /// A frame of the window: the typed field's (a held arrow, the
    /// cursor's blink).
    fn field_frame(&mut self) {
        if let Some(l) = &mut self.live {
            l.frame();
        }
        self.field_ended();
        if let Some(ui) = self.saveui.as_mut().filter(|u| u.typing.is_some()) {
            let e = ui.frame();
            self.saveui_ended(e);
        }
        if let Some(c) = &mut self.creation {
            c.field_frame();
        }
    }

    /// A box read. The last before a level's attribute screen (its notice)
    /// gives way to it as 2686:02D8 does: the screen faded out in 8
    /// steps, the screen's colours set.
    /// A level gained: its notice up over play (1194:122C's message box,
    /// drawn over the world as the original draws it), the attribute
    /// screen only after it's read (`box_read`).
    fn level_notice(&self) -> bool {
        !self.boxes.is_empty() && self.creation.is_some() && matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spend(_)))
    }

    /// The game's page the window shows for the box up (`hud::Said`): a
    /// box over play, the engine's drawing of it on the game's own block,
    /// which holds the 3D view and the interface as the original's page
    /// does. None where another screen is up (a movie, the save menu, the
    /// options screen, creation, the spellmaker, a shop's, the travel
    /// map, a sheet, the automap: their pictures are the window's own).
    fn box_page_shown(&self) -> Option<std::sync::Arc<Vec<u8>>> {
        #[cfg(test)]
        if self.own_drawing {
            return None;
        }
        // A box to read first; else the menu asked; a list as its loop
        // has drawn it; a field in its prompt's box as the game's editor
        // has it.
        let page = match self.boxes.front() {
            Some(b) => b.page.clone()?,
            None => match self.pending.as_ref()? {
                Pending { need: dialog::Need::Menu(m), .. } if m.list => self.list_page.clone()?,
                Pending { need: dialog::Need::Menu(_), page: Some(p), .. } => p.clone(),
                Pending { need: dialog::Need::Text(dialog::Ask { prompt: dialog::Prompt::Boxed(..), .. }), .. } => std::sync::Arc::new(self.live.as_ref()?.page()?),
                _ => return None,
            },
        };
        let other = !self.movies.is_empty()
            || self.saveui.is_some()
            || self.options.is_some()
            || (self.creation.is_some() && !self.level_notice())
            || self.map;
        (!other).then_some(page)
    }

    fn box_read(&mut self) {
        self.boxes.pop_front();
        self.status_box = false;
        if self.boxes.is_empty() {
            self.box_backdrop = None;
            // The travel map's box clicked away: its page put back (933C)
            // and the rest of its pass's drawing the screen.
            if let Some(t) = &mut self.travel {
                t.box_read();
            }
        }
        if self.boxes.is_empty() && self.creation.is_some() && matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spend(_))) {
            let c = self.creation.take();
            self.fade_open(8, 0, |a| a.creation = c);
        }
    }

    /// The menu or list the game waits on, if any.
    fn menu_asked(&self) -> Option<&hud::Menu> {
        match self.pending.as_ref().map(|p| &p.need) {
            Some(dialog::Need::Menu(m)) => Some(m),
            _ => None,
        }
    }

    /// A click while the game waits: a box read, or a menu's entry picked.
    fn dialog_click(&mut self) {
        let (x, y) = self.screen_point();
        if !self.boxes.is_empty() {
            // The box's reads of the mouse (1194:5536's that takes the
            // press, 5E21's that takes the release) leave the cursor where
            // the click was, 4 up and left of the window's. A click on the
            // world whose handling showed the box (a door's trigger in
            // 2826:09AC) goes on after it to leave the click for the actors
            // (0A4C: C39A bit 80h, CB60 the cursor + 4) from there; the
            // port's box doesn't wait, so that was left at the click's
            // place before: moved to where the box's reads left the cursor.
            use arena::mem::DS;
            let m = &mut self.engine.m;
            m.mem.w16(DS, arena::controls::var::MOUSE_X, (x - 4).max(0) as u16);
            m.mem.w16(DS, arena::controls::var::MOUSE_Y, (y - 4).max(0) as u16);
            m.mouse_clamp();
            if m.mem.r16(DS, arena::movement::var::LEVEL_FLAGS) & 0x80 != 0 {
                let cx = m.mem.r16(DS, arena::controls::var::MOUSE_X).wrapping_add(4);
                let cy = m.mem.r16(DS, arena::controls::var::MOUSE_Y).wrapping_add(4);
                m.mem.w16(DS, arena::click::var::ACTOR_CLICK, cx);
                m.mem.w16(DS, arena::click::var::ACTOR_CLICK + 2, cy);
            }
            self.box_read();
            return;
        }
        // The spellmaker waits for a click on its screen or form.
        if matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spellmaker)) {
            self.answer(dialog::Answer::Click(x.clamp(0, 319) as u16, y.clamp(0, 199) as u16));
            return;
        }
        // A list (1194:5734): its arrows, track and lines as 3159:0834
        // takes them (`hud::list_click`): a line clicked is marked, the
        // line marked clicked picked (its first marked as it comes up).
        // A line past the lines (the box's part row, scrolled to the
        // end) is marked as it is, and picked as the last (5988).
        if let Some(n) = self.menu_asked().filter(|m| m.list).map(|m| m.entries.len()) {
            let mut picked = None;
            self.list_read(1, 0);
            self.list_read(0, 0);
            if let Some(end) = self.list_followed() {
                if let Some(ax) = end {
                    self.list_answer(ax);
                }
                return;
            }
            self.list_moved(|m| picked = hud::list_click(m, x, y));
            if let Some(i) = picked {
                self.list_answer(i.min(n.saturating_sub(1)) as u16);
            }
            return;
        }
        let hud = self.hud();
        if let Some(i) = hud::menu_pick(&self.font, &hud, x, y) {
            self.menu_read(1, &[]);
            self.menu_read(0, &[]);
            self.menu_answer(i as u16);
        }
    }

    /// A read of the mouse as the menu's loop makes it (0000:8FC4): its
    /// `buttons`, the cursor's place on the 320 x 200 screen, the bytes the
    /// keyboard gave since (`codes`); the loop's first read, as it starts
    /// with nothing held, before the first.
    fn menu_read(&mut self, buttons: u8, codes: &[u8]) {
        let (x, y) = self.screen_point();
        let (x, y) = (x.clamp(0, 319) as u16, y.clamp(0, 199) as u16);
        if self.menu_reads.is_empty() {
            self.menu_reads.push(arena::menubox::MenuRead { buttons: 0, x, y, codes: Vec::new() });
        }
        self.menu_reads.push(arena::menubox::MenuRead { buttons, x, y, codes: codes.to_vec() });
    }

    /// The menu's entry `pick` answered (FFFFh: left), with the reads that
    /// came to it.
    fn menu_answer(&mut self, pick: u16) {
        let reads = std::mem::take(&mut self.menu_reads);
        self.answer(dialog::Answer::Menu(pick, reads));
    }

    /// A read of the mouse as the list's loop makes it (0000:8FC4): its
    /// `buttons`, the cursor's place on the 320 x 200 screen, the list's
    /// `keys` held (`arena::listwindow::list_keys`).
    fn list_read(&mut self, buttons: u8, keys: u8) {
        let (x, y) = self.screen_point();
        self.list_reads.push(arena::listwindow::ListRead { buttons, x: x.clamp(0, 319) as u16, y: y.clamp(0, 199) as u16, keys });
    }

    /// The list's key `bit` pressed and let go: a read with it held, one
    /// without.
    fn list_tap(&mut self, bit: u8) {
        self.list_read(0, bit);
        self.list_read(0, 0);
    }

    /// `list_tap` of a list's key (the wheel's are Up and Down).
    fn list_key_read(&mut self, k: hud::ListKey) {
        use arena::listwindow::list_keys::*;
        let bit = match k {
            hud::ListKey::Up => UP.0,
            hud::ListKey::Down => DOWN.0,
            hud::ListKey::PageUp => PGUP.0,
            hud::ListKey::PageDown => PGDN.0,
            hud::ListKey::Home => HOME.0,
            hud::ListKey::End => END.0,
        };
        self.list_tap(bit);
    }

    /// The list's loop as the game runs it over the reads so far
    /// (`ListCapture::live`, from the copy taken as the list came up): its
    /// line marked and first line shown made the window's; Some(its
    /// answer) where the reads reach its end. None without the copy (the
    /// window's own model then: `hud::list_click`, `list_key`).
    fn list_followed(&mut self) -> Option<Option<u16>> {
        let live = self.list_live.as_ref()?;
        let (mark, top, end) = live.live(&self.list_reads);
        if end.is_none() {
            self.list_page = live.page(&self.list_reads).map(std::sync::Arc::new);
        }
        self.selected = if mark == 0xFFFF { usize::MAX } else { mark as usize };
        self.list_top = top as usize;
        Some(end)
    }

    /// A key or click that ends the list where the window's own model
    /// has it end (`pick`): with the game's copy, its loop's answer
    /// instead, if the reads end it.
    fn list_ended(&mut self, pick: u16) {
        match self.list_followed() {
            Some(Some(ax)) => self.list_answer(ax),
            Some(None) => {}
            None => self.list_answer(pick),
        }
    }

    /// The list's line `pick` answered (FFFFh: left), with the reads that
    /// came to it.
    fn list_answer(&mut self, pick: u16) {
        let reads = std::mem::take(&mut self.list_reads);
        self.answer(dialog::Answer::List(pick, reads));
    }

    /// The list asked, as shown (its line marked and first line shown),
    /// given to `f`; what `f` leaves marked and shown kept.
    fn list_moved(&mut self, f: impl FnOnce(&mut hud::Menu)) {
        let Some(mut m) = self.hud().menu.filter(|m| m.list) else { return };
        f(&mut m);
        self.selected = m.selected;
        self.list_top = hud::list_first(&m);
    }
}

/// An input the window gives, apart from winit's event loop and the GPU,
/// so that a test can give them too (`App::input`).
#[derive(Clone, Debug)]
enum In {
    /// A key down or up, with the text it types.
    Key { code: winit::keyboard::KeyCode, down: bool, text: Option<String> },
    /// The cursor moved to (x, y) in the window (pixels).
    Cursor(f64, f64),
    /// A mouse button down or up.
    Button { button: winit::event::MouseButton, down: bool },
    /// The mouse's raw motion (mickeys), for looking and dragging.
    Motion(f64, f64),
    /// The mouse's wheel turned (lines; up positive).
    Wheel(f64),
    /// The window's new size (pixels).
    Resized(f64, f64),
}

impl App {
    /// One input: an animation's next lines; creation's screens; a box or
    /// a question answered; a panel's keys and clicks; else the game's
    /// keys, the mouse captured, a click in the view. Then a screen up
    /// inside a frame, ended by it, has the frame's rest run
    /// (`screen_ended`).
    fn input(&mut self, ev: In) {
        self.input_one(ev);
        self.screen_ended();
    }

    fn input_one(&mut self, ev: In) {
        use winit::event::MouseButton;
        use winit::keyboard::KeyCode;
        self.ring_screen();
        // A key goes through the key interrupt (0000:718A, `key_event`)
        // whatever is up, as the original's every key does: its state
        // where the game keeps it (DS:ACB5 + its scan code, FFh down), for
        // the frame's own keys (- and =, F2, F3, F4, N, the debug keys)
        // and the menu keys, and a make's character into the ring of keys
        // typed (`ring_screen`). A key let go has its latch (2542:05B8)
        // cleared too, as the original's next frame asking for it clears
        // it, so its next press is taken.
        if let In::Key { code, down, .. } = ev {
            if let Some(scan) = interface::scan_code(code) {
                if down {
                    if self.engine.m.key_event(scan).is_none() {
                        eprintln!("the key interrupt met a fault of the original's: {}", arena::giveup::describe());
                    }
                } else if let Some(h) = self.menu_held.iter_mut().find(|h| h.0 == scan) {
                    h.1 = true;
                } else {
                    self.let_go(scan);
                }
            }
        }
        match ev {
            In::Resized(w, h) => self.size = (w, h),
            In::Cursor(x, y) => {
                self.cursor = (x, y);
                // The travel map's next pass reads the cursor where it is.
                if self.boxes.is_empty() {
                    let (x, y) = self.screen_point();
                    if let Some(t) = &mut self.travel {
                        t.moved(x, y);
                    }
                }
                // A button the logbook's loop has seen stays seen (AFF9 or'd
                // in, 27E3:0216): the cursor moved is read as a click there.
                if matches!(self.panel, Some(panels::Panel::Log(_))) && self.engine.m.log_buttons_held() {
                    self.log_turn(0);
                }
            }
            // The wheel moves a list's line, as Up and Down do (a list
            // scrolls to keep it shown); a menu's loop has no arrows.
            In::Wheel(dy) => {
                if self.boxes.is_empty() && dy != 0.0 {
                    if let Some(Pending { need: dialog::Need::Menu(m), .. }) = &self.pending {
                        if m.list {
                            let k = if dy > 0.0 { hud::ListKey::Up } else { hud::ListKey::Down };
                            self.list_key_read(k);
                            if self.list_followed().is_none() {
                                self.list_moved(|m| hud::list_key(m, k));
                            }
                        }
                    }
                }
            }
            In::Motion(dx, dy) => {
                if self.captured && !self.waiting() {
                    if self.keys.swing {
                        // A mickey a pixel.
                        self.drag.0 += dx;
                        self.drag.1 += dy;
                        self.right_drag += dx.abs() + dy.abs();
                    } else {
                        // About a quarter turn for 400 pixels.
                        self.look += dx * 128.0 / 400.0;
                    }
                }
            }
            // An animation up: a key or a click its next lines, or its end.
            In::Button { down: true, .. } | In::Key { down: true, .. } if !self.movies.is_empty() => {
                if let Some(mv) = self.movies.front_mut() {
                    if mv.lines.is_empty() {
                        mv.over = true;
                    } else {
                        mv.key();
                    }
                    if mv.over {
                        self.movies.pop_front();
                        self.movies_over();
                    }
                }
            }
            // The options screen up: its clicks (the left button's),
            // "Are you sure ?"'s keys and right button (`options`).
            In::Button { down: true, button: MouseButton::Left } if self.options.is_some() && self.saveui.is_none() => {
                let (x, y) = self.screen_point();
                if let Some(ui) = &mut self.options {
                    let e = ui.click(x, y, &self.font);
                    self.options_ended(e);
                }
            }
            In::Button { down: true, button: MouseButton::Right } if self.options.is_some() && self.saveui.is_none() => {
                if let Some(ui) = &mut self.options {
                    let e = ui.right();
                    self.options_ended(e);
                }
            }
            In::Key { code, down: true, .. } if self.options.is_some() && self.saveui.is_none() => {
                if let Some(ui) = &mut self.options {
                    let e = ui.key(code);
                    self.options_ended(e);
                }
            }
            In::Button { .. } | In::Key { .. } if self.options.is_some() && self.saveui.is_none() => {}
            // The save or load menu up: its clicks and keys (`saveui`).
            In::Button { down: true, button } if self.saveui.is_some() => {
                let (x, y) = self.screen_point();
                if let Some(ui) = &mut self.saveui {
                    let e = ui.click(x, y, button == MouseButton::Right);
                    self.saveui_ended(e);
                }
            }
            // A save's name typed: its keys, pressed and let go, to the
            // game's editor.
            In::Key { code, down, .. } if self.saveui.as_ref().is_some_and(|u| u.typing.is_some()) => {
                if let (Some(scan), Some(ui)) = (interface::scan_code(code), &mut self.saveui) {
                    let e = ui.field_key(scan, down);
                    self.saveui_ended(e);
                }
            }
            In::Key { code, down: true, text } if self.saveui.is_some() => {
                if let Some(ui) = &mut self.saveui {
                    let e = ui.key(code, text.as_deref());
                    self.saveui_ended(e);
                }
            }
            In::Button { .. } | In::Key { .. } if self.saveui.is_some() => {}
            // A level's notice read before its attribute screen.
            In::Button { down: true, button: MouseButton::Left } if self.creation.is_some() && !self.boxes.is_empty() => self.box_read(),
            In::Button { button, down } if self.creation.is_some() => {
                if down && button == MouseButton::Left {
                    let (x, y) = self.screen_point();
                    self.creation_input(create::Input::Click(x, y));
                } else if down && button == MouseButton::Right {
                    self.creation_input(create::Input::Right);
                }
            }
            In::Key { code, down, text } if self.creation.is_some() => {
                // The name typed: the keys, pressed and let go, to the
                // game's editor.
                if self.creation.as_ref().is_some_and(|c| c.typing()) {
                    if let Some(scan) = interface::scan_code(code) {
                        self.creation_input(create::Input::Key(scan, down));
                    }
                    return;
                }
                if !down {
                    // (Esc let go: the intro tests it held, `Creation::take_key`.)
                    if code == KeyCode::Escape {
                        self.creation_input(create::Input::Key(0x01, false));
                    }
                    return;
                }
                let inp = match code {
                    KeyCode::Enter | KeyCode::NumpadEnter => Some(create::Input::Enter),
                    KeyCode::Escape => Some(create::Input::Esc),
                    KeyCode::Backspace => Some(create::Input::Back),
                    KeyCode::ArrowUp => Some(create::Input::Up),
                    KeyCode::ArrowDown => Some(create::Input::Down),
                    _ => text.as_deref().and_then(|t| t.bytes().next()).filter(|c| (0x20..0x7F).contains(c)).map(create::Input::Char),
                };
                if let Some(inp) = inp {
                    self.creation_input(inp);
                }
            }
            // A typed field asked (the boxes before it read first): its
            // keys, pressed and let go, through the game's key interrupt
            // (`field_key`); the right button ends it (A172).
            In::Key { code, down, .. } if self.live.is_some() && self.boxes.is_empty() => self.field_key(code, down),
            In::Button { down: true, button: MouseButton::Right } if self.live.is_some() && self.boxes.is_empty() => {
                if let Some(l) = &mut self.live {
                    l.button();
                }
                self.field_ended();
            }
            // The travel map up (a box over it read first): its clicks and
            // keys (`travel`); it may be a turn's question (the way out of
            // a dungeon).
            In::Button { down: true, button } if self.travel.is_some() && self.boxes.is_empty() => {
                let (x, y) = self.screen_point();
                self.travel_input(|t, e, a| t.click(e, a, x, y, button == MouseButton::Right));
            }
            In::Button { down: true, button: MouseButton::Left } if self.waiting() => self.dialog_click(),
            // The right button on a menu leaves it (3159:10D6), and on a
            // list whose record has flag 4000h (3159:095F), nothing
            // picked; the classes' list (kind 6) waits on.
            // The dialog's buttons take either button's click (1194:6001).
            In::Button { down: true, button: MouseButton::Right } if self.waiting() && self.boxes.is_empty() && self.menu_asked().is_some() => {
                if self.menu_asked().is_some_and(|m| m.kind == hud::DIALOG_BUTTONS) {
                    self.dialog_click();
                } else if self.menu_asked().is_some_and(|m| m.list && hud::leaves(m)) {
                    self.list_read(2, 0);
                    self.list_read(0, 0);
                    self.list_ended(0xFFFF);
                } else if self.menu_asked().is_some_and(hud::leaves) {
                    self.menu_read(2, &[]);
                    self.menu_read(0, &[]);
                    self.menu_answer(0xFFFF);
                }
            }
            In::Button { down: true, button } if self.panel.is_some() => self.panel_click(button == MouseButton::Right),
            // The search's name typed on the map: its keys, pressed and
            // let go, to the game's editor.
            In::Key { code, down, .. } if self.travel.as_ref().is_some_and(|t| t.typing()) && self.boxes.is_empty() => {
                if let Some(scan) = interface::scan_code(code) {
                    self.travel_input(|t, e, _| t.field_key(e, scan, down));
                }
            }
            In::Key { code, down: true, text } if self.travel.is_some() && self.boxes.is_empty() => {
                let at = self.screen_point();
                self.travel_input(|t, e, _| t.key(e, code, text.as_deref(), at));
            }
            // The automap screen up: its buttons and its exit (0000:2E30).
            In::Button { down: true, .. } if self.map => {
                let (x, y) = self.screen_point();
                // The loop's turn that reads the click (0000:2DB8: the pen
                // drawn where it is).
                automap::turn(&mut self.engine.m, (x, y));
                if automap::exit_at(x, y) {
                    // The press its loop's last read (0000:8FC4) saw, in
                    // AFF9 for the rest of the frame (`screen_press`).
                    let b = self.engine.m.mem.r8(arena::mem::DS, arena::controls::var::BUTTONS) | 1;
                    self.engine.m.mem.w8(arena::mem::DS, arena::controls::var::BUTTONS, b);
                    self.screen_press = true;
                    self.close_automap();
                } else if let Some(b) = automap::button_at(x, y) {
                    self.engine.m.automap_view_button(b);
                    self.engine.m.automap_origin();
                    automap::redraw(&mut self.engine);
                } else if automap::notes_here(&self.engine.m) {
                    self.note_turn(x, y);
                }
            }
            In::Button { .. } if self.map => {}
            // Uncaptured, a click on the interface bar (rows 147 on) is its
            // button's (2542:06CE); off the buttons it captures the mouse.
            In::Button { down: true, button } if !self.captured && self.screen_point().1 >= 147 => {
                let (x, y) = self.screen_point();
                if !self.bar(x, y, button == MouseButton::Right) {
                    self.capture();
                }
            }
            // The right button held: dragging swings the weapon (0000:3C9F).
            // Let go with no more than a swing's least drag, a right click:
            // looking (2826:0C15).
            In::Button { down, button: MouseButton::Right } if self.captured => {
                if down {
                    self.right_drag = 0.0;
                } else if self.keys.swing && self.right_drag < 64.0 {
                    self.look_click = true;
                }
                self.keys.swing = down;
            }
            // Uncaptured, a left click in the view clicks where it is (a
            // corpse on the floor); another button captures the mouse.
            In::Button { down: true, button: MouseButton::Left } if !self.captured && self.screen_point().1 < 147 => {
                self.click_point = Some(self.screen_point());
                self.click = true;
            }
            In::Button { down: true, .. } if !self.captured => self.capture(),
            In::Button { down: true, button: MouseButton::Left } => self.click = true,
            In::Button { .. } => {}
            In::Key { code, down, text } => {
                if down && self.waiting() {
                    self.dialog_key(code, text.as_deref());
                    return;
                }
                if down && self.panel.is_some() {
                    self.panel_key(code, text.as_deref());
                    return;
                }
                if self.map {
                    // The automap screen: Esc or M leaves; the arrows and
                    // Home are its buttons (a convenience of the view's).
                    let b = match code {
                        _ if !down => return,
                        KeyCode::Escape | KeyCode::KeyM => {
                            self.close_automap();
                            return;
                        }
                        KeyCode::ArrowUp => 0,
                        KeyCode::ArrowDown => 1,
                        KeyCode::ArrowRight => 2,
                        KeyCode::ArrowLeft => 3,
                        KeyCode::Home => 4,
                        _ => return,
                    };
                    self.engine.m.automap_view_button(b);
                    self.engine.m.automap_origin();
                    return;
                }
                // Any key pressed during a rest ends it (AFF9); not the key
                // that asked for the camp (R), which the original's input
                // step takes before the rest begins.
                let resting = self.engine.m.mem.r8(arena::mem::DS, arena::rest::var::RESTING) != 0;
                if down && resting {
                    self.key = true;
                }
                // (The key went through the key interrupt above.) A menu
                // key's press runs the frame that takes it now: its body
                // asks the menu keys (2542:0100, `Front::menu_keys`) after
                // its actors, drawing and physics, and the screen the key
                // asks for opens inside it (`Talk::screen`).
                let scan = interface::scan_code(code);
                if let (true, false, Some(scan)) = (down, resting, scan.filter(|&s| interface::is_menu_key(s))) {
                    if !self.menu_held.iter().any(|h| h.0 == scan) {
                        self.menu_held.push((scan, false));
                    }
                    self.step();
                }
                // The view's K and Q: the bar's portrait and its crossed
                // swords, as a click on them.
                if down && matches!(code, KeyCode::KeyK | KeyCode::KeyQ) {
                    let (x, y) = sheets::bar_centre(if code == KeyCode::KeyK { 8 } else { 0 });
                    self.bar(x, y, false);
                    return;
                }
                let alt = self.engine.m.mem.r8(arena::mem::DS, arena::controls::var::KEYS + 0x38) != 0;
                let k = &mut self.keys;
                match code {
                    KeyCode::ArrowUp => k.forward = down,
                    KeyCode::ArrowDown => k.back = down,
                    KeyCode::ArrowLeft => k.turn_left = down,
                    KeyCode::ArrowRight => k.turn_right = down,
                    KeyCode::ControlLeft | KeyCode::ControlRight => k.ctrl = down,
                    KeyCode::KeyJ => k.jump = down,
                    KeyCode::ShiftLeft | KeyCode::ShiftRight => k.run = down,
                    // The view's extras, on keys the game leaves alone.
                    KeyCode::Space => k.space = down,
                    KeyCode::KeyF if down && k.run => world::summon_people(&mut self.engine),
                    KeyCode::KeyF if down => world::summon(&mut self.engine),
                    KeyCode::KeyE if down => self.click = true,
                    KeyCode::KeyI if down => self.inventory_key(),
                    // The view's quick save and load of slot --slot (the
                    // game's menus are the options screen's Save and
                    // Load); Alt+F5 is the game's debug key.
                    KeyCode::F5 if down && !alt => self.save(),
                    KeyCode::F9 if down => self.load(),
                    _ => {}
                }
            }
        }
    }

    /// The mouse captured: locked (or confined) and hidden in the window;
    /// with no window (a test), taken as captured.
    fn capture(&mut self) {
        match &self.gpu {
            Some(g) => {
                let ok = g
                    .window
                    .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                    .or_else(|_| g.window.set_cursor_grab(winit::window::CursorGrabMode::Confined))
                    .is_ok();
                g.window.set_cursor_visible(!ok);
                self.captured = ok;
            }
            None => self.captured = true,
        }
    }

    /// The time at `now`: the animations and creation's screens advanced,
    /// and a game turn when its time has come (18.2 timer ticks a second,
    /// a frame taking `frame_ticks`). While the game waits for the player,
    /// no time passes.
    fn tick(&mut self, now: Instant) {
        if !self.fade.is_empty() {
            self.fade_ms += now.duration_since(self.movie_at).as_millis() as u32;
            while self.fade_ms >= film::RETRACE_MS && self.fade.pop_front().is_some() {
                self.fade_ms -= film::RETRACE_MS;
            }
            if self.fade.is_empty() {
                self.fade_ms = 0;
                self.fade_old = None;
                if std::mem::take(&mut self.ending) {
                    self.game_over();
                }
            }
        }
        if let Some(mv) = self.movies.front_mut() {
            mv.advance(now.duration_since(self.movie_at).as_millis() as u32);
            if mv.over {
                self.movies.pop_front();
                self.movies_over();
            }
        }
        if let Some(c) = &mut self.creation {
            c.advance(&mut self.engine.m, now.duration_since(self.movie_at).as_millis() as u32);
            self.creation_outcome();
        }
        self.creation_music();
        // The travel map's passes and journey (the game's turns from the
        // next tick after it ends).
        let travelling = self.travel.is_some();
        if let Some(t) = &mut self.travel {
            t.advance(now.duration_since(self.movie_at).as_millis() as u32);
            // The province's loop's passes go on while no box waits.
            if self.boxes.is_empty() {
                t.idle(&mut self.engine);
            }
            self.travel_after();
        }
        self.movie_at = now;
        self.screen_ended();
        if now >= self.next && !travelling {
            self.step();
            let ticks = self.engine.frame_ticks() as f64;
            self.next = now + Duration::from_secs_f64(ticks / 18.2);
        }
        self.ring_screen();
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(winit::window::Window::default_attributes().with_title("Arena").with_inner_size(winit::dpi::LogicalSize::new(960, 600)))
                .expect("window"),
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).expect("device");
        let caps = surface.get_capabilities(&adapter);
        // Palette colours as they are: a non-sRGB format if there is one.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        // The window's size in pixels, as the cursor's positions are
        // given (a HiDPI screen may make it larger than asked for).
        self.size = (config.width as f64, config.height as f64);
        let renderer = self.assets.renderer(&device, &queue, format);
        let overlay = hud::Overlay::new(&device, format);
        self.gpu = Some(Gpu { window, surface, device, queue, config, renderer, overlay });
    }

    fn window_event(&mut self, el: &winit::event_loop::ActiveEventLoop, _id: winit::window::WindowId, event: winit::event::WindowEvent) {
        use winit::event::{ElementState, WindowEvent};
        use winit::keyboard::PhysicalKey;
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &mut self.gpu {
                    g.config.width = size.width.max(1);
                    g.config.height = size.height.max(1);
                    g.surface.configure(&g.device, &g.config);
                }
                self.input(In::Resized(size.width.max(1) as f64, size.height.max(1) as f64));
            }
            WindowEvent::CursorMoved { position, .. } => self.input(In::Cursor(position.x, position.y)),
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as f64,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y / 20.0,
                };
                self.input(In::Wheel(dy));
            }
            WindowEvent::MouseInput { state, button, .. } => self.input(In::Button { button, down: state == ElementState::Pressed }),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.input(In::Key { code, down: event.state == ElementState::Pressed, text: event.text.as_deref().map(str::to_string) });
                }
            }
            WindowEvent::RedrawRequested => {
                hud::blink_frame();
                self.field_frame();
                let cam = self.compose();
                if let Some(g) = &mut self.gpu {
                    let Ok(frame) = g.surface.get_current_texture() else {
                        g.surface.configure(&g.device, &g.config);
                        return;
                    };
                    let view = frame.texture.create_view(&Default::default());
                    let ticks = (self.start.elapsed().as_secs_f64() * 18.2) as usize;
                    let aspect = g.config.width as f32 / g.config.height.max(1) as f32;
                    let (sky, lights) = self.assets.drawn(&mut self.engine, aspect);
                    let sprites = self.assets.sprites(&mut self.engine, cam, ticks, aspect);
                    g.renderer.set_sky(sky);
                    g.renderer.set_lights(lights);
                    g.renderer.set_window(interface::window(&self.engine.m));
                    g.renderer.set_fade(self.fade.front().map(|f| &f.0));
                    g.renderer.draw(&g.device, &g.queue, &view, g.config.width, g.config.height, cam, &sprites);
                    g.overlay.draw(&g.device, &g.queue, &view, &self.screen);
                    frame.present();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &winit::event_loop::ActiveEventLoop, _id: winit::event::DeviceId, event: winit::event::DeviceEvent) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            self.input(In::Motion(delta.0, delta.1));
        }
    }

    fn about_to_wait(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        if self.quit {
            el.exit();
            return;
        }
        self.tick(Instant::now());
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
        el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(self.next));
    }
}
