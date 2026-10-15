//! arena-view: walk a level of Arena, drawn with wgpu, with the port's
//! engine running the game.
//!
//! Usage: arena-view GAME_DIR LEVEL.MIF [--fight] [--people] [--click] [--hour H] [--at COL ROW HEADING] [--wait N] [--do STEPS] [--picks N,..] [--save-dir DIR] [--slot N] [--shot OUT.png [FRAMES [TURN]]]
//!
//! Keys: W/S or Up/Down forward and back, A/D strafe, Left/Right turn,
//! the mouse turns (click the window to capture it, Esc to release),
//! Space jumps (Shift for a running jump), E or a click (captured) clicks
//! the centre of the view: opens a door. With --shot it renders one
//! picture after turning by TURN (heading units, 512 a turn) and FRAMES
//! frames (default 0) of walking forward, and exits; --wait stands still
//! N frames before the walk, --click clicking first; --at puts the
//! player in a square's centre, facing HEADING, a debugging aid; --hour
//! sets the clock to that hour. --fight brings a random encounter at the
//! start (F does in the window); --people (P) rolls until people come.
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
//! saved in; --load loads it at the start. While the game waits for an
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
//! on), `wear:N` (item N's condition halved), `class:N`, `corpse` (a
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
//! them). The animations the game plays (the dream, the Emperor's
//! audience, the quest's end) show over it, a key their next lines or
//! their end; `--do movie:NAME[:TEXT]` queues one (a debugging aid).

mod archive;
mod areas;
mod audio;
mod automap;
mod create;
mod dialog;
mod flats;
mod flc;
mod hud;
mod interface;
mod newgame;
mod panels;
mod questui;
mod render;
mod shopui;
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
    fn message_box(&mut self, m: &arena::machine::Machine, si: u16) {
        match self.talk.as_deref_mut() {
            Some(t) => t.message_box(m, si),
            None => println!("{}", String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, si)).replace('\r', "\n")),
        }
    }

    fn menu(&mut self, m: &arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        match self.talk.as_deref_mut() {
            Some(t) => t.menu(m, si, bx, di),
            None => 0xFFFF,
        }
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
/// (2294:0B8A); the automap, the quest's level, the level's creatures
/// and the screen aren't the view's yet.
impl arena::hooks::StairsHooks for Front<'_> {
    fn light_table(&mut self, m: &mut arena::machine::Machine) {
        if let Some(l) = self.light.as_deref_mut() {
            l.table(m, arena::weather::NORMAL_LGT);
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
    /// 0000:41A4..4247: the audience, its screens as animations.
    fn palace_audience(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.emperor_audience(files, &mut Audience(self))
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
impl arena::clock::ClockHooks for Front<'_> {
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
impl arena::frame::FrameHooks for Front<'_> {
    /// 1194:DB6C with the view's own talk hooks.
    fn actor_click(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.actor_click(files, self)
    }
    /// 1194:4635: a frame of rest, while resting.
    fn rest(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.rest_frame(files, self)
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
/// Clicking on people and things: the way told is a note; the lists are
/// left at once.
impl arena::talk::TalkHooks for Front<'_> {
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
        m.quest_person_talk(files, &mut questui::QuestFront(self))
    }
    /// 1194:8F1E (from DCE1): the palace's quests.
    fn palace(&mut self, m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) -> Option<()> {
        m.quests_talk(files, &mut questui::QuestFront(self))
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
    /// 1194:D986: the eighth piece, the quest's end: WALK.FLC and
    /// AFLC2.FLC, NUJAGDTH.FLC with text 57Ch in 64h, STAFF.FLC and
    /// KING.FLC, NUKING.FLC with 57Dh and then 57Fh in 99h. (What comes
    /// after, 0000:9426 on, isn't ported.)
    fn last_piece(&mut self, _m: &mut arena::machine::Machine) {
        for (name, text, colour) in [
            ("WALK.FLC", None, 0),
            ("AFLC2.FLC", None, 0),
            ("NUJAGDTH.FLC", Some(0x57C), 0x64),
            ("STAFF.FLC", None, 0),
            ("KING.FLC", None, 0),
            ("NUKING.FLC", Some(0x57D), 0x99),
            ("NUKING.FLC", Some(0x57F), 0x99),
        ] {
            self.0.movie(name, text, colour);
        }
    }
}
/// Resting (src/rest.rs): its menu and the hours typed through `dialog`,
/// its texts boxes, the hours shown kept for the status line.
impl arena::rest::RestHooks for Front<'_> {
    fn menu(&mut self, m: &mut arena::machine::Machine, si: u16, bx: u16, di: u16) -> u16 {
        Front::menu(self, m, si, bx, di)
    }
    fn type_hours(&mut self, m: &mut arena::machine::Machine) {
        if let Some(t) = self.talk.as_deref_mut() {
            let prompt = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, REST_PROMPT)).trim().to_string();
            t.typed(m, arena::rest::var::TYPED, 3, &prompt);
        }
    }
    fn message(&mut self, m: &mut arena::machine::Machine, si: u16) {
        Front::message_box(self, m, si);
    }
    fn hours_text(&mut self, m: &mut arena::machine::Machine) {
        let s = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, arena::rest::var::TEXT)).trim().to_string();
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
}

impl SkyDraw<'_> {
    fn quad(&mut self, m: &arena::machine::Machine, name: Option<&String>, frame: usize, x: u16, y: u16) {
        use arena::mem::DS;
        let Some(f) = name.and_then(|n| self.sheet.sky.get(n)) else { return };
        let rect = self.sheet.rects[f.first + frame % f.count];
        let r = |at| m.mem.r16(DS, at) as i16;
        world::sky_quad(&mut self.out, rect, x as i16, y as i16, r(0xC3D6), r(0xC3D8), r(0xA91A), self.focal);
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
    /// Also enters the area's sky (0000:464A: its objects made).
    fn new(dir: &std::path::Path, engine: &mut Engine) -> Self {
        let flats = world::flats(&engine.m);
        let archive = Rc::new(archive::Archive::open(dir));
        let sheet = world::sprite_sheet(&flats, &archive, &world::creature_names(&engine.m));
        interface::enter(&mut engine.m);
        let mut sky = SkyLoad { archive: archive.clone(), images: HashMap::new() };
        engine.m.area_sky(&mut sky);
        Assets {
            palette: world::palette(dir),
            charsheet: world::palette_file(dir, "CHARSHT.COL"),
            textures: world::textures(&engine.m),
            shading: world::shading(&engine.m),
            mesh: world::mesh(&engine.m),
            sheet,
            dither: ["DITHER.IMG", "DITHER2.IMG"].iter().flat_map(|n| {
                let mut d = archive.read(n).unwrap_or_default();
                d.resize(800, 0);
                d
            }).collect(),
            interface: interface::Interface::new(archive.clone()),
            archive,
            colours: None,
            light: LightState { dir: dir.to_path_buf(), dark: 0, lamp: None, table: engine.m.mem.r16(arena::mem::DS, 0xCCC2), fog: None, sky },
        }
    }

    /// The level's own assets made again after a level change (stairs):
    /// the textures, the shading rows, the walls and the flats. The sky
    /// and the light state stay the area's.
    fn level(&mut self, engine: &Engine) {
        let flats = world::flats(&engine.m);
        self.sheet = world::sprite_sheet(&flats, &self.archive, &world::creature_names(&engine.m));
        self.textures = world::textures(&engine.m);
        self.shading = world::shading(&engine.m);
        self.mesh = world::mesh(&engine.m);
    }

    fn renderer(&self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Renderer {
        let scene = Scene { palette: self.palette, textures: &self.textures, shading: &self.shading, mesh: &self.mesh, sprites: &self.sheet, dither: &self.dither };
        Renderer::new(device, queue, format, &scene)
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
    fn sprites(&self, engine: &mut Engine, cam: Camera, ticks: usize, aspect: f32) -> Vec<world::Vertex> {
        let mut v = world::sprites(&engine.m, &self.sheet, cam.heading, ticks / ANIMATE);
        v.extend(world::doors(&engine.m));
        let focal = (30f32).to_radians().tan() * aspect;
        let mut draw = SkyDraw { sheet: &self.sheet, images: &self.light.sky.images, focal, fog: self.light.fog.is_some(), out: Vec::new() };
        if engine.m.sky_draw(&mut draw).is_some() {
            v.extend(draw.out);
        }
        v
    }
}

/// Timer ticks an animation frame lasts.
const ANIMATE: usize = 4;

/// `--mute` (no sound played) and `--sound-log` (each sound printed),
/// for the window and the shots alike.
static MUTE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SOUND_LOG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    MUTE.store(args.iter().any(|a| a == "--mute"), std::sync::atomic::Ordering::Relaxed);
    SOUND_LOG.store(args.iter().any(|a| a == "--sound-log"), std::sync::atomic::Ordering::Relaxed);
    args.retain(|a| a != "--mute" && a != "--sound-log");
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
    let mut engine = world::load(&dir, &args[2]);
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
        match areas::load_game(&mut engine, &dir, save, slot) {
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
        // The clock at that hour of the first day (its calendar fields
        // agreeing, so no hour passes at once), and the sky's time.
        use arena::clock::var::{HOUR, MINUTE, MINUTES};
        engine.m.mem.w32(arena::mem::DS, MINUTES, h % 24 * 60);
        engine.m.mem.w16(arena::mem::DS, HOUR, (h % 24) as u16);
        engine.m.mem.w16(arena::mem::DS, MINUTE, 0);
        engine.m.sky_time().expect("sky time");
    }
    if !load_at_start {
        world::city_people(&mut engine);
    }
    if let Some((col, row, heading)) = at {
        use arena::movement::var::{HEADING, X, Y};
        engine.m.mem.w16(arena::mem::DS, X, col << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, Y, row << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, HEADING, heading);
    }
    if fight {
        world::summon(&mut engine);
    }
    if people {
        world::summon_people(&mut engine);
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
        shot(&dir, engine, run, out, click, wait, frames, turn, &steps, answers, save_dir.as_deref().map(|d| (d, slot)));
        return;
    }
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let assets = Assets::new(&dir, &mut engine);
    let font = hud::Font::load(&assets.archive, FONT).expect("ARENAFNT.DAT");
    let screen = hud::Screen::new(assets.palette);
    let audio = audio::Audio::new(assets.archive.clone(), !MUTE.load(std::sync::atomic::Ordering::Relaxed), SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let mut app = App {
        audio,
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
        boxes: Default::default(),
        selected: 0,
        typed: String::new(),
        status: None,
        font,
        screen,
        cursor: (0.0, 0.0),
        drag: (0.0, 0.0),
        save_dir,
        slot,
        panel: None,
        equip: None,
        map: false,
        creation,
        creation_engine: None,
        movie_at: Instant::now(),
    };
    event_loop.run_app(&mut app).expect("run");
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
    key: bool,
    tick: usize,
    right: bool,
    drag: (i16, i16),
    /// The window's width over its height (the click's line).
    aspect: f32,
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
    let t = engine.frame_ticks();
    engine.tick(t);
    let level = engine.m.mem.r8(DS, arena::movement::var::LEVEL);
    let window = engine.m.mem.r32(DS, arena::wild::var::WINDOW);
    let mut hooks = Front { sheet: Some(&assets.sheet), light: Some(&mut assets.light), tick: input.tick, clicked: input.clicked, talk: Some(talk), aspect: input.aspect };
    if input.camp && engine.m.camp(&mut engine.files, &mut hooks).is_none() {
        eprintln!("camping met something not ported");
    }
    if let Some(kind) = input.shop {
        if shopui::shop(&mut hooks, &mut engine.m, &mut engine.files, kind).is_none() {
            eprintln!("the shop met something not ported");
        }
    }
    if engine.m.mem.r8(DS, arena::rest::var::RESTING) != 0 {
        engine.m.mem.w8(DS, arena::rest::var::KEY, input.key as u8);
    }
    let buttons = engine.m.mem.r8(DS, arena::controls::var::BUTTONS) & !2 | (input.right as u8) << 1;
    engine.m.mem.w8(DS, arena::controls::var::BUTTONS, buttons);
    let (mx, my) = engine.m.mouse_motion;
    engine.m.mouse_motion = (mx.wrapping_add(input.drag.0 as u16), my.wrapping_add(input.drag.1 as u16));
    let frame = engine.frame(Input::Steer(&input.intent), &mut hooks);
    let stairs = engine.m.mem.r8(DS, arena::movement::var::LEVEL) != level;
    if stairs {
        // 0000:368E, the stairs' level screen: the automap kept and the new
        // level's loaded (here after the frame; nothing touches the map in
        // between).
        engine.m.automap_keep(&mut engine.files);
        engine.m.automap_load(&mut engine.files);
    }
    let level_changed = stairs || wild_moved(engine, window);
    TurnOut { frame, level_changed }
}

/// The animations a turn played (`dialog::Talk::movies`), made: a text's
/// lines through the text port (1194:D459), the files from the game's.
fn movies_of(engine: &mut Engine, archive: &archive::Archive, played: Vec<(&'static str, Option<u16>, u8)>) -> Vec<flc::Movie> {
    played
        .into_iter()
        .filter_map(|(name, text, colour)| {
            let lines = text.map(|t| create::vision_text(engine, t, colour)).unwrap_or_default();
            flc::Movie::new(archive, name, lines)
        })
        .collect()
}

/// The engine copied, for a turn to run again (`dialog`).
fn copy(engine: &Engine) -> Engine {
    Engine { m: engine.m.clone(), files: engine.files.clone() }
}

/// The message line (061A, shown while 061E counts down) as the HUD's.
fn message_line(engine: &Engine) -> Option<Vec<Vec<hud::Span>>> {
    engine.view().message.map(|(seg, off)| hud::lines(engine.m.mem.cstr(seg, off), 0x0F))
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
        key: false,
        tick: st.frames * 4,
        right: std::mem::take(&mut st.right),
        drag: std::mem::take(&mut st.drag),
        aspect: 1.6,
    };
    let mut talk = dialog::Talk { answers: std::mem::take(&mut st.answers), ..Default::default() };
    st.audio.before(&mut engine.m);
    let out = run_turn(engine, &mut st.assets, &input, &mut talk);
    st.audio.after(&mut engine.m);
    let played = std::mem::take(&mut talk.movies);
    let archive = st.assets.archive.clone();
    st.movies.extend(movies_of(engine, &archive, played));
    st.answers = talk.answers;
    if !talk.boxes.is_empty() {
        st.boxes = if talk.need.is_some() { talk.since_answer } else { talk.boxes };
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
            let c = create::Creation::ending(engine, st.assets.archive.clone(), dir, st.assets.palette);
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
fn shot(dir: &std::path::Path, mut engine: Engine, mut run: arena::transit::Run, out: &str, click: bool, wait: usize, frames: usize, turn: i16, steps: &str, answers: std::collections::VecDeque<dialog::Answer>, saves: Option<(&std::path::Path, u8)>) {
    let run = &mut run;
    let assets = Assets::new(dir, &mut engine);
    // Shots play nothing; with --sound-log they print what would play.
    let audio = audio::Audio::new(assets.archive.clone(), false, SOUND_LOG.load(std::sync::atomic::Ordering::Relaxed));
    let st = &mut Shot { audio, movies: Vec::new(), assets, right: false, drag: (0, 0), clicked: false, camp: false, shop: None, answers, message: None, frames: 0, boxes: Vec::new(), need: None, status: None, panel: None, map: false, over: false, ui: None };
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
            shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
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
            // A debugging aid: an animation queued as the game's hooks
            // queue them (NAME, or NAME:TEXT with the text's number in
            // hex, in colour 33h).
            let (name, text) = match spec.split_once(':') {
                Some((n, t)) => (n, Some(u16::from_str_radix(t, 16).expect("--do movie:NAME:TEXT"))),
                None => (spec, None),
            };
            let name: &'static str = Box::leak(name.to_ascii_uppercase().into_boxed_str());
            let archive = st.assets.archive.clone();
            st.movies.extend(movies_of(&mut engine, &archive, vec![(name, text, 0x33)]));
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
                    eprintln!("the cast met something not ported");
                }
                shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
            }
            "compass" => interface::toggle_compass(&mut engine.m),
            // The inventory (I) and the spellbook (C), up until `close`;
            // their actions as the window's keys and clicks make them.
            "inv" => st.panel = Some(panels::open_inventory(&mut engine.m)),
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
                    "drop" => panels::drop_selected(&mut engine.m, &mut said),
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
            "map" => st.map = !st.map,
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
                let m = &mut engine.m;
                let giver = (0..4u16).map(|i| arena::quest::var::OTHERS + i * 0x29).find(|&si| m.quest_kind(si) == 2);
                match giver {
                    Some(si) => {
                        let g = m.mem.r16(DS, si + 0x10);
                        m.mem.w8(DS, TALK, 4);
                        m.mem.w16(DS, TALKER, g & 0xFF00 | ((g as u8 as u16) << 1) & 0xFF);
                        st.shop = Some(4);
                        shot_frame(dir, &mut engine, run, st, &Intent::default()).expect("area");
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
                st.boxes = vec![vec![vec![hud::Span { colour: 0x0F, text: msg }]]];
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
    let (sky, lights) = st.assets.light_frame(&mut engine.m);
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
    let cam = camera(&engine);
    let sprites = st.assets.sprites(&mut engine, cam, (wait + frames) * 4, w as f32 / h as f32);
    let view = target.create_view(&Default::default());
    r.draw(&device, &queue, &view, w, h, cam, &sprites);
    if let Some(font) = hud::Font::load(&st.assets.archive, FONT) {
        let mut hud = hud::Hud { message: message_line(&engine), boxed: st.boxes.last().cloned(), status: None, ..Default::default() };
        if engine.m.mem.r8(arena::mem::DS, arena::rest::var::RESTING) != 0 {
            hud.status = st.status.clone();
        }
        match st.need.clone() {
            Some(dialog::Need::Menu(m)) => hud.menu = Some(m),
            Some(dialog::Need::Text(p)) => hud.typing = Some((p, String::new())),
            Some(dialog::Need::Spend(_)) | None => {}
        }
        let mut screen = hud::Screen::new(if st.panel.is_some() { st.assets.charsheet } else { st.assets.palette });
        st.assets.interface.draw(&mut screen, &font, &engine.m);
        if let Some(panel) = &st.panel {
            let equip = panels::load_image(&st.assets.archive, &engine.m, "EQUIP.IMG");
            panels::draw(&mut screen, &font, equip.as_ref(), panel, &engine.m);
        }
        hud::compose_over(&mut screen, &font, &hud);
        if st.map {
            automap::draw(&mut screen, &engine.m);
        }
        // An animation the turns played: its first frame and lines.
        if let Some(mv) = st.movies.first() {
            println!("animations played: {}", st.movies.len());
            mv.draw(&mut screen, &font);
        }
        hud::Overlay::new(&device, format).draw(&device, &queue, &view, &screen);
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
/// put 4 up and left of it, as the click (CB60, 2826:0393) is the cursor
/// + 4; else as `centre_click`.
fn click_at(engine: &mut Engine, boxes: &mut Vec<Vec<Vec<hud::Span>>>, (x, y): (i32, i32)) -> bool {
    engine.m.set_mouse((x - 4).max(0) as u16, (y - 4).max(0) as u16, 0);
    let mut talk = dialog::Talk::default();
    let mut hooks = Front { talk: Some(&mut talk), ..Default::default() };
    if engine.m.view_click(&mut engine.files, &mut hooks).is_none() {
        eprintln!("the click met something not ported");
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
        if let Err(e) = std::fs::write(&p, data) {
            eprintln!("{}: {e}", p.display());
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
    // player's (25FD's caller sets it in the original; the save menu,
    // 25FD:004F..00D0, isn't ported): the player's.
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
    engine.m.save_game(&mut SaveDir { dir, palette });
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

#[derive(Default)]
struct Keys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    turn_left: bool,
    turn_right: bool,
    jump: bool,
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
}

struct App {
    /// The sound driver (`audio`).
    audio: audio::Audio,
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
    /// menu's entry highlighted; the hours typed so far; the rest's hours.
    pending: Option<Pending>,
    boxes: std::collections::VecDeque<Vec<Vec<hud::Span>>>,
    selected: usize,
    typed: String,
    status: Option<String>,
    font: hud::Font,
    screen: hud::Screen,
    /// The cursor in the window (pixels).
    cursor: (f64, f64),
    /// The mouse's motion with the right button held, since the last turn.
    drag: (f64, f64),
    /// Where saves go (F5) and come from (F9), and the slot.
    save_dir: Option<PathBuf>,
    slot: u8,
    /// A screen up over the game (I, C), which stops it; EQUIP.IMG once
    /// loaded.
    panel: Option<panels::Panel>,
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
}

impl App {
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
                let loaded = match self.save_dir.clone() {
                    Some(dir) => load_from(&mut self.engine, &self.dir.clone(), &dir, self.slot),
                    None => Err("No save directory (--save-dir).".to_string()),
                };
                match loaded {
                    Ok((msg, run)) => {
                        self.creation = None;
                        self.run = run;
                        self.assets = Assets::new(&self.dir, &mut self.engine);
                        self.remake_renderer();
                        self.last = Some(arena::engine::Frame::Running);
                        self.say(&msg);
                    }
                    Err(msg) => {
                        eprintln!("{msg}");
                        if let Some(c) = &mut self.creation {
                            c.back_to_menu();
                        }
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
    fn game_over(&mut self) {
        let archive = self.assets.archive.clone();
        let c = create::Creation::ending(&mut self.engine, archive, &self.dir, self.assets.palette);
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
        if self.waiting() || self.panel.is_some() || self.creation.is_some() || !self.movies.is_empty() {
            return;
        }
        let k = &self.keys;
        let axis = |p: bool, n: bool| (p as i16 - n as i16) * 256;
        let look = self.look.round() as i16;
        self.look -= look as f64;
        let intent = Intent {
            forward: axis(k.forward, k.back),
            strafe: axis(k.right, k.left),
            turn: axis(k.turn_right, k.turn_left),
            look,
            jump: k.jump,
            run: k.run,
        };
        let tick = (self.start.elapsed().as_secs_f64() * 18.2) as usize;
        let drag = std::mem::take(&mut self.drag);
        let input = TurnIn {
            intent,
            clicked: std::mem::take(&mut self.clicked),
            camp: std::mem::take(&mut self.camp),
            shop: None,
            key: std::mem::take(&mut self.key),
            tick,
            right: self.keys.swing,
            drag: (drag.0.round() as i16, drag.1.round() as i16),
            aspect: self.gpu.as_ref().map_or(1.6, |g| g.config.width as f32 / g.config.height.max(1) as f32),
        };
        let before = copy(&self.engine);
        self.turn(before, input, Vec::new());
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
    /// otherwise it stands, and its frame's end is followed.
    fn turn(&mut self, before: Engine, input: TurnIn, given: Vec<dialog::Answer>) {
        let mut answers: std::collections::VecDeque<dialog::Answer> = given.iter().cloned().collect();
        let scripted = self.script.len();
        answers.extend(self.script.iter().cloned());
        self.engine = copy(&before);
        let mut talk = dialog::Talk { answers, ..Default::default() };
        self.audio.before(&mut self.engine.m);
        let out = run_turn(&mut self.engine, &mut self.assets, &input, &mut talk);
        // A turn stopped at a question is run again once answered: its
        // sounds are the rerun's.
        if talk.need.is_none() {
            self.audio.after(&mut self.engine.m);
            let played = std::mem::take(&mut talk.movies);
            let archive = self.assets.archive.clone();
            self.movies.extend(movies_of(&mut self.engine, &archive, played));
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
        self.boxes.extend(talk.since_answer);
        if talk.status.is_some() {
            self.status = talk.status;
        }
        if let Some(need) = talk.need {
            self.engine = copy(&before);
            self.selected = 0;
            self.typed.clear();
            if let dialog::Need::Spend(snap) = &need {
                let mut e = Engine::new((*snap.0).clone(), self.engine.files.clone());
                let c = create::Creation::level_up(&mut e.m, self.assets.archive.clone(), &self.dir, self.assets.palette);
                self.creation = Some(c);
                self.creation_engine = Some(e);
            }
            self.pending = Some(Pending { before, input, given, need });
            if self.captured {
                self.release();
            }
            return;
        }
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
                    // On to the next area, whose assets are made afresh.
                    if let Some(run) = areas::next(&mut self.engine, &self.dir, self.run) {
                        self.run = run;
                        self.assets = Assets::new(&self.dir, &mut self.engine);
                        self.remake_renderer();
                        self.last = Some(arena::engine::Frame::Running);
                        return;
                    }
                    if self.engine.m.mem.r16(arena::mem::DS, 0x05B1) & 0x100 != 0 {
                        self.game_over();
                        return;
                    }
                }
                None => eprintln!("the frame ended: None"),
            }
            self.last = frame;
        }
    }

    /// The answer to the question waiting: its turn runs again.
    fn answer(&mut self, a: dialog::Answer) {
        let Some(p) = self.pending.take() else { return };
        let mut given = p.given;
        given.push(a);
        self.turn(p.before, p.input, given);
    }

    fn remake_renderer(&mut self) {
        if let Some(g) = &mut self.gpu {
            g.renderer = self.assets.renderer(&g.device, &g.queue, g.config.format);
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
        let Some(g) = &self.gpu else { return (0, 0) };
        let (w, h) = (g.config.width as f64, g.config.height as f64);
        ((self.cursor.0 / w * hud::W as f64) as i32, (self.cursor.1 / h * hud::H as f64) as i32)
    }

    /// I and C: the inventory or the spellbook up, or down.
    fn toggle_panel(&mut self, inventory: bool) {
        let open = match &self.panel {
            Some(panels::Panel::Inventory(_)) => inventory,
            Some(panels::Panel::Spellbook(_)) => !inventory,
            None => false,
        };
        if open {
            self.panel = None;
            return;
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
        let shown = panels::book_layout(&self.font).4;
        let m = &mut self.engine.m;
        let mut cast = None;
        match self.panel.as_mut() {
            Some(panels::Panel::Inventory(inv)) => {
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
                } else {
                    match code {
                        KeyCode::Escape | KeyCode::KeyI => self.panel = None,
                        KeyCode::ArrowUp => panels::scroll(m, -1),
                        KeyCode::ArrowDown => panels::scroll(m, 1),
                        KeyCode::KeyD => panels::drop_selected(m, &mut said),
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
                if let Some(panels::Panel::Spellbook(book)) = self.panel.as_mut() {
                    book.first = book.first.min(book.selected).max((book.selected + 1).saturating_sub(shown));
                }
            }
            None => {}
        }
        if let Some(sel) = cast {
            self.panel = None;
            panels::cast(&mut self.engine, sel, &mut said);
        }
        self.boxes.extend(said.boxes);
    }

    /// A click with a panel up (`right`: the right button): an inventory
    /// line's item put on or off, or selected; the use list's entry used;
    /// the spellbook's entry cast.
    fn panel_click(&mut self, right: bool) {
        let Some(g) = &self.gpu else { return };
        let (w, h) = (g.config.width as f64, g.config.height as f64);
        let (x, y) = ((self.cursor.0 / w * hud::W as f64) as i32, (self.cursor.1 / h * hud::H as f64) as i32);
        let mut said = panels::Said::default();
        let mut cast = None;
        match self.panel.as_mut() {
            Some(panels::Panel::Inventory(inv)) => {
                if let Some((names, _)) = &inv.using {
                    let menu = panels::use_menu(names, 0);
                    let (ex, ey, lh, ew) = hud::menu_layout(&self.font, &menu);
                    let i = ((y - ey) / lh.max(1)) as usize;
                    if x >= ex - 3 && x < ex + ew + 3 && y >= ey && i < names.len() {
                        inv.using = None;
                        panels::use_pick(&mut self.engine.m, &mut self.engine.files, i as u16, &mut said);
                    }
                } else if let Some(b) = panels::button_at(x, y) {
                    match b {
                        panels::Button::Exit => self.panel = None,
                        panels::Button::Spellbook => self.panel = Some(panels::Panel::Spellbook(Default::default())),
                        panels::Button::Drop => panels::drop_selected(&mut self.engine.m, &mut said),
                    }
                } else if let Some(row) = panels::line_at(x, y) {
                    if right {
                        panels::select_line(&mut self.engine.m, row);
                    } else {
                        panels::equip_line(&mut self.engine.m, row, &mut said);
                    }
                }
            }
            Some(panels::Panel::Spellbook(book)) => {
                let (bx, by, lh, bw, shown) = panels::book_layout(&self.font);
                let i = ((y - by) / lh.max(1)) as usize;
                if x >= bx - 3 && x < bx + bw + 3 && y >= by && i < shown {
                    let sel = book.first + i;
                    if sel < panels::spells(&self.engine.m).len() {
                        cast = Some(sel);
                    }
                }
            }
            None => {}
        }
        if let Some(sel) = cast {
            self.panel = None;
            panels::cast(&mut self.engine, sel, &mut said);
        }
        self.boxes.extend(said.boxes);
    }

    /// A box of the view's own.
    fn say(&mut self, s: &str) {
        println!("{s}");
        self.boxes.push_back(vec![vec![hud::Span { colour: 0x0F, text: s.to_string() }]]);
    }

    /// What the HUD shows now.
    fn hud(&self) -> hud::Hud {
        let mut h = hud::Hud { message: message_line(&self.engine), boxed: self.boxes.front().cloned(), ..Default::default() };
        if self.engine.m.mem.r8(arena::mem::DS, arena::rest::var::RESTING) != 0 {
            h.status = self.status.clone();
        }
        if self.boxes.is_empty() {
            match self.pending.as_ref().map(|p| &p.need) {
                Some(dialog::Need::Menu(m)) => {
                    let mut m = m.clone();
                    m.selected = self.selected.min(m.entries.len().saturating_sub(1));
                    h.menu = Some(m);
                }
                Some(dialog::Need::Text(prompt)) => h.typing = Some((prompt.clone(), self.typed.clone())),
                // A level's points: the attribute screen (`creation`).
                Some(dialog::Need::Spend(_)) | None => {}
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
                self.boxes.pop_front();
            }
            return;
        }
        let Some(p) = &self.pending else { return };
        match &p.need {
            dialog::Need::Menu(menu) => {
                let n = menu.entries.len().max(1);
                match code {
                    KeyCode::ArrowUp => self.selected = (self.selected + n - 1) % n,
                    KeyCode::ArrowDown => self.selected = (self.selected + 1) % n,
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
            dialog::Need::Text(_) => match code {
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let t = std::mem::take(&mut self.typed);
                    self.answer(dialog::Answer::Text(t));
                }
                KeyCode::Escape => self.answer(dialog::Answer::Text(String::new())),
                KeyCode::Backspace => {
                    self.typed.pop();
                }
                _ => {
                    if let Some(t) = text {
                        for c in t.chars().filter(|c| c.is_ascii_graphic()) {
                            if self.typed.len() < 3 {
                                self.typed.push(c);
                            }
                        }
                    }
                }
            },
        }
    }

    /// A click while the game waits: a box read, or a menu's entry picked.
    fn dialog_click(&mut self) {
        if !self.boxes.is_empty() {
            self.boxes.pop_front();
            return;
        }
        let Some(g) = &self.gpu else { return };
        let (w, h) = (g.config.width as f64, g.config.height as f64);
        let (x, y) = ((self.cursor.0 / w * hud::W as f64) as i32, (self.cursor.1 / h * hud::H as f64) as i32);
        let hud = self.hud();
        if let Some(menu) = &hud.menu {
            let (ex, ey, lh, ew) = hud::menu_layout(&self.font, menu);
            if x >= ex - 3 && x < ex + ew + 3 && y >= ey {
                let i = ((y - ey) / lh) as usize;
                if i < menu.entries.len() {
                    self.answer(dialog::Answer::Pick(i as u16));
                }
            }
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
        let renderer = self.assets.renderer(&device, &queue, format);
        let overlay = hud::Overlay::new(&device, format);
        self.gpu = Some(Gpu { window, surface, device, queue, config, renderer, overlay });
    }

    fn window_event(&mut self, el: &winit::event_loop::ActiveEventLoop, _id: winit::window::WindowId, event: winit::event::WindowEvent) {
        use winit::event::{ElementState, MouseButton, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &mut self.gpu {
                    g.config.width = size.width.max(1);
                    g.config.height = size.height.max(1);
                    g.surface.configure(&g.device, &g.config);
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor = (position.x, position.y),
            // An animation up: a key or a click its next lines, or its end.
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } | WindowEvent::KeyboardInput { event: winit::event::KeyEvent { state: ElementState::Pressed, .. }, .. }
                if !self.movies.is_empty() =>
            {
                if let Some(mv) = self.movies.front_mut() {
                    if mv.lines.is_empty() {
                        mv.over = true;
                    } else {
                        mv.key();
                    }
                    if mv.over {
                        self.movies.pop_front();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } if self.creation.is_some() => {
                if state == ElementState::Pressed && button == MouseButton::Left {
                    let (x, y) = self.screen_point();
                    self.creation_input(create::Input::Click(x, y));
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.creation.is_some() => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let inp = match event.physical_key {
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(create::Input::Enter),
                    PhysicalKey::Code(KeyCode::Escape) => Some(create::Input::Esc),
                    PhysicalKey::Code(KeyCode::Backspace) => Some(create::Input::Back),
                    PhysicalKey::Code(KeyCode::ArrowUp) => Some(create::Input::Up),
                    PhysicalKey::Code(KeyCode::ArrowDown) => Some(create::Input::Down),
                    _ => event.text.as_deref().and_then(|t| t.bytes().next()).filter(|c| (0x20..0x7F).contains(c)).map(create::Input::Char),
                };
                if let Some(inp) = inp {
                    self.creation_input(inp);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } if self.waiting() => self.dialog_click(),
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } if self.panel.is_some() => self.panel_click(button == MouseButton::Right),
            // The right button held: dragging swings the weapon (0000:3C9F).
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } if self.captured => self.keys.swing = state == ElementState::Pressed,
            // Uncaptured, a left click in the view clicks where it is (a
            // corpse on the floor); another button captures the mouse.
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } if !self.captured && self.screen_point().1 < 147 => {
                self.click_point = Some(self.screen_point());
                self.click = true;
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } if !self.captured => {
                if let Some(g) = &self.gpu {
                    let ok = g
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                        .or_else(|_| g.window.set_cursor_grab(winit::window::CursorGrabMode::Confined))
                        .is_ok();
                    g.window.set_cursor_visible(!ok);
                    self.captured = ok;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.click = true,
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    if down && self.waiting() {
                        self.dialog_key(code, event.text.as_deref());
                        return;
                    }
                    if down && self.panel.is_some() {
                        self.panel_key(code);
                        return;
                    }
                    if down {
                        // Any key ends a rest (AFF9).
                        self.key = true;
                    }
                    let k = &mut self.keys;
                    match code {
                        KeyCode::KeyW | KeyCode::ArrowUp => k.forward = down,
                        KeyCode::KeyS | KeyCode::ArrowDown => k.back = down,
                        KeyCode::KeyA => k.left = down,
                        KeyCode::KeyD => k.right = down,
                        KeyCode::ArrowLeft => k.turn_left = down,
                        KeyCode::ArrowRight => k.turn_right = down,
                        KeyCode::Space => k.jump = down,
                        KeyCode::KeyF if down => world::summon(&mut self.engine),
                        KeyCode::KeyP if down => world::summon_people(&mut self.engine),
                        KeyCode::KeyE if down => self.click = true,
                        KeyCode::KeyR if down => self.camp = true,
                        KeyCode::KeyI if down => self.toggle_panel(true),
                        KeyCode::KeyC if down => self.toggle_panel(false),
                        KeyCode::KeyM if down => self.map = !self.map,
                        // The original's A (2542:0137) and F8 (01C5).
                        KeyCode::KeyQ if down => interface::ready(&mut self.engine.m),
                        KeyCode::F8 if down => interface::toggle_compass(&mut self.engine.m),
                        // The game's debug keys (scan codes 3Ch, 3Dh).
                        KeyCode::F2 => self.engine.m.set_key(0x3C, down),
                        KeyCode::F3 => self.engine.m.set_key(0x3D, down),
                        KeyCode::F5 if down => self.save(),
                        KeyCode::F9 if down => self.load(),
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => k.run = down,
                        KeyCode::Escape if down => self.release(),
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let cam = camera(&self.engine);
                let hud = self.hud();
                self.screen.clear();
                if let Some(mv) = self.movies.front() {
                    mv.draw(&mut self.screen, &self.font);
                } else if let Some(c) = &self.creation {
                    c.draw(&mut self.screen, &self.creation_engine.as_ref().unwrap_or(&self.engine).m);
                } else {
                    self.screen.set_palette(if self.panel.is_some() { self.assets.charsheet } else { self.assets.palette });
                    self.assets.interface.draw(&mut self.screen, &self.font, &self.engine.m);
                    if self.panel.is_some() {
                        self.equip_image();
                        let equip = self.equip.as_ref().and_then(|e| e.as_ref());
                        panels::draw(&mut self.screen, &self.font, equip, self.panel.as_ref().unwrap(), &self.engine.m);
                    }
                    hud::compose_over(&mut self.screen, &self.font, &hud);
                    if self.map {
                        automap::draw(&mut self.screen, &self.engine.m);
                    }
                }
                if let Some(g) = &mut self.gpu {
                    let Ok(frame) = g.surface.get_current_texture() else {
                        g.surface.configure(&g.device, &g.config);
                        return;
                    };
                    let view = frame.texture.create_view(&Default::default());
                    let ticks = (self.start.elapsed().as_secs_f64() * 18.2) as usize;
                    let (sky, lights) = self.assets.light_frame(&mut self.engine.m);
                    let aspect = g.config.width as f32 / g.config.height.max(1) as f32;
                    let sprites = self.assets.sprites(&mut self.engine, cam, ticks, aspect);
                    g.renderer.set_sky(sky);
                    g.renderer.set_lights(lights);
                    g.renderer.set_window(interface::window(&self.engine.m));
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
            if self.captured && !self.waiting() {
                if self.keys.swing {
                    // A mickey a pixel.
                    self.drag.0 += delta.0;
                    self.drag.1 += delta.1;
                } else {
                    // About a quarter turn for 400 pixels.
                    self.look += delta.0 * 128.0 / 400.0;
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        // The game runs at its own pace (18.2 timer ticks a second, a frame
        // taking `frame_ticks`); drawing goes as fast as the display. While
        // the game waits for the player, no time passes.
        let now = Instant::now();
        if let Some(mv) = self.movies.front_mut() {
            mv.advance(now.duration_since(self.movie_at).as_millis() as u32);
            if mv.over {
                self.movies.pop_front();
            }
        }
        if let Some(c) = &mut self.creation {
            c.advance(now.duration_since(self.movie_at).as_millis() as u32);
            self.creation_outcome();
        }
        self.movie_at = now;
        if now >= self.next {
            self.step();
            let ticks = self.engine.frame_ticks() as f64;
            self.next = now + Duration::from_secs_f64(ticks / 18.2);
        }
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
        el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(self.next));
    }
}
