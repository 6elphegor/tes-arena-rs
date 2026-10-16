"""Emit the original's drawing of its 2D screens over the user's saved
game, for view/src/screentest.rs.

Usage: gen_screens.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR]

SAVE_DIR holds the saved game in slot 1 (the user's: only read). The
game is started and the save loaded from the title under unicorn
(harness/screens.py: 21EE:0000, then 25FD:0309 into the world loop),
then each screen of SCREENS opened by its own routine from that state
(or, for those in WHERE, from inside another building the original's own
outer loop has walked to: out of the guild into the street, in at the
door of the town's first tavern, temple or store) and run until it first
waits for input (`waits`). It is run twice, the
page (DGROUP A900) and the VGA's memory filled first with 00h and then
with FFh: the pixels the same in both are the ones the screen drew.

  screen | NAME | <page digest> <palette digest> [<mask digest>]

The page digest is FNV-1a over its 64000 bytes (with a mask, the
undrawn ones 0); the palette's over the DAC's 300h 6-bit values; the
mask's (only where the screen leaves pixels undrawn) over a byte a pixel,
1 drawn. With --dump DIR, NAME.page, NAME.mask and NAME.dac are written
to DIR as well, for screentest's diff images (game pictures: never
commit them).

The colours on the way to the first pass's wait, at each wait for the
retrace (0000:6BB8, 9B77: a fade to black, the screen's colours set in
three parts), each kept when different from the one before, go to
screenfades.txt beside OUT for the screens with more than one:

  screenfade | NAME | <count> | <digest>

the digest the FNV-1a over each state's FNV-1a (8 bytes, little-endian)
in turn; with --dump, NAME.fade holds them (768 bytes each).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, fnv
from unicorn import UC_HOOK_CODE
from screens import ScreenArena, PAGE_SEG_FIELD

# The menu keys' handlers in play (2542:0100's), run as a key runs them
# (near calls), each until it waits for input: the mouse asked for a
# third time from one place (0000:8FC4, which every input loop calls, or
# the screen's own wait). A script's clicks and keys (scan codes given to
# the key interrupt, `ScreenArena.press`) answer the waits before the
# last.
MENU = 0x2542
SCREENS = {
    # F1: the character sheet (00BE: 1194:0CF8), its first page.
    'sheet': (0x00BE, []),
    # Its "Next Page" button (6Ah..9Dh, B2h..C0h): the equipment page.
    'equipment': (0x00BE, [('click', 0x80, 0xB8)]),
    # A line of the equipment page's list clicked (its second: marked),
    # and its Spellbook button (the sheet's spells).
    'equipmentpick': (0x00BE, [('click', 0x80, 0xB8), ('click', 60, 62)]),
    'equipmentspells': (0x00BE, [('click', 0x80, 0xB8), ('click', 78, 194)]),
    # A line right-clicked: marked, its item described (1194:1988).
    'equipmentinfo': (0x00BE, [('click', 0x80, 0xB8), ('rclick', 60, 62)]),
    # The marked line clicked again: its item (worn) taken off (1194:1AFB).
    'equipmentoff': (0x00BE, [('click', 0x80, 0xB8), ('click', 60, 62), ('click', 60, 62)]),
    # S: the status box (037C: 04B1).
    'status': (0x037C, []),
    # L: the logbook (02A3: 27E3:0088), its first page.
    'logbook': (0x02A3, []),
    # Its More... (27E3's): the next page.
    'logbookmore': (0x02A3, [('click', 285, 172)]),
    # C: the spellbook (026D: 056A).
    'spellbook': (0x026D, []),
    # A line of it clicked (the second): marked, as the use list's.
    'spellpick': (0x026D, [('click', 0xF0, 164)]),
    # Its fourth line (Arc Lightning, an area's spell) clicked twice: cast,
    # "Release spell in your area..." put on the message line, drawn as
    # after the use (`AFTER`).
    'spellarea': (0x026D, [('click', 0xF0, 182), ('click', 0xF0, 182)]),
    # Its first and fifth lines (Absorb Spell, Area Light VIII: the
    # caster's own) clicked twice: cast, nothing drawn over play, nor on
    # the message line the next frame draws (`AFTER`).
    'spellself': (0x026D, [('click', 0xF0, 155), ('click', 0xF0, 155)]),
    'spelllight': (0x026D, [('click', 0xF0, 191), ('click', 0xF0, 191)]),
    # U: the items to use (034D).
    'use': (0x034D, []),
    # A line of it clicked (the fourth, Ring of Shielding: the lines 8 or
    # 9 apart from 9Bh in the bar's corner): the line picked marked (a
    # second click uses it, the list gone).
    'usepick': (0x034D, [('click', 0xF0, 181)]),
    # Its marked line clicked again: the ring used (2542:034D's way on,
    # the list gone) and what it said put on the message line (061A),
    # which the next frame draws (`AFTER`).
    'usemessage': (0x034D, [('click', 0xF0, 181), ('click', 0xF0, 181)]),
    # Esc: the options screen (000A).
    'options': (0x000A, []),
    # Its bottom row's buttons (the table at 5C2C: y 118..146), each
    # clicked: Load (65..128, 061E) and Save (129..192, 0617), the save
    # menu's slots (25FD); Exit (193..256, 05DB), "Are you sure ?". (New,
    # 0..64, closes the screen at once.)
    'optionsload': (0x000A, [('click', 96, 132)]),
    'optionssave': (0x000A, [('click', 160, 132)]),
    'optionsexit': (0x000A, [('click', 224, 132)]),
    # The mages' guild's menu (1194:A73E, kind 7 of 869F's: its screen,
    # banner and menu), as the guild master's click runs it.
    'guild': ((0x1194, 0xA73E, {}), []),
    # The other buildings' services, each in its own building (WHERE),
    # run as the keeper's click runs them (the table at 869F): the tavern
    # (B93C), the temple (C258), the equipment store (F48E), each its
    # screen, banner and menu.
    'tavern': ((0x1194, 0xB93C, {}), []),
    'temple': ((0x1194, 0xC258, {}), []),
    'store': ((0x1194, 0xF48E, {}), []),
    # Their menus' entries clicked (each a box 18h high, the menu's
    # first entry at 3Eh for five, 4Ah for three): what each asks next.
    'taverndrinks': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x43)]),
    'tavernroom': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x5B)]),
    'tavernrumor': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x8B)]),
    'templebless': ((0x1194, 0xC258, {}), [('click', 0xA0, 0x4F)]),
    'templecure': ((0x1194, 0xC258, {}), [('click', 0xA0, 0x67)]),
    'templeheal': ((0x1194, 0xC258, {}), [('click', 0xA0, 0x7F)]),
    # The drinks' first line clicked: what the tavern says to it.
    'taverndrink': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x43), ('click', 0x70, 0x26)]),
    # The blessing's donation typed (1, 0, 0 and Enter, a key a wait):
    # what the temple says to it.
    'templedonate': ((0x1194, 0xC258, {}), [('click', 0xA0, 0x4F)] + [('keys', [k]) for k in (0x02, 0x0B, 0x0B, 0x1C)]),
    'storebuy': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43)]),
    'storesell': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x5B)]),
    'storerepair': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x73)]),
    # Its first line clicked (marked as the list comes up, so picked):
    # "does not need any repairing"; with the items worn (`worn`), the
    # repair's price asked.
    'storerepairitem': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x73), ('click', 0x70, 0x26)]),
    'storerepairworn': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x73), ('click', 0x70, 0x26)]),
    # Its Steal (the box at 83h..9Ah), with a slot free: the lockpicking
    # roll from SEED fails, "You are unsuccessful...".
    'storesteal': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x8B)]),
    # From STEAL_SEED the roll succeeds: "You successfully stole a ...".
    'storestolen': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x8B)]),
    # The Buy with a slot free (`room`: the save's inventory is full).
    'storebuyroom': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43)]),
    # Its Weapon and Armor (boxes 18h high under BUY OPTIONS): the lists;
    # a weapon picked: its price haggled.
    'storeweapons': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43), ('click', 0xA0, 0x65)]),
    'storearmor': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43), ('click', 0xA0, 0x7D)]),
    'storeweapon': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43), ('click', 0xA0, 0x65), ('click', 0x70, 0x26)]),
    # The marked weapon clicked again: its price haggled.
    'storebuyhaggle': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43), ('click', 0xA0, 0x65), ('click', 0x70, 0x26),
                                              ('click', 0x70, 0x26)]),
    # A line of those lists clicked: the days asked for the room (its
    # typed field), the haggle over the item sold.
    'tavernroomdays': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x31)]),
    # A day typed (the keys' scan codes: 1, Enter): the room's price.
    'tavernroomprice': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x31),
                                               ('keys', [0x02, 0x1C])]),
    # The price accepted (the haggle's Yes, 1194:6001's first button at
    # 59h..88h, 7Fh..8Bh) and the deal confirmed (Yes again): the room
    # taken, the tavern's menu again without its rooms.
    'tavernroomtaken': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x31),
                                               ('keys', [0x02, 0x1C]), ('click', 0x70, 0x85),
                                               ('click', 0x70, 0x85)]),
    # R in the tavern with the room taken (`rented`): the hours to rest
    # asked (01E6: 1194:424E), its typed field.
    'tavernrest': (0x01E6, []),
    # Its Camp for a while (the box at 59h..70h): the hours asked.
    'tavernresthours': (0x01E6, [('click', 0xA0, 0x64)]),
    # Its second entry (Rest until healed, the box at 71h..88h) with the
    # player rested (`healed`): "You don't need to rest." (1194:4290) in a
    # box centred in the rest's clip (A91A 92h from 1194:4252: the one box
    # the rest shows inside it besides its menu and prompt; the rest's
    # other messages come in its frames, 1194:4635, after it is put back).
    'tavernrestneed': (0x01E6, [('click', 0xA0, 0x7C)]),
    # A frame of rest (1194:4635, as the frame body calls it at 0000:48C9
    # while 1013 is set: `resting`): the hours drawn (1194:4659, 0000:5364:
    # centred at A0h, 14h in [1026]'s font, colour 3Eh) and 20 minutes
    # passed. Counting down the hours typed (5 left); until healed (2
    # passed); its last step with the player then rested (`healed`): "You
    # are healed..." in its box over the hours (1194:4430).
    'restleft': ((0x1194, 0x4635, {}), []),
    'restpassed': ((0x1194, 0x4635, {}), []),
    'restrested': ((0x1194, 0x4635, {}), []),

    'storesellequipped': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x25)]),
    # The rumours' General (the box at 5Ch..6Eh): a rumour said in the
    # strip (1194:4E88, three lines to a page, "...More" between them).
    'tavernrumorgeneral': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x8B), ('click', 0xA0, 0x65)]),
    'tavernrumorheard': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x8B), ('click', 0xA0, 0x65)]),
    # Its Yes: the haggle.
    'storesellhaggle': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x25),
                                               ('click', 0xA0, 0x69)]),
    # In the street outside the guild (WHERE): R, camping (01E6), refused
    # in a town (over the street's colours); Shift+M, the world map (03BF:
    # 26F7's travel map).
    'streetcamp': (0x01E6, []),
    'worldmap': (0x03BF, []),
    # Its first province's region (DATA:6D90's first: High Rock) clicked:
    # the province's map, the name of the place nearest the mouse (left
    # where it clicked) over it; a place clicked: the journey's length
    # asked; its Search (the button at 34, 167): a name asked.
    'worldprovince': (0x03BF, [('click', 70, 45)]),
    'worldtrip': (0x03BF, [('click', 70, 45), ('click', 200, 100)]),
    'worldsearch': (0x03BF, [('click', 70, 45), ('click', 43, 175)]),
    # The province's Exit (the button at 72, 167): the world map again.
    'worldback': (0x03BF, [('click', 70, 45), ('click', 80, 180)]),
    # The search's name typed (d, a, g: a key a wait); a name none starts
    # (z, Enter): the province's places listed (1194:5734, kind 8); its
    # second line (Daggerfall) clicked: marked.
    'worldsearchtyped': (0x03BF, [('click', 70, 45), ('click', 43, 175)] + [('keys', [k]) for k in (0x20, 0x1E, 0x22)]),
    'worldsearchlist': (0x03BF, [('click', 70, 45), ('click', 43, 175)] + [('keys', [k]) for k in (0x2C, 0x1C)]),
    'worldsearchpick': (0x03BF, [('click', 70, 45), ('click', 43, 175)] + [('keys', [k]) for k in (0x2C, 0x1C)]
                        + [('click', 110, 46)]),
    # The marked line clicked again: the journey there asked.
    'worldsearchgo': (0x03BF, [('click', 70, 45), ('click', 43, 175)] + [('keys', [k]) for k in (0x2C, 0x1C)]
                      + [('click', 110, 46), ('click', 110, 46)]),
    # A townsperson clicked in the street (`beside`: the first person's
    # slot, CLICKED CB5E, the player on their square, within talking's two
    # squares), as the frame runs the click (1194:DB6C): what to ask
    # about, and each of its entries (boxes 18h high from 41h).
    'talk': ((0x1194, 0xDB6C, {}), []),
    'talkwho': ((0x1194, 0xDB6C, {}), [('click', 0xA0, 0x4D)]),
    'talkwhere': ((0x1194, 0xDB6C, {}), [('click', 0xA0, 0x65)]),
    'talkrumor': ((0x1194, 0xDB6C, {}), [('click', 0xA0, 0x7D)]),
    # Its Buy (the box at 3Ah..51h): the spells for sale.
    'guildbuy': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46)]),
    # Its kinds (boxes 18h high, PICK ITEM's title first): the potions,
    # the magic items, the spells for sale, each a list.
    'guildpotions': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46), ('click', 0xA0, 0x59)]),
    'guildmagic': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46), ('click', 0xA0, 0x71)]),
    'guildspells': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46), ('click', 0xA0, 0x88)]),
    # The magic items with a slot free (`room`).
    'guildmagicroom': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46), ('click', 0xA0, 0x71)]),
    # Their first lines clicked (the list's first row at 26h): what each
    # asks.
    'guildpotionpick': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46), ('click', 0xA0, 0x59), ('click', 0x70, 0x26)]),
    'guildspellpick': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46), ('click', 0xA0, 0x88), ('click', 0x70, 0x26)]),
    # Its Detect Magic (52h..69h): the items to look at.
    'guilddetect': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x5E)]),
    # R: camping (01E6: 1194:424E), refused in the guild: its box.
    'camp': (0x01E6, []),
    # With enemies near (`hunted`): "enemies near".
    'campenemies': (0x01E6, []),
    # M: the automap (045A: 0000:2CFE), whose loop asks the mouse by the
    # near entry (2DB8: 8FC8) rather than 8FC4.
    'automap': (0x045A, [], {(0, 0x8FC8)}),
    # Shift+M: the world map (03BF: 26F7's travel map).
    'travelmap': (0x03BF, []),
    # The loot window (1194:7B16, 2433:0583) as a click on a flat of
    # treasure at the player's square (0x071A in the guild) runs it: AH 0Fh
    # with a flat not the staff's (AL 1); with no pile there the piles are
    # made first (2433:0000, from LOOT_SEED: a bag of gold and a staff),
    # then listed (1194:5734). Its first line clicked: the gold taken, the
    # rest listed again.
    'loot': ((0x1194, 0x7B16, {'eax': 0x0F01, 'ebx': 0x071A, 'esi': 0}), []),
    'loottake': ((0x1194, 0x7B16, {'eax': 0x0F01, 'ebx': 0x071A, 'esi': 0}), [('click', 0x60, 0x27)]),
    # A level gained (2686:02D8, as the minute tick calls it with 05CA
    # bit 100h set, `leveled`): its notice (1194:1034's first box, 4F25),
    # then, the box clicked away, the attribute screen with the level's
    # points to spend (1194:14C5).
    'levelup': ((0x2686, 0x02D8, {}), []),
    'levelupstats': ((0x2686, 0x02D8, {}), [('click', 0xA0, 0x64)]),
    # The mages' guild's spellmaker (1194:6064, SI the player, as the
    # guild's menu calls it: the save is in Daggerfall's guild).
    'spellmaker': ((0x1194, 0x6064, {'esi': 0x01A4}), []),
    # A riddle (`riddled`: the save has none within reach) as the frame
    # shows the level's text (0000:0550): the text drawn, then its answer
    # asked under it (292B:05A2's typed field); then an answer typed in it
    # (the keys' scan codes: W, A, T, E, R), a key a wait (the editor
    # takes one a pass).
    'riddle': ((0, 0x0550, {}), []),
    'riddletyped': ((0, 0x0550, {}), [('keys', [k]) for k in (0x11, 0x1E, 0x14, 0x12, 0x13)]),
}
WAIT = (0, 0x8FC4)

# The buildings the screens not in the guild are in: the first of the
# town's list of their kind (its far pointer, its count), its door entered
# from the street (`ScreenArena.area_ended` out of the guild, then
# `enter`).
PLACES = {'tavern': (0x1AB5, 0x1387), 'temple': (0x1004, 0x1389), 'store': (0x1008, 0x138B), 'street': None,
          'people': None}
WHERE = {'tavern': 'tavern', 'temple': 'temple', 'store': 'store', 'streetcamp': 'street', 'worldmap': 'street',
         'worldprovince': 'street', 'worldtrip': 'street', 'worldsearch': 'street', 'worldback': 'street',
         'worldsearchtyped': 'street', 'worldsearchpick': 'street', 'worldsearchgo': 'street', 'worldsearchlist': 'street',
         'talk': 'people', 'talkwho': 'people', 'talkwhere': 'people', 'talkrumor': 'people'}
WHERE.update({n: next(k for k in ('tavern', 'temple', 'store') if n.startswith(k)) for n in
              ('taverndrinks', 'taverndrink', 'tavernroom', 'tavernrumor', 'templebless', 'templedonate', 'templecure', 'templeheal',
               'storebuy', 'storebuyroom', 'storeweapons', 'storearmor', 'storeweapon', 'storebuyhaggle', 'storesell', 'storerepair', 'storerepairitem', 'storerepairworn', 'storesteal', 'storestolen', 'tavernroomdays', 'storesellequipped', 'storesellhaggle',
               'tavernrumorgeneral', 'tavernrumorheard', 'tavernroomprice', 'tavernroomtaken',
               'tavernrest', 'tavernresthours', 'tavernrestneed')})


def place(a, base, where, cache={}):
    """The snapshot of the game in the building `where` (PLACES)."""
    if where not in cache:
        a.restore(base)
        if where == 'people':
            # The street with the guild left from SEED (`seeded`): the
            # townspeople its arrival places (0000:45B2, 1793) the same
            # on both sides, whatever the view's frames rolled before.
            seeded(a)
        assert a.area_ended() == ('world',), 'the guild not left'
        if PLACES[where] is not None:
            at, count = PLACES[where]
            assert a.r16(count), f'no {where} in the town'
            # An entry's square is row << 8 | column (1194:40CF); the
            # map's offset doubles the column.
            sq = a.r16(a.r16(at), seg=a.r16(at + 2))
            door = (sq & 0xFF00) | ((sq & 0xFF) << 1)
            assert a.enter(door) == ('world',), f'the {where} not entered'
        cache[where] = a.snapshot()
    return cache[where]


def square(a):
    """The player's column and row as the renderer sets them each frame
    (0000:C941..C958: C3D0, C3D4 shifted down 7, into CCA4, CCA6), which
    the automap marks: the state here is the load's, before any frame."""
    a.w16(0xCCA4, a.r16(0xC3D0) >> 7)
    a.w16(0xCCA6, a.r16(0xC3D4) >> 7)


# A seed both sides start the screen's routine from, where what it draws
# depends on the game's random numbers (the haggle's lines, 1194:83FD).
SEED = 0x2F6E_1A35


def seeded(a, seed=None):
    seed = SEED if seed is None else seed
    a.w16(0x91F6, seed & 0xFFFF)
    a.w16(0x91F8, seed >> 16)


# A seed the store's lockpicking roll (1194:FCC6) succeeds from.
STEAL_SEED = 0x2F6E_1A36


# The seed the piles at the square are made from (2433:0000) in `loot`.
LOOT_SEED = 1


def loot_seeded(a):
    a.w16(0x91F6, LOOT_SEED & 0xFFFF)
    a.w16(0x91F8, LOOT_SEED >> 16)


# The clock and the timer both sides start the rumours from (1194:E3F7:
# the seed from the ticks at 0151, what is heard by the parity of the
# clock's 05CC plus the player's x over 8), where the view's frames have
# moved them on: odd, "nothing"; even, what is heard of the quest told
# (these ticks: four lines, wrapped).
RUMOURS = ((0x0001_2345, 0x0456), (0x0001_2385, 0x043F))


def clocked(clock, ticks):
    def at(a):
        a.w16(0x05CC, clock & 0xFFFF)
        a.w16(0x05CE, clock >> 16)
        a.w16(0x0151, ticks)
    return at


# The townsperson talked to: the first actor slot (0AB3), placed by the
# street's arrival (0000:45B2: fifteen by day).
PERSON = 0x0AB3
# The timer (0151) as the talk starts: what they say of themselves is
# rolled from it (1194:E273). The view's turn into the talk moves it on
# by its tick (`Engine::tick`) from the street's 0.
TALK_TICKS = 1


def beside(a):
    """The player on the person's square (C3D0, C3D4 their x and y) and
    the person clicked (CLICKED CB5E their slot), as the hit test leaves
    it for the frame's click."""
    a.w16(0xC3D0, a.r16(PERSON))
    a.w16(0xC3D4, a.r16(PERSON + 4))
    a.w16(0xCB5E, PERSON)
    a.w16(0x0151, TALK_TICKS)


def rented(a):
    """A room taken as `tavernroomtaken` takes it, then the menu's Exit
    (its third box, at 7Bh..92h): the tavern left with the room the
    player's."""
    seeded(a)
    a.waits = {WAIT}
    a.script = SCREENS['tavernroomtaken'][1] + [('click', 0xA0, 0x87)]
    a.clicks = []
    a.mouse = (160, 100)
    a.buttons = 0
    assert a.run_screen(0x1194, 0xB93C, regs={}) is None, 'the tavern not left'
    assert not a.script


def healed(a):
    """The player rested as 1194:4345 tests it (2433's 4320): health
    (01FD) and spell points (020A) at their maximums (01FF, 020C), the
    fatigue (0201) well within its limit."""
    a.w16(0x01FD, a.r16(0x01FF))
    a.w16(0x020A, a.r16(0x020C))
    a.w16(0x0201, 0x7FFF)


def leveled(a):
    """The player's XP (05AD) the least past their next level's
    threshold (2686:0333's: from the class's first, DATA:47AE, each
    times 30/16 to the ninth level, 24/16 on), the level's check asked
    for (05CA bit 100h) and the seed fixed (`seeded`: the hit die's and
    the points' rolls)."""
    data = 0x3276
    from emu import LOAD_SEG
    cls = a.mu.mem_read(a.lin(a.ds, 0x01A9), 1)[0] & 0x1F
    base = a.r16(0x47AE + cls * 2, seg=LOAD_SEG + data)
    old = a.mu.mem_read(a.lin(a.ds, 0x01AA), 1)[0]

    # The thresholds as 2686:039E grows them: each the one before times
    # 30/16 up to the ninth level's (where its count stops), then 24/16.
    thresholds, t = [], base
    for lv in range(old + 1):
        thresholds.append(t)
        t = (t * (30 if lv < 8 else 24) & 0xFFFF_FFFF) >> 4
    lo = thresholds[old]
    a.w32(0x05AD, lo)
    a.w16(0x05CA, a.r16(0x05CA) | 0x100)
    seeded(a)


# The riddle `riddled` sets: its text, the answers after `:` (to a CR or
# NUL), the backquote's block after them, put at RIDDLE_AT in the level's
# texts' bank (C8FE) as the EMS frame (9030) maps it.
RIDDLE = b"What burns?\r:fire\r:flame\0`C\r"
RIDDLE_AT = 0xFF00


def riddled(a):
    """A riddle on the player's square as the square's lookup leaves it
    (0000:06BE): the text shown (3D42) and the riddle waiting (7C5E) the
    same, its answers (7C62), the texts after a wrong answer (7C64, the
    riddle again) and a right one (7C66, none), the triggers' numbers
    (7C68, 7C6A), the text's time (3C82) and square (0DBC, the player's
    CB90)."""
    from emu import LOAD_SEG
    bank = a.mu.mem_read(a.lin(a.ds, 0xC8FE), 1)[0]
    a.call_near(0, 0x5C8A, bx=bank)
    seg = a.r16(0x9030)
    a.mu.mem_write(a.lin(seg, RIDDLE_AT), RIDDLE)
    for at, v in ((0x3D42, RIDDLE_AT), (0x7C5E, RIDDLE_AT), (0x7C62, RIDDLE_AT + 12), (0x7C64, RIDDLE_AT),
                  (0x7C66, 0), (0x7C68, 1), (0x7C6A, 2), (0x0DBC, a.r16(0xCB90))):
        a.w16(at, v)
    a.mu.mem_write(a.lin(a.ds, 0x3C82), bytes([0x28]))


def resting(countdown, hours, step=0):
    """Resting as 1194:424E leaves it (1013 1, the fatigue's limit at
    1014) with HOURS (1016), COUNTDOWN (1018) and the hour's STEP (49E6),
    the seed fixed (`seeded`: the clock's 20 minutes roll encounters)."""
    def at(a):
        a.mu.mem_write(a.lin(a.ds, 0x1013), bytes([1]))
        a.w16(0x1014, (a.mu.mem_read(a.lin(a.ds, 0x01CD), 1)[0] + a.mu.mem_read(a.lin(a.ds, 0x01D2), 1)[0]) << 6)
        a.w16(0x1016, hours)
        a.mu.mem_write(a.lin(a.ds, 0x1018), bytes([countdown]))
        a.mu.mem_write(a.lin(a.ds, 0x49E6), bytes([step]))
        seeded(a)
    return at


def hurt(a):
    """The player's health (01FD) at half its maximum (01FF): not rested."""
    a.w16(0x01FD, a.r16(0x01FF) // 2)


def room(a):
    """A slot of the player's free (the last of 40, DGROUP 0212h on,
    13h each: its flags at +0Eh, 40h taken), as 1194:5EB5 looks for one."""
    at = 0x0212 + 39 * 0x13 + 0x0E
    a.mu.mem_write(a.lin(a.ds, at), bytes([a.mu.mem_read(a.lin(a.ds, at), 1)[0] & ~0x40 & 0xFF]))


def hunted(a):
    """Enemies near as the frame's scan of the actors leaves it (0FFB)."""
    a.w16(0x0FFB, 1)


def worn(a):
    """Each of the player's items (40 slots of 13h bytes from DGROUP 0212h,
    taken: +0Eh bit 40h) worn to half its condition: +6 of its maximum +8,
    and +3 of +8's low byte (an armour's of kind 0, 1194:C7DB)."""
    for i in range(40):
        at = 0x0212 + i * 0x13
        if a.mu.mem_read(a.lin(a.ds, at + 0x0E), 1)[0] & 0x40:
            top = a.r16(at + 8)
            a.w16(at + 6, top // 2)
            a.mu.mem_write(a.lin(a.ds, at + 3), bytes([(top & 0xFF) // 2]))


# What a screen's state needs first that a frame would have done.
BEFORE = {'automap': square, 'storesellhaggle': seeded, 'tavernrumorgeneral': clocked(*RUMOURS[0]),
          'tavernrumorheard': clocked(*RUMOURS[1]), 'tavernroomprice': seeded,
          'tavernroomtaken': seeded, 'tavernrest': rented, 'tavernresthours': rented,
          'tavernrestneed': lambda a: (rented(a), healed(a)),
          'talk': beside, 'talkwho': beside, 'talkwhere': beside, 'talkrumor': beside,
          'levelup': leveled, 'levelupstats': leveled,
          'campenemies': hunted,
          'restleft': resting(1, 5), 'restpassed': lambda a: (resting(0, 2)(a), hurt(a)),
          'restrested': lambda a: (resting(0, 1, 2)(a), healed(a)),
          'loot': loot_seeded, 'loottake': loot_seeded, 'riddle': riddled, 'riddletyped': riddled,
          'storebuyroom': room, 'guildmagicroom': room, 'storeweapons': room, 'storearmor': room,
          'storeweapon': lambda a: (room(a), seeded(a)), 'storebuyhaggle': lambda a: (room(a), seeded(a)), 'storerepairitem': seeded,
          'storerepairworn': lambda a: (worn(a), seeded(a)),
          'storesteal': lambda a: (room(a), seeded(a)),
          'storestolen': lambda a: (room(a), seeded(a, STEAL_SEED))}


# What a screen's routine is followed by that the next frame would do:
# the message line drawn (0000:0511: in [102C]'s font, centred at x A0h,
# y 14h, in colour 70h, then one pixel right in 9Fh).
AFTER = {n: lambda a: a.call_near(0, 0x0511) for n in ('usemessage', 'spellarea', 'spellself', 'spelllight')}


def draw(a, base, name, handler, script, fill, waits=None):
    a.restore(base)
    if name in BEFORE:
        BEFORE[name](a)
    a.mu.mem_write(a.lin(a.r16(PAGE_SEG_FIELD), 0), bytes([fill]) * 64000)
    a.mu.mem_write(0xA0000, bytes([fill]) * 64000)
    a.waits = waits or {WAIT}
    a.script = list(script)
    a.clicks = []
    # The mouse where the start left it (not in a snapshot: an earlier
    # screen's clicks moved it, and the automap draws its pen there).
    a.mouse = (160, 100)
    a.buttons = 0
    if isinstance(handler, tuple):
        seg, off, regs = handler
        a.run_screen(seg, off, regs=regs)
    else:
        a.run_screen(MENU, handler, near=True)
    if name in AFTER:
        AFTER[name](a)
    return a.page(), bytes(a.dac)


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    base = a.snapshot()
    out = open(OUT, 'w', buffering=1)
    fades = open(os.path.join(os.path.dirname(os.path.abspath(OUT)), 'screenfades.txt'), 'w', buffering=1)
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    for name, (handler, script, *waits) in SCREENS.items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        at = place(a, base, WHERE[name]) if name in WHERE else base
        # The colours at each wait for the retrace on the way (0000:6BB8,
        # 9B77): a fade, its colours set in parts.
        seen = []

        def sample(*_):
            if not seen or seen[-1] != bytes(a.dac):
                seen.append(bytes(a.dac))
        hooks = [a.mu.hook_add(UC_HOOK_CODE, sample, None, a.lin(LOAD_SEG, o), a.lin(LOAD_SEG, o)) for o in (0x6BB8, 0x9B77)]
        p0, dac = draw(a, at, name, handler, script, 0x00, *waits)
        for h in hooks:
            a.mu.hook_del(h)
        if len(seen) > 1:
            fades.write(f'screenfade | {name} | {len(seen)} | {fnv(b"".join(struct.pack("<Q", fnv(d)) for d in seen)):x}\n')
            if dump:
                os.makedirs(dump, exist_ok=True)
                open(os.path.join(dump, f'{name}.fade'), 'wb').write(b''.join(seen))
        p1, _ = draw(a, at, name, handler, script, 0xFF, *waits)
        mask = bytes(int(x == y) for x, y in zip(p0, p1))
        page = bytes(x if k else 0 for x, k in zip(p0, mask))
        digests = [fnv(page), fnv(dac)] + ([fnv(mask)] if 0 in mask else [])
        out.write(f'screen | {name} | {" ".join(f"{d:x}" for d in digests)}\n')
        if dump:
            os.makedirs(dump, exist_ok=True)
            for ext, data in (('page', page), ('mask', mask), ('dac', dac)):
                open(os.path.join(dump, f'{name}.{ext}'), 'wb').write(data)
    out.close()
    fades.close()


if __name__ == '__main__':
    main()
