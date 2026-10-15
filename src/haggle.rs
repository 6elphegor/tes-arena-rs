//! Haggling over a price (1194:8020), for buying (modes 1 and 2, and 3
//! a tavern's room: the buying path with TAVERN.DAT's lines), selling
//! (mode 4) and the main quest's informant (modes 5 and 6, his price for
//! the way: 0FEE).
//!
//! The shopkeeper starts at the asking price and has a limit: for a
//! purchase the lowest price they will take, for a sale the most they
//! will pay. Each round the player accepts the current price, walks away
//! or types an offer. An offer at or past the asking price is taken at
//! once; one past the limit is refused, which costs one of the rounds
//! (8 - quality / 52); otherwise the shopkeeper may take it (buying
//! only, on a roll out of 256) or counters at about the middle of the
//! two prices, moved by the shop's mood.
//!
//! Every message draws one random number (1194:83FD picks one of three
//! lines of the shop's dialogue file). The line is copied to LINE, its
//! codes made a format there (84F1: %n the name at DS:[65AB], %c entry
//! DL - 1 of the list at 2A62, %mm the price, which also becomes the
//! result, %i the text at DS:[1376]) and formatted into SHOWN (0000:8D14);
//! its line breaks become spaces, the colour 137E is 3Ch, and it is shown
//! (1194:49FF, `HaggleIo::say`). The player's answers come from a
//! `HaggleIo`.
//!
//! With the informant (modes 5, 6) the price is paid at once from the
//! player's gold (05C2), before the confirmation, which always agrees;
//! too little gold is no deal. His price follows the haggle (0FEE), and
//! 0FE6 bit 0 says a price was named (83E4). His lines are line
//! step * 15 + kind * 3 (+ a d3 but for the confirmation) of the
//! dialogue file, the step the main quest's (0FDD), put through the
//! text's substitutions (2DA2:000A) rather than the %mm scan.
//!
//! Every counter-offer's menu (8575) sets 0FE6 bit 0, and every message
//! (862E) the text colour 9288 to 3Ch, in all modes.
//!
//! Bugs kept: selling never reaches its acceptance roll (`and ax,ax`
//! clears carry before a carry test); accepting the first price returns
//! DGROUP 65B8 as left by an earlier haggle; an asking price equal to the
//! limit divides by zero; the informant is paid before the player
//! confirms, and a refusal then keeps the gold.

use crate::bsa::Files;
use crate::dice::roll;
use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Over-65000 flag (prices are halved, offers too), rounds made, the
    /// current price, the last offer, the first and highest offer, the
    /// limit, rounds left, the mood term and the result.
    pub const HALVED: u16 = 0x65A5;
    pub const ROUND: u16 = 0x65A2;
    pub const PRICE: u16 = 0x65A3;
    pub const OFFER_SEEN: u16 = 0x65A6;
    pub const OFFER: u16 = 0x65AF;
    pub const FIRST_OFFER: u16 = 0x65B4;
    pub const FIRST_OFFER_2: u16 = 0x65B6;
    pub const LIMIT: u16 = 0x65AD;
    pub const ROUNDS_LEFT: u16 = 0x65B1;
    pub const MOOD: u16 = 0x65B2;
    pub const RESULT: u16 = 0x65B8;
    /// Inputs kept: personality (0..100), the caller's DI and DL, mode.
    pub const PERSONALITY: u16 = 0x65AA;
    pub const NAME: u16 = 0x65AB;
    pub const DL: u16 = 0x6598;
    pub const MODE: u16 = 0x65A1;
    /// The shop's quality and mood bytes.
    pub const QUALITY: u16 = 0x13CA;
    pub const MOOD_BYTE: u16 = 0x13CB;
    /// Typed offer (6 bytes) and its value (dword), and the price agreed.
    pub const TYPED: u16 = 0x6599;
    pub const TYPED_VALUE: u16 = 0x1044;
    pub const AGREED: u16 = 0x1391;
    /// The player's gold, the informant's flags and price, the main
    /// quest's step (his lines), the text colour.
    pub const GOLD: u16 = 0x05C2;
    pub const QUEST_FLAGS: u16 = 0x0FE6;
    pub const QUEST_PRICE: u16 = 0x0FEE;
    pub const QUEST_STEP: u16 = 0x0FDD;
    pub const COLOUR: u16 = 0x9288;
    /// The line read (and, for the informant, substituted), the line
    /// shown, and its colour.
    pub const LINE: u16 = 0x929E;
    pub const SHOWN: u16 = 0x987A;
    pub const SHOWN_COLOUR: u16 = 0x137E;
    /// What the input routine (1194:4E88) is asked for: 0 a key, 1 a
    /// menu choice, 2 typed text.
    pub const INPUT_KIND: u16 = 0x13D3;
    /// The item's name (%i) and the list %c picks from.
    pub const ITEM: u16 = 0x1376;
    pub const CATEGORIES: u16 = 0x2A62;
}
use var::*;

/// Dialogue files by mode (1..6), loaded into the buffer segment.
const DIALOGUE: [u16; 6] = [0x65EF, 0x65F9, 0x6605, 0x6610, 0x661C, 0x6629];

/// What the game asks the player. `menu` answers a choice (0 accept,
/// 2 leave, anything else haggle; also 0 to confirm a deal), `text` an
/// offer typed (up to 6 characters; empty gives up), `key` a keypress
/// after a message. None stops the haggle (the port returns None).
pub trait HaggleIo {
    fn menu(&mut self) -> Option<u16>;
    fn text(&mut self) -> Option<Vec<u8>>;
    fn key(&mut self) -> Option<()>;
    /// 2DA2:000A: the informant's line at DS:`di` has its codes
    /// substituted (the text port, `Machine::expand`).
    fn expand(&mut self, m: &mut Machine, files: &mut Files, di: u16) -> Option<()> {
        m.expand(files, &mut crate::text::NoTextHooks, DS, di).map(|_| ())
    }
    /// 1194:49FF: the line at DS:`si` (SHOWN, in colour 137E) shown, before
    /// the menu, offer or key it goes with.
    fn say(&mut self, _m: &mut Machine, _si: u16) {}
}

/// A player who walks away: the menus left (2), no offer typed, keys
/// pressed. For where no one answers (a front end's hooks answer for
/// the player).
pub struct Leave;

impl HaggleIo for Leave {
    fn menu(&mut self) -> Option<u16> {
        Some(2)
    }
    fn text(&mut self) -> Option<Vec<u8>> {
        Some(Vec::new())
    }
    fn key(&mut self) -> Option<()> {
        Some(())
    }
}

/// The result: a deal at a price (EAX with carry clear), or none (carry
/// set; EAX means nothing then).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haggle {
    Deal(u32),
    NoDeal,
}

impl Machine {
    /// 1194:8020. Haggles over `price` with the player's personality
    /// `per` (0..100, as 1194:5B07 gives it), the shop's `quality` and
    /// `mood` bytes, `mode` 1, 2 (buying), 3 (a tavern's room), 4
    /// (selling), 5 or 6 (the informant); `di` and `dl` are kept for the
    /// dialogue. The dialogue file for the mode is loaded into the buffer
    /// segment, as the original does. None for another mode, where the
    /// original faults, or where the player stops answering.
    #[allow(clippy::too_many_arguments)]
    pub fn haggle(
        &mut self,
        files: &mut Files,
        io: &mut impl HaggleIo,
        price: u32,
        per: u16,
        quality: u8,
        mood: u8,
        dl: u8,
        mode: u8,
        di: u16,
    ) -> Option<Haggle> {
        if !matches!(mode, 1..=6) {
            return None;
        }
        self.mem.w8(DS, HALVED, 0);
        self.mem.w8(DS, ROUND, 0);
        let mut price = price;
        if price > 65000 {
            self.mem.w8(DS, HALVED, 1);
            price >>= 1;
        }
        self.mem.w16(DS, PRICE, price as u16);
        self.mem.w8(DS, QUALITY, quality);
        self.mem.w8(DS, MOOD_BYTE, mood);
        self.mem.w8(DS, PERSONALITY, per as u8);
        self.mem.w16(DS, NAME, di);
        self.mem.w8(DS, DL, dl);
        self.mem.w8(DS, MODE, mode);
        // 1194:8665: the mode's dialogue file into the buffer segment.
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        self.file_load(files, DIALOGUE[mode as usize - 1], buf, 0);
        self.mem.w8(DS, ROUNDS_LEFT, 8 - quality / 0x34);
        let p = self.mem.r16(DS, PRICE);
        let f = ((0x100 - quality as u16) >> 1).wrapping_add(0x19).wrapping_add((per as u8 as u16) >> 1) >> 1;
        let mut t = (((f as u32) * (p as u32)) >> 8) as u16;
        if (t as i16) >= 0 && mode != 4 {
            t = t.wrapping_neg();
        }
        self.mem.w16(DS, LIMIT, t.wrapping_add(p));
        self.mem.w16(DS, MOOD, ((mood as i16 - 0x80) >> 1) as u16);

        // The opening menu: 0 accept, 2 leave, else haggle.
        match self.haggle_menu(files, io, 0)? {
            0 => return self.haggle_accept_current(files, io),
            2 => return self.haggle_leave(files, io),
            _ => {}
        }
        loop {
            // 80E9: one round.
            let r = self.mem.r8(DS, ROUND).wrapping_add(1);
            self.mem.w8(DS, ROUND, r);
            let offer = loop {
                for i in 0..6 {
                    self.mem.w8(DS, TYPED + i, 0);
                }
                // 80F7: the prompt (65CA) in colour 3, typed text asked.
                self.mem.w8(DS, SHOWN_COLOUR, 3);
                self.mem.w8(DS, INPUT_KIND, 2);
                let typed = io.text()?;
                for (i, &c) in typed.iter().take(6).enumerate() {
                    self.mem.w8(DS, TYPED + i as u16, c);
                }
                if self.mem.r8(DS, TYPED) == 0 {
                    return Some(Haggle::NoDeal);
                }
                // 1194:485C: digits only, at least one.
                let text = self.mem.cstr(DS, TYPED).to_vec();
                if text.iter().all(u8::is_ascii_digit) {
                    break text;
                }
            };
            // 0000:52F2: 32-bit decimal.
            let mut value = offer.iter().fold(0u32, |v, &c| v.wrapping_mul(10).wrapping_add((c - b'0') as u32));
            self.mem.w32(DS, TYPED_VALUE, value);
            if self.mem.r8(DS, HALVED) == 1 {
                value = (value >> 1) & 0xFFFF;
            }
            let ax = value as u16;
            self.mem.w16(DS, OFFER, ax);
            if self.mem.r8(DS, ROUND) <= 1 {
                self.mem.w16(DS, FIRST_OFFER, ax);
                self.mem.w16(DS, FIRST_OFFER_2, ax);
            }
            let price = self.mem.r16(DS, PRICE);
            let limit = self.mem.r16(DS, LIMIT);
            let per_d = (self.mem.r8(DS, PERSONALITY) as u16).wrapping_sub(0x80);
            let q_d = (self.mem.r8(DS, QUALITY) as u16).wrapping_sub(0x80);
            // Some(true): counter; Some(false): refuse; with the accept
            // cases returning directly.
            let counter = if mode == 4 {
                if ax <= price {
                    return self.haggle_deal(files, io, ax);
                }
                if ax > self.mem.r16(DS, FIRST_OFFER_2) || ax > limit {
                    false
                } else {
                    self.mem.w16(DS, OFFER_SEEN, ax);
                    // The shifted difference is divided with DX cleared.
                    let d = (limit.wrapping_sub(ax)) << 8;
                    let div = price.wrapping_sub(limit);
                    if div == 0 {
                        return None;
                    }
                    let _ = q_d.wrapping_sub(d / div).wrapping_add(per_d);
                    // 81F0: AND clears carry, so JAE is always taken.
                    true
                }
            } else {
                self.mem.w16(DS, OFFER_SEEN, ax);
                if ax >= price {
                    self.mem.w16(DS, PRICE, ax);
                    return self.haggle_accept_current(files, io);
                }
                if ax < limit {
                    false
                } else {
                    let v = ((ax.wrapping_sub(limit) as i16 as i32) << 8) as u32;
                    let div = price.wrapping_sub(limit) as u32;
                    if div == 0 || v / div > 0xFFFF {
                        return None;
                    }
                    let a = ((v / div) as u16).wrapping_sub(q_d).wrapping_add(per_d);
                    // 820F
                    if (a as i16) <= 0 {
                        true
                    } else if a >= 0x100 {
                        return self.haggle_deal(files, io, ax);
                    } else {
                        let r = roll(&mut self.rng, 0x100)?;
                        if r <= a {
                            return self.haggle_deal(files, io, ax);
                        }
                        true
                    }
                }
            };
            if !counter {
                // 82AC: refused.
                let left = self.mem.r8(DS, ROUNDS_LEFT).wrapping_sub(1);
                self.mem.w8(DS, ROUNDS_LEFT, left);
                if left == 0 {
                    self.haggle_message(files, io, 3)?;
                    return self.haggle_leave(files, io);
                }
                self.haggle_message(files, io, 2)?;
                continue;
            }
            // 822F: the counter-offer.
            let price = self.mem.r16(DS, PRICE);
            let offer = self.mem.r16(DS, OFFER);
            let sum = price as u32 + offer as u32;
            let bx = (sum >> 1) as u16;
            let half = (bx >> 1) as i16 as i32;
            let mood_term = ((half * self.mem.r16(DS, MOOD) as i16 as i32) >> 7) as u16;
            let next = if mode == 4 {
                let a = bx.wrapping_sub(1);
                if a > offer {
                    return self.haggle_deal(files, io, a);
                }
                let a = if a == offer { a } else { a.wrapping_add(1) };
                a.min(self.mem.r16(DS, FIRST_OFFER_2))
            } else {
                let a = if bx > self.mem.r16(DS, LIMIT) { bx } else { mood_term.wrapping_add(bx) };
                if a <= offer {
                    return self.haggle_deal(files, io, bx);
                }
                bx.max(self.mem.r16(DS, FIRST_OFFER))
            };
            self.mem.w16(DS, PRICE, next);
            let left = self.mem.r8(DS, ROUNDS_LEFT).wrapping_sub(1);
            self.mem.w8(DS, ROUNDS_LEFT, left);
            if left == 0 {
                self.haggle_message(files, io, 3)?;
                return self.haggle_leave(files, io);
            }
            if matches!(mode, 5 | 6) {
                self.mem.w16(DS, QUEST_PRICE, next);
            }
            // 8575: the counter-offer's menu marks a price named.
            let f = self.mem.r8(DS, QUEST_FLAGS) | 1;
            self.mem.w8(DS, QUEST_FLAGS, f);
            match self.haggle_menu(files, io, 4)? {
                0 => return self.haggle_accept_current(files, io),
                2 => return self.haggle_leave(files, io),
                _ => {}
            }
        }
    }

    /// 1194:8349: the current price is accepted; the first round's deal
    /// returns RESULT as it stands. The informant (modes 5, 6) is paid
    /// first: with too little gold (a message, 4D31) the price is AGREED,
    /// the round counted as 1 and no deal made (83E4 marks the price).
    fn haggle_accept_current(&mut self, files: &mut Files, io: &mut impl HaggleIo) -> Option<Haggle> {
        let price = self.mem.r16(DS, PRICE);
        if matches!(self.mem.r8(DS, MODE), 5 | 6) && !self.haggle_pay(price) {
            self.mem.w32(DS, AGREED, price as u32);
            self.mem.w8(DS, ROUND, 1);
            self.haggle_named();
            return Some(Haggle::NoDeal);
        }
        if self.mem.r8(DS, ROUND) == 0 {
            self.mem.w32(DS, AGREED, price as u32);
        }
        if self.haggle_confirm(files, io)? {
            Some(Haggle::Deal(self.mem.r32(DS, RESULT)))
        } else {
            Some(Haggle::NoDeal)
        }
    }

    /// 1194:82E2: a deal at `ax`, if the player confirms. The informant
    /// takes his price (0FEE) and is paid first, as above.
    fn haggle_deal(&mut self, files: &mut Files, io: &mut impl HaggleIo, ax: u16) -> Option<Haggle> {
        self.mem.w32(DS, AGREED, ax as u32);
        self.mem.w32(DS, RESULT, ax as u32);
        self.mem.w16(DS, PRICE, ax);
        if matches!(self.mem.r8(DS, MODE), 5 | 6) {
            self.mem.w16(DS, QUEST_PRICE, ax);
            if !self.haggle_pay(ax) {
                self.mem.w8(DS, ROUND, 1);
                self.haggle_named();
                return Some(Haggle::NoDeal);
            }
        }
        if self.haggle_confirm(files, io)? {
            Some(Haggle::Deal(ax as u32))
        } else {
            Some(Haggle::NoDeal)
        }
    }

    /// The gold (a dword) paid if there is enough.
    fn haggle_pay(&mut self, price: u16) -> bool {
        let gold = self.mem.r32(DS, GOLD);
        if gold < price as u32 {
            return false;
        }
        self.mem.w32(DS, GOLD, gold - price as u32);
        true
    }

    /// 1194:83E4: once a round was made, 0FE6 bit 0 set and the price
    /// kept at 0FEE; else bit 0 cleared.
    fn haggle_named(&mut self) {
        let f = self.mem.r8(DS, QUEST_FLAGS);
        if self.mem.r8(DS, ROUND) != 0 {
            self.mem.w8(DS, QUEST_FLAGS, f | 1);
            let p = self.mem.r16(DS, PRICE);
            self.mem.w16(DS, QUEST_PRICE, p);
        } else {
            self.mem.w8(DS, QUEST_FLAGS, f & 0xFE);
        }
    }

    /// 1194:82BB: no deal; the informant (modes 5, 6) has his price
    /// marked (83E4) and a last word (85D6).
    fn haggle_leave(&mut self, files: &mut Files, io: &mut impl HaggleIo) -> Option<Haggle> {
        if matches!(self.mem.r8(DS, MODE), 5 | 6) {
            self.haggle_named();
            self.haggle_message(files, io, 3)?;
        }
        Some(Haggle::NoDeal)
    }

    /// 1194:858C: a message, then a menu choice. At a price of 1, a
    /// choice of 1 counts as 2 (leave).
    fn haggle_menu(&mut self, files: &mut Files, io: &mut impl HaggleIo, kind: u8) -> Option<u16> {
        let one = self.mem.r16(DS, PRICE) == 1;
        self.haggle_say(files, io, kind)?;
        self.haggle_ask(1);
        let c = io.menu()?;
        Some(if one && c == 1 { 2 } else { c })
    }

    /// 1194:85DE: the confirmation; true for a choice of 0. The
    /// informant's (modes 5, 6) asks for a key (13D3 left 0) and always
    /// agrees.
    fn haggle_confirm(&mut self, files: &mut Files, io: &mut impl HaggleIo) -> Option<bool> {
        let informant = matches!(self.mem.r8(DS, MODE), 5 | 6);
        self.mem.w8(DS, INPUT_KIND, if informant { 0 } else { 1 });
        self.haggle_say(files, io, 1)?;
        self.mem.w8(DS, SHOWN_COLOUR, 3);
        if informant {
            io.key()?;
            return Some(true);
        }
        Some(io.menu()? == 0)
    }

    /// 1194:862E: a message (kind 2 a refusal, 3 the last one) in colour
    /// 3Ch and a keypress.
    fn haggle_message(&mut self, files: &mut Files, io: &mut impl HaggleIo, kind: u8) -> Option<()> {
        self.mem.w8(DS, COLOUR, 0x3C);
        self.haggle_say(files, io, kind)?;
        self.haggle_ask(0);
        io.key()
    }

    /// The prompt after a line (1194:4E54 in colour 3, 137E) and what is
    /// asked of the input routine (13D3).
    fn haggle_ask(&mut self, kind: u8) {
        self.mem.w8(DS, SHOWN_COLOUR, 3);
        self.mem.w8(DS, INPUT_KIND, kind);
    }

    /// 1194:83FD, 84F1 and 84C3 for modes 1 to 4: message `kind` (0 the
    /// opening, 1 the confirmation, 2 and 3 refusals, 4 a counter-offer)
    /// is line kind * 15 + tier * 3 + range(0, 3) of the dialogue file in
    /// the buffer segment, where tier is quality / 20 rounded up, less 1
    /// (all in 8 bits: a quality of 0 wraps). It is copied to LINE, made a
    /// format (84F1, scanning back from its end to the first zero byte,
    /// which may lie before LINE: %n, %c, %mm, %i as the module says; each
    /// "%mm" puts the current price, doubled back if halved, in RESULT),
    /// formatted into SHOWN and shown. None where a code lies before
    /// LINE: its argument is pushed too, and the formatter (which reads
    /// the stack's words as each conversion asks) would take it for the
    /// line's first; not modelled. The bytes before LINE are DGROUP's
    /// variables, not text.
    fn haggle_say(&mut self, files: &mut Files, io: &mut impl HaggleIo, kind: u8) -> Option<()> {
        if matches!(self.mem.r8(DS, MODE), 5 | 6) {
            return self.haggle_say_informant(files, io, kind);
        }
        let q = self.mem.r8(DS, QUALITY);
        let tier = (q / 20 + (q % 20 != 0) as u8).wrapping_sub(1);
        let base = kind.wrapping_mul(15) as u16 + tier.wrapping_mul(3) as u16;
        let r = self.rng.range(0, 3)?;
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let si = self.skip_strings(buf, 0, base.wrapping_add(r));
        self.haggle_copy(buf, si, LINE);
        // 84F1: from the end back to a zero byte, each code rewritten for
        // the formatter, its argument pushed (so the first in the line is
        // the formatter's first).
        let mut si = LINE;
        while self.mem.r8(DS, si) != 0 {
            si = si.wrapping_add(1);
        }
        si = si.wrapping_sub(1);
        let mut args = Vec::new();
        let mut early = false;
        while self.mem.r8(DS, si) != 0 {
            if self.mem.r8(DS, si) == b'%' {
                let pushed = args.len();
                let next = self.mem.r8(DS, si.wrapping_add(1));
                if next == b'n' {
                    self.mem.w8(DS, si.wrapping_add(1), b's');
                    args.push(crate::format::Arg::Str(DS, self.mem.r16(DS, NAME)));
                } else if next == b'c' {
                    self.mem.w8(DS, si.wrapping_add(1), b's');
                    let n = self.mem.r8(DS, DL).wrapping_sub(1) as u16;
                    args.push(crate::format::Arg::Str(DS, self.skip_strings(DS, CATEGORIES, n)));
                } else if self.mem.r16(DS, si.wrapping_add(1)) == 0x6D6D {
                    self.mem.w16(DS, si.wrapping_add(1), 0x756C);
                    let mut price = self.mem.r16(DS, PRICE) as u32;
                    if self.mem.r8(DS, HALVED) == 1 {
                        price = price.wrapping_add(price);
                    }
                    args.push(crate::format::Arg::Long(price));
                    self.mem.w32(DS, RESULT, price);
                } else if next == b'i' {
                    self.mem.w8(DS, si.wrapping_add(1), b's');
                    args.push(crate::format::Arg::Str(DS, self.mem.r16(DS, ITEM)));
                }
                early |= args.len() != pushed && si < LINE;
            }
            si = si.wrapping_sub(1);
        }
        if early {
            return None;
        }
        args.reverse();
        self.sprintf((DS, SHOWN), (DS, LINE), &args);
        self.haggle_show(io);
        Some(())
    }

    /// 0000:9CA0: the string at seg:`si` copied to DS:`di` with its NUL.
    fn haggle_copy(&mut self, seg: u16, mut si: u16, mut di: u16) {
        loop {
            let c = self.mem.r8(seg, si);
            self.mem.w8(DS, di, c);
            if c == 0 {
                break;
            }
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
        }
    }

    /// 1194:84C3: SHOWN's line breaks (CR, LF) made spaces, the colour
    /// 3Ch, and the line shown (1194:49FF).
    fn haggle_show(&mut self, io: &mut impl HaggleIo) {
        let mut si = SHOWN;
        loop {
            let c = self.mem.r8(DS, si);
            if c == 0 {
                break;
            }
            if c == b'\n' || c == b'\r' {
                self.mem.w8(DS, si, b' ');
            }
            si = si.wrapping_add(1);
        }
        self.mem.w8(DS, SHOWN_COLOUR, 0x3C);
        io.say(self, SHOWN);
    }

    /// 1194:83FD..8467 and 84AA for the informant (modes 5, 6): line
    /// step * 15 (8 bits) + kind * 3 (8 bits) of the dialogue file, plus
    /// a d3 but for the confirmation (kind 1), copied to LINE and its
    /// codes substituted (2DA2:000A, `HaggleIo::expand`), copied to SHOWN
    /// and shown (84C3).
    fn haggle_say_informant(&mut self, files: &mut Files, io: &mut impl HaggleIo, kind: u8) -> Option<()> {
        let step = (self.mem.r8(DS, QUEST_STEP) as u16 * 15) & 0xFF;
        let mut line = step.wrapping_add((kind as u16 * 3) & 0xFF);
        if kind != 1 {
            line = line.wrapping_add(self.rng.range(0, 3)?);
        }
        let buf = self.mem.r16(DS, crate::mif::var::BUFFER_SEG);
        let mut si = self.skip_strings(buf, 0, line);
        let mut di = LINE;
        loop {
            let c = self.mem.r8(buf, si);
            self.mem.w8(DS, di, c);
            if c == 0 {
                break;
            }
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
        }
        io.expand(self, files, LINE)?;
        self.haggle_copy(DS, LINE, SHOWN);
        self.haggle_show(io);
        Some(())
    }
}
