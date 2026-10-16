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
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv
from screens import ScreenArena, PAGE_SEG_FIELD

# The menu keys' handlers in play (2542:0100's), run as a key runs them
# (near calls), each until it waits for input: the mouse asked for a
# third time from one place (0000:8FC4, which every input loop calls, or
# the screen's own wait). A script's clicks answer the waits before the
# last.
MENU = 0x2542
SCREENS = {
    # F1: the character sheet (00BE: 1194:0CF8), its first page.
    'sheet': (0x00BE, []),
    # Its "Next Page" button (6Ah..9Dh, B2h..C0h): the equipment page.
    'equipment': (0x00BE, [('click', 0x80, 0xB8)]),
    # S: the status box (037C: 04B1).
    'status': (0x037C, []),
    # L: the logbook (02A3: 27E3:0088), its first page.
    'logbook': (0x02A3, []),
    # C: the spellbook (026D: 056A).
    'spellbook': (0x026D, []),
    # U: the items to use (034D).
    'use': (0x034D, []),
    # Esc: the options screen (000A).
    'options': (0x000A, []),
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
    'storebuy': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x43)]),
    'storesell': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x5B)]),
    'storerepair': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x73)]),
    # A line of those lists clicked: the days asked for the room (its
    # typed field), the haggle over the item sold.
    'tavernroomdays': ((0x1194, 0xB93C, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x31)]),
    'storesellequipped': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x25)]),
    # Its Yes: the haggle.
    'storesellhaggle': ((0x1194, 0xF48E, {}), [('click', 0xA0, 0x5B), ('click', 0x80, 0x25),
                                               ('click', 0xA0, 0x69)]),
    # In the street outside the guild (WHERE): R, camping (01E6), refused
    # in a town (over the street's colours); Shift+M, the world map (03BF:
    # 26F7's travel map).
    'streetcamp': (0x01E6, []),
    'worldmap': (0x03BF, []),
    # Its Buy (the box at 3Ah..51h): the spells for sale.
    'guildbuy': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x46)]),
    # Its Detect Magic (52h..69h): the items to look at.
    'guilddetect': ((0x1194, 0xA73E, {}), [('click', 0xA0, 0x5E)]),
    # R: camping (01E6: 1194:424E), refused in the guild: its box.
    'camp': (0x01E6, []),
    # M: the automap (045A: 0000:2CFE), whose loop asks the mouse by the
    # near entry (2DB8: 8FC8) rather than 8FC4.
    'automap': (0x045A, [], {(0, 0x8FC8)}),
    # Shift+M: the world map (03BF: 26F7's travel map).
    'travelmap': (0x03BF, []),
    # The mages' guild's spellmaker (1194:6064, SI the player, as the
    # guild's menu calls it: the save is in Daggerfall's guild).
    'spellmaker': ((0x1194, 0x6064, {'esi': 0x01A4}), []),
}
WAIT = (0, 0x8FC4)

# The buildings the screens not in the guild are in: the first of the
# town's list of their kind (its far pointer, its count), its door entered
# from the street (`ScreenArena.area_ended` out of the guild, then
# `enter`).
PLACES = {'tavern': (0x1AB5, 0x1387), 'temple': (0x1004, 0x1389), 'store': (0x1008, 0x138B), 'street': None}
WHERE = {'tavern': 'tavern', 'temple': 'temple', 'store': 'store', 'streetcamp': 'street', 'worldmap': 'street'}
WHERE.update({n: next(k for k in ('tavern', 'temple', 'store') if n.startswith(k)) for n in
              ('taverndrinks', 'tavernroom', 'tavernrumor', 'templebless', 'templecure', 'templeheal',
               'storebuy', 'storesell', 'storerepair', 'tavernroomdays', 'storesellequipped', 'storesellhaggle')})


def place(a, base, where, cache={}):
    """The snapshot of the game in the building `where` (PLACES)."""
    if where not in cache:
        a.restore(base)
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


# What a screen's state needs first that a frame would have done.
BEFORE = {'automap': square}


def draw(a, base, name, handler, script, fill, waits=None):
    a.restore(base)
    if name in BEFORE:
        BEFORE[name](a)
    a.mu.mem_write(a.lin(a.r16(PAGE_SEG_FIELD), 0), bytes([fill]) * 64000)
    a.mu.mem_write(0xA0000, bytes([fill]) * 64000)
    a.waits = waits or {WAIT}
    a.script = list(script)
    a.clicks = []
    if isinstance(handler, tuple):
        seg, off, regs = handler
        a.run_screen(seg, off, regs=regs)
    else:
        a.run_screen(MENU, handler, near=True)
    return a.page(), bytes(a.dac)


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    base = a.snapshot()
    out = open(OUT, 'w', buffering=1)
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    for name, (handler, script, *waits) in SCREENS.items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        at = place(a, base, WHERE[name]) if name in WHERE else base
        p0, dac = draw(a, at, name, handler, script, 0x00, *waits)
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


if __name__ == '__main__':
    main()
