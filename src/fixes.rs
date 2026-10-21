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
/// #19: an Archer's critical chance uses the missile search's CX, not the level (2AA1:0852).
pub const ARCHER_CRIT: u8 = 19;
/// #21: "Spell succeeded/failed" tested on the wrong register (2CDD:0B77, 0404).
pub const SPELL_MESSAGE: u8 = 21;
/// #22: 2CDD:030F's timed flag set on the player, not the target.
pub const TIMED_FLAG_PLAYER: u8 = 22;
/// #23: an effect's pulse recomputes the player's modifiers from percent attributes (1194:361D).
pub const EFFECT_SCALE: u8 = 23;
/// #24: the Luck modifier always 50, the Willpower half-modifier unsigned (1194:3621).
pub const ATTRIBUTE_MODIFIERS: u8 = 24;
/// #26: a placed creature's speed read from the floor plane (25CA:006A).
pub const CREATURE_SPEED: u8 = 26;
/// #27: striking from behind is worked out and thrown away (0000:3B6D).
pub const REAR_ATTACK: u8 = 27;
/// #28: the fatigue drain tests the wrong carry (2CDD:0180).
pub const FATIGUE_DRAIN_CARRY: u8 = 28;
/// #30: a spell striking a person or nobody never frees its cast record (29CF:09B8).
pub const CAST_RECORD_LEAK: u8 = 30;
/// #31: an arrow's C8h head start is lost (29CF:037F).
pub const ARROW_HEAD_START: u8 = 31;
/// #44: a dark run's ceiling x step has the EMS frame's segment for its low word (0000:D865).
pub const CEILING_STEP: u8 = 44;
/// #45: the lightning bolt image's y put back into DGROUP, not the image (2294:05A6).
pub const BOLT_Y: u8 = 45;
/// #46: a returning snowflake banded by the view's width, not its count (0000:E26B).
pub const SNOW_BANDS: u8 = 46;
/// #47: the second moon's phase decoded at offset CCAEh, not through its pointer (0000:E499).
pub const MOON2_PHASE: u8 = 47;
/// #51: the dream speech maps back VISION.EMS's bank word >> 4, not >> 2 (2F75:014E).
pub const VISION_BANK: u8 = 51;
/// #59: last fit leaves a stale back pointer; first fit's order check tests seg | off (0000:9CD6, 51F2).
pub const HEAP_LINKS: u8 = 59;
/// #62: a creature leaving range gives its trigger a loop count for a column (0000:25CE).
pub const TRIGGER_COLUMN: u8 = 62;
/// #63: the bare-handed gauntlet search reads the slot count in the record's segment (1194:7316).
pub const GAUNTLET_COUNT: u8 = 63;
/// #64: a negative HP never counts as low, the compare unsigned (2AA1:0166).
pub const LOW_HP_SIGNED: u8 = 64;
/// #65: a listed class's armour quality reads the level off the list (1194:7C1D).
pub const NPC_ARMOR_LEVEL: u8 = 65;
/// #66: an NPC seed with a rolled level rolls the class too (1194:38B9).
pub const NPC_SEED_CLASS: u8 = 66;
/// #67: ending an effect decrements DGROUP's counter, not the record's (2EBD:0004).
pub const END_EFFECT_COUNTER: u8 = 67;
/// #69: spell kinds 2 and 6 leave DS at the target's segment (29CF:0941).
pub const SUB_KIND_DS: u8 = 69;
/// #70: a new effect's deltas start at what the slot held (2EBD:0102).
pub const EFFECT_DELTAS: u8 = 70;
/// #71: a player's element-5 launch takes the caller's DX for its delay (29CF:00D2).
pub const LAUNCH_DX: u8 = 71;
/// #77: GetByte lets the end of the input in as FFFFh; a size of 0 leaks the tables (3065:0D2D).
pub const MIF_EOF: u8 = 77;
