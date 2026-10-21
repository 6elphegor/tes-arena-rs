//! The fixed mode: the original's bugs the port keeps (docs/kept-bugs.md)
//! corrected, each behind its own bit of `Machine::fixes`.
//!
//! The port is faithful by default: a machine starts with `Fixes::NONE`,
//! and every test, vector and generator runs that way, byte for byte as
//! the original. A front end that wants the game as it was meant to play
//! sets `Fixes::ALL` (the view does, unless `--original`). The bit for a
//! bug is its number in docs/kept-bugs.md, so a fix is looked up where it
//! is made with `Machine::fixed(fixes::NAME)` and nowhere else: a
//! machine's copies (`Clone`, `clone_from`) carry it, a saved game
//! doesn't (it is how this program plays, not the game's state).
//!
//! A fix keeps the faithful code as it was, the comment on the original's
//! bug beside it, and says what fixed mode does instead and why that is
//! what the code meant; its test (named `fixed_...`, next to the routine's
//! faithful tests) shows the corrected behaviour and fails with the bit
//! off.

/// Which of the kept bugs are fixed: bit n for docs/kept-bugs.md's #n.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fixes(u128);

impl Fixes {
    /// The original's behaviour, bugs included (the default).
    pub const NONE: Fixes = Fixes(0);
    /// Every fix there is, and every one added later.
    pub const ALL: Fixes = Fixes(!0);

    /// Only bug `n` fixed.
    pub const fn only(n: u8) -> Fixes {
        Fixes(1 << n)
    }

    /// These fixes and bug `n`'s.
    pub const fn with(self, n: u8) -> Fixes {
        Fixes(self.0 | 1 << n)
    }

    /// These fixes but bug `n`'s.
    pub const fn without(self, n: u8) -> Fixes {
        Fixes(self.0 & !(1 << n))
    }

    /// Whether bug `n` is fixed.
    pub const fn has(self, n: u8) -> bool {
        self.0 >> n & 1 != 0
    }
}

// The bugs fixed so far, by their numbers in docs/kept-bugs.md, in order.

/// #1: %en outside an equipment store returns into 2DA2 (2DA2:05DC).
pub const EN_OUTSIDE_STORE: u8 = 1;
/// #2: the temple's "Cure all" pays its index and crashes (1194:C36E).
pub const CURE_ALL: u8 = 2;
/// #3: an automap note's hit test overflows its divide (0000:343C).
pub const NOTE_DIVIDE: u8 = 3;
/// #5: a pile made in the wild reads its tables past their end (2433:0358).
pub const WILD_PILES: u8 = 5;
/// #6: a guild stock pick past the list's end faults (1194:5ED1).
pub const GUILD_PICK_PAST: u8 = 6;
/// #7: a chest's failed lockpicking looked up under the player's square.
pub const CHEST_RETRIED: u8 = 7;
/// #8: a stolen store record stays for sale, an unenchanted one's resale
/// price is its type byte (1194:FCC6, FC78).
pub const STORE_STOLEN: u8 = 8;
/// #9: from level 11 on every crime is seen (0000:3F50).
pub const CRIME_SEEN: u8 = 9;
/// #10: selling never reaches the merchant's acceptance roll (1194:81F0).
pub const SELLING_ROLL: u8 = 10;
/// #11: the guild's potions and spells haggled with 44h, 44h (1194:B51A).
pub const GUILD_HAGGLE: u8 = 11;
/// #12: a donation within a day of the last blesses nothing (1194:C58B).
pub const SECOND_DONATION: u8 = 12;
/// #13: taking off an item wraps a drained attribute to its maximum.
pub const UNEQUIP_WRAP: u8 = 13;
/// #14: the first price accepted returns an earlier haggle's (1194:8349).
pub const FIRST_PRICE: u8 = 14;
/// #15: a room sneaked into rests for the end time's low word in hours.
pub const SNEAK_HOURS: u8 = 15;
/// #16: the pile picked is found ignoring the kind of place; a corpse's
/// segment written at 4BF4 (2433:0883, 0748).
pub const LOOT_PICK: u8 = 16;
/// #17: fast travel's terrain read 12 bytes early, the header (1194:3290).
pub const TRAVEL_HEADER: u8 = 17;
/// #20: a caster's "Spells" without stock runs the magic items' (1194:AA8C).
pub const GUILD_SPELLS_PICK: u8 = 20;
/// #32: the automap's groups of four squares read back reversed (23F1:037A).
pub const AUTOMAP_ORDER: u8 = 32;
/// #33: the logbook is never trimmed, its new path built twice (27E3:0169).
pub const LOG_TRIM: u8 = 33;
/// #34: the logbook's Exit takes two clicks off the last page, and a
/// click stays held (27E3:0137, 01FC).
pub const LOGBOOK_CLICKS: u8 = 34;
/// #35: the logbook's line count written into the frame (1194:4E54).
pub const LOG_COUNT: u8 = 35;
/// #36: a text field's click counts the first character's widths; a
/// character typed inside the text leaves no NUL (0000:A1B9, 9EE9).
pub const EDIT_TEXT: u8 = 36;
/// #37: a full key ring marks the last key down; left Shift marks the
/// right down (0000:718A).
pub const KEY_RING: u8 = 37;
/// #43: piles' odd halved gold dropped, magic types forced by a scratch
/// register, the level's limit read from the frame (2433:0358, 04C8, 0101).
pub const PILE_QUIRKS: u8 = 43;
/// #61: the loot window puts back a bank it never saved (2433:0583).
pub const LOOT_BANK: u8 = 61;
