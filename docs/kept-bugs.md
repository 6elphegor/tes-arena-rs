# Original ACD.EXE bugs kept by arena-port

## Fixed mode

The port stays faithful by default; each bug below can also be corrected behind a switch.

- **The switch:** `Machine::fixes` (`src/fixes.rs`), a set with one bit per entry here, bit n for #n.
  `Machine::new` starts it at `Fixes::NONE`, so every test, vector and generator stays byte for
  byte the original's. The view sets `Fixes::ALL` for the machines it makes (`view/src/world.rs`
  `machine`) unless it is run with `--original`. A machine's copies (`Clone`, `clone_from`) keep
  the set. A saved game doesn't store it.
- **Adding a fix:**
  1. Add a constant named after the bug to `src/fixes.rs`, set to its number here, in number order.
  2. At the bug, branch on `self.fixed(fixes::NAME)` and leave the faithful code exactly as it
     was. Keep the comment on the original's bug, and add one saying what fixed mode does and why
     the surrounding code shows that was the intent.
  3. Add a test named `fixed_...` to the routine's existing test file, next to its faithful
     tests (which stay as they are). It sets `m.fixes = Fixes::only(NAME)`, checks the corrected
     behaviour, and fails with the bit off; where it can, it also shows the original's behaviour
     with `Fixes::NONE`.
  4. Mark the entry's Fix cell here **Fixed** with the constant's name.
  Don't regenerate vectors for a fix: faithful runs don't change.
- `Fixes::ALL` sets every bit, so a new fix is on in the view as soon as it is added.

Read-only survey of `arena-port/src`, `arena-port/view/src` and `arena-port/docs` (2026-10-21).
Search terms: bug / bugs kept / the original's bug / (kept) / meant / wrong / never / always /
instead of / leftover / garbage / overflow / clobber / lost / forgotten / never freed / quirk,
plus every `giveup::original` / `original_fault` site. I read the code and comments around each hit.

Paths are relative to `arena-port/src/` unless they start with `view/`. Addresses are the
original's `seg:off`, as the port's comments give them.

**How the entries are ranked:** the player-visible bugs come first, roughly in order of how likely an
ordinary player is to hit and notice them. "Invisible" means the bug only changes internal state (wrong
memory written, a leak, a stale register) and nothing shows on screen in normal play. When
a comment says what happens but not whether a player would notice, the entry is marked
*(visibility unsure)*.

**Difficulty:** trivial = a one-line change (flip a comparison, use the right
variable). Small = a local change to one routine. Involved = it touches several routines or
data layouts, or it breaks the port's bit-exact checks against the emulator (every fix
does that, so a fix would have to go behind a "fixed" switch).

---

## A. Player-visible bugs (most noticeable first)

### A1. Crashes and hangs reached in normal play

| # | Where | Original addr | What the player sees | Category | Fix |
|---|---|---|---|---|---|
| 1 | text.rs:1054-1074 (also view/src/playtest.rs:43-48) | 2DA2:05DC | A townsperson's answer that contains `%en` ("work for the owner of %en", #0271, #0289, #0303...) outside an equipment store makes the text routine return into the middle of code segment 2DA2. Usually the game crashes or prints garbage. The port's playtest reaches this (seed 105, CITY:1:2, hour 10). | UI/display (crash) | small: on the non-store path, pop DS correctly (or name the nearest store). **Fixed** (`EN_OUTSIDE_STORE`): in the street %en names the town's first equipment store as inside it (nothing with none) |
| 2 | shops.rs:24-29, 437-442 | 1194:C36E, 2EBD:0004 | **Temple "Cure all":** it charges the list *index* (1-2 gold) instead of the total, cures only the first affliction, then calls through a table with a record offset as the index, which is not code, so it crashes. The port stops (None) at that point. | economy (crash) | small: charge TOTAL, loop the cure over each affliction, drop the bad indirect call. **Fixed** (`CURE_ALL`): pays the listed total, cures each affliction listed |
| 3 | mapnotes.rs:161-175; view/src/playtest.rs:49 | 0000:343C (31FE/32FE) | Clicking a note on the automap can overflow the divide in the hit test (`div cl`), which is a CPU fault and crashes the original. | UI/display (crash) | small: do the hit test with a wider divide. **Fixed** (`NOTE_DIVIDE`) |
| 4 | haggle.rs:41, 304-309 | 0000:52F2 | When a merchant's asking price equals his limit, the next counter-offer divides by zero (crash). | economy (crash) | **Not reached, no fix needed:** the offer's tests before each divide (selling: past the price, not past the limit; buying: not under the limit, under the price) leave no offer to divide when price == limit, nor a quotient that overflows (haggle.rs comments) |
| 5 | piles.rs:36-41, 315, 347 | 2433:0358, 2433:7525 | Treasure piles made in the wild (kind of place 5): the routine tables are read past their end, and an item, weapon or magic pile calls into a string, so the game crashes. *(visibility unsure: depends on whether play makes piles there)* | gameplay (crash) | small: give kind 5 proper table entries. **Fixed** (`WILD_PILES`): the wild's piles are rolled by the outdoors' row (kind 3: gold always, then items, weapons, magic); the pile keeps kind 5 |
| 6 | guild.rs:936-944 | 1194:5ED1 | Removing a guild stock entry with a pick past the end of the list copies 64K entries, a #GP crash. It follows from #20 (Spells with no stock), and from an item's name over 33h bytes (built at 1395) running into the pick (13C8). | UI (crash) | trivial: bounds check. **Fixed** (`GUILD_PICK_PAST`): a pick past the stock buys nothing; the entry taken out is the record bought, not PICK read again |

### A2. Exploits and economy bugs

| # | Where | Original addr | What the player sees | Category | Fix |
|---|---|---|---|---|---|
| 7 | talk.rs:1368-1396 | 292B:0743, 1194:53F9, 2826:0B30 | **You can retry a locked chest forever.** The failed-pick list is checked against a square made from the player's position (SI left at 01A4) but recorded under the chest's square, so a chest never counts as "already failed". | gameplay | trivial: look up the chest's square, not 01A4's. **Fixed** (`CHEST_RETRIED`) |
| 8 | store.rs:91-93 | 1194:FC78 | **A stolen equipment-store item stays for sale**, so you can steal it again and again. Also, the sell-back price of an unenchanted item (+0Fh = FFh) is the type byte or the material's scaled price, not table price + material, so sell prices come out odd. | economy | small: remove the record on a successful steal; compute the price as table + material. **Fixed** (`STORE_STOLEN`): a stolen magic record leaves the list as a bought one does (plain records stay, as when bought: the plain stock is endless); an unenchanted record's resale price is the table's (times the material's), the base the enchantment's price is added to |
| 9 | strike.rs:163-170 | 0000:3F50 | **From level 11 on, every crime is witnessed** (killing townsfolk, failed steals): `100 - 10*level` is unsigned and wraps to over 65000, so the roll always succeeds. The intended floor is 50%. | gameplay | trivial: signed subtraction / clamp. **Fixed** (`CRIME_SEEN`) |
| 10 | haggle.rs:39, 312 | 0000:81F0 | **Selling: the merchant always accepts any counter-offer within his limit.** `and ax,ax` clears carry before the JAE, so the acceptance roll is never reached. | economy | trivial: do the roll's compare before the JAE. **Fixed** (`SELLING_ROLL`): the sale's terms worked out signed, as the purchase's mirrored, and decided as a purchase is (haggle.rs) |
| 11 | guild.rs:30-33, 739-748 | 1194:B51A | Mages' guild potions and spells are haggled with quality and factor 44h instead of the guild's own values, and the holiday price worked out first is thrown away. Holiday discounts on potions and spells are lost. | economy | trivial: pass the guild's quality/factor and the holiday price. **Fixed** (`GUILD_HAGGLE`): the guild's quality and factor. The holiday half is not a bug: the callers' prices carry their holiday already (a potion's list price, a spell's undoubled cost), so using B51A's would discount twice |
| 12 | shops.rs:27-29, 584-592 | 1194:C58B | Temple blessing: a second donation within a day of the first takes your gold and blesses nothing. The single cure's price is also added to the list's total a second time. | economy | small. **Fixed** (`SECOND_DONATION`): a second donation within a day blesses when it is more than the day's last (the compare the code makes, but before overwriting DONATED); a smaller one still blesses nothing (the conservative reading: one blessing a day unless you give more). The total's second add is unseen (the total is only read for the list, before) and left as is |
| 13 | inventory.rs:485-512 | 1194:511A | Taking off an attribute-raising item when the attribute (in percent) is below the item's bonus (for example after a drain): the 8-bit subtraction wraps and clamps at 255, so **the attribute jumps to its maximum**. | gameplay/economy (exploit) | trivial: saturating subtract. **Fixed** (`UNEQUIP_WRAP`) |
| 14 | haggle.rs:39-43, 15-27, 396-415; sidequest.rs:1253-1263 | 1194:8349, 1194:D019 | Informant: he is paid before you confirm, and a refusal keeps the gold. Accepting a merchant's *first* price returns RESULT (65B8) as an earlier haggle left it, unless that opening line contained `%mm`, so you can be charged or paid a previous haggle's price. With no days left on the informant, his flag is cleared and a return address is mapped as an EMS bank. | economy/quests | small each. **Fixed** (`FIRST_PRICE`): the first price accepted is the deal's (doubled back if halved), what %mm puts in RESULT. The informant's parts aren't reached: his confirmation (85DE in modes 5, 6) always agrees, so no refusal follows the payment; and D019's no-days path is behind CED7's own check of the days, which lets him go first |
| 15 | tavern.rs:30-35, 816-828 | 1194:BCEA | Sneaking into a tavern room gives a one-day room, but its hours counter (1019) is set to the low word of the absolute end time. "Rest" in that room can then run for thousands of hours, not 24. | gameplay | trivial: hours = 24. **Fixed** (`SNEAK_HOURS`) |
| 16 | loot_window.rs:31-38 | 2433:0883, 4BF4 | Looting: after a corpse's gold is shown, the "gold shown" flag is cleared in the corpse's segment, not DGROUP's, so an emptied corpse that had gold shows no message. The picked pile is found again by counting piles on that square and level without checking the kind of place, so a pile of another kind there makes **you get a different pile than the one you clicked**. | UI/gameplay | small. **Fixed** (`LOOT_PICK`): the pile found is counted among the piles of the kind of place listed; the stray write to the corpse's segment is skipped. The message part is not a bug: the flag is set to keep "has nothing" from following the gold, DGROUP's is cleared each time the window opens, and clearing it before the first list (as meant by the stray write) would defeat it |
| 17 | travel.rs:14-16 | 1194:2E69 | Fast-travel time: terrain pixels are read from the start of the loaded file, header included, so every sample is 12 pixels off the drawn route and journey lengths are slightly wrong. (The unbounded 500-byte line buffers cannot overflow on a 320-wide map, so that part is invisible.) | gameplay | trivial: add the 12-byte header offset. **Fixed** (`TRAVEL_HEADER`) |

### A3. Combat and magic

| # | Where | Original addr | What the player sees | Category | Fix |
|---|---|---|---|---|---|
| 18 | spell.rs:735-753 | 2CDD:03C1 | **Instant-death spells: the success comparison is the wrong way round**, so they kill on a roll at or above the chance (with a 10% chance they kill 90% of the time). For targets outside DGROUP the original then reads its message and handler tables through the target's segment; the port stops there (not followed). | combat | trivial: flip the comparison (fixing the DS problem is small) |
| 19 | combat.rs:83-85, 175-178 | 2AA1:0852, 1194:78DE | Archer critical chance is 3 × `missile_cx` (3 minus the weapon's index in the missile table, so 0-3) instead of 3 × level. **Archer crits never get better with level.** | combat | trivial: restore CX/level |
| 20 | guild.rs:24-27 | 1194:AA8C (table 7149) | A spellcaster in a guild with no magic-item stock who picks "Spells" from Buy gets the (empty) magic-items screen, because the table is indexed by the menu pick. | UI | trivial: index by the item, not the pick. **Fixed** (`GUILD_SPELLS_PICK`) |
| 21 | spell.rs:465-476, 758-759 | 2CDD:0B77, 2CDD:0404 | The "Spell succeeded/failed" message: the is-the-player check compares DX after the MUL has overwritten it, so the message almost never shows. For kind 0404 the check looks at the slot pointer, so the message always shows. | UI/combat | trivial |
| 22 | spell.rs:714-725 | 2CDD:030F | One timed-effect spell kind sets its timer on the target but sets the flag (05BE bit 2) on the *player*; the target never gets the flag. | combat | small |
| 23 | effects.rs:497-500 | 1194:361D | While an attribute-changing effect pulses, derived stats (damage bonus, carry weight, max SP, ...) are recomputed with a routine that expects 256-scale attributes on a record that holds percent. *(visibility unsure, but probably visible as odd max-SP/weight swings)* | combat | small: convert scales first |
| 24 | character.rs:181-196 | 1194:3621 | Attribute modifiers: the Luck modifier is always 50 (Luck's value is thrown away and the divisor doubled). The Willpower half-modifier uses a logical shift, so a negative modifier becomes a large positive one. Two derived bytes (B_5F, B_6A) are always 0. Applies to the player too (tavern drinks call it). | combat | trivial each |
| 25 | inventory.rs:372-381 | 1194:7772 | The "Knights can't use armour kind 2" check after a shield match never applies (the class register has been doubled by then), so Knights can use kind-2 shields. | gameplay | trivial |
| 26 | encounter.rs:147-148, 196-200 | 25CA:006A, 0000:26DC | Wandering creatures' speed is read from the *floor map* at the record's offset (FS left on the floor plane) instead of the creature record's +2Dh, so monsters walk at speeds that depend on map data. | combat | trivial: read through the creature segment |
| 27 | strike.rs:174-180 | 0000:3B6D, 0000:3D17 | The game works out whether the player is behind the monster, then `xor ax,ax` clears the carry before the JAE, so the result is always 0. Whatever rear-attack effect was planned never happens. *(visibility unsure: the port's ARG is stored but nothing reads it)* | combat | trivial |
| 28 | spell.rs:633-640 | 2CDD:0180 | The fatigue-drain spell tests the wrong carry and writes the fatigue delta even when the effect wasn't applied, straight into the target. | combat | trivial |
| 29 | spell.rs:249 | 2CDD:03F2 (table 843E) | Spell kind 03F2 zeroes DI before its call, so it always applies the first entry of its sub-table, whatever the spell says. | combat | trivial |
| 30 | projectile.rs:626-631 | 29CF:09B8 | A targeted spell that hits a person (slot below 0Fh) does nothing, and its cast record is never freed. If cast records run out, the next cast "searches on through memory" (29CF:01C4). *(visibility unsure)* | combat | small: free the record |
| 31 | projectile.rs:143-148 | 29CF:037F | Arrows lose their C8h head start: the projectile starts at the shooter's own position. *(visibility unsure)* | combat | trivial |

### A4. UI, display and logbook

| # | Where | Original addr | What the player sees | Category | Fix |
|---|---|---|---|---|---|
| 32 | automap.rs:20-22 | 23F1:0145/0149 | **Explored automap squares come back reversed within each group of four** after the map is written out and read back (leaving and re-entering an area or level): packing puts a group's first square in the top bits, unpacking takes it from the bottom. | UI | trivial: match the bit order. **Fixed** (`AUTOMAP_ORDER`): the unpacking takes the top bits first, so the files (the original's too) keep their format |
| 33 | log.rs:188-198 | 27E3:0169..01AA | **The logbook is never trimmed.** The trim's new file name is built twice ("C:\ARENA_CD\C:\ARENA_CD\log.nn"), DOS refuses it, and the failure isn't checked. LOG.nn and the logbook only grow. | UI/saves | trivial: build the path once. **Fixed** (`LOG_TRIM`) |
| 34 | logbook.rs:28-37 | 27E3:0137, 01FC | Logbook: Exit on any page but the last needs two clicks. Once a button has been seen, AFF9 stays set, so on the last page just moving the cursor over Exit or Print counts as a click. | UI | small. **Fixed** (`LOGBOOK_CLICKS`): Exit, Print or Esc on any page leaves the screen; a click off the buttons on the last page clears its bit as a release wait would |
| 35 | logbook.rs:18-19, 41-46 | 1194:4E54, 27E3:0272 | Logbook Print: the line count (FF5) is written into the EMS bank (DS is the frame's). The print loop is a LOOP, so with a count of 0 it prints 65536 lines. The form feed's write isn't checked. | UI | trivial. **Fixed** (`LOG_COUNT`): the count goes to DGROUP's FF5 (where 4E54's other callers have it), so two bytes of a log over FF5h bytes are no longer overwritten on its page; a count of 0 prints nothing. The 0 count isn't otherwise reached (lines + 1), and the form feed's write can't fall short (PRN's writes stop only at a ^Z) |
| 36 | lineedit.rs:9-11, 26-29, 146-158 | 0000:A1B9, 9EE9 | Text fields (save names, spell names...): clicking inside the text puts the cursor by counting how many *first-character* widths fit, not at the clicked character. A first space puts it at 0, and a first character of width 0 hangs. Typing in the middle of the text moves the NUL up without writing a new one, so the byte that followed the old end becomes part of the text (**a stray character can appear**). | UI | small. **Fixed** (`EDIT_TEXT`): the click counts the text's characters by their own widths (a width of 0 passed over); a NUL is written after the new end |
| 37 | lineedit.rs:44-49 | 0000:718A | Keyboard interrupt: when the 100-byte ring is full, a new key still overwrites the write slot, and its key-state goes to the last key taken. Typing with left Shift held also marks right Shift down. | UI | small. **Fixed** (`KEY_RING`): with the ring full the key's character is dropped but its own state set; the Shifts' states are their own (the left still shifts the character typed) |
| 38 | spellpage.rs:24-30 | (spell book page) | Deleting the last spell in the spellbook leaves the deleted spell's page on screen (the mark is already the last line, so nothing is redrawn). | UI | trivial. **Fixed** (`SPELL_DELETED`): the page shows the spell now in the deleted one's place, or the new last one (this also fixes deleting the first, which moved on past the spell now there) |
| 39 | status.rs:146-150 | 2686:01C3 | Status screen: only the first fortified attribute is listed. The player pointer in EAX is overwritten by the first line printed. | UI | trivial. **Fixed** (`STATUS_FORTIFIED`): every fortifying effect's attributes are listed |
| 40 | sidequest.rs:1068-1079 | 1194:9162..9321, 26F7:0C0A | Side quests: the "find a person" deadline (days doubled unless stage 6) reads the stage through SI, which points at the person's name buffer, so the deadline depends on the name's 5th and 6th characters. | quests | trivial. **Fixed** (`OFFER_STAGE`): the record's own stage |
| 41 | quest.rs:386-391 | 1194:9C74 | When a quest record's item isn't in the item list, the search runs off the 10-byte list and writes FFh to 0FDD, **the main-quest step**. If this happens it could break main-quest progress. *(how often it triggers is unknown, probably rare)* | quests | trivial: stop at the list's end |
| 42 | dungeon.rs:483-488 | 0000:2A73 (dun_load_chunk) | Dungeon chunk loader: the light count is reset for each chunk, so each chunk's lights overwrite the previous ones from the start of the list. The lock-list offset is 3 × count in 8 bits (it wraps). Targets that would pass 0x40 skip the whole chunk. Lights, locks or triggers can go missing in multi-chunk dungeons. *(visibility unsure)* | gameplay | small |
| 43 | piles.rs:32-38 | 2433:0358, 041C | Treasure piles: in climate 2 the gold amount is halved with `shr` and an odd amount's carry drops that pile. Magic-weapon/armour piles take their forced type from the level handler's BX, so their type is fixed by the kind of place. The full-level test reads its limit from frame:4D76. | gameplay/economy | small. **Fixed** (`PILE_QUIRKS`): the halving keeps an odd amount's pile, magic piles' types are unforced (FFFFh, as the plain pile's FFh), the limit is read from DGROUP |
| 44 | render.rs:806-812; view/src/world.rs:250 | 0000:D865..D878 | Ceiling texels after a "dark run" use a step whose low word is CCCE (EMS frame segment + C00h), so those ceiling pixels depend on where the EMS frame is: a fixed small texture glitch on real hardware. | UI/display | trivial |
| 45 | render.rs:234-240 | 2294:05A6 | The lightning bolt image's y in EMS is set and never put back, so the next frames read the changed value. | UI/display | trivial |
| 46 | frame.rs:1165-1171 | 0000:E2xx (snow) | Snow: a flake that wraps around is put in a size band by the view width left in CX, not by its count, so flake sizes are mis-distributed. | UI/display | trivial |
| 47 | celestial.rs:250-256, 276-281 | 0000:E41A/E46D/E499 | The second moon's phase picture is decoded at offset CCAEh in the EMS frame, not through the far pointer stored there, so it lands in the wrong place. *(visibility unsure: may show a stale or wrong phase for the second moon)* | UI/display | trivial |
| 48 | text.rs:11-12 | 1194:49FF | Text wrapping at 137E was meant for the outermost nesting level only, but the test is lost, so nested texts get wrapped too. *(visibility unsure)* | UI | trivial |

---

## B. Invisible bugs (internal state only)

| # | Where | Original addr | What it does | Category | Fix |
|---|---|---|---|---|---|
| 49 | sidequest.rs:260-275 | 1194:9B26 | A failed quest of the place but a different stage leaves its stage in AX, and later records are compared with that stage. | quests (internal) | trivial. **Fixed** (`FAILED_STAGE`) |
| 50 | sidequest.rs:1311-1316 | 1194:D105, 0000:6EEA | The informant's d3 line choice runs the RNG with DS = the EMS frame, so it uses (and refills) a fake RNG made of the dialogue file's bytes. Which line he says is effectively fixed per file. | quests (internal) | small. **Fixed** (`INFORMANT_RNG`): rolled with the game's RNG |
| 51 | vision.rs:39-42 | 0000:014E | After the dream speech, VISION.EMS bank [8F41]>>4 is mapped, but the bank written was 8FA8's × 4. | internal-only | trivial |
| 52 | listwindow.rs:171-176 | 5884 | The list picture's load bank is stored as the "bank before", so the list's end leaves bank 19h mapped. | internal-only | trivial |
| 53 | listwindow.rs:398-408 | 0000:0B9C | A one-name sort returns without setting the 1044 order table, so the pick is read through stale order data. (With one entry the result is the same.) | internal-only | trivial |
| 54 | newgame.rs:736-741 | 1194:2945..2970 | Making "his home" into "her home": if the text isn't found, "her" is written at 1044+3E8h anyway. | internal-only | trivial |
| 55 | charsheet.rs:921-931 | 1194:1F56 | Clicking the staff quest line again after a description or note uses DI = 5536h, and the quest lines are written over DGROUP there. (Memory corruption: could become visible.) | quests (internal) | small |
| 56 | charsheet.rs:1549-1552 | 1194:745A/75D3 | The worn-item search counts slots looked at, not items, so it can stop before a worn item. *(effect unclear)* | internal-only | trivial |
| 57 | charsheet.rs:1784-1802 | 1194:2020, 0000:573C | Argonian with a helmet: each character-sheet draw takes a heap block for the helmet's background and never frees it (leak). If the heap runs out, the original writes the background to 0:0. | internal-only (possible crash) | trivial: free it |
| 58 | load.rs:413-420 | 0000:AAA2 | Each light-table rebuild takes 1000h bytes from the heap and doesn't free the old one (leak). | internal-only | trivial |
| 59 | heap.rs:12-16, 84-90 | 0000:9CD6, 51F2 | Last-fit allocation doesn't fix the next block's back pointer after a split (a later free can merge into a stale neighbour). The first-fit ordering check compares seg\|off of the header instead of neighbouring segments. | internal-only | small |
| 60 | heap.rs:24-29, 58 | 0000:00DF | SHRINK is 0 and nothing writes it, so startup never shrinks its DOS block. | internal-only | trivial |
| 61 | loot_window.rs:36-42 | 2433 (4D77) | The window maps bank 6 without saving the old bank, then restores 4D77, which the key and corpse paths never set. | internal-only | trivial. **Fixed** (`LOOT_BANK`): the bank mapped before the window is put back |
| 62 | actors.rs:579-606 | 0000:25CE | A creature leaving range: the trigger waiting on it gets the right row but a column taken from a leftover loop count. *(could misplace a scripted trigger)* | internal-only | trivial |
| 63 | attack.rs:176-190 | 1194:7316 | Bare-handed gauntlet search reads the slot count in the creature's segment, not DGROUP. | internal-only | trivial |
| 64 | attack.rs:58 | 2AA1:0166 | Unsigned compare: a negative HP never sets the low-HP flag (CHANGED bit 80h). | internal-only | trivial |
| 65 | encounter.rs:884-889 | 1194:7C1D | NPC armour quality: for listed classes the level is read at list place + 6, in the record's segment. | internal-only | trivial |
| 66 | creature.rs:245-247 | 1194:38B9 | NPC seed with level 0 always rolls the class, ignoring the class requested. | internal-only | trivial |
| 67 | spell.rs:1092-1096 | 2EBD:0004 | Ending an effect decrements the kind counter in DGROUP at the record's offset, whatever the record's segment (corrupts DGROUP for creature targets). | internal-only | trivial |
| 68 | spell.rs:555-560 | 2CDD:00CC | Curse's disease dice use the RNG state at the target's segment (only right for the player, the only record that rolls it). | internal-only | trivial |
| 69 | spell.rs:300-310 | 29CF:0941 | Kinds 2/6 call sub-handlers without saving DS; for targets outside DGROUP the original reads handlers through the target segment. The port stops (not followed), so the outcome is unknown. | combat (undetermined) | small |
| 70 | effects.rs:238-240 | 2EBD:0102 | A new effect's deltas start from whatever the slot held before. | internal-only | trivial |
| 71 | cast.rs:229-232 | 29CF | The missile launch's DX is whatever the caller left, which sets element 5's delay. | internal-only | trivial |
| 72 | wild.rs:154-161 | 217B:052C | Walled-city footprint quadrants are written from offset 0 of the buffer segment, not the grid at F000, so the grid is never filled. | internal-only | trivial |
| 73 | dungeon.rs:268-274 | 0000:28BC | Dungeon sizes stored as tiles (<<5) lose their top 5 bits. | internal-only | trivial |
| 74 | shop.rs:13-16, 663 | 1194:B848 | The class check's finisher reads a base price (1391) the shop loop never sets (it carries over between items). The kind-2 sub-kind is always rolled. | internal-only | trivial |
| 75 | engine.rs:504-509 | (2294) | Reach shifted left then right loses bit 15 for the draw. | internal-only | trivial |
| 76 | text.rs:1328-1340; view/src/questui.rs:207 | 2DA2:0BAF | `%qf` seeds a name with a stale EBX high word. No text uses it. | internal-only | trivial |
| 77 | mif.rs:233-236, 292-296 | 3065:0D2D | GetByte's EOF test is always taken (EOF goes in as FFFFh). A size of 0 leaks the decoder tables. | internal-only | trivial |
| 78 | scale.rs:40-41 | 1194:1012 | Percent to 256 conversion: 100 maps to 255, not 256, and bytes are read signed. | internal-only | trivial |
| 79 | savemenu.rs:161 | 25FD:0718 | The slot picker's remainder test always passes. | internal-only | trivial |
| 80 | interface/view ring: view/src/interface.rs:124, faces.rs:38 | 0000:7608 | The LZSS ring's last 12h bytes start as whatever was in the buffer. | internal-only | trivial |
| 81 | level.rs:100-106 | 0000:E930 | A level number past the last reads a stale chunk until the 32-bit file position wraps around GLOBAL.BSA. | internal-only | small |
| 82 | loot.rs:148-152 | 1194:EDBC | The material roll is 1..99, so any threshold of 100 or above (the top one, 101) is unreachable. *(whether a material is actually unreachable depends on the table data, which isn't in the repo)* | economy (maybe) | trivial |
| 83 | travelmap.rs:437-441 | 26F7:0D50 | Only Spellswords get the class-adjusted day count; for everyone else the jittered value is dropped and DAYS from 1194:2E69 is used. (Possibly by design, so not counted as visible.) | internal/design | trivial |

### B2. Faults on unusual input (summarised)

There are about **143** `giveup::original` / `original_fault` / render `gap(...)` sites where
the original faults (divide by zero, divide overflow, #GP), loops forever (rerolls that can never
succeed, scans with no terminator), jumps into data, or reboots. The port stops (returns
None) there instead of copying the hang. Most need impossible or corrupt data: more than 65535
days of game time (clock.rs:183, sidequest.rs:291/701), zero-size views, empty tables,
no free square, a loot table with no allowed item (loot.rs:179-522), and so on. The ones
known to be reached in play are already listed above (#1, #3, #4, #5, #6). Hardware-specific
ones: options.rs:187 (CyberMan button returns into its pushed registers),
sound.rs:313 (no sound banks: a LOOP writes FFFFh over all of DGROUP), lineedit.rs:565
(Ctrl-Alt-Del reboots). Fixing them is usually trivial (a guard), but they almost never fire.

---

## C. Counts

| Category | Visible (A) | Invisible (B, numbered) |
|---|---|---|
| gameplay / economy | 16 (#2, 4, 5, 7-15, 17, 25, 42, 43) | 2 (#82, #83) |
| combat | 12 (#18, 19, 21-24, 26-31) | 1 (#69, outcome undetermined) |
| UI / display | 17 (#1, 3, 6, 16, 20, 32, 34-39, 44-48) | 0 |
| quests | 2 (#40, 41; #14 is part quests) | 3 (#49, 50, 55) |
| saves / log | 1 (#33) | 0 |
| internal-only | 0 | 29 (#51-54, 56-68, 70-81) |
| **Total** | **48** (#1-48) | **35** (#49-83) + ~143 fault sites |

(Several rows hold two or three related bugs from one routine; counting those
separately gives about 60 visible and 40 invisible.)

---

## D. Community-documented Arena bugs (from memory, unverified)

These come from general knowledge of UESP's Arena bug pages and community lore, **not checked
against any source in this session**. "In port?" says whether the catalogue above has a
matching bug.

| Community bug (from memory, unverified) | In port? |
|---|---|
| Lockpicking a chest can be retried endlessly | **Yes**: #7 (talk.rs:1389) |
| Haggling/selling exploits: merchants accept too easily, prices can be pushed | **Partly**: #10 (selling acceptance never rolled), #14 (stale price) |
| Temple "Cure all" crashes or charges oddly | **Yes**: #2 |
| Stealing from shops can be repeated on the same item | **Yes**: #8 |
| Guards always notice crimes at higher levels | **Yes**: #9 |
| The Luck attribute does little or nothing | **Consistent with** #24 (Luck modifier hard-wired to 50) |
| Attribute-max exploit by removing enchanted items while drained | **Consistent with** #13 |
| Instant-death ("Doom"-style) spells too effective | **Consistent with** #18 |
| Log file grows without limit, saves slow down | **Yes**: #33 |
| Random crashes talking to townspeople | **Consistent with** #1 (%en) |
| Automap markings scrambled after leaving and returning | **Consistent with** #32 |
| Spellmaker cost overflow giving very cheap or free spells | Not found as a "kept bug" note (spellmaker.rs only has divide-overflow faults at 1194:70BA) |
| Falling through the floor / getting stuck in walls in the wilderness or while levitating | Not found |
| Spell Absorption / Reflect not working as described | Not found |
| Level-up attribute bonus points lost or wrong | Not found (levelup.rs has no bug notes) |
| Character-creation reroll / bonus-point exploits | Not found (no bug notes in newgame.rs's rolls) |
| Main quest broken by quest-item handling in some versions | **Possibly related** to #41 (0FDD overwritten), but unverified |
| Dungeons missing lights or locks in large (multi-chunk) dungeons | **Possibly related** to #42 |
| Wandering monsters with erratic speed | **Consistent with** #26 |
| Archers' criticals don't scale | **Consistent with** #19 (not a known community report as far as I recall) |
| Out-of-memory / EMS crashes after long play | **Possibly related** to the leaks #57, #58 |
| Crashes at the travel screen on long journeys | Not found (travel.rs's buffer overrun can't happen at 320 px) |
