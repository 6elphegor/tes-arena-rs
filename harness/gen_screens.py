"""Emit the original's drawing of its 2D screens over the user's saved
game, for view/src/screentest.rs.

Usage: gen_screens.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR]

SAVE_DIR holds the saved game in slot 1 (the user's: only read). The
game is started and the save loaded from the title under unicorn
(harness/screens.py: 21EE:0000, then 25FD:0309 into the world loop),
then each screen of SCREENS opened by its own routine from that state
and run until it first waits for input (`waits`). It is run twice, the
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
        p0, dac = draw(a, base, name, handler, script, 0x00, *waits)
        p1, _ = draw(a, base, name, handler, script, 0xFF, *waits)
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
