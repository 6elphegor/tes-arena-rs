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
/// #18: instant death kills on a roll at or above its chance (2CDD:03C1).
pub const INSTANT_DEATH: u8 = 18;
/// #19: an Archer's critical chance uses the missile search's CX, not the level (2AA1:0852).
pub const ARCHER_CRIT: u8 = 19;
/// #20: a caster's "Spells" without stock runs the magic items' (1194:AA8C).
pub const GUILD_SPELLS_PICK: u8 = 20;
/// #21: "Spell succeeded/failed" tested on the wrong register (2CDD:0B77, 0404).
pub const SPELL_MESSAGE: u8 = 21;
/// #22: 2CDD:030F's timed flag set on the player, not the target.
pub const TIMED_FLAG_PLAYER: u8 = 22;
/// #23: an effect's pulse recomputes the player's modifiers from percent attributes (1194:361D).
pub const EFFECT_SCALE: u8 = 23;
/// #24: the Luck modifier always 50, the Willpower half-modifier unsigned (1194:3621).
pub const ATTRIBUTE_MODIFIERS: u8 = 24;
/// #25: Knights' kind-2 check never applies to shields (1194:7772).
pub const KNIGHT_SHIELD: u8 = 25;
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
/// #38: deleting the spellbook's last spell leaves its page up (1194:52DB).
pub const SPELL_DELETED: u8 = 38;
/// #39: the status lists only the first fortifying effect (2686:01C3).
pub const STATUS_FORTIFIED: u8 = 39;
/// #40: a find-a-person quest's deadline reads the stage from the name
/// (1194:9162).
pub const OFFER_STAGE: u8 = 40;
/// #41: a quest item not in the list writes FFh over the main quest's
/// step (1194:9C74).
pub const QUEST_ITEM_END: u8 = 41;
/// #43: piles' odd halved gold dropped, magic types forced by a scratch
/// register, the level's limit read from the frame (2433:0358, 04C8, 0101).
pub const PILE_QUIRKS: u8 = 43;
/// #44: a dark run's ceiling x step has the EMS frame's segment for its low word (0000:D865).
pub const CEILING_STEP: u8 = 44;
/// #45: the lightning bolt image's y put back into DGROUP, not the image (2294:05A6).
pub const BOLT_Y: u8 = 45;
/// #46: a returning snowflake banded by the view's width, not its count (0000:E26B).
pub const SNOW_BANDS: u8 = 46;
/// #47: the second moon's phase decoded at offset CCAEh, not through its pointer (0000:E499).
pub const MOON2_PHASE: u8 = 47;
/// #48: nested texts are wrapped too, the level's test lost (1194:A4B0).
pub const TEXT_WRAP: u8 = 48;
/// #49: a failed quest's search compares with a stage left over (1194:9B26).
pub const FAILED_STAGE: u8 = 49;
/// #50: the informant's line rolled on a fake RNG in the EMS frame
/// (1194:D105).
pub const INFORMANT_RNG: u8 = 50;
/// #51: the dream speech maps back VISION.EMS's bank word >> 4, not >> 2 (2F75:014E).
pub const VISION_BANK: u8 = 51;
/// #52: the "Where is" list leaves its picture's load bank mapped
/// (1194:5884).
pub const LIST_BANK: u8 = 52;
/// #53: a one-name sort leaves the order table stale (0000:0B9C).
pub const ONE_NAME_SORTED: u8 = 53;
/// #54: "her" written past the text when "his home" isn't in it
/// (1194:2945).
pub const HER_HOME: u8 = 54;
/// #55: the staff's quest lines written over DGROUP at 5536h (1194:1F56).
pub const STAFF_LINES: u8 = 55;
/// #57: an Argonian's helmet leaks a heap block each sheet drawn
/// (1194:2020).
pub const HELMET_LEAK: u8 = 57;
/// #58: each light table made leaks the last (0000:AAA2).
pub const LIGHT_TABLE_LEAK: u8 = 58;
/// #59: last fit leaves a stale back pointer; first fit's order check tests seg | off (0000:9CD6, 51F2).
pub const HEAP_LINKS: u8 = 59;
/// #61: the loot window puts back a bank it never saved (2433:0583).
pub const LOOT_BANK: u8 = 61;
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
