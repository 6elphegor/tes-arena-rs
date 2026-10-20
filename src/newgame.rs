//! A new game: the character created and the game's state started
//! (1194:2498, from the main menu's "new game", 0000:1134).
//!
//! The steps, in the original's order:
//! - the new game's state cleared (`newgame_reset`, 1194:24A6..2512);
//! - the class: by ten questions (the quiz, 222C:0000: `quiz_pick`,
//!   `quiz_find`, `quiz_answers`, `quiz_class`) or picked from the list
//!   of eighteen (1194:2678, the table at 2AEE), then its starting weapon
//!   and weapon class (`class_chosen`, 1194:26DC);
//! - the name (1194:2711) and the sex (2791), as the player gives them;
//! - the province on the map (27C2): the race is the province, the
//!   portrait rolled (1194:35FF), and from that byte a starting location
//!   (1194:3AE8; `province_chosen`);
//! - the attributes (2A69): rolled (`creation_roll`, 1194:2AC7) as often
//!   as the player asks, the bonus points spent on them
//!   (`creation_select`, 1194:14C5; `creation_arrow`, 138E), then kept
//!   (`creation_finish`, 2D28 and 2BAE);
//! - the face (2BC5; `face_chosen`, 2C72);
//! - the end (`creation_end`, 1194:257B..25B7).
//!
//! `new_game` runs them all from the player's choices (`Character`).
//! Then, from the main menu (0000:1139..1171), the game is set up
//! (`game_setup`, 21EE:016C: the first date, `game_date`, 0000:13FC; the
//! weather, 1194:2FB3) and started (0000:0C66: `game_start`, the start
//! dungeon chosen, then its run, `Machine::start_run`). The intro's
//! texts (1194:D459) and the save slot's files (25FD:0219) are the front
//! end's.
//!
//! The screens, the text typed, the menus, the map and the faces' screen
//! are the front end's (`NewGameHooks`); the routines here start from
//! what the player picked there.

use crate::bsa::Files;
use crate::character::PLAYER;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// DGROUP variables.
pub mod var {
    /// The quiz: the questions asked (their numbers, 1..40) and how many,
    /// the three answers' kinds (0, 1 or 2), the counts of each kind
    /// chosen, the byte the answers' parse cut off (put back after), the
    /// question text (a far pointer) and the question shown's position.
    pub const ASKED: u16 = 0x40CB;
    pub const ASKED_COUNT: u16 = 0x40C7;
    pub const KINDS: u16 = 0x40C4;
    pub const COUNTS: u16 = 0x40C8;
    pub const CUT: u16 = 0x40C3;
    pub const QUESTIONS: u16 = 0x40E8;
    pub const SHOWN: u16 = 0x40EC;
    /// The answer picked (0..2).
    pub const ANSWER: u16 = 0x129E;
    /// The file of answer counts to classes, and the questions'.
    pub const CLASSES_DAT: u16 = 0x40F8;
    pub const QUESTION_TXT: u16 = 0x411E;
    /// The sound played at each answer (BURST5.VOC, 23BA:0183's name).
    pub const ANSWER_SOUND: u16 = 0x4167;
    /// The questions' window's record (STARTGAM.MNU), the scroll's
    /// rollers' files (SCROLL01.DFA, SCROLL02.DFA 0Dh on) and their frames'
    /// segments (banks 1 and 2), the scroll's bitmap's segment (bank 3's
    /// frame) and its strings' block's.
    pub const QUIZ_WINDOW: u16 = 0x4172;
    pub const ROLLERS_DFA: u16 = 0x4104;
    pub const ROLLERS: u16 = 0x40F4;
    pub const BITMAP: u16 = 0x40F2;
    pub const STRINGS: u16 = 0x40F0;
    /// The scroll: its strings' count, the last top line it goes to, its
    /// top line, the rows rolled past it (a row 140h) and the rollers'
    /// frame (0..15).
    pub const LINES: u16 = 0x1294;
    pub const SCROLL_LAST: u16 = 0x1296;
    pub const SCROLL_LINE: u16 = 0x1298;
    pub const SCROLL_ROWS: u16 = 0x129A;
    pub const SCROLL_FRAME: u16 = 0x129C;
    /// The answers' kinds' figures' names (words, by kind) and colour 0's
    /// shades by a kind's count less one.
    pub const FIGURES: u16 = 0x4142;
    pub const SHADES: u16 = 0x40DA;
    /// The attribute screen: the attribute picked (0..7), the arrows'
    /// middle line, the points left and the points rolled; the mouse's y.
    pub const PICKED: u16 = 0x1296;
    pub const ARROWS: u16 = 0x1297;
    pub const POINTS: u16 = 0x129C;
    pub const POINTS_ROLLED: u16 = 0x9686;
    pub const MOUSE_Y: u16 = 0x8F94;
    /// The region clicked (the attribute: 1..8).
    pub const REGION: u16 = 0xB7C6;
    /// The face picked (0..9), and the faces in use (the player's first).
    pub const FACE_PICKED: u16 = 0x129A;
    pub const FACES: u16 = 0x0FD4;
    /// The screen flags and the text colour.
    pub const SCREEN: u16 = 0xB7C4;
    pub const COLOUR: u16 = 0x9288;
    /// Shown the attribute screen's note once.
    pub const NOTED: u16 = 0x1299;
}
use var::*;

/// The player's record's fields used here (from `PLAYER`).
mod rec {
    pub const RACE: u16 = 0x04;
    pub const CLASS: u16 = 0x05;
    pub const LEVEL: u16 = 0x06;
    pub const SEX: u16 = 0x07;
    pub const PORTRAIT: u16 = 0x08;
    pub const ATTRS: u16 = 0x29;
    pub const BASE_ATTRS: u16 = 0x31;
    pub const HP_MOD: u16 = 0x4F;
    pub const HP: u16 = 0x59;
    pub const MAX_HP: u16 = 0x5B;
    pub const FACE: u16 = 0x6B;
    pub const ITEMS: u16 = 0x6E;
}

/// The attribute screen's arrows region (its record's y, in the region
/// table at 3276:5312, record 9).
const ARROWS_Y: u16 = 0x5312 + 0x372 + 2;

/// What the front end does during creation.
pub trait NewGameHooks {
    /// 1194:59CC: a message box with the text at `seg`:`si`.
    fn message_box(&mut self, _m: &mut Machine, _seg: u16, _si: u16) {}
    /// 1194:0DC7: the stats screen: CHARSHET.$$$ loaded into bank [1030]
    /// and drawn over the page (1647, `sheet_back`), then the doll (16AA,
    /// `sheet_doll`: the clothes' names by sex and class, the worn items
    /// counted, 129D, the face, the trousers and the clothes, each loaded
    /// into bank [1030] and drawn). Their pixels land where the page is
    /// the heap's block. (Where a file is missing the reason is noted,
    /// `giveup`.)
    fn stats_screen(&mut self, m: &mut Machine, files: &mut Files) {
        let _ = m.sheet_back(files).and_then(|_| m.sheet_doll(files));
    }
    /// 1194:5CD9 with BX 10h in the roll: the picture named at DS:`si`
    /// (CHARSTAT.IMG at 2ADD, BONUS.IMG at 2B9B) loaded into bank [1030]
    /// and drawn (`sheet_image`; its colours and showing the front end's).
    fn roll_picture(&mut self, m: &mut Machine, files: &mut Files, si: u16) {
        // (Where it gives up the reason is noted, `giveup`.)
        let _ = m.sheet_image(files, si);
    }
    /// 1194:2B42..2B52, in the roll before the stats screen: the page's
    /// corner kept in bank [1032] (12BE, `corner_kept`; 1299 cleared
    /// round it), then 157E: the points' number (12E6, `points_drawn`),
    /// the arrows (12F5, `arrows_made`: 1297 38h, drawn) and the
    /// stats' texts (0DE1, `sheet_texts`).
    fn roll_screen(&mut self, m: &mut Machine, files: &mut Files) {
        m.corner_kept();
        m.points_drawn();
        // (Where it gives up the reason is noted, `giveup`.)
        let _ = m.arrows_made(files);
        m.sheet_texts();
    }
    /// 1194:1695: the attribute screen's palette.
    fn palette(&mut self, _m: &mut Machine) {}
    /// 0000:9326 with AX [1038], at the end of creation: the parchment's
    /// resource dropped (`Machine::resource_drop`: its slot's first word
    /// cleared, its block freed, 91BC counted down).
    fn parchment_drop(&mut self, m: &mut Machine) {
        let slot = m.mem.r16(DS, 0x1038);
        m.resource_drop(slot);
    }
    /// 1194:55B6, at the end of creation: the colours at 929E black and
    /// the page cleared (the fade the front end's).
    fn faded(&mut self, m: &mut Machine) {
        m.sheet_black();
    }
    /// 1194:158F, at the end of creation: the sheet's screen and the face
    /// kept (CHARSHET.$$$, SAVEFACE.$$$; 5E0 bit 200h: `sheet_keep`).
    fn sheet_keep(&mut self, m: &mut Machine, files: &mut Files) {
        // (Where it gives up the reason is noted, `giveup`.)
        let _ = m.sheet_keep(files);
    }
    /// 0000:5D70, at the end of creation: EMS banks freed but the
    /// level's (`Machine::ems_reset`).
    fn creation_done(&mut self, m: &mut Machine) {
        m.ems_reset();
    }
    /// 2294:11F2, at the end of creation.
    fn game_screen(&mut self, _m: &mut Machine) {}
    /// 2294:01F1: the game's resources loaded (fonts, images; the far
    /// pointers at 4391, 438D, 1002 and on).
    fn resources(&mut self, _m: &mut Machine) {}
    /// 0000:3C4C (SI 01A4): the hands' graphics for the weapon class.
    fn hands(&mut self, _m: &mut Machine) {}
    /// 29CF:0691: the spells loaded (`Machine::spells_load`).
    fn spells(&mut self, _m: &mut Machine) {}
    /// 21EE:0134: the level's flats loaded again (0000:FEF4, BFA5, BED4,
    /// FECE), and the first flat's number (C38E) from D432.
    fn level_flats(&mut self, _m: &mut Machine) {}
}

/// How the class is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassPick {
    /// The ten questions' answers (0..2 each).
    Quiz([u8; 10]),
    /// The class list's entry (0..17, the table at 2AEE).
    Listed(u8),
}

/// The player's choices in creation.
#[derive(Clone, Debug)]
pub struct Character {
    pub class: ClassPick,
    pub name: Vec<u8>,
    pub sex: u8,
    pub province: u8,
    pub face: u8,
    /// The attributes rolled again this many times.
    pub rerolls: u16,
    /// The attribute screen's clicks; none spends the points in turn.
    pub clicks: Vec<Click>,
}

/// `NewGameHooks` that do nothing.
pub struct NoNewGameHooks;

impl NewGameHooks for NoNewGameHooks {}

/// A click on the attribute screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// An attribute's line (1..8, the region's number).
    Attribute(u16),
    /// The arrows, at the mouse's y.
    Arrows(u16),
}

impl Machine {
    fn ng8(&self, o: u16) -> u8 {
        self.mem.r8(DS, o)
    }

    fn ng16(&self, o: u16) -> u16 {
        self.mem.r16(DS, o)
    }

    /// 0000:77C8: `n` bytes of `al` from DGROUP:`di`, returning DI after.
    fn ng_fill(&mut self, mut di: u16, n: u16, al: u8) -> u16 {
        for _ in 0..n {
            self.mem.w8(DS, di, al);
            di = di.wrapping_add(1);
        }
        di
    }

    /// 1194:24A6..2512. The new game's state cleared: 12BA set, 5E0 bit
    /// 200h clear, 5C8 bit 10h set, 1299 and 5CC cleared, the player's
    /// record (41Eh bytes from 01A4) zeroed, then 0Bh bytes from F76 and
    /// A4h from E6E. The next fill is given SI = 0DCA but fills from DI,
    /// where the last one ended (F12h), D0h bytes: the quest records at
    /// 0DCA are left and F12..FE1 zeroed. Then 1194:D327 (AB2 FFh, the
    /// main quest's 13h bytes from FDD zeroed, FE4 FFh), the faces in use
    /// (FD4, 0Ah bytes) FFh and the keys (652, 0Ch) C5h.
    pub fn newgame_reset(&mut self) {
        self.mem.w16(DS, 0x12BA, 1);
        let v = self.ng16(0x05E0) & 0xFDFF;
        self.mem.w16(DS, 0x05E0, v);
        let v = self.ng16(0x05C8) | 0x10;
        self.mem.w16(DS, 0x05C8, v);
        self.mem.w8(DS, NOTED, 0);
        self.mem.w32(DS, 0x05CC, 0);
        self.ng_fill(PLAYER, 0x41E, 0);
        self.ng_fill(0x0F76, 0xB, 0);
        let di = self.ng_fill(0x0E6E, 0xA4, 0);
        self.ng_fill(di, 0xD0, 0);
        self.main_quest_reset();
        self.ng_fill(FACES, 0xA, 0xFF);
        self.ng_fill(0x0652, 0xC, 0xC5);
    }

    /// 1194:2512..2530, after `newgame_reset`: FONT_C.DAT (its name at
    /// 3EF2) loaded whole (0000:7504) to [102A], the class list's title's
    /// font; PARCH.CIF (3AE0) added to the resources as 20 by 20 tiles
    /// (0000:756C), its slot at 1038 and selected as the boxes' tiles
    /// (9AF8: B038).
    pub fn creation_font(&mut self, files: &mut Files) {
        let seg = self.font_load(files, 0x3EF2);
        self.mem.w16(DS, 0x102A, seg);
        let slot = self.resource_add(files, 0x3AE0, 0x14, 0x14).unwrap_or(0);
        self.mem.w16(DS, 0x1038, slot);
        self.select_tiles(0x1038);
    }

    /// 1194:25B9..25D1, how the class is chosen, its screen: to black
    /// (55B6: 929E black, the page cleared), the page cleared again
    /// (9426), and STARTGAM's picture at 3ED9 drawn (5CD9 with BX 40h:
    /// loaded into bank [1030], onto the page). Showing the page (9466),
    /// the cursor (541E) and setting its colours are the front end's; the
    /// menu after is `creation`'s.
    pub fn creation_method_screen(&mut self, files: &mut Files) -> Option<()> {
        self.sheet_black();
        self.page_clear();
        self.sheet_image(files, 0x3ED9)
    }

    /// 1194:2536..2542, the class chosen (1194:25B9, by the questions or
    /// the list): FONT_C ([102A]) freed (0000:930A: its word in the fonts'
    /// table cleared, its block freed) and the colour EEh (9288) for the
    /// name's prompt.
    pub fn class_done(&mut self) {
        let seg = self.mem.r16(DS, 0x102A);
        self.font_free(seg);
        self.mem.w8(DS, 0x9288, 0xEE);
    }

    /// 1194:D327: the main quest's state started over: AB2 FFh, its 13h
    /// bytes from FDD zeroed, FE4 FFh. (Also the frame's debug keys Alt+F5,
    /// ACED and ACF4, which then say "Artifact Quest now possible.")
    pub fn main_quest_reset(&mut self) {
        self.mem.w8(DS, 0x0AB2, 0xFF);
        self.ng_fill(0x0FDD, 0x13, 0);
        self.mem.w8(DS, 0x0FE4, 0xFF);
    }

    /// 222C:057F. The next question: a number 1..40 (the RNG's next
    /// word mod 40, plus 1) not among those asked (rolled again until
    /// it isn't), added to the list. The first is taken as rolled. None
    /// where the RNG cycles without finding one.
    pub fn quiz_pick(&mut self) -> Option<u8> {
        let n = self.ng8(ASKED_COUNT);
        if n == 0 {
            self.mem.w8(DS, ASKED_COUNT, 1);
            let q = (self.rng.next() % 0x28) as u8 + 1;
            self.mem.w8(DS, ASKED, q);
            return Some(q);
        }
        let mut check = crate::random::CycleCheck::new(&self.rng);
        loop {
            let q = (self.rng.next() % 0x28) as u8 + 1;
            let n = self.ng8(ASKED_COUNT) as u16;
            if !(0..n).any(|i| self.ng8(ASKED.wrapping_add(i)) == q) {
                self.mem.w8(DS, ASKED.wrapping_add(n), q);
                self.mem.w8(DS, ASKED_COUNT, n as u8 + 1);
                return Some(q);
            }
            if check.cycled(&self.rng) {
                return crate::giveup::original("222C:057F: no question left: the original rolls for ever");
            }
        }
    }

    /// 222C:0525. Question `number` in the text at the far pointer
    /// QUESTIONS: from the text's start (past bytes below 21h), each line
    /// that starts with a digit is read as a number (0000:52BA); at the
    /// match, the position past it and its dot (DI + 2, + 3 from 10 on).
    /// Otherwise on past the next line feed (searched at most FDE8h
    /// bytes). None where the original searches forever (no such
    /// question: the search comes round again).
    pub fn quiz_find(&mut self, number: u16) -> Option<u16> {
        let (mut di, es) = (self.ng16(QUESTIONS), self.ng16(QUESTIONS + 2));
        let skip_low = |m: &Self, di: &mut u16| -> Option<()> {
            for _ in 0..0x10000u32 {
                if m.mem.r8(es, *di) >= 0x21 {
                    return Some(());
                }
                *di = di.wrapping_add(1);
            }
            None
        };
        skip_low(self, &mut di)?;
        // Where the number is read, and where a line's search starts: a
        // position seen again means the original goes round forever.
        let mut seen = vec![false; 0x10000];
        let mut searched = vec![false; 0x10000];
        loop {
            if std::mem::replace(&mut seen[di as usize], true) {
                return crate::giveup::original("222C:0525: the question not in the file: the original searches for ever");
            }
            if self.info_atoi(es, di) == number {
                let add = 2 + (number >= 10) as u16;
                return Some(di.wrapping_add(add));
            }
            loop {
                if std::mem::replace(&mut searched[di as usize], true) {
                    return crate::giveup::original("222C:0525: the question not in the file: the original searches for ever");
                }
                for _ in 0..0xFDE8u32 {
                    let b = self.mem.r8(es, di);
                    di = di.wrapping_add(1);
                    if b == 0x0A {
                        break;
                    }
                }
                skip_low(self, &mut di)?;
                if self.mem.r8(es, di).is_ascii_digit() {
                    break;
                }
            }
        }
    }

    /// 222C:02D4..02DE with 222C:05C9. The question at `es`:`di`: past its
    /// colon, its three answers each marked "(Nk)": the kind after the
    /// digit, `v` 0, `l` 2, any other 1, into KINDS; the "(Nk" blanked
    /// (from the parenthesis on, four bytes). The byte five past the
    /// last answer's digit is cut (to 0), kept at CUT. Returns DI after.
    pub fn quiz_answers(&mut self, es: u16, mut di: u16) -> u16 {
        let scan = |m: &Self, di: &mut u16, al: u8| {
            for _ in 0..0xFDE8u32 {
                let b = m.mem.r8(es, *di);
                *di = di.wrapping_add(1);
                if b == al {
                    break;
                }
            }
        };
        scan(self, &mut di, b':');
        for i in 0..3u16 {
            scan(self, &mut di, b'(');
            let kind = match self.mem.r8(es, di.wrapping_add(1)) {
                b'v' => 0,
                b'l' => 2,
                _ => 1,
            };
            self.mem.w8(DS, KINDS + i, kind);
            for k in [0xFFFFu16, 0, 1, 2] {
                self.mem.w8(es, di.wrapping_add(k), b' ');
            }
        }
        let cut = self.mem.r8(es, di.wrapping_add(5));
        self.mem.w8(es, di.wrapping_add(5), 0);
        self.mem.w8(DS, CUT, cut);
        di
    }

    /// The answer picked (0..2) counted: one more of its kind (222C:010B).
    pub fn quiz_count(&mut self, answer: u16) {
        let kind = self.ng8(KINDS.wrapping_add(answer)) as u16;
        let v = self.ng8(COUNTS.wrapping_add(kind)).wrapping_add(1);
        self.mem.w8(DS, COUNTS.wrapping_add(kind), v);
    }

    /// 222C:005B. The class for the counts of the answers' kinds (40C8,
    /// 40C9, 40CA): CLASSES.DAT loaded to the frame (9030:0, EMS off),
    /// its 30h triples from offset 12h searched for the counts: the first
    /// gives the class at 0, the n-th the class at n / 4. Not found, the
    /// next 12h triples are searched on (from 0A2h): the class at 0Ch +
    /// n / 3 (n 12h when not found either). None where the file can't be
    /// loaded.
    pub fn quiz_class(&mut self, files: &mut Files) -> Option<u8> {
        let frame = self.ng16(0x9030);
        self.file_load(files, CLASSES_DAT, frame, 0)?;
        let key = [self.ng8(COUNTS), self.ng8(COUNTS + 1), self.ng8(COUNTS + 2)];
        let search = |m: &Self, si: &mut u16, n: u16| -> (u16, bool) {
            for bx in 0..n {
                let t = [0, 1, 2].map(|k| m.mem.r8(frame, si.wrapping_add(k)));
                if t == key {
                    return (bx, true);
                }
                *si = si.wrapping_add(3);
            }
            (n, false)
        };
        let mut si = 0x12u16;
        let at = match search(self, &mut si, 0x30) {
            (0, true) => 0,
            (bx, true) => bx / 4,
            _ => {
                let (bx, _) = search(self, &mut si, 0x12);
                bx / 3 + 0xC
            }
        };
        Some(self.mem.r8(frame, at))
    }

    /// 1194:2678..26DB: the class list. The cursor put back (541E, the
    /// front end's), the screen at 3ED9 drawn (5CD9 with BX 0: loaded
    /// into bank [1030], its colours the VGA's) and shown (9466, the front
    /// end's); the 18 classes' names in the order of 2AEE (each byte's low
    /// 5 bits the name's place in the list at 2A62, 5585) copied one after
    /// another to 1044 (9CA0); B7C4 cleared, FONT_C ([102A]) selected, the
    /// list of kind 6 over them under the title at 3F56 (1194:5734,
    /// `list_window_scrolled`: `pick` the line the library's loop takes,
    /// `top` the first line it leaves shown), then FONT_B ([102C])
    /// selected. Returns the class picked: 2AEE's byte at the pick (AX
    /// whole, as BX). None where the list gives up.
    pub fn class_list(&mut self, files: &mut Files, pick: u16, top: u16) -> Option<u8> {
        self.sheet_image(files, 0x3ED9)?;
        let mut di = 0x1044u16;
        for k in 0..0x12u16 {
            let cx = (self.mem.r8(DS, 0x2AEE + k) & 0x1F) as u16;
            let mut si = self.skip_strings(DS, 0x2A62, cx);
            loop {
                let c = self.mem.r8(DS, si);
                self.mem.w8(DS, di, c);
                si = si.wrapping_add(1);
                di = di.wrapping_add(1);
                if c == 0 {
                    break;
                }
            }
        }
        self.mem.w8(DS, 0xB7C4, 0);
        self.select_font(0x102A);
        let ax = self.list_window_scrolled(files, 6, (DS, 0x1044), 0x12, 0x3F56, pick, top)?;
        let class = self.mem.r8(DS, 0x2AEEu16.wrapping_add(ax));
        self.select_font(0x102C);
        Some(class)
    }

    /// 1194:2655..2677 then 26DC. The class `class` chosen (its byte to
    /// the player's record), then its starting weapon: the weapon maker
    /// (1194:E8B2) with level 0 and the type forced to 3 (from class 0Ch
    /// on), 2 (from 6) or 1, copied (13h bytes from 13DA) to the first
    /// item slot (0212); the weapon class (1194:731A) set. The class's
    /// number (low 5 bits) decides, its other bits are kept.
    pub fn class_chosen(&mut self, class: u8) -> Option<()> {
        self.mem.w8(DS, PLAYER + rec::CLASS, class);
        self.class_weapon(class & 0x1F)
    }

    /// 1194:26DC with AL `class`.
    pub fn class_weapon(&mut self, class: u8) -> Option<()> {
        let kind = if class >= 0xC {
            3
        } else if class >= 6 {
            2
        } else {
            1
        };
        self.loot_weapon(0, kind)?;
        for i in 0..0x13u16 {
            let b = self.ng8(0x13DA + i);
            self.mem.w8(DS, PLAYER + rec::ITEMS + i, b);
        }
        self.weapon_class(DS, PLAYER);
        Some(())
    }

    /// 1194:2711..2783: the name's prompt made at 1044 for 5C34
    /// (`Machine::named_shown` with AX 3Ch, CX 12h, DI 01AD; the field,
    /// `lineedit::fields::creation_name`, then `named_closed`; 2789 asks
    /// again while 01AD is empty). 1044 cleared for 7Dh dwords, FONT_A
    /// ([1026]) selected (0000:9AE6), then 3276:35F8 formatted there with
    /// the class's name (2686:0645). Its length is taken by 9CB0 from
    /// DGROUP at the name's offset whatever its segment (a player's is
    /// DGROUP's); ten less it, twice, less four (none at ten, none below
    /// nothing) is the room kept at 110C, and that many spaces replace the
    /// text's last character before a CR and a NUL.
    pub fn creation_name_prompt(&mut self) {
        for i in 0..0x7D * 4 {
            self.mem.w8(DS, 0x1044 + i, 0);
        }
        self.select_font(0x1026);
        let (seg, off) = self.class_name(DS, PLAYER);
        let len = self.mem.cstr(DS, off).len() as u16;
        let mut cx = 10u16.wrapping_sub(len);
        if cx != 0 {
            cx = cx.wrapping_add(cx).wrapping_sub(4);
            if cx & 0x8000 != 0 {
                cx = 0;
            }
        }
        self.mem.w16(DS, 0x110C, cx);
        let end = self.sprintf((DS, 0x1044), (DATA, 0x35F8), &[crate::format::Arg::Str(seg, off)]);
        if cx != 0 {
            let mut di = end.wrapping_sub(1);
            for _ in 0..cx {
                self.mem.w8(DS, di, b' ');
                di = di.wrapping_add(1);
            }
            self.mem.w8(DS, di, 0x0D);
            self.mem.w8(DS, di.wrapping_add(1), 0);
        }
    }

    /// 2791 and 2711's results: the sex (0 male, 1 female) and the name
    /// (up to 12h characters, then a NUL), as the player gave them.
    pub fn name_and_sex(&mut self, name: &[u8], sex: u8) {
        let n = name.len().min(0x12);
        for (i, &c) in name[..n].iter().enumerate() {
            self.mem.w8(DS, PLAYER + 9 + i as u16, c);
        }
        self.mem.w8(DS, PLAYER + 9 + n as u16, 0);
        self.mem.w8(DS, PLAYER + rec::SEX, sex);
    }

    /// 1194:27C2..2836, the map's screen made once the sex is given: the
    /// colours at 929E black and the page cleared (55B2, its fade the
    /// front end's), the page cleared again (9426); in bank [1030] the
    /// frame cleared (77C8) and, the frame made the page meanwhile,
    /// TAMRIEL.MNU (3EE6) and NOEXIT.IMG (3EC4) each loaded to a heap
    /// block from its top and drawn there (56C8 with BX 0: 742C with DI
    /// FFFEh, 5828, the block freed; their colours, 9B56, the VGA's); in
    /// bank [1032] TAMRIEL.MNU loaded to the frame's start undrawn (BX
    /// 84h); then the parchment's unrolling (1194:5B58).
    pub fn creation_map(&mut self, files: &mut Files) -> Option<()> {
        self.sheet_black();
        self.page_clear();
        let frame = self.mem.r16(DS, 0x9030);
        self.ems_map(self.mem.r16(DS, 0x1030));
        for i in 0..0xFA00u16 {
            self.mem.w8(frame, i, 0);
        }
        let page = self.mem.r16(DS, 0xA900);
        for si in [0x3EE6, 0x3EC4] {
            self.mem.w16(DS, 0xA900, frame);
            let r = self.picture_heap_drawn(files, si);
            self.mem.w16(DS, 0xA900, page);
            r?;
        }
        self.ems_map(self.mem.r16(DS, 0x1032));
        self.picture_to_frame(files, 0x3EE6, 0x84)?;
        self.ems_map(self.mem.r16(DS, 0x1030));
        self.parchment_unrolled(files)
    }

    /// 0000:56C8 with BX 0: the picture named at DS:`si` loaded whole to a
    /// heap block from its top (742C with DI FFFEh: 9CD6), its header's
    /// x, y, width and height to A836..A83C, drawn from there into the
    /// page ([A900]: the EMS frame here, which 5828 writes as it would any
    /// other), the block freed (6C36: not the frame's). Its colours
    /// (9B56) are the VGA's. None where the file can't be loaded.
    fn picture_heap_drawn(&mut self, files: &mut Files, si: u16) -> Option<()> {
        let seg = self.load_alloc_high(files, si);
        if seg == 0 {
            return crate::giveup::not_ported("0000:742C: a picture with no room on the heap, or no file");
        }
        for k in 0..4u16 {
            let v = self.mem.r16(seg, 2 * k);
            self.mem.w16(DS, 0xA836 + 2 * k, v);
        }
        let dest = self.mem.r16(DS, 0xA900);
        let mut px: Vec<u8> = (0..64000u32).map(|i| self.mem.r8(dest, i as u16)).collect();
        crate::render::image_page(self, &mut px, seg, 0);
        // (As `page_image`: a page that is no heap block, A000h in a game
        // the port starts itself, keeps its bytes; the EMS frame made the
        // page takes them.)
        if dest == self.mem.r16(DS, 0x9030) || self.page_is_block() {
            for (i, &b) in px.iter().enumerate() {
                self.mem.w8(dest, i as u16, b);
            }
        }
        self.heap_free(seg);
        Some(())
    }

    /// 1194:5B58 with SI 3F16, as the state has it: TOP.DFA loaded to the
    /// frame's start with bank 2 mapped (7428); D6h steps, each the first
    /// frame's y (+4) set to the step's (FFF1h on), the DFA's frame A896
    /// drawn (5526) and A896 counted on modulo 10h, the rows the scroll
    /// has passed copied from bank [1030]'s frame (7702, 94EE); then that
    /// frame drawn whole over the page (5998) and bank 2 mapped again.
    /// The page's rows are the front end's but the last drawing's, the
    /// frame over it whole where the page is the heap's block.
    fn parchment_unrolled(&mut self, files: &mut Files) -> Option<()> {
        let frame = self.mem.r16(DS, 0x9030);
        self.ems_map(2);
        self.file_load(files, 0x3F16, frame, 0)?;
        let steps = 0xD6u16;
        self.mem.w16(frame, 4, 0xFFF1u16.wrapping_add(steps - 1));
        self.mem.w16(DS, 0xA896, steps & 0xF);
        self.ems_map(self.mem.r16(DS, 0x1030));
        if self.page_is_block() {
            let page = self.mem.r16(DS, 0xA900);
            for i in 0..64000u16 {
                let b = self.mem.r8(frame, i);
                self.mem.w8(page, i, b);
            }
        }
        self.ems_map(2);
        Some(())
    }

    /// 1194:286B..288E, the map's window up (after its box, 2837..2868,
    /// the front end's): the page cleared (9426), bank [1032]'s frame the
    /// window's buffer (4D74), B7E4 set (the screen black: no colours
    /// read) and B7C4 bit 1, the window on DS:3FC7 opened (3159:000C up to
    /// its loop: TAMRIEL.MNU, its eight provinces' regions) and its first
    /// routine (1194:28EC: NOEXIT.IMG over the exit, 5CD9 with BX 50h).
    pub fn creation_map_window(&mut self, files: &mut Files) -> Option<()> {
        self.page_clear();
        self.screen_bank(self.mem.r16(DS, 0x1032));
        self.mem.w8(DS, crate::window::var::NO_FADE, 1);
        let f = self.mem.r8(DS, SCREEN) | 2;
        self.mem.w8(DS, SCREEN, f);
        self.window_open(files, 0x3FC7, None)?;
        self.sheet_image(files, 0x3EC4)?;
        self.window_opened();
        Some(())
    }

    /// The map's window clicked at (`x`, `y`) (8F92, 8F94): the province
    /// under it (3159:019A, the regions' masks), or None, the window left
    /// up. A province's handler (1194:28F6) takes the region to the race
    /// (1A8) and ends the window's loop (3159:0006); its end (00D0..00EA:
    /// the masks freed, the keys flushed) and 2891..2896 (B7C4 bit 1 and
    /// B7E4 cleared) follow.
    pub fn creation_map_click(&mut self, x: u16, y: u16) -> Option<u8> {
        self.mem.w16(DS, 0x8F92, x);
        self.mem.w16(DS, MOUSE_Y, y);
        self.window_regions();
        let r = self.mem.r16(DS, REGION);
        if r == 0xFFFF {
            return None;
        }
        self.mem.w8(DS, PLAYER + rec::RACE, r as u8);
        self.window_close();
        self.window_ended();
        let f = self.mem.r8(DS, SCREEN) & 0xFD;
        self.mem.w8(DS, SCREEN, f);
        self.mem.w8(DS, crate::window::var::NO_FADE, 0);
        Some(r as u8)
    }

    /// 1194:2557..256D and 2A69..2A89, between the province's texts and
    /// the first roll: the colours black and the page cleared (55B6, the
    /// fade the front end's); the sheet kept (158F: the body drawn and the
    /// page written to CHARSHET.$$$, the face to SAVEFACE.$$$); the page
    /// cleared, the boxes' tiles [103A] (9AF8) and FONT_B ([102C], 9AE6);
    /// then the attribute screen's start: the page cleared, the body drawn
    /// again (161A with BX 50h: by a heap block from the top, 56C8 with BX
    /// bit 2 clear, as `picture_heap_drawn`), its head's corner (DAh, 0;
    /// 4Ah x 37h)
    /// kept in bank 1's frame (2A9D: 8F26), the page cleared and the
    /// colours black again (55B2), D49 9. None where a file can't be read.
    pub fn creation_attributes_begin(&mut self, files: &mut Files) -> Option<()> {
        self.sheet_black();
        self.sheet_keep(files)?;
        self.page_clear();
        self.select_tiles(0x103A);
        self.select_font(0x102C);
        self.page_clear();
        let si = self.body_name();
        self.picture_heap_drawn(files, si)?;
        let old = self.map_bank(1, 0);
        let frame = self.mem.r16(DS, 0x9030);
        self.page_rect_save((frame, 0), 0xDA, 0, 0x4A, 0x37);
        self.map_bank(old, 0);
        self.page_clear();
        self.sheet_black();
        self.mem.w8(DS, 0x0D49, 9);
        Some(())
    }

    /// 1194:2945..2970, the province accepted: in the text made at 1044
    /// (from 3276:36EC) a woman's "his home" made "her home": the eight
    /// bytes at 3F1E sought from 1044 over 3E8h places (9C56), the three
    /// at 3F27 copied where they matched (7702), or at 1044 + 3E8h where
    /// nothing did (the loop's end, bug kept).
    pub fn creation_her_home(&mut self) {
        if self.ng8(PLAYER + rec::SEX) == 0 {
            return;
        }
        let mut di = 0x1044u16;
        for _ in 0..0x3E8 {
            if (0..8u16).all(|k| self.mem.r8(DS, di.wrapping_add(k)) == self.mem.r8(DS, 0x3F1E + k)) {
                break;
            }
            di = di.wrapping_add(1);
        }
        for k in 0..3u16 {
            let b = self.mem.r8(DS, 0x3F27 + k);
            self.mem.w8(DS, di.wrapping_add(k), b);
        }
    }

    /// 1194:2A00..2A5F, after the province's text was made at 1044
    /// (29DE..29FD: text 581h + the province, wrapped at 28h): the colour
    /// EEh, the boxes' tiles [1038] (9AF8) and the font [102C] (9AE6); the
    /// text moved on 13h bytes (7732, 1EDh of them) and "Know ye this
    /// also:\r" (3EAA, 13h bytes) put before it; in the text its line
    /// feeds made spaces (49DF) and the text wrapped again at 137E (49FF);
    /// then every line feed of the whole a CR (49DF), and its NUL a CR
    /// with a NUL after (2A51..2A5F). None where the wrap gives up.
    pub fn creation_known_layout(&mut self) -> Option<()> {
        self.mem.w8(DS, COLOUR, 0xEE);
        self.select_tiles(0x1038);
        self.select_font(0x102C);
        let moved: Vec<u8> = (0..0x1EDu16).map(|i| self.mem.r8(DS, 0x1044 + i)).collect();
        for (i, &b) in moved.iter().enumerate() {
            self.mem.w8(DS, 0x1057 + i as u16, b);
        }
        for i in 0..0x13u16 {
            let b = self.mem.r8(DS, 0x3EAA + i);
            self.mem.w8(DS, 0x1044 + i, b);
        }
        // 1194:49DF: AL made BL up to the text's NUL.
        let replace = |m: &mut Machine, si: u16, al: u8, bl: u8| {
            let mut si = si;
            while m.mem.r8(DS, si) != 0 {
                if m.mem.r8(DS, si) == al {
                    m.mem.w8(DS, si, bl);
                }
                si = si.wrapping_add(1);
            }
        };
        replace(self, 0x1057, 0x0A, 0x20);
        self.line_wrap(0x1057)?;
        replace(self, 0x1044, 0x0A, 0x0D);
        let mut di = 0x1044u16;
        for _ in 0..0x200 {
            let b = self.mem.r8(DS, di);
            di = di.wrapping_add(1);
            if b == 0 {
                break;
            }
        }
        self.mem.w8(DS, di.wrapping_sub(1), 0x0D);
        self.mem.w8(DS, di, 0);
        Some(())
    }

    /// 1194:28F6, 289B and 28E1: the province picked on the map (0..8)
    /// is the race; the portrait rolled (1194:35FF); that byte is the
    /// location the player comes from (its province and a place in it),
    /// whose record is built (1194:3AE8, the Imperial City for province
    /// 8, `city_build_location`).
    pub fn province_chosen(&mut self, files: &mut Files, province: u8) -> Option<()> {
        self.province_clicked(province)?;
        self.province_accepted(files)
    }

    /// 1194:28F6 and 289B: a province clicked on the map: the race, and
    /// the portrait rolled, before the player is asked to accept it (each
    /// click rolls again).
    pub fn province_clicked(&mut self, province: u8) -> Option<()> {
        self.mem.w8(DS, PLAYER + rec::RACE, province);
        self.cr_portrait(DS, PLAYER)
    }

    /// 1194:28E1: the province accepted: the location the portrait byte
    /// names built (1194:3AE8; the Imperial City for province 8), by 3B84
    /// in full (`city_location`: its province's data loaded, 3C0D, and the
    /// climate read from the terrain map, 3D91).
    pub fn province_accepted(&mut self, files: &mut Files) -> Option<()> {
        let mut id = self.ng8(PLAYER + rec::PORTRAIT) as u16;
        if self.ng8(0x0A22 + 0x24) == 8 {
            id = 0x100;
        }
        self.city_location(files, DS, 0x0A22, id)
    }

    /// 1194:2AC7. The attributes rolled: 9686 the points to give (19h),
    /// the gold (1194:2B87), the attributes, hit points and spell points
    /// (2AFA..2B37, `create_player_stats`: the level 0 first and 1297
    /// 34h), the bonus points (1194:2B9B: a die of 19h, into 129C), then
    /// the stats screen; the first time (1299 clear) its palette and the
    /// note on spending the points (3276:37C5), 1299 set. The screen
    /// flag B7C4 bit 4 is set and the text colour FEh.
    pub fn creation_roll(&mut self, files: &mut Files, hooks: &mut dyn NewGameHooks) -> Option<()> {
        self.mem.w16(DS, POINTS_ROLLED, 0x19);
        self.roll_starting_gold()?;
        self.mem.w8(DS, COLOUR, 0xFE);
        hooks.roll_picture(self, files, 0x3DE2);
        let v = self.ng8(SCREEN) | 4;
        self.mem.w8(DS, SCREEN, v);
        self.mem.w8(DS, PLAYER + rec::LEVEL, 0);
        self.mem.w8(DS, ARROWS, 0x34);
        self.create_player_stats()?;
        hooks.roll_picture(self, files, 0x3DFC);
        let p = crate::dice::dice(&mut self.rng, 0x19)?;
        self.mem.w8(DS, POINTS, p as u8);
        hooks.roll_screen(self, files);
        hooks.stats_screen(self, files);
        if self.ng8(NOTED) == 0 {
            hooks.palette(self);
            self.mem.w8(DS, NOTED, 1);
            hooks.message_box(self, DATA, 0x37C5);
        }
        Some(())
    }

    /// 1194:14C5. An attribute's line clicked (REGION `region`, 1..8):
    /// it is picked (1296: region - 1) and the arrows move to its line
    /// (their region's y 30h + 8 × the pick, in the DATA segment's region
    /// table; 1297 that + 8, its low byte).
    pub fn creation_select(&mut self, region: u16) {
        self.mem.w16(DS, REGION, region);
        let bx = region.wrapping_sub(1);
        self.mem.w8(DS, PICKED, bx as u8);
        let y = 8u16.wrapping_mul(bx).wrapping_add(0x30);
        self.mem.w16(DATA, ARROWS_Y, y);
        self.mem.w8(DS, ARROWS, y.wrapping_add(8) as u8);
    }

    /// 1194:138E. The arrows clicked at the mouse's y `y`: at or above
    /// their middle (1297) the picked attribute rises by one, if it is
    /// below 100 and points are left (one is spent); below it, it falls by
    /// one if above its rolled value (01D5..) and fewer points are left
    /// than were rolled (9686, low byte; one comes back). After either,
    /// the derived stats (1194:1408).
    pub fn creation_arrow(&mut self, y: u16) {
        self.mem.w16(DS, MOUSE_Y, y);
        let i = self.ng8(PICKED) as u16;
        let rolled = self.ng8(PLAYER + rec::BASE_ATTRS + i);
        let si = PLAYER + rec::ATTRS + i;
        let points = self.ng8(POINTS);
        if self.ng16(MOUSE_Y) <= self.ng8(ARROWS) as u16 {
            if self.ng8(si) >= 0x64 || points == 0 {
                return;
            }
            self.mem.w8(DS, si, self.ng8(si) + 1);
            self.mem.w8(DS, POINTS, points.wrapping_sub(1));
        } else {
            if self.ng8(si) <= rolled || points >= self.ng16(POINTS_ROLLED) as u8 {
                return;
            }
            self.mem.w8(DS, si, self.ng8(si).wrapping_sub(1));
            self.mem.w8(DS, POINTS, points.wrapping_add(1));
        }
        self.player_derived_stats();
    }

    /// 1194:138E whole, its game's side: the arrows clicked
    /// (`creation_arrow`) and, where that took (a point spent or given
    /// back, the derived stats made), the stats' texts made again (0DE1,
    /// `sheet_texts`); the page put back from bank [1032] (149D), the
    /// points' number (12E6) and the arrows (1354) are drawn.
    pub fn attribute_arrow(&mut self, y: u16) {
        let points = self.ng8(POINTS);
        self.creation_arrow(y);
        if self.ng8(POINTS) != points {
            self.sheet_texts();
        }
    }

    /// A click on the attribute screen.
    pub fn creation_click(&mut self, click: Click) {
        match click {
            Click::Attribute(r) => self.creation_select(r),
            Click::Arrows(y) => self.creation_arrow(y),
        }
    }

    /// 1194:2D28, then 2BAE. The attributes kept: the screen's flag bit 4
    /// cleared; the hit points and their maximum the maximum plus the HP
    /// modifier (recomputed while the points were spent); the attributes
    /// to the 256 scale, and copied as the base ones. A class with bit 20h
    /// (the spellcasters) starts with spells 3Dh and 3Eh (50B, 50C; 50A
    /// 2).
    pub fn creation_finish(&mut self) {
        let v = self.ng8(SCREEN) & 0xFB;
        self.mem.w8(DS, SCREEN, v);
        let hp = self.ng16(PLAYER + rec::MAX_HP).wrapping_add(self.ng16(PLAYER + rec::HP_MOD));
        self.mem.w16(DS, PLAYER + rec::HP, hp);
        self.mem.w16(DS, PLAYER + rec::MAX_HP, hp);
        let a = PLAYER + rec::ATTRS;
        crate::scale::attrs_to256(&mut self.mem, a, a, 8);
        for i in 0..8 {
            let b = self.ng8(a + i);
            self.mem.w8(DS, PLAYER + rec::BASE_ATTRS + i, b);
        }
        if self.ng8(PLAYER + rec::CLASS) & 0x20 != 0 {
            self.mem.w8(DS, 0x050B, 0x3D);
            self.mem.w8(DS, 0x050C, 0x3E);
            self.mem.w8(DS, 0x050A, 2);
        }
    }

    /// 1194:2CC0..2CCF, the faces' screen's Done (its region's handler,
    /// from 5CF4: B7C6 its index, 1, and B7C4 cleared as the loop
    /// begins): B7C4 bit 4 cleared, bit 1 set (3159:0006), 1298 1 (the
    /// loop over the faces left).
    pub fn face_done(&mut self) {
        self.mem.w16(DS, 0xB7C6, 1);
        let v = self.ng8(0xB7C4) & 0xFB | 1;
        self.mem.w8(DS, 0xB7C4, v);
        self.mem.w8(DS, 0x1298, 1);
    }

    /// 1194:2C72..2C95. The face picked (129A, 0..9) is the player's
    /// (+6Bh), and the first of the faces in use (FD4): the province × 10
    /// plus it, in a byte, with bit 80h for a woman.
    pub fn face_chosen(&mut self) {
        let f = self.ng8(FACE_PICKED);
        self.mem.w8(DS, PLAYER + rec::FACE, f);
        let mut v = (self.ng8(PLAYER + rec::RACE) as u16).wrapping_mul(10) as u8;
        v = v.wrapping_add(f);
        if self.ng8(PLAYER + rec::SEX) != 0 {
            v |= 0x80;
        }
        self.mem.w8(DS, FACES, v);
    }

    /// The questions asked and answered (222C:0000; the screens' pixels
    /// the front end's): the start (`quiz_begin`), ten times a question's
    /// window (`quiz_question`) and its answer (`quiz_answer`), then the
    /// class (`quiz_end`).
    pub fn class_quiz(&mut self, files: &mut Files, answers: &[u8; 10]) -> Option<u8> {
        self.quiz_begin(files)?;
        for &a in answers {
            self.quiz_question(files)?;
            self.quiz_answer(files, a)?;
        }
        self.quiz_end(files)
    }

    /// `class_quiz`'s start (222C:0000..0008, 222C:0185): the seed kept at
    /// 12A8, FONT_C ([102E], 9AE6), the answers' sound (BURST5.VOC) loaded
    /// and readied (23BA:0183, `sound_quiz_load`), the counts cleared, the
    /// colour EEh; QUESTION.TXT loaded into bank 4's frame (its far pointer
    /// at QUESTIONS), the scroll's rollers SCROLL01.DFA into bank 1's
    /// (its segment at ROLLERS) and SCROLL02.DFA into bank 2's (at
    /// ROLLERS + 2), bank 2 left mapped.
    pub fn quiz_begin(&mut self, files: &mut Files) -> Option<()> {
        self.store_rng();
        let seed = self.mem.r32(DS, crate::random::SEED);
        self.mem.w32(DS, 0x12A8, seed);
        self.select_font(0x102E);
        self.sound_quiz_load(files, ANSWER_SOUND)?;
        self.mem.w8(DS, ASKED_COUNT, 0);
        self.mem.w16(DS, COUNTS, 0);
        self.mem.w8(DS, COUNTS + 2, 0);
        self.mem.w8(DS, COLOUR, 0xEE);
        self.ems_map(4);
        let frame = self.ng16(0x9030);
        self.mem.w16(DS, QUESTIONS, 0);
        self.mem.w16(DS, QUESTIONS + 2, frame);
        self.file_load(files, QUESTION_TXT, frame, 0)?;
        for (bank, name, at) in [(1, ROLLERS_DFA, ROLLERS), (2, ROLLERS_DFA + 0xD, ROLLERS + 2)] {
            self.ems_map(bank);
            let frame = self.ng16(0x9030);
            self.mem.w16(DS, at, frame);
            self.file_load(files, name, frame, 0)?;
        }
        Some(())
    }

    /// `class_quiz`'s next question (222C:00EA..00F6, 222C:020A..0231): picked
    /// (`quiz_pick`), found in bank 4 (`quiz_find`), its position kept at
    /// SHOWN; then its window: the cursor put back (541E), B7C4 bits 1-2
    /// and B7E4 set (no fades: no colours read), bank 6's frame the
    /// window's buffer (4D74), the window on DS:QUIZ_WINDOW opened
    /// (3159:000C up to its loop: STARTGAM.MNU, its rollers' and answers'
    /// regions) and its first routine (`quiz_scroll`, 222C:025A). Returns
    /// where its text is shown from (the frame's segment and the offset
    /// past its number), to the NUL the parse cut.
    pub fn quiz_question(&mut self, files: &mut Files) -> Option<(u16, u16)> {
        let q = self.quiz_pick()?;
        self.ems_map(4);
        let frame = self.ng16(0x9030);
        let di = self.quiz_find(q as u16)?;
        self.mem.w16(DS, SHOWN, di);
        self.mem.w16(DS, SHOWN + 2, frame);
        self.cursor_hidden();
        let f = self.ng8(SCREEN) | 6;
        self.mem.w8(DS, SCREEN, f);
        self.mem.w8(DS, crate::window::var::NO_FADE, 1);
        self.screen_bank(6);
        self.window_open(files, QUIZ_WINDOW, None)?;
        self.quiz_scroll()?;
        self.window_opened();
        Some((frame, di))
    }

    /// 222C:025A, the question's window's first routine: bank 3's frame
    /// the scroll's bitmap (its segment at BITMAP), its first 5000h bytes
    /// cleared (77C8), the scroll at its top (1298, 129A, 129C 0); the
    /// question's answers parsed (02D4..02DE, `quiz_answers`); its text
    /// made the scroll's strings (02E6..034B: a block from the heap's top
    /// of its length and 2 (9CD2), its segment at STRINGS: a LF, the text
    /// but its last byte, two LFs, each LF then a string's end, their
    /// count at LINES); the first lines drawn into the bitmap (034C), the
    /// rollers and the bitmap over the page (03B1, `quiz_scroll_drawn`),
    /// the screen dissolved in (1194:4FF7). None where the heap has no
    /// room for the strings (the original writes from 0:0).
    pub fn quiz_scroll(&mut self) -> Option<()> {
        self.ems_map(3);
        let bitmap = self.ng16(0x9030);
        self.mem.w16(DS, BITMAP, bitmap);
        for i in 0..0x5000u16 {
            self.mem.w8(bitmap, i, 0);
        }
        for at in [SCROLL_LINE, SCROLL_ROWS, SCROLL_FRAME] {
            self.mem.w16(DS, at, 0);
        }
        self.ems_map(4);
        let (di, es) = (self.ng16(SHOWN), self.ng16(SHOWN + 2));
        self.quiz_answers(es, di);
        self.ems_map(4);
        let mut len = 0u16;
        while self.mem.r8(es, di.wrapping_add(len)) != 0 {
            len = len.wrapping_add(1);
        }
        let size = len.wrapping_add(2);
        let seg = self.heap_alloc_high(size)?;
        if seg == 0 {
            return crate::giveup::not_ported("222C:02F4: no room for the question's strings: the original writes from 0:0");
        }
        self.mem.w16(DS, STRINGS, seg);
        self.mem.w8(seg, 0, 0x0A);
        for i in 0..size {
            self.mem.w8(seg, 1u16.wrapping_add(i), 0);
        }
        for i in 0..len {
            let b = self.mem.r8(es, di.wrapping_add(i));
            self.mem.w8(seg, 1u16.wrapping_add(i), b);
        }
        self.mem.w8(seg, len, 0x0A);
        self.mem.w8(seg, len.wrapping_add(1), 0x0A);
        let (mut p, mut lines) = (0u16, 0u16);
        loop {
            for _ in 0..0xFDE8u32 {
                let b = self.mem.r8(seg, p);
                p = p.wrapping_add(1);
                if b == 0x0A {
                    break;
                }
            }
            if self.mem.r8(seg, p.wrapping_sub(1)) == 0x0A {
                self.mem.w8(seg, p.wrapping_sub(1), 0);
                lines = lines.wrapping_add(1);
            }
            if p >= size {
                break;
            }
        }
        self.mem.w16(DS, LINES, lines);
        // 034C: as many lines as the bitmap shows (at most 8) drawn into it
        // (9E06 at x 14h, a line every 8 rows, the bitmap the draw page
        // meanwhile), then the last top line the scroll goes to.
        let shown = lines.min(8);
        self.mem.w16(DS, SCROLL_LAST, shown);
        let page = self.ng16(0xA900);
        self.ems_map(3);
        self.mem.w16(DS, 0xA900, bitmap);
        let mut si = 0u16;
        for i in 0..shown {
            self.page_text((seg, si), 0x14, i.wrapping_mul(8));
            while self.mem.r8(seg, si) != 0 {
                si = si.wrapping_add(1);
            }
            si = si.wrapping_add(1);
        }
        self.mem.w16(DS, 0xA900, page);
        let last = if lines >= 6 { lines.wrapping_sub(7) } else { 0 };
        self.mem.w16(DS, SCROLL_LAST, last);
        self.quiz_scroll_drawn();
        self.dissolve();
        Some(())
    }

    /// 222C:03B1: the rollers' frame (129C; 0..7 SCROLL01.DFA's in bank 1,
    /// 8..15 SCROLL02.DFA's in bank 2) drawn (5526, the front end's), then
    /// with bank 3 mapped the bitmap's 3C00h bytes from the scroll's row
    /// (129A) over the page from AA00h, its 0s see-through (where the page
    /// is the game's block).
    pub fn quiz_scroll_drawn(&mut self) {
        let frame = self.ng16(SCROLL_FRAME);
        self.ems_map(1);
        if frame >= 8 {
            self.ems_map(2);
        }
        self.ems_map(3);
        if self.page_is_block() {
            let (bitmap, page) = (self.ng16(BITMAP), self.ng16(0xA900));
            let row = self.ng16(SCROLL_ROWS);
            for i in 0..0x3C00u16 {
                let b = self.mem.r8(bitmap, row.wrapping_add(i));
                if b != 0 {
                    self.mem.w8(page, 0xAA00u16.wrapping_add(i), b);
                }
            }
        }
    }

    /// The question's answer `a` (0..2, `A`..`C`) given: its region (2 +
    /// `a`, the key's) found by the window's loop and its handler run
    /// (222C:066F: the answer at ANSWER, 3159:0006); the loop's end
    /// (3159:00D0): the window's last routine (222C:02C0: the strings'
    /// block freed), the masks freed and the keys flushed
    /// (`window_ended`); then 0233..0253 (B7E4 cleared, bank 4 mapped, the
    /// byte the parse cut put back, B7C4 bit 2 cleared) and 010B..013A:
    /// the answer counted (`quiz_count`), its sound played (23BA:014A),
    /// A902 bits 40h and 400h set and its kind's figure played (2294:0895,
    /// `movie_play`: WARRIOR.CEL, ROGUE.CEL or MAGE.CEL), colour 0 made
    /// its count's shade (0149: 929E..92A0, the DAC the front end's).
    /// None where the figure gives up.
    pub fn quiz_answer(&mut self, files: &mut Files, a: u8) -> Option<()> {
        let a = a.min(2) as u16;
        self.mem.w16(DS, REGION, 2 + a);
        self.mem.w16(DS, ANSWER, a);
        self.window_close();
        let seg = self.ng16(STRINGS);
        self.heap_free(seg);
        self.window_ended();
        self.mem.w8(DS, crate::window::var::NO_FADE, 0);
        self.ems_map(4);
        let (di, frame) = (self.ng16(SHOWN), self.ng16(SHOWN + 2));
        let mut p = di;
        for _ in 0..0xFDE8u32 {
            let b = self.mem.r8(frame, p);
            p = p.wrapping_add(1);
            if b == 0 {
                break;
            }
        }
        let cut = self.ng8(CUT);
        self.mem.w8(frame, p.wrapping_sub(1), cut);
        let f = self.ng8(SCREEN) & 0xFB;
        self.mem.w8(DS, SCREEN, f);
        self.quiz_count(a);
        let kind = self.ng8(KINDS.wrapping_add(a)) as u16;
        // 222C:011F: the answers' sound played (23BA:014A).
        self.sound_quiz_play();
        let v = self.ng16(0xA902) | 0x440;
        self.mem.w16(DS, 0xA902, v);
        let name = self.ng16(FIGURES.wrapping_add(kind.wrapping_mul(2)));
        self.movie_play(files, &mut crate::title::NoTitleHooks, name)?;
        let count = self.ng8(COUNTS.wrapping_add(kind)) as u16;
        let shade = self.ng8(SHADES.wrapping_add(count.wrapping_sub(1)));
        self.mem.w16(DS, 0x929E, 0);
        self.mem.w8(DS, 0x92A0, shade);
        Some(())
    }

    /// `class_quiz`'s end: the class (`quiz_class`, through bank 5, the
    /// bank mapped before restored); then (222C:016E) FONT_B ([102C],
    /// 9AE6), the answers' sound stopped and its block given back
    /// (23BA:01A4, `sound_quiz_free`) and bank [1030] mapped.
    pub fn quiz_end(&mut self, files: &mut Files) -> Option<u8> {
        let prev = self.ng16(0x8FA8) >> 2;
        self.ems_map(5);
        let class = self.quiz_class(files);
        self.ems_map(prev);
        let class = class?;
        self.select_font(0x102C);
        self.sound_quiz_free();
        let bank = self.ng16(0x1030);
        self.ems_map(bank);
        Some(class)
    }

    /// Creation from the player's choices, in the original's order:
    /// cleared, the class (by the quiz or the list), the name and sex,
    /// the province, the attributes rolled (and rolled again), the points
    /// spent (by the clicks, or one at a time over the attributes in turn
    /// until none are left or none can rise), kept, the face, the end.
    /// Returns the class. None where a step gives up.
    pub fn new_game(&mut self, files: &mut Files, hooks: &mut dyn NewGameHooks, ch: &Character) -> Option<u8> {
        self.newgame_reset();
        let class = match ch.class {
            ClassPick::Quiz(a) => self.class_quiz(files, &a)?,
            ClassPick::Listed(i) => self.ng8(0x2AEE + (i % 18) as u16),
        };
        self.class_chosen(class)?;
        self.name_and_sex(&ch.name, ch.sex);
        self.province_chosen(files, ch.province)?;
        for _ in 0..=ch.rerolls {
            self.creation_roll(files, hooks)?;
        }
        if ch.clicks.is_empty() {
            while self.ng8(POINTS) != 0 {
                let before = self.ng8(POINTS);
                for k in 1..=8 {
                    if self.ng8(POINTS) == 0 {
                        break;
                    }
                    self.creation_select(k);
                    let y = self.ng8(ARROWS) as u16;
                    self.creation_arrow(y);
                }
                if self.ng8(POINTS) == before {
                    break;
                }
            }
        } else {
            for &c in &ch.clicks {
                self.creation_click(c);
            }
        }
        self.creation_finish();
        self.mem.w8(DS, FACE_PICKED, ch.face);
        self.face_chosen();
        self.creation_end(files, hooks);
        Some(class)
    }

    /// 0000:13FC. The game's first date: the time 548D0h (5CC), the
    /// calendar's fields 5D0 0B7h, 5D2 8, 5D4 0, 5D6 0Ch, 5D8 0, A20 16h;
    /// then the time of day for the sky (0000:16F7, `sky_time`).
    pub fn game_date(&mut self) -> Option<()> {
        self.mem.w32(DS, 0x05CC, 0x548D0);
        for (at, v) in [(0x05D0, 0xB7), (0x05D2, 8), (0x05D4, 0), (0x05D6, 0xC), (0x05D8, 0)] {
            self.mem.w16(DS, at, v);
        }
        self.mem.w8(DS, 0x0A20, 0x16);
        self.sky_time()
    }

    /// 21EE:016C. The game set up, once (3B07 bit 0): A902 bit 10h, the
    /// resources loaded; 280h bytes allocated (0000:51E2), their far
    /// pointer at 1AB5, and the same segment with offsets 100h and 180h
    /// at 1004 and 1008 (EAX keeps the segment the allocator returned in
    /// its high word); 20F0h bytes allocated and cleared (0000:5338), the
    /// segment at 1042 (the people's records); the table at 1B94 (65h
    /// words, n × 28F5Ch >> 16); the first date (`game_date`); the hands'
    /// graphics; the zones' weather (1194:2FB3, `weather_roll`); the
    /// spells; 5C8 bit 2000h; the level's flats; CD00 FFFFh. None where an
    /// allocation fails or the date's division overflows.
    pub fn game_setup(&mut self, hooks: &mut dyn NewGameHooks) -> Option<()> {
        if self.ng8(0x3B07) & 1 != 0 {
            return Some(());
        }
        let v = self.ng8(0x3B07) | 1;
        self.mem.w8(DS, 0x3B07, v);
        let v = self.ng16(0xA902) | 0x10;
        self.mem.w16(DS, 0xA902, v);
        hooks.resources(self);
        let (seg, off) = self.heap_alloc_bytes(0x280)?;
        let hi = (seg as u32) << 16;
        self.mem.w32(DS, 0x1AB5, hi | off as u32);
        self.mem.w32(DS, 0x1004, hi | 0x100);
        self.mem.w32(DS, 0x1008, hi | 0x180);
        let (seg, off) = self.heap_alloc_bytes(0x20F0)?;
        for i in 0..0x20F0u16 {
            self.mem.w8(seg, off.wrapping_add(i), 0);
        }
        self.mem.w16(DS, 0x1042, seg);
        for i in 0..0x65u32 {
            self.mem.w16(DS, 0x1B94 + 2 * i as u16, (i.wrapping_mul(0x28F5C) >> 16) as u16);
        }
        self.game_date()?;
        hooks.hands(self);
        self.weather_roll()?;
        hooks.spells(self);
        let v = self.ng16(0x05C8) | 0x2000;
        self.mem.w16(DS, 0x05C8, v);
        hooks.level_flats(self);
        self.mem.w16(DS, 0xCD00, 0xFFFF);
        Some(())
    }

    /// 0000:0C66..0C94. The game started in the start dungeon: its level
    /// hooks (0000:0B4F: the way out 44BDh), its file's name ("start.mif",
    /// 4BD1) to 0A36 and the place's ("Imperial Dungeons", 4BDB) to 0A22,
    /// indoors (C398 bits 8000h, 4 clear), the area the start dungeon (5E6
    /// 2), BB0A 0, 5E4 bit 0. Its run follows (0997: `start_run`).
    pub fn game_start(&mut self) {
        self.start_level_hooks();
        for (from, to) in [(0x4BD1u16, 0x0A36u16), (0x4BDB, 0x0A22)] {
            let mut i = 0u16;
            loop {
                let b = self.ng8(from.wrapping_add(i));
                self.mem.w8(DS, to.wrapping_add(i), b);
                i = i.wrapping_add(1);
                if b == 0 {
                    break;
                }
            }
        }
        let v = self.ng16(0xC398) & 0x7FFB;
        self.mem.w16(DS, 0xC398, v);
        self.mem.w8(DS, 0x05E6, 2);
        self.mem.w8(DS, 0xBB0A, 0);
        let v = self.ng8(0x05E4) | 1;
        self.mem.w8(DS, 0x05E4, v);
    }

    /// 1194:257B..25B7. Creation's end: 5C8 bit 10h clear; the parchment's
    /// resource dropped (0000:9326 with [1038]); A902 bit 0 clear; the
    /// screen faded out (55B6); the song stopped; 12BA cleared, 5B1 bit
    /// 4000h set, the time 2BCh (5AD); the sheet's screens kept (158F);
    /// then 0000:5D70 and 2294:11F2. (The drop, the fade and the keeping
    /// are hooks, by default the game's own.)
    pub fn creation_end(&mut self, files: &mut Files, hooks: &mut dyn NewGameHooks) {
        let v = self.ng16(0x05C8) & 0xFFEF;
        self.mem.w16(DS, 0x05C8, v);
        hooks.parchment_drop(self);
        let v = self.ng16(0xA902) & 0xFFFE;
        self.mem.w16(DS, 0xA902, v);
        hooks.faded(self);
        self.music_stop();
        self.mem.w16(DS, 0x12BA, 0);
        let v = self.ng16(0x05B1) | 0x4000;
        self.mem.w16(DS, 0x05B1, v);
        self.mem.w32(DS, 0x05AD, 0x2BC);
        hooks.sheet_keep(self, files);
        hooks.creation_done(self);
        hooks.game_screen(self);
    }
}
