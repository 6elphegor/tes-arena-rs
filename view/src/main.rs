//! arena-view: walk a level of Arena, drawn with wgpu, with the port's
//! engine running the game.
//!
//! Usage: arena-view GAME_DIR LEVEL.MIF [--fight] [--people] [--click] [--hour H] [--at COL ROW HEADING] [--wait N] [--do STEPS] [--picks N,..] [--save-dir DIR] [--slot N] [--shot OUT.png [FRAMES [TURN]]]
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
//! the right button (captured) and dragging swings it. --do gains
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
mod interface;
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
use render::{Camera, Renderer, Scene};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
}

impl Front<'_> {
    /// A message box with the text at DS:`si`; 965A's drawing (0000:98xx)
    /// leaves its colour and font codes in the game's state
    /// (`hud::text_codes`).
    fn message_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        match self.talk.as_deref_mut() {
            Some(t) => t.message_box(m, si),
            None => println!("{}", String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, si)).replace('\r', "\n")),
        }
        let text = m.mem.cstr(arena::mem::DS, si).to_vec();
        hud::text_codes(m, &text);
    }

    /// A menu (1194:020E): 3159:0F2A draws its title (DS:`si`) and its
    /// entries (the words at DS:`di`, to a 0), leaving their codes in the
    /// game's state as `message_box` does.
    fn menu(&mut self, m: &mut arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        let pick = match self.talk.as_deref_mut() {
            Some(t) => t.menu(m, si, bx, di),
            None => 0xFFFF,
        };
        let mut texts = vec![m.mem.cstr(arena::mem::DS, si).to_vec()];
        for i in 0..32u16 {
            let p = m.mem.r16(arena::mem::DS, di.wrapping_add(2 * i));
            if p == 0 {
                break;
            }
            texts.push(m.mem.cstr(arena::mem::DS, p).to_vec());
        }
        for t in texts {
            hud::text_codes(m, &t);
        }
        pick
    }

    fn note(&mut self, s: &str) {
        match self.talk.as_deref_mut() {
            Some(t) => t.note(s),
            None => println!("{s}"),
        }
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
/// MORPH.FLC (2294:0895), then JAGAR.FLC with text 5A7h in 39h
/// (1194:D459).
struct Audience<'a, 'b>(&'a mut Front<'b>);

impl arena::quest::QuestHooks for Audience<'_, '_> {
    fn audience(&mut self, _m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files, part: u8) -> Option<()> {
        if part == 0 {
            self.0.movie("MORPH.FLC", None, 0);
            self.0.movie("JAGAR.FLC", Some(0x5A7), 0x39);
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
}
impl ControlHooks for Front<'_> {
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
impl arena::weapon::WeaponHooks for Front<'_> {
    /// A building broken into by the swing.
    fn wall_trigger(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.wall_trigger(files, self)
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
/// Exhaustion's boxes, and the level-up screen with its notice and
/// refusal in boxes (its points spent in turn: no attribute screen yet).
/// A fight's hooks through the front end: Jagar's words before a main
/// quest's encounter (1194:D91C) played as the original plays them.
struct FightFront<'a, 'b>(&'a mut Front<'b>);

impl arena::encounter::FightHooks for FightFront<'_, '_> {
    fn jagar(&mut self, _m: &mut arena::machine::Machine, text: u16) {
        self.0.movie("JAGAR.FLC", Some(text), 0xF5);
    }
}

impl arena::clock::ClockHooks for Front<'_> {
    /// 1194:887A with the fight's hooks the front end's (`FightFront`).
    fn encounter(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.random_encounter(files, &mut FightFront(self))
    }
    fn exhaustion_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    fn level_up_screen(&mut self, m: &mut arena::machine::Machine, levels: u16) -> Option<()> {
        arena::levelup::level_up_around(m, levels, self)
    }
}
impl arena::levelup::LevelUpHooks for Front<'_> {
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
    /// 1194:A36C with AX 0: text number [137A] shown in its box (a
    /// building's words on the way in, 0000:4903; a holiday's; a quest's).
    fn message_box_number(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if m.text(files, self, 0, arena::mem::DS, 0).is_none() {
            eprintln!("the text gave up: {}", arena::giveup::describe());
        }
    }
    /// 0000:4ACD's welcome: ENTERCTY.$$$ (named at 130C, the arrival's
    /// text, `Machine::travel_arrival`) loaded to 1044 and shown in a box,
    /// what the box covers kept at the frame's start (0000:4B18) as the
    /// original keeps it; the view's screen isn't the game's 256 colours,
    /// so the kept pixels are black.
    fn town_welcome(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
        if m.file_load(files, 0x130C, arena::mem::DS, 0x1044).is_some() {
            if let Some(rect) = hud::centred_box(m, m.mem.cstr(arena::mem::DS, 0x1044)) {
                let frame = m.mem.r16(arena::mem::DS, 0x9030);
                m.box_background(frame, 0, rect, None);
            }
            self.message_box(m, 0x1044);
        }
    }
    /// 1194:DB6C with the view's own talk hooks.
    fn actor_click(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.actor_click(files, self)
    }
    /// 1194:4635: a frame of rest, while resting.
    fn rest(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.rest_frame(files, self)
    }
    /// 0000:05B5: the lines are drawn as the layer's (`hud::level_text`);
    /// what the drawing leaves is the row of the last (9E0A's [ADB8]),
    /// which the riddle's field is put under.
    fn level_text_draw(&mut self, m: &mut arena::machine::Machine) {
        if let Some(&(y, _)) = hud::level_text(m).last() {
            m.mem.w16(arena::mem::DS, arena::riddle::var::TEXT_Y, y as u16);
        }
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
    fn list(&mut self, m: &mut arena::machine::Machine, si: u16, cx: u16) -> u16 {
        match self.talk.as_deref_mut() {
            Some(t) => t.list(m, si, cx, ""),
            None => 0xFFFF,
        }
    }
    fn way(&mut self, m: &mut arena::machine::Machine, si: u16, x: u16, y: u16) {
        let s = format!("The way to {} ({x}, {y})", String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, si)));
        self.note(&s);
    }
    /// The loot window (2433:0583) with its boxes and lists answered
    /// through `dialog`.
    fn loot(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, ax: u16, si: u16, seg: u16, bx: u16) -> Option<()> {
        m.loot_window(files, &mut LootFront(self), ax, si, seg, bx)
    }
    /// 1194:D593: the quest's person speaks (`questui::QuestFront`).
    fn quest_person(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quest_person_talk(files, &mut questui::QuestFront(self, files.clone()))
    }
    /// 1194:8F1E (from DCE1): the palace's quests.
    fn palace(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quests_talk(files, &mut questui::QuestFront(self, files.clone()))
    }
    /// A shop entered: its questions through `dialog` (`shopui`).
    fn shop(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files, kind: u8) -> Option<()> {
        shopui::shop(self, m, files, kind)
    }
    /// A dungeon's keeper in the wilderness: in (0000:08EC).
    fn dungeon_entrance(&mut self, m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files) -> Option<()> {
        m.wild_dungeon_entry();
        Some(())
    }
}
/// The loot window's boxes and lists (1194:59CC, 5734), through the
/// front end's `dialog`.
struct LootFront<'a, 'b>(&'a mut Front<'b>);

impl arena::loot_window::LootHooks for LootFront<'_, '_> {
    fn message_box(&mut self, m: &mut arena::machine::Machine, si: u16) {
        self.0.message_box(m, si);
    }
    fn list(&mut self, m: &mut arena::machine::Machine, si: u16, cx: u16) -> u16 {
        match self.0.talk.as_deref_mut() {
            Some(t) => t.list(m, si, cx, ""),
            None => 0xFFFF,
        }
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
    fn movie(&mut self, m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files, si: u16) {
        if let Some(name) = Self::named(m, si) {
            self.0.movie(name, None, 0);
        }
    }
    fn speech(&mut self, m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files, ax: u16, _si: u16, di: u16) {
        if let Some(name) = Self::named(m, di) {
            self.0.movie(name, Some(ax), m.mem.r8(arena::mem::DS, 0x9288));
        }
    }
    fn house(&mut self, _m: &mut arena::machine::Machine, _files: &mut arena::bsa::Files) {
        self.0.movie("HOUSE.IMG", Some(0x579), 0x5C);
    }
}
/// Resting (src/rest.rs): its menu and the hours typed through `dialog`,
/// its texts boxes, the hours shown kept for the status line.
impl arena::rest::RestHooks for Front<'_> {
    fn menu(&mut self, m: &mut arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        Front::menu(self, m, si, bx, di)
    }
    /// 1194:46BF's 4800: the prompt's box, the field at FCh, 44h in
    /// ARENAFNT.DAT ([102C]) and colour 60h.
    fn type_hours(&mut self, m: &mut arena::machine::Machine) {
        if let Some(t) = self.talk.as_deref_mut() {
            let prompt = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, REST_PROMPT)).to_string();
            let look = field::Look { font: "ARENAFNT.DAT", colour: 0x60, put_font: true };
            t.edit(m, arena::lineedit::fields::rest(), look, dialog::Prompt::Boxed(prompt, m.mem.r8(arena::mem::DS, 0x9288), game_clip(m)));
        }
    }
    fn message(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    fn hours_text(&mut self, m: &mut arena::machine::Machine) {
        let s = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, arena::rest::var::TEXT)).into_owned();
        match self.talk.as_deref_mut() {
            Some(t) => t.status = Some(s),
            None => println!("{s}"),
        }
    }
    /// 1194:D3D4: the dream on CHAOSVSN.FLC, its text in 33h (the
    /// colour of the first level's dream; the others' are taken alike).
    fn dream(&mut self, _m: &mut arena::machine::Machine, text: u16) {
        self.movie("CHAOSVSN.FLC", Some(text), 0x33);
    }
}

/// "How many hours do you wish to rest?" (DGROUP), which 1194:4800 shows.
const REST_PROMPT: u16 = 0x4984;
impl EngineHooks for Front<'_> {
    /// The original stepped the changed squares it drew: here those
    /// within the view's reach (C394 squares) step, seen or not.
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
        // The click left for the actors (`view_click`, before the frame,
        // whose input step clears C39A bit 80h) stands while it is drawn,
        // as in the original, where the click comes in the input step: the
        // sprite under it becomes CB5E (0000:C4FA), made ready for 1194:DB6C.
        if let (true, Some(sheet)) = (std::mem::take(&mut self.clicked), self.sheet) {
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
}

/// The view's side of the day and night switch and the fog
/// (`LightHooks`): the row the textures are remapped through (0 as
/// loaded), palette entry 71h (the lamps' glow; PAL.COL's until set), the
/// light table's segment, and this frame's fog colour (filling the view
/// for the sky).
struct LightState {
    dir: PathBuf,
    dark: u32,
    lamp: Option<[u8; 3]>,
    table: u16,
    fog: Option<u8>,
    /// The sky's images loaded (`SkyLoad`).
    sky: SkyLoad,
}

/// The sky's images loaded as 0000:7428 loads them with DI FFFDh: the
/// file's size allocated in EMS (0000:5C26, which maps the bank), the
/// file read there. Their names are kept, by their offset in the EMS
/// frame, for the atlas.
struct SkyLoad {
    archive: Rc<archive::Archive>,
    images: HashMap<u16, String>,
}

impl arena::skyobj::SkyHooks for SkyLoad {
    fn load(&mut self, m: &mut arena::machine::Machine, name: u16) -> Option<(u16, u16)> {
        let file = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, name)).to_ascii_uppercase();
        let data = self.archive.read(&file)?;
        if data.is_empty() || data.len() > 0xFFFF {
            return None;
        }
        let (off, _) = m.ems_alloc(data.len() as u16)?;
        let frame = m.mem.r16(arena::mem::DS, 0x9030);
        for (i, &b) in data.iter().enumerate() {
            m.mem.w8(frame, off.wrapping_add(i as u16), b);
        }
        self.images.insert(off, file);
        Some((frame, off))
    }
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

impl LightState {
    /// The table freed and made again from the .LGT named at `name`.
    fn table(&mut self, m: &mut arena::machine::Machine, name: u16) {
        world::lgt(m, &self.dir, name);
        if self.table != 0 {
            m.heap_free(self.table);
        }
        self.table = world::light_table(m);
    }
}

impl arena::light::LightHooks for LightState {
    fn light_table(&mut self, m: &mut arena::machine::Machine) {
        self.table(m, arena::weather::NORMAL_LGT);
    }

    fn fog_light_table(&mut self, m: &mut arena::machine::Machine) {
        self.table(m, arena::weather::FOG_LGT);
    }

    fn lgt(&mut self, m: &mut arena::machine::Machine, name: u16) {
        world::lgt(m, &self.dir, name);
    }

    fn fog_fill(&mut self, m: &mut arena::machine::Machine) {
        self.fog = Some(m.mem.r8(arena::mem::DS, arena::weather::var::FILL_COLOUR));
    }

    /// 0000:027A: the sky's objects made again, when there are any (their
    /// images are then remapped with the textures: the shader's `dark`).
    fn relight(&mut self, m: &mut arena::machine::Machine, level: u16) {
        use arena::skyobj::var::{CLOUD_COUNT, OBJECT_COUNT};
        if m.mem.r8(arena::mem::DS, OBJECT_COUNT) | m.mem.r8(arena::mem::DS, CLOUD_COUNT) != 0 {
            m.mem.w8(arena::mem::DS, OBJECT_COUNT, 0);
            m.mem.w8(arena::mem::DS, CLOUD_COUNT, 0);
            arena::light::LightHooks::sky_objects(self, m);
        }
        self.dark = (level >> 8) as u32;
    }

    fn restore_textures(&mut self, _m: &mut arena::machine::Machine) {
        self.dark = 0;
    }

    /// 2294:08B9, the sky's objects made again (by day, and before
    /// relighting by night).
    fn sky_objects(&mut self, m: &mut arena::machine::Machine) {
        self.sky.images.clear();
        m.sky_objects(&mut self.sky);
    }

    /// Only entry 71h is written through here; its 6-bit colour made 8-bit.
    fn palette(&mut self, m: &mut arena::machine::Machine, index: u8, at: u16) {
        if index == 0x71 {
            let c = |i: u16| m.mem.r8(arena::mem::DS, at + i) & 0x3F;
            self.lamp = Some([0, 1, 2].map(|i| c(i) << 2 | c(i) >> 4));
        }
    }
}

impl Assets {
    /// The colours on the screen as the original's DAC holds them: PAL.COL
    /// but, after a frame outdoors, entries 1..8 the sky's colours for the
    /// time of day (2294:0341) and, once the game has set it, 71h the
    /// lamps' (0000:9B5B): anything drawn in those colours shows them.
    fn dac(&self) -> [[u8; 4]; 256] {
        let mut p = self.palette;
        if let Some((sky, _, _)) = &self.frame_view {
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

    /// Also enters the area's sky (0000:464A: its objects made).
    fn new(dir: &std::path::Path, engine: &mut Engine) -> Self {
        prof::time("assets.new", || Self::make(dir, engine))
    }

    fn make(dir: &std::path::Path, engine: &mut Engine) -> Self {
        let flats = prof::time("assets.flats", || world::flats(&engine.m));
        let archive = prof::time("assets.archive", || Rc::new(archive::Archive::open(dir)));
        let sheet = prof::time("assets.sheet", || world::sprite_sheet(&flats, &archive, &world::creature_names(&engine.m)));
        interface::enter(&mut engine.m);
        // Outdoors, the sky's images (the sun, the moons, the lightning's
        // bolts, the stars) into the sky's bank, as entering the area does
        // (0000:A4E9 calls 0000:E5C9).
        if prof::time("assets.sky_images", || engine.m.sky_images(&mut engine.files)).is_none() {
            eprintln!("the sky's images did not load");
        }
        let mut sky = SkyLoad { archive: archive.clone(), images: HashMap::new() };
        prof::time("assets.area_sky", || engine.m.area_sky(&mut sky));
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
            light: LightState { dir: dir.to_path_buf(), dark: 0, lamp: None, table: engine.m.mem.r16(arena::mem::DS, 0xCCC2), fog: None, sky },
            flash: std::cell::Cell::new(None),
            frame_view: None,
            moons: [0, 0],
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

    /// The sky after a frame (2294:0428, then 0000:0390 with 2294:0341;
    /// the .COL loads here): outdoors, palette entries 1..8 from the day's
    /// colours at the window for the time of day, the textures darkened by
    /// night and the lamps lit.
    fn sky(&mut self, m: &mut arena::machine::Machine) -> render::Sky {
        use arena::sky::var::{COLOURS, DITHER, DITHER_AFTERNOON, WINDOW};
        let outdoors = m.mem.r16(arena::mem::DS, 0xC398) & 4 != 0;
        if !outdoors {
            return render::Sky::default();
        }
        m.sky_colours();
        m.outdoor_light(&mut self.light);
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
    /// light. None in full daylight (C398 bit 8000h).
    fn lights(&mut self, m: &mut arena::machine::Machine) -> render::Lights {
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

    /// After a game frame: the frame's sky and lights (`light_frame`), then
    /// the sky's hook (2294:0A5D: its objects and the lightning, the flash
    /// filling the sky) for a view `aspect` wide, kept for the redraws
    /// until the next frame.
    fn view_frame(&mut self, engine: &mut Engine, aspect: f32) {
        let (mut sky, lights) = self.light_frame(&mut engine.m);
        let focal = world::TAN_HALF * aspect;
        // 0000:D4FA..D50D: outdoors, not in fog (C39A bit 20h), the sky's
        // bodies, then the sky's hook.
        let mut bodies = Vec::new();
        let flags = (engine.m.mem.r16(arena::mem::DS, 0xC398), engine.m.mem.r16(arena::mem::DS, 0xC39A));
        if flags.0 & 4 != 0 && flags.1 & 0x20 == 0 {
            let mut cd = CelestialDraw { sheet: &self.sheet, focal, moons: &mut self.moons, out: Vec::new() };
            let ok = engine.m.celestial(&mut cd).is_some();
            if ok {
                bodies = cd.out;
            }
        }
        let mut draw = SkyDraw { sheet: &self.sheet, images: &self.light.sky.images, focal, fog: self.light.fog.is_some(), out: Vec::new(), flash: None };
        if engine.m.sky_draw(&mut draw).is_some() {
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

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    MUTE.store(args.iter().any(|a| a == "--mute"), std::sync::atomic::Ordering::Relaxed);
    SOUND_LOG.store(args.iter().any(|a| a == "--sound-log"), std::sync::atomic::Ordering::Relaxed);
    NO_MUSIC.store(args.iter().any(|a| a == "--no-music"), std::sync::atomic::Ordering::Relaxed);
    args.retain(|a| a != "--mute" && a != "--sound-log" && a != "--no-music");
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
    let started = Instant::now();
    let mut engine = prof::time("start.load", || world::load(&dir, &args[2]));
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
                c.input(&mut engine, inp);
            }
            if c.done() {
                newgame::play(&mut engine);
            } else {
                creation = Some(c);
            }
            arena::transit::Run::Start
        }
        (None, None) => areas::start(&mut engine, args[2].starts_with("CITY:")),
    };
    interface::start(&mut engine.m);
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
        shot(&dir, engine, run, out, click, wait, frames, turn, &steps, answers, save_dir.as_deref().map(|d| (d, slot)), &maker_script);
        return;
    }
    let play = !MUTE.load(std::sync::atomic::Ordering::Relaxed);
    let mut app = make_app(dir, engine, run, creation, answers, save_dir, slot, play);
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
        camp: false,
        key: false,
        message: None,
        next: Instant::now(),
        last: Some(arena::engine::Frame::Running),
        dir,
        run,
        script: answers,
        pending: None,
        box_backdrop: None,
        boxes: Default::default(),
        selected: 0,
        arrowed: false,
        live: None,
        status: None,
        status_held: false,
        font,
        screen,
        cursor: (0.0, 0.0),
        size: (960.0, 600.0),
        drag: (0.0, 0.0),
        save_dir,
        slot,
        panel: None,
        travel: None,
        saveui: None,
        saveui_under: None,
        options: None,
        quit: false,
        equip: None,
        map: false,
        creation,
        creation_engine: None,
        movie_at: Instant::now(),
        fade: Default::default(),
        fade_ms: 0,
        fade_old: None,
        ending: false,
        blackout: false,
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
    key: bool,
    tick: usize,
    right: bool,
    drag: (i16, i16),
    /// The window's width over its height (the click's line).
    aspect: f32,
    /// A click on the automap screen's map at this point (0000:31FE): the
    /// note there edited or one begun, its text asked for; no frame runs.
    note: Option<(u16, u16)>,
}

/// What a turn did besides the engine's state: how its frame ended, and
/// whether the level's assets must be made again (stairs, the
/// wilderness's blocks).
struct TurnOut {
    frame: Option<arena::engine::Frame>,
    level_changed: bool,
}

/// One turn (`dialog`): camping if asked, then a frame, with `talk`
/// answering the game's questions.
fn run_turn(engine: &mut Engine, assets: &mut Assets, input: &TurnIn, talk: &mut dialog::Talk) -> TurnOut {
    use arena::mem::DS;
    if let Some((x, y)) = input.note {
        // The automap screen is up: the world waits (no frame).
        engine.m.mem.w16(DS, arena::mapnotes::var::CLICK_X, x);
        engine.m.mem.w16(DS, arena::mapnotes::var::CLICK_Y, y);
        let mut hooks = Front { sheet: Some(&assets.sheet), light: Some(&mut assets.light), tick: input.tick, clicked: false, talk: Some(talk), aspect: input.aspect };
        if engine.m.note_click(&mut engine.files, &mut hooks).is_none() {
            eprintln!("the note met a fault of the original's");
        }
        return TurnOut { frame: Some(arena::engine::Frame::Running), level_changed: false };
    }
    let t = engine.frame_ticks();
    engine.tick(t);
    let level = engine.m.mem.r8(DS, arena::movement::var::LEVEL);
    let window = engine.m.mem.r32(DS, arena::wild::var::WINDOW);
    let mut hooks = Front { sheet: Some(&assets.sheet), light: Some(&mut assets.light), tick: input.tick, clicked: input.clicked, talk: Some(talk), aspect: input.aspect };
    if input.camp && engine.m.camp(&mut engine.files, &mut hooks).is_none() {
        eprintln!("camping gave up: {}", arena::giveup::describe());
    }
    if let Some(kind) = input.shop {
        if shopui::shop(&mut hooks, &mut engine.m, &mut engine.files, kind).is_none() {
            eprintln!("the shop gave up: {}", arena::giveup::describe());
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
    if frame == Some(arena::engine::Frame::Running) {
        creature_flats(engine, &mut hooks);
    }
    let stairs = engine.m.mem.r8(DS, arena::movement::var::LEVEL) != level;
    let level_changed = stairs || wild_moved(engine, window);
    prof::time("view_frame", || assets.view_frame(engine, input.aspect));
    TurnOut { frame, level_changed }
}

/// The drawing's creatures' flats (0000:CF96): each one reached goes to
/// the hook CD56 (25CA:0271, `Machine::flat_creature`) with its square's
/// centre, its number and its word, as the original's drawing calls it.
fn creature_flats(engine: &mut Engine, hooks: &mut Front<'_>) {
    use arena::mem::DS;
    if engine.m.mem.r32(DS, 0xCD56) != 0x25CA_0271 {
        return;
    }
    let seg = engine.m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    for (col, row, di) in world::creature_flats_seen(&engine.m) {
        let flat = engine.m.mem.r8(seg, di);
        let (x, y) = (col << 7 | 0x40, row << 7 | 0x40);
        if engine.m.flat_creature(&mut engine.files, &mut FightFront(hooks), x, y, flat, seg, di).is_none() {
            eprintln!("a creature's flat gave up: {}", arena::giveup::describe());
            return;
        }
    }
}

/// The animations a turn played (`dialog::Talk::movies`), made: a text's
/// lines through the text port (1194:D459), the files from the game's.
fn movies_of(engine: &mut Engine, archive: &archive::Archive, played: Vec<(&'static str, Option<u16>, u8)>) -> Vec<flc::Movie> {
    played
        .into_iter()
        .filter_map(|(name, text, colour)| {
            // Its song (the routines that show them take one).
            if let Some(song) = music::movie_song(name) {
                engine.m.music_song(song);
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
            flc::Movie::new(archive, name, lines)
        })
        .collect()
}

/// The engine copied, for a turn to run again (`dialog`).
fn copy(engine: &Engine) -> Engine {
    prof::time("turn.copy", || Engine { m: engine.m.clone(), files: engine.files.clone() })
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
    boxes: Vec<Vec<Vec<hud::Span>>>,
    need: Option<dialog::Need>,
    /// The spellmaker's screen when the turn stopped inside it.
    maker_screen: Option<(dialog::Snapshot, Option<u16>)>,
    /// The shop's screen behind the question asked (`Talk::backdrop`).
    backdrop: Option<shopui::Backdrop>,
    status: Option<String>,
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
        intent: *intent,
        clicked: std::mem::take(&mut st.clicked),
        camp: std::mem::take(&mut st.camp),
        shop: st.shop.take(),
        loot: None,
        key: false,
        tick: st.frames * 4,
        right: std::mem::take(&mut st.right),
        drag: std::mem::take(&mut st.drag),
        aspect: 1.6,
        note: st.note.take(),
    };
    let mut talk = dialog::Talk { answers: std::mem::take(&mut st.answers), ..Default::default() };
    st.audio.before(&mut engine.m);
    st.music.before(&mut engine.m);
    let ticks = engine.frame_ticks() as u32;
    let out = run_turn(engine, &mut st.assets, &input, &mut talk);
    st.audio.after(&mut engine.m);
    let played = std::mem::take(&mut talk.movies);
    let archive = st.assets.archive.clone();
    st.movies.extend(movies_of(engine, &archive, played));
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
            st.ui = Some((c, world::load(dir, "START.MIF")));
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
    // the turns show are sized in it (the welcome's keeps what it covers,
    // `hud::centred_box`).
    hud::init_style(&assets.archive, &engine.m);
    // Shots play nothing; with --sound-log they print what would play.
    let audio = audio::Audio::new(assets.archive.clone(), false, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let music = music::Music::new(assets.archive.clone(), false, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let st = &mut Shot { audio, music, movies: Vec::new(), assets, right: false, drag: (0, 0), clicked: false, camp: false, shop: None, answers, message: None, frames: 0, boxes: Vec::new(), need: None, maker_screen: None, backdrop: None, status: None, panel: None, map: false, over: false, ui: None, travel: None, cursor: (160, 100), note: None };
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
                match travel::Travel::open(&mut engine, &archive) {
                    Ok(t) => st.travel = Some(t),
                    Err(b) => st.boxes = vec![b],
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
                    t.key(&mut engine, code, text);
                }
            }
            if let Some(t) = &mut st.travel {
                t.skip_journey();
            }
            let mut boxes = Vec::new();
            travel::after(&mut st.travel, &mut engine, &mut boxes, true);
            if !boxes.is_empty() {
                st.boxes = boxes;
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
                    Some(ms) => c.advance(ms.parse().expect("--do ui:ms:N")),
                    None => {
                        for inp in create::parse_script(t) {
                            c.input(e, inp);
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
            st.movies.extend(movies_of(&mut engine, &archive, vec![(name, text, colour)]));
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
                engine.m.drop_place();
                st.panel = Some(panels::open_inventory(&mut engine.m));
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
                    "equip" => panels::equip_line(&mut engine.m, n as usize, &mut said),
                    "select" => panels::select_line(&mut engine.m, n as usize),
                    // The drop button, its question (if it asks) answered
                    // yes, or no with drop:1.
                    "drop" => {
                        let mut inv = panels::Inventory::default();
                        panels::drop_press(&mut engine.m, &mut inv, &mut said);
                        if inv.asking.is_some() {
                            panels::drop_answer(&mut engine.m, n == 0, &mut said);
                        }
                    }
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
            "usable" => {
                if let Some(panels::Panel::Inventory(inv)) = st.panel.as_mut() {
                    let names = panels::usable_names(&engine.m);
                    println!("usable: {names:?}");
                    inv.using = (!names.is_empty()).then_some((names, 0));
                }
            }
            "book" => {
                for (i, sp) in panels::spells(&engine.m).iter().enumerate() {
                    println!("spell {i}: {} ({}), cost {}", sp.name, sp.index, sp.cost);
                }
            }
            "map" => {
                if st.map {
                    automap::close(&mut engine.m);
                } else {
                    sheets::pics(&st.assets.archive, &engine.m);
                    automap::open(&mut engine, &st.assets.archive);
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
                st.panel = Some(panels::Panel::Sheet);
            }
            "page" => st.panel = Some(panels::equipment_page(&mut engine.m)),
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
                        automap::open(&mut engine, &st.assets.archive);
                        st.map = true;
                    }
                    Some(sheets::BarAction::Status(lines)) => st.boxes = vec![lines],
                    Some(sheets::BarAction::Spells) => st.panel = Some(panels::Panel::Spellbook(Default::default())),
                    Some(sheets::BarAction::Log) => st.panel = Some(panels::Panel::Log(sheets::open_log(&engine.m, &engine.files))),
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
                    save_to(&mut engine, sdir, slot, st.assets.palette)
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
                st.boxes = vec![vec![vec![hud::Span { colour: 0x0F, text: msg, font: Default::default(), code: None }]]];
            }
            "key" => {
                // A key (its scan code) down for a frame.
                engine.m.set_key(n as u8, true);
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
                engine.m.set_key(n as u8, false);
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
            let hud = hud::Hud { boxed: st.boxes.last().cloned(), ..Default::default() };
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
        let mut hud = hud::Hud { message: message_line(&engine), boxed: st.boxes.last().cloned(), status: None, level: hud::level_text(&engine.m), clip: game_clip(&engine.m), ..Default::default() };
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
            Some(dialog::Need::Spend(_) | dialog::Need::Spellmaker | dialog::Need::Travel(_)) | None => {}
        }
        let draw_live = |screen: &mut hud::Screen| {
            if let Some(l) = &live {
                l.draw(screen, true);
            }
        };
        let sheet = st.panel.as_ref().is_some_and(|p| !p.over_play());
        let mut screen = hud::Screen::new(if sheet { st.assets.charsheet } else { st.assets.palette });
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
fn centre_click(engine: &mut Engine, boxes: &mut Vec<Vec<Vec<hud::Span>>>) -> bool {
    click_at(engine, boxes, (164, 74))
}

/// A left click at (x, y) of the 320 x 200 screen in the view: the mouse
/// put 4 up and left of it, as the click (CB60, 2826:0A52) is the cursor
/// + 4; else as `centre_click`.
fn click_at(engine: &mut Engine, boxes: &mut Vec<Vec<Vec<hud::Span>>>, (x, y): (i32, i32)) -> bool {
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
/// The screen saved with the level (0xFA00 bytes from A8FE, the original's
/// screen) is left black: the view's picture isn't in the machine.
struct SaveDir<'a> {
    dir: &'a std::path::Path,
    palette: [[u8; 4]; 256],
    /// The files written (their DOS names, their bytes), for the game's
    /// `Files` to have them too (`save_to`).
    written: Vec<(Vec<u8>, Vec<u8>)>,
}

impl SaveDir<'_> {
    fn path(&self, m: &arena::machine::Machine, path: u16) -> PathBuf {
        let p = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, path)).into_owned();
        let name = p.rsplit(['\\', '/']).next().unwrap_or(&p).to_string();
        self.dir.join(name)
    }
}

impl arena::save_write::SaveHooks for SaveDir<'_> {
    fn delete(&mut self, m: &arena::machine::Machine, path: u16) {
        let _ = std::fs::remove_file(self.path(m, path));
    }
    fn write(&mut self, m: &arena::machine::Machine, path: u16, data: &[u8]) {
        let p = self.path(m, path);
        let mut data = data.to_vec();
        if p.file_name().is_some_and(|n| n.to_string_lossy().to_ascii_uppercase().starts_with("SAVEGAME")) {
            let n = data.len().min(0xFA00);
            data[..n].fill(0);
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
fn save_to(engine: &mut Engine, dir: &std::path::Path, slot: u8, palette: [[u8; 4]; 256]) -> String {
    if let Err(e) = std::fs::create_dir_all(dir) {
        return format!("{}: {e}", dir.display());
    }
    // The saved position (BA62: x, z, y), which saving the level makes the
    // player's (the world loop keeps it each frame, A2A3; the view's
    // frames don't): the player's.
    use arena::save_write::var::{POSITION, SAVED_POSITION};
    for i in 0..3 {
        let w = engine.m.mem.r16(arena::mem::DS, POSITION + 2 * i);
        engine.m.mem.w16(arena::mem::DS, SAVED_POSITION + 2 * i, w);
    }
    engine.m.mem.w8(arena::mem::DS, arena::save::var::SLOT, slot);
    // As 25FD:010B..0110: the automap written (23F1:0145), the working
    // slot's files (CITYDATA, SPELLS, AUTOMAP, ...) copied to the slot's
    // (25FD:0612) and kept on disk with the save; then the save itself.
    engine.m.automap_write(&mut engine.files);
    for name in engine.m.slot_files_out(&mut engine.files) {
        let Some(data) = engine.files.written(&name) else { continue };
        let p = dir.join(String::from_utf8_lossy(&name).into_owned());
        if let Err(e) = std::fs::write(&p, data) {
            eprintln!("{}: {e}", p.display());
        }
    }
    let mut out = SaveDir { dir, palette, written: Vec::new() };
    engine.m.save_game(&mut out);
    // The game's files have them too: a load reads the files written in
    // play before the directories' (`Files`), and the slot's copies made
    // above (25FD:0612 copies every "*.65", SAVEGAME's and SAVEENGN's of an
    // earlier load included) would otherwise hide the save just made.
    for (name, data) in out.written {
        engine.files.put(&name, &data);
    }
    // The save's end (25FD:01CB..020C): the wilderness's blocks copied to
    // the slot's names (kept on disk too), the slot back to the working
    // slot, an interior's automap loaded.
    for name in engine.m.save_finish(&mut engine.files) {
        let Some(data) = engine.files.written(&name) else { continue };
        let p = dir.join(String::from_utf8_lossy(&name).into_owned());
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
    /// The shop's screen behind the boxes a turn left after the shop ended
    /// (`Talk::backdrop`): the original's box (42EE) waits over the screen
    /// before the shop's way out (stealing, the guards called).
    box_backdrop: Option<shopui::Backdrop>,
    boxes: std::collections::VecDeque<Vec<Vec<hud::Span>>>,
    selected: usize,
    /// The arrows or the wheel moved `selected` since the menu came up:
    /// only then is its entry marked (the original marks none).
    arrowed: bool,
    live: Option<field::Live>,
    status: Option<String>,
    /// `status` kept under the boxes of the turn that drew it last.
    status_held: bool,
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
        if self.movies.is_empty() {
            self.engine.m.music_resume();
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
                self.engine.m.music_song(s);
                self.music.after(&mut self.engine.m, 0);
            }
        }
    }

    /// A key or click for creation; at its end the game started, in the
    /// start dungeon, its assets made again.
    fn creation_input(&mut self, inp: create::Input) {
        let Some(c) = &mut self.creation else { return };
        match &mut self.creation_engine {
            Some(e) => c.input(e, inp),
            None => c.input(&mut self.engine, inp),
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
                newgame::play(&mut self.engine);
                self.run = arena::transit::Run::Start;
                self.assets = Assets::new(&self.dir, &mut self.engine);
                self.remake_renderer();
                self.last = Some(arena::engine::Frame::Running);
            }
            create::Outcome::Load => {
                // The load menu from the title (`saveui`), then the
                // title-screen load (`areas::load_game_title`).
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

    /// The game over (5B1 bit 100h): the farewell's vision, then the main
    /// menu, on a fresh engine (the start dungeon loaded, as a new game
    /// or a load begins from the program's own state).
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
    /// the vision played, then the main menu.
    fn game_over(&mut self) {
        let archive = self.assets.archive.clone();
        let mut c = create::Creation::ending(&mut self.engine, archive, &self.dir, self.assets.palette, None);
        c.faded();
        self.engine = world::load(&self.dir, "START.MIF");
        self.creation = Some(c);
        self.movie_at = Instant::now();
    }

    /// Whether the game waits for the player (a box, a question).
    fn waiting(&self) -> bool {
        self.pending.is_some() || !self.boxes.is_empty()
    }

    /// One engine turn with the keys and the mouse as an Intent.
    fn step(&mut self) {
        if self.waiting() || !self.fade.is_empty() || self.panel.is_some() || self.creation.is_some() || !self.movies.is_empty() || self.travel.is_some() || self.saveui.is_some() || self.options.is_some() {
            return;
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
            key: std::mem::take(&mut self.key),
            tick,
            right: self.keys.swing,
            drag: (drag.0.round() as i16, drag.1.round() as i16),
            aspect: (self.size.0 / self.size.1.max(1.0)) as f32,
            note: None,
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
        // A click now is found by the next frame's drawing.
        if !self.waiting() && std::mem::take(&mut self.click) {
            let mut boxes = Vec::new();
            self.clicked = match self.click_point.take() {
                Some(at) => click_at(&mut self.engine, &mut boxes, at),
                None => centre_click(&mut self.engine, &mut boxes),
            };
            self.boxes.extend(boxes);
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
        let mut talk = dialog::Talk { answers, ..Default::default() };
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
            self.movies.extend(movies_of(&mut self.engine, &archive, played));
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
        if let Some(need) = talk.need {
            copy_into(&mut self.engine, &before);
            self.selected = 0;
            self.arrowed = false;
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
                a.pending = Some(Pending { before, input, given, need, maker, backdrop });
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
        print_message(&self.engine, &mut self.message);
        if out.level_changed {
            self.assets.level(&self.engine);
            self.remake_renderer();
        }
        let frame = out.frame;
        if frame != self.last {
            match frame {
                Some(arena::engine::Frame::Running) => {}
                Some(arena::engine::Frame::Left) => {
                    // On to the next area, whose assets are made afresh,
                    // its colours faded (`area_fade`).
                    let was = self.fade_from();
                    let blackout = std::mem::take(&mut self.blackout);
                    if self.area_next() {
                        self.area_fade(was, blackout);
                        return;
                    }
                    self.fade = was.0;
                    if self.engine.m.mem.r16(arena::mem::DS, 0x05B1) & 0x100 != 0 {
                        self.game_ending();
                        return;
                    }
                }
                None => eprintln!("the frame ended: None ({})", arena::giveup::describe()),
            }
            self.last = frame;
        }
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
        if prof::on() {
            prof::add("redraw.screen", t.elapsed());
        }
        cam
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
            Some(dir) => save_to(&mut self.engine, &dir, self.slot, self.assets.palette),
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
        let (ui, e) = saveui::SaveUi::open(kind, &self.engine, &dir, &self.assets.archive);
        self.saveui = Some(ui);
        self.saveui_ended(e);
    }

    /// What the menu's last input ended in: still open, a slot to save
    /// (`save_to` on the engine after the menu), a slot to load
    /// (in play `areas::load_game`, from the title `load_game_title`), or
    /// the menu left (a title's back to the main menu).
    fn saveui_ended(&mut self, e: saveui::Ended) {
        let Some(kind) = self.saveui.as_ref().map(|u| u.kind) else { return };
        let Some(dir) = self.save_dir.clone() else { return };
        let title = kind == saveui::Kind::TitleLoad;
        match e {
            saveui::Ended::Open => return,
            saveui::Ended::Save(engine, slot) => {
                self.engine = engine;
                let msg = save_to(&mut self.engine, &dir, slot, self.assets.palette);
                self.say(&msg);
            }
            saveui::Ended::Load(mut engine, slot) => {
                let game = self.dir.clone();
                let r = if title { areas::load_game_title(&mut engine, &game, &dir, slot) } else { areas::load_game(&mut engine, &game, &dir, slot) };
                match r {
                    Ok(run) => {
                        self.engine = engine;
                        self.creation = None;
                        self.run = run;
                        self.assets = Assets::new(&self.dir, &mut self.engine);
                        self.remake_renderer();
                        self.last = Some(arena::engine::Frame::Running);
                        self.say(&format!("Loaded slot {slot}."));
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
            saveui::Ended::Left(msg) => {
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
        self.engine = *engine;
        // The keys let go while the screen was up went to the engine it
        // replaced: the game's key state and latches let go here.
        for scan in 1..0x60u8 {
            self.engine.m.set_key(scan, false);
            self.engine.m.key_pressed(scan as u16);
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
            arena::options::OptionsEnd::NewGame => {
                let archive = self.assets.archive.clone();
                let c = create::Creation::main_menu(&mut self.engine, archive, &self.dir, self.assets.palette);
                self.engine = world::load(&self.dir, "START.MIF");
                self.creation = Some(c);
                self.movie_at = Instant::now();
            }
            arena::options::OptionsEnd::Quit => self.quit = true,
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
        match travel::Travel::open(&mut self.engine, &self.assets.archive) {
            Ok(t) => {
                // 26F7:0000: the screen faded out in 8 steps, the map's
                // colours faded in in 6.
                self.fade_open(8, 6, |a| a.travel = Some(t));
                if self.captured {
                    self.release();
                }
            }
            Err(b) => self.boxes.push_back(b),
        }
        self.travel_after();
    }

    /// An input to the travel map, then its boxes shown and its end
    /// followed (`travel::after`).
    fn travel_input(&mut self, f: impl FnOnce(&mut travel::Travel, &mut Engine, &archive::Archive)) {
        let Some(shown) = self.travel.as_ref().map(|t| t.shown()) else { return self.travel_after() };
        let was = self.fade_from();
        if let Some(t) = &mut self.travel {
            f(t, &mut self.engine, &self.assets.archive);
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
        let carry = travel::after(&mut self.travel, &mut self.engine, &mut boxes, !forced);
        self.boxes.extend(boxes);
        if let (true, Some(carry)) = (forced, carry) {
            let world = dialog::World(Box::new((self.engine.m.clone(), self.engine.files.clone())));
            self.answer(dialog::Answer::Travel(world, carry));
        }
    }

    /// I and C: the inventory or the spellbook up, or down.
    fn toggle_panel(&mut self, inventory: bool) {
        let open = match &self.panel {
            Some(panels::Panel::Inventory(_)) => inventory,
            Some(panels::Panel::Spellbook(_)) => !inventory,
            Some(_) | None => false,
        };
        if open {
            self.panel = None;
            return;
        }
        // The inventory as the sheet's page would show it: where the
        // player stands checked first, as the portrait does (1194:5F13).
        if inventory {
            self.engine.m.drop_place();
        }
        self.panel = Some(if inventory { panels::open_inventory(&mut self.engine.m) } else { panels::Panel::Spellbook(Default::default()) });
        if self.captured {
            self.release();
        }
    }

    /// EQUIP.IMG, loaded the first time.
    fn equip_image(&mut self) {
        if self.equip.is_none() {
            self.equip = Some(panels::load_image(&self.assets.archive, &self.engine.m, "EQUIP.IMG"));
        }
    }

    /// A key with a panel up: see `panels`.
    fn panel_key(&mut self, code: winit::keyboard::KeyCode) {
        use winit::keyboard::KeyCode;
        let mut said = panels::Said::default();
        let m = &mut self.engine.m;
        let mut cast = None;
        let mut page = false;
        match self.panel.as_mut() {
            Some(panels::Panel::Inventory(inv)) => {
                // A spell's page waits for clicks (51D1's 8FC4 loop); its
                // Delete question takes its keys too (020E: Y, N at 4B60).
                if let Some(page) = inv.page.clone() {
                    let a = match code {
                        KeyCode::KeyY if page.asking.is_some() => panels::PageAnswer::Confirm(0),
                        KeyCode::KeyN if page.asking.is_some() => panels::PageAnswer::Confirm(1),
                        _ => return,
                    };
                    self.page_answer(page, a);
                    return;
                }
                // An item's description waits for a key or a click
                // (1194:5536: ACB0 or AFF9), which does nothing more.
                if inv.info.take().is_some() {
                    inv.info_y = None;
                    return;
                }
                if let Some((names, sel)) = inv.using.as_mut() {
                    let n = names.len().max(1);
                    match code {
                        KeyCode::ArrowUp => *sel = (*sel + n - 1) % n,
                        KeyCode::ArrowDown => *sel = (*sel + 1) % n,
                        KeyCode::Enter | KeyCode::NumpadEnter => {
                            let pick = *sel as u16;
                            inv.using = None;
                            panels::use_pick(m, &mut self.engine.files, pick, &mut said);
                        }
                        KeyCode::Escape => inv.using = None,
                        _ => {}
                    }
                } else if let Some((_, sel)) = inv.asking.as_mut() {
                    // The drop's yes or no (020E: its keys Y and N, Esc no).
                    let answer = match code {
                        KeyCode::ArrowUp | KeyCode::ArrowDown => {
                            *sel ^= 1;
                            None
                        }
                        KeyCode::Enter | KeyCode::NumpadEnter => Some(*sel == 0),
                        KeyCode::KeyY => Some(true),
                        KeyCode::KeyN | KeyCode::Escape => Some(false),
                        _ => None,
                    };
                    if let Some(yes) = answer {
                        inv.asking = None;
                        panels::drop_answer(m, yes, &mut said);
                    }
                } else {
                    match code {
                        KeyCode::Escape | KeyCode::KeyI => {
                            let back = inv.from_sheet;
                            self.panel = back.then_some(panels::Panel::Sheet)
                        },
                        KeyCode::ArrowUp => panels::scroll(m, -1),
                        KeyCode::ArrowDown => panels::scroll(m, 1),
                        KeyCode::KeyD => panels::drop_press(m, inv, &mut said),
                        KeyCode::KeyU => {
                            let names = panels::usable_names(m);
                            if !names.is_empty() {
                                inv.using = Some((names, 0));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some(panels::Panel::Spellbook(book)) => {
                let n = panels::spells(m).len().max(1);
                match code {
                    KeyCode::Escape | KeyCode::KeyC => self.panel = None,
                    KeyCode::ArrowUp => book.selected = (book.selected + n - 1) % n,
                    KeyCode::ArrowDown => book.selected = (book.selected + 1) % n,
                    KeyCode::Enter | KeyCode::NumpadEnter => cast = Some(book.selected),
                    _ => {}
                }
            }
            Some(panels::Panel::Use(names, sel)) => {
                let n = names.len().max(1);
                match code {
                    KeyCode::Escape | KeyCode::KeyU => self.panel = None,
                    KeyCode::ArrowUp => *sel = (*sel + n - 1) % n,
                    KeyCode::ArrowDown => *sel = (*sel + 1) % n,
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        let pick = *sel as u16;
                        self.panel = None;
                        panels::use_pick(m, &mut self.engine.files, pick, &mut said);
                    }
                    _ => {}
                }
            }
            Some(panels::Panel::Sheet) => match code {
                // 3276:52B0's records: 'N' the equipment page, Esc leaves.
                KeyCode::KeyN => page = true,
                KeyCode::Escape | KeyCode::KeyK => self.panel = None,
                _ => {}
            },
            Some(panels::Panel::Log(log)) => match code {
                KeyCode::Escape | KeyCode::KeyL => self.panel = None,
                _ if log.page + 1 < log.pages.len() => log.page += 1,
                _ => {}
            },
            None => {}
        }
        if page {
            self.equipment_page();
        }
        if let Some(line) = cast {
            self.panel = None;
            let sel = panels::book_pick(&mut self.engine.m, line);
            panels::cast(&mut self.engine, sel, &mut said);
        }
        self.boxes.extend(said.boxes);
    }

    /// The sheet's equipment page (1194:0D67): its colours set again (9B56).
    fn equipment_page(&mut self) {
        self.fade_open(0, 0, |a| a.panel = Some(panels::equipment_page(&mut a.engine.m)));
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
                if let Some((field, look, game)) = inv.page.as_ref().and_then(|p| p.typing.clone()) {
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

    /// A click with a panel up (`right`: the right button): an inventory
    /// line's item put on or off, or selected; the use list's entry used;
    /// the spellbook's entry cast.
    fn panel_click(&mut self, right: bool) {
        let (x, y) = self.screen_point();
        let mut said = panels::Said::default();
        let mut cast = None;
        match self.panel.as_mut() {
            Some(panels::Panel::Inventory(inv)) => {
                // A spell's page up (1194:51D1): the click its question
                // waits for, or the Delete question's yes or no.
                if let Some(page) = inv.page.clone() {
                    let answer = match &page.asking {
                        Some(_) => page.menu_pick(&self.assets.archive, x, y).map(|i| panels::PageAnswer::Confirm(i as u16)),
                        None if !right => Some(panels::PageAnswer::Click(x as u16, y as u16)),
                        None => None,
                    };
                    if let Some(a) = answer {
                        self.page_answer(page, a);
                    }
                    return;
                }
                // An item's description waits for a click (1194:5536),
                // which does nothing more.
                if inv.info.take().is_some() {
                    inv.info_y = None;
                    return;
                }
                if let Some((names, _)) = &inv.using {
                    let menu = panels::use_menu(names, 0);
                    let (ex, ey, lh, ew) = hud::menu_layout(&self.font, &menu);
                    let i = ((y - ey) / lh.max(1)) as usize;
                    if x >= ex - 3 && x < ex + ew + 3 && y >= ey && i < names.len() {
                        inv.using = None;
                        panels::use_pick(&mut self.engine.m, &mut self.engine.files, i as u16, &mut said);
                    }
                } else if let Some((text, _)) = &inv.asking {
                    let menu = panels::drop_menu(text, 0);
                    let (ex, ey, lh, ew) = hud::menu_layout(&self.font, &menu);
                    let i = ((y - ey) / lh.max(1)) as usize;
                    if x >= ex - 3 && x < ex + ew + 3 && y >= ey && i < 2 {
                        inv.asking = None;
                        panels::drop_answer(&mut self.engine.m, i == 0, &mut said);
                    }
                } else if let Some(b) = panels::button_at(x, y) {
                    match b {
                        panels::Button::Exit => {
                            let back = inv.from_sheet;
                            self.panel = back.then_some(panels::Panel::Sheet)
                        },
                        panels::Button::Spellbook => {
                            inv.bare = false;
                            panels::spells_button(&mut self.engine.m, &mut said)
                        }
                        panels::Button::Drop => panels::drop_press(&mut self.engine.m, inv, &mut said),
                    }
                } else if let Some(d) = panels::arrow_at(x, y) {
                    panels::arrow(&mut self.engine.m, &self.engine.files, d);
                } else if panels::line_at(x, y).is_some() {
                    if panels::opens_page(&self.engine.m, y, right) {
                        // The spell's page sets its colours (68D7).
                        self.fade_open(0, 1, move |s| {
                            let mut said = panels::Said::default();
                            if let Some(panels::Panel::Inventory(inv)) = s.panel.as_mut() {
                                panels::line_click(&mut s.engine.m, &mut s.engine.files, inv, y, right, &mut said);
                            }
                        });
                        return;
                    }
                    panels::line_click(&mut self.engine.m, &mut self.engine.files, inv, y, right, &mut said);
                }
            }
            // A line of the list in the bar's corner clicked (2542:06F8):
            // a line not marked is marked; the line marked is picked.
            Some(p @ (panels::Panel::Spellbook(_) | panels::Panel::Use(..))) => {
                let hud = hud::Hud { menu: p.bar_list(&self.engine.m), ..Default::default() };
                if let Some(i) = hud::menu_pick(&self.font, &hud, x, y) {
                    let marked = match p {
                        panels::Panel::Spellbook(book) => &mut book.selected,
                        panels::Panel::Use(_, sel) => sel,
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
                Some(sheets::SheetClick::Equipment) => self.equipment_page(),
                Some(sheets::SheetClick::Done) => self.panel = None,
                None => {}
            },
            Some(panels::Panel::Log(log)) => match sheets::log_click(x, y) {
                Some(true) => self.panel = None,
                Some(false) if log.page + 1 < log.pages.len() => log.page += 1,
                _ => {}
            },
            None => {}
        }
        if let Some(line) = cast {
            self.panel = None;
            let sel = panels::book_pick(&mut self.engine.m, line);
            panels::cast(&mut self.engine, sel, &mut said);
        }
        self.boxes.extend(said.boxes);
    }

    /// A click on the interface bar (`sheets::bar_click`).
    fn bar(&mut self, x: i32, y: i32, right: bool) -> bool {
        sheets::pics(&self.assets.archive, &self.engine.m);
        let Some(a) = sheets::bar_click(&mut self.engine.m, &mut self.engine.files, x, y, right) else { return false };
        self.bar_action(a);
        true
    }

    /// A menu key pressed: the menu keys' check (2542:0100,
    /// `Machine::menu_keys`) run on the engine, and the screen the key it
    /// took asks for opened, the mouse let go for it, as the bar's button
    /// with the same handler opens it.
    fn menu_key(&mut self) {
        use arena::menukeys::{MenuKey, NoMenuKeyHooks};
        let key = self.engine.m.menu_keys(&mut self.engine.files, &mut NoMenuKeyHooks).unwrap_or(MenuKey::None);
        if matches!(key, MenuKey::None | MenuKey::Ready | MenuKey::Pickpocket | MenuKey::TouchCast | MenuKey::Compass | MenuKey::Camp) {
            if let Some(a) = sheets::menu_action(&mut self.engine.m, key) {
                self.bar_action(a);
            }
            return;
        }
        self.release();
        sheets::pics(&self.assets.archive, &self.engine.m);
        match sheets::menu_action(&mut self.engine.m, key) {
            Some(a) => self.bar_action(a),
            None => self.open_options(),
        }
    }

    /// The automap screen left (0000:2EC8..2EF2): faded out in 12 steps,
    /// the world drawn and faded in to its colours (999E) in 12.
    fn close_automap(&mut self) {
        self.fade_open(12, 12, |a| {
            automap::close(&mut a.engine.m);
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
            sheets::BarAction::Automap => self.fade_open(12, 12, |a| {
                automap::open(&mut a.engine, &a.assets.archive);
                a.map = true;
            }),
            sheets::BarAction::Status(lines) => self.boxes.push_back(lines),
            sheets::BarAction::Spells => self.panel = Some(panels::Panel::Spellbook(Default::default())),
            // 27E3:0088: the screen faded out in 8 steps first.
            sheets::BarAction::Log => self.fade_open(8, 0, |a| a.panel = Some(panels::Panel::Log(sheets::open_log(&a.engine.m, &a.engine.files)))),
            // 2542:066A: no list without an item to use (127C 0).
            sheets::BarAction::Use => {
                let names = panels::usable_names(&self.engine.m);
                if !names.is_empty() {
                    self.panel = Some(panels::Panel::Use(names, 0));
                }
            }
            sheets::BarAction::Camp => self.camp = true,
            // 1194:0CF8: the screen faded out in 8 steps first.
            sheets::BarAction::Sheet => self.fade_open(8, 0, |a| a.panel = Some(panels::Panel::Sheet)),
        }
    }

    /// A click at (x, y) on the automap screen's map: a turn adding or
    /// editing the note there (0000:31FE), its text asked for as other
    /// typed answers are; outside the map nothing (0000:346B).
    fn note_turn(&mut self, x: i32, y: i32) {
        let before = copy(&self.engine);
        let input = TurnIn {
            intent: Default::default(),
            clicked: false,
            camp: false,
            shop: None,
            loot: None,
            key: false,
            tick: 0,
            right: false,
            drag: (0, 0),
            aspect: (self.size.0 / self.size.1.max(1.0)) as f32,
            note: Some((x as u16, y as u16)),
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

    /// The character sheet (K, the view's: the portrait without its
    /// touch cast).
    fn open_sheet(&mut self) {
        sheets::pics(&self.assets.archive, &self.engine.m);
        // As the portrait does before its sheet (1194:5F13).
        self.engine.m.drop_place();
        self.fade_open(8, 0, |a| a.panel = Some(panels::Panel::Sheet));
    }

    /// A box of the view's own.
    fn say(&mut self, s: &str) {
        println!("{s}");
        self.boxes.push_back(vec![vec![hud::Span { colour: 0x0F, text: s.to_string(), font: Default::default(), code: None }]]);
    }

    /// What the HUD shows now.
    fn hud(&self) -> hud::Hud {
        let mut h = hud::Hud { message: message_line(&self.engine), boxed: self.boxes.front().cloned(), level: hud::level_text(&self.engine.m), clip: game_clip(&self.engine.m), ..Default::default() };
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
                    // A menu's boxes mark no entry until the arrows move.
                    if !m.list && !self.arrowed {
                        m.selected = usize::MAX;
                    }
                    h.menu = Some(m);
                }
                Some(dialog::Need::Text(a)) => h.typing = Some(a.prompt.clone()),
                // A level's points: the attribute screen (`creation`); the
                // spellmaker's screen is drawn behind (`maker_screen`).
                Some(dialog::Need::Spend(_) | dialog::Need::Spellmaker | dialog::Need::Travel(_)) | None => {}
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
            if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space | KeyCode::Escape) {
                self.box_read();
            }
            return;
        }
        let Some(p) = &self.pending else { return };
        match &p.need {
            dialog::Need::Menu(menu) => {
                let n = menu.entries.len().max(1);
                match code {
                    KeyCode::ArrowUp => {
                        self.selected = (self.selected + n - 1) % n;
                        self.arrowed = true;
                    }
                    KeyCode::ArrowDown => {
                        self.selected = (self.selected + 1) % n;
                        self.arrowed = true;
                    }
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        let s = self.selected as u16;
                        self.answer(dialog::Answer::Pick(s));
                    }
                    KeyCode::Escape => self.answer(dialog::Answer::Pick(0xFFFF)),
                    _ => {
                        let Some(c) = text.and_then(|t| t.bytes().next()) else { return };
                        let up = c.to_ascii_uppercase();
                        let by_key = menu.keys.iter().position(|&k| k.to_ascii_uppercase() == up);
                        let by_number = (c as char).to_digit(10).filter(|&d| d >= 1).map(|d| d as usize - 1).filter(|&i| i < menu.entries.len());
                        if let Some(i) = by_key.or(by_number) {
                            self.answer(dialog::Answer::Pick(i as u16));
                        }
                    }
                }
            }
            // The attribute screen takes the keys (`creation_input`).
            dialog::Need::Spend(_) => {}
            // The travel map takes the keys (`travel`).
            dialog::Need::Travel(_) => {}
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

    fn box_read(&mut self) {
        self.boxes.pop_front();
        if self.boxes.is_empty() {
            self.box_backdrop = None;
        }
        if self.boxes.is_empty() && self.creation.is_some() && matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spend(_))) {
            let c = self.creation.take();
            self.fade_open(8, 0, |a| a.creation = c);
        }
    }

    /// A click while the game waits: a box read, or a menu's entry picked.
    fn dialog_click(&mut self) {
        if !self.boxes.is_empty() {
            self.box_read();
            return;
        }
        let (x, y) = self.screen_point();
        // The spellmaker waits for a click on its screen or form.
        if matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Spellmaker)) {
            self.answer(dialog::Answer::Click(x.clamp(0, 319) as u16, y.clamp(0, 199) as u16));
            return;
        }
        let hud = self.hud();
        if let Some(i) = hud::menu_pick(&self.font, &hud, x, y) {
            // A list (1194:5734) marks the line clicked; the line marked
            // clicked is picked (its first marked as it comes up).
            let list = matches!(self.pending.as_ref().map(|p| &p.need), Some(dialog::Need::Menu(m)) if m.list);
            if list && i != self.selected {
                self.selected = i;
                return;
            }
            self.answer(dialog::Answer::Pick(i as u16));
        }
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
    /// keys, the mouse captured, a click in the view.
    fn input(&mut self, ev: In) {
        use winit::event::MouseButton;
        use winit::keyboard::KeyCode;
        // A key let go is let go in the game's key state whatever is up,
        // and its latch (2542:05B8) cleared as the original's next frame
        // asking for it clears it, so its next press is taken.
        if let In::Key { code, down: false, .. } = ev {
            if let Some(scan) = interface::scan_code(code) {
                self.engine.m.set_key(scan, false);
                self.engine.m.key_pressed(scan as u16);
            }
        }
        match ev {
            In::Resized(w, h) => self.size = (w, h),
            In::Cursor(x, y) => self.cursor = (x, y),
            // The wheel moves a menu's or a list's entry, as Up and Down do
            // (a list scrolls to keep it shown).
            In::Wheel(dy) => {
                if self.boxes.is_empty() {
                    if let Some(Pending { need: dialog::Need::Menu(m), .. }) = &self.pending {
                        let n = m.entries.len().max(1);
                        self.selected = if dy > 0.0 { (self.selected + n - 1) % n } else if dy < 0.0 { (self.selected + 1) % n } else { self.selected };
                        self.arrowed = true;
                    }
                }
            }
            In::Motion(dx, dy) => {
                if self.captured && !self.waiting() {
                    if self.keys.swing {
                        // A mickey a pixel.
                        self.drag.0 += dx;
                        self.drag.1 += dy;
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
            // The options screen up: its clicks (the left button's) and
            // "Are you sure ?"'s keys (`options`).
            In::Button { down: true, button: MouseButton::Left } if self.options.is_some() && self.saveui.is_none() => {
                let (x, y) = self.screen_point();
                if let Some(ui) = &mut self.options {
                    let e = ui.click(x, y, &self.font);
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
            In::Button { down: true, button } if self.panel.is_some() => self.panel_click(button == MouseButton::Right),
            // The search's name typed on the map: its keys, pressed and
            // let go, to the game's editor.
            In::Key { code, down, .. } if self.travel.as_ref().is_some_and(|t| t.typing()) && self.boxes.is_empty() => {
                if let Some(scan) = interface::scan_code(code) {
                    self.travel_input(|t, e, _| t.field_key(e, scan, down));
                }
            }
            In::Key { code, down: true, text } if self.travel.is_some() && self.boxes.is_empty() => {
                self.travel_input(|t, e, _| t.key(e, code, text.as_deref()));
            }
            // The automap screen up: its buttons and its exit (0000:2E30).
            In::Button { down: true, .. } if self.map => {
                let (x, y) = self.screen_point();
                if automap::exit_at(x, y) {
                    self.close_automap();
                } else if let Some(b) = automap::button_at(x, y) {
                    self.engine.m.automap_view_button(b);
                    self.engine.m.automap_origin();
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
            In::Button { down, button: MouseButton::Right } if self.captured => self.keys.swing = down,
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
                    self.panel_key(code);
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
                // The key's state where the game keeps it (DS:ACB5 + its
                // scan code), for the frame's own keys (- and =, F2, F3,
                // F4, N, the debug keys) and the menu keys; a menu key's
                // press runs the menu keys' check (2542:0100) at once.
                let scan = interface::scan_code(code);
                if let Some(scan) = scan {
                    self.engine.m.set_key(scan, down);
                }
                if down && !resting && scan.is_some_and(interface::is_menu_key) {
                    self.menu_key();
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
                    KeyCode::KeyI if down => self.toggle_panel(true),
                    KeyCode::KeyK if down => self.open_sheet(),
                    KeyCode::KeyQ if down => interface::ready(&mut self.engine.m),
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
            c.advance(now.duration_since(self.movie_at).as_millis() as u32);
            self.creation_outcome();
        }
        self.creation_music();
        // The travel map's passes and journey (the game's turns from the
        // next tick after it ends).
        let travelling = self.travel.is_some();
        if let Some(t) = &mut self.travel {
            t.advance(now.duration_since(self.movie_at).as_millis() as u32);
            self.travel_after();
        }
        self.movie_at = now;
        if now >= self.next && !travelling {
            self.step();
            let ticks = self.engine.frame_ticks() as f64;
            self.next = now + Duration::from_secs_f64(ticks / 18.2);
        }
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
