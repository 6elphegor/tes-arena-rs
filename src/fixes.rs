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

/// #7: a chest's failed lockpicking looked up under the player's square.
pub const CHEST_RETRIED: u8 = 7;
/// #9: from level 11 on every crime is seen (0000:3F50).
pub const CRIME_SEEN: u8 = 9;
/// #10: selling never reaches the merchant's acceptance roll (1194:81F0).
pub const SELLING_ROLL: u8 = 10;
/// #13: taking off an item wraps a drained attribute to its maximum.
pub const UNEQUIP_WRAP: u8 = 13;
/// #15: a room sneaked into rests for the end time's low word in hours.
pub const SNEAK_HOURS: u8 = 15;
/// #18: instant death kills on a roll at or above its chance (2CDD:03C1).
pub const INSTANT_DEATH: u8 = 18;
/// #21: "Spell succeeded/failed" tested on the wrong register (2CDD:0B77, 0404).
pub const SPELL_MESSAGE: u8 = 21;
/// #22: 2CDD:030F's timed flag set on the player, not the target.
pub const TIMED_FLAG_PLAYER: u8 = 22;
/// #28: the fatigue drain tests the wrong carry (2CDD:0180).
pub const FATIGUE_DRAIN_CARRY: u8 = 28;
/// #67: ending an effect decrements DGROUP's counter, not the record's (2EBD:0004).
pub const END_EFFECT_COUNTER: u8 = 67;
/// #69: spell kinds 2 and 6 leave DS at the target's segment (29CF:0941).
pub const SUB_KIND_DS: u8 = 69;
