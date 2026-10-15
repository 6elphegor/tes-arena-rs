//! Clicking on people and things in the view (1194:DB6C, each frame).
//!
//! A click the view didn't take (`click::view_click` leaves it for the
//! actors, C39A bit 80h) is matched by the renderer against what it
//! draws: the actor or map flat under the cursor becomes CLICKED (an
//! actor slot, or the flat copied to RECORD, 2294:047B). That hit test
//! (0000:C4FA: the click at CB60 inside a drawn sprite's rectangle, only
//! its x with C39A bit 1000h) is the renderer's, a front end's to make.
//! Each frame `actor_click` acts on it:
//!
//! - a creature (slots 15..22) near enough takes a waiting touch spell;
//!   otherwise "You see a %s..." and the object click;
//! - anything else gets the object click first (`object_click`,
//!   292B:06DC): chests (locked ones picked, or opened by an Open
//!   spell), items lying in the level, corpses to loot;
//! - indoors, a building's people: the shopkeeper of the building's kind
//!   (D48, the table at 869B), the palace's ruler and quest givers, the
//!   level's quest items, the flats that only say something;
//! - townspeople and the people of a building (the flat kinds at C944)
//!   talk (`talk`): a menu of greeting, "Where is...", rumours and
//!   goodbye, or, with picking armed (5CA bit 0), a pickpocketing try.
//!
//! The text windows, menus and lists are the front end's (`TalkHooks`),
//! as are the parts not ported: the quests, the shops, the loot window
//! (1194:7B16), the wild's way-finding and entrances, and the text
//! substitutions (2DA2:000A).

use crate::bsa::Files;
use crate::cast::Touch;
use crate::click::var::{LOCK_TRIED, MESSAGE, MESSAGE_TIME, PICK_FAILED};
use crate::controls::var::PICKED;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};
use crate::text::TextHooks;

/// DGROUP variables.
pub mod var {
    /// What the click met (an actor slot, RECORD, 0 nothing; FFFFh while
    /// a dialogue owns the click).
    pub const CLICKED: u16 = 0xCB5E;
    /// A clicked map flat, copied as an actor record (2294:047B).
    pub const RECORD: u16 = 0x3D96;
    /// The scratch actor record a map flat is drawn through (0000:D18E):
    /// x, z, y at +0, +2, +4, the flat at +0Ch, its frame +0Dh and frames
    /// +0Eh.
    pub const FLAT_DRAWN: u16 = 0xD7DE;
    /// The colour of the next text window.
    pub const TEXT_COLOUR: u16 = 0x9288;
    /// The text number for 1194:A374, and two of its flags.
    pub const TEXT: u16 = 0x137A;
    pub const TEXT_FLAG: u16 = 0x137E;
    pub const TEXT_FLAG2: u16 = 0x101C;
    /// The talking flat's kind (its index in PEOPLE_FLATS).
    pub const KIND: u16 = 0x126C;
    /// The flat kinds of a building's people (16 bytes), of the flats
    /// that only say something (8) and of the shopkeeper.
    pub const PEOPLE_FLATS: u16 = 0xC944;
    pub const SAYING_FLATS: u16 = 0xC987;
    pub const KEEPER_FLAT: u16 = 0xC943;
    /// The square a way is told to (x, y in squares, words).
    pub const WAY: u16 = 0x8A52;
    /// The list shown (its strings), the quest's entry in "Where is...".
    pub const LIST: u16 = 0x9A9E;
    pub const QUEST_ENTRY: u16 = 0xA862;
    /// The quest record (34h bytes) and the quest target found.
    pub const QUEST: u16 = 0x0F3E;
    pub const TARGET: u16 = 0x0F72;
    /// The player's gold (dword).
    pub const GOLD: u16 = 0x05C2;
    /// The window's scratch buffer (far pointer) and its size in bytes.
    pub const SCRATCH: u16 = 0xBA54;
    pub const SCRATCH_SIZE: u16 = 0xCB8E;
    /// The places of the city: far pointers to lists of 4-byte entries
    /// (x, y in squares, name index, suffix index), and their counts.
    pub const PLACES_A: u16 = 0x1AB5;
    pub const PLACES_B: u16 = 0x1004;
    pub const PLACES_C: u16 = 0x1008;
    pub const PLACES_A_COUNT: u16 = 0x1387;
    pub const PLACES_B_COUNT: u16 = 0x1389;
    pub const PLACES_C_COUNT: u16 = 0x138B;
    /// The gender byte of the one talking (DATA).
    pub const GENDER: u16 = 0x7387;
    /// The text buffer.
    pub const BUFFER: u16 = 0x1044;
}
use var::*;

const FLAT_BASE: u16 = 0xC38E;
const MAP_FLAGS: u16 = 0xC398;
const LEVEL_FLAGS: u16 = 0xC39A;
const MAP: u16 = 0xD806;
const FLOOR: u16 = 0xCCD0;
const PX: u16 = 0xC3D0;
const PY: u16 = 0xC3D4;
const CHEST_LOCKED: u16 = 0xC93B;
const CHEST_OPEN: u16 = 0xC93C;
const QUEST_ITEMS: u16 = 0xC93F;
const WILD_FLAT: u16 = 0xC942;
const ITEM_FLATS: u16 = 0xC934;
const MARKS: u16 = 0x0622;

/// What clicking leaves to the front end or to code not ported. The
/// message boxes (1194:59C8/59CC) and menus (1194:020E) are the
/// `TextHooks`' (the texts shown through them too).
pub trait TalkHooks: TextHooks {
    /// 1194:A374 (A36C): text number TEXT, written to BUFFER (`mode` 2)
    /// or shown (0), when the front end does it itself (Some(true));
    /// by default (Some(false)) the port's `Machine::text` runs. None
    /// stops the click.
    fn text(&mut self, _m: &mut Machine, _files: &mut Files, _mode: u16) -> Option<bool> {
        Some(false)
    }
    /// 1194:5734: a list of `cx` strings at `si`. The pick (FFFFh: none).
    fn list(&mut self, _m: &mut Machine, _si: u16, _cx: u16) -> u16 {
        0xFFFF
    }
    /// 23F1:00A3: the way to square (`x`, `y`) told, the place's name at
    /// `si`.
    fn way(&mut self, _m: &mut Machine, _si: u16, _x: u16, _y: u16) {}
    /// 1194:7B16: the loot window: AH 0 a creature's loot (AL its type),
    /// 1 a person's (AL its level), 3 an item (AL its number), 0Fh the
    /// level's item flat AL; the record at `seg`:`si`, the square `bx`
    /// (`loot_window`, without its windows).
    fn loot(&mut self, m: &mut Machine, files: &mut Files, ax: u16, si: u16, seg: u16, bx: u16) -> Option<()> {
        m.loot_window(files, &mut crate::loot_window::NoLootHooks, ax, si, seg, bx)
    }
    /// 23BA:002A: sound `ax` at the square `cx`.
    fn sound(&mut self, _m: &mut Machine, _ax: u16, _cx: u16) {}
    /// 2DA2:000A: the text at DS:`di` has its substitutions made; DI
    /// after (at its NUL), when the front end does it itself; by default
    /// (None) the port's `Machine::expand` runs.
    fn expand(&mut self, _m: &mut Machine, _di: u16) -> Option<u16> {
        None
    }
    /// 29CF:0000 with AL 0: a waiting touch spell lands on the actor at
    /// `si`. Whether one was waiting (the carry).
    fn touch_cast(&mut self, m: &mut Machine, files: &mut Files, si: u16) -> Option<bool> {
        m.resolve_cast(files, Touch::Actor(si))
    }
    /// 1194:9F04: the level's quest item at `si` is taken. The carry: it
    /// stays (`quest::quest_item_taken`; it stays too where the original
    /// would loop forever).
    fn quest_item(&mut self, m: &mut Machine, _si: u16) -> bool {
        m.quest_item_taken(&mut crate::quest::NoQuestHooks).unwrap_or(true)
    }
    /// 1194:D593: the quest's person speaks.
    fn quest_person(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quest_person_talk(files, &mut crate::quest::NoQuestHooks)
    }
    /// 1194:8F1E (from DCE1): the palace's quests, ended, offered and
    /// taken (`Machine::quests_talk`).
    fn palace(&mut self, m: &mut Machine, files: &mut Files) -> Option<()> {
        m.quests_talk(files, &mut crate::quest::NoQuestHooks)
    }
    /// 1194:CD9A: a quest is offered. The carry: none
    /// (`quest::quest_offer`; none too where the original faults).
    fn quest_offer(&mut self, m: &mut Machine) -> bool {
        m.quest_offer().unwrap_or(true)
    }
    /// 2294:0F33: a chest holding the quest's item (at `si`). The carry:
    /// it was (`quest::quest_chest`).
    fn quest_chest(&mut self, m: &mut Machine, si: u16) -> bool {
        m.quest_chest(&mut crate::quest::NoQuestHooks, si).unwrap_or(false)
    }
    /// The far routine of the building kind `kind` (D48: 4 a tavern, 5 a
    /// temple, 6 an equipment store, 7 a mages' guild; the table at
    /// 869F: B93C, C258, F48E, A73E). The temple (`shops::temple`), the
    /// tavern (`tavern::tavern_shop`) and the equipment store
    /// (`store::equipment_store`) run without their windows; the guild
    /// isn't ported.
    fn shop(&mut self, m: &mut Machine, files: &mut Files, kind: u8) -> Option<()> {
        if kind == 5 {
            return m.temple(&mut crate::shops::NoShopHooks);
        }
        if kind == 4 {
            return m.tavern_shop(files, &mut crate::tavern::NoTavernHooks);
        }
        if kind == 6 {
            return m.equipment_store(files, &mut crate::store::NoStoreHooks);
        }
        Some(())
    }
    /// 2294:0DD0: a flat of kind WILD_FLAT (or the next) in the wild.
    fn wild_flat(&mut self, _m: &mut Machine, _si: u16) {}
    /// 0000:4EFD then 0000:08EC: a dungeon's entrance in the wild.
    fn dungeon_entrance(&mut self, _m: &mut Machine, _files: &mut Files) -> Option<()> {
        Some(())
    }
    /// 217B:0682: the way in the wild (`kind` 1 the nearest inn, 2
    /// temple, 0 dungeon).
    fn wild_way(&mut self, _m: &mut Machine, _kind: u16) {}
    /// 0000:06BE: the level's text or item at map square `ax` (row
    /// high, column low). Some((its record, its number)): CF clear.
    fn square_text(&mut self, _m: &mut Machine, _ax: u16) -> Option<(u16, u16)> {
        None
    }
}

/// `TalkHooks` that do nothing.
pub struct NoTalkHooks;

impl TextHooks for NoTalkHooks {}
impl TalkHooks for NoTalkHooks {}

/// How a dialogue goes on.
enum Flow {
    /// Back to the menu (1194:E3B8).
    Menu,
    /// It ends (E3C3).
    End,
}

/// BX after 0000:1242 (the distance): the larger of |dx| and |dy|.
fn reach_bx(dx: u16, dy: u16) -> u16 {
    let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
    abs(dx).max(abs(dy))
}

/// The map offset of (x, y) as most of this code makes it: the column
/// doubled in 8 bits.
fn square_at(x: u16, y: u16) -> u16 {
    let c = (x >> 7) as u8;
    ((y >> 7) << 8) & 0xFF00 | c.wrapping_add(c) as u16
}

impl Machine {
    fn g16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    fn g8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }

    /// 1194:E702. How far the square (`x`, `y`) is from the player's, in
    /// squares (the larger axis plus a quarter of the smaller).
    fn squares_away(&self, x: u16, y: u16) -> u16 {
        crate::actors::approx(x.wrapping_sub(self.g16(PX) >> 7), y.wrapping_sub(self.g16(PY) >> 7))
    }

    /// The flat number of the actor record at `si`, less the level's
    /// first flat (16 bits).
    fn flat_of(&self, si: u16) -> u16 {
        (self.g8(si.wrapping_add(0xC)) as u16).wrapping_sub(self.g16(FLAT_BASE))
    }

    fn timed_message(&mut self, text: u16) {
        self.mem.w16(DS, MESSAGE_TIME, 0xF);
        self.mem.w16(DS, MESSAGE, text);
        self.mem.w16(DS, MESSAGE + 2, DS);
    }

    /// 1194:5E49: the window's scratch buffer freed (the menu draws in
    /// its memory).
    fn window_free(&mut self) {
        let seg = self.g16(SCRATCH + 2);
        self.heap_free(seg);
    }

    /// 1194:5E54: the scratch buffer allocated again. None where the heap
    /// fails (the original stops).
    fn window_alloc(&mut self) -> Option<()> {
        let (seg, off) = self.heap_alloc_bytes(self.g16(SCRATCH_SIZE))?;
        self.mem.w16(DS, SCRATCH, off);
        self.mem.w16(DS, SCRATCH + 2, seg);
        Some(())
    }

    /// 2542:028F: picking disarmed (and its message gone) when 5CA has
    /// any other bit set.
    fn picking_off(&mut self) {
        let f = self.g16(0x05CA);
        if f & 0xFFFE != 0 {
            self.mem.w16(DS, MESSAGE_TIME, 0);
            self.mem.w16(DS, 0x05CA, f & 0xFFFE);
        }
    }

    /// 0000:541E: the mouse cursor's background put back, if it is drawn
    /// (A902 bit 0; the drawing is the front end's).
    fn hide_cursor(&mut self) {
        let f = self.g16(0xA902);
        if f & 1 != 0 {
            self.mem.w16(DS, 0xA902, f & 0xFFFE);
        }
    }

    /// 2294:047B. What the renderer's hit test left (CLICKED) made ready
    /// for `actor_click`: an actor slot (0AB3h..0D36h) stays; the map
    /// flat drawn through the scratch record (FLAT_DRAWN, 0000:D18E) gets
    /// its +12h set to its x with the bytes swapped plus its y, and is
    /// copied (1Ch bytes) to RECORD, which becomes CLICKED; anything else
    /// is nothing (0). The 3D mode sets it as the hook CD3A, which the
    /// renderer's hit test calls on a hit (0000:C524..C52A).
    pub fn clicked_flat(&mut self) {
        let bx = self.g16(CLICKED);
        if bx == FLAT_DRAWN {
            let v = self.g16(bx).rotate_right(8).wrapping_add(self.g16(bx.wrapping_add(4)));
            self.mem.w16(DS, bx.wrapping_add(0x12), v);
            for i in 0..0x1Cu16 {
                let b = self.g8(bx.wrapping_add(i));
                self.mem.w8(DS, RECORD + i, b);
            }
            self.mem.w16(DS, CLICKED, RECORD);
        } else if !(0x0AB3..0x0D37).contains(&bx) {
            self.mem.w16(DS, CLICKED, 0);
        }
    }

    /// 1194:DB6C. The click on whatever the renderer found under the
    /// cursor (CLICKED). None where a hook, the heap or a roll fails.
    pub fn actor_click(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<()> {
        let si = self.g16(CLICKED);
        if si == 0 {
            return Some(());
        }
        if (0x0C57..0x0D37).contains(&si) {
            let d = self.squares_away(self.g16(si) >> 7, self.g16(si.wrapping_add(4)) >> 7);
            if d <= 1 && hooks.touch_cast(self, files, si)? {
                return Some(());
            }
            let rec = self.g16(si.wrapping_add(0x12));
            let (seg, name) = self.class_name(self.g16(0x1042), rec);
            self.sprintf((DS, 0x7979), (DS, 0x84D4), &[Arg::Str(seg, name)]);
            self.timed_message(0x7979);
            let si = self.g16(CLICKED);
            self.object_click(files, hooks, si)?;
            return Some(());
        }
        if self.object_click(files, hooks, si)? {
            return Some(());
        }
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        let si = self.g16(CLICKED);
        if (0x0AB3..0x0C57).contains(&si) || self.g16(MAP_FLAGS) & 4 != 0 {
            return self.talk_to(files, hooks, si);
        }
        let bx = self.flat_of(si);
        let bl = bx as u8;
        if self.g8(0x05E6) != 4 {
            if bl == self.g8(QUEST_ITEMS) || bl == self.g8(QUEST_ITEMS + 1) {
                if hooks.quest_item(self, si) {
                    return Some(());
                }
                let si = self.g16(CLICKED);
                let sq = square_at(self.g16(si), self.g16(si.wrapping_add(4)));
                self.mem.w16(self.g16(MAP), sq, 0);
                return Some(());
            }
            if self.g16(0x05BE) & 0x80 != 0 {
                return Some(());
            }
            let bh = self.g8(WILD_FLAT);
            if bh == 0xFF {
                return Some(());
            }
            if bl == bh || bl == bh.wrapping_add(1) {
                hooks.wild_flat(self, si);
            }
            return Some(());
        }
        let kind = self.g8(0x0D48);
        if kind == 1 {
            // The palace: its ruler, or the quest's person when the quest
            // (F76) names this palace.
            if bx > 1 {
                return Some(());
            }
            let q = self.g8(0x0F77) as u16;
            let named = self.g8(0x0F76) != 0
                && self.g8(0x8223u16.wrapping_add(q)) == 0
                && self.g8(0x80ECu16.wrapping_add(q)) == self.g8(0x0A46)
                && self.names_equal(self.skip_strings(DS, 0x80F4, q), 0x0A22);
            if named {
                hooks.quest_person(self, files)?;
            } else {
                hooks.palace(self, files)?;
            }
            return Some(());
        }
        if bl == self.g8(KEEPER_FLAT) {
            let Some(idx) = (0..4).find(|&k| self.g8(0x869B + k) == kind) else {
                return Some(());
            };
            let q = self.g8(0x0F77) as u16;
            let at = self.g8(0x8223u16.wrapping_add(q));
            if kind != 4 && self.g8(0x0F76) != 0 && at != 0 && kind != 6 {
                let quest_here = match kind {
                    7 => at == 7,
                    5 => at == 5,
                    _ => false,
                };
                if quest_here && (self.g8(0x0F78) == 0 || self.g8(0x0F7B) == 0 || self.g8(0x0F79) == 0) {
                    hooks.quest_person(self, files)?;
                }
            }
            self.mem.w16(DS, CLICKED, 0xFFFF);
            hooks.shop(self, files, self.g8(0x869B + idx))?;
            self.mem.w16(DS, CLICKED, 0);
            return Some(());
        }
        if (0..8).any(|k| self.g8(SAYING_FLATS + k) == bl) {
            self.mem.w8(DS, TEXT_COLOUR, 0xD4);
            self.mem.w16(DS, TEXT, 0x596);
            self.mem.w8(DS, TEXT_FLAG2, 1);
            self.mem.w8(DS, TEXT_FLAG, 0);
            self.talk_text(files, hooks, 0)?;
            return Some(());
        }
        if !self.people_kind() {
            return Some(());
        }
        let saved = (self.rng.seed, self.g16(0x1030));
        self.talk_flat(files, hooks, saved)
    }

    /// 0000:9C2C: the strings at DS:`a` and DS:`b` equal, ignoring case.
    fn names_equal(&self, mut a: u16, mut b: u16) -> bool {
        let up = |c: u8| if c.is_ascii_lowercase() { c - 0x20 } else { c };
        loop {
            let (x, y) = (up(self.g8(a)), up(self.g8(b)));
            a = a.wrapping_add(1);
            b = b.wrapping_add(1);
            if x != y {
                return false;
            }
            if x == 0 {
                return true;
            }
        }
    }

    /// 1194:E3D5. The clicked flat is one of the building's people (its
    /// kind to KIND).
    fn people_kind(&mut self) -> bool {
        let al = self.flat_of(self.g16(CLICKED)) as u8;
        match (0..0x10).find(|&k| self.g8(PEOPLE_FLATS + k) == al) {
            Some(k) => {
                self.mem.w16(DS, KIND, k);
                true
            }
            None => false,
        }
    }

    /// 1194:DDCE. Talking to a townsperson or a flat outdoors, within two
    /// squares.
    fn talk_to(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, si: u16) -> Option<()> {
        if self.squares_away(self.g16(si) >> 7, self.g16(si.wrapping_add(4)) >> 7) > 2 {
            return Some(());
        }
        let saved = (self.rng.seed, self.g16(0x1030));
        if self.g8(si.wrapping_add(0xC)) >= 0xC0 {
            return self.talk(files, hooks, saved);
        }
        let bl = self.flat_of(si) as u8;
        let wild = self.g16(LEVEL_FLAGS) & 0x200 != 0;
        if bl == self.g8(KEEPER_FLAT) {
            if wild {
                hooks.dungeon_entrance(self, files)?;
            }
            return self.talk_end(saved);
        }
        if wild && self.g16(0x05BE) & 0x80 == 0 {
            let bh = self.g8(WILD_FLAT);
            if bl == bh || bl == bh.wrapping_add(1) {
                hooks.wild_flat(self, si);
                return self.talk_end(saved);
            }
        }
        if wild || !self.people_kind() {
            return self.talk_end(saved);
        }
        self.talk_flat(files, hooks, saved)
    }

    /// 1194:DE50. One of a building's people (or a flat outdoors): not to
    /// a wanted player (5C8 bit 4000h); with picking armed, a theft.
    fn talk_flat(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, saved: (u32, u16)) -> Option<()> {
        if self.g16(0x05C8) & 0x4000 != 0 {
            hooks.message_box(self, 0x84B0);
            return self.talk_end(saved);
        }
        if self.g16(0x05CA) & 1 != 0 {
            self.steal(files, hooks)?;
            self.picking_off();
            return self.talk_end(saved);
        }
        self.hide_cursor();
        self.talk(files, hooks, saved)
    }

    /// 1194:E3C3. The dialogue's end: the seed and 1030 as they were.
    fn talk_end(&mut self, saved: (u32, u16)) -> Option<()> {
        self.mem.w16(DS, 0x1030, saved.1);
        self.rng.seed = saved.0;
        Some(())
    }

    /// 1194:DE83. The talk menu, until goodbye: a theft with picking
    /// armed, otherwise "Ask about?" (greeting, where is, rumours,
    /// goodbye).
    fn talk(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, saved: (u32, u16)) -> Option<()> {
        loop {
            let ax = if self.g16(0x05CA) & 1 != 0 {
                self.steal(files, hooks)?;
                self.picking_off();
                3
            } else {
                self.window_free();
                let ax = hooks.menu(self, 0x8523, 0x8532, 0x8536);
                self.window_alloc()?;
                ax
            };
            if ax == 3 {
                // DEBC: goodbye.
                let f = self.g16(LEVEL_FLAGS) & 0xEF7F;
                self.mem.w16(DS, LEVEL_FLAGS, f);
                self.mem.w16(DS, CLICKED, 0);
                return self.talk_end(saved);
            }
            let si = self.g16(CLICKED);
            let g = ((self.g16(si.wrapping_add(0xF)) & 0x1000) >> 12) as u8;
            self.mem.w8(DATA, GENDER, g);
            if (ax as i16) < 0 {
                return self.talk_end(saved);
            }
            let flow = match ax {
                0 => self.greeting(files, hooks)?,
                1 => self.where_is(files, hooks)?,
                _ => self.rumours(files, hooks)?,
            };
            if let Flow::End = flow {
                return self.talk_end(saved);
            }
        }
    }

    /// 1194:A374 (A36C) at the talk's calls: text TEXT shown (`mode` 0)
    /// or written to BUFFER (2), by the front end if it does it
    /// (`TalkHooks::text`), else by the port.
    fn talk_text(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, mode: u16) -> Option<()> {
        if !hooks.text(self, files, mode)? {
            self.text(files, hooks, mode, DS, BUFFER)?;
        }
        Some(())
    }

    /// 1194:E37D. Text `ax` said in a window (from BUFFER), and the quest
    /// record cleared.
    fn say(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, ax: u16) -> Option<Flow> {
        self.mem.w16(DS, TEXT, ax);
        self.mem.w8(DS, TEXT_FLAG, 0);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.talk_text(files, hooks, 2)?;
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        hooks.message_box(self, BUFFER);
        for i in 0..0x34 {
            self.mem.w8(DS, QUEST + i, 0);
        }
        Some(Flow::Menu)
    }

    /// 1194:E4E3. Text `ax` said in a window.
    fn say_box(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, ax: u16) -> Option<()> {
        self.mem.w16(DS, TEXT, ax);
        self.mem.w8(DS, TEXT_FLAG, 0);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.talk_text(files, hooks, 2)?;
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        hooks.message_box(self, BUFFER);
        Some(())
    }

    /// 1194:DF01. The greeting: a townsperson's first (64h, marking it at
    /// +12h bit 8000h) or later (73h); a building's person by its kind
    /// (the table at 84E4: the text's offset, the gender), the first time
    /// on this square (map bit 100h, then set) or again.
    fn greeting(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<Flow> {
        let si = self.g16(CLICKED);
        let ax = if self.g8(si.wrapping_add(0xC)) >= 0xC0 {
            let f = self.g16(si.wrapping_add(0x12));
            if f & 0x8000 != 0 {
                0x73
            } else {
                self.mem.w16(DS, si.wrapping_add(0x12), f | 0x8000);
                0x64
            }
        } else {
            let di = self.g16(KIND).wrapping_add(self.g16(KIND));
            let cx = self.g8(di.wrapping_add(0x84E4)) as u16;
            let bl = self.g8(di.wrapping_add(0x84E5));
            self.mem.w8(DATA, GENDER, bl);
            let f = self.g16(si.wrapping_add(0xF)) | if bl != 0 { 0x1000 } else { 0 };
            self.mem.w16(DS, si.wrapping_add(0xF), f);
            let sq = ((self.g16(si.wrapping_add(4)) >> 7) << 8).wrapping_add((self.g16(si) >> 7) << 1);
            let map = self.g16(MAP);
            let w = self.mem.r16(map, sq);
            let mut ax = if w & 0x100 != 0 {
                0x75
            } else {
                self.mem.w16(map, sq, w | 0x100);
                0x66
            };
            if cx == 0 && self.g8(0x01AB) != 0 {
                ax -= 1;
            }
            ax + cx
        };
        self.say(files, hooks, ax)
    }

    /// 1194:E7E6 / E7FD. In the wild, "I have no idea..." (84B0) or
    /// "That's in the city..." (849A), and no more.
    fn in_wild(&mut self, hooks: &mut dyn TalkHooks, text: u16) -> bool {
        if self.g16(LEVEL_FLAGS) & 0x200 == 0 {
            return false;
        }
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        hooks.message_box(self, text);
        true
    }

    /// A list shown with the window's buffer freed, LIST kept.
    fn show_list(&mut self, hooks: &mut dyn TalkHooks, si: u16, cx: u16) -> Option<u16> {
        self.window_free();
        let list = self.g16(LIST);
        let ax = hooks.list(self, si, cx);
        self.mem.w16(DS, LIST, list);
        self.window_alloc()?;
        Some(ax)
    }

    /// 1194:DF7F. "Where is...": the places by kind (85D3; the wild's at
    /// 8635), with the quest's place tenth while a quest leads to one.
    fn where_is(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<Flow> {
        let base = if self.g16(LEVEL_FLAGS) & 0x200 != 0 { 0x8635 } else { 0x85D3 };
        let pick = if self.quest_place() {
            let mut di = BUFFER;
            for k in 0..9 {
                let s = self.skip_strings(DS, base, k);
                di = self.copy_string(DS, s, di);
            }
            let s = self.skip_strings(DS, 0x1C7E, self.g8(0x0F77) as u16);
            self.copy_string(DS, s, di);
            self.mem.w16(DS, QUEST_ENTRY, 9);
            self.mem.w16(DS, LIST, BUFFER);
            let ax = self.show_list(hooks, BUFFER, 10)?;
            if (ax as i16) < 0 {
                return Some(Flow::End);
            }
            if ax == self.g16(QUEST_ENTRY) {
                hooks.quest_person(self, files)?;
                return Some(Flow::Menu);
            }
            ax
        } else {
            self.mem.w16(DS, LIST, base);
            self.show_list(hooks, base, 9)?
        };
        if (pick as i16) < 0 {
            return Some(Flow::End);
        }
        match pick {
            0 => {
                // 1194:E04F: with the quest's entry recorded as 0 (it is 9
                // whenever this holds), back to the menu.
                if self.quest_place() && self.g16(QUEST_ENTRY) == 0 {
                    return Some(Flow::Menu);
                }
                if self.in_wild(hooks, 0x84B0) {
                    return Some(Flow::Menu);
                }
                self.names_a(0);
                self.choose_place(files, hooks, PLACES_A, self.g16(PLACES_A_COUNT))
            }
            1 => {
                if self.in_wild(hooks, 0x84B0) {
                    return Some(Flow::Menu);
                }
                self.names_b(1);
                self.choose_place(files, hooks, PLACES_B, self.g16(PLACES_B_COUNT))
            }
            2 => {
                if self.in_wild(hooks, 0x84B0) {
                    return Some(Flow::Menu);
                }
                self.names_c(files, hooks)?;
                self.choose_place(files, hooks, PLACES_C, self.g16(PLACES_C_COUNT))
            }
            3..=5 => {
                if self.in_wild(hooks, 0x849A) {
                    return Some(Flow::Menu);
                }
                let (x, y) = match pick {
                    // The palace.
                    3 => (self.g8(0x138D) as u16, self.g8(0x138E) as u16),
                    // The city's gates (or work).
                    4 => (self.g16(0xC36B), self.g16(0xC36D)),
                    _ => self.nearest_gate(),
                };
                self.mem.w16(DS, WAY, x);
                self.mem.w16(DS, WAY + 2, y);
                self.tell_way(files, hooks, pick)?;
                Some(Flow::Menu)
            }
            6..=8 => {
                if self.g16(LEVEL_FLAGS) & 0x200 != 0 {
                    let kind = match pick {
                        6 => 1,
                        7 => 2,
                        _ => 0,
                    };
                    hooks.wild_way(self, kind);
                    return Some(Flow::Menu);
                }
                self.names_a(pick as u8);
                let (mut list, mut cx) = (PLACES_A, self.g16(PLACES_A_COUNT));
                if pick != 6 {
                    self.names_b(pick as u8);
                    (list, cx) = (PLACES_B, self.g16(PLACES_B_COUNT));
                    if pick != 7 {
                        self.names_c(files, hooks)?;
                        (list, cx) = (PLACES_C, self.g16(PLACES_C_COUNT));
                    }
                }
                if cx == 0 {
                    return Some(Flow::Menu);
                }
                self.nearest_place(files, hooks, list, cx)?;
                Some(Flow::Menu)
            }
            // 1194:E197: reads that decide nothing.
            _ => Some(Flow::End),
        }
    }

    /// A quest leads to a place in town (F76 set, F7D and F78 clear, F77
    /// under 8).
    fn quest_place(&self) -> bool {
        self.g8(0x0F76) != 0 && self.g8(0x0F7D) == 0 && self.g8(0x0F77) < 8 && self.g8(0x0F78) == 0
    }

    /// 1194:E14C. The nearest of the C343 points at C344 (x) and C34C (y)
    /// to the player (the first of equals). With no point the `loop` runs
    /// 65536 times. Where none is nearer than 4E20h the original keeps DI
    /// as the dialogue left it; 0 here.
    fn nearest_gate(&mut self) -> (u16, u16) {
        let n = self.g8(0xC343) as u32;
        let n = if n == 0 { 0x10000 } else { n };
        let (mut best, mut di) = (0x4E20u16, 0u16);
        let mut si = 0xC344u16;
        for _ in 0..n {
            let d = crate::actors::approx(self.g16(PX).wrapping_sub(self.g16(si)), self.g16(PY).wrapping_sub(self.g16(si.wrapping_add(8))));
            if d < best {
                best = d;
                di = si;
            }
            si = si.wrapping_add(2);
        }
        (self.g16(di) >> 7, self.g16(di.wrapping_add(8)) >> 7)
    }

    /// 0000:9CA4: copies the string at `seg`:`si` to DS:`di`, NUL
    /// included; returns DI after it.
    fn copy_string(&mut self, seg: u16, mut si: u16, mut di: u16) -> u16 {
        loop {
            let c = self.mem.r8(seg, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return di;
            }
        }
    }

    /// The far pointer at DS:`at` (offset, segment).
    fn far(&self, at: u16) -> (u16, u16) {
        (self.g16(at.wrapping_add(2)), self.g16(at))
    }

    /// 1194:E50D. The names of the places in PLACES_A at A23E (LIST): a
    /// name (3276:3CB6) and a suffix by the region (3D5D, or 3DF7 in the
    /// Imperial province or where A4C is not among the 3Ah at 47E2);
    /// entries whose name word equals the last one named are left out.
    /// A byte `al` ends the list (0 once a name was written).
    fn names_a(&mut self, al: u8) {
        let n = self.g16(PLACES_A_COUNT);
        if n == 0 {
            return;
        }
        let (seg, mut bx) = self.far(PLACES_A);
        let mut di = 0xA23E;
        self.mem.w16(DS, LIST, di);
        let (mut dx, mut al) = (0xFFFFu16, al);
        for k in (1..=n).rev() {
            if dx != self.mem.r16(seg, bx.wrapping_add(2)) {
                dx = self.mem.r16(seg, bx.wrapping_add(2));
                let cx = (k & 0xFF00) | self.mem.r8(seg, bx.wrapping_add(2)) as u16;
                let mut si = self.skip_strings(DATA, 0x3CB6, cx);
                loop {
                    let c = self.mem.r8(DATA, si);
                    si = si.wrapping_add(1);
                    if c == 0 {
                        break;
                    }
                    self.mem.w8(DS, di, c);
                    di = di.wrapping_add(1);
                }
                self.mem.w8(DS, di, b' ');
                di = di.wrapping_add(1);
                let suffixes = if self.g8(0x0A46) != 8 && (0..0x3A).any(|i| self.g8(0x47E2 + i) == self.g8(0x0A4C)) {
                    0x3D5D
                } else {
                    0x3DF7
                };
                let si = self.skip_strings(DATA, suffixes, self.mem.r8(seg, bx.wrapping_add(3)) as u16);
                di = self.copy_string(DATA, si, di);
                al = 0;
            }
            bx = bx.wrapping_add(4);
        }
        self.mem.w8(DS, di, al);
    }

    /// 1194:E594. The names of the places in PLACES_B at A23E (LIST): a
    /// name (3276:3E81) run into a suffix by the entry's first byte (0
    /// 3EAC, 1 3EE9, else 3F28). As `names_a`, entries repeating the last
    /// named word are left out and `al` ends the list.
    fn names_b(&mut self, al: u8) {
        let n = self.g16(PLACES_B_COUNT);
        if n == 0 {
            return;
        }
        let (seg, mut bx) = self.far(PLACES_B);
        let mut di = 0xA23E;
        self.mem.w16(DS, LIST, di);
        let (mut dx, mut al) = (0xFFFFu16, al);
        for k in (1..=n).rev() {
            if dx != self.mem.r16(seg, bx.wrapping_add(2)) {
                dx = self.mem.r16(seg, bx.wrapping_add(2));
                let kind = self.mem.r8(seg, bx.wrapping_add(2));
                let cx = (k & 0xFF00) | kind as u16;
                let mut si = self.skip_strings(DATA, 0x3E81, cx);
                loop {
                    let c = self.mem.r8(DATA, si);
                    si = si.wrapping_add(1);
                    if c == 0 {
                        break;
                    }
                    self.mem.w8(DS, di, c);
                    di = di.wrapping_add(1);
                }
                let suffixes = match kind {
                    0 => 0x3EAC,
                    1 => 0x3EE9,
                    _ => 0x3F28,
                };
                let si = self.skip_strings(DATA, suffixes, self.mem.r8(seg, bx.wrapping_add(3)) as u16);
                di = self.copy_string(DATA, si, di);
                al = 0;
            }
            bx = bx.wrapping_add(4);
        }
        self.mem.w8(DS, di, al);
    }

    /// 1194:E5FE. The names of the places in PLACES_C at A23E (LIST): two
    /// words (3276:3F71 and 403E) with their substitutions made (2DA2:000A,
    /// with the place's square at WAY and CLICKED FFFFh meanwhile).
    fn names_c(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<()> {
        let n = self.g16(PLACES_C_COUNT);
        if n == 0 {
            return Some(());
        }
        let (seg, mut bx) = self.far(PLACES_C);
        let mut di = 0xA23Eu16;
        self.mem.w16(DS, LIST, di);
        for _ in 0..n {
            let saved = self.g16(CLICKED);
            self.mem.w16(DS, CLICKED, 0xFFFF);
            let start = di;
            self.mem.w16(DS, WAY, self.mem.r8(seg, bx) as u16);
            self.mem.w16(DS, WAY + 2, self.mem.r8(seg, bx.wrapping_add(1)) as u16);
            let dx = self.mem.r16(seg, bx.wrapping_add(2));
            let si = self.skip_strings(DATA, 0x3F71, dx & 0xFF);
            let d = self.copy_string(DATA, si, di);
            self.mem.w8(DS, d.wrapping_sub(1), b' ');
            let si = self.skip_strings(DATA, 0x403E, dx >> 8);
            self.copy_string(DATA, si, d);
            di = match hooks.expand(self, start) {
                Some(d) => d,
                None => self.expand(files, hooks, DS, start)?,
            }
            .wrapping_add(1);
            self.mem.w16(DS, CLICKED, saved);
            bx = bx.wrapping_add(4);
        }
        Some(())
    }

    /// 1194:E087. The places of `list` (`cx` of them, their names at LIST)
    /// sorted and shown; the way told to the one picked.
    fn choose_place(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, list: u16, cx: u16) -> Option<Flow> {
        let (seg, start) = self.far(list);
        let list_at = self.g16(LIST);
        let di = self.sort_names(list_at, cx);
        self.mem.w16(DS, LIST, list_at);
        let ax = self.show_list(hooks, di, cx)?;
        if (ax as i16) < 0 {
            return Some(Flow::End);
        }
        let idx = self.g8(BUFFER.wrapping_add(ax)) as u16;
        let e = start.wrapping_add(idx << 2);
        self.mem.w16(DS, WAY, self.mem.r8(seg, e) as u16);
        self.mem.w16(DS, WAY + 2, self.mem.r8(seg, e.wrapping_add(1)) as u16);
        self.tell_way(files, hooks, idx)?;
        Some(Flow::Menu)
    }

    /// 1194:E20C. The nearest place of `list` (`cx` > 0 entries) to the
    /// player's square (the last of equals); the way told to it. (The
    /// original starts from 3E8h and BP; squares are bytes, so the first
    /// entry is always nearer.)
    fn nearest_place(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, list: u16, cx: u16) -> Option<()> {
        let (seg, start) = self.far(list);
        let (px, py) = (self.g16(PX) >> 7, self.g16(PY) >> 7);
        let mut best = 0x3E8u16;
        let mut bp = start;
        let mut bx = start;
        for _ in 0..cx {
            let a = (self.mem.r8(seg, bx) as u16).wrapping_sub(px);
            let b = (self.mem.r8(seg, bx.wrapping_add(1)) as u16).wrapping_sub(py);
            let d = crate::actors::approx(a, b);
            if best >= d {
                best = d;
                bp = bx;
            }
            bx = bx.wrapping_add(4);
        }
        self.mem.w16(DS, WAY, self.mem.r8(seg, bp) as u16);
        self.mem.w16(DS, WAY + 2, self.mem.r8(seg, bp.wrapping_add(1)) as u16);
        self.tell_way(files, hooks, bp.wrapping_sub(start) >> 2)
    }

    /// 0000:0B9C. The `cx` strings at `si` sorted (0000:0BC6, an exchange
    /// sort by 0000:9C0A), their order kept at BUFFER (the original
    /// indices), the pointers at A09E and the sorted strings after them.
    /// Returns where the sorted strings start (`si` for one or none).
    fn sort_names(&mut self, si: u16, cx: u16) -> u16 {
        if cx <= 1 {
            return si;
        }
        let table = 0x929Eu16 + 0x800;
        let mut al = 0u8;
        for k in 0..cx {
            self.mem.w8(DS, BUFFER.wrapping_add(k), al);
            al = al.wrapping_add(1);
        }
        let (mut s, mut bx) = (si, table);
        for _ in 0..cx {
            self.mem.w16(DS, bx, s);
            let mut n = 0u16;
            while self.g8(s.wrapping_add(n)) != 0 {
                n = n.wrapping_add(1);
            }
            s = s.wrapping_add(n + 1);
            bx = bx.wrapping_add(2);
        }
        let end = bx;
        // 0000:0BC6.
        self.mem.w16(DS, 0xA85E, 0);
        let mut p = table;
        for outer in (1..cx).rev() {
            let mut q = p.wrapping_add(2);
            let v = self.g16(0xA85E).wrapping_add(1);
            self.mem.w16(DS, 0xA860, v);
            for _ in 0..outer {
                let (a, b) = (self.g16(p), self.g16(q));
                if (compare(self, a, b) as i16) < 0 {
                    self.mem.w16(DS, p, b);
                    self.mem.w16(DS, q, a);
                    let x = BUFFER.wrapping_add(self.g16(0xA860));
                    let y = BUFFER.wrapping_add(self.g16(0xA85E));
                    let (bx_, by) = (self.g8(x), self.g8(y));
                    self.mem.w8(DS, y, bx_);
                    self.mem.w8(DS, x, by);
                }
                q = q.wrapping_add(2);
                let v = self.g16(0xA860).wrapping_add(1);
                self.mem.w16(DS, 0xA860, v);
            }
            p = p.wrapping_add(2);
            let v = self.g16(0xA85E).wrapping_add(1);
            self.mem.w16(DS, 0xA85E, v);
        }
        let mut di = end;
        let mut bx = table;
        for _ in 0..cx {
            di = self.copy_string(DS, self.g16(bx), di);
            bx = bx.wrapping_add(2);
        }
        end
    }

    /// 1194:E680. The way to WAY told: indoors (5E6 4) "I'm not sure..."
    /// (85A9); 16 squares or more away, text 104h; else text 105h, then
    /// the way to the list's entry `cx` (23F1:00A3). The list is kept in
    /// the scratch buffer while the text is written.
    fn tell_way(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, cx: u16) -> Option<()> {
        if self.g8(0x05E6) == 4 {
            self.mem.w8(DS, TEXT_COLOUR, 0x60);
            hooks.message_box(self, 0x85A9);
            return Some(());
        }
        let (seg, off) = self.far(SCRATCH);
        let list = self.g16(LIST);
        for i in 0..0x384u16 {
            let c = self.g8(list.wrapping_add(i));
            self.mem.w8(seg, off.wrapping_add(i), c);
        }
        let d = self.squares_away(self.g16(WAY), self.g16(WAY + 2));
        let list = self.g16(LIST);
        if (d as i16) >= 0x10 {
            self.say_box(files, hooks, 0x104);
            self.mem.w16(DS, LIST, list);
            return Some(());
        }
        self.say_box(files, hooks, 0x105);
        self.mem.w16(DS, LIST, list);
        let (seg, off) = self.far(SCRATCH);
        for i in 0..0x384u16 {
            let c = self.mem.r8(seg, off.wrapping_add(i));
            self.mem.w8(DS, list.wrapping_add(i), c);
        }
        let si = self.skip_strings(DS, self.g16(LIST), cx);
        hooks.way(self, si, self.g16(WAY), self.g16(WAY + 2));
        Some(())
    }

    /// 1194:E26B. Rumours: the menu "Rumor Type" (general, work) with the
    /// seed taken from 151 meanwhile; a quest already being told (QUEST
    /// +4 bits 6-7) says where it stands instead. General rumours need a
    /// person who knows any (+12h bit 8), work a roll of d100 within
    /// (level + 1) x 2 (at most 20) for a quest's offer; "Where" a
    /// quest's target is (+F bit 8) by its distance.
    fn rumours(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<Flow> {
        self.rng.seed = self.g16(0x0151) as u32;
        self.window_free();
        let ax = hooks.menu(self, 0x857B, 0x8589, 0x858C);
        self.window_alloc()?;
        let q = self.g16(QUEST + 4);
        if (q & 0xC0) >> 6 != 0 {
            let k = (q & 0xE00) >> 9;
            let t = match k {
                0 => 0xA8,
                1 => 0xA2,
                _ => k + 0xA1,
            };
            return self.say(files, hooks, t);
        }
        if (ax as i16) < 0 {
            return Some(Flow::End);
        }
        if ax == 0 {
            let si = self.g16(CLICKED);
            if self.g16(si.wrapping_add(0x12)) & 8 == 0 {
                return self.say(files, hooks, 0x9F);
            }
            let roll = self.rng.next() % 100;
            let bx = ((self.g8(0x01AA) as u16 + 1) * 2).min(0x14);
            if roll > bx {
                return self.say(files, hooks, 0xB9);
            }
            if self.quest_offered(files, hooks)? {
                return Some(Flow::Menu);
            }
            return self.say(files, hooks, 0x9F);
        }
        let si = self.g16(CLICKED);
        if self.g16(si.wrapping_add(0xF)) & 8 == 0 {
            return self.say(files, hooks, 0xA0);
        }
        if self.in_wild(hooks, 0x84B0) {
            return Some(Flow::Menu);
        }
        let Some(d) = self.nearest_target() else {
            return self.say(files, hooks, 0xA0);
        };
        let si = self.g16(TARGET);
        self.mem.w8(DATA, GENDER, self.g8(si.wrapping_add(0x1E)) & 1);
        let k = (self.g16(si.wrapping_add(4)) & 0xE00) >> 9;
        let t = if (d as i16) <= 0x28 {
            0xBF + k
        } else if (d as i16) <= 0x50 {
            0xBB + k
        } else {
            0xBA
        };
        self.say(files, hooks, t)
    }

    /// 1194:E4A3. A quest offered (1194:CD9A): its text CCh + FDD (8
    /// bits) shown, the way to its square (FDE, FDF) at WAY. Whether it
    /// was (CF clear).
    fn quest_offered(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<bool> {
        if hooks.quest_offer(self) {
            return Some(false);
        }
        self.mem.w16(DS, WAY, self.g8(0x0FDE) as u16);
        self.mem.w16(DS, WAY + 2, self.g8(0x0FDF) as u16);
        self.mem.w16(DS, TEXT, 0xCCu8.wrapping_add(self.g8(0x0FDD)) as u16);
        self.mem.w8(DS, TEXT_FLAG, 0);
        self.mem.w8(DS, TEXT_COLOUR, 0x60);
        self.mem.w8(DS, TEXT_FLAG2, 0);
        self.mem.w8(DS, 0x0AB2, 0);
        self.talk_text(files, hooks, 0)?;
        self.mem.w8(DS, 0x0AB2, 0xFF);
        Some(true)
    }

    /// 1194:E729. The nearest of the four quest targets at DCA (29h
    /// bytes each, +4 bits 6-7 equal to 2) to the player, in squares
    /// (the first of equals): its square to WAY and the record to
    /// TARGET. None when there is none.
    fn nearest_target(&mut self) -> Option<u16> {
        let mut dx = 0xFFFFu16;
        let mut found = None;
        let mut si = 0x0DCAu16;
        for _ in 0..4 {
            if (self.g16(si.wrapping_add(4)) & 0xC0) >> 6 == 2 {
                let (x, y) = (self.g8(si.wrapping_add(0x10)) as u16, self.g8(si.wrapping_add(0x11)) as u16);
                let d = self.squares_away(x, y);
                if d < dx {
                    dx = d;
                    found = Some(si);
                    self.mem.w16(DS, WAY, x);
                    self.mem.w16(DS, WAY + 2, y);
                }
            }
            si = si.wrapping_add(0x29);
        }
        if dx == 0xFFFF {
            return None;
        }
        // The original stores FS (left at the record found by its last
        // `mov fs, si`) to TARGET whenever any was found.
        self.mem.w16(DS, TARGET, found.unwrap_or(0));
        Some(dx)
    }

    /// 1194:E777. A pickpocketing try: the lockpicking roll (1194:53F9)
    /// against 4 less the place's size (A47). Caught (0000:3F6E: d100
    /// under 100 less the chance), the guards come (0000:3EBE), else "You
    /// are not successful..."; done, a purse of 1..4 gold (text 563h +
    /// the roll) or, one time in 3.3, nothing (568h). None where the
    /// roll divides by zero.
    fn steal(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks) -> Option<()> {
        self.mem.w8(DS, LOCK_TRIED, 4u8.wrapping_sub(self.g8(0x0A47)));
        let (ok, chance) = self.lockpick()?;
        if !ok {
            let cx = 100u16.wrapping_sub(chance);
            let roll = self.rng.next() % 100;
            if roll < cx {
                self.call_guards();
            } else {
                self.timed_message(0x8480);
            }
            return Some(());
        }
        if self.rng.next() >= 0x4E20 {
            self.mem.w16(DS, TEXT, 0x568);
        } else {
            let k = self.rng.range(0, 4)?;
            let gold = self.mem.r32(DS, GOLD).wrapping_add(k as u32 + 1);
            self.mem.w32(DS, GOLD, gold);
            self.mem.w16(DS, TEXT, 0x563 + k);
            self.mem.w8(DS, TEXT_FLAG2, 0xFF);
        }
        self.mem.w8(DS, TEXT_FLAG, 0);
        self.talk_text(files, hooks, 0)?;
        Some(())
    }

    /// 292B:06DC. The click on the actor record at `si` as a thing, within
    /// E6h: indoors, a chest (locked ones picked or opened by a waiting
    /// Open spell, the lock's level from the square's seed), an item
    /// lying in the level (its number from the level's text) or one of
    /// the item flats; a corpse (or one dying) looted, or its marked item
    /// moved to the player. Whether it took the click (the carry). None
    /// where a roll faults or a hook fails.
    ///
    /// The distance is 0000:1242's, whose `cdq` makes |dx| only when
    /// EAX's high half is negative; the port takes it as following AX.
    pub fn object_click(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, si: u16) -> Option<bool> {
        let dx = self.g16(si).wrapping_sub(self.g16(PX));
        let dy = self.g16(si.wrapping_add(4)).wrapping_sub(self.g16(PY));
        if crate::actors::approx(dx, dy) > 0xE6 {
            return Some(false);
        }
        if si == RECORD {
            if self.g16(MAP_FLAGS) & 4 != 0 {
                return Some(false);
            }
            let al = self.flat_of(si) as u8;
            if al == self.g8(CHEST_OPEN) {
                if hooks.quest_chest(self, si) {
                    return Some(true);
                }
            } else if al == self.g8(CHEST_LOCKED) {
                if hooks.quest_chest(self, si) {
                    return Some(true);
                }
                return self.chest(hooks, si).map(|()| true);
            }
            return self.item_flat(files, hooks, si, al);
        }
        if !(0x0C57..0x0D37).contains(&si) {
            return Some(false);
        }
        if self.g16(si.wrapping_add(0xF)) & 0x4000 != 0 {
            return Some(false);
        }
        let corpse = (self.g8(crate::actors::var::CORPSE) as u16).wrapping_add(self.g16(FLAT_BASE)) as u8;
        let sprite = self.g8(si.wrapping_add(0xC));
        if sprite != corpse && sprite != 0xC5 {
            return Some(false);
        }
        if let Some(k) = (0..8u16).find(|&k| self.g16(MARKS + 6 * k) == si) {
            let at = MARKS + 6 * k;
            self.mem.w16(DS, at, 0xFFFF);
            let al = self.g8(at + 4);
            let di = self.g16(at + 2);
            if al == 0x7B {
                self.mem.w8(DS, di.wrapping_sub(0x389A), (self.g16(PX) >> 7) as u8);
                self.mem.w8(DS, di.wrapping_sub(0x3899), (self.g16(PY) >> 7) as u8);
                return Some(true);
            }
            self.mem.w16(DS, di.wrapping_sub(0x389A), 0xFFFF);
            hooks.loot(self, files, 0x300 | al as u16, si, DS, reach_bx(dx, dy))?;
            return Some(true);
        }
        let rec = self.g16(si.wrapping_add(0x12));
        let seg = self.g16(0x1042);
        let ax = if self.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x1000 != 0 {
            self.mem.r8(seg, rec.wrapping_add(4)) as u16
        } else {
            0x100 | self.mem.r8(seg, rec.wrapping_add(5)) as u16
        };
        hooks.loot(self, files, ax, rec, seg, reach_bx(dx, dy))?;
        Some(true)
    }

    /// 292B:07FB. A map flat that is one of the 14 item flats from
    /// ITEM_FLATS (not the quest's): the level's item at its square
    /// (taken from the map), or the item flat itself.
    fn item_flat(&mut self, files: &mut Files, hooks: &mut dyn TalkHooks, si: u16, al: u8) -> Option<bool> {
        if al == self.g8(QUEST_ITEMS) || al == self.g8(QUEST_ITEMS + 1) {
            return Some(false);
        }
        if !(0..0xE).any(|k| self.g8(ITEM_FLATS + k) == al) {
            return Some(false);
        }
        let (x, y) = (self.g16(si), self.g16(si.wrapping_add(4)));
        let sq = square_at(x, y);
        if al != self.g8(ITEM_FLATS + 1) {
            let ax = 0x0F00 | (self.flat_of(si) & 0xFF);
            hooks.loot(self, files, ax, si, DS, sq)?;
            return Some(true);
        }
        let key = (((y >> 7) as u8 as u16) << 8) | ((x >> 7) & 0xFF);
        let Some((rec, bx)) = hooks.square_text(self, key) else {
            self.timed_message(0x7CD3);
            return Some(true);
        };
        self.mem.w16(DS, rec, 0xFFFF);
        let map = self.g16(MAP);
        let fs = if (self.mem.r16(map, sq) as i16) < 0 {
            self.mem.w16(map, sq, 0);
            map
        } else {
            self.mem.w8(self.g16(FLOOR), sq, 0);
            self.g16(FLOOR)
        };
        hooks.loot(self, files, 0x300 | (bx & 0xFF), si, fs, sq)?;
        Some(true)
    }

    /// 292B:0743..07FA. A locked chest: its lock's level 1..14 by the
    /// square (the seed, meanwhile); a waiting Open spell (0000:414E)
    /// opens it; otherwise "This lock..." unless picking is armed, then a
    /// try (failures are recorded with the doors', 2826:0B3E, but looked
    /// up under the wrong square, 2826:0B30: tries never end). Opened,
    /// the flat becomes CHEST_OPEN, "Open" with the chest's sound.
    fn chest(&mut self, hooks: &mut dyn TalkHooks, si: u16) -> Option<()> {
        let sq = square_at(self.g16(si), self.g16(si.wrapping_add(4)));
        let saved = self.rng.seed;
        self.rng.seed = sq as u32;
        let level = self.rng.range(0, 0xE)? as u8;
        self.rng.seed = saved;
        let level = level.wrapping_add(1);
        self.mem.w8(DS, LOCK_TRIED, level);
        if !self.open_spell(level) {
            self.lockpick()?;
            if self.g16(0x05CA) & 1 == 0 {
                self.locked_message(self.g8(LOCK_TRIED))?;
                return Some(());
            }
            self.picking_off();
            // The original's bug: 1194:53F9 left SI at the player's record
            // (01A4), so the square looked up among the failures is made
            // from its words, not the chest's; it is recorded below under
            // the chest's own.
            let sq = square_at(self.g16(0x01A4), self.g16(0x01A8));
            if (0..0x20).any(|i| self.g16(PICK_FAILED + 2 * i) == sq) {
                self.pick_outcome(false);
                return Some(());
            }
            if !self.lockpick()?.0 {
                let clicked = self.g16(CLICKED);
                let sq = square_at(self.g16(clicked), self.g16(clicked.wrapping_add(4)));
                self.mem.w16(DS, PICKED, sq);
                if let Some(i) = (0..0x20).find(|&i| self.g16(PICK_FAILED + 2 * i) == 0xFFFF) {
                    self.mem.w16(DS, PICK_FAILED + 2 * i, sq);
                }
                self.mem.w16(DS, PICKED, 0);
                self.pick_outcome(false);
                return Some(());
            }
        }
        self.pick_outcome(true);
        let clicked = self.g16(CLICKED);
        let sq = square_at(self.g16(clicked), self.g16(clicked.wrapping_add(4)));
        self.mem.w8(self.g16(MAP), sq, self.g8(CHEST_OPEN));
        self.mem.w16(DS, MESSAGE, 0x7C4B);
        self.mem.w16(DS, MESSAGE + 2, DS);
        self.mem.w16(DS, MESSAGE_TIME, 0xF);
        hooks.sound(self, 0x15, self.g16(0xCB90));
        Some(())
    }

    /// 292B:0456 / 0469. "Failure..." or "Success..." for 15 frames.
    fn pick_outcome(&mut self, ok: bool) {
        self.timed_message(if ok { 0x7C05 } else { 0x7BFA });
    }
}

/// 0000:9C0A. Compares the strings at DS:`a` and DS:`b` over `b`'s length
/// and its NUL: 0 equal; at the first difference, 1 when the byte
/// difference has its sign bit set, else FFFFh.
fn compare(m: &Machine, a: u16, b: u16) -> u16 {
    let mut n = 0u16;
    while m.mem.r8(DS, b.wrapping_add(n)) != 0 {
        n = n.wrapping_add(1);
    }
    for i in 0..=n {
        let (x, y) = (m.mem.r8(DS, a.wrapping_add(i)), m.mem.r8(DS, b.wrapping_add(i)));
        if x != y {
            return if x.wrapping_sub(y) & 0x80 != 0 { 1 } else { 0xFFFF };
        }
    }
    0
}
